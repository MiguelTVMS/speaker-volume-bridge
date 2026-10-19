//! Async orchestration ports and a bounded synchronization coordinator.
//!
//! This crate knows no native platform API, HTTP implementation, or Tauri type.

use async_trait::async_trait;
use speaker_volume_bridge_domain::{LocalAudioState, LocalOrigin, MuteState, SonosVolume};
use speaker_volume_bridge_synchronization::{
    Effect, SonosObservationSource, SyncEvent, Synchronizer,
};
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::time::{Instant as Deadline, sleep_until};

#[derive(Debug, Error)]
pub enum IntegrationError {
    #[error("Sonos port failed: {0}")]
    Sonos(String),
    #[error("local audio port failed: {0}")]
    Audio(String),
    #[error("synchronizer rejected an event: {0}")]
    State(String),
}

#[async_trait]
pub trait SonosPort: Send + Sync {
    async fn current_state(&self) -> Result<(SonosVolume, MuteState), IntegrationError>;
    async fn set_volume(&self, volume: SonosVolume) -> Result<(), IntegrationError>;
    async fn set_mute(&self, muted: MuteState) -> Result<(), IntegrationError>;
}

#[async_trait]
pub trait LocalAudioPort: Send + Sync {
    async fn apply(&self, state: LocalAudioState) -> Result<(), IntegrationError>;
    async fn observe(&self) -> Result<Option<LocalAudioState>, IntegrationError> {
        Ok(None)
    }
    /// Return newer local input instead of overwriting a changed read baseline.
    async fn apply_observed(
        &self,
        state: LocalAudioState,
        _baseline: Option<LocalAudioState>,
    ) -> Result<Option<LocalAudioState>, IntegrationError> {
        self.apply(state).await?;
        Ok(None)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Health {
    Healthy,
    SubscriptionDegraded,
    PollingFallback,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TimingMetrics {
    pub last_command_to_confirmation: Option<Duration>,
}

#[derive(Clone, Copy, Debug)]
pub struct SynchronizationPolicy {
    pub volume_debounce: Duration,
    pub unhealthy_poll_interval: Duration,
    pub healthy_poll_interval: Duration,
}
impl Default for SynchronizationPolicy {
    fn default() -> Self {
        Self {
            volume_debounce: Duration::from_millis(45),
            unhealthy_poll_interval: Duration::from_secs(1),
            healthy_poll_interval: Duration::from_secs(30),
        }
    }
}

const LOCAL_SETTLING: Duration = Duration::from_millis(500);

/// Coalesces only volume writes. Mute writes bypass debounce and are dispatched immediately.
pub struct Coordinator<S, A> {
    synchronizer: Synchronizer,
    sonos: S,
    audio: A,
    policy: SynchronizationPolicy,
    health: Health,
    pending_local: Option<SyncEvent>,
    local_deadline: Option<Deadline>,
    local_authority_until: Option<Deadline>,
    last_local: Option<LocalAudioState>,
    last_command: Option<Instant>,
    last_sent_volume: Option<SonosVolume>,
    last_sent_mute: Option<MuteState>,
    metrics: TimingMetrics,
    observation_baseline: Option<LocalAudioState>,
    expected_local: Option<LocalAudioState>,
    read_revision: u64,
}

impl<S: SonosPort, A: LocalAudioPort> Coordinator<S, A> {
    pub fn new(
        synchronizer: Synchronizer,
        sonos: S,
        audio: A,
        policy: SynchronizationPolicy,
    ) -> Self {
        Self {
            synchronizer,
            sonos,
            audio,
            policy,
            health: Health::SubscriptionDegraded,
            pending_local: None,
            local_deadline: None,
            local_authority_until: None,
            last_local: None,
            last_command: None,
            last_sent_volume: None,
            last_sent_mute: None,
            metrics: TimingMetrics::default(),
            observation_baseline: None,
            expected_local: None,
            read_revision: 0,
        }
    }
    /// A fixed deadline lets the runtime continue receiving callbacks while coalescing.
    pub const fn local_deadline(&self) -> Option<Deadline> {
        self.local_deadline
    }
    /// Read the final speaker state even when fallback polling is disabled.
    pub fn settling_deadline(&self) -> Option<Deadline> {
        self.local_authority_until
            .filter(|_| self.pending_local.is_none())
    }
    pub async fn finish_local_adjustment(&mut self) -> Result<(), IntegrationError> {
        if let Some(deadline) = self.settling_deadline() {
            sleep_until(deadline).await;
            // Retain authority until the explicit read returns, regardless of
            // which ready select branch wins at the deadline.
            self.reconcile_startup().await?;
        }
        Ok(())
    }
    fn local_is_authoritative(&self) -> bool {
        self.pending_local.is_some() || self.local_authority_until.is_some()
    }
    pub const fn health(&self) -> Health {
        self.health
    }
    pub const fn confirmed_state(
        &self,
    ) -> Option<speaker_volume_bridge_domain::ConfirmedSonosState> {
        self.synchronizer.confirmed()
    }
    pub const fn read_revision(&self) -> u64 {
        self.read_revision
    }
    pub const fn timing_metrics(&self) -> TimingMetrics {
        self.metrics
    }
    /// An already attached new output needs reconciliation, not a network reconnect.
    pub async fn reconcile_output_change(
        &mut self,
        local: LocalAudioState,
    ) -> Result<(), IntegrationError> {
        self.pending_local = None;
        self.local_deadline = None;
        self.last_sent_volume = None;
        self.last_sent_mute = None;
        self.expected_local = None;
        self.reconcile_startup().await?;
        if !self.synchronizer.two_way_synchronization() && self.pending_local.is_none() {
            let local = self.audio.observe().await?.unwrap_or(local);
            self.on_local_event(local, LocalOrigin::User, true).await?;
        }
        Ok(())
    }
    pub async fn reconcile_startup(&mut self) -> Result<(), IntegrationError> {
        self.read_confirmation(SonosObservationSource::ExplicitRead)
            .await
    }
    fn retain_local_input(&mut self, state: LocalAudioState) {
        self.invalidate_sonos_commands();
        self.last_local = Some(state);
        self.expected_local = Some(state);
        self.pending_local = Some(SyncEvent::LocalChanged {
            state,
            origin: LocalOrigin::User,
            at_ms: now_ms(),
        });
        self.local_deadline = Some(Deadline::now());
        self.local_authority_until = Some(Deadline::now() + LOCAL_SETTLING);
    }
    async fn read_confirmation(
        &mut self,
        source: SonosObservationSource,
    ) -> Result<(), IntegrationError> {
        let baseline = self.audio.observe().await?;
        if let (Some(expected), Some(current)) = (self.expected_local, baseline)
            && current != expected
        {
            self.retain_local_input(current);
            return Ok(());
        }
        let (volume, muted) = self.sonos.current_state().await?;
        self.read_revision = self.read_revision.wrapping_add(1);
        self.record_confirmation(volume, muted);
        let current = self.audio.observe().await?;
        if current != baseline {
            if let Some(current) = current {
                self.retain_local_input(current);
            }
            return Ok(());
        }
        self.last_local = None;
        self.local_authority_until = None;
        self.observation_baseline = baseline;
        let result = self
            .handle(SyncEvent::SonosConfirmed {
                volume,
                muted,
                source,
                at_ms: now_ms(),
            })
            .await;
        self.observation_baseline = None;
        result
    }
    /// Returns whether the observation was accepted, so callers can publish only confirmed state.
    pub async fn on_sonos_event(
        &mut self,
        volume: SonosVolume,
        muted: MuteState,
    ) -> Result<bool, IntegrationError> {
        self.health = Health::Healthy;
        // Even a suppressed observation proves a cached command may no longer hold.
        self.record_confirmation(volume, muted);
        if self.local_is_authoritative() {
            return Ok(false);
        }
        let baseline = self.audio.observe().await?;
        if let (Some(expected), Some(current)) = (self.expected_local, baseline)
            && current != expected
        {
            self.retain_local_input(current);
            return Ok(false);
        }
        self.last_local = None;
        self.local_authority_until = None;
        self.observation_baseline = baseline;
        self.handle(SyncEvent::SonosConfirmed {
            volume,
            muted,
            source: SonosObservationSource::Event,
            at_ms: now_ms(),
        })
        .await?;
        self.observation_baseline = None;
        Ok(self.pending_local.is_none())
    }
    pub fn invalidate_sonos_commands(&mut self) {
        self.last_sent_volume = None;
        self.last_sent_mute = None;
    }
    pub fn on_subscription_lost(&mut self) {
        self.health = Health::PollingFallback;
    }
    pub async fn poll_once(&mut self) -> Result<bool, IntegrationError> {
        if self.local_is_authoritative() {
            return Ok(false);
        }
        self.read_confirmation(SonosObservationSource::Poll).await?;
        Ok(self.pending_local.is_none())
    }
    pub fn next_poll_interval(&self) -> Duration {
        if self.health == Health::Healthy {
            self.policy.healthy_poll_interval
        } else {
            self.policy.unhealthy_poll_interval
        }
    }
    pub async fn run_local_once(&mut self) -> Result<(), IntegrationError> {
        if let Some(deadline) = self.local_deadline {
            sleep_until(deadline).await;
        }
        self.local_deadline = None;
        if let Some(event) = self.pending_local.take() {
            self.handle(event).await?;
        }
        Ok(())
    }
    pub async fn on_local_event(
        &mut self,
        state: LocalAudioState,
        origin: LocalOrigin,
        mute_changed: bool,
    ) -> Result<(), IntegrationError> {
        let event = SyncEvent::LocalChanged {
            state,
            origin,
            at_ms: now_ms(),
        };
        if origin == LocalOrigin::Application {
            return self.handle(event).await;
        }
        if self.last_local == Some(state) {
            return Ok(());
        }
        if !self.local_is_authoritative() {
            self.invalidate_sonos_commands();
        }
        self.last_local = Some(state);
        self.expected_local = Some(state);
        self.local_authority_until = Some(Deadline::now() + LOCAL_SETTLING);
        if mute_changed {
            // A mute gesture includes the latest volume and supersedes any queued volume.
            self.pending_local = None;
            self.local_deadline = None;
            self.handle(event).await
        } else {
            self.pending_local = Some(event);
            self.local_deadline
                .get_or_insert_with(|| Deadline::now() + self.policy.volume_debounce);
            Ok(())
        }
    }
    async fn handle(&mut self, event: SyncEvent) -> Result<(), IntegrationError> {
        let effects = self
            .synchronizer
            .handle(event)
            .map_err(|error| IntegrationError::State(error.to_string()))?;
        for effect in effects {
            self.apply(effect).await?;
        }
        Ok(())
    }
    fn record_confirmation(&mut self, volume: SonosVolume, muted: MuteState) {
        if self.last_sent_volume.is_some_and(|sent| sent != volume) {
            self.last_sent_volume = None;
        }
        if self.last_sent_mute.is_some_and(|sent| sent != muted) {
            self.last_sent_mute = None;
        }
    }
    async fn apply(&mut self, effect: Effect) -> Result<(), IntegrationError> {
        match effect {
            Effect::RequestSonosVolume(volume) => {
                if self.last_sent_volume == Some(volume) {
                    return Ok(());
                }
                self.last_command = Some(Instant::now());
                self.sonos.set_volume(volume).await?;
                self.last_sent_volume = Some(volume);
                self.local_authority_until = Some(Deadline::now() + LOCAL_SETTLING);
                Ok(())
            }
            Effect::RequestSonosMute(muted) => {
                if self.last_sent_mute == Some(muted) {
                    return Ok(());
                }
                self.last_command = Some(Instant::now());
                self.sonos.set_mute(muted).await?;
                self.last_sent_mute = Some(muted);
                self.local_authority_until = Some(Deadline::now() + LOCAL_SETTLING);
                Ok(())
            }
            Effect::ApplyLocal(state) => {
                self.metrics.last_command_to_confirmation =
                    self.last_command.map(|start| start.elapsed());
                if let Some(newer) = self
                    .audio
                    .apply_observed(state, self.observation_baseline)
                    .await?
                {
                    self.retain_local_input(newer);
                } else {
                    self.expected_local = self.audio.observe().await?;
                }
                Ok(())
            }
            Effect::SuppressedLocalCallback => Ok(()),
        }
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use speaker_volume_bridge_domain::{NormalizedVolume, VolumeMapping};
    use std::sync::{Arc, Mutex};
    struct Sonos(Arc<Mutex<(SonosVolume, MuteState)>>);
    #[async_trait]
    impl SonosPort for Sonos {
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
    struct Audio(Arc<Mutex<Vec<LocalAudioState>>>);
    #[async_trait]
    impl LocalAudioPort for Audio {
        async fn apply(&self, state: LocalAudioState) -> Result<(), IntegrationError> {
            self.0.lock().unwrap().push(state);
            Ok(())
        }
    }
    fn volume(value: u8) -> SonosVolume {
        SonosVolume::new(value).unwrap()
    }
    fn local(value: u8, muted: bool) -> LocalAudioState {
        LocalAudioState {
            volume: NormalizedVolume::new(value).unwrap(),
            muted: MuteState(muted),
        }
    }

    #[tokio::test(start_paused = true)]
    async fn rejected_observation_invalidates_prior_volume_and_mute_commands() {
        let state = Arc::new(Mutex::new((volume(20), MuteState(false))));
        let applied = Arc::new(Mutex::new(Vec::new()));
        let mut coordinator = Coordinator::new(
            Synchronizer::new(VolumeMapping::Linear, SonosVolume::MAX, true, true, false).unwrap(),
            Sonos(Arc::clone(&state)),
            Audio(Arc::clone(&applied)),
            SynchronizationPolicy::default(),
        );
        coordinator.reconcile_startup().await.unwrap();
        coordinator
            .on_local_event(local(60, true), LocalOrigin::User, true)
            .await
            .unwrap();
        coordinator.finish_local_adjustment().await.unwrap();
        assert_eq!(*state.lock().unwrap(), (volume(60), MuteState(true)));
        // Another gesture begins before the differing external observation arrives.
        coordinator
            .on_local_event(local(50, false), LocalOrigin::User, false)
            .await
            .unwrap();
        *state.lock().unwrap() = (volume(30), MuteState(false));
        assert!(
            !coordinator
                .on_sonos_event(volume(30), MuteState(false))
                .await
                .unwrap()
        );
        assert_eq!(applied.lock().unwrap().last(), Some(&local(60, true)));
        coordinator
            .on_local_event(local(60, true), LocalOrigin::User, true)
            .await
            .unwrap();
        coordinator.run_local_once().await.unwrap();
        assert_eq!(*state.lock().unwrap(), (volume(60), MuteState(true)));
        coordinator.finish_local_adjustment().await.unwrap();
        assert_eq!(coordinator.confirmed_state().unwrap().volume, volume(60));
        assert_eq!(
            coordinator.confirmed_state().unwrap().muted,
            MuteState(true)
        );
    }

    #[tokio::test(start_paused = true)]
    async fn regression_rapid_callbacks_coalesce_and_stale_confirmations_do_not_reverse_volume() {
        let state = Arc::new(Mutex::new((volume(20), MuteState(false))));
        let applied = Arc::new(Mutex::new(Vec::new()));
        let machine =
            Synchronizer::new(VolumeMapping::Linear, SonosVolume::MAX, true, true, false).unwrap();
        let mut coordinator = Coordinator::new(
            machine,
            Sonos(Arc::clone(&state)),
            Audio(Arc::clone(&applied)),
            SynchronizationPolicy::default(),
        );
        coordinator.reconcile_startup().await.unwrap();
        applied.lock().unwrap().clear();
        for value in [37, 37, 44, 44, 51, 63, 69] {
            coordinator
                .on_local_event(local(value, false), LocalOrigin::User, false)
                .await
                .unwrap();
            // Notifications for the previous speaker value arrive during keyboard repeat.
            coordinator
                .on_sonos_event(volume(20), MuteState(false))
                .await
                .unwrap();
        }
        assert!(applied.lock().unwrap().is_empty());
        coordinator.run_local_once().await.unwrap();
        assert_eq!(state.lock().unwrap().0, volume(69));
        for value in [37, 44, 51, 63, 69, 44] {
            coordinator
                .on_sonos_event(volume(value), MuteState(false))
                .await
                .unwrap();
        }
        assert!(applied.lock().unwrap().is_empty());
        tokio::time::advance(Duration::from_millis(501)).await;
        assert!(!coordinator.poll_once().await.unwrap());
        assert!(applied.lock().unwrap().is_empty());
        assert!(coordinator.settling_deadline().is_some());
        coordinator.finish_local_adjustment().await.unwrap();
        assert_eq!(applied.lock().unwrap().as_slice(), &[local(69, false)]);
        // Speaker-side changes resume after the local gesture settles.
        coordinator
            .on_sonos_event(volume(35), MuteState(false))
            .await
            .unwrap();
        assert_eq!(applied.lock().unwrap().last(), Some(&local(35, false)));
    }

    #[tokio::test(start_paused = true)]
    async fn regression_delayed_confirmation_cannot_reverse_a_dispatched_gesture() {
        let state = Arc::new(Mutex::new((volume(20), MuteState(false))));
        let applied = Arc::new(Mutex::new(Vec::new()));
        let machine =
            Synchronizer::new(VolumeMapping::Linear, SonosVolume::MAX, true, true, false).unwrap();
        let mut coordinator = Coordinator::new(
            machine,
            Sonos(Arc::clone(&state)),
            Audio(Arc::clone(&applied)),
            SynchronizationPolicy::default(),
        );
        coordinator.reconcile_startup().await.unwrap();
        applied.lock().unwrap().clear();
        for value in [37, 44, 51, 69] {
            coordinator
                .on_local_event(local(value, false), LocalOrigin::User, false)
                .await
                .unwrap();
            coordinator.run_local_once().await.unwrap();
            coordinator
                .on_sonos_event(volume(20), MuteState(false))
                .await
                .unwrap();
            assert!(applied.lock().unwrap().is_empty());
        }
        assert_eq!(state.lock().unwrap().0, volume(69));
    }

    #[tokio::test(start_paused = true)]
    async fn regression_mute_supersedes_queued_volume_without_replaying_it() {
        let state = Arc::new(Mutex::new((volume(20), MuteState(false))));
        let applied = Arc::new(Mutex::new(Vec::new()));
        let machine =
            Synchronizer::new(VolumeMapping::Linear, SonosVolume::MAX, true, true, false).unwrap();
        let mut coordinator = Coordinator::new(
            machine,
            Sonos(Arc::clone(&state)),
            Audio(applied),
            SynchronizationPolicy::default(),
        );
        coordinator.reconcile_startup().await.unwrap();
        coordinator
            .on_local_event(local(30, false), LocalOrigin::User, false)
            .await
            .unwrap();
        coordinator
            .on_local_event(local(50, true), LocalOrigin::User, true)
            .await
            .unwrap();
        assert_eq!(*state.lock().unwrap(), (volume(50), MuteState(true)));
        coordinator.run_local_once().await.unwrap();
        assert_eq!(*state.lock().unwrap(), (volume(50), MuteState(true)));
    }

    #[tokio::test(start_paused = true)]
    async fn settling_read_recovers_external_change_and_duplicates_do_not_extend_deadline() {
        let state = Arc::new(Mutex::new((volume(20), MuteState(false))));
        let applied = Arc::new(Mutex::new(Vec::new()));
        let machine =
            Synchronizer::new(VolumeMapping::Linear, SonosVolume::MAX, true, true, false).unwrap();
        let mut coordinator = Coordinator::new(
            machine,
            Sonos(Arc::clone(&state)),
            Audio(Arc::clone(&applied)),
            SynchronizationPolicy::default(),
        );
        coordinator.reconcile_startup().await.unwrap();
        coordinator
            .on_local_event(local(69, false), LocalOrigin::User, false)
            .await
            .unwrap();
        let deadline = coordinator.local_deadline();
        tokio::time::advance(Duration::from_millis(20)).await;
        coordinator
            .on_local_event(local(69, false), LocalOrigin::User, false)
            .await
            .unwrap();
        assert_eq!(coordinator.local_deadline(), deadline);
        coordinator.run_local_once().await.unwrap();
        let settling = coordinator.settling_deadline();
        coordinator
            .on_local_event(local(69, false), LocalOrigin::User, false)
            .await
            .unwrap();
        assert_eq!(coordinator.settling_deadline(), settling);
        *state.lock().unwrap() = (volume(35), MuteState(true));
        coordinator
            .on_sonos_event(volume(35), MuteState(true))
            .await
            .unwrap();
        assert_eq!(applied.lock().unwrap().last(), Some(&local(20, false)));
        coordinator.finish_local_adjustment().await.unwrap();
        assert_eq!(applied.lock().unwrap().last(), Some(&local(35, true)));
        assert_eq!(coordinator.settling_deadline(), None);
        coordinator
            .on_local_event(local(69, false), LocalOrigin::User, false)
            .await
            .unwrap();
        coordinator.run_local_once().await.unwrap();
        assert_eq!(state.lock().unwrap().0, volume(69));
    }

    #[tokio::test(start_paused = true)]
    async fn settling_preserves_caps_direction_and_application_echo_suppression() {
        for two_way in [false, true] {
            let state = Arc::new(Mutex::new((volume(20), MuteState(false))));
            let applied = Arc::new(Mutex::new(Vec::new()));
            let machine =
                Synchronizer::new(VolumeMapping::Linear, volume(55), true, two_way, false).unwrap();
            let mut coordinator = Coordinator::new(
                machine,
                Sonos(Arc::clone(&state)),
                Audio(Arc::clone(&applied)),
                SynchronizationPolicy::default(),
            );
            coordinator.reconcile_startup().await.unwrap();
            applied.lock().unwrap().clear();
            coordinator
                .on_local_event(local(80, false), LocalOrigin::User, false)
                .await
                .unwrap();
            coordinator.run_local_once().await.unwrap();
            assert_eq!(state.lock().unwrap().0, volume(55));
            coordinator
                .on_sonos_event(volume(20), MuteState(false))
                .await
                .unwrap();
            assert!(applied.lock().unwrap().is_empty());
            coordinator.finish_local_adjustment().await.unwrap();
            let expected = if two_way {
                vec![local(55, false)]
            } else {
                vec![]
            };
            assert_eq!(*applied.lock().unwrap(), expected);
            coordinator
                .on_local_event(local(55, false), LocalOrigin::Application, false)
                .await
                .unwrap();
            assert_eq!(coordinator.local_deadline(), None);
            assert_eq!(coordinator.settling_deadline(), None);
        }
    }

    #[tokio::test(start_paused = true)]
    async fn changed_output_reconciles_without_resetting_subscription_or_replaying_old_input() {
        for two_way in [true, false] {
            let state = Arc::new(Mutex::new((volume(20), MuteState(false))));
            let applied = Arc::new(Mutex::new(Vec::new()));
            let machine =
                Synchronizer::new(VolumeMapping::Linear, volume(55), true, two_way, false).unwrap();
            let mut coordinator = Coordinator::new(
                machine,
                Sonos(Arc::clone(&state)),
                Audio(Arc::clone(&applied)),
                SynchronizationPolicy::default(),
            );
            coordinator.reconcile_startup().await.unwrap();
            coordinator
                .on_sonos_event(volume(20), MuteState(false))
                .await
                .unwrap();
            applied.lock().unwrap().clear();
            coordinator
                .on_local_event(local(80, false), LocalOrigin::User, false)
                .await
                .unwrap();
            coordinator
                .reconcile_output_change(local(10, true))
                .await
                .unwrap();
            assert_eq!(coordinator.health(), Health::Healthy);
            assert_eq!(coordinator.local_deadline(), None);
            coordinator.run_local_once().await.unwrap();
            if two_way {
                assert_eq!(*state.lock().unwrap(), (volume(20), MuteState(false)));
                assert_eq!(*applied.lock().unwrap(), vec![local(20, false)]);
            } else {
                assert_eq!(*state.lock().unwrap(), (volume(10), MuteState(true)));
                assert!(applied.lock().unwrap().is_empty());
            }
        }
    }

    #[tokio::test]
    async fn startup_reconciles_from_sonos_without_writing_back() {
        let state = Arc::new(Mutex::new((volume(24), MuteState(false))));
        let applied = Arc::new(Mutex::new(Vec::new()));
        let machine =
            Synchronizer::new(VolumeMapping::Linear, SonosVolume::MAX, true, true, false).unwrap();
        let mut coordinator = Coordinator::new(
            machine,
            Sonos(Arc::clone(&state)),
            Audio(Arc::clone(&applied)),
            SynchronizationPolicy::default(),
        );
        coordinator.reconcile_startup().await.unwrap();
        assert_eq!(
            applied.lock().unwrap()[0].volume,
            NormalizedVolume::new(24).unwrap()
        );
        assert_eq!(*state.lock().unwrap(), (volume(24), MuteState(false)));
    }
    #[tokio::test]
    async fn one_way_startup_keeps_local_state_and_accepts_local_commands() {
        let state = Arc::new(Mutex::new((volume(24), MuteState(false))));
        let applied = Arc::new(Mutex::new(Vec::new()));
        let machine =
            Synchronizer::new(VolumeMapping::Linear, SonosVolume::MAX, true, false, false).unwrap();
        let mut coordinator = Coordinator::new(
            machine,
            Sonos(Arc::clone(&state)),
            Audio(Arc::clone(&applied)),
            SynchronizationPolicy::default(),
        );

        coordinator.reconcile_startup().await.unwrap();
        assert_eq!(applied.lock().unwrap().as_slice(), []);

        coordinator
            .on_local_event(
                LocalAudioState {
                    volume: NormalizedVolume::new(42).unwrap(),
                    muted: MuteState(false),
                },
                LocalOrigin::User,
                true,
            )
            .await
            .unwrap();
        assert_eq!(state.lock().unwrap().0, volume(42));
    }
    #[tokio::test(start_paused = true)]
    async fn later_sonos_confirmation_wins_over_local_intent() {
        let state = Arc::new(Mutex::new((volume(20), MuteState(false))));
        let applied = Arc::new(Mutex::new(Vec::new()));
        let machine =
            Synchronizer::new(VolumeMapping::Linear, SonosVolume::MAX, true, true, false).unwrap();
        let mut coordinator = Coordinator::new(
            machine,
            Sonos(Arc::clone(&state)),
            Audio(Arc::clone(&applied)),
            SynchronizationPolicy::default(),
        );
        coordinator
            .on_local_event(
                LocalAudioState {
                    volume: NormalizedVolume::new(30).unwrap(),
                    muted: MuteState(false),
                },
                LocalOrigin::User,
                true,
            )
            .await
            .unwrap();
        tokio::time::advance(LOCAL_SETTLING).await;
        coordinator.finish_local_adjustment().await.unwrap();
        coordinator
            .on_sonos_event(volume(35), MuteState(false))
            .await
            .unwrap();
        assert_eq!(
            applied.lock().unwrap().last().unwrap().volume,
            NormalizedVolume::new(35).unwrap()
        );
    }
    #[tokio::test]
    async fn lost_subscription_uses_fast_polling_until_event_recovers() {
        let state = Arc::new(Mutex::new((volume(20), MuteState(false))));
        let applied = Arc::new(Mutex::new(Vec::new()));
        let machine =
            Synchronizer::new(VolumeMapping::Linear, SonosVolume::MAX, true, true, false).unwrap();
        let mut coordinator = Coordinator::new(
            machine,
            Sonos(state),
            Audio(applied),
            SynchronizationPolicy::default(),
        );
        coordinator.on_subscription_lost();
        assert_eq!(coordinator.health(), Health::PollingFallback);
        assert_eq!(coordinator.next_poll_interval(), Duration::from_secs(1));
        coordinator
            .on_sonos_event(volume(20), MuteState(false))
            .await
            .unwrap();
        assert_eq!(coordinator.health(), Health::Healthy);
    }

    #[tokio::test]
    async fn duplicate_sonos_commands_are_suppressed_until_state_changes() {
        let state = Arc::new(Mutex::new((volume(20), MuteState(false))));
        let applied = Arc::new(Mutex::new(Vec::new()));
        let machine =
            Synchronizer::new(VolumeMapping::Linear, SonosVolume::MAX, true, true, false).unwrap();
        let mut coordinator = Coordinator::new(
            machine,
            Sonos(Arc::clone(&state)),
            Audio(applied),
            SynchronizationPolicy::default(),
        );

        coordinator
            .apply(Effect::RequestSonosVolume(volume(15)))
            .await
            .unwrap();
        coordinator
            .apply(Effect::RequestSonosVolume(volume(15)))
            .await
            .unwrap();
        assert_eq!(state.lock().unwrap().0, volume(15));

        coordinator
            .on_sonos_event(volume(22), MuteState(false))
            .await
            .unwrap();
        coordinator
            .apply(Effect::RequestSonosVolume(volume(15)))
            .await
            .unwrap();
        assert_eq!(state.lock().unwrap().0, volume(15));
    }

    #[tokio::test]
    async fn application_callbacks_do_not_fill_the_user_volume_queue() {
        let state = Arc::new(Mutex::new((volume(20), MuteState(false))));
        let applied = Arc::new(Mutex::new(Vec::new()));
        let machine =
            Synchronizer::new(VolumeMapping::Linear, SonosVolume::MAX, true, true, false).unwrap();
        let mut coordinator = Coordinator::new(
            machine,
            Sonos(Arc::clone(&state)),
            Audio(applied),
            SynchronizationPolicy::default(),
        );
        let local_state = LocalAudioState {
            volume: NormalizedVolume::new(42).unwrap(),
            muted: MuteState(false),
        };

        coordinator
            .on_local_event(local_state, LocalOrigin::Application, false)
            .await
            .unwrap();
        coordinator
            .on_local_event(local_state, LocalOrigin::User, false)
            .await
            .unwrap();
        coordinator.run_local_once().await.unwrap();

        assert_eq!(state.lock().unwrap().0, volume(42));
    }
}

pub mod night_mode;
