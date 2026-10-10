# Acceptance-only native authorization diagnostics

Source: crates.io tauri-plugin-updater 2.10.0. Upstream license files are retained.
Only Cargo.toml, src/lib.rs, src/error.rs and src/updater.rs differ from the
published crate; src/native_diagnostics.rs and src/native_replacement.rs are added. Other bundled source,
permissions, build script and JavaScript are unchanged.

The default-off `native-acceptance-diagnostics` feature is enabled only by the
application's default-off native acceptance feature. Normal builds do not include
or register the updater. With the diagnostic feature disabled, the original
installer path remains intact.

The diagnostic path wraps the existing administrator AppleScript command in a
try/on-error block. It returns the original error number and message instead of
masking them as generic permission denial. The commands, privileges, replacement
order and temporary-directory cleanup are unchanged. Compile, dispatch, receive
and decode failures are separately identified. Raw messages stay in the private
probe result; only numeric script codes and fixed stages reach public evidence.

Regressions run through the shared orchestration used by install_inner. The
application's native-script-diagnostics example executes that same wrapper and
osakit adapter on the main thread with a deliberately raised fixture error,
without requesting privileges or changing any files. CI runs it on macOS.

This patch diagnoses the pinned installer; it is not replacement acceptance or a
new production installation mechanism. Remove it after upstream provides equivalent
error evidence or a separately accepted replacement approach supersedes the probe.

Security review of the vendored source exposed four upstream opt-out calls for
TLS certificate/hostname validation. This local patch removes those calls and
rejects either insecure TLS configuration flag at UpdaterBuilder::build, before
networking. The production-builder regression fails with upstream acceptance of
the flags and passes with rejection. Loopback-only acceptance transport remains
separately configured; no TLS validation is bypassed.


The acceptance installer now calls native_replacement instead of the AppleScript
fallback. NSWorkspace ReplaceFile authorization creates an authorized FileManager
for replacement of the existing destination. No destination rename/deletion occurs
before authorization and no shell fallback exists on this feature path. The old
AppleScript fixture remains only for diagnostic regression history. NSError domain
and message remain private; numeric codes and fixed native stages are reported.
No entitlement is added: signed native acceptance requires Apple's grant and a
matching direct-download profile. The shared replacement orchestration is covered
in the normal macOS acceptance-feature CI tests; fixtures do not prove native
permission UI or bundle replacement.
