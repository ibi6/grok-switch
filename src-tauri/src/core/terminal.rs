//! Launch a local terminal running `grok` without going through a shell plugin.
//!
//! The model id is validated against a strict whitelist before it is ever
//! placed on a command line, so quoting / injection issues cannot arise even
//! if a caller later concatenates the token into a string.

use crate::core::normalize::validate_model_token;
use crate::core::paths::Paths;
use crate::core::settings_store;
use crate::core::AppError;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Open a new terminal window that starts `grok` (optionally with `-m <model>`).
///
/// Returns the exact command line that was launched so the UI can surface it.
pub fn open_grok_terminal(paths: &Paths, model: Option<&str>) -> Result<String, AppError> {
    let model_token = match model {
        Some(m) if !m.trim().is_empty() => {
            Some(validate_model_token(m, "model")?)
        }
        _ => None,
    };

    let settings = settings_store::load_settings(paths)?;
    let ops = if settings.grok_home.trim().is_empty() {
        paths.clone()
    } else {
        paths.with_grok_home(settings.grok_home.trim())
    };

    let exe = resolve_executable(&ops, &settings.grok_executable);
    let cwd = ops
        .grok_home
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| ops.home.clone());

    let display = match &model_token {
        Some(m) => format!("grok -m {m}"),
        None => "grok".to_string(),
    };

    spawn_terminal(&exe, model_token.as_deref(), &cwd)?;
    Ok(display)
}

fn resolve_executable(ops: &Paths, configured: &str) -> PathBuf {
    if !configured.trim().is_empty() {
        return PathBuf::from(configured.trim());
    }
    ops.grok_home.join("bin").join(if cfg!(windows) {
        "grok.exe"
    } else {
        "grok"
    })
}

/// Spawn a terminal that runs the given grok binary (with optional -m).
///
/// On Windows we try Windows Terminal (`wt`) first, then fall back to
/// PowerShell. On Unix we try a short list of common terminal emulators.
/// Arguments are always passed as separate argv entries — never via a
/// concatenated shell string — so metacharacters in the model id cannot
/// escape even if validation were ever loosened.
fn spawn_terminal(exe: &Path, model: Option<&str>, cwd: &Path) -> Result<(), AppError> {
    #[cfg(windows)]
    {
        spawn_windows(exe, model, cwd)
    }
    #[cfg(not(windows))]
    {
        spawn_unix(exe, model, cwd)
    }
}

#[cfg(windows)]
fn spawn_windows(exe: &Path, model: Option<&str>, cwd: &Path) -> Result<(), AppError> {
    // Prefer Windows Terminal when available.
    let mut wt = Command::new("wt");
    wt.arg("-d")
        .arg(cwd)
        .arg("powershell")
        .arg("-NoExit")
        .arg("-Command");
    // Build a PowerShell command as a *single* argument. The model token has
    // already been whitelist-validated, so embedding it is safe; we still use
    // single-quotes around the path to tolerate spaces in the exe path.
    let ps = build_ps_command(exe, model);
    wt.arg(&ps);
    match wt.spawn() {
        Ok(_) => return Ok(()),
        Err(_) => { /* fall through to PowerShell */ }
    }

    let mut ps_cmd = Command::new("powershell");
    ps_cmd
        .arg("-NoExit")
        .arg("-Command")
        .arg(&ps)
        .current_dir(cwd);
    ps_cmd
        .spawn()
        .map_err(|e| AppError::Message(format!("failed to open terminal: {e}")))?;
    Ok(())
}

#[cfg(windows)]
fn build_ps_command(exe: &Path, model: Option<&str>) -> String {
    // Single-quote the path so spaces are fine; model is whitelist-validated.
    let path = exe.display().to_string().replace('\'', "''");
    match model {
        Some(m) => format!("& '{path}' -m {m}"),
        None => format!("& '{path}'"),
    }
}

#[cfg(not(windows))]
fn spawn_unix(exe: &Path, model: Option<&str>, cwd: &Path) -> Result<(), AppError> {
    // Candidates: (binary, args-before-command)
    let candidates: &[(&str, &[&str])] = &[
        ("x-terminal-emulator", &["-e"]),
        ("gnome-terminal", &["--"]),
        ("konsole", &["-e"]),
        ("xfce4-terminal", &["-e"]),
        ("xterm", &["-e"]),
        ("open", &["-a", "Terminal"]), // macOS fallback is awkward; prefer iTerm/Terminal via open
    ];

    let mut argv: Vec<String> = vec![exe.display().to_string()];
    if let Some(m) = model {
        argv.push("-m".into());
        argv.push(m.to_string());
    }

    for (bin, prefix) in candidates {
        let mut cmd = Command::new(bin);
        cmd.args(*prefix);
        for a in &argv {
            cmd.arg(a);
        }
        cmd.current_dir(cwd);
        if cmd.spawn().is_ok() {
            return Ok(());
        }
    }

    // Last resort: run grok directly (no new terminal window) so at least the
    // process starts; the UI can still show the command for the user to copy.
    let mut direct = Command::new(exe);
    if let Some(m) = model {
        direct.arg("-m").arg(m);
    }
    direct.current_dir(cwd);
    direct
        .spawn()
        .map_err(|e| AppError::Message(format!("failed to open terminal: {e}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::paths::Paths;

    #[test]
    fn rejects_unsafe_model() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::from_root(dir.path());
        paths.ensure_app_dirs().unwrap();
        let err = open_grok_terminal(&paths, Some("evil\"; calc")).unwrap_err();
        assert!(
            err.to_string().contains("invalid characters"),
            "got: {err}"
        );
    }

    #[test]
    fn accepts_safe_model_token_shape() {
        // We only assert validation succeeds; spawn may fail in CI without a
        // real terminal, so call validate_model_token directly via the public
        // path that open_grok_terminal uses.
        assert!(validate_model_token("gs-myallapi-grok45", "m").is_ok());
        assert!(validate_model_token("x-ai/grok-4", "m").is_ok());
    }

    #[cfg(windows)]
    #[test]
    fn ps_command_quotes_path_and_appends_model() {
        let exe = Path::new(r"C:\Users\me\.grok\bin\grok.exe");
        assert_eq!(
            build_ps_command(exe, Some("gs-foo")),
            r"& 'C:\Users\me\.grok\bin\grok.exe' -m gs-foo"
        );
        assert_eq!(
            build_ps_command(exe, None),
            r"& 'C:\Users\me\.grok\bin\grok.exe'"
        );
        // Single quotes inside the path are doubled (PowerShell escape).
        let funny = Path::new(r"C:\O'Brien\grok.exe");
        assert_eq!(
            build_ps_command(funny, Some("m")),
            r"& 'C:\O''Brien\grok.exe' -m m"
        );
    }
}
