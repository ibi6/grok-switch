//! Manage Grok CLI MCP servers stored in `~/.grok/config.toml`.
//!
//! Layout (official docs):
//! ```toml
//! [mcp_servers.filesystem]
//! command = "npx"
//! args = ["-y", "@modelcontextprotocol/server-filesystem", "/path"]
//! enabled = true
//! ```
//!
//! Or Streamable HTTP:
//! ```toml
//! [mcp_servers.remote]
//! url = "http://localhost:5000/api/mcp"
//! headers = { "Authorization" = "Bearer …" }
//! ```
//!
//! Provider switching only touches `model.*` / `models` / `endpoints` via
//! `toml_edit`, so MCP tables are preserved across enable flows.

use crate::core::mask::mask_secret;
use crate::core::paths::{atomic_write, Paths};
use crate::core::AppError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};
use toml_edit::{value, Array, DocumentMut, InlineTable, Item, Table, Value};

/// One MCP server entry as shown in the UI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServer {
    pub name: String,
    /// stdio transport
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    /// Streamable HTTP transport
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub env: HashMap<String, String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub headers: HashMap<String, String>,
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub startup_timeout_sec: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_timeout_sec: Option<u64>,
    /// Transport kind for UI badges.
    pub transport: McpTransport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum McpTransport {
    Stdio,
    Http,
    Unknown,
}

/// Draft for create/update (same shape as McpServer without derived fields).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpDraft {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub startup_timeout_sec: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_timeout_sec: Option<u64>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpHealthResult {
    pub ok: bool,
    pub detail: String,
    pub latency_ms: u64,
}

/// Validate MCP server name (toml key safe): `[a-zA-Z0-9_-]{1,64}`.
pub fn validate_mcp_name(name: &str) -> Result<String, AppError> {
    let n = name.trim();
    if n.is_empty() || n.len() > 64 {
        return Err(AppError::Invalid(
            "MCP server name must be 1–64 characters".into(),
        ));
    }
    if !n
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(AppError::Invalid(
            "MCP server name may only contain letters, digits, _ and -".into(),
        ));
    }
    if n.starts_with('-') || n.ends_with('-') {
        return Err(AppError::Invalid(
            "MCP server name must not start or end with '-'".into(),
        ));
    }
    Ok(n.to_string())
}

pub fn list_mcp_servers(paths: &Paths) -> Result<Vec<McpServer>, AppError> {
    let text = read_config_text(paths)?;
    let doc = parse_doc(&text)?;
    let mut out = Vec::new();
    let Some(servers) = doc.get("mcp_servers").and_then(|i| i.as_table()) else {
        return Ok(out);
    };
    for (name, item) in servers.iter() {
        let Some(table) = item.as_table() else {
            continue;
        };
        out.push(table_to_server(name, table));
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

pub fn get_mcp_server(paths: &Paths, name: &str) -> Result<McpServer, AppError> {
    let name = validate_mcp_name(name)?;
    list_mcp_servers(paths)?
        .into_iter()
        .find(|s| s.name == name)
        .ok_or_else(|| AppError::NotFound(format!("MCP server not found: {name}")))
}

pub fn upsert_mcp_server(paths: &Paths, draft: &McpDraft) -> Result<McpServer, AppError> {
    let _guard = crate::core::lock_store();
    let name = validate_mcp_name(&draft.name)?;
    validate_draft(draft)?;

    let text = read_config_text(paths)?;
    let mut doc = parse_doc(&text)?;
    ensure_mcp_root(&mut doc);

    let mut table = Table::new();
    if let Some(cmd) = draft.command.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
        table["command"] = value(cmd);
    }
    if !draft.args.is_empty() {
        let mut arr = Array::new();
        for a in &draft.args {
            arr.push(a.as_str());
        }
        table["args"] = Item::Value(Value::Array(arr));
    }
    if let Some(url) = draft.url.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
        table["url"] = value(url);
    }
    if !draft.env.is_empty() {
        table["env"] = Item::Value(Value::InlineTable(map_to_inline(&draft.env)));
    }
    if !draft.headers.is_empty() {
        table["headers"] = Item::Value(Value::InlineTable(map_to_inline(&draft.headers)));
    }
    table["enabled"] = value(draft.enabled);
    if let Some(s) = draft.startup_timeout_sec {
        table["startup_timeout_sec"] = value(s as i64);
    }
    if let Some(s) = draft.tool_timeout_sec {
        table["tool_timeout_sec"] = value(s as i64);
    }

    doc["mcp_servers"][name.as_str()] = Item::Table(table);
    write_config_text(paths, &doc.to_string())?;

    Ok(draft_to_server(&name, draft))
}

pub fn delete_mcp_server(paths: &Paths, name: &str) -> Result<bool, AppError> {
    let _guard = crate::core::lock_store();
    let name = validate_mcp_name(name)?;
    let text = read_config_text(paths)?;
    let mut doc = parse_doc(&text)?;
    let Some(servers) = doc.get_mut("mcp_servers").and_then(|i| i.as_table_like_mut()) else {
        return Ok(false);
    };
    let removed = servers.remove(&name).is_some();
    if removed {
        write_config_text(paths, &doc.to_string())?;
    }
    Ok(removed)
}

pub fn set_mcp_enabled(paths: &Paths, name: &str, enabled: bool) -> Result<McpServer, AppError> {
    let _guard = crate::core::lock_store();
    let name = validate_mcp_name(name)?;
    let text = read_config_text(paths)?;
    let mut doc = parse_doc(&text)?;
    {
        let Some(servers) = doc.get_mut("mcp_servers").and_then(|i| i.as_table_mut()) else {
            return Err(AppError::NotFound(format!("MCP server not found: {name}")));
        };
        let Some(item) = servers.get_mut(name.as_str()) else {
            return Err(AppError::NotFound(format!("MCP server not found: {name}")));
        };
        let Some(table) = item.as_table_mut() else {
            return Err(AppError::Invalid(format!(
                "mcp_servers.{name} is not a table"
            )));
        };
        table["enabled"] = value(enabled);
    }
    write_config_text(paths, &doc.to_string())?;
    // Re-read from the mutated doc without holding a mutable borrow.
    let servers = doc
        .get("mcp_servers")
        .and_then(|i| i.as_table())
        .ok_or_else(|| AppError::NotFound(format!("MCP server not found: {name}")))?;
    let table = servers
        .get(name.as_str())
        .and_then(|i| i.as_table())
        .ok_or_else(|| AppError::NotFound(format!("MCP server not found: {name}")))?;
    Ok(table_to_server(&name, table))
}

/// Best-effort health: for stdio check command exists; for http try HEAD/GET.
pub fn check_mcp_server(paths: &Paths, name: &str) -> Result<McpHealthResult, AppError> {
    let server = get_mcp_server(paths, name)?;
    let started = Instant::now();
    let result = match server.transport {
        McpTransport::Http => check_http(server.url.as_deref().unwrap_or("")),
        McpTransport::Stdio => check_stdio(server.command.as_deref().unwrap_or("")),
        McpTransport::Unknown => Err("server has neither command nor url".into()),
    };
    let latency_ms = started.elapsed().as_millis() as u64;
    match result {
        Ok(detail) => Ok(McpHealthResult {
            ok: true,
            detail: redact_map_secrets(&detail, &server),
            latency_ms,
        }),
        Err(detail) => Ok(McpHealthResult {
            ok: false,
            detail: redact_map_secrets(&detail, &server),
            latency_ms,
        }),
    }
}

// ─── internals ───────────────────────────────────────────────────────────────

fn validate_draft(draft: &McpDraft) -> Result<(), AppError> {
    let has_cmd = draft
        .command
        .as_ref()
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);
    let has_url = draft
        .url
        .as_ref()
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);
    if !has_cmd && !has_url {
        return Err(AppError::Invalid(
            "MCP server needs either command (stdio) or url (http)".into(),
        ));
    }
    if has_url {
        let u = draft.url.as_ref().unwrap().trim();
        if !(u.starts_with("http://") || u.starts_with("https://")) {
            return Err(AppError::Invalid(
                "url must start with http:// or https://".into(),
            ));
        }
    }
    Ok(())
}

fn read_config_text(paths: &Paths) -> Result<String, AppError> {
    if paths.config_toml.is_file() {
        Ok(fs::read_to_string(&paths.config_toml)?)
    } else {
        Ok(String::new())
    }
}

fn write_config_text(paths: &Paths, text: &str) -> Result<(), AppError> {
    if let Some(parent) = paths.config_toml.parent() {
        fs::create_dir_all(parent)?;
    }
    atomic_write(&paths.config_toml, text)
}

fn parse_doc(text: &str) -> Result<DocumentMut, AppError> {
    text.parse::<DocumentMut>()
        .map_err(|e| AppError::Invalid(format!("TOML parse error: {e}")))
}

fn ensure_mcp_root(doc: &mut DocumentMut) {
    if doc.get("mcp_servers").and_then(|i| i.as_table()).is_none() {
        doc["mcp_servers"] = Item::Table(Table::new());
    }
}

fn table_to_server(name: &str, table: &Table) -> McpServer {
    let command = table
        .get("command")
        .and_then(|i| i.as_str())
        .map(|s| s.to_string());
    let url = table
        .get("url")
        .and_then(|i| i.as_str())
        .map(|s| s.to_string());
    let args = table
        .get("args")
        .and_then(|i| i.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    let env = inline_or_table_map(table.get("env"));
    let headers = inline_or_table_map(table.get("headers"));
    let enabled = table
        .get("enabled")
        .and_then(|i| i.as_bool())
        .unwrap_or(true);
    let startup_timeout_sec = table
        .get("startup_timeout_sec")
        .and_then(|i| i.as_integer())
        .map(|n| n as u64);
    let tool_timeout_sec = table
        .get("tool_timeout_sec")
        .and_then(|i| i.as_integer())
        .map(|n| n as u64);
    let transport = if url.as_ref().is_some_and(|u| !u.is_empty()) {
        McpTransport::Http
    } else if command.as_ref().is_some_and(|c| !c.is_empty()) {
        McpTransport::Stdio
    } else {
        McpTransport::Unknown
    };
    McpServer {
        name: name.to_string(),
        command,
        args,
        url,
        env,
        headers,
        enabled,
        startup_timeout_sec,
        tool_timeout_sec,
        transport,
    }
}

fn draft_to_server(name: &str, draft: &McpDraft) -> McpServer {
    let transport = if draft.url.as_ref().is_some_and(|u| !u.trim().is_empty()) {
        McpTransport::Http
    } else if draft.command.as_ref().is_some_and(|c| !c.trim().is_empty()) {
        McpTransport::Stdio
    } else {
        McpTransport::Unknown
    };
    McpServer {
        name: name.to_string(),
        command: draft.command.clone(),
        args: draft.args.clone(),
        url: draft.url.clone(),
        env: draft.env.clone(),
        headers: draft.headers.clone(),
        enabled: draft.enabled,
        startup_timeout_sec: draft.startup_timeout_sec,
        tool_timeout_sec: draft.tool_timeout_sec,
        transport,
    }
}

fn inline_or_table_map(item: Option<&Item>) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let Some(item) = item else {
        return map;
    };
    if let Some(inline) = item.as_inline_table() {
        for (k, v) in inline.iter() {
            if let Some(s) = v.as_str() {
                map.insert(k.to_string(), s.to_string());
            }
        }
    } else if let Some(table) = item.as_table() {
        for (k, v) in table.iter() {
            if let Some(s) = v.as_str() {
                map.insert(k.to_string(), s.to_string());
            }
        }
    }
    map
}

fn map_to_inline(map: &HashMap<String, String>) -> InlineTable {
    let mut inline = InlineTable::new();
    let mut keys: Vec<_> = map.keys().collect();
    keys.sort();
    for k in keys {
        if let Some(v) = map.get(k) {
            inline.insert(k.as_str(), Value::from(v.as_str()));
        }
    }
    inline
}

fn check_stdio(command: &str) -> Result<String, String> {
    let cmd = command.trim();
    if cmd.is_empty() {
        return Err("command is empty".into());
    }
    // Absolute / relative path that exists.
    if Path::new(cmd).is_file() {
        return Ok(format!("command file exists: {cmd}"));
    }
    // which-like: try `where` on Windows, `command -v` via shell is avoided —
    // spawn with --help / -h and accept "started".
    #[cfg(windows)]
    {
        let out = Command::new("where").arg(cmd).output();
        match out {
            Ok(o) if o.status.success() => {
                let path = String::from_utf8_lossy(&o.stdout);
                let first = path.lines().next().unwrap_or(cmd).trim();
                return Ok(format!("command on PATH: {first}"));
            }
            _ => {}
        }
    }
    #[cfg(not(windows))]
    {
        let out = Command::new("sh").arg("-c").arg(format!("command -v {cmd}")).output();
        // Only use if cmd is simple token — we already whitelist names but command path may have spaces.
        if let Ok(o) = out {
            if o.status.success() {
                let path = String::from_utf8_lossy(&o.stdout);
                return Ok(format!("command on PATH: {}", path.trim()));
            }
        }
    }
    Err(format!("command not found: {cmd}"))
}

fn check_http(url: &str) -> Result<String, String> {
    let url = url.trim();
    if url.is_empty() {
        return Err("url is empty".into());
    }
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .connect_timeout(Duration::from_secs(3))
        .build()
        .map_err(|e| format!("client: {e}"))?;
    // Prefer GET — many MCP HTTP endpoints reject HEAD.
    let resp = client.get(url).send().map_err(|e| format!("request: {e}"))?;
    let status = resp.status();
    // Any HTTP response (even 4xx) means the endpoint is reachable.
    if status.as_u16() < 500 {
        Ok(format!("HTTP {status}"))
    } else {
        Err(format!("HTTP {status}"))
    }
}

fn redact_map_secrets(detail: &str, server: &McpServer) -> String {
    let mut out = detail.to_string();
    for v in server.env.values().chain(server.headers.values()) {
        if v.len() > 8 && out.contains(v) {
            out = out.replace(v, &mask_secret(v));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::paths::Paths;

    fn setup() -> (tempfile::TempDir, Paths) {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::from_root(dir.path());
        paths.ensure_app_dirs().unwrap();
        fs::create_dir_all(&paths.grok_home).unwrap();
        (dir, paths)
    }

    #[test]
    fn validate_names() {
        assert!(validate_mcp_name("filesystem").is_ok());
        assert!(validate_mcp_name("my_tools-1").is_ok());
        assert!(validate_mcp_name("").is_err());
        assert!(validate_mcp_name("a b").is_err());
        assert!(validate_mcp_name("../x").is_err());
        assert!(validate_mcp_name("-bad").is_err());
    }

    #[test]
    fn upsert_list_delete_roundtrip() {
        let (_tmp, paths) = setup();
        let draft = McpDraft {
            name: "filesystem".into(),
            command: Some("npx".into()),
            args: vec![
                "-y".into(),
                "@modelcontextprotocol/server-filesystem".into(),
                "/tmp".into(),
            ],
            url: None,
            env: HashMap::new(),
            headers: HashMap::new(),
            enabled: true,
            startup_timeout_sec: Some(30),
            tool_timeout_sec: None,
        };
        let s = upsert_mcp_server(&paths, &draft).unwrap();
        assert_eq!(s.name, "filesystem");
        assert_eq!(s.transport, McpTransport::Stdio);
        assert!(s.enabled);

        let list = list_mcp_servers(&paths).unwrap();
        assert_eq!(list.len(), 1);

        let toggled = set_mcp_enabled(&paths, "filesystem", false).unwrap();
        assert!(!toggled.enabled);

        assert!(delete_mcp_server(&paths, "filesystem").unwrap());
        assert!(list_mcp_servers(&paths).unwrap().is_empty());
    }

    #[test]
    fn preserves_other_config_sections() {
        let (_tmp, paths) = setup();
        fs::write(
            &paths.config_toml,
            r#"
[models]
default = "gs-x"

[model.gs-x]
model = "grok-4.5"
api_key = "sk-keep"
"#,
        )
        .unwrap();

        let draft = McpDraft {
            name: "github".into(),
            command: Some("npx".into()),
            args: vec!["-y".into(), "@modelcontextprotocol/server-github".into()],
            url: None,
            env: HashMap::from([("GITHUB_PERSONAL_ACCESS_TOKEN".into(), "ghp_xxx".into())]),
            headers: HashMap::new(),
            enabled: true,
            startup_timeout_sec: None,
            tool_timeout_sec: None,
        };
        upsert_mcp_server(&paths, &draft).unwrap();
        let raw = fs::read_to_string(&paths.config_toml).unwrap();
        assert!(raw.contains("[models]"));
        assert!(raw.contains("gs-x"));
        assert!(raw.contains("sk-keep"));
        assert!(raw.contains("[mcp_servers.github]") || raw.contains("mcp_servers.github"));
        assert!(raw.contains("ghp_xxx"));
    }

    #[test]
    fn http_transport_draft() {
        let (_tmp, paths) = setup();
        let draft = McpDraft {
            name: "nebula".into(),
            command: None,
            args: vec![],
            url: Some("http://localhost:5000/api/mcp".into()),
            env: HashMap::new(),
            headers: HashMap::from([("x-session".into(), "abc".into())]),
            enabled: true,
            startup_timeout_sec: None,
            tool_timeout_sec: None,
        };
        let s = upsert_mcp_server(&paths, &draft).unwrap();
        assert_eq!(s.transport, McpTransport::Http);
        assert_eq!(s.url.as_deref(), Some("http://localhost:5000/api/mcp"));
    }

    #[test]
    fn rejects_empty_transport() {
        let (_tmp, paths) = setup();
        let draft = McpDraft {
            name: "empty".into(),
            command: None,
            args: vec![],
            url: None,
            env: HashMap::new(),
            headers: HashMap::new(),
            enabled: true,
            startup_timeout_sec: None,
            tool_timeout_sec: None,
        };
        assert!(upsert_mcp_server(&paths, &draft).is_err());
    }
}
