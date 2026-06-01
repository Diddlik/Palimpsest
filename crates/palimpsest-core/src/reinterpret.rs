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
    GetKeyboardLayout, LoadKeyboardLayoutW, ToUnicodeEx, ACTIVATE_KEYBOARD_LAYOUT_FLAGS, HKL,
    VK_SPACE,
};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

use crate::analyze::Reinterpreter;
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
            let mut buf = [0u16; 8];
            let n = unsafe {
                ToUnicodeEx(key.vk, key.scan, &keystate, &mut buf, 0, hkl)
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

/// KLID (as `"0000xxxx"`) of the keyboard layout active in the foreground
/// window's thread — i.e. the layout that produced what the user just typed.
pub fn current_layout_klid() -> String {
    unsafe {
        let hwnd = GetForegroundWindow();
        let tid = GetWindowThreadProcessId(hwnd, None);
        let hkl = GetKeyboardLayout(tid);
        let langid = (hkl.0 as usize) & 0xFFFF;
        format!("{langid:08x}")
    }
}
