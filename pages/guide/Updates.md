---
layout: guide
---

# Update checks

Speaker Volume Bridge can check the public GitHub release list for an update that
matches the installed edition, operating system, and application architecture.
The app never downloads or installs a package itself.

## Updates page

Version information appears together above the release policy. On Windows, this
uses a single Settings-style card with plain rows, without nested boxes.

When a newer compatible release is available, **Download update** opens its installer
download in your default browser. macOS receives the DMG; Windows and Debian receive
the package matching the installed application's architecture. Complete installation
manually after downloading. The app does not quit, run the installer or replace
itself. **Open release** remains available for release notes and download help.
The selected Stable only or Include prereleases policy still controls the offer.

A version-available notice also appears in the Settings sidebar, including when
native notifications are disabled or denied. Click it to open Updates. **Later**
hides the notice for this session but leaves the download available on Updates.
An expired offer requires another check before downloading. Store installations
continue to use their Store action instead of a direct download.
If a download cannot be opened, the app refreshes the offer. Open release remains
available when the refreshed offer is valid; otherwise use Check for updates to retry.

Open **Updates** in Settings to see the installed version and recognized
distribution, the last successful check, and the current checker state. Choose
**Check for updates** for a manual check. When a verified update is available,
**Open release** opens its validated HTTPS release page in the
default browser.

The last successful check uses your computer's regional date format, local time
zone and 12/24-hour clock preference. Reopening Settings refreshes those preferences.

An available update remains visible after choosing **Later** or denying an
operating-system notification. The app revalidates the offered version and URL
immediately before opening it.

## Automatic checks

Recognized direct and Debian packages check about 30 seconds after startup by default.
After a successful check, the result remains fresh for 24 hours. Temporary
failures use bounded retry delays and remain quiet in the background. Manual
checks can retry and show an error without interrupting volume synchronization.

Wake does not bypass the startup delay. After a failed request, automatic retries
wait one minute, five minutes and thirty minutes, then one day between further
attempts. Restart preserves these delays. **Check for updates** can retry sooner
unless GitHub has imposed a rate-limit cooldown. A failed manual check shows its
error even when the last known offer remains visible. If saving a release policy
fails, check again to recover the saved policy and current offer.

Turn off **Automatically check for updates** to disable background requests.
Debug, demo, custom, sideloaded, or ambiguous packages do not silently enroll in
automatic checks.

## Notifications

**Update notifications** controls native desktop notices separately from update
checks. Enabling it is the only update flow that may request operating-system
notification permission. Turning it off does not hide an available update from
the Updates page.

## Privacy and editions

The API request contains no speaker details, configuration, analytics,
advertising data, or persistent installation identifier. As with any HTTPS
request, GitHub can receive the public IP address, request time,
API path, and network metadata. See [Privacy and security](/guide/Privacy-and-Security.html).

The app requires published release metadata and an uploaded official installer
matching its edition and application architecture. Missing compatible assets mean
unavailable, not up to date. GitHub release metadata does not prove Store
availability, so direct-release assets never produce Store update offers.

## Stable only or Include prereleases

Recognized direct macOS, direct Windows and official Debian packages expose the
Release policy dropdown. **Stable only** is the default and selects GA releases.
Direct checks support macOS ARM64 and Windows/Debian x64 or ARM64 application
builds. Other architectures, including Intel macOS, remain unsupported.
**Include prereleases** considers GA and public prereleases, including Alpha/Beta
packages with ordinary numeric versions. The app selects the highest compatible
semantic version, regardless of publication order. Store, sideloaded, development,
unknown and custom editions omit this selector.

Changing policy saves it for restart and checks once even with automatic checks
off. Previous links disappear immediately; errors leave Check for updates
available. Automatic-check, notification and speaker preferences are preserved.
Returning to Stable only waits for a strictly newer GA release without downgrading.
Equal-version promotion does not reinstall a preview. Open release opens the
specific release's page. A release offered under both policies notifies once.

## Availability and rate limits

Checks use GitHub's public API without a login or token. No separate catalog file
or website deployment is needed. GitHub applies limits shared by your public IP;
rate-limited requests wait until retry is allowed. Forbidden responses also pause
checks for at least one minute, including manual checks, because secondary rate
limits can omit retry headers. Failed checks retain the last
validated offer, which must be refreshed before opening when stale. The app never
downloads or installs updates. Previously published apps are not migrated by this
change; it takes effect after installing a future build that includes it.

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

## Direct macOS updater readiness

The direct macOS edition continues to use **Open release** and manual DMG
installation. Updater archives are prepared separately for verification; their
presence does not enable automatic installation or change release policy.
Native replacement and preservation of settings, login behavior and speaker
synchronization have not yet passed signed packaged-upgrade acceptance.
Mac App Store editions continue to update through the Store.
