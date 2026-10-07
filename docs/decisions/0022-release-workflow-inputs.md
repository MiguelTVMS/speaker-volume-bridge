# 0022: Normalize release dispatch inputs

Status: Accepted for implementation; CI and actual release execution are separate.

## Decision

Expose Version (Major/Minor/Fix, default Fix), Stable (false), Sign Apple Pack
(true), Push Apple Store (false), and Push MS Store (false), in that order.
Stable maps to GA; otherwise the workflow publishes a Beta prerelease. Published
Alpha releases remain eligible for prerelease-aware consumers. No private-draft
input is exposed; native acceptance precedes public release dispatch.

Direct macOS signing is optional. A skipped signing job permits publication only
when signing was disabled; failures never satisfy the gate. Unsigned releases
contain the direct-edition DMG and disclose its signing status. Store packages
always require their own signing. Both Store push paths require stable plus the
corresponding opt-in. Apple validates and uploads the existing signed package to
App Store Connect after GitHub publication, retaining the existing approval
environment. Upload does not establish review acceptance or public availability.

## Verification

The input/default/Store-gate regression fails against the old workflow and passes
with the normalized inputs. Tests evaluate job conditions, cover dependency
outcomes and execute the Apple upload orchestration with stubbed tools to verify
validation failure prevents upload and private-key cleanup always runs. Native
signing, unsigned package installation and real Store delivery remain outstanding.
