---
layout: guide
---

# Getting started

## Before you begin

- If upgrading from Sonos Volume Bridge, quit the old app and follow [Upgrading](/guide/Upgrading.html) first. A detected old app pauses synchronization and scheduled speaker changes.
- Put the computer and Sonos speaker on the same local network.
- Make sure the chosen computer output has software volume controls.
- If a VPN or firewall isolates local devices, allow local-network discovery and communication for Speaker Volume Bridge.

## First setup

1. Open Speaker Volume Bridge from the Windows or Ubuntu system tray, or the macOS menu bar.
2. On **Devices**, choose the speaker you want to control.
3. Under **Follow**, choose:
   - **Follow system output** to move with the computer's default output.
   - A named output to keep the bridge attached to that specific device.
4. Leave **Synchronize mute** on if mute changes should travel between the computer and speaker.
5. Open **Volume** and review the default maximum of 55% before testing.
6. Choose whether **Two-way synchronization** should be enabled.
7. Select **Test speaker volume**. The test reads the current speaker volume and writes the same value back, so it verifies control without intentionally changing the level.

Ordinary settings save automatically after a short delay. For automatic quiet hours, open [Night schedule](/guide/Night-Schedule.html), select blocks, choose **Save schedule**, then enable recurring scheduling.

## Choose the synchronization direction carefully

With **Two-way synchronization** enabled, a confirmed change made on Sonos can update the computer output. During the first connection, the confirmed Sonos state is applied to the computer.

With it disabled, synchronization is computer to Sonos only. During the first connection, the computer's current state is sent to Sonos.

If an unexpected computer-volume change would be disruptive, start with two-way synchronization disabled and a conservative highest speaker volume.

## Know when it is working

The sidebar and tray show **Connected** when the bridge is synchronized or operating through a healthy recovery mode. They show **Disconnected** while setup is incomplete or either selected device is unavailable.

Closing the Settings window does not quit the app. Use **Quit** from the tray or menu-bar menu when you want to stop synchronization.

## Grouped and home-theater setups

The bridge controls the selected Sonos device. It does not automatically resolve every group or stereo-pair configuration.

Create the group in the Sonos app first, then select the group coordinator in Speaker Volume Bridge. Verify the result with your own grouped, stereo-pair, or home-theater setup before relying on it. Reliable group and stereo-pair control is tracked in [issue #86](https://github.com/MiguelTVMS/speaker-volume-bridge/issues/86).

## Next steps

- [Understand every setting](/guide/Settings.html)
- [Use the tray or menu-bar controls](/guide/Tray-and-Menu-Bar.html)
- [Resolve a disconnected state](/guide/Diagnostics-and-Troubleshooting.html)
