# Draft Apple Privileged File Operations entitlement request

Status: prepared for account-holder review, not submitted. Apple approval and a
matching signing profile are prerequisites for signed acceptance. This draft
requests no change to GitHub signing-environment protections.

Official entitlement documentation and request form entry point:
https://developer.apple.com/documentation/bundleresources/entitlements/com.apple.developer.security.privileged-file-operations

## Proposed request text

Speaker Volume Bridge is an independently developed macOS menu bar utility that
synchronizes the computer's output volume with a compatible speaker on the local
network. We distribute separate direct-download and Mac App Store editions.

We request the Privileged File Operations entitlement for the sandboxed,
Developer ID-signed direct-download edition. The intended operation is replacement
of the app's own existing installed bundle during a user-authorized update. The
app would request NSWorkspaceAuthorizationTypeReplaceFile authorization and use
FileManager created with that authorization to replace the existing bundle with
the verified newer bundle. It would not execute privileged shell commands or
request arbitrary administrative execution.

The Mac App Store edition does not use this capability or perform self-updates.
Its updates are delivered through the Mac App Store. We do not request this
entitlement for the Store edition.

The current implementation is an acceptance-only adapter behind a default-off
build feature. Production self-installation remains disabled. The acceptance
probe verifies update eligibility and the updater signature before installation;
artifact preparation separately verifies the direct edition's Apple signature,
notarization and bundle identity. We retain the release-page fallback when native
installation is unavailable. Denial or cancellation must leave the installed app
intact. App Sandbox remains enabled.

Before any production enablement, we will verify signed older-to-newer replacement,
relaunch, settings and update-policy continuity, permission denial, cancellation,
read-only and translocated installations, and release-page fallback. We are not
claiming these native acceptance outcomes have passed yet.

Please confirm whether this entitlement can be granted for this direct-download
self-replacement use case and what provisioning requirements apply.

## Account-holder preparation

Use the direct-download app identifier and the corresponding developer team in
Apple's actual form. Confirm the current app identifier in the developer account;
do not infer entitlement approval from an App Store review or from this draft.
Supply only the contact and app details Apple requests. Do not attach CI logs,
credentials, signing material or private native diagnostics.

Do not add the entitlement to either signing configuration until Apple grants it.
After the grant, verify that the approved direct-download profile includes the
capability, then conduct a separately approved signed acceptance run. Keep the
Store profile and entitlements unchanged. Keep issue 165 open until the full
acceptance contract passes.
