use crate::core::normalize::gs_model_key;
use crate::core::paths::atomic_write;
use crate::core::types::{ApiBackend, ModelEntry, Provider};
use crate::core::AppError;
use std::fs;
use std::path::Path;
use toml_edit::{value, DocumentMut, InlineTable, Item, Table, Value};

/// Validate a provider has a usable default model entry.
pub fn validate_provider(provider: &Provider) -> Result<(), AppError> {
    if provider.models.is_empty() {
        return Err(AppError::Invalid(
            "provider has no models; add at least one model entry".into(),
        ));
    }
    if provider.default_model_entry_id.trim().is_empty() {
        return Err(AppError::Invalid(
            "provider default_model_entry_id is empty".into(),
        ));
    }
    if !provider
        .models
        .iter()
        .any(|m| m.id == provider.default_model_entry_id)
    {
        return Err(AppError::Invalid(format!(
            "default_model_entry_id '{}' not found in provider.models",
            provider.default_model_entry_id
        )));
    }
    // Model entry ids become TOML section keys and CLI `-m` flags; reject
    // anything that could break shell quoting or TOML keys.
    for entry in &provider.models {
        crate::core::validate_model_token(&entry.id, "model entry id")?;
        if !entry.model.trim().is_empty() {
            crate::core::validate_model_token(&entry.model, "model id")?;
        }
    }
    crate::core::validate_model_token(
        &provider.default_model_entry_id,
        "default_model_entry_id",
    )?;
    Ok(())
}

/// Rewrite managed `gs-*` model sections for the given provider and set default.
///
/// When `catalog` is provided, all providers' models are written (so `grok models`
/// can list them), while `[models].default` and `endpoints.models_base_url` still
/// follow the selected `provider`.
///
/// Also sets `[endpoints].models_base_url` so Grok CLI routes catalog/auth
/// through the relay (without this, CLI may still hit api.x.ai).
pub fn apply_provider(config_text: &str, provider: &Provider) -> Result<String, AppError> {
    apply_provider_with_catalog(config_text, provider, None)
}

/// Like [`apply_provider`], but optionally embeds every provider in `catalog`.
pub fn apply_provider_with_catalog(
    config_text: &str,
    provider: &Provider,
    catalog: Option<&[Provider]>,
) -> Result<String, AppError> {
    validate_provider(provider)?;

    let mut doc = parse_doc(config_text)?;
    ensure_table(&mut doc, "models");
    ensure_table(&mut doc, "model");
    ensure_table(&mut doc, "endpoints");
    remove_gs_model_keys(&mut doc);

    // Write selected provider last so it wins on duplicate entry ids.
    let mut ordered: Vec<&Provider> = Vec::new();
    if let Some(all) = catalog {
        for p in all {
            if p.id != provider.id && validate_provider(p).is_ok() {
                ordered.push(p);
            }
        }
    }
    ordered.push(provider);

    for p in ordered {
        for entry in &p.models {
            let key = gs_model_key(&entry.id);
            let table = build_model_table(p, entry);
            doc["model"][key.as_str()] = Item::Table(table);
        }
    }

    let default_key = gs_model_key(&provider.default_model_entry_id);
    doc["models"]["default"] = value(default_key);
    // Critical for Grok CLI custom relays (see Grok custom models docs).
    doc["endpoints"]["models_base_url"] = value(provider.base_url.clone());
    Ok(doc.to_string())
}

/// Point `[models].default` at an official/builtin model without removing `gs-*` tables.
///
/// Clears `[endpoints].models_base_url` so official xAI models are not forced
/// through the last relay endpoint.
pub fn apply_official_default(config_text: &str, model: &str) -> Result<String, AppError> {
    let model = crate::core::validate_model_token(model, "official default model")?;
    let mut doc = parse_doc(config_text)?;
    ensure_table(&mut doc, "models");
    doc["models"]["default"] = value(model);
    if let Some(endpoints) = doc.get_mut("endpoints").and_then(|i| i.as_table_like_mut()) {
        endpoints.remove("models_base_url");
    }
    Ok(doc.to_string())
}

/// Read config.toml text from disk.
pub fn read_config(path: impl AsRef<Path>) -> Result<String, AppError> {
    Ok(fs::read_to_string(path)?)
}

/// Atomically write config.toml text to disk.
pub fn write_config(path: impl AsRef<Path>, text: &str) -> Result<(), AppError> {
    atomic_write(path.as_ref(), text)
}

fn parse_doc(config_text: &str) -> Result<DocumentMut, AppError> {
    config_text
        .parse::<DocumentMut>()
        .map_err(|e| AppError::Invalid(format!("TOML parse error: {e}")))
}

fn ensure_table(doc: &mut DocumentMut, key: &str) {
    if doc.get(key).and_then(|i| i.as_table()).is_none() {
        doc[key] = Item::Table(Table::new());
    }
}

fn remove_gs_model_keys(doc: &mut DocumentMut) {
    let Some(model) = doc.get_mut("model").and_then(|item| item.as_table_like_mut()) else {
        return;
    };
    let keys: Vec<String> = model
        .iter()
        .map(|(k, _)| k.to_string())
        .filter(|k| k.starts_with("gs-"))
        .collect();
    for key in keys {
        model.remove(&key);
    }
}

fn build_model_table(provider: &Provider, entry: &ModelEntry) -> Table {
    let backend = entry.api_backend.unwrap_or(provider.api_backend);
    let context_window = entry.context_window.unwrap_or(provider.context_window);

    let mut table = Table::new();
    table["model"] = value(entry.model.clone());
    table["base_url"] = value(provider.base_url.clone());
    table["name"] = value(entry.name.clone());
    table["api_backend"] = value(backend_str(backend));
    table["context_window"] = value(context_window as i64);

    match backend {
        ApiBackend::Messages => {
            table["extra_headers"] = Item::Value(Value::InlineTable(messages_extra_headers(
                provider,
            )));
        }
        ApiBackend::ChatCompletions | ApiBackend::Responses => {
            table["api_key"] = value(provider.api_key.clone());
        }
    }

    table
}

fn messages_extra_headers(provider: &Provider) -> InlineTable {
    let mut inline = InlineTable::new();
    if let Some(headers) = &provider.extra_headers {
        for (k, v) in headers {
            inline.insert(k.as_str(), Value::from(v.as_str()));
        }
    }
    if !inline.contains_key("x-api-key") && !provider.api_key.is_empty() {
        inline.insert("x-api-key", Value::from(provider.api_key.as_str()));
    }
    if !inline.contains_key("anthropic-version") {
        inline.insert("anthropic-version", Value::from("2023-06-01"));
    }
    inline
}

fn backend_str(backend: ApiBackend) -> &'static str {
    match backend {
        ApiBackend::ChatCompletions => "chat_completions",
        ApiBackend::Responses => "responses",
        ApiBackend::Messages => "messages",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::{ApiBackend, ModelEntry, ProviderSource};
    use std::collections::HashMap;

    fn sample_provider() -> Provider {
        Provider {
            id: "myallapi".into(),
            name: "MyAllAPI".into(),
            base_url: "https://myallapi.example.com/v1".into(),
            api_key: "sk-test-key".into(),
            api_backend: ApiBackend::ChatCompletions,
            default_model_entry_id: "myallapi-grok45".into(),
            models: vec![ModelEntry {
                id: "myallapi-grok45".into(),
                model: "grok-4.5".into(),
                name: "Grok 4.5".into(),
                context_window: Some(1_000_000),
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
    fn apply_provider_rewrites_only_gs() {
        let input = include_str!("../../tests/fixtures/sample_config.toml");
        let provider = sample_provider();
        let out = apply_provider(input, &provider).unwrap();
        assert!(out.contains("[model.gs-myallapi-grok45]"));
        assert!(!out.contains("[model.gs-old-model]"));
        assert!(out.contains("[model.user-custom]"));
        assert!(
            out.contains("default = \"gs-myallapi-grok45\"")
                || out.contains("default = 'gs-myallapi-grok45'")
        );
        assert!(out.contains("keep-me"));
        assert!(out.contains("api_key"));
        assert!(out.contains("sk-test-key"));
        assert!(out.contains("installer = \"internal\"") || out.contains("installer = 'internal'"));
        assert!(
            out.contains("models_base_url")
                && out.contains("https://myallapi.example.com/v1")
        );
    }

    #[test]
    fn apply_provider_rejects_empty_models() {
        let mut provider = sample_provider();
        provider.models.clear();
        let err = apply_provider("", &provider).unwrap_err();
        assert!(err.to_string().contains("no models"));
    }

    #[test]
    fn apply_provider_with_catalog_writes_all_models() {
        let a = sample_provider();
        let mut b = sample_provider();
        b.id = "other".into();
        b.default_model_entry_id = "other-m".into();
        b.models = vec![ModelEntry {
            id: "other-m".into(),
            model: "other-model".into(),
            name: "Other".into(),
            context_window: Some(128_000),
            api_backend: None,
        }];
        b.base_url = "https://other.example/v1".into();

        let out = apply_provider_with_catalog("", &a, Some(&[a.clone(), b])).unwrap();
        assert!(out.contains("[model.gs-myallapi-grok45]"));
        assert!(out.contains("[model.gs-other-m]"));
        assert!(
            out.contains("default = \"gs-myallapi-grok45\"")
                || out.contains("default = 'gs-myallapi-grok45'")
        );
        // Active provider base wins endpoints
        assert!(out.contains("https://myallapi.example.com/v1"));
    }

    #[test]
    fn apply_official_clears_models_base_url() {
        let input = r#"
[models]
default = "gs-x"

[endpoints]
models_base_url = "https://relay.example/v1"

[model.gs-x]
model = "x"
"#;
        let out = apply_official_default(input, "grok-build").unwrap();
        assert!(
            out.contains("default = \"grok-build\"") || out.contains("default = 'grok-build'")
        );
        assert!(!out.contains("models_base_url"));
    }

    #[test]
    fn apply_official_sets_default_keeps_gs() {
        let input = include_str!("../../tests/fixtures/sample_config.toml");
        let out = apply_official_default(input, "grok-build").unwrap();
        assert!(
            out.contains("default = \"grok-build\"") || out.contains("default = 'grok-build'")
        );
        assert!(out.contains("[model.gs-"));
        assert!(out.contains("[model.user-custom]"));
    }

    #[test]
    fn apply_provider_messages_uses_extra_headers_not_api_key() {
        let input = include_str!("../../tests/fixtures/sample_config.toml");
        let mut provider = sample_provider();
        provider.api_backend = ApiBackend::Messages;
        provider.extra_headers = Some(HashMap::from([
            ("x-api-key".into(), "sk-msg".into()),
            ("anthropic-version".into(), "2023-06-01".into()),
        ]));
        let out = apply_provider(input, &provider).unwrap();
        let section = out
            .split("[model.gs-myallapi-grok45]")
            .nth(1)
            .unwrap_or("");
        let section = section.split('[').next().unwrap_or(section);
        assert!(section.contains("extra_headers"));
        assert!(section.contains("x-api-key"));
        assert!(!section.lines().any(|l| l.trim().starts_with("api_key")));
        assert!(section.contains("api_backend = \"messages\"") || section.contains("api_backend = 'messages'"));
    }

    #[test]
    fn read_write_config_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let text = include_str!("../../tests/fixtures/sample_config.toml");
        write_config(&path, text).unwrap();
        let loaded = read_config(&path).unwrap();
        assert!(loaded.contains("user-custom"));
        assert!(loaded.contains("gs-old-model"));
    }
}
