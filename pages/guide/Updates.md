---
layout: guide
---

# Update checks

Speaker Volume Bridge can check the public GitHub release list for an update that
matches the installed edition, operating system, and application architecture.
The app never downloads or installs a package itself.

## Updates page

Open **Updates** in Settings to see the installed version and recognized
distribution, the last successful check, and the current checker state. Choose
**Check for updates** for a manual check. When a verified update is available,
**Open update page** opens its validated HTTPS release page in the
default browser.

An available update remains visible after choosing **Later** or denying an
operating-system notification. The app revalidates the offered version and URL
immediately before opening it.

## Automatic checks

Recognized release packages check about 30 seconds after startup by default.
After a successful check, the result remains fresh for 24 hours. Temporary
failures use bounded retry delays and remain quiet in the background. Manual
checks can retry and show an error without interrupting volume synchronization.

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
**Include prereleases** considers GA and public prereleases, including Alpha/Beta
packages with ordinary numeric versions. The app selects the highest compatible
semantic version, regardless of publication order. Store, sideloaded, development,
unknown and custom editions omit this selector.

Changing policy saves it for restart and checks once even with automatic checks
off. Previous links disappear immediately; errors leave Check for updates
available. Automatic-check, notification and speaker preferences are preserved.
Returning to Stable only waits for a strictly newer GA release without downgrading.
Equal-version promotion does not reinstall a preview. Open update page opens the
specific release's page. A release offered under both policies notifies once.

## Availability and rate limits

Checks use GitHub's public API without a login or token. No separate catalog file
or website deployment is needed. GitHub applies limits shared by your public IP;
rate-limited requests wait until retry is allowed. Failed checks retain the last
validated offer, which must be refreshed before opening when stale. The app never
downloads or installs updates. Previously published apps are not migrated by this
change; it takes effect after installing a future build that includes it.
