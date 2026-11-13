"use strict";
async function loadPlayer() { state.player = await call("player_info"); }
// The skin comes from Mojang in the background; the page only redraws when something changed.
async function refreshSkin() {
  if (!state.player?.signed_in) return;
  const asked = state.settings.active_account;
  let p;
  try { p = await invoke("refresh_skin"); } catch { return; }
  if (state.settings.active_account !== asked) return;
  const old = state.player || {};
  if (p.skin === old.skin && p.name === old.name && p.skin_error === old.skin_error && p.signed_in === old.signed_in) return;
  state.player = p;
  updateAccountButton();
  if (state.route === "home") patchHome();
}
// Decoded skins are kept, so a head is drawn in the same frame it is created.
const skinImages = new Map();
function skinImage(url) {
  const key = url || DEFAULT_SKIN;
  let img = skinImages.get(key);
  if (!img) {
    img = new Image();
    img.onerror = () => { if (key !== DEFAULT_SKIN) img.src = DEFAULT_SKIN; };
    img.src = key;
    skinImages.set(key, img);
    if (skinImages.size > 12) skinImages.delete(skinImages.keys().next().value);
  }
  return img;
}
function headCanvas(url) {
  const c = h("canvas", { class: "avatar", width: 8, height: 8, "aria-hidden": "true" });
  const ctx = c.getContext("2d");
  const draw = (img) => { ctx.clearRect(0, 0, 8, 8); ctx.drawImage(img, 8, 8, 8, 8, 0, 0, 8, 8); ctx.drawImage(img, 40, 8, 8, 8, 0, 0, 8, 8); };
  const img = skinImage(url);
  if (img.complete && img.naturalWidth) draw(img); else img.addEventListener("load", () => draw(img), { once: true });
  return c;
}
let menuCleanup = null;
function closeMenu() {
  menuCleanup?.(); menuCleanup = null;
  document.getElementById("acctmenu")?.remove();
}
// The head, name and skin change the instant you click; saving and Mojang catch up afterwards.
// The start page keeps its elements when the account changes: only the name and the figure are swapped.
function patchHome() {
  const hero = document.querySelector("#view .hero");
  const me = state.player;
  if (!hero || !me || me.skin_error || hero.querySelector(".small[style]")) { render(); return; }
  hero.querySelector("h1").textContent = me.signed_in ? me.name : "Geffeniuse Launcher";
  const old = hero.querySelector(".skinwrap");
  if (!old) return;
  const next = skinFigure(me.skin || DEFAULT_SKIN, me.slim);
  if (old !== next) old.replaceWith(next);
}
async function switchAccount(a) {
  closeMenu();
  state.player = { signed_in: true, name: a.name, skin: a.skin, slim: false, skin_error: null };
  state.settings = { ...state.settings, active_account: a.uuid };
  await skinImage(a.skin).decode().catch(() => {});
  updateAccountButton();
  if (state.route === "home") patchHome();
  const shown = state.player;
  try { await saveSettings({ active_account: a.uuid }); await loadPlayer(); } catch { /* the toast already explains */ }
  // Only draw again if the saved result differs from what the click already showed.
  const same = state.player && shown && state.player.name === shown.name && state.player.skin === shown.skin && state.player.signed_in === shown.signed_in;
  if (!same) { updateAccountButton(); if (state.route === "home") patchHome(); }
  refreshSkin();
}
async function addAccount() {
  closeMenu();
  try {
    const p = await invoke("ms_login");
    accountsCache = null;
    state.settings = await invoke("get_settings");
    toast("Angemeldet als " + p.name);
    await loadPlayer(); updateAccountButton(); render(); refreshSkin();
    if (!$("#setup").hidden) renderSetup();
  } catch (e) { toast(String(e), true); }
}
let menuOpening = false;
async function openAccountMenu() {
  if (document.getElementById("acctmenu")) { closeMenu(); return; }
  if (menuOpening) return;
  menuOpening = true;
  const active = state.settings.active_account;
  const menu = h("div", { id: "acctmenu", class: "menu", role: "menu" });
  let shownKey = "";
  const fill = (accounts) => {
    // Rebuild only when something visible differs, never just because a refresh came back.
    const key = JSON.stringify(accounts.map((a) => [a.uuid, a.name, !!a.skin]));
    if (key === shownKey) return;
    shownKey = key;
    menu.replaceChildren(
      h("div", { class: "menu-h" }, "Konto wechseln"),
      ...accounts.map((a) => h("button", { class: "menu-item" + (active === a.uuid ? " on" : ""), role: "menuitemradio", "aria-checked": String(active === a.uuid), onclick: () => switchAccount(a) },
        headCanvas(a.skin), h("span", { class: "grow" }, a.name))),
      h("div", { class: "menu-sep" }),
      h("button", { class: "menu-item", role: "menuitem", onclick: addAccount }, h("span", { class: "plus" }, "+"), h("span", { class: "grow" }, "Konto hinzufügen")));
  };
  try { fill(await listAccounts()); } finally { menuOpening = false; }
  document.body.append(menu);
  const outside = (e) => { if (!menu.contains(e.target) && !e.target.closest(".acct")) closeMenu(); };
  const esc = (e) => { if (e.key === "Escape") closeMenu(); };
  document.addEventListener("pointerdown", outside, true);
  document.addEventListener("keydown", esc);
  menuCleanup = () => { document.removeEventListener("pointerdown", outside, true); document.removeEventListener("keydown", esc); };
  // Only accounts without a cached skin need Mojang; if all heads are there, nothing is asked at all.
  if ((accountsCache || []).some((a) => !a.skin)) {
    invoke("refresh_account_skins").then((fresh) => { accountsCache = fresh; if (menu.isConnected) fill(fresh); }).catch(() => {});
  }
}

// Playing needs a Microsoft account; without one the player is asked to sign in instead.
function loginPrompt() {
  openModal(h("div", { class: "dialog block" },
    h("h2", null, "Anmeldung nötig"),
    h("p", { class: "dim" }, "Zum Spielen brauchst du ein Microsoft-Konto mit Minecraft: Java Edition."),
    h("div", { class: "row" },
      h("button", { class: "primary", onclick: () => { closeModal(); addAccount(); } }, "Mit Microsoft anmelden"),
      h("button", { onclick: closeModal }, "Abbrechen"))));
}
