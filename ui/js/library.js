"use strict";
const LOADERS = ["vanilla", "fabric", "quilt", "forge", "neoforge"];
async function viewLibrary(v) {
  state.instances = await listInstances();
  const head = h("div", { class: "head" },
    h("h1", null, "Bibliothek"),
    h("div", { class: "row" },
      h("button", { onclick: importFile }, "Modpack importieren"),
      h("button", { class: "primary", onclick: createDialog }, "Neues Modpack")));
  v.append(head);
  if (!state.instances.length) {
    v.append(h("div", { class: "empty block" },
      h("p", null, "Noch keine Instanzen."),
      h("div", { class: "row" },
        h("button", { class: "primary", onclick: createDialog }, "Neues Modpack"),
        h("button", { onclick: () => go("discover") }, "Modpacks entdecken"))));
    return;
  }
  v.append(h("div", { class: "grid" }, state.instances.map(instanceCard)));
}
function instanceCard(i) {
  const busy = state.busy.has(i.id);
  const stop = (fn) => (e) => { e.stopPropagation(); fn(); };
  const el = h("article", { class: "block card", tabindex: "0", role: "link", "aria-label": i.name, onclick: () => openInstance(i) },
    h("div", { class: "top" },
      instIcon(i),
      h("div", { style: "min-width:0" },
        h("div", { class: "title", title: i.name }, i.name),
        tags(i),
        h("div", { class: "small dim", style: "margin-top:4px" }, ago(i.last_played)),
        fmtPlaytime(i.playtime_secs) ? h("div", { class: "small dim" }, fmtPlaytime(i.playtime_secs)) : null)),
    busy ? statusLine(i) : null,
    h("div", { class: "actions" },
      playBtn(i, busy, "", stop),
      h("button", { class: "danger del", disabled: busy, title: "Löschen", onclick: stop(() => confirmDelete(i)) }, "✕")));
  el.addEventListener("keydown", (e) => { if (e.key === "Enter" && e.target === el) openInstance(i); });
  return el;
}
// A second game window of a running modpack (Options can switch this off).
state.extra = new Map();
function extraBtn(i) {
  if (!state.running.has(i.id) || state.settings.multi_start === false) return null;
  const n = state.extra.get(i.id) || 0;
  const start = async (btn) => {
    closeModal(); btn.disabled = true;
    try { await invoke("launch_instance", { id: i.id, join: null, extra: true }); } catch (e) { toast(String(e), true); }
    btn.disabled = false;
  };
  return h("button", { title: "Startet das Modpack ein weiteres Mal, zum Beispiel für einen zweiten Spieler im LAN", onclick: (e) => {
    const btn = e.currentTarget;
    openModal(h("div", { class: "dialog block" },
      h("h2", null, "Weiteres Fenster starten?"),
      h("p", { class: "dim" }, "Beide Fenster nutzen denselben Modpack-Ordner. Öffne darin nicht dieselbe Welt zweimal, sonst kann sie beschädigt werden. Mehr als vier Fenster gehen nicht. „Beenden“ schließt alle Fenster."),
      h("div", { class: "row" }, h("button", { class: "primary", onclick: () => start(btn) }, "Fenster starten"), h("button", { onclick: closeModal }, "Abbrechen"))));
  } }, n ? "Weiteres Fenster (" + (n + 1) + " offen)" : "Weiteres Fenster");
}
listen("extra-started", (e) => { state.extra.set(e.payload, (state.extra.get(e.payload) || 0) + 1); refresh(); });
listen("extra-exited", (e) => { state.extra.set(e.payload, Math.max(0, (state.extra.get(e.payload) || 1) - 1)); refresh(); });
function playBtn(i, busy, size = "", wrap = (fn) => fn) {
  return busy
    ? h("button", { class: "danger " + size, title: "Abbrechen bzw. Spiel beenden", onclick: wrap(() => invoke("stop_instance", { id: i.id }).catch((e) => toast(String(e), true))) }, state.running.has(i.id) ? "Beenden" : "Abbrechen")
    : h("button", { class: ("primary " + size).trim(), onclick: wrap(() => play(i)) }, "Spielen");
}
async function play(i, join = null) {
  if (!state.player?.signed_in) { loginPrompt(); return; }
  state.busy.add(i.id); state.progress = null; render();
  try {
    const note = await invoke("launch_instance", { id: i.id, join });
    if (note === "join-unsupported") toast("Direktes Beitreten geht mit dieser Minecraft-Version nicht. Das Modpack startet normal.");
  } catch (e) {
    state.busy.delete(i.id);
    if (String(e).includes("login required")) { await loadPlayer(); renderNav(); loginPrompt(); } else toast(String(e), true);
    render();
  }
}
// A world or server from the start page: open its modpack, start it, and let the game join on its own.
async function joinRecent(r) {
  const i = state.instances.find((x) => x.id === r.instance_id);
  if (!i) return;
  state.instanceId = i.id; state.tab = "content";
  go("instance");
  if (state.busy.has(i.id)) { toast("Das Modpack läuft schon"); return; }
  await play(i, { kind: r.kind, target: r.target });
}
function exportChoice(i) {
  const pick = async (format) => {
    closeModal();
    try {
      const path = await call("export_instance", { id: i.id, format });
      if (path) toast("Gespeichert: " + path);
    } catch { /* the toast already explains */ }
  };
  const option = (format, title, text) => h("button", { class: "block exportopt", onclick: () => pick(format) }, h("strong", null, title), h("span", { class: "small dim" }, text));
  openModal(h("div", { class: "dialog block" },
    h("h2", null, "„" + i.name + "“ exportieren"),
    option("mrpack", "Modrinth-Modpack (.mrpack)", "Zum Teilen. Mods von Modrinth werden verlinkt statt eingepackt, Einstellungen und andere Dateien sind dabei. Ohne Welten."),
    option("zip", "Sicherung (.zip)", "Alles in einer Datei, auch Welten und Screenshots. Zum Sichern oder Umziehen."),
    h("div", { class: "row" }, h("button", { onclick: closeModal }, "Abbrechen"))));
}
function confirmDelete(i) {
  openModal(h("div", { class: "dialog block" },
    h("h2", null, "„" + i.name + "“ löschen?"),
    h("p", { class: "dim" }, "Welten, Mods und Einstellungen dieser Instanz werden von der Festplatte gelöscht."),
    h("div", { class: "row" },
      h("button", { class: "danger", onclick: async () => { await call("delete_instance", { id: i.id }); closeModal(); refresh(); } }, "Endgültig löschen"),
      h("button", { onclick: closeModal }, "Abbrechen"))));
}
