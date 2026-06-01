//! Enumerate the keyboard layouts the current user actually has installed, so
//! the settings UI offers a real choice instead of every conceivable layout.
//!
//! Read from the registry: the user's input list is `HKCU\Keyboard
//! Layout\Preload` (values point at KLIDs, possibly via `…\Substitutes`), and
//! friendly names come from `HKLM\…\Control\Keyboard Layouts\<KLID>\Layout
//! Text`. KLIDs are normalised to lowercase to match the config's convention;
//! `LoadKeyboardLayoutW` is case-insensitive, so this is purely cosmetic.

use serde::Serialize;
use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
use winreg::RegKey;

use crate::config::{Config, LayoutPair};
use crate::Lang;

/// One installed keyboard layout: its KLID and a human-readable name.
#[derive(Debug, Clone, Serialize)]
pub struct LayoutInfo {
    /// Windows KLID hex string, e.g. `"00000419"`.
    pub klid: String,
    /// Friendly name, e.g. `"Russian"` (falls back to the KLID if unknown).
    pub name: String,
}

const LAYOUTS_DB: &str = r"SYSTEM\CurrentControlSet\Control\Keyboard Layouts";

/// The current user's installed/preloaded keyboard layouts, resolved to friendly
/// names. Best-effort: returns an empty list if the registry can't be read.
pub fn installed() -> Vec<LayoutInfo> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let names_db = hklm.open_subkey(LAYOUTS_DB).ok();
    let substitutes = hkcu.open_subkey(r"Keyboard Layout\Substitutes").ok();

    let Ok(preload) = hkcu.open_subkey(r"Keyboard Layout\Preload") else {
        return Vec::new();
    };

    let mut seen = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    for value in preload.enum_values() {
        let Ok((name, _)) = value else { continue };
        let Ok(raw) = preload.get_value::<String, _>(&name) else { continue };

        // A Preload entry may be remapped by the Substitutes table
        // (e.g. "00000409" → "00010409" for Dvorak).
        let klid = substitutes
            .as_ref()
            .and_then(|s| s.get_value::<String, _>(&raw).ok())
            .unwrap_or(raw)
            .to_lowercase();

        if !seen.insert(klid.clone()) {
            continue;
        }
        let friendly = names_db
            .as_ref()
            .and_then(|db| db.open_subkey(&klid).ok())
            .and_then(|k| k.get_value::<String, _>("Layout Text").ok())
            .unwrap_or_else(|| klid.clone());
        out.push(LayoutInfo { klid, name: friendly });
    }
    out
}

/// Choose a sensible default layout pair from the `installed` layouts: prefer
/// two whose languages the scorer supports — English as the "from" side paired
/// with the first other — otherwise any two distinct installed layouts. Returns
/// `None` when fewer than two layouts are installed (no meaningful pair).
pub fn default_pair(installed: &[LayoutInfo]) -> Option<(String, String)> {
    fn pick(list: &[&LayoutInfo]) -> Option<(String, String)> {
        if list.len() < 2 {
            return None;
        }
        if let Some(en) = list.iter().find(|l| Lang::from_klid(&l.klid) == Some(Lang::En)) {
            if let Some(other) = list.iter().find(|l| l.klid != en.klid) {
                return Some((en.klid.clone(), other.klid.clone()));
            }
        }
        Some((list[0].klid.clone(), list[1].klid.clone()))
    }

    let supported: Vec<&LayoutInfo> =
        installed.iter().filter(|l| Lang::from_klid(&l.klid).is_some()).collect();
    pick(&supported).or_else(|| pick(&installed.iter().collect::<Vec<_>>()))
}

/// Reconcile a config's layout pairs with what's actually installed: drop any
/// pair that references a layout the user doesn't have, and if that empties the
/// list, seed a default from the installed layouts (so the UI never shows
/// "(not installed)" dummy pairs). Returns whether the config changed, so the
/// caller knows to persist it.
pub fn sanitize_pairs(config: &mut Config, installed: &[LayoutInfo]) -> bool {
    let is_installed = |klid: &str| installed.iter().any(|l| l.klid.eq_ignore_ascii_case(klid));
    let original = config.layout_pairs.clone();
    config.layout_pairs.retain(|p| is_installed(&p.from) && is_installed(&p.to));
    if config.layout_pairs.is_empty() {
        if let Some((from, to)) = default_pair(installed) {
            config.layout_pairs.push(LayoutPair::new(from, to));
        }
    }
    config.layout_pairs != original
}

#[cfg(test)]
mod tests {
    use super::*;

    fn li(klid: &str) -> LayoutInfo {
        LayoutInfo { klid: klid.into(), name: klid.into() }
    }

    #[test]
    fn default_pair_prefers_english_as_from() {
        let installed = vec![li("00000419"), li("00000409"), li("00000407")];
        let (from, to) = default_pair(&installed).unwrap();
        assert_eq!(from, "00000409"); // English chosen as the "from" side
        assert_ne!(to, "00000409");
    }

    #[test]
    fn default_pair_needs_two_layouts() {
        assert!(default_pair(&[li("00000409")]).is_none());
        assert!(default_pair(&[]).is_none());
    }

    #[test]
    fn sanitize_drops_uninstalled_and_seeds_default() {
        let installed = vec![li("00000409"), li("00000419")];
        // EN→French, but French isn't installed.
        let mut config = Config {
            layout_pairs: vec![LayoutPair::new("00000409", "0000040c")],
            ..Config::default()
        };
        assert!(sanitize_pairs(&mut config, &installed));
        assert_eq!(config.layout_pairs, vec![LayoutPair::new("00000409", "00000419")]);
    }

    #[test]
    fn sanitize_keeps_fully_installed_pairs() {
        let installed = vec![li("00000409"), li("00000419")];
        let mut config = Config::default(); // EN/RU, both installed here
        assert!(!sanitize_pairs(&mut config, &installed));
        assert_eq!(config.layout_pairs, vec![LayoutPair::new("00000409", "00000419")]);
    }
}
