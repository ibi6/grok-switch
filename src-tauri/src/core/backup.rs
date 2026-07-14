use crate::core::paths::{atomic_write, Paths};
use crate::core::AppError;
use chrono::Local;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

const DEFAULT_KEEP: usize = 30;

/// Metadata written next to backed-up Grok files.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BackupMeta {
    pub reason: String,
    pub created_at: i64,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub extra: HashMap<String, String>,
}

/// Create a timestamped backup of `config.toml` and `auth.json` when present.
///
/// Layout: `backups/<yyyyMMdd-HHmmss>/` with optional files + `meta.json`.
/// Returns the backup id (directory name).
pub fn create_backup(
    paths: &Paths,
    reason: &str,
    meta: Option<HashMap<String, String>>,
) -> Result<String, AppError> {
    paths.ensure_app_dirs()?;

    let backup_id = unique_backup_id(&paths.backups_dir)?;
    let dir = paths.backups_dir.join(&backup_id);
    fs::create_dir_all(&dir)?;

    copy_if_exists(&paths.config_toml, &dir.join("config.toml"))?;
    copy_if_exists(&paths.auth_json, &dir.join("auth.json"))?;

    let backup_meta = BackupMeta {
        reason: reason.to_string(),
        created_at: Local::now().timestamp(),
        extra: meta.unwrap_or_default(),
    };
    let json = serde_json::to_string_pretty(&backup_meta)?;
    atomic_write(&dir.join("meta.json"), json)?;

    Ok(backup_id)
}

/// Restore `config.toml` / `auth.json` from a backup into the Grok home.
/// Only files present in the backup are written (missing backup files are skipped).
pub fn restore_backup(paths: &Paths, backup_id: &str) -> Result<(), AppError> {
    let dir = paths.backups_dir.join(backup_id);
    if !dir.is_dir() {
        return Err(AppError::NotFound(format!("backup not found: {backup_id}")));
    }

    fs::create_dir_all(&paths.grok_home)?;

    let cfg_src = dir.join("config.toml");
    if cfg_src.is_file() {
        let bytes = fs::read(&cfg_src)?;
        atomic_write(&paths.config_toml, bytes)?;
    }

    let auth_src = dir.join("auth.json");
    if auth_src.is_file() {
        let bytes = fs::read(&auth_src)?;
        atomic_write(&paths.auth_json, bytes)?;
    }

    Ok(())
}

/// Keep the newest `keep` backup directories; delete older ones (FIFO by id name).
/// Default keep is 30 when callers pass that value.
pub fn prune_backups(paths: &Paths, keep: usize) -> Result<usize, AppError> {
    let keep = if keep == 0 { DEFAULT_KEEP } else { keep };
    let mut ids = list_backup_ids(paths)?;
    if ids.len() <= keep {
        return Ok(0);
    }
    // ids sorted ascending (oldest first); drop from the front.
    let remove_count = ids.len() - keep;
    let mut removed = 0usize;
    for id in ids.drain(0..remove_count) {
        let dir = paths.backups_dir.join(&id);
        if dir.is_dir() {
            fs::remove_dir_all(&dir)?;
            removed += 1;
        }
    }
    Ok(removed)
}

/// List backup directory names, oldest first (lexicographic on timestamp ids).
pub fn list_backup_ids(paths: &Paths) -> Result<Vec<String>, AppError> {
    if !paths.backups_dir.exists() {
        return Ok(Vec::new());
    }
    let mut ids = Vec::new();
    for entry in fs::read_dir(&paths.backups_dir)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            if let Some(name) = entry.file_name().to_str() {
                ids.push(name.to_string());
            }
        }
    }
    ids.sort();
    Ok(ids)
}

/// Read meta.json for a backup if present.
pub fn read_backup_meta(paths: &Paths, backup_id: &str) -> Result<Option<BackupMeta>, AppError> {
    let path = paths.backups_dir.join(backup_id).join("meta.json");
    if !path.is_file() {
        return Ok(None);
    }
    let raw = fs::read_to_string(path)?;
    Ok(Some(serde_json::from_str(&raw)?))
}

fn unique_backup_id(backups_dir: &Path) -> Result<String, AppError> {
    let base = Local::now().format("%Y%m%d-%H%M%S").to_string();
    let candidate = backups_dir.join(&base);
    if !candidate.exists() {
        return Ok(base);
    }
    // Same-second collision: append millis.
    let millis = Local::now().timestamp_millis() % 1000;
    let with_ms = format!("{base}-{millis:03}");
    if !backups_dir.join(&with_ms).exists() {
        return Ok(with_ms);
    }
    // Extremely unlikely: fall back to uuid suffix.
    Ok(format!("{base}-{}", uuid::Uuid::new_v4().simple()))
}

fn copy_if_exists(src: &Path, dest: &Path) -> Result<(), AppError> {
    if src.is_file() {
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(src, dest)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    fn setup() -> (tempfile::TempDir, Paths) {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::from_root(dir.path());
        paths.ensure_app_dirs().unwrap();
        (dir, paths)
    }

    fn write_grok_files(paths: &Paths, config: &str, auth: &str) {
        fs::create_dir_all(&paths.grok_home).unwrap();
        fs::write(&paths.config_toml, config).unwrap();
        fs::write(&paths.auth_json, auth).unwrap();
    }

    #[test]
    fn create_backup_copies_existing_files_and_meta() {
        let (_tmp, paths) = setup();
        write_grok_files(&paths, "default = \"x\"\n", r#"{"token":"abc"}"#);

        let mut extra = HashMap::new();
        extra.insert("mode".into(), "provider".into());
        extra.insert("providerId".into(), "p1".into());

        let id = create_backup(&paths, "switch_provider", Some(extra)).unwrap();
        let dir = paths.backups_dir.join(&id);
        assert!(dir.is_dir());
        assert_eq!(
            fs::read_to_string(dir.join("config.toml")).unwrap(),
            "default = \"x\"\n"
        );
        assert_eq!(
            fs::read_to_string(dir.join("auth.json")).unwrap(),
            r#"{"token":"abc"}"#
        );

        let meta = read_backup_meta(&paths, &id).unwrap().unwrap();
        assert_eq!(meta.reason, "switch_provider");
        assert_eq!(meta.extra.get("mode").map(String::as_str), Some("provider"));
        assert_eq!(meta.extra.get("providerId").map(String::as_str), Some("p1"));
    }

    #[test]
    fn create_backup_skips_missing_files() {
        let (_tmp, paths) = setup();
        fs::create_dir_all(&paths.grok_home).unwrap();
        fs::write(&paths.config_toml, "only-config\n").unwrap();
        // no auth.json

        let id = create_backup(&paths, "partial", None).unwrap();
        let dir = paths.backups_dir.join(&id);
        assert!(dir.join("config.toml").is_file());
        assert!(!dir.join("auth.json").exists());
        assert!(dir.join("meta.json").is_file());
    }

    #[test]
    fn restore_overwrites_grok_files() {
        let (_tmp, paths) = setup();
        write_grok_files(&paths, "old-config\n", r#"{"v":1}"#);
        let id = create_backup(&paths, "before_change", None).unwrap();

        // Mutate live files
        fs::write(&paths.config_toml, "new-config\n").unwrap();
        fs::write(&paths.auth_json, r#"{"v":2}"#).unwrap();

        restore_backup(&paths, &id).unwrap();
        assert_eq!(fs::read_to_string(&paths.config_toml).unwrap(), "old-config\n");
        assert_eq!(fs::read_to_string(&paths.auth_json).unwrap(), r#"{"v":1}"#);
    }

    #[test]
    fn restore_missing_backup_errors() {
        let (_tmp, paths) = setup();
        let err = restore_backup(&paths, "no-such-backup").unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn prune_keeps_newest() {
        let (_tmp, paths) = setup();
        write_grok_files(&paths, "c\n", "{}");

        // Create distinct backup dirs by writing known ids (bypass clock).
        for i in 0..5 {
            let id = format!("20260101-12000{i}");
            let dir = paths.backups_dir.join(&id);
            fs::create_dir_all(&dir).unwrap();
            fs::write(dir.join("meta.json"), format!(r#"{{"reason":"t{i}","createdAt":{i}}}"#))
                .unwrap();
        }

        let removed = prune_backups(&paths, 3).unwrap();
        assert_eq!(removed, 2);
        let ids = list_backup_ids(&paths).unwrap();
        assert_eq!(ids, vec![
            "20260101-120002".to_string(),
            "20260101-120003".to_string(),
            "20260101-120004".to_string(),
        ]);
    }

    #[test]
    fn create_then_prune_integration() {
        let (_tmp, paths) = setup();
        write_grok_files(&paths, "c\n", "{}");

        // Use direct dirs for deterministic count; also exercise create once.
        let _ = create_backup(&paths, "live", None).unwrap();
        thread::sleep(Duration::from_millis(5));

        for i in 0..3 {
            let id = format!("20250101-00000{i}");
            fs::create_dir_all(paths.backups_dir.join(&id)).unwrap();
        }

        let before = list_backup_ids(&paths).unwrap().len();
        assert!(before >= 4);
        let removed = prune_backups(&paths, 2).unwrap();
        assert_eq!(removed, before - 2);
        assert_eq!(list_backup_ids(&paths).unwrap().len(), 2);
    }
}
