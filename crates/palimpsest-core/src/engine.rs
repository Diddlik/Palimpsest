//! The engine thread — the "brain" side of the hook/engine split.
//!
//! It owns the receiving end of the hook channel and drains it without ever
//! blocking the hook. Each character key-down feeds the [`WordBuffer`]; when a
//! word boundary completes a word, the engine reinterprets it under the active
//! and candidate layouts ([`crate::reinterpret`]), scores each interpretation
//! ([`crate::scoring`]), decides whether to correct it ([`crate::analyze`]), and
//! — new in Phase 3 — executes the correction ([`crate::execute`]).
//!
//! The manual hotkey ([`Config::manual_hotkey`]) forces a re-interpretation of
//! the word in progress (or, if none, the last space-finished word) even when
//! the automatic heuristic wasn't confident enough.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use arc_swap::ArcSwap;
use crossbeam_channel::{select, Receiver, Sender};

use crate::analyze::{analyze, suggest, Decision, Reinterpreter};
use crate::buffer::{Completed, Key, WordBuffer};
use crate::config::{is_ignored, Config};
use crate::context::{focused_is_password, Context};
use crate::events::{Command, CorrectionEvent, EngineEvent};
use crate::reinterpret::{current_layout_klid, WinReinterpreter};
use crate::scoring::Models;
use crate::{execute, RawKey};

/// How long to block on the channel before re-checking the stop flag. Keeps
/// shutdown responsive without busy-waiting.
const POLL_INTERVAL: Duration = Duration::from_millis(200);

/// `VK_SPACE` — the only boundary we auto-correct on. After a space the caret
/// sits right past the word+space, so the backspace count is exact. Tab/Enter
/// move the caret or submit, so auto-correcting them is unsafe; we skip them.
const VK_SPACE: u32 = 0x20;

/// Map a modifier virtual-key (generic or L/R variant) to its [`Config`] bit,
/// or `None` for non-modifier keys. Lets the engine track which modifiers are
/// held so it can recognise chorded hotkeys like Ctrl+Shift+K.
fn modifier_bit(vk: u32) -> Option<u8> {
    use crate::config::{MOD_ALT, MOD_CONTROL, MOD_SHIFT, MOD_WIN};
    match vk {
        0x10 | 0xA0 | 0xA1 => Some(MOD_SHIFT),   // VK_SHIFT / L / R
        0x11 | 0xA2 | 0xA3 => Some(MOD_CONTROL), // VK_CONTROL / L / R
        0x12 | 0xA4 | 0xA5 => Some(MOD_ALT),     // VK_MENU / L / R
        0x5B | 0x5C => Some(MOD_WIN),            // L / R Win
        _ => None,
    }
}

/// The chord-match rule, factored out so it is testable without a live engine:
/// the main key and the held modifiers must match exactly. A `manual_hotkey` of
/// `0` is disabled; the exact modifier match means a chord never fires on a
/// superset of its keys (so Ctrl+Shift+K doesn't trigger plain Shift+K).
fn hotkey_matches(vk: u32, mods: u8, config: &Config) -> bool {
    config.manual_hotkey != 0
        && vk == config.manual_hotkey
        && mods == config.manual_hotkey_mods
}

/// The last word finished with a space, kept so the manual hotkey can still fix
/// it after the fact when auto-correction declined.
struct LastWord {
    keys: Vec<Key>,
    layout: String,
}

/// The most recent correction the engine made, retained so it can be reverted by
/// the [`Command::Undo`] path (the UI's "undo last correction").
struct LastCorrection {
    /// The text the user actually typed (what undo retypes).
    original: String,
    /// Character count of the corrected text that was injected (how many
    /// backspaces undo must send).
    corrected_len: usize,
    /// KLID that was active before the correction (what undo restores).
    from_layout: String,
    /// The boundary char that was re-emitted, if any.
    boundary: Option<char>,
}

/// Mutable per-thread engine state.
struct Engine {
    config: Arc<ArcSwap<Config>>,
    buffer: WordBuffer,
    models: Models,
    reinterp: WinReinterpreter,
    context: Context,
    last: Option<LastWord>,
    last_correction: Option<LastCorrection>,
    events: Sender<EngineEvent>,
    /// Currently-held modifier keys (bitmask of `MOD_*`), tracked from the key
    /// stream so chorded hotkeys can be recognised.
    mods: u8,
}

impl Engine {
    fn new(config: Arc<ArcSwap<Config>>, events: Sender<EngineEvent>) -> Self {
        Self {
            config,
            buffer: WordBuffer::new(),
            models: Models::builtin(),
            reinterp: WinReinterpreter::new(),
            context: Context::new(),
            last: None,
            last_correction: None,
            events,
            mods: 0,
        }
    }

    /// Reason the foreground context forbids correcting right now, if any: a
    /// focused password field, or an ignore-listed process. Checked before every
    /// auto and manual correction so we stay silent where it matters.
    fn blocked(&mut self, config: &Config) -> Option<String> {
        if focused_is_password() {
            return Some("password field".into());
        }
        match self.context.foreground_exe() {
            Some(exe) if is_ignored(&exe, &config.ignore_processes) => Some(exe),
            _ => None,
        }
    }

    /// Handle one observed keystroke. Ignores input we should not analyse: our
    /// own injected output and OS-injected events. Modifier edges update the held
    /// state; other key-ups are dropped.
    fn on_key(&mut self, key: RawKey) {
        if key.ours || key.injected {
            return;
        }
        // Track modifier state from both press and release. Modifiers never form
        // words nor fire the hotkey on their own.
        if let Some(bit) = modifier_bit(key.vk) {
            if key.up {
                self.mods &= !bit;
            } else {
                self.mods |= bit;
            }
            return;
        }
        if key.up {
            return;
        }
        let config = self.config.load();
        if !config.enabled {
            self.buffer.clear();
            self.last = None;
            return;
        }
        if self.matches_hotkey(key.vk, &config) {
            self.on_manual(&config);
            return; // the hotkey itself is not part of any word
        }
        if let Some(done) = self.buffer.push(key) {
            self.on_word(done, &config);
        }
    }

    /// Whether `vk` plus the currently-held modifiers match the configured
    /// manual hotkey.
    fn matches_hotkey(&self, vk: u32, config: &Config) -> bool {
        hotkey_matches(vk, self.mods, config)
    }

    /// A completed word: auto-correct it if it ended with a space and the
    /// heuristic is confident; otherwise remember it for the manual hotkey.
    fn on_word(&mut self, done: Completed, config: &Config) {
        if done.boundary != VK_SPACE {
            self.last = None; // can't safely act on tab/enter-finished words
            return;
        }
        if let Some(reason) = self.blocked(config) {
            tracing::debug!(%reason, "correction suppressed by context");
            self.last = None;
            return;
        }
        let current = current_layout_klid();
        let decision = analyze(
            &done.keys,
            &current,
            &config.layout_pairs,
            &self.reinterp as &dyn Reinterpreter,
            &self.models,
            config.sensitivity,
        );
        match decision {
            Decision::Keep => {
                self.last = Some(LastWord { keys: done.keys, layout: current });
            }
            Decision::Correct { .. } => {
                self.run(&done.keys, &current, &decision, Some(' '), false);
                self.last = None;
            }
        }
    }

    /// Manual hotkey: force-correct the in-progress word, or the last
    /// space-finished one if nothing is being typed.
    fn on_manual(&mut self, config: &Config) {
        if let Some(reason) = self.blocked(config) {
            tracing::debug!(%reason, "manual correction suppressed by context");
            return;
        }
        let current = current_layout_klid();
        if !self.buffer.is_empty() {
            let keys = self.buffer.take_current();
            let decision =
                suggest(&keys, &current, &config.layout_pairs, &self.reinterp, &self.models);
            // In-progress word: no boundary to re-emit.
            self.run(&keys, &current, &decision, None, true);
            self.last = None;
        } else if let Some(last) = self.last.take() {
            let decision = suggest(
                &last.keys,
                &last.layout,
                &config.layout_pairs,
                &self.reinterp,
                &self.models,
            );
            self.run(&last.keys, &last.layout, &decision, Some(' '), true);
        }
    }

    /// Execute a decision (no-op for `Keep`). On a correction it performs the
    /// edit, remembers it for undo, and emits a [`EngineEvent::Corrected`] for
    /// the UI. `keys`/`from_layout` describe the input so we can reconstruct the
    /// original text (for undo and the event); `manual` flags the hotkey path.
    fn run(
        &mut self,
        keys: &[Key],
        from_layout: &str,
        decision: &Decision,
        boundary: Option<char>,
        manual: bool,
    ) {
        let Decision::Correct { backspaces: _, text, target_layout } = decision else {
            return;
        };
        let original = self.reinterp.reinterpret(keys, from_layout);
        // Privacy: the typed text is only ever logged at DEBUG, never at the
        // default level — Palimpsest must not record what the user types.
        tracing::debug!(%target_layout, manual, original = %original, corrected = %text, "correcting");
        if let Err(e) = execute::apply(decision, boundary) {
            tracing::warn!(error = %e, "correction failed");
            return;
        }
        self.last_correction = Some(LastCorrection {
            original: original.clone(),
            corrected_len: text.chars().count(),
            from_layout: from_layout.to_string(),
            boundary,
        });
        let _ = self.events.send(EngineEvent::Corrected(CorrectionEvent {
            original,
            corrected: text.clone(),
            target_layout: target_layout.clone(),
            manual,
        }));
    }

    /// Revert the most recent correction, if one is still undoable. Erases the
    /// corrected text, retypes the original, restores the prior layout, and
    /// emits [`EngineEvent::Undone`].
    fn on_undo(&mut self) {
        let Some(lc) = self.last_correction.take() else {
            tracing::debug!("undo requested but nothing to undo");
            return;
        };
        tracing::debug!(original = %lc.original, "undoing last correction");
        if let Err(e) = execute::undo(&lc.original, lc.corrected_len, &lc.from_layout, lc.boundary) {
            tracing::warn!(error = %e, "undo failed");
            return;
        }
        let _ = self.events.send(EngineEvent::Undone(CorrectionEvent {
            original: lc.original,
            corrected: String::new(),
            target_layout: lc.from_layout,
            manual: false,
        }));
    }
}

/// Run the engine loop until `running` is cleared (then the channels drain) or
/// the key channel disconnects. Intended to own a dedicated thread; blocks the
/// caller.
///
/// - `rx` — observed keystrokes from the hook.
/// - `commands` — UI control messages (e.g. [`Command::Undo`]).
/// - `config` — shared with the UI for live, lock-free updates.
/// - `running` — cleared on shutdown to break the loop.
/// - `events` — outbound channel for [`EngineEvent`]s the UI displays.
pub fn run(
    rx: Receiver<RawKey>,
    commands: Receiver<Command>,
    config: Arc<ArcSwap<Config>>,
    running: Arc<AtomicBool>,
    events: Sender<EngineEvent>,
) {
    let mut engine = Engine::new(config, events);
    while running.load(Ordering::Relaxed) {
        select! {
            recv(rx) -> msg => match msg {
                Ok(key) => engine.on_key(key),
                Err(_) => break, // hook channel disconnected
            },
            recv(commands) -> msg => match msg {
                Ok(Command::Undo) => engine.on_undo(),
                Err(_) => {} // UI dropped its sender; keep running until `running` clears
            },
            // Wake periodically so a cleared `running` flag is noticed promptly.
            default(POLL_INTERVAL) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{MOD_CONTROL, MOD_SHIFT};

    #[test]
    fn modifier_bits_map_generic_and_lr() {
        assert_eq!(modifier_bit(0x11), Some(MOD_CONTROL)); // VK_CONTROL
        assert_eq!(modifier_bit(0xA3), Some(MOD_CONTROL)); // VK_RCONTROL
        assert_eq!(modifier_bit(0x10), Some(MOD_SHIFT)); // VK_SHIFT
        assert_eq!(modifier_bit(0xA0), Some(MOD_SHIFT)); // VK_LSHIFT
        assert_eq!(modifier_bit(0x4B), None); // 'K' is not a modifier
    }

    fn config_with(hotkey: u32, mods: u8) -> Config {
        Config { manual_hotkey: hotkey, manual_hotkey_mods: mods, ..Config::default() }
    }

    #[test]
    fn bare_hotkey_matches_only_without_modifiers() {
        let cfg = config_with(0x13, 0); // bare Pause
        assert!(hotkey_matches(0x13, 0, &cfg));
        assert!(!hotkey_matches(0x13, MOD_SHIFT, &cfg)); // a modifier held → no match
        assert!(!hotkey_matches(0x14, 0, &cfg)); // wrong key
    }

    #[test]
    fn chord_requires_exact_modifiers() {
        let cfg = config_with(0x4B, MOD_CONTROL | MOD_SHIFT); // Ctrl+Shift+K
        assert!(hotkey_matches(0x4B, MOD_CONTROL | MOD_SHIFT, &cfg));
        assert!(!hotkey_matches(0x4B, MOD_SHIFT, &cfg)); // missing Ctrl
        assert!(!hotkey_matches(0x4B, MOD_CONTROL | MOD_SHIFT | crate::config::MOD_ALT, &cfg)); // superset
        assert!(!hotkey_matches(0x4B, 0, &cfg)); // plain K typing
    }

    #[test]
    fn cleared_hotkey_never_matches() {
        let cfg = config_with(0, 0); // disabled
        assert!(!hotkey_matches(0, 0, &cfg));
        assert!(!hotkey_matches(0x13, 0, &cfg));
    }
}
