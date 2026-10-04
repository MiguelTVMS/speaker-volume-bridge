---
layout: guide
---

# Privacy and security

## Local operation

Speaker Volume Bridge communicates directly with Sonos devices on the local network. Normal volume synchronization does not require an online account or an Internet connection.

The app does not include advertising, analytics, or publisher telemetry. It does not automatically upload configuration or log files.

## Information used by the app

The app accesses only what it needs to discover devices, show status, remember choices, and synchronize controls:

- Sonos device identity, friendly name, local address, volume, mute, and supported settings.
- Selected Windows, macOS, or Ubuntu audio-output identity, volume, and mute.
- Application preferences such as device selection, synchronization direction, mapping, maximum volume, startup behavior, and diagnostic level.

It does not intentionally collect names, email addresses, contacts, payment details, precise location, authentication credentials, or audio content.

## Local files

Configuration and diagnostic logs are stored in the operating system's per-user application-data locations. Logs can contain local device or network identifiers needed to diagnose a failure.

Resetting Settings returns the configuration to defaults. Uninstall behavior depends on the platform and distribution. See [Updating and uninstalling](/guide/Updating-and-Uninstalling.html).

## Old-app removal

The app does not inspect running processes to look for the former Sonos Volume Bridge app. Follow [Upgrading](/guide/Upgrading.html) and remove the old app and its startup entry manually so two installations do not send conflicting controls.

## Sharing diagnostics safely

Treat exported diagnostics and logs as private until reviewed. Do not publish raw logs, IP or MAC addresses, device IDs, computer or speaker names, local paths, crash dumps, or screenshots containing those values in a GitHub issue.

For the complete policy, read the v{{ site.data.release.version }} [Privacy Policy](https://github.com/MiguelTVMS/speaker-volume-bridge/blob/v{{ site.data.release.version }}/PRIVACY.md).

## Network protections

The Sonos client accepts only local HTTP device locations using private, loopback, or link-local literal IP addresses. Public addresses and host names from discovery responses are rejected. Protocol responses are bounded and control requests use short deadlines.

Install releases only from the [official GitHub repository](https://github.com/MiguelTVMS/speaker-volume-bridge/releases/latest) or an official store listing.

## Schedules and desktop notifications

The weekly Night schedule and notification preference are stored locally. Scheduling uses the computer's time zone and selected speaker. Native notification permissions are requested when you opt in; notification denial or desktop suppression does not stop synchronization or scheduling.

## Project website and security reports

The desktop app has no publisher analytics. The separate [project website](https://svb.miguel.ms/) offers consent preferences for optional website tracking; these are separate from app settings. See the [website privacy policy](https://svb.miguel.ms/privacy.html).

For a suspected vulnerability, follow the [Security Policy](https://github.com/MiguelTVMS/speaker-volume-bridge/blob/v{{ site.data.release.version }}/SECURITY.md) and use private reporting rather than publishing details in an issue.
