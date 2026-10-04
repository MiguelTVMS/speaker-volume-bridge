# ADR 0015: macOS DMG downloads

**Status:** Accepted

## Decision

Publish a drag-to-Applications DMG as the only macOS direct download, containing the
existing signed and notarized app. Omit the compatibility ZIP from future
releases. The DMG reuses the release executable; no separate compilation is required.

Sign and notarize the DMG itself, require an Accepted response, staple and validate
its ticket, and check image integrity and Gatekeeper before uploading it. Release
download preparation requires the DMG before assigning permanent filenames.
The website links to the stable DMG filename. Deploy that link only after the first
GA release providing that asset is published.

## Verification

Direct-download packaging always embeds its distribution metadata. Without a
Developer ID provisioning profile it uses the checked-in base sandbox/network
entitlements. With a profile it selects the separate profile configuration,
embeds that profile and uses the generated application/team entitlements.
The release bundle-command regression exercises both configuration selections
and resolves their entitlement files; actual Apple signing remains a protected
workflow check.

Release-download tests exercise filename normalization, byte preservation, rejection of
missing/empty installers, and the website link. Local unsigned DMG verification
covers the app and Applications shortcut. Apple signing, notarization and
Gatekeeper verification require the protected release workflow; local unsigned
builds do not establish those results.

## Release asset naming

Publish one permanent filename per package instead of both versioned and
unversioned copies. The release tag supplies version identity and the website
keeps permanent latest-release links. Use explicit x64 and ARM64 Windows
and Linux filenames, with one installer per architecture. Debian package metadata
continues to use amd64; only the public filename is normalized. Validate all inputs before renaming; repeated
preparation must preserve the same five files and their bytes. Previously
published releases are unchanged.
