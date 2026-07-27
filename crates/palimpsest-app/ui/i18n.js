// Settings-window translations. Loaded as a plain script before `main.js`, so
// `t`, `setLocale` and `applyI18n` are globals — no bundler, matching the rest of
// the UI.
//
// The tray menu and the hotkey errors are translated in Rust (`src/i18n.rs`)
// because they exist outside this webview; both sides use the same codes and
// both let the backend resolve "system", so they can never disagree.
//
// Adding a language means: a table here, a `UiLang` variant + `Strings` block in
// `src/i18n.rs`, and an `<option>` in `index.html`.
//
// Placeholders are `{name}` and are filled by the second argument of `t()`.

const I18N = {
  en: {
    tagline: "Wrong-layout typing, quietly fixed.",
    masterSwitch: "Master switch",
    on: "On",
    paused: "Paused",

    sensitivity: "Sensitivity",
    sensitivityHint:
      "How sure Palimpsest must be before it corrects. Lower is more cautious (fewer false positives); higher is more eager.",
    cautious: "Cautious",
    eager: "Eager",

    layoutPairs: "Layout pairs",
    addPair: "+ Add pair",
    layoutPairsHint:
      "Pairs are considered in both directions. Palimpsest reinterprets a word across these layouts and switches to the better one.",
    phonetic: "phonetic",
    phoneticHint:
      "Each Cyrillic letter sits on its Latin sound-alike key, so wrong-layout text reads like “privet” rather than “ghbdtn”. Detected automatically.",
    removePair: "Remove pair",
    notInstalled: "{name} (not installed)",
    unknownLayout: "Unknown ({klid})",

    ignoreApps: "Ignore in these apps",
    ignoreAppsHint:
      "Palimpsest stays completely silent in these — terminals, IDEs, games. Pick one from your running apps, or type executable names (one per line, e.g. <code>cmd.exe</code>).",
    add: "Add",
    refreshList: "Refresh list",

    language: "Language",
    languageHint:
      "Language of this window and the tray menu. It does not change which languages Palimpsest corrects — that follows your layout pairs.",
    systemLanguage: "System language",

    advanced: "Advanced",
    manualHotkey: "Manual hotkey",
    setHotkey: "Set hotkey",
    clear: "Clear",
    manualHotkeyHint:
      "Click “Set hotkey”, then press a combination. Default Ctrl+Shift+F9. On laptops and 75% keyboards prefer a chord — keys like Pause or Scroll Lock often aren't there. Forces a fix on the current word. Esc cancels.",
    selectionHotkey: "Convert selection hotkey",
    selectionHotkeyHint:
      "Select any text, then press this to retype it in the other layout (e.g. “ghbdtn” → “привет”). Default Ctrl+Shift+F8; a modifier chord works on every keyboard. Esc cancels.",
    autostart: "Launch Palimpsest at login",
    openLogs: "Open logs folder",
    logsHint: "Diagnostics for bug reports. Never contains what you type.",

    updates: "Updates",
    checkNow: "Check now",
    checking: "Checking…",
    installRestart: "Install and restart",
    upToDate: "Palimpsest {version} — up to date.",
    updateAvailable: "Version {available} is available (you have {current}).",
    installing: "Downloading and installing…",
    updateUnreachable: "Couldn't reach the update server.",
    updateCheckFailed: "Update check failed: {error}",
    updateFailed: "Update failed: {error}",

    recentCorrections: "Recent corrections",
    undoLast: "Undo last",
    nothingYet: "Nothing yet.",
    tagUndone: "undone",
    tagManual: "manual",

    learnedWords: "Learned words",
    forgetAll: "Forget all",
    learnOnUndo: "Learn a word when I undo its correction",
    learnedWordsHint:
      "Undoing a correction tells Palimpsest the word was right as typed, so it stops correcting it. These words are the only thing Palimpsest saves from what you type — stored locally in <code>user_words.json</code>, never sent anywhere. Turn the setting off to save nothing at all.",
    nothingLearned: "Nothing learned yet.",
    forget: "Forget",
    forgotWord: "Forgot “{word}”.",
    forgotAll: "Forgot every learned word.",
    learnedToast: "Learned “{word}” — won't correct it again",

    saved: "Saved.",
    ready: "Ready.",
    none: "None",
    error: "Error: {error}",
    loadFailed: "Failed to load: {error}",
    listAppsFailed: "Couldn't list apps: {error}",
    listLayoutsFailed: "Couldn't list layouts: {error}",
    loadWordsFailed: "Couldn't load learned words: {error}",
    alreadyIgnored: "{name} is already ignored.",
    pressKey: "Press a key…",
    hotkeyUnchanged: "Hotkey unchanged.",
    hotkeyCleared: "Hotkey cleared.",
    selectionHotkeyCleared: "Selection hotkey cleared.",
    hotkeySet: "Hotkey set to {label}.",
    hotkeyRejected: "{label} can't be used. {error}",
    hotkeyTimeout:
      "Didn't see that key — Windows reserves some combinations (most Win+… ones) and never passes them on. Try Ctrl+Shift+…, Win+F8, or Win+Insert.",
    hotkeyCheckFailed: "Couldn't check hotkeys: {error}",
    conflictManual: "Manual hotkey",
    conflictSelection: "Selection hotkey",
    revertedToast: "Reverted → {text}",
    willLaunch: "Will launch at login.",
    wontLaunch: "Won't launch at login.",
  },

  ru: {
    tagline: "Незаметно исправляет текст, набранный не в той раскладке.",
    masterSwitch: "Главный переключатель",
    on: "Вкл.",
    paused: "Пауза",

    sensitivity: "Чувствительность",
    sensitivityHint:
      "Насколько Palimpsest должен быть уверен, прежде чем исправлять. Ниже — осторожнее (меньше ложных срабатываний), выше — решительнее.",
    cautious: "Осторожно",
    eager: "Решительно",

    layoutPairs: "Пары раскладок",
    addPair: "+ Добавить пару",
    layoutPairsHint:
      "Пары учитываются в обе стороны. Palimpsest переинтерпретирует слово в этих раскладках и выбирает лучшую.",
    phonetic: "фонетическая",
    phoneticHint:
      "Каждая кириллическая буква стоит на латинской клавише со схожим звучанием, поэтому текст не в той раскладке выглядит как «privet», а не «ghbdtn». Определяется автоматически.",
    removePair: "Удалить пару",
    notInstalled: "{name} (не установлена)",
    unknownLayout: "Неизвестная ({klid})",

    ignoreApps: "Не работать в этих приложениях",
    ignoreAppsHint:
      "В них Palimpsest полностью молчит — терминалы, IDE, игры. Выберите из запущенных приложений или впишите имена исполняемых файлов (по одному в строке, например <code>cmd.exe</code>).",
    add: "Добавить",
    refreshList: "Обновить список",

    language: "Язык",
    languageHint:
      "Язык этого окна и меню в трее. На то, какие языки Palimpsest исправляет, он не влияет — это определяют пары раскладок.",
    systemLanguage: "Язык системы",

    advanced: "Дополнительно",
    manualHotkey: "Горячая клавиша вручную",
    setHotkey: "Назначить",
    clear: "Сбросить",
    manualHotkeyHint:
      "Нажмите «Назначить» и введите сочетание. По умолчанию Ctrl+Shift+F9. На ноутбуках и клавиатурах 75% лучше выбирать сочетание — клавиш вроде Pause или Scroll Lock там часто нет. Принудительно исправляет текущее слово. Esc — отмена.",
    selectionHotkey: "Горячая клавиша для выделения",
    selectionHotkeyHint:
      "Выделите любой текст и нажмите это сочетание, чтобы ввести его заново в другой раскладке (например, «ghbdtn» → «привет»). По умолчанию Ctrl+Shift+F8; сочетание с модификатором работает на любой клавиатуре. Esc — отмена.",
    autostart: "Запускать Palimpsest при входе в систему",
    openLogs: "Открыть папку логов",
    logsHint: "Диагностика для сообщений об ошибках. Никогда не содержит набранный текст.",

    updates: "Обновления",
    checkNow: "Проверить",
    checking: "Проверяем…",
    installRestart: "Установить и перезапустить",
    upToDate: "Palimpsest {version} — обновлений нет.",
    updateAvailable: "Доступна версия {available} (у вас {current}).",
    installing: "Загрузка и установка…",
    updateUnreachable: "Не удалось связаться с сервером обновлений.",
    updateCheckFailed: "Проверка обновлений не удалась: {error}",
    updateFailed: "Обновление не удалось: {error}",

    recentCorrections: "Последние исправления",
    undoLast: "Отменить последнее",
    nothingYet: "Пока пусто.",
    tagUndone: "отменено",
    tagManual: "вручную",

    learnedWords: "Выученные слова",
    forgetAll: "Забыть все",
    learnOnUndo: "Запоминать слово, когда я отменяю его исправление",
    learnedWordsHint:
      "Отмена исправления говорит Palimpsest, что слово было набрано правильно, и он перестаёт его исправлять. Эти слова — единственное, что Palimpsest сохраняет из набранного вами: они лежат локально в <code>user_words.json</code> и никуда не отправляются. Выключите настройку, чтобы не сохранялось ничего.",
    nothingLearned: "Пока ничего не выучено.",
    forget: "Забыть",
    forgotWord: "Слово «{word}» забыто.",
    forgotAll: "Все выученные слова забыты.",
    learnedToast: "Запомнили «{word}» — больше не исправляем",

    saved: "Сохранено.",
    ready: "Готово.",
    none: "Нет",
    error: "Ошибка: {error}",
    loadFailed: "Не удалось загрузить: {error}",
    listAppsFailed: "Не удалось получить список приложений: {error}",
    listLayoutsFailed: "Не удалось получить список раскладок: {error}",
    loadWordsFailed: "Не удалось загрузить выученные слова: {error}",
    alreadyIgnored: "{name} уже в списке исключений.",
    pressKey: "Нажмите клавишу…",
    hotkeyUnchanged: "Сочетание не изменено.",
    hotkeyCleared: "Сочетание сброшено.",
    selectionHotkeyCleared: "Сочетание для выделения сброшено.",
    hotkeySet: "Назначено сочетание {label}.",
    hotkeyRejected: "{label} использовать нельзя. {error}",
    hotkeyTimeout:
      "Клавиша не дошла — Windows оставляет часть сочетаний себе (большинство с Win+…) и не передаёт их. Попробуйте Ctrl+Shift+…, Win+F8 или Win+Insert.",
    hotkeyCheckFailed: "Не удалось проверить сочетания: {error}",
    conflictManual: "Горячая клавиша вручную",
    conflictSelection: "Горячая клавиша для выделения",
    revertedToast: "Возвращено → {text}",
    willLaunch: "Будет запускаться при входе в систему.",
    wontLaunch: "Не будет запускаться при входе в систему.",
  },

  uk: {
    tagline: "Непомітно виправляє текст, набраний не в тій розкладці.",
    masterSwitch: "Головний перемикач",
    on: "Увімк.",
    paused: "Пауза",

    sensitivity: "Чутливість",
    sensitivityHint:
      "Наскільки Palimpsest має бути впевнений, перш ніж виправляти. Нижче — обережніше (менше хибних спрацювань), вище — рішучіше.",
    cautious: "Обережно",
    eager: "Рішуче",

    layoutPairs: "Пари розкладок",
    addPair: "+ Додати пару",
    layoutPairsHint:
      "Пари враховуються в обидва боки. Palimpsest переінтерпретує слово в цих розкладках і обирає кращу.",
    phonetic: "фонетична",
    phoneticHint:
      "Кожна кирилична літера стоїть на латинській клавіші зі схожим звучанням, тож текст не в тій розкладці має вигляд «privet», а не «ghbdtn». Визначається автоматично.",
    removePair: "Видалити пару",
    notInstalled: "{name} (не встановлена)",
    unknownLayout: "Невідома ({klid})",

    ignoreApps: "Не працювати в цих застосунках",
    ignoreAppsHint:
      "У них Palimpsest повністю мовчить — термінали, IDE, ігри. Оберіть із запущених застосунків або впишіть імена виконуваних файлів (по одному в рядку, наприклад <code>cmd.exe</code>).",
    add: "Додати",
    refreshList: "Оновити список",

    language: "Мова",
    languageHint:
      "Мова цього вікна та меню в треї. На те, які мови Palimpsest виправляє, вона не впливає — це визначають пари розкладок.",
    systemLanguage: "Мова системи",

    advanced: "Додатково",
    manualHotkey: "Гаряча клавіша вручну",
    setHotkey: "Призначити",
    clear: "Скинути",
    manualHotkeyHint:
      "Натисніть «Призначити» й уведіть сполучення. Типово Ctrl+Shift+F9. На ноутбуках і клавіатурах 75% краще обирати сполучення — клавіш на кшталт Pause чи Scroll Lock там часто немає. Примусово виправляє поточне слово. Esc — скасування.",
    selectionHotkey: "Гаряча клавіша для виділення",
    selectionHotkeyHint:
      "Виділіть будь-який текст і натисніть це сполучення, щоб увести його заново в іншій розкладці (наприклад, «ghbdtn» → «привет»). Типово Ctrl+Shift+F8; сполучення з модифікатором працює на будь-якій клавіатурі. Esc — скасування.",
    autostart: "Запускати Palimpsest під час входу в систему",
    openLogs: "Відкрити теку журналів",
    logsHint: "Діагностика для звітів про помилки. Ніколи не містить набраного тексту.",

    updates: "Оновлення",
    checkNow: "Перевірити",
    checking: "Перевіряємо…",
    installRestart: "Встановити й перезапустити",
    upToDate: "Palimpsest {version} — оновлень немає.",
    updateAvailable: "Доступна версія {available} (у вас {current}).",
    installing: "Завантаження та встановлення…",
    updateUnreachable: "Не вдалося зв'язатися із сервером оновлень.",
    updateCheckFailed: "Перевірка оновлень не вдалася: {error}",
    updateFailed: "Оновлення не вдалося: {error}",

    recentCorrections: "Останні виправлення",
    undoLast: "Скасувати останнє",
    nothingYet: "Поки порожньо.",
    tagUndone: "скасовано",
    tagManual: "вручну",

    learnedWords: "Вивчені слова",
    forgetAll: "Забути все",
    learnOnUndo: "Запам'ятовувати слово, коли я скасовую його виправлення",
    learnedWordsHint:
      "Скасування виправлення каже Palimpsest, що слово було набране правильно, і він перестає його виправляти. Ці слова — єдине, що Palimpsest зберігає з набраного вами: вони лежать локально у <code>user_words.json</code> і нікуди не надсилаються. Вимкніть налаштування, щоб не зберігалося нічого.",
    nothingLearned: "Поки нічого не вивчено.",
    forget: "Забути",
    forgotWord: "Слово «{word}» забуто.",
    forgotAll: "Усі вивчені слова забуто.",
    learnedToast: "Запам'ятали «{word}» — більше не виправляємо",

    saved: "Збережено.",
    ready: "Готово.",
    none: "Немає",
    error: "Помилка: {error}",
    loadFailed: "Не вдалося завантажити: {error}",
    listAppsFailed: "Не вдалося отримати список застосунків: {error}",
    listLayoutsFailed: "Не вдалося отримати список розкладок: {error}",
    loadWordsFailed: "Не вдалося завантажити вивчені слова: {error}",
    alreadyIgnored: "{name} уже у списку винятків.",
    pressKey: "Натисніть клавішу…",
    hotkeyUnchanged: "Сполучення не змінено.",
    hotkeyCleared: "Сполучення скинуто.",
    selectionHotkeyCleared: "Сполучення для виділення скинуто.",
    hotkeySet: "Призначено сполучення {label}.",
    hotkeyRejected: "{label} використати не можна. {error}",
    hotkeyTimeout:
      "Клавіша не дійшла — Windows залишає частину сполучень собі (більшість із Win+…) і не передає їх. Спробуйте Ctrl+Shift+…, Win+F8 або Win+Insert.",
    hotkeyCheckFailed: "Не вдалося перевірити сполучення: {error}",
    conflictManual: "Гаряча клавіша вручну",
    conflictSelection: "Гаряча клавіша для виділення",
    revertedToast: "Повернуто → {text}",
    willLaunch: "Запускатиметься під час входу в систему.",
    wontLaunch: "Не запускатиметься під час входу в систему.",
  },

  de: {
    tagline: "Falsche Tastaturbelegung, still korrigiert.",
    masterSwitch: "Hauptschalter",
    on: "An",
    paused: "Pausiert",

    sensitivity: "Empfindlichkeit",
    sensitivityHint:
      "Wie sicher sich Palimpsest sein muss, bevor es korrigiert. Niedriger ist vorsichtiger (weniger Fehlkorrekturen), höher ist eifriger.",
    cautious: "Vorsichtig",
    eager: "Eifrig",

    layoutPairs: "Belegungspaare",
    addPair: "+ Paar hinzufügen",
    layoutPairsHint:
      "Paare gelten in beide Richtungen. Palimpsest liest ein Wort in diesen Belegungen neu und wechselt zur besseren.",
    phonetic: "phonetisch",
    phoneticHint:
      "Jeder kyrillische Buchstabe liegt auf der lateinischen Taste mit ähnlichem Klang, deshalb liest sich falsch belegter Text wie „privet“ statt „ghbdtn“. Wird automatisch erkannt.",
    removePair: "Paar entfernen",
    notInstalled: "{name} (nicht installiert)",
    unknownLayout: "Unbekannt ({klid})",

    ignoreApps: "In diesen Apps ignorieren",
    ignoreAppsHint:
      "Hier bleibt Palimpsest völlig still — Terminals, IDEs, Spiele. Aus den laufenden Apps auswählen oder Programmnamen eintragen (einer pro Zeile, z. B. <code>cmd.exe</code>).",
    add: "Hinzufügen",
    refreshList: "Liste aktualisieren",

    language: "Sprache",
    languageHint:
      "Sprache dieses Fensters und des Tray-Menüs. Sie ändert nicht, welche Sprachen Palimpsest korrigiert — das ergibt sich aus den Belegungspaaren.",
    systemLanguage: "Systemsprache",

    advanced: "Erweitert",
    manualHotkey: "Manuelles Tastenkürzel",
    setHotkey: "Kürzel festlegen",
    clear: "Löschen",
    manualHotkeyHint:
      "Auf „Kürzel festlegen“ klicken und eine Kombination drücken. Standard ist Strg+Umschalt+F9. Auf Laptops und 75%-Tastaturen besser eine Kombination wählen — Tasten wie Pause oder Rollen fehlen dort oft. Erzwingt eine Korrektur des aktuellen Worts. Esc bricht ab.",
    selectionHotkey: "Kürzel für Auswahl umwandeln",
    selectionHotkeyHint:
      "Beliebigen Text markieren und dieses Kürzel drücken, um ihn in der anderen Belegung neu zu schreiben (z. B. „ghbdtn“ → „привет“). Standard ist Strg+Umschalt+F8; eine Kombination mit Modifikator funktioniert auf jeder Tastatur. Esc bricht ab.",
    autostart: "Palimpsest bei der Anmeldung starten",
    openLogs: "Log-Ordner öffnen",
    logsHint: "Diagnose für Fehlerberichte. Enthält nie, was getippt wird.",

    updates: "Updates",
    checkNow: "Jetzt prüfen",
    checking: "Wird geprüft…",
    installRestart: "Installieren und neu starten",
    upToDate: "Palimpsest {version} — aktuell.",
    updateAvailable: "Version {available} ist verfügbar (installiert ist {current}).",
    installing: "Wird heruntergeladen und installiert…",
    updateUnreachable: "Der Update-Server war nicht erreichbar.",
    updateCheckFailed: "Update-Prüfung fehlgeschlagen: {error}",
    updateFailed: "Update fehlgeschlagen: {error}",

    recentCorrections: "Letzte Korrekturen",
    undoLast: "Letzte rückgängig",
    nothingYet: "Noch nichts.",
    tagUndone: "rückgängig",
    tagManual: "manuell",

    learnedWords: "Gelernte Wörter",
    forgetAll: "Alle vergessen",
    learnOnUndo: "Wort lernen, wenn ich seine Korrektur rückgängig mache",
    learnedWordsHint:
      "Eine rückgängig gemachte Korrektur sagt Palimpsest, dass das Wort so richtig war, und es korrigiert es nicht mehr. Diese Wörter sind das Einzige, was Palimpsest vom Getippten speichert — lokal in <code>user_words.json</code>, niemals irgendwohin gesendet. Die Einstellung ausschalten, um gar nichts zu speichern.",
    nothingLearned: "Noch nichts gelernt.",
    forget: "Vergessen",
    forgotWord: "„{word}“ vergessen.",
    forgotAll: "Alle gelernten Wörter vergessen.",
    learnedToast: "„{word}“ gelernt — wird nicht mehr korrigiert",

    saved: "Gespeichert.",
    ready: "Bereit.",
    none: "Keins",
    error: "Fehler: {error}",
    loadFailed: "Laden fehlgeschlagen: {error}",
    listAppsFailed: "Apps konnten nicht aufgelistet werden: {error}",
    listLayoutsFailed: "Belegungen konnten nicht aufgelistet werden: {error}",
    loadWordsFailed: "Gelernte Wörter konnten nicht geladen werden: {error}",
    alreadyIgnored: "{name} wird bereits ignoriert.",
    pressKey: "Taste drücken…",
    hotkeyUnchanged: "Kürzel unverändert.",
    hotkeyCleared: "Kürzel gelöscht.",
    selectionHotkeyCleared: "Auswahl-Kürzel gelöscht.",
    hotkeySet: "Kürzel auf {label} gesetzt.",
    hotkeyRejected: "{label} ist nicht verwendbar. {error}",
    hotkeyTimeout:
      "Diese Taste kam nicht an — Windows behält manche Kombinationen für sich (die meisten mit Win+…) und gibt sie nie weiter. Strg+Umschalt+…, Win+F8 oder Win+Einfg versuchen.",
    hotkeyCheckFailed: "Kürzel konnten nicht geprüft werden: {error}",
    conflictManual: "Manuelles Tastenkürzel",
    conflictSelection: "Auswahl-Kürzel",
    revertedToast: "Zurückgesetzt → {text}",
    willLaunch: "Startet bei der Anmeldung.",
    wontLaunch: "Startet nicht bei der Anmeldung.",
  },
};

// Active locale. Never "system" — the backend resolves that before we hear it.
let LOCALE = "en";

// Switch the active locale, ignoring anything we don't ship.
function setLocale(code) {
  if (I18N[code]) LOCALE = code;
}

// Look up `key`, falling back to English so a gap in a translation shows the
// English wording rather than a raw key. `{name}` placeholders come from `vars`.
function t(key, vars) {
  const text = I18N[LOCALE][key] ?? I18N.en[key] ?? key;
  if (!vars) return text;
  return text.replace(/\{(\w+)\}/g, (match, name) => (name in vars ? vars[name] : match));
}

// Translate the static markup: `data-i18n` sets text, `data-i18n-html` sets
// markup (for the two hints carrying a <code> element), `data-i18n-title` sets
// the tooltip. Called once at startup and again whenever the language changes.
//
// `innerHTML` is safe here and only here: the source is the literal table above,
// never a config value, a process name, or anything the user typed — those all
// go through `textContent`.
function applyI18n(root = document) {
  for (const node of root.querySelectorAll("[data-i18n]")) {
    node.textContent = t(node.dataset.i18n);
  }
  for (const node of root.querySelectorAll("[data-i18n-html]")) {
    node.innerHTML = t(node.dataset.i18nHtml);
  }
  for (const node of root.querySelectorAll("[data-i18n-title]")) {
    node.title = t(node.dataset.i18nTitle);
  }
  document.documentElement.lang = LOCALE;
}
