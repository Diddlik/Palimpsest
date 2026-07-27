//! Tauri IPC commands invoked from the settings window. They are deliberately
//! thin: read/write the shared config (which the engine reads lock-free) or push
//! a control message, then return. No blocking work on the UI thread.

use palimpsest_core::hotkey::{self, Unusable};
use palimpsest_core::{layouts, processes, Config, Lang, LayoutInfo, UiLang, UserWords};
use tauri::{AppHandle, State};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_updater::UpdaterExt;

use crate::i18n;
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

/// The concrete language the UI should render in: the configured one, or — for
/// [`UiLang::System`] — whatever Windows reports. Resolved here rather than in
/// the webview so the settings window and the tray can never disagree about what
/// "system" means.
#[tauri::command]
pub fn resolved_ui_language(state: State<AppState>) -> String {
    i18n::resolve(state.runtime.config().ui_language).code().to_string()
}

/// Turn a validation failure into something the user can act on, in `lang`.
///
/// The `Busy` case is split by modifier deliberately. Windows reserves most
/// `Win+…` shortcuts for the shell (Win+PageUp moves a window between monitors,
/// Win+K casts, Win+. opens emoji), so `RegisterHotKey` refuses them — but
/// blaming "another app" sends the user hunting for a culprit that does not
/// exist. Naming Windows and offering combinations that actually pass the probe
/// is the difference between a dead end and a fix.
fn hotkey_error(reason: Unusable, mods: u8, lang: UiLang) -> String {
    let s = i18n::strings(lang);
    match reason {
        Unusable::Typing => s.hotkey_typing,
        Unusable::Modifier => s.hotkey_modifier,
        Unusable::Busy if mods & palimpsest_core::config::MOD_WIN != 0 => s.hotkey_busy_win,
        Unusable::Busy => s.hotkey_busy,
    }
    .to_string()
}

/// Set the manual hotkey from a captured key + modifier bitmask, rejecting bare
/// typing keys, lone modifiers, and combinations already claimed by another app
/// or by Windows. Returns a user-facing error message on rejection.
#[tauri::command]
pub fn set_hotkey(app: AppHandle, state: State<AppState>, vk: u32, mods: u8) -> Result<(), String> {
    let lang = state.runtime.config().ui_language;
    hotkey::validate(vk, mods).map_err(|reason| hotkey_error(reason, mods, lang))?;
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

/// Set the "convert selection" hotkey from a captured key + modifier bitmask.
/// Validated exactly like [`set_hotkey`]; returns a user-facing error on
/// rejection.
#[tauri::command]
pub fn set_selection_hotkey(app: AppHandle, state: State<AppState>, vk: u32, mods: u8) -> Result<(), String> {
    let lang = state.runtime.config().ui_language;
    hotkey::validate(vk, mods).map_err(|reason| hotkey_error(reason, mods, lang))?;
    let mut config = (*state.runtime.config()).clone();
    config.selection_hotkey = vk;
    config.selection_hotkey_mods = mods;
    state::apply_config(&app, &state, config);
    Ok(())
}

/// Clear the "convert selection" hotkey so no key triggers a selection fix.
#[tauri::command]
pub fn clear_selection_hotkey(app: AppHandle, state: State<AppState>) {
    let mut config = (*state.runtime.config()).clone();
    config.selection_hotkey = 0;
    config.selection_hotkey_mods = 0;
    state::apply_config(&app, &state, config);
}

/// A configured hotkey that cannot be claimed right now.
#[derive(serde::Serialize)]
pub struct HotkeyConflict {
    /// Which binding: `"manual"` or `"selection"`.
    pub which: String,
    /// User-facing explanation, already phrased for display.
    pub message: String,
}

/// Re-check the *configured* hotkeys against the system.
///
/// A binding is validated when it is set, but the world moves on: another app
/// can claim the same chord afterwards, or the config may have been copied from
/// a machine where it was free. Palimpsest's hotkeys still fire (the low-level
/// hook sees keys before any global registration), but the other owner fires
/// too — so the user needs to be told rather than left guessing why something
/// else also happens. Called when the settings window opens.
#[tauri::command]
pub fn hotkey_conflicts(state: State<AppState>) -> Vec<HotkeyConflict> {
    let config = state.runtime.config();
    [
        ("manual", config.manual_hotkey, config.manual_hotkey_mods),
        ("selection", config.selection_hotkey, config.selection_hotkey_mods),
    ]
    .into_iter()
    .filter(|(_, vk, _)| *vk != 0) // 0 means the binding is switched off
    .filter_map(|(which, vk, mods)| {
        hotkey::validate(vk, mods).err().map(|reason| HotkeyConflict {
            which: which.to_string(),
            message: hotkey_error(reason, mods, config.ui_language),
        })
    })
    .collect()
}

/// Every word Palimpsest has learned from an undo, grouped by language. This is
/// the only typed text the app persists, so it must always be inspectable.
#[tauri::command]
pub fn list_user_words(state: State<AppState>) -> UserWords {
    (*state.runtime.user_words()).clone()
}

/// Forget one learned word, so Palimpsest may correct it again. `lang` is a
/// [`Lang::code`] value (`"en"`, `"ru"`, `"de"`); an unknown one is a no-op.
#[tauri::command]
pub fn forget_word(app: AppHandle, state: State<AppState>, lang: String, word: String) {
    let Some(lang) = lang_from_code(&lang) else {
        return;
    };
    if state.runtime.forget_word(lang, &word) {
        state::save_user_words(&app, &state.runtime.user_words());
    }
}

/// Forget every learned word — the "clear my dictionary" escape hatch that makes
/// the privacy trade-off reversible.
#[tauri::command]
pub fn clear_user_words(app: AppHandle, state: State<AppState>) {
    state.runtime.clear_user_words();
    state::save_user_words(&app, &state.runtime.user_words());
}

/// Outcome of an update check, together with the version already running.
#[derive(serde::Serialize)]
pub struct UpdateStatus {
    /// Version of this build, so the UI can show it without a second round-trip.
    pub current: String,
    /// The newer version on the release feed, or `None` when up to date.
    pub available: Option<String>,
    /// Release notes for `available`, when the feed carries any.
    pub notes: Option<String>,
}

/// Ask the release feed whether a newer build exists.
///
/// This is the only network call Palimpsest ever makes, and it sends nothing but
/// the request for `latest.json` — no typed text, no telemetry, no identifiers.
/// The signature on the update package is checked against the public key baked
/// into `tauri.conf.json`, so a compromised release host still cannot ship code.
#[tauri::command]
pub async fn check_update(app: AppHandle) -> Result<UpdateStatus, String> {
    let current = app.package_info().version.to_string();
    let update = app
        .updater()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| e.to_string())?;
    Ok(UpdateStatus {
        current,
        available: update.as_ref().map(|u| u.version.clone()),
        notes: update.and_then(|u| u.body),
    })
}

/// Download the pending update, run the installer, and come back up on the new
/// build.
///
/// The check is repeated rather than cached from [`check_update`], because the
/// user may sit in Settings for a long time and holding an `Update` across that
/// gap buys nothing — the feed is a single small request. On Windows the
/// installer needs this process gone before it can replace the executable, so
/// the app exits during `download_and_install`; `restart` is the path for
/// platforms where it does not.
#[tauri::command]
pub async fn install_update(app: AppHandle) -> Result<(), String> {
    let update = app
        .updater()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "No update available.".to_string())?;
    update
        .download_and_install(|_chunk, _total| {}, || {})
        .await
        .map_err(|e| e.to_string())?;
    app.restart()
}

/// Map the UI's short language code back to a [`Lang`].
fn lang_from_code(code: &str) -> Option<Lang> {
    [Lang::En, Lang::Ru, Lang::De].into_iter().find(|l| l.code() == code)
}
