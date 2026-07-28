//! Messages crossing the engine ↔ UI boundary.
//!
//! The engine runs headless on its own thread; the Tauri UI (Phase 5) lives on
//! the main thread. They communicate through two lock-free channels carried by
//! the [`crate::runtime::Runtime`]: the engine pushes [`EngineEvent`]s outward
//! (for the status toast and correction history), and the UI pushes
//! [`Command`]s inward (currently just "undo"). Everything here is pure data —
//! no Windows types — so it is shared across both threads and is `serde`-ready
//! for Tauri IPC.

use serde::Serialize;

/// A correction the engine performed, surfaced to the UI for the status toast
/// and the history list.
#[derive(Debug, Clone, Serialize)]
pub struct CorrectionEvent {
    /// The text as the user actually typed it (in the wrong layout).
    pub original: String,
    /// The corrected text that replaced it.
    pub corrected: String,
    /// KLID the OS layout was switched to as part of the correction.
    pub target_layout: String,
    /// `true` if triggered by the manual hotkey rather than automatically.
    pub manual: bool,
}

/// Something the engine did, sent to the UI thread for display.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum EngineEvent {
    /// A word was corrected (auto or manual).
    Corrected(CorrectionEvent),
    /// The most recent correction was reverted.
    Undone(CorrectionEvent),
    /// An undo taught Palimpsest that a word was right as typed, so it was added
    /// to the user dictionary. The UI surfaces this (it is the only path that
    /// persists typed text) and the app persists the dictionary on receipt.
    Learned {
        /// Language the word was learned for — a [`crate::Lang::code`] value.
        lang: String,
        /// The word, as stored: trimmed and lowercased.
        word: String,
    },
    /// A word was typed, for the **opt-in** typing diary
    /// ([`crate::Config::diary_enabled`]). Emitted only when the diary is on and
    /// only from an unblocked context; the app appends it to [`crate::diary`].
    /// Never emitted otherwise, so the channel carries no typed text when the
    /// feature is off.
    Typed {
        /// Unix seconds the word was finished (or the copy was made).
        at: u64,
        /// Executable name of the app it was typed into.
        app: String,
        /// The word as it appeared on screen, or the copied text for a clipboard
        /// capture.
        word: String,
        /// `true` when this came from the clipboard rather than the keyboard.
        clip: bool,
    },
}

/// A control message from the UI to the engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// Revert the most recent correction, if any remains undoable.
    Undo,
}
