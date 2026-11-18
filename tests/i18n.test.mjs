import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";

const ctx = vm.createContext({});
vm.runInContext(readFileSync(new URL("../ui/i18n.js", import.meta.url), "utf8") + "\nLANG = 'en'; this.tr = tr;", ctx);

const MUST_TRANSLATE = [
  "Benötigte Mods installieren?",
  "Dieser Mod braucht einen weiteren Mod, damit er läuft:",
  "Dieser Mod braucht 3 weitere Mods, damit er läuft:",
  "Nur diesen Mod",
  "Mit Abhängigkeit installieren",
  "Mit Abhängigkeiten installieren",
  "Abhängigkeiten",
  "1 Abhängigkeit",
  "3 Abhängigkeiten",
  "3 Abhängigkeiten · 1 installiert",
  "Dieser Mod braucht keine weiteren Mods.",
  "Lizenz",
  "Aktualisiert",
  "Zeigt nur Mods für Minecraft 1.21.1 mit fabric. Beim Installieren fragen wir, ob benötigte Mods mitkommen sollen.",
  "Öffnet den Ordner dieses Modpacks im Dateimanager, zum Beispiel um Mods von Hand hineinzulegen.",
  "Modpack-Ordner",
  "4 Einträge",
  "In diesem Modpack suchen …",
  "Die Landschaft folgt dem Datum. Automatisch ist Blau, zu Halloween, Weihnachten und Silvester mit passenden Farben.",
  "Spiel beendet mit Code 1",
  "Version 0.18.0 ist verfügbar",
  "Gruppe für 1 Eintrag",
  "Gruppe für 5 Einträge",
  "1 Mod aktualisiert",
  "4 Mods aktualisiert",
  "Neue Version 0.6.1",
  "Sodium und 2 benötigte Mods installiert",
  "Sodium installiert",
  "Weiteres Fenster",
  "Weiteres Fenster (2 offen)",
  "Du hast die neueste Version (0.17.6).",
  "Version 0.18.0 ist verfügbar (du hast 0.17.6).",
  "Lade Version 0.18.0 und prüfe sie …",
  "Version 0.18.0 ist installiert. Starte den Launcher neu, um sie zu nutzen.",
  "Du hast Version 0.17.6.",
  "Erneut suchen",
  "Aktualisiere …",
  "Zum Hinzufügen loslassen",
];

test("every German UI sentence has an English translation", () => {
  const missing = MUST_TRANSLATE.filter((s) => ctx.tr(s) === s);
  assert.deepEqual(missing, []);
});

test("numbers and names survive translation", () => {
  assert.equal(ctx.tr("3 Abhängigkeiten · 1 installiert"), "3 dependencies · 1 installed");
  assert.equal(ctx.tr("Sodium und 2 benötigte Mods installiert"), "Sodium and 2 required mods installed");
  assert.equal(ctx.tr("Version 0.18.0 ist verfügbar (du hast 0.17.6)."), "Version 0.18.0 is available (you have 0.17.6).");
});
