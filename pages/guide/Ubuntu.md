---
layout: guide
---

# Ubuntu

Speaker Volume Bridge provides x64 (AMD64) and ARM64 Debian packages for 64-bit Ubuntu.

## Upgrading from the old name

Quit Sonos Volume Bridge before installing the renamed package. The `speaker-volume-bridge` package replaces `sonos-volume-bridge` and retains saved configuration. The old shell command remains a compatibility link to the new executable. Start **Speaker Volume Bridge** from the application menu and check your startup list for manually created duplicate entries. See [Upgrading](/guide/Upgrading.html).

## Requirements

- PulseAudio or PipeWire with its PulseAudio compatibility service.
- The `pactl` command from the `pulseaudio-utils` package.
- A running per-user audio service with at least one available output sink.
- A Sonos speaker on the same local network.

The Debian package declares `pulseaudio-utils` as a dependency. Installing it with Ubuntu's package manager normally installs that dependency automatically.

## How audio control works

The Ubuntu adapter uses `pactl` to list output sinks, read and set volume, synchronize mute, and subscribe to audio-service changes.

**Follow system output** follows the current default PulseAudio-compatible sink. A fixed output selection remembers the selected sink name and does not move when the system default changes.

PulseAudio and PipeWire report changes through `pactl subscribe`. In v1.6.3, the bridge retains its own expected writes briefly across repeated and overlapping notifications so a Sonos-confirmed local change does not create a command loop. Unrelated changes continue to be handled immediately.

## System tray behavior

The app runs in the Ubuntu system tray. Closing Settings hides the window and leaves synchronization running. Choose **Quit** from the tray menu to stop the app.

Desktop environments can differ in how they display tray icons. Tray visibility depends on the desktop environment's app-indicator support.

## Settings and notifications

Settings uses Ubuntu-style grouped rows and supports light and dark appearance. The window resizes vertically with a fixed width. The Night schedule page includes a **Schedule status** row below its notification choices.

This release includes a repair for unresponsive Wayland title-bar controls after opening Settings. Closing Settings still hides the window and keeps the bridge running.

Schedule notifications use the desktop notification service. Check desktop notification settings and Do Not Disturb if they do not appear; see [Night schedule](/guide/Night-Schedule.html) for which actions notify.

## If local audio is unavailable

Confirm that `pactl` can reach the current user's audio service:

```sh
pactl info
pactl get-default-sink
```

If either command fails:

1. Confirm that `pulseaudio-utils` is installed.
2. Confirm that PulseAudio or PipeWire's PulseAudio compatibility service is running for the signed-in user.
3. Restart the audio service or sign out and back in.
4. Reopen Speaker Volume Bridge and refresh the local outputs under **Devices**.

If a fixed sink was removed or renamed, select it again or return to **Follow system output**.

## Compatibility status

Ubuntu support uses the same Sonos discovery, synchronization, settings, diagnostics, and privacy behavior as Windows and macOS. Hardware and desktop combinations still vary, so verify default-output changes, fixed-output behavior, mute, tray visibility, and startup behavior in your own environment.

The general limitations in [Compatibility and limitations](/guide/Compatibility-and-Limitations.html) also apply to Ubuntu.
