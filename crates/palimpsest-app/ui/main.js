// Palimpsest settings UI. Uses the global Tauri API (`withGlobalTauri`), so no
// bundler is required: this file is served straight from the embedded `ui/`
// directory. It reads/writes config through IPC commands and listens for engine
// events to drive the toast and history.
//
// User-facing text comes from `i18n.js` via `t()`; nothing here is hardcoded
// English. The active locale is whatever the backend resolves `ui_language` to,
// so this window and the tray menu always speak the same language.

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

// Fallback friendly names for KLIDs (used for history tags and any layout not in
// the installed list). The picker itself is populated from installed layouts.
const FALLBACK_LAYOUTS = {
  "00000409": "English (US)",
  "00000419": "Russian",
  "00000407": "German",
  "0000040c": "French",
  "0000040a": "Spanish",
  "00000415": "Polish",
  "00000422": "Ukrainian",
};

// VK code → display name for the manual hotkey.
const VK_NAMES = {
  0x13: "Pause", 0x91: "ScrollLock", 0x90: "NumLock", 0x2c: "PrintScreen",
  0x20: "Space", 0x2d: "Insert", 0x2e: "Delete", 0x24: "Home", 0x23: "End",
  0x21: "PageUp", 0x22: "PageDown", 0x25: "Left", 0x26: "Up",
  0x27: "Right", 0x28: "Down", 0x5d: "Menu", 0x1b: "Esc",
};

const $ = (id) => document.getElementById(id);
const el = {
  tabs: $("tabs"),
  enabled: $("enabled"),
  enabledLabel: $("enabled-label"),
  sensitivity: $("sensitivity"),
  sensitivityValue: $("sensitivity-value"),
  pairs: $("pairs"),
  addPair: $("add-pair"),
  ignore: $("ignore"),
  procList: $("proc-list"),
  addProc: $("add-proc"),
  refreshProc: $("refresh-proc"),
  uiLanguage: $("ui-language"),
  hotkeyDisplay: $("hotkey-display"),
  setHotkey: $("set-hotkey"),
  clearHotkey: $("clear-hotkey"),
  selectionHotkeyDisplay: $("selection-hotkey-display"),
  setSelectionHotkey: $("set-selection-hotkey"),
  clearSelectionHotkey: $("clear-selection-hotkey"),
  undoHotkeyDisplay: $("undo-hotkey-display"),
  setUndoHotkey: $("set-undo-hotkey"),
  clearUndoHotkey: $("clear-undo-hotkey"),
  autostart: $("autostart"),
  elevatedAutostart: $("elevated-autostart"),
  undo: $("undo"),
  openLogs: $("open-logs"),
  history: $("history"),
  learnOnUndo: $("learn-on-undo"),
  words: $("words"),
  clearWords: $("clear-words"),
  diaryEnabled: $("diary-enabled"),
  diaryClipboard: $("diary-clipboard"),
  diarySkipShort: $("diary-skip-short"),
  diaryMaxEntries: $("diary-max-entries"),
  diaryRetentionDays: $("diary-retention-days"),
  diaryMaxSizeMb: $("diary-max-size-mb"),
  diaryProtectionState: $("diary-protection-state"),
  diarySetPwRow: $("diary-set-pw-row"),
  diaryPw: $("diary-pw"),
  diaryPw2: $("diary-pw2"),
  diarySetPw: $("diary-set-pw"),
  diaryClearPw: $("diary-clear-pw"),
  diaryLocked: $("diary-locked"),
  diaryUnlockPw: $("diary-unlock-pw"),
  diaryUnlock: $("diary-unlock"),
  diaryOpen: $("diary-open"),
  diarySearch: $("diary-search"),
  diary: $("diary"),
  diaryExport: $("diary-export"),
  clearDiary: $("clear-diary"),
  aboutVersion: $("about-version"),
  checkUpdate: $("check-update"),
  updateStatus: $("update-status"),
  installUpdate: $("install-update"),
  toast: $("toast"),
  status: $("status"),
};

// Guards form-change handlers from firing while we populate the form.
let ready = false;
let toastTimer = null;
let installedLayouts = []; // [{ klid, name }]
let layoutNames = { ...FALLBACK_LAYOUTS };
let currentHotkey = 0x13;
let currentMods = 0;
let currentSelectionHotkey = 0x91;
let currentSelectionMods = 0;
let currentUndoHotkey = 0;
let currentUndoMods = 0;
let capturing = false;
// Corrections as the engine reported them, kept raw so the list can be rebuilt
// in a new language instead of stranding old rows in the old one.
let historyEvents = [];
// This build's version, from the backend — kept so the About line can be
// re-rendered when the language changes.
let appVersion = "";

// Modifier bit values — must match Config::MOD_* in the core (= Win32 MOD_*).
const MOD = { ALT: 0x1, CTRL: 0x2, SHIFT: 0x4, WIN: 0x8 };

function layoutName(klid) {
  return layoutNames[klid] || t("unknownLayout", { klid });
}

function vkName(vk) {
  if (VK_NAMES[vk]) return VK_NAMES[vk];
  if (vk >= 0x70 && vk <= 0x87) return "F" + (vk - 0x70 + 1);
  if (vk >= 0x41 && vk <= 0x5a) return String.fromCharCode(vk); // A-Z
  if (vk >= 0x30 && vk <= 0x39) return String.fromCharCode(vk); // 0-9
  return "VK 0x" + vk.toString(16).toUpperCase();
}

// Build the "Ctrl+Shift+K" label from a modifier bitmask + main key. Modifier
// names stay in their Windows spelling on purpose — that is what is printed on
// the keys and what every other app calls them.
function hotkeyLabel(vk, mods) {
  if (!vk) return t("none");
  const parts = [];
  if (mods & MOD.CTRL) parts.push("Ctrl");
  if (mods & MOD.SHIFT) parts.push("Shift");
  if (mods & MOD.ALT) parts.push("Alt");
  if (mods & MOD.WIN) parts.push("Win");
  parts.push(vkName(vk));
  return parts.join("+");
}

function setStatus(text) {
  el.status.textContent = text;
}

function showToast(text) {
  el.toast.textContent = text;
  el.toast.classList.remove("hidden", "error");
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => el.toast.classList.add("hidden"), 2400);
}

// Failures get the toast too, not just the status bar — a rejected hotkey is
// something the user must act on, and the status line is easy to miss. Held
// longer because these messages explain what to try instead.
function showError(text) {
  setStatus(text);
  el.toast.textContent = text;
  el.toast.classList.remove("hidden");
  el.toast.classList.add("error");
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => el.toast.classList.add("hidden"), 7000);
}

// ----- Language --------------------------------------------------------------

// Ask the backend which language to render in. "system" is resolved there (from
// the Windows UI language) rather than here, so the tray menu and this window
// can never end up in different languages.
async function refreshLocale() {
  try {
    setLocale(await invoke("resolved_ui_language"));
  } catch (e) {
    setStatus(t("error", { error: e }));
  }
  retranslate();
}

// Re-render everything that carries text: the static markup, plus the parts
// built from data (hotkey labels, history rows, learned words).
function retranslate() {
  applyI18n();
  el.enabledLabel.textContent = el.enabled.checked ? t("on") : t("paused");
  renderHotkey();
  renderHistory();
  renderWords();
  renderDiary();
  renderAbout();
}

function renderAbout() {
  el.aboutVersion.textContent = appVersion ? t("aboutVersion", { version: appVersion }) : "—";
}

// ----- Tabs ------------------------------------------------------------------

// Exactly one tab and one pane carry `.active`; everything else is hidden, so
// the panes never fight for the scroll position.
function showPane(name) {
  for (const tab of el.tabs.querySelectorAll(".tab")) {
    tab.classList.toggle("active", tab.dataset.pane === name);
  }
  for (const pane of document.querySelectorAll(".pane")) {
    pane.classList.toggle("active", pane.dataset.pane === name);
  }
  document.querySelector("main").scrollTop = 0;
  // The diary is written by the backend in batches, not pushed per word, so it
  // is re-read (and its lock state re-checked) whenever its pane comes into view.
  if (name === "diary") refreshDiary();
}

// ----- Layout pair rows ------------------------------------------------------

function layoutSelect(selected) {
  const sel = document.createElement("select");
  const options = [...installedLayouts];
  if (selected && !options.some((o) => o.klid === selected)) {
    options.push({ klid: selected, name: t("notInstalled", { name: layoutName(selected) }) });
  }
  for (const { klid, name } of options) {
    const opt = document.createElement("option");
    opt.value = klid;
    opt.textContent = name;
    if (klid === selected) opt.selected = true;
    sel.appendChild(opt);
  }
  // Layout names can be far wider than the window; the box truncates them, so
  // carry the full name as a tooltip and keep it in step with the selection.
  const syncTitle = () => {
    sel.title = sel.selectedOptions[0]?.textContent || "";
  };
  syncTitle();
  sel.addEventListener("change", () => {
    syncTitle();
    apply();
  });
  return sel;
}

function addPairRow(pair) {
  const row = document.createElement("div");
  row.className = "pair";

  // Default a new row to installed layouts only — never a hardcoded dummy.
  const fallbackFrom = installedLayouts[0]?.klid || "";
  const fallbackTo = installedLayouts[1]?.klid || installedLayouts[0]?.klid || "";
  const from = layoutSelect(pair ? pair.from : fallbackFrom);
  const arrow = document.createElement("span");
  arrow.className = "arrow";
  arrow.textContent = "⇄";
  const to = layoutSelect(pair ? pair.to : fallbackTo);

  // Auto-detected by fingerprinting the layout at startup; shown so a
  // misdetection can be corrected, since it selects the whole decision path.
  const phonetic = document.createElement("label");
  phonetic.className = "check inline";
  phonetic.dataset.i18nTitle = "phoneticHint";
  phonetic.title = t("phoneticHint");
  const phoneticBox = document.createElement("input");
  phoneticBox.type = "checkbox";
  phoneticBox.className = "phonetic";
  phoneticBox.checked = pair ? pair.phonetic === true : false;
  phoneticBox.addEventListener("change", apply);
  const phoneticText = document.createElement("span");
  phoneticText.dataset.i18n = "phonetic";
  phoneticText.textContent = t("phonetic");
  phonetic.append(phoneticBox, phoneticText);

  const remove = document.createElement("button");
  remove.className = "icon";
  remove.type = "button";
  remove.textContent = "✕";
  remove.dataset.i18nTitle = "removePair";
  remove.title = t("removePair");
  remove.addEventListener("click", () => {
    row.remove();
    apply();
  });

  row.append(from, arrow, to, phonetic, remove);
  el.pairs.appendChild(row);
}

function readPairs() {
  return [...el.pairs.querySelectorAll(".pair")].map((row) => {
    const [from, to] = row.querySelectorAll("select");
    // Always send an explicit boolean: null would re-trigger auto-detection at
    // the next startup and overwrite what the user just chose.
    return { from: from.value, to: to.value, phonetic: row.querySelector(".phonetic").checked };
  });
}

// ----- Process picker --------------------------------------------------------

async function loadProcesses() {
  try {
    const procs = await invoke("list_processes");
    el.procList.innerHTML = "";
    for (const name of procs) {
      const opt = document.createElement("option");
      opt.value = name;
      opt.textContent = name;
      el.procList.appendChild(opt);
    }
  } catch (e) {
    setStatus(t("listAppsFailed", { error: e }));
  }
}

function ignoreList() {
  return el.ignore.value
    .split("\n")
    .map((s) => s.trim())
    .filter(Boolean);
}

function addIgnore(name) {
  if (!name) return;
  const existing = ignoreList();
  if (existing.some((s) => s.toLowerCase() === name.toLowerCase())) {
    setStatus(t("alreadyIgnored", { name }));
    return;
  }
  existing.push(name);
  el.ignore.value = existing.join("\n");
  apply();
}

// ----- Config <-> form -------------------------------------------------------

function renderHotkey() {
  el.hotkeyDisplay.textContent = hotkeyLabel(currentHotkey, currentMods);
  el.selectionHotkeyDisplay.textContent = hotkeyLabel(
    currentSelectionHotkey,
    currentSelectionMods,
  );
  el.undoHotkeyDisplay.textContent = hotkeyLabel(currentUndoHotkey, currentUndoMods);
}

function populate(config) {
  ready = false;
  el.enabled.checked = config.enabled;
  el.enabledLabel.textContent = config.enabled ? t("on") : t("paused");
  el.sensitivity.value = config.sensitivity;
  el.sensitivityValue.textContent = Number(config.sensitivity).toFixed(2);
  el.uiLanguage.value = config.ui_language || "system";
  currentHotkey = config.manual_hotkey;
  currentMods = config.manual_hotkey_mods || 0;
  currentSelectionHotkey = config.selection_hotkey || 0;
  currentSelectionMods = config.selection_hotkey_mods || 0;
  currentUndoHotkey = config.undo_hotkey || 0;
  currentUndoMods = config.undo_hotkey_mods || 0;
  renderHotkey();
  el.learnOnUndo.checked = config.learn_on_undo !== false;
  el.diaryEnabled.checked = config.diary_enabled === true;
  el.diaryClipboard.checked = config.diary_capture_clipboard === true;
  el.diarySkipShort.checked = config.diary_skip_short !== false;
  el.diaryMaxEntries.value = config.diary_max_entries || 500;
  el.diaryRetentionDays.value = config.diary_retention_days || 0;
  el.diaryMaxSizeMb.value = config.diary_max_size_mb || 0;
  el.ignore.value = (config.ignore_processes || []).join("\n");

  el.pairs.innerHTML = "";
  (config.layout_pairs || []).forEach(addPairRow);

  ready = true;
}

function gather() {
  return {
    enabled: el.enabled.checked,
    sensitivity: parseFloat(el.sensitivity.value),
    layout_pairs: readPairs(),
    ignore_processes: ignoreList(),
    manual_hotkey: currentHotkey,
    manual_hotkey_mods: currentMods,
    selection_hotkey: currentSelectionHotkey,
    selection_hotkey_mods: currentSelectionMods,
    undo_hotkey: currentUndoHotkey,
    undo_hotkey_mods: currentUndoMods,
    learn_on_undo: el.learnOnUndo.checked,
    diary_enabled: el.diaryEnabled.checked,
    diary_capture_clipboard: el.diaryClipboard.checked,
    diary_skip_short: el.diarySkipShort.checked,
    diary_max_entries: Math.min(10000, Math.max(1, parseInt(el.diaryMaxEntries.value, 10) || 500)),
    diary_retention_days: Math.min(36500, Math.max(0, parseInt(el.diaryRetentionDays.value, 10) || 0)),
    diary_max_size_mb: Math.min(1024, Math.max(0, parseInt(el.diaryMaxSizeMb.value, 10) || 0)),
    ui_language: el.uiLanguage.value,
  };
}

async function apply() {
  if (!ready) return;
  try {
    await invoke("set_config", { config: gather() });
    setStatus(t("saved"));
  } catch (e) {
    setStatus(t("error", { error: e }));
  }
}

// ----- Hotkey capture --------------------------------------------------------

function jsToVk(ev) {
  const map = {
    Pause: 0x13, ScrollLock: 0x91, NumLock: 0x90, PrintScreen: 0x2c,
    Insert: 0x2d, Delete: 0x2e, Home: 0x24, End: 0x23,
    PageUp: 0x21, PageDown: 0x22, ArrowLeft: 0x25, ArrowUp: 0x26,
    ArrowRight: 0x27, ArrowDown: 0x28, ContextMenu: 0x5d,
  };
  if (ev.code in map) return map[ev.code];
  const f = /^F(\d{1,2})$/.exec(ev.code || "");
  if (f) return 0x70 + (parseInt(f[1], 10) - 1);
  return ev.keyCode || 0;
}

// Keys that are modifiers on their own — while capturing we wait past these for
// the main key, so the user can build a combination like Ctrl+Shift+K.
const MODIFIER_KEYS = ["Control", "Shift", "Alt", "Meta", "AltGraph"];

function captureMods(ev) {
  let mods = 0;
  if (ev.ctrlKey) mods |= MOD.CTRL;
  if (ev.shiftKey) mods |= MOD.SHIFT;
  if (ev.altKey) mods |= MOD.ALT;
  if (ev.metaKey) mods |= MOD.WIN;
  return mods;
}

// Capture a key combination for one of the hotkeys. `opts` carries the button
// and display element to animate, the IPC command to set it, and an `onSet`
// callback to update the matching state on success.
function startCapture(opts) {
  if (capturing) return;
  capturing = true;
  opts.button.textContent = t("pressKey");
  opts.display.parentElement.classList.add("capturing");

  const finish = () => {
    clearTimeout(timeout);
    window.removeEventListener("keydown", handler, true);
    capturing = false;
    opts.button.textContent = t("setHotkey");
    opts.display.parentElement.classList.remove("capturing");
  };

  // Windows owns the reserved shortcuts (most Win+… combinations) as global
  // hotkeys, so it consumes them before this window ever sees a keydown. Without
  // a deadline the capture would sit in "Press a key…" forever, looking broken
  // rather than explaining itself.
  const timeout = setTimeout(() => {
    finish();
    showError(t("hotkeyTimeout"));
  }, 5000);

  const handler = async (ev) => {
    if (ev.key === "Escape") {
      ev.preventDefault();
      finish();
      setStatus(t("hotkeyUnchanged"));
      return;
    }
    // Ignore bare modifier presses — keep waiting for the main key.
    if (MODIFIER_KEYS.includes(ev.key)) return;

    ev.preventDefault();
    ev.stopPropagation();
    finish();

    const vk = jsToVk(ev);
    const mods = captureMods(ev);
    try {
      await invoke(opts.command, { vk, mods });
      opts.onSet(vk, mods);
      renderHotkey();
      setStatus(t("hotkeySet", { label: hotkeyLabel(vk, mods) }));
    } catch (e) {
      // Rejected — the binding is unchanged, so say why prominently rather than
      // leaving the user to wonder whether it took. The reason comes from the
      // backend already translated.
      showError(t("hotkeyRejected", { label: hotkeyLabel(vk, mods), error: e }));
    }
  };
  window.addEventListener("keydown", handler, true);
}

// ----- History ---------------------------------------------------------------

function addHistory(event) {
  historyEvents.unshift(event);
  while (historyEvents.length > 30) historyEvents.pop();
  renderHistory();
}

function renderHistory() {
  el.history.innerHTML = "";
  if (historyEvents.length === 0) {
    const li = document.createElement("li");
    li.className = "empty";
    li.dataset.i18n = "nothingYet";
    li.textContent = t("nothingYet");
    el.history.append(li);
    return;
  }

  for (const event of historyEvents) {
    const li = document.createElement("li");
    if (event.kind === "undone") {
      const restored = document.createElement("span");
      restored.className = "to";
      restored.textContent = event.original;
      const note = document.createElement("span");
      note.className = "tag";
      note.textContent = t("tagUndone");
      li.append(restored, note);
    } else {
      const from = document.createElement("span");
      from.className = "from";
      from.textContent = event.original;
      const arrow = document.createElement("span");
      arrow.textContent = "→";
      const to = document.createElement("span");
      to.className = "to";
      to.textContent = event.corrected;
      const tag = document.createElement("span");
      tag.className = "tag";
      tag.textContent = event.manual ? t("tagManual") : layoutName(event.target_layout);
      li.append(from, arrow, to, tag);
    }
    el.history.append(li);
  }
}

// ----- Learned words ---------------------------------------------------------

// The dictionary is the only typed text Palimpsest persists, so the list is
// always fully shown and every entry is individually removable.
let learnedWords = {};

async function loadWords() {
  try {
    learnedWords = await invoke("list_user_words");
  } catch (e) {
    setStatus(t("loadWordsFailed", { error: e }));
    return;
  }
  renderWords();
}

function renderWords() {
  el.words.innerHTML = "";
  const entries = ["en", "de", "ru"].flatMap((lang) =>
    (learnedWords[lang] || []).map((word) => ({ lang, word })),
  );
  if (entries.length === 0) {
    const li = document.createElement("li");
    li.className = "empty";
    li.dataset.i18n = "nothingLearned";
    li.textContent = t("nothingLearned");
    el.words.append(li);
    return;
  }

  for (const { lang, word } of entries) {
    const li = document.createElement("li");
    const text = document.createElement("span");
    text.className = "to";
    text.textContent = word;
    const tag = document.createElement("span");
    tag.className = "tag";
    tag.textContent = layoutNameForLang(lang);
    const forget = document.createElement("button");
    forget.className = "ghost";
    forget.type = "button";
    forget.dataset.i18n = "forget";
    forget.textContent = t("forget");
    forget.addEventListener("click", async () => {
      try {
        await invoke("forget_word", { lang, word });
        await loadWords();
        setStatus(t("forgotWord", { word }));
      } catch (e) {
        setStatus(t("error", { error: e }));
      }
    });
    li.append(text, tag, forget);
    el.words.append(li);
  }
}

// Language tag for a learned word. Falls back to the bare code so an unknown
// language never renders as blank.
function layoutNameForLang(lang) {
  const klid = { en: "00000409", ru: "00000419", de: "00000407" }[lang];
  return klid ? layoutName(klid) : lang;
}

// ----- Typing diary ----------------------------------------------------------

// The opt-in keystroke log — the most sensitive thing the app can hold. It gets
// its own tab, shown in full (newest first, grouped by day, searchable), can be
// exported and erased, and can be encrypted behind a password that gates both
// viewing and recording.
let diaryEntries = [];
let diaryStatus = { enabled: false, protected: false, unlocked: false, count: 0 };

// Pull the diary's lock state, then its entries if we're allowed to see them,
// and lay the pane out accordingly. Called whenever the pane opens or its state
// could have changed (toggled recording, set/cleared password, unlocked).
async function refreshDiary() {
  try {
    diaryStatus = await invoke("diary_status");
  } catch (e) {
    setStatus(t("error", { error: e }));
    return;
  }

  const locked = diaryStatus.protected && !diaryStatus.unlocked;
  el.diaryLocked.hidden = !locked;
  el.diaryOpen.hidden = locked;
  // Protection card: offer "set" when open, "remove" when a password exists.
  el.diaryProtectionState.textContent = diaryStatus.protected
    ? t("diaryProtected")
    : t("diaryUnprotected");
  el.diarySetPwRow.hidden = diaryStatus.protected && diaryStatus.unlocked === false;
  el.diaryClearPw.hidden = !(diaryStatus.protected && diaryStatus.unlocked);
  el.diarySetPw.textContent = diaryStatus.protected ? t("diaryChangePw") : t("diarySetPw");

  if (locked) {
    diaryEntries = [];
    return;
  }
  try {
    const diary = await invoke("list_diary");
    diaryEntries = diary.entries || [];
  } catch (e) {
    setStatus(t("loadDiaryFailed", { error: e }));
    return;
  }
  renderDiary();
}

async function unlockDiary() {
  const pw = el.diaryUnlockPw.value;
  if (!pw) return;
  try {
    await invoke("unlock_diary", { password: pw });
    el.diaryUnlockPw.value = "";
    await refreshDiary();
    setStatus(t("diaryUnlocked"));
  } catch (e) {
    showError(String(e));
  }
}

// Unix seconds → the user's locale time. Falls back to the raw stamp if the
// runtime dislikes the value, rather than rendering "Invalid Date".
function stampTime(secs) {
  const d = new Date(secs * 1000);
  return isNaN(d) ? String(secs) : d.toLocaleTimeString(LOCALE);
}

// The day an entry belongs to, for the date headers.
function stampDay(secs) {
  const d = new Date(secs * 1000);
  return isNaN(d) ? String(secs) : d.toLocaleDateString(LOCALE, {
    weekday: "short", year: "numeric", month: "short", day: "numeric",
  });
}

// Copy to the clipboard. `navigator.clipboard` works in the webview's secure
// context; the execCommand path is a fallback for anything that refuses it.
async function copyText(text) {
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    const ta = document.createElement("textarea");
    ta.value = text;
    document.body.append(ta);
    ta.select();
    try { document.execCommand("copy"); } catch {}
    ta.remove();
  }
  showToast(t("copied"));
}

function renderDiary() {
  el.diary.innerHTML = "";

  const term = (el.diarySearch.value || "").trim().toLowerCase();
  const matches = diaryEntries.filter(
    (e) => !term || e.text.toLowerCase().includes(term) || (e.app || "").toLowerCase().includes(term),
  );

  if (matches.length === 0) {
    const li = document.createElement("li");
    li.className = "empty";
    // Distinguish off / on-but-empty / no-search-match so an empty list never
    // reads as a broken feature.
    const key = term ? "diaryNoMatch" : diaryStatus.enabled ? "diaryEmpty" : "diaryOff";
    li.textContent = t(key);
    el.diary.append(li);
    return;
  }

  // Newest first, with a header each time the day changes.
  let lastDay = null;
  for (const entry of [...matches].reverse()) {
    const day = stampDay(entry.at);
    if (day !== lastDay) {
      lastDay = day;
      const head = document.createElement("li");
      head.className = "date-head";
      head.textContent = day;
      el.diary.append(head);
    }

    const li = document.createElement("li");
    const rowHead = document.createElement("div");
    rowHead.className = "row-head";
    const meta = document.createElement("span");
    meta.className = "meta";
    const when = document.createElement("span");
    when.textContent = stampTime(entry.at);
    const app = document.createElement("span");
    app.className = "app";
    app.textContent = entry.app || t("unknownApp");
    meta.append(when, app);
    if (entry.clip) {
      const tag = document.createElement("span");
      tag.className = "clip-tag";
      tag.textContent = t("diaryClipTag");
      meta.append(tag);
    }
    const copy = document.createElement("button");
    copy.className = "ghost copy";
    copy.type = "button";
    copy.textContent = t("copy");
    copy.addEventListener("click", () => copyText(entry.text));
    rowHead.append(meta, copy);

    const text = document.createElement("span");
    text.className = "text";
    text.textContent = entry.text;
    li.append(rowHead, text);
    el.diary.append(li);
  }
}

// ----- Wiring ----------------------------------------------------------------

function wire() {
  el.tabs.addEventListener("click", (ev) => {
    const tab = ev.target.closest(".tab");
    if (tab) showPane(tab.dataset.pane);
  });

  el.enabled.addEventListener("change", async () => {
    if (!ready) return;
    el.enabledLabel.textContent = el.enabled.checked ? t("on") : t("paused");
    try {
      await invoke("set_enabled", { enabled: el.enabled.checked });
    } catch (e) {
      setStatus(t("error", { error: e }));
    }
  });

  el.sensitivity.addEventListener("input", () => {
    el.sensitivityValue.textContent = Number(el.sensitivity.value).toFixed(2);
  });
  el.sensitivity.addEventListener("change", apply);
  el.ignore.addEventListener("change", apply);
  el.addPair.addEventListener("click", () => {
    addPairRow(null);
    apply();
  });

  el.addProc.addEventListener("click", () => addIgnore(el.procList.value));
  el.refreshProc.addEventListener("click", loadProcesses);

  // Saving first, then re-reading the resolved language, is what makes "system"
  // work: only the backend knows what Windows reports.
  el.uiLanguage.addEventListener("change", async () => {
    await apply();
    await refreshLocale();
  });

  el.setHotkey.addEventListener("click", () =>
    startCapture({
      button: el.setHotkey,
      display: el.hotkeyDisplay,
      command: "set_hotkey",
      onSet: (vk, mods) => {
        currentHotkey = vk;
        currentMods = mods;
      },
    }),
  );
  el.clearHotkey.addEventListener("click", async () => {
    try {
      await invoke("clear_hotkey");
      currentHotkey = 0;
      currentMods = 0;
      renderHotkey();
      setStatus(t("hotkeyCleared"));
    } catch (e) {
      setStatus(t("error", { error: e }));
    }
  });

  el.setSelectionHotkey.addEventListener("click", () =>
    startCapture({
      button: el.setSelectionHotkey,
      display: el.selectionHotkeyDisplay,
      command: "set_selection_hotkey",
      onSet: (vk, mods) => {
        currentSelectionHotkey = vk;
        currentSelectionMods = mods;
      },
    }),
  );
  el.clearSelectionHotkey.addEventListener("click", async () => {
    try {
      await invoke("clear_selection_hotkey");
      currentSelectionHotkey = 0;
      currentSelectionMods = 0;
      renderHotkey();
      setStatus(t("selectionHotkeyCleared"));
    } catch (e) {
      setStatus(t("error", { error: e }));
    }
  });

  el.setUndoHotkey.addEventListener("click", () =>
    startCapture({
      button: el.setUndoHotkey,
      display: el.undoHotkeyDisplay,
      command: "set_undo_hotkey",
      onSet: (vk, mods) => {
        currentUndoHotkey = vk;
        currentUndoMods = mods;
      },
    }),
  );
  el.clearUndoHotkey.addEventListener("click", async () => {
    try {
      await invoke("clear_undo_hotkey");
      currentUndoHotkey = 0;
      currentUndoMods = 0;
      renderHotkey();
      setStatus(t("undoHotkeyCleared"));
    } catch (e) {
      setStatus(t("error", { error: e }));
    }
  });

  el.autostart.addEventListener("change", async () => {
    if (!ready) return;
    try {
      await invoke("set_autostart", { enabled: el.autostart.checked });
      setStatus(el.autostart.checked ? t("willLaunch") : t("wontLaunch"));
    } catch (e) {
      el.autostart.checked = !el.autostart.checked; // revert on failure
      setStatus(t("error", { error: e }));
    }
  });

  // Always-elevated mode triggers a UAC prompt when its scheduled task is
  // created/removed. Revert the checkbox if that fails or is cancelled.
  el.elevatedAutostart.addEventListener("change", async () => {
    if (!ready) return;
    const want = el.elevatedAutostart.checked;
    try {
      await invoke("set_elevated_autostart", { enabled: want });
      // The backend turns off the normal Run entry when enabling, so mirror that
      // and lock it out while elevated autostart owns startup.
      if (want) el.autostart.checked = false;
      el.autostart.disabled = want;
      setStatus(want ? t("elevatedAutostartOn") : t("elevatedAutostartOff"));
    } catch (e) {
      el.elevatedAutostart.checked = !want; // revert (incl. cancelled UAC)
      showError(String(e));
    }
  });

  el.undo.addEventListener("click", async () => {
    try {
      await invoke("undo");
    } catch (e) {
      setStatus(t("error", { error: e }));
    }
  });

  el.openLogs.addEventListener("click", async () => {
    try {
      await invoke("open_logs");
    } catch (e) {
      setStatus(t("error", { error: e }));
    }
  });

  el.learnOnUndo.addEventListener("change", apply);

  el.clearWords.addEventListener("click", async () => {
    try {
      await invoke("clear_user_words");
      await loadWords();
      setStatus(t("forgotAll"));
    } catch (e) {
      setStatus(t("error", { error: e }));
    }
  });

  // Turning the diary on is a privacy decision, so it is confirmed rather than
  // taken on a stray click; turning it off never is.
  el.diaryEnabled.addEventListener("change", async () => {
    if (el.diaryEnabled.checked && !confirm(t("diaryConfirm"))) {
      el.diaryEnabled.checked = false;
      return;
    }
    await apply();
    await refreshDiary();
  });

  el.diaryClipboard.addEventListener("change", apply);
  el.diarySkipShort.addEventListener("change", apply);
  el.diaryMaxEntries.addEventListener("change", apply);
  el.diaryRetentionDays.addEventListener("change", apply);
  el.diaryMaxSizeMb.addEventListener("change", apply);
  el.diarySearch.addEventListener("input", renderDiary);

  el.diarySetPw.addEventListener("click", async () => {
    const pw = el.diaryPw.value;
    if (!pw) {
      showError(t("diaryPwEmpty"));
      return;
    }
    if (pw !== el.diaryPw2.value) {
      showError(t("diaryPwMismatch"));
      return;
    }
    if (!confirm(t("diaryPwConfirm"))) return;
    try {
      await invoke("set_diary_password", { password: pw });
      el.diaryPw.value = "";
      el.diaryPw2.value = "";
      await refreshDiary();
      setStatus(t("diaryPwSet"));
    } catch (e) {
      showError(String(e));
    }
  });

  el.diaryClearPw.addEventListener("click", async () => {
    if (!confirm(t("diaryClearPwConfirm"))) return;
    try {
      await invoke("clear_diary_password");
      await refreshDiary();
      setStatus(t("diaryPwRemoved"));
    } catch (e) {
      showError(String(e));
    }
  });

  el.diaryUnlock.addEventListener("click", unlockDiary);
  el.diaryUnlockPw.addEventListener("keydown", (ev) => {
    if (ev.key === "Enter") unlockDiary();
  });

  el.diaryExport.addEventListener("click", async () => {
    try {
      const path = await invoke("export_diary");
      showToast(t("diaryExported"));
      setStatus(t("diaryExportedTo", { path }));
    } catch (e) {
      showError(String(e));
    }
  });

  el.clearDiary.addEventListener("click", async () => {
    if (!confirm(t("diaryClearConfirm"))) return;
    try {
      await invoke("clear_diary");
      await refreshDiary();
      setStatus(t("diaryCleared"));
    } catch (e) {
      setStatus(t("error", { error: e }));
    }
  });

  el.checkUpdate.addEventListener("click", () => checkForUpdate(false));

  el.installUpdate.addEventListener("click", async () => {
    el.installUpdate.disabled = true;
    el.updateStatus.textContent = t("installing");
    try {
      await invoke("install_update");
    } catch (e) {
      // Success never returns — the installer needs this process gone, so the
      // app exits mid-call. Reaching here means it failed.
      el.installUpdate.disabled = false;
      showError(t("updateFailed", { error: e }));
    }
  });

  // Engine pushed a correction, an undo, or a word it learned from one.
  listen("engine-event", (e) => {
    const event = e.payload;
    if (event.kind === "learned") {
      // Learning is silent by design — no prompt steals focus mid-typing — so
      // the list and a toast are how the user finds out it happened.
      loadWords();
      showToast(t("learnedToast", { word: event.word }));
      return;
    }
    addHistory(event);
    if (event.kind === "undone") {
      showToast(t("revertedToast", { text: event.original }));
    } else {
      showToast(`${event.original} → ${event.corrected}`);
    }
  });

  // Master switch was toggled elsewhere (tray); reflect it without re-saving.
  listen("status", (e) => {
    ready = false;
    el.enabled.checked = e.payload.enabled;
    el.enabledLabel.textContent = e.payload.enabled ? t("on") : t("paused");
    ready = true;
  });
}

async function init() {
  // Language first: everything below reports through `t()`, so an error during
  // startup must already be in the user's language.
  await refreshLocale();
  wire();
  try {
    installedLayouts = await invoke("list_layouts");
    for (const { klid, name } of installedLayouts) layoutNames[klid] = name;
  } catch (e) {
    setStatus(t("listLayoutsFailed", { error: e }));
  }
  await loadProcesses();
  await loadWords();
  try {
    const config = await invoke("get_config");
    populate(config);
    el.autostart.checked = await invoke("get_autostart");
    el.elevatedAutostart.checked = await invoke("get_elevated_autostart");
    // While elevated autostart owns startup, the normal one is redundant.
    el.autostart.disabled = el.elevatedAutostart.checked;
    setStatus(t("ready"));
  } catch (e) {
    setStatus(t("loadFailed", { error: e }));
  }
  try {
    appVersion = await invoke("app_version");
    renderAbout();
  } catch (e) {
    setStatus(t("error", { error: e }));
  }
  await warnAboutHotkeyConflicts();
  checkForUpdate(true);
}

// Ask the release feed for a newer build. `quiet` marks the automatic check when
// the window opens: a machine that is offline or behind a proxy must not be
// greeted with an error toast about something the user never asked for. The
// "Check now" button passes false, because then a failure is the answer.
async function checkForUpdate(quiet) {
  el.updateStatus.textContent = t("checking");
  el.installUpdate.hidden = true;
  try {
    const { current, available, notes } = await invoke("check_update");
    if (!available) {
      el.updateStatus.textContent = t("upToDate", { version: current });
      return;
    }
    el.updateStatus.textContent =
      t("updateAvailable", { available, current }) + (notes ? ` ${notes}` : "");
    el.installUpdate.hidden = false;
  } catch (e) {
    el.updateStatus.textContent = t("updateUnreachable");
    if (!quiet) showError(t("updateCheckFailed", { error: e }));
  }
}

// A hotkey that validated when it was set can be claimed by something else
// later. Palimpsest still fires (the hook sees keys first), but so does the
// other owner — so surface it instead of letting the user puzzle over why
// pressing it also does something unrelated.
async function warnAboutHotkeyConflicts() {
  try {
    const conflicts = await invoke("hotkey_conflicts");
    if (conflicts.length === 0) return;
    const label = {
      manual: t("conflictManual"),
      selection: t("conflictSelection"),
      undo: t("conflictUndo"),
    };
    showError(
      conflicts.map((c) => `${label[c.which] || c.which}: ${c.message}`).join(" "),
    );
  } catch (e) {
    setStatus(t("hotkeyCheckFailed", { error: e }));
  }
}

window.addEventListener("DOMContentLoaded", init);
