use crate::instance::Loader;
use crate::paths::Dirs;
use anyhow::{bail, Context, Result};
use serde::Deserialize;

fn meta_base(l: Loader) -> Result<&'static str> {
    Ok(match l {
        Loader::Fabric => "https://meta.fabricmc.net/v2/versions/loader",
        Loader::Quilt => "https://meta.quiltmc.org/v3/versions/loader",
        _ => bail!("not a profile-json loader"),
    })
}

const FORGE_PROMOS: &str = "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json";
const NEOFORGE_VERSIONS: &str = "https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge";

/// NeoForge versions are `<mc minor>.<mc patch>.<build>`, e.g. MC 1.21.1 -> `21.1.*`.
fn neoforge_prefix(mc: &str) -> Option<String> {
    let rest = mc.strip_prefix("1.")?;
    let mut it = rest.split('.');
    let minor = it.next()?;
    let patch = it.next().unwrap_or("0");
    if !minor.bytes().all(|b| b.is_ascii_digit()) || !patch.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some(format!("{minor}.{patch}."))
}

fn pick_neoforge(versions: &[String], mc: &str) -> Option<String> {
    let prefix = neoforge_prefix(mc)?;
    let matching: Vec<&String> = versions.iter().filter(|v| v.starts_with(&prefix)).collect();
    matching.iter().rev().find(|v| !v.contains('-')).or(matching.last()).map(|v| (*v).clone())
}

/// Version id the installer leaves in `versions/`.
pub fn forge_like_id(l: Loader, mc: &str, ver: &str) -> String {
    match l {
        Loader::Neoforge => format!("neoforge-{ver}"),
        _ => format!("{mc}-forge-{ver}"),
    }
}

fn installer_url(l: Loader, mc: &str, ver: &str) -> String {
    match l {
        Loader::Neoforge => format!("https://maven.neoforged.net/releases/net/neoforged/neoforge/{ver}/neoforge-{ver}-installer.jar"),
        _ => format!("https://maven.minecraftforge.net/net/minecraftforge/forge/{mc}-{ver}/forge-{mc}-{ver}-installer.jar"),
    }
}

#[derive(Deserialize)]
struct Entry {
    loader: LoaderVer,
}

#[derive(Deserialize)]
struct LoaderVer {
    version: String,
    #[serde(default)]
    stable: bool,
}

pub async fn latest_loader(client: &reqwest::Client, l: Loader, mc: &str) -> Result<String> {
    if !crate::paths::file_name_ok(mc) {
        bail!("bad mc version");
    }
    match l {
        Loader::Forge => {
            let v: serde_json::Value = client.get(FORGE_PROMOS).send().await?.error_for_status()?.json().await?;
            let promos = &v["promos"];
            return ["recommended", "latest"]
                .iter()
                .find_map(|k| promos[format!("{mc}-{k}")].as_str())
                .map(String::from)
                .context("no Forge version for this Minecraft version");
        }
        Loader::Neoforge => {
            let v: serde_json::Value = client.get(NEOFORGE_VERSIONS).send().await?.error_for_status()?.json().await?;
            let all: Vec<String> = v["versions"].as_array().context("bad NeoForge version list")?.iter().filter_map(|x| x.as_str().map(String::from)).collect();
            return pick_neoforge(&all, mc).context("no NeoForge version for this Minecraft version");
        }
        _ => {}
    }
    let url = format!("{}/{}", meta_base(l)?, mc);
    let list: Vec<Entry> = client.get(url).send().await?.error_for_status()?.json().await?;
    list.iter()
        .find(|e| e.loader.stable)
        .or(list.first())
        .map(|e| e.loader.version.clone())
        .context("no loader version for this Minecraft version")
}

/// Writes the loader's profile json into versions/ and returns its version id.
/// The version id Fabric's and Quilt's meta servers give a profile, e.g. `fabric-loader-0.16.9-1.21.1`.
fn profile_id(l: Loader, mc: &str, loader_ver: &str) -> String {
    format!("{}-loader-{loader_ver}-{mc}", if l == Loader::Quilt { "quilt" } else { "fabric" })
}

pub async fn install_profile(dirs: &Dirs, client: &reqwest::Client, l: Loader, mc: &str, loader_ver: &str) -> Result<String> {
    for s in [mc, loader_ver] {
        if !crate::paths::file_name_ok(s) {
            bail!("bad version string");
        }
    }
    let base = meta_base(l)?;
    // A loader profile for a fixed loader and game version never changes, so the copy on disk is enough.
    let known = profile_id(l, mc, loader_ver);
    if dirs.versions().join(&known).join(format!("{known}.json")).is_file() {
        return Ok(known);
    }
    let url = format!("{base}/{mc}/{loader_ver}/profile/json");
    let text = client.get(&url).send().await?.error_for_status()?.text().await?;
    let v: serde_json::Value = serde_json::from_str(&text)?;
    let id = v["id"].as_str().context("profile without id")?.to_string();
    if !crate::paths::file_name_ok(&id) {
        bail!("bad profile id");
    }
    let dest = dirs.versions().join(&id).join(format!("{id}.json"));
    tokio::fs::create_dir_all(dest.parent().unwrap()).await?;
    tokio::fs::write(&dest, text).await?;
    Ok(id)
}

/// Runs the official Forge/NeoForge installer (downloaded from the loader's own Maven over
/// HTTPS, verified against the published SHA-1) and returns the version id it created.
/// The installer is third-party code, so it only ever runs with the user's own Java.
pub async fn install_forge_like(dirs: &Dirs, client: &reqwest::Client, l: Loader, mc: &str, ver: &str, java: &std::path::Path) -> Result<String> {
    for s in [mc, ver] {
        if !crate::paths::file_name_ok(s) || s.contains(['/', '\\']) {
            bail!("bad version string");
        }
    }
    let id = forge_like_id(l, mc, ver);
    if dirs.versions().join(&id).join(format!("{id}.json")).exists() {
        return Ok(id);
    }
    let url = installer_url(l, mc, ver);
    let jar = dirs.cache().join(format!("installer-{id}.jar"));
    let sha1 = client.get(format!("{url}.sha1")).send().await?.error_for_status()?.text().await?;
    let sha1 = sha1.split_whitespace().next().unwrap_or("").to_string();
    if sha1.len() != 40 || !sha1.bytes().all(|b| b.is_ascii_hexdigit()) {
        bail!("installer checksum missing");
    }
    let job = crate::download::Job { url, dest: jar.clone(), sha1: Some(sha1), size: None };
    crate::download::download_one(client, &job).await?;
    // The installer insists on a launcher profile file next to the game data.
    let profiles = dirs.root.join("launcher_profiles.json");
    if !profiles.exists() {
        tokio::fs::write(&profiles, br#"{"profiles":{}}"#).await?;
    }
    let mut installer = tokio::process::Command::new(java);
    installer.arg("-jar").arg(&jar).arg("--installClient").arg(&dirs.root).current_dir(&dirs.root);
    crate::paths::hide_console(&mut installer);
    let out = tokio::time::timeout(std::time::Duration::from_secs(900), installer.output())
    .await
    .context("loader installer timed out")?
    .context("failed to run loader installer")?;
    if !out.status.success() || !dirs.versions().join(&id).join(format!("{id}.json")).exists() {
        let log = String::from_utf8_lossy(&out.stdout);
        let tail: Vec<&str> = log.lines().rev().take(6).collect();
        bail!("loader installer failed: {}", tail.into_iter().rev().collect::<Vec<_>>().join(" | "));
    }
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_ids_match_the_meta_servers() {
        assert_eq!(profile_id(Loader::Fabric, "1.21.1", "0.16.9"), "fabric-loader-0.16.9-1.21.1");
        assert_eq!(profile_id(Loader::Quilt, "1.20.1", "0.27.1"), "quilt-loader-0.27.1-1.20.1");
    }

    #[tokio::test]
    async fn a_cached_profile_is_used_without_any_request() {
        let root = std::env::temp_dir().join(format!("gn-profile-{}", uuid::Uuid::new_v4()));
        let dirs = Dirs::new(&root);
        let id = profile_id(Loader::Fabric, "1.21.1", "0.16.9");
        std::fs::create_dir_all(dirs.versions().join(&id)).unwrap();
        std::fs::write(dirs.versions().join(&id).join(format!("{id}.json")), "{}").unwrap();
        // A client without TLS roots cannot reach the meta server, so success means nothing was fetched.
        let offline = reqwest::Client::builder().tls_built_in_root_certs(false).build().unwrap();
        assert_eq!(install_profile(&dirs, &offline, Loader::Fabric, "1.21.1", "0.16.9").await.unwrap(), id);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn neoforge_pick() {
        let v: Vec<String> = ["20.4.1", "21.0.5", "21.1.1", "21.1.9-beta", "21.1.172", "21.1.200-beta"].iter().map(std::string::ToString::to_string).collect();
        assert_eq!(pick_neoforge(&v, "1.21.1").as_deref(), Some("21.1.172"));
        assert_eq!(pick_neoforge(&v, "1.21").as_deref(), Some("21.0.5"));
        assert_eq!(pick_neoforge(&v, "1.19.4"), None);
        assert_eq!(neoforge_prefix("1.20.4").as_deref(), Some("20.4."));
        assert_eq!(neoforge_prefix("1.x"), None);
    }

    #[test]
    fn ids_and_urls() {
        assert_eq!(forge_like_id(Loader::Forge, "1.20.1", "47.3.0"), "1.20.1-forge-47.3.0");
        assert_eq!(forge_like_id(Loader::Neoforge, "1.21.1", "21.1.172"), "neoforge-21.1.172");
        assert_eq!(
            installer_url(Loader::Forge, "1.20.1", "47.3.0"),
            "https://maven.minecraftforge.net/net/minecraftforge/forge/1.20.1-47.3.0/forge-1.20.1-47.3.0-installer.jar"
        );
    }

    #[tokio::test]
    #[ignore = "downloads Java, the Forge installer and its libraries"]
    async fn live_forge_install() {
        let root = std::env::temp_dir().join(format!("geff-forge-{}", std::process::id()));
        let dirs = Dirs::new(&root);
        dirs.ensure().await.unwrap();
        let client = reqwest::Client::new();
        let java = crate::java::install_runtime(&client, &dirs.runtimes(), "java-runtime-gamma", |_| {}).await.unwrap();
        let id = install_forge_like(&dirs, &client, Loader::Forge, "1.20.1", "47.3.0", &java).await.unwrap();
        assert_eq!(id, "1.20.1-forge-47.3.0");
        assert!(dirs.versions().join(&id).join(format!("{id}.json")).exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    #[ignore = "downloads Java, the NeoForge installer and its libraries"]
    async fn live_neoforge_install() {
        let root = std::env::temp_dir().join(format!("geff-neo-{}", std::process::id()));
        let dirs = Dirs::new(&root);
        dirs.ensure().await.unwrap();
        let client = reqwest::Client::new();
        let latest = latest_loader(&client, Loader::Neoforge, "1.21.1").await.unwrap();
        let java = crate::java::install_runtime(&client, &dirs.runtimes(), "java-runtime-delta", |_| {}).await.unwrap();
        let id = install_forge_like(&dirs, &client, Loader::Neoforge, "1.21.1", &latest, &java).await.unwrap();
        assert_eq!(id, format!("neoforge-{latest}"));
        let _ = std::fs::remove_dir_all(&root);
    }
}
