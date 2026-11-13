"use strict";
const instIcon = (i, big = false) => h("div", { class: "ico" + (big ? " big" : "") },
  h("img", { src: i.icon ? pUrl("icon/" + encodeURIComponent(i.id) + "/" + i.icon) : "logo.png", class: i.icon ? "custom" : "", alt: "", decoding: "async" }));
const openUrl = (url) => invoke("open_url", { url }).catch((e) => toast(String(e), true));
const IMG_HOSTS = new Set(["cdn.modrinth.com", "media.forgecdn.net", "media.curseforge.com", "edge.forgecdn.net"]);
const imgOk = (u) => { try { const x = new URL(u); return x.protocol === "https:" && IMG_HOSTS.has(x.hostname); } catch { return false; } };
const linkOk = (u) => { try { return new URL(u).protocol === "https:"; } catch { return false; } };
const ENTITIES = { "&amp;": "&", "&lt;": "<", "&gt;": ">", "&quot;": '"', "&#39;": "'", "&nbsp;": " " };
const decode = (t) => t.replace(/&(amp|lt|gt|quot|nbsp|#39);/g, (m) => ENTITIES[m] ?? m);
const mkImg = (src, alt = "") => h("img", { src, alt, loading: "lazy", decoding: "async", referrerpolicy: "no-referrer" });
const mkLink = (kids, url) => h("span", { class: "lnk", role: "link", tabindex: "0", title: url, onclick: () => openUrl(url), onkeydown: (e) => { if (e.key === "Enter") openUrl(url); } }, ...kids);
const INLINE = /!\[([^\]]*)\]\(([^)\s]+)(?:\s+"[^"]*")?\)|\[((?:[^\[\]]|\[[^\]]*\])*)\]\(([^)\s]+)(?:\s+"[^"]*")?\)|\*\*([^*]+)\*\*|__([^_]+)__|~~([^~]+)~~|`([^`]+)`|\*([^*\n]+)\*|<img\s[^>]*?src=["']([^"']+)["'][^>]*>|<br\s*\/?>|<\/?[a-zA-Z][^>]*>/g;
function inline(src) {
  const out = [];
  let i = 0, m;
  const re = new RegExp(INLINE.source, "g");
  while ((m = re.exec(src))) {
    if (m.index > i) out.push(decode(src.slice(i, m.index)));
    if (m[2] !== undefined) { if (imgOk(m[2])) out.push(mkImg(m[2], m[1])); }
    else if (m[4] !== undefined) { const kids = inline(m[3]); out.push(linkOk(m[4]) ? mkLink(kids, m[4]) : h("span", null, ...kids)); }
    else if (m[5] !== undefined) out.push(h("strong", null, ...inline(m[5])));
    else if (m[6] !== undefined) out.push(h("strong", null, ...inline(m[6])));
    else if (m[7] !== undefined) out.push(h("s", null, ...inline(m[7])));
    else if (m[8] !== undefined) out.push(h("code", null, m[8]));
    else if (m[9] !== undefined) out.push(h("em", null, ...inline(m[9])));
    else if (m[10] !== undefined) { if (imgOk(m[10])) out.push(mkImg(m[10])); }
    else if (/^<br/i.test(m[0])) out.push(h("br"));
    i = re.lastIndex;
  }
  if (i < src.length) out.push(decode(src.slice(i)));
  return out;
}
const LIST_ITEM = /^(\s*)([-*+]|\d+[.)])\s+(.*)$/;
const TABLE_SEP = /^\s*\|?\s*:?-{2,}:?\s*(\|\s*:?-{2,}:?\s*)*\|?\s*$/;
// Small Markdown renderer: builds DOM nodes only, never parses HTML, drops raw tags and any
// image that is not on a mod platform CDN.
function renderMarkdown(src) {
  const root = document.createDocumentFragment();
  const lines = src.slice(0, 200000).replace(/\r\n?/g, "\n").split("\n");
  const isBlockStart = (l) => /^(#{1,6}\s|\s*(```|~~~)|\s*>|\s*([-*_])(\s*\3){2,}\s*$)/.test(l) || LIST_ITEM.test(l);
  let i = 0;
  while (i < lines.length) {
    const line = lines[i];
    if (!line.trim()) { i++; continue; }
    const fence = line.match(/^\s*(```|~~~)/);
    if (fence) {
      const code = [];
      for (i++; i < lines.length && !lines[i].trim().startsWith(fence[1]); i++) code.push(lines[i]);
      i++;
      root.append(h("pre", null, h("code", null, code.join("\n"))));
      continue;
    }
    const hd = line.match(/^(#{1,6})\s+(.*?)\s*#*\s*$/);
    if (hd) { root.append(h("h" + Math.min(hd[1].length, 4), null, ...inline(hd[2]))); i++; continue; }
    if (/^\s*([-*_])(\s*\1){2,}\s*$/.test(line)) { root.append(h("hr")); i++; continue; }
    if (/^\s*>/.test(line)) {
      const inner = [];
      for (; i < lines.length && /^\s*>/.test(lines[i]); i++) inner.push(lines[i].replace(/^\s*>\s?/, ""));
      root.append(h("blockquote", null, renderMarkdown(inner.join("\n"))));
      continue;
    }
    if (LIST_ITEM.test(line)) {
      const stack = [];
      for (; i < lines.length; i++) {
        const m = lines[i].match(LIST_ITEM);
        if (!m) { if (lines[i].trim() === "" || !/^\s+\S/.test(lines[i])) break; continue; }
        const depth = Math.floor(m[1].replace(/\t/g, "  ").length / 2);
        const ordered = /\d/.test(m[2]);
        while (stack.length > depth + 1) stack.pop();
        if (stack.length < depth + 1 || stack[stack.length - 1].tagName !== (ordered ? "OL" : "UL")) {
          const list = h(ordered ? "ol" : "ul");
          (stack.length ? stack[stack.length - 1].lastElementChild || stack[stack.length - 1] : root).append(list);
          stack.length > depth ? (stack[depth] = list) : stack.push(list);
        }
        stack[stack.length - 1].append(h("li", null, ...inline(m[3])));
      }
      continue;
    }
    if (line.includes("|") && TABLE_SEP.test(lines[i + 1] || "")) {
      const cells = (l) => l.trim().replace(/^\||\|$/g, "").split("|").map((c) => c.trim());
      const head = cells(line);
      const rows = [];
      for (i += 2; i < lines.length && lines[i].includes("|"); i++) rows.push(cells(lines[i]));
      root.append(h("table", null, h("thead", null, h("tr", null, head.map((c) => h("th", null, ...inline(c))))),
        h("tbody", null, rows.map((r) => h("tr", null, r.map((c) => h("td", null, ...inline(c))))))));
      continue;
    }
    const para = [];
    for (; i < lines.length && lines[i].trim() && !(para.length && isBlockStart(lines[i])); i++) para.push(lines[i].replace(/\s{2,}$/, ""));
    const kids = [];
    para.forEach((l, n) => { if (n) kids.push(h("br")); kids.push(...inline(l)); });
    root.append(h("p", null, ...kids));
  }
  return root;
}
// CurseForge descriptions are HTML: parse them inertly and rebuild only a small allow-list.
const HTML_BLOCKS = new Set(["p", "div", "section", "center", "h1", "h2", "h3", "h4", "h5", "h6", "ul", "ol", "li", "blockquote", "pre", "table", "thead", "tbody", "tr", "th", "td", "hr", "br", "strong", "em", "s", "u", "code"]);
const HTML_ALIAS = { b: "strong", i: "em", strike: "s", del: "s", h5: "h4", h6: "h4", center: "div", section: "div" };
function renderHtml(html) {
  const doc = new DOMParser().parseFromString(html.slice(0, 300000), "text/html");
  const walk = (n) => {
    if (n.nodeType === 3) return [n.textContent];
    if (n.nodeType !== 1) return [];
    const tag = n.tagName.toLowerCase();
    const kids = () => [...n.childNodes].flatMap(walk);
    if (tag === "a") { const href = n.getAttribute("href") || ""; return linkOk(href) ? [mkLink(kids(), href)] : kids(); }
    if (tag === "img") { const src = n.getAttribute("src") || ""; return imgOk(src) ? [mkImg(src, n.getAttribute("alt") || "")] : []; }
    const name = HTML_ALIAS[tag] || tag;
    if (!HTML_BLOCKS.has(tag) && !HTML_ALIAS[tag]) return /^(script|style|iframe|object|embed|form|input|button|svg|video|audio|link|meta)$/.test(tag) ? [] : kids();
    return [h(name, null, ...kids())];
  };
  const frag = document.createDocumentFragment();
  frag.append(...[...doc.body.childNodes].flatMap(walk));
  return frag;
}
