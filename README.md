**English** · [Русский](README.ru.md)

# Palimpsest

Palimpsest is a Windows-first typing assistant for people who often switch
between keyboard layouts. Its goal is to detect when a word was typed using the
wrong layout, remove the incorrect characters, switch the operating-system
layout, and retype the intended text.

The idea is similar to tools such as Punto Switcher, Mahou, and Caramba
Switcher, but this project is built as a local, Rust-based utility with a strong
focus on low-latency keyboard handling, privacy, and predictable correction
behavior.
<img width="1164" height="1383" alt="image" src="https://github.com/user-attachments/assets/3b03aff8-de1e-4027-a0a9-8a1531de736b" />

> Development status: feature-complete preview, pending interactive
> verification and a signed release. The full pipeline is implemented — hook,
> reinterpretation, scoring, automatic correction, context awareness, the Tauri
> tray/settings UI, and an NSIS installer. What remains is human-gated:
> hands-on testing on a real desktop, code signing, and soak/performance runs.

## What Palimpsest does

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

## Why it exists

Typing several languages on Windows is easy to interrupt: one forgotten layout
switch can turn a whole word into unreadable text. Palimpsest aims to fix that
without sending text to a server, without storing typed content, and without
blocking normal keyboard input.

The project favors conservative corrections. A missed correction is acceptable;
a false correction is much more disruptive. The planned UI therefore includes a
sensitivity setting, ignore lists, pause controls, and an undo action.

## How it works

Palimpsest is organized around a strict latency rule: the Windows keyboard hook
must do almost no work. If a hook callback is slow, Windows can silently remove
it. For that reason, the architecture splits responsibilities into separate
threads:

- Hook thread: owns `WH_KEYBOARD_LL`, reads only the virtual key, scan code,
  flags, and timestamp, forwards a tiny event over a bounded channel, then calls
  the next hook.
- Engine thread: owns word buffering, layout reinterpretation, scoring,
  correction decisions, active-window context, and input execution.
- UI thread: Tauri 2 application with tray icon, settings window, configuration
  commands, and live status events.

Synthetic input is tagged with a `dwExtraInfo` signature so Palimpsest can ignore
the keystrokes it generated itself. This prevents correction feedback loops.

## Current status

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
- A fully localized interface — English, Russian, Ukrainian, and German — which
  follows the Windows UI language by default and can be pinned in Settings.
- Serializable configuration persisted to `%APPDATA%`, applied live (no restart).
- File diagnostics logging (never containing typed text at the default level),
  a crash/panic hook, and an NSIS per-user installer.
- Signed in-app updates (Settings → Updates) fed by a tag-driven GitHub Actions
  release build.

Remaining before a public release (human-gated):

- Interactive verification on a real desktop session.
- Code signing with an OV/EV certificate and AV allow-listing.
- Multi-day soak testing and hot-path performance profiling.

## Privacy and safety model

Palimpsest is intended to be local-only:

- No typed text should be sent to a network service.
- Raw keystrokes should not be written to disk in normal operation.
- The word buffer should be short-lived and in memory only.
- Diagnostics should avoid logging typed content by default.
- The only network request is the update check: a plain fetch of the release
  feed, with no identifiers and no telemetry. Update packages are verified
  against a signing key built into the app, so a compromised download host
  cannot ship code.

Because Palimpsest uses a global keyboard hook and synthesizes input, antivirus
and SmartScreen warnings are a real distribution concern. A credible release
will need code signing, transparent behavior, no obfuscation, and clear
documentation.

Windows integrity boundaries still apply. A non-elevated Palimpsest process
cannot reliably inject input into elevated administrator windows, UAC prompts,
the lock screen, or the secure desktop.

## User interface

The desktop app is a Tauri 2 tray utility:

- Tray icon with on/paused state; left-click toggles, right-click opens the menu
  (Enabled, Settings, Open logs folder, Quit).
- Settings window for layout pairs, sensitivity, ignore-list entries, the manual
  hotkey, the interface language, and autostart — changes apply live, with no
  restart.
- Correction history and an undo-last-correction action.
- Live status and correction events streamed from the engine.

It is a single instance (only one hook is ever installed) and lives in the tray;
closing the settings window hides it rather than quitting.

### Interface language

The settings window and the tray menu are available in English, Russian,
Ukrainian, and German. The default, **System language**, follows the Windows UI
language and falls back to English for anything else; Settings → Language pins a
specific one. This is presentation only — it does not change which languages
Palimpsest *corrects*, which follows the configured layout pairs.

## Screenshots

Screenshots are not captured yet (the UI exists but needs a live desktop session
to photograph). Placeholders for future documentation:

![Tray menu placeholder](docs/screenshots/tray-placeholder.png)

![Settings window placeholder](docs/screenshots/settings-placeholder.png)

![Correction history placeholder](docs/screenshots/history-placeholder.png)

## Build and development

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

Releases are built by GitHub Actions from a tag. Bump the version in `Cargo.toml`
and `crates/palimpsest-app/tauri.conf.json` (the workflow refuses a tag that
disagrees with either), then:

```powershell
git tag v0.3.0
git push origin main v0.3.0
```

That publishes the installer and the signed `latest.json` feed the in-app updater
reads.

Interactive spikes for the core, run manually from a local Windows session
(they observe or inject into the focused window):

```powershell
cargo run -p palimpsest-core --example spike_hook
cargo run -p palimpsest-core --example spike_correct  # ghbdtn -> привет in Notepad
cargo run -p palimpsest-core --example spike_context
```

See `plan/DISTRIBUTION.md` for signing, antivirus, logs, and the release
checklist, and `plan/PROGRESS.md` for detailed status.

## Roadmap

1. ✅ Core hook, injection, runtime plumbing, and the global on/off switch.
2. ✅ Headless word analysis: `ToUnicodeEx` reinterpretation, bigram scoring,
   and guardrails with EN/RU/DE test coverage.
3. ✅ The Palimpsest correction effect: backspace, layout switch, Unicode retype,
   plus a manual hotkey and undo.
4. ✅ Active-window context: ignore lists and best-effort password-field detection.
5. ✅ The Tauri tray/settings UI, localized into EN/RU/UK/DE.
6. ✅ Release hardening code: file logging, crash capture, and an NSIS installer.
7. ⏳ Remaining: interactive verification, code signing, and soak/performance runs.

## License

Apache-2.0. See [LICENSE](LICENSE).
