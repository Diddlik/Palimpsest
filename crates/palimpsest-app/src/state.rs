//! Shared application state, config persistence, and the single place that
//! applies a config change everywhere it needs to land: the live runtime, disk,
//! the tray, and the settings window.

use std::path::PathBuf;

use palimpsest_core::{Config, Runtime, UiLang, UserWords};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::tray::TrayMenu;

/// State managed by Tauri and shared across commands and tray handlers.
pub struct AppState {
    /// The running core (hook + engine): holds the live config and the control
    /// surface (`set_config`, `undo`, event stream).
    pub runtime: Runtime,
    /// The tray menu, retained so a toggle from any source (tray click, menu,
    /// settings window) keeps every surface in sync — and so a language change
    /// can relabel it without a restart.
    pub tray: TrayMenu,
}

/// Status pushed to the settings window so its controls reflect the live state.
#[derive(Clone, Serialize)]
pub struct StatusPayload {
    pub enabled: bool,
}

/// Where the user's config lives on disk.
fn config_path(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join("config.json"))
}

/// Load the persisted config, falling back to defaults when it is missing or
/// unreadable. `#[serde(default)]` on [`Config`] makes partial/older files
/// forward-compatible.
pub fn load_config(app: &AppHandle) -> Config {
    config_path(app)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Persist the config as pretty JSON. Best-effort; logs on failure rather than
/// disrupting the user.
pub fn save_config(app: &AppHandle, config: &Config) {
    let Some(path) = config_path(app) else {
        tracing::warn!("no config directory available; config not persisted");
        return;
    };
    let json = match serde_json::to_string_pretty(config) {
        Ok(j) => j,
        Err(e) => {
            tracing::warn!(error = %e, "failed to serialize config");
            return;
        }
    };
    if let Some(dir) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(dir) {
            tracing::warn!(error = %e, "failed to create config directory");
            return;
        }
    }
    if let Err(e) = std::fs::write(&path, json) {
        tracing::warn!(error = %e, "failed to write config");
    }
}

/// Apply a new config to every surface: swap it into the live runtime (engine
/// picks it up lock-free), persist it, sync the tray's check item and tooltip,
/// and notify the settings window. The single choke point so nothing drifts.
pub fn apply_config(app: &AppHandle, state: &AppState, config: Config) {
    let enabled = config.enabled;
    let lang = config.ui_language;
    state.runtime.set_config(config.clone());
    save_config(app, &config);
    let _ = state.tray.toggle.set_checked(enabled);
    crate::tray::retranslate(&state.tray, lang);
    update_tray_tooltip(app, enabled, lang);
    let _ = app.emit("status", StatusPayload { enabled });
}

/// Where the learned-word dictionary lives — beside the config, but in its own
/// file: it is data the engine writes, not settings the user edits, and keeping
/// it separate makes it trivial to inspect or delete on its own.
fn user_words_path(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join("user_words.json"))
}

/// Load the learned words, falling back to an empty dictionary when the file is
/// missing or unreadable — never a reason to fail startup.
pub fn load_user_words(app: &AppHandle) -> UserWords {
    user_words_path(app)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Persist the learned words. Best-effort, like [`save_config`]: a failure to
/// write costs the user a repeated undo, not a broken session.
pub fn save_user_words(app: &AppHandle, words: &UserWords) {
    let Some(path) = user_words_path(app) else {
        tracing::warn!("no config directory available; learned words not persisted");
        return;
    };
    let json = match serde_json::to_string_pretty(words) {
        Ok(j) => j,
        Err(e) => {
            tracing::warn!(error = %e, "failed to serialize learned words");
            return;
        }
    };
    if let Some(dir) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(dir) {
            tracing::warn!(error = %e, "failed to create config directory");
            return;
        }
    }
    if let Err(e) = std::fs::write(&path, json) {
        tracing::warn!(error = %e, "failed to write learned words");
    }
}

/// Reflect the on/off state in the tray tooltip, in the user's language.
pub fn update_tray_tooltip(app: &AppHandle, enabled: bool, lang: UiLang) {
    if let Some(tray) = app.tray_by_id("main") {
        let s = crate::i18n::strings(lang);
        let text = if enabled { s.tooltip_on } else { s.tooltip_paused };
        let _ = tray.set_tooltip(Some(text));
    }
}
