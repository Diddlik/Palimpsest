//! Phonetic Latin → Russian (Cyrillic) transliteration.
//!
//! Provides a character-by-character mapping from ASCII letters to their
//! closest-sounding Russian equivalents. The analyser uses this as an additional
//! reinterpretation candidate alongside the physical-key-position mappings
//! produced by `ToUnicodeEx`, so users who type Russian *by sound* on a Latin
//! keyboard get automatic correction even when they are not using the ЙЦУКЕН
//! physical layout.
//!
//! ## Design choices
//!
//! - **One Latin char → one Cyrillic char.** Digraph sequences (sh→ш, zh→ж,
//!   ya→я, …) are more accurate but require lookahead and produce variable-length
//!   output that complicates backspace counting. The single-char table already
//!   handles the common case well because the bigram scorer does not require the
//!   exact word — only a text that scores unmistakably higher in Russian than in
//!   English.
//! - **Non-letter characters pass through unchanged** so mixed input (digits,
//!   punctuation) is preserved and the result can be inspected for Cyrillic
//!   content by the caller.
//! - The mapping follows the de-facto conventions used by most CIS users when
//!   typing Russian on a Latin keyboard without a Russian layout installed:
//!   sounds that have a near-identical Russian letter get that letter; ambiguous
//!   cases (c, q, w, x, y) use the most common convention.

/// Transliterate ASCII text to Russian Cyrillic phonetically.
///
/// Each ASCII letter is replaced with its phonetic Russian equivalent; all other
/// characters (Cyrillic, digits, punctuation, whitespace) are left unchanged.
/// The returned string will contain Cyrillic characters if and only if the input
/// contained ASCII letters.
pub fn transliterate_ru(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 2);
    for c in text.chars() {
        out.push(phonetic_char(c));
    }
    out
}

/// Returns `true` if `text` contains at least one Cyrillic character, i.e. the
/// transliteration actually produced Russian output (rather than passing all
/// characters through unchanged, which would happen for a digit-only string).
pub fn has_cyrillic(text: &str) -> bool {
    text.chars().any(|c| ('\u{0400}'..='\u{04FF}').contains(&c))
}

/// Map a single character to its phonetic Russian equivalent.  Non-ASCII and
/// non-letter characters are returned unchanged.
fn phonetic_char(c: char) -> char {
    // Work on the lowercase form so the lookup is case-independent; the bigram
    // scorer lowercases everything anyway.
    match c.to_ascii_lowercase() {
        'a' => 'а',
        'b' => 'б',
        'c' => 'с', // /s/ or /ts/; 'с' is the more common phonetic convention
        'd' => 'д',
        'e' => 'е',
        'f' => 'ф',
        'g' => 'г',
        'h' => 'х', // aspirate /x/ is the closest Russian sound
        'i' => 'и',
        'j' => 'й', // /j/ = й before vowels
        'k' => 'к',
        'l' => 'л',
        'm' => 'м',
        'n' => 'н',
        'o' => 'о',
        'p' => 'п',
        'q' => 'к', // no native q-sound; treat as k
        'r' => 'р',
        's' => 'с',
        't' => 'т',
        'u' => 'у',
        'v' => 'в',
        'w' => 'в', // closest single-char Russian sound (some systems use ш)
        'x' => 'х', // simplified: х rather than "кс" to keep char-for-char mapping
        'y' => 'й', // /j/ sound; ы is an alternative but й is more natural here
        'z' => 'з',
        // Everything else (Cyrillic, digits, punctuation) passes through.
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn common_russian_words_transliterate_correctly() {
        assert_eq!(transliterate_ru("privet"), "привет");
        assert_eq!(transliterate_ru("kak"), "как");
        assert_eq!(transliterate_ru("dela"), "дела");
        assert_eq!(transliterate_ru("u"), "у");
    }

    #[test]
    fn non_letters_pass_through() {
        assert_eq!(transliterate_ru("hello, world!"), "хелло, ворлд!");
        assert_eq!(transliterate_ru("42"), "42");
    }

    #[test]
    fn cyrillic_input_passes_through_unchanged() {
        assert_eq!(transliterate_ru("привет"), "привет");
    }

    #[test]
    fn has_cyrillic_detects_output() {
        assert!(has_cyrillic(&transliterate_ru("kak")));
        assert!(!has_cyrillic("kak")); // original is pure Latin
        assert!(!has_cyrillic("42"));
    }
}
