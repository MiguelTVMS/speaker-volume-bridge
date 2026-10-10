// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Default-off acceptance adapter. No entitlement or production registration is added.
use crate::{Error, Result};
use std::path::{Path, PathBuf};

type Completion = Box<dyn FnOnce(Result<()>) + Send>;

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
            message: error.localizedDescription().to_string(),
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
                    let destination = NSURL::fileURLWithPath_isDirectory(
                        &NSString::from_str(&destination.to_string_lossy()),
                        true,
                    );
                    let candidate = NSURL::fileURLWithPath_isDirectory(
                        &NSString::from_str(&candidate.to_string_lossy()),
                        true,
                    );
                    // Preserve the candidate's signed bundle metadata. Apple ignores
                    // backup/options for an authorized manager; no shell fallback.
                    manager.replaceItemAtURL_withItemAtURL_backupItemName_options_resultingItemURL_error(
                    &destination, &candidate, None,
                    NSFileManagerItemReplacementOptions::UsingNewMetadataOnly, None,
                ).map_err(|error| native_error("replace", &error))
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
