//! Scoring — per-language character bigram models (the MVP heuristic).
//!
//! Each language has a model trained on a small embedded corpus. To judge a
//! word we compute its mean per-bigram log-probability under a language's model;
//! a real word in that language scores high, gibberish scores low. The decision
//! layer ([`crate::analyze`]) reinterprets a word under each candidate layout
//! and compares these scores.
//!
//! Cross-alphabet text is handled by an out-of-vocabulary floor: a character the
//! model never saw (e.g. Cyrillic under the English model) contributes a fixed,
//! very low log-probability. That makes "this Latin gibberish is actually
//! Russian" fall out of the same comparison naturally.
//!
//! `fst` wordlists (plan step 2) can later refine the score; this module's
//! public surface (`Models::score`) is the seam they'd plug into.

use std::collections::HashMap;

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
}

/// Log-probability charged for a bigram touching an out-of-vocabulary character.
/// `ln(1e-7)` ≈ -16.1 — low enough that any cross-alphabet reinterpretation is
/// decisively worse than in-alphabet text, but finite so scores stay comparable.
const OOV_LOG_PROB: f64 = -16.118_095_650_958_2;

/// Laplace (add-k) smoothing constant for in-vocabulary bigrams.
const ALPHA: f64 = 0.5;

/// A character bigram language model with add-k smoothing.
#[derive(Debug)]
pub struct NgramModel {
    unigram: HashMap<char, u64>,
    bigram: HashMap<(char, char), u64>,
    vocab_size: usize,
}

impl NgramModel {
    /// Train on `corpus`. Text is lowercased; words are streamed with the spaces
    /// already present in the corpus so word-boundary bigrams are learned.
    pub fn train(corpus: &str) -> Self {
        let mut unigram: HashMap<char, u64> = HashMap::new();
        let mut bigram: HashMap<(char, char), u64> = HashMap::new();
        let normalized: Vec<char> = corpus.to_lowercase().chars().collect();

        for &c in &normalized {
            *unigram.entry(c).or_insert(0) += 1;
        }
        for pair in normalized.windows(2) {
            *bigram.entry((pair[0], pair[1])).or_insert(0) += 1;
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

/// The set of trained language models, built once and shared (cheap to keep in
/// an `Arc`). Construct with [`Models::builtin`].
#[derive(Debug)]
pub struct Models {
    en: NgramModel,
    ru: NgramModel,
    de: NgramModel,
}

impl Models {
    /// Train every language model from its embedded corpus.
    pub fn builtin() -> Self {
        Self {
            en: NgramModel::train(corpus::EN),
            ru: NgramModel::train(corpus::RU),
            de: NgramModel::train(corpus::DE),
        }
    }

    /// Score `text` under `lang`'s model (mean per-bigram log-probability).
    pub fn score(&self, lang: Lang, text: &str) -> f64 {
        self.model(lang).score(text)
    }

    fn model(&self, lang: Lang) -> &NgramModel {
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

/// Embedded training corpora. Deliberately small but rich in common bigrams of
/// each language; never derived from anything the user types. Phase 6 can swap
/// these for larger embedded tables or `fst` wordlists without touching callers.
mod corpus {
    pub const EN: &str = "\
the quick brown fox jumps over the lazy dog. hello there, how are you doing today? \
i would like to thank everyone for their time and attention this morning. \
we should consider whether the weather will be good enough for the trip. \
please remember to bring your laptop, charger, and notebook to the meeting. \
this is a simple example of common english words and ordinary sentences. \
people often write messages to friends about work, family, food, and travel. \
the children played outside while their parents prepared a wonderful dinner. \
knowledge, language, and communication shape the way that humans understand things.";

    pub const RU: &str = "\
привет, как дела сегодня? спасибо большое за помощь и за твоё время. \
я хочу поблагодарить всех за внимание и за участие в нашей встрече. \
сегодня хорошая погода, поэтому мы можем пойти гулять в парк вместе. \
пожалуйста, не забудь взять с собой ноутбук, зарядку и тетрадь на работу. \
это простой пример обычных русских слов и самых распространённых предложений. \
люди часто пишут сообщения друзьям о работе, семье, еде и путешествиях. \
дети играли на улице, пока их родители готовили вкусный и сытный ужин. \
знание, язык и общение определяют то, как человек понимает окружающий мир.";

    pub const DE: &str = "\
hallo, wie geht es dir heute? vielen dank für deine zeit und deine hilfe. \
ich möchte mich bei allen für ihre aufmerksamkeit und teilnahme bedanken. \
heute ist schönes wetter, deshalb können wir zusammen im park spazieren gehen. \
bitte vergiss nicht, deinen laptop, das ladegerät und ein heft mitzubringen. \
dies ist ein einfaches beispiel für gewöhnliche deutsche wörter und sätze. \
menschen schreiben oft nachrichten an freunde über arbeit, familie und reisen. \
die kinder spielten draußen, während ihre eltern ein leckeres abendessen kochten. \
wissen, sprache und kommunikation prägen das verständnis der menschen.";
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
    fn languages_in_collects_both_directions() {
        let mut langs = languages_in(&Config::default());
        langs.sort_by_key(|l| format!("{l:?}"));
        assert_eq!(langs, vec![Lang::En, Lang::Ru]);
    }
}
