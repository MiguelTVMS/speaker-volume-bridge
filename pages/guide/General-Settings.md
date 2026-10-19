---
layout: guide
---

# General settings

## Start at login

Starts Speaker Volume Bridge automatically after you sign in.

Ordinary settings saves do not require login-item registration. The operating system login service is contacted only when **Start at login** changes, so a login-service failure does not block unrelated first-launch settings saves.

Turn this on after the selected speaker and audio output work reliably. On Windows, uninstalling the application removes its startup entry unless the installer is performing an in-place update.

After upgrading from Sonos Volume Bridge, check this preference. On macOS, replacing the differently named app bundle can require checking the login item. On Ubuntu, remove any manually created duplicate startup entry for the old app. See [Upgrading](/guide/Upgrading.html).

## Keep checking if updates are missed

Keeps a conservative polling recovery path available when normal Sonos event notifications are unavailable or stale. This setting is enabled by default.

Leave it on for normal use. Turn it off only when diagnosing network behavior or when a maintainer specifically requests that test. Without fallback polling, a network that blocks callback notifications can take longer to reflect external Sonos changes.

When fallback polling is active, the app can still show **Connected** because synchronization remains operational through the recovery path.

Night scheduling runs independently of this fallback-checking option and of local audio availability. See [Night schedule](/guide/Night-Schedule.html).
