"use strict";
// UI language. The source strings in the files under js/ are German; English is looked up here, either as an
// exact entry or through a pattern for sentences that carry numbers and names.
let LANG = "de";

const EN = {
  "Discord Rich Presence": "Discord Rich Presence",
  "Aktivität mit Discord teilen": "Share activity with Discord",
  "Teilt nach dem Einschalten „Im Launcher“ oder „Spielt Minecraft“ mit deiner Discord-Desktop-App. Discord kann die Aktivität in deinem Profil anzeigen. Keine Konto-, Server-, Welt- oder Modpack-Namen.": "When enabled, shares ‘In the launcher’ or ‘Playing Minecraft’ with your Discord desktop app. Discord may show the activity on your profile. No account, server, world or modpack names.",
  "Standardmäßig aus. Ausschalten entfernt die Anzeige. Discord muss lokal laufen; beim Schließen des Launchers endet die Anzeige, auch wenn Minecraft weiterläuft.": "Off by default. Switching off removes the activity. Discord must be running locally; closing the launcher ends the activity even if Minecraft keeps running.",
  "Discord Application ID must contain 17–20 digits": "Discord Application ID must contain 17–20 digits",

  "Start": "Home", "Bibliothek": "Library", "Entdecken": "Discover", "Screenshots": "Screenshots", "Konsole": "Console", "Welten": "Worlds",
  "Optionen": "Options", "Hauptmenü": "Main menu", "Anmelden": "Sign in", "Konto wechseln": "Switch account", "Konto hinzufügen": "Add account",
  "Konto": "Account", "Mit Microsoft anmelden": "Sign in with Microsoft", "Anmeldung nötig": "Sign-in required",
  "Zum Spielen brauchst du ein Microsoft-Konto mit Minecraft: Java Edition.": "You need a Microsoft account with Minecraft: Java Edition to play.",
  "Nicht angemeldet. Melde dich mit Microsoft an, um zu spielen.": "Not signed in. Sign in with Microsoft to play.",
  "Nutzen": "Use", "Entfernen": "Remove", "Entfernt": "Removed",
  "Willkommen": "Welcome", "Willkommen zurück": "Welcome back", "Geffeniuse Launcher": "Geffeniuse Launcher",
  "Lege deine erste Instanz an": "Create your first instance", "Deine Instanzen": "Your instances",
  "Neue Instanz": "New instance", "Neues Modpack": "New modpack", "Mein Modpack": "My modpack", "← Mods hinzufügen": "← Add mods", "Vanilla, Fabric, Quilt, Forge, NeoForge": "Vanilla, Fabric, Quilt, Forge, NeoForge",
  "Modpacks entdecken": "Discover modpacks", "Modrinth und CurseForge": "Modrinth and CurseForge",
  "Modpack importieren": "Import modpack", ".mrpack oder CurseForge-.zip": ".mrpack or CurseForge .zip",
  "Noch keine Instanzen.": "No instances yet.", "Spielen": "Play", "Löschen": "Delete", "Endgültig löschen": "Delete for good", "Abbrechen": "Cancel",
  "Beenden": "Quit game", "Minecraft läuft": "Minecraft is running", "Bereite vor …": "Preparing …", "Noch nie gespielt": "Never played",
  "Gerade eben gespielt": "Played just now", "unter 1 Min. gespielt": "played under 1 min",
  "Welten, Mods und Einstellungen dieser Instanz werden von der Festplatte gelöscht.": "Worlds, mods and settings of this instance will be deleted from disk.",
  "Abbrechen bzw. Spiel beenden": "Cancel, or quit the game", "Umbenennen": "Rename", "Icon ändern": "Change icon",
  "Standard-Icon wiederherstellen": "Restore default icon", "Name": "Name", "Minecraft-Version": "Minecraft version", "Loader": "Loader",
  "Erstellen": "Create", "Meine Instanz": "My instance", "← Bibliothek": "← Library", "← Entdecken": "← Discover",
  "Import läuft …": "Importing …",
  "Mods": "Mods", "Mods hinzufügen": "Add mods", "Mods suchen": "Search mods", "Mods suchen …": "Search mods …",
  "Noch keine Mods in dieser Instanz.": "No mods in this instance yet.", "Vanilla lädt keine Mods.": "Vanilla loads no mods.",
  "Lege eine neue Instanz mit Fabric, Quilt, Forge oder NeoForge an, um Mods zu nutzen.": "Create a new instance with Fabric, Quilt, Forge or NeoForge to use mods.",
  "Aktiv": "Enabled", "Aktivieren": "Enable", "Deaktivieren": "Disable", "Installieren": "Install", "Installiert": "Installed", "Lädt …": "Loading …",
  "Manueller Download nötig": "Manual download needed", "Manuelle Downloads nötig": "Manual downloads needed",
  "Die Autoren erlauben für diese Mods keinen Download durch Launcher. Lade sie auf curseforge.com herunter und lege sie in den Mods-Ordner der Instanz:": "The authors do not allow launchers to download these mods. Download them from curseforge.com and put them into the instance's mods folder:",
  "CurseForge erlaubt für diese Dateien keinen Download durch Launcher. Lade sie auf curseforge.com herunter und lege sie in den Mods-Ordner der Instanz:": "CurseForge does not allow launchers to download these files. Download them from curseforge.com and put them into the instance's mods folder:",
  " CurseForge schaltest du in den Einstellungen mit deinem API-Key frei.": " You unlock CurseForge in the options with your API key.",
  "Modpacks suchen …": "Search modpacks …", "Suche": "Search", "Suchen": "Search", "Suche …": "Searching …", "Nichts gefunden.": "Nothing found.",
  "Beschreibung": "Description", "Galerie": "Gallery", "Versionen": "Versions", "Lade Versionen …": "Loading versions …", "Version": "Version",
  "Keine Beschreibung.": "No description.", "Links": "Links", "Details": "Details", "Lizenz": "License", "Aktualisiert": "Updated",
  "Herunterladen": "Download", "Minecraft": "Minecraft", "Schließen": "Close",
  "Noch keine Screenshots. Drücke F2 im Spiel.": "No screenshots yet. Press F2 in game.", "Noch keine Ausgabe. Starte eine Instanz.": "No output yet. Start an instance.",
  "Leeren": "Clear", "Noch keine Welten. Sie erscheinen, sobald du in einer Instanz gespielt hast.": "No worlds yet. They appear once you have played an instance.",
  "Module": "Modules", "Blende aus, was du nicht brauchst.": "Hide what you do not need.", "Aussehen": "Appearance", "Farbe": "Color",
  "Automatisch": "Automatic", "Automatisch (nach Datum)": "Automatic (by date)", "Aus": "Off", "Blau": "Blue", "Grün": "Green", "Rot": "Red", "Gold": "Gold", "Lila": "Purple",
  "Automatisch ist Blau, zu Halloween, Weihnachten und Silvester mit passenden Farben.": "Automatic is blue, with matching colors at Halloween, Christmas and New Year's Eve.",
  "Jahreszeit und Events": "Seasons and events", "Halloween": "Halloween", "Weihnachten": "Christmas", "Silvester": "New Year's Eve",
  "Frühling": "Spring", "Sommer": "Summer", "Herbst": "Autumn", "Winter": "Winter", "Pixelgröße (1–4)": "Pixel size (1–4)",
  "Animationen": "Animations", "Kompakte Listen": "Compact lists", "Sprache": "Language", "Deutsch": "Deutsch", "English": "English",
  "Spiel": "Game", "Standard-Arbeitsspeicher (MB)": "Default memory (MB)", "Java-Pfad": "Java path", "Zusätzliche JVM-Argumente": "Extra JVM arguments",
  "Parallele Downloads": "Parallel downloads", "Launcher nach Spielstart schließen": "Close launcher after game start", "Quellen": "Sources",
  "Braucht einen API-Key": "Needs an API key", "Landschaft in den Bannern": "Landscape in the banners", "Jahreszeiten und Events (Halloween, Weihnachten, Silvester …)": "Seasons and events (Halloween, Christmas, New Year …)", "Bewegte Effekte (Schnee, Feuerwerk, Blitze)": "Moving effects (snow, fireworks, lightning)", "3D-Figur auf der Startseite": "3D figure on the start page", "Spielzeit anzeigen": "Show play time", "Eigener CurseForge API-Key (optional)": "Your own CurseForge API key (optional)", "leer lassen": "leave empty", "CurseForge API-Key": "CurseForge API key", "Modrinth": "Modrinth", "CurseForge": "CurseForge",
  "automatisch": "automatic", "Einrichtung wiederholen": "Run setup again",
  "Updates": "Updates", "Nach Updates suchen": "Check for updates", "Erneut suchen": "Check again", "Erneut versuchen": "Try again",
  "Jetzt aktualisieren": "Update now", "Aktualisieren": "Update", "Aktualisiere …": "Updating …", "Aktualisiert.": "Updated.", "Jetzt neu starten": "Restart now", "Später": "Later",
  "Beim Start automatisch nach Updates suchen": "Check for updates on start",
  "Es ist noch keine Version veröffentlicht.": "No version has been published yet.",
  "Die Datei ist beschädigt angekommen. Das Update wurde verworfen, deine Version ist unverändert.": "The file arrived damaged. The update was discarded; your version is unchanged.",
  "Dieser Launcher läuft nicht als AppImage und kann sich nicht selbst ersetzen. Installiere die neue Datei von Hand.": "This launcher cannot replace itself. Install the new file by hand.",
  "Neustart ausstehend. Die neue Version läuft beim nächsten Start.": "Restart pending. The new version runs on the next start.",
  "Einrichtung": "Setup", "Überspringen": "Skip", "Weiter": "Next", "Zurück": "Back", "Los geht's": "Let's go",
  "Willkommen bei Geffeniuse Launcher": "Welcome to Geffeniuse Launcher", "Wähle deine Sprache": "Choose your language",
  "Du kannst das später in den Optionen ändern.": "You can change this later in the options.",
  "Wähle dein Aussehen": "Pick your look", "Minecraft-Konten": "Minecraft accounts",
  "Melde dich mit deinem Microsoft-Konto an. Du kannst mehrere Konten hinzufügen und später oben wechseln.": "Sign in with your Microsoft account. You can add several accounts and switch between them at the top.",
  "Microsoft-Konto hinzufügen": "Add Microsoft account", "Noch kein Konto angemeldet.": "No account signed in yet.",
  "Alles bereit": "All set", "Du kannst jetzt Modpacks entdecken, Instanzen anlegen und spielen.": "You can now discover modpacks, create instances and play.",
  "Zum Spielen ist ein Microsoft-Konto nötig. Du kannst es auch später hinzufügen.": "A Microsoft account is needed to play. You can also add it later.",
  "Vorschau": "Preview",
};

// Sentences with numbers or names. Each entry: [regex over the German text, builder].
const PATTERNS = [
  [/^Zuletzt vor (\d+) Min\.$/, (n) => `Last played ${n} min ago`],
  [/^Zuletzt vor (\d+) Std\.$/, (n) => `Last played ${n} h ago`],
  [/^Zuletzt vor (\d+) Tagen$/, (n) => `Last played ${n} days ago`],
  [/^(\d+) Min\. gespielt$/, (n) => `${n} min played`],
  [/^(\d+) Std\. (\d+) Min\. gespielt$/, (h, m) => `${h} h ${m} min played`],
  [/^(\d+) Std\. gespielt$/, (h) => `${h} h played`],
  [/^(\d+) Instanzen? bereit$/, (n) => (n === "1" ? "1 instance ready" : `${n} instances ready`)],
  [/^(\d+) von (\d+) aktiv$/, (a, b) => `${a} of ${b} enabled`],
  [/^(\d+) Welten$/, (n) => `${n} worlds`],
  [/^(\d+) Bilder$/, (n) => `${n} images`],
  [/^(.+) Downloads$/, (n) => `${n} downloads`],
  [/^(.+) Follower$/, (n) => `${n} followers`],
  [/^\s*von (.+)$/, (a) => `  by ${a}`],
  [/^Version (.+) ist verfügbar \(du hast (.+)\)\.$/, (a, b) => `Version ${a} is available (you have ${b}).`],
  [/^Version (.+) ist verfügbar$/, (a) => `Version ${a} is available`],
  [/^Version (.+) ist installiert\. Starte den Launcher neu, um sie zu nutzen\.$/, (a) => `Version ${a} is installed. Restart the launcher to use it.`],
  [/^Du hast Version (.+)\.$/, (a) => `You have version ${a}.`],
  [/^Du hast die neueste Version \((.+)\)\.$/, (a) => `You have the latest version (${a}).`],
  [/^Lade Version (.+) und prüfe sie …$/, (a) => `Downloading version ${a} and verifying it …`],
  [/^Updates sind gerade nicht erreichbar\. (.*)$/, (a) => `Updates are not reachable right now. ${a}`],
  [/^Nichts gefunden für (.+) mit (.+)\.$/, (a, b) => `Nothing found for ${a} with ${b}.`],
  [/^Zeigt nur Mods für Minecraft (.+) mit (.+)\. Benötigte Mods werden automatisch mitinstalliert\.( .*)?$/, (a, b, c) => `Only shows mods for Minecraft ${a} with ${b}. Required mods are installed automatically.${c ? " You unlock CurseForge in the options with your API key." : ""}`],
  [/^(.+) und (\d+) benötigte Mods installiert$/, (a, n) => `${a} and ${n} required mods installed`],
  [/^(.+) installiert$/, (a) => `${a} installed`],
  [/^„(.+)“ installiert$/, (a) => `“${a}” installed`],
  [/^„(.+)“ importiert$/, (a) => `“${a}” imported`],
  [/^„(.+)“ löschen\?$/, (a) => `Delete “${a}”?`],
  [/^Spiel beendet mit Code (.+)$/, (a) => `Game exited with code ${a}`],
  [/^Angemeldet als (.+)$/, (a) => `Signed in as ${a}`],
  [/^Konto: (.+)$/, (a) => `Account: ${a}`],
  [/^Konto wechseln, angemeldet als (.+)$/, (a) => `Switch account, signed in as ${a}`],
  [/^Skin nicht geladen: (.+)$/, (a) => `Skin not loaded: ${a}`],
  [/^Auf (.+) öffnen ↗$/, (a) => `Open on ${a} ↗`],
  [/^(.+) ↗$/, (a) => `${a} ↗`],
];

function tr(text) {
  if (LANG !== "en" || typeof text !== "string" || !text) return text;
  const hit = EN[text];
  if (hit !== undefined) return hit;
  const t = text.trim();
  if (t !== text && EN[t] !== undefined) return text.replace(t, EN[t]);
  for (const [re, build] of PATTERNS) {
    const m = re.exec(text);
    if (m) return build(...m.slice(1));
  }
  return text;
}

Object.assign(EN, {
  "Inhalt": "Content", "Hinzufügen": "Add", "Noch kein Inhalt in diesem Modpack.": "No content in this modpack yet.",
  "Füge Mods, Ressourcenpakete, Shader oder Datapacks hinzu, oder ziehe Dateien einfach hierher.": "Add mods, resource packs, shaders or data packs, or just drag files here.",
  "Inhalt hinzufügen": "Add content", "Alle auswählen": "Select all", "Auswählen": "Select", "Inhalt durchsuchen …": "Search content …", "Inhalt durchsuchen": "Search content",
  "Sortieren": "Sort", "Name (A–Z)": "Name (A–Z)", "Name (Z–A)": "Name (Z–A)", "Größe": "Size", "Typ": "Type", "Aktive zuerst": "Active first",
  "aktiv": "active", "Alle": "All", "Ressourcenpakete": "Resource packs", "Ressourcenpaket": "Resource pack", "Shader": "Shaders", "Datapacks": "Data packs", "Datapack": "Data pack", "Mod": "Mod",
  "ausgewählt": "selected", "Aktivieren": "Enable", "Deaktivieren": "Disable", "Auswahl aufheben": "Clear selection", "Auswahl entfernen?": "Remove selection?",
  "Einträge werden endgültig gelöscht.": "items will be deleted for good.", "Aktiv": "Enabled", "Nichts gefunden.": "Nothing found.", "Suchen …": "Search …", "Mods suchen …": "Search mods …",
  "Welt": "World", "Datapacks gehören zu einer Welt. Erstelle zuerst eine Welt, indem du das Modpack startest.": "Data packs belong to a world. Create a world first by starting the modpack.",
  "Noch keine Welten. Sie erscheinen, sobald du in diesem Modpack gespielt hast.": "No worlds yet. They appear once you have played this modpack.",
  "Noch keine Ausgabe. Starte dieses Modpack.": "No output yet. Start this modpack.", "Wähle zuerst eine Welt": "Pick a world first", "← Hinzufügen": "← Add",
  "Zum Hinzufügen loslassen": "Drop to add", ".jar, .zip oder .mrpack": ".jar, .zip or .mrpack", "Öffne ein Modpack, um Dateien hinzuzufügen": "Open a modpack to add files",
  "Datei hinzugefügt": "file added", "Dateien hinzugefügt": "files added", "Neues Modpack": "New modpack",
  "Unterstützen": "Support", "Auf Ko-fi unterstützen": "Support on Ko-fi", "Geffeniuse unterstützen (Ko-fi)": "Support Geffeniuse (Ko-fi)", "Geffeniuse unterstützen": "Support Geffeniuse",
  "Geffeniuse ist kostenlos und ohne Werbung. Wenn es dir gefällt, kannst du die Arbeit auf Ko-fi unterstützen. Das ist freiwillig.": "Geffeniuse is free and has no ads. If you like it, you can support the work on Ko-fi. It is entirely optional.",
  "keine Datei": "not a file", "zu groß": "too large", "Vanilla lädt keine Mods": "Vanilla loads no mods", "erstelle zuerst eine Welt": "create a world first",
  "kein Ressourcenpaket, Shader oder Datapack": "not a resource pack, shader or data pack", "Dateityp nicht unterstützt": "unsupported file type", "schon vorhanden": "already there",
});
PATTERNS.push([/^(\d+) Datei hinzugefügt$/, (n) => n + " file added"], [/^(\d+) Dateien hinzugefügt$/, (n) => n + " files added"]);

Object.assign(EN, {
  "Einstellungen": "Settings", "Einstellungen werden geteilt": "Settings are shared", "Dieses Modpack teilt keine Einstellungen mehr": "This modpack no longer shares settings",
  "Als gemeinsame Einstellungen gespeichert": "Saved as shared settings", "Gemeinsame Einstellungen übernommen": "Shared settings applied", "Schon aktuell": "Already up to date",
  "Minecraft-Einstellungen teilen": "Share Minecraft settings", "Einstellungen teilen": "Share settings",
  "Sichtfeld, Steuerung, Grafik und die Serverliste gelten für alle Modpacks, die mitmachen. Vor dem Start übernimmt dieses Modpack die gemeinsamen Werte, nach dem Beenden fließen seine Änderungen zurück.": "Field of view, controls, graphics and the server list apply to every modpack that takes part. Before it starts, this modpack takes over the shared values; when it ends, its changes flow back.",
  "Nicht geteilt werden die aktiven Ressourcenpakete, die Spielversion und der zuletzt besuchte Server. Alte Versionen (bis 1.12) teilen nur untereinander, weil ihre Steuerung anders gespeichert wird.": "The active resource packs, the game version and the last server are not shared. Old versions (up to 1.12) only share among themselves, because they store controls differently.",
  "In den Optionen ist das Teilen insgesamt ausgeschaltet.": "Sharing is switched off altogether in the options.",
  "Meine Einstellungen als gemeinsame speichern": "Save my settings as the shared ones", "Gemeinsame Einstellungen hier anwenden": "Apply the shared settings here",
  "Minecraft-Einstellungen zwischen Modpacks teilen": "Share Minecraft settings between modpacks", "Sichtfeld, Steuerung und Serverliste einmal einstellen. Jedes Modpack kann einzeln aussteigen.": "Set field of view, controls and the server list once. Each modpack can opt out on its own.",
});

Object.assign(EN, {
  "Zuletzt gespielt": "Played recently", "Deine Modpacks": "Your modpacks", "Beitreten": "Join", "Welt": "World", "Server": "Server", "Modpack": "Modpack", "Modpacks": "Modpacks",
  "Das Modpack läuft schon": "The modpack is already running", "Direktes Beitreten geht mit dieser Minecraft-Version nicht. Das Modpack startet normal.": "This Minecraft version cannot join directly. The modpack starts normally.",
  "♥ Geffeniuse unterstützen": "♥ Support Geffeniuse", "Lege dein erstes Modpack an": "Create your first modpack",
  "In eigenem Fenster öffnen": "Open in its own window", "Kopieren": "Copy",
  "Arbeitsspeicher": "Memory", "Standard verwenden": "Use the default", "Dein Rechner hat": "Your computer has", "Gespeichert": "Saved",
  "Wie viel RAM dieses Modpack höchstens nutzen darf. Große Modpacks brauchen mehr, aber mehr ist nicht automatisch schneller.": "How much RAM this modpack may use at most. Large modpacks need more, but more is not automatically faster.",
  "Das ist viel im Verhältnis zu deinem Arbeitsspeicher. Zu viel kann das Spiel ausbremsen.": "That is a lot compared to your RAM. Too much can slow the game down.",
  "Nach dem Start des Spiels": "After the game starts", "Launcher offen lassen": "Keep the launcher open", "Launcher minimieren": "Minimize the launcher", "Launcher schließen": "Close the launcher",
  "Das Spiel läuft auch bei geschlossenem Launcher weiter. Beim nächsten Öffnen erkennt er es wieder. Mit eigenem Konsolenfenster wird nur minimiert.": "The game keeps running when the launcher is closed. It recognises it the next time it opens. With its own console window it only minimizes.",
  "Konsole nach dem Start in einem eigenen Fenster öffnen": "Open the console in its own window after the start",
  "Startseite": "Start page", "Zuletzt gespielte Welten und Server": "Recently played worlds and servers", "Nicht anzeigen": "Do not show", "Modpacks anzeigen": "Show modpacks",
  "Schnellaktionen anzeigen (Neues Modpack, Entdecken, Importieren)": "Show quick actions (new modpack, discover, import)", "Niedrigerer Banner": "Lower banner",
  "Die 3D-Figur und die Landschaft stellst du unter Aussehen ein.": "You set the 3D figure and the landscape under Appearance.",
});
PATTERNS.push([/^(\d+) Modpack bereit$/, (n) => n + " modpack ready"], [/^(\d+) Modpacks bereit$/, (n) => n + " modpacks ready"]);

Object.assign(EN, {
  "Wähle eine Minecraft-Version": "Pick a Minecraft version", "Unterversion": "Minor version", "Loader": "Loader", "Erstellen": "Create",
  "Versionsauswahl bei „Neues Modpack“": "Version picker for “New modpack”", "Mit Bildern (schön)": "With pictures (fancy)", "Einfache Liste": "Simple list", "Versionsauswahl": "Version picker",
  "Ohne Mods, so wie Mojang es baut.": "No mods, just the way Mojang builds it.", "Leicht und schnell, viele moderne Mods.": "Light and fast, plenty of modern mods.",
  "Fabric-kompatibel, offen entwickelt.": "Fabric-compatible, developed in the open.", "Der Klassiker für große Modpacks.": "The classic for big modpacks.", "Der Nachfolger von Forge für neue Versionen.": "Forge's successor for newer versions.",
  "Prüfungskammern, Windladungen und Kupfer in allen Formen.": "Trial chambers, wind charges and copper in every shape.", "Kirschhaine, Bambus, Kamele und Archäologie.": "Cherry groves, bamboo, camels and archaeology.",
  "Mangrovensümpfe, die Tiefen Dunkelheit und der Warden.": "Mangrove swamps, the Deep Dark and the Warden.", "Höhere Berge, tiefere Höhlen und eine neue Weltgenerierung.": "Taller mountains, deeper caves and a new world generation.",
  "Amethyst, Kupfer, Axolotl und Ziegen.": "Amethyst, copper, axolotls and goats.", "Karmesin- und Wirrwälder, Piglins und Netherit.": "Crimson and warped forests, piglins and netherite.", "Bienen, Honig und Bienenstöcke.": "Bees, honey and beehives.",
  "Neue Dörfer, Plünderer und Armbrüste.": "New villages, pillagers and crossbows.", "Ozeane, Schildkröten, Delfine und Seetang.": "Oceans, turtles, dolphins and kelp.", "Beton, glasierte Keramik und Papageien.": "Concrete, glazed terracotta and parrots.",
  "Waldanwesen, Totems und Shulkerkisten.": "Woodland mansions, totems and shulker boxes.", "Magmablöcke, Eisbären und Nether-Warzenblöcke.": "Magma blocks, polar bears and nether wart blocks.", "Schild, Elytren und das neue Kampfsystem.": "Shields, elytra and the new combat system.",
  "Der Klassiker für PvP und viele Server.": "The classic for PvP and many servers.", "Neue Biome und eine neue Weltgenerierung.": "New biomes and a new world generation.", "Neue Version von Mojang.": "A new version from Mojang.",
});
Object.assign(EN, { "Bildnachweis": "Credits", "Die Titelbilder der Minecraft-Versionen ab 1.13 stammen von Mojang, die der Versionen 1.1 bis 1.12 aus der Minecraft Wiki (minecraft.wiki, CC BY-NC-SA 3.0). Minecraft ist eine Marke von Mojang AB. Die Loader-Logos gehören den Projekten Fabric, Quilt, Forge und NeoForge.": "The title pictures of Minecraft versions from 1.13 are from Mojang, those of versions 1.1 to 1.12 from the Minecraft Wiki (minecraft.wiki, CC BY-NC-SA 3.0). Minecraft is a trademark of Mojang AB. The loader logos belong to the Fabric, Quilt, Forge and NeoForge projects." });
Object.assign(EN, { "Releases": "Releases", "Snapshots": "Snapshots", "Experimentell": "Experimental", "Aprilscherze": "April Fools", "Snapshots, Experimente und Aprilscherze laufen nur ohne Loader und sind nicht für Spielstände gedacht, die du behalten willst.": "Snapshots, experiments and April Fools versions only run without a loader and are not meant for worlds you want to keep.",
 "Snapshots und Pre-Releases anbieten": "Offer snapshots and pre-releases", "Experimentelle Versionen anbieten": "Offer experimental versions", "Aprilscherz-Versionen anbieten": "Offer April Fools versions",
 "Das dritte Update 2026, im neuen Jahresschema von Mojang.": "The third update of 2026, in Mojang's new yearly scheme.", "Das zweite Update 2026, im neuen Jahresschema von Mojang.": "The second update of 2026, in Mojang's new yearly scheme.", "Das erste Update 2026 und der Start des neuen Jahresschemas.": "The first update of 2026 and the start of the new yearly scheme.",
 "Pferde, Esel, Namensschilder und Ressourcenpakete.": "Horses, donkeys, name tags and resource packs.", "Trichter, Spender, Komparatoren und Redstone-Blöcke.": "Hoppers, droppers, comparators and redstone blocks.", "Wither, Hexen, Fledermäuse und Kommandoblöcke.": "Wither, witches, bats and command blocks.",
 "Dorfbewohner handeln mit Smaragden, dazu Einzelspieler im LAN.": "Villagers trade for emeralds, plus single player over LAN.", "Dschungel, Ozelots und ein neues Weltformat.": "Jungles, ocelots and a new world format.", "Superflache Welten und neue Sprachen.": "Superflat worlds and new languages.", "Die erste Vollversion: Ende, Enderdrache und Brauen.": "The first full release: the End, the Ender Dragon and brewing." });
Object.assign(EN, { "Hinweise und Bildnachweis": "Notices and credits", "CurseForge über den deldown-Proxy": "CurseForge through the deldown proxy",
 "Ohne eigenen Schlüssel gehen Suchanfragen und deine IP-Adresse über api-geffeniuse.deldown.de zu CurseForge. Aus: nur mit eigenem Schlüssel.": "Without a key of your own, search requests and your IP address go through api-geffeniuse.deldown.de to CurseForge. Off: only with your own key.",
 "Geffeniuse Launcher ist ein inoffizieller Launcher von deldown. Minecraft ist eine Marke von Mojang AB / Microsoft. Verantwortlich für den Launcher ist deldown (Impressum: impressum.deldown.de, Kontakt: support@deldown.de), nicht Mojang oder Microsoft.": "Geffeniuse Launcher is an unofficial launcher by deldown. Minecraft is a trademark of Mojang AB / Microsoft. deldown (imprint: impressum.deldown.de, contact: support@deldown.de), not Mojang or Microsoft, is responsible for the launcher.",
 "Versionsbilder: ab 1.13 von Mojang (öffentlicher Feed des offiziellen Launchers); 1.6 bis 1.12 sind verkleinerte Bilder aus der Minecraft Wiki (minecraft.wiki), sie zeigen Spielmaterial von Mojang. Loader-Logos: Fabric (Fabric-Projekt), Quilt (Quilt-Projekt), Forge (Minecraft Forge), NeoForge (NeoForged), jeweils nur zur Kennzeichnung des Loaders. Schrift: Rubik (SIL Open Font License). Alle Rechte bei den jeweiligen Inhabern.": "Version pictures: from 1.13 on from Mojang (public feed of the official launcher); 1.6 to 1.12 are downsized pictures from the Minecraft Wiki (minecraft.wiki), they show Mojang game material. Loader logos: Fabric (Fabric project), Quilt (Quilt project), Forge (Minecraft Forge), NeoForge (NeoForged), each only to identify the loader. Font: Rubik (SIL Open Font License). All rights belong to their owners.",
 "Versionsbilder anzeigen": "Show version pictures", "Aus: einfache gemalte Bilder, und beim Öffnen von „Neues Modpack“ wird nichts bei Mojang geladen.": "Off: simple painted pictures, and nothing is loaded from Mojang when opening “New modpack”." });
Object.assign(EN, {
  "Selbst installiert": "Installed by me", "Aus Modpack": "From modpack", "Deaktiviert": "Disabled", "Update verfügbar": "Update available", "Filtern": "Filter", "Alle Quellen": "All sources",
  "Nach Updates suchen": "Check for updates", "Suche …": "Searching …", "Alle Mods sind aktuell": "All mods are up to date", "Alle aktualisieren": "Update all", "Aktualisieren": "Update",
  "Update gefunden": "update found", "Updates gefunden": "updates found", "Updates verfügbar": "updates available",
  "1 Mod aktualisiert": "1 mod updated", "Kopieren und exportieren": "Copy and export", "Duplizieren": "Duplicate", "Exportieren …": "Export …",
  "Dupliziere das Modpack samt Mods, Welten und Einstellungen, oder speichere es als Datei zum Teilen oder Sichern.": "Duplicate the modpack with its mods, worlds and settings, or save it as a file to share or back up.",
  "Modrinth-Modpack (.mrpack)": "Modrinth modpack (.mrpack)", "Sicherung (.zip)": "Backup (.zip)",
  "Zum Teilen. Mods von Modrinth werden verlinkt statt eingepackt, Einstellungen und andere Dateien sind dabei. Ohne Welten.": "For sharing. Mods from Modrinth are linked instead of packed, settings and other files are included. No worlds.",
  "Alles in einer Datei, auch Welten und Screenshots. Zum Sichern oder Umziehen.": "Everything in one file, including worlds and screenshots. For backups or moving.",
});
PATTERNS.push([/^(\d+) Mods aktualisiert$/, (n) => n + " mods updated"], [/^(\d+) Update gefunden$/, (n) => n + " update found"], [/^(\d+) Updates gefunden$/, (n) => n + " updates found"], [/^„(.+)“ erstellt$/, (n) => "“" + n + "” created"], [/^Gespeichert: (.+)$/, (p) => "Saved: " + p], [/^„(.+)“ exportieren$/, (n) => "Export “" + n + "”"]);
Object.assign(EN, { "Mod-Updates im Mods-Tab anbieten": "Offer mod updates in the mods tab", "Zeigt die Schaltfläche „Nach Updates suchen“. Es wird nie von selbst gesucht.": "Shows the “Check for updates” button. Nothing is ever checked on its own.", "Nach der Update-Suche nur Mods mit Update zeigen": "After the update check, show only mods with an update" });
Object.assign(EN, { "Gruppe …": "Group …", "Anlegen": "Create", "Aus Gruppe nehmen": "Remove from group", "Neue Gruppe": "New group", "Neue Gruppe, z. B. Performance": "New group, e.g. Performance",
 "Aus der Gruppe genommen": "Removed from group", "Zum Modpack": "To the modpack", "Konsole ansehen": "View console", "Schließen": "Close", "Genannt wird:": "Named:",
 "Absturz-Hilfe anzeigen": "Show crash help", "Erklärt in einfachen Worten, warum das Spiel abgestürzt ist, wenn der Launcher die Ursache erkennt.": "Explains in plain words why the game crashed, when the launcher recognises the cause.",
 "Gruppen im Mods-Tab anbieten": "Offer groups in the mods tab",
 "Java ist zu alt": "Java is too old", "Dieses Modpack braucht ein neueres Java. Lösche in den Optionen den eigenen Java-Pfad, dann lädt der Launcher das richtige Java selbst.": "This modpack needs a newer Java. Clear the custom Java path in the options and the launcher downloads the right one itself.",
 "Java ist zu neu": "Java is too new", "Dieses alte Spiel läuft nur mit Java 8. Lösche in den Optionen den eigenen Java-Pfad, dann nimmt der Launcher das passende Java.": "This old game only runs on Java 8. Clear the custom Java path in the options and the launcher picks the right Java.",
 "Zu wenig Arbeitsspeicher": "Not enough memory", "Dem Spiel ging der Speicher aus. Gib dem Modpack in den Einstellungen mehr RAM.": "The game ran out of memory. Give the modpack more RAM in its settings.",
 "Ein Mod ist doppelt vorhanden": "A mod is there twice", "Entferne die doppelte Datei im Mods-Tab. Meist ist es eine alte Version neben der neuen.": "Remove the duplicate file in the mods tab. Usually it is an old version next to the new one.",
 "Ein benötigter Mod fehlt": "A required mod is missing", "Ein Mod braucht einen anderen Mod, der nicht installiert ist. Suche ihn unter „Hinzufügen“.": "A mod needs another mod that is not installed. Find it under “Add”.",
 "Mods vertragen sich nicht": "Mods do not work together", "Mindestens zwei Mods passen nicht zusammen oder nicht zur Minecraft-Version. Prüfe im Mods-Tab, ob Updates da sind.": "At least two mods do not fit together or the Minecraft version. Check the mods tab for updates.",
 "Ein Mod stört einen anderen": "One mod interferes with another", "Zwei Mods greifen in dieselbe Stelle im Spiel ein. Deaktiviere den zuletzt hinzugefügten Mod, oder suche nach Updates.": "Two mods change the same part of the game. Disable the mod you added last, or look for updates.",
 "Grafikproblem": "Graphics problem", "Das Spiel konnte kein Fenster mit der Grafikkarte öffnen. Aktualisiere den Grafiktreiber.": "The game could not open a window with the graphics card. Update the graphics driver.",
 "Das System hat das Spiel beendet": "The system ended the game", "Meist war der Arbeitsspeicher voll. Verringere den RAM des Modpacks oder schließe andere Programme.": "Usually memory was full. Lower the modpack's RAM or close other programs." });
PATTERNS.push([/^In Gruppe „(.+)“ gelegt$/, (g) => "Put in group “" + g + "”"], [/^Gruppe für (\d+) Einträge?$/, (n) => "Group for " + n + (n === "1" ? " entry" : " entries")], [/^Gruppe: (.+)$/, (g) => "Group: " + g]);
Object.assign(EN, { "Als Ordner": "As folders", "Als Liste": "As list", "Gruppen als Ordner oder als einfache Liste zeigen": "Show groups as folders or as a plain list", "Gruppen als Ordner zeigen": "Show groups as folders", "Aus: eine einfache Liste, die Gruppe steht als Tag an jedem Eintrag.": "Off: a plain list, the group is a tag on each entry.",
 "Weiteres Fenster": "Another window", "Weiteres Fenster eines laufenden Modpacks erlauben": "Allow another window of a running modpack", "Zeigt „Weiteres Fenster“, solange ein Modpack läuft.": "Shows “Another window” while a modpack is running.",
 "Startet das Modpack ein weiteres Mal, zum Beispiel für einen zweiten Spieler im LAN": "Starts the modpack once more, for example for a second player on the LAN", "Weiteres Fenster starten?": "Start another window?", "Fenster starten": "Start window",
 "Beide Fenster nutzen denselben Modpack-Ordner. Öffne darin nicht dieselbe Welt zweimal, sonst kann sie beschädigt werden. Mehr als vier Fenster gehen nicht. „Beenden“ schließt alle Fenster.": "Both windows use the same modpack folder. Do not open the same world twice, or it can be damaged. At most four windows. “Quit game” closes all windows." });
PATTERNS.push([/^Weiteres Fenster \((\d+) offen\)$/, (n) => "Another window (" + n + " open)"]);
Object.assign(EN, { "Installation abgebrochen": "Installation cancelled" });
Object.assign(EN, { "Fehlt": "Missing", "Lade die Datei herunter und lege sie in den Ordner oder hierher.": "Download the file and put it in the folder, or drop it here.", "Auf CurseForge öffnen": "Open on CurseForge", "Ordner öffnen": "Open folder", "Ausblenden": "Hide", "Nicht mehr als fehlend anzeigen": "Stop showing as missing", "fehlen": "missing" });
Object.assign(EN, { "Beta": "Beta", "Der Launcher ist noch in der Beta. Fehler bitte melden.": "The launcher is still in beta. Please report bugs." });
Object.assign(EN, {
  "Benötigte Mods installieren?": "Install required mods?", "Nur diesen Mod": "Only this mod",
  "Mit Abhängigkeit installieren": "Install with dependency", "Mit Abhängigkeiten installieren": "Install with dependencies",
  "Abhängigkeiten": "Dependencies", "Dieser Mod braucht keine weiteren Mods.": "This mod needs no other mods.",
  "Lizenz": "License", "Aktualisiert": "Updated", "Modpack-Ordner": "Modpack folder",
  "Öffnet den Ordner dieses Modpacks im Dateimanager, zum Beispiel um Mods von Hand hineinzulegen.": "Opens this modpack's folder in the file manager, for example to add mods by hand.",
  "In diesem Modpack suchen …": "Search this modpack …",
  "Die Landschaft folgt dem Datum. Automatisch ist Blau, zu Halloween, Weihnachten und Silvester mit passenden Farben.": "The scenery follows the date. Automatic is blue, with matching colours for Halloween, Christmas and New Year's Eve.",
});
PATTERNS.unshift(
  [/^Dieser Mod braucht einen weiteren Mod, damit er läuft:$/, () => "This mod needs one other mod to run:"],
  [/^Dieser Mod braucht (\d+) weitere Mods, damit er läuft:$/, (n) => `This mod needs ${n} other mods to run:`],
  [/^(\d+) Abhängigkeit(en)?( · (\d+) installiert)?$/, (n, pl, _, k) => `${n} ${pl ? "dependencies" : "dependency"}${k ? ` · ${k} installed` : ""}`],
  [/^Zeigt nur Mods für Minecraft (.+) mit (.+)\. Beim Installieren fragen wir, ob benötigte Mods mitkommen sollen\.$/, (a, b) => `Only shows mods for Minecraft ${a} with ${b}. When installing, we ask whether required mods should come along.`],
  [/^(\d+) Einträge$/, (n) => `${n} items`],
  [/^Gruppe für 1 Eintrag$/, () => "Group for 1 item"],
  [/^Neue Version (.+)$/, (v) => `New version ${v}`],
);
