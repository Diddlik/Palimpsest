//! The decision layer: given a buffered word and a way to reinterpret it under
//! candidate layouts, decide whether to correct it — and to what.
//!
//! This is kept pure by abstracting the one OS-specific operation, mapping
//! physical keys to characters under a given layout, behind the [`Reinterpreter`]
//! trait. The Windows implementation (`ToUnicodeEx`) lives in
//! [`crate::reinterpret`]; tests supply a fake. Everything else here, including
//! every guardrail, is exercised headlessly.

use crate::buffer::Key;
use crate::config::LayoutPair;
use crate::scoring::{Lang, Models};

/// Maps physical keystrokes to the text they would produce under a layout.
pub trait Reinterpreter {
    /// Reinterpret `keys` as typed under the keyboard layout identified by
    /// `klid` (a Windows KLID hex string). Returns the resulting text.
    fn reinterpret(&self, keys: &[Key], klid: &str) -> String;
}

/// Re-renders already-visible text across layouts — the seam the
/// "convert selection" hotkey uses. Where [`Reinterpreter`] starts from physical
/// keys, this starts from characters: for each one it finds the key that would
/// produce it under `from`, then asks what that key produces under `to`,
/// preserving case. Characters with no such key pass through unchanged.
pub trait Transliterator {
    /// Re-render `text` as if the keys that typed it under `from` had been
    /// pressed under `to`.
    fn transliterate(&self, text: &str, from: &str, to: &str) -> String;
}

/// A reinterpretation of selected text under a better-scoring layout.
#[derive(Debug, Clone, PartialEq)]
pub struct TextCorrection {
    /// The text to type in place of the selection.
    pub text: String,
    /// KLID the OS layout should switch to so continued typing is correct.
    pub target_layout: String,
}

/// The engine's verdict for one completed word.
#[derive(Debug, Clone, PartialEq)]
pub enum Decision {
    /// Leave the text as typed.
    Keep,
    /// Replace the typed text with a corrected layout interpretation.
    Correct {
        /// How many characters to erase (the word as currently displayed).
        backspaces: usize,
        /// The corrected text to type in their place.
        text: String,
        /// KLID the OS layout should switch to so continued typing is correct.
        target_layout: String,
    },
}

/// Minimum word length (in characters) to consider — shorter words are too
/// ambiguous and a rich source of false positives.
const MIN_WORD_LEN: usize = 2;

/// Most conservative per-bigram log-prob margin the alternate must beat the
/// current interpretation by (sensitivity = 0.0).
const MARGIN_CONSERVATIVE: f64 = 2.0;
/// Most aggressive margin (sensitivity = 1.0). The default sensitivity (0.5)
/// lands at ~1.15, between a genuine wrong-layout word (improves by well over 1
/// nat/bigram) and a correctly typed word (improves by < 0).
const MARGIN_AGGRESSIVE: f64 = 0.3;

/// The alternate interpretation must itself be at least this plausible; stops us
/// "correcting" gibberish into other gibberish that merely scores less badly.
const ALT_PLAUSIBILITY_FLOOR: f64 = -8.0;

/// Map a 0.0..=1.0 sensitivity to a required score margin. Higher sensitivity →
/// smaller margin → more eager to correct.
fn margin_for(sensitivity: f32) -> f64 {
    let s = sensitivity.clamp(0.0, 1.0) as f64;
    MARGIN_CONSERVATIVE - s * (MARGIN_CONSERVATIVE - MARGIN_AGGRESSIVE)
}

/// Candidate target layouts to try given the current layout, treating each
/// configured pair as bidirectional. Each candidate carries its pair's phonetic
/// flag, which selects the decision path further down.
fn candidate_targets<'a>(current: &str, pairs: &'a [LayoutPair]) -> Vec<(&'a str, bool)> {
    let mut out: Vec<(&'a str, bool)> = Vec::new();
    let cur = Lang::from_klid(current);
    let mut consider = |klid: &'a str, phonetic: bool| {
        // Compare by language id so "00000419" and an HKL-derived id match, and
        // never offer the current layout as its own target.
        if Lang::from_klid(klid) != cur && !out.iter().any(|(k, _)| *k == klid) {
            out.push((klid, phonetic));
        }
    };
    for pair in pairs {
        if Lang::from_klid(&pair.from) == cur {
            consider(pair.to.as_str(), pair.is_phonetic());
        } else if Lang::from_klid(&pair.to) == cur {
            consider(pair.from.as_str(), pair.is_phonetic());
        }
    }
    out
}

/// True if the word, as currently displayed, looks like deliberate non-word
/// input we should never touch (contains digits or whitespace).
fn has_disqualifying_chars(text: &str) -> bool {
    text.chars().any(|c| c.is_ascii_digit() || c.is_whitespace())
}

/// One reinterpretation of the word under a candidate layout.
struct Candidate {
    /// KLID the OS should switch to if this candidate wins.
    klid: String,
    lang: Lang,
    text: String,
    score: f64,
    /// Whether the pair that produced this candidate is phonetic.
    phonetic: bool,
}

/// The current interpretation plus the best plausible alternate found.
struct Eval {
    current_text: String,
    /// Language of the layout that produced `current_text`.
    current_lang: Lang,
    current_score: f64,
    /// Best alternate that differs from the current text and clears the
    /// plausibility floor.
    best: Option<Candidate>,
}

/// Shared core of [`analyze`] and [`suggest`]: apply the guardrails, reinterpret
/// under every candidate layout, and pick the best plausible alternate — without
/// yet judging whether the improvement clears any margin.
fn evaluate(
    word: &[Key],
    current_layout: &str,
    pairs: &[LayoutPair],
    reinterp: &dyn Reinterpreter,
    models: &Models,
) -> Option<Eval> {
    if word.len() < MIN_WORD_LEN {
        return None;
    }
    let current_lang = Lang::from_klid(current_layout)?; // unknown layout — can't compare
    let current_text = reinterp.reinterpret(word, current_layout);
    if current_text.chars().count() < MIN_WORD_LEN || has_disqualifying_chars(&current_text) {
        return None;
    }
    let current_score = models.score(current_lang, &current_text);

    let mut best: Option<Candidate> = None;
    for (target, phonetic) in candidate_targets(current_layout, pairs) {
        let Some(target_lang) = Lang::from_klid(target) else { continue };
        let alt_text = reinterp.reinterpret(word, target);
        if alt_text.is_empty() || alt_text == current_text {
            continue;
        }
        let alt_score = models.score(target_lang, &alt_text);
        if alt_score >= ALT_PLAUSIBILITY_FLOOR
            && best.as_ref().is_none_or(|c| alt_score > c.score)
        {
            best = Some(Candidate {
                klid: target.to_string(),
                lang: target_lang,
                text: alt_text,
                score: alt_score,
                phonetic,
            });
        }
    }

    Some(Eval { current_text, current_lang, current_score, best })
}

fn correction_from(eval: &Eval, candidate: &Candidate) -> Decision {
    Decision::Correct {
        backspaces: eval.current_text.chars().count(),
        text: candidate.text.clone(),
        target_layout: candidate.klid.clone(),
    }
}

/// Shortest word a *phonetic* pair may auto-correct. Romanizations of short
/// Russian words collide constantly with real Latin words (`to`, `no`, `da`,
/// `net`, `mir`, `ona`), and unlike the positional case there is no gibberish
/// signal to fall back on — so short words are left to the manual hotkey.
const MIN_PHONETIC_WORD_LEN: usize = 4;

/// The decision for a phonetic pair, where the bigram margin is not a usable
/// signal (see [`crate::phonetic`]). Here the question is not "is this
/// phonotactically plausible" — a romanization always is — but "is this a real
/// word", so the wordlists decide:
///
/// | alternate is a word | current is a word | verdict                        |
/// |---------------------|-------------------|--------------------------------|
/// | yes                 | no                | correct (`spasibo` → `спасибо`)|
/// | no                  | yes               | keep                           |
/// | yes                 | yes               | keep — ambiguous, manual only   |
/// | no                  | no                | fall back to the bigram margin  |
///
/// The both-are-words row is where phonetic layouts generate false positives
/// (`privet`, `net`, `to`, `son`, `mir`), and no global rule can resolve it —
/// which reading is meant depends on the person. Auto keeps; the manual hotkey
/// and the learned-word dictionary settle it per user.
fn phonetic_decision(
    eval: &Eval,
    candidate: &Candidate,
    models: &Models,
    sensitivity: f32,
) -> Decision {
    let alt_is_word = models.is_word(candidate.lang, &candidate.text);
    let current_is_word = models.is_word(eval.current_lang, &eval.current_text);
    match (alt_is_word, current_is_word) {
        (true, false) if candidate.text.chars().count() >= MIN_PHONETIC_WORD_LEN => {
            correction_from(eval, candidate)
        }
        (true, false) => Decision::Keep, // too short to risk automatically
        (false, false) if candidate.score - eval.current_score >= margin_for(sensitivity) => {
            correction_from(eval, candidate)
        }
        _ => Decision::Keep,
    }
}

/// Decide whether to **automatically** correct one completed word.
///
/// - `word` — the buffered physical keys (see [`crate::buffer`]).
/// - `current_layout` — KLID of the layout that produced the on-screen text.
/// - `pairs` — configured layout pairs (bidirectional).
/// - `reinterp` — maps keys → text under a layout.
/// - `models` — per-language scoring.
/// - `sensitivity` — 0.0 (conservative) .. 1.0 (aggressive).
///
/// Corrects only when the best alternate beats the current interpretation by the
/// sensitivity-derived margin — biased toward leaving text alone when unsure.
pub fn analyze(
    word: &[Key],
    current_layout: &str,
    pairs: &[LayoutPair],
    reinterp: &dyn Reinterpreter,
    models: &Models,
    sensitivity: f32,
) -> Decision {
    let Some(eval) = evaluate(word, current_layout, pairs, reinterp, models) else {
        return Decision::Keep;
    };
    // The user undid a correction of this exact word, so they have told us it is
    // right as typed. That outranks the score comparison however wide the margin
    // — a bonus large enough to overturn `ghbdtn` → `привет` would distort every
    // other decision. The manual hotkey still overrides, which is how a word
    // learned by mistake gets corrected anyway.
    if models.is_user_word(eval.current_lang, &eval.current_text) {
        return Decision::Keep;
    }
    let Some(candidate) = &eval.best else { return Decision::Keep };
    if candidate.phonetic {
        return phonetic_decision(&eval, candidate, models, sensitivity);
    }
    if candidate.score - eval.current_score >= margin_for(sensitivity) {
        correction_from(&eval, candidate)
    } else {
        Decision::Keep
    }
}

/// Force a re-interpretation with **no sensitivity margin** — the manual-hotkey
/// path, for when the user knows a word is in the wrong layout but the automatic
/// heuristic wasn't confident enough. It still requires the alternate to be
/// *strictly more plausible than the current text*, so explicitly invoking it on
/// correctly typed text leaves that text alone rather than mangling it.
pub fn suggest(
    word: &[Key],
    current_layout: &str,
    pairs: &[LayoutPair],
    reinterp: &dyn Reinterpreter,
    models: &Models,
) -> Decision {
    let Some(eval) = evaluate(word, current_layout, pairs, reinterp, models) else {
        return Decision::Keep;
    };
    let Some(candidate) = &eval.best else { return Decision::Keep };
    // On a phonetic pair the scores of two real words are not comparable in a
    // useful way — that is the whole problem — so an explicit request converts
    // whenever the alternate is a real word. This is the resolution path for the
    // ambiguous `privet`/`net`/`to` row that auto deliberately declines.
    if candidate.phonetic && models.is_word(candidate.lang, &candidate.text) {
        return correction_from(&eval, candidate);
    }
    if candidate.score > eval.current_score {
        correction_from(&eval, candidate)
    } else {
        Decision::Keep
    }
}

/// Decide how to reinterpret an already-rendered string — a user *selection* —
/// across the configured layout pairs. Unlike [`suggest`], which works from
/// buffered physical keys, this starts from the visible characters via a
/// [`Transliterator`]. Like the manual hotkey it applies no sensitivity margin,
/// but still requires the alternate to be *strictly more plausible* than the
/// selection as it stands, so invoking it on correctly-typed text is a no-op.
///
/// Returns `None` to leave the selection untouched.
pub fn suggest_text(
    text: &str,
    current_layout: &str,
    pairs: &[LayoutPair],
    trans: &dyn Transliterator,
    models: &Models,
) -> Option<TextCorrection> {
    if text.chars().count() < MIN_WORD_LEN {
        return None;
    }
    let current_lang = Lang::from_klid(current_layout)?; // unknown layout — can't compare
    let current_score = models.score(current_lang, text);

    let mut best: Option<(String, String, f64, bool, Lang)> = None;
    for (target, phonetic) in candidate_targets(current_layout, pairs) {
        let Some(target_lang) = Lang::from_klid(target) else { continue };
        let alt = trans.transliterate(text, current_layout, target);
        if alt.is_empty() || alt == text {
            continue;
        }
        let alt_score = models.score(target_lang, &alt);
        if alt_score >= ALT_PLAUSIBILITY_FLOOR
            && best.as_ref().is_none_or(|(_, _, s, _, _)| alt_score > *s)
        {
            best = Some((target.to_string(), alt, alt_score, phonetic, target_lang));
        }
    }

    // Like `suggest`, an explicit request on a phonetic pair converts whenever
    // the alternate reads as a real word, since the score comparison cannot
    // separate two plausible readings.
    match best {
        Some((target, alt, score, phonetic, lang))
            if score > current_score || (phonetic && models.is_word(lang, &alt)) =>
        {
            Some(TextCorrection { text: alt, target_layout: target })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::LayoutPair;

    const EN: &str = "00000409";
    const RU: &str = "00000419";

    /// Fake reinterpreter built on the standard QWERTY↔ЙЦУКЕН letter mapping,
    /// so tests can map physical keys to EN or RU text without Windows.
    struct FakeReinterp;

    impl FakeReinterp {
        fn ru_char(latin: char) -> char {
            const QWERTY: &str = "qwertyuiop[]asdfghjkl;'zxcvbnm,./";
            const RU: &str = "йцукенгшщзхъфывапролджэячсмитьбю.";
            QWERTY
                .chars()
                .position(|c| c == latin)
                .and_then(|i| RU.chars().nth(i))
                .unwrap_or(latin)
        }
    }

    impl Reinterpreter for FakeReinterp {
        fn reinterpret(&self, keys: &[Key], klid: &str) -> String {
            let lang = Lang::from_klid(klid);
            keys.iter()
                .map(|k| {
                    let latin = (k.vk as u8 as char).to_ascii_lowercase();
                    match lang {
                        Some(Lang::Ru) => Self::ru_char(latin),
                        _ => latin,
                    }
                })
                .collect()
        }
    }

    /// Build a word from ASCII letters (vk == uppercase ASCII for letter keys).
    fn keys(s: &str) -> Vec<Key> {
        s.bytes()
            .map(|b| {
                let vk = b.to_ascii_uppercase() as u32;
                Key { vk, scan: vk }
            })
            .collect()
    }

    fn pairs() -> Vec<LayoutPair> {
        vec![LayoutPair::new(EN, RU)]
    }

    fn analyze_word(word: &str, current: &str) -> Decision {
        analyze(&keys(word), current, &pairs(), &FakeReinterp, &Models::builtin(), 0.5)
    }

    #[test]
    fn wrong_layout_word_is_corrected() {
        // "ghbdtn" typed on EN is Russian "привет".
        let d = analyze_word("ghbdtn", EN);
        match d {
            Decision::Correct { backspaces, text, target_layout } => {
                assert_eq!(backspaces, 6);
                assert_eq!(text, "привет");
                assert_eq!(Lang::from_klid(&target_layout), Some(Lang::Ru));
            }
            other => panic!("expected correction, got {other:?}"),
        }
    }

    #[test]
    fn correct_english_is_left_alone() {
        assert_eq!(analyze_word("hello", EN), Decision::Keep);
    }

    #[test]
    fn correctly_typed_russian_is_left_alone() {
        // Physically typing "ghbdtn" while the RU layout is active yields the
        // real word "привет"; the EN reinterpretation is gibberish, so keep.
        assert_eq!(analyze_word("ghbdtn", RU), Decision::Keep);
    }

    #[test]
    fn short_words_are_ignored() {
        assert_eq!(analyze_word("u", EN), Decision::Keep); // 1 char
    }

    #[test]
    fn unknown_current_layout_is_ignored() {
        assert_eq!(analyze_word("ghbdtn", "0000040c"), Decision::Keep); // French
    }

    #[test]
    fn higher_sensitivity_lowers_the_margin() {
        assert!(margin_for(1.0) < margin_for(0.0));
    }

    #[test]
    fn most_conservative_setting_suppresses_the_borderline_correction() {
        // "ldbtybt" reinterprets to "двиение" — Russian-shaped, but not a real
        // word, so no wordlist confirms it and it rests on bigram evidence
        // alone. That clears the default margin but not the conservative one.
        let word = keys("ldbtybt");
        let models = Models::builtin();
        assert_eq!(analyze(&word, EN, &pairs(), &FakeReinterp, &models, 0.0), Decision::Keep);
        assert!(matches!(
            analyze(&word, EN, &pairs(), &FakeReinterp, &models, 0.5),
            Decision::Correct { .. }
        ));
    }

    #[test]
    fn wordlist_confirmed_word_is_corrected_even_when_most_conservative() {
        // "привет" is in the Russian wordlist, so the evidence clears even the
        // widest margin. The conservative setting costs recall only on words no
        // wordlist can vouch for.
        let d = analyze(&keys("ghbdtn"), EN, &pairs(), &FakeReinterp, &Models::builtin(), 0.0);
        assert!(matches!(d, Decision::Correct { .. }));
    }

    #[test]
    fn real_word_whose_reinterpretation_is_near_russian_is_left_alone() {
        // "hers" maps to "рукы" — one letter off the real word "рука". The
        // English wordlist vouches for "hers" and nothing vouches for "рукы",
        // so the comparison must come out negative. This is the false-positive
        // case the whole design is biased against.
        assert_eq!(analyze_word("hers", EN), Decision::Keep);
    }

    #[test]
    fn a_learned_word_is_never_auto_corrected_again() {
        // The learning loop end to end: a word that is corrected by default
        // stops being corrected once an undo has taught it.
        let models = Models::builtin();
        let word = keys("ghbdtn");
        assert!(matches!(
            analyze(&word, EN, &pairs(), &FakeReinterp, &models, 0.5),
            Decision::Correct { .. }
        ));

        assert!(models.learn(Lang::En, "ghbdtn"));
        assert_eq!(analyze(&word, EN, &pairs(), &FakeReinterp, &models, 0.5), Decision::Keep);

        // Even at maximum sensitivity — the veto is not a score, so no slider
        // position can outvote it.
        assert_eq!(analyze(&word, EN, &pairs(), &FakeReinterp, &models, 1.0), Decision::Keep);
    }

    #[test]
    fn learning_is_scoped_to_the_language_it_was_typed_in() {
        // Teaching "ghbdtn" as Russian must not protect it while EN is active.
        let models = Models::builtin();
        assert!(models.learn(Lang::Ru, "ghbdtn"));
        assert!(matches!(
            analyze(&keys("ghbdtn"), EN, &pairs(), &FakeReinterp, &models, 0.5),
            Decision::Correct { .. }
        ));
    }

    #[test]
    fn manual_hotkey_overrides_a_learned_word() {
        // The escape hatch for a word learned by mistake: auto leaves it alone,
        // but explicitly asking still converts it.
        let models = Models::builtin();
        assert!(models.learn(Lang::En, "ghbdtn"));
        assert_eq!(analyze(&keys("ghbdtn"), EN, &pairs(), &FakeReinterp, &models, 0.5), Decision::Keep);
        match suggest(&keys("ghbdtn"), EN, &pairs(), &FakeReinterp, &models) {
            Decision::Correct { text, .. } => assert_eq!(text, "привет"),
            other => panic!("manual hotkey should still override, got {other:?}"),
        }
    }

    #[test]
    fn manual_suggest_forces_correction_ignoring_margin() {
        // A word the conservative auto setting would leave alone is still
        // corrected when the user explicitly asks (manual hotkey).
        let d = suggest(&keys("ghbdtn"), EN, &pairs(), &FakeReinterp, &Models::builtin());
        match d {
            Decision::Correct { text, target_layout, .. } => {
                assert_eq!(text, "привет");
                assert_eq!(Lang::from_klid(&target_layout), Some(Lang::Ru));
            }
            other => panic!("expected forced correction, got {other:?}"),
        }
    }

    #[test]
    fn manual_suggest_still_refuses_a_real_word() {
        // Forcing must not turn good English into Cyrillic garbage.
        assert_eq!(
            suggest(&keys("hello"), EN, &pairs(), &FakeReinterp, &Models::builtin()),
            Decision::Keep
        );
    }

    // ----- Phonetic layouts (plan §6) ----------------------------------------

    /// A phonetic Russian layout, as installed on machines that prefer
    /// sound-alike input. `Lang::from_klid` still resolves it to Russian.
    const PHON: &str = "a0000419";

    /// Fake *phonetic* reinterpreter: every Latin key carries its Cyrillic
    /// sound-alike, so `privet` is what `привет` looks like typed on a Latin
    /// layout — pronounceable, unlike the positional `ghbdtn`.
    struct FakePhonetic;

    impl FakePhonetic {
        fn ru_char(latin: char) -> char {
            const LATIN: &str = "abcdefghijklmnopqrstuvwxyz";
            const CYRILLIC: &str = "абцдефгхийклмнопярстувшчыз";
            LATIN
                .chars()
                .position(|c| c == latin)
                .and_then(|i| CYRILLIC.chars().nth(i))
                .unwrap_or(latin)
        }
    }

    impl Reinterpreter for FakePhonetic {
        fn reinterpret(&self, keys: &[Key], klid: &str) -> String {
            keys.iter()
                .map(|k| {
                    let latin = (k.vk as u8 as char).to_ascii_lowercase();
                    match Lang::from_klid(klid) {
                        Some(Lang::Ru) => Self::ru_char(latin),
                        _ => latin,
                    }
                })
                .collect()
        }
    }

    fn phonetic_pairs() -> Vec<LayoutPair> {
        vec![LayoutPair { from: EN.into(), to: PHON.into(), phonetic: Some(true) }]
    }

    fn phonetic_analyze(word: &str) -> Decision {
        analyze(&keys(word), EN, &phonetic_pairs(), &FakePhonetic, &Models::builtin(), 0.5)
    }

    #[test]
    fn romanization_of_a_russian_word_is_corrected() {
        // The alternate is a real Russian word and the Latin text is not an
        // English one — the unambiguous row of the matrix.
        for (typed, expected) in
            [("spasibo", "спасибо"), ("poka", "пока"), ("kniga", "книга"), ("privet", "привет")]
        {
            match phonetic_analyze(typed) {
                Decision::Correct { text, .. } => assert_eq!(text, expected, "for {typed}"),
                other => panic!("expected {typed} → {expected}, got {other:?}"),
            }
        }
    }

    #[test]
    fn a_latin_word_that_is_also_a_romanization_is_left_alone() {
        // Both readings are real words ("net"/"нет", "son"/"сон", …). No global
        // rule can say which was meant, so auto-correction must decline — this
        // is the false-positive class phonetic layouts generate.
        for typed in ["net", "son", "mir", "dom", "ona", "tak"] {
            assert_eq!(phonetic_analyze(typed), Decision::Keep, "must not touch {typed}");
        }
    }

    #[test]
    fn short_romanizations_are_not_auto_corrected() {
        // Below MIN_PHONETIC_WORD_LEN. Each of these *is* an unambiguous Russian
        // word whose Latin form is not English, so only the length guard keeps
        // them — short romanizations collide too easily to risk automatically.
        for typed in ["gde", "ili", "dve", "uje", "bez", "dva"] {
            assert_eq!(phonetic_analyze(typed), Decision::Keep, "must not touch {typed}");
        }
    }

    #[test]
    fn manual_hotkey_resolves_the_ambiguous_and_short_rows() {
        // Everything auto declines stays reachable by explicitly asking.
        for (typed, expected) in [("net", "нет"), ("son", "сон"), ("gde", "где")] {
            match suggest(&keys(typed), EN, &phonetic_pairs(), &FakePhonetic, &Models::builtin()) {
                Decision::Correct { text, .. } => assert_eq!(text, expected, "for {typed}"),
                other => panic!("manual should convert {typed} → {expected}, got {other:?}"),
            }
        }
    }

    #[test]
    fn a_real_english_word_is_safe_on_a_phonetic_pair() {
        // "hello" → "хелло" is not a Russian word while "hello" is English, so
        // the matrix keeps it — the direction that would destroy trust fastest.
        assert_eq!(phonetic_analyze("hello"), Decision::Keep);
        assert_eq!(phonetic_analyze("keyboard"), Decision::Keep);
    }

    #[test]
    fn a_pair_not_marked_phonetic_keeps_the_bigram_path() {
        // The flag is what selects the decision path: the same word through the
        // same reinterpreter is judged by the margin when the pair is positional.
        let positional = vec![LayoutPair::new(EN, PHON)]; // phonetic: None
        let d = analyze(
            &keys("spasibo"),
            EN,
            &positional,
            &FakePhonetic,
            &Models::builtin(),
            0.5,
        );
        // "спасибо" is a real word, so the margin path happens to agree here —
        // what matters is that it went through `margin_for`, which a phonetic
        // pair never consults for this row.
        assert!(matches!(d, Decision::Correct { .. }));
    }

    /// Fake transliterator over the QWERTY↔ЙЦУКЕН mapping, both directions, with
    /// case preserved — so `suggest_text` can be exercised without Windows.
    struct FakeTrans;

    impl FakeTrans {
        const QWERTY: &'static str = "qwertyuiop[]asdfghjkl;'zxcvbnm,./";
        const RU: &'static str = "йцукенгшщзхъфывапролджэячсмитьбю.";

        fn map_one(c: char, src: &str, dst: &str) -> char {
            let lower = c.to_lowercase().next().unwrap_or(c);
            match src.chars().position(|x| x == lower) {
                Some(i) => {
                    let mapped = dst.chars().nth(i).unwrap();
                    if c == lower { mapped } else { mapped.to_uppercase().next().unwrap_or(mapped) }
                }
                None => c,
            }
        }
    }

    impl Transliterator for FakeTrans {
        fn transliterate(&self, text: &str, from: &str, to: &str) -> String {
            let (src, dst) = match (Lang::from_klid(from), Lang::from_klid(to)) {
                (Some(Lang::En), Some(Lang::Ru)) => (Self::QWERTY, Self::RU),
                (Some(Lang::Ru), Some(Lang::En)) => (Self::RU, Self::QWERTY),
                _ => return String::new(),
            };
            text.chars().map(|c| Self::map_one(c, src, dst)).collect()
        }
    }

    fn suggest_text_in(text: &str, current: &str) -> Option<TextCorrection> {
        suggest_text(text, current, &pairs(), &FakeTrans, &Models::builtin())
    }

    #[test]
    fn selection_in_wrong_layout_is_converted() {
        let c = suggest_text_in("ghbdtn", EN).expect("should convert");
        assert_eq!(c.text, "привет");
        assert_eq!(Lang::from_klid(&c.target_layout), Some(Lang::Ru));
    }

    #[test]
    fn selection_conversion_preserves_case() {
        let c = suggest_text_in("Ghbdtn", EN).expect("should convert");
        assert_eq!(c.text, "Привет");
    }

    #[test]
    fn selection_of_real_text_is_left_alone() {
        assert_eq!(suggest_text_in("hello", EN), None);
        // Russian correctly typed on the RU layout must not be "fixed" to Latin.
        assert_eq!(suggest_text_in("привет", RU), None);
    }

    #[test]
    fn selection_too_short_is_ignored() {
        assert_eq!(suggest_text_in("g", EN), None);
    }

    #[test]
    fn selection_in_unknown_layout_is_ignored() {
        assert_eq!(suggest_text_in("ghbdtn", "0000040c"), None); // French
    }
}
