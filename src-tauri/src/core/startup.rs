//! OS startup integration (CC Switch parity: launchOnStartup).

use crate::core::AppError;
use std::env;
use std::path::PathBuf;

const RUN_VALUE: &str = "GrokSwitch";

/// Absolute path of the running executable.
pub fn current_exe() -> Result<PathBuf, AppError> {
    env::current_exe().map_err(|e| AppError::Message(format!("current_exe: {e}")))
}

/// Whether the app is registered to launch at user login.
pub fn is_launch_on_startup() -> bool {
    #[cfg(windows)]
    {
        read_run_value().is_some()
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// Enable or disable launch-at-login for the current executable.
pub fn set_launch_on_startup(enabled: bool) -> Result<bool, AppError> {
    #[cfg(windows)]
    {
        if enabled {
            let exe = current_exe()?;
            // Quote path; append --silent so silent_startup can take effect later.
            let cmd = format!("\"{}\" --silent", exe.display());
            write_run_value(&cmd)?;
            Ok(true)
        } else {
            delete_run_value()?;
            Ok(false)
        }
    }
    #[cfg(not(windows))]
    {
        let _ = enabled;
        Err(AppError::Message(
            "launch on startup is only implemented on Windows".into(),
        ))
    }
}

#[cfg(windows)]
fn run_key() -> Result<winreg::RegKey, AppError> {
    use winreg::enums::*;
    use winreg::RegKey;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    hkcu.open_subkey_with_flags(
        r"Software\Microsoft\Windows\CurrentVersion\Run",
        KEY_READ | KEY_WRITE,
    )
    .map_err(|e| AppError::Message(format!("open Run key: {e}")))
}

#[cfg(windows)]
fn read_run_value() -> Option<String> {
    let key = run_key().ok()?;
    key.get_value::<String, _>(RUN_VALUE).ok()
}

#[cfg(windows)]
fn write_run_value(cmd: &str) -> Result<(), AppError> {
    let key = run_key()?;
    key.set_value(RUN_VALUE, &cmd)
        .map_err(|e| AppError::Message(format!("set Run value: {e}")))
}

#[cfg(windows)]
fn delete_run_value() -> Result<(), AppError> {
    let key = run_key()?;
    match key.delete_value(RUN_VALUE) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(AppError::Message(format!("delete Run value: {e}"))),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn current_exe_resolves() {
        assert!(super::current_exe().is_ok());
    }
}
