"use strict";
let disc = { source: "modrinth", query: "", hits: [], cache: new Map() };
async function viewDiscover(v) {
  const s = state.settings;
  const sources = [["modrinth", "Modrinth", s.modrinth_enabled], ["curseforge", "CurseForge", s.curseforge_enabled && (s.curseforge_key || s.curseforge_proxy !== false)]].filter((x) => x[2]);
  if (!sources.some((x) => x[0] === disc.source)) disc.source = sources[0]?.[0];
  const results = h("div", null);
  const q = h("input", { type: "search", placeholder: "Modpacks suchen …", value: disc.query, "aria-label": "Suche" });
  const paint = (hits) => results.replaceChildren(...(hits.length ? hits.map(hitRow) : [h("p", { class: "dim" }, "Nichts gefunden.")]));
  // Coming back to this page shows the last results at once; only the button or Enter searches again.
  const run = async (force = false) => {
    disc.query = q.value;
    const key = disc.source + "|" + disc.query, hit = disc.cache.get(key);
    if (hit && !force && Date.now() - hit.at < 10 * 60 * 1000) { paint(hit.hits); return; }
    if (hit) paint(hit.hits); else results.replaceChildren(skeleton(5));
    try {
      const hits = await invoke("search_modpacks", { source: disc.source, query: disc.query, offset: 0 });
      disc.cache.set(key, { hits, at: Date.now() });
      if (disc.cache.size > 30) disc.cache.delete(disc.cache.keys().next().value);
      if (key === disc.source + "|" + q.value) paint(hits);
    } catch (e) { results.replaceChildren(h("div", { class: "empty block" }, String(e))); }
  };
  q.addEventListener("keydown", (e) => { if (e.key === "Enter") run(true); });
  v.append(h("div", { class: "head" }, h("h1", null, "Entdecken"),
    h("div", { class: "row" },
      h("div", { class: "seg" }, sources.map(([id, label]) => h("button", { "aria-pressed": String(disc.source === id), onclick: () => { disc.source = id; render(); } }, label))),
      q, h("button", { class: "primary", onclick: () => run(true) }, "Suchen"))), results);
  if (disc.source) run();
}
function openProject(x) { state.project = { source: x.source, id: x.id, hit: x }; go("project"); }
function hitRow(x) {
  const fmt = new Intl.NumberFormat(undefined, { notation: "compact" });
  const row = h("div", { class: "block hit", tabindex: "0", role: "link", "aria-label": x.title, onclick: () => openProject(x) },
    logo(x.icon),
    h("div", { style: "min-width:0" },
      h("div", { class: "title" }, x.title, x.author ? h("span", { class: "dim small" }, "  von " + x.author) : null),
      h("div", { class: "desc" }, x.description),
      h("div", { class: "small dim" }, fmt.format(x.downloads) + " Downloads")),
    h("button", { class: "primary", onclick: (e) => { e.stopPropagation(); installDialog(x); } }, "Installieren"));
  row.addEventListener("keydown", (e) => { if (e.key === "Enter" && e.target === row) openProject(x); });
  return row;
}
const projCache = new Map();
async function viewProject(v) {
  const { source, id, hit } = state.project || {};
  if (!id) { go("discover"); return; }
  const forMod = state.project?.instanceId, from = state.project?.from;
  const back = from ? () => { state.project = from; state.projectTab = "mods"; go("project"); }
    : forMod ? () => { state.instanceId = forMod; state.tab = "add"; go("instance"); } : () => go("discover");
  v.append(h("button", { class: "ghost back", onclick: back }, from ? "← " + from.hit.title : forMod ? "← Hinzufügen" : "← Entdecken"));
  const holder = h("div", null);
  v.append(holder);
  const key = source + "|" + id;
  const cached = projCache.get(key);
  if (cached) { await buildProject(holder, cached, source, id, hit); return; }
  holder.append(skeleton(1, 130), skeleton(1, 340));
  // The page swaps in right away with placeholders; the details fill it when they arrive.
  invoke("project_details", { source, id }).then(async (d) => {
    projCache.set(key, d);
    if (projCache.size > 20) projCache.delete(projCache.keys().next().value);
    if (!holder.isConnected || state.route !== "project" || state.project?.id !== id) return;
    holder.replaceChildren();
    await buildProject(holder, d, source, id, hit);
  }).catch((e) => { if (holder.isConnected) holder.replaceChildren(h("div", { class: "empty block" }, String(e))); });
}
async function buildProject(v, d, source, id, hit) {
  const fmt = new Intl.NumberFormat(undefined, { notation: "compact" });
  const x = { source, id, title: d.title, icon: d.icon || hit?.icon || null };
  const modFor = state.project?.instanceId ? (await listInstances()).find((n) => n.id === state.project.instanceId) : null;
  const standalone = !!state.project?.from;
  let installBtn = standalone ? null : h("button", { class: "primary xl", onclick: () => installDialog(x) }, "Installieren");
  if (modFor) {
    const installed = new Set((await call("list_content", { id: modFor.id })).map((m) => m.project_id).filter(Boolean));
    const done = installed.has(source === "curseforge" ? "cf:" + id : id);
    const kind = state.project?.kind || "mod";
    installBtn = h("button", { class: "primary xl", disabled: done, onclick: () => installMod(modFor, x, installBtn, installed, kind, state.addWorld || null) }, done ? "Installiert" : "Installieren");
  }
  v.append(h("section", { class: "block pagehead" },
    h("div", { class: "row", style: "gap:18px; flex-wrap:nowrap; min-width:0" },
      d.icon ? h("img", { class: "logo", src: d.icon, alt: "", width: 96, height: 96, style: "width:96px;height:96px", decoding: "async", referrerpolicy: "no-referrer" }) : h("div", { class: "logo ph", style: "width:96px;height:96px" }, cube("stone")),
      h("div", { class: "meta", style: "min-width:0" },
        h("h1", null, d.title), h("div", { class: "dim" }, d.summary),
        h("div", { class: "row", style: "gap:16px" },
          h("span", { class: "stat" }, fmt.format(d.downloads) + " Downloads"),
          d.followers != null ? h("span", { class: "stat" }, fmt.format(d.followers) + " Follower") : null,
          ...d.categories.slice(0, 5).map((c) => h("span", { class: "chip" }, c))))),
    installBtn));
  const modKind = (state.project?.kind || "mod") === "mod";
  const tabs = [["desc", "Beschreibung"], ...(d.gallery.length ? [["gallery", "Galerie"]] : []),
    ...(standalone ? [] : modFor ? (modKind ? [["deps", "Abhängigkeiten"]] : []) : [["mods", "Mods"], ["versions", "Versionen"]])];
  const tab = tabs.some(([k]) => k === state.projectTab) ? state.projectTab : "desc";
  const main = h("div", null);
  const body = h("div", { class: "proj" }, main);
  v.append(h("div", { class: "row", style: "margin:16px 0" }, h("div", { class: "seg" }, tabs.map(([k, label]) =>
    h("button", { "aria-pressed": String(tab === k), onclick: () => { state.projectTab = k; render(); } }, label)))), body);
  if (tab === "desc") main.append(h("div", { class: "block md" }, d.body ? (d.body_format === "html" ? renderHtml(d.body) : renderMarkdown(d.body)) : h("p", { class: "dim" }, "Keine Beschreibung.")));
  if (tab === "gallery") main.append(h("div", { class: "gallery" }, d.gallery.map((g) =>
    h("figure", { onclick: () => openModal(h("div", { class: "dialog wide block" }, h("img", { class: "full", src: g.url, alt: g.title }), h("div", { class: "row" }, h("span", { class: "dim" }, g.title), h("button", { onclick: closeModal }, "Schließen")))) },
      mkImg(g.url, g.title), g.title ? h("figcaption", null, g.title) : null))));
  if (tab === "deps") {
    const list = await loadDeps(modFor, x);
    const installedKeys = new Set((await call("list_content", { id: modFor.id })).map((m) => m.project_id).filter(Boolean));
    const box = h("div", { class: "block" });
    const paint = () => box.replaceChildren(...(list.length ? depItems(modFor, x, list, installedKeys, paint) : [h("p", { class: "dim", style: "padding:14px" }, "Dieser Mod braucht keine weiteren Mods.")]));
    paint(); main.append(box);
  }
  if (tab === "mods") {
    main.append(skeleton(5, 56));
    invoke("modpack_contents", { source, project: id }).then((list) => {
      const names = { mod: "Mods", resourcepack: "Ressourcenpakete", shader: "Shader", other: "Weiteres" };
      const q = h("input", { type: "search", placeholder: "In diesem Modpack suchen …", "aria-label": "Suchen" });
      const rows = h("div", null);
      const paint = () => {
        const term = q.value.trim().toLowerCase();
        const shown = list.filter((e) => !term || e.title.toLowerCase().includes(term));
        rows.replaceChildren(...(shown.length ? Object.keys(names).flatMap((k) => {
          const g = shown.filter((e) => e.kind === k);
          return g.length ? [h("h3", { class: "packgroup" }, names[k] + " · " + g.length), ...g.map((e) => packRow(e, source, id, d, x))] : [];
        }) : [h("p", { class: "dim" }, "Nichts gefunden.")]));
      };
      q.addEventListener("input", paint);
      paint();
      main.replaceChildren(h("div", { class: "row", style: "margin-bottom:10px" }, q, h("span", { class: "dim small" }, list.length + " Einträge")), rows);
    }).catch((e) => main.replaceChildren(h("div", { class: "empty block" }, String(e))));
  }
  if (tab === "versions") {
    try {
      const vs = await invoke(source === "curseforge" ? "curseforge_pack_versions" : "modrinth_pack_versions", { project: id });
      main.append(...vs.slice(0, 60).map((vv) => h("div", { class: "block vrow" },
        h("div", { style: "min-width:0" }, h("div", { class: "title" }, vv.name), h("div", { class: "small dim" }, [vv.game_versions.slice(-1)[0], vv.loaders.join(", ")].filter(Boolean).join(" · "))),
        h("button", { onclick: () => installDialog(x, vv.id) }, "Installieren"))));
    } catch (e) { main.append(h("div", { class: "empty block" }, String(e))); }
  }
  const chips = (list, max) => h("div", { class: "chips" }, list.slice(0, max).map((c) => h("span", { class: "chip" }, c)), list.length > max ? h("span", { class: "chip dim" }, "+" + (list.length - max)) : null);
  const side = h("aside", { class: "side" });
  if (d.game_versions.length) side.append(h("section", { class: "block" }, h("h3", null, "Minecraft"), chips(d.game_versions, 12)));
  if (d.loaders.length) side.append(h("section", { class: "block" }, h("h3", null, "Loader"), chips(d.loaders, 8)));
  const links = [...(d.page_url ? [{ label: "Auf " + (source === "curseforge" ? "CurseForge" : "Modrinth") + " öffnen", url: d.page_url }] : []), ...d.links];
  if (links.length) side.append(h("section", { class: "block" }, h("h3", null, "Links"), links.map((l) => h("button", { class: "linkrow", onclick: () => openUrl(l.url) }, l.label + " ↗"))));
  const facts = [d.license && ["Lizenz", d.license], d.updated && ["Aktualisiert", new Date(d.updated).toLocaleDateString()]].filter(Boolean);
  if (facts.length) side.append(h("section", { class: "block" }, h("h3", null, "Details"), facts.map(([k, val]) => h("div", { class: "small", style: "margin-top:6px" }, h("span", { class: "dim" }, k, ": "), val))));
  body.append(side);
}
// A mod inside a modpack opens its own page; "back" returns to the modpack's list.
function packRow(e, source, packId, pack, packHit) {
  const open = e.project_id && e.kind !== "other"
    ? () => { state.project = { source, id: e.project_id, hit: { source, id: e.project_id, title: e.title, icon: e.icon }, from: { source, id: packId, hit: { ...packHit, title: pack.title } } }; state.projectTab = "desc"; go("project"); }
    : null;
  const row = h("div", { class: "block packrow" + (open ? " clickable" : ""), ...(open ? { tabindex: "0", role: "link", "aria-label": e.title, onclick: open } : {}) }, logo(e.icon), h("span", { class: "title" }, e.title), open ? h("span", { class: "dim" }, "›") : null);
  if (open) row.addEventListener("keydown", (ev) => { if (ev.key === "Enter") open(); });
  return row;
}
async function installDialog(x, preselect) {
  const body = h("div", { class: "dialog block" }, h("h2", null, x.title), h("p", { class: "dim" }, "Lade Versionen …"));
  openModal(body);
  let versions;
  try { versions = await invoke(x.source === "curseforge" ? "curseforge_pack_versions" : "modrinth_pack_versions", { project: x.id }); } catch (e) { closeModal(); toast(String(e), true); return; }
  const pics = state.settings.version_pictures === false ? null : await loadVersionImages();
  const thumb = (vv) => () => {
    const mc = vv.game_versions.slice(-1)[0] || "", fam = familyOf(mc);
    const src = pics && /^\d+\.\d+/.test(mc) ? pics.byId.get(mc) || pics.byFamily.get(fam) || bundledImage(fam) : null;
    const box = h("span", { class: "dd-thumb", "aria-hidden": "true" });
    box.append(src ? h("img", { src, alt: "", loading: "lazy", referrerpolicy: "no-referrer", onerror: (e) => e.target.replaceWith(drawArt(fam)) }) : drawArt(/^\d+\.\d+/.test(mc) ? fam : "1.8"));
    return box;
  };
  const sel = dropdown({ options: versions.map((vv) => [vv.id, `${vv.name}  ·  ${vv.game_versions.slice(-1)[0] || ""}  ·  ${vv.loaders.join(", ")}`, thumb(vv)]), value: preselect || versions[0]?.id, label: "Version" });
  const bar = h("div", { class: "bar", hidden: true }, h("i", { style: "width:0%" }));
  let installing = false;
  const go_ = h("button", { class: "primary", onclick: async () => {
    go_.disabled = true; bar.hidden = false;
    // Closing the dialog (X, Esc, outside click) while installing aborts the installation.
    modalOnClose = () => { if (installing) invoke("cancel_pack_install").catch(() => {}); };
    installing = true;
    try {
      const r = await invoke(x.source === "curseforge" ? "install_curseforge_pack" : "install_modrinth_pack", { project: x.id, version: sel.value, icon: x.icon || null });
      const inst = r.instance || r;
      installing = false; state.installBar = null;
      instCache = null; closeModal(); toast("„" + inst.name + "“ installiert"); go("library"); manualDialog(r.manual || []);
    } catch (e) {
      installing = false; state.installBar = null;
      if (String(e) === "cancelled") { toast("Installation abgebrochen"); return; }
      toast(String(e), true); go_.disabled = false; bar.hidden = true;
    }
  } }, "Installieren");
  body.replaceChildren(h("h2", null, x.title), h("div", { class: "field" }, h("span", null, "Version"), sel), bar,
    h("div", { class: "row" }, go_, h("button", { onclick: closeModal }, "Abbrechen")));
  state.installBar = bar;
}
