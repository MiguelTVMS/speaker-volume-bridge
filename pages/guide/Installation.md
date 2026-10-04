---
layout: guide
---

# Installation

This page covers the current generally available release, [v{{ site.data.release.version }}](https://github.com/MiguelTVMS/speaker-volume-bridge/releases/tag/v{{ site.data.release.version }}).

Only install Speaker Volume Bridge from the [official latest release](https://github.com/MiguelTVMS/speaker-volume-bridge/releases/latest) or an official store listing.

**Night schedule** is included in this release. Choose the download for your operating system and processor architecture.

**Already using Sonos Volume Bridge?** Quit the old app before installing and follow [Upgrading to Speaker Volume Bridge](/guide/Upgrading.html). Updating within the same distribution retains settings; the macOS bundle, Windows default installation folder, and Ubuntu package have platform-specific migration steps.

Use the `speaker-volume-bridge-*` downloads below. The release also includes identical copies named `sonos-volume-bridge-*` to keep older download links working.

## Windows direct download

1. Open the [latest full release](https://github.com/MiguelTVMS/speaker-volume-bridge/releases/latest).
2. Download `speaker-volume-bridge-windows-x64-unsigned.exe` for an x64 (AMD64) PC, or `speaker-volume-bridge-windows-arm64-unsigned.exe` for an ARM64 PC. Check **Settings → System → About → System type** if unsure.
3. Run the installer.
4. Start **Speaker Volume Bridge** from the Start menu.

The direct-download Windows installer is not digitally signed. Windows SmartScreen may therefore show a warning. Continue only if the file came from this project's official GitHub release page.

The normal graphical installer creates a Start menu shortcut and does not create a desktop shortcut. Installing a newer full release performs an in-place update and preserves application settings.

In v{{ site.data.release.version }}, an existing installation in the old default folder is moved to **Speaker Volume Bridge**, while a custom location stays unchanged. If a folder move is blocked, follow the [upgrade guide](/guide/Upgrading.html) before retrying.

## Microsoft Store on Windows

Use the existing [Microsoft Store listing](https://apps.microsoft.com/detail/9N7JKGXCMST0?hl=en-us&gl=PT&ocid=pdpshare) for the Store edition. The rebrand retains the same Store product. The displayed name and available version may lag behind GitHub while Microsoft processes a release.

Use one Windows distribution consistently when possible. If you switch distribution channels, confirm your settings after the new installation starts.

## macOS direct download

1. Open the [latest full release](https://github.com/MiguelTVMS/speaker-volume-bridge/releases/latest).
2. Download `speaker-volume-bridge-macos.dmg` for an Apple Silicon Mac.
3. Open the disk image.
4. Drag **Speaker Volume Bridge.app** onto **Applications**.
5. Eject the disk image and launch the app from Applications.

If **Sonos Volume Bridge.app** is already installed, quit it and remove that old bundle while retaining application data. Dragging in the differently named app does not automatically replace the old bundle. Check **Start at login** after upgrading.

The direct-download macOS app is signed with Developer ID and notarized by Apple. macOS 13 or later is required.

## Ubuntu direct download

The release provides x64 (AMD64) and ARM64 Debian packages for 64-bit Ubuntu. Run `dpkg --print-architecture` if unsure: `amd64` uses the x64 download, and `arm64` uses the ARM64 download.

1. Open the [latest full release](https://github.com/MiguelTVMS/speaker-volume-bridge/releases/latest).
2. Download `speaker-volume-bridge-linux-x64.deb` or `speaker-volume-bridge-linux-arm64.deb` to match your architecture.
3. Open it with Ubuntu's software installer, or install it from a terminal:

   ```sh
   sudo apt install ./speaker-volume-bridge-linux-x64.deb
   ```

   Use the ARM64 filename in the command if that is your download.

4. Start **Speaker Volume Bridge** from the application menu.

The package depends on `pulseaudio-utils`, which supplies `pactl`. It works with PulseAudio or PipeWire's PulseAudio compatibility service. See the [Ubuntu guide](/guide/Ubuntu.html) for audio-service and troubleshooting details.

## Direct download and store editions on macOS

Direct-download and store editions can be separate installations with separate application data. Before moving from a direct-download macOS edition to a Mac App Store edition, uninstall the direct-download edition. Settings are not migrated automatically, so select the speaker and audio preferences again.

## After installation

Continue with [Getting started](/guide/Getting-Started.html). If the app opens but cannot find a speaker, go directly to [Diagnostics and troubleshooting](/guide/Diagnostics-and-Troubleshooting.html).
