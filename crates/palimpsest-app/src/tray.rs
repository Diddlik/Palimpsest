//! System tray: icon, state-reflecting tooltip, context menu, and a left-click
//! toggle of the master switch.

use tauri::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};

use crate::state::{self, AppState};

/// Build the system tray. Returns the "Enabled" check item so the caller can
/// store it in [`AppState`] and keep it in sync with toggles made elsewhere.
pub fn build(app: &AppHandle, enabled: bool) -> tauri::Result<CheckMenuItem<Wry>> {
    let toggle = CheckMenuItem::with_id(app, "toggle", "Enabled", true, enabled, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
    let logs = MenuItem::with_id(app, "logs", "Open logs folder", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Palimpsest", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&toggle, &settings, &logs, &separator, &quit])?;

    let icon = app
        .default_window_icon()
        .cloned()
        .expect("bundled default window icon");

    TrayIconBuilder::with_id("main")
        .icon(icon)
        .tooltip(if enabled { "Palimpsest — on" } else { "Palimpsest — paused" })
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(on_menu_event)
        .on_tray_icon_event(on_tray_icon_event)
        .build(app)?;

    Ok(toggle)
}

fn on_menu_event(app: &AppHandle, event: MenuEvent) {
    match event.id.as_ref() {
        "toggle" => toggle_enabled(app),
        "settings" => show_settings(app),
        "logs" => {
            let _ = crate::logging::reveal();
        }
        "quit" => app.exit(0),
        _ => {}
    }
}

fn on_tray_icon_event(tray: &TrayIcon, event: TrayIconEvent) {
    // Left-click (on release) toggles the master switch; the context menu opens
    // on right-click only (`show_menu_on_left_click(false)`).
    if let TrayIconEvent::Click {
        button: MouseButton::Left,
        button_state: MouseButtonState::Up,
        ..
    } = event
    {
        toggle_enabled(tray.app_handle());
    }
}

/// Flip the master switch, persisting and syncing every surface.
pub fn toggle_enabled(app: &AppHandle) {
    let state = app.state::<AppState>();
    let mut config = (*state.runtime.config()).clone();
    config.enabled = !config.enabled;
    state::apply_config(app, &state, config);
}

/// Reveal and focus the settings window (it starts hidden and only hides on
/// close, so it persists for the life of the process).
pub fn show_settings(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}
