---
layout: guide
---

# Update checks

Speaker Volume Bridge can check the public project catalog for an update that
matches the installed edition, operating system, and application architecture.
The app never downloads or installs a package itself.

## Updates page

Open **Updates** in Settings to see the installed version and recognized
distribution, the last successful check, and the current checker state. Choose
**Check for updates** for a manual check. When a verified update is available,
**Open update page** opens its validated HTTPS release or Store page in the
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

The catalog request contains no speaker details, configuration, analytics,
advertising data, or persistent installation identifier. As with any HTTPS
request, the website host can receive the public IP address, request time,
catalog path, and network metadata. See [Privacy and security](/guide/Privacy-and-Security.html).

Catalog entries are published only after the exact edition and architecture are
publicly available. Direct downloads and Store editions can therefore offer
different versions at the same time. A missing catalog entry means no verified
offer is available for that installation; it does not prove the installed
version is current.

## Stable releases or Prereleases

Recognized direct macOS, direct Windows and official Debian packages offer
**Release policy** in Updates. **Stable releases** is the default and includes
public GA releases. **Prereleases** opts into public Alpha/Beta releases and newer
stable releases. Preview packages may use ordinary numeric version numbers.
Store, sideloaded, development, unknown and custom editions omit this selector.

Changing the policy saves it for the next launch and checks once, even when
automatic checks are off. Previous offers and links disappear immediately while
checking. Errors remain visible with **Check for updates** available to retry.
Your automatic-check, notification and speaker preferences are preserved.

Switching to Stable never downgrades or replaces the app: you will receive the
next newer stable release. Equal-version GA promotion does not reinstall a preview.
Offers always match your distribution and application architecture. Preview offers
open that specific release's page. Empty or missing preview information means
unavailable, not up to date. A release offered through both policies notifies once.

## When a public release becomes available in Updates

A public release first goes through package verification and a catalog review.
GA releases update both feeds; Alpha/Beta update the Prereleases feed. Catalog
approval and delivery can therefore follow application publication. A public
release page alone does not prove that its offer is live in Updates.

Approved catalogs are delivered independently of another application release.
The website keeps the last published GA pages and displayed website version
while the feeds change. Store availability is confirmed separately. Older Stable
clients keep their existing feed address. Temporary delivery failures leave the
app's last validated offer available; **Check for updates** can retry after the
catalog is verified live. The app continues to open an update page and does not
download or install an update automatically.
