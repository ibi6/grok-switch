use crate::core::paths::{atomic_write, Paths};
use crate::core::types::{AppMode, Settings, Theme};
use crate::core::AppError;
use std::fs;

/// Default settings derived from a Paths layout.
pub fn default_settings(paths: &Paths) -> Settings {
    Settings {
        grok_home: paths.grok_home.to_string_lossy().into_owned(),
        grok_executable: paths
            .grok_home
            .join("bin")
            .join("grok.exe")
            .to_string_lossy()
            .into_owned(),
        current_mode: AppMode::None,
        current_provider_id: None,
        current_account_id: None,
        official_default_model: "grok-build".into(),
        auto_backup: true,
        auto_health_check: true,
        launch_on_startup: false,
        theme: Theme::Dark,
        tray_enabled: true,
        proxy_enabled: false,
        proxy_port: 18765,
        pool_strategy: crate::core::types::PoolStrategy::Priority,
    }
}

/// Apply only user-editable preference fields from `incoming` onto the
/// currently persisted settings. Runtime pointers (`current_*`) always come
/// from disk so a stale Settings form cannot clobber tray / switch state.
pub fn merge_user_settings(current: &Settings, incoming: &Settings) -> Settings {
    Settings {
        grok_home: incoming.grok_home.clone(),
        grok_executable: incoming.grok_executable.clone(),
        official_default_model: incoming.official_default_model.clone(),
        auto_backup: incoming.auto_backup,
        auto_health_check: incoming.auto_health_check,
        launch_on_startup: incoming.launch_on_startup,
        theme: incoming.theme,
        tray_enabled: incoming.tray_enabled,
        proxy_enabled: incoming.proxy_enabled,
        proxy_port: if incoming.proxy_port == 0 {
            18765
        } else {
            incoming.proxy_port
        },
        pool_strategy: incoming.pool_strategy,
        // Preserve live switch state.
        current_mode: current.current_mode,
        current_provider_id: current.current_provider_id.clone(),
        current_account_id: current.current_account_id.clone(),
    }
}

/// Load settings from disk, or return defaults when the file is missing.
pub fn load_settings(paths: &Paths) -> Result<Settings, AppError> {
    if !paths.settings_json.exists() {
        return Ok(default_settings(paths));
    }
    let raw = fs::read_to_string(&paths.settings_json)?;
    let settings: Settings = serde_json::from_str(&raw)?;
    Ok(settings)
}

/// Persist settings with atomic write.
pub fn save_settings(paths: &Paths, settings: &Settings) -> Result<(), AppError> {
    let _guard = crate::core::lock_store();
    paths.ensure_app_dirs()?;
    let json = serde_json::to_string_pretty(settings)?;
    atomic_write(&paths.settings_json, json)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_defaults_and_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::from_root(dir.path());
        paths.ensure_app_dirs().unwrap();

        let loaded = load_settings(&paths).unwrap();
        assert_eq!(loaded.current_mode, AppMode::None);
        assert_eq!(loaded.official_default_model, "grok-build");
        assert!(loaded.auto_backup);
        assert!(loaded.auto_health_check);
        assert!(!loaded.launch_on_startup);
        assert_eq!(loaded.theme, Theme::Dark);
        assert!(loaded.tray_enabled);
        assert!(loaded.grok_home.ends_with(".grok"));
        assert!(loaded.grok_executable.contains("grok.exe"));

        let mut updated = loaded;
        updated.theme = Theme::Light;
        updated.current_mode = AppMode::Provider;
        updated.current_provider_id = Some("p1".into());
        save_settings(&paths, &updated).unwrap();

        let again = load_settings(&paths).unwrap();
        assert_eq!(again.theme, Theme::Light);
        assert_eq!(again.current_mode, AppMode::Provider);
        assert_eq!(again.current_provider_id.as_deref(), Some("p1"));
    }
}
