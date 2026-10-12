# ADR 0024: Coalesce local gestures before accepting speaker corrections

**Status:** Accepted

## Decision

The coordinator retains one newest pending local volume and a fixed 45 ms dispatch
deadline. The native runtime keeps receiving callbacks during that interval rather
than awaiting dispatch inside each callback. Buffered callbacks from a network
write are collapsed to the newest user state; application echoes cannot displace
it. Device lifecycle events still stop the session. Duplicate states do not extend
debounce or settling. Mute changes bypass debounce and replace queued volume.

While a local volume is pending and for 500 ms after the latest local change or
completed write, speaker events and health reads cannot overwrite local audio or
clear pending intents. This bounds arbitration at the integration boundary using
a monotonic clock; the pure synchronizer remains unchanged. After settling, a
fresh speaker read reconciles final volume and mute, even with fallback polling
disabled. Ordinary speaker changes again apply in two-way mode. One-way mode
continues to observe speaker confirmations without applying them locally.

The runtime publishes speaker state only when the coordinator accepts an
observation. Suppressed notifications and health reads retain the waiting snapshot,
including its last speaker value. Completing the final read preserves subscription
health: without an event subscription the UI reports polling fallback when enabled,
or degraded delivery otherwise. Notification, polling and settling branches share
the same observation-to-publication orchestration, exercised by runtime regressions.

## Tradeoffs and validation

An independent speaker change during a local gesture is reconciled by the final
read rather than applied immediately. Mapping quantization or a safety cap may
produce one final local correction. The settling interval is bounded protection,
not proof of event provenance; very late notifications and device disconnects
remain hardware verification concerns. The adapter's echo suppression still runs.

Coordinator regressions reproduce startup, repeated input, stale notifications,
newest-volume dispatch, immediate mute replacing queued volume, deadline stability,
final reconciliation and later independent speaker control. The burst and mute
regressions fail with the original coordinator. Runtime tests exercise the same
buffer draining used by the production event loop. Native verification steps and
coverage limits are recorded in the verification matrix.

## Core Audio listener reattachment

A same-output reconnect previously registered replacement listeners with the
same callback context before removing the originals. Core Audio rejects this
registration with its illegal-operation status, and removing the original set
then leaves the replacement without callbacks. A native registration probe
reproduced this exact failure.

The worker now removes its registered listeners before attaching the replacement.
Attachment tracks successful registrations and rolls them back on any failure;
failed attachment stops the worker and reports device unavailability rather than
claiming observation is active. Endpoint destruction removes remaining listeners
while their callback context is still alive. Regression tests exercise the shared
replacement ordering and partial-registration rollback. A separate ignored native
test repeatedly reattaches and verifies all replacement registrations remain
installed, without changing output volume.

## Successful output refresh is not a speaker connection failure

Core Audio device-list/default-output notifications first resolve the selected
output's identity and volume/mute properties. An unchanged, readable endpoint
retains its listeners and emits no lifecycle event. An actual output change
reattaches and emits `DefaultOutputChanged`; the runtime reconciles the new
output in the existing speaker session rather than reporting audio unavailability
and starting exponential network backoff. Pending input for the previous endpoint
is discarded. Two-way mode applies the speaker baseline to the new output;
one-way mode sends the new output's local state. Actual disappearance, unreadable
state or failed attachment still enters bounded recovery.


## Ordering queued notifications across the final read

The callback listener timestamps received observations. Once the final settling
read completes, volume notifications queued before that boundary are stripped of
volume state and volume-read invalidation before delivery. Independent speaker
settings invalidations remain deliverable. This avoids a ready timer winning the
runtime select and releasing authority ahead of an already queued stale event.
The regression uses the real callback listener queue, deliberately executes the
timer first, and verifies both stale-volume rejection and later-event acceptance.
Notifications arriving after the read remain normal observations; the boundary
cannot establish the provenance of arbitrarily late network delivery.

A rejected speaker observation still invalidates mismatched cached volume/mute
commands. This does not pass stale state to the synchronizer, but allows a new
gesture ending at a previously sent value to reassert that value after an external
change. Core Audio channel elements are numbered cumulatively across stream
buffers, preventing duplicate listener addresses on multi-buffer outputs.
