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

use crate::analyze::{analyze, suggest, suggest_text, Decision, Reinterpreter};
use crate::buffer::{Completed, WordBuffer};
use crate::config::{is_ignored, Config};
use crate::context::{focused_is_password, Context};
use crate::events::{Command, CorrectionEvent, EngineEvent};
use crate::reinterpret::{current_layout_klid, WinReinterpreter};
use crate::scoring::{Lang, Models};
use crate::user_dict::UserWords;
use crate::{clipboard, execute, inject, RawKey};

/// How long to block on the channel before re-checking the stop flag. Keeps
/// shutdown responsive without busy-waiting.
const POLL_INTERVAL: Duration = Duration::from_millis(200);

/// How long to keep waiting for the foreground app to service the synthetic
/// `Ctrl+C`, and how often to re-check while waiting.
///
/// This used to be a single fixed 120 ms sleep, which is the wrong shape: fast
/// apps made the user wait for nothing, and slow ones (Electron, browsers,
/// Office, anything over RDP) simply hadn't answered yet, so the clipboard read
/// came back empty and the conversion silently did nothing — the "press it three
/// or four times" symptom. Polling instead returns as soon as the text lands,
/// usually well under the old fixed delay, and tolerates a slow app.
const SELECTION_COPY_TIMEOUT: Duration = Duration::from_millis(600);
/// Gap between clipboard checks while waiting for the copy.
const SELECTION_POLL_INTERVAL: Duration = Duration::from_millis(15);

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

/// The chord-match rule for the "convert selection" hotkey — the same exact-match
/// semantics as [`hotkey_matches`], against [`Config::selection_hotkey`]. A value
/// of `0` disables it.
fn selection_hotkey_matches(vk: u32, mods: u8, config: &Config) -> bool {
    config.selection_hotkey != 0
        && vk == config.selection_hotkey
        && mods == config.selection_hotkey_mods
}

/// The chord-match rule for the "undo the last correction" hotkey — the same
/// exact-match semantics as [`hotkey_matches`], against [`Config::undo_hotkey`].
/// A value of `0` disables it.
fn undo_hotkey_matches(vk: u32, mods: u8, config: &Config) -> bool {
    config.undo_hotkey != 0 && vk == config.undo_hotkey && mods == config.undo_hotkey_mods
}

/// Wall-clock seconds since the Unix epoch, for diary timestamps. A clock set
/// before 1970 (or otherwise unreadable) yields `0` rather than panicking — a
/// wrong timestamp on a diary entry is not worth taking the app down for.
fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Wait for the synthetic `Ctrl+C` to land, returning the copied text.
///
/// `before` is the clipboard sequence number taken just before the copy was
/// sent; a change means somebody published something, which — right after our
/// own `Ctrl+C` — is the selection. Watching the counter instead of reading the
/// clipboard matters: the counter takes no lock, whereas polling `get_text` meant
/// hammering `OpenClipboard` every few milliseconds against the very app that was
/// trying to publish the copy. That contention denied us the lock
/// (`ERROR_ACCESS_DENIED`) and left the foreground app visibly frozen.
///
/// Returns `None` if nothing arrives within [`SELECTION_COPY_TIMEOUT`], meaning
/// either that nothing was selected or that the app never serviced the copy —
/// both harmless no-ops, and both leaving the clipboard untouched.
fn await_selection(before: u32) -> Option<String> {
    let deadline = std::time::Instant::now() + SELECTION_COPY_TIMEOUT;
    loop {
        if clipboard::sequence() != before {
            if let Some(text) = clipboard::get_text() {
                if !text.is_empty() {
                    return Some(text);
                }
            }
        }
        if std::time::Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(SELECTION_POLL_INTERVAL);
    }
}

/// Put `saved` back on the clipboard after a selection conversion borrowed it.
/// When the clipboard held no text to begin with, leave it empty.
///
/// A failure here is not cosmetic — we emptied the clipboard on the way in, so
/// losing the restore loses the user's data. It is logged rather than swallowed,
/// which is how the NULL-owner `SetClipboardData` bug stayed invisible.
fn restore_clipboard(saved: Option<String>) {
    if let Some(text) = saved {
        if let Err(e) = clipboard::set_text(&text) {
            tracing::warn!(error = %e, "couldn't restore the clipboard after a selection conversion");
        }
    }
}

/// The last word finished with a space, kept so the manual hotkey can still fix
/// it after the fact.
///
/// It holds the word **as it currently appears on screen**, not the physical
/// keys that produced it. That is what makes the hotkey dependable: the keys
/// stop describing the text the moment anything rewrites it, so a word that had
/// just been auto-corrected — or converted by an earlier press of this same
/// hotkey — used to be unreachable. Rendered text is always accurate, whatever
/// put it there, so the hotkey can convert it and convert it straight back.
struct LastWord {
    /// The word as displayed, without its trailing space.
    text: String,
    /// KLID the word is currently rendered in — what undo restores.
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
    /// Last clipboard sequence number the diary-capture watcher saw, so it only
    /// reads the clipboard when it actually changes — and can skip the change
    /// Palimpsest's own selection conversion causes.
    clip_seq: u32,
}

impl Engine {
    fn new(
        config: Arc<ArcSwap<Config>>,
        user_words: Arc<ArcSwap<UserWords>>,
        events: Sender<EngineEvent>,
    ) -> Self {
        Self {
            config,
            buffer: WordBuffer::new(),
            models: Models::with_user_words(user_words),
            reinterp: WinReinterpreter::new(),
            context: Context::new(),
            last: None,
            last_correction: None,
            events,
            mods: 0,
            clip_seq: clipboard::sequence(),
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
        if selection_hotkey_matches(key.vk, self.mods, &config) {
            self.on_convert_selection(&config);
            return; // the hotkey itself is not part of any word
        }
        if undo_hotkey_matches(key.vk, self.mods, &config) {
            // Same release-first rule as the other two: the chord is still
            // physically held, so the injected backspaces would otherwise run as
            // Ctrl+Backspace and eat a whole word.
            inject::release_modifiers(self.mods);
            self.on_undo();
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

    /// A completed word: record it in the diary if that is switched on, then
    /// auto-correct it if it ended with a space and the heuristic is confident.
    /// Either way the word is remembered for the manual hotkey.
    fn on_word(&mut self, done: Completed, config: &Config) {
        // The context gate runs first, before the word is rendered at all, so a
        // password field or an ignore-listed app produces neither a correction
        // nor a diary entry.
        if let Some(reason) = self.blocked(config) {
            tracing::debug!(%reason, "correction suppressed by context");
            self.last = None;
            return;
        }
        let current = current_layout_klid();
        let typed = self.reinterp.reinterpret(&done.keys, &current);
        self.record(config, &typed);

        if done.boundary != VK_SPACE {
            self.last = None; // can't safely act on tab/enter-finished words
            return;
        }
        let decision = analyze(
            &done.keys,
            &current,
            &config.layout_pairs,
            &self.reinterp as &dyn Reinterpreter,
            &self.models,
            config.sensitivity,
        );
        self.last = match decision {
            Decision::Keep => Some(LastWord { text: typed, layout: current }),
            // A corrected word stays reachable, now described by the correction:
            // pressing the manual hotkey converts it straight back.
            Decision::Correct { .. } => self
                .run(typed, &current, &decision, Some(' '), false)
                .map(|(text, layout)| LastWord { text, layout }),
        };
    }

    /// Append one typed word to the diary, when the user has switched it on.
    ///
    /// Privacy: this is the one place typed text leaves the engine for anything
    /// other than a correction, and it is gated on an explicit opt-in. The word
    /// goes over the event channel to be written to `diary.json` — never to
    /// `tracing`, not even at DEBUG. See [`crate::diary`].
    fn record(&mut self, config: &Config, word: &str) {
        if !config.diary_enabled || word.is_empty() {
            return;
        }
        let app = self.context.foreground_exe().unwrap_or_default();
        let _ = self.events.send(EngineEvent::Typed {
            at: now_secs(),
            app,
            word: word.to_string(),
            clip: false,
        });
    }

    /// Idle-tick work: capture the clipboard into the diary when that opt-in is
    /// on. Runs on the engine's poll tick, not the hook path, so it never touches
    /// input latency.
    ///
    /// It watches the lock-free clipboard sequence counter and reads only when it
    /// moves — the same discipline the selection hotkey uses, for the same reason
    /// (never hammer `OpenClipboard`). It ignores the bump caused by Palimpsest's
    /// own selection-conversion writes, which reset [`Engine::clip_seq`] on the way
    /// out.
    fn on_tick(&mut self) {
        let config = self.config.load();
        if !config.enabled || !config.diary_enabled || !config.diary_capture_clipboard {
            // Keep the baseline current while the feature is off, so turning it on
            // doesn't immediately capture whatever was copied earlier.
            self.clip_seq = clipboard::sequence();
            return;
        }
        let seq = clipboard::sequence();
        if seq == self.clip_seq {
            return;
        }
        self.clip_seq = seq;
        // Same context gate as typed words: nothing from a password field or an
        // ignore-listed app.
        if self.blocked(&config).is_some() {
            return;
        }
        let Some(text) = clipboard::get_text() else { return };
        let text = text.trim();
        if text.is_empty() {
            return;
        }
        let app = self.context.foreground_exe().unwrap_or_default();
        // Privacy: copied text, like typed text, never goes to `tracing`.
        let _ = self.events.send(EngineEvent::Typed {
            at: now_secs(),
            app,
            word: text.to_string(),
            clip: true,
        });
    }

    /// Manual hotkey: force-correct the in-progress word, or the last
    /// space-finished one if nothing is being typed.
    fn on_manual(&mut self, config: &Config) {
        if let Some(reason) = self.blocked(config) {
            tracing::debug!(%reason, "manual correction suppressed by context");
            return;
        }
        // The chord that triggered us is still physically held; drop its
        // modifiers so the injected backspaces don't run as Ctrl+Backspace etc.
        inject::release_modifiers(self.mods);
        let current = current_layout_klid();
        if !self.buffer.is_empty() {
            let keys = self.buffer.take_current();
            let decision =
                suggest(&keys, &current, &config.layout_pairs, &self.reinterp, &self.models);
            let original = self.reinterp.reinterpret(&keys, &current);
            // In-progress word: no boundary to re-emit.
            self.run(original, &current, &decision, None, true);
            self.last = None;
        } else if let Some(last) = self.last.take() {
            self.last = self.convert_last(last, config);
        }
    }

    /// Convert the last finished word to the other layout, returning what now
    /// stands in its place so a second press converts it back.
    ///
    /// This works from the rendered text rather than the buffered keys, which is
    /// what makes it dependable — see [`LastWord`] — and it therefore uses
    /// [`suggest_text`], the same **never-refuses** path as the selection hotkey.
    /// The reasoning is the same too: this is an explicit instruction, so the
    /// guardrails that stop `analyze` acting unbidden do not apply. A conversion
    /// the user did not want costs one press to reverse (the transliteration is
    /// its own inverse) or one undo; a hotkey that silently declines is
    /// indistinguishable from a broken one, which is the complaint this replaces.
    fn convert_last(&mut self, last: LastWord, config: &Config) -> Option<LastWord> {
        let outcome = suggest_text(&last.text, &config.layout_pairs, &self.reinterp, &self.models);
        let Some(correction) = outcome else {
            // Logged at INFO with no typed text, like the selection no-ops: "I
            // pressed it and nothing happened" is otherwise undiagnosable.
            tracing::info!("manual hotkey: no configured pair renders the last word differently");
            return Some(last); // nothing happened, so keep it reachable
        };
        let decision = Decision::Correct {
            backspaces: last.text.chars().count(),
            text: correction.text,
            target_layout: correction.target_layout,
        };
        self.run(last.text, &last.layout, &decision, Some(' '), true)
            .map(|(text, layout)| LastWord { text, layout })
    }

    /// Convert the user's current *selection* to the other layout: copy it,
    /// reinterpret the characters, and type the result back in its place. Unlike
    /// automatic correction this never declines (see [`suggest_text`]).
    ///
    /// The clipboard is borrowed, not seized: everything up to the copy is
    /// read-only, so every early exit leaves it exactly as it was. Only a
    /// conversion that actually happened has anything to restore.
    fn on_convert_selection(&mut self, config: &Config) {
        // The four ways this can end without visibly doing anything all log at
        // INFO, not DEBUG. "I pressed the hotkey and nothing happened" is the
        // hardest report to act on, and none of these lines carries typed text —
        // only a reason, an exe name, or a KLID — so they cost the user no
        // privacy at the default level. The text itself stays at DEBUG.
        if let Some(reason) = self.blocked(config) {
            tracing::info!(%reason, "selection conversion suppressed by context");
            return;
        }
        // The chord that triggered us (e.g. Ctrl+Shift+X) is still physically
        // held; release its modifiers first so the synthetic Ctrl+C actually
        // copies (not Ctrl+Shift+C) and the replacement typing isn't swallowed as
        // a shortcut — otherwise the selection is deleted instead of converted.
        inject::release_modifiers(self.mods);
        // Snapshot what the user had, and note the clipboard's change counter so
        // we can recognise the copy landing without emptying anything first.
        let saved = clipboard::get_text();
        let before = clipboard::sequence();
        if let Err(e) = inject::copy_selection() {
            tracing::warn!(error = %e, "failed to copy selection");
            return; // nothing was written yet — nothing to restore
        }

        let Some(selected) = await_selection(before) else {
            tracing::info!("selection conversion: nothing selected, or the app never copied");
            return; // the clipboard is untouched, so leave it alone
        };

        // `current` is only the layout undo has to restore — the conversion
        // itself deliberately does not depend on it (see `suggest_text`).
        let current = current_layout_klid();
        let outcome = suggest_text(&selected, &config.layout_pairs, &self.reinterp, &self.models);
        let Some(correction) = outcome else {
            tracing::info!("selection left unchanged (no configured pair renders it differently)");
            restore_clipboard(saved);
            return;
        };

        // Type the converted text over the still-active selection, then switch the
        // OS layout for continued typing. `backspaces: 0` because typing replaces
        // the selection — there is nothing to erase first.
        let corrected_len = correction.text.chars().count();
        let decision = Decision::Correct {
            backspaces: 0,
            text: correction.text.clone(),
            target_layout: correction.target_layout.clone(),
        };
        // Privacy: selected text only ever appears at DEBUG, never the default level.
        tracing::info!(target = %correction.target_layout, "converting selection");
        if let Err(e) = execute::apply(&decision, None) {
            tracing::warn!(error = %e, "selection conversion failed");
            restore_clipboard(saved);
            return;
        }
        restore_clipboard(saved);
        // Our own copy + restore bumped the clipboard counter; move the diary
        // watcher's baseline past it so it doesn't record the selection as a copy.
        self.clip_seq = clipboard::sequence();

        // The caret now sits inside converted text, so whatever word `last` named
        // is no longer the thing behind it.
        self.last = None;
        // Make it undoable through the same path as an inline correction: undo
        // erases the `corrected_len` characters we typed and retypes `original`.
        self.last_correction = Some(LastCorrection {
            original: selected.clone(),
            corrected_len,
            from_layout: current,
            boundary: None,
        });
        let _ = self.events.send(EngineEvent::Corrected(CorrectionEvent {
            original: selected,
            corrected: correction.text,
            target_layout: correction.target_layout,
            manual: true,
        }));
    }

    /// Execute a decision (no-op for `Keep`). On a correction it performs the
    /// edit, remembers it for undo, and emits a [`EngineEvent::Corrected`] for
    /// the UI. `original`/`from_layout` describe the text being replaced (for
    /// undo and the event); `manual` flags a hotkey path.
    ///
    /// Returns the text now on screen and the layout it is in, so the caller can
    /// keep [`Engine::last`] pointing at what the user can actually see —
    /// `None` when nothing was applied.
    fn run(
        &mut self,
        original: String,
        from_layout: &str,
        decision: &Decision,
        boundary: Option<char>,
        manual: bool,
    ) -> Option<(String, String)> {
        let Decision::Correct { backspaces: _, text, target_layout } = decision else {
            return None;
        };
        // Privacy: the typed text is only ever logged at DEBUG, never at the
        // default level — Palimpsest must not record what the user types.
        tracing::debug!(%target_layout, manual, original = %original, corrected = %text, "correcting");
        if let Err(e) = execute::apply(decision, boundary) {
            tracing::warn!(error = %e, "correction failed");
            return None;
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
        Some((text.clone(), target_layout.clone()))
    }

    /// Revert the most recent correction, if one is still undoable. Erases the
    /// corrected text, retypes the original, restores the prior layout, and
    /// emits [`EngineEvent::Undone`].
    fn on_undo(&mut self) {
        let Some(lc) = self.last_correction.take() else {
            tracing::info!("undo requested but nothing to undo");
            return;
        };
        // Whatever `last` described is about to be rewritten, and the user has
        // just said they want the text as they typed it — so stop offering to
        // convert it rather than pointing at stale contents.
        self.last = None;
        tracing::debug!(original = %lc.original, "undoing last correction");
        if let Err(e) = execute::undo(&lc.original, lc.corrected_len, &lc.from_layout, lc.boundary) {
            tracing::warn!(error = %e, "undo failed");
            return;
        }
        self.learn_from_undo(&lc);
        let _ = self.events.send(EngineEvent::Undone(CorrectionEvent {
            original: lc.original,
            corrected: String::new(),
            target_layout: lc.from_layout,
            manual: false,
        }));
    }

    /// An undo says the word was right as typed. Remember it under the layout it
    /// was typed in, so the same word is not corrected again. Only the text the
    /// user *kept* is learned, never the correction they rejected — so this can
    /// only ever suppress a future correction, never cause one.
    fn learn_from_undo(&mut self, lc: &LastCorrection) {
        if !self.config.load().learn_on_undo {
            return;
        }
        let Some(lang) = Lang::from_klid(&lc.from_layout) else {
            return; // unsupported layout — nothing to score it against anyway
        };
        if !self.models.learn(lang, &lc.original) {
            return; // already known, or not a single learnable word
        }
        // Privacy: like every other path that touches typed text, the word
        // itself is logged only at DEBUG.
        tracing::debug!(lang = lang.code(), word = %lc.original, "learned word from undo");
        let _ = self.events.send(EngineEvent::Learned {
            lang: lang.code().to_string(),
            word: lc.original.trim().to_lowercase(),
        });
    }
}

/// Run the engine loop until `running` is cleared (then the channels drain) or
/// the key channel disconnects. Intended to own a dedicated thread; blocks the
/// caller.
///
/// - `rx` — observed keystrokes from the hook.
/// - `commands` — UI control messages (e.g. [`Command::Undo`]).
/// - `config` — shared with the UI for live, lock-free updates.
/// - `user_words` — the learned-word dictionary, shared with the UI so words
///   learned here show up there and words forgotten there stop counting here.
/// - `running` — cleared on shutdown to break the loop.
/// - `events` — outbound channel for [`EngineEvent`]s the UI displays.
pub fn run(
    rx: Receiver<RawKey>,
    commands: Receiver<Command>,
    config: Arc<ArcSwap<Config>>,
    user_words: Arc<ArcSwap<UserWords>>,
    running: Arc<AtomicBool>,
    events: Sender<EngineEvent>,
) {
    let mut engine = Engine::new(config, user_words, events);
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
            // Wake periodically so a cleared `running` flag is noticed promptly;
            // also the beat the opt-in clipboard-capture watcher runs on.
            default(POLL_INTERVAL) => engine.on_tick(),
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

    #[test]
    fn selection_hotkey_matches_exact_chord_and_respects_disable() {
        let cfg = Config { selection_hotkey: 0x91, selection_hotkey_mods: 0, ..Config::default() };
        assert!(selection_hotkey_matches(0x91, 0, &cfg)); // bare Scroll Lock
        assert!(!selection_hotkey_matches(0x91, MOD_SHIFT, &cfg)); // a modifier held → no match
        assert!(!selection_hotkey_matches(0x13, 0, &cfg)); // wrong key

        let off = Config { selection_hotkey: 0, ..Config::default() };
        assert!(!selection_hotkey_matches(0, 0, &off));
    }

    #[test]
    fn undo_hotkey_matches_exact_chord_and_respects_disable() {
        let cfg = Config::default(); // Ctrl+Shift+F10
        assert!(undo_hotkey_matches(0x79, MOD_CONTROL | MOD_SHIFT, &cfg));
        assert!(!undo_hotkey_matches(0x79, MOD_SHIFT, &cfg)); // missing Ctrl
        assert!(!undo_hotkey_matches(0x78, MOD_CONTROL | MOD_SHIFT, &cfg)); // that's manual

        let off = Config { undo_hotkey: 0, ..Config::default() };
        assert!(!undo_hotkey_matches(0, 0, &off));
    }

    #[test]
    fn the_three_hotkeys_do_not_collide_by_default() {
        // They sit on adjacent function keys behind the same chord, so a typo in
        // one default would silently shadow another.
        let c = Config::default();
        let all = [c.manual_hotkey, c.selection_hotkey, c.undo_hotkey];
        assert_eq!(all.iter().collect::<std::collections::HashSet<_>>().len(), 3);
    }
}
