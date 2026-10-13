# ADR 0025: Retain explicit tray settings destinations

**Status:** Accepted

The Diagnostics tray action previously only showed Settings, leaving whichever
page was already selected. Explicit Diagnostics and Updates menu actions now
store their destination before opening the window, then emit `open-settings-page`.
The frontend installs its listener and consumes the pending destination, handling
both an already-loaded hidden window and initial frontend startup. Consumption
clears the request; the newest explicit menu destination wins. Ordinary Settings
keeps the selected page. Update-notification activation shares this destination
slot and event with tray actions. The frontend has one serialized consumer,
registered before startup consumption; it consumes the latest retained destination
rather than independently applying event payloads or update-notification flags.

Tests invoke the production menu action before and after frontend subscription.
Rendered browser coverage verifies cold and warm Diagnostics routing on all three
platform layouts. The user also confirmed the rebuilt native context-menu action opens Diagnostics
directly.

Ordinary Open settings also replaces the pending navigation slot, with no explicit
page. A later ordinary request therefore cancels an unconsumed Diagnostics or
Updates destination during cold startup.

Left double-click on the tray uses the same ordinary Settings action, including
clearing an unconsumed explicit destination. Right and middle double-clicks do
not alter pending navigation. The regression invokes the shared production
double-click action before frontend subscription or consumption.

The notification-to-tray cold-start regression invokes the actual notification
activation and tray actions before consumption. Browser regressions verify that
a newer Diagnostics request supersedes an earlier update notification on all
platform layouts. They fail with the old independent consumers restored.
