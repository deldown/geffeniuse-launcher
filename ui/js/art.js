"use strict";
const CUBE_COLORS = {
  vanilla: "#68c154", fabric: "#e3d3a1", quilt: "#a46fe0", forge: "#e08a5c", neoforge: "#f2c94c", stone: "#8d96a8",
};
const FACES = {
  top: "16,2 30,9 16,16 2,9",
  left: "2,9 16,16 16,30 2,23",
  right: "30,9 16,16 16,30 30,23",
};
function shade(hex, f) {
  const n = parseInt(hex.slice(1), 16);
  const c = [n >> 16, (n >> 8) & 255, n & 255].map((v) => Math.max(0, Math.min(255, Math.round(v * f))));
  return "#" + c.map((v) => v.toString(16).padStart(2, "0")).join("");
}
function cube(loader, size = "") {
  const NS = "http://www.w3.org/2000/svg";
  const el = (tag, attrs) => { const n = document.createElementNS(NS, tag); for (const [k, v] of Object.entries(attrs)) n.setAttribute(k, v); return n; };
  const base = CUBE_COLORS[loader] || CUBE_COLORS.stone;
  const grass = loader === "vanilla";
  const fills = { top: base, left: grass ? "#8a5f3a" : shade(base, 0.8), right: grass ? "#6d4a2c" : shade(base, 0.62) };
  const svg = el("svg", { viewBox: "0 0 32 32", "shape-rendering": "crispEdges", class: "cube " + size, "aria-hidden": "true" });
  for (const [name, pts] of Object.entries(FACES)) svg.append(el("polygon", { points: pts, fill: fills[name] }));
  if (grass) {
    svg.append(el("polygon", { points: "2,9 16,16 16,20 2,13", fill: shade(base, 0.82) }));
    svg.append(el("polygon", { points: "30,9 16,16 16,20 30,13", fill: shade(base, 0.64) }));
  }
  svg.append(el("polyline", { points: "3,9.4 16,3 29,9.4", fill: "none", stroke: shade(base, 1.22), "stroke-width": 1 }));
  for (const pts of Object.values(FACES)) svg.append(el("polygon", { points: pts, fill: "none", stroke: shade(grass ? "#4a6f3a" : base, 0.38), "stroke-width": 1, "stroke-linejoin": "miter" }));
  return svg;
}
function ago(ts) {
  if (!ts) return "Noch nie gespielt";
  const m = Math.max(0, Math.round((Date.now() / 1000 - ts) / 60));
  if (m < 1) return "Gerade eben gespielt";
  if (m < 60) return "Zuletzt vor " + m + " Min.";
  if (m < 1440) return "Zuletzt vor " + Math.round(m / 60) + " Std.";
  return "Zuletzt vor " + Math.round(m / 1440) + " Tagen";
}
function fmtPlaytime(secs) {
  if (!secs || state.settings?.show_playtime === false) return "";
  if (secs < 60) return "unter 1 Min. gespielt";
  const m = Math.floor(secs / 60);
  if (m < 60) return m + " Min. gespielt";
  const hrs = Math.floor(m / 60);
  return (hrs >= 100 ? hrs + " Std." : hrs + " Std. " + (m % 60) + " Min.") + " gespielt";
}
const metaLine = (i) => [ago(i.last_played), fmtPlaytime(i.playtime_secs)].filter(Boolean).map(tr).join(" · ");
const loaderLogo = (l, size = 14) => l === "vanilla" ? cube("vanilla") : h("img", { class: "lico-tag", src: "art/loader-" + l + (l === "forge" ? ".jpg" : ".png"), alt: "", width: size, height: size });
const tags = (i) => h("div", { class: "row" }, h("span", { class: "tag" }, i.mc_version), h("span", { class: "tag accent loadertag" }, loaderLogo(i.loader), (LOADER_INFO[i.loader] || [i.loader])[0]));
function statusLine(i) {
  return state.running.has(i.id) ? h("div", { class: "small", style: "color:var(--accent-hi)" }, "Minecraft läuft") : progressBar();
}
function progressBar() {
  return h("div", { class: "prog" }, h("div", { class: "small dim", "data-prog-text": "" }, "Bereite vor …"),
    h("div", { class: "bar" }, h("i", { "data-prog-bar": "", style: "width:0%" })));
}
let progressQueued = false;
// Progress only patches the bars that are already on screen; it never rebuilds a page.
function applyProgress() {
  progressQueued = false;
  const p = state.progress;
  if (!p) return;
  const pct = p.total ? Math.round(100 * p.done / p.total) + "%" : "0%";
  for (const el of document.querySelectorAll("[data-prog-text]")) el.textContent = p.stage + " " + p.done + "/" + p.total;
  for (const el of document.querySelectorAll("[data-prog-bar]")) el.style.width = pct;
  if (state.installBar) state.installBar.firstChild.style.width = pct;
}
function onProgress(p) {
  state.progress = p;
  if (!progressQueued) { progressQueued = true; requestAnimationFrame(applyProgress); }
}

// Skin boxes are textured straight from the skin PNG by a small software renderer.
const DEFAULT_SKIN = "art/default-skin.png";
const shiftUV = (uv, dx, dy) => Object.fromEntries(Object.entries(uv).map(([k, v]) => [k, Array.isArray(v) ? [v[0] + dx, v[1] + dy, v[2], v[3]] : v]));
function skinParts(slim, tall) {
  const aw = slim ? 3 : 4;
  const arm = (x, y) => ({ right: [x, y + 4, 4, 12], front: [x + 4, y + 4, aw, 12], left: [x + 4 + aw, y + 4, 4, 12], back: [x + 8 + aw, y + 4, aw, 12], top: [x + 4, y, aw, 4], bottom: [x + 4 + aw, y, aw, 4] });
  const leg = (x, y) => ({ right: [x, y + 4, 4, 12], front: [x + 4, y + 4, 4, 12], left: [x + 8, y + 4, 4, 12], back: [x + 12, y + 4, 4, 12], top: [x + 4, y, 4, 4], bottom: [x + 8, y, 4, 4] });
  const head = { right: [0, 8, 8, 8], front: [8, 8, 8, 8], left: [16, 8, 8, 8], back: [24, 8, 8, 8], top: [8, 0, 8, 8], bottom: [16, 0, 8, 8] };
  const body = { right: [16, 20, 4, 12], front: [20, 20, 8, 12], left: [28, 20, 4, 12], back: [32, 20, 8, 12], top: [20, 16, 8, 4], bottom: [28, 16, 8, 4] };
  const rArm = arm(40, 16), rLeg = leg(0, 16), lArm = tall ? arm(32, 48) : rArm, lLeg = tall ? leg(16, 48) : rLeg;
  const base = [
    { c: [0, -96], s: [8, 8, 8], uv: head },
    { c: [0, -16], s: [8, 12, 4], uv: body },
    { c: [-32 - aw * 4, -16], s: [aw, 12, 4], uv: rArm },
    { c: [32 + aw * 4, -16], s: [aw, 12, 4], uv: lArm },
    { c: [-16, 80], s: [4, 12, 4], uv: rLeg },
    { c: [16, 80], s: [4, 12, 4], uv: lLeg },
  ];
  // Second skin layer (hat, jacket, sleeves, pants): drawn a little larger so it floats on top.
  const overlay = [
    { ...base[0], grow: 1.07, uv: shiftUV(head, 32, 0) },
    { ...base[1], grow: 1.05, uv: shiftUV(body, 0, 16) },
    { ...base[2], grow: 1.05, uv: shiftUV(rArm, 0, 16) },
    { ...base[3], grow: 1.05, uv: shiftUV(lArm, 16, 0) },
    { ...base[4], grow: 1.05, uv: shiftUV(rLeg, 0, 16) },
    { ...base[5], grow: 1.05, uv: shiftUV(lLeg, -16, 0) },
  ];
  return tall ? [...base, ...overlay] : [...base, overlay[0]];
}
// Drawn on a plain 2D canvas, so it looks the same on every system. WebGL costs about 120 MB and CSS 3D is flattened by some WebKit builds.
// Drag to turn the figure all the way round, let go and it coasts; a click makes it hop and spin.
// It only draws while something moves, so it costs nothing at rest.
let skinSlot = { key: null, el: null };
function skinFigure(url, slim) {
  const key = url + "|" + slim;
  if (skinSlot.key === key && skinSlot.el) return skinSlot.el;
  skinSlot.el?.dispose?.();
  const wrap = skinFigureNew(url, slim);
  skinSlot = { key, el: wrap };
  return wrap;
}
// Outward-facing corners (top-left, top-right, bottom-left of the texture) and normal of each box face.
// x runs to the viewer's right, y down, z towards the viewer.
const FACE_GEOMETRY = {
  front: (x, y, z) => [[-x, -y, z], [x, -y, z], [-x, y, z], [0, 0, 1]],
  back: (x, y, z) => [[x, -y, -z], [-x, -y, -z], [x, y, -z], [0, 0, -1]],
  right: (x, y, z) => [[-x, -y, -z], [-x, -y, z], [-x, y, -z], [-1, 0, 0]],
  left: (x, y, z) => [[x, -y, z], [x, -y, -z], [x, y, z], [1, 0, 0]],
  top: (x, y, z) => [[-x, -y, -z], [x, -y, -z], [-x, -y, z], [0, -1, 0]],
  bottom: (x, y, z) => [[-x, y, z], [x, y, z], [-x, y, -z], [0, 1, 0]],
};
const LIGHT = (() => { const v = [-0.3, -0.75, 0.6], n = Math.hypot(...v); return v.map((c) => c / n); })();
const SHADES = [1, 0.9, 0.8, 0.7, 0.6];
function skinFigureNew(url, slim) {
  const wrap = h("div", { class: "skinwrap", "aria-hidden": "true" });
  const img = skinImage(url);
  const init = () => {
    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    const W = 230, H = 310, T = 8.4, MID = 16;
    const canvas = h("canvas", { class: "skincanvas", width: Math.round(W * dpr), height: Math.round(H * dpr) });
    const ctx = canvas.getContext("2d");
    ctx.imageSmoothingEnabled = false;
    wrap.append(h("div", { class: "shadow" }), canvas);
    // One copy of the skin per brightness step: faces pick the one that matches how they face the light.
    const textures = SHADES.map((level) => {
      const c = document.createElement("canvas");
      c.width = img.naturalWidth; c.height = img.naturalHeight;
      const g = c.getContext("2d");
      g.drawImage(img, 0, 0);
      g.globalCompositeOperation = "source-atop";
      g.fillStyle = `rgba(0,0,0,${1 - level})`;
      g.fillRect(0, 0, c.width, c.height);
      return c;
    });
    const boxes = skinParts(slim, img.naturalHeight >= 64).map((p) => {
      const g = p.grow || 1;
      return { cx: p.c[0] / 8, cy: p.c[1] / 8 + MID, hx: (p.s[0] * g) / 2, hy: (p.s[1] * g) / 2, hz: (p.s[2] * g) / 2, uv: p.uv };
    });
    let yaw = -24, pitch = 0, vYaw = 0, vPitch = 0, hop = 0, frame = 0, drag = null, spin = null;
    const draw = () => {
      const ya = (yaw * Math.PI) / 180, pa = (pitch * Math.PI) / 180;
      const cy = Math.cos(ya), sy = Math.sin(ya), cp = Math.cos(pa), sp = Math.sin(pa);
      const rot = (x, y, z) => { const x1 = x * cy + z * sy, z1 = -x * sy + z * cy; return [x1, y * cp - z1 * sp, y * sp + z1 * cp]; };
      const faces = [];
      for (const b of boxes) {
        for (const [name, geometry] of Object.entries(FACE_GEOMETRY)) {
          const [tl, tr, bl, n] = geometry(b.hx, b.hy, b.hz);
          const normal = rot(...n);
          if (normal[2] <= 0.001) continue;
          const at = (v) => rot(b.cx + v[0], b.cy - MID + v[1], v[2]);
          const centre = rot(b.cx + n[0] * b.hx, b.cy - MID + n[1] * b.hy, n[2] * b.hz);
          const light = normal[0] * LIGHT[0] + normal[1] * LIGHT[1] + normal[2] * LIGHT[2];
          faces.push({ depth: centre[2], p: [at(tl), at(tr), at(bl)], uv: b.uv[name], shade: Math.max(0, Math.min(SHADES.length - 1, Math.round((1 - Math.max(0, light)) * 3.2))) });
        }
      }
      faces.sort((a, b) => a.depth - b.depth);
      ctx.setTransform(1, 0, 0, 1, 0, 0);
      ctx.clearRect(0, 0, canvas.width, canvas.height);
      const k = T * dpr, ox = (W / 2) * dpr, oy = (H / 2 - 4) * dpr - hop * dpr;
      for (const f of faces) {
        const [sx, sy2, tw, th] = f.uv;
        const [p0, p1, p2] = f.p;
        const ux = (p1[0] - p0[0]) / tw, uy = (p1[1] - p0[1]) / tw, vx = (p2[0] - p0[0]) / th, vy = (p2[1] - p0[1]) / th;
        ctx.setTransform(ux * k, uy * k, vx * k, vy * k, ox + p0[0] * k, oy + p0[1] * k);
        // A hair larger than the face, so neighbouring faces leave no seam.
        ctx.drawImage(textures[f.shade], sx, sy2, tw, th, 0, 0, tw + 0.1, th + 0.1);
      }
    };
    const tick = (now) => {
      frame = 0;
      if (spin) {
        const p = Math.min(1, (now - spin.t0) / 750), e = 1 - (1 - p) ** 3;
        yaw = spin.from + 360 * e; hop = Math.sin(Math.PI * p) * 22; pitch += (0 - pitch) * 0.12;
        if (p >= 1) { spin = null; hop = 0; }
      } else if (!drag) {
        yaw += vYaw; pitch = Math.max(-35, Math.min(25, pitch + vPitch)); vYaw *= 0.93; vPitch *= 0.9;
        if (Math.abs(vYaw) < 0.03 && Math.abs(vPitch) < 0.03) vYaw = vPitch = 0;
      }
      draw();
      if (spin || drag || vYaw || vPitch) frame = requestAnimationFrame(tick);
    };
    const run = () => { if (!frame) frame = requestAnimationFrame(tick); };
    wrap.addEventListener("pointerdown", (e) => {
      if (e.button !== 0) return;
      try { wrap.setPointerCapture(e.pointerId); } catch { /* synthetic or already released pointer */ }
      drag = { x: e.clientX, y: e.clientY, moved: 0, t0: performance.now() }; vYaw = vPitch = 0; spin = null; hop = 0;
      wrap.classList.add("grabbing"); run();
    });
    wrap.addEventListener("pointermove", (e) => {
      if (!drag) return;
      const dx = e.clientX - drag.x, dy = e.clientY - drag.y;
      drag.x = e.clientX; drag.y = e.clientY; drag.moved += Math.abs(dx) + Math.abs(dy);
      yaw += dx * 0.8; pitch = Math.max(-35, Math.min(25, pitch - dy * 0.4));
      vYaw = dx * 0.8; vPitch = -dy * 0.4;
    });
    const release = () => {
      if (!drag) return;
      const click = drag.moved < 6 && performance.now() - drag.t0 < 350;
      drag = null; wrap.classList.remove("grabbing");
      if (click) { vYaw = vPitch = 0; spin = { from: yaw, t0: performance.now() }; }
      run();
    };
    wrap.addEventListener("pointerup", release);
    wrap.addEventListener("pointercancel", release);
    draw();
  };
  if (img.complete && img.naturalWidth) init(); else img.addEventListener("load", init, { once: true });
  return wrap;
}
