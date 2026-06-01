//! Word buffer — accumulates physical keystrokes between word boundaries.
//!
//! Pure and platform-independent: it works on virtual-key + scan code pairs and
//! knows nothing about layouts or `ToUnicodeEx`. The engine feeds it key-downs;
//! it returns a completed word's worth of [`Key`]s when a boundary is crossed,
//! which the heuristics then reinterpret under candidate layouts.
//!
//! Backspace edits the buffer in place. Caret-moving keys (arrows, Home/End,
//! mouse — anything that isn't text, a boundary, or a pure modifier) abandon the
//! current word, because once the caret jumps we can no longer reason about how
//! many backspaces a correction would need.

use crate::RawKey;

/// A buffered physical keystroke: enough to reinterpret it under any layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Key {
    /// Virtual-key code.
    pub vk: u32,
    /// Hardware scan code — the layout-independent physical key identity.
    pub scan: u32,
}

/// A word finished by a boundary keystroke.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Completed {
    /// The physical keys that make up the word (no boundary key).
    pub keys: Vec<Key>,
    /// Virtual-key code of the boundary that ended it (space, tab, or enter).
    pub boundary: u32,
}

/// What a virtual-key code means to the buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    /// A character-producing key — append to the current word.
    Char,
    /// Backspace — delete the last buffered key.
    Backspace,
    /// Space / Tab / Enter — end the current word.
    Boundary,
    /// Shift / Ctrl / Alt / Caps / Win — does not affect the word on its own.
    Modifier,
    /// Anything else (navigation, function keys, …) — abandon the word.
    Other,
}

// Virtual-key codes we care about (subset of the Win32 `VK_*` set).
const VK_BACK: u32 = 0x08;
const VK_TAB: u32 = 0x09;
const VK_RETURN: u32 = 0x0D;
const VK_SHIFT: u32 = 0x10;
const VK_CONTROL: u32 = 0x11;
const VK_MENU: u32 = 0x12;
const VK_CAPITAL: u32 = 0x14;
const VK_SPACE: u32 = 0x20;
const VK_LWIN: u32 = 0x5B;
const VK_RWIN: u32 = 0x5C;

fn classify(vk: u32) -> Class {
    match vk {
        VK_BACK => Class::Backspace,
        VK_SPACE | VK_TAB | VK_RETURN => Class::Boundary,
        // Modifiers and their L/R variants (0xA0–0xA5), plus Caps and Win keys.
        VK_SHIFT | VK_CONTROL | VK_MENU | VK_CAPITAL | VK_LWIN | VK_RWIN
        | 0xA0..=0xA5 => Class::Modifier,
        // Character keys: digits, letters, and the OEM keys that carry letters
        // in non-US layouts (e.g. `;'[]` map to Cyrillic in the Russian layout).
        0x30..=0x39 | 0x41..=0x5A | 0xBA..=0xC0 | 0xDB..=0xDF | 0xE2 => Class::Char,
        _ => Class::Other,
    }
}

/// Accumulates keys for the word currently being typed.
#[derive(Debug, Default)]
pub struct WordBuffer {
    keys: Vec<Key>,
}

impl WordBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of keys currently buffered.
    pub fn len(&self) -> usize {
        self.keys.len()
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    /// The keys of the word currently being typed (not yet ended by a boundary).
    pub fn current(&self) -> &[Key] {
        &self.keys
    }

    /// Discard the current word (e.g. on focus change or timeout).
    pub fn clear(&mut self) {
        self.keys.clear();
    }

    /// Take and clear the in-progress word — used by the manual hotkey to act on
    /// a word the user hasn't finished with a boundary yet.
    pub fn take_current(&mut self) -> Vec<Key> {
        std::mem::take(&mut self.keys)
    }

    /// Feed one **key-down** event. Returns the [`Completed`] word when this
    /// event ends a non-empty word (a boundary after some characters);
    /// otherwise `None`. Key-ups should be filtered out by the caller.
    pub fn push(&mut self, key: RawKey) -> Option<Completed> {
        debug_assert!(!key.up, "WordBuffer::push expects key-down events only");
        match classify(key.vk) {
            Class::Char => {
                self.keys.push(Key { vk: key.vk, scan: key.scan });
                None
            }
            Class::Backspace => {
                self.keys.pop();
                None
            }
            Class::Modifier => None,
            Class::Boundary => {
                if self.keys.is_empty() {
                    None
                } else {
                    Some(Completed { keys: self.take_current(), boundary: key.vk })
                }
            }
            Class::Other => {
                self.keys.clear();
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn down(vk: u32) -> RawKey {
        RawKey { vk, scan: vk, injected: false, ours: false, up: false, time: 0 }
    }

    fn word(vks: &[u32]) -> Vec<u32> {
        let mut b = WordBuffer::new();
        let mut out = None;
        for &vk in vks {
            if let Some(w) = b.push(down(vk)) {
                out = Some(w);
            }
        }
        out.map(|w| w.keys.iter().map(|k| k.vk).collect()).unwrap_or_default()
    }

    #[test]
    fn letters_then_space_yield_the_word() {
        // g h b d t n SPACE
        assert_eq!(word(&[0x47, 0x48, 0x42, 0x44, 0x54, 0x4E, 0x20]), vec![
            0x47, 0x48, 0x42, 0x44, 0x54, 0x4E
        ]);
    }

    #[test]
    fn backspace_edits_the_buffer() {
        let mut b = WordBuffer::new();
        assert!(b.push(down(0x41)).is_none()); // a
        assert!(b.push(down(0x42)).is_none()); // b
        assert!(b.push(down(VK_BACK)).is_none()); // erase b
        assert_eq!(b.len(), 1);
        let w = b.push(down(VK_SPACE)).unwrap();
        assert_eq!(w.keys.iter().map(|k| k.vk).collect::<Vec<_>>(), vec![0x41]);
        assert_eq!(w.boundary, VK_SPACE);
    }

    #[test]
    fn modifiers_do_not_break_or_extend_the_word() {
        // SHIFT a SHIFT b SPACE  →  "ab"
        assert_eq!(word(&[VK_SHIFT, 0x41, VK_SHIFT, 0x42, VK_SPACE]), vec![0x41, 0x42]);
    }

    #[test]
    fn navigation_key_abandons_the_word() {
        let mut b = WordBuffer::new();
        b.push(down(0x41));
        b.push(down(0x42));
        assert!(b.push(down(0x25)).is_none()); // VK_LEFT — abandons
        assert!(b.is_empty());
    }

    #[test]
    fn boundary_on_empty_buffer_yields_nothing() {
        let mut b = WordBuffer::new();
        assert!(b.push(down(VK_SPACE)).is_none());
    }
}
