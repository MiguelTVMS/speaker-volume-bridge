# 0021: Read approved update catalogs through GitHub raw

Status: Accepted for new builds; release and installed acceptance pending.

## Context

The catalog files already live on develop and are changed through reviewed PRs.
Requiring Pages delivery for every new-build consumer couples feed availability
to GA snapshot composition even when no website content changes. Previously
published installations have fixed website endpoints and cannot migrate through
a catalog change.

## Decision

The shell HTTP transport reads the repository's develop-owned v1 and v2 files at
raw.githubusercontent.com. Stable selects v1; Prereleases selects v2 on the same
branch. The transport retains bounded HTTPS requests, timeout, redirect limit,
catalog validation, retry/freshness policy, and generation guards. No request
includes credentials, speaker data or persistent installation identity.

Proposal automation still verifies public package availability and opens reviewed
catalog PRs with normal CI and human approval. Its GitHub App is not needed for
public feed reads. Merge is not a substitute for verifying the public raw bytes.
No package publication, installation or self-update is introduced.

Retain the website feeds and ADR 0020 delivery for previously published clients.
A later separately approved application release and installed verification are
needed to establish migration. Keep the privacy policy and website sources aligned
with both hosting destinations; their existing effective date is preserved.

## Verification

A regression builds the actual production HTTP requests for both policies and
asserts the approved raw branch/file addresses, GET method and absence of query,
credentials and authorization headers. It fails with the previous website
addresses and passes with the new source. Required workspace checks and PR CI
must pass; public raw reads and installed acceptance remain separate outcomes.
