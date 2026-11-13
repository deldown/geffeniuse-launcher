"use strict";
const NAV = [
  ["home", "Start", true],
  ["library", "Bibliothek", true],
  ["discover", "Entdecken", "discover"],
  ["shots", "Screenshots", "screenshots"],
];
const isCurrent = (id) => state.route === id || (id === "library" && (state.route === "instance" || (state.route === "project" && state.project?.instanceId))) || (id === "discover" && state.route === "project" && !state.project?.instanceId);
function navItem(id, label) {
  return h("button", { class: "nav-item", "data-route": id, "aria-current": isCurrent(id) ? "page" : null, title: label, onclick: () => go(id) }, icon(id), h("span", { class: "lbl" }, label));
}
const KOFI = "https://ko-fi.com/deldown";
function acctButton() {
  const p = state.player;
  return p?.signed_in
    ? h("button", { class: "nav-item acct", title: "Konto: " + p.name, "aria-label": "Konto wechseln, angemeldet als " + p.name, "aria-haspopup": "menu", onclick: openAccountMenu }, headCanvas(p.skin))
    : h("button", { class: "nav-item acct signin", onclick: addAccount }, h("span", { class: "lbl" }, "Anmelden"));
}
function updateAccountButton() {
  const old = document.querySelector("#nav .acct");
  const p = state.player;
  if (old && p?.signed_in && !old.classList.contains("signin")) {
    old.querySelector("canvas").replaceWith(headCanvas(p.skin));
    old.title = "Konto: " + p.name;
    old.setAttribute("aria-label", tr("Konto wechseln, angemeldet als ") + p.name);
  } else if (old) { old.replaceWith(acctButton()); movePills(true); } else renderNav();
}
// The bar is built once. Changing page only moves the highlight, so the logo and the player head
// are never decoded again.
function renderNav() {
  const s = state.settings;
  $("#nav").replaceChildren(
    h("div", { class: "brand" }, h("img", { src: "logo.png", alt: "Geffeniuse Launcher", height: 44 }), h("span", { class: "name" }, "Geffeniuse Launcher"), h("span", { class: "betatag", title: "Der Launcher ist noch in der Beta. Fehler bitte melden." }, "Beta")),
    h("nav", { class: "navgroup", "aria-label": "Hauptmenü" }, ...NAV.filter(([, , flag]) => flag === true || s.modules[flag]).map(([id, label]) => navItem(id, label))),
    h("span", { class: "sep" }),
    h("div", { class: "navgroup" }, acctButton(), navItem("settings", "Optionen")));
  movePills(true);
}
function movePills(instant = false) {
  for (const g of document.querySelectorAll("#nav .navgroup")) {
    let pill = g.querySelector(".nav-pill");
    if (!pill) { pill = h("span", { class: "nav-pill", "aria-hidden": "true" }); g.prepend(pill); }
    const a = g.querySelector('.nav-item[aria-current="page"]');
    if (!a) { pill.style.opacity = "0"; continue; }
    const jump = instant || pill.style.opacity !== "1";
    if (jump) pill.style.transition = instant ? "none" : "opacity 160ms var(--ease)";
    pill.style.width = a.offsetWidth + "px";
    pill.style.transform = `translateX(${a.offsetLeft}px)`;
    if (jump) { void pill.offsetWidth; }
    pill.style.opacity = "1";
    if (jump) { void pill.offsetWidth; pill.style.transition = ""; }
  }
}
function setActiveNav() {
  for (const b of document.querySelectorAll("#nav .nav-item[data-route]")) {
    if (isCurrent(b.dataset.route)) b.setAttribute("aria-current", "page"); else b.removeAttribute("aria-current");
  }
  movePills();
}
function go(route) { state.route = route; setActiveNav(); render(); }

const VIEWS = () => ({ project: viewProject, home: viewHome, instance: viewInstance, library: viewLibrary, discover: viewDiscover, shots: viewShots, settings: viewSettings });
let renderSeq = 0;
// The page is built off-screen and swapped in only if no newer render started meanwhile.
async function render() {
  const seq = ++renderSeq;
  const next = document.createElement("div");
  const view = VIEWS()[state.route] || viewHome;
  await view(next);
  if (seq !== renderSeq) return;
  // Decode the page's images first so nothing pops in after the swap.
  const imgs = [...next.querySelectorAll("img:not([loading=lazy])")].filter((i) => i.src && !i.complete);
  if (imgs.length) {
    await Promise.race([Promise.all(imgs.map((i) => i.decode().catch(() => {}))), new Promise((r) => setTimeout(r, 150))]);
    if (seq !== renderSeq) return;
  }
  const v = $("#view");
  const same = v.dataset.route === state.route;
  const keep = same ? v.scrollTop : 0;
  v.dataset.route = state.route;
  v.replaceChildren(...next.childNodes);
  v.scrollTop = keep;
  if (state.progress) applyProgress();
}
