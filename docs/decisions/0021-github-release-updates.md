# 0021: Discover updates through the public GitHub Releases API

Status: Implemented for new builds; release and native acceptance pending.
Supersedes the catalog transport and delivery portions of ADR 0019 and ADR 0020.

## Decision

New builds read published releases anonymously from the official repository's
GitHub Releases API. There is no separately maintained version/catalog file,
GitHub App requirement for checks, website deployment or redirect migration.
The two dropdown labels are Stable only (default) and Include prereleases.
Stable only excludes GitHub prereleases. Include prereleases considers both GA
and prereleases, including numeric preview versions. GitHub's prerelease boolean
is authoritative; Alpha/Beta labels are not required. Version precedence selects
the highest compatible release regardless of API order or publication date.

The shell adapter sends a fixed application user agent, GitHub JSON accept header
and API version, without authorization or installation identifiers. It reads pages
of 100, with at most ten pages and two MiB total, ten seconds per request and three
redirects. Incomplete pagination, network failures, invalid JSON and rate limits
remain unavailable, retaining an existing validated offer. Rate-limit responses
honor Retry-After/reset with a minimum one-minute cooldown, including manual checks.
All forbidden responses receive this cooldown conservatively because secondary
limits can omit Retry-After while the primary quota remains available.

Candidates must have a canonical v-prefixed semantic version, publication time,
exact project release page, and exactly one uploaded nonempty official installer
asset for the installed edition and application architecture. The official
unqualified macOS DMG contract is ARM64; no Intel support is inferred. Store,
custom and ambiguous editions never acquire offers from direct-release assets.
Release metadata establishes presence, not independent package-byte verification.
CI package checks and native installation remain separate evidence.

Selection changes preserve the existing saved keys, clear old actions and reject
obsolete responses. Changing source invalidates old catalog freshness/offers but
preserves user preferences and notification history. The scheduler retains its
startup delay, daily success interval, bounded retries, duplicate-start guard and
shutdown cancellation. Notifications and exact release-page opening are retained.
The application never downloads or installs an update.

Automatic release catalog proposals, scheduled reconciliation and develop website
deployments are retired. Website delivery remains on main. Historical manual
catalog contracts and tooling remain available. Old
applications receive no new migration or redirects. No app release is dispatched
by implementation or tests; installed migration requires a future approved release.

## Verification

Production-request and shared-service regressions fail against the former raw
catalog implementation and pass against this API implementation. Coverage includes
both policies, unordered numeric previews, exact page actions, missing/incomplete
assets, draft/foreign releases, saved policy, failed refresh, rate limits and actual
HTTP pagination through the production transport. Former catalog fixtures remain
isolated under test-only compatibility code for existing lifecycle regressions.
Unsigned packaged macOS ARM64 dropdown, version/edition detection, checks and
restart persistence passed during readiness acceptance. Native newer-offer opening,
notifications, signing, Stores and unavailable platforms remain outstanding; see
[verification matrix](../verification-matrix.md).

Store editions have a separate `store_managed` update state. The shell rejects
GitHub discovery for these editions at the shared service boundary, disables
automatic checks even for migrated enabled preferences, and exposes only fixed
native Store destinations through an edition-authorized command. No release
offer or Store availability claim is created. Direct release-policy behavior is
unchanged. Existing catalog clients require a one-time manual upgrade; see
[upgrade guidance](../rebrand-upgrade.md).
