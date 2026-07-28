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
//! - **Bounded.** At most [`MAX_ENTRIES`] entries are kept; the oldest are
//!   dropped. The diary is a recent-history tool, not an archive.
//! - **Visible and erasable.** Settings → History lists every entry and can wipe
//!   the lot, the same contract as the learned-word dictionary.
//! - **Never logged.** Diary text goes to `diary.json` and the settings window,
//!   never to `tracing` — not even at DEBUG.
//!
//! Entries are grouped rather than stored per word: consecutive words typed in
//! the same app within [`MERGE_GAP_SECS`] join one entry, which is what makes the
//! list readable instead of a column of single words.

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

/// Most entries retained. Older ones fall off the front.
pub const MAX_ENTRIES: usize = 500;

/// Consecutive words in the same app within this many seconds are appended to
/// the same entry instead of starting a new one.
const MERGE_GAP_SECS: u64 = 60;

/// One stretch of typing in one app.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiaryEntry {
    /// Unix seconds when the stretch started.
    pub at: u64,
    /// Executable name of the app it was typed into, e.g. `"notepad.exe"`.
    pub app: String,
    /// The words, space-separated, as they appeared on screen.
    pub text: String,
    /// Unix seconds of the most recent word appended, so a stretch that has gone
    /// quiet does not keep absorbing later typing.
    pub until: u64,
}

/// The recorded stretches, oldest first.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Diary {
    pub entries: VecDeque<DiaryEntry>,
}

impl Diary {
    /// Record one word typed in `app` at `at` (unix seconds). Appends to the
    /// current stretch when it is the same app and still recent, otherwise starts
    /// a new one; drops the oldest entry once [`MAX_ENTRIES`] is reached.
    pub fn push(&mut self, at: u64, app: &str, word: &str) {
        if word.is_empty() {
            return;
        }
        match self.entries.back_mut() {
            Some(last) if last.app == app && at.saturating_sub(last.until) <= MERGE_GAP_SECS => {
                last.text.push(' ');
                last.text.push_str(word);
                last.until = at;
            }
            _ => {
                if self.entries.len() >= MAX_ENTRIES {
                    self.entries.pop_front();
                }
                self.entries.push_back(DiaryEntry {
                    at,
                    app: app.to_string(),
                    text: word.to_string(),
                    until: at,
                });
            }
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
    fn clear_erases_everything() {
        let mut d = Diary::default();
        d.push(1, "notepad.exe", "hello");
        d.clear();
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
