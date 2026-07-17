/// Trim, strip trailing `/`, optionally append `/v1` when no version segment.
///
/// Only appends when the URL does not already end in a version-like segment
/// (`v1`, `v2`, `v1beta`, …), so bases such as `.../v1beta` are left untouched
/// instead of becoming `.../v1beta/v1`.
pub fn normalize_base_url(url: &str, append_v1: bool) -> String {
    let s = url.trim().trim_end_matches('/').to_string();
    if append_v1 && !has_version_suffix(&s) {
        return format!("{s}/v1");
    }
    s
}

/// True when the last path segment looks like an API version (`v` + digit …).
fn has_version_suffix(s: &str) -> bool {
    let last = s.rsplit('/').next().unwrap_or("");
    let mut chars = last.chars();
    matches!(chars.next(), Some('v') | Some('V'))
        && matches!(chars.next(), Some(c) if c.is_ascii_digit())
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

/// Characters allowed in model ids / CLI flags that get embedded into shell
/// commands or TOML section keys. Rejects quotes, spaces, and shell metachars.
pub fn is_safe_model_token(s: &str) -> bool {
    let s = s.trim();
    if s.is_empty() || s.len() > 128 {
        return false;
    }
    s.chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/' | ':' | '+'))
}

/// Validate a free-form model token, returning a trimmed owned string on success.
pub fn validate_model_token(s: &str, field: &str) -> Result<String, crate::core::AppError> {
    let t = s.trim();
    if !is_safe_model_token(t) {
        return Err(crate::core::AppError::Invalid(format!(
            "{field} contains invalid characters (allowed: A-Z a-z 0-9 - _ . / : +)"
        )));
    }
    Ok(t.to_string())
}

/// Safe filesystem id for account / backup directory names (no path traversal).
pub fn is_safe_fs_id(s: &str) -> bool {
    let s = s.trim();
    if s.is_empty() || s.len() > 128 {
        return false;
    }
    if s == "." || s == ".." {
        return false;
    }
    s.chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

pub fn validate_fs_id(s: &str, field: &str) -> Result<String, crate::core::AppError> {
    let t = s.trim();
    if !is_safe_fs_id(t) {
        return Err(crate::core::AppError::Invalid(format!(
            "{field} is not a safe id (allowed: A-Z a-z 0-9 - _ .)"
        )));
    }
    Ok(t.to_string())
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
    fn does_not_double_append_version() {
        // Already-versioned bases are left as-is (no `/v1beta/v1`).
        assert_eq!(
            normalize_base_url("https://api.example.com/v1beta", true),
            "https://api.example.com/v1beta"
        );
        assert_eq!(
            normalize_base_url("https://api.example.com/v2/", true),
            "https://api.example.com/v2"
        );
        // Non-version last segment still gets /v1.
        assert_eq!(
            normalize_base_url("https://api.example.com/openai", true),
            "https://api.example.com/openai/v1"
        );
        // "video" starts with v but not a version → still append.
        assert_eq!(
            normalize_base_url("https://api.example.com/video", true),
            "https://api.example.com/video/v1"
        );
    }

    #[test]
    fn gs_model_key_prefixes_once() {
        assert_eq!(gs_model_key("myallapi-grok45"), "gs-myallapi-grok45");
        assert_eq!(gs_model_key("gs-already"), "gs-already");
    }

    #[test]
    fn model_token_whitelist() {
        assert!(is_safe_model_token("gs-myallapi-grok45"));
        assert!(is_safe_model_token("grok-4.5"));
        assert!(is_safe_model_token("x-ai/grok-4"));
        assert!(is_safe_model_token("org:model+v2"));
        assert!(!is_safe_model_token(""));
        assert!(!is_safe_model_token("a b"));
        assert!(!is_safe_model_token("evil\"; rm -rf /"));
        assert!(!is_safe_model_token("a&b"));
        assert!(!is_safe_model_token("a|b"));
        assert!(!is_safe_model_token("a$(b)"));
        assert!(!is_safe_model_token(&"x".repeat(129)));
        assert!(validate_model_token("ok-model", "m").is_ok());
        assert!(validate_model_token("bad model", "m").is_err());
    }

    #[test]
    fn fs_id_rejects_traversal() {
        assert!(is_safe_fs_id("acc-1"));
        assert!(is_safe_fs_id("20260717-120000"));
        assert!(is_safe_fs_id("uuid-like-abc"));
        assert!(!is_safe_fs_id(""));
        assert!(!is_safe_fs_id(".."));
        assert!(!is_safe_fs_id("../etc"));
        assert!(!is_safe_fs_id("a/b"));
        assert!(!is_safe_fs_id("a\\b"));
        assert!(validate_fs_id("ok_id", "id").is_ok());
        assert!(validate_fs_id("..", "id").is_err());
    }
}
