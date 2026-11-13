use crate::download::{download_all, safe_join, Job, Progress};
use crate::paths::Dirs;
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

pub const MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
const LIBRARIES_URL: &str = "https://libraries.minecraft.net/";
const ASSETS_URL: &str = "https://resources.download.minecraft.net/";

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct ManifestEntry {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub url: String,
    pub sha1: String,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Latest {
    pub release: String,
    pub snapshot: String,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Manifest {
    pub latest: Latest,
    pub versions: Vec<ManifestEntry>,
}

pub async fn manifest(client: &reqwest::Client) -> Result<Manifest> {
    Ok(client.get(MANIFEST_URL).send().await?.error_for_status()?.json().await?)
}

/// Which group a version belongs to in the version picker. Mojang's manifest only says "release" or "snapshot",
/// so April Fools' versions and experimental snapshots are recognised by their ids.
pub fn category(entry: &ManifestEntry) -> Option<&'static str> {
    const APRIL_FOOLS: [&str; 9] = ["15w14a", "3D Shareware v1.34", "1.RV-Pre1", "20w14infinite", "22w13oneblockatatime", "23w13a_or_b", "24w14potato", "25w14craftmine", "2.0"];
    match entry.kind.as_str() {
        "release" => Some("release"),
        "snapshot" if APRIL_FOOLS.contains(&entry.id.as_str()) => Some("aprilfools"),
        "snapshot" if entry.id.contains("experimental") || entry.id.contains("combat") => Some("experimental"),
        "snapshot" => Some("snapshot"),
        _ => None,
    }
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct VersionJson {
    pub id: String,
    pub inherits_from: Option<String>,
    pub main_class: String,
    pub minecraft_arguments: Option<String>,
    pub arguments: Option<Arguments>,
    pub libraries: Vec<Library>,
    pub asset_index: Option<AssetIndexRef>,
    pub assets: Option<String>,
    pub downloads: Option<Downloads>,
    pub java_version: Option<JavaVersion>,
    #[serde(rename = "type")]
    pub kind: String,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Arguments {
    pub game: Vec<Arg>,
    pub jvm: Vec<Arg>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum Arg {
    Plain(String),
    Ruled { rules: Vec<Rule>, value: OneOrMany },
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum OneOrMany {
    One(String),
    Many(Vec<String>),
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Rule {
    pub action: String,
    #[serde(default)]
    pub os: Option<OsRule>,
    #[serde(default)]
    pub features: Option<HashMap<String, bool>>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct OsRule {
    pub name: Option<String>,
    pub arch: Option<String>,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Library {
    pub name: String,
    pub downloads: Option<LibDownloads>,
    pub rules: Vec<Rule>,
    pub natives: Option<HashMap<String, String>>,
    pub url: Option<String>,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
#[serde(default)]
pub struct LibDownloads {
    pub artifact: Option<Artifact>,
    pub classifiers: Option<HashMap<String, Artifact>>,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Artifact {
    pub path: Option<String>,
    pub url: String,
    pub sha1: Option<String>,
    pub size: Option<u64>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AssetIndexRef {
    pub id: String,
    pub url: String,
    pub sha1: Option<String>,
    pub size: Option<u64>,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct Downloads {
    pub client: Option<Artifact>,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct JavaVersion {
    pub major_version: u32,
    #[serde(default)]
    pub component: Option<String>,
}

pub fn os_name() -> &'static str {
    if cfg!(target_os = "windows") { "windows" } else if cfg!(target_os = "macos") { "osx" } else { "linux" }
}

fn arch_name() -> &'static str {
    if cfg!(target_arch = "x86") { "x86" } else if cfg!(target_arch = "aarch64") { "arm64" } else { "x86_64" }
}

#[derive(Clone, Debug, Default)]
pub struct Features {
    pub has_custom_resolution: bool,
}

pub fn rules_allow(rules: &[Rule], f: &Features) -> bool {
    if rules.is_empty() {
        return true;
    }
    let mut allowed = false;
    for r in rules {
        let mut m = true;
        if let Some(os) = &r.os {
            if os.name.as_deref().is_some_and(|n| n != os_name()) {
                m = false;
            }
            if os.arch.as_deref().is_some_and(|a| a != arch_name()) {
                m = false;
            }
        }
        if let Some(feat) = &r.features {
            for (k, v) in feat {
                let have = match k.as_str() {
                    "has_custom_resolution" => f.has_custom_resolution,
                    _ => false,
                };
                if have != *v {
                    m = false;
                }
            }
        }
        if m {
            allowed = r.action == "allow";
        }
    }
    allowed
}

pub fn expand_args(args: &[Arg], f: &Features) -> Vec<String> {
    let mut out = Vec::new();
    for a in args {
        match a {
            Arg::Plain(s) => out.push(s.clone()),
            Arg::Ruled { rules, value } if rules_allow(rules, f) => match value {
                OneOrMany::One(s) => out.push(s.clone()),
                OneOrMany::Many(v) => out.extend(v.iter().cloned()),
            },
            _ => {}
        }
    }
    out
}

/// `group:artifact:version[:classifier][@ext]` -> maven relative path.
pub fn maven_path(name: &str) -> Result<String> {
    let (coords, ext) = name.split_once('@').unwrap_or((name, "jar"));
    let p: Vec<&str> = coords.split(':').collect();
    if p.len() < 3 {
        bail!("bad maven name {name}");
    }
    let file = match p.get(3) {
        Some(c) => format!("{}-{}-{}.{}", p[1], p[2], c, ext),
        None => format!("{}-{}.{}", p[1], p[2], ext),
    };
    Ok(format!("{}/{}/{}/{}", p[0].replace('.', "/"), p[1], p[2], file))
}

#[derive(Clone, Debug)]
pub struct LibFile {
    pub job: Job,
    pub on_classpath: bool,
    /// Needs to be unpacked into the natives directory.
    pub native: bool,
}

pub fn library_files(dirs: &Dirs, libs: &[Library], f: &Features) -> Result<Vec<LibFile>> {
    let mut out = Vec::new();
    for lib in libs {
        if !rules_allow(&lib.rules, f) {
            continue;
        }
        let lib_dir = dirs.libraries();
        let mk = |a: &Artifact, fallback_path: Option<String>| -> Result<Job> {
            let rel = a.path.clone().or(fallback_path).context("library without path")?;
            Ok(Job {
                url: a.url.clone(),
                dest: safe_join(&lib_dir, &rel)?,
                sha1: a.sha1.clone(),
                size: a.size,
            })
        };
        // legacy natives: classifier chosen by os
        if let Some(natives) = &lib.natives {
            if let Some(cls) = natives.get(os_name()) {
                let cls = cls.replace("${arch}", if cfg!(target_pointer_width = "32") { "32" } else { "64" });
                if let Some(a) = lib.downloads.as_ref().and_then(|d| d.classifiers.as_ref()).and_then(|c| c.get(&cls)) {
                    out.push(LibFile { job: mk(a, None)?, on_classpath: false, native: true });
                }
            }
            if lib.downloads.as_ref().and_then(|d| d.artifact.as_ref()).is_none() {
                continue;
            }
        }
        let job = if let Some(a) = lib.downloads.as_ref().and_then(|d| d.artifact.as_ref()) { mk(a, None)? } else {
            // Fabric/Quilt style: only a name and maybe a maven base url.
            let rel = maven_path(&lib.name)?;
            let base = lib.url.clone().unwrap_or_else(|| LIBRARIES_URL.into());
            let a = Artifact { path: Some(rel.clone()), url: format!("{base}{rel}"), sha1: None, size: None };
            mk(&a, None)?
        };
        let native = lib.name.split(':').nth(3).is_some_and(|c| c.starts_with("natives-"));
        out.push(LibFile { job, on_classpath: true, native });
    }
    Ok(out)
}

/// `group:artifact[:classifier]` without the version, so a child's library replaces the parent's.
fn lib_key(name: &str) -> String {
    let base = name.split('@').next().unwrap_or(name);
    let mut it = base.split(':');
    let (g, a) = (it.next().unwrap_or(""), it.next().unwrap_or(""));
    let rest: Vec<&str> = it.skip(1).collect();
    format!("{g}:{a}:{}", rest.join(":"))
}

fn merge(parent: VersionJson, child: VersionJson) -> VersionJson {
    let mut libs = child.libraries.clone();
    libs.extend(parent.libraries);
    let mut seen = std::collections::HashSet::new();
    libs.retain(|l| seen.insert(lib_key(&l.name)));
    let arguments = match (parent.arguments, child.arguments) {
        (Some(mut p), Some(c)) => {
            p.game.extend(c.game);
            p.jvm.extend(c.jvm);
            Some(p)
        }
        (p, c) => c.or(p),
    };
    VersionJson {
        id: child.id,
        inherits_from: None,
        main_class: if child.main_class.is_empty() { parent.main_class } else { child.main_class },
        minecraft_arguments: child.minecraft_arguments.or(parent.minecraft_arguments),
        arguments,
        libraries: libs,
        asset_index: child.asset_index.or(parent.asset_index),
        assets: child.assets.or(parent.assets),
        downloads: child.downloads.or(parent.downloads),
        java_version: child.java_version.or(parent.java_version),
        kind: if child.kind.is_empty() { parent.kind } else { child.kind },
    }
}

fn version_file(dirs: &Dirs, id: &str) -> Result<PathBuf> {
    if !crate::paths::file_name_ok(id) {
        bail!("bad version id");
    }
    Ok(dirs.versions().join(id).join(format!("{id}.json")))
}

/// Load a version json (downloading vanilla ones), resolving `inheritsFrom`.
pub async fn resolve_version(dirs: &Dirs, client: &reqwest::Client, id: &str) -> Result<VersionJson> {
    let mut chain = Vec::new();
    let mut cur = id.to_string();
    for _ in 0..5 {
        let path = version_file(dirs, &cur)?;
        if !path.exists() {
            let m = manifest(client).await?;
            let e = m.versions.iter().find(|v| v.id == cur).with_context(|| format!("unknown version {cur}"))?;
            let job = Job { url: e.url.clone(), dest: path.clone(), sha1: Some(e.sha1.clone()), size: None };
            crate::download::download_one(client, &job).await?;
        }
        let v: VersionJson = serde_json::from_slice(&tokio::fs::read(&path).await?)
            .with_context(|| format!("parse {}", path.display()))?;
        let next = v.inherits_from.clone();
        chain.push(v);
        match next {
            Some(n) => cur = n,
            None => break,
        }
    }
    let mut it = chain.into_iter().rev();
    let mut acc = it.next().context("empty chain")?;
    for child in it {
        acc = merge(acc, child);
    }
    Ok(acc)
}

#[derive(Deserialize)]
struct AssetIndex {
    objects: HashMap<String, AssetObject>,
    #[serde(default, rename = "virtual")]
    is_virtual: bool,
    #[serde(default)]
    map_to_resources: bool,
}

#[derive(Deserialize)]
struct AssetObject {
    hash: String,
    size: u64,
}

/// Download client jar, libraries and assets. Returns the file list needed for launching.
pub async fn install_version(
    dirs: &Dirs,
    client: &reqwest::Client,
    v: &VersionJson,
    game_dir: &std::path::Path,
    concurrency: usize,
    on_progress: impl Fn(&str, Progress) + Send + Sync,
) -> Result<Vec<LibFile>> {
    let base_id = v.id.clone();
    // client jar lives under the *vanilla* id; loader versions inherit it.
    let client_dl = v.downloads.as_ref().and_then(|d| d.client.clone()).context("no client download")?;
    let jar = client_jar(dirs, v)?;
    let _ = base_id;
    let jar_job = Job { url: client_dl.url, dest: jar, sha1: client_dl.sha1, size: client_dl.size };
    let libs = library_files(dirs, &v.libraries, &Features::default())?;

    let mut jobs: Vec<Job> = libs.iter().map(|l| l.job.clone()).collect();
    jobs.push(jar_job);
    // Forge/NeoForge list installer-built libraries without a url; the installer already wrote them.
    jobs.retain(|j| !j.url.is_empty());
    download_all(client, jobs, concurrency, |p| on_progress("libraries", p)).await?;

    if let Some(ai) = &v.asset_index {
        let idx_path = dirs.assets().join("indexes").join(format!("{}.json", crate::paths::asset_id_ok(&ai.id).then_some(ai.id.as_str()).context("bad asset id")?));
        crate::download::download_one(client, &Job { url: ai.url.clone(), dest: idx_path.clone(), sha1: ai.sha1.clone(), size: ai.size }).await?;
        let idx: AssetIndex = serde_json::from_slice(&tokio::fs::read(&idx_path).await?)?;
        let mut jobs = Vec::with_capacity(idx.objects.len());
        for o in idx.objects.values() {
            if o.hash.len() != 40 || !o.hash.bytes().all(|b| b.is_ascii_hexdigit()) {
                bail!("bad asset hash");
            }
            let sub = &o.hash[..2];
            jobs.push(Job {
                url: format!("{ASSETS_URL}{sub}/{}", o.hash),
                dest: dirs.assets().join("objects").join(sub).join(&o.hash),
                sha1: Some(o.hash.clone()),
                size: Some(o.size),
            });
        }
        download_all(client, jobs, concurrency, |p| on_progress("assets", p)).await?;
        if idx.is_virtual || idx.map_to_resources {
            let target = if idx.map_to_resources { game_dir.join("resources") } else { dirs.assets().join("virtual").join(&ai.id) };
            for (name, o) in &idx.objects {
                let dest = safe_join(&target, name)?;
                if dest.exists() {
                    continue;
                }
                if let Some(p) = dest.parent() {
                    tokio::fs::create_dir_all(p).await?;
                }
                let src = dirs.assets().join("objects").join(&o.hash[..2]).join(&o.hash);
                tokio::fs::copy(src, dest).await?;
            }
        }
    }
    Ok(libs)
}

/// The vanilla client jar for this (possibly loader-wrapped) version.
pub fn client_jar(dirs: &Dirs, v: &VersionJson) -> Result<PathBuf> {
    // After merging, `assets` name is not the mc version; use the version id of the root vanilla json.
    // The merged id is the leaf; the jar is always stored as versions/<leaf>/<leaf>.jar to keep it simple.
    if !crate::paths::file_name_ok(&v.id) {
        bail!("bad version id");
    }
    Ok(dirs.versions().join(&v.id).join(format!("{}.jar", v.id)))
}

pub async fn extract_natives(files: &[LibFile], natives_dir: &std::path::Path) -> Result<()> {
    tokio::fs::create_dir_all(natives_dir).await?;
    let jars: Vec<PathBuf> = files.iter().filter(|f| f.native).map(|f| f.job.dest.clone()).collect();
    let dir = natives_dir.to_path_buf();
    tokio::task::spawn_blocking(move || -> Result<()> {
        for jar in jars {
            let mut z = zip::ZipArchive::new(std::fs::File::open(&jar)?)?;
            for i in 0..z.len() {
                let mut e = z.by_index(i)?;
                let name = e.name()?.to_string();
                if e.is_dir() || name.starts_with("META-INF") {
                    continue;
                }
                let ext_ok = [".so", ".dll", ".dylib", ".jnilib"].iter().any(|x| name.ends_with(x));
                if !ext_ok {
                    continue;
                }
                let leaf = name.rsplit('/').next().unwrap_or(&name);
                let out = safe_join(&dir, leaf)?;
                let mut f = std::fs::File::create(out)?;
                std::io::copy(&mut e, &mut f)?;
            }
        }
        Ok(())
    })
    .await??;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn library_keys_ignore_version() {
        use super::lib_key;
        assert_eq!(lib_key("org.apache.commons:commons-lang3:3.14.0"), lib_key("org.apache.commons:commons-lang3:3.12.0"));
        assert_ne!(lib_key("org.lwjgl:lwjgl:3.3.3"), lib_key("org.lwjgl:lwjgl:3.3.3:natives-linux"));
        assert_eq!(lib_key("a:b:1@zip"), "a:b:");
    }

    use super::*;

    #[test]
    fn maven_paths() {
        assert_eq!(maven_path("net.fabricmc:fabric-loader:0.15.0").unwrap(), "net/fabricmc/fabric-loader/0.15.0/fabric-loader-0.15.0.jar");
        assert_eq!(maven_path("org.lwjgl:lwjgl:3.3.3:natives-linux").unwrap(), "org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3-natives-linux.jar");
        assert!(maven_path("bad").is_err());
    }

    #[test]
    fn rules_last_match_wins() {
        let r = |a: &str, os: Option<&str>| Rule {
            action: a.into(),
            os: os.map(|n| OsRule { name: Some(n.into()), arch: None }),
            features: None,
        };
        let f = Features::default();
        assert!(rules_allow(&[], &f));
        assert!(rules_allow(&[r("allow", None), r("disallow", Some("nonexistent"))], &f));
        assert!(!rules_allow(&[r("allow", Some("nonexistent"))], &f));
        assert!(!rules_allow(&[r("allow", None), r("disallow", Some(os_name()))], &f));
    }

    #[test]
    fn merge_prefers_child_main_class() {
        let p = VersionJson { id: "1.20.1".into(), main_class: "net.minecraft.client.main.Main".into(), ..Default::default() };
        let c = VersionJson { id: "fabric".into(), main_class: "net.fabricmc.loader.impl.launch.knot.KnotClient".into(), ..Default::default() };
        let m = merge(p, c);
        assert_eq!(m.id, "fabric");
        assert!(m.main_class.contains("Knot"));
    }
}

#[cfg(test)]
mod category_tests {
    use super::*;

    fn entry(id: &str, kind: &str) -> ManifestEntry {
        ManifestEntry { id: id.into(), kind: kind.into(), url: String::new(), sha1: String::new() }
    }

    #[test]
    fn versions_fall_into_their_groups() {
        assert_eq!(category(&entry("1.21.1", "release")), Some("release"));
        assert_eq!(category(&entry("24w14a", "snapshot")), Some("snapshot"));
        assert_eq!(category(&entry("1.21-pre1", "snapshot")), Some("snapshot"));
        assert_eq!(category(&entry("24w14potato", "snapshot")), Some("aprilfools"));
        assert_eq!(category(&entry("1.RV-Pre1", "snapshot")), Some("aprilfools"));
        assert_eq!(category(&entry("1.19_deep_dark_experimental_snapshot-1", "snapshot")), Some("experimental"));
        assert_eq!(category(&entry("1_15_combat-6", "snapshot")), Some("experimental"));
        assert_eq!(category(&entry("b1.7.3", "old_beta")), None);
    }
}
