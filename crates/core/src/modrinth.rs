use crate::instance::Loader;
use anyhow::{Context, bail, Result};
use serde::{Deserialize, Serialize};

const API: &str = "https://api.modrinth.com/v2";

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Hit {
    pub project_id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub icon_url: Option<String>,
    pub downloads: u64,
    pub author: String,
    #[serde(default)]
    pub categories: Vec<String>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct SearchResult {
    pub hits: Vec<Hit>,
    pub total_hits: u64,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct File {
    pub url: String,
    pub filename: String,
    pub primary: bool,
    pub size: u64,
    pub hashes: Hashes,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Hashes {
    pub sha1: Option<String>,
    #[serde(default)]
    pub sha512: Option<String>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Version {
    pub id: String,
    #[serde(default)]
    pub project_id: String,
    pub name: String,
    pub version_number: String,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub files: Vec<File>,
    pub date_published: String,
    #[serde(default)]
    pub version_type: String,
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Dependency {
    pub project_id: Option<String>,
    pub dependency_type: String,
}

#[derive(Deserialize, Debug)]
pub struct Project {
    #[serde(default)]
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub icon_url: Option<String>,
}

fn check_token(s: &str) -> Result<()> {
    if s.is_empty() || s.len() > 64 || !s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.') {
        bail!("invalid id");
    }
    Ok(())
}

pub async fn search_modpacks(client: &reqwest::Client, query: &str, offset: u32, limit: u32) -> Result<SearchResult> {
    let facets = r#"[["project_type:modpack"]]"#;
    let r = client
        .get(format!("{API}/search"))
        .query(&[
            ("query", query),
            ("facets", facets),
            ("limit", &limit.clamp(1, 50).to_string()),
            ("offset", &offset.to_string()),
        ])
        .send()
        .await?
        .error_for_status()?;
    Ok(r.json().await?)
}

pub async fn versions(client: &reqwest::Client, project: &str) -> Result<Vec<Version>> {
    check_token(project)?;
    Ok(client.get(format!("{API}/project/{project}/version")).send().await?.error_for_status()?.json().await?)
}

/// The `.mrpack` file of a version.
pub fn primary_file(v: &Version) -> Option<&File> {
    v.files.iter().find(|f| f.primary).or(v.files.first())
}

/// Modrinth category tags a mod must carry to run on `loader` (Quilt also runs Fabric mods).
pub fn loader_tags(loader: Loader) -> &'static [&'static str] {
    match loader {
        Loader::Vanilla => &[],
        Loader::Fabric => &["fabric"],
        Loader::Quilt => &["quilt", "fabric"],
        Loader::Forge => &["forge"],
        Loader::Neoforge => &["neoforge"],
    }
}

fn check_mc(s: &str) -> Result<()> {
    check_token(s)
}

pub fn mod_facets(mc: &str, loader: Loader) -> String {
    let mut f = vec![vec!["project_type:mod".to_string()], vec![format!("versions:{mc}")]];
    let tags: Vec<String> = loader_tags(loader).iter().map(|t| format!("categories:{t}")).collect();
    if !tags.is_empty() {
        f.push(tags);
    }
    serde_json::to_string(&f).unwrap_or_default()
}

pub fn content_type(kind: &str) -> Option<&'static str> {
    match kind {
        "mod" => Some("mod"),
        "resourcepack" => Some("resourcepack"),
        "shader" => Some("shader"),
        "datapack" => Some("datapack"),
        _ => None,
    }
}

pub async fn search_content(client: &reqwest::Client, query: &str, mc: &str, kind: &str, offset: u32, limit: u32) -> Result<SearchResult> {
    check_mc(mc)?;
    let ty = content_type(kind).context("unknown content type")?;
    let facets = serde_json::to_string(&[vec![format!("project_type:{ty}")], vec![format!("versions:{mc}")]])?;
    let r = client
        .get(format!("{API}/search"))
        .query(&[
            ("query", query),
            ("facets", facets.as_str()),
            ("limit", &limit.clamp(1, 50).to_string()),
            ("offset", &offset.to_string()),
        ])
        .send()
        .await?
        .error_for_status()?;
    Ok(r.json().await?)
}

/// Versions of a resource pack, shader or data pack for this Minecraft version (these have no mod loader).
pub async fn versions_for_game(client: &reqwest::Client, project: &str, mc: &str) -> Result<Vec<Version>> {
    check_token(project)?;
    check_mc(mc)?;
    let games = serde_json::to_string(&[mc])?;
    let r = client.get(format!("{API}/project/{project}/version")).query(&[("game_versions", games.as_str())]).send().await?.error_for_status()?;
    Ok(r.json().await?)
}

pub async fn search_mods(client: &reqwest::Client, query: &str, mc: &str, loader: Loader, offset: u32, limit: u32) -> Result<SearchResult> {
    check_mc(mc)?;
    let facets = mod_facets(mc, loader);
    let r = client
        .get(format!("{API}/search"))
        .query(&[
            ("query", query),
            ("facets", facets.as_str()),
            ("limit", &limit.clamp(1, 50).to_string()),
            ("offset", &offset.to_string()),
        ])
        .send()
        .await?
        .error_for_status()?;
    Ok(r.json().await?)
}

/// Versions of `project` that run on this Minecraft version and loader, newest first.
pub async fn compatible_versions(client: &reqwest::Client, project: &str, mc: &str, loader: Loader) -> Result<Vec<Version>> {
    check_token(project)?;
    check_mc(mc)?;
    let loaders = serde_json::to_string(loader_tags(loader))?;
    let games = serde_json::to_string(&[mc])?;
    let r = client
        .get(format!("{API}/project/{project}/version"))
        .query(&[("loaders", loaders.as_str()), ("game_versions", games.as_str())])
        .send()
        .await?
        .error_for_status()?;
    Ok(r.json().await?)
}

pub async fn project(client: &reqwest::Client, id: &str) -> Result<Project> {
    check_token(id)?;
    Ok(client.get(format!("{API}/project/{id}")).send().await?.error_for_status()?.json().await?)
}

/// The Modrinth version each local file (by SHA-1) belongs to.
pub async fn versions_by_hash(client: &reqwest::Client, hashes: &[String]) -> Result<std::collections::HashMap<String, Version>> {
    if hashes.is_empty() {
        return Ok(Default::default());
    }
    let r = client
        .post(format!("{API}/version_files"))
        .json(&serde_json::json!({ "hashes": hashes, "algorithm": "sha1" }))
        .send()
        .await?
        .error_for_status()?;
    Ok(r.json().await?)
}

pub async fn projects_by_hash(client: &reqwest::Client, hashes: &[String]) -> Result<std::collections::HashMap<String, String>> {
    let by_hash = versions_by_hash(client, hashes).await?;
    Ok(by_hash.into_iter().filter(|(_, v)| !v.project_id.is_empty()).map(|(h, v)| (h, v.project_id)).collect())
}

/// Newest version for this Minecraft version and loader of the project each local file (by SHA-1) belongs to.
pub async fn latest_by_hash(client: &reqwest::Client, hashes: &[String], mc: &str, loader: Loader) -> Result<std::collections::HashMap<String, Version>> {
    check_mc(mc)?;
    let mut out = std::collections::HashMap::new();
    for chunk in hashes.chunks(200) {
        let r = client
            .post(format!("{API}/version_files/update"))
            .json(&serde_json::json!({ "hashes": chunk, "algorithm": "sha1", "loaders": loader_tags(loader), "game_versions": [mc] }))
            .send()
            .await?
            .error_for_status()?;
        out.extend(r.json::<std::collections::HashMap<String, Version>>().await?);
    }
    Ok(out)
}

pub async fn projects(client: &reqwest::Client, ids: &[String]) -> Result<Vec<Project>> {
    let mut out = Vec::new();
    for chunk in ids.chunks(100) {
        for id in chunk {
            check_token(id)?;
        }
        let list = serde_json::to_string(chunk)?;
        let r = client.get(format!("{API}/projects")).query(&[("ids", list.as_str())]).send().await?.error_for_status()?;
        out.extend(r.json::<Vec<Project>>().await?);
    }
    Ok(out)
}

/// Prefer the newest stable release, otherwise the newest of anything.
pub fn pick_version(versions: &[Version]) -> Option<&Version> {
    versions.iter().find(|v| v.version_type == "release").or(versions.first())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(id: &str, kind: &str) -> Version {
        Version {
            id: id.into(),
            project_id: String::new(),
            name: id.into(),
            version_number: id.into(),
            game_versions: vec![],
            loaders: vec![],
            files: vec![],
            date_published: String::new(),
            version_type: kind.into(),
            dependencies: vec![],
        }
    }

    #[test]
    fn facets_per_loader() {
        assert_eq!(mod_facets("1.21.1", Loader::Neoforge), r#"[["project_type:mod"],["versions:1.21.1"],["categories:neoforge"]]"#);
        assert_eq!(mod_facets("1.20.1", Loader::Quilt), r#"[["project_type:mod"],["versions:1.20.1"],["categories:quilt","categories:fabric"]]"#);
        assert_eq!(mod_facets("1.20.1", Loader::Vanilla), r#"[["project_type:mod"],["versions:1.20.1"]]"#);
    }

    #[test]
    fn picks_release_over_newer_beta() {
        let list = vec![v("beta", "beta"), v("rel", "release"), v("old", "release")];
        assert_eq!(pick_version(&list).unwrap().id, "rel");
        let only_beta = vec![v("b1", "beta"), v("b2", "alpha")];
        assert_eq!(pick_version(&only_beta).unwrap().id, "b1");
        assert!(pick_version(&[]).is_none());
    }

    #[test]
    fn rejects_bad_tokens() {
        assert!(check_token("AANobbMI").is_ok());
        assert!(check_token("../x").is_err());
        assert!(check_token("").is_err());
    }
}
