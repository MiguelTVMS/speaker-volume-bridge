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
struct ProbeFailure {
    operation: &'static str,
    kind: &'static str,
    os_code: Option<i32>,
    io_kind: Option<Box<str>>,
    reason: Option<&'static str>,
    http_status: Option<u16>,
    #[serde(flatten)]
    evidence: Box<NativeEvidence>,
    message: Box<str>,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
struct NativeEvidence {
    native_code: Option<i64>,
    native_stage: Option<&'static str>,
    script_code: Option<i32>,
    script_stage: Option<&'static str>,
}

impl From<String> for ProbeFailure {
    fn from(message: String) -> Self {
        Self {
            operation: "precondition",
            kind: "probe",
            os_code: None,
            io_kind: None,
            reason: None,
            http_status: None,
            evidence: Box::default(),
            message: message.into_boxed_str(),
        }
    }
}

impl From<&str> for ProbeFailure {
    fn from(message: &str) -> Self {
        message.to_owned().into()
    }
}

fn updater_failure(operation: &'static str, error: &tauri_plugin_updater::Error) -> ProbeFailure {
    use tauri_plugin_updater::Error;
    let (kind, os_code, http_status) = match error {
        Error::Io(error) => ("io", error.raw_os_error(), None),
        Error::Network(message) => {
            // Pinned updater 2.10.0 wraps download HTTP status in this fixed
            // message instead of Reqwest. Export only the numeric status.
            let status = message
                .strip_prefix("Download request failed with status: ")
                .and_then(|status| status.split_whitespace().next())
                .and_then(|status| status.parse::<u16>().ok())
                .filter(|status| (100..=599).contains(status));
            (
                if status.is_some() {
                    "http"
                } else {
                    "transport"
                },
                None,
                status,
            )
        }
        Error::Reqwest(error) => (
            if error.is_timeout() {
                "timeout"
            } else if error.is_connect() {
                "connect"
            } else if error.status().is_some() {
                "http"
            } else {
                "transport"
            },
            None,
            error.status().map(|status| status.as_u16()),
        ),
        Error::Minisign(_) => ("signature", None, None),
        Error::Base64(_)
        | Error::SignatureUtf8(_)
        | Error::Serialization(_)
        | Error::InvalidUpdaterFormat => ("format", None, None),
        Error::MacosReplacement {
            stage: "authorize", ..
        } => ("authentication", None, None),
        Error::MacosReplacement { .. } => ("io", None, None),
        Error::MacosAuthorization { .. } | Error::AuthenticationFailed => {
            ("authentication", None, None)
        }
        _ => ("updater", None, None),
    };
    ProbeFailure {
        operation,
        kind,
        os_code,
        io_kind: match error {
            Error::Io(error) => Some(format!("{:?}", error.kind()).into_boxed_str()),
            _ => None,
        },
        reason: match error {
            Error::MacosReplacement { .. } => Some("native_replacement_failed"),
            Error::MacosAuthorization { .. } => Some("replacement_authorization_failed"),
            Error::Io(error) if error.to_string() == "Failed to move the new app into place" => {
                Some("replacement_authorization_failed")
            }
            _ => None,
        },
        evidence: Box::new(NativeEvidence {
            native_code: match error {
                Error::MacosReplacement { code, .. } => *code,
                _ => None,
            },
            native_stage: match error {
                Error::MacosReplacement { stage, .. } => Some(*stage),
                _ => None,
            },
            script_code: match error {
                Error::MacosAuthorization { code, .. } => {
                    code.and_then(|code| i32::try_from(code).ok())
                }
                _ => None,
            },
            script_stage: match error {
                Error::MacosAuthorization { stage, .. } => Some(*stage),
                _ => None,
            },
        }),
        http_status,
        message: error.to_string().into_boxed_str(),
    }
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
    error: Option<ProbeFailure>,
}

async fn report(app: &tauri::AppHandle, phase: &str, error: Option<ProbeFailure>) {
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

async fn run(app: &tauri::AppHandle, mode: &str) -> Result<(), ProbeFailure> {
    let state = app.state::<AppState>();
    let baseline = state
        .store
        .path()
        .with_file_name("native-updater-acceptance-baseline.json");
    if mode == "seed" {
        return seed(app).await.map_err(Into::into);
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
        .map_err(|e| updater_failure("build", &e))?
        .check()
        .await
        .map_err(|e| updater_failure("check", &e))?
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
        .map_err(|e| updater_failure("download", &e))?;
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
    let result = update
        .install(&bytes)
        .map_err(|e| updater_failure("install", &e));
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

    async fn fixture_native_update(destination: &std::path::Path) -> tauri_plugin_updater::Update {
        use std::io::{BufRead, BufReader, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}/manifest.json", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(&mut stream);
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" || line.is_empty() {
                    break;
                }
            }
            let body = serde_json::json!({"version":"99.0.0","platforms":{"darwin-aarch64":{"url":"http://127.0.0.1:8765/payload.app.tar.gz","signature":"fixture"}}}).to_string();
            write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();
        });
        let mut context = tauri::test::mock_context(tauri::test::noop_assets());
        context.config_mut().plugins.0.insert(
            "updater".into(),
            serde_json::json!({"pubkey":"fixture","dangerousInsecureTransportProtocol":true}),
        );
        let app = tauri::test::mock_builder()
            .plugin(tauri_plugin_updater::Builder::new().build())
            .build(context)
            .unwrap();
        let update = native_builder(app.handle())
            .endpoints(vec![endpoint.parse().unwrap()])
            .unwrap()
            .executable_path(destination.join("Contents/MacOS/fixture"))
            .build()
            .unwrap()
            .check()
            .await
            .unwrap()
            .unwrap();
        server.join().unwrap();
        update
    }

    fn fixture_native_archive() -> Vec<u8> {
        let gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        let mut archive = tar::Builder::new(gzip);
        let mut header = tar::Header::new_gnu();
        let bytes = b"new signed bytes";
        header.set_size(bytes.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        archive
            .append_data(&mut header, "Fixture.app/version", &bytes[..])
            .unwrap();
        archive.into_inner().unwrap().finish().unwrap()
    }

    #[tokio::test]
    async fn native_replacement_keeps_existing_app_on_denial_and_cancellation() {
        use tauri_plugin_updater::Error;
        for code in [3072, 513] {
            let root = tempfile::tempdir().unwrap();
            let destination = root.path().join("Old.app");
            let candidate = root.path().join("New.app");
            std::fs::create_dir(&destination).unwrap();
            std::fs::create_dir(&candidate).unwrap();
            std::fs::write(destination.join("version"), "old signed bytes").unwrap();
            std::fs::write(candidate.join("version"), "new signed bytes").unwrap();
            let update = fixture_native_update(&destination)
                .await
                .with_acceptance_replacement(std::sync::Arc::new(
                    move |destination, candidate, complete| {
                        assert!(
                            destination.exists(),
                            "existing bundle must reach native replacement intact"
                        );
                        assert_eq!(
                            std::fs::read_to_string(candidate.join("version")).unwrap(),
                            "new signed bytes"
                        );
                        complete(Err(Error::MacosReplacement {
                            stage: "authorize",
                            domain: "NSCocoaErrorDomain".into(),
                            code: Some(code),
                            message: "fixture denial/cancellation".into(),
                        }));
                    },
                ));
            let result = update.install(fixture_native_archive());
            let failure = updater_failure("install", &result.unwrap_err());
            assert_eq!(failure.evidence.native_code, Some(code));
            assert_eq!(failure.evidence.native_stage, Some("authorize"));
            assert_eq!(
                std::fs::read_to_string(destination.join("version")).unwrap(),
                "old signed bytes"
            );
            assert_eq!(
                std::fs::read_to_string(candidate.join("version")).unwrap(),
                "new signed bytes"
            );
        }
    }

    #[tokio::test]
    async fn native_install_restores_relocated_original_and_retains_it_if_recovery_is_denied() {
        use tauri_plugin_updater::{Error, native_replacement::recover_after_failure};
        for denied in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let destination = root.path().join("Old.app");
            std::fs::create_dir(&destination).unwrap();
            std::fs::write(destination.join("version"), "old signed bytes").unwrap();
            let recovery_location = std::sync::Arc::new(Mutex::new(None));
            let captured = recovery_location.clone();
            let update = fixture_native_update(&destination)
                .await
                .with_acceptance_replacement(std::sync::Arc::new(
                    move |destination, candidate, complete| {
                        let original = candidate.join("relocated-original.app");
                        std::fs::rename(&destination, &original).unwrap();
                        *captured.lock().unwrap() = Some(original.clone());
                        let error = Error::MacosReplacement {
                            stage: "replace",
                            domain: "fixture".into(),
                            code: Some(513),
                            message: "partial native replacement".into(),
                        };
                        complete(Err(recover_after_failure(
                            &destination,
                            Some(&original),
                            error,
                            |original, destination| {
                                if denied {
                                    Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied)
                                        .into())
                                } else {
                                    std::fs::rename(original, destination).map_err(Into::into)
                                }
                            },
                        )));
                    },
                ));
            let error = update.install(fixture_native_archive()).unwrap_err();
            let stage = updater_failure("install", &error).evidence.native_stage;
            let original = recovery_location.lock().unwrap().clone().unwrap();
            if denied {
                assert_eq!(stage, Some("recovery"));
                assert_eq!(
                    std::fs::read_to_string(original.join("version")).unwrap(),
                    "old signed bytes"
                );
                assert!(error.to_string().contains("staging retained"));
                // The fixture explicitly removes retained staging after proving it survives install.
                std::fs::remove_dir_all(original.parent().unwrap()).unwrap();
            } else {
                assert_eq!(stage, Some("replace"));
                assert_eq!(
                    std::fs::read_to_string(destination.join("version")).unwrap(),
                    "old signed bytes"
                );
            }
        }
    }

    #[test]
    fn native_replacement_dispatch_failure_and_success_are_preserved() {
        use tauri_plugin_updater::native_replacement::replace_existing;
        let root = tempfile::tempdir().unwrap();
        let destination = root.path().join("Old.app");
        let candidate = root.path().join("New.app");
        std::fs::create_dir(&destination).unwrap();
        std::fs::create_dir(&candidate).unwrap();
        let error = replace_existing(
            &destination,
            &candidate,
            |_| Err("fixture dispatch".into()),
            |_, _, _| panic!("must not authorize after dispatch failure"),
        )
        .unwrap_err();
        assert_eq!(
            updater_failure("install", &error).evidence.native_stage,
            Some("dispatch")
        );
        replace_existing(
            &destination,
            &candidate,
            |task| {
                task();
                Ok(())
            },
            |_, _, complete| complete(Ok(())),
        )
        .unwrap();
        let missing = root.path().join("missing.app");
        assert!(
            replace_existing(
                &missing,
                &candidate,
                |_| panic!("must validate before dispatch"),
                |_, _, _| unreachable!()
            )
            .is_err()
        );
    }

    #[test]
    fn native_builder_rejects_tls_validation_bypasses() {
        for flag in [
            "dangerousAcceptInvalidCerts",
            "dangerousAcceptInvalidHostnames",
        ] {
            let mut context = tauri::test::mock_context(tauri::test::noop_assets());
            let mut config = serde_json::json!({"pubkey": "fixture"});
            config[flag] = serde_json::Value::Bool(true);
            context
                .config_mut()
                .plugins
                .0
                .insert("updater".into(), config);
            let app = tauri::test::mock_builder()
                .plugin(tauri_plugin_updater::Builder::new().build())
                .build(context)
                .unwrap();
            let result = native_builder(app.handle())
                .endpoints(vec![
                    "https://example.invalid/manifest.json".parse().unwrap(),
                ])
                .unwrap()
                .build();
            assert!(
                result.is_err(),
                "must reject TLS bypass configuration: {flag}"
            );
        }
    }

    #[test]
    fn native_authorization_orchestration_handles_success_dispatch_and_compile_errors() {
        use tauri_plugin_updater::native_diagnostics::{authorization_script, run_authorization};
        assert!(
            run_authorization(
                |task| {
                    task();
                    Ok(())
                },
                || Ok(serde_json::json!([true, 0, ""]))
            )
            .is_ok()
        );
        let error = run_authorization(
            |_| Err("private dispatch failure".into()),
            || panic!("must not execute after dispatch failure"),
        )
        .unwrap_err();
        assert_eq!(
            serde_json::to_value(updater_failure("install", &error)).unwrap()["scriptStage"],
            "dispatch"
        );
        let error = run_authorization(
            |task| {
                task();
                Ok(())
            },
            || Err(("compile", "private compilation failure".into())),
        )
        .unwrap_err();
        assert!(error.to_string().contains("private compilation failure"));
        let source = authorization_script("do shell script \"true\" with administrator privileges");
        assert!(source.contains("on error nativeMessage number nativeNumber"));
        assert!(source.contains("do shell script \"true\" with administrator privileges"));
    }

    #[test]
    fn native_installer_authorization_preserves_underlying_script_error() {
        let error = tauri_plugin_updater::native_diagnostics::run_authorization(
            |task| {
                task();
                Ok(())
            },
            || {
                Ok(serde_json::json!([
                    false,
                    -1743,
                    "private native script message"
                ]))
            },
        )
        .unwrap_err();
        let report = serde_json::to_value(updater_failure("install", &error)).unwrap();
        assert_eq!(report["scriptCode"], -1743);
        assert_eq!(report["scriptStage"], "execute");
        match error {
            tauri_plugin_updater::Error::MacosAuthorization {
                stage,
                code,
                message,
            } => {
                assert_eq!(stage, "execute");
                assert_eq!(code, Some(-1743));
                assert_eq!(message, "private native script message");
            }
            other => panic!("lost underlying script error: {other}"),
        }
    }

    #[test]
    fn native_io_failure_keeps_operation_and_system_code() {
        let error = updater_failure("install", &std::io::Error::from_raw_os_error(1).into());
        let report = serde_json::to_value(error).unwrap();
        assert_eq!(report["operation"], "install");
        assert_eq!(report["kind"], "io");
        assert_eq!(report["osCode"], 1);
        assert_eq!(report["ioKind"], "PermissionDenied");
        assert!(
            report["message"]
                .as_str()
                .is_some_and(|message| !message.is_empty())
        );
    }

    #[test]
    fn native_authorization_failure_has_a_safe_exact_reason() {
        let error = updater_failure(
            "install",
            &std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "Failed to move the new app into place",
            )
            .into(),
        );
        let report = serde_json::to_value(error).unwrap();
        assert_eq!(report["ioKind"], "PermissionDenied");
        assert_eq!(report["reason"], "replacement_authorization_failed");
    }

    #[tokio::test]
    async fn native_download_failure_preserves_http_status() {
        use std::io::{BufRead, BufReader, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let endpoint = format!("{origin}/manifest.json");
        let body = serde_json::json!({
            "version": "99.0.0", "platforms": {"darwin-aarch64": {
                "url": format!("{origin}/payload.app.tar.gz"), "signature": "fixture"
            }}
        })
        .to_string();
        let server = std::thread::spawn(move || {
            for (status, body) in [("200 OK", body.as_str()), ("404 Not Found", "")] {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = String::new();
                BufReader::new(&mut stream).read_line(&mut request).unwrap();
                assert!(request.starts_with("GET /"));
                write!(stream, "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
            }
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
        let error = update.download(|_, _| {}, || {}).await.unwrap_err();
        server.join().unwrap();
        let failure = serde_json::to_value(updater_failure("download", &error)).unwrap();
        assert_eq!(failure["operation"], "download");
        assert_eq!(failure["kind"], "http");
        assert_eq!(failure["httpStatus"], 404);
    }

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
