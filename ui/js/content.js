"use strict";
const KINDS = [["mod", "Mods"], ["resourcepack", "Ressourcenpakete"], ["shader", "Shader"], ["datapack", "Datapacks"]];
const KIND_NAME = { mod: "Mod", resourcepack: "Ressourcenpaket", shader: "Shader", datapack: "Datapack" };
const baseFile = (c) => c.file.replace(/\.disabled$/, "");
const contentId = (c) => c.kind + "|" + (c.world || "") + "|" + baseFile(c);
const contentTitle = (c) => c.title || baseFile(c).replace(/\.(jar|zip)$/i, "");
const byName = (a, b) => contentTitle(a).toLowerCase().localeCompare(contentTitle(b).toLowerCase());
const SORTS = {
  name: byName,
  "name-desc": (a, b) => byName(b, a),
  size: (a, b) => b.size - a.size || byName(a, b),
  kind: (a, b) => KINDS.findIndex((k) => k[0] === a.kind) - KINDS.findIndex((k) => k[0] === b.kind) || byName(a, b),
  active: (a, b) => Number(b.enabled) - Number(a.enabled) || byName(a, b),
};
const FILTER_LABELS = [["all", "Alle Quellen"], ["missing", "Fehlt"], ["manual", "Selbst installiert"], ["pack", "Aus Modpack"], ["off", "Deaktiviert"], ["update", "Update verfügbar"]];
const SORT_LABELS = [["name", "Name (A–Z)"], ["name-desc", "Name (Z–A)"], ["size", "Größe"], ["kind", "Typ"], ["active", "Aktive zuerst"]];

async function contentInstalled(body, i, ctx) {
  const items = await call("list_content", { id: i.id });
  if (!items.length) {
    body.append(h("div", { class: "empty block" }, h("p", null, "Noch kein Inhalt in diesem Modpack."),
      h("p", { class: "small" }, "Füge Mods, Ressourcenpakete, Shader oder Datapacks hinzu, oder ziehe Dateien einfach hierher."),
      h("button", { class: "primary", onclick: () => ctx.go("add") }, "Inhalt hinzufügen")));
    return;
  }
  const view = (state.contentView ||= { kind: "all", sort: "name", q: "", filter: "all" });
  view.open ||= new Set();
  // The first time a modpack with missing files is opened, show those files.
  state.missingSeen ||= new Set();
  if (items.some((c) => c.missing) && !state.missingSeen.has(i.id)) { state.missingSeen.add(i.id); view.filter = "missing"; }
  if (view.filter === "update" || (view.filter === "missing" && !items.some((c) => c.missing))) view.filter = "all";
  const selected = new Set(), rows = new Map(), updates = new Map();
  const list = h("div", { class: "contentlist" });
  const chips = h("div", { class: "filters" });
  const summary = h("span", { class: "small dim", style: "display:flex;gap:14px" });
  const bulk = h("div", { class: "bulk", hidden: true });
  const all = h("input", { type: "checkbox", class: "pick", "aria-label": "Alle auswählen" });
  const search = h("input", { type: "search", placeholder: "Inhalt durchsuchen …", "aria-label": "Inhalt durchsuchen", value: view.q });
  const sort = dropdown({ options: SORT_LABELS, value: view.sort, label: "Sortieren", onChange: (k) => { view.sort = k; paint(); } });
  const groupNames = state.settings.content_groups === false ? [] : [...new Set(items.map((c) => c.group).filter(Boolean))].sort((a, b) => a.localeCompare(b));
  if (view.filter.startsWith("g:") && !groupNames.includes(view.filter.slice(2))) view.filter = "all";
  const filter = dropdown({ options: [...FILTER_LABELS.filter(([k]) => k !== "missing" || items.some((c) => c.missing)), ...groupNames.map((g) => ["g:" + g, "Gruppe: " + g])], value: view.filter, label: "Filtern", onChange: (k) => { view.filter = k; paint(); } });
  const layoutBtn = h("button", { class: "small", hidden: !groupNames.length, title: "Gruppen als Ordner oder als einfache Liste zeigen", onclick: async () => {
    await saveSettings({ groups_as_folders: state.settings.groups_as_folders === false });
    layoutBtn.textContent = state.settings.groups_as_folders === false ? "Als Ordner" : "Als Liste"; paint();
  } }, state.settings.groups_as_folders === false ? "Als Ordner" : "Als Liste");
  const updBar = h("div", { class: "bulk", hidden: true });
  const updBtn = h("button", { class: "small", hidden: state.settings.mod_updates === false || i.loader === "vanilla" || !items.some((c) => c.kind === "mod"), onclick: () => checkUpdates() }, "Nach Updates suchen");

  const visible = () => {
    const q = view.q.trim().toLowerCase();
    const origin = { all: () => true, manual: (c) => c.manual, pack: (c) => !c.manual && !c.missing, off: (c) => !c.enabled, update: (c) => updates.has(baseFile(c)), missing: (c) => c.missing }[view.filter] || (view.filter.startsWith("g:") ? (c) => c.group === view.filter.slice(2) : () => true);
    return items.filter((c) => (view.kind === "all" || c.kind === view.kind) && origin(c) && (!q || (contentTitle(c) + " " + baseFile(c)).toLowerCase().includes(q))).sort((a, b) => Number(!!b.missing) - Number(!!a.missing) || Number(updates.has(baseFile(b))) - Number(updates.has(baseFile(a))) || (SORTS[view.sort] || byName)(a, b));
  };
  const picked = () => items.filter((c) => !c.missing && selected.has(contentId(c)));
  const count = (n, label) => h("span", { class: "inl" }, h("span", null, label), h("span", { class: "count" }, String(n)));
  const syncSelection = () => {
    const vis = visible();
    all.checked = vis.length > 0 && vis.every((c) => selected.has(contentId(c)));
    all.indeterminate = !all.checked && vis.some((c) => selected.has(contentId(c)));
    bulk.hidden = selected.size === 0;
    if (!selected.size) return;
    bulk.replaceChildren(
      h("span", { class: "inl" }, h("strong", null, String(selected.size)), h("span", null, "ausgewählt")),
      h("span", { class: "grow" }),
      h("button", { class: "small", onclick: () => setMany(true) }, "Aktivieren"),
      h("button", { class: "small", onclick: () => setMany(false) }, "Deaktivieren"),
      state.settings.content_groups === false ? null : h("button", { class: "small", onclick: () => groupDialog() }, "Gruppe …"),
      h("button", { class: "small danger", onclick: removeMany }, "Entfernen"),
      h("button", { class: "small ghost", onclick: () => { selected.clear(); paint(); } }, "Auswahl aufheben"));
  };
  const updateSummary = () => { const real = items.filter((c) => !c.missing); summary.replaceChildren(count(real.filter((c) => c.enabled).length + " / " + real.length, "aktiv"), ...(real.length < items.length ? [count(items.length - real.length, "fehlen")] : [])); };
  const folder = (name, members) => {
    const open = view.open.has(name) || !!view.q.trim();
    const head = h("button", { class: "folderhead", "aria-expanded": String(open), onclick: () => { if (view.open.has(name)) view.open.delete(name); else view.open.add(name); paint(); } },
      h("span", { class: "chev", "aria-hidden": "true" }), h("strong", null, name), h("span", { class: "count" }, String(members.length)));
    return h("div", { class: "folder" }, head, open ? h("div", { class: "folderbody" }, ...members.map(row)) : null);
  };
  const paint = () => {
    if (view.kind !== "all" && !items.some((c) => c.kind === view.kind)) view.kind = "all";
    const vis = visible();
    const asFolders = state.settings.groups_as_folders !== false && groupNames.length && !view.filter.startsWith("g:");
    const body = asFolders
      ? [...groupNames.map((g) => [g, vis.filter((c) => c.group === g)]).filter(([, m]) => m.length).map(([g, m]) => folder(g, m)), ...vis.filter((c) => !c.group).map(row)]
      : vis.map(row);
    list.replaceChildren(...(body.length ? body : [h("p", { class: "dim", style: "padding:14px 4px" }, "Nichts gefunden.")]));
    chips.replaceChildren(...[["all", "Alle"], ...KINDS].filter(([k]) => k === "all" || items.some((c) => c.kind === k)).map(([k, label]) =>
      h("button", { class: "chipbtn", "aria-pressed": String(view.kind === k), onclick: () => { view.kind = k; paint(); } }, count(k === "all" ? items.length : items.filter((c) => c.kind === k).length, label))));
    updateSummary();
    syncSelection();
  };

  const applyEnabled = async (c, on) => {
    await call("toggle_content", { id: i.id, kind: c.kind, file: c.file, world: c.world || null, enabled: on });
    c.enabled = on; c.file = baseFile(c) + (on ? "" : ".disabled");
    rows.get(c)?.classList.toggle("off", !on);
    updateSummary();
  };
  const removeOne = async (c) => {
    await call("remove_content", { id: i.id, kind: c.kind, file: c.file, world: c.world || null });
    items.splice(items.indexOf(c), 1); selected.delete(contentId(c)); rows.delete(c);
    toast("Entfernt");
    if (!items.length) ctx.refresh(); else paint();
  };
  const setMany = async (on) => {
    for (const c of picked()) if (c.enabled !== on) { try { await applyEnabled(c, on); } catch { break; } }
    paint();
  };
  const groupDialog = () => {
    const files = picked().map((c) => c.file);
    const apply = async (group) => {
      closeModal();
      try { await call("set_content_group", { id: i.id, files, group }); toast(group ? "In Gruppe „" + group + "“ gelegt" : "Aus der Gruppe genommen"); ctx.refresh(); } catch { /* the toast already explains */ }
    };
    const name = h("input", { type: "text", maxlength: 40, placeholder: "Neue Gruppe, z. B. Performance", "aria-label": "Neue Gruppe" });
    const create = () => { if (name.value.trim()) apply(name.value.trim()); };
    name.addEventListener("keydown", (e) => { if (e.key === "Enter") create(); });
    openModal(h("div", { class: "dialog block" },
      h("h2", null, "Gruppe für " + files.length + (files.length === 1 ? " Eintrag" : " Einträge")),
      ...groupNames.map((g) => h("button", { class: "block exportopt", onclick: () => apply(g) }, h("strong", null, g))),
      h("div", { class: "row", style: "margin:10px 0" }, name, h("button", { class: "primary", onclick: create }, "Anlegen")),
      h("div", { class: "row" }, h("button", { onclick: () => apply(null) }, "Aus Gruppe nehmen"), h("button", { onclick: closeModal }, "Abbrechen"))));
    name.focus();
  };
  const removeMany = () => {
    const n = selected.size;
    openModal(h("div", { class: "dialog block" },
      h("h2", null, "Auswahl entfernen?"),
      h("p", { class: "dim inl" }, h("strong", null, String(n)), h("span", null, "Einträge werden endgültig gelöscht.")),
      h("div", { class: "row" },
        h("button", { class: "danger", onclick: async () => {
          closeModal();
          for (const c of picked()) { try { await call("remove_content", { id: i.id, kind: c.kind, file: c.file, world: c.world || null }); items.splice(items.indexOf(c), 1); rows.delete(c); } catch { break; } }
          selected.clear(); toast("Entfernt");
          if (!items.length) ctx.refresh(); else paint();
        } }, "Entfernen"),
        h("button", { onclick: closeModal }, "Abbrechen"))));
  };
  const runUpdates = async (files, btn) => {
    btn.disabled = true;
    try {
      const done = await call("update_mods", { id: i.id, files });
      toast(done.length === 1 ? "1 Mod aktualisiert" : done.length + " Mods aktualisiert");
      ctx.refresh();
    } catch { btn.disabled = false; }
  };
  const paintUpdates = () => {
    updBar.hidden = updates.size === 0;
    updBar.replaceChildren(
      h("span", { class: "inl" }, h("strong", null, String(updates.size)), h("span", null, updates.size === 1 ? "Update verfügbar" : "Updates verfügbar")),
      h("span", { class: "grow" }),
      h("button", { class: "small primary", onclick: (e) => runUpdates(items.filter((c) => updates.has(baseFile(c))).map((c) => c.file), e.currentTarget) }, "Alle aktualisieren"));
    paint();
  };
  const checkUpdates = async () => {
    updBtn.disabled = true; updBtn.textContent = "Suche …";
    try {
      const found = await call("check_mod_updates", { id: i.id });
      updates.clear();
      for (const u of found) updates.set(u.file.replace(/\.disabled$/, ""), u.version);
      toast(found.length ? "" + found.length + (found.length === 1 ? " Update gefunden" : " Updates gefunden") : "Alle Mods sind aktuell");
      if (found.length && state.settings.mod_updates_autofilter !== false) { view.filter = "update"; filter.value = "update"; }
      paintUpdates();
    } catch { /* the toast already explains */ } finally { updBtn.disabled = false; updBtn.textContent = "Nach Updates suchen"; }
  };
  const missingRow = (c) => {
    const el = h("div", { class: "block modrow missing" }, h("span"), logo(null),
      h("div", { class: "name", style: "min-width:0" },
        h("div", { class: "title", style: "overflow:hidden;text-overflow:ellipsis;white-space:nowrap" }, c.file),
        h("div", { class: "small meta" }, h("span", { class: "kindtag missingtag" }, "Fehlt"), h("span", { class: "dim" }, "Lade die Datei herunter und lege sie in den Ordner oder hierher."))),
      h("div", { class: "row", style: "gap:8px" },
        c.cf_id ? h("button", { class: "small primary", onclick: () => invoke("open_url", { url: "https://www.curseforge.com/projects/" + c.cf_id }).catch((e) => toast(String(e), true)) }, "Auf CurseForge öffnen") : null,
        h("button", { class: "small", onclick: () => invoke("open_instance_folder", { id: i.id, folder: c.kind === "mod" ? "mods" : c.kind === "shader" ? "shaderpacks" : "resourcepacks" }).catch((e) => toast(String(e), true)) }, "Ordner öffnen"),
        h("button", { class: "small ghost", title: "Nicht mehr als fehlend anzeigen", onclick: async () => { await call("dismiss_missing", { id: i.id, name: c.file }); ctx.refresh(); } }, "Ausblenden")));
    return el;
  };
  const row = (c) => {
    if (c.missing) return missingRow(c);
    const pick = h("input", { type: "checkbox", class: "pick", checked: selected.has(contentId(c)), "aria-label": "Auswählen", onchange: (e) => { if (e.target.checked) selected.add(contentId(c)); else selected.delete(contentId(c)); syncSelection(); } });
    const sw = h("label", { class: "switch", title: c.enabled ? "Deaktivieren" : "Aktivieren" },
      h("input", { type: "checkbox", checked: c.enabled, "aria-label": "Aktiv", onchange: async (e) => { const on = e.target.checked; try { await applyEnabled(c, on); } catch { e.target.checked = !on; } } }),
      h("span", { class: "knob" }));
    const el = h("div", { class: "block modrow" + (c.enabled ? "" : " off") }, pick, logo(c.icon),
      h("div", { class: "name", style: "min-width:0" },
        h("div", { class: "title", style: "overflow:hidden;text-overflow:ellipsis;white-space:nowrap" }, contentTitle(c)),
        h("div", { class: "small dim meta" }, h("span", { class: "kindtag" }, KIND_NAME[c.kind]), c.group ? h("span", { class: "kindtag grouptag" }, c.group) : null, h("span", null, [c.world, baseFile(c), fmtMb(c.size)].filter(Boolean).join(" · ")))),
      h("div", { class: "row", style: "gap:12px" },
        updates.has(baseFile(c)) ? h("button", { class: "small primary", title: "Neue Version " + updates.get(baseFile(c)), onclick: (e) => runUpdates([c.file], e.currentTarget) }, "Aktualisieren") : null,
        sw),
      h("button", { class: "danger", onclick: () => removeOne(c) }, "Entfernen"));
    rows.set(c, el);
    return el;
  };

  all.addEventListener("change", () => { for (const c of visible().filter((x) => !x.missing)) { if (all.checked) selected.add(contentId(c)); else selected.delete(contentId(c)); } paint(); });
  search.addEventListener("input", () => { view.q = search.value; paint(); });
  body.append(
    h("div", { class: "toolbar tools" }, search, sort, filter, layoutBtn, updBtn),
    h("div", { class: "toolbar chiprow" }, all, chips, h("span", { class: "grow" }), summary),
    updBar, bulk, list);
  paint();
  if (!state.identified.has(i.id) && items.some((c) => c.kind === "mod" && !c.icon)) {
    state.identified.add(i.id);
    invoke("identify_mods", { id: i.id }).then((n) => {
      if (n > 0 && state.route === "instance" && state.instanceId === i.id && state.tab === "content") ctx.refresh();
    }).catch(() => {});
  }
}

async function contentAdd(body, i, ctx) {
  const kinds = i.loader === "vanilla" ? KINDS.filter(([k]) => k !== "mod") : KINDS;
  if (!kinds.some(([k]) => k === state.addKind)) state.addKind = kinds[0][0];
  const panel = h("div", null);
  const fill = async (kind) => {
    const s = state.settings;
    const sources = kind === "mod" ? [["modrinth", "Modrinth", s.modrinth_enabled], ["curseforge", "CurseForge", s.curseforge_enabled && (s.curseforge_key || s.curseforge_proxy !== false)]].filter((x) => x[2]) : [];
    if (kind === "mod" && !sources.some((x) => x[0] === state.modSource)) state.modSource = sources[0]?.[0] || "modrinth";
    const installed = new Set((await call("list_content", { id: i.id })).map((c) => c.project_id).filter(Boolean));
    const worlds = kind === "datapack" ? await call("list_worlds", { id: i.id }) : [];
    const world = kind === "datapack" && worlds.length
      ? dropdown({ options: worlds.map((w) => [w.folder, w.folder]), value: worlds.some((w) => w.folder === state.addWorld) ? state.addWorld : worlds[0].folder, label: "Welt", onChange: (k) => { state.addWorld = k; } })
      : null;
    if (world && !worlds.some((w) => w.folder === state.addWorld)) state.addWorld = worlds[0].folder;
    const results = h("div", null);
    const q = h("input", { type: "search", placeholder: kind === "mod" ? "Mods suchen …" : "Suchen …", "aria-label": "Suchen" });
    let latest = 0;
    const run = async () => {
      const mine = ++latest;
      results.replaceChildren(skeleton(4));
      let hits;
      try { hits = await invoke(kind === "mod" ? "search_mods" : "search_content", kind === "mod" ? { id: i.id, query: q.value, offset: 0, source: state.modSource } : { id: i.id, kind, query: q.value, offset: 0 }); }
      catch (e) { if (mine === latest) results.replaceChildren(h("div", { class: "empty block" }, String(e))); return; }
      if (mine !== latest) return;
      results.replaceChildren(...(hits.length ? hits.map((x) => modHit(i, x, installed, kind, () => state.addWorld || null)) : [h("p", { class: "dim" }, kind === "mod" ? "Nichts gefunden für " + i.mc_version + " mit " + i.loader + "." : "Nichts gefunden.")]));
    };
    q.addEventListener("keydown", (e) => { if (e.key === "Enter") run(); });
    panel.replaceChildren(...[
      h("div", { class: "row", style: "margin-bottom:14px" },
        sources.length > 1 ? h("div", { class: "seg" }, sources.map(([id, label]) => h("button", { "aria-pressed": String(state.modSource === id), onclick: () => { state.modSource = id; fill(kind); } }, label))) : null,
        world, q, h("button", { class: "primary", onclick: run }, "Suchen")),
      kind === "datapack" && !worlds.length ? h("p", { class: "small dim", style: "margin-bottom:12px" }, "Datapacks gehören zu einer Welt. Erstelle zuerst eine Welt, indem du das Modpack startest.") : null,
      kind === "mod" ? h("p", { class: "small dim", style: "margin-bottom:12px" }, "Zeigt nur Mods für Minecraft " + i.mc_version + " mit " + i.loader + ". Beim Installieren fragen wir, ob benötigte Mods mitkommen sollen.") : null,
      results].filter(Boolean));
    run();
  };
  body.append(h("div", { class: "row", style: "margin-bottom:14px" }, segmented(kinds, state.addKind, (k) => { state.addKind = k; fill(k); })), panel);
  await fill(state.addKind);
}

async function worldsTab(body, i) {
  const worlds = await call("list_worlds", { id: i.id });
  const fmtSize = (n) => n > 1e9 ? (n / 1e9).toFixed(1) + " GB" : Math.max(1, Math.round(n / 1e6)) + " MB";
  if (!worlds.length) { body.append(h("div", { class: "block empty" }, "Noch keine Welten. Sie erscheinen, sobald du in diesem Modpack gespielt hast.")); return; }
  body.append(...worlds.map((w) => h("div", { class: "block hit" },
    cube("vanilla", "sm"),
    h("div", { style: "min-width:0" }, h("div", { class: "title" }, w.folder),
      h("div", { class: "small dim" }, fmtSize(w.size) + " · " + new Date(w.modified * 1000).toLocaleDateString())))));
}

async function consoleTab(body, i) {
  const lines = await call("get_log", { id: i.id });
  const pre = h("pre", { class: "log sunk", tabindex: "0", "aria-live": "off" });
  logQueue.length = 0;
  logChunks = [];
  if (lines.length) { const t = document.createTextNode(lines.join("\n")); pre.append(t); logChunks.push([t, lines.length]); }
  const empty = h("p", { class: "small dim", hidden: lines.length > 0 }, "Noch keine Ausgabe. Starte dieses Modpack.");
  body.append(h("div", { class: "row", style: "margin-bottom:10px; justify-content:flex-end" },
    h("button", { onclick: () => invoke("open_console", { id: i.id }).catch((e) => toast(String(e), true)) }, "In eigenem Fenster öffnen"),
    h("button", { onclick: async () => { await call("clear_log", { id: i.id }); pre.textContent = ""; logQueue.length = 0; logChunks = []; empty.hidden = false; } }, "Leeren")), empty, pre);
  state.logEl = pre; state.logEmpty = empty;
  requestAnimationFrame(() => { pre.scrollTop = pre.scrollHeight; });
}

async function memoryCard(i) {
  const total = await invoke("system_memory_mb").catch(() => 0);
  const dflt = state.settings.default_memory_mb;
  const max = Math.max(2048, Math.floor(((total || 32768) * 0.85) / 512) * 512);
  const gb = (mb) => (mb / 1024).toFixed(mb % 1024 ? 1 : 0) + " GB";
  const out = h("strong", { class: "memval" });
  const warn = h("p", { class: "small", style: "color:#ffd35a", hidden: true }, "Das ist viel im Verhältnis zu deinem Arbeitsspeicher. Zu viel kann das Spiel ausbremsen.");
  const slider = h("input", { type: "range", min: 1024, max, step: 512, "aria-label": "Arbeitsspeicher" });
  const useDefault = h("input", { type: "checkbox", class: "pick", checked: i.memory_mb == null, "aria-label": "Standard verwenden" });
  slider.value = Math.min(max, i.memory_mb ?? dflt);
  const current = () => (useDefault.checked ? dflt : Number(slider.value));
  const paint = () => { out.textContent = gb(current()); slider.disabled = useDefault.checked; warn.hidden = !(total && current() > total * 0.7); };
  const save = async () => {
    const mb = useDefault.checked ? null : Number(slider.value);
    try { await call("set_instance_memory", { id: i.id, mb }); i.memory_mb = mb; toast("Gespeichert"); } catch { /* the toast already explains */ }
  };
  slider.addEventListener("input", paint);
  slider.addEventListener("change", save);
  useDefault.addEventListener("change", () => { paint(); save(); });
  paint();
  return h("section", { class: "block setting-card" },
    h("div", { class: "split" },
      h("div", null, h("h2", null, "Arbeitsspeicher"),
        h("p", { class: "small dim" }, "Wie viel RAM dieses Modpack höchstens nutzen darf. Große Modpacks brauchen mehr, aber mehr ist nicht automatisch schneller.")),
      out),
    slider,
    h("div", { class: "row between" },
      h("label", { class: "inl pointer" }, useDefault, h("span", null, "Standard verwenden"), h("span", { class: "count" }, gb(dflt))),
      total ? h("span", { class: "small dim inl" }, h("span", null, "Dein Rechner hat"), h("span", null, gb(total))) : null),
    warn);
}
async function settingsTab(body, i) {
  const globalOff = state.settings.sync_options === false;
  const sw = h("label", { class: "switch" },
    h("input", { type: "checkbox", checked: i.sync_settings !== false, "aria-label": "Einstellungen teilen", onchange: async (e) => {
      const on = e.target.checked;
      try { await call("set_instance_sync", { id: i.id, on }); i.sync_settings = on; toast(on ? "Einstellungen werden geteilt" : "Dieses Modpack teilt keine Einstellungen mehr"); }
      catch { e.target.checked = !on; }
    } }),
    h("span", { class: "knob" }));
  const run = (direction) => async (e) => {
    const btn = e.currentTarget; btn.disabled = true;
    try {
      const changed = await call("sync_now", { id: i.id, direction });
      toast(changed.length ? (direction === "save" ? "Als gemeinsame Einstellungen gespeichert" : "Gemeinsame Einstellungen übernommen") : "Schon aktuell");
    } finally { btn.disabled = false; }
  };
  const folders = [["mods", "Mods"], ["", "Modpack-Ordner"], ["resourcepacks", "Ressourcenpakete"], ["shaderpacks", "Shader"], ["saves", "Welten"], ["config", "Config"], ["logs", "Logs"]];
  const folderCard = h("section", { class: "block setting-card" },
    h("h2", null, "Ordner öffnen"),
    h("p", { class: "small dim" }, "Öffnet den Ordner dieses Modpacks im Dateimanager, zum Beispiel um Mods von Hand hineinzulegen."),
    h("div", { class: "row wrap" }, folders.map(([folder, label]) => h("button", { onclick: () => invoke("open_instance_folder", { id: i.id, folder }).catch((e) => toast(String(e), true)) }, label))));
  const manageCard = h("section", { class: "block setting-card" },
    h("h2", null, "Kopieren und exportieren"),
    h("p", { class: "small dim" }, "Dupliziere das Modpack samt Mods, Welten und Einstellungen, oder speichere es als Datei zum Teilen oder Sichern."),
    h("div", { class: "row wrap" },
      h("button", { onclick: async (e) => {
        const btn = e.currentTarget; btn.disabled = true;
        try { const copy = await call("duplicate_instance", { id: i.id }); toast("„" + copy.name + "“ erstellt"); state.instanceId = copy.id; state.tab = "content"; go("instance"); } catch { btn.disabled = false; }
      } }, "Duplizieren"),
      h("button", { onclick: () => exportChoice(i) }, "Exportieren …")));
  body.append(await memoryCard(i), manageCard, folderCard, h("section", { class: "block setting-card" },
    h("div", { class: "split" },
      h("div", null, h("h2", null, "Minecraft-Einstellungen teilen"),
        h("p", { class: "small dim" }, "Sichtfeld, Steuerung, Grafik und die Serverliste gelten für alle Modpacks, die mitmachen. Vor dem Start übernimmt dieses Modpack die gemeinsamen Werte, nach dem Beenden fließen seine Änderungen zurück."),
        h("p", { class: "small dim" }, "Nicht geteilt werden die aktiven Ressourcenpakete, die Spielversion und der zuletzt besuchte Server. Alte Versionen (bis 1.12) teilen nur untereinander, weil ihre Steuerung anders gespeichert wird.")),
      sw),
    globalOff ? h("p", { class: "small", style: "color:#ffd35a" }, "In den Optionen ist das Teilen insgesamt ausgeschaltet.") : null,
    h("div", { class: "row" },
      h("button", { onclick: run("save") }, "Meine Einstellungen als gemeinsame speichern"),
      h("button", { onclick: run("load") }, "Gemeinsame Einstellungen hier anwenden"))));
}

// Dependency lookups are cached per modpack and limited to three at a time.
const depCache = new Map();
const depKey = (i, x) => i.id + "|" + x.source + "|" + x.id;
function loadDeps(i, x) {
  const k = depKey(i, x);
  if (!depCache.has(k)) depCache.set(k, invoke("mod_dependencies", { id: i.id, project: x.id, source: x.source }).catch(() => []));
  return depCache.get(k);
}
let depSlots = 0; const depWaiting = [];
const withDepSlot = (job) => new Promise((resolve) => {
  const go = () => { depSlots++; job().then(resolve, () => resolve([])).finally(() => { depSlots--; depWaiting.shift()?.(); }); };
  depSlots < 3 ? go() : depWaiting.push(go);
});

function askDependencies(missing) {
  return new Promise((resolve) => {
    let answer = null;
    const done = (v) => { answer = v; closeModal(); };
    openModal(h("div", { class: "dialog block" },
      h("h2", null, "Benötigte Mods installieren?"),
      h("p", { class: "dim" }, "Dieser Mod braucht " + (missing.length === 1 ? "einen weiteren Mod" : missing.length + " weitere Mods") + ", damit er läuft:"),
      h("div", { class: "deplist" }, missing.map(depRow)),
      h("div", { class: "row", style: "margin-top:16px; justify-content:flex-end" },
        h("button", { onclick: () => done(false) }, "Nur diesen Mod"),
        h("button", { class: "primary", onclick: () => done(true) }, "Mit " + (missing.length === 1 ? "Abhängigkeit" : "Abhängigkeiten") + " installieren"))));
    modalOnClose = () => resolve(answer);
  });
}
const depRow = (d) => h("div", { class: "dep" }, logo(d.icon), h("span", { class: "title" }, d.title));

// Installs one search result and keeps the button in step. For mods it first asks about the mods it needs.
async function installMod(i, x, btn, installed, kind = "mod", world = null, onDone = null) {
  const key = x.source === "curseforge" ? "cf:" + x.id : x.id;
  if (kind === "datapack" && !world) { toast("Wähle zuerst eine Welt", true); return; }
  btn.disabled = true; btn.textContent = tr("Lädt …");
  const reset = () => { btn.disabled = false; btn.textContent = tr("Installieren"); };
  try {
    let deps = false;
    if (kind === "mod") {
      const missing = (await loadDeps(i, x)).filter((d) => !d.installed);
      if (missing.length) {
        const answer = await askDependencies(missing);
        if (answer === null) { reset(); return; }
        deps = answer;
        btn.disabled = true; btn.textContent = tr("Lädt …");
      }
    }
    const r = await invoke(kind === "mod" ? "install_mod" : "install_content", kind === "mod" ? { id: i.id, project: x.id, source: x.source, deps } : { id: i.id, kind, project: x.id, world });
    if (r.installed.length) {
      installed.add(key); btn.textContent = tr("Installiert");
      depCache.delete(depKey(i, x));
      toast(r.installed.length > 1 ? r.installed[r.installed.length - 1] + " und " + (r.installed.length - 1) + " benötigte Mods installiert" : r.installed[0] + " installiert");
      onDone?.();
    } else reset();
    if (r.manual.length) openModal(h("div", { class: "dialog block" },
      h("h2", null, "Manueller Download nötig"),
      h("p", { class: "dim" }, "Die Autoren erlauben für diese Mods keinen Download durch Launcher. Lade sie auf curseforge.com herunter und lege sie in den Mods-Ordner der Instanz:"),
      h("ul", null, r.manual.map((m) => h("li", null, m))), h("button", { onclick: closeModal }, "Schließen")));
  } catch (e) { toast(String(e), true); reset(); }
}
function openModProject(i, x, kind = "mod") { state.project = { source: x.source, id: x.id, hit: x, instanceId: i.id, kind }; go("project"); }
// One row per mod that x needs, each installable on its own.
function depItems(i, x, list, installed, repaint) {
  return list.map((d) => {
    const b = h("button", { class: d.installed ? "" : "primary", disabled: d.installed }, d.installed ? "Installiert" : "Installieren");
    b.onclick = async () => {
      b.disabled = true; b.textContent = tr("Lädt …");
      try {
        const r = await invoke("install_mod", { id: i.id, project: d.id, source: x.source, deps: false });
        if (r.installed.length) { d.installed = true; installed.add(x.source === "curseforge" ? "cf:" + d.id : d.id); toast(r.installed[0] + " installiert"); }
        repaint();
      } catch (e) { toast(String(e), true); b.disabled = false; b.textContent = tr("Installieren"); }
    };
    return h("div", { class: "dep" }, logo(d.icon), h("span", { class: "title" }, d.title), b);
  });
}
function modHit(i, x, installed, kind = "mod", world = () => null) {
  const fmt = new Intl.NumberFormat(undefined, { notation: "compact" });
  const key = x.source === "curseforge" ? "cf:" + x.id : x.id;
  const btn = h("button", { class: "primary" }, installed.has(key) ? "Installiert" : "Installieren");
  btn.disabled = installed.has(key);
  const depChip = h("button", { class: "depchip", hidden: true, "aria-expanded": "false" });
  const depPanel = h("div", { class: "deppanel", hidden: true });
  const paintDeps = (list) => {
    const needed = list.filter((d) => !d.installed);
    depChip.hidden = list.length === 0;
    depChip.replaceChildren(h("span", { class: "chev" }), h("span", null, list.length + (list.length === 1 ? " Abhängigkeit" : " Abhängigkeiten") + (needed.length < list.length ? " · " + (list.length - needed.length) + " installiert" : "")));
    depPanel.replaceChildren(...depItems(i, x, list, installed, () => paintDeps(list)));
  };
  depChip.onclick = (e) => {
    e.stopPropagation();
    const open = depPanel.hidden;
    depPanel.hidden = !open; depChip.setAttribute("aria-expanded", String(open));
  };
  const refreshDeps = () => withDepSlot(() => loadDeps(i, x)).then((list) => { if (list.length) paintDeps(list); });
  btn.onclick = (e) => { e.stopPropagation(); installMod(i, x, btn, installed, kind, world(), () => { depCache.delete(depKey(i, x)); refreshDeps(); }); };
  const row = h("div", { class: "block hit", tabindex: "0", role: "link", "aria-label": x.title, onclick: () => openModProject(i, x, kind) },
    logo(x.icon),
    h("div", { style: "min-width:0" },
      h("div", { class: "title" }, x.title, x.author ? h("span", { class: "dim small" }, "  von " + x.author) : null),
      h("div", { class: "desc" }, x.description),
      h("div", { class: "small dim" }, fmt.format(x.downloads) + " Downloads"),
      depChip),
    btn);
  row.addEventListener("keydown", (e) => { if (e.key === "Enter" && e.target === row) openModProject(i, x, kind); });
  if (kind !== "mod") return row;
  setTimeout(refreshDeps, 0);
  return h("div", { class: "hitwrap" }, row, depPanel);
}

// Placeholder rows keep the page's shape while data loads, so it does not jump when it arrives.
const skeleton = (n = 4, height = 84) => h("div", null, Array.from({ length: n }, () => h("div", { class: "skel", style: `height:${height}px` })));
