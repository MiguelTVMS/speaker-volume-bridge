---
layout: guide
---

# Night schedule

Included in the documented release, [v1.6.3](https://github.com/MiguelTVMS/speaker-volume-bridge/releases/tag/v1.6.3).

Use **Night schedule** in Settings to turn Night Mode on automatically during selected weekly periods. It controls the same speaker feature labeled **Night sound** on the Speaker page.

Do not run the former Sonos Volume Bridge app at the same time. Two installations can send conflicting Night Mode changes. Quit and remove the old app before using schedule controls. See [Upgrading](/guide/Upgrading.html).

## Set up quiet hours

1. Select a compatible Sonos speaker under **Devices**.
2. Open **Night schedule**.
3. Select the half-hour blocks when Night Mode should stay on.
4. Choose **Save schedule**. This saves the grid and applies its current block immediately.
5. Turn on **Enable schedule** for recurring operation.
6. Optionally choose **Night schedule notifications**.

The default is an empty grid, scheduling off, and notifications set to **Never**. One global schedule follows the selected speaker. Switching speakers applies that schedule to the new selection without changing the previous speaker. Unavailable or unsupported speakers pause scheduling and preserve the grid.

## Edit the week

The grid has Monday–Sunday rows and 48 half-hour cells per day. Hover a cell to see its exact interval. Times follow the computer's time zone and native clock preferences: macOS preferences, Windows regional time format, or GNOME clock format with an LC_TIME fallback on Linux. Reopen or focus Settings after changing clock preferences.

- Click a cell or use Space to toggle it. Arrow keys move between cells.
- Drag from an empty cell to select a rectangle across times and days. Drag from a selected cell to clear a rectangle.
- Moving back during a drag shrinks the rectangle and restores cells outside it.
- **Clear all** clears the draft. It takes effect only after saving.
- **Cancel** restores the saved grid.
- Background refreshes and speaker selection changes preserve unsaved drafts.

For a period crossing midnight, select the late-evening cells on one day and the early-morning cells on the next. Adjacent selected blocks form one continuous period, including across midnight and Sunday into Monday.

## Saving and enabling are different

**Save schedule** applies the current saved block once when speaker control is available: Night Mode on inside a selected block, off outside. This works even with recurring scheduling disabled and can be repeated without editing the grid. An old-app conflict blocks speaker writes.

**Enable schedule** saves immediately and starts recurring operation using the saved grid. The notification preference also saves immediately; neither replaces **Save schedule** for draft grid edits.

While enabled:

- Inside selected periods, Night Mode stays on. Settings and the tray block manual off, and external off changes are corrected.
- Leaving a selected period turns Night Mode off once.
- Outside selected periods, manual changes remain effective until the next applicable schedule action.

Disable scheduling before turning Night sound off during a selected period. **Disabling scheduling alone leaves the speaker's current Night Mode state unchanged.**

The tray's checked **Night schedule** item toggles recurring scheduling. Edit the grid through Settings.

## Notifications

| Choice | Notification |
| --- | --- |
| Never | None; the default |
| On start | When a scheduled period starts |
| On end | When a scheduled period ends |
| On start and end | At both boundaries |

Notifications require a confirmed speaker state. Adjacent selected cells do not create repeated notifications. Startup, wake, reconnection, selection changes, manual changes, and enforcement corrections are silent.

**Save schedule** sends a notification only when the edit makes the current time enter or leave an enabled scheduled period, after speaker confirmation and only for the selected direction. Saving while remaining inside or outside the period sends no notification. Saving with scheduling disabled is silent.

Enabling a disabled schedule during a selected period applies Night Mode immediately and can send a start notification. Enabling outside a selected period, enabling an already enabled schedule, and disabling scheduling are silent. To check start notifications, choose **On start** or **On start and end**, save a block covering the current time, then enable a previously disabled schedule.

Allow Speaker Volume Bridge notifications in system settings when requested. Focus, Do Not Disturb, or desktop notification settings can suppress delivery. Scheduling and volume synchronization continue even when notifications are blocked.

Windows shows schedule state, the next change, notification guidance, and errors in the **Status** card. Ubuntu shows a **Schedule status** row below notifications. Successful saves do not show a confirmation message.

## Sleep, travel, and interruptions

The app must be running for scheduling to act. Closing Settings keeps it running; quitting stops scheduling. After startup, wake, reconnection, speaker selection, schedule enabling, or clock/time-zone changes, it reconciles the current expected state without replaying missed transitions. Daylight-saving changes follow local civil time: skipped boundary times move to the first valid instant, and repeated times use the first occurrence.

Scheduling runs independently of computer audio availability and **Keep checking if updates are missed**. A missing local output can affect volume synchronization while the schedule still works with a reachable compatible speaker.

See [Speaker settings](/guide/Speaker-Settings.html) and [Diagnostics and troubleshooting](/guide/Diagnostics-and-Troubleshooting.html).
