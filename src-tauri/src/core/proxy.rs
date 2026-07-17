//! Local OpenAI-compatible reverse proxy with provider pool + request logging.
//!
//! Binds `127.0.0.1:<port>` only. When enabled, point Grok
//! `endpoints.models_base_url` / provider base_url at
//! `http://127.0.0.1:<port>/v1` so traffic flows through the pool.
//!
//! Failover: on 401/403/429/5xx, try the next eligible provider.

use crate::core::db;
use crate::core::paths::Paths;
use crate::core::pool;
use crate::core::provider_store;
use crate::core::settings_store;
use crate::core::types::{PoolStrategy, Provider};
use crate::core::AppError;
use serde_json::Value;
use std::sync::atomic::{AtomicBool, AtomicU16, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;
use tiny_http::{Header, Method, Response, Server, StatusCode};

static RUNNING: AtomicBool = AtomicBool::new(false);
static BOUND_PORT: AtomicU16 = AtomicU16::new(0);
static STOP_FLAG: AtomicBool = AtomicBool::new(false);

/// Shared handle so the accept thread can see the latest Paths root.
static PATHS_SLOT: Mutex<Option<Paths>> = Mutex::new(None);

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyStatus {
    pub running: bool,
    pub port: u16,
    pub listen: String,
}

pub fn status() -> ProxyStatus {
    let port = BOUND_PORT.load(Ordering::SeqCst);
    let running = RUNNING.load(Ordering::SeqCst);
    ProxyStatus {
        running,
        port,
        listen: if running {
            format!("http://127.0.0.1:{port}/v1")
        } else {
            String::new()
        },
    }
}

pub fn start(paths: &Paths) -> Result<ProxyStatus, AppError> {
    if RUNNING.load(Ordering::SeqCst) {
        return Ok(status());
    }
    let settings = settings_store::load_settings(paths)?;
    let port = if settings.proxy_port == 0 {
        18765
    } else {
        settings.proxy_port
    };

    {
        let mut slot = PATHS_SLOT.lock().unwrap_or_else(|p| p.into_inner());
        *slot = Some(paths.clone());
    }

    let addr = format!("127.0.0.1:{port}");
    let server = Server::http(&addr).map_err(|e| {
        AppError::Message(format!("proxy bind {addr} failed: {e}"))
    })?;

    STOP_FLAG.store(false, Ordering::SeqCst);
    BOUND_PORT.store(port, Ordering::SeqCst);
    RUNNING.store(true, Ordering::SeqCst);

    let server = Arc::new(server);
    let server_thread = Arc::clone(&server);
    thread::spawn(move || accept_loop(server_thread));

    Ok(status())
}

pub fn stop() -> ProxyStatus {
    STOP_FLAG.store(true, Ordering::SeqCst);
    // Unblock accept by connecting to self if running.
    let port = BOUND_PORT.load(Ordering::SeqCst);
    if port != 0 {
        let _ = std::net::TcpStream::connect(("127.0.0.1", port));
    }
    RUNNING.store(false, Ordering::SeqCst);
    BOUND_PORT.store(0, Ordering::SeqCst);
    status()
}

fn accept_loop(server: Arc<Server>) {
    loop {
        if STOP_FLAG.load(Ordering::SeqCst) {
            break;
        }
        match server.recv() {
            Ok(req) => {
                if STOP_FLAG.load(Ordering::SeqCst) {
                    let _ = req.respond(Response::from_string("shutting down").with_status_code(503));
                    break;
                }
                if let Err(e) = handle_request(req) {
                    eprintln!("proxy request error: {e}");
                }
            }
            Err(e) => {
                if STOP_FLAG.load(Ordering::SeqCst) {
                    break;
                }
                eprintln!("proxy accept error: {e}");
            }
        }
    }
    RUNNING.store(false, Ordering::SeqCst);
}

fn current_paths() -> Result<Paths, AppError> {
    PATHS_SLOT
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone()
        .ok_or_else(|| AppError::Message("proxy paths not set".into()))
}

fn handle_request(mut request: tiny_http::Request) -> Result<(), AppError> {
    let method = request.method().clone();
    let url = request.url().to_string();
    // Strip query
    let path = url.split('?').next().unwrap_or(&url).to_string();

    // Health for the proxy itself.
    if method == Method::Get && (path == "/health" || path == "/v1/health") {
        let _ = request.respond(Response::from_string(r#"{"ok":true}"#).with_header(
            Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap(),
        ));
        return Ok(());
    }

    let mut body = Vec::new();
    request.as_reader().read_to_end(&mut body)?;

    let paths = current_paths()?;
    let settings = settings_store::load_settings(&paths).unwrap_or_else(|_| {
        settings_store::default_settings(&paths)
    });
    let providers = provider_store::list_providers(&paths).unwrap_or_default();
    let candidates = pool::order_candidates(&providers, settings.pool_strategy);

    if candidates.is_empty() {
        let _ = request.respond(
            Response::from_string(r#"{"error":"no eligible providers in pool"}"#)
                .with_status_code(StatusCode(503))
                .with_header(
                    Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap(),
                ),
        );
        return Ok(());
    }

    let model_hint = extract_model(&body);
    let mut last_status = 502u16;
    let mut last_body = br#"{"error":"all providers failed"}"#.to_vec();
    let mut last_provider: Option<String> = None;

    for provider in &candidates {
        let started = Instant::now();
        match forward(&method, &path, &body, provider) {
            Ok((status, resp_body, prompt_tokens, completion_tokens)) => {
                let latency = started.elapsed().as_millis() as u64;
                let ok = status < 400;
                let _ = db::insert_request_log(
                    &paths,
                    Some(&provider.id),
                    model_hint.as_deref(),
                    method_str(&method),
                    &path,
                    status,
                    latency,
                    prompt_tokens,
                    completion_tokens,
                    ok,
                    if ok { "ok" } else { "upstream error" },
                );

                if ok || !is_failover_status(status) {
                    let mut response = Response::from_data(resp_body).with_status_code(StatusCode(status));
                    if let Ok(h) =
                        Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..])
                    {
                        response = response.with_header(h);
                    }
                    let _ = request.respond(response);
                    return Ok(());
                }

                // Cool down and try next.
                last_status = status;
                last_body = resp_body;
                last_provider = Some(provider.id.clone());
                let cooled = pool::with_cooldown(provider, 30);
                let _ = provider_store::upsert_provider(&paths, cooled);
            }
            Err(e) => {
                let latency = started.elapsed().as_millis() as u64;
                let _ = db::insert_request_log(
                    &paths,
                    Some(&provider.id),
                    model_hint.as_deref(),
                    method_str(&method),
                    &path,
                    0,
                    latency,
                    0,
                    0,
                    false,
                    &e,
                );
                last_status = 502;
                last_body = format!(r#"{{"error":"{}"}}"#, e.replace('"', "'")).into_bytes();
                last_provider = Some(provider.id.clone());
                let cooled = pool::with_cooldown(provider, 30);
                let _ = provider_store::upsert_provider(&paths, cooled);
            }
        }
    }

    let _ = last_provider;
    let mut response =
        Response::from_data(last_body).with_status_code(StatusCode(last_status));
    if let Ok(h) = Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]) {
        response = response.with_header(h);
    }
    let _ = request.respond(response);
    Ok(())
}

fn is_failover_status(status: u16) -> bool {
    matches!(status, 401 | 403 | 429) || (500..600).contains(&status)
}

fn method_str(m: &Method) -> &'static str {
    match *m {
        Method::Get => "GET",
        Method::Post => "POST",
        Method::Put => "PUT",
        Method::Delete => "DELETE",
        Method::Patch => "PATCH",
        Method::Head => "HEAD",
        Method::Options => "OPTIONS",
        _ => "GET",
    }
}

fn extract_model(body: &[u8]) -> Option<String> {
    let v: Value = serde_json::from_slice(body).ok()?;
    v.get("model")
        .and_then(|m| m.as_str())
        .map(|s| s.to_string())
}

fn forward(
    method: &Method,
    path: &str,
    body: &[u8],
    provider: &Provider,
) -> Result<(u16, Vec<u8>, u64, u64), String> {
    let base = provider.base_url.trim().trim_end_matches('/');
    // Map /v1/... onto provider base (which usually already ends with /v1).
    let suffix = if path.starts_with("/v1/") {
        &path[3..] // keep leading /
    } else if path == "/v1" {
        ""
    } else {
        path
    };
    let url = format!("{base}{suffix}");

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| e.to_string())?;

    let mut builder = match *method {
        Method::Get => client.get(&url),
        Method::Post => client.post(&url),
        Method::Put => client.put(&url),
        Method::Delete => client.delete(&url),
        Method::Patch => client.patch(&url),
        Method::Head => client.head(&url),
        _ => client.post(&url),
    };

    builder = builder
        .header("Authorization", format!("Bearer {}", provider.api_key))
        .header("Content-Type", "application/json");

    if !body.is_empty() && *method != Method::Get && *method != Method::Head {
        builder = builder.body(body.to_vec());
    }

    let resp = builder.send().map_err(|e| e.to_string())?;
    let status = resp.status().as_u16();
    let bytes = resp.bytes().map_err(|e| e.to_string())?.to_vec();
    let (pt, ct) = parse_usage(&bytes);
    Ok((status, bytes, pt, ct))
}

fn parse_usage(body: &[u8]) -> (u64, u64) {
    let Ok(v) = serde_json::from_slice::<Value>(body) else {
        return (0, 0);
    };
    let usage = v.get("usage");
    let pt = usage
        .and_then(|u| u.get("prompt_tokens"))
        .and_then(|x| x.as_u64())
        .unwrap_or(0);
    let ct = usage
        .and_then(|u| u.get("completion_tokens"))
        .and_then(|x| x.as_u64())
        .unwrap_or(0);
    (pt, ct)
}

/// Apply proxy base_url into Grok config when user enables proxy mode.
pub fn proxy_base_url(port: u16) -> String {
    format!("http://127.0.0.1:{port}/v1")
}

#[allow(dead_code)]
pub fn strategy_label(s: PoolStrategy) -> &'static str {
    match s {
        PoolStrategy::Priority => "priority",
        PoolStrategy::Weighted => "weighted",
        PoolStrategy::RoundRobin => "round_robin",
    }
}
