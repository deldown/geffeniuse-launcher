use crate::paths::{valid_id, Dirs};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Loader {
    Vanilla,
    Fabric,
    Quilt,
    Forge,
    Neoforge,
}

impl Loader {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "vanilla" => Self::Vanilla,
            "fabric" => Self::Fabric,
            "quilt" => Self::Quilt,
            "forge" => Self::Forge,
            "neoforge" => Self::Neoforge,
            _ => return None,
        })
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Instance {
    pub id: String,
    pub name: String,
    pub mc_version: String,
    pub loader: Loader,
    pub loader_version: Option<String>,
    pub memory_mb: Option<u32>,
    pub java_path: Option<String>,
    pub jvm_args: Option<String>,
    pub created: u64,
    /// Where the pack came from, e.g. "modrinth:<project>" or "curseforge:<id>".
    pub source: Option<String>,
    #[serde(default)]
    pub last_played: Option<u64>,
    /// File name of a custom icon inside the instance directory; `None` shows the Geffeniuse logo.
    #[serde(default)]
    pub icon: Option<String>,
    /// Total time the game has run for this instance, in seconds.
    #[serde(default)]
    pub playtime_secs: u64,
    /// Share FOV, controls and the like with the other modpacks (see `sync`).
    #[serde(default = "yes")]
    pub sync_settings: bool,
}

fn yes() -> bool {
    true
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn slug(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect();
    let s = s.trim_matches('-').to_string();
    if s.is_empty() { "instance".into() } else { s.chars().take(40).collect() }
}

fn clean_name(name: &str) -> Result<&str> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 60 || name.chars().any(char::is_control) {
        bail!("invalid instance name");
    }
    Ok(name)
}

/// RAM for this modpack in MB; `None` goes back to the launcher-wide default.
pub async fn set_memory(dirs: &Dirs, id: &str, mb: Option<u32>) -> Result<()> {
    if mb.is_some_and(|m| !(512..=262_144).contains(&m)) {
        anyhow::bail!("memory must be between 512 MB and 256 GB");
    }
    let mut inst = get(dirs, id).await?;
    inst.memory_mb = mb;
    save(dirs, &inst).await
}

pub async fn set_sync(dirs: &Dirs, id: &str, on: bool) -> Result<()> {
    let mut inst = get(dirs, id).await?;
    inst.sync_settings = on;
    save(dirs, &inst).await
}

pub async fn rename(dirs: &Dirs, id: &str, name: &str) -> Result<()> {
    let name = clean_name(name)?.to_string();
    let mut inst = get(dirs, id).await?;
    inst.name = name;
    save(dirs, &inst).await
}

pub async fn add_playtime(dirs: &Dirs, id: &str, secs: u64) -> Result<()> {
    if secs == 0 {
        return Ok(());
    }
    let mut inst = get(dirs, id).await?;
    inst.playtime_secs = inst.playtime_secs.saturating_add(secs);
    save(dirs, &inst).await
}

pub async fn list(dirs: &Dirs) -> Result<Vec<Instance>> {
    let mut out = Vec::new();
    let mut rd = match tokio::fs::read_dir(dirs.instances()).await {
        Ok(r) => r,
        Err(_) => return Ok(out),
    };
    while let Some(e) = rd.next_entry().await? {
        let f = e.path().join("instance.json");
        if let Ok(b) = tokio::fs::read(&f).await {
            if let Ok(i) = serde_json::from_slice::<Instance>(&b) {
                if valid_id(&i.id) {
                    out.push(i);
                }
            }
        }
    }
    out.sort_by_key(|x| std::cmp::Reverse(x.created));
    Ok(out)
}

pub async fn get(dirs: &Dirs, id: &str) -> Result<Instance> {
    if !valid_id(id) {
        bail!("bad instance id");
    }
    let b = tokio::fs::read(dirs.instance(id).join("instance.json")).await.context("instance not found")?;
    Ok(serde_json::from_slice(&b)?)
}

pub async fn create(
    dirs: &Dirs,
    name: &str,
    mc_version: &str,
    loader: Loader,
    loader_version: Option<String>,
    source: Option<String>,
) -> Result<Instance> {
    let name = clean_name(name)?;
    if !crate::paths::file_name_ok(mc_version) {
        bail!("invalid Minecraft version");
    }
    let base = slug(name);
    let mut id = base.clone();
    let mut n = 1;
    while dirs.instance(&id).exists() {
        n += 1;
        id = format!("{base}-{n}");
    }
    let inst = Instance {
        id: id.clone(),
        name: name.into(),
        mc_version: mc_version.into(),
        loader,
        loader_version,
        memory_mb: None,
        java_path: None,
        jvm_args: None,
        created: now(),
        source,
        last_played: None,
        icon: None,
        playtime_secs: 0,
        sync_settings: true,
    };
    tokio::fs::create_dir_all(dirs.instance(&id).join("minecraft")).await?;
    save(dirs, &inst).await?;
    Ok(inst)
}

pub async fn save(dirs: &Dirs, inst: &Instance) -> Result<()> {
    if !valid_id(&inst.id) {
        bail!("bad instance id");
    }
    let dir = dirs.instance(&inst.id);
    tokio::fs::create_dir_all(&dir).await?;
    let tmp = dir.join("instance.json.tmp");
    tokio::fs::write(&tmp, serde_json::to_vec_pretty(inst)?).await?;
    tokio::fs::rename(tmp, dir.join("instance.json")).await?;
    Ok(())
}

const MAX_ICON_BYTES: usize = 3 * 1024 * 1024;

fn image_ext(b: &[u8]) -> Option<&'static str> {
    if b.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("png")
    } else if b.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("jpg")
    } else if b.len() > 12 && &b[..4] == b"RIFF" && &b[8..12] == b"WEBP" {
        Some("webp")
    } else {
        None
    }
}

/// `icon-<digits>.<png|jpg|webp>`: the only names the icon protocol will serve.
pub fn icon_name_ok(name: &str) -> bool {
    name.strip_prefix("icon-")
        .and_then(|r| r.split_once('.'))
        .is_some_and(|(n, e)| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) && matches!(e, "png" | "jpg" | "webp"))
}

pub fn icon_path(dirs: &Dirs, id: &str, name: &str) -> Result<std::path::PathBuf> {
    if !valid_id(id) || !icon_name_ok(name) {
        bail!("bad icon reference");
    }
    Ok(dirs.instance(id).join(name))
}

pub async fn set_icon_bytes(dirs: &Dirs, id: &str, bytes: &[u8]) -> Result<()> {
    if bytes.len() > MAX_ICON_BYTES {
        bail!("icon is too large (max 3 MB)");
    }
    let ext = image_ext(bytes).context("icon must be a PNG, JPEG or WebP image")?;
    let mut inst = get(dirs, id).await?;
    let name = format!("icon-{}.{ext}", now());
    tokio::fs::write(dirs.instance(id).join(&name), bytes).await?;
    if let Some(old) = inst.icon.replace(name) {
        if icon_name_ok(&old) {
            let _ = tokio::fs::remove_file(dirs.instance(id).join(old)).await;
        }
    }
    save(dirs, &inst).await
}

pub async fn set_icon(dirs: &Dirs, id: &str, src: &std::path::Path) -> Result<()> {
    let meta = tokio::fs::metadata(src).await?;
    if !meta.is_file() || meta.len() as usize > MAX_ICON_BYTES {
        bail!("icon must be a file of at most 3 MB");
    }
    set_icon_bytes(dirs, id, &tokio::fs::read(src).await?).await
}

pub async fn clear_icon(dirs: &Dirs, id: &str) -> Result<()> {
    let mut inst = get(dirs, id).await?;
    if let Some(old) = inst.icon.take() {
        if icon_name_ok(&old) {
            let _ = tokio::fs::remove_file(dirs.instance(id).join(old)).await;
        }
    }
    save(dirs, &inst).await
}

pub async fn touch(dirs: &Dirs, id: &str) -> Result<()> {
    let mut inst = get(dirs, id).await?;
    inst.last_played = Some(now());
    save(dirs, &inst).await
}

pub async fn delete(dirs: &Dirs, id: &str) -> Result<()> {
    if !valid_id(id) {
        bail!("bad instance id");
    }
    let dir = dirs.instance(id);
    if !dir.join("instance.json").exists() {
        bail!("not an instance directory");
    }
    tokio::fs::remove_dir_all(dir).await?;
    Ok(())
}

/// The directory Minecraft runs in (`.minecraft` equivalent).
pub fn game_dir(dirs: &Dirs, id: &str) -> std::path::PathBuf {
    dirs.instance(id).join("minecraft")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs() {
        assert_eq!(slug("My Pack!"), "my-pack");
        assert_eq!(slug("../.."), "instance");
        assert_eq!(slug("Ä"), "instance");
    }

    #[tokio::test]
    async fn icon_set_replace_clear() {
        let tmp = std::env::temp_dir().join(format!("gn-icon-{}", uuid::Uuid::new_v4()));
        let dirs = Dirs::new(&tmp);
        let inst = create(&dirs, "I", "1.21.1", Loader::Fabric, None, None).await.unwrap();
        let png = b"\x89PNG\r\n\x1a\nrest".to_vec();
        set_icon_bytes(&dirs, &inst.id, &png).await.unwrap();
        let first = get(&dirs, &inst.id).await.unwrap().icon.unwrap();
        assert!(icon_name_ok(&first) && dirs.instance(&inst.id).join(&first).is_file());
        assert!(set_icon_bytes(&dirs, &inst.id, b"GIF89a nope").await.is_err());
        assert!(set_icon_bytes(&dirs, &inst.id, &vec![0xFF; 4 * 1024 * 1024]).await.is_err());
        clear_icon(&dirs, &inst.id).await.unwrap();
        assert!(get(&dirs, &inst.id).await.unwrap().icon.is_none());
        assert!(!dirs.instance(&inst.id).join(&first).exists());
        let _ = std::fs::remove_dir_all(tmp);
    }

    #[test]
    fn icon_names() {
        assert!(icon_name_ok("icon-1793000000.png"));
        assert!(icon_name_ok("icon-5.webp"));
        for bad in ["icon-.png", "icon-1.gif", "../icon-1.png", "icon-1.png/x", "instance.json", "icon-a1.png"] {
            assert!(!icon_name_ok(bad), "{bad}");
        }
        let dirs = Dirs::new(std::env::temp_dir());
        assert!(icon_path(&dirs, "../x", "icon-1.png").is_err());
    }

    #[tokio::test]
    async fn touch_records_last_played() {
        let tmp = std::env::temp_dir().join(format!("gn-touch-{}", uuid::Uuid::new_v4()));
        let dirs = Dirs::new(&tmp);
        let inst = create(&dirs, "T", "1.21.1", Loader::Fabric, None, None).await.unwrap();
        assert_eq!(get(&dirs, &inst.id).await.unwrap().last_played, None);
        touch(&dirs, &inst.id).await.unwrap();
        assert!(get(&dirs, &inst.id).await.unwrap().last_played.is_some());
        assert!(touch(&dirs, "../x").await.is_err());
        let _ = std::fs::remove_dir_all(tmp);
    }

    #[tokio::test]
    async fn memory_is_validated_and_can_be_reset() {
        let root = std::env::temp_dir().join(format!("gn-mem-{}", uuid::Uuid::new_v4()));
        let dirs = Dirs::new(&root);
        let i = create(&dirs, "M", "1.21.1", Loader::Fabric, None, None).await.unwrap();
        set_memory(&dirs, &i.id, Some(6144)).await.unwrap();
        assert_eq!(get(&dirs, &i.id).await.unwrap().memory_mb, Some(6144));
        assert!(set_memory(&dirs, &i.id, Some(64)).await.is_err() && set_memory(&dirs, &i.id, Some(9_999_999)).await.is_err());
        set_memory(&dirs, &i.id, None).await.unwrap();
        assert_eq!(get(&dirs, &i.id).await.unwrap().memory_mb, None);
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn rename_and_playtime() {
        let tmp = std::env::temp_dir().join(format!("gn-ren-{}", uuid::Uuid::new_v4()));
        let dirs = Dirs::new(&tmp);
        let inst = create(&dirs, "Old name", "1.21.1", Loader::Fabric, None, None).await.unwrap();
        rename(&dirs, &inst.id, "  New name  ").await.unwrap();
        let got = get(&dirs, &inst.id).await.unwrap();
        assert_eq!((got.name.as_str(), got.id.as_str()), ("New name", inst.id.as_str()), "folder id must not change");
        for bad in ["", "   ", "a\nb", &"x".repeat(61)] {
            assert!(rename(&dirs, &inst.id, bad).await.is_err(), "{bad:?}");
        }
        assert!(rename(&dirs, "../x", "ok").await.is_err());
        add_playtime(&dirs, &inst.id, 60).await.unwrap();
        add_playtime(&dirs, &inst.id, 0).await.unwrap();
        add_playtime(&dirs, &inst.id, 95).await.unwrap();
        assert_eq!(get(&dirs, &inst.id).await.unwrap().playtime_secs, 155);
        let _ = std::fs::remove_dir_all(tmp);
    }

    #[test]
    fn old_instance_json_without_last_played_still_loads() {
        let j = r#"{"id":"a","name":"A","mc_version":"1.21.1","loader":"fabric","loader_version":null,"memory_mb":null,"java_path":null,"jvm_args":null,"created":1,"source":null}"#;
        let i = serde_json::from_str::<Instance>(j).unwrap();
        assert!(i.last_played.is_none() && i.playtime_secs == 0);
    }

    #[tokio::test]
    async fn create_list_delete() {
        let tmp = std::env::temp_dir().join(format!("gn-test-{}", uuid::Uuid::new_v4()));
        let dirs = Dirs::new(&tmp);
        let a = create(&dirs, "Test Pack", "1.20.1", Loader::Fabric, None, None).await.unwrap();
        let b = create(&dirs, "Test Pack", "1.20.1", Loader::Vanilla, None, None).await.unwrap();
        assert_ne!(a.id, b.id);
        assert_eq!(list(&dirs).await.unwrap().len(), 2);
        assert!(create(&dirs, "  ", "1.20.1", Loader::Vanilla, None, None).await.is_err());
        assert!(create(&dirs, "x", "../evil", Loader::Vanilla, None, None).await.is_err());
        delete(&dirs, &a.id).await.unwrap();
        assert!(delete(&dirs, "../x").await.is_err());
        assert_eq!(list(&dirs).await.unwrap().len(), 1);
        let _ = std::fs::remove_dir_all(tmp);
    }
}
