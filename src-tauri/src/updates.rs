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
#[allow(clippy::struct_excessive_bools)] // Status transports independent persisted preferences and offer freshness.
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
    pub offer_stale: bool,
    pub check_failed: bool,
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
#[serde(default, rename_all = "camelCase")]
pub struct UpdatePreferences {
    pub automatic_checks: bool,
    pub update_notifications: bool,
    pub last_successful_check: Option<u64>,
    pub last_attempted_check: Option<u64>,
    pub last_notified_target: Option<String>,
    pub cached_offer: Option<CachedOffer>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CachedOffer {
    pub version: String,
    pub action: OpenUrlAction,
    #[serde(default)]
    pub target: Option<CachedTarget>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CachedTarget {
    pub edition: String,
    pub channel: String,
    pub os: String,
    pub architecture: String,
}

impl Default for UpdatePreferences {
    fn default() -> Self {
        Self {
            automatic_checks: false,
            update_notifications: true,
            last_successful_check: None,
            last_attempted_check: None,
            last_notified_target: None,
            cached_offer: None,
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
        let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&self.0)?)?;
        let cached_offer = value
            .as_object_mut()
            .and_then(|object| object.remove("cachedOffer"));
        let mut preferences: UpdatePreferences = serde_json::from_value(value)?;
        preferences.cached_offer =
            cached_offer.and_then(|offer| serde_json::from_value(offer).ok());
        Ok(preferences)
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
#[serde(rename_all = "camelCase")]
struct Catalog {
    schema_version: u32,
    generated_at: String,
    entries: Vec<CatalogEntry>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
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
        let mut preferences = persistence.load(distribution.check_supported)?;
        if let Some(offer) = preferences.cached_offer.as_ref()
            && !cached_offer_valid(offer, &distribution)
        {
            preferences.cached_offer = None;
            persistence.save(&preferences)?;
        }
        let phase = if !distribution.check_supported {
            UpdatePhase::Unsupported
        } else if preferences.cached_offer.is_some() {
            UpdatePhase::UpdateAvailable
        } else {
            UpdatePhase::Idle
        };
        Ok(Self {
            status: Mutex::new(UpdateStatus {
                phase,
                installed_version: distribution.version.clone(),
                edition: edition_name(distribution.edition).into(),
                available_version: preferences
                    .cached_offer
                    .as_ref()
                    .map(|offer| offer.version.clone()),
                action: preferences
                    .cached_offer
                    .as_ref()
                    .map(|offer| offer.action.clone()),
                last_successful_check: preferences.last_successful_check,
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
        status.offer_stale = status.available_version.is_some()
            && status.last_successful_check.is_none_or(|last| {
                self.clock.now().saturating_sub(last) >= SUCCESS_INTERVAL.as_secs()
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
    #[cfg(test)]
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
    #[cfg(test)]
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
    #[allow(dead_code)] // Used in release builds; test delivery deliberately suppresses native notifications.
    pub fn claim_notification(&self, status: &UpdateStatus) -> Result<bool, UpdateError> {
        if status.phase != UpdatePhase::UpdateAvailable || status.offer_stale {
            return Ok(false);
        }
        let version = status
            .available_version
            .as_deref()
            .ok_or(UpdateError::State)?;
        let target = format!("{}:{version}", self.distribution.edition_key());
        let mut preferences = self.preferences.lock().map_err(|_| UpdateError::State)?;
        if !preferences.update_notifications
            || preferences.last_notified_target.as_deref() == Some(&target)
        {
            return Ok(false);
        }
        preferences.last_notified_target = Some(target);
        self.persistence.save(&preferences)?;
        Ok(true)
    }
    pub fn claim_offer(&self, version: &str, url: &str) -> Result<OpenUrlAction, UpdateError> {
        let status = self.status()?;
        let action = status.action.ok_or(UpdateError::State)?;
        if status.phase != UpdatePhase::UpdateAvailable
            || status.offer_stale
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
        let mut checking = self.status().unwrap_or_default();
        checking.phase = if checking.available_version.is_some() {
            UpdatePhase::UpdateAvailable
        } else {
            UpdatePhase::Checking
        };
        checking.message = None;
        self.replace_status(checking);
        let result = self
            .transport
            .fetch()
            .await
            .and_then(|bytes| self.evaluate(&bytes));
        let status = match result {
            Ok(mut status) => {
                if let Ok(mut preferences) = self.preferences.lock() {
                    preferences.last_successful_check = Some(now);
                    preferences.cached_offer = status
                        .available_version
                        .as_ref()
                        .zip(status.action.as_ref())
                        .map(|(version, action)| CachedOffer {
                            version: version.clone(),
                            action: action.clone(),
                            target: Some(cached_target(&self.distribution)),
                        });
                    let _ = self.persistence.save(&preferences);
                    status.last_successful_check = Some(now);
                }
                status
            }
            Err(error) => {
                let mut cached = self.status().unwrap_or_default();
                if cached.phase != UpdatePhase::UpdateAvailable {
                    cached.phase = UpdatePhase::Unavailable;
                }
                cached.check_failed = true;
                cached.message = manual.then(|| error.to_string());
                cached
            }
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
            let expected_os = match entry.edition.as_str() {
                "direct_macos" | "mac_app_store" => "macos",
                "direct_windows" | "microsoft_store" => "windows",
                "debian" => "linux",
                _ => return Err(UpdateError::InvalidCatalog("unsupported edition")),
            };
            if entry.os != expected_os {
                return Err(UpdateError::InvalidCatalog("unsupported platform"));
            }
            if !matches!(entry.architecture.as_str(), "aarch64" | "x86_64") {
                return Err(UpdateError::InvalidCatalog("unsupported architecture"));
            }
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

fn cached_target(distribution: &InstalledDistribution) -> CachedTarget {
    let (edition, channel, os, architecture) = target_key(distribution);
    CachedTarget {
        edition: edition.into(),
        channel: channel.into(),
        os: os.into(),
        architecture: architecture.into(),
    }
}

fn cached_offer_valid(offer: &CachedOffer, distribution: &InstalledDistribution) -> bool {
    let Some(target) = offer.target.as_ref() else {
        return false;
    };
    let Ok(installed) = Version::parse(&distribution.version) else {
        return false;
    };
    let Ok(available) = Version::parse(&offer.version) else {
        return false;
    };
    available > installed
        && (
            target.edition.as_str(),
            target.channel.as_str(),
            target.os.as_str(),
            target.architecture.as_str(),
        ) == target_key(distribution)
        && offer.action.clone().validate().is_ok()
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
    pub async fn check_and_deliver(&self, app: &tauri::AppHandle, manual: bool) -> UpdateStatus {
        run_check_and_deliver(app, &self.service, manual).await
    }
    pub async fn wake_service<R: tauri::Runtime>(
        app: &tauri::AppHandle<R>,
        service: Arc<UpdateService>,
    ) {
        if service.automatic_due() {
            run_check_and_deliver(app, &service, false).await;
        }
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
                    let status = run_check_and_deliver(&app_handle, &service, false).await;
                    if status.check_failed {
                        for delay in backoff {
                            tokio::time::sleep(delay).await;
                            let status = run_check_and_deliver(&app_handle, &service, false).await;
                            if !status.check_failed {
                                break;
                            }
                        }
                    }
                }
                #[allow(clippy::duration_suboptimal_units)]
                tokio::time::sleep(Duration::from_secs(60)).await;
            }
        }));
    }
}

async fn run_check_and_deliver<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    service: &UpdateService,
    manual: bool,
) -> UpdateStatus {
    #[cfg(test)]
    let permitted = || async { true };
    #[cfg(not(test))]
    let permitted = {
        let app = app.clone();
        move || async move { crate::schedule_notifications::permitted(&app, false).await }
    };
    #[cfg(test)]
    let deliver = |_| async {};
    #[cfg(not(test))]
    let deliver = {
        let app = app.clone();
        move |status: UpdateStatus| async move {
            let version = status.available_version.as_deref().unwrap_or("new");
            crate::schedule_notifications::send_update(
                &app,
                "Speaker Volume Bridge update available",
                &format!("Version {version} is available. Open Settings to view the update page."),
            )
            .await;
        }
    };
    run_check_and_deliver_with(app, service, manual, permitted, deliver).await
}

async fn run_check_and_deliver_with<R, P, PFut, F, Fut>(
    app: &tauri::AppHandle<R>,
    service: &UpdateService,
    manual: bool,
    permitted: P,
    deliver: F,
) -> UpdateStatus
where
    R: tauri::Runtime,
    P: FnOnce() -> PFut,
    PFut: std::future::Future<Output = bool>,
    F: FnOnce(UpdateStatus) -> Fut,
    Fut: std::future::Future<Output = ()>,
{
    let status = service.check(manual).await;
    let _ = app.emit("update-status-changed", &status);
    if status.phase == UpdatePhase::UpdateAvailable
        && permitted().await
        && service.claim_notification(&status).unwrap_or(false)
    {
        deliver(status.clone()).await;
    }
    status
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
    use tauri::Manager;

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
    async fn cached_offer_and_success_timestamp_are_discoverable_after_restart() {
        let clock = Arc::new(FakeClock(AtomicU64::new(100)));
        let persistence = Arc::new(MemoryPersistence::default());
        let (initial, _) = service(catalog("2.0.0"), clock.clone(), persistence.clone());
        let first = initial.check(true).await;
        assert_eq!(first.phase, UpdatePhase::UpdateAvailable);
        let restarted = UpdateService::new(
            distribution(),
            Arc::new(FakeTransport {
                result: Mutex::new(Err("offline")),
                calls: AtomicUsize::new(0),
            }),
            clock,
            persistence,
        )
        .unwrap();
        let restored = restarted.status().unwrap();
        assert_eq!(restored.phase, UpdatePhase::UpdateAvailable);
        assert_eq!(restored.available_version.as_deref(), Some("2.0.0"));
        assert_eq!(restored.action, first.action);
        assert_eq!(restored.last_successful_check, Some(100));
    }

    fn cached_offer(version: &str, target: Option<CachedTarget>, url: &str) -> CachedOffer {
        CachedOffer {
            version: version.into(),
            action: OpenUrlAction {
                kind: "open_url".into(),
                url: url.into(),
            },
            target,
        }
    }

    fn this_target() -> CachedTarget {
        cached_target(&distribution())
    }

    #[tokio::test]
    async fn startup_revalidates_cached_offer_after_manual_upgrade_and_distribution_change() {
        let clock = Arc::new(FakeClock(AtomicU64::new(100)));
        for offer in [
            cached_offer(
                "1.7.1",
                Some(this_target()),
                "https://svb.miguel.ms/guide/Upgrading.html",
            ),
            cached_offer(
                "1.7.0",
                Some(this_target()),
                "https://svb.miguel.ms/guide/Upgrading.html",
            ),
            cached_offer(
                "not-semver",
                Some(this_target()),
                "https://svb.miguel.ms/guide/Upgrading.html",
            ),
            cached_offer(
                "2.0.0",
                Some(CachedTarget {
                    edition: "debian".into(),
                    channel: "stable".into(),
                    os: "linux".into(),
                    architecture: "x86_64".into(),
                }),
                "https://svb.miguel.ms/guide/Upgrading.html",
            ),
            cached_offer("2.0.0", None, "https://svb.miguel.ms/guide/Upgrading.html"),
            cached_offer("2.0.0", Some(this_target()), "https://evil.example/upgrade"),
        ] {
            let persistence = Arc::new(MemoryPersistence::default());
            *persistence.0.lock().unwrap() = Some(UpdatePreferences {
                automatic_checks: true,
                last_successful_check: Some(100),
                cached_offer: Some(offer),
                ..UpdatePreferences::default()
            });
            let (service, transport) =
                service(catalog("2.0.0"), clock.clone(), persistence.clone());
            let status = service.status().unwrap();
            assert!(status.available_version.is_none());
            assert_eq!(status.last_successful_check, Some(100));
            assert_eq!(transport.calls.load(Ordering::Relaxed), 0);
            assert!(!service.automatic_due());
            assert!(
                persistence
                    .0
                    .lock()
                    .unwrap()
                    .as_ref()
                    .unwrap()
                    .cached_offer
                    .is_none()
            );
        }
    }

    #[tokio::test]
    async fn startup_preserves_valid_newer_cache_offline_and_migrates_target_identity() {
        let clock = Arc::new(FakeClock(AtomicU64::new(100)));
        let persistence = Arc::new(MemoryPersistence::default());
        *persistence.0.lock().unwrap() = Some(UpdatePreferences {
            automatic_checks: true,
            last_successful_check: Some(100),
            cached_offer: Some(cached_offer(
                "2.0.0",
                Some(this_target()),
                "https://svb.miguel.ms/guide/Upgrading.html",
            )),
            ..UpdatePreferences::default()
        });
        let (service, transport) = service(catalog("2.0.0"), clock, persistence.clone());
        assert_eq!(
            service.status().unwrap().available_version.as_deref(),
            Some("2.0.0")
        );
        assert_eq!(transport.calls.load(Ordering::Relaxed), 0);
        assert_eq!(
            persistence
                .0
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .last_successful_check,
            Some(100)
        );
    }

    #[test]
    fn file_preferences_drop_malformed_cached_offer_without_losing_success_timestamp() {
        let directory =
            std::env::temp_dir().join(format!("svb-update-cache-test-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("updates.json");
        std::fs::write(
            &path,
            br#"{"automaticChecks":true,"updateNotifications":false,"lastSuccessfulCheck":100,"cachedOffer":{"version":4,"action":{"type":"install_package","url":"file:///tmp/pkg"},"target":[]}}"#,
        )
        .unwrap();
        let loaded = FileUpdatePersistence::new(path).load(true).unwrap();
        assert!(loaded.cached_offer.is_none());
        assert!(loaded.automatic_checks);
        assert!(!loaded.update_notifications);
        assert_eq!(loaded.last_successful_check, Some(100));
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[tokio::test]
    async fn successful_refresh_withdraws_cached_offer_and_invalidates_open_action() {
        let clock = Arc::new(FakeClock(AtomicU64::new(100)));
        let persistence = Arc::new(MemoryPersistence::default());
        let (active, transport) = service(catalog("2.0.0"), clock.clone(), persistence.clone());
        let offer = active.check(true).await;
        let withdrawn =
            br#"{"schemaVersion":1,"generatedAt":"2026-10-04T00:00:00Z","entries":[]}"#.to_vec();
        *transport.result.lock().unwrap() = Ok(withdrawn);
        clock.0.store(101, Ordering::Relaxed);
        let result = active.check(true).await;
        assert_eq!(result.phase, UpdatePhase::Unavailable);
        assert!(result.available_version.is_none());
        assert!(
            active
                .claim_offer("2.0.0", offer.action.unwrap().url.as_str())
                .is_err()
        );
        let restarted = UpdateService::new(
            distribution(),
            Arc::new(FakeTransport {
                result: Mutex::new(Err("offline")),
                calls: AtomicUsize::new(0),
            }),
            clock,
            persistence,
        )
        .unwrap();
        assert_eq!(restarted.status().unwrap().phase, UpdatePhase::Idle);
    }

    #[tokio::test]
    async fn cached_offer_becomes_stale_after_success_interval_and_cannot_be_opened() {
        let clock = Arc::new(FakeClock(AtomicU64::new(100)));
        let persistence = Arc::new(MemoryPersistence::default());
        let (active, _) = service(catalog("2.0.0"), clock.clone(), persistence);
        let offer = active.check(true).await;
        clock
            .0
            .store(100 + SUCCESS_INTERVAL.as_secs(), Ordering::Relaxed);
        let stale = active.status().unwrap();
        assert_eq!(stale.phase, UpdatePhase::UpdateAvailable);
        assert_eq!(stale.available_version.as_deref(), Some("2.0.0"));
        assert!(stale.offer_stale);
        assert!(
            active
                .claim_offer("2.0.0", &offer.action.unwrap().url)
                .is_err()
        );
    }

    #[tokio::test]
    async fn wake_orchestration_updates_ui_and_never_navigates_without_activation() {
        use tauri::Listener;
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap();
        let (service, transport) = service(
            catalog("2.0.0"),
            Arc::new(FakeClock(AtomicU64::new(100))),
            Arc::new(MemoryPersistence::default()),
        );
        service
            .set_automatic_checks(true)
            .expect("automatic checks can be enabled");
        assert!(app.manage(UpdateManager::new(service)));
        let state_events = Arc::new(AtomicUsize::new(0));
        let opened_pages = Arc::new(AtomicUsize::new(0));
        let observed_state = state_events.clone();
        let observed_navigation = opened_pages.clone();
        let _state_listener = app.listen("update-status-changed", move |_| {
            observed_state.fetch_add(1, Ordering::SeqCst);
        });
        let _navigation_listener = app.listen("open-updates", move |_| {
            observed_navigation.fetch_add(1, Ordering::SeqCst);
        });
        crate::schedule_wake::wake(app.handle());
        tokio::time::timeout(Duration::from_secs(2), async {
            while state_events.load(Ordering::SeqCst) == 0 {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("wake check delivers status to the UI");
        assert_eq!(transport.calls.load(Ordering::Relaxed), 1);
        assert_eq!(state_events.load(Ordering::SeqCst), 1);
        assert_eq!(opened_pages.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn shared_orchestration_delivers_at_most_one_update_notice_across_triggers() {
        use tauri::Listener;
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap();
        let (service, transport) = service(
            catalog("2.0.0"),
            Arc::new(FakeClock(AtomicU64::new(100))),
            Arc::new(MemoryPersistence::default()),
        );
        let status_events = Arc::new(AtomicUsize::new(0));
        let open_events = Arc::new(AtomicUsize::new(0));
        let notifications = Arc::new(AtomicUsize::new(0));
        let permission_barrier = Arc::new(tokio::sync::Barrier::new(2));
        let observed_status = status_events.clone();
        let observed_open = open_events.clone();
        let _status_listener = app.listen("update-status-changed", move |_| {
            observed_status.fetch_add(1, Ordering::SeqCst);
        });
        let _open_listener = app.listen("open-updates", move |_| {
            observed_open.fetch_add(1, Ordering::SeqCst);
        });

        let mut triggers = Vec::new();
        for _ in 0..2 {
            let app = app.handle().clone();
            let service = service.clone();
            let permission_barrier = permission_barrier.clone();
            let delivered = notifications.clone();
            triggers.push(tokio::spawn(async move {
                run_check_and_deliver_with(
                    &app,
                    &service,
                    true,
                    move || async move {
                        permission_barrier.wait().await;
                        true
                    },
                    move |_| async move {
                        delivered.fetch_add(1, Ordering::SeqCst);
                        tokio::task::yield_now().await;
                    },
                )
                .await;
            }));
        }
        for trigger in triggers {
            trigger.await.unwrap();
        }

        assert_eq!(transport.calls.load(Ordering::Relaxed), 2);
        assert_eq!(status_events.load(Ordering::SeqCst), 2);
        assert_eq!(notifications.load(Ordering::SeqCst), 1);
        assert_eq!(open_events.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn additive_metadata_is_accepted_but_required_fields_and_action_types_stay_validated() {
        let mut value: serde_json::Value = serde_json::from_slice(&catalog("2.0.0")).unwrap();
        value["futureCatalogMetadata"] = serde_json::json!({"version": 2});
        value["entries"][0]["autoUpdate"] = serde_json::json!({"package": "app.pkg"});
        value["entries"][0]["action"]["autoUpdate"] = serde_json::json!({"package": "app.pkg"});
        let (accepted, _) = service(
            serde_json::to_vec(&value).unwrap(),
            Arc::new(FakeClock(AtomicU64::new(100))),
            Arc::default(),
        );
        assert_eq!(
            accepted.check(true).await.phase,
            UpdatePhase::UpdateAvailable
        );
        value["entries"][0]
            .as_object_mut()
            .unwrap()
            .remove("releaseNotes");
        let (missing, _) = service(
            serde_json::to_vec(&value).unwrap(),
            Arc::new(FakeClock(AtomicU64::new(100))),
            Arc::default(),
        );
        assert_eq!(missing.check(true).await.phase, UpdatePhase::Unavailable);
        value["entries"][0]["releaseNotes"] = serde_json::json!("Notes");
        value["entries"][0]["action"]["type"] = serde_json::json!("install_package");
        let (unsupported, _) = service(
            serde_json::to_vec(&value).unwrap(),
            Arc::new(FakeClock(AtomicU64::new(100))),
            Arc::default(),
        );
        assert_eq!(
            unsupported.check(true).await.phase,
            UpdatePhase::Unavailable
        );
    }

    #[tokio::test]
    async fn shared_catalog_fixture_additive_metadata_is_accepted() {
        let bytes = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../tests/fixtures/update-catalog/additive-metadata.json"
        ))
        .unwrap();
        let (service, _) = service(
            bytes,
            Arc::new(FakeClock(AtomicU64::new(100))),
            Arc::default(),
        );
        let status = service.check(true).await;
        assert_eq!(status.phase, UpdatePhase::UpdateAvailable, "{status:?}");
    }

    #[tokio::test]
    async fn shared_catalog_fixture_rejection_documents_are_rejected() {
        for name in [
            "missing-required.json",
            "unsupported-action.json",
            "duplicate-key.json",
            "duplicate-target.json",
        ] {
            let bytes = std::fs::read(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../tests/fixtures/update-catalog")
                    .join(name),
            )
            .unwrap();
            let (service, _) = service(
                bytes,
                Arc::new(FakeClock(AtomicU64::new(100))),
                Arc::default(),
            );
            assert_eq!(
                service.check(true).await.phase,
                UpdatePhase::Unavailable,
                "{name}"
            );
        }
    }

    #[tokio::test]
    async fn catalog_rejects_unknown_edition_platform_architecture_and_channel_values() {
        let cases = [
            ("\"edition\":\"direct_macos\"", "\"edition\":\"custom\""),
            ("\"os\":\"macos\"", "\"os\":\"freebsd\""),
            (
                "\"architecture\":\"aarch64\"",
                "\"architecture\":\"universal\"",
            ),
            ("\"channel\":\"stable\"", "\"channel\":\"beta\""),
        ];
        for (from, to) in cases {
            let bytes = String::from_utf8(catalog("2.0.0"))
                .unwrap()
                .replacen(from, to, 1)
                .into_bytes();
            let (service, _) = service(
                bytes,
                Arc::new(FakeClock(AtomicU64::new(100))),
                Arc::default(),
            );
            assert_eq!(service.check(true).await.phase, UpdatePhase::Unavailable);
        }
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
            .replace(
                "https://svb.miguel.ms/guide/Upgrading.html",
                "file:///tmp/run",
            )
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
    async fn failed_refresh_preserves_offer_but_marks_failure_for_bounded_retry() {
        let clock = Arc::new(FakeClock(AtomicU64::new(100)));
        let persistence = Arc::new(MemoryPersistence::default());
        let (active, transport) = service(catalog("2.0.0"), clock.clone(), persistence);
        assert_eq!(active.check(true).await.phase, UpdatePhase::UpdateAvailable);
        *transport.result.lock().unwrap() = Err("offline");
        clock.0.store(101, Ordering::Relaxed);
        let failed = active.check(true).await;
        assert_eq!(failed.phase, UpdatePhase::UpdateAvailable);
        assert_eq!(failed.available_version.as_deref(), Some("2.0.0"));
        assert!(failed.check_failed);
        assert_eq!(failed.last_successful_check, Some(100));
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
        assert!(active.claim_notification(&status).unwrap());
        assert!(!active.claim_notification(&status).unwrap());
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
