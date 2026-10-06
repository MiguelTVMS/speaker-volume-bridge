# Catalog publication and independent website delivery

Both catalogs are owned by `develop`. Stable clients continue to read
`/updates/v1/catalog.json`; Prereleases clients read `/updates/v2/catalog.json`.
Neither preference selects a Git branch. There is no client URL migration or
installer in this phase. See [ADR 0020](decisions/0020-independent-catalog-delivery.md).

## Publication states and approval

`release-published` means application publication completed. `catalog-pending`
means package verification or its review PR still requires action. Only a
successful comparison with both served feeds establishes `catalog-live`.
Application publication remains successful if catalog preparation fails.

The Release workflow invokes Propose update catalog only after successful public
publication. It independently retrieves the release and all five required direct
packages. Package version, installed payload architecture, distribution provenance,
optional stamped classification, release body classification, public asset length
and available digest must agree. Missing or partial packages reject the entire
proposal. The generic DMG filename supplies no architecture evidence: inspect the
application's Mach-O slices. Windows inspection reads the installed executable,
not the NSIS stub; Debian control architecture must agree with its ELF payload.
GA updates both feeds. Alpha/Beta updates only v2. Store entries remain unchanged;
Store upload, submission and certification cannot create public availability.

A deterministic release branch and focused catalog PR preserve retries' review
history with normal merge/push, never force push, approval or auto-merge. Normal
required CI must pass and a human must approve. The proposer rereads develop and
reprepares both catalogs when the base advances. Separate pending proposals can
conflict: merge one, rerun verification for the other, then review its refreshed
result. Older versions cannot replace newer targets. Additive catalog, entry and
action metadata survive insertion/replacement/withdrawal. Withdrawal records in
the catalogs retain target/classification/version tombstones; do not remove them
in routine cleanup. An equal or older replay cannot restore a withdrawn offer.
A verified newer release can replace that target without discarding the ledger.
For an already absent offer, supply the reviewed `version` cutoff on the withdrawal
record; this records intent before backfill. Without an existing entry or explicit
cutoff, a withdrawal is an idempotent no-op because its version cannot be guessed.

Run Propose update catalog on `develop` with `release_tag` for recovery of an
already public release, including a draft published later outside Release. It
uses the same package and catalog orchestration. Do not dispatch Release to retry
catalogs. A scheduled sweep independently verifies supported public releases to
recover pending events superseded by GitHub concurrency. An incomplete sweep
reports catalog-pending and can be retried. Scheduled workflows require the new
workflow definitions on the default branch; until then use explicit recovery.
An installation token is required for proposal pushes/PR creation so GitHub
actually triggers ordinary pull-request validation. No GITHUB_TOKEN fallback is
permitted. Missing credentials are accepted at the reusable-workflow boundary and checked
by an Ubuntu preflight. It emits an explicit catalog-pending warning and recovery
instructions, then skips the proposal runner. Application publication therefore
remains successful, without claiming that a catalog PR exists or catalogs are live.
This does not supply credentials or bypass review; configure the dedicated App
and retry Propose update catalog for the public tag. Present but invalid
credentials still fail token creation and require correction.

## Hosting prerequisites

Keep GitHub Pages as the hosting provider. Configure these prerequisites separately,
with the existing administrator and approval process:

- An installed GitHub App with repository contents and pull-request write access;
  configure `CATALOG_APP_ID` and `CATALOG_APP_PRIVATE_KEY` for the proposal workflow.
  The App does not approve or merge its PRs.
- The existing `github-pages` environment must allow the `develop` catalog-delivery
  workflow as well as GA delivery from `main`, retaining existing required approvals.
- Permit workflow writes to the dedicated `catalog-hosting-state` branch. This is
  hosting state, never a replacement for approved catalog commits on develop.
  Keep snapshots and branch history; back up that branch independently.
- Coordinate activation of the composition workflow on main through an approved
  workflow-only promotion before any subsequent GA website deployment. An old main
  workflow can still deploy stale main catalogs until this handoff is complete.
  Both branches must use the same concurrency group and composition entry point.
  This handoff does not authorize publishing new website content.

Snapshots use immutable content-addressed ZIP files in the hosting-state branch.
They survive Actions artifact expiry. A 90 MiB uncompressed limit rejects oversized
bundles before publication; if growth exceeds it, provision reviewed durable
storage explicitly rather than selecting another host. A Git push failure cannot
fall back to an ephemeral Actions artifact. State separately retains the last
verified published GA reference and a candidate awaiting deployment/verification.
A pending candidate blocks catalog-only delivery, preventing rollback to an older
GA website after an ambiguous deployment outcome.

## Safe bootstrap and expired-artifact recovery

Before initial catalog delivery, approve the catalog backfill and retain the
actual currently served GA bundle. Run Website with `mode=bootstrap` and the
successful main Website run containing its `github-pages` artifact. Bootstrap
checks the run's successful main/website identity, safely extracts regular files,
and compares every artifact file with actual HTTPS-served bytes, both normally
and with cache refresh requested. It retains the snapshot and records the served
state without deploying. It refuses to replace existing hosting state.
A stale artifact, unexpected generated website version, missing file, stale cache
or expired artifact stops bootstrap. It never builds a website from develop.

If the original Actions artifact expired, recover the exact retained archive and
state from the hosting-state branch or its backup. If no immutable copy exists,
bootstrap remains blocked. Reconstruct only from the exact approved GA source,
its original `pages/_data/build.json` and release metadata, the original pinned
Jekyll/build image and generated Markdown inputs. Preserve an inventory of the
original published files; verify its non-catalog hashes and compare every rebuilt
file against the served site with `compose-catalog-site.py verify --all-files`.
Use an administrator-reviewed recovery of the durable snapshot/reference only
after those byte comparisons pass. A reconstruction with any changed byte is
insufficient. The workflow does not automate reconstruction from missing inputs
or scrape an incomplete website inventory. Missing historical inputs are an
explicit hosting prerequisite, not permission to publish arbitrary main/develop
content. Reconstructing or adopting hosting state is a separate operator action.

## Independent catalog delivery

An approved catalog merge on develop triggers Website without another application
release or main promotion. It restores the last verified GA archive, verifies its
manifest, overlays the current develop catalog bytes and validates both documents.
All non-catalog bytes remain identical, including GA version metadata, generated
guide content, Markdown counterparts and assets. The complete Pages artifact is
uploaded and deployed through the same serialized path as GA website builds.
GA builds use main content but overlay current develop catalogs before delivery.
Stale main builds and changed catalog inputs fail before deployment.

GitHub concurrency can supersede pending work; scheduled reconciliation catches
up to the newest approved catalogs and intended main GA source. Main workflow
activation is required for those schedules. A develop recovery run delivers
catalogs; a main recovery run rebuilds only approved main GA website content.
Changing branches after validation can occur while Pages accepts a deployment;
subsequent serialized reconciliation converges on current approved inputs. An
already newer successful hosted generation is never intentionally replaced by an
older queued catalog snapshot. No application bump/release is dispatched.

After deploy, both ordinary client URLs and cache-refresh requests must return
exact intended bytes, not merely parseable JSON. Cache policy is required and
non-catalog files are checked against the intended GA artifact too. Bounded retries
allow CDN convergence. Failed builds or pre-deploy checks leave the previous
served site; a deploy or post-deploy verification failure leaves state unverified
and retains previous and candidate snapshots for explicit recovery. Pages may
already have switched artifacts when verification fails; do not claim rollback
or catalog-live in that case.

Retry a catalog-only failure with Website on develop. For an ambiguous GA result,
use `verify-candidate` to verify the candidate already served, without deployment,
or retry Website on main to complete the intended GA build/delivery. Catalogs
that advanced since the candidate was served can require a GA recovery run before
verification succeeds. Never clear pending state merely to bypass verification.

## Reviewable backfill

`tests/fixtures/update-catalog/backfill-v1.json` and `backfill-v2.json` are review
fixtures prepared from independently fetched public GA and Beta packages. They
are deliberately outside `pages/updates/`: merging the implementation cannot
publish those entries. The GA has five supported direct targets; v2 additionally
retains five Beta targets. No Store or unsupported macOS target is invented.
Older public formats without the complete current package set are excluded.

Recreate a review directory with:

```sh
python3 scripts/release-catalog.py PUBLIC_GA_TAG PUBLIC_PREVIEW_TAG --output REVIEW_DIRECTORY
```

Run on macOS with `sevenzip`, `dpkg` and pinned `pefile` installed. This command
writes review files only. For publication, rerun Propose update catalog against
current develop, independently reverify the packages and obtain normal approval.
Do not copy stale fixtures over current catalogs or withdrawal records.

## Acceptance boundaries

Offline production orchestration tests cover preparation, retries, ordering,
withdrawals, metadata, snapshot integrity, composition and served-content/cache
comparison. The real UpdateService consumes the verified backfill through its
shared transport orchestration: equal versions are up to date, older compatible
versions receive the exact action, wrong edition/architecture remains unavailable,
and Stable/Prereleases remain separate. These controlled clients do not prove
native installed-package recognition or actual Pages deployment. See the
[verification matrix](verification-matrix.md) for the live procedure.
