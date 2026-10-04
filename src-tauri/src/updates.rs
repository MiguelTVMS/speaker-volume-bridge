use crate::distribution::{
    ApplicationArchitecture, DistributionEdition, InstalledDistribution, ReleaseChannel,
};
use async_trait::async_trait;
use semver::Version;
use serde::{Deserialize, Serialize};
use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::Emitter;
use tokio::sync::Mutex as AsyncMutex;
use url::Url;

pub const CATALOG_URL: &str = "https://svb.miguel.ms/updates/v1/catalog.json";
const MAX_CATALOG_BYTES: usize = 256 * 1024;
#[allow(clippy::duration_suboptimal_units)] // Keep compatibility with the repository's pinned Rust.
const SUCCESS_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdatePhase {
    #[default]
    Idle,
    Checking,
    UpToDate,
    UpdateAvailable,
    Unavailable,
    Unsupported,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    pub phase: UpdatePhase,
    pub installed_version: String,
    pub available_version: Option<String>,
    pub edition: String,
    pub last_successful_check: Option<u64>,
    pub action: Option<OpenUrlAction>,
    pub message: Option<String>,
    pub automatic_checks: bool,
    pub update_notifications: bool,
    pub prompt_dismissed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct OpenUrlAction {
    #[serde(rename = "type")]
    kind: String,
    pub url: String,
}

impl OpenUrlAction {
    fn validate(self) -> Result<Self, UpdateError> {
        if self.kind != "open_url" {
            return Err(UpdateError::InvalidCatalog("unsupported action"));
        }
        let parsed =
            Url::parse(&self.url).map_err(|_| UpdateError::InvalidCatalog("invalid action URL"))?;
        if parsed.scheme() != "https"
            || !matches!(
                parsed.host_str(),
                Some("svb.miguel.ms" | "github.com" | "apps.microsoft.com" | "apps.apple.com")
            )
        {
            return Err(UpdateError::InvalidCatalog("untrusted action URL"));
        }
        Ok(self)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdatePreferences {
    pub automatic_checks: bool,
    pub update_notifications: bool,
    pub last_successful_check: Option<u64>,
    pub last_attempted_check: Option<u64>,
    pub last_notified_target: Option<String>,
}

impl Default for UpdatePreferences {
    fn default() -> Self {
        Self {
            automatic_checks: false,
            update_notifications: true,
            last_successful_check: None,
            last_attempted_check: None,
            last_notified_target: None,
        }
    }
}

pub trait UpdatePersistence: Send + Sync {
    fn load(&self, automatic_default: bool) -> Result<UpdatePreferences, UpdateError>;
    fn save(&self, preferences: &UpdatePreferences) -> Result<(), UpdateError>;
}

pub struct FileUpdatePersistence(PathBuf);
impl FileUpdatePersistence {
    pub fn new(path: PathBuf) -> Self {
        Self(path)
    }
}
impl UpdatePersistence for FileUpdatePersistence {
    fn load(&self, automatic_default: bool) -> Result<UpdatePreferences, UpdateError> {
        if !self.0.exists() {
            return Ok(UpdatePreferences {
                automatic_checks: automatic_default,
                ..UpdatePreferences::default()
            });
        }
        Ok(serde_json::from_slice(&fs::read(&self.0)?)?)
    }
    fn save(&self, preferences: &UpdatePreferences) -> Result<(), UpdateError> {
        if let Some(parent) = self.0.parent() {
            fs::create_dir_all(parent)?;
        }
        let temporary = self.0.with_extension("json.tmp");
        fs::write(&temporary, serde_json::to_vec_pretty(preferences)?)?;
        fs::rename(temporary, &self.0)?;
        Ok(())
    }
}

#[async_trait]
pub trait CatalogTransport: Send + Sync {
    async fn fetch(&self) -> Result<Vec<u8>, UpdateError>;
}

pub struct HttpCatalogTransport(reqwest::Client);
impl HttpCatalogTransport {
    pub fn new() -> Result<Self, UpdateError> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::limited(3))
            .build()?;
        Ok(Self(client))
    }
}
#[async_trait]
impl CatalogTransport for HttpCatalogTransport {
    async fn fetch(&self) -> Result<Vec<u8>, UpdateError> {
        let response = self.0.get(CATALOG_URL).send().await?.error_for_status()?;
        if response
            .content_length()
            .is_some_and(|length| length > MAX_CATALOG_BYTES as u64)
        {
            return Err(UpdateError::InvalidCatalog("catalog too large"));
        }
        let bytes = response.bytes().await?;
        if bytes.len() > MAX_CATALOG_BYTES {
            return Err(UpdateError::InvalidCatalog("catalog too large"));
        }
        Ok(bytes.to_vec())
    }
}

pub trait UpdateClock: Send + Sync {
    fn now(&self) -> u64;
}
pub struct SystemUpdateClock;
impl UpdateClock for SystemUpdateClock {
    fn now(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |value| value.as_secs())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum UpdateError {
    #[error("update catalog is unavailable")]
    Network(#[from] reqwest::Error),
    #[error("update state I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("update state is invalid: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid update catalog: {0}")]
    InvalidCatalog(&'static str),
    #[error("update state is unavailable")]
    State,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Catalog {
    schema_version: u32,
    generated_at: String,
    entries: Vec<CatalogEntry>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CatalogEntry {
    edition: String,
    channel: String,
    os: String,
    architecture: String,
    version: String,
    published_at: String,
    release_notes: String,
    action: OpenUrlAction,
}

pub struct UpdateService {
    distribution: InstalledDistribution,
    transport: Arc<dyn CatalogTransport>,
    clock: Arc<dyn UpdateClock>,
    persistence: Arc<dyn UpdatePersistence>,
    preferences: Mutex<UpdatePreferences>,
    status: Mutex<UpdateStatus>,
    dismissed_target: Mutex<Option<String>>,
    opening: AtomicBool,
    in_flight: AsyncMutex<()>,
}

impl UpdateService {
    pub fn new(
        distribution: InstalledDistribution,
        transport: Arc<dyn CatalogTransport>,
        clock: Arc<dyn UpdateClock>,
        persistence: Arc<dyn UpdatePersistence>,
    ) -> Result<Self, UpdateError> {
        let preferences = persistence.load(distribution.check_supported)?;
        let phase = if distribution.check_supported {
            UpdatePhase::Idle
        } else {
            UpdatePhase::Unsupported
        };
        Ok(Self {
            status: Mutex::new(UpdateStatus {
                phase,
                installed_version: distribution.version.clone(),
                edition: edition_name(distribution.edition).into(),
                automatic_checks: preferences.automatic_checks,
                update_notifications: preferences.update_notifications,
                ..UpdateStatus::default()
            }),
            distribution,
            transport,
            clock,
            persistence,
            preferences: Mutex::new(preferences),
            dismissed_target: Mutex::new(None),
            opening: AtomicBool::new(false),
            in_flight: AsyncMutex::new(()),
        })
    }
    pub fn status(&self) -> Result<UpdateStatus, UpdateError> {
        let mut status = self.status.lock().map_err(|_| UpdateError::State)?.clone();
        status.automatic_checks = self.preferences()?.automatic_checks;
        status.update_notifications = self.preferences()?.update_notifications;
        status.prompt_dismissed = status.available_version.as_ref().is_some_and(|version| {
            self.dismissed_target
                .lock()
                .is_ok_and(|value| value.as_ref() == Some(version))
        });
        Ok(status)
    }
    pub fn preferences(&self) -> Result<UpdatePreferences, UpdateError> {
        self.preferences
            .lock()
            .map(|value| value.clone())
            .map_err(|_| UpdateError::State)
    }
    pub fn set_automatic_checks(&self, enabled: bool) -> Result<(), UpdateError> {
        let mut preferences = self.preferences.lock().map_err(|_| UpdateError::State)?;
        preferences.automatic_checks = enabled && self.distribution.check_supported;
        self.persistence.save(&preferences)
    }
    pub fn set_update_notifications(&self, enabled: bool) -> Result<(), UpdateError> {
        let mut preferences = self.preferences.lock().map_err(|_| UpdateError::State)?;
        preferences.update_notifications = enabled;
        self.persistence.save(&preferences)
    }
    pub fn dismiss(&self, version: &str) -> Result<(), UpdateError> {
        if self.status()?.available_version.as_deref() != Some(version) {
            return Err(UpdateError::State);
        }
        *self
            .dismissed_target
            .lock()
            .map_err(|_| UpdateError::State)? = Some(version.into());
        Ok(())
    }
    pub fn notification_needed(&self, status: &UpdateStatus) -> bool {
        let Some(version) = status.available_version.as_deref() else {
            return false;
        };
        let target = format!("{}:{version}", self.distribution.edition_key());
        self.preferences().is_ok_and(|preferences| {
            preferences.update_notifications
                && preferences.last_notified_target.as_deref() != Some(&target)
        })
    }
    pub fn mark_notified(&self, status: &UpdateStatus) -> Result<(), UpdateError> {
        let version = status
            .available_version
            .as_deref()
            .ok_or(UpdateError::State)?;
        let mut preferences = self.preferences.lock().map_err(|_| UpdateError::State)?;
        preferences.last_notified_target =
            Some(format!("{}:{version}", self.distribution.edition_key()));
        self.persistence.save(&preferences)
    }
    pub fn claim_offer(&self, version: &str, url: &str) -> Result<OpenUrlAction, UpdateError> {
        let status = self.status()?;
        let action = status.action.ok_or(UpdateError::State)?;
        if status.phase != UpdatePhase::UpdateAvailable
            || status.available_version.as_deref() != Some(version)
            || action.url != url
        {
            return Err(UpdateError::State);
        }
        let action = action.validate()?;
        if self.opening.swap(true, Ordering::SeqCst) {
            return Err(UpdateError::State);
        }
        Ok(action)
    }
    pub fn finish_open(&self) {
        self.opening.store(false, Ordering::SeqCst);
    }
    pub fn automatic_due(&self) -> bool {
        let Ok(preferences) = self.preferences.lock() else {
            return false;
        };
        let now = self.clock.now();
        preferences.automatic_checks
            && preferences
                .last_attempted_check
                .is_none_or(|last| now.saturating_sub(last) >= 60)
            && preferences
                .last_successful_check
                .is_none_or(|last| now.saturating_sub(last) >= SUCCESS_INTERVAL.as_secs())
    }
    pub async fn check(&self, manual: bool) -> UpdateStatus {
        if !self.distribution.check_supported {
            return self.status().unwrap_or_default();
        }
        let observed_attempt = self
            .preferences()
            .ok()
            .and_then(|value| value.last_attempted_check);
        let _guard = self.in_flight.lock().await;
        if self
            .preferences()
            .ok()
            .and_then(|value| value.last_attempted_check)
            != observed_attempt
        {
            return self.status().unwrap_or_default();
        }
        if !manual && !self.automatic_due() {
            return self.status().unwrap_or_default();
        }
        let now = self.clock.now();
        if let Ok(mut preferences) = self.preferences.lock() {
            preferences.last_attempted_check = Some(now);
            let _ = self.persistence.save(&preferences);
        }
        self.replace_status(UpdateStatus {
            phase: UpdatePhase::Checking,
            installed_version: self.distribution.version.clone(),
            edition: edition_name(self.distribution.edition).into(),
            last_successful_check: self
                .preferences()
                .ok()
                .and_then(|value| value.last_successful_check),
            ..UpdateStatus::default()
        });
        let result = self
            .transport
            .fetch()
            .await
            .and_then(|bytes| self.evaluate(&bytes));
        let status = match result {
            Ok(mut status) => {
                if let Ok(mut preferences) = self.preferences.lock() {
                    preferences.last_successful_check = Some(now);
                    let _ = self.persistence.save(&preferences);
                    status.last_successful_check = Some(now);
                }
                status
            }
            Err(error) => UpdateStatus {
                phase: UpdatePhase::Unavailable,
                installed_version: self.distribution.version.clone(),
                edition: edition_name(self.distribution.edition).into(),
                last_successful_check: self
                    .preferences()
                    .ok()
                    .and_then(|value| value.last_successful_check),
                message: manual.then(|| error.to_string()),
                ..UpdateStatus::default()
            },
        };
        self.replace_status(status.clone());
        self.status().unwrap_or(status)
    }
    fn replace_status(&self, status: UpdateStatus) {
        if let Ok(mut current) = self.status.lock() {
            *current = status;
        }
    }
    fn evaluate(&self, bytes: &[u8]) -> Result<UpdateStatus, UpdateError> {
        if bytes.len() > MAX_CATALOG_BYTES {
            return Err(UpdateError::InvalidCatalog("catalog too large"));
        }
        let catalog: Catalog = serde_json::from_slice(bytes)?;
        if catalog.schema_version != 1 || catalog.generated_at.parse::<jiff::Timestamp>().is_err() {
            return Err(UpdateError::InvalidCatalog("unsupported schema"));
        }
        let mut targets = std::collections::HashSet::new();
        for entry in &catalog.entries {
            if !targets.insert((
                &entry.edition,
                &entry.channel,
                &entry.os,
                &entry.architecture,
            )) {
                return Err(UpdateError::InvalidCatalog("duplicate target"));
            }
            let version = Version::parse(&entry.version)
                .map_err(|_| UpdateError::InvalidCatalog("invalid semantic version"))?;
            if entry.channel != "stable" || !version.pre.is_empty() {
                return Err(UpdateError::InvalidCatalog("invalid stable version"));
            }
            entry
                .published_at
                .parse::<jiff::Timestamp>()
                .map_err(|_| UpdateError::InvalidCatalog("invalid publication time"))?;
            if entry.release_notes.is_empty() || entry.release_notes.len() > 16 * 1024 {
                return Err(UpdateError::InvalidCatalog("invalid release notes"));
            }
            entry.action.clone().validate()?;
        }
        let target = target_key(&self.distribution);
        let mut matching = catalog.entries.into_iter().filter(|entry| {
            (
                entry.edition.as_str(),
                entry.channel.as_str(),
                entry.os.as_str(),
                entry.architecture.as_str(),
            ) == target
        });
        let Some(entry) = matching.next() else {
            return Ok(self.base_status(UpdatePhase::Unavailable));
        };
        if matching.next().is_some() {
            return Err(UpdateError::InvalidCatalog("duplicate target"));
        }
        let available = Version::parse(&entry.version)
            .map_err(|_| UpdateError::InvalidCatalog("invalid semantic version"))?;
        let installed = Version::parse(&self.distribution.version)
            .map_err(|_| UpdateError::InvalidCatalog("invalid installed version"))?;
        if available <= installed {
            return Ok(self.base_status(UpdatePhase::UpToDate));
        }
        let action = entry.action.validate()?;
        let mut status = self.base_status(UpdatePhase::UpdateAvailable);
        status.available_version = Some(available.to_string());
        status.action = Some(action);
        Ok(status)
    }
    fn base_status(&self, phase: UpdatePhase) -> UpdateStatus {
        UpdateStatus {
            phase,
            installed_version: self.distribution.version.clone(),
            edition: edition_name(self.distribution.edition).into(),
            ..UpdateStatus::default()
        }
    }
}

fn target_key(
    distribution: &InstalledDistribution,
) -> (&'static str, &'static str, &'static str, &'static str) {
    let edition = match distribution.edition {
        DistributionEdition::DirectMacos => "direct_macos",
        DistributionEdition::DirectWindows => "direct_windows",
        DistributionEdition::MicrosoftStore => "microsoft_store",
        DistributionEdition::MacAppStore => "mac_app_store",
        DistributionEdition::Debian => "debian",
        _ => "unknown",
    };
    let os = match distribution.edition {
        DistributionEdition::DirectMacos | DistributionEdition::MacAppStore => "macos",
        DistributionEdition::DirectWindows | DistributionEdition::MicrosoftStore => "windows",
        DistributionEdition::Debian => "linux",
        _ => "unknown",
    };
    let architecture = match distribution.architecture {
        ApplicationArchitecture::Aarch64 => "aarch64",
        ApplicationArchitecture::X86_64 => "x86_64",
        ApplicationArchitecture::Unknown => "unknown",
    };
    let channel = match distribution.channel {
        ReleaseChannel::Stable => "stable",
    };
    (edition, channel, os, architecture)
}

const fn edition_name(edition: DistributionEdition) -> &'static str {
    match edition {
        DistributionEdition::DirectMacos => "direct_macos",
        DistributionEdition::DirectWindows => "direct_windows",
        DistributionEdition::MicrosoftStore => "microsoft_store",
        DistributionEdition::MacAppStore => "mac_app_store",
        DistributionEdition::Debian => "debian",
        DistributionEdition::WindowsSideload => "windows_sideload",
        DistributionEdition::Development => "development",
        DistributionEdition::Unknown => "unknown",
    }
}

pub struct UpdateManager {
    service: Arc<UpdateService>,
    task: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
}
impl UpdateManager {
    pub fn new(service: Arc<UpdateService>) -> Self {
        Self {
            service,
            task: Mutex::new(None),
        }
    }
    pub fn service(&self) -> &Arc<UpdateService> {
        &self.service
    }
    pub fn start(&self, app: &tauri::AppHandle) {
        let Ok(mut task) = self.task.lock() else {
            return;
        };
        if task.is_some() {
            return;
        }
        let service = Arc::clone(&self.service);
        let app_handle = app.clone();
        *task = Some(tauri::async_runtime::spawn(async move {
            tokio::time::sleep(Duration::from_secs(30)).await;
            #[allow(clippy::duration_suboptimal_units)] // Keep compatibility with pinned Rust.
            let backoff = [
                Duration::from_secs(60),
                Duration::from_secs(5 * 60),
                Duration::from_secs(30 * 60),
            ];
            loop {
                if service.automatic_due() {
                    let status = service.check(false).await;
                    notify_available(&app_handle, &service, &status).await;
                    if status.phase == UpdatePhase::Unavailable {
                        for delay in backoff {
                            tokio::time::sleep(delay).await;
                            let status = service.check(false).await;
                            notify_available(&app_handle, &service, &status).await;
                            if status.phase != UpdatePhase::Unavailable {
                                break;
                            }
                        }
                    }
                }
                #[allow(clippy::duration_suboptimal_units)]
                tokio::time::sleep(Duration::from_secs(60 * 60)).await;
            }
        }));
    }
}

async fn notify_available(app: &tauri::AppHandle, service: &UpdateService, status: &UpdateStatus) {
    let _ = app.emit("update-status-changed", status);
    if status.phase != UpdatePhase::UpdateAvailable
        || !service.notification_needed(status)
        || !crate::schedule_notifications::permitted(app, false).await
    {
        return;
    }
    let version = status.available_version.as_deref().unwrap_or("new");
    crate::schedule_notifications::send(
        app,
        "Speaker Volume Bridge update available",
        &format!("Version {version} is available. Open Settings to view the update page."),
    )
    .await;
    let _ = service.mark_notified(status);
    let _ = app.emit("open-updates", ());
}
impl Drop for UpdateManager {
    fn drop(&mut self) {
        if let Ok(mut task) = self.task.lock()
            && let Some(task) = task.take()
        {
            task.abort();
        }
    }
}

pub fn state_path(config_path: &Path) -> PathBuf {
    config_path.with_file_name("updates.json")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

    struct FakeClock(AtomicU64);
    impl UpdateClock for FakeClock {
        fn now(&self) -> u64 {
            self.0.load(Ordering::Relaxed)
        }
    }
    struct FakeTransport {
        result: Mutex<Result<Vec<u8>, &'static str>>,
        calls: AtomicUsize,
    }
    #[async_trait]
    impl CatalogTransport for FakeTransport {
        async fn fetch(&self) -> Result<Vec<u8>, UpdateError> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            self.result
                .lock()
                .unwrap()
                .clone()
                .map_err(UpdateError::InvalidCatalog)
        }
    }
    #[derive(Default)]
    struct MemoryPersistence(Mutex<Option<UpdatePreferences>>);
    impl UpdatePersistence for MemoryPersistence {
        fn load(&self, default: bool) -> Result<UpdatePreferences, UpdateError> {
            Ok(self.0.lock().unwrap().clone().unwrap_or(UpdatePreferences {
                automatic_checks: default,
                ..UpdatePreferences::default()
            }))
        }
        fn save(&self, value: &UpdatePreferences) -> Result<(), UpdateError> {
            *self.0.lock().unwrap() = Some(value.clone());
            Ok(())
        }
    }
    fn distribution() -> InstalledDistribution {
        InstalledDistribution {
            edition: DistributionEdition::DirectMacos,
            version: "1.7.1".into(),
            architecture: ApplicationArchitecture::Aarch64,
            channel: ReleaseChannel::Stable,
            check_supported: true,
            direct_install_supported: false,
        }
    }
    fn catalog(version: &str) -> Vec<u8> {
        format!(r#"{{"schemaVersion":1,"generatedAt":"2026-10-04T00:00:00Z","entries":[{{"edition":"direct_macos","channel":"stable","os":"macos","architecture":"aarch64","version":"{version}","publishedAt":"2026-10-04T00:00:00Z","releaseNotes":"Notes","action":{{"type":"open_url","url":"https://svb.miguel.ms/guide/Upgrading.html"}}}}]}}"#).into_bytes()
    }
    fn service(
        bytes: Vec<u8>,
        clock: Arc<FakeClock>,
        persistence: Arc<MemoryPersistence>,
    ) -> (Arc<UpdateService>, Arc<FakeTransport>) {
        let transport = Arc::new(FakeTransport {
            result: Mutex::new(Ok(bytes)),
            calls: AtomicUsize::new(0),
        });
        (
            Arc::new(
                UpdateService::new(distribution(), transport.clone(), clock, persistence).unwrap(),
            ),
            transport,
        )
    }

    #[tokio::test]
    async fn available_current_missing_malformed_and_prerelease_are_distinct() {
        for (bytes, expected) in [
            (catalog("2.0.0"), UpdatePhase::UpdateAvailable),
            (catalog("1.7.1"), UpdatePhase::UpToDate),
            (catalog("1.0.0"), UpdatePhase::UpToDate),
            (
                br#"{"schemaVersion":1,"generatedAt":"x","entries":[]}"#.to_vec(),
                UpdatePhase::Unavailable,
            ),
            (b"{".to_vec(), UpdatePhase::Unavailable),
            (catalog("2.0.0-beta.1"), UpdatePhase::Unavailable),
        ] {
            let (service, _) = service(
                bytes,
                Arc::new(FakeClock(AtomicU64::new(100))),
                Arc::default(),
            );
            assert_eq!(service.check(true).await.phase, expected);
        }
    }
    #[tokio::test]
    async fn successful_checks_persist_interval_and_restart_state() {
        let clock = Arc::new(FakeClock(AtomicU64::new(100)));
        let persistence = Arc::new(MemoryPersistence::default());
        let (initial, transport) = service(catalog("1.7.1"), clock.clone(), persistence.clone());
        assert_eq!(initial.check(false).await.phase, UpdatePhase::UpToDate);
        assert!(!initial.automatic_due());
        assert_eq!(transport.calls.load(Ordering::Relaxed), 1);
        let (restarted, _) = service(catalog("1.7.1"), clock.clone(), persistence);
        assert!(!restarted.automatic_due());
        clock
            .0
            .store(100 + SUCCESS_INTERVAL.as_secs(), Ordering::Relaxed);
        assert!(restarted.automatic_due());
    }
    #[tokio::test]
    async fn concurrent_manual_and_background_checks_share_one_request() {
        let (service, transport) = service(
            catalog("2.0.0"),
            Arc::new(FakeClock(AtomicU64::new(100))),
            Arc::default(),
        );
        let (left, right) = tokio::join!(service.check(true), service.check(false));
        assert_eq!(left.phase, UpdatePhase::UpdateAvailable);
        assert_eq!(right.phase, UpdatePhase::UpdateAvailable);
        assert_eq!(transport.calls.load(Ordering::Relaxed), 1);
    }
    #[tokio::test]
    async fn invalid_target_never_reaches_persisted_status() {
        let bytes = String::from_utf8(catalog("2.0.0"))
            .unwrap()
            .replace("https://svb.miguel.ms/guide/Upgrading.html", "file:///tmp/run")
            .into_bytes();
        let (active, _) = service(
            bytes,
            Arc::new(FakeClock(AtomicU64::new(100))),
            Arc::default(),
        );
        let status = active.check(true).await;
        assert_eq!(status.phase, UpdatePhase::Unavailable);
        assert!(status.action.is_none());
    }

    #[tokio::test]
    async fn background_failures_are_quiet_and_manual_failures_are_retryable() {
        let clock = Arc::new(FakeClock(AtomicU64::new(100)));
        let persistence = Arc::new(MemoryPersistence::default());
        let (service, transport) = service(catalog("2.0.0"), clock.clone(), persistence);
        *transport.result.lock().unwrap() = Err("offline");
        let background = service.check(false).await;
        assert_eq!(background.phase, UpdatePhase::Unavailable);
        assert!(background.message.is_none());
        clock.0.store(101, Ordering::Relaxed);
        let manual = service.check(true).await;
        assert_eq!(
            manual.message.as_deref(),
            Some("invalid update catalog: offline")
        );
        *transport.result.lock().unwrap() = Ok(catalog("2.0.0"));
        clock.0.store(102, Ordering::Relaxed);
        assert_eq!(
            service.check(true).await.phase,
            UpdatePhase::UpdateAvailable
        );
    }

    #[tokio::test]
    async fn notification_later_and_open_actions_are_bound_to_the_current_offer() {
        let persistence = Arc::new(MemoryPersistence::default());
        let (active, _) = service(
            catalog("2.0.0"),
            Arc::new(FakeClock(AtomicU64::new(100))),
            persistence.clone(),
        );
        let status = active.check(true).await;
        active.set_update_notifications(false).unwrap();
        assert!(!active.notification_needed(&status));
        active.set_update_notifications(true).unwrap();
        assert!(active.notification_needed(&status));
        active.mark_notified(&status).unwrap();
        assert!(!active.notification_needed(&status));
        let (restarted, _) = service(
            catalog("2.0.0"),
            Arc::new(FakeClock(AtomicU64::new(101))),
            persistence,
        );
        let restarted_status = restarted.check(true).await;
        assert!(!restarted.notification_needed(&restarted_status));
        active.dismiss("2.0.0").unwrap();
        assert!(active.status().unwrap().prompt_dismissed);
        assert!(active.dismiss("2.0.1").is_err());
        assert!(
            active
                .claim_offer("1.9.0", "https://svb.miguel.ms/guide/Upgrading.html")
                .is_err()
        );
        assert!(
            active
                .claim_offer("2.0.0", "https://svb.miguel.ms/guide/Upgrading.html")
                .is_ok()
        );
        assert!(
            active
                .claim_offer("2.0.0", "https://svb.miguel.ms/guide/Upgrading.html")
                .is_err()
        );
        active.finish_open();
        assert!(
            active
                .claim_offer("2.0.0", "https://svb.miguel.ms/guide/Upgrading.html")
                .is_ok()
        );
    }

    #[test]
    fn existing_preferences_default_update_notifications_on() {
        let preferences: UpdatePreferences = serde_json::from_str("{}").unwrap();
        assert!(preferences.update_notifications);
    }
}
