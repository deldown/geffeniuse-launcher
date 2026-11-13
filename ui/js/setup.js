"use strict";
const SETUP_STEPS = 4;
async function finishSetup() {
  await saveSettings({ setup_done: true });
  const el = $("#setup");
  el.classList.add("leaving");
  setTimeout(() => { el.hidden = true; el.classList.remove("leaving"); el.replaceChildren(); }, 180);
  renderNav(); render();
}
function showSetup(step = 0) {
  state.setupStep = step;
  $("#setup").hidden = false;
  renderSetup(true);
}
async function renderSetup(first = false) {
  const el = $("#setup");
  const step = state.setupStep || 0;
  const s = state.settings;
  const accounts = step === 2 ? await listAccounts() : [];
  const colors = [["auto", "Automatisch", "conic-gradient(#3f7ae0, #f08a1c, #e0453a, #f2c94c, #3f7ae0)"], ["blue", "Blau", "#3f7ae0"], ["green", "Grün", "#4fb948"], ["red", "Rot", "#e0503c"], ["gold", "Gold", "#e0a91f"], ["purple", "Lila", "#9a62e0"]];
  let body;
  if (step === 0) {
    body = [h("h1", null, "Willkommen bei Geffeniuse Launcher"), h("p", { class: "dim" }, "Wähle deine Sprache"),
      h("div", { class: "langs" }, [["de", "Deutsch"], ["en", "English"]].map(([code, label]) =>
        h("button", { class: "lang", "aria-pressed": String(s.language === code), onclick: async () => { await saveSettings({ language: code }); renderNav(); renderSetup(); } }, label))),
      h("p", { class: "small dim" }, "Du kannst das später in den Optionen ändern.")];
  } else if (step === 1) {
    body = [h("h1", null, "Wähle dein Aussehen"),
      h("div", { class: "preview", "aria-label": "Vorschau" }),
      h("div", { class: "field" }, h("span", null, "Farbe"),
        h("div", { class: "swatches" }, colors.map(([k, label, bg]) => h("button", { title: label, "aria-label": label, "aria-pressed": String((s.color || "auto") === k), style: `background:${bg}`, onclick: async () => { await saveSettings({ color: k }); applyTheme(); renderSetup(); } }))))];
  } else if (step === 2) {
    body = [h("h1", null, "Minecraft-Konten"),
      h("p", { class: "dim" }, "Melde dich mit deinem Microsoft-Konto an. Du kannst mehrere Konten hinzufügen und später oben wechseln."),
      accounts.length ? h("div", { class: "acctlist" }, accounts.map((a) => h("div", { class: "acctrow" }, headCanvas(a.skin), h("span", null, a.name)))) : h("p", { class: "small dim" }, "Noch kein Konto angemeldet."),
      h("div", { class: "row" }, h("button", { class: "primary", onclick: addAccount }, "Microsoft-Konto hinzufügen")),
      h("p", { class: "small dim" }, "Zum Spielen ist ein Microsoft-Konto nötig. Du kannst es auch später hinzufügen.")];
  } else {
    body = [h("h1", null, "Alles bereit"), h("p", { class: "dim" }, "Du kannst jetzt Modpacks entdecken, Instanzen anlegen und spielen.")];
  }
  const last = step === SETUP_STEPS - 1;
  // The card is built once; later steps only swap what is inside, so nothing blinks or moves.
  let card = el.querySelector(".setup-card");
  if (!card || first) {
    card = h("div", { class: "setup-card block" + (first ? " enter" : ""), role: "dialog", "aria-modal": "true", "aria-label": "Einrichtung" },
      h("div", { class: "setup-top" }, h("div", { class: "dots", "aria-hidden": "true" }), h("span", { class: "skipslot" })),
      h("div", { class: "setup-body" }),
      h("div", { class: "row setup-nav" }));
    el.replaceChildren(card);
  }
  card.querySelector(".dots").replaceChildren(...Array.from({ length: SETUP_STEPS }, (_, n) => h("i", { class: n === step ? "on" : n < step ? "done" : "" })));
  card.querySelector(".skipslot").replaceChildren(...(last ? [] : [h("button", { class: "ghost small", onclick: finishSetup }, "Überspringen")]));
  card.querySelector(".setup-body").replaceChildren(...body);
  card.querySelector(".setup-nav").replaceChildren(
    ...(step > 0 ? [h("button", { onclick: () => { state.setupStep = step - 1; renderSetup(); } }, "Zurück")] : []),
    h("span", { class: "grow" }),
    h("button", { class: "primary", onclick: last ? finishSetup : () => { state.setupStep = step + 1; renderSetup(); } }, last ? "Los geht's" : "Weiter"));
  $("#setup .setup-card button.primary")?.focus();
}
