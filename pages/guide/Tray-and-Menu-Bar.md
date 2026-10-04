---
layout: guide
---

# Tray and menu bar

Speaker Volume Bridge is designed to run in the background.

If upgrading from Sonos Volume Bridge, choose **Quit** from the old app's tray or menu-bar menu before launching the renamed app. Closing its Settings window leaves it running and can cause conflicting controls. Follow the [upgrade guide](/guide/Upgrading.html).

On Windows and Ubuntu it lives in the system tray. On macOS it lives in the menu bar and does not appear as a normal Dock application.

## Status at a glance

The menu shows:

- **State: Connected** or **State: Disconnected**.
- The selected speaker name and cached volume when available.
- Shortcuts to **Settings** and **Diagnostics**.
- Supported Sonos speaker controls.
- A checked **Night schedule** toggle directly above **Night sound**.
- **Quit**.

The icon uses a disconnected badge when setup or connectivity needs attention. Both left-click and right-click refresh the menu before it is shown.

## Connected does not mean only one internal state

**Connected** covers normal synchronization, a pending Sonos confirmation, degraded event delivery, and polling fallback. These are all operating states.

**Disconnected** covers incomplete setup, discovery or connection in progress, unavailable speaker or output, unsupported local audio, and errors.

Open [Diagnostics](/guide/Diagnostics-and-Troubleshooting.html) when the binary label is not specific enough.

## Closing and quitting

Closing the Settings window hides it. Synchronization continues in the background.

Choose **Quit** from the tray or menu-bar menu to stop the runtime completely.

On Ubuntu, tray visibility depends on the desktop environment's app-indicator support. Synchronization can still run when a desktop hides the icon.

## Speaker controls and scheduling

Supported controls load asynchronously at startup without requiring an initial click. Slow responses do not block the interface, and results from a previously selected speaker are discarded. Speaker notifications and backup refreshes keep controls current.

The **Night schedule** checkmark means recurring scheduling is enabled. Use it to toggle the saved schedule; open Settings → **Night schedule** to edit the grid. Disabling it leaves the current Night sound value unchanged and remains possible when the speaker is unavailable. While a scheduled period is active, disable the schedule before turning Night sound off manually.
