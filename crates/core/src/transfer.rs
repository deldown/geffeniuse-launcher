//! Copying a modpack and exporting it as a `.mrpack` (to share) or a `.zip` (full backup).
use crate::instance::{self, Instance, Loader};
use crate::modrinth;
use crate::paths::{valid_id, Dirs};
use anyhow::{bail, Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const NOT_EXPORTED: [&str; 3] = ["logs", "crash-reports", ".cache"];
const SHARED_FOLDERS: [&str; 7] = ["config", "resourcepacks", "shaderpacks", "scripts", "defaultconfigs", "kubejs", "datapacks"];
const STORED: [&str; 7] = ["jar", "zip", "png", "jpg", "webp", "ogg", "mca"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Mrpack,
    Zip,
}

impl Format {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "mrpack" => Some(Self::Mrpack),
            "zip" => Some(Self::Zip),
            _ => None,
        }
    }
    pub fn extension(self) -> &'static str {
        match self {
            Self::Mrpack => "mrpack",
            Self::Zip => "zip",
        }
    }
}

/// Copies files and folders, never following symbolic links.
fn copy_dir(from: &Path, to: &Path, skip_top: &[&str]) -> Result<()> {
    std::fs::create_dir_all(to)?;
    for e in std::fs::read_dir(from)? {
        let e = e?;
        let name = e.file_name();
        if skip_top.contains(&name.to_string_lossy().as_ref()) {
            continue;
        }
        let kind = e.file_type()?;
        if kind.is_dir() {
            copy_dir(&e.path(), &to.join(&name), &[])?;
        } else if kind.is_file() {
            std::fs::copy(e.path(), to.join(&name))?;
        }
    }
    Ok(())
}

/// A full copy of the modpack (mods, worlds, settings) under a new name.
pub async fn duplicate(dirs: &Dirs, id: &str) -> Result<Instance> {
    let src = instance::get(dirs, id).await?;
    let name: String = format!("{} (Kopie)", src.name.chars().take(52).collect::<String>());
    let mut copy = instance::create(dirs, &name, &src.mc_version, src.loader, src.loader_version.clone(), src.source.clone()).await?;
    let (from, to) = (dirs.instance(&src.id), dirs.instance(&copy.id));
    let result = async {
        tokio::task::spawn_blocking(move || copy_dir(&from, &to, &["instance.json", "instance.json.tmp"])).await??;
        copy.memory_mb = src.memory_mb;
        copy.java_path = src.java_path.clone();
        copy.jvm_args = src.jvm_args.clone();
        copy.icon = src.icon.clone();
        copy.sync_settings = src.sync_settings;
        instance::save(dirs, &copy).await
    }
    .await;
    if let Err(e) = result {
        let _ = instance::delete(dirs, &copy.id).await;
        return Err(e);
    }
    Ok(copy)
}

/// Every regular file below `dir` as (path on disk, name in the archive), without symbolic links.
fn collect(dir: &Path, prefix: &str, skip_top: &[&str], out: &mut Vec<(PathBuf, String)>) -> Result<()> {
    let Ok(rd) = std::fs::read_dir(dir) else { return Ok(()) };
    for e in rd {
        let e = e?;
        let name = e.file_name().to_string_lossy().to_string();
        if skip_top.contains(&name.as_str()) {
            continue;
        }
        let archive_name = format!("{prefix}/{name}");
        let kind = e.file_type()?;
        if kind.is_dir() {
            collect(&e.path(), &archive_name, &[], out)?;
        } else if kind.is_file() {
            out.push((e.path(), archive_name));
        }
    }
    Ok(())
}

fn write_zip(dest: &Path, extra: Vec<(String, Vec<u8>)>, files: Vec<(PathBuf, String)>) -> Result<()> {
    use zip::write::SimpleFileOptions;
    let part = dest.with_extension("part");
    let mut z = zip::ZipWriter::new(std::fs::File::create(&part)?);
    for (name, bytes) in extra {
        z.start_file(name, SimpleFileOptions::default())?;
        std::io::Write::write_all(&mut z, &bytes)?;
    }
    for (src, name) in files {
        let size = std::fs::metadata(&src)?.len();
        let stored = src.extension().and_then(|e| e.to_str()).is_some_and(|e| STORED.contains(&e.to_ascii_lowercase().as_str()));
        let method = if stored { zip::CompressionMethod::Stored } else { zip::CompressionMethod::Deflated };
        z.start_file(name, SimpleFileOptions::default().compression_method(method).large_file(size >= u64::from(u32::MAX)))?;
        std::io::copy(&mut std::fs::File::open(&src)?, &mut z)?;
    }
    z.finish()?;
    std::fs::rename(part, dest)?;
    Ok(())
}

fn loader_key(l: Loader) -> Option<&'static str> {
    Some(match l {
        Loader::Fabric => "fabric-loader",
        Loader::Quilt => "quilt-loader",
        Loader::Forge => "forge",
        Loader::Neoforge => "neoforge",
        Loader::Vanilla => return None,
    })
}

fn mrpack_index(inst: &Instance, files: Vec<serde_json::Value>) -> Result<Vec<u8>> {
    let mut deps = serde_json::Map::new();
    deps.insert("minecraft".into(), inst.mc_version.clone().into());
    if let Some(key) = loader_key(inst.loader) {
        let v = inst.loader_version.clone().context("the loader version of this modpack is unknown")?;
        deps.insert(key.into(), v.into());
    }
    Ok(serde_json::to_vec_pretty(&serde_json::json!({
        "formatVersion": 1,
        "game": "minecraft",
        "versionId": "1.0.0",
        "name": inst.name,
        "files": files,
        "dependencies": deps,
    }))?)
}

/// The Modrinth download entry for a local mod, if Modrinth has that exact file.
fn mrpack_file(rel: &str, sha1: &str, found: &modrinth::Version) -> Option<serde_json::Value> {
    let f = found.files.iter().find(|f| f.hashes.sha1.as_deref().is_some_and(|h| h.eq_ignore_ascii_case(sha1)))?;
    if !crate::download::pack_host_allowed(&f.url) {
        return None;
    }
    Some(serde_json::json!({
        "path": rel,
        "hashes": { "sha1": f.hashes.sha1, "sha512": f.hashes.sha512.as_ref()? },
        "env": { "client": "required", "server": "required" },
        "downloads": [f.url],
        "fileSize": f.size,
    }))
}

/// Writes the modpack to `dest`. A `.mrpack` links mods that Modrinth has and carries everything else
/// (settings, other mods, resource packs, shaders); a `.zip` is a full backup including worlds.
pub async fn export(dirs: &Dirs, client: &reqwest::Client, id: &str, format: Format, dest: &Path) -> Result<()> {
    if !valid_id(id) {
        bail!("bad instance id");
    }
    if dest.extension().and_then(|e| e.to_str()) != Some(format.extension()) {
        bail!("the file must end in .{}", format.extension());
    }
    let inst = instance::get(dirs, id).await?;
    let game = instance::game_dir(dirs, id);
    let (mut extra, mut files) = (Vec::new(), Vec::new());
    match format {
        Format::Zip => {
            extra.push(("instance.json".to_string(), tokio::fs::read(dirs.instance(id).join("instance.json")).await?));
            collect(&game, "minecraft", &NOT_EXPORTED, &mut files)?;
        }
        Format::Mrpack => {
            let mods = crate::mods::mods_dir(dirs, id);
            let mut hashes = HashMap::new();
            for m in crate::mods::list(dirs, id).await?.into_iter().filter(|m| m.enabled) {
                hashes.insert(crate::download::sha1_file(&mods.join(&m.file)).await?, m.file);
            }
            let known = modrinth::versions_by_hash(client, &hashes.keys().cloned().collect::<Vec<_>>()).await.unwrap_or_default();
            let mut linked = Vec::new();
            let mut skipped = std::collections::HashSet::new();
            for (sha1, file) in &hashes {
                if let Some(entry) = known.get(sha1).and_then(|v| mrpack_file(&format!("mods/{file}"), sha1, v)) {
                    linked.push(entry);
                    skipped.insert(format!("overrides/mods/{file}"));
                }
            }
            extra.push(("modrinth.index.json".into(), mrpack_index(&inst, linked)?));
            collect(&mods, "overrides/mods", &[], &mut files)?;
            files.retain(|(_, name)| !skipped.contains(name));
            for folder in SHARED_FOLDERS {
                collect(&game.join(folder), &format!("overrides/{folder}"), &[], &mut files)?;
            }
            if game.join("options.txt").is_file() {
                files.push((game.join("options.txt"), "overrides/options.txt".into()));
            }
        }
    }
    let dest = dest.to_path_buf();
    tokio::task::spawn_blocking(move || write_zip(&dest, extra, files)).await?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    fn tmp() -> PathBuf {
        std::env::temp_dir().join(format!("gn-transfer-{}", uuid::Uuid::new_v4()))
    }

    fn names(zip: &Path) -> Vec<String> {
        let mut z = zip::ZipArchive::new(std::fs::File::open(zip).unwrap()).unwrap();
        let mut v: Vec<String> = (0..z.len()).map(|i| z.by_index(i).unwrap().name().unwrap().to_string()).collect();
        v.sort();
        v
    }

    #[tokio::test]
    async fn duplicate_copies_everything_but_the_identity() {
        let root = tmp();
        let dirs = Dirs::new(&root);
        let a = instance::create(&dirs, "Pack", "1.21.1", Loader::Fabric, Some("0.16".into()), None).await.unwrap();
        instance::set_memory(&dirs, &a.id, Some(4096)).await.unwrap();
        let game = instance::game_dir(&dirs, &a.id);
        std::fs::create_dir_all(game.join("saves/World")).unwrap();
        std::fs::write(game.join("saves/World/level.dat"), b"world").unwrap();
        std::fs::write(dirs.instance(&a.id).join("mods.json"), b"{}").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink("/etc/passwd", game.join("link")).unwrap();

        let b = duplicate(&dirs, &a.id).await.unwrap();
        assert_ne!(a.id, b.id);
        assert_eq!(b.name, "Pack (Kopie)");
        assert_eq!((b.memory_mb, b.loader_version.as_deref()), (Some(4096), Some("0.16")));
        let copy = instance::game_dir(&dirs, &b.id);
        assert_eq!(std::fs::read(copy.join("saves/World/level.dat")).unwrap(), b"world");
        assert!(dirs.instance(&b.id).join("mods.json").is_file());
        assert!(!copy.join("link").exists(), "symbolic links are not followed");
        assert_eq!(instance::get(&dirs, &b.id).await.unwrap().name, "Pack (Kopie)");
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn zip_backup_has_worlds_but_no_logs() {
        let root = tmp();
        let dirs = Dirs::new(&root);
        let a = instance::create(&dirs, "Pack", "1.21.1", Loader::Vanilla, None, None).await.unwrap();
        let game = instance::game_dir(&dirs, &a.id);
        std::fs::create_dir_all(game.join("saves/W")).unwrap();
        std::fs::create_dir_all(game.join("logs")).unwrap();
        std::fs::write(game.join("saves/W/level.dat"), b"w").unwrap();
        std::fs::write(game.join("logs/latest.log"), b"l").unwrap();
        let out = root.join("backup.zip");
        export(&dirs, &crate::http_client(), &a.id, Format::Zip, &out).await.unwrap();
        assert_eq!(names(&out), ["instance.json", "minecraft/saves/W/level.dat"]);
        assert!(export(&dirs, &crate::http_client(), &a.id, Format::Zip, &root.join("x.mrpack")).await.is_err(), "wrong extension");
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    #[ignore = "talks to the live Modrinth API"]
    async fn live_mrpack_links_modrinth_mods_and_updates_find_nothing() {
        let root = tmp();
        let dirs = Dirs::new(&root);
        let client = crate::http_client();
        let inst = instance::create(&dirs, "Pack", "1.21.1", Loader::Fabric, Some("0.16.5".into()), None).await.unwrap();
        let hits = modrinth::search_mods(&client, "sodium", "1.21.1", Loader::Fabric, 0, 5).await.unwrap();
        let sodium = hits.hits.iter().find(|h| h.slug == "sodium").unwrap();
        crate::mods::install(&dirs, &client, &inst, &sodium.project_id, false).await.unwrap();
        std::fs::write(crate::mods::mods_dir(&dirs, &inst.id).join("private-mod.jar"), b"not on modrinth").unwrap();
        assert!(crate::mods::check_updates(&dirs, &client, &inst).await.unwrap().is_empty(), "a fresh install is current");
        let out = root.join("p.mrpack");
        export(&dirs, &client, &inst.id, Format::Mrpack, &out).await.unwrap();
        let all = names(&out);
        assert!(all.contains(&"modrinth.index.json".to_string()) && all.contains(&"overrides/mods/private-mod.jar".to_string()), "{all:?}");
        assert!(!all.iter().any(|n| n.contains("sodium")), "Modrinth mods are linked, not packed: {all:?}");
        let mut idx = String::new();
        zip::ZipArchive::new(std::fs::File::open(&out).unwrap()).unwrap().by_name("modrinth.index.json").unwrap().read_to_string(&mut idx).unwrap();
        assert!(idx.contains("cdn.modrinth.com") && idx.contains("sha512"), "{idx}");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn mrpack_index_names_the_loader() {
        let mut inst = Instance {
            id: "a".into(),
            name: "Pack".into(),
            mc_version: "1.21.1".into(),
            loader: Loader::Fabric,
            loader_version: Some("0.16.5".into()),
            memory_mb: None,
            java_path: None,
            jvm_args: None,
            created: 0,
            source: None,
            last_played: None,
            icon: None,
            playtime_secs: 0,
            sync_settings: true,
        };
        let json: serde_json::Value = serde_json::from_slice(&mrpack_index(&inst, vec![]).unwrap()).unwrap();
        assert_eq!(json["dependencies"]["fabric-loader"], "0.16.5");
        assert_eq!(json["dependencies"]["minecraft"], "1.21.1");
        inst.loader_version = None;
        assert!(mrpack_index(&inst, vec![]).is_err());
        inst.loader = Loader::Vanilla;
        assert!(mrpack_index(&inst, vec![]).is_ok());
    }

    #[test]
    fn mrpack_links_only_exact_files_from_known_hosts() {
        let file = |url: &str| modrinth::Version {
            id: "v".into(),
            project_id: "p".into(),
            name: "n".into(),
            version_number: "1".into(),
            game_versions: vec![],
            loaders: vec![],
            files: vec![modrinth::File { url: url.into(), filename: "a.jar".into(), primary: true, size: 3, hashes: modrinth::Hashes { sha1: Some("aa".into()), sha512: Some("bb".into()) } }],
            date_published: String::new(),
            version_type: "release".into(),
            dependencies: vec![],
        };
        let ok = mrpack_file("mods/a.jar", "AA", &file("https://cdn.modrinth.com/a.jar")).unwrap();
        assert_eq!(ok["hashes"]["sha512"], "bb");
        assert!(mrpack_file("mods/a.jar", "cc", &file("https://cdn.modrinth.com/a.jar")).is_none(), "other file of the version");
        assert!(mrpack_file("mods/a.jar", "aa", &file("https://evil.example/a.jar")).is_none());
    }

    #[test]
    fn mrpack_zip_roundtrips_through_the_importer_layout() {
        let root = tmp();
        std::fs::create_dir_all(&root).unwrap();
        let src = root.join("o.txt");
        std::fs::write(&src, b"hi").unwrap();
        let out = root.join("p.mrpack");
        write_zip(&out, vec![("modrinth.index.json".into(), b"{}".to_vec())], vec![(src, "overrides/config/o.txt".into())]).unwrap();
        let mut z = zip::ZipArchive::new(std::fs::File::open(&out).unwrap()).unwrap();
        let mut s = String::new();
        z.by_name("overrides/config/o.txt").unwrap().read_to_string(&mut s).unwrap();
        assert_eq!(s, "hi");
        assert!(!out.with_extension("part").exists());
        let _ = std::fs::remove_dir_all(root);
    }
}
