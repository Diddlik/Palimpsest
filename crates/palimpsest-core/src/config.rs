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
    /// Whether this pair involves a *phonetic* Cyrillic layout, which needs the
    /// wordlist-primary decision path instead of the bigram margin (see
    /// [`crate::phonetic`]).
    ///
    /// `None` means "not classified yet": the app fingerprints the pair at
    /// startup and stores the answer. `Some` is authoritative, so a user who
    /// corrects a misdetection in Settings keeps their choice.
    #[serde(default)]
    pub phonetic: Option<bool>,
}

impl LayoutPair {
    /// Convenience constructor for a pair that has not been classified yet.
    pub fn new(from: impl Into<String>, to: impl Into<String>) -> Self {
        Self { from: from.into(), to: to.into(), phonetic: None }
    }

    /// Whether to use the phonetic decision path. An unclassified pair is
    /// treated as positional — the behaviour Palimpsest had before phonetic
    /// support, so a failed fingerprint degrades to the old heuristic.
    pub fn is_phonetic(&self) -> bool {
        self.phonetic.unwrap_or(false)
    }
}

/// Language of the user-facing surfaces — settings window and tray menu. This is
/// purely presentational: it never affects which languages the *scorer* knows
/// (that follows the layout pairs, see [`crate::scoring::Lang`]).
///
/// [`UiLang::System`] is the default and follows the Windows UI language, so a
/// fresh install speaks the user's language without them finding the setting.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UiLang {
    /// Follow the Windows UI language, falling back to English.
    #[default]
    System,
    /// English.
    En,
    /// Russian / русский.
    Ru,
    /// Ukrainian / українська.
    Uk,
    /// German / Deutsch.
    De,
}

impl UiLang {
    /// Short code used by the settings UI (`"system"`, `"en"`, `"ru"`, `"uk"`,
    /// `"de"`).
    pub fn code(self) -> &'static str {
        match self {
            UiLang::System => "system",
            UiLang::En => "en",
            UiLang::Ru => "ru",
            UiLang::Uk => "uk",
            UiLang::De => "de",
        }
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
    /// make automatically. Defaults to `VK_F9` (`0x78`) held with
    /// [`manual_hotkey_mods`] = Ctrl+Shift — i.e. Ctrl+Shift+F9. Function keys
    /// exist on every keyboard and, unlike Ctrl+Shift+Space, are not adjacent to
    /// normal typing. A value of `0` disables the manual hotkey entirely.
    ///
    /// Defaulted per-field to `0` (disabled) when missing from an on-disk config,
    /// so a partial/older file can never deserialize into an accidental bare
    /// typing-key hotkey.
    #[serde(default)]
    pub manual_hotkey: u32,
    /// Modifier keys that must be held together with [`manual_hotkey`] for it to
    /// fire — a bitmask of `MOD_*` (see [`MOD_ALT`] etc.). `0` means the bare
    /// key with no modifiers. Ignored when `manual_hotkey` is `0`. Defaults to
    /// Ctrl+Shift; field-defaults to `0` when absent from an on-disk config so an
    /// older bare-key binding stays bare.
    #[serde(default)]
    pub manual_hotkey_mods: u8,
    /// Virtual-key code of the "convert the current selection" hotkey. Unlike
    /// [`manual_hotkey`], which fixes the word being typed, this acts on whatever
    /// text the user has *selected* anywhere: it copies the selection,
    /// reinterprets it under the better-scoring layout, and types the result back
    /// in its place. Defaults to `VK_F8` (`0x77`) held with
    /// [`selection_hotkey_mods`] = Ctrl+Shift — i.e. Ctrl+Shift+F8, next to the
    /// manual hotkey on F9. `0` disables it. Field-defaults to `0` when absent
    /// from an on-disk config.
    #[serde(default)]
    pub selection_hotkey: u32,
    /// Modifier keys held together with [`selection_hotkey`] — a `MOD_*` bitmask
    /// (see [`manual_hotkey_mods`]). `0` means the bare key. Ignored when
    /// [`selection_hotkey`] is `0`. Defaults to Ctrl+Shift; field-defaults to `0`
    /// when absent from an on-disk config.
    #[serde(default)]
    pub selection_hotkey_mods: u8,
    /// Whether undoing a correction teaches Palimpsest that the word was right
    /// as typed, so it stops correcting it. This is the one path that writes
    /// typed text to disk (see [`crate::user_dict`]), so it is switchable —
    /// turning it off keeps the "nothing you type is ever stored" guarantee
    /// absolute at the cost of repeating the same undo.
    #[serde(default = "default_true")]
    pub learn_on_undo: bool,
    /// Language the settings window and tray menu are shown in. Defaults to
    /// [`UiLang::System`] — follow Windows — so the app is readable out of the
    /// box; the user can pin a language in Settings.
    #[serde(default)]
    pub ui_language: UiLang,
}

/// serde default for [`Config::learn_on_undo`] — a missing field means "on",
/// matching [`Config::default`] rather than `bool`'s `false`.
fn default_true() -> bool {
    true
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
            manual_hotkey: 0x78, // VK_F9 — with Ctrl+Shift below: Ctrl+Shift+F9
            manual_hotkey_mods: MOD_CONTROL | MOD_SHIFT,
            selection_hotkey: 0x77, // VK_F8 — with Ctrl+Shift below: Ctrl+Shift+F8
            selection_hotkey_mods: MOD_CONTROL | MOD_SHIFT,
            learn_on_undo: true,
            ui_language: UiLang::System,
        }
    }
}

/// Upgrade hotkeys that still carry a *superseded shipped default* to the
/// current one. Two generations need migrating:
///
/// - The original bare `VK_PAUSE` / `VK_SCROLL` bindings. Those keys are
///   physically absent on 75% keyboards and most laptops, so a low-level hook
///   never sees them and the hotkey silently did nothing.
/// - The Ctrl+Shift+Space / Ctrl+Shift+X chords that replaced them. They work,
///   but sit awkwardly close to normal typing; the current defaults are
///   Ctrl+Shift+F9 (manual) and Ctrl+Shift+F8 (selection).
///
/// Only untouched shipped defaults are migrated; any binding the user chose
/// themselves is left alone. Returns `true` if anything changed, so the caller
/// can persist the result.
pub fn migrate_legacy_hotkeys(config: &mut Config) -> bool {
    const MANUAL: u32 = 0x78; // VK_F9
    const SELECTION: u32 = 0x77; // VK_F8
    const CHORD: u8 = MOD_CONTROL | MOD_SHIFT;

    let mut changed = false;
    // Manual: bare Pause, or the superseded Ctrl+Shift+Space → Ctrl+Shift+F9.
    let manual_legacy = (config.manual_hotkey == 0x13 && config.manual_hotkey_mods == 0)
        || (config.manual_hotkey == 0x20 && config.manual_hotkey_mods == CHORD);
    if manual_legacy {
        config.manual_hotkey = MANUAL;
        config.manual_hotkey_mods = CHORD;
        changed = true;
    }
    // Selection: bare Scroll Lock, or the superseded Ctrl+Shift+X → Ctrl+Shift+F8.
    let selection_legacy = (config.selection_hotkey == 0x91 && config.selection_hotkey_mods == 0)
        || (config.selection_hotkey == 0x58 && config.selection_hotkey_mods == CHORD);
    if selection_legacy {
        config.selection_hotkey = SELECTION;
        config.selection_hotkey_mods = CHORD;
        changed = true;
    }
    changed
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
    fn default_hotkeys_are_function_key_chords() {
        // Function keys exist on every keyboard (unlike the original bare
        // Pause/Scroll Lock) and sit away from normal typing (unlike the
        // Ctrl+Shift+Space/X chords that replaced them).
        let c = Config::default();
        assert_eq!(c.manual_hotkey, 0x78); // F9
        assert_eq!(c.manual_hotkey_mods, MOD_CONTROL | MOD_SHIFT);
        assert_eq!(c.selection_hotkey, 0x77); // F8
        assert_eq!(c.selection_hotkey_mods, MOD_CONTROL | MOD_SHIFT);
    }

    #[test]
    fn migrate_upgrades_only_untouched_legacy_defaults() {
        // A config still on the bare Pause/Scroll Lock defaults gets upgraded.
        let mut legacy = Config {
            manual_hotkey: 0x13,
            manual_hotkey_mods: 0,
            selection_hotkey: 0x91,
            selection_hotkey_mods: 0,
            ..Config::default()
        };
        assert!(migrate_legacy_hotkeys(&mut legacy));
        assert_eq!(legacy.manual_hotkey, 0x78);
        assert_eq!(legacy.manual_hotkey_mods, MOD_CONTROL | MOD_SHIFT);
        assert_eq!(legacy.selection_hotkey, 0x77);
        assert_eq!(legacy.selection_hotkey_mods, MOD_CONTROL | MOD_SHIFT);

        // So does the intermediate Ctrl+Shift+Space / Ctrl+Shift+X generation.
        let mut previous = Config {
            manual_hotkey: 0x20,
            manual_hotkey_mods: MOD_CONTROL | MOD_SHIFT,
            selection_hotkey: 0x58,
            selection_hotkey_mods: MOD_CONTROL | MOD_SHIFT,
            ..Config::default()
        };
        assert!(migrate_legacy_hotkeys(&mut previous));
        assert_eq!(previous.manual_hotkey, 0x78);
        assert_eq!(previous.selection_hotkey, 0x77);

        // A fresh default (already chords) is left untouched.
        let mut fresh = Config::default();
        assert!(!migrate_legacy_hotkeys(&mut fresh));
        assert_eq!(fresh, Config::default());

        // A user who deliberately put Pause behind a modifier keeps their binding
        // (it isn't the *bare* legacy default, so it's not migrated).
        let mut custom =
            Config { manual_hotkey: 0x13, manual_hotkey_mods: MOD_ALT, ..Config::default() };
        let before = custom.clone();
        assert!(!migrate_legacy_hotkeys(&mut custom));
        assert_eq!(custom, before);
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
        // ...and one written before UI languages follows the system.
        assert_eq!(c.ui_language, UiLang::System);
    }

    #[test]
    fn ui_language_round_trips_by_code() {
        for lang in [UiLang::System, UiLang::En, UiLang::Ru, UiLang::Uk, UiLang::De] {
            let json = serde_json::to_string(&lang).unwrap();
            assert_eq!(json, format!("\"{}\"", lang.code()));
            assert_eq!(serde_json::from_str::<UiLang>(&json).unwrap(), lang);
        }
    }
}
