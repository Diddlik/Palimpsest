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

/// Embedded training corpora. Rich enough to give reliable bigram statistics for
/// common words in each language; never derived from anything the user types.
/// Phase 6 can swap these for larger embedded tables or `fst` wordlists without
/// touching callers.
///
/// Size target: ≥ 3 000 characters per language so the smoothed bigram
/// probabilities are stable enough to separate in-language words from near-miss
/// phonetic transliterations (e.g. English "dela" vs Russian "дела").
mod corpus {
    pub const EN: &str = "\
the quick brown fox jumps over the lazy dog. hello there, how are you doing today? \
i would like to thank everyone for their time and attention this morning. \
we should consider whether the weather will be good enough for the trip. \
please remember to bring your laptop, charger, and notebook to the meeting. \
this is a simple example of common english words and ordinary sentences. \
people often write messages to friends about work, family, food, and travel. \
the children played outside while their parents prepared a wonderful dinner. \
knowledge, language, and communication shape the way that humans understand things. \
she said that the door was open and the light inside was still on. \
they walked together down the street until they reached the corner store. \
he looked at the screen and tried to figure out what had gone wrong again. \
the team decided to meet on monday morning before the project deadline arrived. \
after a long day of work, he finally sat down and had a cup of coffee. \
the report was finished on time and sent to the client without any issues. \
students need to practice every day if they want to improve their skills. \
the weather was cold and windy, so they decided to stay home and rest. \
she opened the book and started reading from the first chapter carefully. \
all the documents were signed and the deal was officially closed today. \
he drove to the office early in the morning to avoid the traffic jam. \
the flowers in the garden were beautiful and smelled wonderful this spring. \
every morning she made herself a cup of tea and sat by the window reading. \
the new software update fixed several bugs and improved overall performance. \
they agreed to split the cost and share the responsibility equally between them. \
the sunset over the mountains was one of the most beautiful things he had seen. \
it is important to listen carefully before you respond or make a decision. \
the museum was closed on monday so they decided to visit the park instead. \
when the rain finally stopped, they went outside and enjoyed the fresh air. \
she sent him a long message explaining everything that had happened last week. \
the company announced a new product that would change the way people communicate. \
he always kept his desk clean and organised so he could focus on his work.";

    pub const RU: &str = "\
привет, как дела сегодня? спасибо большое за помощь и за твоё время. \
я хочу поблагодарить всех за внимание и за участие в нашей встрече. \
сегодня хорошая погода, поэтому мы можем пойти гулять в парк вместе. \
пожалуйста, не забудь взять с собой ноутбук, зарядку и тетрадь на работу. \
это простой пример обычных русских слов и самых распространённых предложений. \
люди часто пишут сообщения друзьям о работе, семье, еде и путешествиях. \
дети играли на улице, пока их родители готовили вкусный и сытный ужин. \
знание, язык и общение определяют то, как человек понимает окружающий мир. \
она открыла дверь и увидела, что в комнате горит свет и кто-то есть дома. \
они шли вместе по улице, разговаривая о работе и о планах на выходные. \
он посмотрел на экран и попытался понять, что именно пошло не так сегодня. \
команда решила собраться в понедельник утром до начала важного совещания. \
после долгого рабочего дня он наконец сел и выпил чашку горячего чая. \
отчёт был готов вовремя и отправлен клиенту без каких-либо задержек. \
студентам нужно заниматься каждый день, если они хотят улучшить свои результаты. \
погода была холодной и ветреной, поэтому они решили остаться дома и отдохнуть. \
она открыла книгу и начала читать с самого начала очень внимательно. \
все документы были подписаны и сделка официально закрыта сегодня вечером. \
он приехал в офис рано утром, чтобы успеть до начала рабочего дня. \
цветы в саду были красивыми и приятно пахли в это солнечное весеннее утро. \
каждое утро она готовила себе чашку кофе и садилась у окна с книгой. \
новое обновление программы исправило несколько ошибок и улучшило производительность. \
они договорились разделить расходы и распределить обязанности поровну между собой. \
закат над горами был одним из самых красивых зрелищ, которые он когда-либо видел. \
важно внимательно слушать собеседника, прежде чем отвечать или принимать решение. \
музей был закрыт в понедельник, поэтому они решили вместо этого посетить парк. \
когда дождь наконец прекратился, они вышли на улицу и наслаждались свежим воздухом. \
она отправила ему длинное сообщение, в котором объяснила всё, что произошло на прошлой неделе. \
компания объявила о новом продукте, который изменит способ общения людей друг с другом. \
он всегда держал свой стол в чистоте и порядке, чтобы лучше сосредоточиться на работе.";

    pub const DE: &str = "\
hallo, wie geht es dir heute? vielen dank für deine zeit und deine hilfe. \
ich möchte mich bei allen für ihre aufmerksamkeit und teilnahme bedanken. \
heute ist schönes wetter, deshalb können wir zusammen im park spazieren gehen. \
bitte vergiss nicht, deinen laptop, das ladegerät und ein heft mitzubringen. \
dies ist ein einfaches beispiel für gewöhnliche deutsche wörter und sätze. \
menschen schreiben oft nachrichten an freunde über arbeit, familie und reisen. \
die kinder spielten draußen, während ihre eltern ein leckeres abendessen kochten. \
wissen, sprache und kommunikation prägen das verständnis der menschen. \
sie öffnete die tür und sah, dass das licht im zimmer noch an war. \
sie gingen gemeinsam die straße entlang und unterhielten sich über ihre pläne. \
er schaute auf den bildschirm und versuchte herauszufinden, was schief gelaufen war. \
das team beschloss, sich montagmorgen vor der wichtigen besprechung zu treffen. \
nach einem langen arbeitstag setzte er sich endlich hin und trank einen kaffee. \
der bericht war rechtzeitig fertig und wurde ohne verzögerung an den kunden geschickt. \
studenten müssen jeden tag üben, wenn sie ihre leistungen verbessern wollen. \
das wetter war kalt und windig, deshalb entschlossen sie sich, zu hause zu bleiben. \
sie öffnete das buch und begann von anfang an sehr aufmerksam zu lesen. \
alle unterlagen wurden unterschrieben und das geschäft wurde heute abend offiziell abgeschlossen. \
er fuhr früh morgens ins büro, um vor dem arbeitsbeginn dort zu sein. \
die blumen im garten waren wunderschön und dufteten herrlich an diesem frühlingstag.";
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
