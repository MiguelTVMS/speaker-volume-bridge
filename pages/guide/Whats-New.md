---
layout: guide
---

# What changed

This guide documents [v{{ site.data.release.version }}](https://github.com/MiguelTVMS/speaker-volume-bridge/releases/tag/v{{ site.data.release.version }}), the current generally available release.

## Speaker Volume Bridge is the new name

Sonos Volume Bridge is now **Speaker Volume Bridge**. The app name, icon, executable, repository, and primary download filenames use the new brand. The app still controls compatible Sonos speakers, and the project remains independent of Sonos. The name change does not add support for other speaker brands.

The existing Store products, saved settings, and upgrade identities are retained. The [project website](https://svb.miguel.ms/) and abbreviation **SVB** remain unchanged. Updating within the same distribution preserves settings; moving between the direct-download and Mac App Store editions still requires setting up the app again.

Read [Upgrading to Speaker Volume Bridge](/guide/Upgrading.html) before replacing an older installation.

## Manual old-app removal in v{{ site.data.release.version }}

Automatic old-process detection was replaced with clear manual upgrade guidance because process inspection was not reliable enough across supported platforms. Quit and remove Sonos Volume Bridge, including its startup entry, before running Speaker Volume Bridge. See [Upgrading](/guide/Upgrading.html) and the [complete removal guide](/guide/Removing-the-Old-App.html).

## Windows upgrade fix introduced in v1.6.2

The direct-download installer now moves an existing installation from the former default **Sonos Volume Bridge** folder to **Speaker Volume Bridge**. It retains custom locations and saved settings, and updates shortcuts and startup targets. A blocked folder move or existing destination stops the installer safely. Quit the installed app before updating and follow the [Windows upgrade guidance](/guide/Upgrading.html).

## Renamed downloads and compatibility links

There are five platform downloads: Windows x64 and ARM64 installers, an Apple Silicon macOS DMG, and Ubuntu x64 and ARM64 Debian packages. Their primary names begin with `speaker-volume-bridge-`. Copies under the former `sonos-volume-bridge-` names preserve existing links, making ten assets in this release.

On Ubuntu, the new package replaces the old package and keeps the old shell command as a compatibility link. On macOS, remove the old app bundle after quitting it so the two differently named bundles do not remain side by side. See [Installation](/guide/Installation.html) and [Upgrading](/guide/Upgrading.html).

Windows direct downloads remain unsigned. The documented GA baseline has a signed and notarized macOS app and disk image. Later releases can disable signing; check their exact release notes before installing. Store review and availability are separate from GitHub publication.

## Features carried forward

- **Volume feedback protection:** the fixes introduced in v1.5.5 remain included. Repeated or overlapping callbacks from the app's own audio writes do not become fresh volume commands, and unchanged local volume or mute values are not rewritten. See [Volume settings](/guide/Volume-Settings.html).
- **Night Mode scheduling:** weekly half-hour blocks, drag and keyboard editing, explicit Save/Cancel, a tray toggle, and optional desktop notifications. See [Night schedule](/guide/Night-Schedule.html) for saving and notification behavior.
- **Speaker controls:** supported sound settings, live volume and mute updates, and capability-aware controls. See [Speaker settings](/guide/Speaker-Settings.html).
- **Windows and Ubuntu improvements:** native notification integration, schedule status, updated Settings layouts, and Ubuntu Wayland title-bar fixes remain included.
