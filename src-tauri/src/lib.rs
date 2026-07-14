pub mod commands;
pub mod core;
mod tray;

use core::paths::Paths;
use std::fs;
use std::net::TcpListener;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Manager};

/// Shared application state injected into Tauri commands.
pub struct AppState {
    pub paths: Paths,
}

/// Best-effort single-instance guard (no extra crate; works offline).
/// Holds a localhost port for the process lifetime. If bind fails, another
/// instance is likely already running.
#[allow(dead_code)]
struct InstanceGuard(TcpListener);

static INSTANCE: Mutex<Option<InstanceGuard>> = Mutex::new(None);

fn try_acquire_single_instance() -> bool {
    match TcpListener::bind(("127.0.0.1", 47821)) {
        Ok(listener) => {
            if let Ok(mut slot) = INSTANCE.lock() {
                *slot = Some(InstanceGuard(listener));
            }
            true
        }
        Err(_) => false,
    }
}

fn focus_signal_path(paths: &Paths) -> std::path::PathBuf {
    paths.app_home.join("focus.signal")
}

/// Second instance writes this file so the first instance can focus its window.
fn request_focus(paths: &Paths) {
    let _ = paths.ensure_app_dirs();
    let path = focus_signal_path(paths);
    let _ = fs::write(
        &path,
        format!("{}", chrono::Local::now().timestamp_millis()),
    );
}

fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// Poll focus.signal and bring main window forward when another launch is attempted.
fn spawn_focus_watcher(app: AppHandle, paths: Paths) {
    thread::spawn(move || {
        let signal = focus_signal_path(&paths);
        let mut last = fs::read_to_string(&signal).unwrap_or_default();
        loop {
            thread::sleep(Duration::from_millis(400));
            let cur = fs::read_to_string(&signal).unwrap_or_default();
            if !cur.is_empty() && cur != last {
                last = cur;
                show_main_window(&app);
            }
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Resolve paths early so a second instance can still ping the first.
    let paths = Paths::resolve().unwrap_or_else(|_| {
        // Fallback: still try home via empty root failure path — use temp-ish
        Paths::from_root(
            dirs::home_dir().unwrap_or_else(|| std::env::temp_dir()),
        )
    });
    let _ = paths.ensure_app_dirs();

    if !try_acquire_single_instance() {
        request_focus(&paths);
        eprintln!(
            "Grok Switch is already running — requested focus on the existing window."
        );
        std::process::exit(0);
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            paths: paths.clone(),
        })
        .setup(move |app| {
            if let Err(err) = tray::setup_tray(app.handle(), &paths) {
                eprintln!("tray setup failed: {err}");
            }
            spawn_focus_watcher(app.handle().clone(), paths.clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::update_settings,
            commands::list_providers,
            commands::upsert_provider,
            commands::delete_provider,
            commands::enable_provider,
            commands::test_provider,
            commands::test_provider_draft,
            commands::list_accounts,
            commands::delete_account,
            commands::capture_current_account,
            commands::enable_account,
            commands::import_ccswitch_preview,
            commands::import_ccswitch_apply,
            commands::get_cli_status,
            commands::list_activity,
            commands::list_backups,
            commands::restore_backup,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Grok Switch");
}
