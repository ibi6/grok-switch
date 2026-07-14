use crate::core::error::AppError;
use crate::core::normalize::{normalize_base_url, sanitize_model_name};
use crate::core::types::{ApiBackend, ModelEntry, Provider, ProviderSource};
use chrono::Local;
use rusqlite::{Connection, OpenFlags};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

/// Preview row from a CC Switch Claude provider.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportCandidate {
    /// Original cc-switch provider id.
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub api_key: String,
    pub default_model: String,
    pub website_url: Option<String>,
    /// Default for Claude-protocol imports.
    pub suggested_backend: ApiBackend,
}

#[derive(Debug, Deserialize)]
struct SettingsConfig {
    #[serde(default)]
    env: HashMap<String, String>,
}

/// Open a CC Switch SQLite database read-only and list importable Claude providers.
pub fn preview_ccswitch(db_path: &Path) -> Result<Vec<ImportCandidate>, AppError> {
    if !db_path.is_file() {
        return Err(AppError::NotFound(format!(
            "CC Switch database not found at {}",
            db_path.display()
        )));
    }

    // URI path: on Windows use forward slashes for SQLite file URIs.
    let path_str = db_path.display().to_string().replace('\\', "/");
    let uri = format!("file:{path_str}?mode=ro");
    let conn = Connection::open_with_flags(
        &uri,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    )
    .map_err(|e| AppError::Message(format!("failed to open CC Switch DB: {e}")))?;

    preview_from_connection(&conn)
}

/// Query + parse candidates from an open connection (used by tests with in-memory DBs).
pub fn preview_from_connection(conn: &Connection) -> Result<Vec<ImportCandidate>, AppError> {
    let mut stmt = conn
        .prepare(
            "SELECT id, name, settings_config, website_url FROM providers WHERE app_type = 'claude'",
        )
        .map_err(|e| AppError::Message(format!("CC Switch query prepare failed: {e}")))?;

    let rows = stmt
        .query_map([], |row| {
            let id: String = row.get(0)?;
            let name: String = row.get(1)?;
            let settings_config: Option<String> = row.get(2)?;
            let website_url: Option<String> = row.get(3)?;
            Ok((id, name, settings_config, website_url))
        })
        .map_err(|e| AppError::Message(format!("CC Switch query failed: {e}")))?;

    let mut out = Vec::new();
    for row in rows {
        let (id, name, settings_config, website_url) =
            row.map_err(|e| AppError::Message(format!("CC Switch row read failed: {e}")))?;
        if let Some(candidate) = parse_candidate(id, name, settings_config.as_deref(), website_url)
        {
            out.push(candidate);
        }
    }
    Ok(out)
}

fn parse_candidate(
    id: String,
    name: String,
    settings_config: Option<&str>,
    website_url: Option<String>,
) -> Option<ImportCandidate> {
    let raw = settings_config?.trim();
    if raw.is_empty() {
        return None;
    }
    let config: SettingsConfig = serde_json::from_str(raw).ok()?;
    let base = config.env.get("ANTHROPIC_BASE_URL")?.trim();
    let key = config.env.get("ANTHROPIC_AUTH_TOKEN")?.trim();
    if base.is_empty() || key.is_empty() {
        return None;
    }

    let model_raw = config
        .env
        .get("ANTHROPIC_DEFAULT_SONNET_MODEL")
        .map(|s| s.as_str())
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            config
                .env
                .get("ANTHROPIC_MODEL")
                .map(|s| s.as_str())
                .filter(|s| !s.trim().is_empty())
        })
        .unwrap_or("grok-build");

    let website_url = website_url.and_then(|u| {
        let t = u.trim().to_string();
        if t.is_empty() {
            None
        } else {
            Some(t)
        }
    });

    Some(ImportCandidate {
        id,
        name,
        base_url: normalize_base_url(base, true),
        api_key: key.to_string(),
        default_model: sanitize_model_name(model_raw),
        website_url,
        suggested_backend: ApiBackend::Messages,
    })
}

/// Convert selected import candidates into Grok Switch `Provider` records.
pub fn candidates_to_providers(selected: &[ImportCandidate]) -> Vec<Provider> {
    let now = Local::now().timestamp();
    selected
        .iter()
        .map(|c| {
            let entry_id = model_entry_slug(&c.name, &c.default_model);
            let model_entry = ModelEntry {
                id: entry_id.clone(),
                model: c.default_model.clone(),
                name: c.default_model.clone(),
                context_window: Some(200_000),
                api_backend: Some(c.suggested_backend),
            };
            Provider {
                id: uuid::Uuid::new_v4().to_string(),
                name: c.name.clone(),
                base_url: c.base_url.clone(),
                api_key: c.api_key.clone(),
                api_backend: c.suggested_backend,
                default_model_entry_id: entry_id,
                models: vec![model_entry],
                extra_headers: None,
                context_window: 200_000,
                website_url: c.website_url.clone(),
                notes: None,
                source: ProviderSource::CcSwitch,
                created_at: now,
                updated_at: now,
            }
        })
        .collect()
}

/// Filter out candidates already present among existing providers (normalized URL + api key).
pub fn dedup_candidates(
    candidates: Vec<ImportCandidate>,
    existing: &[Provider],
) -> Vec<ImportCandidate> {
    candidates
        .into_iter()
        .filter(|c| {
            !existing.iter().any(|p| {
                normalize_base_url(&p.base_url, true) == normalize_base_url(&c.base_url, true)
                    && p.api_key == c.api_key
            })
        })
        .collect()
}

/// Build a model entry id slug from `{name}-{model}` keeping only `[a-z0-9-]+`.
fn model_entry_slug(name: &str, model: &str) -> String {
    let raw = format!("{name}-{model}").to_lowercase();
    let mut out = String::with_capacity(raw.len());
    let mut last_dash = false;
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    let slug = out.trim_matches('-').to_string();
    if slug.is_empty() {
        "model".into()
    } else {
        slug
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;

    fn setup_memory_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE providers (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                settings_config TEXT,
                website_url TEXT,
                app_type TEXT NOT NULL
            );
            "#,
        )
        .unwrap();
        conn
    }

    fn insert_provider(
        conn: &Connection,
        id: &str,
        name: &str,
        settings_config: &str,
        website_url: Option<&str>,
        app_type: &str,
    ) {
        conn.execute(
            "INSERT INTO providers (id, name, settings_config, website_url, app_type) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, name, settings_config, website_url, app_type],
        )
        .unwrap();
    }

    #[test]
    fn preview_finds_valid_claude_row() {
        let conn = setup_memory_db();
        let settings = r#"{
            "env": {
                "ANTHROPIC_BASE_URL": "https://myallapi.example.com:8443/",
                "ANTHROPIC_AUTH_TOKEN": "sk-test-key-123",
                "ANTHROPIC_DEFAULT_SONNET_MODEL": "grok-4.5[1M]"
            }
        }"#;
        insert_provider(
            &conn,
            "cc-1",
            "MyAllAPI",
            settings,
            Some("https://example.com"),
            "claude",
        );
        // Non-claude should be ignored
        insert_provider(
            &conn,
            "cc-2",
            "Other",
            settings,
            None,
            "codex",
        );
        // Missing token should be skipped
        insert_provider(
            &conn,
            "cc-3",
            "NoToken",
            r#"{"env":{"ANTHROPIC_BASE_URL":"https://x.com"}}"#,
            None,
            "claude",
        );

        let candidates = preview_from_connection(&conn).unwrap();
        assert_eq!(candidates.len(), 1);
        let c = &candidates[0];
        assert_eq!(c.id, "cc-1");
        assert_eq!(c.name, "MyAllAPI");
        assert_eq!(c.base_url, "https://myallapi.example.com:8443/v1");
        assert_eq!(c.api_key, "sk-test-key-123");
        assert_eq!(c.default_model, "grok-4.5"); // sanitize strips [1M]
        assert_eq!(c.website_url.as_deref(), Some("https://example.com"));
        assert_eq!(c.suggested_backend, ApiBackend::Messages);
    }

    #[test]
    fn model_fallback_and_sanitize() {
        let conn = setup_memory_db();
        insert_provider(
            &conn,
            "a",
            "Prov",
            r#"{
                "env": {
                    "ANTHROPIC_BASE_URL": "https://api.example.com",
                    "ANTHROPIC_AUTH_TOKEN": "k",
                    "ANTHROPIC_MODEL": "gpt-5.6-sol[1M]"
                }
            }"#,
            None,
            "claude",
        );
        let c = preview_from_connection(&conn).unwrap();
        assert_eq!(c[0].default_model, "gpt-5.6-sol");
    }

    #[test]
    fn default_model_when_missing() {
        let conn = setup_memory_db();
        insert_provider(
            &conn,
            "a",
            "Prov",
            r#"{
                "env": {
                    "ANTHROPIC_BASE_URL": "https://api.example.com",
                    "ANTHROPIC_AUTH_TOKEN": "k"
                }
            }"#,
            None,
            "claude",
        );
        let c = preview_from_connection(&conn).unwrap();
        assert_eq!(c[0].default_model, "grok-build");
    }

    #[test]
    fn candidates_to_providers_builds_provider() {
        let candidates = vec![ImportCandidate {
            id: "cc-1".into(),
            name: "My All API".into(),
            base_url: "https://api.example.com/v1".into(),
            api_key: "sk-abc".into(),
            default_model: "grok-4.5".into(),
            website_url: Some("https://site.example".into()),
            suggested_backend: ApiBackend::Messages,
        }];
        let providers = candidates_to_providers(&candidates);
        assert_eq!(providers.len(), 1);
        let p = &providers[0];
        assert!(!p.id.is_empty());
        assert_eq!(p.name, "My All API");
        assert_eq!(p.base_url, "https://api.example.com/v1");
        assert_eq!(p.api_key, "sk-abc");
        assert_eq!(p.api_backend, ApiBackend::Messages);
        assert_eq!(p.source, ProviderSource::CcSwitch);
        assert_eq!(p.context_window, 200_000);
        assert_eq!(p.website_url.as_deref(), Some("https://site.example"));
        assert_eq!(p.models.len(), 1);
        assert_eq!(p.default_model_entry_id, p.models[0].id);
        assert_eq!(p.models[0].model, "grok-4.5");
        assert_eq!(p.models[0].context_window, Some(200_000));
        // slug from name-model → [a-z0-9-]+
        assert!(p.models[0]
            .id
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-'));
        assert_eq!(p.models[0].id, "my-all-api-grok-4-5");
    }

    #[test]
    fn dedup_filters_existing_url_and_key() {
        let candidates = vec![
            ImportCandidate {
                id: "1".into(),
                name: "A".into(),
                base_url: "https://api.example.com/v1".into(),
                api_key: "same".into(),
                default_model: "m".into(),
                website_url: None,
                suggested_backend: ApiBackend::Messages,
            },
            ImportCandidate {
                id: "2".into(),
                name: "B".into(),
                base_url: "https://other.example.com/v1".into(),
                api_key: "other".into(),
                default_model: "m".into(),
                website_url: None,
                suggested_backend: ApiBackend::Messages,
            },
        ];
        let existing = vec![Provider {
            id: "existing".into(),
            name: "Old".into(),
            base_url: "https://api.example.com/".into(), // will normalize + /v1
            api_key: "same".into(),
            api_backend: ApiBackend::Messages,
            default_model_entry_id: "x".into(),
            models: vec![],
            extra_headers: None,
            context_window: 200_000,
            website_url: None,
            notes: None,
            source: ProviderSource::Manual,
            created_at: 1,
            updated_at: 1,
        }];
        let filtered = dedup_candidates(candidates, &existing);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, "2");
    }

    #[test]
    fn preview_ccswitch_missing_file() {
        let err = preview_ccswitch(Path::new("C:/definitely/missing/cc-switch.db")).unwrap_err();
        match err {
            AppError::NotFound(_) => {}
            other => panic!("expected NotFound, got {other}"),
        }
    }
}
