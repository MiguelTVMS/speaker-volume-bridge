// Copyright 2019-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Test-only diagnostic orchestration used by the actual macOS installer.
use crate::{Error, Result};
use serde_json::Value;

pub fn authorization_script(command: &str) -> String {
    format!("try\n{command}\nreturn {{true, 0, \"\"}}\non error nativeMessage number nativeNumber\nreturn {{false, nativeNumber, nativeMessage}}\nend try")
}

pub fn run_authorization(
    dispatch: impl FnOnce(Box<dyn FnOnce() + Send + Sync>) -> std::result::Result<(), String>,
    execute: impl FnOnce() -> std::result::Result<Value, (&'static str, String)> + Send + Sync + 'static,
) -> Result<()> {
    let (tx, rx) = std::sync::mpsc::channel();
    dispatch(Box::new(move || {
        let _ = tx.send(execute());
    }))
    .map_err(|message| Error::MacosAuthorization {
        stage: "dispatch",
        code: None,
        message,
    })?;
    let result = rx.recv().map_err(|error| Error::MacosAuthorization {
        stage: "receive",
        code: None,
        message: error.to_string(),
    })?;
    match result {
        Ok(Value::Array(values)) if values.first() == Some(&Value::Bool(true)) => Ok(()),
        Ok(Value::Array(values)) if values.first() == Some(&Value::Bool(false)) => {
            Err(Error::MacosAuthorization {
                stage: "execute",
                code: values.get(1).and_then(Value::as_i64),
                message: values
                    .get(2)
                    .and_then(Value::as_str)
                    .unwrap_or("invalid native error response")
                    .to_owned(),
            })
        }
        Ok(_) => Err(Error::MacosAuthorization {
            stage: "decode",
            code: None,
            message: "invalid native authorization response".into(),
        }),
        Err((stage, message)) => Err(Error::MacosAuthorization {
            stage,
            code: None,
            message,
        }),
    }
}

#[cfg(target_os = "macos")]
pub fn execute_script(source: &str) -> std::result::Result<Value, (&'static str, String)> {
    let mut script = osakit::Script::new_from_source(osakit::Language::AppleScript, source);
    script
        .compile()
        .map_err(|error| ("compile", error.to_string()))?;
    script
        .execute()
        .map_err(|error| ("execute", error.to_string()))
}
