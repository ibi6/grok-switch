use crate::core::AppError;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Resolved filesystem layout for Grok Switch + Grok CLI.
#[derive(Debug, Clone)]
pub struct Paths {
    pub home: PathBuf,
    pub app_home: PathBuf,
    pub grok_home: PathBuf,
    pub config_toml: PathBuf,
    pub auth_json: PathBuf,
    pub providers_json: PathBuf,
    pub settings_json: PathBuf,
    pub activity_jsonl: PathBuf,
    pub accounts_dir: PathBuf,
    pub backups_dir: PathBuf,
    pub skill_backups_dir: PathBuf,
    pub grok_skills_dir: PathBuf,
    pub ccswitch_db: PathBuf,
    pub ccswitch_skills_dir: PathBuf,
    pub claude_skills_dir: PathBuf,
}

impl Paths {
    /// Resolve paths from the real user home directory.
    pub fn resolve() -> Result<Self, AppError> {
        let home = dirs::home_dir().ok_or_else(|| {
            AppError::Message("could not resolve user home directory".into())
        })?;
        Ok(Self::from_root(home))
    }

    /// Build paths under an arbitrary root (user home or tempfile for tests).
    pub fn from_root(root: impl AsRef<Path>) -> Self {
        let home = root.as_ref().to_path_buf();
        let app_home = home.join(".grok-switch");
        let grok_home = home.join(".grok");
        Self {
            home: home.clone(),
            app_home: app_home.clone(),
            grok_home: grok_home.clone(),
            config_toml: grok_home.join("config.toml"),
            auth_json: grok_home.join("auth.json"),
            providers_json: app_home.join("providers.json"),
            settings_json: app_home.join("settings.json"),
            activity_jsonl: app_home.join("activity.jsonl"),
            accounts_dir: app_home.join("accounts"),
            backups_dir: app_home.join("backups"),
            skill_backups_dir: app_home.join("skill-backups"),
            grok_skills_dir: grok_home.join("skills"),
            ccswitch_db: home.join(".cc-switch").join("cc-switch.db"),
            ccswitch_skills_dir: home.join(".cc-switch").join("skills"),
            claude_skills_dir: home.join(".claude").join("skills"),
        }
    }

    /// Create app_home, accounts, and backups directories if missing.
    pub fn ensure_app_dirs(&self) -> Result<(), AppError> {
        fs::create_dir_all(&self.app_home)?;
        fs::create_dir_all(&self.accounts_dir)?;
        fs::create_dir_all(&self.backups_dir)?;
        fs::create_dir_all(&self.skill_backups_dir)?;
        Ok(())
    }

    /// Directory for a managed Grok user skill: `~/.grok/skills/<name>/`.
    pub fn skill_dir(&self, name: &str) -> PathBuf {
        self.grok_skills_dir.join(name)
    }

    /// Path to a skill's SKILL.md.
    pub fn skill_md(&self, name: &str) -> PathBuf {
        self.skill_dir(name).join("SKILL.md")
    }

    /// Override Grok home / config / auth paths from settings (if non-empty).
    /// App data (providers, settings) stays under `app_home`.
    pub fn with_grok_home(&self, grok_home: impl AsRef<Path>) -> Self {
        let mut next = self.clone();
        let gh = grok_home.as_ref();
        if gh.as_os_str().is_empty() {
            return next;
        }
        next.grok_home = gh.to_path_buf();
        next.config_toml = next.grok_home.join("config.toml");
        next.auth_json = next.grok_home.join("auth.json");
        next
    }

    /// Path to accounts index: `accounts/index.json`.
    pub fn accounts_index(&self) -> PathBuf {
        self.accounts_dir.join("index.json")
    }

    /// Directory for a single account: `accounts/<id>/`.
    pub fn account_dir(&self, id: &str) -> PathBuf {
        self.accounts_dir.join(id)
    }

    /// Account meta file: `accounts/<id>/meta.json`.
    pub fn account_meta(&self, id: &str) -> PathBuf {
        self.account_dir(id).join("meta.json")
    }

    /// Account auth blob: `accounts/<id>/auth.json`.
    pub fn account_auth(&self, id: &str) -> PathBuf {
        self.account_dir(id).join("auth.json")
    }
}

/// Atomically write bytes via temp file + rename (same directory).
///
/// `std::fs::rename` atomically replaces an existing destination on both
/// Windows and Unix, so we rename straight over the target. We deliberately do
/// NOT `remove_file` the destination first: that would open a window where the
/// file is missing, and a crash/power-loss in that window would lose the
/// original (auth.json / config.toml / providers.json) with only a `.tmp` left.
///
/// The temp name is unique per write so two writers targeting the same file
/// cannot clobber each other's staging file.
pub fn atomic_write(path: &Path, bytes: impl AsRef<[u8]>) -> Result<(), AppError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let file_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("file");
    let tmp_path =
        path.with_file_name(format!(".{file_name}.{}.tmp", uuid::Uuid::new_v4().simple()));

    // Write + flush + fsync so the bytes are durable before we swap them in.
    let write_result = (|| -> std::io::Result<()> {
        let mut file = fs::File::create(&tmp_path)?;
        file.write_all(bytes.as_ref())?;
        file.flush()?;
        file.sync_all()
    })();
    if let Err(e) = write_result {
        let _ = fs::remove_file(&tmp_path);
        return Err(e.into());
    }

    // Atomic replace. On failure, clean up the temp so it does not accumulate.
    if let Err(e) = fs::rename(&tmp_path, path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(e.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_root_layout() {
        let root = PathBuf::from("C:\\fake-home");
        let p = Paths::from_root(&root);
        assert_eq!(p.home, root);
        assert_eq!(p.app_home, root.join(".grok-switch"));
        assert_eq!(p.grok_home, root.join(".grok"));
        assert_eq!(p.config_toml, root.join(".grok").join("config.toml"));
        assert_eq!(p.auth_json, root.join(".grok").join("auth.json"));
        assert_eq!(
            p.providers_json,
            root.join(".grok-switch").join("providers.json")
        );
        assert_eq!(
            p.settings_json,
            root.join(".grok-switch").join("settings.json")
        );
        assert_eq!(
            p.activity_jsonl,
            root.join(".grok-switch").join("activity.jsonl")
        );
        assert_eq!(p.accounts_dir, root.join(".grok-switch").join("accounts"));
        assert_eq!(p.backups_dir, root.join(".grok-switch").join("backups"));
        assert_eq!(
            p.skill_backups_dir,
            root.join(".grok-switch").join("skill-backups")
        );
        assert_eq!(p.grok_skills_dir, root.join(".grok").join("skills"));
        assert_eq!(
            p.ccswitch_db,
            root.join(".cc-switch").join("cc-switch.db")
        );
        assert_eq!(
            p.ccswitch_skills_dir,
            root.join(".cc-switch").join("skills")
        );
        assert_eq!(p.claude_skills_dir, root.join(".claude").join("skills"));
    }

    #[test]
    fn atomic_write_creates_and_overwrites() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("nested").join("data.json");

        // Creates parent dirs + file.
        atomic_write(&target, b"first").unwrap();
        assert_eq!(fs::read_to_string(&target).unwrap(), "first");

        // Overwrites an existing destination (must not fail on Windows).
        atomic_write(&target, b"second").unwrap();
        assert_eq!(fs::read_to_string(&target).unwrap(), "second");

        // No stray temp files left behind.
        let leftovers: Vec<_> = fs::read_dir(target.parent().unwrap())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "temp files leaked: {leftovers:?}");
    }

    #[test]
    fn ensure_app_dirs_creates() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::from_root(dir.path());
        paths.ensure_app_dirs().unwrap();
        assert!(paths.app_home.is_dir());
        assert!(paths.accounts_dir.is_dir());
        assert!(paths.backups_dir.is_dir());
    }

    #[test]
    fn with_grok_home_overrides_only_grok_paths() {
        let root = PathBuf::from("C:\\fake-home");
        let p = Paths::from_root(&root);
        let custom = PathBuf::from("D:\\custom-grok");
        let o = p.with_grok_home(&custom);
        assert_eq!(o.grok_home, custom);
        assert_eq!(o.config_toml, custom.join("config.toml"));
        assert_eq!(o.auth_json, custom.join("auth.json"));
        // App data stays put
        assert_eq!(o.app_home, p.app_home);
        assert_eq!(o.providers_json, p.providers_json);
        assert_eq!(o.backups_dir, p.backups_dir);
    }
}
