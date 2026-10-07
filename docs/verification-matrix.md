# Hardware verification matrix

## Phase-one update catalog recovery

- Automated production-orchestration coverage: `UpdateService::new` revalidates
  persisted offers on startup, removing offers equal to/older than the installed
  version, malformed versions/actions, wrong distribution targets, and legacy
  cache entries without target identity. It preserves valid newer offers and the
  last successful check while offline; it makes no request and does not bypass
  the 24-hour freshness interval.
- Shared JSON fixtures under `tests/fixtures/update-catalog` are exercised by the
  Rust consumer, site validator, and publication preparation tests. They cover
  additive catalog/entry/action metadata, missing required fields, unsupported
  actions, duplicate object keys, and duplicate targets.
- Native acceptance (not available in this environment): install an older signed
  package, obtain a newer offer, install that update manually, disconnect the
  network, and restart. Confirm the former offer is absent, the last successful
  check is retained, and no browser opens. Repeat after changing edition/channel/
  platform/architecture or restoring a legacy cache file; confirm the offer is
  discarded. Restore connectivity and confirm a valid newer offer remains
  discoverable. No live catalog entry is published by these checks.

## Linux ARM64 build

Local validation (2026-09-26): Ubuntu 26.04 ARM64, Rust 1.98.1, Node.js
24.21.0 LTS and pnpm 12.6.0 passed the standalone debug Tauri build, ELF
architecture and linked-library checks, Rust formatting, workspace Clippy and
tests, UI demo Rust tests, and frontend build, tests, lint and formatting.
Graphical and physical-device scenarios below remain manual verification.

Automated coverage: the Branch desktop check workflow builds on native
`ubuntu-24.04-arm`, runs frontend and Rust checks, verifies the executable's ELF
machine is AArch64, and uploads an artifact with the architecture in its name.
PR Rust quality checks run workspace and UI demo tests on the same architecture.
Workflow configuration alone is not evidence of a successful build; confirm the
ARM64 job passes before treating an artifact as verified.

Manual verification on an ARM64 Ubuntu desktop or VM:

1. Follow the native build commands in [development](development.md#linux-arm64).
2. Confirm `file` and `readelf -h` identify the executable as AArch64.
3. Start the normal app in a graphical session. Open Settings from the tray,
   select a local output and Sonos speaker, save, quit, and relaunch.
4. Verify saved selections, volume/mute synchronization, tray controls, and
   Night schedule interactions using the existing scenarios below.
5. Build again with `--debug --features ui-demo`, launch without a physical
   speaker, and repeat the UI demo scenarios below, including saved startup.
6. If testing a Debian package, confirm `dpkg-deb -f <package.deb> Architecture`
   reports `arm64`, install it, and repeat the desktop checks.

Coverage limits: CI compilation and automated tests do not verify a graphical
session, tray integration, audio service access, physical Sonos hardware, or
package installation. Building on Ubuntu 24.04 does not establish compatibility
with older distributions. Windows ARM64 and macOS builds are separate targets.

## Windows 11 settings presentation

- Automated: the normal frontend suite validates platform selection, native
  window identity, resize bounds, tray-first startup, and retained Windows chrome.
- Automated browser tests run the production renderer through the isolated preview:
  all seven pages at default/minimum size in light/dark mode, visible sidebar footer,
  card nesting, keyboard switch/select/slider changes across rerenders, forced colors,
  reduced motion, visible focus, and macOS/Linux presentation isolation.
- Regression mutation verified: removing the compact-height rules makes the
  minimum-size browser test fail because the speaker footer extends below the
  window. Restoring the rules makes the same test pass.
- Visual checks: launch `cargo tauri dev`, open Settings from the tray, and visit
  Devices, Speaker, Volume, General, Diagnostics, and About. Check the 960-by-760
  default and 760-by-460 minimum content sizes. Confirm all sidebar entries and
  the selected speaker remain reachable, cards scroll vertically, and long
  output names and diagnostics values do not create horizontal overflow.
- Repeat with light/dark appearance, increased contrast, reduced motion, and
  keyboard navigation. Switches must toggle with Space; selectors and sliders
  retain keyboard operation and visible focus. Use the isolated preview for
  interaction tests that should not change physical speaker settings.
- Development verification: Windows native light-mode window and isolated browser
  previews are checked locally. CI covers configuration and shared interaction
  logic, not native rendering, system contrast themes, or physical Sonos hardware.

Run this matrix for every release candidate on a private local network with one
supported Sonos speaker and one physical output device per operating system.
Complete this matrix before treating a release as ready for general use. The
macOS direct-download package is Developer ID signed and notarized. The Mac App
Store package is Apple Distribution signed with an embedded provisioning
profile. The Windows installer is currently unsigned.
Record the operating-system version, Sonos firmware version, speaker model, and
result in the release issue. Do not record device serial numbers, LAN addresses,
or diagnostic payloads.

## Shared Sonos checks

| Check | Expected result |
| --- | --- |
| Platform settings presentation | On each OS, check all six settings sections at default and minimum window heights, light/dark appearance, increased contrast, keyboard navigation, disabled controls, and long device names. Confirm the matching platform theme and no clipped controls. |
| Fresh-install settings save | With no saved configuration and Start at login off, choose a speaker and output and change a volume option. Confirm saving and reconnecting work, then relaunch and verify persistence without toggling Start at login. |
| Speaker control freshness | Change Speech Enhancement in another controller, then focus Settings or switch sections and verify the new state. Enable and disable from this app and verify authoritative state after refresh. Open the tray after hovering its icon and check matching controls. |
| Discovery and selection | Discovery lists the selected speaker by friendly name and stable UDN. Saving selection connects without exposing an address to the frontend. |
| Cached-address reconnect | Restart the app with network unchanged. The selected UDN is resolved through its cached local description URL before SSDP is used. |
| Sonos-originated volume change | A physical Sonos volume change updates local output after Sonos confirmation. |
| Local-originated volume change | A local volume change is coalesced, capped, reaches Sonos, and returns to `Synchronized` only after confirmation. |
| Mute synchronization | Mute is sent immediately when enabled; disabling mute synchronization leaves the other endpoint unchanged. |
| GENA lifecycle | Confirm a callback event, renewal before expiry, and status recovery from `Subscription degraded`. |
| Polling fallback | Temporarily block callback delivery. Confirm `Polling fallback` and recovery to event-driven synchronization after delivery resumes. |
| Reconnect and shutdown | Disconnect/reconnect the speaker network, then quit. Confirm reconnect backoff, no stale status update, and best-effort unsubscribe. |

## Windows checks

| Check | Expected result |
| --- | --- |
| Default output replacement | Change the default multimedia render endpoint. The adapter detaches, reattaches, and resumes synchronization. |
| Fixed output selection | Select a fixed endpoint and confirm default-device changes do not move synchronization. |
| Application write suppression | Sonos-confirmed local writes do not trigger a second Sonos command. |
| Device failure | Disconnect or disable the endpoint. The tray reports `Local audio unavailable` and recovers when available. |

## macOS checks

| Check | Expected result |
| --- | --- |
| First left-click after startup | Fully quit and relaunch with a selected speaker. After the initial speaker read completes, left-click the tray before any right-click. All supported Night sound, Loudness, Status light, and Speech enhancement controls appear with correct values. The automated startup test covers the refresh trigger and control mapping; this check covers native menu presentation and real device responses. |
| Bundle signature | `codesign --verify --deep --strict --verbose=4` succeeds for the downloaded app bundle and reports the expected Developer ID identity. |
| Notarization ticket | `xcrun stapler validate` succeeds for the downloaded app bundle. |
| Gatekeeper assessment | `spctl --assess --type execute --verbose=4` accepts the downloaded app bundle. |
| Sandbox entitlements | The signed app reports App Sandbox plus incoming and outgoing network entitlements, with no unrelated sandbox entitlement. |
| App Store profile | The App Store build contains `Contents/embedded.provisionprofile`; its App ID matches the bundle identifier and its entitlements authorize App Sandbox. |
| Clean-account launch | On a clean macOS account, install and launch each distribution separately. Confirm there are no sandbox denials affecting discovery, control, callbacks, reconnection, polling, Core Audio, tray operation, login launch, settings, diagnostics, or uninstall. |
| Default output replacement | Change the default output device and confirm safe listener replacement and recovery. |
| Master/channel volume | Test a device with master volume and, where available, a channel-only device. Both apply the expected local value. |
| Expected-write suppression | Sonos-confirmed local writes within configured tolerance do not produce a second Sonos command. |
| Unsupported device | A device without software volume reports `Unsupported local device` clearly. |

## Release decision

All required rows must pass on Windows and macOS. Document an exception in the
release issue with its device class, user impact, mitigation, and a follow-up
issue before declaring a release candidate ready.

### Speaker capability and push regression checks

- Automated: programmable local HTTP speaker fixtures exercise successful off/zero
  reads, explicit unsupported faults, timeout and malformed-reply recovery,
  independent feature failures, model-specific Speech Enhancement, advertised
  service URLs, and switching between speakers without sharing capabilities.
- Automated: real NOTIFY callbacks followed by SOAP reads confirm an external
  Speech Enhancement change and preserve capabilities omitted from the event.
  Frontend mock views test the same updater used on render and push refresh.
- Regression mutation: treating temporary read failures as unsupported makes the
  recovery test fail; restoring the availability classifier makes it pass.
- Manual (pending): leave Speaker settings open, toggle Speech Enhancement in an
  external controller, and confirm the displayed value updates without navigation;
  repeat off/on and check the tray. Disconnect/reconnect the speaker and verify
  controls recover. Repeat on legacy and Ultra soundbars when available. Mocks do
  not verify real firmware behavior, native menu timing, or network reachability.

### Slider and live telemetry regressions

- Automated shared production gesture binding with mocked input events: pointer
  drag/pause/release, keyboard repeat/release, cancellation, accessibility changes,
  and exactly one final commit. Readback of both echoes and external changes must
  produce zero additional writes. Rapid writes serialize and recover after failure.
- Automated telemetry delivery: a push updates immediately and supersedes an older
  pending snapshot read. Partial volume/mute notifications request fresh reads.
- Mutation checks: committing on change while held, or accepting stale snapshot
  reads, each makes its corresponding regression test fail.
- Manual: drag and hold Bass/Treble/maximum volume through a backup refresh, then
  release; confirm no jump during drag and only the final value applies. Change
  volume externally with Diagnostics open and check prompt updates. Native drag
  timing and physical notification latency are not simulated by the mock suite.

## Global Night Mode schedule

Automated coverage exercises calendar boundaries, DST, week wrapping, read/write
confirmation, external-off restoration, selected-speaker rebinding, disabled/no-target
behavior, manual-off rejection, retries and notification intent. Browser tests use
the production editor through the isolated preview; they do not touch real speakers.

Manual checks for the local candidate (not yet completed on physical hardware):

1. Select a compatible speaker. Open the Night schedule sidebar item. Verify the grid is absent from Speaker. Paint a block containing the current time, save,
   and enable. Confirm Night Mode on and the locked Settings/tray explanation.
   Disable it from another controller; it should restore within the backup-read
   interval plus network latency. Volume synchronization must continue.
2. Disable scheduling: Night Mode stays unchanged and manual off succeeds. Re-enable
   outside a selected block, manually turn on, and refresh Settings: it stays on.
3. Cross a start/end boundary. Confirm one on/off application and, with the matching notification
   mode and OS permission, one native notification after confirmation. Consecutive
   selected cells must not notify. Try with Settings open and closed.
4. Deny notifications, then enable them in system settings. Confirm clear guidance,
   working synchronization, and subsequent delivery without a background permission
   prompt. Check OS Do Not Disturb suppression. Windows requires an installed app;
   development-shell notification branding is not representative.
5. While selected, disconnect/reconnect the speaker and sleep/wake across a boundary.
   Confirm the current expected value is applied without replayed transitions or
   notifications. Repeat with volume fallback polling disabled and audio unavailable.
6. Switch between compatible, unsupported, and unavailable speakers while writes
   are pending. Only the backend's current selection may receive new commands;
   switching sends no cleanup commands to the former speaker. The global grid stays.
7. Verify light/dark presentation and keyboard painting on each desktop OS. Scroll
   the grid while a draft is dirty and allow background refreshes: preserve cells,
   focus, and scroll. Save/Cancel must not alter another speaker's configuration.

Coverage limits: native wake delivery, OS permission prompts/toast presentation,
physical speaker support, and network timing remain manual checks. Linux without
logind uses the clock-discontinuity fallback and cannot guarantee immediate native
wake delivery. Native notification display is controlled by the OS.

8. On Night schedule, confirm there is no repeated selected-speaker label. Leave recurring
   scheduling disabled, select the current block and Save: Night Mode turns on.
   Save unchanged again: no notification and no redundant Sonos write. Clear the current block and Save: Night Mode turns off. Permission denial
   must not prevent application, and failed confirmation must not announce success.

9. Open the native tray menu with a compatible speaker selected. Verify Night schedule is immediately above Night sound and its checkmark reflects whether scheduling is enabled and no editor shortcut remains. Toggle
   it on and verify the saved schedule takes effect, then uncheck Night schedule and verify Night Mode stays unchanged and manual off is available.
   Repeat disabling with the speaker unavailable. Native menu placement and click
   delivery need this local check; CI covers the menu model and shared command policy.

10. On macOS, launch the app, open the tray immediately, hover repeatedly, then
    click the schedule toggle and Night sound while background speaker reads finish.
    Verify the process stays running and the controls update in place. Repeat after
    speaker loss/recovery. CI covers the shared menu-diff sequence; native Cocoa
    action-target lifetime and menu tracking require this packaged-app check.

11. In the schedule form, verify On start/On end/On start and end/Never at both boundaries and
    with Save. Confirm Portuguese locale uses 24-hour labels and US English uses
    AM/PM, including midnight tooltips. Hover a grid cell and verify the tooltip
    appears promptly and disappears on leaving or painting.
    On Windows, repeat in light and dark modes: hover a middle-row cell so the
    tooltip overlaps the grid and verify no cells show through its background.
    CI exercises the production renderer's hover sequence and checks an opaque
    computed background in both modes; native WebView rendering remains manual.

12. With macOS language English (US), region Portugal and a 24-hour clock, verify
    the schedule shows 13:00 rather than 01:00 PM. Change the system clock format,
    refocus Settings, and verify labels/tooltips update without losing grid edits.
    CI covers a US WebView locale with a native 24-hour override at startup.

### Windows native notification delivery

Windows delivery verification must distinguish an accepted `Show` request from a
visible notification. For an unpackaged normal release, trigger a configured state
transition and verify its entry in Windows Notification Center, then click it to
open Settings. Close the app and click a retained entry to verify cold activation.
Repeat after rebuilding at a different location and with the installed build.
Do not disturb may suppress banners; it must not be used to explain missing entries
without checking Notification Center. In Parallels, compare with another Windows
app and record whether the VM is in Coherence or desktop mode. API success or broker
history alone is insufficient evidence of visible delivery.

Windows CI covers sender/COM launch registration before permission and delivery,
native class-factory activation, and shortcut creation/repair with both shell identity
properties read back from disk. Actual shell rendering, retained entries,
and Parallels presentation require the manual checks above.

### Windows and Linux branch trial

Use `feat/night-mode-schedule` before opening a PR. The Branch desktop check
workflow builds and tests on Windows and Linux and uploads an unsigned debug
executable for each platform. Windows requires the WebView2 runtime; Linux needs
GTK3, WebKitGTK 4.1, and an AppIndicator implementation. Native notifications may
require an installed/packaged app identity, so the standalone executable does not
replace packaged-app notification validation.

Run `pnpm --dir ui install --frozen-lockfile` and
`pnpm dlx @tauri-apps/cli@2 dev` from a local checkout for a source trial.
Verify rectangular add/clear gestures, the checked tray schedule control, selected
speaker changes, wake/recovery and all four notification modes. Windows clock
format comes from the user's regional time format. Linux honors GNOME's explicit
clock format when available, otherwise LC_TIME; other desktop-specific overrides
remain dependent on that desktop's locale configuration. No AM/PM preview override
is enabled in branch builds.

### Night schedule notification dropdown sizing

The macOS Settings window opens at 740 by 760 points. Check Night schedule at
that default size for unnecessary vertical scrolling; horizontal resizing must
remain disabled. Browser coverage also checks the page with save feedback shown.

On macOS, open Settings → Night schedule and select On start, then On end.
After each immediate save, verify the complete selected text remains visible
beside the dropdown arrow. Leave and reopen Settings to check the restored value;
repeat at the minimum window width. The browser regression selects all four modes
through the production form, waits for the save rerender, and checks the sizing
label and rendered text width. It fails before the mount-order fix. Browser
coverage does not establish native WebView popup rendering; repeat this check in
the packaged macOS app before release.

## UI demo builds

Autostart isolation regression: the normal Rust suite builds normal and demo
mock applications through the shared startup identity configuration and checks
the package names consumed by the autostart plugin remain distinct. This test
fails with identifier-only isolation and passes with the demo package suffix.
It does not write real OS login entries. On Windows and Linux, enable Start at
login in the normal app, then enable and disable it in the demo. Confirm the
normal registry Run entry or autostart desktop file is unchanged and still
launches the normal executable after signing in again.

The Windows presentation browser test uses native select change events for
portable save/rerender coverage, retaining keyboard checks for switches and
sliders. Native dropdown popup keyboard behavior must be checked on Windows:
focus Follow, choose an output by keyboard, and verify it survives navigation.

Build without flags and confirm there is no demo label. Build with `--debug
--features ui-demo`, start without a speaker, and verify Settings opens with
Living Room (simulated). Check normal tray controls, speaker settings, local
output selection, volume/mute synchronization, diagnostics/export and reset.
Demo saved settings/logs are separate; no real Sonos devices are discovered or
contacted. Local audio and OS services operate normally.

Regression sequence for Windows Night Sound: start a fresh demo, open Night
schedule, select the current day's current half-hour cell and Save schedule.
With recurrence disabled, turn Night sound off on Speaker. Enable schedule and
return to Speaker: Night sound becomes on, its switch is disabled and the message
explains that scheduling must be disabled first. Try the tray Night sound action
as well. Disable the schedule: Night sound stays on but manual off works. Save
an empty schedule and enable: manual on/off is available outside active periods.
Repeat after restarting with an enabled active schedule, including opening Speaker
immediately before the first scheduler tick. Cross a half-hour start/end boundary
and verify on enforcement, one-time off at exit, and the selected notification mode.

With Loudness initially on, enable **Disable loudness during night schedule** and enter
an active block. Confirm Loudness turns off, Settings and tray reject manual enable,
and Night Mode remains scheduled. Exit the block and confirm Loudness returns on.
Repeat with Loudness initially off and confirm it remains off after exit. Restart
during the active block and confirm restoration still occurs afterward. Disable the
option and then the schedule during separate active runs; each must restore only a
bridge-owned change. Select another speaker while restoration ownership exists and
confirm the marker is not applied to that speaker. Simulate unsupported and
temporarily unavailable Loudness reads and confirm Night Mode scheduling continues.

Restart: demo app settings persist and the real runtime reconciles fresh simulated
speaker state. Repeat with `ui-windows`, `ui-macos`, and `ui-ubuntu`; native window
constraints and chrome remain host-specific. Check keyboard navigation and resizing
at default/minimum sizes and across display scaling settings.

Automated routing tests cover the delayed startup handshake and native commands
in both normal and demo builds (the old desktop mock routing fails these tests).
Native tests use actual SOAP/GENA clients and production scheduler/command entry
points to cover enabling, startup timing, enforcement, manual-off rejection,
disabling and outside-period control. They run in the ordinary CI test suite and
feature-enabled checks. Browser tests cover presentation only. Native WebView,
tray interaction, notification delivery, wake/time changes and physical hardware
still require target-OS checks; simulated firmware cannot establish hardware behavior.

Windows native tests require the Common Controls v6 manifest. The shell build
script supplies it to library test executables; application binaries retain the
full Tauri manifest. Without this, the test process fails before running tests.
Local Windows verification of the revised native demo: startup showed Connected
with the simulated speaker. Saving the current Saturday half-hour and enabling
scheduling made Night sound checked and disabled, with the schedule-lock message
and next transition visible. Left the active demo period enabled for UI testing.
Rust workspace/demo suites, formatting and Clippy passed; frontend unit tests,
lint/format/build and all 21 browser tests passed. Native notification delivery,
clock boundaries and restart persistence were not manually exercised in this pass.

## Schedule notification speaker names

With a speaker whose discovery name contains a Sonos Media Renderer suffix, enable
start/end notifications and save a schedule once inside and once outside its
current block. Confirm each notification uses only the human speaker name. Verify
scheduled start and end notifications likewise. Names with ordinary hyphens must
remain intact. Automated tests cover the shared production notification formatter
for all four paths and fail when raw discovery names are used. Native delivery
and banner layout still require a real notification-capable host.


## Ubuntu settings presentation

- Automated browser tests cover every page at 1080 by 800 and 1080 by 460 in
  light and dark modes, keyboard switch and slider changes, and persisted schedule
  selection. The rounded-cell regression was reproduced before the CSS fix.
- On native Ubuntu, start `ui-demo` from a closed app and compare with GNOME
  Settings: wide window, neutral sidebar, white rounded groups, row separators,
  trailing dropdowns, orange switches and white slider thumbs. Check text renders
  in Ubuntu/system sans-serif throughout, with no unexpected serif paragraphs.
- Confirm horizontal resizing is blocked. At the default height, open Night
  schedule and Save: the grid, actions and save notice must fit without scrolling.
  Resize vertically to the minimum; scroll to every setting and action. Switch pages, change a dropdown, drag a slider, and save/reopen a schedule.
  Schedule cells must stay rectangular before and after updates.
- Repeat in dark mode, with increased text scaling and keyboard-only input.
  Confirm select popups, native title bar and tray behavior in WebKitGTK.
- Browser coverage cannot establish native title-bar appearance or desktop theme
  integration. HTML controls follow Yaru styling but are not native GTK widgets.

Ubuntu window follow-up: verify width stays at 1080 while height can change.
At the 800-pixel default height, Night schedule must fit after Save without
scrolling. On Volume, the Test speaker volume button must be inset from the card
and usable with Enter. Automated browser regressions cover these cases.

## Ubuntu top-bar icon contrast

Start with Ubuntu in light application mode and the app closed. Launch the UI
demo: the top-bar glyph must be white immediately, including before connection
completes. Switch the application theme dark then light; the glyph must remain
white. In a normal build, disconnect and reconnect the selected speaker and check
the red disconnected badge appears and clears without changing glyph contrast.
The CI regression exercises the production image-selection path in this sequence
and failed before the fix. It cannot verify native AppIndicator rendering or
custom desktop panels; manually check against Ubuntu's default dark top bar.

## Linux release architectures

Release CI builds native AMD64 and ARM64 Debian packages and checks their
architecture metadata before upload. Automated release-script tests use real
Debian package fixtures and reject mismatched, missing, empty or ambiguous
installers. Alias tests require both Linux packages and check the website URLs.
After the first release, download each architecture from its website button,
check `dpkg-deb -f <package.deb> Architecture`, and install on the matching Ubuntu
machine. The ARM64 stable link is unavailable until that asset reaches a GA release.

## macOS release DMG

For direct-download signing, run the release once without the optional Developer
ID profile and once with it. Both must bundle the direct-distribution metadata,
sign successfully, pass notarization, and pass the existing artifact checks. The
profile case must also embed the profile and its application/team entitlements.
The CI distribution-packaging regression executes the production bundle command
with a recording Cargo adapter and verifies configuration, entitlement-file
availability and profile selection. It does not perform Apple signing or
notarization; those require the protected macOS release job.

After a GA release, download the DMG from the website. Verify its stapled ticket
and Gatekeeper assessment, open it, drag the app to Applications, eject the image,
and launch the installed app. Confirm Settings, tray and saved configuration work.
Repeat on a clean supported Mac to check quarantine handling. Release CI verifies
signing, notarization acceptance, stapling and image integrity; alias tests cannot
establish native installation or Apple notarization behavior.

## Reset during autosave

Change a General setting and immediately open Diagnostics and reset. Wait past
the autosave debounce, return to General, and confirm defaults remain restored
and no success notice appears. Repeat with a slow
configuration write. Browser regressions freeze the debounce clock or hold IPC
completion to exercise both sequences deterministically through the real UI.

## Linux Night schedule status consolidation

- Automated: the Linux browser suite opens Night schedule from startup, checks
  the third row below notifications, enables scheduling, saves, navigates away
  and returns. Status and error feedback must stay in that row, with no duplicate
  status below the card or confirmation below the editor. Successful saves remain
  silent; a failed save shows an error, cleared on a successful retry. The regression fails
  before the fix and passes after it; default-window fit is also checked.
- Manual: launch the normal Linux app, open Settings > Night schedule, enable
  scheduling, and save a block. Confirm the status updates in the third white row
  below notifications. Navigate away and back, then test an active period and
  an unavailable speaker. Confirm current status and errors remain in the row.
  Browser mocks cover layout and frontend orchestration, not native event delivery
  or real speaker transitions. Native checks remain to be performed.

## Wayland close button after first show

1. In an Ubuntu Wayland session, launch the normal app with Settings initially hidden.
2. Open Settings from the tray and click the native close button once, without
   resizing, maximizing, or double-clicking the title bar first.
3. Confirm Settings hides and synchronization and the tray continue running.
4. Reopen from the tray and repeat the immediate close at least three times.
5. Confirm title-bar dragging, minimize, maximize, and vertical resizing still work.

Before the dependency repair, close could ignore clicks until a resize; repeated
clicks could maximize instead. The lockfile regression fails on that dependency
and passes with the upgrade. The normal Rust suite exercises the shared close
handler through first show and reopen without resize. Native GTK/compositor input
is outside mock coverage; manual validation is pending.

## Linux schedule notification delivery

- Automated: the normal Rust suite starts a private D-Bus notification service
  and a subprocess using the production sender inside Tokio. Applied-on, applied-off,
  scheduled-start, and scheduled-end must all arrive. Before the fix all four were
  missing because the blocking plugin sender panicked inside the async runtime;
  the same regression passes with asynchronous delivery. Linux tests require
  `dbus-daemon`, installed by CI. The test never targets the desktop session bus.
- Manual: in the normal Ubuntu app, select On start and end. Save once with the
  schedule enabled and the current half-hour selected, and again with it cleared. Confirm a native banner
  only when the edit enters or leaves the enabled schedule. Repeat each save and confirm
  there is no additional notification. Then enable the
  schedule and verify an actual start and end boundary. Enabling during a selected
  period also notifies after speaker confirmation; disabling does not notify. Confirm Never suppresses both directions, On start only shows
  on notifications, and On end only shows off notifications.
- Coverage limit: the service test verifies delivery and message content, not
  GNOME banner rendering or desktop suppression. Native edit-triggered banner
  confirmation is recorded below.

The save-transition regression additionally exercises the shared application and
notification orchestration with all four preferences, recurring scheduling on
and off, and the sequence outside → enter → unchanged save → leave → unchanged
save. Only permitted entry/exit transitions with recurrence enabled reach the private
service after speaker confirmation. Disabled recurrence stays silent.

Also turn Night Mode on manually while outside the schedule, then save an edit
that includes the current half-hour. With On start selected, the entry notification
should appear even though the speaker was already on. Editing another day while
remaining inside the current period should not notify. Save with recurrence off
and confirm no notification, even if the one-shot apply changes Night Mode.

### GNOME notification source lifetime

The private-service regression also checks that repeated notifications use the
same sender and that it still owns its bus name after the production sender
returns. The previous per-send connection was closed immediately after Ubuntu
accepted delivery, causing GNOME to remove the app notification source. This
regression fails before persistent-connection delivery and passes afterward.

Manual reproduction: keep Settings open, enable the schedule and On start and
end notifications. Clear the current half-hour block and save, then select it
and save. Confirm both banners appear and remain available in Ubuntu's
notification list after the send completes. Repeat without closing Settings.
The mock service checks connection lifetime; actual GNOME presentation still
requires this native verification.

Native verification (2026-09-27): the user confirmed that both Ubuntu banners
appeared after clearing the current half-hour and saving, then selecting it and
saving again with Settings open. This verifies edit-triggered exit and entry
presentation; timed boundaries and other desktop sessions remain separate checks.

### Enabling during an active period

Automated Linux regression calls the production enable command before the worker's
first tick with a simulated speaker and private notification service. It covers
all four notification preferences, selected and unselected current-time periods,
repeated enable, and disable. It verifies speaker confirmation and exactly two
start notifications (Start and Both). The regression receives zero notifications
before the fix and passes afterward. Existing worker tests cover reconciliation.

Native check: with scheduling disabled, select/save the current half-hour, then
manually turn Night Mode off. Choose On start or On start and end and enable the
schedule from Settings; confirm Night Mode turns on and one Ubuntu banner appears.
Repeat using the tray. Repeat outside selected periods and with On end/Never;
expect no banner. Disable scheduling and confirm Night Mode stays unchanged with
no end banner. The private service cannot prove GNOME banner presentation or
desktop suppression settings.

Native verification (2026-09-27): after the enable-notification fix, the user
reported that everything was working. This confirms the reported active-period
enable notification in the normal Ubuntu app. The report does not separately
verify every preference, tray interaction, timed boundary, or first-open close
sequence listed above.

## Windows Night schedule status and notifications

Open the normal Windows app, navigate to Night schedule, enable scheduling, edit
blocks and save. The third card must contain current state, next change when
available, notification guidance and error feedback. Successful saves must show no
confirmation. The Status label must be on the left with wrapping text on the right
in the 960-by-820 default Windows window. There must be no second
status line beneath the card or notice beneath Save. Disable and re-enable the
schedule, navigate to General, save a setting and return; errors must stay visible
in the appropriate page while successful actions stay silent. Check light/dark and
minimum/default window sizes.

On a fresh Windows user profile, launch with `cargo tauri dev` without demo features.
Choose a compatible speaker, select On start and end, and save a block covering
the current time. Confirm a Speaker Volume Bridge notification, then save an empty
grid and confirm the off notification. Enable a schedule spanning the next
half-hour boundary and verify a single start/end notification at matching
boundaries. Repeat with On start, On end and Never to check filtering. Repeat
with an installed desktop build and a packaged build. With app notifications
disabled, confirm the status guidance and that scheduling still works; restore
notifications and confirm recovery. Do not disturb may send notifications directly
to Notification Center instead of showing a banner.

CI browser coverage uses the production form and mocked IPC to check initial
state, toggling, grid editing, saving, next transition, notification guidance,
failure feedback and navigation. The layout regression fails before consolidation.
Error-only notice regressions on all three platforms fail with the old save
confirmation. They wait for the production form to finish saving, then exercise
a failed save and successful retry, preserving visible errors and clearing them
after recovery. Settings autosaves are also verified to remain silent.
Rust CI exercises shared Windows notification orchestration from an unregistered
sender through the initial permission check and Save/start/end delivery, and
preserves denied/unavailable outcomes. Removing first-use preregistration fails
that regression. Existing integration tests cover schedule boundary/filter policy.

The opt-in `cargo test -p speaker-volume-bridge native_development_notification_smoke
-- --ignored --nocapture` sends a real Windows test notification and checks native
API success. It passed locally after the fix. Automated success does not prove a
visible banner, native WebView layout, hardware timing or packaged delivery;
complete the manual sequences above before release.

### Windows installer and architecture coverage

CI builds a normal NSIS installer on Windows x64 and ARM64. Packaging the custom
template with the updated Tauri bundler failed with a missing Restart Manager
macro before its include was added, and succeeded after the fix. Keep this
production packaging check in PR CI. The architecture script checks both valid
PE machine types, rejects malformed files and invokes the MSIX packaging entry
point with mismatched input to verify it fails before packaging. Local validation
on ARM64 covers actual MSIX pack/unpack; x64 packaging is verified on its CI runner.

Install the matching normal NSIS package, launch it from its installed location,
then set notifications to On start and end. Save a change that excludes the current
half-hour, and another that includes it. Confirm both speaker state changes and
that Night Mode schedule ended/started appear in Windows Notification Center.
Save again without crossing a boundary and confirm no additional notification.
Restore the original grid. Automated delivery acceptance cannot prove that the
Windows shell displayed a notification; record visible delivery separately.

Windows unpackaged sender registration includes a persistent copy of the bundled
PNG icon through `IconUri`, before the notifier is created. A regression test
asserts the display assets exist at sender creation time. On the locally installed
ARM64 NSIS build, the user confirmed notifications appear; verify the branded
icon on a new notification after updating (old notifications may keep cached art).

When launched by a packaged development host, also verify that the registered
PNG and Start menu ICO paths resolve to the actual files outside that host's
file-system redirection. The shortcut regression checks persisted icon location,
sender and activation metadata through COM after initial creation and repair.
Reinstall in place, check the Start menu icon and a fresh toast, and confirm no
desktop shortcut was created. Desktop shortcut creation remains disabled for
interactive, passive and silent NSIS installation.
The legacy WiX migration exception that could recreate a desktop shortcut was
removed as well; only Start menu shortcuts are created or repaired.

### Installing from a packaged development host

Launching NSIS directly from a packaged terminal/agent can redirect its AppData
files and uninstall registry writes into that host's private environment. Successful
installer exit and a registry read from the same host do not prove the app is
visible in Windows Installed apps. Run the installer through the existing Windows
Explorer desktop instead, preserving configuration when moving between redirected
and normal locations. Check the uninstall DisplayName through the independent
Windows registry provider, then reopen Settings > Apps > Installed apps and search
for Speaker Volume Bridge. Confirm its uninstall action is available and that no
desktop shortcut is created. This is an installation-context verification step;
the NSIS template already writes the normal uninstall metadata.

### Shortcut ownership and notification cleanup

The native shortcut regression creates a development shortcut, starts with an
installed target, then simulates another development startup and reads the actual
persisted shell link. The installed target must win both times. Windows CI also
compiles and executes the production NSIS registry-cleanup block against isolated
test keys: full uninstall removes sender and activator; update mode retains both.
The release test suite checks the cleanup remains inside the full-uninstall guard.

Manual coverage: run a normal standalone build before installing, install and
launch the installed app, run the standalone build again, and check Start still
launches the installed executable. Uninstall using Windows Installed apps and
verify both native notification registrations disappear without manual cleanup.
Repeat an in-place update and verify notification delivery remains available.
Automated tests isolate installation lookup from real machine registry state and
do not prove native shell cache refresh or elevation/account behavior.

## Manual rebrand cleanup and normal startup

Follow the [old-app removal guide](removing-old-app.md) on each supported desktop.
Check startup-entry removal, preserved preferences during an ordinary upgrade,
and optional preference deletion during a clean reinstall. Launch the renamed
app and verify synchronization starts without a process-inspection step. Navigate
Settings, save preferences, use manual speaker controls and apply a Night schedule;
no legacy warning or diagnostics field should remain. Existing Night Mode locks
and explicit Stop behavior must still work. Browser regressions cover Settings
navigation and saves, while the loopback speaker test exercises production manual
commands and the scheduler. Native installed startup and cleanup remain separate
manual checks.

## Renamed Windows installation directory

On x64 and ARM64, install an older direct-download build into its default folder,
set preferences and enable startup, then quit it and run the new installer.
The directory page must propose Speaker Volume Bridge. Verify files move, settings
persist, one installed-app entry remains, shortcuts and startup launch the new
executable, existing notification activation works before/after first launch, and
uninstall removes the new installation. Repeat with startup disabled, a custom
installation directory (retained), the already-renamed executable in the old
folder, and a clean install. Cancel on the directory page and verify no move.
With an occupied destination or a locked source, installation must abort and
preserve the source. The NSIS harness executes production location-selection and
move functions on both Windows CI architectures, including collision and locked
source cases. Native full-installer, login and toast checks remain manual gates;
macOS-hosted tests do not establish Windows behavior.

## Installed distribution provenance

Automated resolver tests cover each supported edition, missing and conflicting
evidence, a reused executable in direct and Store packages, x64 application on an
ARM64 host, development/demo builds, sideloaded MSIX, and resolver failure during
application startup. Packaging regressions assert every bundle receives its own
provenance and that Store MSIX staging replaces direct provenance.

Native acceptance remains required before enabling live catalog entries. On each
supported architecture, install the direct package and the applicable Store test
package, then verify the startup log reports the expected edition and compiled
application architecture. Confirm a development build, copied/repackaged Debian
binary, test-signed or sideloaded MSIX, App Store-style sandbox without a receipt,
and package with removed/conflicting metadata report development, sideloaded, or
unknown rather than an official edition. These installed-package checks are not
proved by unit tests or bundle inspection and were not performed for phase 1.2 on
the macOS development host.

## Background update checking

Automated fake transport/clock/persistence and shared-orchestration tests cover
available/current/missing, malformed, prerelease and additive-metadata catalogs,
required fields, unsupported actions, exact target selection, invalid action
targets, persisted offers and success time across restart, withdrawal of a cached
offer, the 24-hour interval, and concurrent manual/background calls sharing one
request. Wake orchestration verifies UI delivery without page navigation before
notification activation. The production scheduler starts once after speaker
synchronization, reevaluates due state every minute, uses bounded retry, and aborts
on drop. Native sleep/wake timing, proxy/redirect
behavior and shutdown cancellation remain installed-app checks on macOS, Windows
x64/ARM64 and Linux x64/ARM64; they were not performed during phase 1.3.

Settings and orchestration tests cover all visible update states, the persisted
notification switch and deduplication, denied-permission rollback, Later, manual
rediscovery of a prior offer, stale and invalid activation, concurrent open
requests, and native-opener invocation for the explicit HTTPS repository link.

### Native update acceptance (manual; required on every supported OS/architecture)

Use a signed, installed package for the target distribution and architecture, plus
an approved non-production catalog fixture that contains a valid `open_url` offer.
Do not enable or publish a live catalog entry for this verification. Record the OS
version, architecture, package edition, and fixture revision with the results.

1. Install and launch the package from the OS app launcher. Confirm the installed
   edition and architecture are reported correctly. Open **General** in Settings,
   then open **Updates** and enable automatic checks and update notifications.
2. Start with a fresh isolated test profile, return to **General**, and allow the
   startup check to run after its 30-second delay. Confirm Settings stays on
   **General**, the last-success time updates, and exactly one update notification
   is delivered for the offer. Separately verify the 24-hour scheduled interval;
   use an isolated profile whose last-success time is over 24 hours old if the
   acceptance harness can seed persisted state, otherwise wait for the interval.
3. With the check due, put the machine to sleep, then wake it. Confirm the UI
   check status updates and at most one notification is delivered for that check.
   Repeat with notifications disabled and confirm the status still updates with
   no notification.
4. Activate the update notification. Confirm it opens **Updates** and the cached
   offer is visible. Return to **General**, then activate the tray's Updates
   action and confirm it opens **Updates**. Trigger a schedule notification and
   activate it; confirm it opens the schedule page rather than **Updates**.
5. With the offer cached, quit and relaunch the app. Confirm the offer and last
   successful check are restored. Make the fixture unavailable and check again:
   the offer remains discoverable, is marked stale when past its freshness window,
   and cannot launch its action while stale. Restore the fixture with no offer,
   check again, and confirm the withdrawn offer is removed.
6. With a fresh valid offer available, click its `open_url` action and confirm the
   system's default browser opens the expected HTTPS page. Deny notification
   permission in OS settings, attempt to enable notifications in the app, and
   confirm the preference rolls back with an error. Restore permission afterward.

The automated suite covers shared orchestration, notification identity/deduplication,
platform action metadata and the Windows activation callback. It cannot verify
actual OS notification delivery, notification-center activation, real sleep/wake
timing, installed-package classification, or LaunchServices/default-browser
behavior. These checks remain outstanding on macOS, Windows x64/ARM64, and Linux
x64/ARM64; source-level macOS tests do not establish Windows or Linux behavior.

## Update catalog publication

Shared additive-metadata fixture regressions exercise actual insertion, version
update and withdrawal through `prepare_catalog`. All three fail before metadata
preservation and pass afterward. They compare the complete document, including
retained entry/action metadata, and verify byte-identical repeats with a later
timestamp. These run in the existing CI and website validation publication suite.
This tooling coverage does not establish native installed-package acceptance or
served catalog deployment.

Catalog publication tests reject drafts, prereleases, missing assets, unverified
or mismatched Store availability, invalid documents, stale concurrent inputs,
same-version changes and downgrades. They also cover partial publication,
idempotent repeats and target-specific withdrawal. The normal CI path filters run
these tests. ADR 0020 now composes approved develop catalogs with retained GA
site bytes independently, comparing actual served content and cache refresh.

The controlled application fixture verifies that an older direct macOS build
selects its exact edition/architecture entry, retains only the validated HTTPS
page, and claims that action without any install path. UI automation verifies the
same page action remains available after Later. This does not prove a real
storefront listing, installed-package classification, native default-browser
activation, or a Pages deployment. No release was published and no `main` site
deployment was triggered during phase 1.5; those checks remain separate release
acceptance gates.

## Release policy prerequisite (#178)

Automated: consumer, validator and actual publication mutations share
`release-policy.json`. Coverage includes numeric GA/Beta/Alpha classification,
newer GA selection, metadata retention, stable-feed exclusion, publisher evidence,
compatible assets, semantic ordering, capability/backend matrix, legacy defaults,
restart, edition normalization, missing/empty feed, architecture isolation and
explicit checks with automatic checks off. Production `run_check_and_deliver_with`
regressions gate a response across rapid Stable/Prereleases switches and gate
notification permission across a switch. The obsolete-response test fails when
its commit guard is removed and passes with the guard restored. Browser tests
exercise selection, copy, progress/error/retry and unsupported-edition absence.

Native acceptance remains unverified: local browser preview uses simulated IPC
and does not prove signed package provenance, OS notifications or default browser
opening. On installed direct macOS, Windows x64/ARM64 and official Debian x64/ARM64:
start with Stable, disable automatic checks, select Prereleases, observe one check,
restart and confirm selection persists. With a delayed controlled feed, switch
Stable/Prereleases rapidly and confirm obsolete offers never return. Activate an
old notification and confirm it opens current Updates state; an old queued link
must fail. Open the available release page and verify the exact release and native
browser. Return to Stable on a newer preview and confirm no downgrade. Verify
Store and sideloaded packages omit/reject policy selection; repeat after distribution
change with preserved preferences. Keep speaker synchronization active throughout.
Signed packages and those other operating systems are unavailable in this local
run; do not infer native acceptance from browser or orchestration tests. No catalog
entry, deployment, release, download or installation is authorized by this work.

Dark dropdown follow-up: the supplied dark-mode popup used inherited light text on
an unstyled light native menu surface. Shared option styling now supplies both
foreground and background from the same palette. Browser regression reproduces
transparent option backgrounds before the fix, then verifies opaque backgrounds
and at least 4.5:1 text contrast for Devices, Night schedule and Updates. Native
platform popup rendering still needs installed-package verification.

## Independent release catalogs (issue #183)

`test_catalog_delivery.py` runs in normal release automation and website CI with
filters covering the production scripts and both workflows. Shared production
orchestration exercises GA to both feeds, Alpha/Beta only v2, complete public
assets, payload version/architecture/provenance, draft/publication failures,
Store preservation, additive metadata, withdrawals, idempotency, out-of-order
classification events and latest-develop retry/reused normal-push proposals.
The actual Debian reader runs against a locally constructed package in Linux CI.
Actual macOS DMG and Windows executable readers require platform tools; both were
also exercised locally against the public packages used for the review backfill.

A real local-Git regression opens separate GA/Beta proposals, simulates an approved
concurrent merge, and reruns the original production proposer. It checks retained
review ancestry, normal pushes and both classifications in the refreshed PR.

The unchanged release-success/develop-delivery/CI-wiring regressions fail three
checks against the prior develop implementation and pass with the new workflows.
Snapshot composition, integrity, pending-candidate restore blocking and complete
artifact preservation run through production Python functions and CLI. Served
verification tests inject ordinary/refresh HTTPS responses: valid-but-wrong feed
content, stale cache, absent cache policy and network failure fail verification.
`verified_backfill_uses_production_client_for_version_policy_and_target_selection`
uses the real Rust UpdateService to consume the checked-in unpublished backfill:
equal versions are up to date; older compatible versions get the correct action;
wrong macOS architecture and absent Store entries remain unavailable; preview-only
Beta selection and newer GA selection obey semantic precedence. Fixtures are
review inputs outside the served catalog directories, not live publication.

Required local checks: Rust formatting, workspace clippy with warnings denied,
workspace tests, all Python release/catalog/website tests, workflow validation,
website Markdown/sitemap generation checks, consent/external-link tests and a
local rendered GA website build. Runtime loops, mocked transports and source
workflow checks do not execute GitHub Actions/App authorization, Pages environment
approval, actual protected merge, CDN rollout or native installed-client checks.

Live acceptance, after separate authorization and prerequisites:

1. Adopt the actual served GA artifact with Website bootstrap; compare every file,
   including generated metadata and Markdown guide counterparts. Retain/backup the
   immutable hosting snapshot. Confirm bootstrap deploys nothing.
2. Reverify the backfill with Propose update catalog. Confirm only the applicable
   catalog files change, required PR CI really runs for the App-created PR, and
   required human approval remains pending. No live entries before that gate.
3. After approval and merge, verify a develop catalog-only Pages deployment without
   another app release or main promotion. Compare all non-catalog hashes and the
   displayed GA version with the retained snapshot. Fetch both existing client
   URLs normally and with refresh requested; require exact expected bytes.
4. Exercise GA and Alpha/Beta public completion plus explicit recovery, duplicate
   retries, older events, independently merged pending targets and a withdrawal.
   Verify no Store invention, unsupported architecture, version rollback or replay
   restoration. Keep tombstones through refreshed proposals.
5. Interleave an approved main GA website deployment with preview catalog merges;
   rerun a superseded event. Verify newest approved catalog bytes and intended GA
   page bytes converge. Activate the same composer on main before this check.
6. Remove access to a retained artifact in a controlled environment, inject stale
   cache and a failed deploy, and check explicit pending/retry outcomes. For an
   ambiguous GA deployment, verify-candidate or GA recovery must resolve state
   before catalog-only delivery. Do not infer automatic rollback after Pages has
   accepted a new artifact.
7. On installed macOS ARM64, Windows x64/ARM64 and Debian x64/ARM64, use controlled
   equal-version and older-version packages: check up-to-date, update offer, exact
   architecture/edition, policy separation, action page and retry after delivery.
   Native notifications and installed provenance remain separate acceptance.

Hosting/App/main-workflow activation and live/native acceptance have not been
performed by this implementation. See [operator prerequisites and recovery](catalog-operations.md).

### Missing catalog credentials after public publication

The regression executes the production workflow preflight shell with both
credentials absent, each independently absent and both present. It checks the
configured output, catalog-pending summary, secret redaction and downstream
proposal gate. It fails against the required-secret/no-preflight workflow and
passes after the fix. Normal CI runs it with the catalog orchestration suite.
Live acceptance requires an approved workflow update, an already public release,
and a missing-credential invocation: verify publication remains successful, the
warning identifies pending catalog preparation, and no proposal runner starts.
Then configure the App and verify normal proposal CI and approval. This live
acceptance and credential provisioning have not been performed locally.

### Workflow-only main activation regression

`test_workflow_only_promotion_preserves_ga_source_for_push_and_schedule` invokes
the production composition CLI on a real repository. A workflow-only commit
selects retained catalog composition; a subsequent GA content commit selects a
GA rebuild. Both push and schedule call that source comparison in the workflow.
The previous SHA-only comparison fails the regression; the corrected source
comparison passes. Missing published-source evidence fails closed.

An exact reconstruction from the successful GA source, original generated
metadata and pinned image produced a complete 66-file inventory. Normal and
cache-refresh comparisons matched 40 files, including both catalogs; 26 HTML
files differed because the hosting edge injected a script loader and rewrote
script tags. No snapshot was adopted or deployed. Resolve that approved hosting
prerequisite and repeat all-file byte verification before bootstrap acceptance.

Main PRs are excluded by the ordinary release-validation workflows. The targeted
catalog promotion check now verifies workflow-only handoffs against approved
develop definitions and runs that branch's production catalog regression suite.
The draft promotion is expected to wait for its develop dependency; local lint
alone is insufficient to approve the handoff.

Workflow review regressions execute the actual shell steps. One creates a real
published GA source, an approved but unserved GA change, and a workflow-only push:
the reconciliation step must select GA recovery using the published reference.
The other submits an extra workflow to the promotion allowlist guard and requires
rejection. Reverting either fix fails its regression. Neither test claims a live
Pages deployment or supplies administrator authorization.

The promotion classifier also covers both recognized workflow extensions. A
production-shell regression adds only an unreviewed `.yaml` workflow and verifies
that classification routes it to the approval guard, which rejects it. The prior
`.yml`-only classifier fails this test. The trigger watches the whole workflow
directory, and NUL-delimited Git paths prevent quoted filenames from evading
classification or the allowlist.

Mixed-content main PRs also validate every changed workflow definition and run
the approved catalog suite. The production-shell regression combines a workflow
change with an unrelated documentation change: the prior gate fails the test by
skipping approval checks; the fix rejects unapproved definitions and permits the
path only after they match approved develop. Changed workflow paths are compared
literally, and the focused workflow-only handoff retains its stricter allowlist.

Trusted promotion regression: a PR replaces the validator and deployment workflow
while adding unrelated content. The previous validator accepts it; the base-side
validator rejects it without checking out or executing PR code. Both outcomes
were reproduced locally. The reviewed validator is merged on the default branch,
develop, and its trusted check has passed on the main activation PR. A separate
main gate bootstrap is unnecessary. Activation prerequisites remain outstanding;
a passing gate does not establish deployment or native acceptance.

## Phase-one operational readiness audit

The operational acceptance issues remain open until their applicable criteria
are verified. Implemented behavior and passing CI do not establish public catalog
availability or native acceptance. No self-update implementation or new application
release is part of this audit.

| Work package | Implemented / automated evidence | Outstanding acceptance |
| --- | --- | --- |
| #160 catalog contract | Typed contract, validator, fixtures, additive compatibility, site artifact inclusion verified | Satisfied; live publication is tracked separately |
| #161 distribution resolver | Startup resolver and package provenance tests | Installed direct/Store packages and unavailable architectures |
| #162 background checks | Service tests and production manager scheduler regression; normal PR CI passed and scheduler fix merged | Native startup/restart checks |
| #163 notifications and controls | UI and shared orchestration tests | Native tray, notification activation, persistence and exact page opening |
| #164 publication integration | Verified-package proposal tooling and controlled consumers | Approved deployment, cache/content checks and installed-package acceptance |
| #178 channel selection | Backend policy, persistence and controlled catalog tests | Installed channel selection, restart and newer-GA/preview behavior |
| #183 independent delivery | Production proposal/composition and workflow coverage | App authorization, Pages permission, approved main workflow handoff, exact GA snapshot adoption, backfill approval and served verification |

`production_scheduler_delays_retries_once_and_cancels_on_shutdown` enters the
production UpdateManager startup method with only its timer injected. It uses the
real service, fake transport and clock, and native mock application. It checks no
request before the 30-second boundary, one worker after repeated start, waits of
60/300/1800 seconds after transient failures, return to the ordinary poll after
the bounded retry sequence, and no request after dropping the manager during a
wait. Four separate deliberately faulty variants (zero startup delay, shortened
retry, removed duplicate-start guard, removed shutdown abort) each fail this
regression; the restored implementation passes.

`production_scheduler_shutdown_cancels_in_flight_transport` blocks the actual
transport request and verifies that dropping the manager cancels that future
without committing a successful check. Both tests run in the normal workspace
suite. Timer injection changes no user timing or network policy.

Native follow-up: fully quit and restart an installed recognized package; observe
no automatic request before 30 seconds, switch Stable/Prereleases, restart and
confirm persistence, then use a reviewed controlled offer to test notification
deduplication and explicit exact-page opening. Repeat on every supported platform
and package edition. Mock application tests cannot prove OS presentation or
installed provenance. During this audit the installed macOS package metadata
reported version 1.8.0; the Computer Use service timed out on app inspection, so
version presentation, policy selection, restart persistence, notification delivery
and native page opening remain unverified. Windows, Linux and Store checks remain
outstanding without those installations.

Current activation observations: proposal App configuration is absent; Pages
permits only the GA branch; no durable hosting-state branch exists; the successful
GA workflow has no retained Pages artifact. Exact reconstruction/adoption is
required before catalog-only delivery. Bootstrap must compare all served bytes
and deploy nothing. Do not substitute current develop site output.

The normal Linux CI suite exposed a ready-timer shutdown race: task abort alone
is cooperative, and a timer already being polled can continue into a new check.
`production_scheduler_shutdown_ready_timer_cannot_begin_check` deterministically
holds the production worker inside that poll, drops its manager, then releases
the ready timer. The original manager starts one request and fails; the fix
publishes a shutdown flag before abort and checks it at scheduling boundaries,
so no request begins. The test then waits for worker destruction before asserting
the transport count. Repeat native quit at startup/retry boundaries when native
inspection is available; CI does not establish OS shutdown presentation.

Main gate regression: branch from main, approve a workflow change on develop,
then advance main with a nonconflicting workflow edit. The prior scope step
accepts the stale head; the fixed production step rejects it and stale event
bases. A fresh content-only PR passes classification with no workflow checks.
The trigger has no path filter so ordinary GA proposals complete the required
check. The live main-targeting gate uses the approved default-branch definition;
its passing check does not establish catalog activation.

Pinned divergent release regression: the release head and current main share
source but diverge because of prior promotion history. The prospective merge
tree equals the published release tree. The prior unconditional ancestry gate
rejects that supported production route; the fixed production scope step accepts
it without changing the pinned head. Workflow-changing divergent heads and stale
event bases remain rejected by the existing regression.

The base-advance regression now first obtains successful validation, advances
main without changing the proposed head, and proves that fresh validation rejects
the previously accepted workflow tree. The live acceptance additionally requires
strict up-to-date required checks: verify the old success cannot authorize merge
after main advances, update the handoff, and require fresh passing validation.
That administrator protection and compatibility with pinned release promotions
remain outstanding; this PR does not configure branch policy.

Workflow move regression executes the production scope and approval steps after
moving an unchanged workflow outside its directory. The previous classifier
misses the removal; the fixed inventory identifies it and rejects the unapproved
deletion. Rename detection is disabled so both sides participate in validation.

## Anonymous GitHub Releases redesign

The production-request and shared-service regressions fail against the former raw
catalog implementation and pass with the release API. New coverage exercises both
policies, unordered numeric previews, exact release-page actions, actual HTTP
pagination, saved policy/offer restart, incomplete/foreign/draft metadata and rate
limits. Existing scheduler lifecycle regressions remain in the normal suite.
Package naming establishes metadata compatibility, not inspected package bytes.
New source migration invalidates former catalog cache freshness. Native dropdown,
version detection, notification activation and exact browser opening remain
outstanding, as do unavailable Windows/Linux and Store platform checks. The code
change is not an application release or installed-client verification. Legacy
catalog delivery/redirects are no longer acceptance requirements for this rollout.

### Release input normalization

Automated workflow regressions verify input ordering/defaults, stable-only Store
gates, optional direct macOS signing, publication dependency outcomes, and Apple
validation before upload with private-key cleanup on failure. The input/gate
regression fails against the former workflow and passes with this change. Apple
commands are stubbed; actual signing, unsigned DMG installation, App Store Connect
upload, Store review and public availability remain native/operator checks.
No application release is dispatched by these tests.
