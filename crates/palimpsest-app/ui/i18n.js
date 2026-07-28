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

    tabGeneral: "General",
    tabLayouts: "Layouts",
    tabHotkeys: "Hotkeys",
    tabHistory: "History",
    tabAbout: "About",

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

    manualHotkey: "Manual hotkey",
    setHotkey: "Set hotkey",
    clear: "Clear",
    manualHotkeyHint:
      "Click “Set hotkey”, then press a combination. Default Ctrl+Shift+F9. On laptops and 75% keyboards prefer a chord — keys like Pause or Scroll Lock often aren't there. Converts the word you are typing, or the one just behind the caret. Esc cancels.",
    selectionHotkey: "Convert selection hotkey",
    selectionHotkeyHint:
      "Select any text, then press this to retype it in the other layout (e.g. “ghbdtn” → “привет”). Default Ctrl+Shift+F8; a modifier chord works on every keyboard. Esc cancels.",
    undoHotkey: "Undo hotkey",
    undoHotkeyHint:
      "Reverts the last correction without leaving what you are typing — the same as “Undo last”. Default Ctrl+Shift+F10. Esc cancels.",

    startupAndLogs: "Startup and diagnostics",
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
      "Undoing a correction tells Palimpsest the word was right as typed, so it stops correcting it. Stored locally in <code>user_words.json</code>, never sent anywhere. Turn the setting off to save nothing at all.",
    nothingLearned: "Nothing learned yet.",
    forget: "Forget",
    forgotWord: "Forgot “{word}”.",
    forgotAll: "Forgot every learned word.",
    learnedToast: "Learned “{word}” — won't correct it again",

    tabDiary: "Diary",
    diary: "Typing diary",
    diaryEnabled: "Record",
    diaryClipboard: "Also record clipboard (copied text)",
    diarySkipShort: "Skip entries shorter than two words",
    clearDiary: "Erase diary",
    diaryWarning:
      "<strong>This is a keystroke log.</strong> With it on, every word you type is written to disk on this machine — not just corrections. Anyone with access to your user profile can read it unless you protect it with a password below. Nothing is recorded in password fields or ignored apps, nothing is ever sent anywhere, and it is off unless you switch it on here.",
    diaryConfirm:
      "Turn the typing diary on?\n\nEvery word you type will be written to disk on this machine, not just corrections. Anyone who can read your user profile can read it unless you set a password. You can erase it at any time.",
    diaryProtection: "Password",
    diaryProtectionHint:
      "A password encrypts the diary on disk so it can't be read without it. You'll need it to view the diary each time you open Palimpsest. It cannot be recovered — if you forget it, the diary is lost for good.",
    diaryProtected: "protected",
    diaryUnprotected: "not protected",
    diaryPwPlaceholder: "Password",
    diaryPw2Placeholder: "Repeat password",
    diarySetPw: "Set password",
    diaryChangePw: "Change password",
    diaryClearPw: "Remove password",
    diaryPwConfirm:
      "Set a diary password?\n\nThe diary will be encrypted on disk. You'll need this password to view it, and it CANNOT be recovered — if you forget it, the diary is lost for good.",
    diaryClearPwConfirm: "Remove the password and store the diary unencrypted?",
    diaryPwEmpty: "Enter a password.",
    diaryPwMismatch: "The two passwords don't match.",
    diaryPwSet: "Diary password set.",
    diaryPwRemoved: "Diary password removed.",
    diaryEntries: "Entries",
    diaryExport: "Export…",
    diaryExported: "Diary exported.",
    diaryExportedTo: "Exported to {path}",
    diarySearch: "Search…",
    diaryNoMatch: "No entries match your search.",
    diaryClipTag: "clipboard",
    diaryLockedHint: "The diary is locked. Enter your password to view it and resume recording.",
    diaryUnlock: "Unlock",
    diaryUnlocked: "Diary unlocked.",
    diaryClearConfirm: "Erase the entire diary? This can't be undone.",
    diaryOff: "The diary is off. Nothing is being recorded.",
    diaryEmpty: "Nothing recorded yet.",
    diaryCleared: "Diary erased.",
    loadDiaryFailed: "Couldn't load the diary: {error}",
    unknownApp: "unknown app",
    copy: "Copy",
    copied: "Copied.",

    aboutVersion: "Version {version}",
    aboutWhat:
      "Palimpsest watches for a word typed in the wrong keyboard layout, erases it, switches the layout, and retypes what you meant. Everything happens on this machine.",
    aboutCredits: "Credits",
    creditWordlists:
      "Word lists derived from <strong>FrequencyWords</strong> by Hermit Dave (MIT), built from 2018 OpenSubtitles corpora.",
    creditTauri: "Built with <strong>Tauri 2</strong> and Rust.",
    creditPunto:
      "Inspired by <strong>Punto Switcher</strong>, which taught a generation of bilingual typists that this problem was solvable.",
    aboutLicence: "Licence and source",
    aboutLicenceText: "Licensed under Apache-2.0. Source, issues, and releases:",

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
    undoHotkeyCleared: "Undo hotkey cleared.",
    hotkeySet: "Hotkey set to {label}.",
    hotkeyRejected: "{label} can't be used. {error}",
    hotkeyTimeout:
      "Didn't see that key — Windows reserves some combinations (most Win+… ones) and never passes them on. Try Ctrl+Shift+…, Win+F8, or Win+Insert.",
    hotkeyCheckFailed: "Couldn't check hotkeys: {error}",
    conflictManual: "Manual hotkey",
    conflictSelection: "Selection hotkey",
    conflictUndo: "Undo hotkey",
    revertedToast: "Reverted → {text}",
    willLaunch: "Will launch at login.",
    wontLaunch: "Won't launch at login.",
  },

  ru: {
    tagline: "Незаметно исправляет текст, набранный не в той раскладке.",
    masterSwitch: "Главный переключатель",
    on: "Вкл.",
    paused: "Пауза",

    tabGeneral: "Общие",
    tabLayouts: "Раскладки",
    tabHotkeys: "Сочетания",
    tabHistory: "История",
    tabAbout: "О программе",

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

    manualHotkey: "Горячая клавиша вручную",
    setHotkey: "Назначить",
    clear: "Сбросить",
    manualHotkeyHint:
      "Нажмите «Назначить» и введите сочетание. По умолчанию Ctrl+Shift+F9. На ноутбуках и клавиатурах 75% лучше выбирать сочетание — клавиш вроде Pause или Scroll Lock там часто нет. Преобразует слово, которое вы набираете, или предыдущее — то, что стоит перед курсором. Esc — отмена.",
    selectionHotkey: "Горячая клавиша для выделения",
    selectionHotkeyHint:
      "Выделите любой текст и нажмите это сочетание, чтобы ввести его заново в другой раскладке (например, «ghbdtn» → «привет»). По умолчанию Ctrl+Shift+F8; сочетание с модификатором работает на любой клавиатуре. Esc — отмена.",
    undoHotkey: "Горячая клавиша отмены",
    undoHotkeyHint:
      "Отменяет последнее исправление, не отрываясь от набора, — то же, что «Отменить последнее». По умолчанию Ctrl+Shift+F10. Esc — отмена.",

    startupAndLogs: "Запуск и диагностика",
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
      "Отмена исправления говорит Palimpsest, что слово было набрано правильно, и он перестаёт его исправлять. Слова лежат локально в <code>user_words.json</code> и никуда не отправляются. Выключите настройку, чтобы не сохранялось ничего.",
    nothingLearned: "Пока ничего не выучено.",
    forget: "Забыть",
    forgotWord: "Слово «{word}» забыто.",
    forgotAll: "Все выученные слова забыты.",
    learnedToast: "Запомнили «{word}» — больше не исправляем",

    tabDiary: "Дневник",
    diary: "Дневник набора",
    diaryEnabled: "Записывать",
    diaryClipboard: "Записывать также буфер обмена (скопированный текст)",
    diarySkipShort: "Пропускать записи короче двух слов",
    clearDiary: "Стереть дневник",
    diaryWarning:
      "<strong>Это журнал нажатий клавиш.</strong> Когда он включён, каждое набранное слово записывается на диск этого компьютера — не только исправления. Прочитать его сможет любой, у кого есть доступ к вашему профилю, если не защитить дневник паролем ниже. В полях паролей и в приложениях-исключениях ничего не пишется, никуда ничего не отправляется, и по умолчанию всё это выключено.",
    diaryConfirm:
      "Включить дневник набора?\n\nКаждое набранное слово будет записываться на диск этого компьютера, не только исправления. Без пароля прочитать его сможет любой, у кого есть доступ к вашему профилю. Стереть дневник можно в любой момент.",
    diaryProtection: "Пароль",
    diaryProtectionHint:
      "Пароль шифрует дневник на диске, так что без него прочитать нельзя. Он понадобится, чтобы открывать дневник при каждом запуске Palimpsest. Восстановить его невозможно — если забудете, дневник потерян навсегда.",
    diaryProtected: "защищён",
    diaryUnprotected: "без пароля",
    diaryPwPlaceholder: "Пароль",
    diaryPw2Placeholder: "Повторите пароль",
    diarySetPw: "Задать пароль",
    diaryChangePw: "Сменить пароль",
    diaryClearPw: "Убрать пароль",
    diaryPwConfirm:
      "Задать пароль дневника?\n\nДневник будет зашифрован на диске. Пароль понадобится для просмотра, и восстановить его НЕЛЬЗЯ — если забудете, дневник потерян навсегда.",
    diaryClearPwConfirm: "Убрать пароль и хранить дневник без шифрования?",
    diaryPwEmpty: "Введите пароль.",
    diaryPwMismatch: "Пароли не совпадают.",
    diaryPwSet: "Пароль дневника задан.",
    diaryPwRemoved: "Пароль дневника убран.",
    diaryEntries: "Записи",
    diaryExport: "Экспорт…",
    diaryExported: "Дневник экспортирован.",
    diaryExportedTo: "Экспортировано в {path}",
    diarySearch: "Поиск…",
    diaryNoMatch: "Нет записей по вашему запросу.",
    diaryClipTag: "буфер",
    diaryLockedHint: "Дневник заблокирован. Введите пароль, чтобы просмотреть его и возобновить запись.",
    diaryUnlock: "Разблокировать",
    diaryUnlocked: "Дневник разблокирован.",
    diaryClearConfirm: "Стереть весь дневник? Отменить нельзя.",
    diaryOff: "Дневник выключен. Ничего не записывается.",
    diaryEmpty: "Пока ничего не записано.",
    diaryCleared: "Дневник стёрт.",
    loadDiaryFailed: "Не удалось загрузить дневник: {error}",
    unknownApp: "неизвестное приложение",
    copy: "Копировать",
    copied: "Скопировано.",

    aboutVersion: "Версия {version}",
    aboutWhat:
      "Palimpsest замечает слово, набранное не в той раскладке, стирает его, переключает раскладку и набирает то, что вы имели в виду. Всё происходит на этом компьютере.",
    aboutCredits: "Благодарности",
    creditWordlists:
      "Словари построены на основе <strong>FrequencyWords</strong> Hermit Dave (MIT) по корпусам OpenSubtitles 2018 года.",
    creditTauri: "Сделано на <strong>Tauri 2</strong> и Rust.",
    creditPunto:
      "Вдохновлено <strong>Punto Switcher</strong>, который показал целому поколению двуязычных наборщиков, что эта задача решаема.",
    aboutLicence: "Лицензия и исходный код",
    aboutLicenceText: "Лицензия Apache-2.0. Исходный код, задачи и релизы:",

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
    undoHotkeyCleared: "Сочетание отмены сброшено.",
    hotkeySet: "Назначено сочетание {label}.",
    hotkeyRejected: "{label} использовать нельзя. {error}",
    hotkeyTimeout:
      "Клавиша не дошла — Windows оставляет часть сочетаний себе (большинство с Win+…) и не передаёт их. Попробуйте Ctrl+Shift+…, Win+F8 или Win+Insert.",
    hotkeyCheckFailed: "Не удалось проверить сочетания: {error}",
    conflictManual: "Горячая клавиша вручную",
    conflictSelection: "Горячая клавиша для выделения",
    conflictUndo: "Горячая клавиша отмены",
    revertedToast: "Возвращено → {text}",
    willLaunch: "Будет запускаться при входе в систему.",
    wontLaunch: "Не будет запускаться при входе в систему.",
  },

  uk: {
    tagline: "Непомітно виправляє текст, набраний не в тій розкладці.",
    masterSwitch: "Головний перемикач",
    on: "Увімк.",
    paused: "Пауза",

    tabGeneral: "Загальні",
    tabLayouts: "Розкладки",
    tabHotkeys: "Сполучення",
    tabHistory: "Історія",
    tabAbout: "Про програму",

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

    manualHotkey: "Гаряча клавіша вручну",
    setHotkey: "Призначити",
    clear: "Скинути",
    manualHotkeyHint:
      "Натисніть «Призначити» й уведіть сполучення. Типово Ctrl+Shift+F9. На ноутбуках і клавіатурах 75% краще обирати сполучення — клавіш на кшталт Pause чи Scroll Lock там часто немає. Перетворює слово, яке ви набираєте, або попереднє — те, що стоїть перед курсором. Esc — скасування.",
    selectionHotkey: "Гаряча клавіша для виділення",
    selectionHotkeyHint:
      "Виділіть будь-який текст і натисніть це сполучення, щоб увести його заново в іншій розкладці (наприклад, «ghbdtn» → «привет»). Типово Ctrl+Shift+F8; сполучення з модифікатором працює на будь-якій клавіатурі. Esc — скасування.",
    undoHotkey: "Гаряча клавіша скасування",
    undoHotkeyHint:
      "Скасовує останнє виправлення, не відриваючись від набору, — те саме, що «Скасувати останнє». Типово Ctrl+Shift+F10. Esc — скасування.",

    startupAndLogs: "Запуск і діагностика",
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
      "Скасування виправлення каже Palimpsest, що слово було набране правильно, і він перестає його виправляти. Слова лежать локально у <code>user_words.json</code> і нікуди не надсилаються. Вимкніть налаштування, щоб не зберігалося нічого.",
    nothingLearned: "Поки нічого не вивчено.",
    forget: "Забути",
    forgotWord: "Слово «{word}» забуто.",
    forgotAll: "Усі вивчені слова забуто.",
    learnedToast: "Запам'ятали «{word}» — більше не виправляємо",

    tabDiary: "Щоденник",
    diary: "Щоденник набору",
    diaryEnabled: "Записувати",
    diaryClipboard: "Записувати також буфер обміну (скопійований текст)",
    diarySkipShort: "Пропускати записи, коротші за два слова",
    clearDiary: "Стерти щоденник",
    diaryWarning:
      "<strong>Це журнал натискань клавіш.</strong> Коли він увімкнений, кожне набране слово записується на диск цього комп'ютера — не лише виправлення. Будь-хто, хто має доступ до вашого профілю, зможе його прочитати, якщо не захистити щоденник паролем нижче. У полях паролів і в застосунках-винятках нічого не пишеться, нікуди нічого не надсилається, і типово все це вимкнено.",
    diaryConfirm:
      "Увімкнути щоденник набору?\n\nКожне набране слово записуватиметься на диск цього комп'ютера, не лише виправлення. Без пароля прочитати його зможе будь-хто, хто має доступ до вашого профілю. Стерти щоденник можна будь-коли.",
    diaryProtection: "Пароль",
    diaryProtectionHint:
      "Пароль шифрує щоденник на диску, тож без нього прочитати не можна. Він знадобиться, щоб відкривати щоденник під час кожного запуску Palimpsest. Відновити його неможливо — якщо забудете, щоденник втрачено назавжди.",
    diaryProtected: "захищений",
    diaryUnprotected: "без пароля",
    diaryPwPlaceholder: "Пароль",
    diaryPw2Placeholder: "Повторіть пароль",
    diarySetPw: "Задати пароль",
    diaryChangePw: "Змінити пароль",
    diaryClearPw: "Прибрати пароль",
    diaryPwConfirm:
      "Задати пароль щоденника?\n\nЩоденник буде зашифровано на диску. Пароль знадобиться для перегляду, і відновити його НЕМОЖЛИВО — якщо забудете, щоденник втрачено назавжди.",
    diaryClearPwConfirm: "Прибрати пароль і зберігати щоденник без шифрування?",
    diaryPwEmpty: "Уведіть пароль.",
    diaryPwMismatch: "Паролі не збігаються.",
    diaryPwSet: "Пароль щоденника задано.",
    diaryPwRemoved: "Пароль щоденника прибрано.",
    diaryEntries: "Записи",
    diaryExport: "Експорт…",
    diaryExported: "Щоденник експортовано.",
    diaryExportedTo: "Експортовано до {path}",
    diarySearch: "Пошук…",
    diaryNoMatch: "Немає записів за вашим запитом.",
    diaryClipTag: "буфер",
    diaryLockedHint: "Щоденник заблоковано. Уведіть пароль, щоб переглянути його та відновити запис.",
    diaryUnlock: "Розблокувати",
    diaryUnlocked: "Щоденник розблоковано.",
    diaryClearConfirm: "Стерти весь щоденник? Скасувати не можна.",
    diaryOff: "Щоденник вимкнено. Нічого не записується.",
    diaryEmpty: "Поки нічого не записано.",
    diaryCleared: "Щоденник стерто.",
    loadDiaryFailed: "Не вдалося завантажити щоденник: {error}",
    unknownApp: "невідомий застосунок",
    copy: "Копіювати",
    copied: "Скопійовано.",

    aboutVersion: "Версія {version}",
    aboutWhat:
      "Palimpsest помічає слово, набране не в тій розкладці, стирає його, перемикає розкладку й набирає те, що ви мали на увазі. Усе відбувається на цьому комп'ютері.",
    aboutCredits: "Подяки",
    creditWordlists:
      "Словники побудовані на основі <strong>FrequencyWords</strong> Hermit Dave (MIT) за корпусами OpenSubtitles 2018 року.",
    creditTauri: "Зроблено на <strong>Tauri 2</strong> і Rust.",
    creditPunto:
      "Натхнено <strong>Punto Switcher</strong>, який показав цілому поколінню двомовних набирачів, що це завдання можна розв'язати.",
    aboutLicence: "Ліцензія та вихідний код",
    aboutLicenceText: "Ліцензія Apache-2.0. Вихідний код, задачі та релізи:",

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
    undoHotkeyCleared: "Сполучення скасування скинуто.",
    hotkeySet: "Призначено сполучення {label}.",
    hotkeyRejected: "{label} використати не можна. {error}",
    hotkeyTimeout:
      "Клавіша не дійшла — Windows залишає частину сполучень собі (більшість із Win+…) і не передає їх. Спробуйте Ctrl+Shift+…, Win+F8 або Win+Insert.",
    hotkeyCheckFailed: "Не вдалося перевірити сполучення: {error}",
    conflictManual: "Гаряча клавіша вручну",
    conflictSelection: "Гаряча клавіша для виділення",
    conflictUndo: "Гаряча клавіша скасування",
    revertedToast: "Повернуто → {text}",
    willLaunch: "Запускатиметься під час входу в систему.",
    wontLaunch: "Не запускатиметься під час входу в систему.",
  },

  de: {
    tagline: "Falsche Tastaturbelegung, still korrigiert.",
    masterSwitch: "Hauptschalter",
    on: "An",
    paused: "Pausiert",

    tabGeneral: "Allgemein",
    tabLayouts: "Belegungen",
    tabHotkeys: "Kürzel",
    tabHistory: "Verlauf",
    tabAbout: "Über",

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

    manualHotkey: "Manuelles Tastenkürzel",
    setHotkey: "Kürzel festlegen",
    clear: "Löschen",
    manualHotkeyHint:
      "Auf „Kürzel festlegen“ klicken und eine Kombination drücken. Standard ist Strg+Umschalt+F9. Auf Laptops und 75%-Tastaturen besser eine Kombination wählen — Tasten wie Pause oder Rollen fehlen dort oft. Wandelt das Wort um, das gerade getippt wird, oder das direkt vor dem Cursor. Esc bricht ab.",
    selectionHotkey: "Kürzel für Auswahl umwandeln",
    selectionHotkeyHint:
      "Beliebigen Text markieren und dieses Kürzel drücken, um ihn in der anderen Belegung neu zu schreiben (z. B. „ghbdtn“ → „привет“). Standard ist Strg+Umschalt+F8; eine Kombination mit Modifikator funktioniert auf jeder Tastatur. Esc bricht ab.",
    undoHotkey: "Kürzel zum Rückgängigmachen",
    undoHotkeyHint:
      "Macht die letzte Korrektur rückgängig, ohne das Tippen zu unterbrechen — dasselbe wie „Letzte rückgängig“. Standard ist Strg+Umschalt+F10. Esc bricht ab.",

    startupAndLogs: "Start und Diagnose",
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
      "Eine rückgängig gemachte Korrektur sagt Palimpsest, dass das Wort so richtig war, und es korrigiert es nicht mehr. Lokal in <code>user_words.json</code> gespeichert, niemals irgendwohin gesendet. Die Einstellung ausschalten, um gar nichts zu speichern.",
    nothingLearned: "Noch nichts gelernt.",
    forget: "Vergessen",
    forgotWord: "„{word}“ vergessen.",
    forgotAll: "Alle gelernten Wörter vergessen.",
    learnedToast: "„{word}“ gelernt — wird nicht mehr korrigiert",

    tabDiary: "Tagebuch",
    diary: "Tipp-Tagebuch",
    diaryEnabled: "Aufzeichnen",
    diaryClipboard: "Auch die Zwischenablage aufzeichnen (kopierter Text)",
    diarySkipShort: "Einträge unter zwei Wörtern überspringen",
    clearDiary: "Tagebuch löschen",
    diaryWarning:
      "<strong>Das ist ein Tastenprotokoll.</strong> Eingeschaltet wird jedes getippte Wort auf die Festplatte dieses Rechners geschrieben — nicht nur Korrekturen. Wer Zugriff auf Ihr Benutzerprofil hat, kann es lesen, sofern Sie es unten nicht mit einem Passwort schützen. In Passwortfeldern und ignorierten Apps wird nichts aufgezeichnet, es wird nie irgendwohin gesendet, und ohne diesen Schalter bleibt alles aus.",
    diaryConfirm:
      "Tipp-Tagebuch einschalten?\n\nJedes getippte Wort wird auf die Festplatte dieses Rechners geschrieben, nicht nur Korrekturen. Ohne Passwort kann jeder es lesen, der Ihr Benutzerprofil lesen kann. Löschen können Sie es jederzeit.",
    diaryProtection: "Passwort",
    diaryProtectionHint:
      "Ein Passwort verschlüsselt das Tagebuch auf der Festplatte, sodass es ohne dieses nicht lesbar ist. Sie brauchen es, um das Tagebuch bei jedem Start von Palimpsest anzusehen. Es kann nicht wiederhergestellt werden — vergessen Sie es, ist das Tagebuch für immer verloren.",
    diaryProtected: "geschützt",
    diaryUnprotected: "ungeschützt",
    diaryPwPlaceholder: "Passwort",
    diaryPw2Placeholder: "Passwort wiederholen",
    diarySetPw: "Passwort festlegen",
    diaryChangePw: "Passwort ändern",
    diaryClearPw: "Passwort entfernen",
    diaryPwConfirm:
      "Ein Tagebuch-Passwort festlegen?\n\nDas Tagebuch wird auf der Festplatte verschlüsselt. Sie brauchen dieses Passwort zum Ansehen, und es kann NICHT wiederhergestellt werden — vergessen Sie es, ist das Tagebuch für immer verloren.",
    diaryClearPwConfirm: "Das Passwort entfernen und das Tagebuch unverschlüsselt speichern?",
    diaryPwEmpty: "Bitte ein Passwort eingeben.",
    diaryPwMismatch: "Die beiden Passwörter stimmen nicht überein.",
    diaryPwSet: "Tagebuch-Passwort festgelegt.",
    diaryPwRemoved: "Tagebuch-Passwort entfernt.",
    diaryEntries: "Einträge",
    diaryExport: "Exportieren…",
    diaryExported: "Tagebuch exportiert.",
    diaryExportedTo: "Exportiert nach {path}",
    diarySearch: "Suchen…",
    diaryNoMatch: "Keine Einträge passen zur Suche.",
    diaryClipTag: "Zwischenablage",
    diaryLockedHint: "Das Tagebuch ist gesperrt. Passwort eingeben, um es anzusehen und die Aufzeichnung fortzusetzen.",
    diaryUnlock: "Entsperren",
    diaryUnlocked: "Tagebuch entsperrt.",
    diaryClearConfirm: "Das ganze Tagebuch löschen? Das lässt sich nicht rückgängig machen.",
    diaryOff: "Das Tagebuch ist aus. Es wird nichts aufgezeichnet.",
    diaryEmpty: "Noch nichts aufgezeichnet.",
    diaryCleared: "Tagebuch gelöscht.",
    loadDiaryFailed: "Tagebuch konnte nicht geladen werden: {error}",
    unknownApp: "unbekannte App",
    copy: "Kopieren",
    copied: "Kopiert.",

    aboutVersion: "Version {version}",
    aboutWhat:
      "Palimpsest erkennt ein Wort in der falschen Tastaturbelegung, löscht es, wechselt die Belegung und tippt das, was gemeint war. Alles passiert auf diesem Rechner.",
    aboutCredits: "Dank",
    creditWordlists:
      "Wortlisten abgeleitet aus <strong>FrequencyWords</strong> von Hermit Dave (MIT), auf Basis der OpenSubtitles-Korpora von 2018.",
    creditTauri: "Gebaut mit <strong>Tauri 2</strong> und Rust.",
    creditPunto:
      "Inspiriert von <strong>Punto Switcher</strong>, das einer ganzen Generation zweisprachiger Tippender zeigte, dass dieses Problem lösbar ist.",
    aboutLicence: "Lizenz und Quellcode",
    aboutLicenceText: "Lizenziert unter Apache-2.0. Quellcode, Issues und Releases:",

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
    undoHotkeyCleared: "Rückgängig-Kürzel gelöscht.",
    hotkeySet: "Kürzel auf {label} gesetzt.",
    hotkeyRejected: "{label} ist nicht verwendbar. {error}",
    hotkeyTimeout:
      "Diese Taste kam nicht an — Windows behält manche Kombinationen für sich (die meisten mit Win+…) und gibt sie nie weiter. Strg+Umschalt+…, Win+F8 oder Win+Einfg versuchen.",
    hotkeyCheckFailed: "Kürzel konnten nicht geprüft werden: {error}",
    conflictManual: "Manuelles Tastenkürzel",
    conflictSelection: "Auswahl-Kürzel",
    conflictUndo: "Rückgängig-Kürzel",
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
  for (const node of root.querySelectorAll("[data-i18n-placeholder]")) {
    node.placeholder = t(node.dataset.i18nPlaceholder);
  }
  document.documentElement.lang = LOCALE;
}
