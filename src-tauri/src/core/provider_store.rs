use crate::core::paths::{atomic_write, Paths};
use crate::core::types::Provider;
use crate::core::AppError;
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProvidersFile {
    version: u32,
    items: Vec<Provider>,
}

impl Default for ProvidersFile {
    fn default() -> Self {
        Self {
            version: 1,
            items: Vec::new(),
        }
    }
}

fn read_file(paths: &Paths) -> Result<ProvidersFile, AppError> {
    if !paths.providers_json.exists() {
        return Ok(ProvidersFile::default());
    }
    let raw = fs::read_to_string(&paths.providers_json)?;
    let file: ProvidersFile = serde_json::from_str(&raw)?;
    Ok(file)
}

fn write_file(paths: &Paths, file: &ProvidersFile) -> Result<(), AppError> {
    paths.ensure_app_dirs()?;
    let json = serde_json::to_string_pretty(file)?;
    atomic_write(&paths.providers_json, json)?;
    Ok(())
}

pub fn list_providers(paths: &Paths) -> Result<Vec<Provider>, AppError> {
    Ok(read_file(paths)?.items)
}

pub fn get_provider(paths: &Paths, id: &str) -> Result<Option<Provider>, AppError> {
    Ok(read_file(paths)?
        .items
        .into_iter()
        .find(|p| p.id == id))
}

pub fn upsert_provider(paths: &Paths, provider: Provider) -> Result<(), AppError> {
    let _guard = crate::core::lock_store();
    let mut file = read_file(paths)?;
    if let Some(existing) = file.items.iter_mut().find(|p| p.id == provider.id) {
        *existing = provider;
    } else {
        file.items.push(provider);
    }
    write_file(paths, &file)
}

pub fn delete_provider(paths: &Paths, id: &str) -> Result<bool, AppError> {
    let _guard = crate::core::lock_store();
    let mut file = read_file(paths)?;
    let before = file.items.len();
    file.items.retain(|p| p.id != id);
    let removed = file.items.len() != before;
    if removed {
        write_file(paths, &file)?;
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::{ApiBackend, ProviderSource};

    fn sample_provider(id: &str, name: &str) -> Provider {
        Provider {
            id: id.into(),
            name: name.into(),
            base_url: "https://api.example.com/v1".into(),
            api_key: "sk-test-key-abcdefghijklmnop".into(),
            api_backend: ApiBackend::ChatCompletions,
            default_model_entry_id: "m1".into(),
            models: vec![],
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
    fn provider_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::from_root(dir.path());
        paths.ensure_app_dirs().unwrap();

        assert!(list_providers(&paths).unwrap().is_empty());

        upsert_provider(&paths, sample_provider("a", "Alpha")).unwrap();
        upsert_provider(&paths, sample_provider("b", "Beta")).unwrap();

        let list = list_providers(&paths).unwrap();
        assert_eq!(list.len(), 2);

        let got = get_provider(&paths, "a").unwrap().unwrap();
        assert_eq!(got.name, "Alpha");

        upsert_provider(&paths, sample_provider("a", "Alpha2")).unwrap();
        let got = get_provider(&paths, "a").unwrap().unwrap();
        assert_eq!(got.name, "Alpha2");
        assert_eq!(list_providers(&paths).unwrap().len(), 2);

        assert!(delete_provider(&paths, "a").unwrap());
        assert!(!delete_provider(&paths, "a").unwrap());
        assert_eq!(list_providers(&paths).unwrap().len(), 1);
        assert!(get_provider(&paths, "a").unwrap().is_none());
    }
}
