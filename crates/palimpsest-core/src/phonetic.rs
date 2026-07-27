//! Telling a *phonetic* Cyrillic layout from a *positional* one.
//!
//! Palimpsest's default heuristic works because wrong-layout text is
//! phonotactically illegal: typing `привет` on ЙЦУКЕН while English is active
//! yields `ghbdtn`, which no bigram model likes. Phonetic layouts (Russian
//! Phonetic, KLID `a0000419` and friends) put each Cyrillic letter on the key of
//! its Latin sound-alike, so the same word is typed `p r i v e t` and comes out
//! as **`privet`** — pronounceable, and here an actual English word. The margin
//! that drives the default decision collapses, and the cross-alphabet floor
//! never fires because no exotic character appears.
//!
//! So we must know which family a layout belongs to before scoring against it.
//! Fingerprint it: ask what a handful of Latin letter keys produce under the
//! layout, and count how many land on their own transliteration. A phonetic
//! layout scores near 1.0; ЙЦУКЕН scores near 0.
//!
//! The check is deliberately pure — it takes a [`Reinterpreter`] rather than
//! calling Windows — so the whole classification is unit-tested headlessly.

use crate::analyze::Reinterpreter;
use crate::buffer::Key;
use crate::config::LayoutPair;
use crate::scoring::Lang;

/// Latin keys and the Cyrillic letter each carries on a phonetic layout. Chosen
/// to be unambiguous: every one of these has a single obvious sound-alike, and
/// ЙЦУКЕН maps all of them somewhere unrelated (`a`→`ф`, `o`→`щ`, `e`→`у`, …).
const PROBES: &[(char, char)] = &[
    ('a', 'а'),
    ('o', 'о'),
    ('e', 'е'),
    ('k', 'к'),
    ('m', 'м'),
    ('t', 'т'),
    ('s', 'с'),
    ('b', 'б'),
    ('d', 'д'),
];

/// Fraction of probes that must match before we call a layout phonetic. Well
/// clear of both families: a real phonetic layout hits ~1.0 and ЙЦУКЕН ~0.0, so
/// the exact cut-off never decides a real case. Variants that move one or two
/// letters still classify correctly.
const PHONETIC_THRESHOLD: f64 = 0.7;

/// Fraction of [`PROBES`] whose key produces its transliteration match under
/// `klid`. `0.0` for any non-Cyrillic layout — "phonetic" is only meaningful for
/// a layout that types a different alphabet.
pub fn phonetic_score(klid: &str, reinterp: &dyn Reinterpreter) -> f64 {
    if Lang::from_klid(klid) != Some(Lang::Ru) {
        return 0.0;
    }
    let matched = PROBES
        .iter()
        .filter(|(latin, cyrillic)| {
            // The virtual-key code of a letter is its uppercase ASCII value, and
            // a zero scan code lets the reinterpreter derive the real one.
            let key = Key { vk: latin.to_ascii_uppercase() as u32, scan: 0 };
            reinterp.reinterpret(&[key], klid) == cyrillic.to_string()
        })
        .count();
    matched as f64 / PROBES.len() as f64
}

/// Whether `klid` is a phonetic Cyrillic layout. Fails safe: an unloadable
/// layout scores 0.0 and is treated as positional, which is the behaviour
/// Palimpsest had before phonetic support existed.
pub fn is_phonetic(klid: &str, reinterp: &dyn Reinterpreter) -> bool {
    phonetic_score(klid, reinterp) >= PHONETIC_THRESHOLD
}

/// Fingerprint every pair that has not been classified yet and record the
/// answer, so the cost is paid once at startup rather than per word. Returns
/// `true` if anything changed, so the caller can persist.
///
/// Pairs the user has already answered for (`Some(_)`) are left alone — a
/// misdetection corrected in Settings must stick.
pub fn classify_pairs(pairs: &mut [LayoutPair], reinterp: &dyn Reinterpreter) -> bool {
    let mut changed = false;
    for pair in pairs.iter_mut().filter(|p| p.phonetic.is_none()) {
        pair.phonetic =
            Some(is_phonetic(&pair.from, reinterp) || is_phonetic(&pair.to, reinterp));
        changed = true;
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Positional (ЙЦУКЕН) and phonetic mappings for the probe keys, so both
    /// families can be classified without Windows.
    struct FakeLayouts;

    impl Reinterpreter for FakeLayouts {
        fn reinterpret(&self, keys: &[Key], klid: &str) -> String {
            const QWERTY: &str = "qwertyuiop[]asdfghjkl;'zxcvbnm,./";
            const YCUKEN: &str = "йцукенгшщзхъфывапролджэячсмитьбю.";
            keys.iter()
                .map(|k| {
                    let latin = (k.vk as u8 as char).to_ascii_lowercase();
                    match klid {
                        // Phonetic: each Latin key carries its sound-alike.
                        "a0000419" => PROBES
                            .iter()
                            .find(|(l, _)| *l == latin)
                            .map(|(_, c)| *c)
                            .unwrap_or(latin),
                        // Positional ЙЦУКЕН.
                        "00000419" => QWERTY
                            .chars()
                            .position(|c| c == latin)
                            .and_then(|i| YCUKEN.chars().nth(i))
                            .unwrap_or(latin),
                        _ => latin,
                    }
                })
                .collect()
        }
    }

    #[test]
    fn phonetic_layout_is_recognised() {
        assert_eq!(phonetic_score("a0000419", &FakeLayouts), 1.0);
        assert!(is_phonetic("a0000419", &FakeLayouts));
    }

    #[test]
    fn positional_layout_is_not_phonetic() {
        // ЙЦУКЕН maps every probe somewhere unrelated to its Latin sound.
        assert_eq!(phonetic_score("00000419", &FakeLayouts), 0.0);
        assert!(!is_phonetic("00000419", &FakeLayouts));
    }

    #[test]
    fn latin_layouts_are_never_phonetic() {
        // "Phonetic" only means something for a layout typing another alphabet;
        // English and German must not be dragged onto the phonetic path.
        assert_eq!(phonetic_score("00000409", &FakeLayouts), 0.0);
        assert_eq!(phonetic_score("00000407", &FakeLayouts), 0.0);
    }

    #[test]
    fn classify_fills_in_unknown_pairs_only() {
        let mut pairs = vec![
            LayoutPair::new("00000409", "a0000419"), // unclassified → phonetic
            LayoutPair::new("00000409", "00000419"), // unclassified → positional
            // The user said "not phonetic" about a layout that fingerprints as
            // one; their answer must survive.
            LayoutPair { from: "00000409".into(), to: "a0000419".into(), phonetic: Some(false) },
        ];
        assert!(classify_pairs(&mut pairs, &FakeLayouts));
        assert_eq!(pairs[0].phonetic, Some(true));
        assert_eq!(pairs[1].phonetic, Some(false));
        assert_eq!(pairs[2].phonetic, Some(false));

        // Second run has nothing left to do.
        assert!(!classify_pairs(&mut pairs, &FakeLayouts));
    }

    #[test]
    fn an_unloadable_layout_falls_back_to_positional() {
        struct Dead;
        impl Reinterpreter for Dead {
            fn reinterpret(&self, _: &[Key], _: &str) -> String {
                String::new() // LoadKeyboardLayoutW failed
            }
        }
        assert!(!is_phonetic("a0000419", &Dead));
    }
}
