//! OS keyboard-layout switching.
//!
//! Decoupled from retyping on purpose: corrected text is injected as literal
//! Unicode ([`crate::inject`]), so it lands correctly regardless of the active
//! layout. This module exists only so that *continued* typing after a correction
//! uses the right layout — e.g. after fixing `ghbdtn ` → `привет `, the user is
//! left in Russian.
//!
//! We use the legacy HKL path (`LoadKeyboardLayoutW` + `WM_INPUTLANGCHANGEREQUEST`)
//! per the plan; migrate to TSF only if this proves flaky on Windows 11.

use windows::core::PCWSTR;
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    LoadKeyboardLayoutW, ACTIVATE_KEYBOARD_LAYOUT_FLAGS, KLF_ACTIVATE,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, PostMessageW, WM_INPUTLANGCHANGEREQUEST,
};

/// Switch the foreground window's input language to the layout identified by
/// `klid` (a Windows KLID hex string, e.g. `"00000419"` for Russian).
///
/// Loads and activates the layout, then asks the foreground window to adopt it.
/// `PostMessageW` is asynchronous, which is fine: the Unicode retype does not
/// depend on the switch having completed.
pub fn switch_to(klid: &str) -> windows::core::Result<()> {
    let wide: Vec<u16> = klid.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let hkl = LoadKeyboardLayoutW(
            PCWSTR(wide.as_ptr()),
            ACTIVATE_KEYBOARD_LAYOUT_FLAGS(KLF_ACTIVATE.0),
        )?;
        let hwnd = GetForegroundWindow();
        PostMessageW(
            hwnd,
            WM_INPUTLANGCHANGEREQUEST,
            WPARAM(0),
            LPARAM(hkl.0 as isize),
        )?;
    }
    Ok(())
}
