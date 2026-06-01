# Palimpsest

Palimpsest is a Windows-first typing assistant for people who often switch
between keyboard layouts. Its goal is to detect when a word was typed using the
wrong layout, remove the incorrect characters, switch the operating-system
layout, and retype the intended text.

The idea is similar to tools such as Punto Switcher, Mahou, and Caramba
Switcher, but this project is built as a local, Rust-based utility with a strong
focus on low-latency keyboard handling, privacy, and predictable correction
behavior.

> Development status: feature-complete preview, pending interactive
> verification and a signed release. The full pipeline is implemented — hook,
> reinterpretation, scoring, automatic correction, context awareness, the Tauri
> tray/settings UI, and an NSIS installer. What remains is human-gated:
> hands-on testing on a real desktop, code signing, and soak/performance runs.

## Contents

- [English](#english)
  - [What Palimpsest does](#what-palimpsest-does)
  - [Why it exists](#why-it-exists)
  - [How it works](#how-it-works)
  - [Current status](#current-status)
  - [Privacy and safety model](#privacy-and-safety-model)
  - [Planned user interface](#planned-user-interface)
  - [Screenshots](#screenshots)
  - [Build and development](#build-and-development)
  - [Roadmap](#roadmap)
- [Русский](#русский)
  - [Что делает Palimpsest](#что-делает-palimpsest)
  - [Зачем он нужен](#зачем-он-нужен)
  - [Как это работает](#как-это-работает)
  - [Текущее состояние](#текущее-состояние)
  - [Приватность и безопасность](#приватность-и-безопасность)
  - [Планируемый интерфейс](#планируемый-интерфейс)
  - [Скриншоты](#скриншоты)
  - [Сборка и разработка](#сборка-и-разработка)
  - [Дорожная карта](#дорожная-карта)

---

## English

### What Palimpsest does

Palimpsest watches typed keystrokes locally and tries to recognize wrong-layout
words. For example, if the user intended to type Russian but the English layout
was active, the word `ghbdtn` can be interpreted as `привет`.

The intended correction flow is:

1. Observe physical key presses through a Windows low-level keyboard hook.
2. Buffer one word at a time until a boundary such as space, enter, tab, or
   punctuation.
3. Reinterpret the buffered scan codes under configured keyboard layouts.
4. Score the possible interpretations with language heuristics.
5. If the alternate layout is clearly better, send backspaces, switch the OS
   layout, and retype the corrected text.

Palimpsest is designed for alphabetic layout pairs first, especially EN/RU and
later EN/DE. Complex IME scenarios are out of scope for the first version.

### Why it exists

Typing several languages on Windows is easy to interrupt: one forgotten layout
switch can turn a whole word into unreadable text. Palimpsest aims to fix that
without sending text to a server, without storing typed content, and without
blocking normal keyboard input.

The project favors conservative corrections. A missed correction is acceptable;
a false correction is much more disruptive. The planned UI therefore includes a
sensitivity setting, ignore lists, pause controls, and an undo action.

### How it works

Palimpsest is organized around a strict latency rule: the Windows keyboard hook
must do almost no work. If a hook callback is slow, Windows can silently remove
it. For that reason, the architecture splits responsibilities into separate
threads:

- Hook thread: owns `WH_KEYBOARD_LL`, reads only the virtual key, scan code,
  flags, and timestamp, forwards a tiny event over a bounded channel, then calls
  the next hook.
- Engine thread: owns word buffering, layout reinterpretation, scoring,
  correction decisions, active-window context, and input execution.
- UI thread: planned Tauri 2 application with tray icon, settings window,
  configuration commands, and live status events.

Synthetic input is tagged with a `dwExtraInfo` signature so Palimpsest can ignore
the keystrokes it generated itself. This prevents correction feedback loops.

### Current status

Implemented:

- Cargo workspace with `palimpsest-core` (engine) and `palimpsest-app` (Tauri).
- Windows low-level keyboard hook with `dwExtraInfo` tagging to ignore
  Palimpsest's own injected input (no feedback loop).
- Three-thread runtime (hook / engine / UI) with clean start and stop, plus a
  global on/off switch.
- Word buffer with boundary and backspace handling.
- Scan-code reinterpretation through `ToUnicodeEx` across configured layouts.
- Per-language bigram scoring (EN, RU, DE) with a cross-alphabet floor and
  sensitivity-driven guardrails.
- Automatic correction during typing (backspace → layout switch → Unicode
  retype, as one atomic injection) plus a manual hotkey and one-step undo.
- Active-window context: ignore-list (terminals/IDEs/games) and best-effort
  native password-field detection.
- Tauri 2 tray + settings UI: enable/pause, sensitivity, layout pairs,
  ignore-list, autostart, manual hotkey, live correction history, and undo.
- Serializable configuration persisted to `%APPDATA%`, applied live (no restart).
- File diagnostics logging (never containing typed text at the default level),
  a crash/panic hook, and an NSIS per-user installer.

Remaining before a public release (human-gated):

- Interactive verification on a real desktop session.
- Code signing with an OV/EV certificate and AV allow-listing.
- Multi-day soak testing and hot-path performance profiling.

### Privacy and safety model

Palimpsest is intended to be local-only:

- No typed text should be sent to a network service.
- Raw keystrokes should not be written to disk in normal operation.
- The word buffer should be short-lived and in memory only.
- Diagnostics should avoid logging typed content by default.

Because Palimpsest uses a global keyboard hook and synthesizes input, antivirus
and SmartScreen warnings are a real distribution concern. A credible release
will need code signing, transparent behavior, no obfuscation, and clear
documentation.

Windows integrity boundaries still apply. A non-elevated Palimpsest process
cannot reliably inject input into elevated administrator windows, UAC prompts,
the lock screen, or the secure desktop.

### User interface

The desktop app is a Tauri 2 tray utility:

- Tray icon with on/paused state; left-click toggles, right-click opens the menu
  (Enabled, Settings, Open logs folder, Quit).
- Settings window for layout pairs, sensitivity, ignore-list entries, the manual
  hotkey, and autostart — changes apply live, with no restart.
- Correction history and an undo-last-correction action.
- Live status and correction events streamed from the engine.

It is a single instance (only one hook is ever installed) and lives in the tray;
closing the settings window hides it rather than quitting.

### Screenshots

Screenshots are not captured yet (the UI exists but needs a live desktop session
to photograph). Placeholders for future documentation:

![Tray menu placeholder](docs/screenshots/tray-placeholder.png)

![Settings window placeholder](docs/screenshots/settings-placeholder.png)

![Correction history placeholder](docs/screenshots/history-placeholder.png)

### Build and development

Requirements:

- Windows.
- Rust stable toolchain for `x86_64-pc-windows-msvc`.
- Tauri CLI (`cargo install tauri-cli --version "^2"`) — only for building the
  installer; plain `cargo run` works without it.

Build the workspace and run checks:

```powershell
cargo build --workspace --examples
cargo test --workspace
cargo clippy --workspace --examples
```

Run the app (tray + settings):

```powershell
cargo run -p palimpsest-app
```

Build the installer (NSIS, per-user):

```powershell
cd crates/palimpsest-app
cargo tauri build
# → target/release/bundle/nsis/Palimpsest_<version>_x64-setup.exe
```

Interactive spikes for the core, run manually from a local Windows session
(they observe or inject into the focused window):

```powershell
cargo run -p palimpsest-core --example spike_hook
cargo run -p palimpsest-core --example spike_correct  # ghbdtn -> привет in Notepad
cargo run -p palimpsest-core --example spike_context
```

See `plan/DISTRIBUTION.md` for signing, antivirus, logs, and the release
checklist, and `plan/PROGRESS.md` for detailed status.

### Roadmap

1. ✅ Core hook, injection, runtime plumbing, and the global on/off switch.
2. ✅ Headless word analysis: `ToUnicodeEx` reinterpretation, bigram scoring,
   and guardrails with EN/RU/DE test coverage.
3. ✅ The Palimpsest correction effect: backspace, layout switch, Unicode retype,
   plus a manual hotkey and undo.
4. ✅ Active-window context: ignore lists and best-effort password-field detection.
5. ✅ The Tauri tray/settings UI.
6. ✅ Release hardening code: file logging, crash capture, and an NSIS installer.
7. ⏳ Remaining: interactive verification, code signing, and soak/performance runs.

---

## Русский

### Что делает Palimpsest

Palimpsest — это Windows-утилита для тех, кто часто печатает на нескольких
раскладках. Программа должна замечать, что слово набрано в неправильной
раскладке, удалять ошибочные символы, переключать раскладку Windows и заново
вводить правильный текст.

Например, если пользователь хотел набрать русское слово, но активной осталась
английская раскладка, `ghbdtn` может быть распознано как `привет`.

Планируемый сценарий коррекции:

1. Наблюдать физические нажатия клавиш через низкоуровневый hook Windows.
2. Собирать слово до разделителя: пробел, Enter, Tab, пунктуация.
3. Переинтерпретировать scan code нажатых клавиш в разных раскладках.
4. Оценить варианты с помощью языковых эвристик.
5. Если другая раскладка явно лучше, отправить Backspace, переключить раскладку
   ОС и ввести исправленный текст.

Первая версия ориентирована на алфавитные пары раскладок, прежде всего EN/RU и
позже EN/DE. Сложные IME-сценарии не входят в первую цель.

### Зачем он нужен

При работе с несколькими языками легко забыть переключить раскладку и получить
на экране набор бессмысленных символов. Palimpsest должен исправлять такие
ошибки локально: без отправки текста на сервер, без сохранения набранного текста
и без задержек при обычной печати.

Проект специально выбирает осторожное поведение. Лучше пропустить сомнительное
исправление, чем ошибочно заменить правильный текст. Поэтому в интерфейсе
планируются настройка чувствительности, список исключений, пауза и быстрое
откатывание последнего исправления.

### Как это работает

Главное архитектурное ограничение: callback клавиатурного hook должен быть
максимально коротким. Если он работает слишком долго, Windows может молча
удалить hook. Поэтому Palimpsest разделяет работу на несколько потоков:

- Поток hook: владеет `WH_KEYBOARD_LL`, считывает только virtual key, scan code,
  флаги и время события, отправляет маленькое событие в канал и сразу передает
  управление дальше.
- Поток engine: собирает слова, переинтерпретирует раскладки, считает оценки,
  принимает решение об исправлении, учитывает активное окно и выполняет ввод.
- Поток UI: Tauri 2 интерфейс с tray-иконкой, окном настроек, командами
  конфигурации и событиями статуса.

Синтетический ввод помечается специальным значением `dwExtraInfo`, чтобы
Palimpsest отличал собственные нажатия от пользовательских. Это защищает от
бесконечного цикла, когда программа исправляет текст, который сама же ввела.

### Текущее состояние

Уже реализовано:

- Cargo workspace с `palimpsest-core` (engine) и `palimpsest-app` (Tauri).
- Низкоуровневый keyboard hook Windows с маркировкой `dwExtraInfo`, чтобы
  игнорировать собственный синтетический ввод (без петли обратной связи).
- Трёхпоточный runtime (hook / engine / UI) с чистым запуском и остановкой и
  глобальным переключателем вкл/выкл.
- Модуль word buffer с обработкой границ слова и Backspace.
- Переинтерпретация scan code через `ToUnicodeEx` по настроенным раскладкам.
- Bigram-оценка по языкам (EN, RU, DE) с порогом для разных алфавитов и
  ограничителями, зависящими от чувствительности.
- Автоматическое исправление при печати (Backspace → смена раскладки → Unicode
  retype одним атомарным вводом), а также ручная горячая клавиша и undo.
- Контекст активного окна: список исключений (терминалы/IDE/игры) и best-effort
  определение нативных полей пароля.
- Tauri 2 tray и окно настроек: включение/пауза, чувствительность, пары
  раскладок, список исключений, autostart, ручная клавиша, история и undo.
- Сериализуемая конфигурация в `%APPDATA%`, применяется на лету (без рестарта).
- Логи диагностики (без набранного текста на уровне по умолчанию), panic hook и
  per-user installer NSIS.

Осталось до публичного релиза (требует человека):

- Интерактивная проверка в реальной сессии Windows.
- Code signing сертификатом OV/EV и allow-listing у антивирусов.
- Многодневное soak-тестирование и профилирование горячего пути.

### Приватность и безопасность

Palimpsest задуман как полностью локальная программа:

- Набранный текст не должен отправляться в сеть.
- Raw keystrokes не должны записываться на диск в обычном режиме.
- Буфер слова должен быть коротким и храниться только в памяти.
- Диагностические логи по умолчанию не должны содержать набранный текст.

Так как программа использует глобальный keyboard hook и синтетический ввод,
антивирусы и SmartScreen могут воспринимать ее подозрительно. Для нормального
релиза нужны code signing, прозрачное поведение, отсутствие обфускации и понятная
документация.

Ограничения безопасности Windows остаются в силе. Не повышенный процесс
Palimpsest не сможет надежно вводить текст в окна администратора, UAC prompt,
экран блокировки или secure desktop.

### Интерфейс

Настольное приложение на Tauri 2:

- Tray-иконка со статусом вкл/пауза: левый клик переключает, правый открывает
  меню (Enabled, Settings, Open logs folder, Quit).
- Окно настроек для пар раскладок, чувствительности, исключений, ручной горячей
  клавиши и autostart — изменения применяются на лету, без рестарта.
- История исправлений и undo последнего исправления.
- Живые status- и correction-события от engine.

Приложение работает в единственном экземпляре (hook ставится только один раз) и
живёт в трее; закрытие окна настроек прячет его, а не завершает программу.

### Скриншоты

Скриншотов пока нет (интерфейс готов, но для съёмки нужна живая сессия рабочего
стола). Будущие места для изображений:

![Tray menu placeholder](docs/screenshots/tray-placeholder.png)

![Settings window placeholder](docs/screenshots/settings-placeholder.png)

![Correction history placeholder](docs/screenshots/history-placeholder.png)

### Сборка и разработка

Требования:

- Windows.
- Rust stable toolchain для `x86_64-pc-windows-msvc`.
- Tauri CLI (`cargo install tauri-cli --version "^2"`) — только для сборки
  installer; обычный `cargo run` работает и без неё.

Сборка workspace и проверки:

```powershell
cargo build --workspace --examples
cargo test --workspace
cargo clippy --workspace --examples
```

Запуск приложения (tray + настройки):

```powershell
cargo run -p palimpsest-app
```

Сборка installer (NSIS, per-user):

```powershell
cd crates/palimpsest-app
cargo tauri build
# → target/release/bundle/nsis/Palimpsest_<version>_x64-setup.exe
```

Интерактивные spikes для ядра — запускать вручную в локальной Windows-сессии
(они наблюдают или вводят данные в активное окно):

```powershell
cargo run -p palimpsest-core --example spike_hook
cargo run -p palimpsest-core --example spike_correct  # ghbdtn -> привет в Notepad
cargo run -p palimpsest-core --example spike_context
```

Подробности о подписи, антивирусах, логах и релизе — в `plan/DISTRIBUTION.md`,
детальный статус — в `plan/PROGRESS.md`.

### Дорожная карта

1. ✅ Ядро: hook, injection, runtime и глобальный переключатель вкл/выкл.
2. ✅ Headless-анализ слов: `ToUnicodeEx`, bigram-оценка и ограничители с
   покрытием тестами для EN/RU/DE.
3. ✅ Эффект Palimpsest: Backspace, переключение раскладки, Unicode retype,
   а также ручная горячая клавиша и undo.
4. ✅ Контекст активного окна: список исключений и best-effort проверка полей
   пароля.
5. ✅ Tauri tray/settings UI.
6. ✅ Код для релиза: logging, crash capture и installer NSIS.
7. ⏳ Осталось: интерактивная проверка, code signing и soak/perf-тесты.
