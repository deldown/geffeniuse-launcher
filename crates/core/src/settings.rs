use crate::paths::Dirs;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const SEASONS: [&str; 10] = ["auto", "off", "halloween", "christmas", "newyear", "spring", "summer", "autumn", "winter", "none"];

/// Everything the user can switch on or off. Defaults are the clean, minimal launcher.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Settings {
    /// UI modules shown in the sidebar: discover, screenshots, console, ...
    pub modules: BTreeMap<String, bool>,
    /// Accent colour: "auto" (blue, or the event colour during an event), "blue", "green", "red", "gold", "purple".
    pub color: String,
    /// Seasonal scenery: "auto" follows the calendar, "off" keeps the plain scenes, or a fixed theme name.
    pub season: String,
    /// UI language: "de" or "en".
    pub language: String,
    /// The first-start setup has been completed or skipped.
    pub setup_done: bool,
    /// Landscape pictures in the banners; off gives plain banners.
    pub scene: bool,
    /// Seasonal and event looks (Halloween, Christmas, New Year, seasons); off keeps the plain day and night scenes.
    pub seasonal: bool,
    /// Moving effects such as snow, fireworks and lightning.
    pub effects: bool,
    /// Share Minecraft settings (FOV, controls, ...) between modpacks.
    pub sync_options: bool,
    /// The "check for updates" button for mods in a modpack.
    pub mod_updates: bool,
    /// After a mod update check, show only mods with an update.
    pub mod_updates_autofilter: bool,
    /// Allow a second game window of a modpack that is already running.
    pub multi_start: bool,
    /// Explain a crash in plain words when the game ends badly.
    pub crash_help: bool,
    /// Groups for mods, packs and shaders.
    pub content_groups: bool,
    /// Show groups as folders that open and close; off lists everything flat.
    pub groups_as_folders: bool,
    /// The turnable 3D skin on the start page.
    pub skin_figure: bool,
    /// Play time shown on instance cards and pages.
    pub show_playtime: bool,
    pub pixel_scale: u8,
    pub animations: bool,
    pub compact_lists: bool,
    /// Older switch for closing the launcher after the start; now `after_start`.
    pub close_on_launch: bool,
    /// What the launcher does once the game is running: "keep" it open, "minimize" it, or "close" it.
    pub after_start: String,
    /// Open the game's console in its own window after the start.
    pub console_window: bool,
    /// How many recent worlds and servers the start page shows (0 hides the list).
    pub home_recent: u8,
    /// Show the modpack cards on the start page.
    pub home_modpacks: bool,
    /// Show the quick actions (new modpack, discover, import) on the start page.
    pub home_actions: bool,
    /// A lower banner on the start page.
    pub home_compact: bool,
    /// Offer snapshots, pre-releases and release candidates when creating a modpack.
    pub show_snapshots: bool,
    /// Offer experimental snapshots (combat tests, experimental features).
    pub show_experimental: bool,
    /// Offer the April Fools' versions.
    pub show_april_fools: bool,
    /// Load the picture for each Minecraft version from Mojang's public feed (and show the bundled ones); off draws plain scenes and makes no request.
    pub version_pictures: bool,
    /// Let CurseForge requests without a key of your own go through deldown's proxy.
    pub curseforge_proxy: bool,
    /// The new-modpack dialog: "fancy" (pictures per Minecraft version) or "simple" (plain lists).
    pub version_picker: String,
    pub default_memory_mb: u32,
    pub java_path: Option<String>,
    pub extra_jvm_args: String,
    pub parallel_downloads: u8,
    pub curseforge_enabled: bool,
    pub modrinth_enabled: bool,
    /// CurseForge API key (user supplied until a key proxy exists).
    pub curseforge_key: String,
    /// Microsoft app (client) id. Placeholder default; swap for the project's own id.
    pub ms_client_id: String,
    pub active_account: Option<String>,
    /// Look for a newer launcher version a few seconds after start.
    pub auto_update_check: bool,
    pub discord_presence: bool,
    pub discord_application_id: String,
}

impl Default for Settings {
    fn default() -> Self {
        let modules = [
            ("discover", true),
            ("screenshots", true),
            ("console", true),
            ("worlds", true),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
        Self {
            modules,
            color: "auto".into(),
            season: "auto".into(),
            language: "de".into(),
            setup_done: false,
            scene: true,
            seasonal: true,
            effects: true,
            sync_options: true,
            mod_updates: true,
            mod_updates_autofilter: true,
            multi_start: true,
            crash_help: true,
            content_groups: true,
            groups_as_folders: true,
            skin_figure: true,
            show_playtime: true,
            pixel_scale: 2,
            animations: true,
            compact_lists: false,
            close_on_launch: false,
            after_start: "keep".into(),
            console_window: false,
            home_recent: 3,
            home_modpacks: true,
            home_actions: false,
            home_compact: false,
            show_snapshots: false,
            show_experimental: false,
            show_april_fools: false,
            version_pictures: true,
            curseforge_proxy: true,
            version_picker: "fancy".into(),
            default_memory_mb: 4096,
            java_path: None,
            extra_jvm_args: String::new(),
            parallel_downloads: 16,
            curseforge_enabled: true,
            modrinth_enabled: true,
            curseforge_key: String::new(),
            ms_client_id: "00000000402b5328".into(),
            active_account: None,
            auto_update_check: true,
            discord_presence: false,
            discord_application_id: option_env!("GEFFENIUSE_DISCORD_APPLICATION_ID").unwrap_or("").into(),
        }
    }
}

impl Settings {
    pub async fn load(dirs: &Dirs) -> Self {
        match tokio::fs::read(dirs.settings_file()).await {
            Ok(b) => serde_json::from_slice::<Self>(&b).unwrap_or_default().sanitized(),
            Err(_) => Self::default(),
        }
    }

    /// Clamp values from the UI to sane ranges before they are stored.
    pub fn sanitized(mut self) -> Self {
        self.discord_application_id = self.discord_application_id.trim().to_owned();
        if !valid_discord_application_id(&self.discord_application_id) {
            self.discord_application_id.clear();
            self.discord_presence = false;
        }
        self.pixel_scale = self.pixel_scale.clamp(1, 4);
        self.default_memory_mb = self.default_memory_mb.clamp(512, 65536);
        self.parallel_downloads = self.parallel_downloads.clamp(1, 32);
        self.extra_jvm_args.truncate(2000);
        self.curseforge_key.truncate(200);
        self.ms_client_id.retain(|c| c.is_ascii_alphanumeric() || c == '-');
        self.ms_client_id.truncate(64);
        if !matches!(self.color.as_str(), "auto" | "blue" | "green" | "red" | "gold" | "purple") {
            self.color = "auto".into();
        }
        if !SEASONS.contains(&self.season.as_str()) {
            self.season = "auto".into();
        }
        if !matches!(self.language.as_str(), "de" | "en") {
            self.language = "de".into();
        }
        if self.close_on_launch && self.after_start == "keep" {
            self.after_start = "close".into();
        }
        self.close_on_launch = false;
        if !matches!(self.after_start.as_str(), "keep" | "minimize" | "close") {
            self.after_start = "keep".into();
        }
        self.home_recent = self.home_recent.min(6);
        if !matches!(self.version_picker.as_str(), "fancy" | "simple") {
            self.version_picker = "fancy".into();
        }
        self.modules.retain(|k, _| k.len() <= 32);
        self
    }

    /// CurseForge can be used with a key of your own, or through the proxy if that is allowed.
    pub fn curseforge_usable(&self) -> bool {
        self.curseforge_enabled && (!self.curseforge_key.trim().is_empty() || self.curseforge_proxy)
    }

    pub async fn save(&self, dirs: &Dirs) -> anyhow::Result<()> {
        tokio::fs::create_dir_all(&dirs.root).await?;
        let tmp = dirs.root.join("settings.json.tmp");
        tokio::fs::write(&tmp, serde_json::to_vec_pretty(self)?).await?;
        tokio::fs::rename(tmp, dirs.settings_file()).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn theme_settings_default_and_reject_nonsense() {
        let d = Settings::default();
        assert_eq!((d.color.as_str(), d.season.as_str(), d.language.as_str(), d.setup_done), ("auto", "auto", "de", false));
        let old: Settings = serde_json::from_str(r#"{"accent":"grass","halloween":"on"}"#).unwrap();
        let old = old.sanitized();
        assert_eq!((old.color.as_str(), old.season.as_str()), ("auto", "auto"), "files from before the theme rework load with defaults");
        let bad = Settings { color: "<script>".into(), season: "x".into(), language: "fr".into(), ..Settings::default() }.sanitized();
        assert_eq!((bad.color.as_str(), bad.season.as_str(), bad.language.as_str()), ("auto", "auto", "de"));
        for c in ["blue", "green", "red", "gold", "purple"] {
            assert_eq!(Settings { color: c.into(), ..Settings::default() }.sanitized().color, c);
        }
        for s in SEASONS {
            assert_eq!(Settings { season: s.into(), ..Settings::default() }.sanitized().season, s);
        }
        assert_eq!(Settings { language: "en".into(), ..Settings::default() }.sanitized().language, "en");
    }

    use super::*;

    #[test]
    fn settings_from_before_the_offline_profile_was_removed_still_load() {
        let old = r#"{"color":"gold","offline_name":"Steve","active_account":null}"#;
        let s: Settings = serde_json::from_str(old).unwrap();
        assert_eq!(s.color, "gold");
    }
}

#[cfg(test)]
mod personalisation_tests {
    use super::*;

    #[test]
    fn personal_switches_default_to_on() {
        let s = Settings::default();
        assert!(s.scene && s.seasonal && s.effects && s.skin_figure && s.show_playtime);
    }

    #[test]
    fn older_settings_without_the_switches_keep_everything_on_and_honour_saved_ones() {
        let old: Settings = serde_json::from_str(r#"{"color":"gold"}"#).unwrap();
        assert!(old.scene && old.effects && old.skin_figure);
        let off: Settings = serde_json::from_str(r#"{"effects":false,"skin_figure":false}"#).unwrap();
        assert!(!off.effects && !off.skin_figure && off.scene);
    }
}

#[cfg(test)]
mod launcher_behaviour_tests {
    use super::*;

    #[test]
    fn defaults_keep_the_launcher_open_and_show_three_recent_entries() {
        let s = Settings::default();
        assert_eq!((s.after_start.as_str(), s.console_window, s.home_recent, s.home_modpacks, s.home_actions), ("keep", false, 3, true, false));
    }

    #[test]
    fn the_old_close_switch_becomes_the_new_choice_and_nonsense_is_repaired() {
        let old: Settings = serde_json::from_str(r#"{"close_on_launch":true}"#).unwrap();
        let old = old.sanitized();
        assert_eq!((old.after_start.as_str(), old.close_on_launch), ("close", false));
        let odd: Settings = serde_json::from_str(r#"{"after_start":"explode","home_recent":200}"#).unwrap();
        let odd = odd.sanitized();
        assert_eq!((odd.after_start.as_str(), odd.home_recent), ("keep", 6));
        let picker: Settings = serde_json::from_str(r#"{"version_picker":"wobbly"}"#).unwrap();
        assert_eq!(picker.sanitized().version_picker, "fancy");
        let mut off = Settings { curseforge_proxy: false, ..Settings::default() };
        assert!(!off.curseforge_usable());
        off.curseforge_key = "abc".into();
        assert!(off.curseforge_usable());
        assert!(Settings::default().curseforge_usable() && Settings::default().version_pictures);
        assert_eq!(Settings::default().version_picker, "fancy");
    }
}

pub fn valid_discord_application_id(id: &str) -> bool {
    (17..=20).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_digit())
        && id.parse::<u64>().is_ok_and(|n| n > 0)
}

#[cfg(test)]
mod discord_tests {
    use super::*;

    #[test]
    fn presence_is_opt_in_and_requires_a_valid_public_id() {
        let old: Settings = serde_json::from_str("{}").unwrap();
        assert!(!old.discord_presence);
        for id in ["", "123", "../discord", "00000000000000000", "99999999999999999999"] {
            assert!(!valid_discord_application_id(id));
            let settings = Settings { discord_presence: true, discord_application_id: id.into(), ..Settings::default() }.sanitized();
            assert!(!settings.discord_presence);
            assert!(settings.discord_application_id.is_empty());
        }
        let settings = Settings { discord_presence: true, discord_application_id: " 123456789012345678 ".into(), ..Settings::default() }.sanitized();
        assert!(settings.discord_presence);
        assert_eq!(settings.discord_application_id, "123456789012345678");
    }
}
