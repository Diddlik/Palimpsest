//! Palimpsest core — the OS-facing primitives and (later) the heuristics engine.
//!
//! Phase 0 scope: a low-level keyboard hook that observes keystrokes, and an
//! injector that synthesizes Unicode input. Both cooperate through a single
//! `dwExtraInfo` signature so the hook can recognise — and ignore — its own
//! injected output, which is the mechanism that prevents an infinite
//! correction feedback loop.

/// Magic value stamped into `dwExtraInfo` on every synthetic input event we
/// generate. The hook compares against this to identify keystrokes that
/// Palimpsest itself produced. ASCII "PALI".
pub const PALIMPSEST_SIGNATURE: usize = 0x5041_4C49;

/// A single keystroke as observed by the low-level keyboard hook.
///
/// This is intentionally tiny and `Copy`: the hook callback builds one and
/// forwards it over a channel, doing zero further work on the OS's thread.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawKey {
    /// Virtual-key code (`VK_*`).
    pub vk: u32,
    /// Hardware scan code — the layout-independent physical key identity that
    /// later phases reinterpret under candidate layouts.
    pub scan: u32,
    /// `true` if Windows flagged this event as injected (`LLKHF_INJECTED`).
    pub injected: bool,
    /// `true` if this event carries our [`PALIMPSEST_SIGNATURE`] — i.e. we
    /// produced it and must not react to it.
    pub ours: bool,
    /// `true` for key-up, `false` for key-down.
    pub up: bool,
    /// System tick count when the event was generated.
    pub time: u32,
}

pub mod analyze;
pub use analyze::{
    analyze, suggest, suggest_text, Decision, Reinterpreter, TextCorrection, Transliterator,
};

pub mod buffer;
pub use buffer::{Key, WordBuffer};

pub mod config;
pub use config::{is_ignored, migrate_legacy_hotkeys, Config, LayoutPair, UiLang};

pub mod events;
pub use events::{Command, CorrectionEvent, EngineEvent};

#[cfg(windows)]
pub mod clipboard;

#[cfg(windows)]
pub mod context;

pub mod phonetic;

pub mod scoring;
pub use scoring::{Lang, Models};

pub mod user_dict;
pub use user_dict::UserWords;

#[cfg(windows)]
pub mod engine;

#[cfg(windows)]
pub mod execute;

#[cfg(windows)]
pub mod hook;

#[cfg(windows)]
pub mod hotkey;

#[cfg(windows)]
pub mod layouts;
#[cfg(windows)]
pub use layouts::LayoutInfo;

#[cfg(windows)]
pub mod processes;

#[cfg(windows)]
pub mod inject;

#[cfg(windows)]
pub mod layout;

#[cfg(windows)]
pub mod reinterpret;

#[cfg(windows)]
pub mod runtime;

#[cfg(windows)]
pub use runtime::Runtime;
