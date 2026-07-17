pub mod commands;
pub mod core;
mod tray;

use core::paths::Paths;
use std::fs;
use std::fs::OpenOptions;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Manager};

/// Shared application state injected into Tauri commands.
pub struct AppState {
    pub paths: Paths,
}

/// Holds an exclusive lock file for the process lifetime so a second instance
/// cannot start. A file lock (not a TCP port) avoids false positives when some
/// unrelated process already occupies a fixed port.
#[allow(dead_code)]
struct InstanceGuard(fs::File);

static INSTANCE: Mutex<Option<InstanceGuard>> = Mutex::new(None);

fn lock_path(paths: &Paths) -> std::path::PathBuf {
    paths.app_home.join("instance.lock")
}

fn try_acquire_single_instance(paths: &Paths) -> bool {
    let path = lock_path(paths);
    let file = match OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&path)
    {
        Ok(f) => f,
        Err(_) => return false,
    };

    if !try_lock_exclusive(&file) {
        return false;
    }

    if let Ok(mut slot) = INSTANCE.lock() {
        *slot = Some(InstanceGuard(file));
    }
    // Human-readable pid for diagnostics (separate from the lock fd).
    let _ = fs::write(
        paths.app_home.join("instance.pid"),
        format!("{}", std::process::id()),
    );
    true
}

#[cfg(windows)]
fn try_lock_exclusive(file: &fs::File) -> bool {
    use std::os::windows::io::AsRawHandle;
    // LOCKFILE_EXCLUSIVE_LOCK | LOCKFILE_FAIL_IMMEDIATELY
    const LOCKFILE_EXCLUSIVE_LOCK: u32 = 0x2;
    const LOCKFILE_FAIL_IMMEDIATELY: u32 = 0x1;

    #[repr(C)]
    struct Overlapped {
        internal: usize,
        internal_high: usize,
        offset: u32,
        offset_high: u32,
        h_event: *mut std::ffi::c_void,
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn LockFileEx(
            h_file: *mut std::ffi::c_void,
            dw_flags: u32,
            dw_reserved: u32,
            n_number_of_bytes_to_lock_low: u32,
            n_number_of_bytes_to_lock_high: u32,
            lp_overlapped: *mut Overlapped,
        ) -> i32;
    }

    let mut ov = Overlapped {
        internal: 0,
        internal_high: 0,
        offset: 0,
        offset_high: 0,
        h_event: std::ptr::null_mut(),
    };
    let ok = unsafe {
        LockFileEx(
            file.as_raw_handle() as *mut _,
            LOCKFILE_EXCLUSIVE_LOCK | LOCKFILE_FAIL_IMMEDIATELY,
            0,
            1,
            0,
            &mut ov,
        )
    };
    ok != 0
}

#[cfg(unix)]
fn try_lock_exclusive(file: &fs::File) -> bool {
    use std::os::unix::io::AsRawFd;
    const LOCK_EX: i32 = 2;
    const LOCK_NB: i32 = 4;
    extern "C" {
        fn flock(fd: i32, operation: i32) -> i32;
    }
    unsafe { flock(file.as_raw_fd(), LOCK_EX | LOCK_NB) == 0 }
}

#[cfg(not(any(windows, unix)))]
fn try_lock_exclusive(_file: &fs::File) -> bool {
    true
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
    let paths = Paths::resolve().unwrap_or_else(|_| {
        Paths::from_root(dirs::home_dir().unwrap_or_else(std::env::temp_dir))
    });
    let _ = paths.ensure_app_dirs();

    if !try_acquire_single_instance(&paths) {
        request_focus(&paths);
        eprintln!(
            "Grok Switch is already running — requested focus on the existing window."
        );
        std::process::exit(0);
    }

    let paths_for_close = paths.clone();
    tauri::Builder::default()
        .manage(AppState {
            paths: paths.clone(),
        })
        .setup(move |app| {
            if let Err(err) = tray::setup_tray(app.handle(), &paths) {
                eprintln!("tray setup failed: {err}");
            }
            spawn_focus_watcher(app.handle().clone(), paths.clone());

            // Auto-start local proxy when settings.proxy_enabled is sticky.
            match crate::core::settings_store::load_settings(&paths) {
                Ok(s) if s.proxy_enabled => {
                    match crate::core::proxy::start(&paths) {
                        Ok(st) => eprintln!("proxy auto-started on {}", st.listen),
                        Err(e) => eprintln!("proxy auto-start failed: {e}"),
                    }
                }
                _ => {}
            }

            // Close-to-tray when tray is enabled: hide window instead of quit.
            if let Some(window) = app.get_webview_window("main") {
                let paths_close = paths_for_close.clone();
                let handle = app.handle().clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        let tray_on = crate::core::settings_store::load_settings(&paths_close)
                            .map(|s| s.tray_enabled)
                            .unwrap_or(true);
                        if tray_on {
                            api.prevent_close();
                            if let Some(w) = handle.get_webview_window("main") {
                                let _ = w.hide();
                            }
                        }
                    }
                });
            }
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
            commands::upsert_account,
            commands::delete_account,
            commands::capture_current_account,
            commands::enable_account,
            commands::import_ccswitch_preview,
            commands::import_ccswitch_apply,
            commands::get_cli_status,
            commands::list_activity,
            commands::list_backups,
            commands::restore_backup,
            commands::open_grok_terminal,
            commands::list_skills,
            commands::get_skill,
            commands::upsert_skill,
            commands::delete_skill,
            commands::import_skills,
            commands::list_mcp_servers,
            commands::get_mcp_server,
            commands::upsert_mcp_server,
            commands::delete_mcp_server,
            commands::set_mcp_enabled,
            commands::test_mcp_server,
            commands::list_request_logs,
            commands::get_token_stats,
            commands::get_proxy_status,
            commands::start_proxy,
            commands::stop_proxy,
            commands::clear_provider_cooldown,
            commands::list_prompts,
            commands::upsert_prompt,
            commands::delete_prompt,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Grok Switch");
}
