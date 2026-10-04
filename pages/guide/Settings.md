---
layout: guide
---

# Settings guide

Open Settings from the Windows or Ubuntu system tray, or the macOS menu bar. The sidebar contains eight sections: Devices, Speaker, Night schedule, Volume, General, Updates, Diagnostics, and About.

Most configuration changes save automatically after a short delay. Speaker sound controls are applied directly to the selected speaker. The Night schedule grid is saved explicitly with **Save schedule**; its enable switch and notification preference save immediately.

If Settings shows **Sonos Volume Bridge is still running**, quit the old app and choose **Check again**. While that conflict is active, synchronization, speaker writes, and scheduled Night Mode changes pause. See [Upgrading](/guide/Upgrading.html) and [Diagnostics](/guide/Diagnostics-and-Troubleshooting.html) for the recheck states.

## Default configuration

| Setting | Default | Purpose |
| --- | --- | --- |
| Sonos speaker | None | The speaker controlled by the bridge |
| Follow | Follow system output | Tracks the current default computer output |
| Synchronize mute | On | Synchronizes mute when supported |
| Two-way synchronization | On | Allows Sonos changes to update the computer |
| Mute speaker at zero volume | Off | Treats zero computer volume as Sonos mute |
| Highest speaker volume | 55% | Caps the level sent to Sonos |
| Volume feel | Balanced | Gives finer control at low volumes |
| Enable schedule | Off | Runs the saved Night Mode schedule for the selected speaker |
| Night schedule grid | Empty | Seven days of 30-minute blocks |
| Night schedule notifications | Never | Optional native desktop notifications |
| Start at login | Off | Starts the app after sign-in |
| Keep checking if updates are missed | On | Polls as a recovery path when events are missing |
| Automatically check for updates | On for recognized releases | Checks the public catalog for the installed edition |
| Update notifications | On for recognized releases | Allows native notices when a verified update is available |

## Settings sections

- [Devices](/guide/Devices.html) selects the Sonos speaker, computer output, and mute behavior.
- [Speaker](/guide/Speaker-Settings.html) controls features exposed by the selected Sonos model.
- [Night schedule](/guide/Night-Schedule.html) sets weekly Night Mode periods and notifications.
- [Volume](/guide/Volume-Settings.html) controls direction, maximum volume, mapping, and testing.
- [General](/guide/General-Settings.html) controls startup and fallback checking.
- [Updates](/guide/Updates.html) shows the installed distribution, manual and automatic checks, and notification choices.
- [Diagnostics](/guide/Diagnostics-and-Troubleshooting.html) shows live connection information and reset tools.
- **About** shows the installed version, source repository, license, and independence notice.

## Reset settings

Open **Diagnostics** and choose **Reset settings** to restore the defaults in the table above. This removes your speaker and fixed-output selection and clears the saved Night schedule and notification preferences, so setup must be completed again.

## Platform presentation

macOS uses grouped controls, colored sidebar icons, compact switches and sliders, and previous/next section navigation. Windows uses Windows 11-style cards and responsive controls, with light/dark, high-contrast, and reduced-motion support. Ubuntu uses grouped settings rows, a sidebar, system fonts, and light/dark styling. macOS and Ubuntu windows resize vertically while keeping their width fixed. Settings retain keyboard navigation and accessible labels.

Sliders preview while dragging and apply on release, including key release for keyboard changes. Background updates preserve active edits. Volume, mute, and connection status update live, with backup reads when notifications are missed.
