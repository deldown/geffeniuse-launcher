use crate::curseforge::CurseForge;
use anyhow::{bail, Result};
use serde::Serialize;
use serde_json::Value;

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct Link {
    pub label: String,
    pub url: String,
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct GalleryItem {
    pub url: String,
    pub title: String,
}

/// Everything the project page shows, normalised across Modrinth and CurseForge.
#[derive(Serialize, Clone, Debug, Default)]
pub struct ProjectDetails {
    pub source: String,
    pub id: String,
    pub title: String,
    pub summary: String,
    pub icon: Option<String>,
    pub body: String,
    /// "markdown" (Modrinth) or "html" (CurseForge); the UI renders both without innerHTML.
    pub body_format: String,
    pub downloads: u64,
    pub followers: Option<u64>,
    pub categories: Vec<String>,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub gallery: Vec<GalleryItem>,
    pub links: Vec<Link>,
    pub license: Option<String>,
    pub updated: Option<String>,
    pub page_url: Option<String>,
}

fn https(u: &str) -> Option<String> {
    reqwest::Url::parse(u).ok().filter(|p| p.scheme() == "https" && u.len() <= 1000).map(|_| u.to_string())
}

/// Images are only shown from the two mod platforms' own CDNs.
pub fn image_host_ok(u: &str) -> bool {
    reqwest::Url::parse(u)
        .is_ok_and(|p| p.scheme() == "https" && matches!(p.host_str(), Some("cdn.modrinth.com" | "media.forgecdn.net" | "media.curseforge.com" | "edge.forgecdn.net")))
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default()
}

fn push_link(links: &mut Vec<Link>, label: &str, v: &Value) {
    if let Some(url) = v.as_str().and_then(https) {
        links.push(Link { label: label.into(), url });
    }
}

/// Newest first, numerically where possible ("1.21.10" after "1.21.9").
fn sort_versions(v: &mut Vec<String>) {
    let key = |s: &String| s.split('.').map(|p| p.parse::<u32>().unwrap_or(0)).collect::<Vec<_>>();
    v.sort_by_key(|s| std::cmp::Reverse(key(s)));
    v.dedup();
}

pub fn from_modrinth(p: &Value) -> ProjectDetails {
    let mut links = Vec::new();
    push_link(&mut links, "Fehler melden", &p["issues_url"]);
    push_link(&mut links, "Quellcode", &p["source_url"]);
    push_link(&mut links, "Wiki", &p["wiki_url"]);
    push_link(&mut links, "Discord", &p["discord_url"]);
    for d in p["donation_urls"].as_array().into_iter().flatten() {
        push_link(&mut links, "Spenden", &d["url"]);
    }
    let mut game_versions = strings(&p["game_versions"]);
    sort_versions(&mut game_versions);
    let slug = p["slug"].as_str().unwrap_or_default();
    let kind = p["project_type"].as_str().unwrap_or("modpack");
    ProjectDetails {
        source: "modrinth".into(),
        id: p["id"].as_str().unwrap_or_default().into(),
        title: p["title"].as_str().unwrap_or_default().into(),
        summary: p["description"].as_str().unwrap_or_default().into(),
        icon: p["icon_url"].as_str().filter(|u| image_host_ok(u)).map(String::from),
        body: p["body"].as_str().unwrap_or_default().into(),
        body_format: "markdown".into(),
        downloads: p["downloads"].as_u64().unwrap_or(0),
        followers: p["followers"].as_u64(),
        categories: strings(&p["categories"]),
        game_versions,
        loaders: strings(&p["loaders"]),
        gallery: p["gallery"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|g| Some(GalleryItem { url: g["url"].as_str().filter(|u| image_host_ok(u))?.into(), title: g["title"].as_str().unwrap_or_default().into() }))
            .collect(),
        links,
        license: p["license"]["name"].as_str().map(String::from),
        updated: p["updated"].as_str().map(String::from),
        page_url: https(&format!("https://modrinth.com/{kind}/{slug}")).filter(|_| !slug.is_empty()),
    }
}

fn cf_loader(n: u64) -> Option<&'static str> {
    match n {
        1 => Some("forge"),
        4 => Some("fabric"),
        5 => Some("quilt"),
        6 => Some("neoforge"),
        _ => None,
    }
}

pub fn from_curseforge(m: &Value, description_html: &str) -> ProjectDetails {
    let mut links = Vec::new();
    push_link(&mut links, "Fehler melden", &m["links"]["issuesUrl"]);
    push_link(&mut links, "Quellcode", &m["links"]["sourceUrl"]);
    push_link(&mut links, "Wiki", &m["links"]["wikiUrl"]);
    let mut game_versions = Vec::new();
    let mut loaders = Vec::new();
    for f in m["latestFilesIndexes"].as_array().into_iter().flatten() {
        if let Some(v) = f["gameVersion"].as_str().filter(|v| v.chars().next().is_some_and(|c| c.is_ascii_digit())) {
            game_versions.push(v.to_string());
        }
        if let Some(l) = f["modLoader"].as_u64().and_then(cf_loader) {
            if !loaders.iter().any(|x| x == l) {
                loaders.push(l.to_string());
            }
        }
    }
    sort_versions(&mut game_versions);
    ProjectDetails {
        source: "curseforge".into(),
        id: m["id"].as_u64().map(|i| i.to_string()).unwrap_or_default(),
        title: m["name"].as_str().unwrap_or_default().into(),
        summary: m["summary"].as_str().unwrap_or_default().into(),
        icon: m["logo"]["thumbnailUrl"].as_str().filter(|u| image_host_ok(u)).map(String::from),
        body: description_html.into(),
        body_format: "html".into(),
        downloads: m["downloadCount"].as_f64().unwrap_or(0.0) as u64,
        followers: None,
        categories: m["categories"].as_array().into_iter().flatten().filter_map(|c| c["name"].as_str().map(String::from)).collect(),
        game_versions,
        loaders,
        gallery: m["screenshots"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|g| Some(GalleryItem { url: g["url"].as_str().filter(|u| image_host_ok(u))?.into(), title: g["title"].as_str().unwrap_or_default().into() }))
            .collect(),
        links,
        license: None,
        updated: m["dateModified"].as_str().map(String::from),
        page_url: m["links"]["websiteUrl"].as_str().and_then(https),
    }
}

pub async fn fetch(client: &reqwest::Client, cf_key: &str, source: &str, id: &str) -> Result<ProjectDetails> {
    match source {
        "modrinth" => {
            if id.is_empty() || id.len() > 64 || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') {
                bail!("invalid id");
            }
            let v: Value = client.get(format!("https://api.modrinth.com/v2/project/{id}")).send().await?.error_for_status()?.json().await?;
            Ok(from_modrinth(&v))
        }
        "curseforge" => {
            let n: u32 = id.parse()?;
            let cf = CurseForge::new(cf_key);
            let get = |path: String| cf.get_json(client, path);
            let m = get(format!("/mods/{n}")).await?;
            let d = get(format!("/mods/{n}/description")).await.ok();
            Ok(from_curseforge(&m["data"], d.as_ref().and_then(|d| d["data"].as_str()).unwrap_or_default()))
        }
        _ => bail!("unknown source"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn maps_modrinth_project() {
        let p = json!({
            "id": "abc", "slug": "fo", "project_type": "modpack", "title": "Fabulously Optimized", "description": "Short.",
            "body": "# Hi", "icon_url": "https://cdn.modrinth.com/data/abc/icon.png", "downloads": 19600000, "followers": 5019,
            "categories": ["lightweight"], "loaders": ["fabric"], "game_versions": ["1.20.1", "1.21.1", "1.21.10", "1.21.9"],
            "gallery": [{ "url": "https://cdn.modrinth.com/g.png", "title": "A" }, { "url": "https://evil.example/x.png", "title": "B" }],
            "issues_url": "https://github.com/x/issues", "wiki_url": "javascript:alert(1)", "source_url": null,
            "donation_urls": [{ "platform": "Ko-fi", "url": "https://ko-fi.com/x" }], "license": { "name": "MIT" }
        });
        let d = from_modrinth(&p);
        assert_eq!((d.title.as_str(), d.body_format.as_str(), d.downloads, d.followers), ("Fabulously Optimized", "markdown", 19600000, Some(5019)));
        assert_eq!(d.game_versions, ["1.21.10", "1.21.9", "1.21.1", "1.20.1"]);
        assert_eq!(d.gallery.len(), 1, "foreign image hosts are dropped");
        assert_eq!(d.links.iter().map(|l| l.label.as_str()).collect::<Vec<_>>(), ["Fehler melden", "Spenden"], "non-https links are dropped");
        assert_eq!(d.page_url.as_deref(), Some("https://modrinth.com/modpack/fo"));
        assert_eq!(d.license.as_deref(), Some("MIT"));
    }

    #[test]
    fn maps_curseforge_project() {
        let m = json!({
            "id": 42, "name": "Pack", "summary": "S", "downloadCount": 1234.0, "logo": { "thumbnailUrl": "https://media.forgecdn.net/a.png" },
            "links": { "websiteUrl": "https://www.curseforge.com/minecraft/modpacks/pack", "issuesUrl": "http://insecure.example" },
            "categories": [{ "name": "Tech" }], "screenshots": [{ "url": "https://media.forgecdn.net/s.png", "title": "T" }],
            "latestFilesIndexes": [{ "gameVersion": "1.20.1", "modLoader": 1 }, { "gameVersion": "Forge", "modLoader": 1 }, { "gameVersion": "1.19.2", "modLoader": 4 }]
        });
        let d = from_curseforge(&m, "<p>Hello</p>");
        assert_eq!((d.id.as_str(), d.body_format.as_str(), d.body.as_str()), ("42", "html", "<p>Hello</p>"));
        assert_eq!(d.game_versions, ["1.20.1", "1.19.2"]);
        assert_eq!(d.loaders, ["forge", "fabric"]);
        assert!(d.links.is_empty());
        assert_eq!(d.gallery.len(), 1);
    }
}
