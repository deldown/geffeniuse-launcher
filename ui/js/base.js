"use strict";
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

// No innerHTML anywhere: all text goes through text nodes.
const TRANSLATED_ATTRS = new Set(["placeholder", "title", "aria-label", "alt"]);
function h(tag, props, ...kids) {
  const el = document.createElement(tag);
  for (const [k, v] of Object.entries(props || {})) {
    if (v == null || v === false) continue;
    if (k.startsWith("on")) el.addEventListener(k.slice(2), v);
    else if (k === "class") el.className = v;
    else if (k === "checked" || k === "value" || k === "disabled" || k === "hidden") el[k] = v;
    else el.setAttribute(k, v === true ? "" : TRANSLATED_ATTRS.has(k) ? tr(String(v)) : v);
  }
  for (const kid of kids.flat()) if (kid != null && kid !== false) el.append(kid.nodeType ? kid : document.createTextNode(tr(String(kid))));
  return el;
}
const $ = (s) => document.querySelector(s);

// Custom protocol: geffeniuse://icon/... on Linux, http://geffeniuse.localhost/icon/... on Windows (WebView2).
const IS_WINDOWS = /Windows/i.test(navigator.userAgent);
const pUrl = (path) => (IS_WINDOWS ? "http://geffeniuse.localhost/" : "geffeniuse://") + path;

const ICONS = {
  home: ["M3 11l9-8 9 8", "M5 10v10h5v-6h4v6h5V10"],
  library: ["M4 4h7v7H4z", "M13 4h7v7h-7z", "M4 13h7v7H4z", "M13 13h7v7h-7z"],
  discover: ["M11 4a7 7 0 1 0 0 14 7 7 0 0 0 0-14z", "M20 20l-4.2-4.2"],
  shots: ["M3 8h4l2-3h6l2 3h4v11H3z", "M12 17a3.5 3.5 0 1 0 0-7 3.5 3.5 0 0 0 0 7z"],
  console: ["M3 5h18v14H3z", "M7 10l3 2-3 2", "M12.5 15h4.5"],
  worlds: ["M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18z", "M3 12h18", "M12 3c3.2 3 3.2 15 0 18", "M12 3c-3.2 3-3.2 15 0 18"],
  plus: ["M12 5v14", "M5 12h14"],
  download: ["M12 4v11", "M7 11l5 5 5-5", "M5 20h14"],
  heart: ["M12 20s-7-4.4-7-10a4 4 0 0 1 7-2.6A4 4 0 0 1 19 10c0 5.6-7 10-7 10z"],
  check: ["M5 12.5l4.5 4.5L19 7.5"],
  play: ["M8 5.5v13l11-6.5z"],
  server: ["M4 4h16v6H4z", "M4 14h16v6H4z", "M8 7h.01", "M8 17h.01"],
  close: ["M6 6l12 12", "M18 6L6 18"],
  edit: ["M12 20h9", "M16.5 3.5a2.12 2.12 0 0 1 3 3L7 19l-4 1 1-4z"],
  settings: ["M4 7h8", "M18 7h2", "M15 4a3 3 0 1 0 0 6 3 3 0 0 0 0-6z", "M4 17h2", "M12 17h8", "M9 14a3 3 0 1 0 0 6 3 3 0 0 0 0-6z"],
};
function icon(name) {
  const NS = "http://www.w3.org/2000/svg";
  const svg = document.createElementNS(NS, "svg");
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("class", "ic");
  svg.setAttribute("aria-hidden", "true");
  for (const d of ICONS[name]) {
    const path = document.createElementNS(NS, "path");
    path.setAttribute("d", d);
    svg.append(path);
  }
  return svg;
}

const state = { settings: null, route: "home", identified: new Set(), instances: [], busy: new Set(), running: new Set(), progress: null };

function toast(msg, error = false) {
  const t = h("div", { class: "toast" + (error ? " error" : "") }, msg);
  $("#toast").append(t);
  setTimeout(() => { t.classList.add("out"); setTimeout(() => t.remove(), 200); }, error ? 7000 : 3500);
}
async function call(cmd, args) {
  try { return await invoke(cmd, args); }
  catch (e) { toast(String(e), true); throw e; }
}
// Closing fades out first. A dialog opened during the fade cancels the cleanup (modalGen).
let modalGen = 0, modalOnClose = null;
const fireModalClose = () => { const cb = modalOnClose; modalOnClose = null; cb?.(); };
function closeModal() {
  const m = $("#modal");
  if (m.hidden) return;
  modalGen++;
  fireModalClose();
  m.hidden = true; m.replaceChildren(); m.classList.remove("leaving");
}
function openModal(...kids) {
  modalGen++;
  fireModalClose();
  const m = $("#modal"); m.classList.remove("leaving"); m.replaceChildren(...kids); m.hidden = false;
  // Every dialog closes with an X in its corner instead of a "Cancel" button.
  for (const d of m.querySelectorAll(".dialog")) {
    for (const b of d.querySelectorAll("button")) if (["Abbrechen", "Cancel"].includes(b.textContent.trim())) b.remove();
    if (!d.querySelector(".dlg-x")) d.prepend(h("button", { class: "dlg-x ghost", title: "Schließen", "aria-label": "Schließen", onclick: closeModal }, icon("close")));
  }
  m.onclick = (e) => { if (e.target === m) closeModal(); };
}
document.addEventListener("keydown", (e) => { if (e.key === "Escape") closeModal(); });
// Firefox-style navigation on file drop must never happen inside the app.
for (const ev of ["dragover", "drop"]) window.addEventListener(ev, (e) => e.preventDefault());

function applySettings() {
  const s = state.settings;
  LANG = s.language === "en" ? "en" : "de";
  document.documentElement.lang = LANG;
  $("#nav").setAttribute("aria-label", tr("Hauptmenü"));
  document.documentElement.dataset.motion = s.animations ? "on" : "off";
  document.documentElement.style.setProperty("--px", s.pixel_scale + "px");
  $("#view").classList.toggle("list-compact", s.compact_lists);
  applyTheme();
}
async function saveSettings(patch) {
  state.settings = await call("save_settings", { settings: { ...state.settings, ...patch } });
  applySettings();
  if ("modules" in patch) renderNav();
}

// Instances are served stale-while-revalidate, so a page never waits for the disk.
let instCache = null, instRequest = null, instGeneration = 0, instCheckedAt = 0;
const INSTANCE_CACHE_MS = 2000;
async function readInstances(background = false) {
  if (instRequest) return instRequest;
  const generation = instGeneration;
  instCheckedAt = Date.now();
  const request = (background ? invoke : call)("list_instances").then((fresh) => {
    if (generation === instGeneration) {
      instCache = fresh;
      instCheckedAt = Date.now();
    }
    return fresh;
  }).finally(() => {
    if (instRequest === request) instRequest = null;
  });
  instRequest = request;
  return request;
}
async function listInstances() {
  if (instCache !== null) {
    if (Date.now() - instCheckedAt >= INSTANCE_CACHE_MS) revalidateInstances();
    return instCache;
  }
  return readInstances();
}
async function revalidateInstances() {
  if (instRequest) return;
  const generation = instGeneration, previous = instCache;
  try {
    const fresh = await readInstances(true);
    if (generation === instGeneration && JSON.stringify(fresh) !== JSON.stringify(previous)
        && ["home", "library", "instance"].includes(state.route)) render();
  } catch { /* keep the cached list until the next successful read */ }
}
// A mutation invalidates pending reads as well as the saved list.
function refresh() {
  instGeneration++;
  instCache = null;
  instRequest = null;
  instCheckedAt = 0;
  return render();
}

let versionCache = null, accountsCache = null, accountsRevalidating = false;
async function appVersion() { return versionCache ??= await call("app_version"); }
// Same idea for the account list: show it at once, correct it if the file changed meanwhile.
async function listAccounts() {
  if (accountsCache) {
    if (!accountsRevalidating) {
      accountsRevalidating = true;
      invoke("list_accounts").then((fresh) => {
        if (JSON.stringify(fresh) !== JSON.stringify(accountsCache)) { accountsCache = fresh; if (state.route === "settings") render(); }
      }).catch(() => {}).finally(() => { accountsRevalidating = false; });
    }
    return accountsCache;
  }
  return (accountsCache = await call("list_accounts"));
}
