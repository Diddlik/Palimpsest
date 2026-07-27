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
soak/performance runs. See `README.md` for the user-facing description and
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
  capabilities, icons, static UI under `ui/`.
- `crates/palimpsest-core/examples` — manual Windows spike programs
  (`spike_hook`, `spike_correct`, `spike_context`); interactive, not a substitute
  for tests.
- `.github/workflows` — `ci.yml` (clippy + tests on push/PR, Windows only) and
  `release.yml` (tag-driven build and publish). See "Releasing" below.
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

## Testing

Unit tests live beside the implementation in `#[cfg(test)] mod tests` (see
`scoring.rs`, `engine.rs`, `buffer.rs`). Name tests by behavior, e.g.
`backspace_removes_last_character`. Cover scoring thresholds, buffer boundaries,
config serialization, and correction decisions. Anything requiring a live
desktop is manual and must be flagged as such, not automated away.

## Security & Privacy

- Never log typed text, raw word buffers, secrets, or full keystroke streams.
- Preserve the `dwExtraInfo` synthetic-input tagging path — it is what prevents
  correction feedback loops.
- The word buffer stays short-lived and in memory only.
- **The one exception is the learned-word dictionary** (`user_dict.rs`,
  persisted to `%APPDATA%\com.palimpsest.desktop\user_words.json`). Undoing a
  correction stores that single word so it is not corrected again. It is
  deliberate, switchable (`Config::learn_on_undo`), listed and individually
  erasable in Settings → Learned words. Keep all four of those properties: it is
  the only place typed text survives a restart, so it must stay visible and
  reversible. Do not extend it to phrases, context, or frequency counts.
- Learning is **silent** — no prompt. A dialog would steal focus from the window
  being typed into, which is exactly the wrong thing for an app that injects
  keystrokes. The toast and the Settings list are how the user finds out.
- No network transmission of typed content, ever. The app makes exactly one kind
  of network request: the updater fetching `latest.json` and, if the user asks
  for it, the update package. It carries no identifiers and no telemetry. Do not
  add a second one.
- Config changes apply live (no restart) and persist under `%APPDATA%`.
- A non-elevated process cannot inject into elevated windows, UAC prompts, the
  lock screen, or the secure desktop. Do not pretend otherwise in code or docs.

## Commits & Pull Requests

Concise imperative subjects (`Add correction threshold tests`, `Fix tray pause
state`). PRs describe user-visible behavior, list verification commands, note
manual Windows testing, and include screenshots for UI changes. Link the related
`plan/` item when there is one.

Repository: `github.com/Diddlik/Palimpsest` (public). Licensed Apache-2.0.
`plan/`, `AGENTS.md`, and `.claude/` are intentionally gitignored.

## Releasing

Releases are tag-driven. `release.yml` refuses to build unless the tag matches
both `Cargo.toml` and `tauri.conf.json`, so bump them together:

```powershell
# edit version in Cargo.toml + crates/palimpsest-app/tauri.conf.json
git commit -am "Release v0.3.0"
git tag v0.3.0
git push origin main v0.3.0
```

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
