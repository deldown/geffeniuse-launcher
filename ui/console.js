(() => {
  const id = new URLSearchParams(location.search).get("id") || "";
  if (!/^[a-z0-9][a-z0-9-]*$/.test(id)) return;
  const { invoke } = window.__TAURI__.core;
  const { listen } = window.__TAURI__.event;
  const log = document.getElementById("log");
  const MAX_LINES = 3000;
  let chunks = [];
  const append = (lines) => {
    if (!lines.length) return;
    const stick = log.scrollTop + log.clientHeight >= log.scrollHeight - 24;
    const node = document.createTextNode((chunks.length ? "\n" : "") + lines.join("\n"));
    log.append(node);
    chunks.push([node, lines.length]);
    let total = chunks.reduce((n, c) => n + c[1], 0);
    while (total > MAX_LINES && chunks.length > 1) { const [old, n] = chunks.shift(); old.remove(); total -= n; }
    if (stick) log.scrollTop = log.scrollHeight;
  };
  const words = () => {
    for (const [elId, text] of [["copy", "Kopieren"], ["clear", "Leeren"]]) document.getElementById(elId).textContent = typeof tr === "function" ? tr(text) : text;
  };
  invoke("get_settings").then((s) => { if (typeof LANG !== "undefined") LANG = s.language === "en" ? "en" : "de"; words(); }).catch(words);
  invoke("get_log", { id }).then((lines) => { append(lines); log.scrollTop = log.scrollHeight; });
  listen("game-log", (e) => { const [from, lines] = e.payload; if (from === id) append(lines); });
  document.getElementById("clear").onclick = async () => { await invoke("clear_log", { id }); log.textContent = ""; chunks = []; };
  document.getElementById("copy").onclick = () => navigator.clipboard?.writeText(log.textContent);
})();
