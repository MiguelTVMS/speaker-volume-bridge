# ADR 0020: Approved release catalogs and independent GA website composition

Status: Accepted for implementation; live activation remains operator-gated.

## Context

Public releases can outpace empty or stale served catalogs. GA-only website
promotion cannot deliver preview availability promptly. Existing clients embed
versioned catalog URLs, and publishing develop website content would change the
last GA site's version and guide unexpectedly.

## Decision

Keep v1 and v2 canonical on develop. Successful release completion and explicit
recovery share independently fetched metadata and package inspection. Prepare all
supported direct targets atomically: GA advances both feeds, Alpha/Beta only v2.
Never infer installed architecture from a generic filename, substitute Store
submission for public availability, or accept a partial package set. Preserve
additive metadata and persistent withdrawal tombstones. Version precedence and
latest-base validation prevent replay/ordering regressions.

Use deterministic release branches and focused PRs. An installation token makes
normal required PR CI run; approval and the normal merge path remain explicit.
Serialize writers, preserve review history using normal pushes, and periodically
reconcile public releases to recover superseded pending events. Catalog failure
has its own pending state and cannot rebuild or republish the application.

Route GA website and catalog-only delivery through one serialized Pages composer.
Persist immutable content-addressed GA bundles and manifests on a dedicated
hosting-state branch, outside expiring Actions artifacts. Bootstrap only from a
successful main artifact matching actual served bytes. Restore the last verified
GA bundle for catalog-only delivery; overlay current approved develop feeds for
both modes. Validate latest inputs before deploying the complete composed artifact.
Missing/corrupt snapshots, stale inputs and pending GA candidates fail closed.
After delivery, compare actual served bytes for both feeds, ordinary/cache-refresh
requests, cache policy and all non-catalog files. Mark live only after verification.

Retain previous verified and pending candidate states for recovery. Pages can
switch artifacts before post-deploy verification completes, so verification failure
means unverified, not automatic rollback. Periodic reconciliation converges on
current approved inputs; an old queued event cannot intentionally roll back a
newer completed generation. Exact historical source, generated inputs and rendered
bytes are necessary if immutable artifacts must be reconstructed.

## Consequences

No client URL/schema migration, Git branch preference or auto-installation is
introduced. V1 consumers ignore additive metadata including withdrawal records.
Catalog PR approval and hosting environment approval can delay live availability.
A GitHub App, environment eligibility, durable-state write permission and approved
activation of the composer on main are prerequisites. Git-backed snapshots have a
bounded size and require backups. Missing original build inputs cannot be guessed.
Review fixtures provide unpublished backfill; live feeds stay unchanged by this
implementation PR. See [operator instructions](../catalog-operations.md) and
[verification matrix](../verification-matrix.md).

## Credential preflight recovery

Reusable-workflow secrets are optional at GitHub evaluation time so missing
setup cannot invalidate an already published release. An explicit preflight
checks both credentials before scheduling the proposal runner; missing values
produce catalog-pending warnings and retry instructions, never a success claim
for catalog creation or delivery. No GITHUB_TOKEN fallback is introduced.
Present but invalid credentials still fail token creation.

### Workflow-only activation preserves GA source

Main activation promotes workflow definitions through review without a GA source
change. Production composition compares the approved source trees while excluding
workflow files for both the promotion push and later scheduled reconciliation.
Such a promotion uses the retained bundle, preserving its generated metadata and
all non-catalog bytes. Actual source changes and explicit GA recovery still use
the GA build path. A missing source or snapshot fails closed. Real-Git and CLI
regressions distinguish workflow-only changes from changed GA content.

Exact served-byte bootstrap must account for edge transformations operationally.
Do not normalize differences or adopt transformed content without a separately
reviewed verification design. Reconstruction does not authorize publishing.

Push reconciliation uses the last verified published GA revision, as scheduled
reconciliation does. Comparing only adjacent commits would miss an approved GA
source change whose earlier deployment failed or was superseded. A production
workflow-shell regression reproduces that sequence and requires GA recovery on
the following workflow-only push. The promotion gate also rejects workflow files
outside its reviewed allowlist before comparing approved definitions; its actual
shell guard is tested with an additional unreviewed workflow.

Workflow approval validation is independent of whether other content accompanies
the definitions. Any changed workflow in a main PR must match approved develop,
and the approved production catalog suite runs. The workflow-only classification
retains its focused allowlist; ordinary GA content keeps its existing content
validation route without exempting included workflows from review checks.

Promotion authorization uses a trusted default-branch validator. The repository
default branch is develop; the approved validator merge there installs the gate
for main-targeting PRs without a separate main bootstrap. It reads PR Git objects
but executes only trusted workflow code and approved develop tests.

The required promotion gate has no event path filter. It rejects stale base
events and workflow-changing heads that do not contain current main, ensuring
compared workflow definitions represent the merge result. Content-only published
release promotions retain their existing exact prospective merge-tree proof and
pinned release commit; workflow ancestry does not block that supported route.

A prior successful gate must not authorize merging after the base advances.
Activation requires the gate as a strict up-to-date required check, with existing
approvals preserved and pinned-release compatibility coordinated. If that cannot
be configured compatibly, an approved equivalent freshness guarantee is required
before activation. PR-event validation alone is not that guarantee.
