use crate::core::mask::mask_secret;
use crate::core::paths::Paths;
use crate::core::types::ApiBackend;
use serde::{Deserialize, Serialize};
use std::fs;
use std::time::{Duration, Instant};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const TOTAL_TIMEOUT: Duration = Duration::from_secs(20);

/// Result of a provider or official health probe.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthResult {
    pub ok: bool,
    pub latency_ms: u64,
    pub detail: String,
}

/// Probe a relay provider endpoint for the given API backend.
///
/// Never panics; network and HTTP failures become `ok: false` with a redacted detail.
pub fn check_provider(
    base_url: &str,
    api_key: &str,
    backend: ApiBackend,
    model: &str,
) -> HealthResult {
    let started = Instant::now();
    let result = match build_client() {
        Ok(client) => probe_backend(&client, base_url, api_key, backend, model),
        Err(e) => Err(format!("client build failed: {e}")),
    };
    finish(started, result, api_key)
}

/// Official-mode health: `auth.json` exists and is parseable JSON.
pub fn check_official(paths: &Paths) -> HealthResult {
    let started = Instant::now();
    if !paths.auth_json.is_file() {
        return finish(
            started,
            Err(format!("auth.json not found at {}", paths.auth_json.display())),
            "",
        );
    }
    match fs::read_to_string(&paths.auth_json) {
        Ok(raw) => match serde_json::from_str::<serde_json::Value>(&raw) {
            Ok(_) => finish(
                started,
                Ok("auth.json present and parseable".into()),
                "",
            ),
            Err(e) => finish(started, Err(format!("auth.json parse error: {e}")), ""),
        },
        Err(e) => finish(started, Err(format!("read auth.json failed: {e}")), ""),
    }
}

fn build_client() -> Result<reqwest::blocking::Client, reqwest::Error> {
    reqwest::blocking::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(TOTAL_TIMEOUT)
        .build()
}

fn probe_backend(
    client: &reqwest::blocking::Client,
    base_url: &str,
    api_key: &str,
    backend: ApiBackend,
    model: &str,
) -> Result<String, String> {
    match backend {
        ApiBackend::ChatCompletions => probe_chat_completions(client, base_url, api_key, model),
        ApiBackend::Messages => probe_messages(client, base_url, api_key, model),
        ApiBackend::Responses => probe_responses(client, base_url, api_key, model),
    }
}

fn probe_chat_completions(
    client: &reqwest::blocking::Client,
    base_url: &str,
    api_key: &str,
    model: &str,
) -> Result<String, String> {
    let models_url = join_url(base_url, "models");
    let models_resp = client
        .get(&models_url)
        .header("Authorization", format!("Bearer {api_key}"))
        .send()
        .map_err(|e| format!("GET /models error: {e}"))?;

    let status = models_resp.status();
    if status.is_success() {
        return Ok(format!("GET /models -> {status}"));
    }

    // Fallback: minimal chat completions ping
    let chat_url = join_url(base_url, "chat/completions");
    let body = serde_json::json!({
        "model": model,
        "messages": [{"role": "user", "content": "ping"}],
        "max_tokens": 1
    });
    let chat_resp = client
        .post(&chat_url)
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .map_err(|e| {
            format!("GET /models -> {status}; POST /chat/completions error: {e}")
        })?;

    let chat_status = chat_resp.status();
    let snippet = response_snippet(chat_resp);
    if chat_status.is_success() {
        Ok(format!(
            "GET /models -> {status}; POST /chat/completions -> {chat_status}"
        ))
    } else {
        Err(format!(
            "GET /models -> {status}; POST /chat/completions -> {chat_status}: {snippet}"
        ))
    }
}

fn probe_messages(
    client: &reqwest::blocking::Client,
    base_url: &str,
    api_key: &str,
    model: &str,
) -> Result<String, String> {
    let url = join_url(base_url, "messages");
    let body = serde_json::json!({
        "model": model,
        "max_tokens": 1,
        "messages": [{"role": "user", "content": "ping"}]
    });
    let resp = client
        .post(&url)
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .map_err(|e| format!("POST /messages error: {e}"))?;

    let status = resp.status();
    let snippet = response_snippet(resp);
    if status.is_success() {
        Ok(format!("POST /messages -> {status}"))
    } else {
        Err(format!("POST /messages -> {status}: {snippet}"))
    }
}

fn probe_responses(
    client: &reqwest::blocking::Client,
    base_url: &str,
    api_key: &str,
    model: &str,
) -> Result<String, String> {
    let url = join_url(base_url, "responses");
    let body = serde_json::json!({
        "model": model,
        "input": "ping"
    });
    let resp = client
        .post(&url)
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .map_err(|e| format!("POST /responses error: {e}"))?;

    let status = resp.status();
    let snippet = response_snippet(resp);
    if status.is_success() {
        Ok(format!("POST /responses -> {status}"))
    } else {
        Err(format!("POST /responses -> {status}: {snippet}"))
    }
}

fn response_snippet(resp: reqwest::blocking::Response) -> String {
    let text = resp.text().unwrap_or_default();
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return "(empty body)".into();
    }
    let chars: Vec<char> = trimmed.chars().collect();
    if chars.len() > 200 {
        format!("{}…", chars.iter().take(200).collect::<String>())
    } else {
        trimmed.to_string()
    }
}

fn join_url(base: &str, path: &str) -> String {
    let base = base.trim().trim_end_matches('/');
    let path = path.trim().trim_start_matches('/');
    format!("{base}/{path}")
}

fn finish(started: Instant, result: Result<String, String>, api_key: &str) -> HealthResult {
    let latency_ms = started.elapsed().as_millis() as u64;
    match result {
        Ok(detail) => HealthResult {
            ok: true,
            latency_ms,
            detail: redact_secrets(&detail, api_key),
        },
        Err(detail) => HealthResult {
            ok: false,
            latency_ms,
            detail: redact_secrets(&detail, api_key),
        },
    }
}

/// Replace any occurrence of `secret` in `detail` with its masked form.
fn redact_secrets(detail: &str, secret: &str) -> String {
    let secret = secret.trim();
    if secret.is_empty() || !detail.contains(secret) {
        return detail.to_string();
    }
    detail.replace(secret, &mask_secret(secret))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::paths::Paths;

    #[test]
    fn redacts_api_key_in_detail() {
        let key = "sk-demo-key-abcdefghijklmnop";
        let detail = format!("Authorization failed for key={key} status=401");
        let redacted = redact_secrets(&detail, key);
        assert!(!redacted.contains(key));
        assert!(redacted.contains(&mask_secret(key)));
        assert!(redacted.contains("status=401"));
    }

    #[test]
    fn redact_skips_empty_secret() {
        let detail = "nothing to hide";
        assert_eq!(redact_secrets(detail, ""), detail);
        assert_eq!(redact_secrets(detail, "   "), detail);
    }

    #[test]
    fn check_official_missing_auth() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::from_root(dir.path());
        let result = check_official(&paths);
        assert!(!result.ok);
        assert!(result.detail.contains("auth.json not found"));
    }

    #[test]
    fn check_official_parseable_auth() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::from_root(dir.path());
        fs::create_dir_all(&paths.grok_home).unwrap();
        fs::write(&paths.auth_json, r#"{"token":"abc"}"#).unwrap();
        let result = check_official(&paths);
        assert!(result.ok);
        assert!(result.detail.contains("parseable"));
    }

    #[test]
    fn check_official_invalid_json() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::from_root(dir.path());
        fs::create_dir_all(&paths.grok_home).unwrap();
        fs::write(&paths.auth_json, "not-json").unwrap();
        let result = check_official(&paths);
        assert!(!result.ok);
        assert!(result.detail.contains("parse error"));
    }

    #[test]
    fn join_url_trims_slashes() {
        assert_eq!(
            join_url("https://api.example.com/v1/", "/models"),
            "https://api.example.com/v1/models"
        );
    }

    /// Live network probe — ignored by default.
    #[test]
    #[ignore]
    fn network_check_provider_openai_style() {
        let result = check_provider(
            "https://api.openai.com/v1",
            "sk-invalid-test-key-xxxxxxxx",
            ApiBackend::ChatCompletions,
            "gpt-4o-mini",
        );
        // Expect failure with invalid key, but request should complete.
        assert!(!result.ok);
        assert!(!result.detail.is_empty());
    }
}
