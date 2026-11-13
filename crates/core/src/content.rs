//! Resource packs, shaders and data packs of an instance. Mods have their own module; this one
//! lists all kinds together, switches packs on and off by renaming (`.disabled`), and installs
//! from Modrinth.
use crate::download::{download_one, pack_host_allowed, Job};
use crate::instance::{self, Instance};
use crate::mods::{read_index, write_index, Entry, Installed};
use crate::modrinth;
use crate::paths::{base_name, file_name_ok, has_extension, valid_id, Dirs, DISABLED};
use anyhow::{bail, Context, Result};
use serde::Serialize;
use std::path::PathBuf;

const MAX_PACK_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_WORLDS: usize = 100;

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct ContentFile {
    pub kind: &'static str,
    pub file: String,
    /// Data packs live inside a world; this is that world's folder.
    pub world: Option<String>,
    pub enabled: bool,
    pub size: u64,
    pub title: Option<String>,
    pub project_id: Option<String>,
    pub icon: Option<String>,
    pub manual: bool,
    pub group: Option<String>,
}

fn pack_name_ok(file: &str) -> bool {
    file_name_ok(file) && has_extension(file, "zip")
}

fn static_kind(kind: &str) -> Result<&'static str> {
    Ok(match kind {
        "resourcepack" => "resourcepack",
        "shader" => "shader",
        "datapack" => "datapack",
        _ => bail!("unknown content type"),
    })
}

/// The folder a pack of this kind belongs in. Data packs need an existing world.
fn dir_of(dirs: &Dirs, id: &str, kind: &str, world: Option<&str>) -> Result<PathBuf> {
    if !valid_id(id) {
        bail!("bad instance id");
    }
    let game = instance::game_dir(dirs, id);
    Ok(match kind {
        "resourcepack" => game.join("resourcepacks"),
        "shader" => game.join("shaderpacks"),
        "datapack" => {
            let world = world.filter(|w| file_name_ok(w)).context("a data pack needs a world")?;
            let dir = game.join("saves").join(world);
            if !crate::worlds::is_world(&dir) {
                bail!("world not found");
            }
            dir.join("datapacks")
        }
        _ => bail!("unknown content type"),
    })
}

async fn scan(dir: PathBuf, kind: &'static str, world: Option<String>, idx: &crate::mods::Index, groups: &crate::groups::Groups, out: &mut Vec<ContentFile>) {
    let Ok(mut rd) = tokio::fs::read_dir(dir).await else { return };
    while let Ok(Some(e)) = rd.next_entry().await {
        let file = e.file_name().to_string_lossy().to_string();
        if !pack_name_ok(&file) || !e.file_type().await.is_ok_and(|t| t.is_file()) {
            continue;
        }
        let known = idx.get(base_name(&file));
        out.push(ContentFile {
            kind,
            enabled: !file.ends_with(DISABLED),
            size: e.metadata().await.map_or(0, |m| m.len()),
            title: known.map(|k| k.title.clone()),
            project_id: known.map(|k| k.project_id.clone()),
            icon: known.and_then(|k| k.icon.clone()),
            manual: known.is_some_and(|k| k.manual),
            group: groups.get(base_name(&file)).cloned(),
            world: world.clone(),
            file,
        });
    }
}

/// Resource packs, shaders and the data packs of every world, sorted by name.
pub async fn list(dirs: &Dirs, id: &str) -> Result<Vec<ContentFile>> {
    if !valid_id(id) {
        bail!("bad instance id");
    }
    let idx = read_index(dirs, id).await;
    let groups = crate::groups::read(dirs, id).await;
    let game = instance::game_dir(dirs, id);
    let mut out = Vec::new();
    scan(game.join("resourcepacks"), "resourcepack", None, &idx, &groups, &mut out).await;
    scan(game.join("shaderpacks"), "shader", None, &idx, &groups, &mut out).await;
    if let Ok(mut rd) = tokio::fs::read_dir(game.join("saves")).await {
        let mut seen = 0;
        while let Ok(Some(e)) = rd.next_entry().await {
            let name = e.file_name().to_string_lossy().to_string();
            if seen >= MAX_WORLDS {
                break;
            }
            if file_name_ok(&name) && crate::worlds::is_world(&e.path()) {
                seen += 1;
                scan(e.path().join("datapacks"), "datapack", Some(name), &idx, &groups, &mut out).await;
            }
        }
    }
    out.sort_by_key(|c| base_name(&c.file).to_lowercase());
    Ok(out)
}

async fn existing(dirs: &Dirs, id: &str, kind: &str, world: Option<&str>, file: &str) -> Result<PathBuf> {
    if !pack_name_ok(file) {
        bail!("bad file reference");
    }
    let p = dir_of(dirs, id, kind, world)?.join(file);
    if !tokio::fs::metadata(&p).await.is_ok_and(|m| m.is_file()) {
        bail!("file not found");
    }
    Ok(p)
}

pub async fn set_enabled(dirs: &Dirs, id: &str, kind: &str, world: Option<&str>, file: &str, enabled: bool) -> Result<()> {
    let from = existing(dirs, id, kind, world, file).await?;
    let base = base_name(file);
    let to = from.with_file_name(if enabled { base.to_string() } else { format!("{base}{DISABLED}") });
    if from != to {
        tokio::fs::rename(from, to).await?;
    }
    Ok(())
}

pub async fn remove(dirs: &Dirs, id: &str, kind: &str, world: Option<&str>, file: &str) -> Result<()> {
    let p = existing(dirs, id, kind, world, file).await?;
    tokio::fs::remove_file(p).await?;
    let mut idx = read_index(dirs, id).await;
    if idx.remove(base_name(file)).is_some() {
        write_index(dirs, id, &idx).await?;
    }
    crate::groups::forget(dirs, id, file).await
}

/// Installs the newest version of a Modrinth resource pack, shader or data pack for the instance's Minecraft version.
pub async fn install(dirs: &Dirs, client: &reqwest::Client, inst: &Instance, kind: &str, project_id: &str, world: Option<&str>) -> Result<Installed> {
    let kind = static_kind(kind)?;
    let dir = dir_of(dirs, &inst.id, kind, world)?;
    tokio::fs::create_dir_all(&dir).await?;
    let mut idx = read_index(dirs, &inst.id).await;
    if idx.values().any(|e| e.project_id == project_id) && kind != "datapack" {
        return Ok(Installed::default());
    }
    let versions = modrinth::versions_for_game(client, project_id, &inst.mc_version).await?;
    let v = modrinth::pick_version(&versions).with_context(|| format!("no version for Minecraft {}", inst.mc_version))?;
    let f = modrinth::primary_file(v).context("version has no file")?;
    if !pack_name_ok(&f.filename) || f.filename.ends_with(DISABLED) || !pack_host_allowed(&f.url) || f.size > MAX_PACK_BYTES {
        bail!("unexpected pack file");
    }
    let job = Job { url: f.url.clone(), dest: dir.join(&f.filename), sha1: f.hashes.sha1.clone(), size: Some(f.size) };
    download_one(client, &job).await?;
    let info = modrinth::project(client, project_id).await.ok();
    let title = info.as_ref().map_or_else(|| f.filename.clone(), |p| p.title.clone());
    idx.insert(f.filename.clone(), Entry { project_id: project_id.to_string(), title: title.clone(), icon: info.and_then(|p| p.icon_url), manual: true });
    write_index(dirs, &inst.id, &idx).await?;
    Ok(Installed { installed: vec![title], manual: Vec::new() })
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct AddedItem {
    pub name: String,
    pub kind: &'static str,
    pub world: Option<String>,
}

#[derive(Serialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct AddResult {
    pub added: Vec<AddedItem>,
    /// File name and why it was not added.
    pub skipped: Vec<(String, String)>,
}

const MAX_DROPPED: usize = 50;
const MAX_ZIP_ENTRIES: usize = 20_000;

/// What a zip is, judged by its entry names: shaders carry a `shaders/` folder, a resource pack has
/// `assets/`, a data pack has `data/` and no `assets/`.
pub fn classify_zip(names: &[String]) -> Option<&'static str> {
    let has = |prefix: &str| names.iter().any(|n| n.starts_with(prefix));
    if has("shaders/") {
        Some("shader")
    } else if names.iter().any(|n| n == "pack.mcmeta") {
        Some(if has("assets/") || !has("data/") { "resourcepack" } else { "datapack" })
    } else {
        None
    }
}

fn zip_names(path: &std::path::Path) -> Result<Vec<String>> {
    let z = zip::ZipArchive::new(std::fs::File::open(path)?)?;
    Ok(z.file_names().take(MAX_ZIP_ENTRIES).filter_map(std::result::Result::ok).map(std::borrow::Cow::into_owned).collect())
}

/// Copies dropped files into the instance: `.jar` as a mod, `.zip` by what it contains. Nothing is
/// overwritten and nothing outside the instance folders is touched.
pub async fn add_files(dirs: &Dirs, inst: &Instance, paths: Vec<PathBuf>) -> Result<AddResult> {
    let mut out = AddResult::default();
    for path in paths.into_iter().take(MAX_DROPPED) {
        let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let mut skip = |why: &str| out.skipped.push((name.clone(), why.to_string()));
        let meta = tokio::fs::metadata(&path).await;
        if !path.is_absolute() || !file_name_ok(&name) || !meta.as_ref().is_ok_and(std::fs::Metadata::is_file) {
            skip("not a file");
            continue;
        }
        if meta.map_or(0, |m| m.len()) > MAX_PACK_BYTES {
            skip("too large");
            continue;
        }
        let lower = name.to_lowercase();
        let (kind, world): (&'static str, Option<String>) = if lower.ends_with(".jar") {
            if inst.loader == instance::Loader::Vanilla {
                skip("Vanilla loads no mods");
                continue;
            }
            ("mod", None)
        } else if lower.ends_with(".zip") {
            let p = path.clone();
            let names = tokio::task::spawn_blocking(move || zip_names(&p)).await?.unwrap_or_default();
            match classify_zip(&names) {
                Some("datapack") => if let Some(w) = crate::worlds::list_instance(dirs, &inst.id).await?.into_iter().next() { ("datapack", Some(w.folder)) } else {
                    skip("create a world first");
                    continue;
                },
                Some(k) => (k, None),
                None => {
                    skip("not a resource pack, shader or data pack");
                    continue;
                }
            }
        } else {
            skip("unsupported file type");
            continue;
        };
        let dir = if kind == "mod" { crate::mods::mods_dir(dirs, &inst.id) } else { dir_of(dirs, &inst.id, kind, world.as_deref())? };
        tokio::fs::create_dir_all(&dir).await?;
        let dest = dir.join(&name);
        if tokio::fs::metadata(&dest).await.is_ok() || tokio::fs::metadata(dir.join(format!("{name}{DISABLED}"))).await.is_ok() {
            skip("already there");
            continue;
        }
        let tmp = dir.join(format!("{name}.part"));
        tokio::fs::copy(&path, &tmp).await?;
        tokio::fs::rename(&tmp, &dest).await?;
        out.added.push(AddedItem { name, kind, world });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp() -> (Dirs, String, PathBuf) {
        let root = std::env::temp_dir().join(format!("gn-content-{}", uuid::Uuid::new_v4()));
        (Dirs::new(&root), "inst1".to_string(), root)
    }

    fn put(path: PathBuf, bytes: &[u8]) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }

    #[tokio::test]
    async fn lists_all_kinds_and_toggles_and_removes() {
        let (dirs, id, root) = temp();
        let game = instance::game_dir(&dirs, &id);
        put(game.join("resourcepacks/Faithful.zip"), &[0; 4]);
        put(game.join("shaderpacks/bsl.zip.disabled"), &[0; 6]);
        put(game.join("resourcepacks/readme.txt"), b"x");
        put(game.join("saves/World1/level.dat"), &[0; 1]);
        put(game.join("saves/World1/datapacks/vein.zip"), &[0; 2]);
        put(game.join("saves/NotAWorld/datapacks/x.zip"), &[0; 2]);

        let l = list(&dirs, &id).await.unwrap();
        let names: Vec<_> = l.iter().map(|c| (c.kind, c.file.as_str(), c.enabled, c.world.as_deref())).collect();
        assert_eq!(names, [("shader", "bsl.zip.disabled", false, None), ("resourcepack", "Faithful.zip", true, None), ("datapack", "vein.zip", true, Some("World1"))]);

        set_enabled(&dirs, &id, "resourcepack", None, "Faithful.zip", false).await.unwrap();
        assert!(game.join("resourcepacks/Faithful.zip.disabled").is_file());
        set_enabled(&dirs, &id, "datapack", Some("World1"), "vein.zip", false).await.unwrap();
        assert!(game.join("saves/World1/datapacks/vein.zip.disabled").is_file());
        remove(&dirs, &id, "shader", None, "bsl.zip.disabled").await.unwrap();
        assert!(!game.join("shaderpacks/bsl.zip.disabled").exists());
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn refuses_bad_references() {
        let (dirs, id, root) = temp();
        for (kind, world, file) in [("resourcepack", None, "../x.zip"), ("shader", None, "a/b.zip"), ("shader", None, "x.jar"), ("mod", None, "x.zip"), ("datapack", None, "x.zip"), ("datapack", Some("../evil"), "x.zip"), ("datapack", Some("Missing"), "x.zip")] {
            assert!(set_enabled(&dirs, &id, kind, world, file, false).await.is_err(), "{kind} {world:?} {file}");
            assert!(remove(&dirs, &id, kind, world, file).await.is_err(), "{kind} {world:?} {file}");
        }
        assert!(list(&dirs, "../etc").await.is_err());
        let _ = std::fs::remove_dir_all(root);
    }


    fn write_zip(path: &std::path::Path, entries: &[&str]) {
        let mut w = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        for n in entries {
            w.start_file(*n, zip::write::SimpleFileOptions::default()).unwrap();
            std::io::Write::write_all(&mut w, b"x").unwrap();
        }
        w.finish().unwrap();
    }

    #[test]
    fn zips_are_told_apart_by_their_contents() {
        let n = |v: &[&str]| v.iter().map(std::string::ToString::to_string).collect::<Vec<_>>();
        assert_eq!(classify_zip(&n(&["pack.mcmeta", "assets/minecraft/x.png"])), Some("resourcepack"));
        assert_eq!(classify_zip(&n(&["pack.mcmeta", "data/a/b.json"])), Some("datapack"));
        assert_eq!(classify_zip(&n(&["shaders/final.fsh", "shaders/lib/x.glsl"])), Some("shader"));
        assert_eq!(classify_zip(&n(&["pack.mcmeta"])), Some("resourcepack"));
        assert_eq!(classify_zip(&n(&["readme.txt", "src/a.java"])), None);
    }

    #[tokio::test]
    async fn dropped_files_land_in_the_right_folders_and_nothing_is_overwritten() {
        let (dirs, id, root) = temp();
        let game = instance::game_dir(&dirs, &id);
        put(game.join("saves/World1/level.dat"), &[0; 1]);
        let inst: Instance = serde_json::from_value(serde_json::json!({
            "id": id, "name": "t", "mc_version": "1.21.1", "loader": "fabric", "loader_version": null, "memory_mb": null,
            "java_path": null, "jvm_args": null, "created": 0, "source": null
        }))
        .unwrap();
        let src = root.join("incoming");
        std::fs::create_dir_all(&src).unwrap();
        put(src.join("cool.jar"), b"jar");
        write_zip(&src.join("faith.zip"), &["pack.mcmeta", "assets/a.png"]);
        write_zip(&src.join("bsl.zip"), &["shaders/a.fsh"]);
        write_zip(&src.join("vein.zip"), &["pack.mcmeta", "data/a.json"]);
        put(src.join("notes.txt"), b"x");
        write_zip(&src.join("random.zip"), &["a.txt"]);

        let paths: Vec<_> = ["cool.jar", "faith.zip", "bsl.zip", "vein.zip", "notes.txt", "random.zip", "missing.jar"].iter().map(|n| src.join(n)).collect();
        let r = add_files(&dirs, &inst, paths.clone()).await.unwrap();
        let added: Vec<_> = r.added.iter().map(|a| (a.name.as_str(), a.kind, a.world.as_deref())).collect();
        assert_eq!(added, [("cool.jar", "mod", None), ("faith.zip", "resourcepack", None), ("bsl.zip", "shader", None), ("vein.zip", "datapack", Some("World1"))]);
        assert_eq!(r.skipped.len(), 3);
        assert!(game.join("mods/cool.jar").is_file() && game.join("resourcepacks/faith.zip").is_file() && game.join("shaderpacks/bsl.zip").is_file());
        assert!(game.join("saves/World1/datapacks/vein.zip").is_file());
        let again = add_files(&dirs, &inst, paths).await.unwrap();
        assert!(again.added.is_empty() && again.skipped.iter().any(|(_, why)| why == "already there"));
        let vanilla = Instance { loader: instance::Loader::Vanilla, ..inst };
        let v = add_files(&dirs, &vanilla, vec![src.join("cool.jar")]).await.unwrap();
        assert!(v.added.is_empty());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn names_and_kinds() {
        assert!(pack_name_ok("a.zip") && pack_name_ok("a.zip.disabled") && !pack_name_ok("a.jar") && !pack_name_ok("../a.zip"));
        assert!(static_kind("shader").is_ok() && static_kind("mod").is_err());
    }
}
