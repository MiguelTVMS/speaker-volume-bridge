---
layout: guide
---

# Volume settings

The **Volume** page controls synchronization direction, safety limits, and how computer volume maps to Sonos volume.

## Two-way synchronization

When enabled, confirmed Sonos volume changes can update the computer output. On the first successful connection, Sonos state is applied locally.

When disabled, the bridge runs computer to Sonos only. Sonos observations still confirm connection health, but they do not change the computer output. On the first successful connection, the computer state is sent to Sonos.

The current treatment of computer output volume is being discussed in [issue #74](https://github.com/MiguelTVMS/speaker-volume-bridge/issues/74).

## Repeated callbacks and volume feedback

In v1.6.3, repeated or overlapping notifications from the app's own computer-volume writes are kept from becoming new Sonos commands on macOS and Ubuntu. The app also skips computer volume and mute writes when the requested property already matches. This protection is automatic and does not pause listening for other volume changes.

A computer-volume change at connection time remains expected with two-way synchronization enabled. If volume keeps moving after you stop adjusting it, follow [Diagnostics and troubleshooting](/guide/Diagnostics-and-Troubleshooting.html).

## Mute speaker at zero volume

When enabled, moving computer volume to 0% also mutes the speaker. Raising volume above zero allows it to be unmuted through normal synchronization, provided the computer itself is not muted.

In v1.6.3, this option also sends the computer's mute state with local changes even when **Synchronize mute** is off. Turn both options off to stop computer-to-Sonos mute commands. Sonos-to-computer mute follows **Two-way synchronization**. See [Devices](/guide/Devices.html).

## Highest speaker volume

This is a safety cap for volume commands sent by the bridge to Sonos. The default is 55%. It does not prevent another controller or the speaker itself from setting a higher volume.

Start conservatively, especially with powerful speakers, amplifiers, or unfamiliar volume mappings. Raising the computer volume to 100% never authorizes the bridge to exceed this cap.

## Volume feel

| Choice | Behavior | Good starting point |
| --- | --- | --- |
| Balanced | Gives more control at lower levels and rises more gently | Normal desktop listening |
| Direct | Keeps Sonos volume close to the computer percentage, subject to the cap | Users who want similar numbers on both sides |
| Scaled | Maps the full 0% to 100% computer range across 0% to the selected maximum | Using the full range of keyboard or system controls |

In v1.6.3, the Balanced curve reaches 55% Sonos volume when the computer reaches 100%. The highest-volume setting can lower that result, but raising the cap above 55% does not raise the Balanced curve. Choose Direct or Scaled if you need the selected maximum to be reachable above 55%.

## Test speaker volume

The test reads the speaker's current volume and writes the same value back. It verifies that the selected device accepts a volume command without intentionally making the speaker louder or quieter.

If the test fails, check [Diagnostics and troubleshooting](/guide/Diagnostics-and-Troubleshooting.html).
