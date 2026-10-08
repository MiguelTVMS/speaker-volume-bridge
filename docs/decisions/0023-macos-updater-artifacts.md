# 0023: Direct macOS updater artifacts with native acceptance withheld

Status: Artifact preparation implemented; native installation acceptance pending.

## Reconcile issue 165 with current discovery

ADR 0021 supersedes issue 165's catalog representation and catalog-publication
prerequisites. Issue 178's supported-edition, saved policy, generation and strictly
newer version rules already live in the shared GitHub Releases service. Its issue
remains open; this work does not claim acceptance or closure of that broader scope.
Issue 164's catalog deployment path is historical and is not reinstated here.
No catalog, latest-version endpoint or additional discovery source is introduced.

GitHub Releases continues to select the exact official DMG and release page.
Extra updater payload/signature/descriptor assets do not grant install capability.
Older clients retain the same asset and open-page contract. Stable only excludes
GitHub prereleases; Include prereleases considers stable and previews; equal/older
versions remain ineligible. Store editions remain Store-managed and are excluded
from direct artifact preparation. Issue 166 owns consent, download, installation,
shutdown, relaunch and result presentation. None is added by this decision.

## Artifact preparation

Use Tauri CLI 2.11.3's supported signer format and the single `.app` root gzip tar
layout documented by [Tauri](https://v2.tauri.app/plugin/updater/). Run preparation
only after the existing direct bundle signing, notarization, stapling and native
verification. Archive the final bundle without rebuilding, modifying entitlements,
or stripping its files. Preserve file contents, modes and in-bundle symlinks;
reject foreign, duplicate, missing or escaping entries. AppleDouble sidecars are
excluded, matching Tauri's tar layout. Extended attributes are not a portable
archive contract; final extracted-bundle Apple acceptance must be tested natively.

The offline verifier uses pinned minisign-verify 0.2.5, the signature library used
by the inspected updater 2.10.0 source. This is an operator packaging tool and does
not register an updater plugin, frontend permission or installation command.
The normal build does not register an updater plugin. The opt-in
`native-updater-acceptance` feature pins updater 2.10.0 for native tests only.
Tauri itself is 2.12.1.

The payload and its detached signature are mandatory. The audit descriptor binds
the package version, direct edition, ARM64 target, filenames and digest, but is
not an authenticated discovery manifest. The verified archive contains the version,
identity and distribution metadata, so version binding must be checked against the
chosen release after signature verification. A signature alone authenticates bytes,
not arbitrary surrounding JSON. Do not trust descriptor version or digest as an
alternative to validating the signed application. It always records installation
capability as false and native acceptance as pending, with open_url fallback.

The signed release job optionally prepares retained CI validation
artifacts. They are deliberately outside the `*-release` publication selection.
Missing both updater key and public key reports unavailable and retains the DMG;
partial setup, rejected native bundle, signer failure or verifier failure fails
the job. No production key, credentials or environment settings are changed here.
Future Release titles contain only their version tag.

## Native approach and go/no-go

Inspect [updater 2.10.0](https://github.com/tauri-apps/plugins-workspace/blob/updater-v2.10.0/plugins/updater/src/updater.rs)
before adoption: macOS extracts to a temporary directory, moves the old app to
backup, and replaces the installation. Permission failure attempts an AppleScript
administrator operation. This is not proof of compatibility with App Sandbox or
read-only/translocated installations. A sandboxed app generally cannot replace an
Applications bundle simply because the user owns it. An administrator fallback
must not be assumed available inside the app's protections. The mutable upstream
branch has a different replacement implementation and is not evidence for 2.10.0.

Decision: **no-go for enabling automatic installation in this phase**. This host
has no valid signing identity; the available installed app is same-version and
fails strict code-signature validation. Therefore a genuine older-to-newer,
Developer ID signed, ordinary Applications upgrade could not be performed.
This is unavailable acceptance, not a demonstrated universal Tauri incompatibility.
The existing sandbox, DMG and open-page behavior remain intact. Any later updater
adoption needs a pinned version, sandbox-compatible replacement evidence and shared
policy revalidation before install, without custom policy or downgrade bypasses.

See the [verification matrix](../verification-matrix.md#direct-macos-updater-artifacts)
for exact acceptance steps and evidence boundaries.

## Signed native acceptance probe

The existing signing-verification workflow has an explicit, default-off updater
acceptance input. It uses a disposable checkout to build the current version and
one higher patch version, restoring the workspace manifests afterward without a
commit, tag or release. Both packages retain the product identifier, production
sandbox and real runtime; neither enables production install capability.

The feature requires a signed bundled fixture and explicit command-line mode.
It does not grant updater IPC permissions to the frontend. Transport for the
shared update service is a controlled GitHub-release fixture; stable/preview
selection, version precedence, generation invalidation and claim reauthorization
still execute in the production UpdateService. The native plugin uses only a fixed
loopback fixture endpoint and a temporary updater key. That test transport is
compiled out of normal editions. Private key material is deleted before artifacts
are retained; no production updater key or signing environment is changed.

In a disposable native account, the runner refuses an existing Applications app
and the seed mode refuses an existing configuration. It seeds non-default app,
Night schedule and update preferences through their normal persistence adapters,
launches the actual sandboxed installer, and accepts success only after relaunch
reports the newer version with matching saved settings and update preferences.
The signed bytes' bundle identity/version are rechecked before installation, and
policy generation is claimed again after download. Failures are recorded as no-go
results, not successful upgrades. A controlled test package is retained for manual
native follow-up. Physical synchronization, enabled login-item/session launch and
unsupported destinations remain separate acceptance requirements.
