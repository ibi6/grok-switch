//! System tray setup for Grok Switch.

use crate::core::account_store::list_accounts;
use crate::core::provider_store::list_providers;
use crate::core::settings_store::load_settings;
use crate::core::types::AppMode;
use crate::core::Paths;
use tauri::{
    image::Image,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, Runtime,
};

/// Build tooltip: `Grok Switch · {mode}: {name}` when a current target is known.
pub fn tray_tooltip(paths: &Paths) -> String {
    let settings = load_settings(paths).unwrap_or_else(|_| {
        crate::core::settings_store::default_settings(paths)
    });

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
            format!("Grok Switch · provider: {name}")
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
            format!("Grok Switch · official: {name}")
        }
        AppMode::None => "Grok Switch · none".into(),
    }
}

fn show_main_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// Create the tray icon when settings allow it.
pub fn setup_tray<R: Runtime>(app: &AppHandle<R>, paths: &Paths) -> tauri::Result<()> {
    let settings = load_settings(paths).unwrap_or_else(|_| {
        crate::core::settings_store::default_settings(paths)
    });
    if !settings.tray_enabled {
        return Ok(());
    }

    let open_i = MenuItem::with_id(app, "open", "打开", true, None::<&str>)?;
    let quit_i = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open_i, &quit_i])?;

    let tooltip = tray_tooltip(paths);
    let icon = app
        .default_window_icon()
        .cloned()
        .unwrap_or_else(|| Image::new_owned(vec![0, 0, 0, 255], 1, 1));

    let _tray = TrayIconBuilder::new()
        .icon(icon)
        .tooltip(&tooltip)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_main_window(app),
            "quit" => {
                app.exit(0);
            }
            _ => {}
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

    Ok(())
}
