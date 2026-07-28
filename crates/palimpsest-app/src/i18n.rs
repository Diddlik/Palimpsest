//! Localisation of the surfaces Rust owns: the tray menu, the tray tooltip, and
//! the hotkey rejection messages returned across IPC.
//!
//! The settings window has its own table (`ui/i18n.js`) because it re-renders
//! itself; this module covers only what the webview cannot reach. Both are keyed
//! by the same codes (`en`, `ru`, `uk`, `de`) and both resolve
//! [`UiLang::System`] through [`resolve`], so the two never disagree.
//!
//! Adding a language means: a variant in [`UiLang`], a [`Strings`] block here, a
//! table in `ui/i18n.js`, and an `<option>` in `ui/index.html`.

use palimpsest_core::UiLang;

/// Every user-facing string that lives outside the webview.
pub struct Strings {
    /// Tray menu: the master-switch check item.
    pub tray_enabled: &'static str,
    /// Tray menu: open the settings window.
    pub tray_settings: &'static str,
    /// Tray menu: reveal the diagnostics folder.
    pub tray_logs: &'static str,
    /// Tray menu: exit the process for real.
    pub tray_quit: &'static str,
    /// Tray tooltip while corrections are running.
    pub tooltip_on: &'static str,
    /// Tray tooltip while the master switch is off.
    pub tooltip_paused: &'static str,
    /// Hotkey rejected: the key types a character on its own.
    pub hotkey_typing: &'static str,
    /// Hotkey rejected: a lone modifier key.
    pub hotkey_modifier: &'static str,
    /// Hotkey rejected: a `Win+…` chord Windows reserves for the shell.
    pub hotkey_busy_win: &'static str,
    /// Hotkey rejected: already claimed by another application.
    pub hotkey_busy: &'static str,
    /// Diary unlock: the password did not decrypt it.
    pub diary_wrong_password: &'static str,
    /// Diary password: an empty password was submitted.
    pub diary_password_empty: &'static str,
    /// Diary: an action needs the diary unlocked first.
    pub diary_locked: &'static str,
    /// Diary: its lock was momentarily unavailable (a poisoned mutex).
    pub diary_busy: &'static str,
    /// Elevated-autostart: the user dismissed the UAC prompt.
    pub elevated_cancelled: &'static str,
}

const EN: Strings = Strings {
    tray_enabled: "Enabled",
    tray_settings: "Settings…",
    tray_logs: "Open logs folder",
    tray_quit: "Quit Palimpsest",
    tooltip_on: "Palimpsest — on",
    tooltip_paused: "Palimpsest — paused",
    hotkey_typing: "That key types a character — add a modifier (e.g. Ctrl+Shift) or pick a key like Pause, F8, or Insert.",
    hotkey_modifier: "A modifier key can't be used on its own.",
    hotkey_busy_win: "Windows keeps most Win+… shortcuts for itself. Try Win+F8 or Win+Insert, or add Shift — Win+Shift+PageUp is free.",
    hotkey_busy: "That combination is already in use by another app.",
    diary_wrong_password: "Wrong password.",
    diary_password_empty: "Enter a password.",
    diary_locked: "Unlock the diary first.",
    diary_busy: "The diary is busy — try again.",
    elevated_cancelled: "Cancelled — nothing changed.",
};

const RU: Strings = Strings {
    tray_enabled: "Включено",
    tray_settings: "Настройки…",
    tray_logs: "Открыть папку логов",
    tray_quit: "Выйти из Palimpsest",
    tooltip_on: "Palimpsest — включён",
    tooltip_paused: "Palimpsest — на паузе",
    hotkey_typing: "Эта клавиша вводит символ — добавьте модификатор (например, Ctrl+Shift) или выберите клавишу вроде Pause, F8 или Insert.",
    hotkey_modifier: "Клавишу-модификатор нельзя использовать саму по себе.",
    hotkey_busy_win: "Windows оставляет большинство сочетаний Win+… себе. Попробуйте Win+F8 или Win+Insert либо добавьте Shift — Win+Shift+PageUp свободно.",
    hotkey_busy: "Это сочетание уже занято другим приложением.",
    diary_wrong_password: "Неверный пароль.",
    diary_password_empty: "Введите пароль.",
    diary_locked: "Сначала разблокируйте дневник.",
    diary_busy: "Дневник занят — попробуйте ещё раз.",
    elevated_cancelled: "Отменено — ничего не изменилось.",
};

const UK: Strings = Strings {
    tray_enabled: "Увімкнено",
    tray_settings: "Налаштування…",
    tray_logs: "Відкрити теку журналів",
    tray_quit: "Вийти з Palimpsest",
    tooltip_on: "Palimpsest — увімкнено",
    tooltip_paused: "Palimpsest — призупинено",
    hotkey_typing: "Ця клавіша вводить символ — додайте модифікатор (наприклад, Ctrl+Shift) або оберіть клавішу на кшталт Pause, F8 чи Insert.",
    hotkey_modifier: "Клавішу-модифікатор не можна використовувати саму по собі.",
    hotkey_busy_win: "Windows залишає більшість сполучень Win+… собі. Спробуйте Win+F8 або Win+Insert чи додайте Shift — Win+Shift+PageUp вільне.",
    hotkey_busy: "Це сполучення вже зайняте іншим застосунком.",
    diary_wrong_password: "Невірний пароль.",
    diary_password_empty: "Уведіть пароль.",
    diary_locked: "Спочатку розблокуйте щоденник.",
    diary_busy: "Щоденник зайнятий — спробуйте ще раз.",
    elevated_cancelled: "Скасовано — нічого не змінилося.",
};

const DE: Strings = Strings {
    tray_enabled: "Aktiviert",
    tray_settings: "Einstellungen…",
    tray_logs: "Log-Ordner öffnen",
    tray_quit: "Palimpsest beenden",
    tooltip_on: "Palimpsest — an",
    tooltip_paused: "Palimpsest — pausiert",
    hotkey_typing: "Diese Taste schreibt ein Zeichen — bitte einen Modifikator ergänzen (z. B. Strg+Umschalt) oder eine Taste wie Pause, F8 oder Einfg wählen.",
    hotkey_modifier: "Eine Modifikatortaste allein ist nicht verwendbar.",
    hotkey_busy_win: "Windows behält die meisten Win+…-Kürzel für sich. Win+F8 oder Win+Einfg versuchen, oder Umschalt ergänzen — Win+Umschalt+BildAuf ist frei.",
    hotkey_busy: "Diese Kombination wird bereits von einer anderen App verwendet.",
    diary_wrong_password: "Falsches Passwort.",
    diary_password_empty: "Bitte ein Passwort eingeben.",
    diary_locked: "Zuerst das Tagebuch entsperren.",
    diary_busy: "Das Tagebuch ist beschäftigt — bitte erneut versuchen.",
    elevated_cancelled: "Abgebrochen — nichts geändert.",
};

/// The string table for `lang`, resolving [`UiLang::System`] first.
pub fn strings(lang: UiLang) -> &'static Strings {
    match resolve(lang) {
        UiLang::Ru => &RU,
        UiLang::Uk => &UK,
        UiLang::De => &DE,
        _ => &EN,
    }
}

/// Turn [`UiLang::System`] into a concrete language by asking Windows for the
/// user's UI language; anything we don't translate resolves to English. Any
/// other value is returned unchanged — an explicit choice always wins.
pub fn resolve(lang: UiLang) -> UiLang {
    if lang != UiLang::System {
        return lang;
    }
    system_language()
}

/// The Windows UI language, mapped onto the languages we ship.
///
/// Only the *primary* language ID matters (the low 10 bits of the LANGID), so
/// every regional variant — de-AT, ru-BY, uk-UA — lands on the same table.
#[cfg(windows)]
fn system_language() -> UiLang {
    const LANG_GERMAN: u16 = 0x07;
    const LANG_RUSSIAN: u16 = 0x19;
    const LANG_UKRAINIAN: u16 = 0x22;

    let primary = unsafe { windows::Win32::Globalization::GetUserDefaultUILanguage() } & 0x3ff;
    match primary {
        LANG_GERMAN => UiLang::De,
        LANG_RUSSIAN => UiLang::Ru,
        LANG_UKRAINIAN => UiLang::Uk,
        _ => UiLang::En,
    }
}

/// Non-Windows builds (tests, `cargo check` on CI hosts) have no UI language to
/// read; English keeps the table lookups total.
#[cfg(not(windows))]
fn system_language() -> UiLang {
    UiLang::En
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_language_wins_over_system() {
        assert_eq!(resolve(UiLang::De), UiLang::De);
        assert_eq!(resolve(UiLang::Uk), UiLang::Uk);
        // System resolves to something concrete, never back to System.
        assert_ne!(resolve(UiLang::System), UiLang::System);
    }

    #[test]
    fn every_language_has_a_table() {
        for lang in [UiLang::System, UiLang::En, UiLang::Ru, UiLang::Uk, UiLang::De] {
            assert!(!strings(lang).tray_quit.is_empty());
        }
    }
}
