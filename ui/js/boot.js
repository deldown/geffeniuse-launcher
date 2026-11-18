"use strict";
listen("progress", (e) => onProgress(e.payload));
listen("game-exited", (e) => { state.busy.delete(e.payload[0]); state.running.delete(e.payload[0]); state.progress = null; if (e.payload[1]) toast("Spiel beendet mit Code " + e.payload[1], true); refresh(); });
// Log lines arrive in batches; they are appended as one text node per frame and old chunks are
// dropped by count, so the page never re-reads or re-lays-out its whole log.
const LOG_MAX_LINES = 3000;
const logQueue = [];
let logQueued = false;
let logChunks = [];
function flushLog() {
  logQueued = false;
  const el = state.logEl;
  const lines = logQueue.splice(0);
  if (!el || !el.isConnected || !lines.length) return;
  const stick = el.scrollTop + el.clientHeight >= el.scrollHeight - 24;
  const chunk = document.createTextNode((logChunks.length ? "\n" : "") + lines.join("\n"));
  el.append(chunk);
  logChunks.push([chunk, lines.length]);
  let total = logChunks.reduce((n, c) => n + c[1], 0);
  while (total > LOG_MAX_LINES && logChunks.length > 1) { const [old, n] = logChunks.shift(); old.remove(); total -= n; }
  if (state.logEmpty) state.logEmpty.hidden = true;
  if (stick) el.scrollTop = el.scrollHeight;
}
listen("game-log", (e) => {
  const [id, lines] = e.payload;
  if (id !== state.logId) return;
  const latest = lines.slice(-LOG_MAX_LINES);
  const excess = logQueue.length + latest.length - LOG_MAX_LINES;
  if (excess > 0) logQueue.splice(0, excess);
  logQueue.push(...latest);
  if (!logQueued) { logQueued = true; requestAnimationFrame(flushLog); }
});
// Files dropped on the window go into the open modpack, or a .mrpack is imported.
const SKIP_REASONS = {
  "not a file": "keine Datei", "too large": "zu groß", "Vanilla loads no mods": "Vanilla lädt keine Mods", "create a world first": "erstelle zuerst eine Welt",
  "not a resource pack, shader or data pack": "kein Ressourcenpaket, Shader oder Datapack", "unsupported file type": "Dateityp nicht unterstützt", "already there": "schon vorhanden",
};
const dropzone = h("div", { id: "dropzone", hidden: true });
document.body.append(dropzone);
// Built when shown, so it follows the language chosen after start.
const showDrop = (on) => {
  if (on && dropzone.hidden) dropzone.replaceChildren(h("div", { class: "dropcard" }, h("strong", null, "Zum Hinzufügen loslassen"), h("span", { class: "small dim" }, ".jar, .zip oder .mrpack")));
  dropzone.hidden = !on;
};
async function handleDrop(paths) {
  if (!paths.length) return;
  if (state.route === "instance" && state.instanceId) {
    try {
      const r = await invoke("add_files", { id: state.instanceId, paths });
      const n = r.added.length;
      if (n) toast(n + (n === 1 ? " Datei hinzugefügt" : " Dateien hinzugefügt"));
      if (r.skipped.length) toast(r.skipped.slice(0, 2).map(([name, why]) => name + ": " + tr(SKIP_REASONS[why] || why)).join(" · "), !n);
      if (n) {
        if (r.added.some((a) => a.kind === "mod")) state.identified.delete(state.instanceId);
        state.tabRefresh?.();
      }
    } catch (e) { toast(String(e), true); }
    return;
  }
  const packs = paths.filter((p) => /\.mrpack$/i.test(p));
  if (!packs.length) { toast("Öffne ein Modpack, um Dateien hinzuzufügen"); return; }
  toast("Import läuft …");
  for (const path of packs) {
    try { const r = await invoke("import_pack_file", { path }); toast("„" + r.instance.name + "“ importiert"); manualDialog(r.manual); }
    catch (e) { toast(String(e), true); }
  }
  refresh();
}
listen("tauri://drag-enter", () => showDrop(true));
listen("tauri://drag-leave", () => showDrop(false));
listen("tauri://drag-drop", (e) => { showDrop(false); handleDrop(e.payload?.paths || []); });
const CRASH_TEXT = {
  java_too_old: ["Java ist zu alt", "Dieses Modpack braucht ein neueres Java. Lösche in den Optionen den eigenen Java-Pfad, dann lädt der Launcher das richtige Java selbst."],
  java_too_new: ["Java ist zu neu", "Dieses alte Spiel läuft nur mit Java 8. Lösche in den Optionen den eigenen Java-Pfad, dann nimmt der Launcher das passende Java."],
  out_of_memory: ["Zu wenig Arbeitsspeicher", "Dem Spiel ging der Speicher aus. Gib dem Modpack in den Einstellungen mehr RAM."],
  duplicate_mods: ["Ein Mod ist doppelt vorhanden", "Entferne die doppelte Datei im Mods-Tab. Meist ist es eine alte Version neben der neuen."],
  missing_dependency: ["Ein benötigter Mod fehlt", "Ein Mod braucht einen anderen Mod, der nicht installiert ist. Suche ihn unter „Hinzufügen“."],
  incompatible_mods: ["Mods vertragen sich nicht", "Mindestens zwei Mods passen nicht zusammen oder nicht zur Minecraft-Version. Prüfe im Mods-Tab, ob Updates da sind."],
  mod_conflict: ["Ein Mod stört einen anderen", "Zwei Mods greifen in dieselbe Stelle im Spiel ein. Deaktiviere den zuletzt hinzugefügten Mod, oder suche nach Updates."],
  graphics: ["Grafikproblem", "Das Spiel konnte kein Fenster mit der Grafikkarte öffnen. Aktualisiere den Grafiktreiber."],
  killed: ["Das System hat das Spiel beendet", "Meist war der Arbeitsspeicher voll. Verringere den RAM des Modpacks oder schließe andere Programme."],
};
listen("game-crashed", (e) => {
  if (state.settings.crash_help === false) return;
  const [id, hint] = e.payload, text = CRASH_TEXT[hint.kind];
  if (!text) return;
  const i = state.instances.find((x) => x.id === id);
  openModal(h("div", { class: "dialog block" },
    h("h2", null, text[0]),
    h("p", { class: "dim" }, text[1]),
    hint.subject ? h("p", { class: "small inl" }, h("span", null, "Genannt wird:"), h("strong", null, hint.subject)) : null,
    h("div", { class: "row" },
      h("button", { class: "primary", onclick: () => { closeModal(); if (i) { state.instanceId = i.id; state.tab = hint.kind.includes("mod") || hint.kind === "duplicate_mods" || hint.kind === "missing_dependency" ? "content" : "settings"; go("instance"); } } }, "Zum Modpack"),
      h("button", { onclick: () => { closeModal(); if (i) { state.instanceId = i.id; state.tab = "console"; go("instance"); } } }, "Konsole ansehen"),
      h("button", { onclick: closeModal }, "Schließen"))));
});
listen("game-started", (e) => { state.running.add(e.payload); state.progress = null; refresh(); });

// Quiet startup check: a banner appears only if a newer version exists; failures stay silent.
async function checkUpdate() {
  if (state.settings.auto_update_check === false) return;
  let u;
  try { u = await invoke("check_update"); } catch { return; }
  if (!u) return;
  const bar = h("div", { class: "toast update" }, h("div", null, "Version " + u.version + " ist verfügbar"),
    h("div", { class: "row", style: "margin-top:8px" },
      u.can_apply
        ? h("button", { class: "primary small", onclick: async (e) => { e.target.disabled = true; e.target.textContent = tr("Lädt …"); try { await invoke("apply_update"); bar.replaceChildren(h("div", null, "Aktualisiert."), h("div", { class: "row", style: "margin-top:8px" }, h("button", { class: "primary small", onclick: () => invoke("restart_app").catch((x) => toast(String(x), true)) }, "Jetzt neu starten"))); } catch (x) { toast(String(x), true); e.target.disabled = false; e.target.textContent = "Aktualisieren"; } } }, "Aktualisieren")
        : (u.download_url ? h("button", { class: "primary small", onclick: () => openUrl(u.download_url) }, "Herunterladen") : null),
      h("button", { class: "small", onclick: () => bar.remove() }, "Später")));
  $("#toast").append(bar);
}

(async () => {
  state.settings = await invoke("get_settings");
  applySettings();
  try { await loadPlayer(); } catch { state.player = null; }
  // A game started before the launcher was closed is still running: show it as such.
  for (const id of await invoke("running_instances").catch(() => [])) { state.busy.add(id); state.running.add(id); }
  renderNav(); render(); refreshSkin(); loadVersions().catch(() => {}); if (state.settings.version_pictures !== false) loadVersionImages();
  listAccounts().then((l) => l.forEach((a) => a.skin && skinImage(a.skin))).catch(() => {});
  if (state.player?.skin) skinImage(state.player.skin);
  if (!state.settings.setup_done) showSetup();
  new ResizeObserver(() => movePills(true)).observe($("#nav"));
  document.fonts?.ready.then(() => movePills(true));
  setInterval(applyTheme, 5 * 60 * 1000);
  setTimeout(checkUpdate, 3000);
})();
