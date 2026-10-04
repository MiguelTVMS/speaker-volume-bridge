---
layout: guide
---

# Speaker Volume Bridge

> [!WARNING]
> **Upgrading from Sonos Volume Bridge? Quit and remove the old app first.**
> Running both apps can cause conflicting volume and Night Mode changes. Follow
> the [complete old-app removal guide](/guide/Removing-the-Old-App.html), including login
> items and optional settings cleanup, before using Speaker Volume Bridge.

Control a Sonos speaker with the volume controls you already use on your computer.

> [!NOTE]
> This guide documents [v1.6.3](https://github.com/MiguelTVMS/speaker-volume-bridge/releases/tag/v1.6.3), the current generally available release, including the app rebrand and Windows upgrade fix.

**Formerly Sonos Volume Bridge.** This is the same app with a new name. Sonos remains the supported speaker family; the rename does not add other speaker integrations. Existing users should read [Upgrading to Speaker Volume Bridge](/guide/Upgrading.html) and quit the old app before installing the update.

Speaker Volume Bridge is a small background utility for Windows, macOS, and Ubuntu. It keeps one selected Sonos speaker and one computer audio output in step. You can use keyboard volume keys, system controls, or supported audio-device controls instead of opening the Sonos app for every adjustment.

The app communicates directly with the speaker on your local network. It does not require a cloud account, Home Assistant, or a separate remote control.

## Start here

1. [Install Speaker Volume Bridge](/guide/Installation.html).
2. Make sure the computer and Sonos speaker are on the same local network.
3. Open Settings and select a **Sonos speaker**.
4. Select **Follow system output** or choose one fixed computer audio output.
5. Set your preferred synchronization and volume behavior.
6. Close Settings. The bridge keeps running in the Windows or Ubuntu system tray, or the macOS menu bar.

Most settings are saved automatically. The Night schedule grid uses **Save schedule**.

## What it does

- Synchronizes computer volume changes to the selected Sonos speaker.
- Optionally synchronizes Sonos volume changes back to the computer.
- Follows the current system audio output or stays attached to one fixed output.
- Optionally synchronizes mute state.
- Caps the highest permitted Sonos volume.
- Provides three volume-response choices: Balanced, Direct, and Scaled.
- Exposes supported Sonos sound settings such as Night sound, Loudness, Status light, Speech enhancement, Treble, and Bass.
- Schedules Night Mode in weekly 30-minute blocks for the selected compatible speaker, with optional desktop notifications.
- Reconnects after temporary speaker, output, or network interruptions.
- Shows live status and sanitized diagnostics.

## What it does not do

Speaker Volume Bridge does not play, stream, capture, redirect, or modify audio. It is not a replacement for the Sonos app and is not a general multi-room controller.

## Requirements

| Requirement | Details |
| --- | --- |
| Operating system | Windows x64/ARM64; Apple Silicon with macOS 13 and later; or x64/ARM64 Ubuntu with PulseAudio or PipeWire and `pactl` |
| Sonos | A compatible speaker on the same local network |
| Computer audio | An output whose volume can be changed by software |
| Internet | Needed to download the app, but not for normal synchronization |

Development is hands-on tested primarily with a Sonos Ray. Other speaker and audio-device combinations may need validation. Read [Compatibility and limitations](/guide/Compatibility-and-Limitations.html) before relying on the app in a new setup.

## Learn more

- [Getting started](/guide/Getting-Started.html)
- [Ubuntu guide](/guide/Ubuntu.html)
- [Settings guide](/guide/Settings.html)
- [Night schedule](/guide/Night-Schedule.html)
- [What changed](/guide/Whats-New.html)
- [Upgrading from the old app](/guide/Upgrading.html)
- [Tray and menu bar](/guide/Tray-and-Menu-Bar.html)
- [Diagnostics and troubleshooting](/guide/Diagnostics-and-Troubleshooting.html)
- [Frequently asked questions](/guide/FAQ.html)
- [Privacy and security](/guide/Privacy-and-Security.html)
- [License and disclaimer](/guide/License-and-Disclaimer.html)

For a problem that is not covered here, use the [GitHub issue tracker](https://github.com/MiguelTVMS/speaker-volume-bridge/issues). Do not post raw logs, network addresses, device identifiers, or other private diagnostic data in a public issue.

## Independent project notice

Speaker Volume Bridge is an independent, community-developed project. It is not affiliated with, sponsored by, endorsed by, or supported by Sonos. The project contains no Sonos source code. “Sonos” and related product names are trademarks of their respective owners and are used only to identify compatibility. Read the complete [license and disclaimer](/guide/License-and-Disclaimer.html).
