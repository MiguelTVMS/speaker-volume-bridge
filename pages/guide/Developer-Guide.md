---
layout: guide
---

# Developer guide

The user guide is the guide's primary purpose. These notes provide an entry point for contributors without replacing the repository's source-controlled technical documentation.

## Prerequisites

- Rust 1.98.1 with `rustfmt` and `clippy`.
- Node.js 24 LTS, as specified in the released development guide.
- pnpm 12.
- Platform build tools required by Tauri.

Install frontend dependencies and start the desktop app with:

```sh
pnpm --dir ui install
cargo tauri dev
```

Run the complete local verification suite with:

```sh
pnpm run ci:all
```

## Local environment

Copy `.env.example` to `.env.development` when local validation needs environment-specific values. The repository's local scripts load that file before running pnpm and Rust commands while preserving variables already exported by the shell.

Never commit `.env.development`, credentials, certificates, or private keys.

## Ubuntu development

For Ubuntu development, install Tauri's desktop build dependencies and `pulseaudio-utils` for the PulseAudio-compatible `pactl` interface:

```sh
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev pulseaudio-utils dbus-daemon
```

The audio adapter works with PulseAudio or PipeWire's `pipewire-pulse` service. Installed applications also require `pulseaudio-utils` at runtime.

## Repository layout

| Path | Responsibility |
| --- | --- |
| `crates/domain` | Validated values, mappings, confirmed state, and pending intent |
| `crates/synchronization` | Platform-neutral state machine and side effects |
| `crates/integration` | Coordinator, serialization, and command coalescing |
| `crates/sonos` | SSDP, device descriptions, SOAP control, and GENA events |
| `crates/platform-audio` | Windows Core Audio, macOS Core Audio, and Ubuntu PulseAudio-compatible adapters |
| `crates/test-support` | Local Sonos mock server and fixtures |
| `src-tauri` | Application composition, configuration, runtime, tray, and commands |
| `ui` | Settings interface and frontend tests |
| `docs` | Architecture decisions, protocol notes, verification, and release details |

## Contribution workflow

The application repository uses Gitflow. Create focused `feat/`, `fix/`, or `chore/` branches from current `develop`, and target pull requests to `develop`. Keep `main` stable for completed releases.

For behavior changes:

- Add or update tests. Every bug fix needs a regression test that reproduces the interaction through its production entry point or shared orchestration, fails with the faulty behavior, and passes with the fix.
- Keep domain and synchronization crates independent of Tauri, OS APIs, and networking.
- Keep platform and protocol details in their adapters.
- Run the complete validation suite.
- Record hardware-specific behavior as unverified until it is tested on the actual platform and device. Document manual reproduction and CI coverage limits in the verification matrix.

Public issues and pull requests must not contain credentials, local paths, network addresses, device identifiers, raw logs, diagnostic payloads, or other sensitive environment details.

## Useful source documentation at v{{ site.data.release.version }}

- [Documentation index](https://github.com/MiguelTVMS/speaker-volume-bridge/blob/v{{ site.data.release.version }}/docs/README.md)
- [Architecture](https://github.com/MiguelTVMS/speaker-volume-bridge/blob/v{{ site.data.release.version }}/docs/architecture.md)
- [How it works](https://github.com/MiguelTVMS/speaker-volume-bridge/blob/v{{ site.data.release.version }}/docs/how-it-works.md)
- [Development](https://github.com/MiguelTVMS/speaker-volume-bridge/blob/v{{ site.data.release.version }}/docs/development.md)
- [State machine](https://github.com/MiguelTVMS/speaker-volume-bridge/blob/v{{ site.data.release.version }}/docs/state-machine.md)
- [Sonos local protocol](https://github.com/MiguelTVMS/speaker-volume-bridge/blob/v{{ site.data.release.version }}/docs/sonos-local-protocol.md)
- [Architecture decisions](https://github.com/MiguelTVMS/speaker-volume-bridge/blob/v{{ site.data.release.version }}/docs/decisions/README.md)
- [Verification matrix](https://github.com/MiguelTVMS/speaker-volume-bridge/blob/v{{ site.data.release.version }}/docs/verification-matrix.md)
- [Release process](https://github.com/MiguelTVMS/speaker-volume-bridge/blob/v{{ site.data.release.version }}/docs/release.md)
- [Night schedule architecture decision (v{{ site.data.release.version }})](https://github.com/MiguelTVMS/speaker-volume-bridge/blob/v{{ site.data.release.version }}/docs/decisions/0013-global-night-mode-schedule.md)
- [Rebrand and legacy-app protection decision](https://github.com/MiguelTVMS/speaker-volume-bridge/blob/v{{ site.data.release.version }}/docs/decisions/0018-rebrand-and-legacy-protection.md)
- [Upgrade behavior and validation](https://github.com/MiguelTVMS/speaker-volume-bridge/blob/v{{ site.data.release.version }}/docs/rebrand-upgrade.md)
