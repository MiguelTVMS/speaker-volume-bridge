# ADR 0025: Order native Settings destinations by revision

**Status:** Accepted

Diagnostics, Updates and ordinary Settings tray actions, tray left-double-click,
and update notifications share one pending navigation request. Each request has a
monotonically increasing revision and optional page. Ordinary Settings emits a
null-page cancellation revision, preserving the visible page while superseding an
older destination, including one already consumed by an IPC response in flight.
Right and middle double-clicks leave navigation unchanged.

The frontend registers one listener before consuming the startup request. Events
and startup IPC responses pass through the same revision gate: older or duplicate
requests cannot activate pages. Delayed notification events therefore cannot
supersede newer tray choices. There is no promise chain for an IPC failure to poison.
Consuming a request removes the retained copy; a later duplicate event is harmless.
Warm event delivery acknowledges its exact revision through native IPC. The native
slot is cleared only if it still contains that revision, preserving a newer request
while preventing an accepted destination from replaying after WebView recreation.

Production-action tests cover cold/warm tray and notification ordering. Frontend
unit tests hold a consumed response across cancellation and reject replayed events.
Browser tests hold actual preview startup IPC, issue ordinary Settings or Updates,
then release the older Diagnostics response on all three platform layouts. These
ordering assertions fail with the faulty acceptance behavior restored. Prior native
tray acceptance is recorded separately; this repair has no new native acceptance claim.
