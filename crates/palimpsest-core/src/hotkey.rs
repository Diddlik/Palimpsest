//! Validation for the user-chosen manual hotkey.
//!
//! The settings UI lets the user *press* a key (or a combination) to bind it.
//! A bare key (no modifiers) is rejected if it is a character/flow key, since it
//! would fire while typing; with modifiers held (e.g. Ctrl+Shift+K) any main key
//! is allowed, because the modifiers disambiguate it from normal typing. The
//! main key may never itself be a bare modifier. We also detect whether the
//! combination is already claimed as a global hotkey by another application, via
//! a transient `RegisterHotKey` probe.

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS,
};

/// Why a virtual-key code can't serve as the manual hotkey.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unusable {
    /// A character-producing or text-flow key (letter, digit, space, Tab,
    /// Enter, Backspace, OEM punctuation) — it would trigger during normal
    /// typing.
    Typing,
    /// A bare modifier (Shift/Ctrl/Alt/Win/Caps) — meaningless on its own.
    Modifier,
    /// Already registered as a global hotkey by another application.
    Busy,
}

/// A character-producing or text-flow key we must not hijack.
fn is_typing_key(vk: u32) -> bool {
    matches!(vk,
        0x08 | 0x09 | 0x0D | 0x20            // Backspace, Tab, Enter, Space
        | 0x30..=0x39                        // digits 0-9
        | 0x41..=0x5A                        // letters A-Z
        | 0x60..=0x6F                        // numpad digits & operators
        | 0xBA..=0xC0 | 0xDB..=0xDF | 0xE2   // OEM punctuation / layout letter keys
    )
}

/// A bare modifier key (incl. L/R variants).
fn is_modifier(vk: u32) -> bool {
    matches!(vk, 0x10..=0x12 | 0x14 | 0x5B | 0x5C | 0xA0..=0xA5)
}

/// Arbitrary id for the availability probe ("PL").
const PROBE_ID: i32 = 0x504C;

/// Whether `vk` + `mods` is already a global hotkey owned by another app. The
/// `mods` bitmask uses the `Config::MOD_*` values, which equal the Win32 `MOD_*`
/// flags. Tested by registering the combination momentarily and releasing it.
fn is_busy(vk: u32, mods: u8) -> bool {
    unsafe {
        match RegisterHotKey(HWND::default(), PROBE_ID, HOT_KEY_MODIFIERS(mods as u32), vk) {
            Ok(()) => {
                let _ = UnregisterHotKey(HWND::default(), PROBE_ID);
                false
            }
            Err(_) => true,
        }
    }
}

/// Validate a candidate manual hotkey: `vk` is the main key, `mods` the held
/// modifiers (`Config::MOD_*` bitmask, `0` for none).
pub fn validate(vk: u32, mods: u8) -> Result<(), Unusable> {
    if is_modifier(vk) {
        return Err(Unusable::Modifier);
    }
    // A bare key would fire while typing; with modifiers it's disambiguated.
    if mods == 0 && is_typing_key(vk) {
        return Err(Unusable::Typing);
    }
    if is_busy(vk, mods) {
        return Err(Unusable::Busy);
    }
    Ok(())
}
