//! Tauri IPC commands invoked from the settings window. They are deliberately
//! thin: read/write the shared config (which the engine reads lock-free) or push
//! a control message, then return. No blocking work on the UI thread.

use palimpsest_core::hotkey::{self, Unusable};
use palimpsest_core::{layouts, processes, Config, LayoutInfo};
use tauri::{AppHandle, State};
use tauri_plugin_autostart::ManagerExt;

use crate::state::{self, AppState};

/// The current live config (a snapshot the UI renders its form from).
#[tauri::command]
pub fn get_config(state: State<AppState>) -> Config {
    (*state.runtime.config()).clone()
}

/// Replace the whole config (the settings form's "save on change").
#[tauri::command]
pub fn set_config(app: AppHandle, state: State<AppState>, config: Config) {
    state::apply_config(&app, &state, config);
}

/// Flip just the master switch — used by the settings window's enable toggle so
/// it stays cheap and never has to round-trip the entire form.
#[tauri::command]
pub fn set_enabled(app: AppHandle, state: State<AppState>, enabled: bool) {
    let mut config = (*state.runtime.config()).clone();
    config.enabled = enabled;
    state::apply_config(&app, &state, config);
}

/// Revert the most recent correction (the "undo last correction" button).
#[tauri::command]
pub fn undo(state: State<AppState>) {
    state.runtime.undo();
}

/// Whether Palimpsest is registered to launch at login.
#[tauri::command]
pub fn get_autostart(app: AppHandle) -> bool {
    app.autolaunch().is_enabled().unwrap_or(false)
}

/// Enable or disable launch-at-login.
#[tauri::command]
pub fn set_autostart(app: AppHandle, enabled: bool) -> Result<(), String> {
    let launcher = app.autolaunch();
    let result = if enabled { launcher.enable() } else { launcher.disable() };
    result.map_err(|e| e.to_string())
}

/// Open the diagnostics log folder in Explorer (export for a bug report).
#[tauri::command]
pub fn open_logs() -> Result<(), String> {
    crate::logging::reveal().map_err(|e| e.to_string())
}

/// Executable names of currently running processes, for the ignore-list picker.
#[tauri::command]
pub fn list_processes() -> Vec<String> {
    processes::running()
}

/// Keyboard layouts the user actually has installed, for the layout-pair picker.
#[tauri::command]
pub fn list_layouts() -> Vec<LayoutInfo> {
    layouts::installed()
}

/// Set the manual hotkey from a captured key + modifier bitmask, rejecting bare
/// typing keys, lone modifiers, and combinations already claimed by another app.
/// Returns a user-facing error message on rejection.
#[tauri::command]
pub fn set_hotkey(app: AppHandle, state: State<AppState>, vk: u32, mods: u8) -> Result<(), String> {
    hotkey::validate(vk, mods).map_err(|reason| match reason {
        Unusable::Typing => {
            "That key types a character — add a modifier (e.g. Ctrl+Shift) or pick a key like Pause, F8, or Insert.".to_string()
        }
        Unusable::Modifier => "A modifier key can't be used on its own.".to_string(),
        Unusable::Busy => "That combination is already in use by another app.".to_string(),
    })?;
    let mut config = (*state.runtime.config()).clone();
    config.manual_hotkey = vk;
    config.manual_hotkey_mods = mods;
    state::apply_config(&app, &state, config);
    Ok(())
}

/// Clear the manual hotkey so no key triggers a forced correction.
#[tauri::command]
pub fn clear_hotkey(app: AppHandle, state: State<AppState>) {
    let mut config = (*state.runtime.config()).clone();
    config.manual_hotkey = 0;
    config.manual_hotkey_mods = 0;
    state::apply_config(&app, &state, config);
}
