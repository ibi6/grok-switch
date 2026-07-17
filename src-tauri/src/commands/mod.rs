//! Tauri command surface + pure orchestration over `core` modules.

use crate::core::account_store;
use crate::core::activity;
use crate::core::auth_vault;
use crate::core::backup::{self, BackupMeta};
use crate::core::ccswitch_import::{self, ImportCandidate};
use crate::core::cli_status::{self, CliStatus};
use crate::core::config_writer;
use crate::core::health::{self, HealthResult};
use crate::core::paths::Paths;
use crate::core::provider_store;
use crate::core::settings_store;
use crate::core::skill_store::{self, SkillDetail, SkillDraft, SkillInfo, SkillScope};
use crate::core::terminal;
use crate::core::types::{
    Account, AccountStatus, Activity, ActivityType, ApiBackend, AppMode, Provider, Settings,
};
use crate::core::validate_model_token;
use crate::core::AppError;
use crate::AppState;
use chrono::Local;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::State;

// ─── ApiResult ───────────────────────────────────────────────────────────────

/// Structured response for every Tauri command.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiResult<T> {
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl<T> ApiResult<T> {
    pub fn ok(data: T) -> Self {
        Self {
            ok: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn err(msg: impl Into<String>) -> Self {
        Self {
            ok: false,
            data: None,
            error: Some(msg.into()),
        }
    }

    pub fn from_result(result: Result<T, AppError>) -> Self {
        match result {
            Ok(data) => Self::ok(data),
            Err(e) => Self::err(e.to_string()),
        }
    }
}

// ─── DTOs ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderDraft {
    pub base_url: String,
    pub api_key: String,
    pub api_backend: ApiBackend,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupInfo {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<BackupMeta>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnableProviderResult {
    pub provider_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backup_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub health: Option<HealthResult>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub post_health: Option<HealthResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnableAccountResult {
    pub account_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backup_id: Option<String>,
}

// ─── Pure orchestration (testable without Tauri) ─────────────────────────────

/// Resolve operational Paths, honoring settings.grok_home when set.
fn ops_paths(base: &Paths) -> Paths {
    match settings_store::load_settings(base) {
        Ok(s) if !s.grok_home.trim().is_empty() => base.with_grok_home(s.grok_home.trim()),
        _ => base.clone(),
    }
}

/// Write provider models into config.toml (selected + full catalog).
fn write_provider_config(
    ops: &Paths,
    provider: &Provider,
    catalog: &[Provider],
) -> Result<(), AppError> {
    if let Some(parent) = ops.config_toml.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let config_text = if ops.config_toml.is_file() {
        config_writer::read_config(&ops.config_toml)?
    } else {
        String::new()
    };
    let next =
        config_writer::apply_provider_with_catalog(&config_text, provider, Some(catalog))?;
    config_writer::write_config(&ops.config_toml, &next)
}

/// Re-apply the currently active provider into config after an edit (no health gate).
pub fn reapply_active_provider_config(base: &Paths, provider: &Provider) -> Result<(), AppError> {
    let ops = ops_paths(base);
    let catalog = provider_store::list_providers(base).unwrap_or_default();
    write_provider_config(&ops, provider, &catalog)?;
    log_activity(
        base,
        ActivityType::SwitchProvider,
        &format!("Re-applied active provider {}", provider.name),
        Some(HashMap::from([
            ("providerId".into(), provider.id.clone()),
            ("providerName".into(), provider.name.clone()),
            ("reason".into(), "upsert_active".into()),
        ])),
    );
    Ok(())
}

/// Enable a relay provider: optional health → backup → write config → settings → activity.
pub fn enable_provider_flow(
    paths: &Paths,
    id: &str,
    force: bool,
) -> Result<EnableProviderResult, AppError> {
    let provider = provider_store::get_provider(paths, id)?
        .ok_or_else(|| AppError::NotFound(format!("provider not found: {id}")))?;
    config_writer::validate_provider(&provider)?;

    let mut settings = settings_store::load_settings(paths)?;
    let ops = ops_paths(paths);
    let model = resolve_provider_model(&provider);

    let mut pre_health = None;
    if settings.auto_health_check && !force {
        let result = health::check_provider(
            &provider.base_url,
            &provider.api_key,
            provider.api_backend,
            &model,
        );
        if !result.ok {
            return Err(AppError::Message(format!(
                "NEEDS_FORCE: health check failed: {}",
                result.detail
            )));
        }
        pre_health = Some(result);
    }

    let mut backup_id = None;
    if settings.auto_backup {
        let mut extra = HashMap::new();
        extra.insert("mode".into(), "provider".into());
        extra.insert("providerId".into(), id.to_string());
        // Backup against the effective Grok home.
        let bid = backup::create_backup(&ops, "switch_provider", Some(extra))?;
        let _ = backup::prune_backups(&ops, 30);
        backup_id = Some(bid);
    }

    let catalog = provider_store::list_providers(paths).unwrap_or_default();
    if let Err(e) = write_provider_config(&ops, &provider, &catalog) {
        if let Some(ref bid) = backup_id {
            let _ = backup::restore_backup(&ops, bid);
        }
        return Err(e);
    }

    settings.current_mode = AppMode::Provider;
    settings.current_provider_id = Some(id.to_string());
    // Keep account id for later restore, but mode is provider.
    settings_store::save_settings(paths, &settings)?;

    log_activity(
        paths,
        ActivityType::SwitchProvider,
        &format!("Switched to provider {}", provider.name),
        Some(HashMap::from([
            ("providerId".into(), id.to_string()),
            ("providerName".into(), provider.name.clone()),
        ])),
    );

    // Post-check reuses the pre-switch probe when we have one: it targets the
    // same endpoint with the same inputs, so re-probing would only burn a second
    // (possibly metered) request. Only probe here when pre was skipped (forced).
    let post_health = match &pre_health {
        Some(pre) => Some(pre.clone()),
        None if settings.auto_health_check => {
            let result = health::check_provider(
                &provider.base_url,
                &provider.api_key,
                provider.api_backend,
                &model,
            );
            if !result.ok {
                log_activity(
                    paths,
                    ActivityType::Health,
                    &format!("Post-switch health soft-fail: {}", result.detail),
                    Some(HashMap::from([("providerId".into(), id.to_string())])),
                );
            }
            Some(result)
        }
        None => None,
    };

    Ok(EnableProviderResult {
        provider_id: id.to_string(),
        backup_id,
        health: pre_health,
        post_health,
    })
}

/// Enable an official account snapshot.
pub fn enable_account_flow(
    paths: &Paths,
    id: &str,
) -> Result<EnableAccountResult, AppError> {
    let mut account = account_store::get_account(paths, id)?
        .ok_or_else(|| AppError::NotFound(format!("account not found: {id}")))?;

    if !paths.account_auth(id).is_file() {
        return Err(AppError::NotFound(format!(
            "account auth not found: {id}"
        )));
    }

    let mut settings = settings_store::load_settings(paths)?;
    let ops = ops_paths(paths);

    let mut backup_id = None;
    if settings.auto_backup {
        let mut extra = HashMap::new();
        extra.insert("mode".into(), "official".into());
        extra.insert("accountId".into(), id.to_string());
        let bid = backup::create_backup(&ops, "switch_account", Some(extra))?;
        let _ = backup::prune_backups(&ops, 30);
        backup_id = Some(bid);
    }

    // enable_auth copies vault → ops.auth_json (must use ops paths)
    if let Err(e) = auth_vault::enable_auth_at(&ops, paths, id) {
        if let Some(ref bid) = backup_id {
            let _ = backup::restore_backup(&ops, bid);
        }
        return Err(e);
    }

    let config_text = if ops.config_toml.is_file() {
        config_writer::read_config(&ops.config_toml)?
    } else {
        String::new()
    };
    let next = config_writer::apply_official_default(&config_text, &settings.official_default_model)?;
    if let Err(e) = config_writer::write_config(&ops.config_toml, &next) {
        if let Some(ref bid) = backup_id {
            let _ = backup::restore_backup(&ops, bid);
        }
        return Err(e);
    }

    settings.current_mode = AppMode::Official;
    settings.current_account_id = Some(id.to_string());
    settings_store::save_settings(paths, &settings)?;

    account.status = AccountStatus::Active;
    account.last_used_at = Some(Local::now().timestamp());
    account_store::save_account_meta(paths, account.clone())?;

    // Mark other accounts non-active if they were Active.
    if let Ok(all) = account_store::list_accounts(paths) {
        for mut other in all {
            if other.id != id && other.status == AccountStatus::Active {
                other.status = AccountStatus::Ready;
                let _ = account_store::save_account_meta(paths, other);
            }
        }
    }

    log_activity(
        paths,
        ActivityType::SwitchAccount,
        &format!("Switched to account {}", account.name),
        Some(HashMap::from([
            ("accountId".into(), id.to_string()),
            ("accountName".into(), account.name),
        ])),
    );

    Ok(EnableAccountResult {
        account_id: id.to_string(),
        backup_id,
    })
}

fn resolve_provider_model(provider: &Provider) -> String {
    provider
        .models
        .iter()
        .find(|m| m.id == provider.default_model_entry_id)
        .map(|m| m.model.clone())
        .or_else(|| provider.models.first().map(|m| m.model.clone()))
        .unwrap_or_else(|| "grok-build".into())
}

fn log_activity(
    paths: &Paths,
    activity_type: ActivityType,
    message: &str,
    meta: Option<HashMap<String, String>>,
) {
    let entry = Activity {
        ts: Local::now().timestamp(),
        activity_type,
        message: message.to_string(),
        meta,
    };
    let _ = activity::append_activity(paths, &entry);
}

fn list_backup_infos(paths: &Paths) -> Result<Vec<BackupInfo>, AppError> {
    // Backups always live under app_home; a grok_home override moves config/auth
    // but not the backups dir, so a single listing is authoritative.
    let mut ids = backup::list_backup_ids(paths)?;
    ids.sort();
    ids.reverse(); // newest first
    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        let meta = backup::read_backup_meta(paths, &id).ok().flatten();
        out.push(BackupInfo {
            id,
            reason: meta.as_ref().map(|m| m.reason.clone()),
            created_at: meta.as_ref().map(|m| m.created_at),
            meta,
        });
    }
    Ok(out)
}

fn import_apply_flow(paths: &Paths, ids: &[String]) -> Result<Vec<Provider>, AppError> {
    let candidates = ccswitch_import::preview_ccswitch(&paths.ccswitch_db)?;
    let selected: Vec<ImportCandidate> = candidates
        .into_iter()
        .filter(|c| ids.iter().any(|id| id == &c.id))
        .collect();
    if selected.is_empty() {
        return Err(AppError::Invalid(
            "no matching import candidates for given ids".into(),
        ));
    }
    let existing = provider_store::list_providers(paths)?;
    let selected = ccswitch_import::dedup_candidates(selected, &existing);
    if selected.is_empty() {
        return Err(AppError::Invalid(
            "all selected providers already exist (dedup)".into(),
        ));
    }
    let providers = ccswitch_import::candidates_to_providers(&selected);
    for p in &providers {
        provider_store::upsert_provider(paths, p.clone())?;
    }
    log_activity(
        paths,
        ActivityType::Import,
        &format!("Imported {} provider(s) from CC Switch", providers.len()),
        Some(HashMap::from([(
            "count".into(),
            providers.len().to_string(),
        )])),
    );
    Ok(providers)
}

fn capture_account_flow(paths: &Paths, name: &str) -> Result<Account, AppError> {
    let id = uuid::Uuid::new_v4().to_string();
    let ops = ops_paths(paths);
    let account = auth_vault::capture_auth_from(&ops, paths, &id, name)?;
    log_activity(
        paths,
        ActivityType::CaptureAccount,
        &format!("Captured account {}", account.name),
        Some(HashMap::from([("accountId".into(), account.id.clone())])),
    );
    Ok(account)
}

fn restore_backup_flow(paths: &Paths, backup_id: &str) -> Result<(), AppError> {
    let ops = ops_paths(paths);
    backup::restore_backup(&ops, backup_id)?;

    // After restore, mode is unknown — clear active pointers so UI does not lie.
    if let Ok(mut settings) = settings_store::load_settings(paths) {
        settings.current_mode = AppMode::None;
        settings.current_provider_id = None;
        settings.current_account_id = None;
        let _ = settings_store::save_settings(paths, &settings);
    }

    log_activity(
        paths,
        ActivityType::Restore,
        &format!("Restored backup {backup_id}"),
        Some(HashMap::from([("backupId".into(), backup_id.to_string())])),
    );
    Ok(())
}

fn test_provider_by_id(paths: &Paths, id: &str) -> Result<HealthResult, AppError> {
    let provider = provider_store::get_provider(paths, id)?
        .ok_or_else(|| AppError::NotFound(format!("provider not found: {id}")))?;
    let model = resolve_provider_model(&provider);
    let result = health::check_provider(
        &provider.base_url,
        &provider.api_key,
        provider.api_backend,
        &model,
    );
    log_activity(
        paths,
        ActivityType::Health,
        &format!(
            "Health check {}: {}",
            provider.name,
            if result.ok { "ok" } else { "fail" }
        ),
        Some(HashMap::from([
            ("providerId".into(), id.to_string()),
            ("ok".into(), result.ok.to_string()),
        ])),
    );
    Ok(result)
}

fn test_provider_draft_flow(draft: &ProviderDraft) -> HealthResult {
    health::check_provider(
        &draft.base_url,
        &draft.api_key,
        draft.api_backend,
        &draft.model,
    )
}

// ─── Tauri commands ──────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> ApiResult<Settings> {
    ApiResult::from_result(settings_store::load_settings(&state.paths))
}

#[tauri::command]
pub fn update_settings(state: State<'_, AppState>, settings: Settings) -> ApiResult<Settings> {
    // official_default_model ends up on a shell command line via open_grok_terminal.
    if let Err(e) = validate_model_token(&settings.official_default_model, "officialDefaultModel") {
        return ApiResult::err(e.to_string());
    }
    match settings_store::save_settings(&state.paths, &settings) {
        Ok(()) => ApiResult::ok(settings),
        Err(e) => ApiResult::err(e.to_string()),
    }
}

#[tauri::command]
pub fn list_providers(state: State<'_, AppState>) -> ApiResult<Vec<Provider>> {
    ApiResult::from_result(provider_store::list_providers(&state.paths))
}

#[tauri::command]
pub fn upsert_provider(state: State<'_, AppState>, provider: Provider) -> ApiResult<Provider> {
    if let Err(e) = config_writer::validate_provider(&provider) {
        return ApiResult::err(e.to_string());
    }
    match provider_store::upsert_provider(&state.paths, provider.clone()) {
        Ok(()) => {
            // If this is the active provider, rewrite config.toml immediately so
            // key/url/model edits take effect without a manual re-enable.
            if let Ok(settings) = settings_store::load_settings(&state.paths) {
                if settings.current_mode == AppMode::Provider
                    && settings.current_provider_id.as_deref() == Some(provider.id.as_str())
                {
                    if let Err(e) = reapply_active_provider_config(&state.paths, &provider) {
                        return ApiResult::err(format!(
                            "provider saved, but failed to re-apply active config: {e}"
                        ));
                    }
                }
            }
            ApiResult::ok(provider)
        }
        Err(e) => ApiResult::err(e.to_string()),
    }
}

#[tauri::command]
pub fn delete_provider(state: State<'_, AppState>, id: String) -> ApiResult<bool> {
    match provider_store::delete_provider(&state.paths, &id) {
        Ok(removed) => {
            if removed {
                if let Ok(mut settings) = settings_store::load_settings(&state.paths) {
                    if settings.current_provider_id.as_deref() == Some(id.as_str()) {
                        settings.current_provider_id = None;
                        if settings.current_mode == AppMode::Provider {
                            settings.current_mode = AppMode::None;
                        }
                        let _ = settings_store::save_settings(&state.paths, &settings);
                    }
                }
            }
            ApiResult::ok(removed)
        }
        Err(e) => ApiResult::err(e.to_string()),
    }
}

#[tauri::command]
pub fn enable_provider(
    state: State<'_, AppState>,
    id: String,
    force: Option<bool>,
) -> ApiResult<EnableProviderResult> {
    ApiResult::from_result(enable_provider_flow(
        &state.paths,
        &id,
        force.unwrap_or(false),
    ))
}

#[tauri::command]
pub fn test_provider(state: State<'_, AppState>, id: String) -> ApiResult<HealthResult> {
    ApiResult::from_result(test_provider_by_id(&state.paths, &id))
}

#[tauri::command]
pub fn test_provider_draft(draft: ProviderDraft) -> ApiResult<HealthResult> {
    ApiResult::ok(test_provider_draft_flow(&draft))
}

#[tauri::command]
pub fn list_accounts(state: State<'_, AppState>) -> ApiResult<Vec<Account>> {
    ApiResult::from_result(account_store::list_accounts(&state.paths))
}

#[tauri::command]
pub fn delete_account(state: State<'_, AppState>, id: String) -> ApiResult<bool> {
    match account_store::delete_account_dir(&state.paths, &id) {
        Ok(removed) => {
            if removed {
                if let Ok(mut settings) = settings_store::load_settings(&state.paths) {
                    if settings.current_account_id.as_deref() == Some(id.as_str()) {
                        settings.current_account_id = None;
                        if settings.current_mode == AppMode::Official {
                            settings.current_mode = AppMode::None;
                        }
                        let _ = settings_store::save_settings(&state.paths, &settings);
                    }
                }
            }
            ApiResult::ok(removed)
        }
        Err(e) => ApiResult::err(e.to_string()),
    }
}

#[tauri::command]
pub fn capture_current_account(
    state: State<'_, AppState>,
    name: String,
) -> ApiResult<Account> {
    ApiResult::from_result(capture_account_flow(&state.paths, &name))
}

#[tauri::command]
pub fn enable_account(
    state: State<'_, AppState>,
    id: String,
) -> ApiResult<EnableAccountResult> {
    ApiResult::from_result(enable_account_flow(&state.paths, &id))
}

#[tauri::command]
pub fn import_ccswitch_preview(state: State<'_, AppState>) -> ApiResult<Vec<ImportCandidate>> {
    ApiResult::from_result(ccswitch_import::preview_ccswitch(&state.paths.ccswitch_db))
}

#[tauri::command]
pub fn import_ccswitch_apply(
    state: State<'_, AppState>,
    ids: Vec<String>,
) -> ApiResult<Vec<Provider>> {
    ApiResult::from_result(import_apply_flow(&state.paths, &ids))
}

#[tauri::command]
pub fn get_cli_status(state: State<'_, AppState>) -> ApiResult<CliStatus> {
    ApiResult::from_result(cli_status::get_cli_status(&state.paths))
}

#[tauri::command]
pub fn list_activity(
    state: State<'_, AppState>,
    limit: Option<usize>,
) -> ApiResult<Vec<Activity>> {
    let limit = limit.unwrap_or(50);
    ApiResult::from_result(activity::list_activity(&state.paths, limit))
}

#[tauri::command]
pub fn list_backups(state: State<'_, AppState>) -> ApiResult<Vec<BackupInfo>> {
    ApiResult::from_result(list_backup_infos(&state.paths))
}

#[tauri::command]
pub fn restore_backup(state: State<'_, AppState>, id: String) -> ApiResult<()> {
    ApiResult::from_result(restore_backup_flow(&state.paths, &id))
}

/// Open a system terminal running `grok` (optionally with `-m <model>`).
/// Model is whitelist-validated in Rust; no shell plugin is used.
#[tauri::command]
pub fn open_grok_terminal(
    state: State<'_, AppState>,
    model: Option<String>,
) -> ApiResult<String> {
    ApiResult::from_result(terminal::open_grok_terminal(
        &state.paths,
        model.as_deref(),
    ))
}

#[tauri::command]
pub fn list_skills(state: State<'_, AppState>) -> ApiResult<Vec<SkillInfo>> {
    ApiResult::from_result(skill_store::list_skills(&state.paths))
}

#[tauri::command]
pub fn get_skill(state: State<'_, AppState>, name: String) -> ApiResult<SkillDetail> {
    ApiResult::from_result(skill_store::get_skill(&state.paths, &name))
}

#[tauri::command]
pub fn upsert_skill(state: State<'_, AppState>, draft: SkillDraft) -> ApiResult<SkillDetail> {
    match skill_store::upsert_skill(&state.paths, &draft) {
        Ok(detail) => {
            log_activity(
                &state.paths,
                ActivityType::Skill,
                &format!("Saved skill {}", detail.info.name),
                Some(HashMap::from([("skill".into(), detail.info.name.clone())])),
            );
            ApiResult::ok(detail)
        }
        Err(e) => ApiResult::err(e.to_string()),
    }
}

#[tauri::command]
pub fn delete_skill(state: State<'_, AppState>, name: String) -> ApiResult<bool> {
    match skill_store::delete_skill(&state.paths, &name) {
        Ok(removed) => {
            if removed {
                log_activity(
                    &state.paths,
                    ActivityType::Skill,
                    &format!("Deleted skill {name}"),
                    Some(HashMap::from([("skill".into(), name)])),
                );
            }
            ApiResult::ok(removed)
        }
        Err(e) => ApiResult::err(e.to_string()),
    }
}

#[tauri::command]
pub fn import_skills(
    state: State<'_, AppState>,
    names: Vec<String>,
    source: Option<String>,
) -> ApiResult<Vec<SkillInfo>> {
    let scope = match source.as_deref().unwrap_or("cc-switch") {
        "claude" => SkillScope::Claude,
        "cc-switch" | "ccswitch" => SkillScope::CcSwitch,
        other => {
            return ApiResult::err(format!("unknown skill import source: {other}"));
        }
    };
    match skill_store::import_skills(&state.paths, &names, scope) {
        Ok(items) => {
            log_activity(
                &state.paths,
                ActivityType::Skill,
                &format!("Imported {} skill(s) from {scope:?}", items.len()),
                Some(HashMap::from([("count".into(), items.len().to_string())])),
            );
            ApiResult::ok(items)
        }
        Err(e) => ApiResult::err(e.to_string()),
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::{ModelEntry, ProviderSource, Theme};
    use std::fs;

    fn setup() -> (tempfile::TempDir, Paths) {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::from_root(dir.path());
        paths.ensure_app_dirs().unwrap();
        fs::create_dir_all(&paths.grok_home).unwrap();
        (dir, paths)
    }

    fn sample_provider(id: &str) -> Provider {
        Provider {
            id: id.into(),
            name: format!("Provider {id}"),
            base_url: "https://api.example.com/v1".into(),
            api_key: "sk-test-key-abcdefghijklmnop".into(),
            api_backend: ApiBackend::ChatCompletions,
            default_model_entry_id: "m1".into(),
            models: vec![ModelEntry {
                id: "m1".into(),
                model: "grok-4.5".into(),
                name: "Grok 4.5".into(),
                context_window: Some(200_000),
                api_backend: None,
            }],
            extra_headers: None,
            context_window: 200_000,
            website_url: None,
            notes: None,
            source: ProviderSource::Manual,
            created_at: 1,
            updated_at: 2,
        }
    }

    #[test]
    fn enable_provider_writes_config_and_settings_without_health() {
        let (_tmp, paths) = setup();
        // Disable auto health so we never hit the network.
        let mut settings = settings_store::default_settings(&paths);
        settings.auto_health_check = false;
        settings.auto_backup = true;
        settings_store::save_settings(&paths, &settings).unwrap();

        fs::write(
            &paths.config_toml,
            r#"
[models]
default = "old"

[model.user-custom]
model = "keep"
"#,
        )
        .unwrap();

        provider_store::upsert_provider(&paths, sample_provider("p1")).unwrap();

        let result = enable_provider_flow(&paths, "p1", false).unwrap();
        assert_eq!(result.provider_id, "p1");
        assert!(result.backup_id.is_some());

        let cfg = fs::read_to_string(&paths.config_toml).unwrap();
        assert!(cfg.contains("[model.gs-m1]"));
        assert!(cfg.contains("gs-m1"));
        assert!(cfg.contains("[model.user-custom]"));
        assert!(
            cfg.contains("models_base_url") && cfg.contains("https://api.example.com/v1"),
            "enable must set endpoints.models_base_url for Grok CLI relay routing; got:\n{cfg}"
        );

        let settings = settings_store::load_settings(&paths).unwrap();
        assert_eq!(settings.current_mode, AppMode::Provider);
        assert_eq!(settings.current_provider_id.as_deref(), Some("p1"));

        let acts = activity::list_activity(&paths, 10).unwrap();
        assert!(!acts.is_empty());
        assert_eq!(acts[0].activity_type, ActivityType::SwitchProvider);
    }

    #[test]
    fn delete_provider_clears_current_settings() {
        let (_tmp, paths) = setup();
        let mut settings = settings_store::default_settings(&paths);
        settings.auto_health_check = false;
        settings.auto_backup = false;
        settings_store::save_settings(&paths, &settings).unwrap();
        provider_store::upsert_provider(&paths, sample_provider("p1")).unwrap();
        enable_provider_flow(&paths, "p1", true).unwrap();

        // Simulate command-layer cleanup after delete
        assert!(provider_store::delete_provider(&paths, "p1").unwrap());
        let mut settings = settings_store::load_settings(&paths).unwrap();
        if settings.current_provider_id.as_deref() == Some("p1") {
            settings.current_provider_id = None;
            if settings.current_mode == AppMode::Provider {
                settings.current_mode = AppMode::None;
            }
            settings_store::save_settings(&paths, &settings).unwrap();
        }
        let settings = settings_store::load_settings(&paths).unwrap();
        assert_eq!(settings.current_mode, AppMode::None);
        assert!(settings.current_provider_id.is_none());
    }

    #[test]
    fn enable_provider_missing_returns_not_found() {
        let (_tmp, paths) = setup();
        let err = enable_provider_flow(&paths, "nope", true).unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn enable_provider_needs_force_on_health_fail() {
        let (_tmp, paths) = setup();
        let mut settings = settings_store::default_settings(&paths);
        settings.auto_health_check = true;
        settings.auto_backup = false;
        settings_store::save_settings(&paths, &settings).unwrap();

        let mut p = sample_provider("bad");
        // Unreachable host — health should fail quickly-ish.
        p.base_url = "http://127.0.0.1:1".into();
        provider_store::upsert_provider(&paths, p).unwrap();

        let err = enable_provider_flow(&paths, "bad", false).unwrap_err();
        assert!(
            err.to_string().starts_with("NEEDS_FORCE:"),
            "got: {err}"
        );

        // force=true should proceed despite health fail
        let result = enable_provider_flow(&paths, "bad", true).unwrap();
        assert_eq!(result.provider_id, "bad");
        let settings = settings_store::load_settings(&paths).unwrap();
        assert_eq!(settings.current_mode, AppMode::Provider);
    }

    #[test]
    fn enable_account_copies_auth_and_sets_official() {
        let (_tmp, paths) = setup();
        let mut settings = settings_store::default_settings(&paths);
        settings.auto_backup = true;
        settings.official_default_model = "grok-build".into();
        settings_store::save_settings(&paths, &settings).unwrap();

        fs::write(&paths.auth_json, r#"{"email":"a@b.com","token":"t1"}"#).unwrap();
        fs::write(
            &paths.config_toml,
            r#"
[models]
default = "gs-old"
"#,
        )
        .unwrap();

        let account = auth_vault::capture_auth(&paths, "acc1", "Work").unwrap();
        assert_eq!(account.id, "acc1");

        // Change live auth so enable must restore vault copy.
        fs::write(&paths.auth_json, r#"{"token":"live-other"}"#).unwrap();

        let result = enable_account_flow(&paths, "acc1").unwrap();
        assert_eq!(result.account_id, "acc1");
        assert!(result.backup_id.is_some());

        assert_eq!(
            fs::read_to_string(&paths.auth_json).unwrap(),
            r#"{"email":"a@b.com","token":"t1"}"#
        );
        let cfg = fs::read_to_string(&paths.config_toml).unwrap();
        assert!(
            cfg.contains("default = \"grok-build\"") || cfg.contains("default = 'grok-build'")
        );

        let settings = settings_store::load_settings(&paths).unwrap();
        assert_eq!(settings.current_mode, AppMode::Official);
        assert_eq!(settings.current_account_id.as_deref(), Some("acc1"));

        let acc = account_store::get_account(&paths, "acc1").unwrap().unwrap();
        assert_eq!(acc.status, AccountStatus::Active);
    }

    #[test]
    fn api_result_serde_shape() {
        let ok = ApiResult::ok(42);
        let json = serde_json::to_string(&ok).unwrap();
        assert!(json.contains("\"ok\":true"));
        assert!(json.contains("\"data\":42"));

        let err: ApiResult<i32> = ApiResult::err("NEEDS_FORCE: x");
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("\"ok\":false"));
        assert!(json.contains("NEEDS_FORCE"));
    }

    #[test]
    fn settings_update_roundtrip_via_store() {
        let (_tmp, paths) = setup();
        let mut s = settings_store::default_settings(&paths);
        s.theme = Theme::Light;
        s.auto_backup = false;
        settings_store::save_settings(&paths, &s).unwrap();
        let loaded = settings_store::load_settings(&paths).unwrap();
        assert_eq!(loaded.theme, Theme::Light);
        assert!(!loaded.auto_backup);
    }
}
