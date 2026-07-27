//! System tray: icon, state-reflecting tooltip, context menu, and a left-click
//! toggle of the master switch.

use palimpsest_core::UiLang;
use tauri::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};

use crate::i18n;
use crate::state::{self, AppState};

/// The tray menu's items, retained after construction.
///
/// `toggle` keeps the check mark in step with the master switch whatever flips
/// it; all four are held so [`retranslate`] can relabel the menu in place when
/// the UI language changes — settings apply live, and the tray is no exception.
pub struct TrayMenu {
    /// The "Enabled" check item (also the master-switch mirror).
    pub toggle: CheckMenuItem<Wry>,
    settings: MenuItem<Wry>,
    logs: MenuItem<Wry>,
    quit: MenuItem<Wry>,
}

/// Build the system tray in `lang`. Returns the menu items so the caller can
/// store them in [`AppState`].
pub fn build(app: &AppHandle, enabled: bool, lang: UiLang) -> tauri::Result<TrayMenu> {
    let s = i18n::strings(lang);
    let toggle = CheckMenuItem::with_id(app, "toggle", s.tray_enabled, true, enabled, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", s.tray_settings, true, None::<&str>)?;
    let logs = MenuItem::with_id(app, "logs", s.tray_logs, true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", s.tray_quit, true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&toggle, &settings, &logs, &separator, &quit])?;

    let icon = app
        .default_window_icon()
        .cloned()
        .expect("bundled default window icon");

    TrayIconBuilder::with_id("main")
        .icon(icon)
        .tooltip(if enabled { s.tooltip_on } else { s.tooltip_paused })
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(on_menu_event)
        .on_tray_icon_event(on_tray_icon_event)
        .build(app)?;

    Ok(TrayMenu { toggle, settings, logs, quit })
}

/// Relabel the menu for a new UI language. Best-effort: a failed relabel leaves
/// the previous wording, which is cosmetic, not a reason to fail a config save.
pub fn retranslate(menu: &TrayMenu, lang: UiLang) {
    let s = i18n::strings(lang);
    let _ = menu.toggle.set_text(s.tray_enabled);
    let _ = menu.settings.set_text(s.tray_settings);
    let _ = menu.logs.set_text(s.tray_logs);
    let _ = menu.quit.set_text(s.tray_quit);
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
