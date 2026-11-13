//! Groups the user puts mods, packs and shaders into (e.g. "Performance", "Kosmetik").
//! Kept in `groups.json` next to the modpack, keyed by file name without `.disabled`.
use crate::paths::{valid_id, Dirs};
use anyhow::{bail, Result};
use std::collections::BTreeMap;
use std::path::PathBuf;

const MAX_GROUP_LEN: usize = 40;
const MAX_FILES: usize = 5000;

pub type Groups = BTreeMap<String, String>;

fn path(dirs: &Dirs, id: &str) -> PathBuf {
    dirs.instance(id).join("groups.json")
}

pub fn base(file: &str) -> &str {
    file.strip_suffix(".disabled").unwrap_or(file)
}

pub async fn read(dirs: &Dirs, id: &str) -> Groups {
    match tokio::fs::read(path(dirs, id)).await {
        Ok(b) => serde_json::from_slice(&b).unwrap_or_default(),
        Err(_) => Groups::new(),
    }
}

async fn write(dirs: &Dirs, id: &str, groups: &Groups) -> Result<()> {
    let p = path(dirs, id);
    if groups.is_empty() {
        let _ = tokio::fs::remove_file(&p).await;
        return Ok(());
    }
    let tmp = p.with_extension("json.tmp");
    tokio::fs::write(&tmp, serde_json::to_vec(groups)?).await?;
    tokio::fs::rename(tmp, p).await?;
    Ok(())
}

fn clean(group: &str) -> Result<String> {
    let g = group.trim();
    if g.is_empty() || g.chars().count() > MAX_GROUP_LEN || g.chars().any(char::is_control) {
        bail!("invalid group name (1 to {MAX_GROUP_LEN} characters)");
    }
    Ok(g.to_string())
}

/// Puts the files into `group`, or takes them out of any group when `group` is `None`.
pub async fn set(dirs: &Dirs, id: &str, files: &[String], group: Option<&str>) -> Result<()> {
    if !valid_id(id) {
        bail!("bad instance id");
    }
    if files.len() > MAX_FILES || files.iter().any(|f| !crate::paths::file_name_ok(f)) {
        bail!("bad file reference");
    }
    let group = group.map(clean).transpose()?;
    let mut all = read(dirs, id).await;
    for f in files {
        match &group {
            Some(g) => all.insert(base(f).to_string(), g.clone()),
            None => all.remove(base(f)),
        };
    }
    write(dirs, id, &all).await
}

/// A file got a new name (an update): its group follows.
pub async fn rename(dirs: &Dirs, id: &str, from: &str, to: &str) -> Result<()> {
    let mut all = read(dirs, id).await;
    if let Some(g) = all.remove(base(from)) {
        all.insert(base(to).to_string(), g);
        write(dirs, id, &all).await?;
    }
    Ok(())
}

pub async fn forget(dirs: &Dirs, id: &str, file: &str) -> Result<()> {
    let mut all = read(dirs, id).await;
    if all.remove(base(file)).is_some() {
        write(dirs, id, &all).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn set_move_and_forget() {
        let root = std::env::temp_dir().join(format!("gn-groups-{}", uuid::Uuid::new_v4()));
        let dirs = Dirs::new(&root);
        std::fs::create_dir_all(dirs.instance("a")).unwrap();
        set(&dirs, "a", &["x.jar".into(), "y.jar.disabled".into()], Some(" Look ")).await.unwrap();
        let g = read(&dirs, "a").await;
        assert_eq!((g.get("x.jar").map(String::as_str), g.get("y.jar").map(String::as_str)), (Some("Look"), Some("Look")));
        rename(&dirs, "a", "x.jar", "x2.jar").await.unwrap();
        assert!(read(&dirs, "a").await.contains_key("x2.jar"));
        forget(&dirs, "a", "x2.jar").await.unwrap();
        set(&dirs, "a", &["y.jar".into()], None).await.unwrap();
        assert!(!path(&dirs, "a").exists(), "an empty list leaves no file behind");
        assert!(set(&dirs, "a", &["x.jar".into()], Some("")).await.is_err());
        assert!(set(&dirs, "a", &["../x.jar".into()], Some("G")).await.is_err());
        assert!(set(&dirs, "../a", &["x.jar".into()], Some("G")).await.is_err());
        let _ = std::fs::remove_dir_all(root);
    }
}
