//! geffeniuse core: everything the launcher does without any UI.

pub mod auth;
pub mod content;
pub mod crash;
pub mod curseforge;
pub mod details;
pub mod download;
pub mod groups;
pub mod instance;
pub mod java;
pub mod launch;
pub mod loaders;
pub mod modrinth;
pub mod mods;
pub mod mojang;
pub mod packs;
pub mod recent;
pub mod paths;
pub mod running;
pub mod screenshots;
pub mod settings;
pub mod skin;
pub mod sync;
pub mod transfer;
pub mod update;
pub mod worlds;

pub const APP_NAME: &str = "geffeniuse";
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// One shared HTTP client. Identifies itself as Modrinth requires; sends nothing else.
pub fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(format!("deldown/{APP_NAME}/{APP_VERSION}"))
        .https_only(true)
        .connect_timeout(std::time::Duration::from_secs(15))
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .expect("http client")
}

#[cfg(test)]
mod version_tests {
    #[test]
    fn every_place_that_names_the_version_agrees() {
        let window = include_str!("../../../src-tauri/Cargo.toml");
        let config: serde_json::Value = serde_json::from_str(include_str!("../../../src-tauri/tauri.conf.json")).unwrap();
        assert!(window.contains(&format!("\nversion = \"{}\"", super::APP_VERSION)), "src-tauri/Cargo.toml");
        assert_eq!(config["version"], super::APP_VERSION, "tauri.conf.json");
    }
}
