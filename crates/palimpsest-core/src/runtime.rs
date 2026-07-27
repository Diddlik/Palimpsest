//! Process-wide runtime: owns the hook thread, the engine thread, and the
//! shared [`Config`], and exposes a small control surface for the UI.
//!
//! Lifecycle:
//! 1. [`Runtime::start`] spawns the hook (its own message pump) and the engine
//!    (draining the hook channel), then arms the hook.
//! 2. [`Runtime::arm`] / [`Runtime::disarm`] toggle event forwarding live.
//! 3. [`Runtime::set_config`] swaps the config atomically; readers see it on
//!    their next `load`.
//! 4. [`Runtime::stop`] (or drop) disarms, breaks the hook pump for a clean
//!    uninstall, stops the engine, and joins both threads.
//!
//! The hook is a process-global OS resource, so a single `Runtime` is expected
//! to exist at a time.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use arc_swap::ArcSwap;
use crossbeam_channel::{bounded, unbounded, Receiver, Sender};

use crate::config::Config;
use crate::events::{Command, EngineEvent};
use crate::scoring::Lang;
use crate::user_dict::UserWords;
use crate::{engine, hook};

/// Capacity of the hook→engine channel. A word is short and the engine drains
/// continuously; this is slack for bursts. If it ever fills, the hook drops
/// events (a missed correction) rather than stalling the OS input pipeline.
const CHANNEL_CAPACITY: usize = 1024;

/// Owns the running Palimpsest core. Dropping it performs a clean shutdown.
pub struct Runtime {
    config: Arc<ArcSwap<Config>>,
    user_words: Arc<ArcSwap<UserWords>>,
    engine_running: Arc<AtomicBool>,
    commands: Sender<Command>,
    events: Receiver<EngineEvent>,
    hook_thread: Option<JoinHandle<()>>,
    engine_thread: Option<JoinHandle<()>>,
}

impl Runtime {
    /// Start the hook and engine threads with `config` and arm the hook.
    pub fn start(config: Config) -> Self {
        let config = Arc::new(ArcSwap::from_pointee(config));
        let user_words = Arc::new(ArcSwap::from_pointee(UserWords::default()));
        let engine_running = Arc::new(AtomicBool::new(true));
        let (tx, rx) = bounded(CHANNEL_CAPACITY);
        let (cmd_tx, cmd_rx) = unbounded();
        let (ev_tx, ev_rx) = unbounded();

        let hook_thread = thread::Builder::new()
            .name("palimpsest-hook".into())
            .spawn(move || {
                if let Err(e) = hook::run(tx) {
                    tracing::error!(error = %e, "hook thread exited with error");
                }
            })
            .expect("spawn hook thread");

        let engine_thread = {
            let config = config.clone();
            let user_words = user_words.clone();
            let running = engine_running.clone();
            thread::Builder::new()
                .name("palimpsest-engine".into())
                .spawn(move || engine::run(rx, cmd_rx, config, user_words, running, ev_tx))
                .expect("spawn engine thread")
        };

        hook::set_armed(true);

        Self {
            config,
            user_words,
            engine_running,
            commands: cmd_tx,
            events: ev_rx,
            hook_thread: Some(hook_thread),
            engine_thread: Some(engine_thread),
        }
    }

    /// A receiver for [`EngineEvent`]s (corrections / undos) the UI can drain to
    /// drive its status toast and history. Cloning shares the same channel.
    pub fn events(&self) -> Receiver<EngineEvent> {
        self.events.clone()
    }

    /// Revert the most recent correction, if any. Asynchronous: the engine
    /// performs the revert on its own thread.
    pub fn undo(&self) {
        let _ = self.commands.send(Command::Undo);
    }

    /// Resume event forwarding (the global on switch).
    pub fn arm(&self) {
        hook::set_armed(true);
    }

    /// Halt event forwarding without tearing anything down (the global off
    /// switch). The hook stays installed but becomes a pass-through no-op.
    pub fn disarm(&self) {
        hook::set_armed(false);
    }

    /// Whether the hook is currently forwarding events.
    pub fn is_armed(&self) -> bool {
        hook::is_armed()
    }

    /// Current config snapshot (cheap `Arc` clone).
    pub fn config(&self) -> Arc<Config> {
        self.config.load_full()
    }

    /// Atomically replace the live config. The engine picks it up on its next
    /// read; no restart required.
    pub fn set_config(&self, config: Config) {
        self.config.store(Arc::new(config));
    }

    /// Current learned-word dictionary (cheap `Arc` clone) — for displaying it
    /// in the settings window and for persisting it after a change.
    pub fn user_words(&self) -> Arc<UserWords> {
        self.user_words.load_full()
    }

    /// Replace the whole learned-word dictionary — used once at startup to
    /// install what was loaded from disk.
    pub fn set_user_words(&self, words: UserWords) {
        self.user_words.store(Arc::new(words));
    }

    /// Forget one learned word, so Palimpsest may correct it again. Returns
    /// `true` if it was present. Uses `rcu` because the engine thread may be
    /// learning a word concurrently.
    pub fn forget_word(&self, lang: Lang, word: &str) -> bool {
        let mut removed = false;
        self.user_words.rcu(|current| {
            let mut next = (**current).clone();
            removed = next.remove(lang, word);
            next
        });
        removed
    }

    /// Forget every learned word, in every language.
    pub fn clear_user_words(&self) {
        self.user_words.store(Arc::new(UserWords::default()));
    }

    /// Stop both threads and uninstall the hook. Idempotent; also run on drop.
    pub fn stop(&mut self) {
        hook::set_armed(false);

        // Break the hook's message pump so it uninstalls and returns.
        hook::request_stop();
        if let Some(h) = self.hook_thread.take() {
            let _ = h.join();
        }

        // Tell the engine to finish draining and exit.
        self.engine_running.store(false, Ordering::Relaxed);
        if let Some(h) = self.engine_thread.take() {
            let _ = h.join();
        }
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        self.stop();
    }
}
