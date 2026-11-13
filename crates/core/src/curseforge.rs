use crate::instance::Loader;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

const MINECRAFT: u32 = 432;
const CLASS_MODPACKS: u32 = 4471;

/// Without a key of your own the launcher talks to deldown's pass-through, which holds the shared key
/// server-side; a key entered in Settings goes to CurseForge directly.
pub const PROXY_BASE: &str = "https://api-geffeniuse.deldown.de/v1";
const OFFICIAL_BASE: &str = "https://api.curseforge.com/v1";

#[derive(Clone)]
pub struct CurseForge {
    pub base: String,
    pub key: String,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Mod {
    pub id: u32,
    pub name: String,
    pub summary: String,
    pub download_count: f64,
    pub logo: Option<Logo>,
    #[serde(default)]
    pub class_id: Option<u32>,
    pub links: Option<Links>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Logo {
    pub thumbnail_url: String,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Links {
    pub website_url: Option<String>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FileInfo {
    pub id: u32,
    pub mod_id: u32,
    pub file_name: String,
    pub download_url: Option<String>,
    pub file_length: u64,
    #[serde(default)]
    pub hashes: Vec<FileHash>,
    #[serde(default)]
    pub game_versions: Vec<String>,
    /// 1 release, 2 beta, 3 alpha
    #[serde(default)]
    pub release_type: u32,
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Dependency {
    pub mod_id: u32,
    /// 3 = required dependency
    pub relation_type: u32,
}

const MODS_CLASS: u32 = 6;
const REQUIRED: u32 = 3;

pub fn loader_type(l: Loader) -> Option<u32> {
    match l {
        Loader::Forge => Some(1),
        Loader::Fabric => Some(4),
        Loader::Quilt => Some(5),
        Loader::Neoforge => Some(6),
        Loader::Vanilla => None,
    }
}

/// Newest stable file, otherwise the newest of anything (higher file id = newer).
pub fn pick_file(files: &[FileInfo]) -> Option<&FileInfo> {
    files.iter().filter(|f| f.release_type == 1).max_by_key(|f| f.id).or_else(|| files.iter().max_by_key(|f| f.id))
}

impl FileInfo {
    pub fn required_deps(&self) -> impl Iterator<Item = u32> + '_ {
        self.dependencies.iter().filter(|d| d.relation_type == REQUIRED).map(|d| d.mod_id)
    }
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct FileHash {
    pub value: String,
    pub algo: u32, // 1 = sha1
}

impl FileInfo {
    pub fn sha1(&self) -> Option<String> {
        self.hashes.iter().find(|h| h.algo == 1).map(|h| h.value.clone())
    }
}

#[derive(Deserialize)]
struct Data<T> {
    data: T,
}

impl CurseForge {
    pub fn new(key: impl Into<String>) -> Self {
        let key = key.into().trim().to_string();
        let base = if key.is_empty() { PROXY_BASE } else { OFFICIAL_BASE };
        Self { base: base.into(), key }
    }

    fn with_key(&self, r: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if self.key.is_empty() { r } else { r.header("x-api-key", &self.key) }
    }

    fn get(&self, c: &reqwest::Client, path: &str) -> reqwest::RequestBuilder {
        self.with_key(c.get(format!("{}{}", self.base, path)))
    }

    pub async fn get_json(&self, c: &reqwest::Client, path: String) -> Result<serde_json::Value> {
        Ok(self.get(c, &path).send().await?.error_for_status()?.json().await?)
    }

    pub async fn search_modpacks(&self, c: &reqwest::Client, query: &str, index: u32, size: u32) -> Result<Vec<Mod>> {
        let r = self
            .get(c, "/mods/search")
            .query(&[
                ("gameId", MINECRAFT.to_string()),
                ("classId", CLASS_MODPACKS.to_string()),
                ("searchFilter", query.to_string()),
                ("index", index.to_string()),
                ("pageSize", size.clamp(1, 50).to_string()),
                ("sortField", "2".into()),
                ("sortOrder", "desc".into()),
            ])
            .send()
            .await?
            .error_for_status()?;
        Ok(r.json::<Data<Vec<Mod>>>().await?.data)
    }

    pub async fn search_mods(&self, c: &reqwest::Client, query: &str, mc: &str, loader: Loader, index: u32, size: u32) -> Result<Vec<Mod>> {
        let lt = loader_type(loader).context("Vanilla loads no mods")?;
        let r = self
            .get(c, "/mods/search")
            .query(&[
                ("gameId", MINECRAFT.to_string()),
                ("classId", MODS_CLASS.to_string()),
                ("gameVersion", mc.to_string()),
                ("modLoaderType", lt.to_string()),
                ("searchFilter", query.to_string()),
                ("index", index.to_string()),
                ("pageSize", size.clamp(1, 50).to_string()),
                ("sortField", "2".into()),
                ("sortOrder", "desc".into()),
            ])
            .send()
            .await?
            .error_for_status()?;
        Ok(r.json::<Data<Vec<Mod>>>().await?.data)
    }

    pub async fn compatible_files(&self, c: &reqwest::Client, mod_id: u32, mc: &str, loader: Loader) -> Result<Vec<FileInfo>> {
        let lt = loader_type(loader).context("Vanilla loads no mods")?;
        let r = self
            .get(c, &format!("/mods/{mod_id}/files"))
            .query(&[("gameVersion", mc.to_string()), ("modLoaderType", lt.to_string()), ("pageSize", "50".into())])
            .send()
            .await?
            .error_for_status()?;
        Ok(r.json::<Data<Vec<FileInfo>>>().await?.data)
    }

    pub async fn mod_files(&self, c: &reqwest::Client, mod_id: u32) -> Result<Vec<FileInfo>> {
        let r = self.get(c, &format!("/mods/{mod_id}/files")).query(&[("pageSize", "50")]).send().await?.error_for_status()?;
        Ok(r.json::<Data<Vec<FileInfo>>>().await?.data)
    }

    pub async fn files(&self, c: &reqwest::Client, ids: &[u32]) -> Result<Vec<FileInfo>> {
        let r = self
            .with_key(c.post(format!("{}/mods/files", self.base)))
            .json(&serde_json::json!({ "fileIds": ids }))
            .send()
            .await?
            .error_for_status()?;
        Ok(r.json::<Data<Vec<FileInfo>>>().await?.data)
    }

    pub async fn mods(&self, c: &reqwest::Client, ids: &[u32]) -> Result<Vec<Mod>> {
        let r = self
            .with_key(c.post(format!("{}/mods", self.base)))
            .json(&serde_json::json!({ "modIds": ids, "filterPcOnly": true }))
            .send()
            .await?
            .error_for_status()?;
        Ok(r.json::<Data<Vec<Mod>>>().await?.data)
    }
}

pub fn folder_for_class(class_id: Option<u32>) -> &'static str {
    match class_id {
        Some(12) => "resourcepacks",
        Some(6552) => "shaderpacks",
        Some(6945) => "datapacks",
        _ => "mods",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(id: u32, release_type: u32) -> FileInfo {
        FileInfo { id, mod_id: 1, file_name: format!("{id}.jar"), download_url: None, file_length: 1, hashes: vec![], game_versions: vec![], release_type, dependencies: vec![] }
    }

    #[test]
    fn loader_types() {
        assert_eq!(loader_type(Loader::Forge), Some(1));
        assert_eq!(loader_type(Loader::Fabric), Some(4));
        assert_eq!(loader_type(Loader::Quilt), Some(5));
        assert_eq!(loader_type(Loader::Neoforge), Some(6));
        assert_eq!(loader_type(Loader::Vanilla), None);
    }

    #[test]
    fn picks_newest_release_over_newer_beta() {
        let files = vec![f(10, 2), f(7, 1), f(5, 1)];
        assert_eq!(pick_file(&files).unwrap().id, 7);
        assert_eq!(pick_file(&[f(3, 2), f(9, 3)]).unwrap().id, 9);
        assert!(pick_file(&[]).is_none());
    }

    #[test]
    fn required_dependencies_only() {
        let mut x = f(1, 1);
        x.dependencies = vec![Dependency { mod_id: 5, relation_type: 3 }, Dependency { mod_id: 6, relation_type: 2 }];
        assert_eq!(x.required_deps().collect::<Vec<_>>(), [5]);
    }
}

#[cfg(test)]
mod proxy_tests {
    use super::*;

    #[test]
    fn no_key_uses_the_proxy_and_a_key_goes_straight_to_curseforge() {
        assert_eq!(CurseForge::new("").base, PROXY_BASE);
        assert_eq!(CurseForge::new("  ").base, PROXY_BASE);
        let own = CurseForge::new(" abc ");
        assert_eq!(own.base, OFFICIAL_BASE);
        assert_eq!(own.key, "abc");
    }
}
