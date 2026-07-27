//! The user's learned words — the one thing Palimpsest deliberately remembers
//! from what you type.
//!
//! Everywhere else the word buffer is short-lived and in-memory only. This is
//! the documented exception: when the user *undoes* a correction they are
//! telling us the word was right as typed, and the only way to stop correcting
//! it tomorrow is to write it down. See "Privacy" in `AGENT_INSTRUCTIONS.md`.
//!
//! Two properties keep that exception safe:
//!
//! - **Only the kept text is ever learned**, never the text the user rejected.
//!   A learned word can only raise the score of the interpretation the user
//!   already chose, so it can only ever *suppress* a future correction — it can
//!   never cause one. Over-learning costs a missed correction, which the project
//!   accepts; it cannot produce a false correction, which it does not.
//! - **Nothing reaches here from a blocked context.** `engine::blocked()` stops
//!   corrections in password fields and ignore-listed apps, and no correction
//!   means no undo, which means nothing to learn.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::scoring::Lang;

/// Shortest word worth remembering. Matches `analyze`'s minimum word length —
/// below it the engine never corrects, so there would be nothing to suppress.
const MIN_LEARNABLE_LEN: usize = 2;

/// Words the user has taught Palimpsest, grouped by language. `BTreeSet` so the
/// persisted JSON has a stable, diffable order.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UserWords {
    pub en: BTreeSet<String>,
    pub ru: BTreeSet<String>,
    pub de: BTreeSet<String>,
}

/// Normalize a candidate word, or reject it. Words are stored lowercase and
/// trimmed. A candidate with inner whitespace is refused: the selection-
/// conversion path can undo a whole phrase, and a phrase is not a dictionary
/// entry.
fn normalize(word: &str) -> Option<String> {
    let w = word.trim().to_lowercase();
    let ok = w.chars().count() >= MIN_LEARNABLE_LEN && !w.chars().any(char::is_whitespace);
    ok.then_some(w)
}

impl UserWords {
    fn set(&self, lang: Lang) -> &BTreeSet<String> {
        match lang {
            Lang::En => &self.en,
            Lang::Ru => &self.ru,
            Lang::De => &self.de,
        }
    }

    fn set_mut(&mut self, lang: Lang) -> &mut BTreeSet<String> {
        match lang {
            Lang::En => &mut self.en,
            Lang::Ru => &mut self.ru,
            Lang::De => &mut self.de,
        }
    }

    /// Whether `word` has been learned for `lang`. Case-insensitive.
    pub fn contains(&self, lang: Lang, word: &str) -> bool {
        match normalize(word) {
            Some(w) => self.set(lang).contains(&w),
            None => false,
        }
    }

    /// Learn `word` for `lang`. Returns `true` if it was new and acceptable, so
    /// the caller knows whether anything actually needs persisting.
    pub fn insert(&mut self, lang: Lang, word: &str) -> bool {
        match normalize(word) {
            Some(w) => self.set_mut(lang).insert(w),
            None => false,
        }
    }

    /// Forget one word. Returns `true` if it was present.
    pub fn remove(&mut self, lang: Lang, word: &str) -> bool {
        match normalize(word) {
            Some(w) => self.set_mut(lang).remove(&w),
            None => false,
        }
    }

    /// Forget everything, in every language.
    pub fn clear(&mut self) {
        self.en.clear();
        self.ru.clear();
        self.de.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn learning_is_case_insensitive_and_trimmed() {
        let mut w = UserWords::default();
        assert!(w.insert(Lang::En, "  Hers  "));
        assert!(w.contains(Lang::En, "hers"));
        assert!(w.contains(Lang::En, "HERS"));
        // Stored normalized, not as typed.
        assert!(w.en.contains("hers"));
    }

    #[test]
    fn languages_are_independent() {
        let mut w = UserWords::default();
        w.insert(Lang::En, "hers");
        assert!(!w.contains(Lang::Ru, "hers"));
        assert!(!w.contains(Lang::De, "hers"));
    }

    #[test]
    fn phrases_and_too_short_words_are_refused() {
        let mut w = UserWords::default();
        // The selection-conversion path can undo a whole phrase; that is not a
        // dictionary entry.
        assert!(!w.insert(Lang::En, "hello there"));
        assert!(!w.insert(Lang::En, "a"));
        assert_eq!(w, UserWords::default());
    }

    #[test]
    fn inserting_twice_reports_no_change() {
        let mut w = UserWords::default();
        assert!(w.insert(Lang::Ru, "привет"));
        assert!(!w.insert(Lang::Ru, "привет")); // already known — nothing to persist
    }

    #[test]
    fn remove_and_clear() {
        let mut w = UserWords::default();
        w.insert(Lang::En, "hers");
        w.insert(Lang::Ru, "привет");
        assert!(w.remove(Lang::En, "Hers")); // normalized on the way in
        assert!(!w.remove(Lang::En, "hers")); // already gone
        w.clear();
        assert_eq!(w, UserWords::default());
    }

    #[test]
    fn json_round_trips_and_tolerates_missing_languages() {
        let mut w = UserWords::default();
        w.insert(Lang::De, "straße");
        let json = serde_json::to_string(&w).unwrap();
        assert_eq!(serde_json::from_str::<UserWords>(&json).unwrap(), w);
        // An older/partial file still loads.
        let partial: UserWords = serde_json::from_str(r#"{"en":["hers"]}"#).unwrap();
        assert!(partial.contains(Lang::En, "hers"));
        assert!(partial.ru.is_empty());
    }
}
