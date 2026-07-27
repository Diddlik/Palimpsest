//! Scoring — per-language frequency wordlists over character bigram models.
//!
//! To judge a word we compute its mean per-bigram log-probability under a
//! language's model; a real word in that language scores high, gibberish scores
//! low. On top of that, a word found in the language's frequency wordlist earns
//! a flat bonus — a dictionary hit is far stronger evidence than any n-gram,
//! because wrong-layout text essentially never spells a real word. The decision
//! layer ([`crate::analyze`]) reinterprets a word under each candidate layout
//! and compares these scores.
//!
//! Cross-alphabet text is handled by an out-of-vocabulary floor: a character the
//! model never saw (e.g. Cyrillic under the English model) contributes a fixed,
//! very low log-probability. That makes "this Latin gibberish is actually
//! Russian" fall out of the same comparison naturally.
//!
//! The wordlists live as plain text under `wordlists/` and are compiled to
//! finite state transducers by `build.rs`; see there for the pipeline.

use std::collections::HashMap;
use std::sync::Arc;

use arc_swap::ArcSwap;
use fst::Set;

use crate::user_dict::UserWords;
use crate::Config;

/// Languages Palimpsest can score. Alphabetic pairs only for v1 (no CJK IMEs).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Lang {
    En,
    Ru,
    De,
}

impl Lang {
    /// Map a Windows KLID (or any hex layout id) to a language by its primary
    /// language id (the low 16 bits). Returns `None` for unsupported layouts.
    pub fn from_klid(klid: &str) -> Option<Lang> {
        let raw = u32::from_str_radix(klid.trim(), 16).ok()?;
        match raw & 0xFFFF {
            0x0409 => Some(Lang::En),
            0x0419 => Some(Lang::Ru),
            0x0407 => Some(Lang::De),
            _ => None,
        }
    }

    /// Short stable identifier, used as the key in the persisted user dictionary
    /// and in the events the UI displays.
    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Ru => "ru",
            Lang::De => "de",
        }
    }
}

/// Log-probability charged for a bigram touching an out-of-vocabulary character.
/// `ln(1e-7)` ≈ -16.1 — low enough that any cross-alphabet reinterpretation is
/// decisively worse than in-alphabet text, but finite so scores stay comparable.
const OOV_LOG_PROB: f64 = -16.118_095_650_958_2;

/// Laplace (add-k) smoothing constant for in-vocabulary bigrams.
const ALPHA: f64 = 0.5;

/// Score added when a word is present in its language's frequency wordlist.
/// Comfortably above the largest correction margin in [`crate::analyze`], so a
/// dictionary hit decides the comparison — but finite, so that when *both*
/// interpretations are real words the bonus cancels and the bigram models still
/// choose between them.
const DICT_BONUS: f64 = 3.0;

/// Shortest word eligible for [`DICT_BONUS`]. Two-letter words are where an
/// accidental cross-layout collision with a real word is most likely, and the
/// class is small enough that skipping it costs almost no recall.
const MIN_DICT_LEN: usize = 3;

/// A character bigram language model with add-k smoothing.
#[derive(Debug)]
pub struct NgramModel {
    unigram: HashMap<char, u64>,
    bigram: HashMap<(char, char), u64>,
    vocab_size: usize,
}

impl NgramModel {
    /// Train on `words`, counted as one space-separated stream so that the
    /// word-boundary bigrams match the padding [`NgramModel::score`] applies.
    pub fn train(words: impl Iterator<Item = String>) -> Self {
        let mut unigram: HashMap<char, u64> = HashMap::new();
        let mut bigram: HashMap<(char, char), u64> = HashMap::new();

        let mut prev = ' ';
        *unigram.entry(' ').or_insert(0) += 1;
        for word in words {
            for c in word.to_lowercase().chars().chain([' ']) {
                *unigram.entry(c).or_insert(0) += 1;
                *bigram.entry((prev, c)).or_insert(0) += 1;
                prev = c;
            }
        }

        let vocab_size = unigram.len();
        Self { unigram, bigram, vocab_size }
    }

    /// Mean per-bigram log-probability of `text` (already a single word). The
    /// word is padded with spaces so leading/trailing letter likelihood counts.
    /// Returns [`OOV_LOG_PROB`] for an empty/degenerate word.
    pub fn score(&self, text: &str) -> f64 {
        let chars: Vec<char> = format!(" {} ", text.to_lowercase()).chars().collect();
        if chars.len() < 2 {
            return OOV_LOG_PROB;
        }
        let mut sum = 0.0;
        for pair in chars.windows(2) {
            sum += self.bigram_log_prob(pair[0], pair[1]);
        }
        sum / (chars.len() - 1) as f64
    }

    fn bigram_log_prob(&self, c1: char, c2: char) -> f64 {
        // Either character unseen in training → cross-alphabet / exotic: floor it.
        if !self.unigram.contains_key(&c1) || !self.unigram.contains_key(&c2) {
            return OOV_LOG_PROB;
        }
        let joint = *self.bigram.get(&(c1, c2)).unwrap_or(&0) as f64;
        let context = *self.unigram.get(&c1).unwrap_or(&0) as f64;
        let p = (joint + ALPHA) / (context + ALPHA * self.vocab_size as f64);
        p.ln()
    }
}

/// One language's evidence: the bigram model plus its frequency wordlist.
struct LangModel {
    ngram: NgramModel,
    dict: Set<&'static [u8]>,
}

impl LangModel {
    /// Build both halves from the one embedded wordlist: the FST is the
    /// dictionary, and streaming its keys trains the bigram fallback.
    fn new(fst: &'static [u8]) -> Self {
        let dict = Set::new(fst).expect("valid embedded fst");
        let mut words = Vec::with_capacity(dict.len());
        let mut stream = dict.stream();
        while let Some(key) = fst::Streamer::next(&mut stream) {
            words.push(String::from_utf8(key.to_vec()).expect("wordlist is utf-8"));
        }
        Self { ngram: NgramModel::train(words.into_iter()), dict }
    }

    /// Whether the built-in wordlist vouches for `word` (already normalized).
    fn knows(&self, word: &str) -> bool {
        word.chars().count() >= MIN_DICT_LEN && self.dict.contains(word)
    }
}

// Hand-written so `{:?}` reports the wordlist size instead of dumping every
// word in it.
impl std::fmt::Debug for LangModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LangModel").field("words", &self.dict.len()).finish()
    }
}

/// The set of trained language models, built once and shared (cheap to keep in
/// an `Arc`). Construct with [`Models::builtin`].
#[derive(Debug)]
pub struct Models {
    en: LangModel,
    ru: LangModel,
    de: LangModel,
    /// Words the user taught us by undoing a correction. Shared with the UI,
    /// which can list and forget them, so it is read through an `ArcSwap`
    /// rather than owned outright.
    user: Arc<ArcSwap<UserWords>>,
}

impl Models {
    /// Build every language model from its embedded wordlist — the same list
    /// backing both the dictionary lookup and the bigram fallback — with an
    /// empty user dictionary.
    pub fn builtin() -> Self {
        Self::with_user_words(Arc::new(ArcSwap::from_pointee(UserWords::default())))
    }

    /// As [`Models::builtin`], but sharing an existing user dictionary so the
    /// engine sees the UI's edits (and vice versa) without a restart.
    pub fn with_user_words(user: Arc<ArcSwap<UserWords>>) -> Self {
        Self {
            en: LangModel::new(include_bytes!(concat!(env!("OUT_DIR"), "/en.fst"))),
            ru: LangModel::new(include_bytes!(concat!(env!("OUT_DIR"), "/ru.fst"))),
            de: LangModel::new(include_bytes!(concat!(env!("OUT_DIR"), "/de.fst"))),
            user,
        }
    }

    /// Score `text` under `lang` — mean per-bigram log-probability, plus
    /// [`DICT_BONUS`] when either the built-in wordlist or the user's own
    /// dictionary vouches for the word.
    pub fn score(&self, lang: Lang, text: &str) -> f64 {
        let word = text.to_lowercase();
        let model = self.model(lang);
        let ngram = model.ngram.score(&word);
        // The user's own words carry no length floor: unlike a frequency list
        // they cannot collide by accident, because the user put them there.
        if model.knows(&word) || self.user.load().contains(lang, &word) {
            ngram + DICT_BONUS
        } else {
            ngram
        }
    }

    /// Whether `text` is a real word of `lang` — in the built-in wordlist or the
    /// user's own. This is the primary signal on the phonetic path, where a
    /// romanization is always phonotactically plausible and only "is it a word"
    /// separates the readings.
    pub fn is_word(&self, lang: Lang, text: &str) -> bool {
        let word = text.to_lowercase();
        self.model(lang).knows(&word) || self.user.load().contains(lang, &word)
    }

    /// Whether the *user* has vouched for `word` in `lang` (as opposed to the
    /// built-in wordlist). [`crate::analyze::analyze`] treats this as a veto on
    /// automatic correction rather than merely as evidence: a flat score bonus
    /// cannot outweigh a wide margin like `ghbdtn` → `привет`, and "I already
    /// told you this word is fine" has to win regardless of the statistics.
    pub fn is_user_word(&self, lang: Lang, text: &str) -> bool {
        self.user.load().contains(lang, &text.to_lowercase())
    }

    /// Learn `word` as a real word of `lang`, so future corrections of it are
    /// suppressed. Returns `true` if this changed anything (the caller persists
    /// on `true`). Safe against concurrent edits from the UI thread.
    pub fn learn(&self, lang: Lang, word: &str) -> bool {
        let mut changed = false;
        self.user.rcu(|current| {
            let mut next = (**current).clone();
            changed = next.insert(lang, word);
            next
        });
        changed
    }

    fn model(&self, lang: Lang) -> &LangModel {
        match lang {
            Lang::En => &self.en,
            Lang::Ru => &self.ru,
            Lang::De => &self.de,
        }
    }
}

impl Default for Models {
    fn default() -> Self {
        Self::builtin()
    }
}

/// Languages enabled by a [`Config`]'s layout pairs (both directions), for
/// callers that want to limit scoring to configured layouts.
pub fn languages_in(config: &Config) -> Vec<Lang> {
    let mut langs = Vec::new();
    for pair in &config.layout_pairs {
        for klid in [&pair.from, &pair.to] {
            if let Some(l) = Lang::from_klid(klid) {
                if !langs.contains(&l) {
                    langs.push(l);
                }
            }
        }
    }
    langs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn klid_maps_to_language() {
        assert_eq!(Lang::from_klid("00000409"), Some(Lang::En));
        assert_eq!(Lang::from_klid("00000419"), Some(Lang::Ru));
        assert_eq!(Lang::from_klid("00000407"), Some(Lang::De));
        // HKL-style value with a device id in the high word still resolves.
        assert_eq!(Lang::from_klid("04190419"), Some(Lang::Ru));
        assert_eq!(Lang::from_klid("0000040c"), None); // French — unsupported
    }

    #[test]
    fn real_words_beat_gibberish_in_their_language() {
        let m = Models::builtin();
        assert!(m.score(Lang::En, "hello") > m.score(Lang::En, "ghbdtn"));
        assert!(m.score(Lang::Ru, "привет") > m.score(Lang::Ru, "руддщ"));
    }

    #[test]
    fn cross_alphabet_is_floored() {
        let m = Models::builtin();
        // Cyrillic under the English model is all out-of-vocabulary.
        assert!(m.score(Lang::En, "привет") <= OOV_LOG_PROB + 1e-9);
    }

    #[test]
    fn wordlist_hit_beats_a_merely_plausible_nonword() {
        let m = Models::builtin();
        // "hello" is in the wordlist; "hellu" is bigram-plausible but is not.
        assert!(m.score(Lang::En, "hello") - m.score(Lang::En, "hellu") > 1.0);
        assert!(m.score(Lang::De, "wetter") - m.score(Lang::De, "wetteru") > 1.0);
        assert!(m.score(Lang::Ru, "спасибо") - m.score(Lang::Ru, "спасибу") > 1.0);
    }

    #[test]
    fn wordlists_cover_words_the_tiny_corpora_never_saw() {
        let m = Models::builtin();
        for (lang, word) in
            [(Lang::En, "keyboard"), (Lang::De, "tastatur"), (Lang::Ru, "клавиатура")]
        {
            assert!(m.score(lang, word) > -6.0, "{word} should score as a real word");
        }
    }

    #[test]
    fn wordlist_bonus_skips_two_letter_words() {
        let m = Models::builtin();
        // Below MIN_DICT_LEN, so scoring falls through to the bigram model
        // regardless of whether the wordlist happens to contain the word.
        assert!(m.score(Lang::En, "on") < 0.0);
    }

    #[test]
    fn languages_in_collects_both_directions() {
        let mut langs = languages_in(&Config::default());
        langs.sort_by_key(|l| format!("{l:?}"));
        assert_eq!(langs, vec![Lang::En, Lang::Ru]);
    }
}
