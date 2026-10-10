# 0023: Direct macOS updater artifacts with native acceptance withheld

Status: Artifact preparation verified; native probe failed; installation acceptance incomplete.

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

## Signed acceptance follow-up, 2026-10-09

The approved verification run on develop built the signed/notarized/stapled older
and newer test packages and authenticated payload. The older Applications app
reported a successful baseline with matching seeded settings and persisted update
preferences. The install attempt then reported failure while still running the
older version; no installed event or newer-version snapshot was retained.
The overall workflow succeeded because native failure is allowed for evidence
retention. That workflow conclusion does not establish replacement acceptance.

Independent downloaded-bundle Apple verification and offline payload-signature
verification passed. The app extracted from the actual updater archive also passed
strict Apple signature, stapled-ticket and Gatekeeper checks. The sanitized native summary does not retain the failure
operation, so this result does not yet distinguish a transport, policy or native
replacement failure. Preserve the no-go for production installation and the
release-page fallback. Permission/cancellation, read-only/translocated destinations,
login-session launch and real synchronization remain incomplete and require a
disposable native account. Issue 165 remains open. See the verification matrix
for the precise passed, failed and unavailable outcomes.

The native probe explicitly selects the `darwin-aarch64` updater manifest target.
Pinned updater 2.10.0 otherwise reports only `darwin` in `Update.target`, which
conflicts with the probe's exact-target guard even after selecting the ARM64 asset.
A real updater-check regression through the shared probe builder fails with the
default target and passes with explicit selection. This repairs test-only target
wiring; signed replacement acceptance still requires rerunning the fixed probe.

The signed rerun with explicit target selection still failed native acceptance.
Failure reporting now preserves typed updater operation/category and numeric
system/status codes in sanitized evidence. Arbitrary native error messages remain
private to the disposable account. The former summaries cannot recover the actual
error. A runner regression demonstrates error-context loss before the evidence
fix and safe retention afterward. Another signed probe is required to identify
and address the native cause; production installation remains a no-go.

The approved diagnostic run now identifies installation I/O permission denial
after the pinned updater's backup rename and administrator fallback both fail.
Signed packaging, download verification and authenticated archive checks complete
before this failure. The pinned plugin discards the underlying AppleScript error,
so sandbox rejection and unavailable hosted interactive authorization cannot yet
be distinguished. Settings and update preferences remain intact in the older
version; there is no newer-version relaunch evidence. Diagnostic regression
coverage is not an installer fix. Preserve the no-go and keep issue 165 open until
a supported replacement mechanism passes the native contract. See the verification
matrix's typed signed replacement result for the remaining evidence boundary.


### Acceptance-only underlying AppleScript diagnostic

The pinned updater is locally patched behind the default-off acceptance feature
at the point where it masked administrator AppleScript errors. The existing
replacement command and privileges are unchanged. A try/on-error wrapper retains
the native script error number and message; compilation and main-thread dispatch
failures are distinguished from script execution failures. Raw messages remain
private, while only the integer script code and a fixed stage enter retained
public evidence. This is diagnostic capture, not an authorization bypass or an
installer cause fix.

The shared installer-orchestration regression failed with the former generic
permission-denied result and passes retaining the script code/message. The runner
regression likewise failed before numeric-code retention and passes afterward,
including rejection of injected stages and boolean codes. A real main-thread
osakit fixture raises a known script error and verifies the wrapper retains its
number and message. Restoring the original error masking makes this real-script
fixture fail; restoring the diagnostic makes it pass. Normal macOS CI executes it
without privileged operations.
Signed acceptance still requires the protected-branch diagnostic build and its
existing signing review. Native cause and replacement acceptance remain pending;
issue 165 stays open. Website review: no update needed for test-only diagnostics.


Vendored-source security analysis identified upstream optional TLS-validation
bypasses. The local patch removes all four bypass calls and rejects configuration
requesting invalid-certificate or invalid-hostname acceptance before networking.
A regression through the real probe builder fails with the former behavior and
passes with rejection. Signing controls are unchanged; no security alert is
suppressed or dismissed to permit merging.

## Sandbox-compatible acceptance adapter

The signed diagnostic captured errAuthorizationDenied (-60005). Apple's
[Authorization Services documentation](https://developer.apple.com/documentation/security/authorization-services)
excludes that privilege mechanism from App Sandbox. The default-off acceptance
adapter instead requests NSWorkspace ReplaceFile authorization and uses the
corresponding authorized FileManager. No pre-delete, pre-rename or privileged shell
fallback is used in this path. Native NSError evidence stays private except numeric
codes and fixed stages. Shared-orchestration fixtures cover retention on denial
and cancellation, missing destinations, dispatch errors and success callbacks.

Apple's [Privileged File Operations entitlement](https://developer.apple.com/documentation/bundleresources/entitlements/com.apple.developer.security.privileged-file-operations)
is a prerequisite, not an approval implied by successful compilation or Store
review. The [request draft](../apple-privileged-file-operations-request.md) is not
submitted. No entitlement/profile/protection changes are made here. Signed native
replacement, relaunch and preservation remain unavailable pending the grant and
full acceptance. Store editions continue to use Store delivery; production
self-installation remains disabled and issue 165 remains open. Website review:
no user-facing behavior changed; this adapter is test-only, so no website update
is needed.

Replacement failures now consume Apple's NSFileOriginalItemLocationKey recovery
URL before reporting the failure. Restoration uses ordinary rename if the target
is absent, or the existing authorized replacement manager if it remains present.
No extra privilege is requested. If recovery fails or the destination is still
absent, staging is retained and a distinct private recovery failure is reported.
Tests exercise Update::install, extraction and routing for denied/cancelled
operations and partial failures, including preservation of recovery bytes when
restoration is denied. These tests replace the original helper-only assurance;
actual native recovery remains unverified pending the signed acceptance gate.
