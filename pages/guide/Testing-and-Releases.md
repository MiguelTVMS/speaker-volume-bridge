---
layout: guide
---

# Testing and releases

## Local verification

The baseline suite is:

```sh
pnpm run ci:all
```

It covers Rust formatting, strict Clippy, Rust tests, frontend formatting, frontend linting, frontend unit tests, and the frontend production build. Browser interaction tests run separately with `pnpm --dir ui run test:ui` after installing Playwright browsers. They cover platform presentation and Night schedule interactions. The test server starts Vite directly so shutdown completes cleanly.

Native adapter probes are available on their respective platforms:

```sh
cargo run -p speaker-volume-bridge-platform-audio --example windows_audio_probe
cargo run -p speaker-volume-bridge-platform-audio --example macos_audio_probe
```

The test-support crate provides a local RenderingControl mock server and recorded XML fixtures. Mock and unit validation does not replace physical Sonos and operating-system testing.

## Volume feedback regression coverage

The v{{ site.data.release.version }} tests cover repeated and overlapping volume/mute callbacks, expiry, tolerance, bounded history, macOS intermediate channel states, Windows callback identity, unchanged local applications, and synchronization mappings and directions. Native Windows and Ubuntu tests run on both supported architectures; macOS tests run on macOS. Simulated callback tests do not establish behavior on every physical output device.

See [the released regression design](https://github.com/MiguelTVMS/speaker-volume-bridge/blob/v{{ site.data.release.version }}/docs/decisions/0017-volume-feedback-suppression.md).

## Rebrand and upgrade verification

The released tests cover the manual upgrade guidance and removal links. Packaging checks cover retained installation identities, renamed downloads and compatibility aliases, Debian replacement, and Windows default-folder migration with custom-path preservation and blocked-move handling.

These checks do not prove a native upgrade on every platform. Verify clean installation and upgrade, preserved settings, stopped/running preferences, startup, shortcuts, notifications, uninstall, and manual old-app removal on the actual signed or packaged distribution. See the [released upgrade checklist](https://github.com/MiguelTVMS/speaker-volume-bridge/blob/v{{ site.data.release.version }}/docs/rebrand-upgrade.md).

## Hardware acceptance

Every release candidate should verify:

- Discovery, stable selection, and reconnect.
- Computer-to-Sonos volume and mute.
- Sonos-to-computer behavior with two-way synchronization.
- Maximum-volume enforcement and mapping behavior.
- Event subscription, renewal, polling fallback, and recovery.
- Default and fixed output replacement.
- Clean shutdown and restart.
- Night schedule saving, recurring boundaries, manual-off enforcement, speaker changes, unsupported speakers, and independence from local audio availability.
- Native schedule notifications and permissions, sleep/wake recovery, time-zone changes, and daylight-saving transitions on each desktop.
- Platform signing, installation, startup, and uninstall behavior.
- On Ubuntu, PulseAudio and PipeWire compatibility, `pactl` availability, default and fixed sink behavior, tray visibility, and Debian package installation.

Record only non-sensitive model, operating-system, firmware, and pass/fail information. Do not publish serial numbers, network addresses, raw diagnostics, or crash payloads.

## Release flow

The Release workflow runs from `develop`, increments the single Cargo workspace version, validates that exact commit, builds Windows x64/ARM64 installers, an Apple Silicon macOS DMG, and Ubuntu x64/ARM64 Debian packages, creates the annotated version tag, and publishes GitHub release assets. Microsoft Store submission is opt-in for GA releases after GitHub publication. Mac App Store package creation is also opt-in and does not submit to Apple automatically.

Release channels are:

- **GA** for a full generally available release.
- **Beta** for broader testing before general availability.
- **Alpha** for early testing.

The workflow merges no branches. After successful GA publication, a separate job opens an approval-required promotion PR from a release branch pinned to the validated release commit into `main`. Publication does not wait for approval. Merge that PR with a merge commit to preserve the tagged history; alpha and beta releases do not open promotion PRs.

## Microsoft Store recovery and verification

Normal release submission and manual retries share the **Microsoft Store Publish** workflow. A retry validates an existing published GA tag and reuses its retained combined upload, checking the version and embedded x64 and ARM64 packages. The default dry run validates without submitting. Missing, expired, or incomplete artifacts are rejected rather than rebuilt automatically.

After submission, Store metadata must match the verified package. A combined upload may appear as **Neutral**; that label is accepted only with matching artifact/version evidence and local verification of both architectures. Submission checks are separate from certification and public availability.

Follow the [released Store recovery guide](https://github.com/MiguelTVMS/speaker-volume-bridge/blob/v{{ site.data.release.version }}/docs/release.md#retry-or-debug-an-existing-microsoft-store-release) and [decision record](https://github.com/MiguelTVMS/speaker-volume-bridge/blob/v{{ site.data.release.version }}/docs/decisions/0016-store-release-recovery.md).

## Guide release policy

The website guide is maintained with the application repository and published from `main`. It is updated only after a full GA release succeeds. The current GA baseline is v{{ site.data.release.version }}. All five platform downloads are published under the new name, with five compatibility copies under the old name. Store availability is separate from GitHub publication.

Before a guide update:

1. Confirm the GitHub release is not marked as a prerelease.
2. Confirm it is returned by the repository's latest-release endpoint.
3. Use the released tag as the source of truth for features, settings, defaults, assets, and limitations.
4. Recheck issue state before changing any limitation link.
5. Update the version banner and validate all internal and external links.

## Independent catalog activation

Catalog updates require verified public packages, normal pull-request checks and
human approval. They can be delivered from develop while retaining the last
approved GA website. A workflow-only hosting promotion preserves that website
and its displayed version; it does not publish another application release.

Initial activation requires an exact verified copy of the served GA website.
Missing retained artifacts or hosting transformations that change HTML bytes
leave activation pending until reviewed recovery and full comparison succeed.
Catalog delivery and installed-app verification are separate acceptance steps.
