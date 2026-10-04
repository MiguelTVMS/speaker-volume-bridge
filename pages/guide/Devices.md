---
layout: guide
---

# Devices settings

The **Devices** page chooses the two endpoints kept in step.

## Sonos speaker

Select one discovered Sonos speaker by name. The refresh button searches the local network again.

The app remembers the speaker by its stable Sonos identity rather than by an IP address. If the saved speaker is temporarily unavailable, it appears as **Speaker unavailable** so the selection is not silently lost.

Discovery accepts Sonos ZonePlayer identities and filters unrelated network media renderers from the speaker list.

## Follow

**Follow system output** is the normal choice for laptops and computers that switch between outputs. When the operating system changes its default multimedia output, the bridge detaches from the old output and follows the new one.

Choose a named output when the bridge should stay attached to one device regardless of system-default changes. If that saved device is missing, Settings reports **Output unavailable** and waits for it to return.

Only outputs whose volume can be changed by software are offered. HDMI, digital, professional, or virtual devices may expose no writable volume control and therefore may not appear.

On Ubuntu, outputs are PulseAudio-compatible sinks reported by `pactl`. See [Ubuntu](/guide/Ubuntu.html) for the required audio service.

## Synchronize mute

When enabled, computer mute changes are sent to Sonos immediately rather than waiting for the volume debounce.

In v1.6.3, turning this off stops ordinary computer-to-Sonos mute commands only when **Mute speaker at zero volume** is also off. With **Two-way synchronization** enabled, confirmed Sonos mute state is still applied to the computer. Turn off two-way synchronization if Sonos must not change the computer's volume or mute.

This setting is separate from **Mute speaker at zero volume**, which is explained in [Volume settings](/guide/Volume-Settings.html).
