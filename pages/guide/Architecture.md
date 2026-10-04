---
layout: guide
---

# Architecture

Speaker Volume Bridge keeps policy, protocol, native audio, integration, and application-shell concerns separate.

```text
Native audio adapter -> Synchronizer -> Sonos adapter
        ^                    |               |
        +---- confirmed state and effects ---+
```

## Layers

The `domain` crate owns validated volume values, mappings, confirmed state, pending intent, and suppression of expected writes.

The `synchronization` crate consumes normalized events and emits side effects. It depends on domain abstractions, not Tauri, networking, or operating-system APIs.

The `integration` crate serializes effects, coalesces rapid volume changes through a bounded channel, and applies confirmed Sonos state back through the native adapter when two-way synchronization permits it.

The `sonos` and `platform-audio` crates are adapters. Platform audio uses Windows Core Audio, macOS Core Audio, or Ubuntu's PulseAudio-compatible `pactl` interface. The Tauri application composes them and provides configuration, the hidden Settings window, tray controls, status snapshots, and restricted frontend commands.

## Runtime lifecycle

Changes to the volume-runtime configuration cancel the previous runtime generation before starting a new one. The runtime:

1. Resolves the selected Sonos identity using its cached local description URL or bounded SSDP discovery, while filtering unrelated media renderers.
2. Opens the selected native audio adapter.
3. Reads the initial Sonos state.
4. Starts a GENA callback listener on the local interface used to reach the speaker.
5. Renews subscriptions before expiry.
6. Uses bounded reconnect backoff and polling fallback when event delivery is unavailable or stale.

Stale runtime generations cannot overwrite the state of a newer configuration.

## Rebrand and upgrade compatibility

The app uses manual old-app removal guidance rather than operating-system process inspection. This keeps process APIs and unreliable cross-platform detection out of the runtime. Users must quit Sonos Volume Bridge and remove its startup entry before running the renamed app.

The product and executable names change while settings, startup, notification, and Store identities remain stable for upgrades. Windows migrates only the former default installation folder; Debian replaces the old package and supplies a compatibility command. See the [rebrand decision](https://github.com/MiguelTVMS/speaker-volume-bridge/blob/v1.6.3/docs/decisions/0018-rebrand-and-legacy-protection.md) and [upgrade guide](/guide/Upgrading.html).

## State and health layers

- Domain synchronization state uses `Connecting`, `Synchronized`, `WaitingForSonosConfirmation`, and `Degraded`.
- Integration health uses `Healthy`, `SubscriptionDegraded`, and `PollingFallback`.
- Runtime and UI status includes `Connecting`, `Synchronized`, `SonosUnavailable`, `LocalAudioUnavailable`, `UnsupportedLocalDevice`, and related states.

Application-originated local callbacks are suppressed so a Sonos-confirmed write does not loop back into another Sonos command. Before applying a confirmation, the shell reads local state and skips each unchanged volume or mute property. Volume changes are debounced and coalesced. Mute changes bypass the debounce.

macOS and Ubuntu retain up to 64 expected local states for 500 ms per write. Matching callbacks neither consume nor extend that protection, so repeated and overlapping callbacks remain suppressed. Nonmatching changes are forwarded immediately. macOS includes intermediate per-channel averages and a volume tolerance; Ubuntu matches exact normalized states. Windows uses native callback context identity.

Without native origin metadata, a real change matching a recent expected value can be indistinguishable from an echo until expiry. Very late callbacks and device-specific quantization still need hardware investigation. See [the volume feedback decision](https://github.com/MiguelTVMS/speaker-volume-bridge/blob/v1.6.3/docs/decisions/0017-volume-feedback-suppression.md).

## Sonos protocol boundary

Discovery uses SSDP. RenderingControl uses SOAP with `InstanceID=0` and `Channel=Master`. GENA notifications provide confirmed volume and mute events.

The client accepts only bounded local HTTP device locations at private, loopback, or link-local literal addresses. Public addresses and host names supplied by discovery are rejected. The frontend has no direct filesystem or network permission.

## Platform adapters

The Windows adapter keeps Core Audio COM objects on a dedicated worker thread and handles default-output replacement.

The macOS adapter listens for default-output, mute, and volume changes, uses channel controls when master volume is unavailable, and suppresses expected local writes with a short-lived tolerance.

The Ubuntu adapter uses `pactl` to enumerate sinks, follow the default sink or a fixed sink, control volume and mute, and subscribe to PulseAudio-compatible changes. It supports PulseAudio and PipeWire's PulseAudio compatibility service and suppresses expected writes for a short period.

## Live Settings and speaker capabilities

Speaker-setting GENA events trigger authoritative reads even without volume or mute fields. Capability detection distinguishes explicit lack of support from temporary read failures. Runtime snapshots push volume, mute, and connection changes to Settings; periodic reads provide recovery. The frontend serializes user writes and preserves active slider gestures, while readbacks only update displayed state.

## Night Mode scheduling

The domain owns weekly half-hour intervals and time-zone-aware boundary calculation. Integration applies Night Mode through an abstract port, confirms changes, enforces active periods, and filters notification intent. The shell owns an independent scheduling worker, persistence, native wake events, system time preferences, and desktop notification delivery.

One global schedule follows the selected speaker. The worker runs independently of local audio and volume fallback polling. Configuration changes and Night Mode commands share a write gate to prevent stale-selection writes. Wake, clock changes, and recovery reconcile current state without replaying missed boundaries. Schedule commands preserve the existing volume synchronization state machine.

See the [Night Mode schedule decision](https://github.com/MiguelTVMS/speaker-volume-bridge/blob/v1.6.3/docs/decisions/0013-global-night-mode-schedule.md).
