---
layout: guide
---

# Compatibility and limitations

This page applies to the v1.6.3 generally available release. Every item under **Tracked v1.6.3 limitations** is tied to an existing GitHub issue.

## Compatibility baseline

- Windows x64 or ARM64; Apple Silicon with macOS 13 and later; or 64-bit Ubuntu with PulseAudio or PipeWire and `pactl`.
- A Sonos speaker reachable on the same local network.
- A computer audio output with software-controlled volume.
- Hands-on development testing is primarily performed with a Sonos Ray.

Sonos features vary by model. Speaker controls are probed at runtime and unavailable controls are shown in gray.

## Tracked v1.6.3 limitations

### Computer output volume remains part of synchronization

The current bridge synchronizes the selected computer output rather than holding the source at 100%. Whether source volume should instead remain fixed is tracked in open [issue #74](https://github.com/MiguelTVMS/speaker-volume-bridge/issues/74).

### Group and stereo-pair control is not reliable in every configuration

The app targets the Sonos device selected in Settings and does not automatically resolve every group or stereo-pair configuration. Build the group in the Sonos app, select its coordinator in Volume Bridge, and verify volume and mute behavior in that specific setup. Reliable group and stereo-pair control is tracked in open [issue #86](https://github.com/MiguelTVMS/speaker-volume-bridge/issues/86).

## Distribution caveats

- The Windows direct-download installer is unsigned, so SmartScreen may warn. This is stated in the [v1.6.3 release](https://github.com/MiguelTVMS/speaker-volume-bridge/releases/tag/v1.6.3).
- The released Ubuntu packages support x64 (AMD64) and ARM64 systems and require a PulseAudio-compatible user audio service plus `pactl`.
- Direct-download and store editions can keep separate settings.
- An output without writable software volume is unsupported even if it can play audio.
- The app controls volume, mute, selected sound settings, and TV input where supported. It is not a playback or multi-room management application.

## Reporting another limitation

Use the [issue tracker](https://github.com/MiguelTVMS/speaker-volume-bridge/issues) for a reproducible limitation that is not listed. Do not include serial numbers, network addresses, device identifiers, raw logs, or diagnostic payloads in a public report.

## Night schedule compatibility

Scheduling requires Night Mode support on the selected speaker. An unsupported or unreachable speaker pauses scheduling without erasing the grid. The schedule controls only the selected speaker and requires the computer and app to be running. Time labels follow macOS and Windows regional preferences, or GNOME clock format and `LC_TIME` on Linux. Other Linux desktop clock overrides may require a matching `LC_TIME` setting.

Notification delivery follows the operating system's permissions and suppression settings. Linux desktop notification services do not expose a portable permission prompt or status. These are operating requirements; see [Night schedule](/guide/Night-Schedule.html) for details.
