//! Synthetic input via `SendInput`.
//!
//! Two design choices that carry through the whole project:
//!
//! 1. **Unicode injection** (`KEYEVENTF_UNICODE`) types the literal characters
//!    regardless of the active keyboard layout. This decouples *what gets
//!    typed* from *which layout is active*, which is far more reliable than
//!    switching the layout and then sending virtual-key codes.
//! 2. **Signature tagging** stamps every event's `dwExtraInfo` with
//!    [`PALIMPSEST_SIGNATURE`] so our own hook recognises and ignores it,
//!    preventing a correction feedback loop.

use std::mem::size_of;

use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE,
    VIRTUAL_KEY, VK_BACK, VK_CONTROL,
};

use crate::PALIMPSEST_SIGNATURE;

/// Build one keyboard `INPUT`, tagged with our signature.
///
/// `scan` is the UTF-16 code unit for Unicode events, or the hardware scan code
/// when `unicode` is `false`. `vk` is the virtual-key code (0 for Unicode).
fn make_input(vk: u16, scan: u16, key_up: bool, unicode: bool) -> INPUT {
    let mut flags = if unicode { KEYEVENTF_UNICODE } else { Default::default() };
    if key_up {
        flags |= KEYEVENTF_KEYUP;
    }
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(vk),
                wScan: scan,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: PALIMPSEST_SIGNATURE,
            },
        },
    }
}

/// Send a batch of input events in one `SendInput` call (atomic w.r.t. other
/// threads' input). Returns an error if the OS accepted fewer than requested.
fn send(inputs: &[INPUT]) -> windows::core::Result<()> {
    if inputs.is_empty() {
        return Ok(());
    }
    let sent = unsafe { SendInput(inputs, size_of::<INPUT>() as i32) };
    if sent as usize != inputs.len() {
        return Err(windows::core::Error::from_win32());
    }
    Ok(())
}

/// Append the down/up Unicode events for `text` to `inputs`.
fn push_unicode(inputs: &mut Vec<INPUT>, text: &str) {
    for unit in text.encode_utf16() {
        inputs.push(make_input(0, unit, false, true)); // key down
        inputs.push(make_input(0, unit, true, true)); // key up
    }
}

/// Append `count` Backspace down/up events to `inputs`.
fn push_backspaces(inputs: &mut Vec<INPUT>, count: usize) {
    for _ in 0..count {
        inputs.push(make_input(VK_BACK.0, 0, false, false)); // key down
        inputs.push(make_input(VK_BACK.0, 0, true, false)); // key up
    }
}

/// Type `text` as literal Unicode, independent of the active layout.
pub fn type_unicode(text: &str) -> windows::core::Result<()> {
    let mut inputs = Vec::with_capacity(text.len() * 2);
    push_unicode(&mut inputs, text);
    send(&inputs)
}

/// Erase `backspaces` characters and type `text` in their place, as **one**
/// `SendInput` batch. Windows guarantees a single call's events are injected
/// serially with no other input interleaved, so the whole edit is atomic with
/// respect to the user's real typing — this is the race guard for corrections.
pub fn replace(backspaces: usize, text: &str) -> windows::core::Result<()> {
    let mut inputs = Vec::with_capacity(backspaces * 2 + text.len() * 2);
    push_backspaces(&mut inputs, backspaces);
    push_unicode(&mut inputs, text);
    send(&inputs)
}

/// Press Backspace `count` times — the "scrape away the old text" half of the
/// palimpsest effect.
pub fn press_backspace(count: usize) -> windows::core::Result<()> {
    let mut inputs = Vec::with_capacity(count * 2);
    push_backspaces(&mut inputs, count);
    send(&inputs)
}

/// Synthesize key-up events for every modifier currently held in `mods` (a
/// [`crate::config`] `MOD_*` bitmask), clearing the OS modifier state before a
/// hotkey-triggered injection. A chorded hotkey (e.g. Ctrl+Shift+X) is still
/// physically held when the engine reacts; without this, our synthetic `Ctrl+C`
/// copy degrades to `Ctrl+Shift+C` and replacement typing/backspaces fire as
/// editing shortcuts (`Ctrl+Backspace` deletes a word), which manifests as the
/// selection being wiped instead of converted.
///
/// Both physical side variants (L/R) plus the generic VK are released so the
/// state is cleared regardless of which the user pressed. The events carry our
/// [`PALIMPSEST_SIGNATURE`], so the hook tags them `ours` and neither reacts to
/// them nor disturbs its own held-modifier tracking. We do **not** re-press the
/// modifiers afterward: the user releases them naturally, and re-pressing risks a
/// stuck key. Best-effort — failure to release is non-fatal.
pub fn release_modifiers(mods: u8) {
    use crate::config::{MOD_ALT, MOD_CONTROL, MOD_SHIFT, MOD_WIN};
    let mut vks: Vec<u16> = Vec::new();
    if mods & MOD_CONTROL != 0 {
        vks.extend_from_slice(&[0x11, 0xA2, 0xA3]); // VK_CONTROL / L / R
    }
    if mods & MOD_SHIFT != 0 {
        vks.extend_from_slice(&[0x10, 0xA0, 0xA1]); // VK_SHIFT / L / R
    }
    if mods & MOD_ALT != 0 {
        vks.extend_from_slice(&[0x12, 0xA4, 0xA5]); // VK_MENU / L / R
    }
    if mods & MOD_WIN != 0 {
        vks.extend_from_slice(&[0x5B, 0x5C]); // L / R Win
    }
    let inputs: Vec<INPUT> = vks.iter().map(|&vk| make_input(vk, 0, true, false)).collect();
    let _ = send(&inputs);
}

/// `'C'` virtual-key code, for the synthetic copy chord.
const VK_C: u16 = 0x43;

/// Synthesize `Ctrl+C` to copy the foreground window's selection — the first
/// step of the "convert selection" hotkey. Sent by virtual-key (not Unicode) so
/// the app sees a real copy shortcut; tagged with our signature so the hook
/// ignores the echo.
pub fn copy_selection() -> windows::core::Result<()> {
    let ctrl = VK_CONTROL.0;
    let inputs = [
        make_input(ctrl, 0, false, false), // Ctrl down
        make_input(VK_C, 0, false, false), // C down
        make_input(VK_C, 0, true, false),  // C up
        make_input(ctrl, 0, true, false),  // Ctrl up
    ];
    send(&inputs)
}
