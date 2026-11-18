"use strict";
async function viewShots(v) {
  const shots = await call("list_screenshots");
  v.append(h("div", { class: "head" }, h("h1", null, "Screenshots"), h("span", { class: "dim" }, shots.length + " Bilder")));
  if (!shots.length) { v.append(h("div", { class: "empty block" }, "Noch keine Screenshots. Drücke F2 im Spiel.")); return; }
  const src = (s) => pUrl(`shot/${encodeURIComponent(s.instance_id)}/${encodeURIComponent(s.file)}`);
  v.append(h("div", { class: "shots" }, shots.map((s) =>
    h("button", { class: "shot sunk", title: s.file, onclick: () => openModal(h("div", { class: "dialog wide block" }, h("img", { class: "full", src: src(s), alt: s.file }), h("div", { class: "row" }, h("span", { class: "dim small" }, s.instance_id + " · " + s.file), h("button", { onclick: closeModal }, "Schließen")))) },
      h("img", { src: src(s), alt: s.file, loading: "lazy", decoding: "async" })))));
}

async function accountsSection() {
  const list = await listAccounts();
  const act = state.settings.active_account;
  return h("section", { class: "block" }, h("h2", null, "Konto"),
    list.length ? list.map((a) => h("div", { class: "row", style: "margin-bottom:8px" },
      h("span", null, a.name), act === a.uuid ? h("span", { class: "tag" }, "aktiv") : h("button", { onclick: () => saveSettings({ active_account: a.uuid }).then(render) }, "Nutzen"),
      h("button", { class: "danger", onclick: async () => { accountsCache = null; await call("remove_account", { uuid: a.uuid }); state.settings = await invoke("get_settings"); render(); } }, "Entfernen")))
      : h("p", { class: "small dim" }, "Nicht angemeldet. Melde dich mit Microsoft an, um zu spielen."),
    h("button", { class: "primary", onclick: async () => { try { const p = await invoke("ms_login"); accountsCache = null; state.settings = await invoke("get_settings"); toast("Angemeldet als " + p.name); render(); } catch (e) { toast(String(e), true); } } }, "Mit Microsoft anmelden"));
}
function updateError(e) {
  const m = String(e);
  if (m.includes("no release has been published yet")) return "Es ist noch keine Version veröffentlicht.";
  if (m.includes("checksum")) return "Die Datei ist beschädigt angekommen. Das Update wurde verworfen, deine Version ist unverändert.";
  return "Updates sind gerade nicht erreichbar. " + m;
}
async function updateSection() {
  const version = await appVersion();
  const text = h("p", { class: "dim", style: "margin:6px 0 10px" });
  const row = h("div", { class: "row" });
  const note = h("p", { class: "small dim", style: "margin-top:8px" });
  const show = (msg, ...buttons) => { text.textContent = tr(msg); row.replaceChildren(...buttons); };
  const checkBtn = (label = "Nach Updates suchen") => h("button", { class: "primary", onclick: check }, label);
  async function check() {
    show("Suche …", h("button", { disabled: true }, "Suche …"));
    note.textContent = "";
    let u;
    try { u = await invoke("check_update"); } catch (e) { show(updateError(e), checkBtn("Erneut versuchen")); return; }
    if (!u) { show("Du hast die neueste Version (" + version + ").", checkBtn("Erneut suchen")); return; }
    note.textContent = u.notes || "";
    if (!u.can_apply) {
      show("Version " + u.version + " ist verfügbar (du hast " + version + ").",
        u.download_url ? h("button", { class: "primary", onclick: () => openUrl(u.download_url) }, "Herunterladen") : null, checkBtn("Erneut suchen"));
      note.textContent = (note.textContent ? note.textContent + " " : "") + tr("Dieser Launcher läuft nicht als AppImage und kann sich nicht selbst ersetzen. Installiere die neue Datei von Hand.");
      return;
    }
    show("Version " + u.version + " ist verfügbar (du hast " + version + ").", h("button", { class: "primary", onclick: () => apply(u) }, "Jetzt aktualisieren"));
  }
  async function apply(u) {
    show("Lade Version " + u.version + " und prüfe sie …", h("button", { disabled: true }, "Aktualisiere …"));
    try { await invoke("apply_update"); }
    catch (e) { show(updateError(e), checkBtn("Erneut versuchen")); return; }
    show("Version " + u.version + " ist installiert. Starte den Launcher neu, um sie zu nutzen.",
      h("button", { class: "primary", onclick: () => invoke("restart_app").catch((e) => toast(String(e), true)) }, "Jetzt neu starten"),
      h("button", { onclick: () => show("Neustart ausstehend. Die neue Version läuft beim nächsten Start.") }, "Später"));
  }
  show("Du hast Version " + version + ".", checkBtn());
  const auto = h("label", { class: "toggle" }, h("input", { type: "checkbox", checked: state.settings.auto_update_check !== false, onchange: (e) => saveSettings({ auto_update_check: e.target.checked }) }), h("span", null, "Beim Start automatisch nach Updates suchen"));
  return h("section", { class: "block" }, h("h2", null, "Updates"), text, row, note, auto);
}
async function viewSettings(v) {
  const s = state.settings;
  const toggle = (label, key, hint) => h("label", { class: "toggle" }, h("input", { type: "checkbox", checked: !!s[key], onchange: (e) => saveSettings({ [key]: e.target.checked }) }), h("span", null, label, hint ? h("div", { class: "small dim" }, hint) : null));
  const mod = (label, key) => h("label", { class: "toggle" }, h("input", { type: "checkbox", checked: !!s.modules[key], onchange: (e) => saveSettings({ modules: { ...state.settings.modules, [key]: e.target.checked } }) }), label);
  const num = (label, key, min, max, step = 1) => h("label", { class: "field" }, label, h("input", { type: "number", min, max, step, value: s[key], onchange: (e) => saveSettings({ [key]: Number(e.target.value) }) }));
  const text = (label, key, ph = "") => h("label", { class: "field" }, label, h("input", { type: "text", value: s[key] || "", placeholder: ph, onchange: (e) => saveSettings({ [key]: e.target.value || null }) }));
  const colors = [["auto", "Automatisch", "conic-gradient(#3f7ae0, #f08a1c, #e0453a, #f2c94c, #3f7ae0)"], ["blue", "Blau", "#3f7ae0"], ["green", "Grün", "#4fb948"], ["red", "Rot", "#e0503c"], ["gold", "Gold", "#e0a91f"], ["purple", "Lila", "#9a62e0"]];
  v.append(h("div", { class: "head" }, h("h1", null, "Optionen")),
    h("div", { class: "settings-grid" }, await accountsSection(), await updateSection(),
      h("section", { class: "block" }, h("h2", null, "Module"), h("p", { class: "small dim" }, "Blende aus, was du nicht brauchst."),
        mod("Entdecken", "discover"), mod("Screenshots", "screenshots")),
      h("section", { class: "block" }, h("h2", null, "Aussehen"),
        h("div", { class: "field" }, h("span", null, "Farbe"),
          h("div", { class: "swatches" }, colors.map(([k, label, bg]) => h("button", { title: label, "aria-label": label, "aria-pressed": String((s.color || "auto") === k), style: `background:${bg}`, onclick: async () => { await saveSettings({ color: k }); applyTheme(); render(); } }))),
          h("div", { class: "small dim" }, "Die Landschaft folgt dem Datum. Automatisch ist Blau, zu Halloween, Weihnachten und Silvester mit passenden Farben.")),
        h("div", { class: "field", style: "margin-top:10px" }, h("span", null, "Sprache"),
          h("div", { class: "seg" }, [["de", "Deutsch"], ["en", "English"]].map(([code, label]) =>
            h("button", { "aria-pressed": String((s.language || "de") === code), onclick: async () => { await saveSettings({ language: code }); renderNav(); render(); } }, label)))),
        ...[["scene", "Landschaft in den Bannern"], ["seasonal", "Jahreszeiten und Events (Halloween, Weihnachten, Silvester …)"], ["effects", "Bewegte Effekte (Schnee, Feuerwerk, Blitze)"], ["skin_figure", "3D-Figur auf der Startseite"], ["show_playtime", "Spielzeit anzeigen"]]
          .map(([key, label]) => h("label", { class: "toggle" }, h("input", { type: "checkbox", checked: s[key] !== false, onchange: async (e) => { await saveSettings({ [key]: e.target.checked }); applyTheme(); render(); } }), h("span", null, label))),
        h("div", { class: "field" }, h("span", null, "Versionsauswahl bei „Neues Modpack“"),
          dropdown({ options: [["fancy", "Mit Bildern (schön)"], ["simple", "Einfache Liste"]], value: s.version_picker || "fancy", label: "Versionsauswahl", onChange: (k) => saveSettings({ version_picker: k }) })),
        ...[["show_snapshots", "Snapshots und Pre-Releases anbieten"], ["show_experimental", "Experimentelle Versionen anbieten"], ["show_april_fools", "Aprilscherz-Versionen anbieten"]]
          .map(([key, label]) => h("label", { class: "toggle" }, h("input", { type: "checkbox", checked: s[key] === true, onchange: async (e) => { await saveSettings({ [key]: e.target.checked }); versionList = null; } }), h("span", null, label))),
        num("Pixelgröße (1–4)", "pixel_scale", 1, 4),
        toggle("Animationen", "animations"), toggle("Kompakte Listen", "compact_lists"),
        h("div", { class: "row", style: "margin-top:10px" }, h("button", { onclick: () => showSetup(0) }, "Einrichtung wiederholen"))),
      h("section", { class: "block" }, h("h2", null, "Spiel"),
        num("Standard-Arbeitsspeicher (MB)", "default_memory_mb", 512, 65536, 256),
        text("Java-Pfad", "java_path", "automatisch"), text("Zusätzliche JVM-Argumente", "extra_jvm_args"),
        num("Parallele Downloads", "parallel_downloads", 1, 32),
        h("div", { class: "field" }, h("span", null, "Nach dem Start des Spiels"),
          dropdown({ options: [["keep", "Launcher offen lassen"], ["minimize", "Launcher minimieren"], ["close", "Launcher schließen"]], value: s.after_start || "keep", label: "Nach dem Start des Spiels", onChange: (k) => saveSettings({ after_start: k }) }),
          h("span", { class: "small dim" }, "Das Spiel läuft auch bei geschlossenem Launcher weiter. Beim nächsten Öffnen erkennt er es wieder. Mit eigenem Konsolenfenster wird nur minimiert.")),
        toggle("Konsole nach dem Start in einem eigenen Fenster öffnen", "console_window"),
        toggle("Weiteres Fenster eines laufenden Modpacks erlauben", "multi_start", "Zeigt „Weiteres Fenster“, solange ein Modpack läuft."),
        toggle("Fabric API automatisch in neue Modpacks packen", "auto_api_mod", "Bei Fabric kommt Fabric API dazu, bei Quilt QFAPI. Fast alle Mods brauchen das. Forge und NeoForge bringen es selbst mit."),
        toggle("Absturz-Hilfe anzeigen", "crash_help", "Erklärt in einfachen Worten, warum das Spiel abgestürzt ist, wenn der Launcher die Ursache erkennt."),
        toggle("Gruppen im Mods-Tab anbieten", "content_groups"),
        toggle("Gruppen als Ordner zeigen", "groups_as_folders", "Aus: eine einfache Liste, die Gruppe steht als Tag an jedem Eintrag."),
        toggle("Mod-Updates im Mods-Tab anbieten", "mod_updates", "Zeigt die Schaltfläche „Nach Updates suchen“. Es wird nie von selbst gesucht."),
        toggle("Nach der Update-Suche nur Mods mit Update zeigen", "mod_updates_autofilter"),
        toggle("Minecraft-Einstellungen zwischen Modpacks teilen", "sync_options", "Sichtfeld, Steuerung und Serverliste einmal einstellen. Jedes Modpack kann einzeln aussteigen.")),
      h("section", { class: "block" }, h("h2", null, "Hinweise und Bildnachweis"),
        h("p", { class: "small", style: "font-weight:600" }, "NOT AN OFFICIAL MINECRAFT PRODUCT. NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT."),
        h("p", { class: "small dim" }, "Geffeniuse Launcher ist ein inoffizieller Launcher von deldown. Minecraft ist eine Marke von Mojang AB / Microsoft. Verantwortlich für den Launcher ist deldown (Impressum: impressum.deldown.de, Kontakt: support@deldown.de), nicht Mojang oder Microsoft."),
        h("p", { class: "small dim" }, "Versionsbilder: ab 1.13 von Mojang (öffentlicher Feed des offiziellen Launchers); 1.6 bis 1.12 sind verkleinerte Bilder aus der Minecraft Wiki (minecraft.wiki), sie zeigen Spielmaterial von Mojang. Loader-Logos: Fabric (Fabric-Projekt), Quilt (Quilt-Projekt), Forge (Minecraft Forge), NeoForge (NeoForged), jeweils nur zur Kennzeichnung des Loaders. Schrift: Rubik (SIL Open Font License). Alle Rechte bei den jeweiligen Inhabern."),
        toggle("Versionsbilder anzeigen", "version_pictures", "Aus: einfache gemalte Bilder, und beim Öffnen von „Neues Modpack“ wird nichts bei Mojang geladen.")),
      h("section", { class: "block" }, h("h2", null, "Unterstützen"),
        h("p", { class: "small dim" }, "Geffeniuse ist kostenlos und ohne Werbung. Wenn es dir gefällt, kannst du die Arbeit auf Ko-fi unterstützen. Das ist freiwillig."),
        h("div", { class: "row" }, h("button", { onclick: () => openUrl(KOFI) }, "Auf Ko-fi unterstützen"))),
      h("section", { class: "block" }, h("h2", null, "Startseite"),
        h("div", { class: "field" }, h("span", null, "Zuletzt gespielte Welten und Server"),
          dropdown({ options: [0, 1, 2, 3, 4, 5, 6].map((n) => [String(n), n === 0 ? "Nicht anzeigen" : String(n)]), value: String(s.home_recent ?? 3), label: "Zuletzt gespielte Welten und Server", onChange: (k) => saveSettings({ home_recent: Number(k) }) })),
        ...[["home_modpacks", "Modpacks anzeigen"], ["home_actions", "Schnellaktionen anzeigen (Neues Modpack, Entdecken, Importieren)"], ["home_compact", "Niedrigerer Banner"]]
          .map(([key, label]) => h("label", { class: "toggle" }, h("input", { type: "checkbox", checked: key === "home_modpacks" ? s[key] !== false : s[key] === true, onchange: async (e) => { await saveSettings({ [key]: e.target.checked }); } }), h("span", null, label))),
        h("p", { class: "small dim" }, "Die 3D-Figur und die Landschaft stellst du unter Aussehen ein.")),
      s.discord_application_id && h("section", { class: "block" }, h("h2", null, "Discord Rich Presence"),
        h("p", { class: "small dim" }, "Teilt nach dem Einschalten „Im Launcher“ oder „Spielt Minecraft“ mit deiner Discord-Desktop-App. Discord kann die Aktivität in deinem Profil anzeigen. Keine Konto-, Server-, Welt- oder Modpack-Namen."),
        h("label", { class: "toggle" }, h("input", { type: "checkbox", checked: !!s.discord_presence, onchange: async (e) => {
          try { await saveSettings({ discord_presence: e.target.checked }); }
          catch { e.target.checked = !!state.settings.discord_presence; }
        } }), h("span", null, "Aktivität mit Discord teilen")),
        h("p", { class: "small dim" }, "Standardmäßig aus. Ausschalten entfernt die Anzeige. Discord muss lokal laufen; beim Schließen des Launchers endet die Anzeige, auch wenn Minecraft weiterläuft.")),
      h("section", { class: "block" }, h("h2", null, "Quellen"),
        toggle("Modrinth", "modrinth_enabled"), toggle("CurseForge", "curseforge_enabled"),
        toggle("CurseForge über den deldown-Proxy", "curseforge_proxy", "Ohne eigenen Schlüssel gehen Suchanfragen und deine IP-Adresse über api-geffeniuse.deldown.de zu CurseForge. Aus: nur mit eigenem Schlüssel."),
        text("Eigener CurseForge API-Key (optional)", "curseforge_key", "leer lassen"))));
}
