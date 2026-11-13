use crate::curseforge::{self, CurseForge};
use crate::download::{download_all, pack_host_allowed, safe_join, Job, Progress};
use crate::instance::{self, Instance, Loader};
use crate::paths::Dirs;
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

const MAX_ENTRIES: usize = 50_000;
const MAX_UNPACKED: u64 = 4 * 1024 * 1024 * 1024;
const MAX_INDEX: u64 = 16 * 1024 * 1024;

fn read_entry(path: &Path, name: &str) -> Result<Vec<u8>> {
    let mut z = zip::ZipArchive::new(std::fs::File::open(path)?)?;
    let mut e = z.by_name(name).with_context(|| format!("{name} missing in archive"))?;
    if e.size() > MAX_INDEX {
        bail!("{name} too large");
    }
    let mut buf = Vec::new();
    (&mut e).take(MAX_INDEX).read_to_end(&mut buf)?;
    Ok(buf)
}

/// Extract `prefix/...` entries into `dest`, with zip-slip, count and size limits.
fn extract_prefix(path: &Path, prefix: &str, dest: &Path) -> Result<u64> {
    let mut z = zip::ZipArchive::new(std::fs::File::open(path)?)?;
    if z.len() > MAX_ENTRIES {
        bail!("archive has too many entries");
    }
    let mut total = 0u64;
    for i in 0..z.len() {
        let mut e = z.by_index(i)?;
        let name = e.name()?.replace('\\', "/");
        let Some(rel) = name.strip_prefix(prefix).and_then(|r| r.strip_prefix('/')) else { continue };
        if rel.is_empty() || e.is_dir() {
            continue;
        }
        let out = safe_join(dest, rel)?;
        total = total.saturating_add(e.size());
        if total > MAX_UNPACKED {
            bail!("archive too large when unpacked");
        }
        if let Some(p) = out.parent() {
            std::fs::create_dir_all(p)?;
        }
        let mut f = std::fs::File::create(&out)?;
        std::io::copy(&mut (&mut e).take(MAX_UNPACKED), &mut f)?;
    }
    Ok(total)
}

#[derive(Deserialize)]
struct MrIndex {
    name: String,
    #[serde(default)]
    files: Vec<MrFile>,
    dependencies: HashMap<String, String>,
}

#[derive(Deserialize)]
struct MrFile {
    path: String,
    hashes: HashMap<String, String>,
    #[serde(default)]
    env: Option<HashMap<String, String>>,
    downloads: Vec<String>,
    #[serde(rename = "fileSize")]
    file_size: Option<u64>,
}

fn loader_from_deps(deps: &HashMap<String, String>) -> (Loader, Option<String>) {
    for (key, l) in [("fabric-loader", Loader::Fabric), ("quilt-loader", Loader::Quilt), ("neoforge", Loader::Neoforge), ("forge", Loader::Forge)] {
        if let Some(v) = deps.get(key) {
            return (l, Some(v.clone()));
        }
    }
    (Loader::Vanilla, None)
}

pub async fn import_mrpack(
    dirs: &Dirs,
    client: &reqwest::Client,
    file: &Path,
    concurrency: usize,
    on_progress: impl Fn(Progress) + Send + Sync,
) -> Result<Instance> {
    let f = file.to_path_buf();
    let raw = tokio::task::spawn_blocking(move || read_entry(&f, "modrinth.index.json")).await??;
    let idx: MrIndex = serde_json::from_slice(&raw).context("invalid modrinth.index.json")?;
    let mc = idx.dependencies.get("minecraft").context("pack has no minecraft version")?.clone();
    let (loader, lv) = loader_from_deps(&idx.dependencies);
    let inst = instance::create(dirs, &idx.name, &mc, loader, lv, Some("mrpack".into())).await?;
    let game = instance::game_dir(dirs, &inst.id);

    let res: Result<()> = async {
        let mut jobs = Vec::new();
        for f in &idx.files {
            if f.env.as_ref().and_then(|e| e.get("client")).is_some_and(|c| c == "unsupported") {
                continue;
            }
            let url = f.downloads.iter().find(|u| pack_host_allowed(u)).with_context(|| format!("no allowed download host for {}", f.path))?;
            let sha1 = f.hashes.get("sha1").context("file without sha1")?.clone();
            jobs.push(Job { url: url.clone(), dest: safe_join(&game, &f.path)?, sha1: Some(sha1), size: f.file_size });
        }
        let (a, g) = (file.to_path_buf(), game.clone());
        tokio::task::spawn_blocking(move || -> Result<()> {
            extract_prefix(&a, "overrides", &g)?;
            extract_prefix(&a, "client-overrides", &g)?;
            Ok(())
        })
        .await??;
        download_all(client, jobs, concurrency, on_progress).await
    }
    .await;
    if let Err(e) = res {
        let _ = instance::delete(dirs, &inst.id).await;
        return Err(e);
    }
    Ok(inst)
}

#[derive(Deserialize)]
struct CfManifest {
    minecraft: CfMc,
    name: String,
    #[serde(default)]
    files: Vec<CfFile>,
    #[serde(default = "default_overrides")]
    overrides: String,
}
fn default_overrides() -> String { "overrides".into() }

#[derive(Deserialize)]
struct CfMc {
    version: String,
    #[serde(rename = "modLoaders", default)]
    mod_loaders: Vec<CfLoader>,
}
#[derive(Deserialize)]
struct CfLoader {
    id: String,
    #[serde(default)]
    primary: bool,
}
#[derive(Deserialize)]
struct CfFile {
    #[serde(rename = "projectID")]
    project_id: u32,
    #[serde(rename = "fileID")]
    file_id: u32,
    #[serde(default = "yes")]
    required: bool,
}
fn yes() -> bool { true }

/// Files CurseForge does not let third-party launchers download.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ManualFile {
    pub name: String,
    pub project_id: u32,
    /// Where the file belongs inside the game folder (mods, resourcepacks, shaderpacks).
    pub folder: String,
}

fn missing_path(dirs: &Dirs, id: &str) -> std::path::PathBuf {
    dirs.instance(id).join("missing.json")
}

async fn write_missing(dirs: &Dirs, id: &str, list: &[ManualFile]) {
    let p = missing_path(dirs, id);
    if list.is_empty() {
        let _ = tokio::fs::remove_file(p).await;
    } else if let Ok(b) = serde_json::to_vec(list) {
        let _ = tokio::fs::write(p, b).await;
    }
}

/// Files of the modpack that still have to be added by hand. A file drops off the list as soon as it is in its folder.
pub async fn missing(dirs: &Dirs, id: &str) -> Vec<ManualFile> {
    if !crate::paths::valid_id(id) {
        return Vec::new();
    }
    let Ok(b) = tokio::fs::read(missing_path(dirs, id)).await else { return Vec::new() };
    let all: Vec<ManualFile> = serde_json::from_slice(&b).unwrap_or_default();
    let game = instance::game_dir(dirs, id);
    let mut still = Vec::new();
    for m in &all {
        let dir = game.join(&m.folder);
        let present = crate::paths::file_name_ok(&m.name) && (dir.join(&m.name).exists() || dir.join(format!("{}.disabled", m.name)).exists());
        if !present {
            still.push(m.clone());
        }
    }
    if still.len() != all.len() {
        write_missing(dirs, id, &still).await;
    }
    still
}

/// Stops listing a file as missing (the user does not want it).
pub async fn dismiss_missing(dirs: &Dirs, id: &str, name: &str) {
    let mut all = missing(dirs, id).await;
    all.retain(|m| m.name != name);
    write_missing(dirs, id, &all).await;
}

pub struct CfImport {
    pub instance: Instance,
    pub manual: Vec<ManualFile>,
}

pub async fn import_curseforge_zip(
    dirs: &Dirs,
    client: &reqwest::Client,
    cf: &CurseForge,
    file: &Path,
    concurrency: usize,
    on_progress: impl Fn(Progress) + Send + Sync,
) -> Result<CfImport> {
    let f = file.to_path_buf();
    let raw = tokio::task::spawn_blocking(move || read_entry(&f, "manifest.json")).await??;
    let m: CfManifest = serde_json::from_slice(&raw).context("invalid manifest.json")?;
    let primary = m.minecraft.mod_loaders.iter().find(|l| l.primary).or(m.minecraft.mod_loaders.first());
    let (loader, lv) = match primary.and_then(|l| l.id.split_once('-')) {
        Some(("fabric", v)) => (Loader::Fabric, Some(v.to_string())),
        Some(("quilt", v)) => (Loader::Quilt, Some(v.to_string())),
        Some(("neoforge", v)) => (Loader::Neoforge, Some(v.to_string())),
        Some(("forge", v)) => (Loader::Forge, Some(v.to_string())),
        _ => (Loader::Vanilla, None),
    };
    let inst = instance::create(dirs, &m.name, &m.minecraft.version, loader, lv, Some("curseforge".into())).await?;
    let game = instance::game_dir(dirs, &inst.id);

    let res: Result<Vec<ManualFile>> = async {
        let req: Vec<&CfFile> = m.files.iter().filter(|f| f.required).collect();
        let ids: Vec<u32> = req.iter().map(|f| f.file_id).collect();
        let proj: Vec<u32> = req.iter().map(|f| f.project_id).collect();
        let mut infos = Vec::new();
        for chunk in ids.chunks(200) {
            infos.extend(cf.files(client, chunk).await?);
        }
        let mut class_of: HashMap<u32, Option<u32>> = HashMap::new();
        for chunk in proj.chunks(200) {
            for md in cf.mods(client, chunk).await? {
                class_of.insert(md.id, md.class_id);
            }
        }
        let (mut jobs, mut manual) = (Vec::new(), Vec::new());
        for fi in infos {
            let folder = curseforge::folder_for_class(class_of.get(&fi.mod_id).copied().flatten());
            match &fi.download_url {
                Some(u) if pack_host_allowed(u) => {
                    let dest = safe_join(&game.join(folder), &fi.file_name)?;
                    jobs.push(Job { url: u.clone(), dest, sha1: fi.sha1(), size: Some(fi.file_length) });
                }
                _ => manual.push(ManualFile { name: fi.file_name.clone(), project_id: fi.mod_id, folder: folder.to_string() }),
            }
        }
        let (a, g, ov) = (file.to_path_buf(), game.clone(), m.overrides.clone());
        tokio::task::spawn_blocking(move || extract_prefix(&a, &ov, &g)).await??;
        download_all(client, jobs, concurrency, on_progress).await?;
        Ok(manual)
    }
    .await;
    match res {
        Ok(manual) => {
            write_missing(dirs, &inst.id, &manual).await;
            Ok(CfImport { instance: inst, manual })
        }
        Err(e) => {
            let _ = instance::delete(dirs, &inst.id).await;
            Err(e)
        }
    }
}

const MAX_PACK_DOWNLOAD: u64 = 512 * 1024 * 1024;

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct PackEntry {
    pub title: String,
    pub icon: Option<String>,
    /// "mod", "resourcepack", "shader" or "other"
    pub kind: &'static str,
    pub file: String,
    /// Project id on the platform the pack comes from, when the file could be matched.
    pub project_id: Option<String>,
}

fn kind_of_folder(path: &str) -> &'static str {
    match path.split('/').next() {
        Some("mods") => "mod",
        Some("resourcepacks") => "resourcepack",
        Some("shaderpacks") => "shader",
        _ => "other",
    }
}

fn file_title(path: &str) -> String {
    let name = path.rsplit('/').next().unwrap_or(path);
    name.strip_suffix(".jar").or_else(|| name.strip_suffix(".zip")).unwrap_or(name).to_string()
}

fn mr_files(raw: &[u8]) -> Result<Vec<(String, Option<String>)>> {
    let idx: MrIndex = serde_json::from_slice(raw).context("invalid modrinth.index.json")?;
    Ok(idx
        .files
        .into_iter()
        .filter(|f| f.env.as_ref().and_then(|e| e.get("client")).is_none_or(|c| c != "unsupported"))
        .map(|f| (f.path, f.hashes.get("sha1").cloned()))
        .collect())
}

fn cf_project_ids(raw: &[u8]) -> Result<Vec<u32>> {
    let m: CfManifest = serde_json::from_slice(raw).context("invalid manifest.json")?;
    let mut ids: Vec<u32> = m.files.iter().filter(|f| f.required).map(|f| f.project_id).collect();
    ids.sort_unstable();
    ids.dedup();
    Ok(ids)
}

fn sorted(mut v: Vec<PackEntry>) -> Vec<PackEntry> {
    v.sort_by(|a, b| a.kind.cmp(b.kind).then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase())));
    v
}

async fn fetch_pack(client: &reqwest::Client, dirs: &Dirs, name: String, job: Job) -> Result<std::path::PathBuf> {
    let dest = dirs.cache().join(name);
    crate::download::download_one(client, &Job { dest: dest.clone(), ..job }).await?;
    Ok(dest)
}

/// Downloads the newest version of a Modrinth modpack and lists what it installs.
pub async fn contents_modrinth(dirs: &Dirs, client: &reqwest::Client, project: &str) -> Result<Vec<PackEntry>> {
    let versions = crate::modrinth::versions(client, project).await?;
    let v = crate::modrinth::pick_version(&versions).context("no version")?;
    let f = crate::modrinth::primary_file(v).context("version has no file")?;
    if !pack_host_allowed(&f.url) || !f.filename.ends_with(".mrpack") || f.size > MAX_PACK_DOWNLOAD || !crate::paths::asset_id_ok(&v.id) {
        bail!("unexpected pack file");
    }
    let job = Job { url: f.url.clone(), dest: Default::default(), sha1: f.hashes.sha1.clone(), size: Some(f.size) };
    let path = fetch_pack(client, dirs, format!("list-{}.mrpack", v.id), job).await?;
    let p = path.clone();
    let raw = tokio::task::spawn_blocking(move || read_entry(&p, "modrinth.index.json")).await?;
    let _ = tokio::fs::remove_file(&path).await;
    let files = mr_files(&raw?)?;
    let hashes: Vec<String> = files.iter().filter_map(|(_, h)| h.clone()).collect();
    let mut project_of = HashMap::new();
    for chunk in hashes.chunks(400) {
        project_of.extend(crate::modrinth::projects_by_hash(client, chunk).await?);
    }
    let ids: Vec<String> = project_of.values().cloned().collect::<std::collections::BTreeSet<_>>().into_iter().collect();
    let infos: HashMap<String, crate::modrinth::Project> = crate::modrinth::projects(client, &ids).await?.into_iter().map(|p| (p.id.clone(), p)).collect();
    Ok(sorted(
        files
            .into_iter()
            .map(|(path, hash)| {
                let info = hash.and_then(|h| project_of.get(&h)).and_then(|id| infos.get(id));
                PackEntry { title: info.map_or_else(|| file_title(&path), |p| p.title.clone()), icon: info.and_then(|p| p.icon_url.clone()), kind: kind_of_folder(&path), file: path, project_id: info.map(|p| p.id.clone()) }
            })
            .collect(),
    ))
}

/// Downloads the newest file of a CurseForge modpack and lists the projects in its manifest.
pub async fn contents_curseforge(dirs: &Dirs, client: &reqwest::Client, cf: &CurseForge, project: u32) -> Result<Vec<PackEntry>> {
    let files = cf.mod_files(client, project).await?;
    let f = curseforge::pick_file(&files).context("no version")?;
    let url = f.download_url.clone().context("The author does not allow downloads outside curseforge.com")?;
    if !pack_host_allowed(&url) || !f.file_name.ends_with(".zip") || f.file_length > MAX_PACK_DOWNLOAD {
        bail!("unexpected pack file");
    }
    let job = Job { url, dest: Default::default(), sha1: f.sha1(), size: Some(f.file_length) };
    let path = fetch_pack(client, dirs, format!("list-cf-{}.zip", f.id), job).await?;
    let p = path.clone();
    let raw = tokio::task::spawn_blocking(move || read_entry(&p, "manifest.json")).await?;
    let _ = tokio::fs::remove_file(&path).await;
    let ids = cf_project_ids(&raw?)?;
    let mut out = Vec::with_capacity(ids.len());
    for chunk in ids.chunks(200) {
        for m in cf.mods(client, chunk).await? {
            let kind = match curseforge::folder_for_class(m.class_id) {
                "resourcepacks" => "resourcepack",
                "shaderpacks" => "shader",
                "datapacks" => "other",
                _ => "mod",
            };
            out.push(PackEntry { title: m.name, icon: m.logo.map(|l| l.thumbnail_url), kind, file: String::new(), project_id: Some(m.id.to_string()) });
        }
    }
    Ok(sorted(out))
}

/// Decide by content, not by extension, which pack format a file is.
pub fn detect(file: &Path) -> Result<&'static str> {
    let mut z = zip::ZipArchive::new(std::fs::File::open(file)?)?;
    if z.by_name("modrinth.index.json").is_ok() {
        return Ok("mrpack");
    }
    if z.by_name("manifest.json").is_ok() {
        return Ok("curseforge");
    }
    bail!("not a Modrinth or CurseForge modpack")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn make_zip(entries: &[(&str, &[u8])]) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("gn-{}.zip", uuid::Uuid::new_v4()));
        let mut w = zip::ZipWriter::new(std::fs::File::create(&p).unwrap());
        for (n, d) in entries {
            w.start_file(*n, zip::write::SimpleFileOptions::default()).unwrap();
            w.write_all(d).unwrap();
        }
        w.finish().unwrap();
        p
    }

    #[test]
    fn detects_formats() {
        let a = make_zip(&[("modrinth.index.json", b"{}")]);
        let b = make_zip(&[("manifest.json", b"{}")]);
        let c = make_zip(&[("x.txt", b"")]);
        assert_eq!(detect(&a).unwrap(), "mrpack");
        assert_eq!(detect(&b).unwrap(), "curseforge");
        assert!(detect(&c).is_err());
    }

    #[test]
    fn zip_slip_is_rejected() {
        let z = make_zip(&[("overrides/../../evil.txt", b"x")]);
        let dest = std::env::temp_dir().join(format!("gn-{}", uuid::Uuid::new_v4()));
        assert!(extract_prefix(&z, "overrides", &dest).is_err());
    }

    #[test]
    fn extracts_overrides() {
        let z = make_zip(&[("overrides/config/a.toml", b"hi"), ("other/b", b"no")]);
        let dest = std::env::temp_dir().join(format!("gn-{}", uuid::Uuid::new_v4()));
        extract_prefix(&z, "overrides", &dest).unwrap();
        assert_eq!(std::fs::read(dest.join("config/a.toml")).unwrap(), b"hi");
        assert!(!dest.join("other").exists());
    }

    #[test]
    fn loader_mapping() {
        let mut d = HashMap::new();
        d.insert("minecraft".to_string(), "1.20.1".to_string());
        d.insert("fabric-loader".to_string(), "0.15.0".to_string());
        assert_eq!(loader_from_deps(&d), (Loader::Fabric, Some("0.15.0".into())));
    }

    /// Minimal HTTP server that answers every request from a path -> JSON table.
    async fn mock_cf(routes: Vec<(&'static str, String)>) -> String {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = l.local_addr().unwrap();
        tokio::spawn(async move {
            loop {
                let Ok((mut sock, _)) = l.accept().await else { return };
                let routes = routes.clone();
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 8192];
                    let n = sock.read(&mut buf).await.unwrap_or(0);
                    let head = String::from_utf8_lossy(&buf[..n]).to_string();
                    let line = head.lines().next().unwrap_or("");
                    let key_ok = head.to_ascii_lowercase().contains("x-api-key: k3y");
                    let body = routes.iter().find(|(p, _)| line.contains(p)).map(|(_, b)| b.clone());
                    let (code, body) = match (key_ok, body) {
                        (false, _) => ("401 Unauthorized", "{}".to_string()),
                        (true, Some(b)) => ("200 OK", b),
                        (true, None) => ("404 Not Found", "{}".to_string()),
                    };
                    let resp = format!("HTTP/1.1 {code}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
                    let _ = sock.write_all(resp.as_bytes()).await;
                });
            }
        });
        format!("http://{addr}/v1")
    }

    #[tokio::test]
    async fn curseforge_search_and_import_against_mock() {
        let search = r#"{"data":[{"id":7,"name":"Pack","summary":"s","downloadCount":1234.0,"logo":{"thumbnailUrl":"https://media.forgecdn.net/x.png"},"classId":4471}]}"#;
        let files = r#"{"data":[
            {"id":11,"modId":100,"fileName":"blocked.jar","downloadUrl":null,"fileLength":5,"hashes":[],"gameVersions":["1.20.1","Forge"]},
            {"id":12,"modId":101,"fileName":"evil.jar","downloadUrl":"https://evil.example/x.jar","fileLength":5,"hashes":[],"gameVersions":[]}]}"#;
        let mods = r#"{"data":[{"id":100,"name":"A","summary":"","downloadCount":1.0,"classId":6},{"id":101,"name":"B","summary":"","downloadCount":1.0,"classId":6}]}"#;
        let base = mock_cf(vec![("/mods/search", search.into()), ("POST /v1/mods/files", files.into()), ("POST /v1/mods ", mods.into())]).await;
        let cf = CurseForge { base, key: "k3y".into() };
        let client = reqwest::Client::new();

        let hits = cf.search_modpacks(&client, "pack", 0, 20).await.unwrap();
        assert_eq!((hits[0].id, hits[0].name.as_str()), (7, "Pack"));
        let wrong_key = CurseForge { base: cf.base.clone(), key: "nope".into() };
        assert!(wrong_key.search_modpacks(&client, "pack", 0, 20).await.is_err());

        let manifest = br#"{"minecraft":{"version":"1.20.1","modLoaders":[{"id":"forge-47.3.0","primary":true}]},"name":"Mock Pack","overrides":"overrides",
            "files":[{"projectID":100,"fileID":11,"required":true},{"projectID":101,"fileID":12,"required":true}]}"#;
        let zip = make_zip(&[("manifest.json", manifest), ("overrides/config/a.toml", b"x=1")]);
        let root = std::env::temp_dir().join(format!("gn-cf-{}", uuid::Uuid::new_v4()));
        let dirs = Dirs::new(&root);
        dirs.ensure().await.unwrap();
        let r = import_curseforge_zip(&dirs, &client, &cf, &zip, 4, |_| {}).await.unwrap();
        assert_eq!(r.instance.name, "Mock Pack");
        assert_eq!(r.instance.loader, Loader::Forge);
        let mut names: Vec<_> = r.manual.iter().map(|m| m.name.as_str()).collect();
        names.sort();
        assert_eq!(names, ["blocked.jar", "evil.jar"], "null and foreign-host downloads must go to the manual list");
        assert!(instance::game_dir(&dirs, &r.instance.id).join("config/a.toml").is_file());
        let _ = std::fs::remove_file(zip);
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    #[ignore = "needs CURSEFORGE_API_KEY and talks to the live CurseForge API"]
    async fn live_curseforge_search_pack_and_manifest() {
        let key = std::env::var("CURSEFORGE_API_KEY").expect("CURSEFORGE_API_KEY");
        let cf = CurseForge::new(key);
        let client = crate::http_client();
        let hits = cf.search_modpacks(&client, "all the mods", 0, 5).await.unwrap();
        assert!(!hits.is_empty(), "search returned nothing");
        let mut zip = None;
        for hit in &hits {
            let files = cf.mod_files(&client, hit.id).await.unwrap();
            if let Some(f) = files.into_iter().find(|f| f.download_url.as_deref().is_some_and(pack_host_allowed) && f.file_name.ends_with(".zip") && f.file_length < 20_000_000) {
                zip = Some(f);
                break;
            }
        }
        let f = zip.expect("a downloadable pack zip among the first hits");
        let dest = std::env::temp_dir().join(format!("gn-cf-live-{}.zip", uuid::Uuid::new_v4()));
        let job = Job { url: f.download_url.clone().unwrap(), dest: dest.clone(), sha1: f.sha1(), size: Some(f.file_length) };
        crate::download::download_one(&client, &job).await.unwrap();
        assert_eq!(detect(&dest).unwrap(), "curseforge");
        let raw = read_entry(&dest, "manifest.json").unwrap();
        let m: CfManifest = serde_json::from_slice(&raw).unwrap();
        assert!(!m.files.is_empty());
        let ids: Vec<u32> = m.files.iter().take(8).map(|f| f.file_id).collect();
        let projects: Vec<u32> = m.files.iter().take(8).map(|f| f.project_id).collect();
        let infos = cf.files(&client, &ids).await.unwrap();
        let mods = cf.mods(&client, &projects).await.unwrap();
        assert!(!infos.is_empty() && !mods.is_empty());
        let direct = infos.iter().filter(|i| i.download_url.as_deref().is_some_and(pack_host_allowed)).count();
        println!("pack={} files={} sampled_infos={} sampled_mods={} direct_downloads={}", m.name, m.files.len(), infos.len(), mods.len(), direct);
        let _ = std::fs::remove_file(dest);
    }

    #[tokio::test]
    #[ignore = "needs CURSEFORGE_API_KEY and downloads a small real modpack"]
    async fn live_curseforge_import_small_pack() {
        let key = std::env::var("CURSEFORGE_API_KEY").expect("CURSEFORGE_API_KEY");
        let cf = CurseForge::new(key);
        let client = crate::http_client();
        let root = std::env::temp_dir().join(format!("gn-cf-import-{}", uuid::Uuid::new_v4()));
        let dirs = Dirs::new(&root);
        dirs.ensure().await.unwrap();
        let mut chosen = None;
        'search: for query in ["lite", "starter", "small", "vanilla"] {
            for hit in cf.search_modpacks(&client, query, 0, 20).await.unwrap() {
                let Ok(files) = cf.mod_files(&client, hit.id).await else { continue };
                let Some(f) = files.into_iter().find(|f| f.download_url.as_deref().is_some_and(pack_host_allowed) && f.file_name.ends_with(".zip") && f.file_length < 3_000_000) else { continue };
                let dest = dirs.cache().join(format!("{}.zip", f.id));
                let job = Job { url: f.download_url.clone().unwrap(), dest: dest.clone(), sha1: f.sha1(), size: Some(f.file_length) };
                crate::download::download_one(&client, &job).await.unwrap();
                let Ok(raw) = read_entry(&dest, "manifest.json") else { continue };
                let Ok(m) = serde_json::from_slice::<CfManifest>(&raw) else { continue };
                if (1..=12).contains(&m.files.len()) {
                    chosen = Some((dest, m.name, m.files.len()));
                    break 'search;
                }
            }
        }
        let (zip, name, expected) = chosen.expect("a small CurseForge pack");
        let r = import_curseforge_zip(&dirs, &client, &cf, &zip, 4, |_| {}).await.unwrap();
        let game = instance::game_dir(&dirs, &r.instance.id);
        let mut downloaded = 0;
        for folder in ["mods", "resourcepacks", "shaderpacks", "datapacks"] {
            if let Ok(rd) = std::fs::read_dir(game.join(folder)) {
                downloaded += rd.flatten().filter(|e| e.path().extension().is_some_and(|x| x == "jar" || x == "zip")).count();
            }
        }
        println!("pack={name} manifest_files={expected} downloaded={downloaded} manual={} loader={:?}", r.manual.len(), r.instance.loader);
        assert_eq!(downloaded + r.manual.len(), expected, "every manifest file is either downloaded or listed for manual download");
        let _ = std::fs::remove_dir_all(root);
    }
}

#[cfg(test)]
mod contents_tests {
    #[tokio::test]
    async fn missing_files_leave_the_list_once_they_are_added() {
        let root = std::env::temp_dir().join(format!("gn-missing-{}", uuid::Uuid::new_v4()));
        let dirs = Dirs::new(&root);
        let inst = instance::create(&dirs, "P", "1.21.1", Loader::Fabric, None, None).await.unwrap();
        let list = vec![
            ManualFile { name: "a.jar".into(), project_id: 1, folder: "mods".into() },
            ManualFile { name: "b.jar".into(), project_id: 2, folder: "mods".into() },
            ManualFile { name: "../c.jar".into(), project_id: 3, folder: "mods".into() },
        ];
        write_missing(&dirs, &inst.id, &list).await;
        assert_eq!(missing(&dirs, &inst.id).await.len(), 3);
        let mods = instance::game_dir(&dirs, &inst.id).join("mods");
        std::fs::create_dir_all(&mods).unwrap();
        std::fs::write(mods.join("a.jar"), b"x").unwrap();
        std::fs::write(mods.join("b.jar.disabled"), b"x").unwrap();
        let left = missing(&dirs, &inst.id).await;
        assert_eq!(left.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(), ["../c.jar"], "a bad name is never treated as present");
        dismiss_missing(&dirs, &inst.id, "../c.jar").await;
        assert!(missing(&dirs, &inst.id).await.is_empty());
        assert!(!missing_path(&dirs, &inst.id).exists());
        let _ = std::fs::remove_dir_all(root);
    }

    use super::*;

    #[test]
    fn folders_map_to_kinds() {
        assert_eq!(kind_of_folder("mods/sodium.jar"), "mod");
        assert_eq!(kind_of_folder("resourcepacks/x.zip"), "resourcepack");
        assert_eq!(kind_of_folder("shaderpacks/x.zip"), "shader");
        assert_eq!(kind_of_folder("config/x.toml"), "other");
        assert_eq!(file_title("mods/sodium-1.0.jar"), "sodium-1.0");
    }

    #[test]
    fn mrpack_index_lists_client_files() {
        let raw = br#"{"name":"p","dependencies":{"minecraft":"1.21"},"files":[
            {"path":"mods/a.jar","hashes":{"sha1":"aa"},"downloads":["https://cdn.modrinth.com/a"]},
            {"path":"mods/server.jar","hashes":{"sha1":"bb"},"env":{"client":"unsupported"},"downloads":["https://cdn.modrinth.com/b"]}]}"#;
        assert_eq!(mr_files(raw).unwrap(), [("mods/a.jar".to_string(), Some("aa".to_string()))]);
    }

    #[test]
    fn curseforge_manifest_lists_required_projects_once() {
        let raw = br#"{"minecraft":{"version":"1.20.1","modLoaders":[]},"name":"p","files":[
            {"projectID":5,"fileID":1},{"projectID":5,"fileID":2},{"projectID":3,"fileID":9,"required":false},{"projectID":2,"fileID":4}]}"#;
        assert_eq!(cf_project_ids(raw).unwrap(), [2, 5]);
    }

    #[test]
    fn entries_sort_by_kind_then_name() {
        let e = |t: &str, k| PackEntry { title: t.into(), icon: None, kind: k, file: String::new(), project_id: None };
        let v = sorted(vec![e("b", "mod"), e("Z", "resourcepack"), e("A", "mod")]);
        assert_eq!(v.iter().map(|x| x.title.as_str()).collect::<Vec<_>>(), ["A", "b", "Z"]);
    }

    #[tokio::test]
    #[ignore = "talks to the live Modrinth API"]
    async fn live_modrinth_pack_contents() {
        let root = std::env::temp_dir().join(format!("gn-contents-{}", uuid::Uuid::new_v4()));
        let dirs = Dirs::new(&root);
        dirs.ensure().await.unwrap();
        let list = contents_modrinth(&dirs, &crate::http_client(), "fabulously-optimized").await.unwrap();
        println!("{} entries, first: {:?}", list.len(), list.first());
        assert!(list.iter().any(|e| e.kind == "mod" && e.title.to_lowercase().contains("sodium")), "{list:?}");
        let _ = std::fs::remove_dir_all(root);
    }
}
