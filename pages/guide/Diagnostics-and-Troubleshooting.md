---
layout: guide
---

# Diagnostics and troubleshooting

Start on the **Diagnostics** page. It shows the connection label, selected speaker, speaker volume, speaker input format when available, selected computer output, output volume, mute state, and speaker-search result.

## If disabling Start at login reports Operation not permitted on macOS

Open **System Settings > General > Login Items & Extensions** and remove the
bridge's entry from **Open at Login**, then retry the app's switch. After an
upgrade, check for both **Sonos Volume Bridge** and **Speaker Volume Bridge**.
If you want to start over, follow [Fully removing the old app](/guide/Removing-the-Old-App.html)
for the complete uninstall and optional settings reset. Deleting shared settings
also resets the renamed app's preferences.

## If volume or Night Mode changes appear to conflict

Make sure Sonos Volume Bridge is not still running under the former name. Closing its Settings window is not enough: choose **Quit** from its menu bar or system tray and remove any old startup entry. Version {{ site.data.release.version }} does not inspect running processes automatically. Follow [Upgrading](/guide/Upgrading.html) and [Fully removing the old app](/guide/Removing-the-Old-App.html) before retesting.

## If the app says Disconnected

Work through these checks in order:

1. Confirm that a Sonos speaker is selected under **Devices**.
2. Confirm that the computer and speaker are on the same local network.
3. Refresh the speaker list.
4. Confirm that the selected computer output is still present and has software volume control.
5. If following the system output, verify that the operating system has a usable default multimedia output.
6. Temporarily test without a VPN or network-isolation rule that blocks local discovery or callbacks.
7. Keep **Keep checking if updates are missed** enabled.
8. Quit the app from its tray menu, start it again, and retest.

The runtime retries recoverable speaker, output, and network failures automatically.

On Ubuntu, a missing `pactl` command or unavailable PulseAudio-compatible service appears as unavailable local audio. Follow the [Ubuntu troubleshooting steps](/guide/Ubuntu.html) before resetting Settings.

## If the speaker is found but volume does not change

- Use **Test speaker volume** on the Volume page.
- Confirm that the selected Sonos device is the group coordinator for a grouped setup.
- Confirm that the selected computer output is the device whose volume you are changing.
- Check whether a hardware control changes a different mixer or endpoint than the operating system's selected output.

## If Sonos changes do not update the computer

- Confirm that **Two-way synchronization** is enabled.
- Confirm that the local output allows software volume changes.
- Keep fallback checking enabled in case Sonos event callbacks are blocked.
- **Two-way synchronization** also applies confirmed Sonos mute state to the computer in v{{ site.data.release.version }}. **Synchronize mute** controls ordinary computer-to-Sonos mute commands.

## If volume keeps changing after an adjustment

1. Check the installed version under **About**. Update older installations to v{{ site.data.release.version }}, which includes the feedback fixes introduced in v1.5.5. Confirm that the former app is not also running.
2. Confirm the selected computer output and review **Two-way synchronization**, **Volume feel**, and **Highest speaker volume**. A Sonos-confirmed computer-volume adjustment is expected in two-way mode.
3. Retest with one volume controller at a time and note whether the movement follows a keyboard adjustment, a Sonos adjustment, or an output switch.
4. If the problem persists, record the operating system, generic output type, mapping choice, synchronization direction, and reproduction steps for a support report. Keep raw diagnostics private.

The fix handles repeated callbacks; it does not change the selected mapping or turn off two-way synchronization.

## If a Speaker control is gray

Check whether the label says **Not supported by this speaker** or **Temporarily unavailable**. Reconnect the speaker and use **Refresh speaker settings**. Temporary failures can recover; a gray control alone does not prove the feature is unsupported.

## Technical details

Expand **Speaker technical details** to refresh and display a sanitized diagnostic snapshot. The exact internal state can distinguish synchronization, fallback, unavailable-device, and error conditions.

**Export diagnostics** produces the sanitized diagnostic text. Review it before sharing. Do not paste raw logs, local paths, network addresses, device identifiers, host names, screenshots containing them, or other private details into a public issue.

Application logs are stored locally and are not uploaded automatically. They can contain technical identifiers needed for diagnosis. Use the default information level for normal operation; debug or trace logging should be limited to a short, controlled diagnostic session.

## Reset settings

Use **Reset settings** only after recording the non-sensitive choices you want to restore. Resetting returns the app to defaults and clears the selected speaker and fixed audio output.

## Request support

Search the [existing issues](https://github.com/MiguelTVMS/speaker-volume-bridge/issues) before opening a new one. In a public report, include only:

- The generally available app version.
- Windows, macOS, or Ubuntu and its version.
- The generic speaker model and audio-output class.
- The expected behavior and a concise description of what happened.
- Which troubleshooting steps passed or failed.

Keep raw diagnostic evidence private until a maintainer provides an approved way to share it.

## If Night scheduling does not behave as expected

- Confirm that the former Sonos Volume Bridge app is not also running.
- Confirm the selected speaker supports Night Mode and is reachable.
- Check **Enable schedule**, the saved grid, and the computer's time zone. Draft grid changes need **Save schedule**.
- Keep the app running. After startup, wake, or recovery, only the current expected state is applied.
- During a selected period, manual off is blocked and external off changes are corrected. Disable scheduling before using manual off.
- Switching speakers applies the same schedule to the new selection; it does not reset the previous speaker.
- If notifications are missing, check **Night schedule notifications**, system notification permissions, and Focus or Do Not Disturb. Saving an unchanged grid does not test notifications. Enable a previously disabled schedule during a selected block with **On start** or **On start and end** selected to check start delivery. Permission denial does not stop scheduling.

See [Night schedule](/guide/Night-Schedule.html) for the full editing and notification rules.
