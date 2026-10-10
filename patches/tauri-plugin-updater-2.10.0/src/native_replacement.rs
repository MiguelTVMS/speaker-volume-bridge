// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Default-off acceptance adapter. No entitlement or production registration is added.
use crate::{Error, Result};
use std::path::{Path, PathBuf};

pub type Completion = Box<dyn FnOnce(Result<()>) + Send>;

pub type Start = std::sync::Arc<dyn Fn(PathBuf, PathBuf, Completion) + Send + Sync>;

fn failure(stage: &'static str, message: String) -> Error {
    Error::MacosReplacement {
        stage,
        domain: String::new(),
        code: None,
        message,
    }
}

/// Shared by the real installer and regression fixtures. Keep the existing app
/// intact until the authorized FileManager replacement itself succeeds.
pub fn replace_existing(
    destination: &Path,
    candidate: &Path,
    dispatch: impl FnOnce(Box<dyn FnOnce() + Send + Sync>) -> std::result::Result<(), String>,
    start: impl FnOnce(PathBuf, PathBuf, Completion) + Send + Sync + 'static,
) -> Result<()> {
    if !destination.is_dir() || !candidate.is_dir() || destination == candidate {
        return Err(failure(
            "precondition",
            "replacement requires distinct existing bundles".into(),
        ));
    }
    if destination.to_str().is_none() || candidate.to_str().is_none() {
        return Err(failure(
            "precondition",
            "non-Unicode bundle paths are unsupported".into(),
        ));
    }
    let destination = destination.to_owned();
    let candidate = candidate.to_owned();
    let (tx, rx) = std::sync::mpsc::channel();
    dispatch(Box::new(move || {
        start(
            destination,
            candidate,
            Box::new(move |result| {
                let _ = tx.send(result);
            }),
        )
    }))
    .map_err(|message| failure("dispatch", message))?;
    // Do not time out while a user authorization/replacement is still pending:
    // reporting failure and deleting staging would race a late successful callback.
    rx.recv()
        .map_err(|error| failure("receive", error.to_string()))?
}

/// Restore Apple's relocated original before propagating the replacement failure.
/// If restoration is denied, retain the recovery location in private diagnostics.
pub fn recover_after_failure(
    destination: &Path,
    original: Option<&Path>,
    error: Error,
    restore: impl FnOnce(&Path, &Path) -> Result<()>,
) -> Error {
    if let Some(original) = original.filter(|original| *original != destination) {
        if let Err(recovery) = restore(original, destination) {
            return failure(
                "recovery",
                format!(
                    "{error}; original retained at {}; recovery failed: {recovery}",
                    original.display()
                ),
            );
        }
    }
    if !destination.is_dir() {
        return failure("recovery", format!("{error}; original destination unavailable after recovery; staging must be retained"));
    }
    error
}

#[cfg(target_os = "macos")]
pub fn start_native(destination: PathBuf, candidate: PathBuf, complete: Completion) {
    use block2::RcBlock;
    use objc2_app_kit::{
        NSFileManagerNSWorkspaceAuthorization, NSWorkspace, NSWorkspaceAuthorization,
        NSWorkspaceAuthorizationType,
    };
    use objc2_foundation::{
        NSError, NSFileManager, NSFileManagerItemReplacementOptions, NSString, NSURL,
    };
    use std::sync::Mutex;

    fn native_error(stage: &'static str, error: &NSError) -> Error {
        Error::MacosReplacement {
            stage,
            domain: error.domain().to_string(),
            code: Some(error.code() as i64),
            // NSError debug description includes its userInfo/underlying error evidence.
            // This text stays in the private probe report, never the public summary.
            message: format!("{error:?}"),
        }
    }
    let complete = Mutex::new(Some(complete));
    let callback = RcBlock::new(
        move |authorization: *mut NSWorkspaceAuthorization, error: *mut NSError| {
            // Cocoa owns both callback arguments for the duration of this invocation.
            let result = unsafe {
                if let Some(error) = error.as_ref() {
                    Err(native_error("authorize", error))
                } else if let Some(authorization) = authorization.as_ref() {
                    let manager = NSFileManager::fileManagerWithAuthorization(authorization);
                    let destination_url = NSURL::fileURLWithPath_isDirectory(
                        &NSString::from_str(&destination.to_string_lossy()),
                        true,
                    );
                    let candidate_url = NSURL::fileURLWithPath_isDirectory(
                        &NSString::from_str(&candidate.to_string_lossy()),
                        true,
                    );
                    match manager.replaceItemAtURL_withItemAtURL_backupItemName_options_resultingItemURL_error(
                        &destination_url, &candidate_url, None,
                        NSFileManagerItemReplacementOptions::UsingNewMetadataOnly, None,
                    ) {
                        Ok(()) => Ok(()),
                        Err(error) => {
                            let original = error.userInfo()
                                .objectForKey(&NSString::from_str("NSFileOriginalItemLocationKey"))
                                .and_then(|value| value.downcast::<NSURL>().ok())
                                .and_then(|url| url.path())
                                .map(|path| PathBuf::from(path.to_string()));
                            Err(recover_after_failure(&destination, original.as_deref(), native_error("replace", &error), |original, destination| {
                                if !destination.exists() {
                                    // Restoration is an ordinary rename, never privilege escalation.
                                    std::fs::rename(original, destination).map_err(Into::into)
                                } else {
                                    let original_url = NSURL::fileURLWithPath_isDirectory(&NSString::from_str(&original.to_string_lossy()), true);
                                    manager.replaceItemAtURL_withItemAtURL_backupItemName_options_resultingItemURL_error(
                                        &destination_url, &original_url, None, NSFileManagerItemReplacementOptions::UsingNewMetadataOnly, None,
                                    ).map_err(|error| native_error("recovery", &error))
                                }
                            }))
                        }
                    }
                } else {
                    Err(failure(
                        "authorize",
                        "authorization returned neither permission nor an error".into(),
                    ))
                }
            };
            if let Some(complete) = complete
                .lock()
                .expect("replacement completion poisoned")
                .take()
            {
                complete(result);
            }
        },
    );
    NSWorkspace::sharedWorkspace().requestAuthorizationOfType_completionHandler(
        NSWorkspaceAuthorizationType::ReplaceFile,
        &callback,
    );
}
