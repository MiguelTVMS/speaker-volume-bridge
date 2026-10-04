<!-- Generated from upgrade.html; edit the HTML source. -->

# Upgrading to Speaker Volume Bridge

Formerly Sonos Volume Bridge. The same app, with the same settings and Sonos compatibility. Our address remains svb.miguel.ms.

## Quit and remove the old app first

Running Sonos Volume Bridge and Speaker Volume Bridge together can cause conflicting volume and Night Mode changes. Choose Quit from the old app's menu bar or system tray, then remove it before using the replacement. Closing Settings alone leaves the app running.

Follow the [complete old-app removal guide](https://svb.miguel.ms/guide/Removing-the-Old-App.html)  for startup entries, uninstalling, and an optional clean settings reset. The [repository Markdown guide](https://github.com/MiguelTVMS/speaker-volume-bridge/blob/develop/docs/removing-old-app.md)  contains the same steps.

## macOS

Quit both bridge apps. Open System Settings > General > Login Items & Extensions and remove their entries from Open at Login. Remove Sonos Volume Bridge.app from Applications and check for other copies. For a clean reinstall, remove Speaker Volume Bridge.app too. Install the current DMG by dragging the new app into Applications, eject the DMG, and launch it from Applications. Check Start at login again.

If disabling Start at login reports Operation not permitted, remove the app's login entry in System Settings and retry. The removal guide also explains how to erase saved settings when you intentionally want a fresh setup. Ordinary upgrades can keep settings.

[Download macOS DMG ↓](https://github.com/MiguelTVMS/speaker-volume-bridge/releases/latest/download/speaker-volume-bridge-macos.dmg)

## Windows

Quit the old app from the system tray. For an ordinary upgrade, run the current installer over the existing installation; it preserves settings and updates startup targets. For a clean reinstall, disable its Startup apps entry, uninstall through Settings > Apps > Installed apps, and choose whether to erase app data before installing one edition again.

[Microsoft Store ↗](https://apps.microsoft.com/detail/9N7JKGXCMST0?cid=website&referrer=download&source=svb.miguel.ms) [Windows x64 installer ↓](https://github.com/MiguelTVMS/speaker-volume-bridge/releases/latest/download/speaker-volume-bridge-windows-x64-unsigned.exe) [Windows ARM64 installer ↓](https://github.com/MiguelTVMS/speaker-volume-bridge/releases/latest/download/speaker-volume-bridge-windows-arm64-unsigned.exe)

## Linux

Quit the old app and remove any manually created duplicate startup entry. The speaker-volume-bridge Debian package replaces the old package and preserves settings. For a clean reinstall, remove the installed package through your package manager, clean only the bridge's autostart entry, and optionally erase its saved data as described in the removal guide. The sonos-volume-bridge command supplied by the new package is a compatibility alias, not a second app.

[Ubuntu x64 DEB ↓](https://github.com/MiguelTVMS/speaker-volume-bridge/releases/latest/download/speaker-volume-bridge-linux-x64.deb) [Ubuntu ARM64 DEB ↓](https://github.com/MiguelTVMS/speaker-volume-bridge/releases/latest/download/speaker-volume-bridge-linux-arm64.deb)

## Saved settings and Store editions

The old and new names share settings within a distribution. Deleting those files also resets the renamed app's speaker choice, volume preferences and Night schedule. Keep them for an ordinary upgrade. Update Store installations through the same Store, and avoid running the Store and direct-download editions together.

Speaker Volume Bridge is independently developed and is not affiliated with, endorsed by, or sponsored by Sonos, Inc. Sonos is a trademark of Sonos, Inc.

[Speaker Volume Bridge](https://svb.miguel.ms/)

[Privacy policy](https://svb.miguel.ms/privacy.html) [MIT license](https://svb.miguel.ms/license.txt) [Source code ↗](https://github.com/MiguelTVMS/speaker-volume-bridge)

Independent software. Not affiliated with, sponsored by, endorsed by, or supported by Sonos. Sonos and related product names are trademarks of their respective owners and are used only to identify compatibility. This project contains no Sonos source code.

Made for the volume controls you already use.

{% if site.data.build %}

Version {{ site.data.build.version }} · {{ site.data.build.ref }} · {{ site.data.build.revision }}

{% endif %}
