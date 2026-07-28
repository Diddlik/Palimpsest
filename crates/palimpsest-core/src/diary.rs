//! The typing diary — an opt-in log of everything the engine sees typed.
//!
//! This is the Punto Switcher "дневник" feature, and it is exactly what it
//! sounds like: a keystroke log on disk. It is the most privacy-invasive thing
//! Palimpsest can do, so it is fenced in deliberately, and the fences are the
//! feature — do not quietly remove any of them:
//!
//! - **Off by default** ([`crate::Config::diary_enabled`]). Nothing is recorded,
//!   nor is `diary.json` created, until the user switches it on. An upgrade never
//!   turns it on.
//! - **Nothing reaches here from a blocked context.** The engine records only
//!   after `engine::blocked()` has cleared the foreground window, so password
//!   fields and ignore-listed apps produce no entries — the same gate that stops
//!   corrections.
//! - **Bounded.** User-configured count, age, and file-size limits remove the
//!   oldest entries first. [`MAX_ENTRIES`] is the absolute in-memory ceiling.
//! - **Visible and erasable.** Settings → History lists every entry and can wipe
//!   the lot, the same contract as the learned-word dictionary.
//! - **Never logged.** Diary text goes to `diary.json` and the settings window,
//!   never to `tracing` — not even at DEBUG.
//!
//! Entries are grouped rather than stored per word: consecutive words typed in
//! the same app within [`MERGE_GAP_SECS`] join one entry, which is what makes the
//! list readable instead of a column of single words. A clipboard capture is
//! always its own entry (copying is a discrete act) and never merges with typing.

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

/// Absolute in-memory safety ceiling. The configured default is 500.
pub const MAX_ENTRIES: usize = 10_000;

/// Consecutive words in the same app within this many seconds are appended to
/// the same entry instead of starting a new one.
const MERGE_GAP_SECS: u64 = 60;

/// One stretch of typing (or one clipboard capture) in one app.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiaryEntry {
    /// Unix seconds when the stretch started.
    pub at: u64,
    /// Executable name of the app it was typed into, e.g. `"notepad.exe"`.
    pub app: String,
    /// The words, space-separated, as they appeared on screen — or, for a
    /// clipboard capture, the copied text.
    pub text: String,
    /// Unix seconds of the most recent word appended, so a stretch that has gone
    /// quiet does not keep absorbing later typing.
    pub until: u64,
    /// `true` when this entry came from the clipboard rather than the keyboard.
    /// Clipboard captures are deliberate, so they are exempt from the short-entry
    /// guard ([`Diary::prune_short`]).
    #[serde(default)]
    pub clip: bool,
}

/// Words in `text`, used by the short-entry guard.
fn word_count(text: &str) -> usize {
    text.split_whitespace().count()
}

/// The recorded stretches, oldest first.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Diary {
    pub entries: VecDeque<DiaryEntry>,
}

impl Diary {
    /// Record one word typed in `app` at `at` (unix seconds). Appends to the
    /// current stretch when it is the same app, still recent, and also typed
    /// (never a clipboard entry); otherwise starts a new one. Drops the oldest
    /// entry once [`MAX_ENTRIES`] is reached.
    pub fn push(&mut self, at: u64, app: &str, word: &str) {
        self.record(at, app, word, false);
    }

    /// Record a clipboard capture: always its own entry, flagged [`clip`], never
    /// merged into surrounding typing.
    ///
    /// [`clip`]: DiaryEntry::clip
    pub fn push_clip(&mut self, at: u64, app: &str, text: &str) {
        self.record(at, app, text, true);
    }

    fn record(&mut self, at: u64, app: &str, text: &str, clip: bool) {
        if text.is_empty() {
            return;
        }
        match self.entries.back_mut() {
            // Only typed words merge, and only into another typed stretch.
            Some(last)
                if !clip
                    && !last.clip
                    && last.app == app
                    && at.saturating_sub(last.until) <= MERGE_GAP_SECS =>
            {
                last.text.push(' ');
                last.text.push_str(text);
                last.until = at;
            }
            _ => {
                if self.entries.len() >= MAX_ENTRIES {
                    self.entries.pop_front();
                }
                self.entries.push_back(DiaryEntry {
                    at,
                    app: app.to_string(),
                    text: text.to_string(),
                    until: at,
                    clip,
                });
            }
        }
    }

    /// Drop typed entries shorter than two words — Punto Switcher's guard against
    /// logging a lone password or username. Clipboard captures are always kept
    /// (copying is deliberate). When `include_last` is false the most recent entry
    /// is spared, because it may still be growing into a longer stretch; pass
    /// `true` on a final flush, when nothing more will be appended.
    pub fn prune_short(&mut self, include_last: bool) {
        let last = self.entries.len().saturating_sub(1);
        let mut i = 0;
        self.entries.retain(|e| {
            let spare_growing = !include_last && i == last;
            let keep = e.clip || word_count(&e.text) >= 2 || spare_growing;
            i += 1;
            keep
        });
    }

    /// Remove expired entries, then keep only the newest `max_entries`.
    pub fn prune_limits(&mut self, now: u64, max_entries: usize, max_age_secs: u64) {
        if max_age_secs > 0 {
            let cutoff = now.saturating_sub(max_age_secs);
            self.entries.retain(|e| e.until >= cutoff);
        }
        while self.entries.len() > max_entries {
            self.entries.pop_front();
        }
    }

    /// Erase everything — the escape hatch that makes the whole feature
    /// reversible.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consecutive_words_in_one_app_join_a_single_entry() {
        let mut d = Diary::default();
        d.push(100, "notepad.exe", "hello");
        d.push(105, "notepad.exe", "there");
        assert_eq!(d.entries.len(), 1);
        assert_eq!(d.entries[0].text, "hello there");
        assert_eq!(d.entries[0].at, 100); // the stretch keeps its start time
        assert_eq!(d.entries[0].until, 105);
    }

    #[test]
    fn a_different_app_or_a_long_pause_starts_a_new_entry() {
        let mut d = Diary::default();
        d.push(100, "notepad.exe", "hello");
        d.push(101, "chrome.exe", "search");
        assert_eq!(d.entries.len(), 2);

        // Back in the first app, but an hour later — a separate stretch.
        d.push(100 + 3600, "chrome.exe", "later");
        assert_eq!(d.entries.len(), 3);
    }

    #[test]
    fn the_diary_is_bounded() {
        let mut d = Diary::default();
        // Each push is its own entry: same app, but always past the merge gap.
        for i in 0..(MAX_ENTRIES + 10) {
            d.push(i as u64 * 1000, "notepad.exe", "word");
        }
        assert_eq!(d.entries.len(), MAX_ENTRIES);
        // The oldest went, the newest stayed.
        assert_eq!(d.entries.front().unwrap().at, 10 * 1000);
    }

    #[test]
    fn empty_words_are_not_recorded() {
        let mut d = Diary::default();
        d.push(1, "notepad.exe", "");
        assert!(d.is_empty());
    }

    #[test]
    fn a_clipboard_capture_is_its_own_entry_and_does_not_merge() {
        let mut d = Diary::default();
        d.push(100, "notepad.exe", "typing");
        d.push_clip(105, "notepad.exe", "copied text");
        d.push(110, "notepad.exe", "more");
        // Three entries: the clipboard capture breaks the typed stretch in two.
        assert_eq!(d.entries.len(), 3);
        assert!(d.entries[1].clip);
        assert!(!d.entries[0].clip && !d.entries[2].clip);
    }

    #[test]
    fn prune_short_drops_lone_typed_words_but_keeps_clips_and_the_open_entry() {
        let mut d = Diary::default();
        d.push(100, "a.exe", "hi"); // 1 word, closed → dropped
        d.push_clip(200, "b.exe", "x"); // clip, 1 word → kept
        d.push(300, "c.exe", "two words"); // 2 words → kept
        d.push(400, "d.exe", "tail"); // 1 word, last/open → spared when include_last=false

        let mut open = d.clone();
        open.prune_short(false);
        assert_eq!(open.entries.iter().map(|e| e.text.as_str()).collect::<Vec<_>>(),
            vec!["x", "two words", "tail"]);

        // On a final flush the trailing lone word goes too.
        d.prune_short(true);
        assert_eq!(d.entries.iter().map(|e| e.text.as_str()).collect::<Vec<_>>(),
            vec!["x", "two words"]);
    }

    #[test]
    fn clear_erases_everything() {
        let mut d = Diary::default();
        d.push(1, "notepad.exe", "hello");
        d.clear();
        assert!(d.is_empty());
    }

    #[test]
    fn configured_limits_remove_oldest_and_expired_entries() {
        let mut d = Diary::default();
        d.push(100, "a.exe", "one");
        d.push(200, "b.exe", "two");
        d.push(300, "c.exe", "three");
        d.prune_limits(350, 2, 200);
        assert_eq!(d.entries.iter().map(|e| e.text.as_str()).collect::<Vec<_>>(),
            vec!["two", "three"]);

        d.prune_limits(1_000, 10, 100);
        assert!(d.is_empty());
    }

    #[test]
    fn json_round_trips_and_tolerates_an_empty_file() {
        let mut d = Diary::default();
        d.push(1, "notepad.exe", "hello");
        let json = serde_json::to_string(&d).unwrap();
        assert_eq!(serde_json::from_str::<Diary>(&json).unwrap(), d);
        assert_eq!(serde_json::from_str::<Diary>("{}").unwrap(), Diary::default());
    }
}
