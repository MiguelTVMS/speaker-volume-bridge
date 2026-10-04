---
layout: guide
---

# Speaker settings

The **Speaker** page controls sound features reported by the selected Sonos device.

The app probes the selected speaker instead of assuming support from its model name. Gray controls distinguish **Not supported by this speaker** from **Temporarily unavailable**. Temporary read failures are retried on refresh. An off switch or zero tone value is a valid setting, not evidence that a feature is unsupported.

## Available controls

| Control | What it changes |
| --- | --- |
| Night sound | Reduces the impact of loud effects for quieter listening when supported |
| Loudness | Applies the Sonos loudness contour when supported |
| Status light | Turns the speaker's status light on or off when supported |
| Speech enhancement | Emphasizes dialogue on supported home-theater speakers |
| Treble | Adjusts high-frequency tone from -10 to +10 |
| Bass | Adjusts low-frequency tone from -10 to +10 |

Changes are sent directly to the selected speaker. Speech enhancement supports the appropriate legacy or Ultra soundbar control and reads back the result. Failed or ambiguous reads are treated as temporary unavailability unless the speaker explicitly reports that the feature is unsupported.

Changes from the Sonos app or another controller refresh Settings and the tray through speaker notifications. Opening or focusing Settings, switching sections, and periodic backup reads also refresh controls. Treble and Bass preview during a slider gesture and apply on release.

## Use TV audio

**Use TV audio** asks a supported home-theater speaker to select its television input. Speakers without a compatible home-theater input reject the command without changing their source.

The Diagnostics page can show the current **Speaker input format** when the device exposes it.

## Tray controls

Supported on/off controls can also appear in the tray or menu-bar menu. Unsupported controls are omitted from that menu, while Settings keeps them visible in gray so the capability difference is clear.

## Scheduled Night sound

**Night sound** is the speaker control managed by [Night schedule](/guide/Night-Schedule.html). During an enabled scheduled period, it stays on and manual off is blocked. Disable the schedule before turning it off manually. Outside scheduled periods, manual changes are allowed.
