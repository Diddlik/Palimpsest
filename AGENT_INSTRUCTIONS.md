# Agent Instructions

Single source of truth for AI agents (Claude Code, Codex, others) working in this
repository. `CLAUDE.md` and `AGENTS.md` are pointers to this file.

## Living Document

This file must always reflect the current state of the repository. When
structure, commands, testing practice, security rules, or workflow change
meaningfully, update this file **in the same change**. Do not let it drift; do
not duplicate its content into `CLAUDE.md` or `AGENTS.md`.

## What Palimpsest Is

Windows-first typing assistant that detects a word typed in the wrong keyboard
layout, deletes it, switches the OS layout, and retypes the intended text
(`ghbdtn` → `привет`). Local-only, low-latency, conservative: a missed
correction is acceptable, a false correction is not.

Status: feature-complete preview. The whole pipeline is implemented — hook,
reinterpretation, scoring, automatic correction, context awareness, Tauri
tray/settings UI, NSIS installer. What remains is human-gated: interactive
verification on a real desktop, code signing + AV allow-listing, and
soak/performance runs. See `README.md` for the user-facing description (English;
`README.ru.md` is the Russian translation, linked from the top of both) and
`plan/PROGRESS.md` for detailed status.

## Project Structure

Rust workspace, two crates:

- `crates/palimpsest-core` — hook, word buffering, layout reinterpretation
  (`ToUnicodeEx`), wordlist + bigram scoring, correction engine, runtime,
  injection, active-window context, config, clipboard.
- `crates/palimpsest-core/wordlists` — one plain-text frequency word list per
  language (`en`, `de`, `ru`), lowercase, one word per line. `build.rs` compiles
  them into FSTs in `OUT_DIR`, which `scoring.rs` embeds with `include_bytes!`.
  Edit the text files, never the generated FSTs. See "Scoring Data" below.
- `crates/palimpsest-app` — Tauri 2 tray/settings app: commands, logging, state,
  capabilities, icons, static UI under `ui/`. The settings window is a tab bar
  over six panes (General, Layouts, Hotkeys, History, Diary, About); a `.pane` is
  shown by `.active`, and `main.js` keeps exactly one tab and one pane marked.
  User-facing text is localized — see "Localization" below.
- `crates/palimpsest-core/examples` — manual Windows spike programs
  (`spike_hook`, `spike_correct`, `spike_context`); interactive, not a substitute
  for tests.
- `.github/workflows` — `ci.yml` (clippy + tests on push/PR, Windows only) and
  `release.yml` (tag-driven build and publish). See "Releasing" below.
- `scripts/bump.ps1` — the supported way to cut a release: bumps both manifests
  and `Cargo.lock` together, commits, tags, pushes. See "Releasing".
- `plan/` — planning and release notes (`IMPLEMENTATION_PLAN.md`, `PROGRESS.md`,
  `DISTRIBUTION.md`). Local-only, gitignored.
- `target/`, generated Tauri schemas (`crates/palimpsest-app/gen`), installer
  output — build artifacts, never edit or commit.

## Architecture Rule (non-negotiable)

Three threads, strict latency budget:

- **Hook thread** owns `WH_KEYBOARD_LL`. It reads only virtual key, scan code,
  flags, timestamp, pushes a tiny event onto a bounded channel, and calls the
  next hook. Nothing else — a slow callback gets the hook silently removed by
  Windows.
- **Engine thread** owns buffering, reinterpretation, scoring, correction
  decisions, window context, and input execution.
- **UI thread** owns the Tauri app, config commands, and live status events.

Keep new work off the hook path. Move it to the engine/runtime path.

## Build, Test, Development

Run from the repository root unless noted (PowerShell):

```powershell
cargo build --workspace --examples
cargo test --workspace
cargo clippy --workspace --examples
cargo run -p palimpsest-app
```

Installer (NSIS, per-user) — needs `cargo install tauri-cli --version "^2"`:

```powershell
cd crates/palimpsest-app
cargo tauri build
# → target/release/bundle/nsis/Palimpsest_<version>_x64-setup.exe
```

Interactive spikes, manual local Windows session only (they observe or inject
into the focused window):

```powershell
cargo run -p palimpsest-core --example spike_hook
cargo run -p palimpsest-core --example spike_correct  # ghbdtn -> привет in Notepad
cargo run -p palimpsest-core --example spike_context
```

`spike_hook` is the tool for "did the hook even see that key" — it prints `vk`,
`scan`, `injected` and `ours` per key-down. Note that `hook::run` installs the
hook **disarmed**; anything calling it directly must also call
`hook::set_armed(true)` or it will observe nothing and look broken.
`Runtime::start` does this for you.

`run.cmd` is a local convenience launcher.

## Coding Style

Rust 2021, four-space indentation. **Do not run `cargo fmt`** — this repo is
hand-formatted in a compact style and a default `rustfmt` pass causes repo-wide
churn. Match the formatting of the surrounding code instead.

Prefer small modules with explicit names (`buffer`, `scoring`, `runtime`,
`commands`). `snake_case` for functions, modules, variables, tests;
`PascalCase` for types; `SCREAMING_SNAKE_CASE` for constants.

## Scoring Data

`scoring.rs` judges a word with two pieces of evidence, both derived from the
same embedded wordlist:

1. **Wordlist membership** — an FST lookup. A hit adds a flat `DICT_BONUS`,
   deliberately larger than any correction margin in `analyze.rs`, because
   wrong-layout text essentially never spells a real word.
2. **Character bigram model** — trained by streaming the FST's keys. This is the
   fallback for words no list contains: names, German compounds, rare
   inflections.

### Two decision paths

Which path `analyze()` takes depends on `LayoutPair::phonetic`:

- **Positional** (ЙЦУКЕН, `00000419`) — wrong-layout text is phonotactically
  illegal (`ghbdtn`), so the bigram margin decides, scaled by sensitivity.
- **Phonetic** (`a0000419` and friends, each Cyrillic letter on its Latin
  sound-alike key) — wrong-layout text is a legal-looking romanization
  (`privet`), so the margin collapses and the *wordlists* decide instead:
  correct only when the Cyrillic reading is a real word and the Latin one is
  not, and only from `MIN_PHONETIC_WORD_LEN` characters up. When both readings
  are words (`net`/`нет`, `son`/`сон`, `mir`/`мир`) auto always keeps — no global
  rule can know which was meant.

`phonetic` is fingerprinted once at startup (`phonetic::classify_pairs`, probe
keys through `ToUnicodeEx`) and stored on the pair; `None` means "not yet
classified" and is treated as positional, so a failed fingerprint degrades to
the pre-phonetic behaviour rather than misfiring. A user override in Settings is
never overwritten.

On top of both sits the user's own dictionary (`user_dict.rs`), learned by
undoing a correction. `analyze()` treats a user word as a **veto on automatic
correction**, not as a score bonus — a bonus large enough to overturn a wide
margin like `ghbdtn` → `привет` (~7 nats) would distort every other decision.
`suggest()` (the manual hotkey) deliberately ignores the veto, which is how a
word learned by mistake can still be converted.

The lists come from [FrequencyWords](https://github.com/hermitdave/FrequencyWords)
(MIT, 2018 subtitle corpora), filtered to words that are at least three
characters, occur at least 50 times, and use only that language's alphabet.
`Models::builtin()` costs ~23 ms once at startup; a single score is ~0.6 µs.

When changing scoring, re-check the margins empirically rather than by
intuition — a real wrong-layout word should beat its alternative by several nats
(`ghbdtn` → `привет` clears by ~7), while a correctly typed word whose
reinterpretation is merely near-plausible must come out negative (`hers` →
`рукы` sits at about −5). Adding words to a list can only be validated by
measuring both directions: recall on genuine wrong-layout input, and the
false-positive rate on correctly typed input. A missed correction is acceptable;
a false correction is not.

## Localization

The UI ships in English, Russian, Ukrainian, and German. `Config::ui_language`
(a `UiLang`) holds the choice; `UiLang::System` is the default and resolves
through `GetUserDefaultUILanguage`, falling back to English.

The strings live in two tables because they render in two places:

- `crates/palimpsest-app/src/i18n.rs` — tray menu, tray tooltip, and the hotkey
  rejection messages returned over IPC.
- `crates/palimpsest-app/ui/i18n.js` — everything in the settings window. Markup
  is tagged `data-i18n` / `data-i18n-html` / `data-i18n-title`; code calls
  `t(key, vars)`. Missing keys fall back to English rather than showing the key.

"System" is resolved **only** in Rust (`commands::resolved_ui_language`); the
webview asks for the answer instead of probing the OS itself, so the window and
the tray can never end up in different languages.

Adding a language means all four of: a `UiLang` variant, a `Strings` block in
`i18n.rs`, a table in `i18n.js`, and an `<option>` in `ui/index.html`. Never add
a user-facing string without adding it to every table — the coverage is
deliberate, and English-only leftovers are the usual way localization rots.

This is presentation only. It has nothing to do with the languages the *scorer*
knows (`scoring::Lang`, driven by the layout pairs and the wordlists); adding a
UI language does not add a correction language, which needs a wordlist and the
empirical margin checks described under "Scoring Data".

## Hotkeys

Three bindings, all matched inside the engine from the observed key stream
(exact modifier match, `0` disables), all defaulting to a Ctrl+Shift chord on an
adjacent function key — function keys exist on every board, unlike the original
bare Pause/Scroll Lock:

| Binding                      | Default      | Acts on                          |
|------------------------------|--------------|----------------------------------|
| `manual_hotkey`              | Ctrl+Shift+F9  | the word being typed, else the last finished one |
| `selection_hotkey`           | Ctrl+Shift+F8  | the current selection            |
| `undo_hotkey`                | Ctrl+Shift+F10 | the last correction              |

Every one of them must `inject::release_modifiers(self.mods)` before injecting.
The chord is still physically held when the engine reacts, so without it the
backspaces run as Ctrl+Backspace (eating a whole word) and the retyped text is
swallowed as shortcuts.

`undo_hotkey` field-defaults to the shipped chord rather than to `0`, so an
upgrade picks it up; the older two default to `0` because a partial file could
otherwise resurrect a *bare* typing key. An explicit `0` always means "the user
cleared it" and is never migrated back.

**The manual hotkey's last-word path works from rendered text, not keys.**
`Engine::last` holds the word as it currently appears on screen plus its layout,
and conversion goes through `suggest_text` — the same never-refuses path as the
selection hotkey, for the same reason (an explicit instruction is not
second-guessed). Storing keys was the bug: the keys stop describing the text the
moment anything rewrites it, so a word that had just been auto-corrected, or
converted by an earlier press of this same hotkey, became unreachable and the
hotkey silently did nothing. Rendered text is accurate whatever put it there,
which makes the press repeatable — it converts, and converts straight back. Do
not "fix" this by reverting to buffered keys or by adding a plausibility veto.

## Convert Selection

The selection hotkey is the one flow that leaves the keystroke pipeline: it
snapshots the clipboard, empties it, synthesizes `Ctrl+C`, polls for the text,
reinterprets it (`analyze::suggest_text`), types the result over the selection,
and restores the clipboard. Three rules it has already been broken by:

- **Open the clipboard with a real `HWND`, on a thread that pumps.**
  `clipboard::owner_window` exists because `OpenClipboard(NULL)` +
  `EmptyClipboard` sets the owner to NULL, which makes every later
  `SetClipboardData` fail. But owning the clipboard obliges the owner window to
  answer `WM_DESTROYCLIPBOARD`, which Windows delivers by `SendMessage` — so the
  window gets its own pumping thread. Parking it on the engine thread froze the
  app the user was copying in, mid-copy, while it held the clipboard lock.
- **Never poll `OpenClipboard` in a loop.** It is one system-wide lock and the
  foreground app needs it to publish the copy; competing for it produced
  `ERROR_ACCESS_DENIED` storms. Watch `clipboard::sequence()`
  (`GetClipboardSequenceNumber`, lock-free) and open only once it moves. That is
  also why there is no `clear()`: the counter distinguishes "nothing was copied"
  from stale contents without writing, so the read path is non-destructive and an
  aborted conversion leaves the clipboard untouched.
- **`suggest_text` must not depend on the active layout.** It tries both
  directions of every configured pair. Anchoring on the live layout broke the
  normal workflow (type it wrong, switch layout, *then* select and convert).
- **`suggest_text` never refuses.** Scoring ranks candidates there; it does not
  veto them. The selection hotkey is an explicit instruction, so the guardrails
  that stop `analyze` from acting unbidden do not apply — a conversion the user
  did not want costs one undo, whereas a hotkey that silently declines is
  indistinguishable from a broken one. It returns `None` only for an empty
  selection or when no pair renders the text differently. Do not "improve" this
  by adding a plausibility check back.
- **Its no-op paths log at INFO.** "I pressed it and nothing happened" is
  otherwise undiagnosable. Those lines carry a reason, an exe name, or a KLID —
  never typed text, which stays at DEBUG. Keep that split when adding paths.

## Testing

Unit tests live beside the implementation in `#[cfg(test)] mod tests` (see
`scoring.rs`, `engine.rs`, `buffer.rs`). Name tests by behavior, e.g.
`backspace_removes_last_character`. Cover scoring thresholds, buffer boundaries,
config serialization, and correction decisions. Anything requiring a live
desktop is manual and must be flagged as such, not automated away.

## Security & Privacy

- Never log typed text, raw word buffers, secrets, or full keystroke streams.
  This covers `tracing` unconditionally, including the diary below — diary text
  goes to its own file and the settings window, never to a log line.
- Preserve the `dwExtraInfo` synthetic-input tagging path — it is what prevents
  correction feedback loops.
- The word buffer stays short-lived and in memory only.
- **Two features deliberately persist typed text.** Nothing else may. Both write
  to `%APPDATA%\com.palimpsest.desktop\`, in their own file, and both are
  switchable, listable, and erasable from Settings. Keep every one of those
  properties on both:
  - The **learned-word dictionary** (`user_dict.rs`, `user_words.json`). Undoing
    a correction stores that single word so it is not corrected again.
    Switchable via `Config::learn_on_undo` (**on** by default), listed and
    individually erasable in Settings → History → Learned words. Do not extend it
    to phrases, context, or frequency counts.
  - The **typing diary** (`diary.rs`, `diary.json`, its own Diary tab). This one
    is a keystroke log: every word the engine sees, grouped by app, whether or not
    it was corrected — the Punto Switcher "дневник" (issue #1). It is fenced
    accordingly, and the fences are the feature:
    - `Config::diary_enabled` is **off** by default and an upgrade never turns it
      on; the UI confirms before enabling and carries a plain-language warning.
    - Nothing reaches it from a blocked context — the engine records only after
      `blocked()` has cleared the foreground window, so password fields and
      ignore-listed apps produce no entries. The same gate covers clipboard
      capture.
    - `Config::diary_skip_short` (**on** by default) drops closed entries under
      two words — Punto's guard against logging a lone password. Clipboard entries
      are exempt. Applied on the app's save path (`state::flush_diary`).
    - `Config::diary_capture_clipboard` (**off**) also logs copied text, via the
      engine's idle-tick clipboard watcher (`Engine::on_tick`) — never the hook
      path. It watches the lock-free sequence counter and resets its baseline
      after Palimpsest's own selection-conversion writes so it doesn't record
      them.
    - Entries are capped at `diary::MAX_ENTRIES`; Settings → Diary shows the lot,
      searches, exports to a text file, and erases in one click.
    - **Encryption + password** (`crypt.rs`: Argon2id + XChaCha20-Poly1305).
      Setting a password *is* what enables at-rest encryption — there is no key
      without one. A password-protected diary loads **locked**: it neither shows
      nor records until unlocked this session, because there is no key to decrypt
      or append with. The key is held in memory only while unlocked. A forgotten
      password is unrecoverable by design — say so, don't add a backdoor. The lock
      state lives in `state::DiaryState`; the app never writes a
      protected-but-locked diary (empty in memory, no key).

    Do not add a "just this once" bypass of any of that, and do not let the diary
    or clipboard capture become a default.
- Learning is **silent** — no prompt. A dialog would steal focus from the window
  being typed into, which is exactly the wrong thing for an app that injects
  keystrokes. The toast and the Settings list are how the user finds out.
- No network transmission of typed content, ever. The app makes exactly one kind
  of network request: the updater fetching `latest.json` and, if the user asks
  for it, the update package. It carries no identifiers and no telemetry. Do not
  add a second one.
- Config changes apply live (no restart) and persist under `%APPDATA%`.
- A non-elevated process cannot inject into elevated windows, UAC prompts, the
  lock screen, or the secure desktop — and, by UIPI, its low-level hook does not
  even *see* keystrokes going to an elevated window, so a hotkey pressed there
  fires nothing and looks broken. Do not pretend otherwise in code or docs. The
  opt-in remedy is Settings → General → "Start as administrator at login"
  (`elevate.rs`): a `HighestAvailable` scheduled task started at logon, so
  Palimpsest runs elevated and sees every window. It is off by default, prompts
  UAC once when enabled (creating the task needs it), and turns off the normal
  Run-key autostart so the two do not race the single-instance guard.

## Commits & Pull Requests

Concise imperative subjects (`Add correction threshold tests`, `Fix tray pause
state`). PRs describe user-visible behavior, list verification commands, note
manual Windows testing, and include screenshots for UI changes. Link the related
`plan/` item when there is one.

Repository: `github.com/Diddlik/Palimpsest` (public). Licensed Apache-2.0.
`plan/`, `AGENTS.md`, and `.claude/` are intentionally gitignored.

## Releasing

Releases are tag-driven, and `release.yml` refuses to build unless the tag
matches **both** `Cargo.toml` and `tauri.conf.json`. Use `scripts/bump.ps1`
rather than editing those by hand — it moves both manifests plus the tracked
`Cargo.lock`, commits, tags, and pushes as one step, which is the whole point:
the CI guard otherwise fails *after* the tag is already public.

```powershell
./scripts/bump.ps1                 # patch: 0.3.0 -> 0.3.1, tag, push
./scripts/bump.ps1 -Part minor     # 0.3.0 -> 0.4.0
./scripts/bump.ps1 -Version 1.0.0  # explicit
./scripts/bump.ps1 -Part minor -DryRun   # print the plan, change nothing
./scripts/bump.ps1 -NoPush         # commit + tag locally only
```

It refuses to run on a dirty tree or over an existing tag. Keep the CI version
check as it is — the script makes the common path correct, it does not replace
the safety net.

The workflow runs the tests, builds the NSIS installer and the signed updater
artifacts (`createUpdaterArtifacts`), and publishes a GitHub release carrying
`latest.json`.

The in-app updater (`commands::check_update` / `install_update`, Settings →
Updates) polls `releases/latest/download/latest.json` and verifies the package
signature against the public key in `tauri.conf.json`. Two consequences worth
keeping in mind:

- The release must be **published, not a draft or prerelease** — GitHub excludes
  both from `latest`, which would silently strand every installed copy.
- The signing key lives only in the repository secrets
  (`TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`). Losing it
  means no existing installation can ever be updated again — a new key would
  have to be shipped by a manual reinstall. Never commit it.

Builds are not code-signed yet, so SmartScreen warns on first run. That is a
known gap, not something to work around in code.
