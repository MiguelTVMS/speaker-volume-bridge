//! Opt-in packaged native probe. No frontend command or production install capability.
use crate::{
    config::AppConfiguration,
    distribution::{ApplicationArchitecture, DistributionEdition, InstalledDistribution},
    state::AppState,
    updates::{ReleaseTransport, UpdateError, UpdateManager},
};
use serde::{Deserialize, Serialize};
use std::{io::Read, path::Path, sync::Arc, time::Duration};
use tauri::Manager;
use tauri_plugin_updater::UpdaterExt;

const LOOPBACK: &str = "http://127.0.0.1:8765";

fn native_builder<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> tauri_plugin_updater::UpdaterBuilder {
    app.updater_builder()
        // Pin the manifest key as well as the target reported by the plugin.
        // Its default reported target is only the OS ("darwin").
        .target("darwin-aarch64")
        .no_proxy()
        .timeout(Duration::from_secs(20))
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Spec {
    version: String,
    pubkey: String,
    prerelease: bool,
}

fn load_spec(path: &Path) -> Result<Spec, Box<dyn std::error::Error>> {
    let spec: Spec = serde_json::from_slice(&std::fs::read(path)?)?;
    semver::Version::parse(&spec.version)?;
    if spec.pubkey.is_empty() {
        return Err("Missing acceptance key".into());
    }
    Ok(spec)
}

pub fn configure(
    context: &mut tauri::Context<tauri::Wry>,
) -> Result<(), Box<dyn std::error::Error>> {
    let executable = std::env::current_exe()?;
    let resources = executable
        .parent()
        .ok_or("Executable directory")?
        .join("../Resources");
    let spec = load_spec(&resources.join("native-updater-acceptance.json"))?;
    // Insecure transport is confined to a compiled probe and fixed loopback server.
    context.config_mut().plugins.0.insert(
        "updater".into(),
        serde_json::json!({
            "pubkey": spec.pubkey, "endpoints": [format!("{LOOPBACK}/manifest.json")],
            "dangerousInsecureTransportProtocol": true
        }),
    );
    Ok(())
}

struct FixtureTransport(Spec);
#[async_trait::async_trait]
impl ReleaseTransport for FixtureTransport {
    async fn fetch(&self) -> Result<Vec<u8>, UpdateError> {
        let version = &self.0.version;
        let page =
            format!("https://github.com/MiguelTVMS/speaker-volume-bridge/releases/tag/v{version}");
        Ok(serde_json::to_vec(&serde_json::json!([{
            "tag_name": format!("v{version}"), "draft": false, "prerelease": self.0.prerelease,
            "published_at": "2026-10-08T00:00:00Z", "html_url": page,
            "assets": [{"name": "speaker-volume-bridge-macos.dmg", "state": "uploaded", "size": 1,
                "browser_download_url": format!("https://github.com/MiguelTVMS/speaker-volume-bridge/releases/download/v{version}/speaker-volume-bridge-macos.dmg")}]
        }]))?)
    }
}

pub fn transport(app: &tauri::AppHandle) -> Result<Arc<dyn ReleaseTransport>, UpdateError> {
    let path = app.path().resource_dir().map_err(|_| UpdateError::State)?;
    let spec = load_spec(&path.join("native-updater-acceptance.json"))
        .map_err(|_| UpdateError::InvalidRelease("Invalid acceptance fixture"))?;
    Ok(Arc::new(FixtureTransport(spec)))
}

fn eligible(distribution: &InstalledDistribution, candidate: &str) -> Result<(), String> {
    if distribution.edition != DistributionEdition::DirectMacos
        || distribution.architecture != ApplicationArchitecture::Aarch64
        || !distribution.check_supported
    {
        return Err("Acceptance requires a recognized direct ARM64 package".into());
    }
    let installed = semver::Version::parse(&distribution.version).map_err(|e| e.to_string())?;
    let candidate = semver::Version::parse(candidate).map_err(|e| e.to_string())?;
    if candidate <= installed {
        return Err("Acceptance candidate must be strictly newer".into());
    }
    Ok(())
}

fn validate_payload(bytes: &[u8], version: &str, identifier: &str) -> Result<(), String> {
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(bytes));
    let mut found = false;
    for entry in archive.entries().map_err(|e| e.to_string())? {
        let mut entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path().map_err(|e| e.to_string())?.into_owned();
        if path != Path::new("Speaker Volume Bridge.app/Contents/Info.plist") {
            continue;
        }
        if found || !entry.header().entry_type().is_file() || entry.size() > 1024 * 1024 {
            return Err("Invalid signed bundle metadata".into());
        }
        let mut data = Vec::new();
        entry.read_to_end(&mut data).map_err(|e| e.to_string())?;
        let info =
            plist::Value::from_reader(std::io::Cursor::new(data)).map_err(|e| e.to_string())?;
        let info = info.as_dictionary().ok_or("Invalid bundle plist")?;
        if info
            .get("CFBundleIdentifier")
            .and_then(plist::Value::as_string)
            != Some(identifier)
            || info
                .get("CFBundleShortVersionString")
                .and_then(plist::Value::as_string)
                != Some(version)
        {
            return Err(
                "Signed bundle identity or version does not match the selected release".into(),
            );
        }
        found = true;
    }
    if !found {
        return Err("Signed payload lacks bundle metadata".into());
    }
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Report {
    stage: String,
    version: String,
    settings_preserved: bool,
    update_preferences_preserved: bool,
    runtime_status: String,
    login_preference: bool,
    error: Option<String>,
}

async fn report(app: &tauri::AppHandle, phase: &str, error: Option<String>) {
    let state = app.state::<AppState>();
    let configuration = state.store.load_or_default().ok();
    let baseline = state
        .store
        .path()
        .with_file_name("native-updater-acceptance-baseline.json");
    let settings_preserved = configuration.as_ref().is_some_and(|configuration| {
        std::fs::read(&baseline)
            .ok()
            .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
            == serde_json::to_value(configuration).ok()
    });
    let update_preferences_preserved = app
        .state::<UpdateManager>()
        .service()
        .preferences()
        .is_ok_and(|preferences| {
            !preferences.automatic_checks
                && !preferences.update_notifications
                && preferences.policy == crate::updates::UpdatePolicy::Prereleases
        });
    let runtime_status = state.snapshot.lock().map_or_else(
        |_| "unavailable".into(),
        |s| serde_json::to_string(&s.status).unwrap_or_default(),
    );
    let report = Report {
        stage: phase.into(),
        version: app.package_info().version.to_string(),
        settings_preserved,
        update_preferences_preserved,
        runtime_status,
        login_preference: configuration.is_some_and(|c| c.start_at_login),
        error,
    };
    // Private native evidence stays in the actual application container and loopback runner.
    if let Ok(bytes) = serde_json::to_vec_pretty(&report) {
        let _ = std::fs::write(
            state
                .store
                .path()
                .with_file_name("native-updater-acceptance-result.json"),
            bytes,
        );
    }
    if let Ok(client) = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(10))
        .build()
    {
        let _ = client
            .post(format!("{LOOPBACK}/result"))
            .body(serde_json::to_vec(&report).unwrap_or_default())
            .send()
            .await;
    }
}

async fn seed(app: &tauri::AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let baseline = state
        .store
        .path()
        .with_file_name("native-updater-acceptance-baseline.json");
    if state.store.path().exists() || baseline.exists() {
        return Err("Seed refuses an existing settings container; use a disposable account".into());
    }
    let mut configuration = AppConfiguration {
        synchronize_mute: false,
        mute_speaker_at_zero_volume: true,
        two_way_synchronization: false,
        fallback_polling: false,
        ..AppConfiguration::default()
    };
    configuration.night_mode_schedule.blocks[0][0] = true;
    configuration.night_mode_schedule.blocks[6][47] = true;
    let service = app.state::<UpdateManager>().service().clone();
    service
        .set_policy(crate::updates::UpdatePolicy::Prereleases)
        .map_err(|e| e.to_string())?;
    service
        .set_automatic_checks(false)
        .map_err(|e| e.to_string())?;
    service
        .set_update_notifications(false)
        .map_err(|e| e.to_string())?;
    state
        .store
        .save(&configuration)
        .map_err(|e| e.to_string())?;
    state.replace_configuration(configuration.clone());
    std::fs::write(
        &baseline,
        serde_json::to_vec_pretty(&configuration).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    report(app, "baseline", None).await;
    state.stop_runtime();
    app.exit(0);
    Ok(())
}

async fn run(app: &tauri::AppHandle, mode: &str) -> Result<(), String> {
    let state = app.state::<AppState>();
    let baseline = state
        .store
        .path()
        .with_file_name("native-updater-acceptance-baseline.json");
    if mode == "seed" {
        return seed(app).await;
    }
    if !baseline.exists() {
        return Err("Acceptance requires a seeded disposable container".into());
    }
    let spec = load_spec(
        &app.path()
            .resource_dir()
            .map_err(|e| e.to_string())?
            .join("native-updater-acceptance.json"),
    )
    .map_err(|e| e.to_string())?;
    if app.package_info().version.to_string() == spec.version || mode == "snapshot" {
        report(app, "snapshot", None).await;
        return Ok(());
    }
    eligible(&app.state::<InstalledDistribution>(), &spec.version)?;
    let service = app.state::<UpdateManager>().service().clone();
    let offer = service.check(true).await;
    let action = offer
        .action
        .as_ref()
        .ok_or("Shared policy did not select a release")?;
    if offer.available_version.as_deref() != Some(&spec.version) {
        return Err("Shared policy selected a different release".into());
    }
    let update = native_builder(app)
        .build()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| e.to_string())?
        .ok_or("No native update")?;
    if update.version != spec.version
        || update.target != "darwin-aarch64"
        || update.download_url.as_str() != format!("{LOOPBACK}/payload.app.tar.gz")
    {
        return Err("Native manifest does not match the fixed acceptance target".into());
    }
    let bytes = update
        .download(|_, _| {}, || {})
        .await
        .map_err(|e| e.to_string())?;
    validate_payload(&bytes, &spec.version, &app.config().identifier)?;
    service
        .claim_offer_generation(&spec.version, &action.url, offer.generation)
        .map_err(|e| e.to_string())?;
    {
        let _gate = state.speaker_gate.lock().await;
        let configuration = state
            .configuration
            .lock()
            .map_err(|e| e.to_string())?
            .clone();
        state
            .store
            .save(&configuration)
            .map_err(|e| e.to_string())?;
        state.stop_runtime();
    }
    // This is the pinned plugin's actual installer inside the signed sandboxed app.
    let result = update.install(&bytes).map_err(|e| e.to_string());
    service.finish_open();
    result?;
    report(app, "installed", None).await;
    app.request_restart();
    Ok(())
}

pub fn start(app: tauri::AppHandle) {
    let args: Vec<_> = std::env::args().collect();
    let Some(position) = args
        .iter()
        .position(|arg| arg == "--native-updater-acceptance")
    else {
        return;
    };
    let Some(mode) = args
        .get(position + 1)
        .filter(|mode| matches!(mode.as_str(), "seed" | "install" | "snapshot"))
        .cloned()
    else {
        return;
    };
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(5)).await;
        if let Err(error) = run(&app, &mode).await {
            report(&app, "failed", Some(error)).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        distribution::ReleaseChannel,
        updates::{SystemUpdateClock, UpdatePersistence, UpdatePreferences, UpdateService},
    };
    use std::sync::Mutex;

    #[tokio::test]
    async fn native_probe_builder_selects_the_exact_manifest_target() {
        use std::io::{BufRead, BufReader, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}/manifest.json", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = String::new();
            BufReader::new(&mut stream).read_line(&mut request).unwrap();
            assert!(request.starts_with("GET /manifest.json "));
            let body = serde_json::json!({
                "version": "99.0.0",
                "platforms": {"darwin-aarch64": {
                    "url": "http://127.0.0.1:8765/payload.app.tar.gz",
                    "signature": "fixture"
                }}
            })
            .to_string();
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body).unwrap();
        });
        let mut context = tauri::test::mock_context(tauri::test::noop_assets());
        context.config_mut().plugins.0.insert(
            "updater".into(),
            serde_json::json!({
                "pubkey": "fixture", "dangerousInsecureTransportProtocol": true
            }),
        );
        let app = tauri::test::mock_builder()
            .plugin(tauri_plugin_updater::Builder::new().build())
            .build(context)
            .unwrap();
        let update = native_builder(app.handle())
            .endpoints(vec![endpoint.parse().unwrap()])
            .unwrap()
            .executable_path(
                "/Applications/Speaker Volume Bridge.app/Contents/MacOS/speaker-volume-bridge",
            )
            .build()
            .unwrap()
            .check()
            .await
            .unwrap()
            .unwrap();
        server.join().unwrap();
        assert_eq!(update.target, "darwin-aarch64");
        assert_eq!(update.version, "99.0.0");
        assert_eq!(
            update.download_url.as_str(),
            format!("{LOOPBACK}/payload.app.tar.gz")
        );
    }

    fn distribution() -> InstalledDistribution {
        InstalledDistribution {
            edition: DistributionEdition::DirectMacos,
            version: "1.8.2".into(),
            architecture: ApplicationArchitecture::Aarch64,
            channel: ReleaseChannel::Stable,
            published_classification: None,
            check_supported: true,
            direct_install_supported: false,
        }
    }
    struct Persistence(Mutex<UpdatePreferences>);
    impl UpdatePersistence for Persistence {
        fn load(&self, _: bool) -> Result<UpdatePreferences, UpdateError> {
            Ok(self.0.lock().unwrap().clone())
        }
        fn save(&self, preferences: &UpdatePreferences) -> Result<(), UpdateError> {
            *self.0.lock().unwrap() = preferences.clone();
            Ok(())
        }
    }
    fn payload(version: &str, identifier: &str) -> Vec<u8> {
        let mut info = plist::Dictionary::new();
        info.insert(
            "CFBundleShortVersionString".into(),
            plist::Value::String(version.into()),
        );
        info.insert(
            "CFBundleIdentifier".into(),
            plist::Value::String(identifier.into()),
        );
        let mut plist = Vec::new();
        plist::Value::Dictionary(info)
            .to_writer_xml(&mut plist)
            .unwrap();
        let encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        let mut archive = tar::Builder::new(encoder);
        let mut header = tar::Header::new_gnu();
        header.set_size(plist.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        archive
            .append_data(
                &mut header,
                "Speaker Volume Bridge.app/Contents/Info.plist",
                plist.as_slice(),
            )
            .unwrap();
        archive.into_inner().unwrap().finish().unwrap()
    }
    #[test]
    fn signed_payload_version_and_identity_must_match_selected_release() {
        let bytes = payload("1.8.3", "fixture.identifier");
        assert!(validate_payload(&bytes, "1.8.3", "fixture.identifier").is_ok());
        assert!(validate_payload(&bytes, "1.8.4", "fixture.identifier").is_err());
        assert!(validate_payload(&bytes, "1.8.3", "foreign.identifier").is_err());
        assert!(validate_payload(b"not an updater bundle", "1.8.3", "fixture.identifier").is_err());
    }
    #[test]
    fn native_probe_rejects_store_unknown_architecture_and_downgrades() {
        for edition in [
            DistributionEdition::MacAppStore,
            DistributionEdition::MicrosoftStore,
            DistributionEdition::Unknown,
            DistributionEdition::Development,
        ] {
            let mut installed = distribution();
            installed.edition = edition;
            assert!(eligible(&installed, "1.8.3").is_err());
        }
        let mut installed = distribution();
        installed.architecture = ApplicationArchitecture::X86_64;
        assert!(eligible(&installed, "1.8.3").is_err());
        assert!(eligible(&distribution(), "1.8.2").is_err());
        assert!(eligible(&distribution(), "1.8.1").is_err());
        assert!(eligible(&distribution(), "1.8.3").is_ok());
    }
    #[tokio::test]
    async fn native_probe_consumes_shared_policy_and_rejects_stale_generation() {
        let fixture = Spec {
            version: "1.8.3".into(),
            pubkey: "fixture".into(),
            prerelease: true,
        };
        let service = UpdateService::new(
            distribution(),
            Arc::new(FixtureTransport(fixture)),
            Arc::new(SystemUpdateClock),
            Arc::new(Persistence(Mutex::new(UpdatePreferences::default()))),
        )
        .unwrap();
        assert!(service.check(true).await.available_version.is_none());
        service
            .set_policy(crate::updates::UpdatePolicy::Prereleases)
            .unwrap();
        let offer = service.check(true).await;
        let action = offer.action.unwrap();
        assert_eq!(offer.available_version.as_deref(), Some("1.8.3"));
        assert!(
            service
                .claim_offer_generation("1.8.3", &action.url, offer.generation)
                .is_ok()
        );
        service.finish_open();
        service
            .set_policy(crate::updates::UpdatePolicy::Stable)
            .unwrap();
        assert!(
            service
                .claim_offer_generation("1.8.3", &action.url, offer.generation)
                .is_err()
        );
    }
}
