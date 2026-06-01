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
/// configured pair as bidirectional.
fn candidate_targets<'a>(current: &str, pairs: &'a [LayoutPair]) -> Vec<&'a str> {
    let mut out = Vec::new();
    let cur = Lang::from_klid(current);
    let mut consider = |klid: &'a str| {
        // Compare by language id so "00000419" and an HKL-derived id match, and
        // never offer the current layout as its own target.
        if Lang::from_klid(klid) != cur && !out.contains(&klid) {
            out.push(klid);
        }
    };
    for pair in pairs {
        if Lang::from_klid(&pair.from) == cur {
            consider(pair.to.as_str());
        } else if Lang::from_klid(&pair.to) == cur {
            consider(pair.from.as_str());
        }
    }
    out
}

/// True if the word, as currently displayed, looks like deliberate non-word
/// input we should never touch (contains digits or whitespace).
fn has_disqualifying_chars(text: &str) -> bool {
    text.chars().any(|c| c.is_ascii_digit() || c.is_whitespace())
}

/// The current interpretation plus the best plausible alternate found.
struct Eval {
    current_text: String,
    current_score: f64,
    /// Best alternate that differs from the current text and clears the
    /// plausibility floor, with its KLID, text, and score.
    best: Option<(String, String, f64)>,
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

    let mut best: Option<(String, String, f64)> = None;
    for target in candidate_targets(current_layout, pairs) {
        let Some(target_lang) = Lang::from_klid(target) else { continue };
        let alt_text = reinterp.reinterpret(word, target);
        if alt_text.is_empty() || alt_text == current_text {
            continue;
        }
        let alt_score = models.score(target_lang, &alt_text);
        if alt_score >= ALT_PLAUSIBILITY_FLOOR
            && best.as_ref().is_none_or(|(_, _, s)| alt_score > *s)
        {
            best = Some((target.to_string(), alt_text, alt_score));
        }
    }

    Some(Eval { current_text, current_score, best })
}

fn correction_from(eval: &Eval, target: &str, text: &str) -> Decision {
    Decision::Correct {
        backspaces: eval.current_text.chars().count(),
        text: text.to_string(),
        target_layout: target.to_string(),
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
    match &eval.best {
        Some((target, text, score)) if score - eval.current_score >= margin_for(sensitivity) => {
            correction_from(&eval, target, text)
        }
        _ => Decision::Keep,
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
    match &eval.best {
        Some((target, text, score)) if *score > eval.current_score => {
            correction_from(&eval, target, text)
        }
        _ => Decision::Keep,
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
        // At sensitivity 0 the margin is high enough that even a real
        // wrong-layout word is left alone — biasing hard against false positives.
        let d = analyze(&keys("ghbdtn"), EN, &pairs(), &FakeReinterp, &Models::builtin(), 0.0);
        assert_eq!(d, Decision::Keep);
    }

    #[test]
    fn manual_suggest_forces_correction_ignoring_margin() {
        // The same word the most-conservative auto setting leaves alone is
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
}
