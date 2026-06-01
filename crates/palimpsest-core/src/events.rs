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
}

/// A control message from the UI to the engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// Revert the most recent correction, if any remains undoable.
    Undo,
}
