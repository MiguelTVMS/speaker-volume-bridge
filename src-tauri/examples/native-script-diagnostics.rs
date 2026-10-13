// SPDX-License-Identifier: Apache-2.0 OR MIT
// Exercises the actual osakit path on the process main thread, without privileges.
#[cfg(target_os = "macos")]
fn main() {
    use tauri_plugin_updater::native_diagnostics::{authorization_script, run_authorization};
    let source = authorization_script("error \"fixture native error\" number -1743");
    let error = run_authorization(
        |task| {
            task();
            Ok(())
        },
        move || tauri_plugin_updater::native_diagnostics::execute_script(&source),
    )
    .unwrap_err();
    match error {
        tauri_plugin_updater::Error::MacosAuthorization {
            stage,
            code,
            message,
        } => {
            assert_eq!(stage, "execute");
            assert_eq!(code, Some(-1743));
            assert_eq!(message, "fixture native error");
        }
        other => panic!("lost native AppleScript error: {other}"),
    }
}

#[cfg(not(target_os = "macos"))]
fn main() {}
