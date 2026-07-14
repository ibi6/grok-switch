/// Trim, strip trailing `/`, optionally append `/v1` when missing.
pub fn normalize_base_url(url: &str, append_v1: bool) -> String {
    let mut s = url.trim().trim_end_matches('/').to_string();
    if append_v1 && !s.ends_with("/v1") {
        s.push_str("/v1");
    }
    s
}

/// Strip optional context-window suffix like `[1M]` from a model id.
pub fn sanitize_model_name(raw: &str) -> String {
    let raw = raw.trim();
    // Prefer non-empty bracket body: ^(.+?)\s*\[[^\]]+\]$
    if let Some(open) = raw.find('[') {
        let has_body = open + 1 < raw.len().saturating_sub(1);
        if open > 0 && raw.ends_with(']') && has_body {
            return raw[..open].trim_end().to_string();
        }
    }
    raw.to_string()
}

/// Build managed model section key: `gs-{entry_id}` without double-prefixing.
pub fn gs_model_key(entry_id: &str) -> String {
    let id = entry_id.trim();
    if id.starts_with("gs-") {
        id.to_string()
    } else {
        format!("gs-{id}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_context_suffix() {
        assert_eq!(sanitize_model_name("grok-4.5[1M]"), "grok-4.5");
        assert_eq!(sanitize_model_name("gpt-5.6-sol[1M]"), "gpt-5.6-sol");
        assert_eq!(sanitize_model_name("plain"), "plain");
        assert_eq!(sanitize_model_name("grok-4.5 [1M]"), "grok-4.5");
        assert_eq!(sanitize_model_name("model[]"), "model[]");
    }

    #[test]
    fn normalizes_base_url() {
        assert_eq!(
            normalize_base_url("https://relay.example.com:8443/", true),
            "https://relay.example.com:8443/v1"
        );
        assert_eq!(
            normalize_base_url("https://x/v1/", true),
            "https://x/v1"
        );
        assert_eq!(
            normalize_base_url("https://x/v1", false),
            "https://x/v1"
        );
    }

    #[test]
    fn gs_model_key_prefixes_once() {
        assert_eq!(gs_model_key("myallapi-grok45"), "gs-myallapi-grok45");
        assert_eq!(gs_model_key("gs-already"), "gs-already");
    }
}
