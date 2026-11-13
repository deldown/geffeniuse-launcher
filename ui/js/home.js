"use strict";
async function viewHome(v) {
  const list = await listInstances();
  const me = state.player || (state.player = await call("player_info"));
  state.instances = list;
  const st = state.settings;
  const recentPlays = st.home_recent > 0 ? await invoke("recent_play", { limit: st.home_recent }).catch(() => []) : [];
  const recent = [...list].sort((a, b) => (b.last_played || 0) - (a.last_played || 0));
  const skin = state.settings.skin_figure === false ? null : skinFigure(me.skin || DEFAULT_SKIN, me.slim);
  const hero = h("section", { class: "hero" }, h("div", { class: "bolt", "aria-hidden": "true" }), h("div", { class: "fall", "aria-hidden": "true" }, h("i"), h("i"), h("i"), h("i"), h("i")),
    h("div", { class: "who" },
      h("div", { class: "kicker" }, me.signed_in ? "Willkommen zurück" : "Willkommen"),
      h("h1", null, me.signed_in ? me.name : "Geffeniuse Launcher"),
      me.skin_error ? h("div", { class: "small", style: "color:#ffd35a" }, "Skin nicht geladen: " + me.skin_error) : null,
      me.signed_in ? null
        : h("div", { class: "row", style: "margin-top:6px" }, h("button", { class: "primary xl", onclick: addAccount }, "Mit Microsoft anmelden"))),
    skin);
  hero.classList.toggle("compact", st.home_compact === true);
  v.append(hero);
  const tile = (glyph, title, sub, fn) => h("button", { class: "tile-btn block", onclick: fn }, h("span", { class: "slot" }, icon(glyph)), h("div", null, h("div", { class: "t" }, title), h("div", { class: "s" }, sub)));
  const nothingElse = !(st.home_modpacks !== false && recent.length) && !(st.home_recent > 0);
  if (recentPlays.length) v.append(h("h2", null, "Zuletzt gespielt"), h("div", { class: "recentgrid" }, recentPlays.map((r) => recentCard(r, list))));
  if (st.home_actions === true || !list.length || nothingElse) v.append(h("div", { class: "quick" },
    tile("plus", "Neues Modpack", "Vanilla, Fabric, Quilt, Forge, NeoForge", createDialog),
    tile("discover", "Modpacks entdecken", "Modrinth und CurseForge", () => go("discover")),
    tile("download", "Modpack importieren", ".mrpack oder CurseForge-.zip", importFile)));
  if (st.home_modpacks !== false && recent.length) v.append(h("h2", null, "Deine Modpacks"), h("div", { class: "grid" }, recent.slice(0, 6).map(instanceCard)));
  v.append(h("div", { class: "homefoot" }, h("button", { class: "ghost small support", onclick: () => openUrl(KOFI) }, "♥ Geffeniuse unterstützen")), h("p", { class: "disclaimer" }, "NOT AN OFFICIAL MINECRAFT PRODUCT. NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT."));
}
function recentCard(r, instances) {
  const i = instances.find((x) => x.id === r.instance_id);
  const pic = r.picture
    ? h("img", { class: "rpic", src: r.picture, alt: "", width: 52, height: 52 })
    : h("span", { class: "rpic ph" }, r.kind === "world" ? cube("vanilla") : icon("server"));
  return h("button", { class: "block recent", onclick: () => joinRecent(r), "aria-label": tr("Beitreten") + ": " + r.target, title: tr("Beitreten") },
    pic,
    h("div", { class: "rtext" },
      h("div", { class: "title", title: r.target }, r.target),
      h("div", { class: "small dim rsub" }, h("span", null, tr(r.kind === "world" ? "Welt" : "Server")), h("span", { class: "dot" }, "·"), h("span", null, i ? i.name : r.instance_name)),
      h("div", { class: "small dim" }, tr(ago(r.when)))),
    h("span", { class: "rplay", "aria-hidden": "true" }, icon("play")));
}
