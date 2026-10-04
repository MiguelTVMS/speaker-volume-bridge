# 0019: Distribution-aware update catalog and phase-one checks

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

The complete UTF-8 JSON document is limited to 256 KiB. Object members and entry
targets must be unique. Unknown fields, editions, channels, platforms,
architectures, actions and schema versions invalidate the document. Stable
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
downgrade offer.

## Consequences

The catalog can lag releases and different editions can legitimately advertise
different versions. Publication needs an explicit availability workflow rather
than being coupled directly to compilation or release creation. Phase 1 performs
no package download or installation.
