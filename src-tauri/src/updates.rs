use crate::distribution::{
    ApplicationArchitecture, DistributionEdition, InstalledDistribution, ReleaseChannel,
};
use async_trait::async_trait;
use semver::Version;
use serde::de::{MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Serialize};
use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::Emitter;
use tokio::sync::Mutex as AsyncMutex;
use url::Url;

pub const RELEASES_URL: &str =
    "https://api.github.com/repos/MiguelTVMS/speaker-volume-bridge/releases";
const MAX_RELEASE_BYTES: usize = 2 * 1024 * 1024;
const MAX_RELEASE_PAGES: u32 = 10;
#[allow(clippy::duration_suboptimal_units)] // Keep compatibility with the repository's pinned Rust.
const SUCCESS_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

/// Shared policy contract for checking and future installer revalidation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdatePolicy {
    #[default]
    Stable,
    Prereleases,
}
impl UpdatePolicy {
    const fn key(self) -> &'static str {
        match self {
            Self::Stable => "stable",
            Self::Prereleases => "prereleases",
        }
    }
}

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
    StoreManaged,
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
    pub policy: UpdatePolicy,
    pub prerelease_supported: bool,
    pub generation: u64,
    pub source_available: Option<bool>,
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
            return Err(UpdateError::InvalidRelease("unsupported action"));
        }
        let parsed =
            Url::parse(&self.url).map_err(|_| UpdateError::InvalidRelease("invalid action URL"))?;
        if parsed.scheme() != "https"
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || !matches!(
                parsed.host_str(),
                Some("svb.miguel.ms" | "github.com" | "apps.microsoft.com" | "apps.apple.com")
            )
        {
            return Err(UpdateError::InvalidRelease("untrusted action URL"));
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
    pub notified_targets: Vec<String>,
    pub cached_offer: Option<CachedOffer>,
    pub policy: UpdatePolicy,
    pub freshness_target: Option<CachedTarget>,
    pub generation: u64,
    pub stable_source_available: Option<bool>,
    pub preview_source_available: Option<bool>,
    #[serde(default)]
    pub update_source: Option<String>,
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
            notified_targets: Vec::new(),
            cached_offer: None,
            policy: UpdatePolicy::Stable,
            freshness_target: None,
            generation: 0,
            stable_source_available: None,
            preview_source_available: None,
            update_source: Some("github_releases_v1".into()),
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
pub trait ReleaseTransport: Send + Sync {
    async fn fetch(&self) -> Result<Vec<u8>, UpdateError>;
    async fn fetch_policy(&self, _policy: UpdatePolicy) -> Result<Vec<u8>, UpdateError> {
        self.fetch().await
    }
}

pub struct HttpReleaseTransport {
    client: reqwest::Client,
    endpoint: String,
    retry_after: AtomicU64,
}
impl HttpReleaseTransport {
    pub fn new() -> Result<Self, UpdateError> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::limited(3))
            .build()?;
        Ok(Self {
            client,
            endpoint: RELEASES_URL.into(),
            retry_after: AtomicU64::new(0),
        })
    }
    fn request(&self, _policy: UpdatePolicy) -> reqwest::RequestBuilder {
        self.page_request(1)
    }
    fn page_request(&self, page: u32) -> reqwest::RequestBuilder {
        self.client
            .get(format!("{}?per_page=100&page={page}", self.endpoint))
            .header(reqwest::header::USER_AGENT, "Speaker-Volume-Bridge")
            .header(reqwest::header::ACCEPT, "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2026-03-10")
    }
}
#[async_trait]
impl ReleaseTransport for HttpReleaseTransport {
    async fn fetch(&self) -> Result<Vec<u8>, UpdateError> {
        self.fetch_policy(UpdatePolicy::Stable).await
    }
    async fn fetch_policy(&self, policy: UpdatePolicy) -> Result<Vec<u8>, UpdateError> {
        if SystemUpdateClock.now() < self.retry_after.load(Ordering::SeqCst) {
            return Err(UpdateError::RateLimited);
        }
        let mut releases = Vec::new();
        let mut total = 0;
        for page in 1..=MAX_RELEASE_PAGES {
            let request = if page == 1 {
                self.request(policy)
            } else {
                self.page_request(page)
            };
            let response = request.send().await?;
            if let Some(until) = rate_limit_deadline(
                response.status(),
                response.headers(),
                SystemUpdateClock.now(),
            ) {
                self.retry_after.store(until, Ordering::SeqCst);
                return Err(UpdateError::RateLimited);
            }
            let mut response = response.error_for_status()?;
            let mut bytes = Vec::new();
            while let Some(chunk) = response.chunk().await? {
                total += chunk.len();
                if total > MAX_RELEASE_BYTES {
                    return Err(UpdateError::InvalidRelease("release response too large"));
                }
                bytes.extend_from_slice(&chunk);
            }
            let mut decoder = serde_json::Deserializer::from_slice(&bytes);
            let unique = UniqueJson::deserialize(&mut decoder)?.0;
            decoder.end()?;
            let values = unique
                .as_array()
                .ok_or(UpdateError::InvalidRelease("invalid release list"))?;
            let complete = values.len() < 100;
            releases.extend(values.iter().cloned());
            if complete {
                let bytes = serde_json::to_vec(&releases)?;
                if bytes.len() > MAX_RELEASE_BYTES {
                    return Err(UpdateError::InvalidRelease("release response too large"));
                }
                return Ok(bytes);
            }
        }
        Err(UpdateError::InvalidRelease(
            "release pagination limit exceeded",
        ))
    }
}

fn rate_limit_deadline(
    status: reqwest::StatusCode,
    headers: &reqwest::header::HeaderMap,
    now: u64,
) -> Option<u64> {
    // Secondary limits can omit Retry-After while the primary quota remains.
    // Conservatively cool down every forbidden response without reading its body.
    if status != reqwest::StatusCode::TOO_MANY_REQUESTS && status != reqwest::StatusCode::FORBIDDEN
    {
        return None;
    }
    let number = |name| {
        headers
            .get(name)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u64>().ok())
    };
    Some(
        now.saturating_add(number("retry-after").unwrap_or(60).max(60))
            .max(number("x-ratelimit-reset").unwrap_or(0)),
    )
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
    #[error("release information is unavailable")]
    Network(#[from] reqwest::Error),
    #[error("update state I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("update state is invalid: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid release information: {0}")]
    InvalidRelease(&'static str),
    #[error("GitHub rate limit reached; try again later")]
    RateLimited,
    #[error("update state is unavailable")]
    State,
}

#[cfg(test)]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Catalog {
    schema_version: u32,
    generated_at: String,
    entries: Vec<CatalogEntry>,
}
#[cfg(test)]
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
    #[serde(default)]
    classification: Option<String>,
}

#[derive(Deserialize)]
struct GitHubRelease {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    published_at: Option<String>,
    html_url: String,
    assets: Vec<GitHubAsset>,
}
#[derive(Deserialize)]
struct GitHubAsset {
    name: String,
    state: String,
    size: u64,
    browser_download_url: String,
}

fn release_asset_name(distribution: &InstalledDistribution) -> Option<&'static str> {
    use ApplicationArchitecture::{Aarch64, X86_64};
    use DistributionEdition::{Debian, DirectMacos, DirectWindows};
    // The official unqualified DMG is built on the ARM macOS release runner.
    // It does not establish support for an Intel installation.
    match (distribution.edition, distribution.architecture) {
        (DirectMacos, Aarch64) => Some("speaker-volume-bridge-macos.dmg"),
        (DirectWindows, X86_64) => Some("speaker-volume-bridge-windows-x64-unsigned.exe"),
        (DirectWindows, Aarch64) => Some("speaker-volume-bridge-windows-arm64-unsigned.exe"),
        (Debian, X86_64) => Some("speaker-volume-bridge-linux-x64.deb"),
        (Debian, Aarch64) => Some("speaker-volume-bridge-linux-arm64.deb"),
        _ => None,
    }
}

struct UniqueJson(serde_json::Value);

impl<'de> Deserialize<'de> for UniqueJson {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct UniqueJsonVisitor;
        impl<'de> Visitor<'de> for UniqueJsonVisitor {
            type Value = UniqueJson;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a JSON value without duplicate object members")
            }

            fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
                Ok(UniqueJson(value.into()))
            }
            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
                Ok(UniqueJson(value.into()))
            }
            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
                Ok(UniqueJson(value.into()))
            }
            fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                serde_json::Number::from_f64(value)
                    .map(|number| UniqueJson(number.into()))
                    .ok_or_else(|| E::custom("non-finite JSON number"))
            }
            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> {
                Ok(UniqueJson(value.into()))
            }
            fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
                Ok(UniqueJson(value.into()))
            }
            fn visit_none<E>(self) -> Result<Self::Value, E> {
                Ok(UniqueJson(serde_json::Value::Null))
            }
            fn visit_unit<E>(self) -> Result<Self::Value, E> {
                Ok(UniqueJson(serde_json::Value::Null))
            }
            fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut values = Vec::new();
                while let Some(UniqueJson(value)) = sequence.next_element()? {
                    values.push(value);
                }
                Ok(UniqueJson(values.into()))
            }
            fn visit_map<A>(self, mut object: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut values = serde_json::Map::new();
                while let Some(key) = object.next_key::<String>()? {
                    if values.contains_key(&key) {
                        return Err(serde::de::Error::custom("duplicate object member"));
                    }
                    let UniqueJson(value) = object.next_value()?;
                    values.insert(key, value);
                }
                Ok(UniqueJson(values.into()))
            }
        }
        deserializer.deserialize_any(UniqueJsonVisitor)
    }
}

// Fixed native Store destinations never come from release metadata or frontend input.
fn store_url(edition: DistributionEdition) -> Option<&'static str> {
    match edition {
        DistributionEdition::MicrosoftStore => {
            Some("ms-windows-store://pdp/?ProductId=9N7JKGXCMST0")
        }
        DistributionEdition::MacAppStore => Some("macappstore://showUpdatesPage"),
        _ => None,
    }
}

pub struct UpdateService {
    distribution: InstalledDistribution,
    transport: Arc<dyn ReleaseTransport>,
    clock: Arc<dyn UpdateClock>,
    persistence: Arc<dyn UpdatePersistence>,
    preferences: Mutex<UpdatePreferences>,
    status: Mutex<UpdateStatus>,
    dismissed_target: Mutex<Option<String>>,
    opening: AtomicBool,
    in_flight: AsyncMutex<()>,
    request_sequence: AtomicU64,
    completed_generation: AtomicU64,
}

impl UpdateService {
    pub fn new(
        distribution: InstalledDistribution,
        transport: Arc<dyn ReleaseTransport>,
        clock: Arc<dyn UpdateClock>,
        persistence: Arc<dyn UpdatePersistence>,
    ) -> Result<Self, UpdateError> {
        let mut preferences = persistence.load(distribution.check_supported)?;
        if preferences.update_source.as_deref() != Some("github_releases_v1") {
            preferences.cached_offer = None;
            preferences.freshness_target = None;
            preferences.last_successful_check = None;
            preferences.last_attempted_check = None;
            preferences.stable_source_available = None;
            preferences.preview_source_available = None;
            preferences.update_source = Some("github_releases_v1".into());
            preferences.generation += 1;
            persistence.save(&preferences)?;
        }
        if !distribution.prerelease_supported() && preferences.policy != UpdatePolicy::Stable {
            preferences.policy = UpdatePolicy::Stable;
            preferences.cached_offer = None;
            preferences.last_successful_check = None;
            preferences.last_attempted_check = None;
            preferences.freshness_target = None;
            preferences.generation += 1;
            persistence.save(&preferences)?;
        }
        if preferences
            .freshness_target
            .as_ref()
            .is_some_and(|target| target != &policy_target(&distribution, preferences.policy))
        {
            preferences.cached_offer = None;
            preferences.last_successful_check = None;
            preferences.last_attempted_check = None;
            preferences.freshness_target = None;
            persistence.save(&preferences)?;
        }
        if let Some(offer) = preferences.cached_offer.as_ref()
            && !cached_offer_valid_for_policy(offer, &distribution, preferences.policy)
        {
            preferences.cached_offer = None;
            persistence.save(&preferences)?;
        }
        let phase = if store_url(distribution.edition).is_some() {
            preferences.automatic_checks = false;
            preferences.cached_offer = None;
            preferences.last_successful_check = None;
            preferences.freshness_target = None;
            persistence.save(&preferences)?;
            UpdatePhase::StoreManaged
        } else if !distribution.check_supported {
            UpdatePhase::Unsupported
        } else if preferences.cached_offer.is_some() {
            UpdatePhase::UpdateAvailable
        } else {
            UpdatePhase::Idle
        };
        Ok(Self {
            status: Mutex::new(UpdateStatus {
                phase,
                policy: preferences.policy,
                prerelease_supported: distribution.prerelease_supported(),
                generation: preferences.generation,
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
            request_sequence: AtomicU64::new(0),
            completed_generation: AtomicU64::new(0),
        })
    }
    pub fn store_action(&self) -> Result<&'static str, UpdateError> {
        store_url(self.distribution.edition).ok_or(UpdateError::State)
    }
    pub fn status(&self) -> Result<UpdateStatus, UpdateError> {
        let preferences = self.preferences.lock().map_err(|_| UpdateError::State)?;
        let mut status = self.status.lock().map_err(|_| UpdateError::State)?.clone();
        status.automatic_checks = preferences.automatic_checks;
        status.update_notifications = preferences.update_notifications;
        status.policy = preferences.policy;
        status.generation = preferences.generation;
        status.prerelease_supported = self.distribution.prerelease_supported();
        status.source_available = match preferences.policy {
            UpdatePolicy::Stable => preferences.stable_source_available,
            UpdatePolicy::Prereleases => preferences.preview_source_available,
        };
        status.prompt_dismissed = status.available_version.as_ref().is_some_and(|version| {
            self.dismissed_target
                .lock()
                .is_ok_and(|value| value.as_ref() == Some(version))
        });
        status.offer_stale = status.available_version.is_some()
            && (preferences.freshness_target.as_ref()
                != Some(&policy_target(&self.distribution, preferences.policy))
                || status.last_successful_check.is_none_or(|last| {
                    self.clock.now().saturating_sub(last) >= SUCCESS_INTERVAL.as_secs()
                }));
        Ok(status)
    }
    pub fn preferences(&self) -> Result<UpdatePreferences, UpdateError> {
        self.preferences
            .lock()
            .map(|value| value.clone())
            .map_err(|_| UpdateError::State)
    }
    pub fn set_policy(&self, policy: UpdatePolicy) -> Result<(), UpdateError> {
        if !self.distribution.prerelease_supported() {
            return Err(UpdateError::State);
        }
        let mut preferences = self.preferences.lock().map_err(|_| UpdateError::State)?;
        let mut next = preferences.clone();
        next.policy = policy;
        next.generation += 1;
        next.cached_offer = None;
        next.last_successful_check = None;
        next.last_attempted_check = None;
        next.freshness_target = None;
        self.persistence.save(&next)?;
        *preferences = next;
        *self
            .dismissed_target
            .lock()
            .map_err(|_| UpdateError::State)? = None;
        self.replace_status(self.base_status(UpdatePhase::Checking));
        Ok(())
    }
    pub fn set_automatic_checks(&self, enabled: bool) -> Result<(), UpdateError> {
        let mut preferences = self.preferences.lock().map_err(|_| UpdateError::State)?;
        preferences.automatic_checks = enabled
            && self.distribution.check_supported
            && store_url(self.distribution.edition).is_none();
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
        let target = notification_target(&self.distribution, version);
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
        preferences.last_notified_target = Some(notification_target(&self.distribution, version));
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
        let target = notification_target(&self.distribution, version);
        let mut preferences = self.preferences.lock().map_err(|_| UpdateError::State)?;
        if preferences.generation != status.generation
            || preferences.policy != status.policy
            || preferences
                .cached_offer
                .as_ref()
                .is_none_or(|offer| offer.version != version)
            || !preferences.update_notifications
            || preferences.last_notified_target.as_deref() == Some(&target)
            || preferences.notified_targets.contains(&target)
        {
            return Ok(false);
        }
        preferences.notified_targets.push(target.clone());
        preferences.last_notified_target = Some(target);
        self.persistence.save(&preferences)?;
        Ok(true)
    }
    #[cfg(test)]
    pub fn claim_offer(&self, version: &str, url: &str) -> Result<OpenUrlAction, UpdateError> {
        self.claim_offer_generation(version, url, self.preferences()?.generation)
    }
    pub fn claim_offer_generation(
        &self,
        version: &str,
        url: &str,
        generation: u64,
    ) -> Result<OpenUrlAction, UpdateError> {
        let preferences = self.preferences.lock().map_err(|_| UpdateError::State)?;
        if preferences.generation != generation
            || preferences.freshness_target.as_ref()
                != Some(&policy_target(&self.distribution, preferences.policy))
        {
            return Err(UpdateError::State);
        }
        let status = self.status.lock().map_err(|_| UpdateError::State)?.clone();
        let action = status.action.ok_or(UpdateError::State)?;
        if status.phase != UpdatePhase::UpdateAvailable
            || status.last_successful_check.is_none_or(|last| {
                self.clock.now().saturating_sub(last) >= SUCCESS_INTERVAL.as_secs()
            })
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
        store_url(self.distribution.edition).is_none()
            && self.distribution.check_supported
            && preferences.automatic_checks
            && preferences
                .last_attempted_check
                .is_none_or(|last| now.saturating_sub(last) >= 60)
            && (preferences.freshness_target.as_ref()
                != Some(&policy_target(&self.distribution, preferences.policy))
                || preferences
                    .last_successful_check
                    .is_none_or(|last| now.saturating_sub(last) >= SUCCESS_INTERVAL.as_secs()))
    }
    pub async fn check(&self, manual: bool) -> UpdateStatus {
        if store_url(self.distribution.edition).is_some() || !self.distribution.check_supported {
            return self.status().unwrap_or_default();
        }
        let sequence = self.request_sequence.load(Ordering::SeqCst);
        let observed = self.preferences().unwrap_or_default();
        let _guard = self.in_flight.lock().await;
        let current = self.preferences().unwrap_or_default();
        if current.generation != observed.generation
            || (self.request_sequence.load(Ordering::SeqCst) != sequence
                && self.completed_generation.load(Ordering::SeqCst) == observed.generation)
        {
            return self.status().unwrap_or_default();
        }
        if !manual && !self.automatic_due() {
            return self.status().unwrap_or_default();
        }
        let now = self.clock.now();
        if self.begin_check(observed.generation, now).is_err() {
            return self.status().unwrap_or_default();
        }
        let result = self
            .transport
            .fetch_policy(observed.policy)
            .await
            .and_then(|bytes| self.evaluate_policy(&bytes, observed.policy));
        // Commit under the same lock as preference changes: obsolete work cannot
        // restore caches, success timestamps, offers or notification intent.
        let Ok(mut preferences) = self.preferences.lock() else {
            return UpdateStatus::default();
        };
        self.completed_generation
            .store(observed.generation, Ordering::SeqCst);
        self.request_sequence.fetch_add(1, Ordering::SeqCst);
        if preferences.generation != observed.generation {
            drop(preferences);
            return self.status().unwrap_or_default();
        }
        let source_available = result.is_ok();
        match observed.policy {
            UpdatePolicy::Stable => preferences.stable_source_available = Some(source_available),
            UpdatePolicy::Prereleases => {
                preferences.preview_source_available = Some(source_available);
            }
        }
        let status = match result {
            Ok(mut status) => {
                {
                    preferences.last_successful_check = Some(now);
                    preferences.cached_offer = status
                        .available_version
                        .as_ref()
                        .zip(status.action.as_ref())
                        .map(|(version, action)| CachedOffer {
                            version: version.clone(),
                            action: action.clone(),
                            target: Some(policy_target(&self.distribution, observed.policy)),
                        });
                    preferences.freshness_target =
                        Some(policy_target(&self.distribution, observed.policy));
                    let _ = self.persistence.save(&preferences);
                    status.last_successful_check = Some(now);
                }
                status
            }
            Err(error) => {
                let mut cached = self
                    .status
                    .lock()
                    .map_or_else(|_| UpdateStatus::default(), |value| value.clone());
                if cached.phase != UpdatePhase::UpdateAvailable {
                    cached.phase = UpdatePhase::Unavailable;
                }
                cached.check_failed = true;
                cached.message = manual.then(|| error.to_string());
                cached
            }
        };
        let mut status = status;
        status.policy = observed.policy;
        status.generation = observed.generation;
        let _ = self.persistence.save(&preferences);
        self.replace_status(status.clone());
        drop(preferences);
        self.status().unwrap_or(status)
    }
    fn begin_check(&self, generation: u64, now: u64) -> Result<(), UpdateError> {
        let mut preferences = self.preferences.lock().map_err(|_| UpdateError::State)?;
        if preferences.generation != generation {
            return Err(UpdateError::State);
        }
        preferences.last_attempted_check = Some(now);
        self.persistence.save(&preferences)?;
        let mut checking = self.status.lock().map_err(|_| UpdateError::State)?.clone();
        checking.phase = if checking.available_version.is_some() {
            UpdatePhase::UpdateAvailable
        } else {
            UpdatePhase::Checking
        };
        checking.message = None;
        self.replace_status(checking);
        Ok(())
    }
    fn replace_status(&self, status: UpdateStatus) {
        if let Ok(mut current) = self.status.lock() {
            *current = status;
        }
    }
    fn evaluate_policy(
        &self,
        bytes: &[u8],
        policy: UpdatePolicy,
    ) -> Result<UpdateStatus, UpdateError> {
        if bytes.len() > MAX_RELEASE_BYTES {
            return Err(UpdateError::InvalidRelease("release response too large"));
        }
        let mut decoder = serde_json::Deserializer::from_slice(bytes);
        let unique = UniqueJson::deserialize(&mut decoder)?.0;
        decoder.end()?;
        // Retain the former contract fixtures for lifecycle regressions only.
        // Production accepts the GitHub release array exclusively.
        #[cfg(test)]
        if unique.is_object() {
            return self.evaluate_catalog_policy(bytes, policy);
        }
        let releases: Vec<GitHubRelease> = serde_json::from_value(unique)?;
        let asset_name = release_asset_name(&self.distribution);
        let installed = Version::parse(&self.distribution.version)
            .map_err(|_| UpdateError::InvalidRelease("invalid installed version"))?;
        let mut selected: Option<(Version, OpenUrlAction)> = None;
        for release in releases {
            if release.draft || (policy == UpdatePolicy::Stable && release.prerelease) {
                continue;
            }
            let Some(version) = release
                .tag_name
                .strip_prefix('v')
                .and_then(|tag| Version::parse(tag).ok())
            else {
                continue;
            };
            if !release.prerelease && !version.pre.is_empty() {
                continue;
            }
            if release
                .published_at
                .as_deref()
                .is_none_or(|time| time.parse::<jiff::Timestamp>().is_err())
                || release.html_url != release_page(&version.to_string())
            {
                continue;
            }
            let Some(name) = asset_name else {
                continue;
            };
            let matching: Vec<_> = release
                .assets
                .iter()
                .filter(|asset| asset.name == name)
                .collect();
            if matching.len() != 1 {
                continue;
            }
            let asset = matching[0];
            let expected = format!(
                "https://github.com/MiguelTVMS/speaker-volume-bridge/releases/download/{}/{name}",
                release.tag_name
            );
            if asset.state != "uploaded"
                || asset.size == 0
                || asset.browser_download_url != expected
            {
                continue;
            }
            if selected
                .as_ref()
                .is_none_or(|(current, _)| version.cmp_precedence(current).is_gt())
            {
                selected = Some((
                    version,
                    OpenUrlAction {
                        kind: "open_url".into(),
                        url: release.html_url,
                    }
                    .validate()?,
                ));
            }
        }
        let Some((version, action)) = selected else {
            let mut status = self.base_status(UpdatePhase::Unavailable);
            status.message =
                Some("No compatible public release is available. Check again later.".into());
            return Ok(status);
        };
        if !version.cmp_precedence(&installed).is_gt() {
            return Ok(self.base_status(UpdatePhase::UpToDate));
        }
        let mut status = self.base_status(UpdatePhase::UpdateAvailable);
        status.available_version = Some(version.to_string());
        status.action = Some(action);
        Ok(status)
    }
    #[cfg(test)]
    fn evaluate_catalog_policy(
        &self,
        bytes: &[u8],
        policy: UpdatePolicy,
    ) -> Result<UpdateStatus, UpdateError> {
        if bytes.len() > MAX_RELEASE_BYTES {
            return Err(UpdateError::InvalidRelease("catalog too large"));
        }
        let mut decoder = serde_json::Deserializer::from_slice(bytes);
        let unique = UniqueJson::deserialize(&mut decoder)?.0;
        decoder.end()?;
        let catalog: Catalog = serde_json::from_value(unique)?;
        if catalog.schema_version != if policy == UpdatePolicy::Stable { 1 } else { 2 }
            || catalog.generated_at.parse::<jiff::Timestamp>().is_err()
        {
            return Err(UpdateError::InvalidRelease("unsupported schema"));
        }
        let mut targets = std::collections::HashSet::new();
        for entry in &catalog.entries {
            validate_entry(entry, policy)?;
            if !targets.insert((
                &entry.edition,
                &entry.channel,
                &entry.os,
                &entry.architecture,
                if policy == UpdatePolicy::Prereleases {
                    entry.classification.as_deref()
                } else {
                    None
                },
            )) {
                return Err(UpdateError::InvalidRelease("duplicate target"));
            }
        }
        let target = target_key(&self.distribution);
        let entry = catalog
            .entries
            .into_iter()
            .filter(|entry| {
                entry.edition == target.0 && entry.os == target.2 && entry.architecture == target.3
            })
            .max_by(|left, right| {
                Version::parse(&left.version)
                    .unwrap()
                    .cmp_precedence(&Version::parse(&right.version).unwrap())
            });
        let Some(entry) = entry else {
            let mut status = self.base_status(UpdatePhase::Unavailable);
            status.message = Some(
                "No compatible release is available in the selected catalog. Check again later."
                    .into(),
            );
            return Ok(status);
        };
        let available = Version::parse(&entry.version)
            .map_err(|_| UpdateError::InvalidRelease("invalid semantic version"))?;
        let installed = Version::parse(&self.distribution.version)
            .map_err(|_| UpdateError::InvalidRelease("invalid installed version"))?;
        if !available.cmp_precedence(&installed).is_gt() {
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

#[cfg(test)]
fn validate_entry(entry: &CatalogEntry, policy: UpdatePolicy) -> Result<(), UpdateError> {
    let expected_os = match entry.edition.as_str() {
        "direct_macos" | "mac_app_store" => "macos",
        "direct_windows" | "microsoft_store" => "windows",
        "debian" => "linux",
        _ => return Err(UpdateError::InvalidRelease("unsupported edition")),
    };
    if entry.os != expected_os {
        return Err(UpdateError::InvalidRelease("unsupported platform"));
    }
    if !matches!(entry.architecture.as_str(), "aarch64" | "x86_64") {
        return Err(UpdateError::InvalidRelease("unsupported architecture"));
    }
    let version = Version::parse(&entry.version)
        .map_err(|_| UpdateError::InvalidRelease("invalid semantic version"))?;
    let valid = if policy == UpdatePolicy::Stable {
        entry.channel == "stable"
            && version.pre.is_empty()
            && entry
                .classification
                .as_deref()
                .is_none_or(|value| value == "GA")
    } else {
        matches!(
            entry.edition.as_str(),
            "direct_macos" | "direct_windows" | "debian"
        ) && match entry.classification.as_deref() {
            Some("GA") => entry.channel == "stable" && version.pre.is_empty(),
            Some("Alpha" | "Beta") => entry.channel == "prereleases",
            _ => false,
        }
    };
    if !valid {
        return Err(UpdateError::InvalidRelease("invalid stable version"));
    }
    entry
        .published_at
        .parse::<jiff::Timestamp>()
        .map_err(|_| UpdateError::InvalidRelease("invalid publication time"))?;
    if entry.release_notes.is_empty() || entry.release_notes.len() > 16 * 1024 {
        return Err(UpdateError::InvalidRelease("invalid release notes"));
    }
    entry.action.clone().validate()?;
    if policy == UpdatePolicy::Prereleases && entry.action.url != release_page(&entry.version) {
        return Err(UpdateError::InvalidRelease(
            "preview action must open exact release",
        ));
    }
    Ok(())
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

fn release_page(version: &str) -> String {
    format!("https://github.com/MiguelTVMS/speaker-volume-bridge/releases/tag/v{version}")
}
fn notification_target(distribution: &InstalledDistribution, version: &str) -> String {
    let (_, _, os, architecture) = target_key(distribution);
    format!(
        "{}:{os}:{architecture}:{version}",
        distribution.edition_key()
    )
}
fn policy_target(distribution: &InstalledDistribution, policy: UpdatePolicy) -> CachedTarget {
    let mut target = cached_target(distribution);
    target.channel = policy.key().into();
    target
}
fn cached_offer_valid_for_policy(
    offer: &CachedOffer,
    distribution: &InstalledDistribution,
    policy: UpdatePolicy,
) -> bool {
    let Some(target) = offer.target.as_ref() else {
        return false;
    };
    let Ok(installed) = Version::parse(&distribution.version) else {
        return false;
    };
    let Ok(available) = Version::parse(&offer.version) else {
        return false;
    };
    available.cmp_precedence(&installed).is_gt()
        && (
            target.edition.as_str(),
            target.channel.as_str(),
            target.os.as_str(),
            target.architecture.as_str(),
        ) == {
            let target = target_key(distribution);
            (target.0, policy.key(), target.2, target.3)
        }
        && (policy == UpdatePolicy::Prereleases || available.pre.is_empty())
        && offer.action.clone().validate().is_ok()
        && (policy == UpdatePolicy::Stable || offer.action.url == release_page(&offer.version))
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
    stopped: Arc<AtomicBool>,
}
impl UpdateManager {
    pub fn new(service: Arc<UpdateService>) -> Self {
        Self {
            service,
            task: Mutex::new(None),
            stopped: Arc::new(AtomicBool::new(false)),
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
        self.start_with_sleep(app, tokio::time::sleep);
    }

    // Inject only the timer; lifecycle ownership and check delivery remain shared
    // with production startup so tests can drive every scheduling boundary.
    fn start_with_sleep<R, S, F>(&self, app: &tauri::AppHandle<R>, sleep: S)
    where
        R: tauri::Runtime,
        S: Fn(Duration) -> F + Send + 'static,
        F: std::future::Future<Output = ()> + Send,
    {
        let Ok(mut task) = self.task.lock() else {
            return;
        };
        if task.is_some() {
            return;
        }
        let service = Arc::clone(&self.service);
        let app_handle = app.clone();
        let stopped = Arc::clone(&self.stopped);
        *task = Some(tauri::async_runtime::spawn(async move {
            sleep(Duration::from_secs(30)).await;
            #[allow(clippy::duration_suboptimal_units)] // Keep compatibility with pinned Rust.
            let backoff = [
                Duration::from_secs(60),
                Duration::from_secs(5 * 60),
                Duration::from_secs(30 * 60),
            ];
            loop {
                if stopped.load(Ordering::SeqCst) {
                    return;
                }
                if service.automatic_due() {
                    let status = run_check_and_deliver(&app_handle, &service, false).await;
                    if status.check_failed {
                        for delay in backoff {
                            sleep(delay).await;
                            if stopped.load(Ordering::SeqCst) {
                                return;
                            }
                            let status = run_check_and_deliver(&app_handle, &service, false).await;
                            if !status.check_failed {
                                break;
                            }
                        }
                    }
                }
                #[allow(clippy::duration_suboptimal_units)]
                sleep(Duration::from_secs(60)).await;
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
        // Abort is cooperative; a currently polling ready timer must also see
        // shutdown before it can begin another catalog request.
        self.stopped.store(true, Ordering::SeqCst);
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
    impl ReleaseTransport for FakeTransport {
        async fn fetch(&self) -> Result<Vec<u8>, UpdateError> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            self.result
                .lock()
                .unwrap()
                .clone()
                .map_err(UpdateError::InvalidRelease)
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
            published_classification: None,
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
    async fn store_managed_production_paths_never_discover_or_notify() {
        for edition in [
            DistributionEdition::MicrosoftStore,
            DistributionEdition::MacAppStore,
        ] {
            let mut installed = distribution();
            installed.edition = edition;
            let persistence = Arc::new(MemoryPersistence::default());
            // Reproduce an existing catalog client's enabled automatic preference.
            *persistence.0.lock().unwrap() = Some(UpdatePreferences {
                automatic_checks: true,
                last_successful_check: Some(99),
                ..UpdatePreferences::default()
            });
            let transport = Arc::new(FakeTransport {
                result: Mutex::new(Err("Store must not contact releases")),
                calls: AtomicUsize::new(0),
            });
            let service = Arc::new(
                UpdateService::new(
                    installed.clone(),
                    transport.clone(),
                    Arc::new(FakeClock(AtomicU64::new(100))),
                    persistence.clone(),
                )
                .unwrap(),
            );
            let app = tauri::test::mock_builder()
                .build(tauri::test::mock_context(tauri::test::noop_assets()))
                .unwrap();
            service.set_automatic_checks(true).unwrap();
            UpdateManager::wake_service(app.handle(), service.clone()).await;
            let manual = run_check_and_deliver_with(
                app.handle(),
                &service,
                true,
                || async { panic!("Store must not request notification permission") },
                |_| async { panic!("Store must not notify") },
            )
            .await;
            assert_eq!(transport.calls.load(Ordering::SeqCst), 0);
            assert!(!service.automatic_due());
            assert_eq!(manual.phase, UpdatePhase::StoreManaged);
            assert!(manual.action.is_none());
            assert!(manual.available_version.is_none());
            assert!(manual.last_successful_check.is_none());
            assert!(service.set_policy(UpdatePolicy::Prereleases).is_err());
            let expected = if edition == DistributionEdition::MicrosoftStore {
                "ms-windows-store://pdp/?ProductId=9N7JKGXCMST0"
            } else {
                "macappstore://showUpdatesPage"
            };
            assert_eq!(service.store_action().unwrap(), expected);
            assert!(
                service
                    .claim_offer_generation("2.0.0", expected, manual.generation)
                    .is_err()
            );
            let manager = UpdateManager::new(service);
            let (tick, mut ticks) = tokio::sync::mpsc::unbounded_channel();
            manager.start_with_sleep(app.handle(), move |duration| {
                let tick = tick.clone();
                async move {
                    tick.send(duration).unwrap();
                    if duration != Duration::from_secs(30) {
                        std::future::pending::<()>().await;
                    }
                }
            });
            assert_eq!(ticks.recv().await.unwrap(), Duration::from_secs(30));
            assert_eq!(ticks.recv().await.unwrap(), Duration::from_secs(60));
            drop(manager);
            assert_eq!(transport.calls.load(Ordering::SeqCst), 0);
            let restarted = UpdateService::new(
                installed,
                transport,
                Arc::new(FakeClock(AtomicU64::new(101))),
                persistence,
            )
            .unwrap();
            assert_eq!(restarted.status().unwrap().phase, UpdatePhase::StoreManaged);
            assert!(!restarted.status().unwrap().automatic_checks);
        }
        let (direct, _) = service(
            Vec::new(),
            Arc::new(FakeClock(AtomicU64::new(100))),
            Arc::new(MemoryPersistence::default()),
        );
        assert!(direct.store_action().is_err());
    }

    #[tokio::test]
    async fn production_scheduler_delays_retries_once_and_cancels_on_shutdown() {
        async fn boundary(
            timers: &mut tokio::sync::mpsc::UnboundedReceiver<(
                Duration,
                tokio::sync::oneshot::Sender<()>,
            )>,
        ) -> (Duration, tokio::sync::oneshot::Sender<()>) {
            tokio::time::timeout(Duration::from_secs(5), timers.recv())
                .await
                .unwrap()
                .unwrap()
        }
        let clock = Arc::new(FakeClock(AtomicU64::new(100)));
        let (service, transport) = service(
            serde_json::to_vec(&vec![public_release("2.0.0", false)]).unwrap(),
            clock.clone(),
            Arc::new(MemoryPersistence::default()),
        );
        *transport.result.lock().unwrap() = Err("offline");
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap();
        let manager = UpdateManager::new(service);
        let (sender, mut timers) = tokio::sync::mpsc::unbounded_channel();
        let timer = move |delay| {
            let (resume, wait) = tokio::sync::oneshot::channel();
            sender.send((delay, resume)).unwrap();
            async move {
                let _ = wait.await;
            }
        };
        manager.start_with_sleep(app.handle(), timer.clone());
        manager.start_with_sleep(app.handle(), timer);
        let (delay, resume) = boundary(&mut timers).await;
        assert_eq!(delay, Duration::from_secs(30));
        assert_eq!(transport.calls.load(Ordering::SeqCst), 0);
        assert!(
            timers.try_recv().is_err(),
            "duplicate start created a worker"
        );
        clock.0.fetch_add(delay.as_secs(), Ordering::SeqCst);
        resume.send(()).unwrap();
        for (attempt, expected) in [60, 300, 1800, 60].into_iter().enumerate() {
            let (delay, resume) = boundary(&mut timers).await;
            assert_eq!(delay.as_secs(), expected);
            assert_eq!(transport.calls.load(Ordering::SeqCst), attempt + 1);
            if attempt == 3 {
                drop(manager);
                // Releasing the timer after shutdown must never fetch again.
                let _ = resume.send(());
                assert!(
                    tokio::time::timeout(Duration::from_secs(5), timers.recv())
                        .await
                        .unwrap()
                        .is_none()
                );
                assert_eq!(transport.calls.load(Ordering::SeqCst), 4);
                return;
            }
            clock.0.fetch_add(delay.as_secs(), Ordering::SeqCst);
            resume.send(()).unwrap();
        }
    }

    #[tokio::test]
    async fn production_scheduler_shutdown_ready_timer_cannot_begin_check() {
        struct WorkerDropped(Option<tokio::sync::oneshot::Sender<()>>);
        impl Drop for WorkerDropped {
            fn drop(&mut self) {
                let _ = self.0.take().unwrap().send(());
            }
        }
        let (service, transport) = service(
            serde_json::to_vec(&vec![public_release("2.0.0", false)]).unwrap(),
            Arc::new(FakeClock(AtomicU64::new(100))),
            Arc::new(MemoryPersistence::default()),
        );
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap();
        let manager = UpdateManager::new(service);
        let (entered, wait_entered) = tokio::sync::oneshot::channel();
        let entered = Mutex::new(Some(entered));
        let (release, wait_release) = std::sync::mpsc::channel();
        let wait_release = Mutex::new(wait_release);
        let (dropped, wait_dropped) = tokio::sync::oneshot::channel();
        let worker_dropped = WorkerDropped(Some(dropped));
        manager.start_with_sleep(app.handle(), move |_| {
            let _keep_alive = &worker_dropped;
            let first = entered.lock().unwrap().take();
            // Hold the worker inside its current poll while shutdown occurs.
            // Abort cannot interrupt that poll, and the timer then becomes ready.
            let is_first = first.is_some();
            if let Some(entered) = first {
                entered.send(()).unwrap();
                wait_release.lock().unwrap().recv().unwrap();
            }
            async move {
                if !is_first {
                    std::future::pending::<()>().await;
                }
            }
        });
        tokio::time::timeout(Duration::from_secs(5), wait_entered)
            .await
            .unwrap()
            .unwrap();
        drop(manager);
        release.send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(5), wait_dropped)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(transport.calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn production_scheduler_shutdown_cancels_in_flight_transport() {
        struct CancelOnDrop(Option<tokio::sync::oneshot::Sender<()>>);
        impl Drop for CancelOnDrop {
            fn drop(&mut self) {
                if let Some(sender) = self.0.take() {
                    let _ = sender.send(());
                }
            }
        }
        struct PendingTransport {
            entered: tokio::sync::Notify,
            cancelled: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
        }
        #[async_trait]
        impl ReleaseTransport for PendingTransport {
            async fn fetch(&self) -> Result<Vec<u8>, UpdateError> {
                let _guard = CancelOnDrop(self.cancelled.lock().unwrap().take());
                self.entered.notify_one();
                std::future::pending().await
            }
        }
        let (cancelled, wait) = tokio::sync::oneshot::channel();
        let transport = Arc::new(PendingTransport {
            entered: tokio::sync::Notify::new(),
            cancelled: Mutex::new(Some(cancelled)),
        });
        let persistence = Arc::new(MemoryPersistence::default());
        let service = Arc::new(
            UpdateService::new(
                distribution(),
                transport.clone(),
                Arc::new(FakeClock(AtomicU64::new(100))),
                persistence.clone(),
            )
            .unwrap(),
        );
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap();
        let manager = UpdateManager::new(service);
        manager.start_with_sleep(app.handle(), |_| async {});
        tokio::time::timeout(Duration::from_secs(5), transport.entered.notified())
            .await
            .unwrap();
        drop(manager);
        tokio::time::timeout(Duration::from_secs(5), wait)
            .await
            .unwrap()
            .unwrap();
        assert!(
            persistence
                .0
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .last_successful_check
                .is_none()
        );
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
            assert!(service.automatic_due()); // Legacy timestamps do not establish target-scoped freshness.
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
            Some("invalid release information: offline")
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
    #[test]
    fn production_requests_use_anonymous_release_api() {
        let transport = HttpReleaseTransport::new().unwrap();
        for policy in [UpdatePolicy::Stable, UpdatePolicy::Prereleases] {
            let request = transport.request(policy).build().unwrap();
            assert_eq!(request.url().host_str(), Some("api.github.com"));
            assert_eq!(
                request.url().path(),
                "/repos/MiguelTVMS/speaker-volume-bridge/releases"
            );
            assert!(
                !request
                    .headers()
                    .contains_key(reqwest::header::AUTHORIZATION)
            );
            assert!(request.headers().contains_key(reqwest::header::USER_AGENT));
        }
    }

    fn public_release(version: &str, prerelease: bool) -> serde_json::Value {
        serde_json::json!({
            "tag_name": format!("v{version}"), "draft": false, "prerelease": prerelease,
            "published_at": "2026-10-07T00:00:00Z", "html_url": release_page(version),
            "assets": [{"name": "speaker-volume-bridge-macos.dmg", "state": "uploaded", "size": 100,
                "browser_download_url": format!("https://github.com/MiguelTVMS/speaker-volume-bridge/releases/download/v{version}/speaker-volume-bridge-macos.dmg")}]
        })
    }

    #[tokio::test]
    async fn release_api_service_selects_policy_by_version_and_opens_exact_page() {
        let releases = serde_json::to_vec(&vec![
            public_release("1.6.0", false),
            public_release("1.8.1", true),
            public_release("1.8.0", false),
        ])
        .unwrap();
        let (client, _) = service(
            releases,
            Arc::new(FakeClock(AtomicU64::new(100))),
            Arc::new(MemoryPersistence::default()),
        );
        let stable = client.check(true).await;
        assert_eq!(stable.available_version.as_deref(), Some("1.8.0"));
        assert_eq!(
            client
                .claim_offer("1.8.0", &release_page("1.8.0"))
                .unwrap()
                .url,
            release_page("1.8.0")
        );
        client.set_policy(UpdatePolicy::Prereleases).unwrap();
        let preview = client.check(true).await;
        assert_eq!(preview.available_version.as_deref(), Some("1.8.1"));
        assert_eq!(preview.action.unwrap().url, release_page("1.8.1"));
    }

    #[tokio::test]
    async fn updater_artifacts_preserve_release_page_policy_and_do_not_create_install_offers() {
        let mut stable = public_release("1.8.0", false);
        let mut preview = public_release("1.8.1", true);
        for release in [&mut stable, &mut preview] {
            let version = release["tag_name"].as_str().unwrap().to_owned();
            for name in [
                "speaker-volume-bridge-macos-aarch64.app.tar.gz",
                "speaker-volume-bridge-macos-aarch64.app.tar.gz.sig",
                "speaker-volume-bridge-macos-aarch64.updater.json",
            ] {
                release["assets"].as_array_mut().unwrap().push(serde_json::json!({
                    "name": name, "state": "uploaded", "size": 100,
                    "browser_download_url": format!(
                        "https://github.com/MiguelTVMS/speaker-volume-bridge/releases/download/{version}/{name}"
                    )
                }));
            }
        }
        let (client, transport) = service(
            serde_json::to_vec(&vec![stable, preview.clone()]).unwrap(),
            Arc::new(FakeClock(AtomicU64::new(100))),
            Arc::new(MemoryPersistence::default()),
        );
        assert_eq!(
            client.check(true).await.available_version.as_deref(),
            Some("1.8.0")
        );
        client.set_policy(UpdatePolicy::Prereleases).unwrap();
        let offer = client.check(true).await;
        assert_eq!(offer.available_version.as_deref(), Some("1.8.1"));
        assert_eq!(offer.action.unwrap().url, release_page("1.8.1"));
        assert!(client.claim_offer("1.8.1", &release_page("1.8.1")).is_ok());
        client.finish_open();
        client.set_policy(UpdatePolicy::Stable).unwrap();
        client.set_policy(UpdatePolicy::Prereleases).unwrap();
        preview["assets"].as_array_mut().unwrap().remove(0);
        *transport.result.lock().unwrap() = Ok(serde_json::to_vec(&vec![preview]).unwrap());
        assert!(client.check(true).await.available_version.is_none());
    }

    #[tokio::test]
    async fn release_api_rejects_ineligible_packages_and_preserves_saved_policy() {
        let mut draft = public_release("9.0.0", false);
        draft["draft"] = true.into();
        let mut missing = public_release("8.0.0", false);
        missing["assets"] = serde_json::json!([]);
        let mut foreign = public_release("7.0.0", false);
        foreign["html_url"] = "https://github.com/other/repo/releases/tag/v7.0.0".into();
        let mut incomplete = public_release("6.0.0", false);
        incomplete["assets"][0]["state"] = "new".into();
        let persistence = Arc::new(MemoryPersistence::default());
        let (client, transport) = service(
            serde_json::to_vec(&vec![
                draft,
                missing,
                foreign,
                incomplete,
                public_release("1.8.1", true),
                public_release("1.8.0", false),
            ])
            .unwrap(),
            Arc::new(FakeClock(AtomicU64::new(100))),
            persistence.clone(),
        );
        client.set_policy(UpdatePolicy::Prereleases).unwrap();
        assert_eq!(
            client.check(true).await.available_version.as_deref(),
            Some("1.8.1")
        );
        let restored = UpdateService::new(
            distribution(),
            transport.clone(),
            Arc::new(FakeClock(AtomicU64::new(101))),
            persistence,
        )
        .unwrap();
        assert_eq!(restored.status().unwrap().policy, UpdatePolicy::Prereleases);
        assert_eq!(
            restored.status().unwrap().available_version.as_deref(),
            Some("1.8.1")
        );
        *transport.result.lock().unwrap() = Err("rate limited");
        let failure = restored.check(true).await;
        assert!(failure.check_failed);
        assert_eq!(failure.available_version.as_deref(), Some("1.8.1"));
        restored.set_policy(UpdatePolicy::Stable).unwrap();
        *transport.result.lock().unwrap() =
            Ok(serde_json::to_vec(&vec![public_release("1.7.1", false)]).unwrap());
        assert_eq!(restored.check(true).await.phase, UpdatePhase::UpToDate);
    }

    #[test]
    fn release_api_source_migration_discards_old_catalog_freshness() {
        let persistence = Arc::new(MemoryPersistence::default());
        let old = UpdatePreferences {
            automatic_checks: false,
            update_notifications: false,
            policy: UpdatePolicy::Prereleases,
            update_source: None,
            last_successful_check: Some(100),
            last_attempted_check: Some(100),
            freshness_target: Some(policy_target(&distribution(), UpdatePolicy::Prereleases)),
            notified_targets: vec!["previous-notice".into()],
            ..UpdatePreferences::default()
        };
        *persistence.0.lock().unwrap() = Some(old);
        let (client, _) = service(
            serde_json::to_vec(&vec![public_release("1.8.1", true)]).unwrap(),
            Arc::new(FakeClock(AtomicU64::new(101))),
            persistence,
        );
        let preferences = client.preferences().unwrap();
        assert_eq!(preferences.policy, UpdatePolicy::Prereleases);
        assert!(!preferences.automatic_checks);
        assert!(!preferences.update_notifications);
        assert!(preferences.last_successful_check.is_none());
        assert!(preferences.freshness_target.is_none());
        assert_eq!(preferences.notified_targets, ["previous-notice"]);
    }

    #[test]
    fn release_api_rate_limit_respects_reset_and_retry_after() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("x-ratelimit-remaining", "0".parse().unwrap());
        headers.insert("x-ratelimit-reset", "1000".parse().unwrap());
        assert_eq!(
            rate_limit_deadline(reqwest::StatusCode::FORBIDDEN, &headers, 100),
            Some(1000)
        );
        headers.insert("retry-after", "1200".parse().unwrap());
        assert_eq!(
            rate_limit_deadline(reqwest::StatusCode::TOO_MANY_REQUESTS, &headers, 100),
            Some(1300)
        );
        assert_eq!(
            rate_limit_deadline(reqwest::StatusCode::OK, &headers, 100),
            None
        );
    }

    #[tokio::test]
    async fn release_api_production_transport_paginates_before_service_selection() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            for (page, data) in [
                (1, vec![public_release("1.6.0", false); 100]),
                (
                    2,
                    vec![
                        public_release("1.8.1", true),
                        public_release("1.8.0", false),
                    ],
                ),
            ] {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut request = Vec::new();
                while !request.ends_with(b"\r\n\r\n") {
                    let mut byte = [0];
                    socket.read_exact(&mut byte).await.unwrap();
                    request.push(byte[0]);
                }
                let request = String::from_utf8(request).unwrap().to_lowercase();
                assert!(request.contains(&format!("per_page=100&page={page}")));
                assert!(request.contains("user-agent: speaker-volume-bridge"));
                assert!(!request.contains("authorization:"));
                let body = serde_json::to_vec(&data).unwrap();
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                socket.write_all(header.as_bytes()).await.unwrap();
                socket.write_all(&body).await.unwrap();
            }
        });
        let mut transport = HttpReleaseTransport::new().unwrap();
        transport.endpoint = format!("http://{address}/releases");
        let client = UpdateService::new(
            distribution(),
            Arc::new(transport),
            Arc::new(FakeClock(AtomicU64::new(100))),
            Arc::new(MemoryPersistence::default()),
        )
        .unwrap();
        assert_eq!(
            client.check(true).await.available_version.as_deref(),
            Some("1.8.0")
        );
        server.await.unwrap();
    }

    #[tokio::test]
    async fn release_api_production_transport_does_not_repeat_rate_limited_requests() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                socket.read_exact(&mut byte).await.unwrap();
                request.push(byte[0]);
            }
            socket.write_all(b"HTTP/1.1 429 Too Many Requests\r\nRetry-After: 120\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}").await.unwrap();
        });
        let mut transport = HttpReleaseTransport::new().unwrap();
        transport.endpoint = format!("http://{address}/releases");
        assert!(matches!(
            transport.fetch().await,
            Err(UpdateError::RateLimited)
        ));
        server.await.unwrap();
        // The server no longer exists. A second network request would fail differently.
        assert!(matches!(
            transport.fetch().await,
            Err(UpdateError::RateLimited)
        ));
    }

    #[tokio::test]
    async fn release_api_production_transport_cools_down_headerless_secondary_limit() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                socket.read_exact(&mut byte).await.unwrap();
                request.push(byte[0]);
            }
            socket.write_all(b"HTTP/1.1 403 Forbidden\r\nX-RateLimit-Remaining: 100\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}").await.unwrap();
        });
        let mut transport = HttpReleaseTransport::new().unwrap();
        transport.endpoint = format!("http://{address}/releases");
        assert!(matches!(
            transport.fetch().await,
            Err(UpdateError::RateLimited)
        ));
        server.await.unwrap();
        assert!(transport.retry_after.load(Ordering::SeqCst) >= SystemUpdateClock.now() + 59);
        // The server no longer exists. A second network request would fail differently.
        assert!(matches!(
            transport.fetch().await,
            Err(UpdateError::RateLimited)
        ));
    }

    #[test]
    fn release_api_never_infers_store_or_intel_macos_availability() {
        for (edition, architecture) in [
            (
                DistributionEdition::MicrosoftStore,
                ApplicationArchitecture::Aarch64,
            ),
            (
                DistributionEdition::MacAppStore,
                ApplicationArchitecture::Aarch64,
            ),
            (
                DistributionEdition::DirectMacos,
                ApplicationArchitecture::X86_64,
            ),
        ] {
            let mut installed = distribution();
            installed.edition = edition;
            installed.architecture = architecture;
            assert!(release_asset_name(&installed).is_none());
        }
    }

    #[tokio::test]
    async fn verified_backfill_uses_production_client_for_version_policy_and_target_selection() {
        let stable = include_bytes!("../../tests/fixtures/update-catalog/backfill-v1.json");
        let preview = include_bytes!("../../tests/fixtures/update-catalog/backfill-v2.json");
        for (edition, architecture, version, policy, expected) in [
            (
                DistributionEdition::DirectMacos,
                ApplicationArchitecture::Aarch64,
                "1.8.0",
                UpdatePolicy::Stable,
                UpdatePhase::UpToDate,
            ),
            (
                DistributionEdition::DirectMacos,
                ApplicationArchitecture::Aarch64,
                "1.7.1",
                UpdatePolicy::Stable,
                UpdatePhase::UpdateAvailable,
            ),
            (
                DistributionEdition::DirectWindows,
                ApplicationArchitecture::X86_64,
                "1.7.1",
                UpdatePolicy::Prereleases,
                UpdatePhase::UpdateAvailable,
            ),
            (
                DistributionEdition::Debian,
                ApplicationArchitecture::Aarch64,
                "1.8.0",
                UpdatePolicy::Prereleases,
                UpdatePhase::UpToDate,
            ),
            (
                DistributionEdition::DirectMacos,
                ApplicationArchitecture::X86_64,
                "1.7.1",
                UpdatePolicy::Stable,
                UpdatePhase::Unavailable,
            ),
            (
                DistributionEdition::MacAppStore,
                ApplicationArchitecture::Aarch64,
                "1.7.1",
                UpdatePolicy::Stable,
                UpdatePhase::StoreManaged,
            ),
        ] {
            let mut package = distribution();
            package.edition = edition;
            package.architecture = architecture;
            package.version = version.into();
            let transport = Arc::new(FakeTransport {
                result: Mutex::new(Ok(if policy == UpdatePolicy::Stable {
                    stable.to_vec()
                } else {
                    preview.to_vec()
                })),
                calls: AtomicUsize::new(0),
            });
            let client = UpdateService::new(
                package,
                transport.clone(),
                Arc::new(FakeClock(AtomicU64::new(100))),
                Arc::new(MemoryPersistence::default()),
            )
            .unwrap();
            if policy == UpdatePolicy::Prereleases {
                client.set_policy(policy).unwrap();
            }
            let status = client.check(true).await;
            assert_eq!(
                status.phase, expected,
                "{edition:?}/{architecture:?}/{version}/{policy:?}"
            );
            assert_eq!(
                transport.calls.load(Ordering::Relaxed),
                usize::from(expected != UpdatePhase::StoreManaged)
            );
            if expected == UpdatePhase::UpdateAvailable {
                assert_eq!(status.available_version.as_deref(), Some("1.8.0"));
                let action = status.action.unwrap();
                assert_eq!(
                    action.url,
                    if policy == UpdatePolicy::Stable {
                        "https://svb.miguel.ms/guide/Upgrading.html".to_owned()
                    } else {
                        release_page("1.8.0")
                    }
                );
            }
        }
    }

    #[tokio::test]
    async fn verified_backfill_retains_preview_only_beta_for_the_production_client() {
        let preview = include_bytes!("../../tests/fixtures/update-catalog/backfill-v2.json");
        // The prerelease-inclusive backfill retains the verified Beta even
        // though the newer GA currently wins semantic precedence.
        let mut beta: serde_json::Value = serde_json::from_slice(preview).unwrap();
        beta["entries"]
            .as_array_mut()
            .unwrap()
            .retain(|entry| entry["classification"] == "Beta");
        let (client, _) = service(
            serde_json::to_vec(&beta).unwrap(),
            Arc::new(FakeClock(AtomicU64::new(100))),
            Arc::new(MemoryPersistence::default()),
        );
        client.set_policy(UpdatePolicy::Prereleases).unwrap();
        assert_eq!(
            client.check(true).await.available_version.as_deref(),
            Some("1.7.4")
        );
    }

    fn preview_fixture() -> Vec<u8> {
        include_bytes!("../../tests/fixtures/update-catalog/release-policy.json").to_vec()
    }

    #[tokio::test]
    async fn policy_fixture_selects_newer_ga_and_numeric_previews_without_downgrades() {
        let (service, transport) = service(
            preview_fixture(),
            Arc::new(FakeClock(AtomicU64::new(100))),
            Arc::new(MemoryPersistence::default()),
        );
        service.set_automatic_checks(false).unwrap();
        service.set_update_notifications(false).unwrap();
        service.set_policy(UpdatePolicy::Prereleases).unwrap();
        let status = service.check(true).await;
        assert_eq!(status.available_version.as_deref(), Some("1.10.0"));
        assert_eq!(status.action.unwrap().url, release_page("1.10.0"));
        assert!(!status.automatic_checks);
        assert!(!status.update_notifications);
        let mut fixture: serde_json::Value = serde_json::from_slice(&preview_fixture()).unwrap();
        fixture["entries"].as_array_mut().unwrap().remove(0);
        *transport.result.lock().unwrap() = Ok(serde_json::to_vec(&fixture).unwrap());
        assert_eq!(
            service.check(true).await.available_version.as_deref(),
            Some("1.9.0")
        );
        service.set_policy(UpdatePolicy::Stable).unwrap();
        *transport.result.lock().unwrap() = Ok(catalog("1.7.1"));
        assert_eq!(service.check(true).await.phase, UpdatePhase::UpToDate);
        assert!(service.status().unwrap().action.is_none());
    }

    #[tokio::test]
    async fn policy_defaults_restart_and_edition_changes_preserve_unrelated_preferences() {
        let persistence = Arc::new(MemoryPersistence::default());
        let (service, _) = service(
            preview_fixture(),
            Arc::new(FakeClock(AtomicU64::new(100))),
            persistence.clone(),
        );
        assert_eq!(service.preferences().unwrap().policy, UpdatePolicy::Stable);
        service.set_automatic_checks(false).unwrap();
        service.set_policy(UpdatePolicy::Prereleases).unwrap();
        service.check(true).await;
        let transport = Arc::new(FakeTransport {
            result: Mutex::new(Ok(preview_fixture())),
            calls: AtomicUsize::new(0),
        });
        let restarted = UpdateService::new(
            distribution(),
            transport.clone(),
            Arc::new(FakeClock(AtomicU64::new(100))),
            persistence.clone(),
        )
        .unwrap();
        assert_eq!(
            restarted.status().unwrap().policy,
            UpdatePolicy::Prereleases
        );
        assert_eq!(
            restarted.status().unwrap().available_version.as_deref(),
            Some("1.10.0")
        );
        let mut changed = distribution();
        changed.edition = DistributionEdition::MacAppStore;
        let changed = UpdateService::new(
            changed,
            transport,
            Arc::new(FakeClock(AtomicU64::new(100))),
            persistence,
        )
        .unwrap();
        assert_eq!(changed.status().unwrap().policy, UpdatePolicy::Stable);
        assert!(changed.status().unwrap().action.is_none());
        assert!(!changed.status().unwrap().automatic_checks);
        assert!(changed.set_policy(UpdatePolicy::Prereleases).is_err());
        let legacy: UpdatePreferences =
            serde_json::from_str(r#"{"automaticChecks":false,"updateNotifications":false}"#)
                .unwrap();
        assert_eq!(legacy.policy, UpdatePolicy::Stable);
        assert!(!legacy.update_notifications);
    }

    #[test]
    fn policy_capability_matrix_is_enforced_by_backend() {
        for edition in [
            DistributionEdition::DirectMacos,
            DistributionEdition::DirectWindows,
            DistributionEdition::Debian,
            DistributionEdition::MicrosoftStore,
            DistributionEdition::MacAppStore,
            DistributionEdition::WindowsSideload,
            DistributionEdition::Unknown,
            DistributionEdition::Development,
        ] {
            for architecture in [
                ApplicationArchitecture::Aarch64,
                ApplicationArchitecture::X86_64,
                ApplicationArchitecture::Unknown,
            ] {
                let mut package = distribution();
                package.edition = edition;
                package.architecture = architecture;
                let expected = matches!(
                    edition,
                    DistributionEdition::DirectMacos
                        | DistributionEdition::DirectWindows
                        | DistributionEdition::Debian
                ) && architecture != ApplicationArchitecture::Unknown;
                let service = UpdateService::new(
                    package,
                    Arc::new(FakeTransport {
                        result: Mutex::new(Ok(catalog("2.0.0"))),
                        calls: AtomicUsize::new(0),
                    }),
                    Arc::new(FakeClock(AtomicU64::new(100))),
                    Arc::new(MemoryPersistence::default()),
                )
                .unwrap();
                assert_eq!(service.status().unwrap().prerelease_supported, expected);
                assert_eq!(
                    service.set_policy(UpdatePolicy::Prereleases).is_ok(),
                    expected
                );
            }
        }
    }

    struct GatedPolicyTransport {
        entered: tokio::sync::Notify,
        release: tokio::sync::Notify,
        calls: AtomicUsize,
    }
    #[async_trait]
    impl ReleaseTransport for GatedPolicyTransport {
        async fn fetch(&self) -> Result<Vec<u8>, UpdateError> {
            self.fetch_policy(UpdatePolicy::Stable).await
        }
        async fn fetch_policy(&self, policy: UpdatePolicy) -> Result<Vec<u8>, UpdateError> {
            if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
                self.entered.notify_one();
                self.release.notified().await;
            }
            Ok(if policy == UpdatePolicy::Stable {
                catalog("1.10.0")
            } else {
                preview_fixture()
            })
        }
    }

    #[tokio::test]
    async fn policy_switch_regression_obsolete_response_cannot_restore_cache_or_notify() {
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap();
        let transport = Arc::new(GatedPolicyTransport {
            entered: tokio::sync::Notify::new(),
            release: tokio::sync::Notify::new(),
            calls: AtomicUsize::new(0),
        });
        let persistence = Arc::new(MemoryPersistence::default());
        let service = Arc::new(
            UpdateService::new(
                distribution(),
                transport.clone(),
                Arc::new(FakeClock(AtomicU64::new(100))),
                persistence.clone(),
            )
            .unwrap(),
        );
        let notices = Arc::new(AtomicUsize::new(0));
        let old = {
            let app = app.handle().clone();
            let service = service.clone();
            let notices = notices.clone();
            tokio::spawn(async move {
                run_check_and_deliver_with(
                    &app,
                    &service,
                    true,
                    || async { true },
                    move |_| async move {
                        notices.fetch_add(1, Ordering::SeqCst);
                    },
                )
                .await
            })
        };
        transport.entered.notified().await;
        service.set_policy(UpdatePolicy::Prereleases).unwrap();
        service.set_policy(UpdatePolicy::Stable).unwrap();
        service.set_policy(UpdatePolicy::Prereleases).unwrap();
        assert!(service.status().unwrap().action.is_none());
        let fresh = {
            let service = service.clone();
            tokio::spawn(async move { service.check(true).await })
        };
        tokio::task::yield_now().await;
        transport.release.notify_one();
        old.await.unwrap();
        let current = fresh.await.unwrap();
        assert_eq!(current.policy, UpdatePolicy::Prereleases);
        assert_eq!(current.available_version.as_deref(), Some("1.10.0"));
        assert_eq!(notices.load(Ordering::SeqCst), 0);
        assert_eq!(
            persistence
                .0
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .cached_offer
                .as_ref()
                .unwrap()
                .target
                .as_ref()
                .unwrap()
                .channel,
            "prereleases"
        );
        assert!(
            service
                .claim_offer_generation("1.10.0", &release_page("1.10.0"), 0)
                .is_err()
        );
        assert!(service.claim_notification(&current).unwrap());
        service.set_policy(UpdatePolicy::Stable).unwrap();
        let stable = service.check(true).await;
        assert!(!service.claim_notification(&stable).unwrap());
    }

    #[tokio::test]
    async fn policy_switch_regression_permission_wait_cannot_deliver_old_notice() {
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap();
        let (service, _) = service(
            catalog("2.0.0"),
            Arc::new(FakeClock(AtomicU64::new(100))),
            Arc::new(MemoryPersistence::default()),
        );
        let entered = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        let notices = Arc::new(AtomicUsize::new(0));
        let task = {
            let app = app.handle().clone();
            let service = service.clone();
            let entered = entered.clone();
            let release = release.clone();
            let notices = notices.clone();
            tokio::spawn(async move {
                run_check_and_deliver_with(
                    &app,
                    &service,
                    true,
                    || async {
                        entered.notify_one();
                        release.notified().await;
                        true
                    },
                    move |_| async move {
                        notices.fetch_add(1, Ordering::SeqCst);
                    },
                )
                .await
            })
        };
        entered.notified().await;
        service.set_policy(UpdatePolicy::Prereleases).unwrap();
        release.notify_one();
        task.await.unwrap();
        assert_eq!(notices.load(Ordering::SeqCst), 0);
        assert!(
            service
                .preferences()
                .unwrap()
                .last_notified_target
                .is_none()
        );
    }

    #[tokio::test]
    async fn policy_missing_empty_invalid_feed_and_exact_target_boundaries() {
        let (service, transport) = service(
            preview_fixture(),
            Arc::new(FakeClock(AtomicU64::new(100))),
            Arc::new(MemoryPersistence::default()),
        );
        service.set_policy(UpdatePolicy::Prereleases).unwrap();
        for entries in [
            serde_json::json!([]),
            serde_json::json!([{"edition":"microsoft_store"}]),
        ] {
            *transport.result.lock().unwrap() = Ok(serde_json::to_vec(&serde_json::json!({"schemaVersion":2,"generatedAt":"2026-10-04T00:00:00Z","entries":entries})).unwrap());
            assert_eq!(service.check(true).await.phase, UpdatePhase::Unavailable);
        }
        *transport.result.lock().unwrap() = Err("missing preview feed");
        assert_eq!(service.check(true).await.phase, UpdatePhase::Unavailable);
        let mut fixture: serde_json::Value = serde_json::from_slice(&preview_fixture()).unwrap();
        for entry in fixture["entries"].as_array_mut().unwrap() {
            entry["architecture"] = "x86_64".into();
        }
        *transport.result.lock().unwrap() = Ok(serde_json::to_vec(&fixture).unwrap());
        assert_eq!(service.check(true).await.phase, UpdatePhase::Unavailable);
    }
    #[tokio::test]
    async fn policy_semantic_ordering_promotion_and_return_to_stable_use_running_version() {
        for (installed, candidate, classification, expected) in [
            ("1.9.0", "1.10.0", "Alpha", UpdatePhase::UpdateAvailable),
            (
                "1.10.0-beta.2",
                "1.10.0-beta.10",
                "Beta",
                UpdatePhase::UpdateAvailable,
            ),
            (
                "1.10.0-beta.10",
                "1.10.0",
                "GA",
                UpdatePhase::UpdateAvailable,
            ),
            ("1.10.0", "1.10.0", "GA", UpdatePhase::UpToDate),
            ("1.10.0", "1.9.0", "GA", UpdatePhase::UpToDate),
            ("1.10.0+old", "1.10.0+new", "GA", UpdatePhase::UpToDate),
        ] {
            let mut fixture: serde_json::Value =
                serde_json::from_slice(&preview_fixture()).unwrap();
            fixture["entries"] = serde_json::json!([fixture["entries"][0].clone()]);
            fixture["entries"][0]["version"] = candidate.into();
            fixture["entries"][0]["classification"] = classification.into();
            fixture["entries"][0]["channel"] = if classification == "GA" {
                "stable"
            } else {
                "prereleases"
            }
            .into();
            fixture["entries"][0]["action"]["url"] = release_page(candidate).into();
            let mut package = distribution();
            package.version = installed.into();
            let service = UpdateService::new(
                package,
                Arc::new(FakeTransport {
                    result: Mutex::new(Ok(serde_json::to_vec(&fixture).unwrap())),
                    calls: AtomicUsize::new(0),
                }),
                Arc::new(FakeClock(AtomicU64::new(100))),
                Arc::new(MemoryPersistence::default()),
            )
            .unwrap();
            service.set_policy(UpdatePolicy::Prereleases).unwrap();
            assert_eq!(
                service.check(true).await.phase,
                expected,
                "{installed} -> {candidate}"
            );
        }
    }

    #[tokio::test]
    async fn policy_concurrent_same_second_manual_background_triggers_coalesce() {
        let transport = Arc::new(GatedPolicyTransport {
            entered: tokio::sync::Notify::new(),
            release: tokio::sync::Notify::new(),
            calls: AtomicUsize::new(0),
        });
        let service = Arc::new(
            UpdateService::new(
                distribution(),
                transport.clone(),
                Arc::new(FakeClock(AtomicU64::new(100))),
                Arc::new(MemoryPersistence::default()),
            )
            .unwrap(),
        );
        let first = {
            let service = service.clone();
            tokio::spawn(async move { service.check(true).await })
        };
        transport.entered.notified().await;
        let second = {
            let service = service.clone();
            tokio::spawn(async move { service.check(true).await })
        };
        let background = {
            let service = service.clone();
            tokio::spawn(async move { service.check(false).await })
        };
        tokio::task::yield_now().await;
        transport.release.notify_one();
        assert_eq!(first.await.unwrap().phase, UpdatePhase::UpdateAvailable);
        assert_eq!(second.await.unwrap().phase, UpdatePhase::UpdateAvailable);
        assert_eq!(
            background.await.unwrap().phase,
            UpdatePhase::UpdateAvailable
        );
        assert_eq!(transport.calls.load(Ordering::SeqCst), 1);
    }
    #[tokio::test]
    async fn policy_notification_history_survives_intervening_preview_and_restart() {
        let persistence = Arc::new(MemoryPersistence::default());
        let (service, transport) = service(
            catalog("1.10.0"),
            Arc::new(FakeClock(AtomicU64::new(100))),
            persistence.clone(),
        );
        assert!(
            service
                .claim_notification(&service.check(true).await)
                .unwrap()
        );
        service.set_policy(UpdatePolicy::Prereleases).unwrap();
        let mut fixture: serde_json::Value = serde_json::from_slice(&preview_fixture()).unwrap();
        fixture["entries"][2]["version"] = "1.11.0".into();
        fixture["entries"][2]["action"]["url"] = release_page("1.11.0").into();
        *transport.result.lock().unwrap() = Ok(serde_json::to_vec(&fixture).unwrap());
        assert!(
            service
                .claim_notification(&service.check(true).await)
                .unwrap()
        );
        service.set_policy(UpdatePolicy::Stable).unwrap();
        *transport.result.lock().unwrap() = Ok(catalog("1.10.0"));
        assert!(
            !service
                .claim_notification(&service.check(true).await)
                .unwrap()
        );
        let restarted = UpdateService::new(
            distribution(),
            transport,
            Arc::new(FakeClock(AtomicU64::new(100))),
            persistence,
        )
        .unwrap();
        assert!(
            !restarted
                .claim_notification(&restarted.status().unwrap())
                .unwrap()
        );
    }
}
