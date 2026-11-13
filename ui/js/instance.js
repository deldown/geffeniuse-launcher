"use strict";
function openInstance(i, tab = "content") { state.instanceId = i.id; state.tab = tab; go("instance"); }
// The icon itself is the button: hovering shows what clicking does, no separate caption needed.
function iconEditor(i) {
  const src = i.icon ? pUrl("icon/" + encodeURIComponent(i.id) + "/" + i.icon) : "logo.png";
  const main = h("button", { class: "ico big edit", "aria-label": "Icon ändern", onclick: () => changeIcon(i) },
    h("img", { src, class: i.icon ? "custom" : "", alt: "", decoding: "async" }), h("span", { class: "hint" }, "Icon ändern"));
  const reset = i.icon ? h("button", { class: "icon-reset", title: "Standard-Icon wiederherstellen", "aria-label": "Standard-Icon wiederherstellen",
    onclick: async () => { await call("clear_instance_icon", { id: i.id }); refresh(); } }, "×") : null;
  return h("div", { class: "iconwrap" }, main, reset);
}
function nameEditor(i) {
  const wrap = h("div", { class: "nameline" });
  const show = () => wrap.replaceChildren(h("h1", null, i.name),
    h("button", { class: "ghost small iconbtn", title: "Umbenennen", "aria-label": "Umbenennen", onclick: edit }, icon("edit")));
  function edit() {
    const input = h("input", { type: "text", value: i.name, maxlength: 60, "aria-label": "Name", class: "nameinput" });
    wrap.replaceChildren(input);
    input.focus(); input.select();
    let done = false;
    const finish = async (save) => {
      if (done) return;
      done = true;
      const name = input.value.trim();
      if (save && name && name !== i.name) {
        try { await invoke("rename_instance", { id: i.id, name }); } catch (e) { toast(String(e), true); }
      }
      refresh();
    };
    input.addEventListener("keydown", (e) => { if (e.key === "Enter") finish(true); else if (e.key === "Escape") finish(false); });
    input.addEventListener("blur", () => finish(true));
  }
  show();
  return wrap;
}
async function changeIcon(i) {
  const path = await call("pick_image");
  if (!path) return;
  await call("set_instance_icon", { id: i.id, path });
  refresh();
}
// A segmented control whose highlight glides to the chosen button, like the top bar.
function segmented(items, current, onPick) {
  let cur = current;
  const pill = h("span", { class: "seg-pill", "aria-hidden": "true" });
  const buttons = items.map(([id, label]) => h("button", { "data-id": id, "aria-pressed": String(id === cur), onclick: () => { if (id !== cur) { set(id); onPick(id); } } }, label));
  const seg = h("div", { class: "seg pilled" }, pill, ...buttons);
  const place = (instant) => {
    const b = buttons.find((x) => x.dataset.id === cur);
    if (!b || !seg.isConnected || !b.offsetWidth) return;
    if (instant) pill.style.transition = "none";
    pill.style.width = b.offsetWidth + "px";
    pill.style.transform = `translateX(${b.offsetLeft}px)`;
    pill.style.opacity = "1";
    if (instant) { void pill.offsetWidth; pill.style.transition = ""; }
  };
  const set = (id) => { cur = id; for (const b of buttons) b.setAttribute("aria-pressed", String(b.dataset.id === id)); place(false); };
  seg.set = set;
  new ResizeObserver(() => place(true)).observe(seg);
  return seg;
}

const INSTANCE_TABS = [["content", "Inhalt"], ["add", "Hinzufügen"], ["worlds", "Welten"], ["console", "Konsole"], ["settings", "Einstellungen"]];
async function viewInstance(v) {
  state.instances = await listInstances();
  const i = state.instances.find((x) => x.id === state.instanceId);
  if (!i) { go("library"); return; }
  const busy = state.busy.has(i.id);
  v.append(
    h("button", { class: "ghost back", onclick: () => go("library") }, "← Bibliothek"),
    h("section", { class: "block pagehead scene" }, h("div", { class: "bolt", "aria-hidden": "true" }), h("div", { class: "fall", "aria-hidden": "true" }, h("i"), h("i"), h("i"), h("i"), h("i")),
      h("div", { class: "row", style: "gap:20px; align-items:flex-end" },
        iconEditor(i),
        h("div", { class: "meta" }, nameEditor(i), tags(i), h("div", { class: "small" }, metaLine(i)))),
      h("div", { class: "row", style: "gap:10px; align-items:center" }, extraBtn(i), playBtn(i, busy, "xl"))),
    ...(busy ? [statusLine(i)] : []));
  if (!INSTANCE_TABS.some(([id]) => id === state.tab)) state.tab = "content";
  const body = h("div", { class: "tabbody" });
  const builders = { content: contentInstalled, add: contentAdd, worlds: worldsTab, console: consoleTab, settings: settingsTab };
  let shown = 0;
  // Only the tab's content is swapped; the header and the highlight stay where they are.
  const show = async (id) => {
    state.tab = id;
    state.logId = id === "console" ? i.id : null;
    const n = ++shown, next = h("div", null);
    try { await builders[id](next, i, ctx); } catch (e) { next.replaceChildren(h("div", { class: "empty block" }, String(e))); }
    if (n === shown) body.replaceChildren(...next.childNodes);
  };
  const seg = segmented(INSTANCE_TABS, state.tab, show);
  const ctx = { go: (id) => { seg.set(id); show(id); }, refresh: () => show(state.tab) };
  state.tabRefresh = ctx.refresh;
  v.append(h("div", { class: "row", style: "margin:16px 0" }, seg), body);
  await show(state.tab);
}
const fmtMb = (n) => (n >= 1e6 ? (n / 1e6).toFixed(1) + " MB" : Math.max(1, Math.round(n / 1e3)) + " KB");
const logo = (url) => url
  ? h("img", { class: "logo", src: url, alt: "", width: 52, height: 52, loading: "lazy", referrerpolicy: "no-referrer" })
  : h("div", { class: "logo ph" }, cube("stone", "sm"));
