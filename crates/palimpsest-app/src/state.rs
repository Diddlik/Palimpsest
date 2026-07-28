//! Shared application state, config persistence, and the single place that
//! applies a config change everywhere it needs to land: the live runtime, disk,
//! the tray, and the settings window.

use std::path::PathBuf;
use std::sync::Mutex;

use palimpsest_core::crypt::{Sealed, Unlocked};
use palimpsest_core::{Config, Diary, Runtime, UiLang, UserWords};
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
    /// The opt-in typing diary and its lock state. Behind a `Mutex` because the
    /// engine-event relay thread appends to it while IPC commands read, clear, and
    /// (un)lock it.
    pub diary: Mutex<DiaryState>,
}

/// In-memory diary plus everything needed to persist it correctly.
///
/// Three states matter: no password (plaintext file, always usable), password
/// set and unlocked this session (encrypted file, key held), and password set
/// but still locked (encrypted file, no key — records nothing and shows nothing
/// until the user unlocks). See [`crate::commands`] for the transitions.
pub struct DiaryState {
    /// Decrypted entries in memory. Empty while locked.
    pub diary: Diary,
    /// A password is set, so the on-disk file is encrypted.
    pub protected: bool,
    /// Usable this session: recording appends and the UI may show entries. Always
    /// true when there is no password; set by a successful unlock otherwise.
    pub unlocked: bool,
    /// The derived key, present exactly when `protected && unlocked`, so saves can
    /// re-encrypt without re-deriving or re-prompting.
    pub key: Option<Unlocked>,
}

impl DiaryState {
    /// A fresh, unprotected diary — the state before any password is set.
    fn plain(diary: Diary) -> Self {
        Self { diary, protected: false, unlocked: true, key: None }
    }
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
    // Keep the elevated title marker in the just-chosen language.
    crate::elevate::apply_window_title(app, lang);
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

/// Where the typing diary lives — its own file again, so the most sensitive
/// thing Palimpsest can hold is trivial to find and delete on its own.
fn diary_path(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join("diary.json"))
}

/// The on-disk diary file: either a plaintext diary, or a sealed envelope when a
/// password is set. `untagged` lets serde pick by shape — a `Sealed` has `ct`, a
/// `Diary` has `entries` — so old plaintext files still load.
#[derive(serde::Deserialize)]
#[serde(untagged)]
enum DiaryFile {
    Sealed(Sealed),
    Plain(Diary),
}

/// Load the diary into its correct lock state. A missing file is the normal case
/// (it is only ever written while the feature is on); a sealed file loads locked,
/// with no entries in memory until the user unlocks it.
pub fn load_diary(app: &AppHandle) -> DiaryState {
    let raw = diary_path(app).and_then(|p| std::fs::read_to_string(p).ok());
    match raw.and_then(|s| serde_json::from_str::<DiaryFile>(&s).ok()) {
        Some(DiaryFile::Sealed(_)) => {
            // Encrypted on disk: locked until unlocked with the password. The
            // engine records nothing while locked, because there is no key.
            DiaryState { diary: Diary::default(), protected: true, unlocked: false, key: None }
        }
        Some(DiaryFile::Plain(diary)) => DiaryState::plain(diary),
        None => DiaryState::plain(Diary::default()),
    }
}

/// Persist the diary from a locked [`DiaryState`], encrypting it when a password
/// is set. Best-effort, like the other writers.
///
/// Called on a batch of entries rather than on every word: the diary is capped at
/// [`palimpsest_core::diary::MAX_ENTRIES`], so re-serializing (and re-encrypting)
/// it per keystroke would rewrite the whole file several times a second while the
/// user types. The cost of the batch is losing the last few words to a hard
/// crash, which is the right side of that trade for a convenience log.
///
/// A protected-but-locked diary is never written: there is no key, and its
/// in-memory copy is empty, so there is nothing to persist and nothing to lose.
pub fn save_diary(app: &AppHandle, ds: &DiaryState) {
    let Some(path) = diary_path(app) else {
        tracing::warn!("no config directory available; diary not persisted");
        return;
    };
    let json = if ds.protected {
        let Some(key) = &ds.key else { return }; // locked — nothing to write
        match serde_json::to_vec(&ds.diary) {
            Ok(bytes) => match serde_json::to_string(&key.seal(&bytes)) {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!(error = %e, "failed to serialize the sealed diary");
                    return;
                }
            },
            Err(e) => {
                tracing::warn!(error = %e, "failed to serialize the diary");
                return;
            }
        }
    } else {
        match serde_json::to_string(&ds.diary) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(error = %e, "failed to serialize the diary");
                return;
            }
        }
    };
    if let Some(dir) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(dir) {
            tracing::warn!(error = %e, "failed to create config directory");
            return;
        }
    }
    if let Err(e) = std::fs::write(&path, json) {
        tracing::warn!(error = %e, "failed to write the diary");
    }
}

/// Format a Unix timestamp as `YYYY-MM-DD HH:MM:SS UTC` without pulling in a date
/// crate. UTC keeps it correct without a timezone database; the in-app list shows
/// local time via the webview's `toLocaleString`, so this is only the export's
/// concern. Uses Howard Hinnant's days-to-civil algorithm.
fn format_unix_utc(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let tod = secs % 86_400;
    let (h, m, s) = (tod / 3600, (tod % 3600) / 60, tod % 60);

    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { year + 1 } else { year };

    format!("{year:04}-{month:02}-{day:02} {h:02}:{m:02}:{s:02} UTC")
}

/// Read the sealed envelope from disk, for the unlock path. `None` when the file
/// is missing or is plaintext (i.e. not protected).
pub fn read_sealed_diary(app: &AppHandle) -> Option<Sealed> {
    let raw = diary_path(app).and_then(|p| std::fs::read_to_string(p).ok())?;
    match serde_json::from_str::<DiaryFile>(&raw).ok()? {
        DiaryFile::Sealed(s) => Some(s),
        DiaryFile::Plain(_) => None,
    }
}

/// Write the diary as a readable text file next to the config and reveal it in
/// Explorer, selected. Returns the file path. A fixed file plus `explorer
/// /select` avoids pulling in a dialog plugin and its capability grant.
pub fn export_diary_text(app: &AppHandle, diary: &Diary) -> std::io::Result<String> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::NotFound, e.to_string()))?;
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("diary-export.txt");

    let mut out = String::new();
    for e in &diary.entries {
        let kind = if e.clip { " [clipboard]" } else { "" };
        out.push_str(&format!("[{}] {}{}\n{}\n\n", format_unix_utc(e.at), e.app, kind, e.text));
    }
    std::fs::write(&path, out)?;

    // `/select,` opens the folder with the file highlighted; best-effort.
    let _ = std::process::Command::new("explorer")
        .arg("/select,")
        .arg(&path)
        .spawn();
    Ok(path.to_string_lossy().into_owned())
}

/// Write the in-memory diary out now, applying the short-entry guard first when
/// it is on. Used by the batching relay, the clear command, and process exit.
/// `final_flush` prunes even the last (still-open) entry — pass `true` only on
/// the way out, when nothing more will be appended.
pub fn flush_diary(app: &AppHandle, final_flush: bool) {
    let Some(state) = app.try_state::<AppState>() else { return };
    let skip_short = state.runtime.config().diary_skip_short;
    let Ok(mut ds) = state.diary.lock() else { return };
    if skip_short {
        ds.diary.prune_short(final_flush);
    }
    save_diary(app, &ds);
}

/// Reflect the on/off state in the tray tooltip, in the user's language.
pub fn update_tray_tooltip(app: &AppHandle, enabled: bool, lang: UiLang) {
    if let Some(tray) = app.tray_by_id("main") {
        let s = crate::i18n::strings(lang);
        let text = if enabled { s.tooltip_on } else { s.tooltip_paused };
        let _ = tray.set_tooltip(Some(text));
    }
}
