use crate::core::paths::Paths;
use crate::core::settings_store;
use crate::core::AppError;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;
use std::time::Duration;

/// CLI / Grok home status for the dashboard.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CliStatus {
    pub found: bool,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub config_ok: bool,
    pub auth_present: bool,
}

/// Probe whether the Grok CLI binary, config, and auth look usable.
pub fn get_cli_status(paths: &Paths) -> Result<CliStatus, AppError> {
    let settings = settings_store::load_settings(paths)?;
    // Honor custom grok home from settings when probing config/auth.
    let ops = if settings.grok_home.trim().is_empty() {
        paths.clone()
    } else {
        paths.with_grok_home(settings.grok_home.trim())
    };

    let exe = if settings.grok_executable.trim().is_empty() {
        default_executable(&ops)
    } else {
        settings.grok_executable.clone()
    };

    let found = Path::new(&exe).is_file();
    let version = if found {
        probe_version(&exe)
    } else {
        None
    };

    let config_ok = if ops.config_toml.is_file() {
        match std::fs::read_to_string(&ops.config_toml) {
            Ok(raw) => raw.parse::<toml_edit::DocumentMut>().is_ok(),
            Err(_) => false,
        }
    } else {
        // Missing config is not a parse error; treat as ok for first-run.
        true
    };

    let auth_present = ops.auth_json.is_file();

    Ok(CliStatus {
        found,
        path: exe,
        version,
        config_ok,
        auth_present,
    })
}

fn default_executable(paths: &Paths) -> String {
    paths
        .grok_home
        .join("bin")
        .join(if cfg!(windows) {
            "grok.exe"
        } else {
            "grok"
        })
        .to_string_lossy()
        .into_owned()
}

/// Try `grok -v` then `grok version` with a short timeout. Best-effort.
fn probe_version(exe: &str) -> Option<String> {
    for args in [["-v"].as_slice(), ["version"].as_slice()] {
        if let Some(v) = run_version_cmd(exe, args) {
            return Some(v);
        }
    }
    None
}

fn run_version_cmd(exe: &str, args: &[&str]) -> Option<String> {
    use std::io::Read;

    let mut child = Command::new(exe)
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .ok()?;

    let start = std::time::Instant::now();
    let timeout = Duration::from_secs(5);
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(_) => return None,
        }
    };

    let mut stdout = String::new();
    if let Some(mut out) = child.stdout.take() {
        let _ = out.read_to_string(&mut stdout);
    }
    let mut stderr = String::new();
    if let Some(mut err) = child.stderr.take() {
        let _ = err.read_to_string(&mut stderr);
    }

    if !status.success() && stdout.trim().is_empty() && stderr.trim().is_empty() {
        return None;
    }
    let text = if !stdout.trim().is_empty() {
        stdout
    } else {
        stderr
    };
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    // First line only, capped.
    let line = text.lines().next().unwrap_or(text).trim();
    let clipped: String = line.chars().take(120).collect();
    Some(clipped)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn status_missing_binary_reports_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::from_root(dir.path());
        paths.ensure_app_dirs().unwrap();

        let status = get_cli_status(&paths).unwrap();
        assert!(!status.found);
        assert!(status.config_ok); // missing config ok
        assert!(!status.auth_present);
        assert!(status.version.is_none());
    }

    #[test]
    fn status_detects_config_and_auth() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::from_root(dir.path());
        paths.ensure_app_dirs().unwrap();
        fs::create_dir_all(&paths.grok_home).unwrap();
        fs::write(&paths.config_toml, "models = {}\n").unwrap();
        fs::write(&paths.auth_json, r#"{"token":"x"}"#).unwrap();

        // Fake executable path that exists
        let fake_exe = paths.grok_home.join("bin").join("grok.exe");
        fs::create_dir_all(fake_exe.parent().unwrap()).unwrap();
        fs::write(&fake_exe, b"not-a-real-binary").unwrap();

        let mut settings = settings_store::default_settings(&paths);
        settings.grok_executable = fake_exe.to_string_lossy().into_owned();
        settings_store::save_settings(&paths, &settings).unwrap();

        let status = get_cli_status(&paths).unwrap();
        assert!(status.found);
        assert!(status.config_ok);
        assert!(status.auth_present);
        // version probe will fail (not a real binary) — ok
    }

    #[test]
    fn status_invalid_config() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::from_root(dir.path());
        paths.ensure_app_dirs().unwrap();
        fs::create_dir_all(&paths.grok_home).unwrap();
        fs::write(&paths.config_toml, "[[[broken").unwrap();

        let status = get_cli_status(&paths).unwrap();
        assert!(!status.config_ok);
    }
}
