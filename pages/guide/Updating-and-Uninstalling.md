---
layout: guide
---

# Updating and uninstalling

For a complete removal of the former **Sonos Volume Bridge** app, including
startup entries and optional saved-data cleanup, follow
[Fully removing the old app](/guide/Removing-the-Old-App.html).

## Update only to full releases

This guide follows generally available releases only. For normal use, install the version returned by the [latest full release link](https://github.com/MiguelTVMS/speaker-volume-bridge/releases/latest). v{{ site.data.release.version }} includes the rebrand, Windows upgrade fix, volume feedback protection, and Night scheduling. Check **About** after updating to confirm the installed version.

If your installed app is still called **Sonos Volume Bridge**, start with [Upgrading to Speaker Volume Bridge](/guide/Upgrading.html). Quit it from its menu bar or system tray before installing the renamed app.

## In-app update checks

Recognized direct-release installations can query the GitHub Releases API for an
edition- and architecture-compatible update. Open **Updates** in Settings to run
a manual check, change automatic checks or native notifications, and open the
validated release page. Store editions must use the Store that installed them. The app does not download or install packages.
See [Update checks](/guide/Updates.html) for timing, privacy, and distribution
rules.

## Windows direct-download updates

Download and run the newer full-release installer. A newer version is installed in place and preserves application data and shortcuts.

The v{{ site.data.release.version }} installer moves the former default **Sonos Volume Bridge** installation folder to **Speaker Volume Bridge**. It preserves custom locations and refuses to merge with an existing destination. Quit the installed app before retrying a blocked folder move.

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

Use the store that installed the app to check for updates. GitHub publication and Store certification are separate, so a store installation may remain on an earlier version after v{{ site.data.release.version }} becomes available as a direct download.

The in-app checker does not offer Store updates or infer Store availability from
direct-release packages. Use the Store to check for updates to its edition.

## Switching distribution channels

Direct-download and store editions can be separate applications with separate data. Before installing a Mac App Store edition, remove the direct-download edition. Settings are not migrated automatically, so select the speaker and audio preferences again.

After any channel change, verify the selected speaker, selected output, highest speaker volume, and two-way synchronization before normal use.

## Store delegation and migration from catalog clients

Recognized Microsoft Store and Mac App Store editions show **Updates are managed
by the Store**. **Open Microsoft Store** opens this product in Microsoft Store;
**Open Mac App Store** opens the Store's Updates page. Check, automatic-check,
notification and release-policy controls are unavailable for Store editions.
Startup, periodic, wake and manual calls make no GitHub discovery requests and
never infer Store availability from direct-download releases. The Store owns
update installation and notification behavior.

Existing catalog-based direct clients need a **one-time manual upgrade** to a
future released version containing GitHub Releases discovery. Download the correct
edition and architecture from the official release page, quit the current app,
and install it using the platform instructions. Keep application data to preserve
settings. Old clients cannot discover this redesign through the retired catalog
pipeline; no redirect, catalog backfill or silent source switch is provided.
After upgrading, verify the installed version and edition in Updates and run a
manual check. The new client discards old catalog offers and check freshness while
preserving release-policy and notification preferences. Store clients upgrade
through their existing Store when that version becomes available. This readiness
work does not publish that version or implement self-installation.
