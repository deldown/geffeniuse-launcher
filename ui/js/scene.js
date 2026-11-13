"use strict";
// The banner follows the local clock.
function timeOfDay(d = new Date()) {
  const h = d.getHours() + d.getMinutes() / 60;
  return h >= 7.5 && h < 17.5 ? "day" : (h >= 5 && h < 7.5) || (h >= 17.5 && h < 20) ? "dusk" : "night";
}
const EVENT_THEMES = ["halloween", "christmas", "newyear"];
// The calendar alone decides which scenery is active.
function themeFor(d = new Date()) {
  const m = d.getMonth() + 1, day = d.getDate();
  if ((m === 10 && day >= 24) || (m === 11 && day <= 1)) return "halloween";
  if (m === 12 && day <= 26) return "christmas";
  if ((m === 12 && day >= 27) || (m === 1 && day <= 2)) return "newyear";
  if (m >= 3 && m <= 5) return "spring";
  if (m >= 6 && m <= 8) return "summer";
  if (m >= 9 && m <= 11) return "autumn";
  return "winter";
}
// "auto" colour is blue, except during the three events, which bring their own palette.
function colorFor(theme, pref = state.settings?.color || "auto") {
  return pref !== "auto" ? pref : EVENT_THEMES.includes(theme) ? theme : "blue";
}
function sceneFile(theme, tod) {
  if (EVENT_THEMES.includes(theme)) return `hero-${theme}.svg`;
  return theme === "none" ? `hero-${tod}.svg` : `hero-${theme}-${tod}.svg`;
}
let lightningTimer = 0;
let sceneImage = null;
// Rockets only go up at midnight: from 00:00 for half an hour on 1 January.
const fireworksAt = (d = new Date()) => d.getMonth() === 0 && d.getDate() === 1 && d.getHours() === 0 && d.getMinutes() < 30;
function nextFireworkEdge(d) {
  const y = d.getFullYear();
  return [new Date(y, 0, 1, 0, 0), new Date(y, 0, 1, 0, 30), new Date(y + 1, 0, 1, 0, 0)].map((e) => e - d).filter((ms) => ms > 0).sort((a, b) => a - b)[0];
}
let edgeTimer = 0, shownScene = "";
function applyTheme() {
  const root = document.documentElement, s = state.settings || {};
  const theme = s.seasonal === false ? "none" : themeFor(), tod = timeOfDay();
  root.dataset.tod = tod;
  root.dataset.theme = theme;
  root.dataset.color = colorFor(theme);
  root.dataset.scene = s.scene === false ? "off" : "on";
  root.dataset.effects = s.effects === false ? "off" : "on";
  root.dataset.fireworks = fireworksAt() ? "on" : "off";
  const scene = `art/${sceneFile(theme, tod)}`;
  if (scene !== shownScene) {
    shownScene = scene;
    sceneImage = new Image();
    sceneImage.src = scene;
    sceneImage.decode?.().catch(() => {});
  }
  root.style.setProperty("--scene", s.scene === false ? "none" : `url(${scene})`);
  clearTimeout(lightningTimer);
  if (theme === "halloween" && s.effects !== false) scheduleLightning();
  clearTimeout(edgeTimer);
  const edge = nextFireworkEdge(new Date());
  if (theme === "newyear" && edge < 6 * 60 * 1000) edgeTimer = setTimeout(applyTheme, edge + 100);
}
// A soft, rare flash in the sky: one slow pulse every 25 to 60 seconds, never when the window is
// hidden, and never with reduced motion. It fades in and out over about a second, not a strobe.
function scheduleLightning() {
  clearTimeout(lightningTimer);
  lightningTimer = setTimeout(() => {
    if (!document.hidden && !matchMedia("(prefers-reduced-motion: reduce)").matches && state.settings?.animations !== false) {
      const el = document.querySelector(".hero .bolt, .pagehead.scene .bolt");
      if (el) { el.classList.remove("on"); void el.offsetWidth; el.classList.add("on"); }
    }
    scheduleLightning();
  }, 25000 + Math.random() * 35000);
}
