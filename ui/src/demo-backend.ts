// Lightweight in-memory simulator for development-only browser layout previews.
// No native commands, device discovery, files, or audio APIs are used here.
export function createDemoBackend(
  options: {
    hour12?: boolean | null;
    now?: () => Date;
    edition?: string;
    policyFailure?: boolean;
    policyDelay?: number;
  } = {},
) {
  const snapshot = {
    configuration: {
      schemaVersion: 1,
      nightModeSchedule: {
        enabled: false,
        blocks: Array.from({ length: 7 }, () => Array<boolean>(48).fill(false)),
      },
      notifyNightModeScheduleTransitions: 'never',
      disableLoudnessDuringNightSchedule: false,
      nightScheduleLoudnessRestoreSpeakerId: null,
      selectedSonosId: 'sample-speaker',
      lastKnownSonosAddress: null,
      followDefaultAudioDevice: true,
      fixedAudioDeviceId: null,
      synchronizeMute: true,
      muteSpeakerAtZeroVolume: false,
      twoWaySynchronization: true,
      startAtLogin: false,
      fallbackPolling: true,
      maximumSonosVolume: 70,
      mapping: { type: 'linear' },
    },
    status: 'synchronized',
    sonosName: 'Living Room',
    sonosVolume: 28,
    localVolume: 28,
    muted: false,
  };
  const speakerSettings = {
    nightSound: false,
    loudness: true,
    statusLight: true,
    speechEnhancement: false,
    treble: 0,
    bass: 0,
  };
  const initialSnapshot = structuredClone(snapshot);
  const initialSpeakerSettings = structuredClone(speakerSettings);
  const storeManaged = ['microsoft_store', 'mac_app_store'].includes(options.edition ?? '');
  const updateStatus = {
    phase: storeManaged ? 'store_managed' : 'update_available',
    installedVersion: '1.7.1',
    availableVersion: storeManaged ? null : '1.8.0',
    edition: options.edition ?? 'direct_macos',
    policy: 'stable',
    prereleaseSupported: ['direct_macos', 'direct_windows', 'debian'].includes(
      options.edition ?? 'direct_macos',
    ),
    generation: 0,
    lastSuccessfulCheck: storeManaged ? null : 1791108000,
    action: storeManaged
      ? null
      : { type: 'open_url', url: 'https://svb.miguel.ms/guide/Upgrading.html' },
    message: null,
    automaticChecks: !storeManaged,
    updateNotifications: true,
    promptDismissed: false,
    offerStale: false,
  };
  const dispatch = (command: string, payload?: Record<string, unknown>): unknown => {
    switch (command) {
      case 'plugin:app|version':
        return 'Preview';
      case 'get_schedule_status':
        return {
          active: false,
          supported: true,
          message: snapshot.configuration.nightModeSchedule.enabled
            ? 'Outside scheduled hours. Manual control is available.'
            : 'Schedule disabled.',
          nextTransition: null,
          timeZone: 'Europe/Lisbon',
          notificationsBlocked: false,
        };
      case 'save_night_schedule': {
        snapshot.configuration.nightModeSchedule.blocks = (
          payload as { blocks: boolean[][] }
        ).blocks;
        const now = options.now?.() ?? new Date();
        speakerSettings.nightSound =
          snapshot.configuration.nightModeSchedule.blocks[(now.getDay() + 6) % 7][
            now.getHours() * 2 + Math.floor(now.getMinutes() / 30)
          ];
        return snapshot;
      }
      case 'enable_night_schedule':
        snapshot.configuration.nightModeSchedule.enabled = (
          payload as { enabled: boolean }
        ).enabled;
        return snapshot;
      case 'set_schedule_notifications':
        snapshot.configuration.notifyNightModeScheduleTransitions = (
          payload as { mode: string }
        ).mode;
        return snapshot;
      case 'set_disable_loudness_during_night_schedule':
        snapshot.configuration.disableLoudnessDuringNightSchedule = (
          payload as { enabled: boolean }
        ).enabled;
        return snapshot;
      case 'get_system_hour12':
        return options.hour12 ?? null;
      case 'get_update_status':
      case 'check_for_updates':
        return updateStatus;
      case 'set_update_policy': {
        if (!updateStatus.prereleaseSupported) throw new Error('Unsupported edition');
        updateStatus.policy = String(payload?.policy);
        updateStatus.generation += 1;
        return new Promise((resolve, reject) =>
          setTimeout(() => {
            if (options.policyFailure) reject(new Error('Preview feed unavailable'));
            else resolve({ ...updateStatus });
          }, options.policyDelay ?? 0),
        );
      }
      case 'set_automatic_update_checks':
        updateStatus.automaticChecks = (payload as { enabled: boolean }).enabled;
        return;
      case 'dismiss_update':
        updateStatus.promptDismissed = true;
        return;
      case 'request_update_notification_permission':
        return true;
      case 'set_update_notifications':
        updateStatus.updateNotifications = (payload as { enabled: boolean }).enabled;
        return updateStatus.updateNotifications;
      case 'open_update_store':
      case 'open_update_page':
      case 'open_project_repository':
        return;
      case 'get_snapshot':
        return snapshot;
      case 'save_configuration':
        Object.assign(snapshot.configuration, (payload as { configuration: object }).configuration);
        snapshot.status = snapshot.configuration.selectedSonosId
          ? 'synchronized'
          : 'configuration_required';
        return snapshot;
      case 'discover_sonos':
        return [{ id: 'sample-speaker', friendlyName: 'Living Room', location: '' }];
      case 'list_audio_outputs':
        return [{ id: 'sample-output', name: 'Built-in speakers', writableVolume: true }];
      case 'get_speaker_settings':
        return speakerSettings;
      case 'set_speaker_setting': {
        const { setting, enabled } = payload as { setting: string; enabled: boolean };
        Object.assign(speakerSettings, { [setting]: enabled });
        return;
      }
      case 'set_speaker_level': {
        const { setting, value } = payload as { setting: string; value: number };
        Object.assign(speakerSettings, { [setting]: value });
        return;
      }
      case 'diagnostics':
        return {
          ...snapshot,
          configurationPresent: true,
          speakerName: snapshot.sonosName,
          message: 'Simulated devices — no hardware connected.',
          audioInputFormat: 'Stereo PCM (simulated)',
          sanitized: true,
        };
      case 'export_diagnostics':
        return 'Preview only — no file written.';
      case 'reset_configuration':
        Object.assign(snapshot, structuredClone(initialSnapshot));
        Object.assign(speakerSettings, initialSpeakerSettings);
        return snapshot;
      case 'test_volume':
        snapshot.localVolume = Math.min(snapshot.configuration.maximumSonosVolume, 30);
        snapshot.sonosVolume = snapshot.localVolume;
        return;
      case 'use_tv_audio':
        return;
      default:
        throw new Error(`Unexpected preview command: ${command}`);
    }
  };
  return (command: string, payload?: Record<string, unknown>): unknown => {
    const result = dispatch(command, structuredClone(payload));
    return result instanceof Promise
      ? result.then((value: unknown) => structuredClone(value))
      : structuredClone(result);
  };
}
