//! Runtime configuration.
//!
//! `Config` is read constantly by the engine and written rarely by the UI, so
//! the runtime wraps it in an [`arc_swap::ArcSwap`] for lock-free reads and
//! atomic pointer swaps on write — no `Mutex` on the hot path. The struct is
//! `serde`-serializable so Phase 5 can persist it to disk and round-trip it
//! through Tauri IPC.

use serde::{Deserialize, Serialize};

/// A directional keyboard-layout pair the engine considers when reinterpreting
/// a word. Layouts are identified by their Windows KLID hex string (the value
/// `LoadKeyboardLayoutW` takes), e.g. `"00000409"` for English (US) and
/// `"00000419"` for Russian.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayoutPair {
    /// KLID the text was *probably* typed in (the wrong one).
    pub from: String,
    /// KLID the text was *meant* for (the correction target).
    pub to: String,
}

impl LayoutPair {
    /// Convenience constructor.
    pub fn new(from: impl Into<String>, to: impl Into<String>) -> Self {
        Self { from: from.into(), to: to.into() }
    }
}

/// Modifier bit flags for [`Config::manual_hotkey_mods`]. The values match the
/// Win32 `MOD_*` constants, so the bitmask passes straight to `RegisterHotKey`
/// when the UI probes whether a chord is already taken.
pub const MOD_ALT: u8 = 0x1;
/// Ctrl modifier bit (see [`MOD_ALT`]).
pub const MOD_CONTROL: u8 = 0x2;
/// Shift modifier bit (see [`MOD_ALT`]).
pub const MOD_SHIFT: u8 = 0x4;
/// Win modifier bit (see [`MOD_ALT`]).
pub const MOD_WIN: u8 = 0x8;

/// User-facing configuration. Cheap to clone (small `Vec`s); the runtime stores
/// it behind `ArcSwap` and hands out `Arc<Config>` snapshots to readers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Master switch. When `false` the engine performs no corrections even if
    /// the hook is armed and forwarding events.
    pub enabled: bool,
    /// Scoring margin (Phase 2): how much better the alternate layout's
    /// interpretation must score before we act. Higher = more conservative,
    /// fewer false positives. `0.0..=1.0`.
    pub sensitivity: f32,
    /// Layout pairs to consider, in priority order. Defaults to EN(US)↔RU.
    pub layout_pairs: Vec<LayoutPair>,
    /// Process executable names (case-insensitive, no path) where Palimpsest
    /// stays silent — terminals, IDEs, games, etc. Wired up in Phase 4.
    pub ignore_processes: Vec<String>,
    /// Virtual-key code of the manual "re-interpret the current/last word now"
    /// hotkey, used to force a correction the heuristics weren't sure enough to
    /// make automatically. Defaults to `VK_PAUSE` (Pause/Break, `0x13`). A value
    /// of `0` disables the manual hotkey entirely.
    pub manual_hotkey: u32,
    /// Modifier keys that must be held together with [`manual_hotkey`] for it to
    /// fire — a bitmask of `MOD_*` (see [`MOD_ALT`] etc.). `0` means the bare
    /// key with no modifiers. Ignored when `manual_hotkey` is `0`.
    pub manual_hotkey_mods: u8,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            sensitivity: 0.5,
            layout_pairs: vec![LayoutPair::new("00000409", "00000419")],
            ignore_processes: vec![
                "cmd.exe".into(),
                "WindowsTerminal.exe".into(),
                "Code.exe".into(),
            ],
            manual_hotkey: 0x13, // VK_PAUSE
            manual_hotkey_mods: 0,
        }
    }
}

/// Whether `exe` (a process executable name like `"code.exe"`, no path) appears
/// on the ignore-list. Case-insensitive; pure so it can be unit-tested without
/// touching the OS.
pub fn is_ignored(exe: &str, ignore: &[String]) -> bool {
    ignore.iter().any(|p| p.eq_ignore_ascii_case(exe))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_has_en_ru_pair() {
        let c = Config::default();
        assert!(c.enabled);
        assert_eq!(c.layout_pairs, vec![LayoutPair::new("00000409", "00000419")]);
    }

    #[test]
    fn json_round_trips() {
        let c = Config::default();
        let json = serde_json::to_string(&c).unwrap();
        let back: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(c, back);
    }

    #[test]
    fn ignore_match_is_case_insensitive() {
        let c = Config::default();
        assert!(is_ignored("code.exe", &c.ignore_processes));
        assert!(is_ignored("CMD.EXE", &c.ignore_processes));
        assert!(!is_ignored("notepad.exe", &c.ignore_processes));
        assert!(!is_ignored("code", &c.ignore_processes)); // needs the .exe
    }

    #[test]
    fn missing_fields_fall_back_to_defaults() {
        // `#[serde(default)]` lets a partial config (e.g. an older on-disk file)
        // deserialize without error, filling gaps from `Default`.
        let c: Config = serde_json::from_str(r#"{ "enabled": false }"#).unwrap();
        assert!(!c.enabled);
        assert_eq!(c.sensitivity, Config::default().sensitivity);
        assert_eq!(c.layout_pairs, Config::default().layout_pairs);
        // A config written before chord support has no modifiers (bare key).
        assert_eq!(c.manual_hotkey_mods, 0);
    }
}
