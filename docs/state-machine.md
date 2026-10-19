# Synchronization state machine

`Connecting` waits for a Sonos read. With two-way synchronization enabled,
its first confirmed state is applied to the local system. With two-way
synchronization disabled, Sonos observations update connection state without
changing local audio, and the initial local state is sent to Sonos.
`Synchronized` has a current Sonos-confirmed state.
`WaitingForSonosConfirmation` retains only the newest local desired volume.
`SubscriptionDegraded` and `PollingFallback` represent unavailable or stale
event delivery. `SonosUnavailable`, `LocalAudioUnavailable`, and
`UnsupportedLocalDevice` are recoverable runtime states. A restored connection
returns to `Connecting` and reconciles from Sonos.

An application-originated local callback is suppressed. Windows identifies it
through a stable Core Audio event-context GUID; macOS compares it with a
short-lived expected write using an adapter-configured tolerance; Ubuntu uses
the PulseAudio-compatible `pactl` event stream with short-lived expected-write
tracking. A Sonos
confirmation accepted by the coordinator clears pending intents. During a local
gesture the coordinator defers speaker corrections until a fresh read after
500 ms of settling. Local authority stays latched until that read returns, even
if a notification branch runs first at the deadline. Accepted confirmations apply
locally only in two-way mode.

Volume changes are debounced and coalesced; mute changes bypass the debounce.
The coordinator does not resend a volume or mute value until it differs from
the last sent value. When GENA delivery is healthy it polls slowly for health;
on subscription loss it polls once per second until event delivery recovers.

Volume changes use a fixed 45 ms dispatch deadline while callbacks continue to
arrive; only the newest pending volume is dispatched. Mute changes bypass the
debounce and supersede queued volume. A final speaker read runs after settling
even when fallback polling is disabled. Rejected speaker observations do not
change the waiting UI snapshot. They still invalidate mismatched cached commands.
All speaker reads share local-input guards and notification read windows.
Notifications from before a read are fenced. Notifications overlapping it remain
invalidations for another read, rather than being assumed stale or authoritative.
Newer local input cannot be replaced by the older read result. Notifications are
ordered by subscription-scoped sender sequence when available, otherwise by
acceptance revision; delayed handlers cannot reverse that order. After settling,
status remains degraded or polling fallback when the subscription is unavailable.
See ADR 0024. Duplicate commands are coalesced within a gesture; a new gesture
reasserts state because the speaker may have changed externally in between.

Night Mode scheduling is an independent optional controller. Disabled or unselected
means no recurring effects. Explicit Save remains a one-shot apply operation even
while recurring scheduling is disabled. Unsupported/unavailable means paused with configuration retained.
Reconciliation establishes the expected current value without a notification.
Scheduled entry enforces on; scheduled exit applies off once and permits subsequent
manual changes. Transient failures retry with bounded backoff; confirmed normal
boundaries produce notification intent. Applying a saved schedule produces
notification intent only when the edit changes whether the current time is inside
the enabled schedule, after confirming the resulting speaker state. Saves that
remain inside/outside and saves with recurrence disabled stay silent. On start/On end/On start and end/Never filters
that intent without changing controller state. Selection or schedule changes discard old
pending intent. Notification preferences never reset this controller. See ADR 0013.

The optional night schedule Loudness policy runs beside the Night Mode controller.
Active plus opted-in enforces Loudness off. The bridge persists speaker-scoped
ownership before changing an on value, retains it across restart, and restores on
exit or policy disablement. An already-off value creates no restoration ownership.
Unavailable or unsupported Loudness retries independently without changing the
Night Mode state or notification intent. A restoration marker for one speaker is
never applied to a different selected speaker.

Enabling a previously disabled schedule during a selected period applies Night Mode
immediately and sends a start notification after speaker confirmation when On start or
On start and end is selected. Enabling outside selected periods, enabling an already
enabled schedule, and disabling scheduling do not notify. Disabling leaves the speaker
state unchanged. The worker subsequently reconciles silently to avoid duplicate
notifications.

Opt-in UI demo builds run this state machine and the normal Night Mode scheduler
against a loopback simulated Sonos speaker. Local audio remains native. See ADR 0014.

Audio echo suppression retains repeated and overlapping expected local states for
500 ms without pausing listening. Unchanged local properties are not rewritten.
See [ADR 0017](decisions/0017-volume-feedback-suppression.md) for platform behavior,
regression coverage, and the limitations of value-based origin detection.

## Update checks

Download update is an explicit browser action on a fresh update_available offer,
not an installing state. It shares the release-page generation/freshness gate and
opening guard. Policy changes clear eligibility immediately. Later dismisses the
in-app notice without removing either action. Store-managed and unsupported
editions cannot claim a direct download.

The update state is independent of synchronization: idle, checking, up_to_date,
update_available, unavailable or unsupported. Only complete public release
metadata with a compatible official asset can produce an offer. Stable only
excludes prereleases; Include prereleases includes GA and previews. Version
precedence rejects equal/older versions. Missing candidates and failed API requests
remain unavailable. A policy change clears old actions and increments generation;
only matching responses can commit or notify. Source migration invalidates former
catalog offers/freshness. Shutdown cancels the worker and pending requests;
duplicate starts do not create another checker. See ADR 0021.

Automatic triggers remain gated during the startup delay and after shutdown.
Target-scoped failure state survives restart and enforces three retries before
daily recovery checks. Failed attempt persistence transitions Checking to
Unavailable without issuing a request; an explicit retry can recover. Cached
actions for a different release invalidate the offer and freshness under both
policies. Unsupported application architectures cannot enter direct discovery.

## Upgrading from the former app

Users quit and remove the former app before using the renamed app. The shell
starts synchronization directly and does not inspect other processes, gate writes
on a legacy-app check, or display a conflict warning. Settings and Night Mode keep
their normal selection/write serialization and explicit stop behavior. See
[decision 0018](decisions/0018-rebrand-and-legacy-protection.md) and the
[manual removal guide](removing-old-app.md).

Store editions have a separate `store_managed` update state. The shell rejects
GitHub discovery for these editions at the shared service boundary, disables
automatic checks even for migrated enabled preferences, and exposes only fixed
native Store destinations through an edition-authorized command. No release
offer or Store availability claim is created. Direct release-policy behavior is
unchanged. Existing catalog clients require a one-time manual upgrade; see
[upgrade guidance](rebrand-upgrade.md).

Direct macOS updater preparation is a packaging-only boundary. Verified payloads
remain validation artifacts retained in CI and never create installation actions. The
GitHub release-page fallback, version/policy guards and Store delegation remain
unchanged. Native sandboxed replacement acceptance is pending; see
[ADR 0023](decisions/0023-macos-updater-artifacts.md).

Healthy unchanged macOS outputs retain their listeners on unrelated topology
notifications. Successful output changes reconcile within the existing speaker
session, without connection backoff. Pending input from the former output is
discarded. Only actual attachment/read failures trigger recovery. See ADR 0024.

## Camera Speech Enhancement policy

This policy is independent of volume synchronization. It begins waiting, debounces
aggregate active for one second, and reads before claiming an off-to-on change.
Owned state restores after three seconds continuously inactive and a fresh matching
read. A pre-existing on value creates no ownership. A manual action or observed
external change pauses activation until a complete inactive interval and new use.
Unknown/detection failure and observation loss relinquish ownership immediately.
Disable, speaker switch, reset, and orderly exit attempt bounded guarded cleanup;
unconfirmed cleanup records a user-visible warning and is not retried later against
another session. Startup after a crash has no ownership to restore.

Native macOS and Windows wake callbacks invalidate ownership before the next
observation, including short sleeps without a Tauri Resumed event. See ADR 0026.
