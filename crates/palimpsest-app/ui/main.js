// Palimpsest settings UI. Uses the global Tauri API (`withGlobalTauri`), so no
// bundler is required: this file is served straight from the embedded `ui/`
// directory. It reads/writes config through IPC commands and listens for engine
// events to drive the toast and history.

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
  0x2d: "Insert", 0x2e: "Delete", 0x24: "Home", 0x23: "End",
  0x21: "PageUp", 0x22: "PageDown", 0x25: "Left", 0x26: "Up",
  0x27: "Right", 0x28: "Down", 0x5d: "Menu", 0x1b: "Esc",
};

const $ = (id) => document.getElementById(id);
const el = {
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
  hotkeyDisplay: $("hotkey-display"),
  setHotkey: $("set-hotkey"),
  clearHotkey: $("clear-hotkey"),
  autostart: $("autostart"),
  undo: $("undo"),
  openLogs: $("open-logs"),
  history: $("history"),
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
let capturing = false;

// Modifier bit values — must match Config::MOD_* in the core (= Win32 MOD_*).
const MOD = { ALT: 0x1, CTRL: 0x2, SHIFT: 0x4, WIN: 0x8 };

function layoutName(klid) {
  return layoutNames[klid] || `Unknown (${klid})`;
}

function vkName(vk) {
  if (VK_NAMES[vk]) return VK_NAMES[vk];
  if (vk >= 0x70 && vk <= 0x87) return "F" + (vk - 0x70 + 1);
  if (vk >= 0x41 && vk <= 0x5a) return String.fromCharCode(vk); // A-Z
  if (vk >= 0x30 && vk <= 0x39) return String.fromCharCode(vk); // 0-9
  return "VK 0x" + vk.toString(16).toUpperCase();
}

// Build the "Ctrl+Shift+K" label from a modifier bitmask + main key.
function hotkeyLabel(vk, mods) {
  if (!vk) return "None";
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
  el.toast.classList.remove("hidden");
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => el.toast.classList.add("hidden"), 2400);
}

// ----- Layout pair rows ------------------------------------------------------

function layoutSelect(selected) {
  const sel = document.createElement("select");
  const options = [...installedLayouts];
  if (selected && !options.some((o) => o.klid === selected)) {
    options.push({ klid: selected, name: `${layoutName(selected)} (not installed)` });
  }
  for (const { klid, name } of options) {
    const opt = document.createElement("option");
    opt.value = klid;
    opt.textContent = name;
    if (klid === selected) opt.selected = true;
    sel.appendChild(opt);
  }
  sel.addEventListener("change", apply);
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

  const remove = document.createElement("button");
  remove.className = "icon";
  remove.type = "button";
  remove.textContent = "✕";
  remove.title = "Remove pair";
  remove.addEventListener("click", () => {
    row.remove();
    apply();
  });

  row.append(from, arrow, to, remove);
  el.pairs.appendChild(row);
}

function readPairs() {
  return [...el.pairs.querySelectorAll(".pair")].map((row) => {
    const [from, to] = row.querySelectorAll("select");
    return { from: from.value, to: to.value };
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
    setStatus(`Couldn't list apps: ${e}`);
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
    setStatus(`${name} is already ignored.`);
    return;
  }
  existing.push(name);
  el.ignore.value = existing.join("\n");
  apply();
}

// ----- Config <-> form -------------------------------------------------------

function renderHotkey() {
  el.hotkeyDisplay.textContent = hotkeyLabel(currentHotkey, currentMods);
}

function populate(config) {
  ready = false;
  el.enabled.checked = config.enabled;
  el.enabledLabel.textContent = config.enabled ? "On" : "Paused";
  el.sensitivity.value = config.sensitivity;
  el.sensitivityValue.textContent = Number(config.sensitivity).toFixed(2);
  currentHotkey = config.manual_hotkey;
  currentMods = config.manual_hotkey_mods || 0;
  renderHotkey();
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
  };
}

async function apply() {
  if (!ready) return;
  try {
    await invoke("set_config", { config: gather() });
    setStatus("Saved.");
  } catch (e) {
    setStatus(`Error: ${e}`);
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

function startCapture() {
  if (capturing) return;
  capturing = true;
  el.setHotkey.textContent = "Press a key…";
  el.hotkeyDisplay.parentElement.classList.add("capturing");

  const finish = () => {
    window.removeEventListener("keydown", handler, true);
    capturing = false;
    el.setHotkey.textContent = "Set hotkey";
    el.hotkeyDisplay.parentElement.classList.remove("capturing");
  };

  const handler = async (ev) => {
    if (ev.key === "Escape") {
      ev.preventDefault();
      finish();
      setStatus("Hotkey unchanged.");
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
      await invoke("set_hotkey", { vk, mods });
      currentHotkey = vk;
      currentMods = mods;
      renderHotkey();
      setStatus(`Hotkey set to ${hotkeyLabel(vk, mods)}.`);
    } catch (e) {
      setStatus(`${e}`);
    }
  };
  window.addEventListener("keydown", handler, true);
}

// ----- History ---------------------------------------------------------------

function addHistory(event) {
  const empty = el.history.querySelector(".empty");
  if (empty) empty.remove();

  const li = document.createElement("li");
  if (event.kind === "undone") {
    const restored = document.createElement("span");
    restored.className = "to";
    restored.textContent = event.original;
    const note = document.createElement("span");
    note.className = "tag";
    note.textContent = "undone";
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
    tag.textContent = event.manual ? "manual" : layoutName(event.target_layout);
    li.append(from, arrow, to, tag);
  }

  el.history.prepend(li);
  while (el.history.children.length > 30) el.history.lastChild.remove();
}

// ----- Wiring ----------------------------------------------------------------

function wire() {
  el.enabled.addEventListener("change", async () => {
    if (!ready) return;
    el.enabledLabel.textContent = el.enabled.checked ? "On" : "Paused";
    try {
      await invoke("set_enabled", { enabled: el.enabled.checked });
    } catch (e) {
      setStatus(`Error: ${e}`);
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
  el.setHotkey.addEventListener("click", startCapture);
  el.clearHotkey.addEventListener("click", async () => {
    try {
      await invoke("clear_hotkey");
      currentHotkey = 0;
      currentMods = 0;
      renderHotkey();
      setStatus("Hotkey cleared.");
    } catch (e) {
      setStatus(`Error: ${e}`);
    }
  });

  el.autostart.addEventListener("change", async () => {
    if (!ready) return;
    try {
      await invoke("set_autostart", { enabled: el.autostart.checked });
      setStatus(el.autostart.checked ? "Will launch at login." : "Won't launch at login.");
    } catch (e) {
      el.autostart.checked = !el.autostart.checked; // revert on failure
      setStatus(`Error: ${e}`);
    }
  });

  el.undo.addEventListener("click", async () => {
    try {
      await invoke("undo");
    } catch (e) {
      setStatus(`Error: ${e}`);
    }
  });

  el.openLogs.addEventListener("click", async () => {
    try {
      await invoke("open_logs");
    } catch (e) {
      setStatus(`Error: ${e}`);
    }
  });

  // Engine pushed a correction or an undo.
  listen("engine-event", (e) => {
    const event = e.payload;
    addHistory(event);
    if (event.kind === "undone") {
      showToast(`Reverted → ${event.original}`);
    } else {
      showToast(`${event.original} → ${event.corrected}`);
    }
  });

  // Master switch was toggled elsewhere (tray); reflect it without re-saving.
  listen("status", (e) => {
    ready = false;
    el.enabled.checked = e.payload.enabled;
    el.enabledLabel.textContent = e.payload.enabled ? "On" : "Paused";
    ready = true;
  });
}

async function init() {
  wire();
  try {
    installedLayouts = await invoke("list_layouts");
    for (const { klid, name } of installedLayouts) layoutNames[klid] = name;
  } catch (e) {
    setStatus(`Couldn't list layouts: ${e}`);
  }
  await loadProcesses();
  try {
    const config = await invoke("get_config");
    populate(config);
    el.autostart.checked = await invoke("get_autostart");
    setStatus("Ready.");
  } catch (e) {
    setStatus(`Failed to load: ${e}`);
  }
}

window.addEventListener("DOMContentLoaded", init);
