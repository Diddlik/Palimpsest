//! Windows implementation of [`Reinterpreter`] via `ToUnicodeEx`.
//!
//! Maps buffered physical keys to the characters they would produce under an
//! arbitrary keyboard layout, without touching the live keyboard state:
//!
//! - We pass our own zeroed key-state array, so reinterpretation is independent
//!   of which modifiers are physically held and never mutates system state.
//! - Loaded `HKL`s are cached per KLID; `LoadKeyboardLayoutW` without
//!   `KLF_ACTIVATE` resolves a layout without switching the user to it.
//! - Dead keys are handled defensively: `ToUnicodeEx` keeps a thread-internal
//!   pending-dead-key state, so we flush it before each word to stop one word's
//!   trailing dead key from corrupting the next.

use std::collections::HashMap;
use std::sync::Mutex;

use windows::core::PCWSTR;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyboardLayout, LoadKeyboardLayoutW, MapVirtualKeyExW, ToUnicodeEx, VkKeyScanExW,
    ACTIVATE_KEYBOARD_LAYOUT_FLAGS, HKL, MAPVK_VK_TO_VSC, VK_SPACE,
};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

use crate::analyze::{Reinterpreter, Transliterator};
use crate::buffer::Key;

/// Caches loaded keyboard layouts so repeated reinterpretation is cheap. `HKL`
/// is stored as its raw `isize` to stay `Send`/`Sync` behind the mutex.
#[derive(Default)]
pub struct WinReinterpreter {
    cache: Mutex<HashMap<String, isize>>,
}

impl WinReinterpreter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Resolve (and cache) the `HKL` for a KLID hex string, loading it if needed.
    fn hkl_for(&self, klid: &str) -> Option<HKL> {
        if let Some(&raw) = self.cache.lock().unwrap().get(klid) {
            return Some(HKL(raw as _));
        }
        let wide: Vec<u16> = klid.encode_utf16().chain(std::iter::once(0)).collect();
        let hkl = unsafe {
            LoadKeyboardLayoutW(PCWSTR(wide.as_ptr()), ACTIVATE_KEYBOARD_LAYOUT_FLAGS(0))
        }
        .ok()?;
        self.cache.lock().unwrap().insert(klid.to_string(), hkl.0 as isize);
        Some(hkl)
    }
}

/// Clear any pending dead-key state on the calling thread for `hkl`, discarding
/// the result, so the next reinterpretation starts clean.
fn flush_dead_key(hkl: HKL) {
    let keystate = [0u8; 256];
    let mut buf = [0u16; 8];
    unsafe {
        ToUnicodeEx(VK_SPACE.0 as u32, 0, &keystate, &mut buf, 0, hkl);
    }
}

impl Reinterpreter for WinReinterpreter {
    fn reinterpret(&self, keys: &[Key], klid: &str) -> String {
        let Some(hkl) = self.hkl_for(klid) else { return String::new() };

        // Zeroed state = no modifiers held → unshifted (lowercase) interpretation,
        // which matches how the scorer lowercases everything anyway.
        let keystate = [0u8; 256];
        let mut out = String::new();

        flush_dead_key(hkl);
        for key in keys {
            // Buffered keys carry the real scan code from the hook. Synthetic
            // probes (layout fingerprinting) leave it zero, so derive it — some
            // layouts translate nothing without one.
            let scan = match key.scan {
                0 => unsafe { MapVirtualKeyExW(key.vk, MAPVK_VK_TO_VSC, hkl) },
                s => s,
            };
            let mut buf = [0u16; 8];
            let n = unsafe {
                ToUnicodeEx(key.vk, scan, &keystate, &mut buf, 0, hkl)
            };
            match n {
                // Normal: n code units produced.
                n if n > 0 => out.push_str(&String::from_utf16_lossy(&buf[..n as usize])),
                // Dead key: take its spacing form, then reset the pending state.
                -1 => {
                    out.push_str(&String::from_utf16_lossy(&buf[..1]));
                    flush_dead_key(hkl);
                }
                // 0 = no translation (e.g. a non-character key slipped through).
                _ => {}
            }
        }
        out
    }
}

impl WinReinterpreter {
    /// Map one character to the text the *same physical key* produces under `to`.
    /// We ask `from` which key (and shift state) yields `ch`, then ask `to` what
    /// that key yields. Returns `None` when the character isn't on `from`'s
    /// keyboard, needs an AltGr/Ctrl combination we can't reproduce, or yields
    /// nothing under `to` — the caller keeps the original character in that case.
    fn transliterate_char(&self, ch: char, from: HKL, to: HKL) -> Option<String> {
        let mut units = [0u16; 2];
        let encoded = ch.encode_utf16(&mut units);
        if encoded.len() != 1 {
            return None; // only single-unit (BMP) characters sit on one key
        }
        let scan_state = unsafe { VkKeyScanExW(units[0], from) };
        if scan_state == -1 {
            return None; // no key on `from` produces this character
        }
        let vk = (scan_state & 0xFF) as u32;
        let shift_state = (scan_state >> 8) & 0xFF;
        // Bit 0 = Shift; bits 1/2 = Ctrl/Alt. We reproduce Shift (to carry case)
        // but not Ctrl/Alt (AltGr) combinations, so we pass those through as-is.
        if shift_state & 0x6 != 0 {
            return None;
        }
        let mut keystate = [0u8; 256];
        if shift_state & 0x1 != 0 {
            keystate[0x10] = 0x80; // VK_SHIFT held high → the shifted (upper) form
        }
        let scan = unsafe { MapVirtualKeyExW(vk, MAPVK_VK_TO_VSC, to) };
        let mut buf = [0u16; 8];
        let n = unsafe { ToUnicodeEx(vk, scan, &keystate, &mut buf, 0, to) };
        match n {
            n if n > 0 => Some(String::from_utf16_lossy(&buf[..n as usize])),
            -1 => {
                let dead = String::from_utf16_lossy(&buf[..1]);
                flush_dead_key(to);
                Some(dead)
            }
            _ => None,
        }
    }
}

impl Transliterator for WinReinterpreter {
    fn transliterate(&self, text: &str, from: &str, to: &str) -> String {
        let (Some(from_hkl), Some(to_hkl)) = (self.hkl_for(from), self.hkl_for(to)) else {
            return String::new();
        };
        flush_dead_key(to_hkl);
        let mut out = String::new();
        for ch in text.chars() {
            match self.transliterate_char(ch, from_hkl, to_hkl) {
                Some(mapped) => out.push_str(&mapped),
                None => out.push(ch), // unmappable — keep the original character
            }
        }
        out
    }
}

/// Reverse lookup from a loaded `HKL` to the KLID that produced it, built lazily
/// from the installed layouts. `None` until first use.
static KLID_BY_HKL: Mutex<Option<HashMap<isize, String>>> = Mutex::new(None);

/// Load every installed layout and record which `HKL` each KLID resolves to.
///
/// `LoadKeyboardLayoutW` without `KLF_ACTIVATE` returns the *same* handle the OS
/// reports for that layout when it is active, so this is an exact reverse
/// lookup rather than a heuristic.
fn build_klid_map() -> HashMap<isize, String> {
    crate::layouts::installed()
        .into_iter()
        .filter_map(|layout| {
            let wide: Vec<u16> = layout.klid.encode_utf16().chain(std::iter::once(0)).collect();
            let hkl = unsafe {
                LoadKeyboardLayoutW(PCWSTR(wide.as_ptr()), ACTIVATE_KEYBOARD_LAYOUT_FLAGS(0))
            }
            .ok()?;
            Some((hkl.0 as isize, layout.klid))
        })
        .collect()
}

/// KLID of the keyboard layout active in the foreground window's thread — i.e.
/// the layout that produced what the user just typed.
///
/// An `HKL` is **not** a KLID. Its low word is the language id, but its high
/// word is a device handle, so masking to 16 bits destroys the identity of any
/// *substituted* layout — and two layouts of the same language collapse onto
/// each other. On a machine with Russian Phonetic (`a0000419`, reported as
/// `0xf0c00419`) alongside the base ЙЦУКЕН (`0x04190419`), masking turned both
/// into `"00000419"`, so Palimpsest reinterpreted phonetic input against the
/// positional keyboard — a different layout entirely, which is why conversions
/// silently produced nothing whenever the phonetic layout was active.
///
/// So match the handle against the layouts we can actually load, and fall back
/// to the bare language id only when nothing matches.
pub fn current_layout_klid() -> String {
    let raw = unsafe {
        let hwnd = GetForegroundWindow();
        let tid = GetWindowThreadProcessId(hwnd, None);
        GetKeyboardLayout(tid).0 as isize
    };

    let mut guard = KLID_BY_HKL.lock().unwrap();
    let map = guard.get_or_insert_with(build_klid_map);
    if let Some(klid) = map.get(&raw) {
        return klid.clone();
    }
    // Unknown handle — the user may have added a layout since the map was
    // built. Refresh once before giving up.
    *map = build_klid_map();
    if let Some(klid) = map.get(&raw) {
        return klid.clone();
    }
    // Still unknown: the language id is right for every standard layout, and is
    // exactly the behaviour Palimpsest had before substitutes were handled.
    format!("{:08x}", (raw as usize) & 0xFFFF)
}
