//! System tray setup for Grok Switch.

use crate::commands::enable_provider_flow;
use crate::core::account_store::list_accounts;
use crate::core::provider_store::list_providers;
use crate::core::settings_store::load_settings;
use crate::core::types::AppMode;
use crate::core::Paths;
use std::sync::Mutex;
use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, Runtime, Wry,
};

static TRAY_ICON: Mutex<Option<tauri::tray::TrayIcon<Wry>>> = Mutex::new(None);

/// Build tooltip: `Grok Switch · {mode}: {name}` when a current target is known.
pub fn tray_tooltip(paths: &Paths) -> String {
    let settings =
        load_settings(paths).unwrap_or_else(|_| crate::core::settings_store::default_settings(paths));

    match settings.current_mode {
        AppMode::Provider => {
            let name = settings
                .current_provider_id
                .as_ref()
                .and_then(|id| {
                    list_providers(paths)
                        .ok()
                        .and_then(|items| items.into_iter().find(|p| &p.id == id))
                        .map(|p| p.name)
                })
                .unwrap_or_else(|| "provider".into());
            format!("Grok Switch · 中转: {name}")
        }
        AppMode::Official => {
            let name = settings
                .current_account_id
                .as_ref()
                .and_then(|id| {
                    list_accounts(paths)
                        .ok()
                        .and_then(|items| items.into_iter().find(|a| &a.id == id))
                        .map(|a| a.name)
                })
                .unwrap_or_else(|| "account".into());
            format!("Grok Switch · 官方: {name}")
        }
        AppMode::None => "Grok Switch · 未启用".into(),
    }
}

fn show_main_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn recent_providers(paths: &Paths) -> (Vec<crate::core::types::Provider>, Option<String>) {
    let settings =
        load_settings(paths).unwrap_or_else(|_| crate::core::settings_store::default_settings(paths));
    let providers = list_providers(paths).unwrap_or_default();
    let current = settings.current_provider_id.clone();
    let mut recent: Vec<_> = providers.into_iter().collect();
    recent.sort_by(|a, b| {
        let ac = Some(&a.id) == current.as_ref();
        let bc = Some(&b.id) == current.as_ref();
        match (ac, bc) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => b.updated_at.cmp(&a.updated_at),
        }
    });
    recent.truncate(5);
    (recent, current)
}

fn build_menu<R: Runtime>(
    app: &AppHandle<R>,
    paths: &Paths,
) -> tauri::Result<(Menu<R>, Vec<MenuItem<R>>)> {
    let open_i = MenuItem::with_id(app, "open", "打开主界面", true, None::<&str>)?;
    let quit_i = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let refresh_i = MenuItem::with_id(app, "refresh", "刷新菜单", true, None::<&str>)?;
    let proxy_running = crate::core::proxy::status().running;
    let proxy_label = if proxy_running {
        "停止本地代理"
    } else {
        "启动本地代理"
    };
    let proxy_i = MenuItem::with_id(app, "proxy_toggle", proxy_label, true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;

    let (recent, current) = recent_providers(paths);
    let mut provider_items: Vec<MenuItem<R>> = Vec::new();
    for p in &recent {
        let mark = if Some(&p.id) == current.as_ref() {
            "✓ "
        } else {
            "   "
        };
        let label = format!("{mark}{}", p.name);
        let id = format!("prov:{}", p.id);
        if let Ok(item) = MenuItem::with_id(app, id, label, true, None::<&str>) {
            provider_items.push(item);
        }
    }

    let mut items: Vec<&dyn tauri::menu::IsMenuItem<R>> = Vec::new();
    for it in &provider_items {
        items.push(it);
    }
    if !provider_items.is_empty() {
        items.push(&sep);
    }
    items.push(&proxy_i);
    items.push(&sep2);
    items.push(&open_i);
    items.push(&refresh_i);
    items.push(&quit_i);

    let menu = Menu::with_items(app, &items)?;
    let _keep = (open_i, quit_i, refresh_i, proxy_i, sep, sep2);
    let _ = _keep;
    Ok((menu, provider_items))
}

/// Refresh tray menu + tooltip from current settings/providers.
pub fn refresh_tray(app: &AppHandle<Wry>, paths: &Paths) {
    let Ok(mut slot) = TRAY_ICON.lock() else {
        return;
    };
    let Some(tray) = slot.as_mut() else {
        return;
    };
    if let Ok((menu, _items)) = build_menu(app, paths) {
        let _ = tray.set_menu(Some(menu));
    }
    let _ = tray.set_tooltip(Some(tray_tooltip(paths)));
}

/// Create the tray icon when settings allow it.
pub fn setup_tray(app: &AppHandle<Wry>, paths: &Paths) -> tauri::Result<()> {
    let settings =
        load_settings(paths).unwrap_or_else(|_| crate::core::settings_store::default_settings(paths));
    if !settings.tray_enabled {
        return Ok(());
    }

    let (menu, _provider_items) = build_menu(app, paths)?;
    let tooltip = tray_tooltip(paths);
    let icon = app
        .default_window_icon()
        .cloned()
        .unwrap_or_else(|| Image::new_owned(vec![0, 0, 0, 255], 1, 1));

    let paths_for_menu = paths.clone();
    let paths_for_refresh = paths.clone();
    let tray = TrayIconBuilder::new()
        .icon(icon)
        .tooltip(&tooltip)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| {
            let id = event.id.as_ref();
            if id == "open" {
                show_main_window(app);
                return;
            }
            if id == "quit" {
                app.exit(0);
                return;
            }
            if id == "refresh" {
                refresh_tray(app, &paths_for_refresh);
                return;
            }
            if id == "proxy_toggle" {
                if crate::core::proxy::status().running {
                    let _ = crate::core::proxy::stop();
                    if let Ok(mut s) = load_settings(&paths_for_menu) {
                        s.proxy_enabled = false;
                        let _ = crate::core::settings_store::save_settings(&paths_for_menu, &s);
                    }
                    eprintln!("tray: proxy stopped");
                } else {
                    match crate::core::proxy::start(&paths_for_menu) {
                        Ok(st) => {
                            if let Ok(mut s) = load_settings(&paths_for_menu) {
                                s.proxy_enabled = true;
                                s.proxy_port = st.port;
                                let _ =
                                    crate::core::settings_store::save_settings(&paths_for_menu, &s);
                            }
                            eprintln!("tray: proxy started on {}", st.listen);
                        }
                        Err(e) => eprintln!("tray: proxy start failed: {e}"),
                    }
                }
                refresh_tray(app, &paths_for_menu);
                return;
            }
            if let Some(pid) = id.strip_prefix("prov:") {
                match enable_provider_flow(&paths_for_menu, pid, true) {
                    Ok(_) => {
                        eprintln!("tray: switched provider {pid}");
                        refresh_tray(app, &paths_for_menu);
                    }
                    Err(e) => eprintln!("tray switch failed: {e}"),
                }
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        })
        .build(app)?;

    if let Ok(mut slot) = TRAY_ICON.lock() {
        *slot = Some(tray);
    }

    Ok(())
}
