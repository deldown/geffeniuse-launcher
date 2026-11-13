"use strict";
// Version pictures are small pixel scenes drawn in code, one per release family.
const FAMILY_INFO = {
  "26.3": ["Minecraft 26.3", "Das dritte Update 2026, im neuen Jahresschema von Mojang.", "future"],
  "26.2": ["Minecraft 26.2", "Das zweite Update 2026, im neuen Jahresschema von Mojang.", "future"],
  "26.1": ["Minecraft 26.1", "Das erste Update 2026 und der Start des neuen Jahresschemas.", "future"],
  "1.6": ["Horse Update", "Pferde, Esel, Namensschilder und Ressourcenpakete.", "classic"],
  "1.5": ["Redstone Update", "Trichter, Spender, Komparatoren und Redstone-Blöcke.", "classic"],
  "1.4": ["Pretty Scary Update", "Wither, Hexen, Fledermäuse und Kommandoblöcke.", "classic"],
  "1.3": ["Minecraft 1.3", "Dorfbewohner handeln mit Smaragden, dazu Einzelspieler im LAN.", "classic"],
  "1.2": ["Minecraft 1.2", "Dschungel, Ozelots und ein neues Weltformat.", "classic"],
  "1.1": ["Minecraft 1.1", "Superflache Welten und neue Sprachen.", "classic"],
  "1.0": ["Adventure Update", "Die erste Vollversion: Ende, Enderdrache und Brauen.", "classic"],
  "1.21": ["Tricky Trials", "Prüfungskammern, Windladungen und Kupfer in allen Formen.", "trial"],
  "1.20": ["Trails & Tales", "Kirschhaine, Bambus, Kamele und Archäologie.", "cherry"],
  "1.19": ["The Wild", "Mangrovensümpfe, die Tiefen Dunkelheit und der Warden.", "wild"],
  "1.18": ["Caves & Cliffs II", "Höhere Berge, tiefere Höhlen und eine neue Weltgenerierung.", "mountain"],
  "1.17": ["Caves & Cliffs I", "Amethyst, Kupfer, Axolotl und Ziegen.", "amethyst"],
  "1.16": ["Nether Update", "Karmesin- und Wirrwälder, Piglins und Netherit.", "nether"],
  "1.15": ["Buzzy Bees", "Bienen, Honig und Bienenstöcke.", "honey"],
  "1.14": ["Village & Pillage", "Neue Dörfer, Plünderer und Armbrüste.", "village"],
  "1.13": ["Update Aquatic", "Ozeane, Schildkröten, Delfine und Seetang.", "ocean"],
  "1.12": ["World of Color", "Beton, glasierte Keramik und Papageien.", "color"],
  "1.11": ["Exploration Update", "Waldanwesen, Totems und Shulkerkisten.", "classic"],
  "1.10": ["Frostburn Update", "Magmablöcke, Eisbären und Nether-Warzenblöcke.", "classic"],
  "1.9": ["Combat Update", "Schild, Elytren und das neue Kampfsystem.", "classic"],
  "1.8": ["Bountiful Update", "Der Klassiker für PvP und viele Server.", "classic"],
  "1.7": ["The Update that Changed the World", "Neue Biome und eine neue Weltgenerierung.", "classic"],
};
let versionImages = null;
const loadVersionImages = () => (versionImages ||= invoke("version_images").then((l) => {
  const byId = new Map(l), byFamily = new Map();
  for (const [v, url] of l) if (/^\d+\.\d+(\.\d+)?$/.test(v) && !byFamily.has(familyOf(v))) byFamily.set(familyOf(v), url);
  return { byId, byFamily };
}).catch(() => { versionImages = null; return { byId: new Map(), byFamily: new Map() }; }));
// A pre-release shares the picture of the release it leads up to.
const BUNDLED_FAMILIES = new Set(["1.6", "1.7", "1.8", "1.9", "1.10", "1.11", "1.12"]);
const bundledImage = (f) => BUNDLED_FAMILIES.has(f) ? "art/ver-" + f + ".webp" : null;
const pictureFor = (x, pics) => x.category === "aprilfools" ? null : pics.byId.get(x.id) || pics.byFamily.get((x.id.match(/^(\d+\.\d+)/) || [])[1]) || null;
const familyOf = (id) => id.split(".").slice(0, 2).join(".");
const hashStr = (str) => [...str].reduce((a, c) => Math.imul(a ^ c.charCodeAt(0), 16777619) >>> 0, 2166136261);
function seeded(seed) {
  return () => { seed = (seed + 0x6D2B79F5) | 0; let t = Math.imul(seed ^ (seed >>> 15), 1 | seed); t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t; return ((t ^ (t >>> 14)) >>> 0) / 4294967296; };
}
const rgb = (hex) => [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16));
const mixHex = (a, b, t) => "#" + rgb(a).map((v, i) => Math.round(v + (rgb(b)[i] - v) * t).toString(16).padStart(2, "0")).join("");
const artUrls = new Map();
function drawArt(family) {
  const img = document.createElement("img");
  img.className = "vart"; img.alt = "";
  let url = artUrls.get(family);
  if (!url) artUrls.set(family, url = new Promise((r) => drawArtRaw(family).toBlob((b) => r(URL.createObjectURL(b)))));
  url.then((u) => { img.src = u; });
  return img;
}
function drawArtRaw(family) {
  const W = 64, H = 36;
  const c = document.createElement("canvas");
  c.width = W; c.height = H; c.className = "vart";
  const g = c.getContext("2d"), rnd = seeded(hashStr(family));
  const px = (x, y, w, h, col) => { g.fillStyle = col; g.fillRect(Math.round(x), Math.round(y), w, h); };
  const sky = (a, b, until = H) => { for (let y = 0; y < until; y++) px(0, y, W, 1, mixHex(a, b, Math.floor((y / until) * 6) / 6)); };
  const disk = (cx, cy, r, col) => { for (let y = -r; y <= r; y++) for (let x = -r; x <= r; x++) if (x * x + y * y <= r * r + r / 2) px(cx + x, cy + y, 1, 1, col); };
  const noise = (x0, y0, w, h, cols, chance = 1) => { for (let y = y0; y < y0 + h; y++) for (let x = x0; x < x0 + w; x++) if (rnd() < chance) px(x, y, 1, 1, cols[Math.floor(rnd() * cols.length)]); };
  const scenes = {
    trial() {
      sky("#12323a", "#c9743a");
      for (let y = 18; y < H; y += 6) for (let x = ((y / 6) % 2) * 3 - 3; x < W; x += 6) { const a = rnd(); px(x, y, 6, 6, a < 0.55 ? "#8c8f87" : a < 0.8 ? "#7a7d76" : a < 0.92 ? "#d9784a" : "#3fb59a"); px(x, y, 6, 1, "#00000030"); px(x, y, 1, 6, "#00000030"); }
      for (let i = 0; i < 9; i++) px(rnd() * W, rnd() * 16, 1, 1, "#9fe4ff");
      px(26, 6, 12, 10, "#14110f"); px(28, 8, 8, 8, "#3a2c20"); px(30, 10, 4, 2, "#9fe4ff");
    },
    cherry() {
      sky("#f9d7e6", "#fff4f0", 22);
      for (let y = 22; y < H; y++) px(0, y, W, 1, y < 25 ? "#7ec850" : "#5faa3a");
      px(31, 16, 4, 16, "#7a4f33"); px(30, 30, 6, 2, "#5a3a24");
      for (const [x, y, r] of [[22, 12, 8], [34, 8, 9], [44, 13, 8], [33, 15, 7]]) disk(x, y, r, "#f4a6c4");
      for (const [x, y, r] of [[26, 10, 4], [38, 9, 5], [46, 14, 3]]) disk(x, y, r, "#fbd0e0");
      for (let i = 0; i < 18; i++) px(rnd() * W, 10 + rnd() * 24, 1, 1, "#f9b8d0");
    },
    wild() {
      sky("#0f2a1c", "#2f5d3a", 24);
      for (let y = 24; y < H; y++) px(0, y, W, 1, y < 27 ? "#2c4a3c" : "#0b1620");
      for (const x of [8, 20, 34, 48, 58]) { px(x, 8, 2, 20, "#5b3a22"); for (let i = 0; i < 5; i++) { px(x - i, 26 + i / 2, 1, 2, "#5b3a22"); px(x + 2 + i, 26 + i / 2, 1, 2, "#5b3a22"); } disk(x + 1, 8, 5, "#2f7a3a"); }
      noise(0, 28, W, 8, ["#0b1620", "#10283a", "#19e0d2"], 0.3);
      for (let i = 0; i < 10; i++) px(rnd() * W, 29 + rnd() * 6, 1, 1, "#7ffff0");
    },
    mountain() {
      sky("#7ec0f0", "#dff0ff", 24);
      const peak = (cx, top, base, w) => { for (let y = top; y < base; y++) { const half = ((y - top) / (base - top)) * w; px(cx - half, y, half * 2, 1, y - top < (base - top) * 0.28 ? "#f4f8ff" : "#7d848f"); } };
      peak(18, 6, 28, 22); peak(44, 3, 28, 26); peak(58, 12, 28, 14);
      for (let y = 28; y < H; y++) px(0, y, W, 1, "#4b4f57"); noise(0, 28, W, 8, ["#3d4048", "#5a5f69"], 0.5);
      px(26, 28, 12, 8, "#0c0d10");
    },
    amethyst() {
      sky("#241040", "#5b2d86");
      for (let y = 26; y < H; y++) px(0, y, W, 1, "#2a1a3a"); noise(0, 26, W, 10, ["#3a2550", "#1e1230"], 0.5);
      for (const [x, h] of [[8, 12], [14, 8], [26, 16], [34, 9], [46, 14], [54, 10]]) { for (let y = 0; y < h; y++) { const w = Math.max(1, Math.round(3 - (y / h) * 2.5)); px(x - w, 28 - y, w * 2 + 1, 1, y % 5 === 0 ? "#e6c2ff" : "#a56bd6"); } px(x - 1, 28 - h, 1, 1, "#ffffff"); }
      for (let i = 0; i < 12; i++) px(rnd() * W, rnd() * 24, 1, 1, "#d9a8ff");
    },
    nether() {
      sky("#2a0808", "#7a1a12");
      for (let y = 24; y < H; y++) px(0, y, W, 1, "#5a1c1c"); noise(0, 24, W, 12, ["#4a1414", "#6e2424", "#3a0f0f"], 0.6);
      px(0, 30, W, 3, "#ff7a1a"); noise(0, 30, W, 3, ["#ffc94a", "#ff5a0a"], 0.5);
      for (const [x, col] of [[12, "#b8282a"], [28, "#1fa89a"], [46, "#b8282a"], [56, "#1fa89a"]]) { px(x, 12, 2, 14, "#4a2230"); disk(x + 1, 11, 5, col); }
      for (let i = 0; i < 14; i++) px(rnd() * W, rnd() * 24, 1, 1, "#ff9a3a");
    },
    honey() {
      sky("#f2b01e", "#ffd96a");
      for (let row = 0; row < 7; row++) for (let col = 0; col < 12; col++) { const x = col * 6 + (row % 2) * 3, y = row * 5; px(x + 1, y, 4, 1, "#d98a0a"); px(x, y + 1, 6, 3, "#e89a14"); px(x + 1, y + 4, 4, 1, "#d98a0a"); px(x + 2, y + 2, 2, 1, "#ffe08a"); }
      px(40, 12, 7, 4, "#ffd21e"); px(42, 12, 1, 4, "#1c1408"); px(45, 12, 1, 4, "#1c1408"); px(41, 10, 3, 2, "#e8f6ff"); px(46, 13, 2, 1, "#1c1408");
    },
    village() {
      sky("#f08a3a", "#6a3a8a", 24);
      disk(48, 18, 5, "#ffd98a");
      for (let y = 24; y < H; y++) px(0, y, W, 1, "#5f9a3c");
      for (const [x, w, col] of [[6, 14, "#8a6a44"], [26, 12, "#9a9a96"], [44, 16, "#8a6a44"]]) { px(x, 16, w, 12, col); for (let i = 0; i < 6; i++) px(x - 1 + i, 15 - i, w + 2 - i * 2, 1, "#5a2e22"); px(x + w / 2 - 1, 22, 3, 6, "#3a2414"); px(x + 2, 19, 3, 3, "#ffd25a"); }
    },
    ocean() {
      sky("#2a7fd0", "#0c2f6a");
      for (let i = 0; i < 6; i++) px(8 + i * 11, 0, 2, 14, "#7fc0ff40");
      for (let y = 28; y < H; y++) px(0, y, W, 1, "#d8c88a"); noise(0, 28, W, 8, ["#c4b478", "#e6d89a"], 0.4);
      for (const x of [6, 18, 40, 54]) for (let y = 14; y < 28; y++) px(x + Math.round(Math.sin(y / 2 + x) * 1.2), y, 1, 1, "#2fa04a");
      px(28, 22, 7, 4, "#3a9a4a"); px(30, 20, 3, 2, "#2a7a38"); px(34, 23, 3, 2, "#78c070"); px(27, 25, 2, 1, "#78c070");
    },
    color() {
      const cols = ["#f9fafa", "#f08a14", "#c64fbd", "#3ab3da", "#f8c627", "#70b919", "#ed8dac", "#16a3a0", "#8932b8", "#3c44aa", "#b02e26", "#5e7c16"];
      for (let y = 0; y < H; y += 6) for (let x = 0; x < W; x += 6) { const col = cols[Math.floor(rnd() * cols.length)]; px(x, y, 6, 6, col); px(x, y, 6, 1, "#ffffff30"); px(x, y + 5, 6, 1, "#00000030"); }
      disk(48, 10, 4, "#ffffff55");
    },
    classic() {
      const hue = ["#7ec8f0", "#8ad0e8", "#9ad8f4", "#78bce6"][hashStr(family) % 4];
      sky(hue, "#e6f6ff", 22);
      for (let y = 22; y < H; y++) px(0, y, W, 1, y < 25 ? "#5faa3a" : "#7a5a3a"); noise(0, 25, W, 11, ["#6e4e30", "#85643f", "#5e4228"], 0.6);
      px(46, 4, 7, 7, "#fff2a8"); px(48, 6, 3, 3, "#ffe35a");
      for (const [x, y] of [[8, 6], [24, 10], [34, 4]]) { px(x, y, 9, 3, "#ffffff"); px(x + 2, y - 1, 5, 1, "#ffffff"); }
    },
    future() {
      sky("#0b0f2a", "#3a2a7a");
      for (let i = 0; i < 26; i++) px(rnd() * W, rnd() * 24, 1, 1, rnd() < 0.3 ? "#ffd98a" : "#cfd8ff");
      for (let y = 26; y < H; y++) px(0, y, W, 1, "#1a1d3a"); noise(0, 26, W, 10, ["#242850", "#14172e"], 0.6);
      for (const [x, h, col] of [[10, 10, "#55e0c0"], [22, 15, "#7a9cff"], [38, 12, "#c78aff"], [52, 9, "#55e0c0"]]) { px(x, 26 - h, 5, h, col); px(x, 26 - h, 5, 1, "#ffffff90"); px(x + 4, 26 - h, 1, h, "#00000030"); }
    },
  };
  (scenes[(FAMILY_INFO[family] || [])[2]] || scenes.future)();
  return c;
}

let versionList = null;
const loadVersions = () => (versionList ||= invoke("minecraft_versions").catch((e) => { versionList = null; throw e; }));
// A drop-down in the launcher's own style. The browser's list cannot be styled, so this draws it.
let closeOpenDropdown = null;
function dropdown({ options, value, onChange = () => {}, label = "" }) {
  let cur = value, menu = null;
  const labelOf = (v) => (options.find(([k]) => k === v) || options[0] || ["", ""])[1];
  const thumbOf = (v) => (options.find(([k]) => k === v) || [])[2];
  const text = h("span", { class: "dd-label" });
  const btn = h("button", { type: "button", class: "dd-btn", "aria-haspopup": "listbox", "aria-expanded": "false", "aria-label": label || null }, text, h("span", { class: "dd-chev", "aria-hidden": "true" }));
  const el = h("div", { class: "dd" }, btn);
  const paint = () => {
    text.textContent = tr(labelOf(cur));
    btn.querySelector(".dd-thumb")?.remove();
    const t = thumbOf(cur);
    if (t) btn.prepend(t());
  };
  const close = (refocus = false) => {
    if (!menu) return;
    menu.remove(); menu = null;
    btn.setAttribute("aria-expanded", "false");
    document.removeEventListener("pointerdown", outside, true);
    window.removeEventListener("resize", close);
    el.removeEventListener("keydown", keys);
    if (closeOpenDropdown === close) closeOpenDropdown = null;
    if (refocus) btn.focus();
  };
  const outside = (e) => { if (menu && !menu.contains(e.target) && !el.contains(e.target)) close(); };
  const choose = (k) => { close(true); if (k !== cur) { cur = k; paint(); onChange(k); } };
  const items = () => [...menu.querySelectorAll(".menu-item")];
  const keys = (e) => {
    if (!menu) return;
    const list = items(), at = list.indexOf(document.activeElement);
    if (e.key === "Escape") { e.preventDefault(); e.stopPropagation(); close(true); }
    else if (e.key === "ArrowDown") { e.preventDefault(); list[Math.min(list.length - 1, at + 1)]?.focus(); }
    else if (e.key === "ArrowUp") { e.preventDefault(); list[Math.max(0, at - 1)]?.focus(); }
    else if (e.key === "Home") { e.preventDefault(); list[0]?.focus(); }
    else if (e.key === "End") { e.preventDefault(); list[list.length - 1]?.focus(); }
  };
  const open = () => {
    closeOpenDropdown?.();
    menu = h("div", { class: "menu dd-menu", role: "listbox" }, options.map(([k, lab, thumb]) =>
      h("button", { type: "button", class: "menu-item" + (k === cur ? " on" : ""), role: "option", "aria-selected": String(k === cur), onclick: () => choose(k) },
        thumb ? thumb() : null, h("span", { class: "grow" }, lab), k === cur ? icon("check") : null)));
    document.body.append(menu);
    const r = btn.getBoundingClientRect(), mh = menu.offsetHeight, below = innerHeight - r.bottom - 12;
    menu.style.minWidth = r.width + "px";
    menu.style.left = Math.max(8, Math.min(r.left, innerWidth - menu.offsetWidth - 8)) + "px";
    menu.style.top = (below >= Math.min(mh, 220) ? r.bottom + 6 : Math.max(8, r.top - mh - 6)) + "px";
    menu.style.maxHeight = Math.max(160, Math.max(below, r.top - 20)) + "px";
    btn.setAttribute("aria-expanded", "true");
    document.addEventListener("pointerdown", outside, true);
    window.addEventListener("resize", close);
    el.addEventListener("keydown", keys);
    menu.addEventListener("keydown", keys);
    closeOpenDropdown = close;
    (items().find((x) => x.classList.contains("on")) || items()[0])?.focus();
    menu.querySelector(".on")?.scrollIntoView({ block: "nearest" });
  };
  btn.addEventListener("click", () => (menu ? close() : open()));
  Object.defineProperty(el, "value", { get: () => cur, set: (v) => { cur = v; paint(); } });
  paint();
  return el;
}

async function createDialogSimple(versions) {
  const name = h("input", { type: "text", value: "Mein Modpack", maxlength: 60 });
  const ver = dropdown({ options: versions.slice(0, 80).map((x) => [x.id, x.id]), value: versions[0]?.id, label: "Minecraft-Version" });
  const loader = dropdown({ options: LOADERS.map((l) => [l, l]), value: LOADERS[0], label: "Loader" });
  openModal(h("div", { class: "dialog block" },
    h("h2", null, "Neues Modpack"),
    h("label", { class: "field" }, "Name", name),
    h("div", { class: "field" }, h("span", null, "Minecraft-Version"), ver),
    h("div", { class: "field" }, h("span", null, "Loader"), loader),
    h("div", { class: "row" },
      h("button", { class: "primary", onclick: async () => { await call("create_instance", { name: name.value, mcVersion: ver.value, loader: loader.value }); closeModal(); refresh(); } }, "Erstellen"),
      h("button", { onclick: closeModal }, "Abbrechen"))));
  name.select();
}
const LOADER_INFO = {
  vanilla: ["Vanilla", "Ohne Mods, so wie Mojang es baut."],
  fabric: ["Fabric", "Leicht und schnell, viele moderne Mods."],
  quilt: ["Quilt", "Fabric-kompatibel, offen entwickelt."],
  forge: ["Forge", "Der Klassiker für große Modpacks."],
  neoforge: ["NeoForge", "Der Nachfolger von Forge für neue Versionen."],
};
const VERSION_GROUPS = [["release", "Releases"], ["snapshot", "Snapshots"], ["experimental", "Experimentell"], ["aprilfools", "Aprilscherze"]];
async function createDialog() {
  let versions;
  try { versions = await loadVersions(); } catch (e) { toast(String(e), true); return; }
  if (state.settings.version_picker === "simple") return createDialogSimple(versions);
  const noPics = state.settings.version_pictures === false;
  const pictures = noPics ? { byId: new Map(), byFamily: new Map() } : await Promise.race([loadVersionImages(), new Promise((r) => setTimeout(() => r({ byId: new Map(), byFamily: new Map() }), 1500))]);
  const groups = VERSION_GROUPS.filter(([g]) => versions.some((x) => x.category === g));
  let group = "release", family = "", version = "", loader = "fabric";
  const inGroup = () => versions.filter((x) => x.category === group);
  const families = () => [...new Set(inGroup().map((x) => familyOf(x.id)))];
  const name = h("input", { type: "text", value: "Mein Modpack", maxlength: 60, "aria-label": "Name" });
  const grid = h("div", { class: "vgrid" });
  const go_ = h("button", { class: "primary xl", onclick: async () => {
    go_.disabled = true;
    try { await call("create_instance", { name: name.value, mcVersion: version, loader: group === "release" ? loader : "vanilla" }); closeModal(); refresh(); } catch { go_.disabled = false; }
  } });
  const info = h("div", { class: "vinfo" });
  const chips = h("div", { class: "vchips" });
  const loaders = h("div", { class: "vloaders" });
  const unsure = h("p", { class: "small", style: "color:#ffd35a", hidden: true }, "Snapshots, Experimente und Aprilscherze laufen nur ohne Loader und sind nicht für Spielstände gedacht, die du behalten willst.");
  const paintCards = () => { for (const card of grid.children) card.setAttribute("aria-pressed", String(card.dataset.key === (group === "release" ? family : version))); };
  const paintSide = () => {
    const rel = group === "release";
    const [title, desc] = rel ? FAMILY_INFO[family] || ["Minecraft " + family, "Neue Version von Mojang."] : ["Minecraft " + version, VERSION_GROUPS.find(([g]) => g === group)[1]];
    info.replaceChildren(...[h("h3", null, title), rel ? h("p", { class: "small dim" }, desc) : null].filter(Boolean));
    chips.parentElement.style.display = rel ? "" : "none";
    chips.replaceChildren(...(rel ? versions.filter((x) => x.category === "release" && familyOf(x.id) === family).map((x) => h("button", { class: "vchip", "aria-pressed": String(x.id === version), onclick: () => { version = x.id; paintSide(); } }, x.id)) : []));
    loaders.parentElement.style.display = rel ? "" : "none";
    unsure.hidden = rel;
    loaders.replaceChildren(...LOADERS.map((l) => h("button", { class: "vloader", "aria-pressed": String(l === loader), onclick: () => { loader = l; paintSide(); } },
      l === "vanilla" ? cube(l) : h("img", { class: "lico", src: "art/loader-" + l + (l === "forge" ? ".jpg" : ".png"), alt: "", width: 34, height: 34 }), h("span", { class: "vl-name" }, LOADER_INFO[l][0]), h("span", { class: "vl-hint small dim" }, LOADER_INFO[l][1]))));
    go_.replaceChildren(h("span", null, "Erstellen"), h("span", { class: "count" }, version + (rel ? " · " + LOADER_INFO[loader][0] : "")));
  };
  const cardCache = new Map(), cardState = new Map();
  grid.addEventListener("scroll", () => { if (group !== "release" && grid.scrollTop + grid.clientHeight > grid.scrollHeight - 300) cardState.get(group)?.more(); }, { passive: true });
  const fillGrid = () => {
    const cached = cardCache.get(group);
    if (cached) {
      grid.replaceChildren(...cached);
      if (group === "release") { family = families().includes(family) ? family : families()[0]; version = versions.some((x) => x.id === version && familyOf(x.id) === family && x.category === "release") ? version : versions.find((x) => x.category === "release" && familyOf(x.id) === family).id; }
      else version = inGroup().some((x) => x.id === version) ? version : inGroup()[0].id;
      paintCards(); paintSide();
      return;
    }
    grid.replaceChildren();
    if (group === "release") {
      for (const f of families()) {
        grid.append(h("button", { class: "vcard", "data-key": f, "aria-label": f + (FAMILY_INFO[f] ? " " + FAMILY_INFO[f][0] : ""), onclick: () => { family = f; version = versions.find((x) => x.category === "release" && familyOf(x.id) === f).id; paintCards(); paintSide(); } },
          !noPics && (pictures.byFamily.has(f) || BUNDLED_FAMILIES.has(f)) ? h("img", { class: "vart photo", src: pictures.byFamily.get(f) || bundledImage(f), decoding: "async", alt: "", loading: "lazy", referrerpolicy: "no-referrer", onerror: (e) => e.target.replaceWith(drawArt(f)) }) : drawArt(f), h("span", { class: "vshade" }), h("span", { class: "vnum" }, f), h("span", { class: "vname" }, FAMILY_INFO[f]?.[0] || "")));
      }
      family = families().includes(family) ? family : families()[0];
      version = versions.some((x) => x.id === version && familyOf(x.id) === family && x.category === "release") ? version : versions.find((x) => x.category === "release" && familyOf(x.id) === family).id;
    } else {
      const items = inGroup(), nodes = [];
      const make = (x) => h("button", { class: "vcard small", "data-key": x.id, "aria-label": x.id, onclick: () => { version = x.id; paintCards(); paintSide(); } },
          !noPics && pictureFor(x, pictures) ? h("img", { class: "vart photo", src: pictureFor(x, pictures), alt: "", loading: "lazy", referrerpolicy: "no-referrer", onerror: (e) => e.target.replaceWith(drawArt("1.8")) }) : drawArt("1.8"), h("span", { class: "vshade" }), h("span", { class: "vnum small" }, x.id));
      const state = { nodes, more() { if (nodes.length >= items.length) return; const next = items.slice(nodes.length, nodes.length + 24).map(make); nodes.push(...next); grid.append(...next); paintCards(); } };
      cardState.set(group, state);
      state.more();
      version = inGroup().some((x) => x.id === version) ? version : inGroup()[0].id;
    }
    cardCache.set(group, group === "release" ? [...grid.children] : cardState.get(group).nodes);
    paintCards(); paintSide();
  };
  const picker = groups.length > 1 ? segmented(groups, group, (g) => { group = g; fillGrid(); }) : null;
  const right = h("div", { class: "vside" }, h("div", { class: "field" }, h("span", null, "Name"), name), info, unsure,
    h("div", { class: "field" }, h("span", null, "Unterversion"), chips),
    h("div", { class: "field" }, h("span", null, "Loader"), loaders),
    h("div", { class: "row vfoot" }, go_));
  openModal(h("div", { class: "dialog block fancy" },
    h("div", { class: "vtop" }, h("h2", null, "Neues Modpack"), h("span", { class: "small dim" }, "Wähle eine Minecraft-Version"), h("span", { class: "grow" }), picker),
    h("div", { class: "vbody" }, grid, right)));
  fillGrid();
  name.select();
}
async function importFile() {
  const path = await call("pick_pack_file");
  if (!path) return;
  toast("Import läuft …");
  const r = await call("import_pack_file", { path });
  toast("„" + r.instance.name + "“ importiert");
  manualDialog(r.manual);
  refresh();
}
function manualDialog(manual) {
  if (!manual.length) return;
  openModal(h("div", { class: "dialog block" },
    h("h2", null, "Manuelle Downloads nötig"),
    h("p", { class: "dim" }, "CurseForge erlaubt für diese Dateien keinen Download durch Launcher. Lade sie auf curseforge.com herunter und lege sie in den Mods-Ordner der Instanz:"),
    h("ul", null, manual.map((m) => h("li", null, m.name))),
    h("button", { onclick: closeModal }, "Schließen")));
}
