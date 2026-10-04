---
layout: guide
---

# Updating and uninstalling

For a complete removal of the former **Sonos Volume Bridge** app, including
startup entries and optional saved-data cleanup, follow
[Fully removing the old app](/guide/Removing-the-Old-App.html).

## Update only to full releases

This guide follows generally available releases only. For normal use, install the version returned by the [latest full release link](https://github.com/MiguelTVMS/speaker-volume-bridge/releases/latest). v1.6.3 includes the rebrand, Windows upgrade fix, volume feedback protection, and Night scheduling. Check **About** after updating to confirm the installed version.

If your installed app is still called **Sonos Volume Bridge**, start with [Upgrading to Speaker Volume Bridge](/guide/Upgrading.html). Quit it from its menu bar or system tray before installing the renamed app.

## Windows direct-download updates

Download and run the newer full-release installer. A newer version is installed in place and preserves application data and shortcuts.

The v1.6.3 installer moves the former default **Sonos Volume Bridge** installation folder to **Speaker Volume Bridge**. It preserves custom locations and refuses to merge with an existing destination. Quit the installed app before retrying a blocked folder move.

The normal graphical installer keeps the desktop clear. It uses a Start menu shortcut and does not offer a desktop shortcut by default.

## Windows uninstall

Uninstall Speaker Volume Bridge from Windows Settings or its uninstaller. The uninstaller can delete application data when that option is selected. If application data is retained, a later installation can reuse the saved configuration.

Uninstalling also removes the normal startup entry and app notification registration. In-place upgrades preserve notification registration for the installed app.

## macOS direct-download updates

Quit the current app, download and open the newest full-release DMG, and drag **Speaker Volume Bridge.app** into Applications. Replace an existing copy with that name. When upgrading from **Sonos Volume Bridge.app**, remove the old bundle separately because its name differs. Keep application data and settings, eject the disk image, and check **Start at login** after launching the renamed app.

## Ubuntu updates and uninstall

Download the newer full-release Debian package and install it over the current version with Ubuntu's software installer or:

```sh
sudo apt install ./speaker-volume-bridge-linux-x64.deb
```

For ARM64, use `speaker-volume-bridge-linux-arm64.deb` instead.

The renamed `speaker-volume-bridge` package replaces `sonos-volume-bridge` and keeps the old shell command as a compatibility link. Check for manually created duplicate startup entries after the change.

Remove the application through Ubuntu's software manager or package manager. Removing the package normally leaves per-user configuration in place. Delete that application data separately only when you intentionally want a clean setup.

## Store updates

Use the store that installed the app to check for updates. GitHub publication and Store certification are separate, so a store installation may remain on an earlier version after v1.6.3 becomes available as a direct download.

## Switching distribution channels

Direct-download and store editions can be separate applications with separate data. Before installing a Mac App Store edition, remove the direct-download edition. Settings are not migrated automatically, so select the speaker and audio preferences again.

After any channel change, verify the selected speaker, selected output, highest speaker volume, and two-way synchronization before normal use.
