use crate::core::account_store;
use crate::core::paths::{atomic_write, Paths};
use crate::core::types::{Account, AccountStatus};
use crate::core::AppError;
use chrono::Local;
use std::fs;

/// Capture the current Grok `auth.json` into the account vault.
///
/// Requires `~/.grok/auth.json` (under paths) to exist; otherwise returns an error.
/// Writes `accounts/<id>/auth.json`, persists meta via account_store, and returns the Account.
pub fn capture_auth(paths: &Paths, account_id: &str, name: &str) -> Result<Account, AppError> {
    if account_id.trim().is_empty() {
        return Err(AppError::Invalid("account_id must not be empty".into()));
    }
    if name.trim().is_empty() {
        return Err(AppError::Invalid("account name must not be empty".into()));
    }
    if !paths.auth_json.is_file() {
        return Err(AppError::NotFound(format!(
            "auth.json not found at {}",
            paths.auth_json.display()
        )));
    }

    paths.ensure_app_dirs()?;
    let dir = paths.account_dir(account_id);
    fs::create_dir_all(&dir)?;

    let bytes = fs::read(&paths.auth_json)?;
    atomic_write(&paths.account_auth(account_id), bytes)?;

    let account = Account {
        id: account_id.to_string(),
        name: name.to_string(),
        email: extract_email_hint(&paths.auth_json).ok().flatten(),
        label_color: "#3b82f6".into(),
        status: AccountStatus::Ready,
        last_used_at: None,
        created_at: Local::now().timestamp(),
        priority: 0,
        weight: 100,
        pool_enabled: true,
        cooldown_until: None,
    };
    account_store::save_account_meta(paths, account.clone())?;
    Ok(account)
}

/// Copy vaulted `accounts/<id>/auth.json` into the live Grok `auth.json`.
pub fn enable_auth(paths: &Paths, account_id: &str) -> Result<(), AppError> {
    enable_auth_at(paths, paths, account_id)
}

/// Copy vault auth from `vault_paths` into live Grok home under `ops`.
///
/// Use when settings.grok_home differs from the default layout: vault stays
/// under app data (`vault_paths`), while the active CLI auth is written to `ops`.
pub fn enable_auth_at(
    ops: &Paths,
    vault_paths: &Paths,
    account_id: &str,
) -> Result<(), AppError> {
    let vault = vault_paths.account_auth(account_id);
    if !vault.is_file() {
        return Err(AppError::NotFound(format!(
            "account auth not found: {account_id}"
        )));
    }
    fs::create_dir_all(&ops.grok_home)?;
    let bytes = fs::read(&vault)?;
    atomic_write(&ops.auth_json, bytes)?;
    Ok(())
}

/// Capture live Grok auth from `ops` into the account vault under `vault_paths`.
pub fn capture_auth_from(
    ops: &Paths,
    vault_paths: &Paths,
    account_id: &str,
    name: &str,
) -> Result<Account, AppError> {
    if account_id.trim().is_empty() {
        return Err(AppError::Invalid("account_id must not be empty".into()));
    }
    if name.trim().is_empty() {
        return Err(AppError::Invalid("account name must not be empty".into()));
    }
    if !ops.auth_json.is_file() {
        return Err(AppError::NotFound(format!(
            "auth.json not found at {}",
            ops.auth_json.display()
        )));
    }

    vault_paths.ensure_app_dirs()?;
    let dir = vault_paths.account_dir(account_id);
    fs::create_dir_all(&dir)?;

    let bytes = fs::read(&ops.auth_json)?;
    atomic_write(&vault_paths.account_auth(account_id), bytes)?;

    let account = Account {
        id: account_id.to_string(),
        name: name.to_string(),
        email: extract_email_hint(&ops.auth_json).ok().flatten(),
        label_color: "#3b82f6".into(),
        status: AccountStatus::Ready,
        last_used_at: None,
        created_at: Local::now().timestamp(),
        priority: 0,
        weight: 100,
        pool_enabled: true,
        cooldown_until: None,
    };
    account_store::save_account_meta(vault_paths, account.clone())?;
    Ok(account)
}

/// Best-effort email extraction from a Grok auth blob (optional field).
fn extract_email_hint(auth_path: &std::path::Path) -> Result<Option<String>, AppError> {
    let raw = fs::read_to_string(auth_path)?;
    let value: serde_json::Value = serde_json::from_str(&raw)?;
    let email = value
        .get("email")
        .or_else(|| value.pointer("/user/email"))
        .or_else(|| value.pointer("/account/email"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    Ok(email)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::account_store::{get_account, list_accounts};

    fn setup() -> (tempfile::TempDir, Paths) {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::from_root(dir.path());
        paths.ensure_app_dirs().unwrap();
        (dir, paths)
    }

    #[test]
    fn capture_requires_auth_json() {
        let (_tmp, paths) = setup();
        let err = capture_auth(&paths, "acc1", "Work").unwrap_err();
        assert!(err.to_string().contains("auth.json"));
    }

    #[test]
    fn capture_copies_and_indexes_account() {
        let (_tmp, paths) = setup();
        fs::create_dir_all(&paths.grok_home).unwrap();
        fs::write(
            &paths.auth_json,
            r#"{"email":"user@x.com","token":"secret"}"#,
        )
        .unwrap();

        let account = capture_auth(&paths, "acc1", "Work").unwrap();
        assert_eq!(account.id, "acc1");
        assert_eq!(account.name, "Work");
        assert_eq!(account.email.as_deref(), Some("user@x.com"));
        assert_eq!(account.status, AccountStatus::Ready);
        assert!(paths.account_auth("acc1").is_file());
        assert_eq!(
            fs::read_to_string(paths.account_auth("acc1")).unwrap(),
            r#"{"email":"user@x.com","token":"secret"}"#
        );

        let listed = list_accounts(&paths).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, "acc1");
        assert!(get_account(&paths, "acc1").unwrap().is_some());
    }

    #[test]
    fn enable_auth_copies_vault_to_grok() {
        let (_tmp, paths) = setup();
        fs::create_dir_all(&paths.grok_home).unwrap();
        fs::write(&paths.auth_json, r#"{"token":"live"}"#).unwrap();
        capture_auth(&paths, "acc1", "Work").unwrap();

        // Change live auth, then re-enable vaulted copy.
        fs::write(&paths.auth_json, r#"{"token":"changed"}"#).unwrap();
        enable_auth(&paths, "acc1").unwrap();
        assert_eq!(
            fs::read_to_string(&paths.auth_json).unwrap(),
            r#"{"token":"live"}"#
        );
    }

    #[test]
    fn enable_missing_vault_errors() {
        let (_tmp, paths) = setup();
        let err = enable_auth(&paths, "missing").unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn capture_rejects_empty_ids() {
        let (_tmp, paths) = setup();
        fs::create_dir_all(&paths.grok_home).unwrap();
        fs::write(&paths.auth_json, "{}").unwrap();
        assert!(capture_auth(&paths, "", "n").is_err());
        assert!(capture_auth(&paths, "id", "").is_err());
    }
}
