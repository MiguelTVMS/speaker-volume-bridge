# ADR 0024: Order local gestures, speaker reads and notification delivery

**Status:** Accepted

## Decision

The coordinator retains one newest pending local volume and a fixed 45 ms dispatch
deadline. Native callbacks continue while input is coalesced; buffered user state
wins over application echoes. Mute bypasses debounce and supersedes queued volume.
Duplicate input does not extend dispatch or settling. The 500 ms settling deadline
makes a final read ready; authority stays latched until reconciliation, protecting
both timer-first and notification-first selection. New gestures invalidate the
previous gesture's command cache so they can reassert state after external changes.

Startup, settling, health polling and output changes use the same coordinator
read-and-reconcile operation. Native local state is observed before and after a
speaker read. If input changed, the read cannot overwrite it: the current local
state is retained and queued for dispatch. Local application checks its observed
baseline again immediately before writing, and rechecks mute before writing it after an
asynchronous volume write. Device volume quantization does not masquerade as a
new mute intent. These guards protect the asynchronous read window;
native OS property reads and writes are separate calls, not an atomic hardware
transaction. Polling and notifications leave the waiting snapshot intact while
local input is authoritative. Accepted publications preserve subscription health.

The callback listener assigns monotonically increasing acceptance revisions before
spawning handlers. Each runtime reconciliation records the acceptance revision at
its start and closes that read window only if an actual read occurred. Volume
notifications from before the read are obsolete. Those overlapping the operation
are ambiguous because GetVolume and GetMute are separate requests: their raw values
are replaced by a retained invalidation, which triggers the same fresh-read path,
even with periodic fallback polling disabled. Notifications after the window can
be applied normally. Independent speaker-settings invalidations are preserved.

Delivery rejects older completed handlers. When a sender supplies SEQ, its serial
order takes precedence, scoped to SID; rollover from the maximum key to 1 is
accepted and a new SID resets sequence tracking. Old-subscription queued state is
ignored. Without SEQ, acceptance revisions order delivery. See the
[UPnP eventing specification](https://upnp.org/specs/arch/UPnP-arch-DeviceArchitecture-v2.0-20140901.pdf).

The listener owns its accept task, which owns and reaps child handlers through a
JoinSet. Dropping a session aborts the accept task and its pending children,
releasing sockets rather than accumulating detached listeners across reconnects.

## Tradeoffs and validation

A dirty read window may require another GET pair. A fresh read batches older queued
invalidations behind its starting revision, bounding redundant processing for a
fixed queue. It does not infer atomic state across independent speaker actions.
A safety cap or mapping quantization may still produce an intentional final local
correction. One-way mode observes speaker state without applying it locally.

The normal suite covers both select orders, accepted sockets with delayed handlers,
handler completion reordering, sender replay/rollover/SID changes, notifications
inside GETs and during local application/publication, local input during each read
path, health polling fences, partial volume invalidations during authority, and
listener/child socket cleanup. The six whole-flow reproductions fail when their
corresponding protection is removed. Verification details and native hardware
coverage limits are in the verification matrix. Domain code remains independent
of Tauri, networking and platform APIs.

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
