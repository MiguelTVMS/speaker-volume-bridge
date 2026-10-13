//! Supervised application runtime.
//!
//! This module is the composition root for protocol and platform adapters. Tauri
//! commands only request a restart; they do not perform network or audio work.

use crate::{
    config::AppConfiguration,
    state::{UiSnapshot, UiStatus},
    tray,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use speaker_volume_bridge_domain::{LocalAudioState, LocalOrigin, MuteState, SonosVolume};
use speaker_volume_bridge_integration::{
    Coordinator, IntegrationError, LocalAudioPort, SonosPort, SynchronizationPolicy,
};
use speaker_volume_bridge_platform_audio::{
    AudioDeviceSelection, PlatformAudioError, SystemAudioController, SystemAudioEvent,
};
#[cfg(any(test, not(feature = "ui-demo")))]
use speaker_volume_bridge_sonos::SonosId;
#[cfg(not(feature = "ui-demo"))]
use speaker_volume_bridge_sonos::discover;
use speaker_volume_bridge_sonos::{
    CallbackListener, EventDeduplicator, GenaClient, SonosClient, SonosDevice,
};
use speaker_volume_bridge_synchronization::Synchronizer;
use std::{
    net::{IpAddr, SocketAddr},
    sync::{Arc, Mutex},
    time::Duration,
};
use tauri::{AppHandle, Emitter, Manager};
use tokio::{
    net::UdpSocket,
    sync::watch,
    time::{Instant, sleep, sleep_until},
};
use tracing::warn;
#[cfg(not(feature = "ui-demo"))]
use url::Url;

const SONOS_TIMEOUT: Duration = Duration::from_secs(3);
#[cfg(not(feature = "ui-demo"))]
const DISCOVERY_TIMEOUT: Duration = Duration::from_secs(2);
const REQUESTED_SUBSCRIPTION: Duration = Duration::from_secs(300);
const INITIAL_RECONNECT_DELAY: Duration = Duration::from_secs(1);
const MAX_RECONNECT_DELAY: Duration = Duration::from_secs(30);
#[cfg(any(test, not(feature = "ui-demo")))]
const SONOS_UDN_PREFIX: &str = "uuid:RINCON_";

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredSonos {
    pub id: String,
    pub friendly_name: String,
    pub location: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvailableAudioOutput {
    pub id: String,
    pub name: String,
    pub writable_volume: bool,
}

#[cfg(any(windows, target_os = "macos", target_os = "linux"))]
fn map_audio_outputs(
    devices: Vec<speaker_volume_bridge_platform_audio::AudioOutputDevice>,
) -> Vec<AvailableAudioOutput> {
    devices
        .into_iter()
        .map(|device| AvailableAudioOutput {
            id: device.id,
            name: device.name,
            writable_volume: device.writable_volume,
        })
        .collect()
}

#[cfg_attr(
    not(any(windows, target_os = "macos", target_os = "linux")),
    allow(clippy::unnecessary_wraps)
)]
pub fn available_audio_outputs() -> Result<Vec<AvailableAudioOutput>, String> {
    #[cfg(windows)]
    {
        speaker_volume_bridge_platform_audio::windows::list_output_devices()
            .map(map_audio_outputs)
            .map_err(|_| "Unable to list local output devices.".to_owned())
    }
    #[cfg(target_os = "macos")]
    {
        speaker_volume_bridge_platform_audio::macos::list_output_devices()
            .map(map_audio_outputs)
            .map_err(|_| "Unable to list local output devices.".to_owned())
    }
    #[cfg(target_os = "linux")]
    {
        speaker_volume_bridge_platform_audio::linux::list_output_devices()
            .map(map_audio_outputs)
            .map_err(|_| "Unable to list local output devices.".to_owned())
    }
    #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
    {
        Ok(Vec::new())
    }
}
/// Removes the implementation suffix emitted by some AirPlay renderer names.
/// The stable UDN remains the identity used by the application.
pub(crate) fn display_speaker_name(name: &str) -> String {
    name.split_once(" - Sonos ")
        .filter(|(_, suffix)| suffix.contains("Media Renderer") && suffix.contains("RINCON"))
        .map_or_else(
            || name.to_owned(),
            |(room_name, _)| room_name.trim().to_owned(),
        )
}

/// Performs bounded local-network discovery for the settings application service.
pub async fn discover_available() -> Result<Vec<DiscoveredSonos>, String> {
    #[cfg(feature = "ui-demo")]
    {
        let speaker = crate::demo::speaker().await.map_err(|e| e.to_string())?;
        Ok(vec![DiscoveredSonos {
            id: crate::demo::SPEAKER_ID.into(),
            friendly_name: "Living Room (simulated)".into(),
            location: speaker.location.to_string(),
        }])
    }
    #[cfg(not(feature = "ui-demo"))]
    discover_network().await
}

#[cfg(not(feature = "ui-demo"))]
async fn discover_network() -> Result<Vec<DiscoveredSonos>, String> {
    let client = SonosClient::builder()
        .timeout(SONOS_TIMEOUT)
        .build()
        .map_err(|_| "Unable to create the local Sonos client.".to_owned())?;
    let locations = discover(DISCOVERY_TIMEOUT)
        .await
        .map_err(|_| "Sonos discovery is unavailable on this network.".to_owned())?;
    let mut devices = Vec::new();
    for location in locations {
        if let Ok(found) = client.retrieve_device(location.clone()).await
            && is_sonos_speaker(&found.device)
        {
            devices.push(DiscoveredSonos {
                id: found.device.id.as_str().to_owned(),
                friendly_name: display_speaker_name(&found.device.friendly_name),
                location: location.to_string(),
            });
        }
    }
    devices.sort_by(|left, right| left.friendly_name.cmp(&right.friendly_name));
    devices.dedup_by(|left, right| left.id == right.id);
    Ok(devices)
}

/// Confirms that the selected speaker accepts a RenderingControl volume write
/// without changing its currently confirmed volume.
pub async fn test_selected_device(configuration: AppConfiguration) -> Result<(), String> {
    let client = SonosClient::builder()
        .timeout(SONOS_TIMEOUT)
        .build()
        .map_err(|_| "Unable to create the local Sonos client.".to_owned())?;
    let device = resolve_device(&client, &configuration)
        .await
        .map_err(|_| "The selected Sonos device is unavailable.".to_owned())?;
    let volume = client
        .get_volume(&device)
        .await
        .map_err(|_| "The selected Sonos device did not return its volume.".to_owned())?;
    client
        .set_volume(&device, volume)
        .await
        .map_err(|_| "The selected Sonos device rejected the volume test.".to_owned())
}

pub use speaker_volume_bridge_sonos::SpeakerSettings;

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SpeakerSetting {
    Loudness,
    StatusLight,
    NightSound,
    SpeechEnhancement,
    Balance,
    Treble,
    Bass,
}

pub async fn speaker_settings(configuration: AppConfiguration) -> SpeakerSettings {
    let Ok(client) = SonosClient::builder().timeout(SONOS_TIMEOUT).build() else {
        return SpeakerSettings::default();
    };
    let Ok(device) = resolve_device(&client, &configuration).await else {
        return SpeakerSettings::default();
    };
    client.get_speaker_settings(&device).await
}

pub async fn set_speaker_setting(
    configuration: AppConfiguration,
    setting: SpeakerSetting,
    enabled: bool,
) -> Result<(), String> {
    if matches!(setting, SpeakerSetting::NightSound) {
        return crate::night_schedule::set_manual(&configuration, enabled).await;
    }
    let client = SonosClient::builder()
        .timeout(SONOS_TIMEOUT)
        .build()
        .map_err(|_| "Unable to create the local Sonos client.".to_owned())?;
    let device = resolve_device(&client, &configuration)
        .await
        .map_err(|_| "The selected Sonos device is unavailable.".to_owned())?;
    match setting {
        SpeakerSetting::Loudness => client.set_loudness(&device, enabled).await,
        SpeakerSetting::StatusLight => client.set_status_light(&device, enabled).await,
        SpeakerSetting::NightSound => client.set_eq(&device, "NightMode", enabled).await,
        SpeakerSetting::Balance | SpeakerSetting::Treble | SpeakerSetting::Bass => unreachable!(),
        SpeakerSetting::SpeechEnhancement => client.set_speech_enhancement(&device, enabled).await,
    }
    .map_err(|_| "Could not confirm the speaker setting. Refresh and try again.".to_owned())
}

pub async fn set_speaker_level(
    configuration: AppConfiguration,
    setting: SpeakerSetting,
    value: i8,
) -> Result<(), String> {
    let client = SonosClient::builder()
        .timeout(SONOS_TIMEOUT)
        .build()
        .map_err(|_| "Unable to create the local Sonos client.".to_owned())?;
    let device = resolve_device(&client, &configuration)
        .await
        .map_err(|_| "The selected Sonos device is unavailable.".to_owned())?;
    match setting {
        SpeakerSetting::Balance => Err(
            "Balance is temporarily unavailable while its Ray protocol mapping is corrected."
                .to_owned(),
        ),
        SpeakerSetting::Treble | SpeakerSetting::Bass if (-10..=10).contains(&value) => client
            .set_tone(
                &device,
                if matches!(setting, SpeakerSetting::Treble) {
                    "Treble"
                } else {
                    "Bass"
                },
                value,
            )
            .await
            .map_err(|_| {
                "Could not confirm the speaker setting. Refresh and try again.".to_owned()
            }),
        _ => Err("Invalid speaker setting value.".to_owned()),
    }
}
pub async fn use_tv_audio(configuration: AppConfiguration) -> Result<(), String> {
    let client = SonosClient::builder()
        .timeout(SONOS_TIMEOUT)
        .build()
        .map_err(|_| "Unable to create the local Sonos client.".to_owned())?;
    let device = resolve_device(&client, &configuration)
        .await
        .map_err(|_| "The selected Sonos device is unavailable.".to_owned())?;
    client
        .select_home_theater_input(&device)
        .await
        .map_err(|_| "The selected speaker does not support TV audio.".to_owned())
}
pub async fn audio_input_format(configuration: AppConfiguration) -> Option<String> {
    let client = SonosClient::builder().timeout(SONOS_TIMEOUT).build().ok()?;
    let device = resolve_device(&client, &configuration).await.ok()?;
    client.get_audio_input_format(&device).await.ok().flatten()
}
/// Owns exactly one cancellable runtime generation. A newer configuration
/// invalidates updates from all older generations before starting its replacement.
#[derive(Default)]
pub struct RuntimeManager {
    active: Mutex<Option<watch::Sender<bool>>>,
    generation: std::sync::atomic::AtomicU64,
}

impl RuntimeManager {
    pub fn stop(&self) {
        if let Ok(mut active) = self.active.lock()
            && let Some(shutdown) = active.take()
        {
            let _ = shutdown.send(true);
        }
    }
    pub fn restart(
        &self,
        configuration: AppConfiguration,
        snapshot: Arc<Mutex<UiSnapshot>>,
        app: AppHandle,
    ) {
        if let Ok(mut active) = self.active.lock() {
            if let Some(shutdown) = active.take() {
                let _ = shutdown.send(true);
            }
            let (shutdown, receiver) = watch::channel(false);
            *active = Some(shutdown);
            let generation = self
                .generation
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                + 1;
            if let Ok(mut current) = snapshot.lock() {
                current.runtime_generation = generation;
            }
            tauri::async_runtime::spawn(run_supervisor(
                configuration,
                snapshot,
                app,
                generation,
                receiver,
            ));
        }
    }
}

async fn run_supervisor(
    configuration: AppConfiguration,
    snapshot: Arc<Mutex<UiSnapshot>>,
    app: AppHandle,
    generation: u64,
    mut shutdown: watch::Receiver<bool>,
) {
    if configuration.selected_sonos_id.is_none() {
        update_snapshot(
            &snapshot,
            &app,
            generation,
            UiStatus::ConfigurationRequired,
            None,
        );
        return;
    }

    let mut delay = INITIAL_RECONNECT_DELAY;
    loop {
        if *shutdown.borrow() {
            return;
        }
        update_snapshot(&snapshot, &app, generation, UiStatus::Connecting, None);
        let mut cancelled = shutdown.clone();
        let result = tokio::select! {
            biased;
            _ = cancelled.changed() => return,
            result = run_session(&configuration, &snapshot, &app, generation, &mut shutdown) => result,
        };
        if *shutdown.borrow() {
            return;
        }
        let integration_error = match &result {
            Err(RuntimeError::Integration(message)) => Some(message.as_str()),
            _ => None,
        };
        let status = match &result {
            Ok(()) => UiStatus::Connecting,
            Err(RuntimeError::Local(PlatformAudioError::UnsupportedDevice)) => {
                UiStatus::UnsupportedLocalDevice
            }
            Err(RuntimeError::Local(_)) => UiStatus::LocalAudioUnavailable,
            Err(RuntimeError::SonosUnavailable) => UiStatus::SonosUnavailable,
            Err(RuntimeError::Cancelled) => return,
            Err(RuntimeError::Integration(_)) => UiStatus::Error,
        };
        warn!(
            kind = status_name(&status),
            error = ?result,
            integration_error,
            "runtime session stopped; reconnecting with bounded backoff"
        );
        update_snapshot(&snapshot, &app, generation, status, None);
        tokio::select! {
            () = sleep(delay) => {},
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() { return; }
            }
        }
        delay = (delay * 2).min(MAX_RECONNECT_DELAY);
    }
}

#[allow(clippy::too_many_lines)] // This select loop owns one session's complete lifecycle.
async fn run_session(
    configuration: &AppConfiguration,
    snapshot: &Arc<Mutex<UiSnapshot>>,
    app: &AppHandle,
    generation: u64,
    shutdown: &mut watch::Receiver<bool>,
) -> Result<(), RuntimeError> {
    let client = SonosClient::builder()
        .timeout(SONOS_TIMEOUT)
        .build()
        .map_err(|_| RuntimeError::SonosUnavailable)?;
    let device = resolve_device(&client, configuration).await?;
    tracing::info!("resolved selected Sonos speaker");
    let speaker_name = display_speaker_name(&device.friendly_name);
    let audio = create_audio(configuration)?;
    tracing::info!("attached selected local audio output");
    let machine = Synchronizer::new(
        configuration.mapping.clone(),
        configuration.maximum_sonos_volume,
        configuration.synchronize_mute,
        configuration.two_way_synchronization,
        configuration.mute_speaker_at_zero_volume,
    )
    .map_err(|error| RuntimeError::Integration(error.to_string()))?;
    let sonos = DeviceSonosPort {
        client: client.clone(),
        device: device.clone(),
    };
    let local = DeviceAudioPort(Arc::clone(&audio));
    let mut coordinator = Coordinator::new(machine, sonos, local, SynchronizationPolicy::default());
    let mut audio_events = audio.subscribe();

    update_snapshot(
        snapshot,
        app,
        generation,
        UiStatus::Connecting,
        Some(SnapshotValues {
            name: &speaker_name,
            sonos_volume: None,
            local_volume: None,
            muted: None,
        }),
    );
    coordinator
        .reconcile_startup()
        .await
        .map_err(RuntimeError::from)?;
    if configuration.two_way_synchronization {
        tracing::info!("reconciled initial Sonos state to local audio");
    } else {
        tracing::info!("observed initial Sonos state without changing local audio");
    }
    let confirmed_volume = client
        .get_volume(&device)
        .await
        .map_err(|_| RuntimeError::SonosUnavailable)?;
    let confirmed_mute = client
        .get_mute(&device)
        .await
        .map_err(|_| RuntimeError::SonosUnavailable)?;
    let local_state = audio.current_state().await.map_err(RuntimeError::Local)?;
    tracing::info!("read current local audio state");
    if !configuration.two_way_synchronization {
        coordinator
            .on_local_event(local_state, LocalOrigin::User, true)
            .await
            .map_err(RuntimeError::from)?;
        tracing::info!("applied initial local audio state to Sonos in one-way mode");
    }
    update_snapshot(
        snapshot,
        app,
        generation,
        UiStatus::SubscriptionDegraded,
        Some(SnapshotValues {
            name: &speaker_name,
            sonos_volume: None,
            local_volume: Some(local_state.volume.get()),
            muted: Some(local_state.muted.0),
        }),
    );

    let peer = peer_address(&device)?;
    let bind = selected_local_bind(peer).await?;
    let mut listener = CallbackListener::bind(bind, peer.ip())
        .await
        .map_err(|_| RuntimeError::SonosUnavailable)?;
    tracing::info!("started local Sonos event listener");
    let gena =
        GenaClient::new(SONOS_TIMEOUT, 64 * 1024).map_err(|_| RuntimeError::SonosUnavailable)?;
    let mut subscription = gena
        .subscribe(&device, listener.callback_url(), REQUESTED_SUBSCRIPTION)
        .await
        .ok();
    tracing::info!(
        subscribed = subscription.is_some(),
        "requested Sonos event subscription"
    );
    if let Some(value) = subscription.as_ref() {
        listener.set_subscription(value);
    }
    update_snapshot(
        snapshot,
        app,
        generation,
        if subscription.is_some() {
            UiStatus::Synchronized
        } else {
            UiStatus::SubscriptionDegraded
        },
        Some(SnapshotValues {
            name: &speaker_name,
            sonos_volume: Some(confirmed_volume.get()),
            local_volume: Some(local_state.volume.get()),
            muted: Some(confirmed_mute.0),
        }),
    );
    let mut renew_at = subscription
        .as_ref()
        .map_or_else(|| Instant::now() + Duration::from_secs(5), renewal_at);
    let mut last_local = Some(local_state);
    let mut deduplicator = EventDeduplicator::default();
    let mut last_settings_sequence = None;
    let mut health_at = Instant::now() + Duration::from_secs(5);

    loop {
        let local_deadline = coordinator.local_deadline();
        let settling_deadline = coordinator.settling_deadline();
        tokio::select! {
            () = sleep_until(settling_deadline.unwrap_or_else(Instant::now)), if settling_deadline.is_some() => {
                reconcile_session_observations(&mut listener, &mut coordinator, audio.as_ref(), SessionObservation::Settled,
                    subscription.is_some(), configuration.fallback_polling,
                    |confirmed, local, status| update_snapshot(snapshot, app, generation, status,
                        Some(SnapshotValues { name: &speaker_name, sonos_volume: Some(confirmed.volume.get()), local_volume: Some(local.volume.get()), muted: Some(confirmed.muted.0) }))).await?;
            }
            () = sleep_until(local_deadline.unwrap_or_else(Instant::now)), if local_deadline.is_some() => {
                coordinator.run_local_once().await.map_err(RuntimeError::from)?;
            }
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    if let Some(value) = subscription.as_ref() { let _ = gena.unsubscribe(&device, value).await; }
                    return Err(RuntimeError::Cancelled);
                }
            }
            notification = listener.recv() => {
                let Some(notification) = notification else { return Err(RuntimeError::SonosUnavailable); };
                let current_generation = snapshot.lock().is_ok_and(|state| state.runtime_generation == generation);
                if current_generation && notification.settings_changed
                    && (notification.sequence.is_none() || notification.sequence != last_settings_sequence) {
                    last_settings_sequence = notification.sequence;
                    app.state::<crate::state::AppState>().schedule_refresh.store(true, std::sync::atomic::Ordering::Relaxed);
                    let _ = app.emit("speaker-settings-changed", ());
                    tray::refresh_speaker_controls(app);
                }
                let volume_state = client.notification_state(&device, &notification).await.map_err(|_| RuntimeError::SonosUnavailable)?;
                if let Some(event) = volume_state.filter(|event| deduplicator.accept(event.clone())) {
                    process_session_observation(&mut coordinator, audio.as_ref(), SessionObservation::Speaker(event.state.volume, event.state.muted),
                        subscription.is_some(), configuration.fallback_polling,
                        |confirmed, local, status| update_snapshot(snapshot, app, generation, status,
                            Some(SnapshotValues { name: &speaker_name, sonos_volume: Some(confirmed.volume.get()), local_volume: Some(local.volume.get()), muted: Some(confirmed.muted.0) }))).await?;
                }
            }
            received = audio_events.recv() => match newest_audio_event(received, &mut audio_events) {
                Ok(SystemAudioEvent::StateChanged { state, origin }) => {
                    if origin == LocalOrigin::Application {
                        last_local = Some(state);
                        continue;
                    }
                    let mute_changed = last_local.is_none_or(|previous| previous.muted != state.muted);
                    last_local = Some(state);
                    update_snapshot(snapshot, app, generation, UiStatus::WaitingForSonosConfirmation, Some(SnapshotValues { name: &speaker_name, sonos_volume: None, local_volume: Some(state.volume.get()), muted: Some(state.muted.0) }));
                    coordinator.on_local_event(state, origin, mute_changed).await.map_err(RuntimeError::from)?;
                }
                Ok(SystemAudioEvent::DefaultOutputChanged) => {
                    reconcile_session_observations(&mut listener, &mut coordinator, audio.as_ref(), SessionObservation::OutputChanged,
                        subscription.is_some(), configuration.fallback_polling,
                        |confirmed, local, status| {
                            last_local = Some(local);
                            update_snapshot(snapshot, app, generation, status,
                                Some(SnapshotValues { name: &speaker_name, sonos_volume: Some(confirmed.volume.get()), local_volume: Some(local.volume.get()), muted: Some(confirmed.muted.0) }));
                        }).await?;
                    tracing::info!("reconciled changed local output without reconnecting to the speaker");
                }
                Ok(SystemAudioEvent::DeviceUnavailable { .. }) | Err(_) => return Err(RuntimeError::Local(PlatformAudioError::DeviceUnavailable)),
            },
            () = sleep_until(renew_at) => {
                if let Some(current) = subscription.as_ref() {
                    if let Ok(next) = gena.renew(&device, current, REQUESTED_SUBSCRIPTION).await {
                        listener.set_subscription(&next);
                        renew_at = renewal_at(&next);
                        subscription = Some(next);
                    } else {
                        subscription = None;
                        coordinator.on_subscription_lost();
                        update_snapshot(snapshot, app, generation, UiStatus::SubscriptionDegraded, None);
                        renew_at = Instant::now() + Duration::from_secs(5);
                    }
                } else {
                    subscription = gena.subscribe(&device, listener.callback_url(), REQUESTED_SUBSCRIPTION).await.ok();
                    if let Some(next) = subscription.as_ref() { listener.set_subscription(next); renew_at = renewal_at(next); } else { renew_at = Instant::now() + Duration::from_secs(5); }
                }
            }
            () = sleep_until(health_at), if configuration.fallback_polling => {
                // A fixed deadline cannot be starved by unrelated audio/events.
                let volume = client.get_volume(&device).await.map_err(|_| RuntimeError::SonosUnavailable)?;
                let muted = client.get_mute(&device).await.map_err(|_| RuntimeError::SonosUnavailable)?;
                process_session_observation(&mut coordinator, audio.as_ref(), SessionObservation::Speaker(volume, muted),
                    subscription.is_some(), configuration.fallback_polling,
                    |confirmed, local, status| update_snapshot(snapshot, app, generation, status,
                        Some(SnapshotValues { name: &speaker_name, sonos_volume: Some(confirmed.volume.get()), local_volume: Some(local.volume.get()), muted: Some(confirmed.muted.0) }))).await?;
                let _ = app.emit("speaker-settings-changed", ());
                tray::refresh_speaker_controls(app);
                health_at = Instant::now() + if subscription.is_some() { Duration::from_secs(5) } else { Duration::from_secs(1) };
            }
        }
    }
}

// Keep the queue boundary and the fresh read in one production lifecycle operation.
async fn reconcile_session_observations<S: SonosPort, A: LocalAudioPort>(
    listener: &mut CallbackListener,
    coordinator: &mut Coordinator<S, A>,
    audio: &dyn SystemAudioController,
    observation: SessionObservation,
    subscribed: bool,
    fallback_polling: bool,
    publish: impl FnOnce(speaker_volume_bridge_domain::ConfirmedSonosState, LocalAudioState, UiStatus),
) -> Result<(), RuntimeError> {
    process_session_observation(
        coordinator,
        audio,
        observation,
        subscribed,
        fallback_polling,
        publish,
    )
    .await?;
    if let Some(read_completed_at) = coordinator.last_read_completed_at() {
        listener.discard_volume_observations_through(read_completed_at);
    }
    Ok(())
}

// Shared production orchestration for observations and their UI publication.
// Rejected notifications and health reads must leave the waiting snapshot intact.
enum SessionObservation {
    Settled,
    OutputChanged,
    Speaker(SonosVolume, MuteState),
}

async fn process_session_observation<S: SonosPort, A: LocalAudioPort>(
    coordinator: &mut Coordinator<S, A>,
    audio: &dyn SystemAudioController,
    observation: SessionObservation,
    subscribed: bool,
    fallback_polling: bool,
    publish: impl FnOnce(speaker_volume_bridge_domain::ConfirmedSonosState, LocalAudioState, UiStatus),
) -> Result<(), RuntimeError> {
    match observation {
        SessionObservation::Settled => coordinator.finish_local_adjustment().await?,
        SessionObservation::OutputChanged => {
            let local = audio.current_state().await.map_err(RuntimeError::Local)?;
            coordinator.reconcile_output_change(local).await?;
        }
        SessionObservation::Speaker(volume, muted) => {
            if !coordinator.on_sonos_event(volume, muted).await? {
                return Ok(());
            }
        }
    }
    if let Some(confirmed) = coordinator.confirmed_state() {
        let local = audio.current_state().await.map_err(RuntimeError::Local)?;
        let status = if subscribed {
            UiStatus::Synchronized
        } else if fallback_polling {
            UiStatus::PollingFallback
        } else {
            UiStatus::SubscriptionDegraded
        };
        publish(confirmed, local, status);
    }
    Ok(())
}

// Collapse callbacks buffered during a network request before issuing another write.
fn newest_audio_event(
    first: Result<SystemAudioEvent, tokio::sync::broadcast::error::RecvError>,
    receiver: &mut tokio::sync::broadcast::Receiver<SystemAudioEvent>,
) -> Result<SystemAudioEvent, tokio::sync::broadcast::error::RecvError> {
    use tokio::sync::broadcast::error::{RecvError, TryRecvError};
    let mut newest = match first {
        Ok(event) => Some(event),
        Err(RecvError::Lagged(_)) => None,
        Err(error) => return Err(error),
    };
    // A bounded snapshot cannot be starved by continuing platform callbacks.
    for _ in 0..receiver.len() {
        if newest
            .as_ref()
            .is_some_and(|event| !matches!(event, SystemAudioEvent::StateChanged { .. }))
        {
            break;
        }
        match receiver.try_recv() {
            Ok(event) => {
                let application_echo = matches!(
                    event,
                    SystemAudioEvent::StateChanged {
                        origin: LocalOrigin::Application,
                        ..
                    }
                );
                let user_waiting = matches!(
                    newest,
                    Some(SystemAudioEvent::StateChanged {
                        origin: LocalOrigin::User | LocalOrigin::Unknown,
                        ..
                    })
                );
                if !application_echo || !user_waiting {
                    newest = Some(event);
                }
            }
            Err(TryRecvError::Lagged(_)) => {}
            Err(TryRecvError::Empty | TryRecvError::Closed) => break,
        }
    }
    newest.ok_or(RecvError::Closed)
}

fn renewal_at(subscription: &speaker_volume_bridge_sonos::Subscription) -> Instant {
    Instant::now()
        + subscription
            .timeout
            .mul_f32(0.8)
            .max(Duration::from_secs(1))
}

fn peer_address(device: &SonosDevice) -> Result<SocketAddr, RuntimeError> {
    let host = device
        .rendering_control
        .event_url
        .host_str()
        .ok_or(RuntimeError::SonosUnavailable)?;
    let ip = host
        .parse::<IpAddr>()
        .map_err(|_| RuntimeError::SonosUnavailable)?;
    Ok(SocketAddr::new(
        ip,
        device
            .rendering_control
            .event_url
            .port_or_known_default()
            .unwrap_or(1400),
    ))
}

async fn selected_local_bind(peer: SocketAddr) -> Result<SocketAddr, RuntimeError> {
    let socket = UdpSocket::bind(match peer {
        SocketAddr::V4(_) => "0.0.0.0:0",
        SocketAddr::V6(_) => "[::]:0",
    })
    .await
    .map_err(|_| RuntimeError::SonosUnavailable)?;
    socket
        .connect(peer)
        .await
        .map_err(|_| RuntimeError::SonosUnavailable)?;
    Ok(SocketAddr::new(
        socket
            .local_addr()
            .map_err(|_| RuntimeError::SonosUnavailable)?
            .ip(),
        0,
    ))
}

pub(crate) async fn resolve_device(
    client: &SonosClient,
    configuration: &AppConfiguration,
) -> Result<SonosDevice, RuntimeError> {
    #[cfg(feature = "ui-demo")]
    {
        if configuration.selected_sonos_id.as_deref() != Some(crate::demo::SPEAKER_ID) {
            return Err(RuntimeError::SonosUnavailable);
        }
        let speaker = crate::demo::speaker()
            .await
            .map_err(|_| RuntimeError::SonosUnavailable)?;
        return client
            .retrieve_device(speaker.location.clone())
            .await
            .map(|found| found.device)
            .map_err(|_| RuntimeError::SonosUnavailable);
    }
    #[cfg(not(feature = "ui-demo"))]
    resolve_network_device(client, configuration).await
}

#[cfg(not(feature = "ui-demo"))]
async fn resolve_network_device(
    client: &SonosClient,
    configuration: &AppConfiguration,
) -> Result<SonosDevice, RuntimeError> {
    let selected = SonosId::new(
        configuration
            .selected_sonos_id
            .clone()
            .ok_or(RuntimeError::SonosUnavailable)?,
    )
    .map_err(|_| RuntimeError::SonosUnavailable)?;
    if let Some(address) = configuration.last_known_sonos_address.as_deref()
        && let Ok(location) = Url::parse(address)
        && let Ok(found) = client.retrieve_device(location).await
        && is_sonos_speaker(&found.device)
        && found.device.id == selected
    {
        return Ok(found.device);
    }
    let locations = discover(DISCOVERY_TIMEOUT)
        .await
        .map_err(|_| RuntimeError::SonosUnavailable)?;
    for location in locations {
        if let Ok(found) = client.retrieve_device(location).await
            && is_sonos_speaker(&found.device)
            && found.device.id == selected
        {
            return Ok(found.device);
        }
    }
    Err(RuntimeError::SonosUnavailable)
}

#[cfg(any(test, not(feature = "ui-demo")))]
fn is_sonos_speaker(device: &SonosDevice) -> bool {
    device.id.as_str().starts_with(SONOS_UDN_PREFIX)
}

fn create_audio(
    configuration: &AppConfiguration,
) -> Result<Arc<dyn SystemAudioController>, RuntimeError> {
    let selection = if configuration.follow_default_audio_device {
        AudioDeviceSelection::FollowDefault
    } else if let Some(device_id) = configuration.fixed_audio_device_id.clone() {
        AudioDeviceSelection::Fixed { device_id }
    } else {
        AudioDeviceSelection::FollowDefault
    };
    #[cfg(windows)]
    {
        Ok(Arc::new(
            speaker_volume_bridge_platform_audio::windows::WindowsAudioController::start(selection)
                .map_err(RuntimeError::Local)?,
        ))
    }
    #[cfg(target_os = "macos")]
    {
        Ok(Arc::new(
            speaker_volume_bridge_platform_audio::macos::MacosAudioController::start(selection, 1)
                .map_err(RuntimeError::Local)?,
        ))
    }
    #[cfg(target_os = "linux")]
    {
        Ok(Arc::new(
            speaker_volume_bridge_platform_audio::linux::LinuxAudioController::start(selection)
                .map_err(RuntimeError::Local)?,
        ))
    }
    #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
    {
        let _ = selection;
        Err(RuntimeError::Local(PlatformAudioError::DeviceUnavailable))
    }
}

#[derive(Clone)]
struct DeviceSonosPort {
    client: SonosClient,
    device: SonosDevice,
}
#[async_trait]
impl SonosPort for DeviceSonosPort {
    async fn current_state(&self) -> Result<(SonosVolume, MuteState), IntegrationError> {
        Ok((
            self.client
                .get_volume(&self.device)
                .await
                .map_err(|error| IntegrationError::Sonos(error.to_string()))?,
            self.client
                .get_mute(&self.device)
                .await
                .map_err(|error| IntegrationError::Sonos(error.to_string()))?,
        ))
    }
    async fn set_volume(&self, volume: SonosVolume) -> Result<(), IntegrationError> {
        self.client
            .set_volume(&self.device, volume)
            .await
            .map_err(|error| IntegrationError::Sonos(error.to_string()))
    }
    async fn set_mute(&self, muted: MuteState) -> Result<(), IntegrationError> {
        self.client
            .set_mute(&self.device, muted)
            .await
            .map_err(|error| IntegrationError::Sonos(error.to_string()))
    }
}
struct DeviceAudioPort(Arc<dyn SystemAudioController>);
#[async_trait]
impl LocalAudioPort for DeviceAudioPort {
    async fn apply(&self, state: LocalAudioState) -> Result<(), IntegrationError> {
        let current = self
            .0
            .current_state()
            .await
            .map_err(|error| IntegrationError::Audio(error.to_string()))?;
        if current.volume != state.volume {
            self.0
                .set_volume(state.volume, LocalOrigin::Application)
                .await
                .map_err(|error| IntegrationError::Audio(error.to_string()))?;
        }
        if current.muted == state.muted {
            return Ok(());
        }
        match self
            .0
            .set_muted(state.muted.0, LocalOrigin::Application)
            .await
        {
            Ok(()) | Err(PlatformAudioError::MuteUnavailable) => Ok(()),
            Err(error) => Err(IntegrationError::Audio(error.to_string())),
        }
    }
}

#[derive(Debug)]
pub(crate) enum RuntimeError {
    SonosUnavailable,
    Local(PlatformAudioError),
    Integration(String),
    Cancelled,
}
impl From<IntegrationError> for RuntimeError {
    fn from(value: IntegrationError) -> Self {
        Self::Integration(value.to_string())
    }
}

struct SnapshotValues<'a> {
    name: &'a str,
    sonos_volume: Option<u8>,
    local_volume: Option<u8>,
    muted: Option<bool>,
}

fn update_snapshot(
    snapshot: &Arc<Mutex<UiSnapshot>>,
    app: &AppHandle,
    generation: u64,
    status: UiStatus,
    values: Option<SnapshotValues<'_>>,
) {
    let updated = if let Ok(mut current) = snapshot.lock() {
        // A stopped generation may not overwrite a newer runtime's state.
        if current.runtime_generation != generation {
            return;
        }
        current.status = status;
        if let Some(values) = values {
            current.sonos_name = Some(values.name.to_owned());
            if values.sonos_volume.is_some() {
                current.sonos_volume = values.sonos_volume;
            }
            if values.local_volume.is_some() {
                current.local_volume = values.local_volume;
            }
            if values.muted.is_some() {
                current.muted = values.muted;
            }
        }
        Some(current.clone())
    } else {
        None
    };
    if let Some(updated) = updated {
        let _ = app.emit("runtime-status-changed", updated);
    }
    tray::refresh(app);
}

fn status_name(status: &UiStatus) -> &'static str {
    match status {
        UiStatus::Discovering => "discovering",
        UiStatus::Connecting => "connecting",
        UiStatus::Synchronized => "synchronized",
        UiStatus::WaitingForSonosConfirmation => "waiting",
        UiStatus::SubscriptionDegraded => "subscription_degraded",
        UiStatus::PollingFallback => "polling_fallback",
        UiStatus::SonosUnavailable => "sonos_unavailable",
        UiStatus::LocalAudioUnavailable => "local_audio_unavailable",
        UiStatus::UnsupportedLocalDevice => "unsupported_local_device",
        UiStatus::ConfigurationRequired => "configuration_required",
        UiStatus::Error => "error",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use speaker_volume_bridge_domain::NormalizedVolume;
    use speaker_volume_bridge_sonos::RenderingControlService;
    use tokio::sync::broadcast;
    use url::Url;

    struct SessionSonos(Arc<Mutex<(SonosVolume, MuteState)>>);

    #[async_trait]
    impl SonosPort for SessionSonos {
        async fn current_state(&self) -> Result<(SonosVolume, MuteState), IntegrationError> {
            Ok(*self.0.lock().unwrap())
        }
        async fn set_volume(&self, volume: SonosVolume) -> Result<(), IntegrationError> {
            self.0.lock().unwrap().0 = volume;
            Ok(())
        }
        async fn set_mute(&self, muted: MuteState) -> Result<(), IntegrationError> {
            self.0.lock().unwrap().1 = muted;
            Ok(())
        }
    }

    struct SessionAudio(Mutex<LocalAudioState>);

    #[async_trait]
    impl SystemAudioController for SessionAudio {
        async fn current_state(&self) -> Result<LocalAudioState, PlatformAudioError> {
            Ok(*self.0.lock().unwrap())
        }
        async fn set_volume(
            &self,
            volume: NormalizedVolume,
            _: LocalOrigin,
        ) -> Result<(), PlatformAudioError> {
            self.0.lock().unwrap().volume = volume;
            Ok(())
        }
        async fn set_muted(&self, muted: bool, _: LocalOrigin) -> Result<(), PlatformAudioError> {
            self.0.lock().unwrap().muted = MuteState(muted);
            Ok(())
        }
        fn subscribe(&self) -> broadcast::Receiver<SystemAudioEvent> {
            broadcast::channel(1).1
        }
    }

    async fn enqueue_test_notification(listener: &CallbackListener, value: u8) {
        enqueue_test_notification_at(listener.callback_url(), value).await;
    }

    async fn enqueue_test_notification_at(callback: &url::Url, value: u8) {
        let body = format!(
            "<LastChange>&lt;Event&gt;&lt;InstanceID val=\"0\"&gt;&lt;Volume channel=\"Master\" val=\"{value}\"/&gt;&lt;Mute channel=\"Master\" val=\"0\"/&gt;&lt;Bass val=\"1\"/&gt;&lt;/InstanceID&gt;&lt;/Event&gt;</LastChange>"
        );
        let response = reqwest::Client::builder()
            .no_proxy()
            .build()
            .unwrap()
            .request(
                reqwest::Method::from_bytes(b"NOTIFY").unwrap(),
                callback.clone(),
            )
            .body(body)
            .send()
            .await
            .unwrap();
        assert!(response.status().is_success());
    }

    struct PausedPostReadAudio {
        state: Mutex<LocalAudioState>,
        armed: std::sync::atomic::AtomicBool,
        pause_on_write: bool,
        started: tokio::sync::Notify,
        resume: tokio::sync::Notify,
    }

    impl PausedPostReadAudio {
        async fn pause(&self) {
            if self.armed.swap(false, std::sync::atomic::Ordering::SeqCst) {
                self.started.notify_one();
                self.resume.notified().await;
            }
        }
    }

    #[async_trait]
    impl SystemAudioController for PausedPostReadAudio {
        async fn current_state(&self) -> Result<LocalAudioState, PlatformAudioError> {
            let state = *self.state.lock().unwrap();
            // Once the read's correction has been applied, the next state read
            // belongs to snapshot publication rather than the audio effect.
            if !self.pause_on_write && state.volume.get() == 45 {
                self.pause().await;
            }
            Ok(state)
        }
        async fn set_volume(
            &self,
            volume: NormalizedVolume,
            _: LocalOrigin,
        ) -> Result<(), PlatformAudioError> {
            if self.pause_on_write {
                self.pause().await;
            }
            self.state.lock().unwrap().volume = volume;
            Ok(())
        }
        async fn set_muted(&self, muted: bool, _: LocalOrigin) -> Result<(), PlatformAudioError> {
            self.state.lock().unwrap().muted = MuteState(muted);
            Ok(())
        }
        fn subscribe(&self) -> broadcast::Receiver<SystemAudioEvent> {
            broadcast::channel(1).1
        }
    }

    #[tokio::test(start_paused = true)]
    async fn post_read_notifications_survive_local_apply_and_publication_without_polling() {
        for pause_on_write in [true, false] {
            let mut listener = CallbackListener::bind(
                "127.0.0.1:0".parse().unwrap(),
                "127.0.0.1".parse().unwrap(),
            )
            .await
            .unwrap();
            let callback = listener.callback_url().clone();
            let local = LocalAudioState {
                volume: NormalizedVolume::new(20).unwrap(),
                muted: MuteState(false),
            };
            let controlled = Arc::new(PausedPostReadAudio {
                state: Mutex::new(local),
                armed: std::sync::atomic::AtomicBool::new(false),
                pause_on_write,
                started: tokio::sync::Notify::new(),
                resume: tokio::sync::Notify::new(),
            });
            let audio: Arc<dyn SystemAudioController> = controlled.clone();
            let speaker = Arc::new(Mutex::new((
                SonosVolume::new(20).unwrap(),
                MuteState(false),
            )));
            let mut coordinator = Coordinator::new(
                Synchronizer::new(
                    speaker_volume_bridge_domain::VolumeMapping::Linear,
                    SonosVolume::MAX,
                    true,
                    true,
                    false,
                )
                .unwrap(),
                SessionSonos(Arc::clone(&speaker)),
                DeviceAudioPort(Arc::clone(&audio)),
                SynchronizationPolicy::default(),
            );
            coordinator.reconcile_startup().await.unwrap();
            audio
                .set_volume(NormalizedVolume::new(60).unwrap(), LocalOrigin::User)
                .await
                .unwrap();
            coordinator
                .on_local_event(
                    audio.current_state().await.unwrap(),
                    LocalOrigin::User,
                    false,
                )
                .await
                .unwrap();
            coordinator.run_local_once().await.unwrap();
            // The settling GET returns a correction requiring local audio work.
            *speaker.lock().unwrap() = (SonosVolume::new(45).unwrap(), MuteState(false));
            controlled
                .armed
                .store(true, std::sync::atomic::Ordering::SeqCst);
            let task = tokio::spawn(async move {
                let mut published = 0;
                reconcile_session_observations(
                    &mut listener,
                    &mut coordinator,
                    audio.as_ref(),
                    SessionObservation::Settled,
                    true,
                    false,
                    |confirmed, _, _| published = confirmed.volume.get(),
                )
                .await
                .unwrap();
                (listener, coordinator, audio, published)
            });
            controlled.started.notified().await;
            tokio::time::advance(Duration::from_millis(1)).await;
            *speaker.lock().unwrap() = (SonosVolume::new(35).unwrap(), MuteState(false));
            enqueue_test_notification_at(&callback, 35).await;
            controlled.resume.notify_one();
            let (mut listener, mut coordinator, audio, mut published) = task.await.unwrap();
            assert_eq!(published, 45);
            let fresh = listener
                .recv()
                .await
                .unwrap()
                .volume_state
                .expect("post-read notification must survive");
            process_session_observation(
                &mut coordinator,
                audio.as_ref(),
                SessionObservation::Speaker(fresh.state.volume, fresh.state.muted),
                true,
                false,
                |confirmed, _, _| published = confirmed.volume.get(),
            )
            .await
            .unwrap();
            assert_eq!(published, 35);
            assert_eq!(audio.current_state().await.unwrap().volume.get(), 35);
        }
    }

    #[tokio::test(start_paused = true)]
    async fn output_change_fences_already_queued_volume_before_reconciliation() {
        let mut listener =
            CallbackListener::bind("127.0.0.1:0".parse().unwrap(), "127.0.0.1".parse().unwrap())
                .await
                .unwrap();
        let audio: Arc<dyn SystemAudioController> =
            Arc::new(SessionAudio(Mutex::new(LocalAudioState {
                volume: NormalizedVolume::new(20).unwrap(),
                muted: MuteState(false),
            })));
        let speaker = Arc::new(Mutex::new((
            SonosVolume::new(20).unwrap(),
            MuteState(false),
        )));
        let mut coordinator = Coordinator::new(
            Synchronizer::new(
                speaker_volume_bridge_domain::VolumeMapping::Linear,
                SonosVolume::MAX,
                true,
                true,
                false,
            )
            .unwrap(),
            SessionSonos(Arc::clone(&speaker)),
            DeviceAudioPort(Arc::clone(&audio)),
            SynchronizationPolicy::default(),
        );
        coordinator.reconcile_startup().await.unwrap();
        enqueue_test_notification(&listener, 20).await;
        *speaker.lock().unwrap() = (SonosVolume::new(60).unwrap(), MuteState(false));
        audio
            .set_volume(NormalizedVolume::new(10).unwrap(), LocalOrigin::User)
            .await
            .unwrap();
        let mut published = 0;
        // Output change wins while an older speaker event is already queued.
        reconcile_session_observations(
            &mut listener,
            &mut coordinator,
            audio.as_ref(),
            SessionObservation::OutputChanged,
            true,
            false,
            |confirmed, _, _| published = confirmed.volume.get(),
        )
        .await
        .unwrap();
        assert_eq!(published, 60);
        let stale = listener.recv().await.unwrap();
        assert!(stale.volume_state.is_none());
        assert!(!stale.volume_changed);
        assert!(stale.settings_changed);
        assert_eq!(audio.current_state().await.unwrap().volume.get(), 60);
        tokio::time::advance(Duration::from_millis(1)).await;
        enqueue_test_notification(&listener, 35).await;
        let fresh = listener.recv().await.unwrap().volume_state.unwrap();
        process_session_observation(
            &mut coordinator,
            audio.as_ref(),
            SessionObservation::Speaker(fresh.state.volume, fresh.state.muted),
            true,
            false,
            |confirmed, _, _| published = confirmed.volume.get(),
        )
        .await
        .unwrap();
        assert_eq!(published, 35);
        assert_eq!(audio.current_state().await.unwrap().volume.get(), 35);
    }

    async fn complete_delayed_test_notification(
        stream: &mut tokio::net::TcpStream,
        callback: &url::Url,
    ) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let body = "<LastChange>&lt;Event&gt;&lt;InstanceID val=\"0\"&gt;&lt;Volume channel=\"Master\" val=\"20\"/&gt;&lt;Mute channel=\"Master\" val=\"0\"/&gt;&lt;Bass val=\"1\"/&gt;&lt;/InstanceID&gt;&lt;/Event&gt;</LastChange>";
        let request = format!(
            "NOTIFY {} HTTP/1.1\r\nContent-Length: {}\r\n\r\n{body}",
            callback.path(),
            body.len()
        );
        stream.write_all(request.as_bytes()).await.unwrap();
        let mut response = Vec::new();
        stream.read_to_end(&mut response).await.unwrap();
        assert!(response.starts_with(b"HTTP/1.1 200"));
    }

    #[tokio::test(start_paused = true)]
    async fn accepted_notification_delayed_across_reconciliation_keeps_arrival_timestamp() {
        for observation in [
            SessionObservation::Settled,
            SessionObservation::OutputChanged,
        ] {
            let accepted = Arc::new(tokio::sync::Notify::new());
            let observed = Arc::clone(&accepted);
            let mut listener = CallbackListener::bind_with_accept_observer(
                "127.0.0.1:0".parse().unwrap(),
                "127.0.0.1".parse().unwrap(),
                move |_| {
                    observed.notify_one();
                },
            )
            .await
            .unwrap();
            let local = LocalAudioState {
                volume: NormalizedVolume::new(20).unwrap(),
                muted: MuteState(false),
            };
            let audio: Arc<dyn SystemAudioController> = Arc::new(SessionAudio(Mutex::new(local)));
            let mut coordinator = Coordinator::new(
                Synchronizer::new(
                    speaker_volume_bridge_domain::VolumeMapping::Linear,
                    SonosVolume::MAX,
                    true,
                    true,
                    false,
                )
                .unwrap(),
                SessionSonos(Arc::new(Mutex::new((
                    SonosVolume::new(20).unwrap(),
                    MuteState(false),
                )))),
                DeviceAudioPort(Arc::clone(&audio)),
                SynchronizationPolicy::default(),
            );
            coordinator.reconcile_startup().await.unwrap();
            audio
                .set_volume(NormalizedVolume::new(60).unwrap(), LocalOrigin::User)
                .await
                .unwrap();
            coordinator
                .on_local_event(
                    audio.current_state().await.unwrap(),
                    LocalOrigin::User,
                    false,
                )
                .await
                .unwrap();
            coordinator.run_local_once().await.unwrap();
            let callback = listener.callback_url().clone();
            let mut stream = tokio::net::TcpStream::connect((
                callback.host_str().unwrap(),
                callback.port().unwrap(),
            ))
            .await
            .unwrap();
            // Prove accept completed, but hold the child handler's read until
            // after the production reconciliation GET and queue cutoff.
            accepted.notified().await;
            let mut published = 0;
            reconcile_session_observations(
                &mut listener,
                &mut coordinator,
                audio.as_ref(),
                observation,
                true,
                false,
                |confirmed, _, _| published = confirmed.volume.get(),
            )
            .await
            .unwrap();
            assert_eq!(published, 60);
            tokio::time::advance(Duration::from_millis(1)).await;
            complete_delayed_test_notification(&mut stream, &callback).await;
            let stale = listener.recv().await.unwrap();
            assert!(stale.volume_state.is_none());
            assert!(!stale.volume_changed);
            assert!(stale.settings_changed);
            assert_eq!(audio.current_state().await.unwrap().volume.get(), 60);
            // A genuinely later connection remains usable with polling disabled.
            enqueue_test_notification(&listener, 35).await;
            let fresh = listener.recv().await.unwrap().volume_state.unwrap();
            process_session_observation(
                &mut coordinator,
                audio.as_ref(),
                SessionObservation::Speaker(fresh.state.volume, fresh.state.muted),
                true,
                false,
                |confirmed, _, _| published = confirmed.volume.get(),
            )
            .await
            .unwrap();
            assert_eq!(published, 35);
            assert_eq!(audio.current_state().await.unwrap().volume.get(), 35);
        }
    }

    #[tokio::test(start_paused = true)]
    async fn queued_notification_first_at_deadline_preserves_settling_read() {
        let mut listener =
            CallbackListener::bind("127.0.0.1:0".parse().unwrap(), "127.0.0.1".parse().unwrap())
                .await
                .unwrap();
        let local = LocalAudioState {
            volume: NormalizedVolume::new(60).unwrap(),
            muted: MuteState(false),
        };
        let audio: Arc<dyn SystemAudioController> = Arc::new(SessionAudio(Mutex::new(local)));
        let mut coordinator = Coordinator::new(
            Synchronizer::new(
                speaker_volume_bridge_domain::VolumeMapping::Linear,
                SonosVolume::MAX,
                true,
                true,
                false,
            )
            .unwrap(),
            SessionSonos(Arc::new(Mutex::new((
                SonosVolume::new(20).unwrap(),
                MuteState(false),
            )))),
            DeviceAudioPort(Arc::clone(&audio)),
            SynchronizationPolicy::default(),
        );
        coordinator.reconcile_startup().await.unwrap();
        audio
            .set_volume(local.volume, LocalOrigin::User)
            .await
            .unwrap();
        coordinator
            .on_local_event(local, LocalOrigin::User, false)
            .await
            .unwrap();
        coordinator.run_local_once().await.unwrap();
        let deadline = coordinator.settling_deadline().unwrap();
        tokio::time::sleep_until(deadline).await;
        enqueue_test_notification(&listener, 20).await;
        // Deliberately execute the notification branch first when both branches
        // are ready, before the fresh-read queue fence exists.
        let stale = listener.recv().await.unwrap().volume_state.unwrap();
        let mut published = (20, "waiting");
        process_session_observation(
            &mut coordinator,
            audio.as_ref(),
            SessionObservation::Speaker(stale.state.volume, stale.state.muted),
            true,
            false,
            |confirmed, _, status| {
                published = (confirmed.volume.get(), status_name(&status));
            },
        )
        .await
        .unwrap();
        assert_eq!(published, (20, "waiting"));
        assert_eq!(audio.current_state().await.unwrap(), local);
        assert_eq!(coordinator.settling_deadline(), Some(deadline));
        let read_before = coordinator.last_read_completed_at();
        reconcile_session_observations(
            &mut listener,
            &mut coordinator,
            audio.as_ref(),
            SessionObservation::Settled,
            true,
            false,
            |confirmed, _, status| {
                published = (confirmed.volume.get(), status_name(&status));
            },
        )
        .await
        .unwrap();
        assert_eq!(published, (60, "synchronized"));
        assert!(coordinator.last_read_completed_at() > read_before);
        assert_eq!(coordinator.settling_deadline(), None);
        // Authority is released by the read, allowing later speaker changes.
        tokio::time::advance(Duration::from_millis(1)).await;
        enqueue_test_notification(&listener, 35).await;
        let fresh = listener.recv().await.unwrap().volume_state.unwrap();
        process_session_observation(
            &mut coordinator,
            audio.as_ref(),
            SessionObservation::Speaker(fresh.state.volume, fresh.state.muted),
            true,
            false,
            |confirmed, _, status| {
                published = (confirmed.volume.get(), status_name(&status));
            },
        )
        .await
        .unwrap();
        assert_eq!(published, (35, "synchronized"));
        assert_eq!(audio.current_state().await.unwrap().volume.get(), 35);
    }

    #[tokio::test(start_paused = true)]
    async fn settling_timer_before_queued_notification_keeps_fresh_state() {
        let mut listener =
            CallbackListener::bind("127.0.0.1:0".parse().unwrap(), "127.0.0.1".parse().unwrap())
                .await
                .unwrap();
        let local = LocalAudioState {
            volume: NormalizedVolume::new(60).unwrap(),
            muted: MuteState(false),
        };
        let audio: Arc<dyn SystemAudioController> = Arc::new(SessionAudio(Mutex::new(local)));
        let mut coordinator = Coordinator::new(
            Synchronizer::new(
                speaker_volume_bridge_domain::VolumeMapping::Linear,
                SonosVolume::MAX,
                true,
                true,
                false,
            )
            .unwrap(),
            SessionSonos(Arc::new(Mutex::new((
                SonosVolume::new(20).unwrap(),
                MuteState(false),
            )))),
            DeviceAudioPort(Arc::clone(&audio)),
            SynchronizationPolicy::default(),
        );
        coordinator.reconcile_startup().await.unwrap();
        audio
            .set_volume(local.volume, LocalOrigin::User)
            .await
            .unwrap();
        coordinator
            .on_local_event(local, LocalOrigin::User, false)
            .await
            .unwrap();
        coordinator.run_local_once().await.unwrap();
        tokio::time::sleep_until(coordinator.settling_deadline().unwrap()).await;
        // Acknowledgement proves the old notification is already in the real
        // production listener queue. Execute the timer branch first deliberately.
        enqueue_test_notification(&listener, 20).await;
        let mut published = 0;
        reconcile_session_observations(
            &mut listener,
            &mut coordinator,
            audio.as_ref(),
            SessionObservation::Settled,
            true,
            true,
            |confirmed, _, _| published = confirmed.volume.get(),
        )
        .await
        .unwrap();
        assert_eq!(published, 60);
        let stale = listener.recv().await.unwrap();
        assert!(
            stale.settings_changed,
            "Do not lose independent speaker settings invalidations"
        );
        assert!(stale.volume_state.is_none());
        assert!(
            !stale.volume_changed,
            "Do not turn stale values into another health read"
        );
        assert_eq!(audio.current_state().await.unwrap(), local);
        // Later independent notifications remain usable.
        tokio::time::advance(Duration::from_millis(1)).await;
        enqueue_test_notification(&listener, 35).await;
        let fresh = listener.recv().await.unwrap().volume_state.unwrap();
        process_session_observation(
            &mut coordinator,
            audio.as_ref(),
            SessionObservation::Speaker(fresh.state.volume, fresh.state.muted),
            true,
            true,
            |confirmed, _, _| published = confirmed.volume.get(),
        )
        .await
        .unwrap();
        assert_eq!(published, 35);
        assert_eq!(audio.current_state().await.unwrap().volume.get(), 35);
    }

    #[tokio::test(start_paused = true)]
    async fn session_settling_preserves_subscription_health() {
        for (subscribed, fallback, expected) in [
            (false, false, "subscription_degraded"),
            (false, true, "polling_fallback"),
            (true, false, "synchronized"),
        ] {
            let local = LocalAudioState {
                volume: NormalizedVolume::new(60).unwrap(),
                muted: MuteState(false),
            };
            let audio: Arc<dyn SystemAudioController> = Arc::new(SessionAudio(Mutex::new(local)));
            let mut coordinator = Coordinator::new(
                Synchronizer::new(
                    speaker_volume_bridge_domain::VolumeMapping::Linear,
                    SonosVolume::MAX,
                    true,
                    true,
                    false,
                )
                .unwrap(),
                SessionSonos(Arc::new(Mutex::new((
                    SonosVolume::new(20).unwrap(),
                    MuteState(false),
                )))),
                DeviceAudioPort(Arc::clone(&audio)),
                SynchronizationPolicy::default(),
            );
            coordinator.reconcile_startup().await.unwrap();
            audio
                .set_volume(local.volume, LocalOrigin::User)
                .await
                .unwrap();
            coordinator
                .on_local_event(local, LocalOrigin::User, false)
                .await
                .unwrap();
            coordinator.run_local_once().await.unwrap();
            if !subscribed {
                coordinator.on_subscription_lost();
            }
            let mut published = None;
            process_session_observation(
                &mut coordinator,
                audio.as_ref(),
                SessionObservation::Settled,
                subscribed,
                fallback,
                |confirmed, local, status| {
                    published = Some((
                        confirmed.volume.get(),
                        local.volume.get(),
                        status_name(&status),
                    ));
                },
            )
            .await
            .unwrap();
            assert_eq!(published, Some((60, 60, expected)));
        }
    }

    #[tokio::test(start_paused = true)]
    async fn session_rejected_observations_preserve_waiting_publication() {
        let local = LocalAudioState {
            volume: NormalizedVolume::new(60).unwrap(),
            muted: MuteState(false),
        };
        let audio: Arc<dyn SystemAudioController> = Arc::new(SessionAudio(Mutex::new(local)));
        let mut coordinator = Coordinator::new(
            Synchronizer::new(
                speaker_volume_bridge_domain::VolumeMapping::Linear,
                SonosVolume::MAX,
                true,
                true,
                false,
            )
            .unwrap(),
            SessionSonos(Arc::new(Mutex::new((
                SonosVolume::new(20).unwrap(),
                MuteState(false),
            )))),
            DeviceAudioPort(Arc::clone(&audio)),
            SynchronizationPolicy::default(),
        );
        coordinator.reconcile_startup().await.unwrap();
        audio
            .set_volume(local.volume, LocalOrigin::User)
            .await
            .unwrap();
        coordinator
            .on_local_event(local, LocalOrigin::User, false)
            .await
            .unwrap();
        let mut published = (20, 60, "waiting");
        // A stale notification while queued, then a stale event or health read
        // after the write but before the local-authority deadline.
        for queued in [true, false] {
            if !queued {
                coordinator.run_local_once().await.unwrap();
            }
            process_session_observation(
                &mut coordinator,
                audio.as_ref(),
                SessionObservation::Speaker(SonosVolume::new(20).unwrap(), MuteState(true)),
                true,
                true,
                |confirmed, local, status| {
                    published = (
                        confirmed.volume.get(),
                        local.volume.get(),
                        status_name(&status),
                    );
                },
            )
            .await
            .unwrap();
            assert_eq!(published, (20, 60, "waiting"));
            assert_eq!(audio.current_state().await.unwrap(), local);
        }
        process_session_observation(
            &mut coordinator,
            audio.as_ref(),
            SessionObservation::Settled,
            true,
            true,
            |confirmed, local, status| {
                published = (
                    confirmed.volume.get(),
                    local.volume.get(),
                    status_name(&status),
                );
            },
        )
        .await
        .unwrap();
        assert_eq!(published, (60, 60, "synchronized"));
        process_session_observation(
            &mut coordinator,
            audio.as_ref(),
            SessionObservation::Speaker(SonosVolume::new(35).unwrap(), MuteState(false)),
            true,
            true,
            |confirmed, local, status| {
                published = (
                    confirmed.volume.get(),
                    local.volume.get(),
                    status_name(&status),
                );
            },
        )
        .await
        .unwrap();
        assert_eq!(published, (35, 35, "synchronized"));
    }

    #[tokio::test]
    async fn buffered_audio_callbacks_keep_latest_gesture_and_device_failures() {
        let (sender, mut receiver) = broadcast::channel(16);
        let event = |value, origin| SystemAudioEvent::StateChanged {
            state: LocalAudioState {
                volume: NormalizedVolume::new(value).unwrap(),
                muted: MuteState(false),
            },
            origin,
        };
        sender.send(event(37, LocalOrigin::User)).unwrap();
        let first = receiver.recv().await;
        for value in [44, 51, 63, 69] {
            sender.send(event(value, LocalOrigin::User)).unwrap();
        }
        sender.send(event(44, LocalOrigin::Application)).unwrap();
        assert_eq!(
            newest_audio_event(first, &mut receiver).unwrap(),
            event(69, LocalOrigin::User)
        );
        sender.send(event(75, LocalOrigin::User)).unwrap();
        let first = receiver.recv().await;
        sender
            .send(SystemAudioEvent::DeviceUnavailable { device_id: None })
            .unwrap();
        sender.send(event(80, LocalOrigin::User)).unwrap();
        assert_eq!(
            newest_audio_event(first, &mut receiver).unwrap(),
            SystemAudioEvent::DeviceUnavailable { device_id: None }
        );
    }

    struct MuteUnavailableAudio {
        events: broadcast::Sender<SystemAudioEvent>,
        applied_volume: Arc<Mutex<Option<NormalizedVolume>>>,
    }

    #[async_trait]
    impl SystemAudioController for MuteUnavailableAudio {
        async fn current_state(&self) -> Result<LocalAudioState, PlatformAudioError> {
            Ok(LocalAudioState {
                volume: NormalizedVolume::new(20).expect("valid test volume"),
                muted: MuteState(false),
            })
        }

        async fn set_volume(
            &self,
            volume: NormalizedVolume,
            _: LocalOrigin,
        ) -> Result<(), PlatformAudioError> {
            *self.applied_volume.lock().expect("test mutex") = Some(volume);
            Ok(())
        }

        async fn set_muted(&self, muted: bool, _: LocalOrigin) -> Result<(), PlatformAudioError> {
            assert!(muted, "unchanged mute must not be written");
            Err(PlatformAudioError::MuteUnavailable)
        }

        fn subscribe(&self) -> broadcast::Receiver<SystemAudioEvent> {
            self.events.subscribe()
        }
    }

    #[tokio::test]
    async fn unsupported_mute_does_not_block_volume_application() {
        let (events, _) = broadcast::channel(1);
        let applied_volume = Arc::new(Mutex::new(None));
        let port = DeviceAudioPort(Arc::new(MuteUnavailableAudio {
            events,
            applied_volume: Arc::clone(&applied_volume),
        }));
        let state = LocalAudioState {
            volume: NormalizedVolume::new(42).expect("valid test volume"),
            muted: MuteState(true),
        };

        port.apply(state)
            .await
            .expect("volume application succeeds");

        assert_eq!(
            *applied_volume.lock().expect("test mutex"),
            Some(state.volume)
        );
    }

    #[tokio::test]
    async fn unchanged_confirmation_does_not_write_local_audio() {
        let (events, _) = broadcast::channel(1);
        let applied_volume = Arc::new(Mutex::new(None));
        let port = DeviceAudioPort(Arc::new(MuteUnavailableAudio {
            events,
            applied_volume: Arc::clone(&applied_volume),
        }));
        port.apply(LocalAudioState {
            volume: NormalizedVolume::new(20).unwrap(),
            muted: MuteState(false),
        })
        .await
        .unwrap();
        assert_eq!(*applied_volume.lock().unwrap(), None);
    }

    #[test]
    fn renderer_suffix_is_hidden_from_speaker_name() {
        assert_eq!(
            display_speaker_name("Office - Sonos Ray Media Renderer - RINCON_38420BBDB00301400"),
            "Office"
        );
    }

    #[test]
    fn ordinary_speaker_name_is_preserved() {
        assert_eq!(display_speaker_name("Living Room"), "Living Room");
    }

    #[test]
    fn renewal_happens_before_subscription_expiry() {
        let subscription = speaker_volume_bridge_sonos::Subscription {
            id: "uuid:test".to_owned(),
            timeout: Duration::from_secs(10),
        };
        let remaining = renewal_at(&subscription).saturating_duration_since(Instant::now());
        assert!(remaining < subscription.timeout);
        assert!(remaining >= Duration::from_secs(7));
    }
    #[test]
    fn reconnect_delay_is_bounded() {
        assert_eq!(
            (MAX_RECONNECT_DELAY * 2).min(MAX_RECONNECT_DELAY),
            MAX_RECONNECT_DELAY
        );
    }

    #[test]
    fn discoverable_sonos_filter_accepts_rincon_ids() {
        let sonos_device = SonosDevice {
            id: SonosId::new("uuid:RINCON_00000000000101400").unwrap(),
            friendly_name: "Office".to_owned(),
            model_name: None,
            model_number: None,
            rendering_control: RenderingControlService {
                control_url: Url::parse("http://192.168.1.10:1400/control").unwrap(),
                event_url: Url::parse("http://192.168.1.10:1400/event").unwrap(),
            },
            av_transport: None,
            device_properties: None,
        };
        assert!(is_sonos_speaker(&sonos_device));
    }

    #[test]
    fn discoverable_sonos_filter_rejects_non_rincon_ids() {
        let media_renderer_device = SonosDevice {
            id: SonosId::new("uuid:AA-BB-CC").unwrap(),
            friendly_name: "Bedroom".to_owned(),
            model_name: None,
            model_number: None,
            rendering_control: RenderingControlService {
                control_url: Url::parse("http://192.168.1.11:1400/control").unwrap(),
                event_url: Url::parse("http://192.168.1.11:1400/event").unwrap(),
            },
            av_transport: None,
            device_properties: None,
        };
        assert!(!is_sonos_speaker(&media_renderer_device));
    }
}
