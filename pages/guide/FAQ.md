---
layout: guide
---

# Frequently asked questions

## Is Speaker Volume Bridge the same app as Sonos Volume Bridge?

Yes. The app has a new name, icon, and executable, while retaining the existing Store products and saved settings for updates within the same distribution. Quit the old app before updating and follow [Upgrading](/guide/Upgrading.html). Switching between macOS direct-download and Store editions still requires configuring settings again.

## Does the new name mean other speaker brands are supported?

No. Sonos is the only supported speaker integration in v{{ site.data.release.version }}. The project is independently developed and is not affiliated with or endorsed by Sonos.

## Why did synchronization pause after the rename?

The renamed app pauses synchronization and scheduled speaker changes when it detects the old app running. Quit **Sonos Volume Bridge** from its menu bar or system tray, then choose **Check again**. A **Check unavailable** status by itself is not a confirmed conflict. See [Diagnostics and troubleshooting](/guide/Diagnostics-and-Troubleshooting.html).

## Why do some downloads and settings still use the old name?

Release assets with the former filename are compatibility copies of the renamed downloads. Some internal identifiers and settings locations intentionally remain stable so upgrades preserve preferences. Prefer the `speaker-volume-bridge-*` files for new downloads and keep existing application data.

## Does the app send my computer audio to Sonos?

No. It synchronizes volume and optional mute state. It does not capture, stream, redirect, or modify audio.

## Does it need a Sonos cloud account?

No. Control is local between the computer and speaker.

## Does closing Settings stop synchronization?

No. Closing Settings hides the window. Use **Quit** from the tray or menu-bar menu to stop the app.

## Can it control several speakers?

It controls one selected Sonos device. For a group, create the group in the Sonos app and select its coordinator in Volume Bridge. Reliable group and stereo-pair control is tracked in [issue #86](https://github.com/MiguelTVMS/speaker-volume-bridge/issues/86).

## Why did my computer volume change when the app connected?

Two-way synchronization is enabled by default. On the first confirmed connection, Sonos state is applied to the computer. Disable two-way synchronization to make the direction computer to Sonos only.

## Does v{{ site.data.release.version }} include the volume feedback fix?

Yes. It includes the fixes introduced in v1.5.5 for repeated and overlapping callbacks from the app's own audio writes on macOS and Ubuntu, and skips unchanged local volume and mute writes. The fix works automatically. If volume still moves unexpectedly, follow [Diagnostics and troubleshooting](/guide/Diagnostics-and-Troubleshooting.html).

## Why is a Speaker control gray?

Read the message beside the control: **Not supported by this speaker** means support is absent; **Temporarily unavailable** means the read failed or was inconclusive. Refresh after reconnecting. An off switch or zero tone value does not mean unsupported.

## Why does Connected appear during polling fallback?

Polling fallback is a working recovery mode used when normal event delivery is unavailable or stale. The bridge can continue synchronization, so the user-facing state remains Connected.

## Why does Windows show SmartScreen?

The v{{ site.data.release.version }} direct-download Windows installer is not digitally signed. Install it only from the project's official full-release page.

## What does Ubuntu require?

The v{{ site.data.release.version }} Ubuntu x64 (AMD64) and ARM64 packages require PulseAudio or PipeWire's PulseAudio compatibility service and the `pactl` command supplied by `pulseaudio-utils`. See the [Ubuntu guide](/guide/Ubuntu.html).

## Can I keep the computer source volume at 100%?

Not as a separate mode in v{{ site.data.release.version }}. The design question is tracked in [issue #74](https://github.com/MiguelTVMS/speaker-volume-bridge/issues/74).

## Can Night Mode turn on automatically?

Yes. Open [Night schedule](/guide/Night-Schedule.html), select weekly half-hour blocks, save, and enable the schedule. One global schedule follows the speaker selected under Devices.

## Why can I not turn Night sound off?

An enabled schedule keeps it on during selected periods. Disable **Night schedule** first, then turn Night sound off. Disabling scheduling alone does not change the speaker.

## Why did Save schedule change Night sound with scheduling disabled?

Saving explicitly applies the current block once: on inside a selected block, off outside. **Enable schedule** controls recurring operation.

## Must the app stay running for the schedule?

Yes. The schedule runs on the computer. Closing Settings leaves it running; quitting the app stops it. After sleep or an interruption, the app applies the current expected state without replaying missed transitions.
