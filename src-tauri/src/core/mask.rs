/// Mask a secret for UI/logs: short secrets become `***`;
/// longer ones show first 6 (or `sk-` + 4) + `...` + last 4.
pub fn mask_secret(secret: &str) -> String {
    let s = secret.trim();
    let len = s.chars().count();
    if len <= 12 {
        return "***".to_string();
    }

    let chars: Vec<char> = s.chars().collect();
    let prefix: String = if s.starts_with("sk-") && len >= 7 {
        // keep `sk-` + 4 more when possible
        chars.iter().take(7).collect()
    } else {
        chars.iter().take(6).collect()
    };
    let suffix: String = chars[len - 4..].iter().collect();
    format!("{prefix}...{suffix}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_key() {
        let m = mask_secret("sk-demo-key-abcdefghijklmnop");
        assert!(m.starts_with("sk-demo"));
        assert!(m.ends_with("mnop"));
        assert!(m.contains("..."));
        assert!(!m.contains("key-abcdefghi"));
    }

    #[test]
    fn masks_short_secret() {
        assert_eq!(mask_secret("short"), "***");
        assert_eq!(mask_secret("123456789012"), "***");
    }

    #[test]
    fn masks_non_sk_secret() {
        let m = mask_secret("abcdefghijklmnop");
        assert_eq!(m, "abcdef...mnop");
    }
}
