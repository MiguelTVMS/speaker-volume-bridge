# Speaker Volume Bridge upgrade

Formerly Sonos Volume Bridge. SVB and svb.miguel.ms stay unchanged. Sonos compatibility remains the only speaker integration in this release.

## User upgrade steps

- Quit Sonos Volume Bridge from its menu bar or system tray before upgrading.
- **macOS direct download:** install Speaker Volume Bridge.app, remove the old Sonos Volume Bridge.app bundle, and keep application settings/data. Check Start at login after replacing the bundle. Do not run the Store and direct-download editions together.
- **Windows:** update the existing installation or the existing Microsoft Store product. The installer migrates the former default installation folder to Speaker Volume Bridge, preserves custom installation paths and settings, and updates shortcuts and startup targets. Do not create a second Store product. Quit the installed app before upgrading. A blocked folder move stops the installer without removing the existing folder; an existing destination is never merged or overwritten.
- **Linux:** install the new speaker-volume-bridge Debian package. It replaces sonos-volume-bridge; the old shell command remains a compatibility symlink. Check your desktop environment's startup list and remove a manually created duplicate startup entry if present.

## Remove the old app before using the replacement

Follow the [complete removal guide](removing-old-app.md) for macOS, Windows or
Linux. It covers quitting the old process, removing startup entries and app
bundles/packages, and optionally erasing saved settings for a clean reinstall.
Both names share settings within a distribution, so erase them only when you
want to reset the renamed app too. Running both versions can cause competing
volume and Night Mode commands. The application does not detect or pause for
another installed version.

### macOS Start at login recovery

If disabling Start at login reports “Operation not permitted” after replacing
the old app, open **System Settings > General > Login Items & Extensions**.
Under **Open at Login**, remove any **Sonos Volume Bridge** or **Speaker Volume
Bridge** entry using the minus button. Launch the installed app from Applications
and retry disabling Start at login. Removing a bundle can leave the saved app
preference enabled even when its macOS login registration is already absent.
The login adapter accepts that absent state without attempting removal. Other
removal failures retain the saved preference and show the recovery steps.
Application settings do not need to be deleted for this recovery.

## Engineering and CI responsibilities

CI checks Rust, the frontend, website, workflow contracts and package assembly. CI builds the renamed installers and both new and legacy download filenames. It cannot reserve names, prove a real upgrade, exercise physical Sonos hardware, approve Apple jobs, select an App Store build or confirm Store certification.

Before release, verify clean install and upgrade on macOS, Windows x64/ARM64 and
Linux AMD64/ARM64. Follow the manual removal guide and confirm the old app no
longer starts at login. Check retained versus erased preferences, normal startup,
single-instance behavior, manual speaker controls, Night scheduling, notification
activation and uninstall. Signed Store and packaged desktop behavior requires
native validation; unit tests do not establish an installed upgrade outcome.

## Publication sequence

1. Save/reserve the new name on the existing Apple and Microsoft records.
2. Complete implementation, automated checks, local UI review and approved PR.
3. Rename the GitHub repository in place when the PR is ready; verify redirects, integrations and the custom domain. Never reuse the old repository name.
4. Merge through the approved PR to develop. Choose the next Minor increment, enable Apple package creation and disable automatic Microsoft submission for this first transition.
5. Verify installers and download aliases, approve promotion to main, deploy the website and publish the reviewed wiki.
6. Complete the existing Microsoft draft without discarding unrelated changes. Upload Apple's package, select the build, update screenshots and submit the branding review response. Complete Apple's displayed trader-status requirement if distributing in the EU.
7. Confirm each Store outcome separately before resuming routine Microsoft CI submissions. Fix forward with a higher version after publication.

Apple name: Speaker Volume Bridge. Subtitle: Sync Mac and speaker volume. Retain accurate Sonos compatibility and independent-development wording.

## Intentional legacy identifiers

[The allowlist](legacy-identifiers.json) records files retaining the former product name or stable identifiers and their reasons. `python3 scripts/check-legacy-identifiers.py` rejects undocumented references, including new files. Protocol-specific Sonos names remain accurate and are outside the rebrand.

Ready-to-review [Store copy and Apple reply](store-rebrand-copy.md) are staged for the verified build.

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
