---
layout: guide
---

# Upgrading to Speaker Volume Bridge

> [!WARNING]
> Quit and remove **Sonos Volume Bridge** before using the renamed app. Running
> both can cause conflicting volume and Night Mode changes. Use the
> [complete removal guide](/guide/Removing-the-Old-App.html) for login-item cleanup,
> uninstalling, and an optional clean settings reset.

**Speaker Volume Bridge was previously named Sonos Volume Bridge.** This guide applies to [v1.6.3](https://github.com/MiguelTVMS/speaker-volume-bridge/releases/tag/v1.6.3), the current generally available release.

The app name, icon, executable, and download filenames have changed. The abbreviation **SVB**, [project website](https://svb.miguel.ms/), existing Store products, and Sonos compatibility remain the same. Sonos is still the only supported speaker integration. Speaker Volume Bridge is independently developed and is not affiliated with or endorsed by Sonos.

Updating within the same distribution preserves saved settings and installation identity. Switching between the direct-download and Mac App Store editions is different: those editions use separate application data and settings are not migrated automatically.

## Before updating

1. Choose **Quit** in the old app's menu bar or system tray. Closing Settings alone leaves it running.
2. Download the installer for your platform from the [latest full release](https://github.com/MiguelTVMS/speaker-volume-bridge/releases/latest), or update your existing Store installation through its Store.
3. Follow the platform steps below, then launch **Speaker Volume Bridge**.
4. Check the installed version under **About**, your selected speaker and computer output, volume preferences, saved Night schedule, and **Start at login**.

## macOS direct download

Open `speaker-volume-bridge-macos.dmg` and drag **Speaker Volume Bridge.app** into Applications. Remove the old **Sonos Volume Bridge.app** bundle after quitting it; the different bundle names can otherwise leave both apps installed. Keep application data and settings.

Eject the disk image and open the renamed app from Applications. Check **Start at login** after replacing the bundle. Avoid running the direct-download and Mac App Store editions together. When changing to the Store edition, remove the direct-download edition and configure the speaker and audio preferences again.

## Windows direct download

Run the v1.6.3 installer for your architecture over the existing installation. It preserves settings and updates the app's shortcuts and startup targets.

If the previous installation used the old default **Sonos Volume Bridge** folder, this release moves it to **Speaker Volume Bridge**. A custom installation location is retained.

If the installer says the folder could not be moved, quit the installed app from its system tray, close files open in its installation folder, and retry. If the destination already exists, use an empty destination or remove a confirmed unused duplicate installation before retrying. The installer stops rather than merging or overwriting an existing destination, and a failed folder move leaves the old folder in place.

## Ubuntu

Install the new `speaker-volume-bridge` Debian package using the x64 or ARM64 download described in [Installation](/guide/Installation.html). It replaces the old `sonos-volume-bridge` package and retains saved configuration.

Start **Speaker Volume Bridge** from the application menu. The old `sonos-volume-bridge` shell command remains a compatibility link to the renamed executable. Check your desktop startup list and remove any manually created duplicate entry for the old app.

## Store installations

Update the existing Store app. The rebrand retains the same Store products, so it does not require installing a separate product. Store review and rollout are separate from GitHub publication; the displayed name and available version can lag behind the GitHub release. Use **About** to check the installed version.

## Prevent conflicts with the old app

Version 1.6.3 replaced automatic old-process detection with clear manual upgrade guidance because operating-system process inspection was not reliable enough on every supported platform. The app does not close, pause, or uninstall the old app for you.

Before opening Speaker Volume Bridge, quit Sonos Volume Bridge from its menu bar or system tray and remove its old startup entry. Closing the Settings window alone leaves the old app running. If both apps run, quit both, follow the [complete removal guide](/guide/Removing-the-Old-App.html), then start only Speaker Volume Bridge.

## Why some old names remain

The release includes copies of each download under the former `sonos-volume-bridge-*` filenames so existing download links keep working. Prefer the `speaker-volume-bridge-*` filenames for new downloads. Both sets contain the renamed app.

Some internal settings, notification, startup, and package identifiers deliberately retain the old name to preserve upgrades and saved preferences. These are compatibility details, not a second app or a reason to delete application data.

See [Updating and uninstalling](/guide/Updating-and-Uninstalling.html) for routine updates and removal, or the [released upgrade reference](https://github.com/MiguelTVMS/speaker-volume-bridge/blob/v1.6.3/docs/rebrand-upgrade.md) for technical details.
