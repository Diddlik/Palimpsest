//! Correction executor — performs the Palimpsest effect for a [`Decision`].
//!
//! The whole text edit (backspaces + retype) goes out in a single atomic
//! `SendInput` batch ([`inject::replace`]) so the user's real keystrokes can
//! never interleave mid-correction; the injected events carry our signature, so
//! the hook tags them `ours` and the engine ignores their echo. The OS layout
//! switch follows, leaving the user in the corrected layout for continued
//! typing. Because the edit is one atomic batch and self-injection is filtered
//! by signature, no separate `is_correcting` guard is needed.

use crate::analyze::Decision;
use crate::{inject, layout};

/// Apply a [`Decision`] to the foreground window.
///
/// `boundary` is the character that ended the word (e.g. `' '` for a space) and
/// should be re-typed after the correction, or `None` when correcting an
/// in-progress word that has no boundary yet (the manual hotkey case). For a
/// [`Decision::Keep`] this is a no-op.
pub fn apply(decision: &Decision, boundary: Option<char>) -> windows::core::Result<()> {
    let Decision::Correct { backspaces, text, target_layout } = decision else {
        return Ok(());
    };

    // Erase the word (plus its boundary char, if any) and type the correction
    // followed by that same boundary char — one atomic edit.
    let total_backspaces = backspaces + boundary.is_some() as usize;
    let mut retyped = text.clone();
    if let Some(c) = boundary {
        retyped.push(c);
    }
    inject::replace(total_backspaces, &retyped)?;

    // Then leave the OS in the target layout so continued typing is correct.
    layout::switch_to(target_layout)
}

/// Revert a previously applied correction: erase the corrected text (plus its
/// boundary char, if one was re-emitted) and retype the `original` the user
/// actually typed, then restore the layout that was active before the
/// correction. Like [`apply`], the text edit is one atomic `SendInput` batch.
///
/// `corrected_len` is the character count of the text that was injected (so we
/// know how many backspaces to send); `boundary` matches the value passed to the
/// original [`apply`] call.
pub fn undo(
    original: &str,
    corrected_len: usize,
    from_layout: &str,
    boundary: Option<char>,
) -> windows::core::Result<()> {
    let total_backspaces = corrected_len + boundary.is_some() as usize;
    let mut retyped = original.to_string();
    if let Some(c) = boundary {
        retyped.push(c);
    }
    inject::replace(total_backspaces, &retyped)?;
    layout::switch_to(from_layout)
}
