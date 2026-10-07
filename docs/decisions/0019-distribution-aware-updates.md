# 0019: Distribution-aware update catalog and phase-one checks

The catalog transport/delivery decision is superseded for new builds by [ADR 0021](0021-github-release-updates.md). Historical catalog tooling is retained; legacy delivery and redirects are outside the current rollout scope. Existing
catalog clients require a one-time manual upgrade; see
[upgrade guidance](../rebrand-upgrade.md). Catalog activation is not a readiness
prerequisite for GitHub Releases discovery.


## Status

Accepted for phase 1.

## Decision

The public site owns a versioned catalog at `/updates/v1/catalog.json`. A catalog
entry is keyed by installed edition, stable channel, operating system and
application architecture. It reports only independently verified public
availability. Builds, uploads, submissions, workspace versions and tags are not
publication evidence.

Schema v1 contains `schemaVersion`, `generatedAt` and `entries`. Each entry has:

- `edition`: `direct_macos`, `direct_windows`, `microsoft_store`,
  `mac_app_store` or `debian`;
- `channel`: `stable`;
- `os`: `macos`, `windows` or `linux`;
- `architecture`: application architecture, `aarch64` or `x86_64`, never host
  architecture;
- `version`: canonical Semantic Versioning text without a leading `v`;
- `publishedAt`: an RFC 3339 timestamp with timezone;
- non-empty human-readable `releaseNotes`, limited to 16 KiB UTF-8;
- `action`: `{ "type": "open_url", "url": "https://..." }`.

The complete UTF-8 JSON document is limited to 256 KiB. Duplicate JSON object
members and duplicate entry targets are invalid. Additive metadata fields are
ignored by phase-one clients at the catalog, entry, and `open_url` action levels;
required fields, editions, channels, platforms, architectures, actions and schema
versions remain validated. Unknown actions invalidate the document. Stable
entries cannot contain prerelease versions. URLs must be absolute HTTPS URLs
without embedded credentials. Missing targets mean that edition is not currently
offered; they do not mean the running version is current.

Consumers reject an unsupported or invalid document as unavailable, retain no
unvalidated action, and continue normal synchronization. Unknown, custom, debug
and demo builds are intentionally absent: they may show a static informational
downloads page but never claim binary compatibility. Future schemas use a new
path. Additive action data may be introduced only while retaining the v1
`open_url` fallback so older phase-one clients remain useful.

The shell owns an injectable installed-distribution resolver. It combines a
package resource with platform evidence and uses the workspace package version
plus compiled target architecture. Windows Store classification requires a
Store-signed package; other packaged Windows builds are sideloaded. macOS App
Store classification requires both matching provenance and an App Store receipt;
sandboxing alone is insufficient. Debian classification requires matching
provenance and the official package metadata. Older packages without provenance,
conflicts, source errors, custom/debug/demo builds and unsupported architectures
resolve conservatively and never enable direct installation.

Publishers validate the entire catalog and update entries only after confirming
the corresponding edition and architecture are publicly obtainable. Withdrawal
removes the entry; clients never interpret removal or an older entry as a
downgrade offer. Publication preserves all existing top-level catalog fields,
replacing only `generatedAt` and `entries` when a target changes. Retained entries
and their action metadata are preserved; a no-op returns the original bytes.

Publication is serialized and fail-closed. An availability record supplies
independent evidence for exactly one target: a non-draft, non-prerelease GitHub
release with its required public assets, or an explicitly published matching
Store edition. A reviewed catalog digest prevents stale concurrent mutations.
The automation is idempotent, rejects same-version changes and downgrades, and
opens a focused PR to `develop`; it does not deploy or publish. Catalog changes
reach the site only through the normal release flow into `main`, where the Pages
job verifies the served document and cache header after its atomic deployment.

One application-level service owns update state. Startup, periodic, wake and
manual checks use one orchestration path that emits status and claims any
notification at most once. Sending a notification never changes the selected
Settings page. The Updates page opens only after notification activation or an
explicit tray action. Update notices carry platform-specific activation intent;
schedule notices continue to open Settings. Its HTTPS transport is bounded
to 10 seconds, 256 KiB and three redirects, and accepted actions are `open_url`
targets on approved project/Store origins. The service persists automatic-check
preference, last attempt, last success and last-notified target separately from
speaker configuration, including validated offers for restart recovery. Each
cached offer includes edition, channel, OS, and application architecture and is
checked at startup against the running package version and target. Malformed,
equal or older, and different-target offers are discarded. Legacy cache records
without target identity are discarded conservatively while preferences and the
last successful check remain intact. A retained offer's action URL is revalidated
before it can be opened. Recognized
release packages default on; development and unknown packages are unsupported.
Startup waits 30 seconds. Successful refreshes replace or withdraw the cached
offer; failed refreshes retain the last known offer without advancing success time.
Checks are fresh for 24 hours, transient failures use bounded backoff, and all
callers share one in-flight request. Background failures are quiet; manual failures
are visible.
No package download or installer path exists in phase 1.

## Consequences

The catalog can lag releases and different editions can legitimately advertise
different versions. Publication needs an explicit availability workflow rather
than being coupled directly to compilation or release creation. Phase 1 performs
no package download or installation.

## Release policy prerequisite (issue #178)

The persisted `UpdatePolicy` is separate from installed package provenance and
`publishedClassification` (GA, Alpha or Beta; absent evidence remains unknown).
The release workflow stamps that classification into package resources; it never
selects the user's policy. Existing preferences migrate to Stable. An explicit
provenance-based capability permits selection only for recognized direct macOS,
direct Windows and official Debian packages on supported application architectures.
Store, sideloaded, custom, development and unknown architectures reject the backend
command as well as omitting the control. Later installers must reuse this capability,
policy, generation and exact target contract and revalidate before installing.

Stable continues to read schema v1 at `/updates/v1/catalog.json`; its contract is
unchanged. Prereleases reads only schema v2 at `/updates/v2/catalog.json`. V2 contains
both GA and Alpha/Beta candidates, with one entry per edition/channel/OS/architecture/
classification. `classification` is mandatory: GA uses `stable`; Alpha/Beta use
`prereleases`. Store targets are forbidden. Select the maximum semantic version
precedence across compatible candidates, ignoring publication time and build metadata.
Only strictly newer versions produce offers; numeric Alpha/Beta versions are valid.
An equal numeric GA promotion cannot reinstall the same version. Returning to Stable
waits for the next newer GA, never downgrading. V2 actions must point to the exact
published release tag page; latest-stable redirects are forbidden.

Publication independently fetches the public release's tag, draft/prerelease flags,
publisher body classification and nonempty compatible uploaded assets. The
`**Release channel:** GA|Alpha|Beta` marker is the existing publisher contract, not a
version-suffix guess. Conflicting or absent evidence fails closed. V1 rejects previews;
v2 permits only validated public GA/Alpha/Beta records. Availability remains independent
per distribution and classification. Existing additive metadata, duplicate rejection,
reviewed digest, serialized writers, idempotency and approved PR/site flow remain.
Both checked-in feeds remain empty until separately authorized publication.

An explicit policy change atomically persists a new generation, clears offers,
dismissals and freshness, emits checking state and requests one manual check even
when automatic checks are disabled. Requests retain their starting policy/generation;
obsolete results cannot commit cache, timestamps or notification intent. Notification
permission completion revalidates generation and current cache. Queued page actions
carry a generation and are rejected after a switch. Freshness/cache target includes
policy, edition, OS and architecture. Source availability is persisted independently
for stable and preview feeds. A missing, empty or incompatible preview feed is unavailable,
never current. Unsupported policy or changed cache target is normalized conservatively
on restart without modifying speaker settings or notification/automatic preferences.
Persisted notice history includes edition/OS/architecture/version but excludes policy
so a GA reachable in both feeds notifies once, including after intervening previews. All network and platform work stays in the shell.

## Independent publication follow-up (issue #183)

[ADR 0020](0020-independent-catalog-delivery.md) replaces the manual one-target
proposal/site-promotion flow with verified public package orchestration, approved
develop PRs and independent catalog composition over retained GA website bytes.
The consumer URLs, release policy and open-page-only actions remain unchanged.

### Production scheduler verification

The application manager owns one background task. Only the timer is injected for
lifecycle tests; request, delivery, task ownership and cancellation use production
orchestration. Delayed startup, bounded retry waits, repeated start and shutdown
are checked through this path, including cancellation of an in-flight transport.
These tests do not establish installed-package or OS notification acceptance.

Task abort is cooperative. Manager shutdown first publishes an explicit stop flag
and the scheduler checks it at request boundaries, including a timer that became
ready within an already running poll. A deterministic production lifecycle test
fails with abort alone and passes with the stop flag; a separate pending-transport
regression confirms cancellation still occurs.
