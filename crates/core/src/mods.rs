use crate::download::{download_one, pack_host_allowed, Job};
use crate::instance::{self, Instance, Loader};
use crate::modrinth;
use crate::paths::{base_name, file_name_ok, has_extension, valid_id, Dirs, DISABLED};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

const MAX_MOD_BYTES: u64 = 256 * 1024 * 1024;
const MAX_DEPTH: u8 = 3;
const MAX_INSTALLS: usize = 30;

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct ModFile {
    pub file: String,
    pub enabled: bool,
    pub size: u64,
    pub title: Option<String>,
    pub project_id: Option<String>,
    pub icon: Option<String>,
    /// Installed from the launcher's mod browser (not a dependency, not part of the modpack).
    pub manual: bool,
    pub group: Option<String>,
}

/// Result of an install: what was added, and files the author only allows on curseforge.com.
#[derive(Serialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct Installed {
    pub installed: Vec<String>,
    pub manual: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub(crate) struct Entry {
    pub(crate) project_id: String,
    pub(crate) title: String,
    #[serde(default)]
    pub(crate) icon: Option<String>,
    #[serde(default)]
    pub(crate) manual: bool,
}

pub(crate) type Index = BTreeMap<String, Entry>;

pub fn mods_dir(dirs: &Dirs, id: &str) -> PathBuf {
    instance::game_dir(dirs, id).join("mods")
}

fn index_path(dirs: &Dirs, id: &str) -> PathBuf {
    dirs.instance(id).join("mods.json")
}

pub(crate) async fn read_index(dirs: &Dirs, id: &str) -> Index {
    match tokio::fs::read(index_path(dirs, id)).await {
        Ok(b) => serde_json::from_slice(&b).unwrap_or_default(),
        Err(_) => Index::new(),
    }
}

pub(crate) async fn write_index(dirs: &Dirs, id: &str, idx: &Index) -> Result<()> {
    let path = index_path(dirs, id);
    let tmp = path.with_extension("json.tmp");
    tokio::fs::write(&tmp, serde_json::to_vec(idx)?).await?;
    tokio::fs::rename(&tmp, &path).await?;
    Ok(())
}

fn mod_name_ok(file: &str) -> bool {
    file_name_ok(file) && has_extension(file, "jar")
}

pub async fn list(dirs: &Dirs, id: &str) -> Result<Vec<ModFile>> {
    if !valid_id(id) {
        bail!("bad instance id");
    }
    let idx = read_index(dirs, id).await;
    let groups = crate::groups::read(dirs, id).await;
    let mut out = Vec::new();
    let Ok(mut rd) = tokio::fs::read_dir(mods_dir(dirs, id)).await else { return Ok(out) };
    while let Some(e) = rd.next_entry().await? {
        let file = e.file_name().to_string_lossy().to_string();
        if !mod_name_ok(&file) || !e.file_type().await.is_ok_and(|t| t.is_file()) {
            continue;
        }
        let known = idx.get(base_name(&file));
        out.push(ModFile {
            enabled: !file.ends_with(DISABLED),
            size: e.metadata().await.map_or(0, |m| m.len()),
            title: known.map(|k| k.title.clone()),
            project_id: known.map(|k| k.project_id.clone()),
            icon: known.and_then(|k| k.icon.clone()),
            manual: known.is_some_and(|k| k.manual),
            group: groups.get(base_name(&file)).cloned(),
            file,
        });
    }
    out.sort_by_key(|m| base_name(&m.file).to_lowercase());
    Ok(out)
}

async fn existing(dirs: &Dirs, id: &str, file: &str) -> Result<PathBuf> {
    if !valid_id(id) || !mod_name_ok(file) {
        bail!("bad mod reference");
    }
    let p = mods_dir(dirs, id).join(file);
    if !tokio::fs::metadata(&p).await.is_ok_and(|m| m.is_file()) {
        bail!("mod file not found");
    }
    Ok(p)
}

pub async fn set_enabled(dirs: &Dirs, id: &str, file: &str, enabled: bool) -> Result<()> {
    let from = existing(dirs, id, file).await?;
    let base = base_name(file);
    let to = mods_dir(dirs, id).join(if enabled { base.to_string() } else { format!("{base}{DISABLED}") });
    if from != to {
        tokio::fs::rename(from, to).await?;
    }
    Ok(())
}

pub async fn remove(dirs: &Dirs, id: &str, file: &str) -> Result<()> {
    let p = existing(dirs, id, file).await?;
    tokio::fs::remove_file(p).await?;
    let mut idx = read_index(dirs, id).await;
    if idx.remove(base_name(file)).is_some() {
        write_index(dirs, id, &idx).await?;
    }
    crate::groups::forget(dirs, id, file).await
}

/// Looks up mods that came from a modpack (or by hand) on Modrinth by file hash so they get a
/// name and logo. Returns how many files were newly recognised.
pub async fn identify(dirs: &Dirs, client: &reqwest::Client, id: &str) -> Result<usize> {
    const BATCH: usize = 300;
    const MAX_ROUNDS: usize = 10;
    let mut idx = read_index(dirs, id).await;
    let mut total = 0;
    let mut tried = std::collections::HashSet::new();
    for _ in 0..MAX_ROUNDS {
        let mut by_hash = BTreeMap::new();
        let batch: Vec<ModFile> = list(dirs, id).await?.into_iter().filter(|m| !idx.contains_key(base_name(&m.file)) && !tried.contains(&m.file)).take(BATCH).collect();
        for m in batch {
            tried.insert(m.file.clone());
            if m.size > MAX_MOD_BYTES {
                continue;
            }
            if let Ok(h) = crate::download::sha1_file(&mods_dir(dirs, id).join(&m.file)).await {
                by_hash.insert(h, base_name(&m.file).to_string());
            }
        }
        if by_hash.is_empty() {
            break;
        }
        let hashes: Vec<String> = by_hash.keys().cloned().collect();
        let found = modrinth::projects_by_hash(client, &hashes).await?;
        let ids: Vec<String> = found.values().cloned().collect::<std::collections::BTreeSet<_>>().into_iter().collect();
        let infos: std::collections::HashMap<String, modrinth::Project> = modrinth::projects(client, &ids).await?.into_iter().map(|p| (p.id.clone(), p)).collect();
        for (hash, pid) in found {
            let (Some(file), Some(p)) = (by_hash.get(&hash), infos.get(&pid)) else { continue };
            idx.insert(file.clone(), Entry { project_id: pid, title: p.title.clone(), icon: p.icon_url.clone(), manual: false });
            total += 1;
        }
        write_index(dirs, id, &idx).await?;
    }
    Ok(total)
}

/// Installs the newest compatible version of a Modrinth project plus its required dependencies.
/// Returns the titles that were newly installed.
/// The library mod most mods of this loader need: Fabric API, or QFAPI for Quilt. Forge and NeoForge ship their own.
pub fn api_mod(loader: Loader) -> Option<&'static str> {
    match loader {
        Loader::Fabric => Some("P7dR8mSH"),
        Loader::Quilt => Some("qvIfYCYJ"),
        _ => None,
    }
}

pub async fn install(dirs: &Dirs, client: &reqwest::Client, inst: &Instance, project_id: &str, with_deps: bool) -> Result<Installed> {
    if inst.loader == Loader::Vanilla {
        bail!("Vanilla loads no mods. Create the instance with Fabric, Quilt, Forge or NeoForge.");
    }
    let dir = mods_dir(dirs, &inst.id);
    tokio::fs::create_dir_all(&dir).await?;
    let mut idx = read_index(dirs, &inst.id).await;
    let mut queue = vec![(project_id.to_string(), 0u8)];
    let mut installed = Vec::new();
    while let Some((pid, depth)) = queue.pop() {
        if idx.values().any(|e| e.project_id == pid) {
            continue;
        }
        if installed.len() >= MAX_INSTALLS {
            bail!("too many dependencies");
        }
        let versions = modrinth::compatible_versions(client, &pid, &inst.mc_version, inst.loader).await?;
        let Some(v) = modrinth::pick_version(&versions) else {
            if depth == 0 {
                bail!("no version for Minecraft {} with {:?}", inst.mc_version, inst.loader);
            }
            continue;
        };
        let f = modrinth::primary_file(v).context("version has no file")?;
        if !mod_name_ok(&f.filename) || f.filename.ends_with(DISABLED) || !pack_host_allowed(&f.url) || f.size > MAX_MOD_BYTES {
            bail!("unexpected mod file");
        }
        let job = Job { url: f.url.clone(), dest: dir.join(&f.filename), sha1: f.hashes.sha1.clone(), size: Some(f.size) };
        download_one(client, &job).await?;
        let info = modrinth::project(client, &pid).await.ok();
        let title = info.as_ref().map_or_else(|| f.filename.clone(), |p| p.title.clone());
        idx.insert(f.filename.clone(), Entry { project_id: pid.clone(), title: title.clone(), icon: info.and_then(|p| p.icon_url), manual: depth == 0 });
        installed.push(title);
        if with_deps && depth < MAX_DEPTH {
            for d in v.dependencies.iter().filter(|d| d.dependency_type == "required") {
                if let Some(dp) = &d.project_id {
                    queue.push((dp.clone(), depth + 1));
                }
            }
        }
    }
    write_index(dirs, &inst.id, &idx).await?;
    Ok(Installed { installed, manual: Vec::new() })
}

/// CurseForge variant of `install`. Mods are keyed `cf:<id>` in the index.
pub async fn install_curseforge(dirs: &Dirs, client: &reqwest::Client, cf: &crate::curseforge::CurseForge, inst: &Instance, mod_id: u32, with_deps: bool) -> Result<Installed> {
    if inst.loader == Loader::Vanilla {
        bail!("Vanilla loads no mods. Create the instance with Fabric, Quilt, Forge or NeoForge.");
    }
    let dir = mods_dir(dirs, &inst.id);
    tokio::fs::create_dir_all(&dir).await?;
    let mut idx = read_index(dirs, &inst.id).await;
    let mut queue = vec![(mod_id, 0u8)];
    let mut out = Installed::default();
    while let Some((id, depth)) = queue.pop() {
        let key = format!("cf:{id}");
        if idx.values().any(|e| e.project_id == key) {
            continue;
        }
        if out.installed.len() + out.manual.len() >= MAX_INSTALLS {
            bail!("too many dependencies");
        }
        let files = cf.compatible_files(client, id, &inst.mc_version, inst.loader).await?;
        let Some(f) = crate::curseforge::pick_file(&files) else {
            if depth == 0 {
                bail!("no version for Minecraft {} with {:?}", inst.mc_version, inst.loader);
            }
            continue;
        };
        let info = cf.mods(client, &[id]).await.ok().and_then(|v| v.into_iter().next());
        let title = info.as_ref().map_or_else(|| f.file_name.clone(), |m| m.name.clone());
        let url = f.download_url.clone().filter(|u| pack_host_allowed(u));
        let Some(url) = url else {
            out.manual.push(title);
            continue;
        };
        if !mod_name_ok(&f.file_name) || f.file_name.ends_with(DISABLED) || f.file_length > MAX_MOD_BYTES {
            bail!("unexpected mod file");
        }
        let job = Job { url, dest: dir.join(&f.file_name), sha1: f.sha1(), size: Some(f.file_length) };
        download_one(client, &job).await?;
        idx.insert(f.file_name.clone(), Entry { project_id: key, title: title.clone(), icon: info.and_then(|m| m.logo).map(|l| l.thumbnail_url), manual: depth == 0 });
        out.installed.push(title);
        if with_deps && depth < MAX_DEPTH {
            queue.extend(f.required_deps().map(|d| (d, depth + 1)));
        }
    }
    write_index(dirs, &inst.id, &idx).await?;
    Ok(out)
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct ModUpdate {
    pub file: String,
    pub title: Option<String>,
    pub version: String,
}

/// The file to update to, if `latest` is a stable release that differs from the one installed.
fn update_target<'a>(current_sha1: &str, latest: &'a modrinth::Version) -> Option<&'a modrinth::File> {
    if latest.version_type != "release" {
        return None;
    }
    let f = modrinth::primary_file(latest)?;
    let ok = mod_name_ok(&f.filename) && !f.filename.ends_with(DISABLED) && pack_host_allowed(&f.url) && f.size <= MAX_MOD_BYTES;
    (ok && f.hashes.sha1.as_deref().is_some_and(|h| !h.eq_ignore_ascii_case(current_sha1))).then_some(f)
}

async fn find_updates(dirs: &Dirs, client: &reqwest::Client, inst: &Instance) -> Result<Vec<(ModFile, modrinth::Version)>> {
    if inst.loader == Loader::Vanilla {
        return Ok(Vec::new());
    }
    let mut by_hash = BTreeMap::new();
    for m in list(dirs, &inst.id).await?.into_iter().filter(|m| m.size <= MAX_MOD_BYTES && m.project_id.as_deref().is_none_or(|p| !p.starts_with("cf:"))) {
        if let Ok(h) = crate::download::sha1_file(&mods_dir(dirs, &inst.id).join(&m.file)).await {
            by_hash.insert(h, m);
        }
    }
    let hashes: Vec<String> = by_hash.keys().cloned().collect();
    let latest = modrinth::latest_by_hash(client, &hashes, &inst.mc_version, inst.loader).await?;
    let mut out: Vec<_> = latest.into_iter().filter_map(|(h, v)| by_hash.remove(&h).filter(|_| update_target(&h, &v).is_some()).map(|m| (m, v))).collect();
    out.sort_by_key(|(m, _)| base_name(&m.file).to_lowercase());
    Ok(out)
}

/// Mods of the instance that have a newer stable version on Modrinth for its Minecraft version and loader.
pub async fn check_updates(dirs: &Dirs, client: &reqwest::Client, inst: &Instance) -> Result<Vec<ModUpdate>> {
    Ok(find_updates(dirs, client, inst).await?.into_iter().map(|(m, v)| ModUpdate { file: m.file, title: m.title, version: v.version_number }).collect())
}

/// Replaces the given mod files by their newer versions. A disabled mod stays disabled.
/// Returns the titles that were updated.
pub async fn update(dirs: &Dirs, client: &reqwest::Client, inst: &Instance, files: &[String]) -> Result<Vec<String>> {
    let dir = mods_dir(dirs, &inst.id);
    let mut idx = read_index(dirs, &inst.id).await;
    let mut done = Vec::new();
    for (m, v) in find_updates(dirs, client, inst).await?.into_iter().filter(|(m, _)| files.contains(&m.file)) {
        let current = crate::download::sha1_file(&dir.join(&m.file)).await?;
        let f = update_target(&current, &v).context("update no longer applies")?;
        download_one(client, &Job { url: f.url.clone(), dest: dir.join(&f.filename), sha1: f.hashes.sha1.clone(), size: Some(f.size) }).await?;
        let old = base_name(&m.file).to_string();
        if m.file != f.filename {
            tokio::fs::remove_file(dir.join(&m.file)).await?;
        }
        if !m.enabled {
            tokio::fs::rename(dir.join(&f.filename), dir.join(format!("{}{DISABLED}", f.filename))).await?;
        }
        let kept = idx.remove(&old);
        idx.insert(f.filename.clone(), Entry {
            project_id: kept.as_ref().map_or_else(|| v.project_id.clone(), |e| e.project_id.clone()),
            title: kept.as_ref().map(|e| e.title.clone()).or(m.title.clone()).unwrap_or_else(|| f.filename.clone()),
            icon: kept.as_ref().and_then(|e| e.icon.clone()),
            manual: kept.is_some_and(|e| e.manual),
        });
        crate::groups::rename(dirs, &inst.id, &m.file, &f.filename).await?;
        done.push(m.title.unwrap_or(old));
    }
    write_index(dirs, &inst.id, &idx).await?;
    Ok(done)
}

/// A mod another mod needs. `id` is the project id (CurseForge: the numeric id), `installed` is
/// already true when the instance has it.
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct Dep {
    pub id: String,
    pub title: String,
    pub icon: Option<String>,
    pub installed: bool,
}

pub async fn dependencies(dirs: &Dirs, client: &reqwest::Client, inst: &Instance, project_id: &str) -> Result<Vec<Dep>> {
    let idx = read_index(dirs, &inst.id).await;
    let mut seen = std::collections::HashSet::from([project_id.to_string()]);
    let mut queue = vec![(project_id.to_string(), 0u8)];
    let mut found: Vec<String> = Vec::new();
    while let Some((pid, depth)) = queue.pop() {
        if found.len() >= MAX_INSTALLS {
            break;
        }
        let versions = modrinth::compatible_versions(client, &pid, &inst.mc_version, inst.loader).await.unwrap_or_default();
        let Some(v) = modrinth::pick_version(&versions) else { continue };
        if depth > 0 {
            found.push(pid.clone());
        }
        if depth < MAX_DEPTH {
            for d in v.dependencies.iter().filter(|d| d.dependency_type == "required") {
                if let Some(dp) = d.project_id.clone().filter(|dp| seen.insert(dp.clone())) {
                    queue.push((dp, depth + 1));
                }
            }
        }
    }
    if found.is_empty() {
        return Ok(Vec::new());
    }
    let infos = modrinth::projects(client, &found).await?;
    Ok(found
        .iter()
        .filter_map(|id| infos.iter().find(|p| &p.id == id))
        .map(|p| Dep { id: p.id.clone(), title: p.title.clone(), icon: p.icon_url.clone(), installed: idx.values().any(|e| e.project_id == p.id) })
        .collect())
}

pub async fn dependencies_curseforge(dirs: &Dirs, client: &reqwest::Client, cf: &crate::curseforge::CurseForge, inst: &Instance, mod_id: u32) -> Result<Vec<Dep>> {
    let idx = read_index(dirs, &inst.id).await;
    let mut seen = std::collections::HashSet::from([mod_id]);
    let mut queue = vec![(mod_id, 0u8)];
    let mut found: Vec<u32> = Vec::new();
    while let Some((id, depth)) = queue.pop() {
        if found.len() >= MAX_INSTALLS {
            break;
        }
        let files = cf.compatible_files(client, id, &inst.mc_version, inst.loader).await.unwrap_or_default();
        let Some(f) = crate::curseforge::pick_file(&files) else { continue };
        if depth > 0 {
            found.push(id);
        }
        if depth < MAX_DEPTH {
            for d in f.required_deps().filter(|d| seen.insert(*d)) {
                queue.push((d, depth + 1));
            }
        }
    }
    if found.is_empty() {
        return Ok(Vec::new());
    }
    let infos = cf.mods(client, &found).await?;
    Ok(found
        .iter()
        .filter_map(|id| infos.iter().find(|m| m.id == *id))
        .map(|m| Dep { id: m.id.to_string(), title: m.name.clone(), icon: m.logo.as_ref().map(|l| l.thumbnail_url.clone()), installed: idx.values().any(|e| e.project_id == format!("cf:{}", m.id)) })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "downloads Fabric API and QFAPI from Modrinth"]
    async fn live_new_fabric_and_quilt_modpacks_get_their_api_mod() {
        let root = std::env::temp_dir().join(format!("gn-api-{}", uuid::Uuid::new_v4()));
        let dirs = Dirs::new(&root);
        let client = crate::http_client();
        for (loader, needle) in [(Loader::Fabric, "fabric-api"), (Loader::Quilt, "qfapi")] {
            let inst = instance::create(&dirs, "A", "1.20.1", loader, None, None).await.unwrap();
            let r = install(&dirs, &client, &inst, api_mod(loader).unwrap(), true).await.unwrap();
            let files: Vec<String> = list(&dirs, &inst.id).await.unwrap().into_iter().map(|m| m.file.to_lowercase()).collect();
            println!("{loader:?}: installed {:?}, files {files:?}", r.installed);
            assert!(files.iter().any(|f| f.contains(needle)), "{files:?}");
        }
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn fabric_and_quilt_get_their_api_mod_and_the_others_none() {
        assert_eq!(api_mod(Loader::Fabric), Some("P7dR8mSH"));
        assert_eq!(api_mod(Loader::Quilt), Some("qvIfYCYJ"));
        for l in [Loader::Forge, Loader::Neoforge, Loader::Vanilla] {
            assert_eq!(api_mod(l), None);
        }
    }

    async fn setup() -> (Dirs, String, PathBuf) {
        let root = std::env::temp_dir().join(format!("gn-mods-{}", uuid::Uuid::new_v4()));
        let dirs = Dirs::new(&root);
        let id = "inst1".to_string();
        let m = mods_dir(&dirs, &id);
        tokio::fs::create_dir_all(&m).await.unwrap();
        (dirs, id, root)
    }

    #[tokio::test]
    async fn list_toggle_remove() {
        let (dirs, id, root) = setup().await;
        let m = mods_dir(&dirs, &id);
        std::fs::write(m.join("b-mod.jar"), [0u8; 5]).unwrap();
        std::fs::write(m.join("A-mod.jar.disabled"), [0u8; 3]).unwrap();
        std::fs::write(m.join("notes.txt"), b"x").unwrap();
        let mut idx = Index::new();
        idx.insert("b-mod.jar".into(), Entry { project_id: "P1".into(), title: "B Mod".into(), icon: None, manual: false });
        write_index(&dirs, &id, &idx).await.unwrap();

        let l = list(&dirs, &id).await.unwrap();
        assert_eq!(l.iter().map(|x| x.file.as_str()).collect::<Vec<_>>(), ["A-mod.jar.disabled", "b-mod.jar"]);
        assert_eq!((l[0].enabled, l[1].enabled), (false, true));
        assert_eq!(l[1].title.as_deref(), Some("B Mod"));

        set_enabled(&dirs, &id, "b-mod.jar", false).await.unwrap();
        assert!(m.join("b-mod.jar.disabled").is_file() && !m.join("b-mod.jar").exists());
        set_enabled(&dirs, &id, "b-mod.jar.disabled", true).await.unwrap();
        assert!(m.join("b-mod.jar").is_file());

        remove(&dirs, &id, "b-mod.jar").await.unwrap();
        assert!(!m.join("b-mod.jar").exists());
        assert!(read_index(&dirs, &id).await.is_empty());
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn rejects_bad_names() {
        let (dirs, id, root) = setup().await;
        for bad in ["../x.jar", "a/b.jar", "x.txt", "", ".jar.disabled.disabled"] {
            assert!(set_enabled(&dirs, &id, bad, false).await.is_err(), "{bad}");
            assert!(remove(&dirs, &id, bad).await.is_err(), "{bad}");
        }
        assert!(list(&dirs, "../etc").await.is_err());
        let _ = std::fs::remove_dir_all(root);
    }

    fn version(kind: &str, url: &str, name: &str, sha1: &str) -> modrinth::Version {
        modrinth::Version {
            id: "v".into(),
            project_id: "p".into(),
            name: name.into(),
            version_number: "2.0".into(),
            game_versions: vec![],
            loaders: vec![],
            files: vec![modrinth::File { url: url.into(), filename: name.into(), primary: true, size: 10, hashes: modrinth::Hashes { sha1: Some(sha1.into()), sha512: None } }],
            date_published: String::new(),
            version_type: kind.into(),
            dependencies: vec![],
        }
    }

    #[test]
    fn updates_only_to_other_stable_files_from_known_hosts() {
        let url = "https://cdn.modrinth.com/data/p/versions/v/a-2.0.jar";
        assert!(update_target("old", &version("release", url, "a-2.0.jar", "new")).is_some());
        assert!(update_target("NEW", &version("release", url, "a-2.0.jar", "new")).is_none(), "same file");
        assert!(update_target("old", &version("beta", url, "a-2.0.jar", "new")).is_none(), "not stable");
        assert!(update_target("old", &version("release", "https://evil.example/a.jar", "a-2.0.jar", "new")).is_none(), "foreign host");
        assert!(update_target("old", &version("release", url, "../a.jar", "new")).is_none(), "bad file name");
        assert!(update_target("old", &version("release", url, "a.zip", "new")).is_none(), "not a jar");
    }

    #[tokio::test]
    #[ignore = "talks to the live Modrinth API"]
    async fn live_old_mod_is_found_and_updated_even_when_disabled() {
        let (dirs, _, root) = setup().await;
        let client = crate::http_client();
        let inst = crate::instance::create(&dirs, "t", "1.21.1", Loader::Fabric, None, None).await.unwrap();
        let versions = modrinth::compatible_versions(&client, "AANobbMI", "1.21.1", Loader::Fabric).await.unwrap();
        let old = versions.iter().rev().find(|v| v.version_type == "release").unwrap();
        let f = modrinth::primary_file(old).unwrap();
        let dir = mods_dir(&dirs, &inst.id);
        tokio::fs::create_dir_all(&dir).await.unwrap();
        download_one(&client, &Job { url: f.url.clone(), dest: dir.join(format!("{}{DISABLED}", f.filename)), sha1: f.hashes.sha1.clone(), size: Some(f.size) }).await.unwrap();
        let found = check_updates(&dirs, &client, &inst).await.unwrap();
        assert_eq!(found.len(), 1, "{found:?}");
        let done = update(&dirs, &client, &inst, &[found[0].file.clone()]).await.unwrap();
        assert_eq!(done.len(), 1);
        let l = list(&dirs, &inst.id).await.unwrap();
        assert_eq!(l.len(), 1, "{l:?}");
        assert!(!l[0].enabled && l[0].file != format!("{}{DISABLED}", f.filename), "stays disabled, new file: {l:?}");
        assert!(check_updates(&dirs, &client, &inst).await.unwrap().is_empty());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn names() {
        assert!(mod_name_ok("sodium-0.5.jar"));
        assert!(mod_name_ok("sodium-0.5.jar.disabled"));
        assert!(!mod_name_ok("sodium.zip"));
        assert_eq!(base_name("a.jar.disabled"), "a.jar");
    }

    #[tokio::test]
    #[ignore = "talks to the live Modrinth API"]
    async fn live_search_and_install_sodium() {
        let (dirs, _, root) = setup().await;
        let client = crate::http_client();
        let hits = modrinth::search_mods(&client, "sodium", "1.21.1", Loader::Fabric, 0, 5).await.unwrap();
        let sodium = hits.hits.iter().find(|h| h.slug == "sodium").expect("sodium in results");
        let inst = crate::instance::create(&dirs, "t", "1.21.1", Loader::Fabric, None, None).await.unwrap();
        let titles = install(&dirs, &client, &inst, &sodium.project_id, true).await.unwrap();
        assert!(titles.installed.iter().any(|t| t.to_lowercase().contains("sodium")), "{titles:?}");
        let l = list(&dirs, &inst.id).await.unwrap();
        assert!(l.iter().any(|m| m.file.ends_with(".jar") && m.size > 100_000 && m.project_id.is_some()), "{l:?}");
        assert!(install(&dirs, &client, &inst, &sodium.project_id, true).await.unwrap().installed.is_empty(), "second install must be a no-op");
        assert!(l.iter().any(|m| m.icon.as_deref().is_some_and(|i| i.starts_with("https://cdn.modrinth.com/"))), "install should store the logo: {l:?}");
        let mut idx = read_index(&dirs, &inst.id).await;
        idx.clear();
        write_index(&dirs, &inst.id, &idx).await.unwrap();
        assert_eq!(identify(&dirs, &client, &inst.id).await.unwrap(), 1, "hash lookup should recognise the jar again");
        assert!(list(&dirs, &inst.id).await.unwrap()[0].icon.is_some());
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    #[ignore = "needs CURSEFORGE_API_KEY and talks to the live CurseForge API"]
    async fn live_curseforge_mod_search_and_install() {
        let (dirs, _, root) = setup().await;
        let cf = crate::curseforge::CurseForge::new(std::env::var("CURSEFORGE_API_KEY").expect("CURSEFORGE_API_KEY"));
        let client = crate::http_client();
        let inst = crate::instance::create(&dirs, "t", "1.20.1", Loader::Forge, None, None).await.unwrap();
        let hits = cf.search_mods(&client, "jei", "1.20.1", Loader::Forge, 0, 10).await.unwrap();
        let jei = hits.iter().find(|m| m.name.to_lowercase().contains("jei") || m.name.to_lowercase().contains("just enough items")).expect("JEI in results");
        let r = install_curseforge(&dirs, &client, &cf, &inst, jei.id, true).await.unwrap();
        println!("installed={:?} manual={:?}", r.installed, r.manual);
        assert!(!r.installed.is_empty() || !r.manual.is_empty());
        if !r.installed.is_empty() {
            let l = list(&dirs, &inst.id).await.unwrap();
            assert!(l.iter().any(|m| m.file.ends_with(".jar") && m.size > 50_000 && m.project_id.as_deref().is_some_and(|p| p.starts_with("cf:"))), "{l:?}");
            assert!(install_curseforge(&dirs, &client, &cf, &inst, jei.id, true).await.unwrap().installed.is_empty(), "second install is a no-op");
        }
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    #[ignore = "talks to the live Modrinth API"]
    async fn live_dependencies_of_iris_include_sodium() {
        let (dirs, _, root) = setup().await;
        let client = crate::http_client();
        let inst = crate::instance::create(&dirs, "t", "1.21.1", Loader::Fabric, None, None).await.unwrap();
        let hits = modrinth::search_mods(&client, "iris shaders", "1.21.1", Loader::Fabric, 0, 5).await.unwrap();
        let iris = hits.hits.iter().find(|h| h.slug == "iris").expect("iris in results");
        let deps = dependencies(&dirs, &client, &inst, &iris.project_id).await.unwrap();
        assert!(deps.iter().any(|d| d.title.to_lowercase().contains("sodium") && !d.installed), "{deps:?}");
        let _ = std::fs::remove_dir_all(root);
    }
}
