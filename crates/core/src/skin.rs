use crate::paths::{valid_id, Dirs};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const MAX_SKIN_BYTES: usize = 64 * 1024;
const MAX_AGE_SECS: u64 = 6 * 3600;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct SkinInfo {
    pub slim: bool,
}

fn base64_decode(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0u32);
    for c in s.bytes().filter(|c| *c != b'=') {
        let v = u32::from(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => return None,
        });
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

pub fn base64_encode(b: &[u8]) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(b.len().div_ceil(3) * 4);
    for c in b.chunks(3) {
        let n = u32::from(c[0]) << 16 | u32::from(*c.get(1).unwrap_or(&0)) << 8 | u32::from(*c.get(2).unwrap_or(&0));
        for i in 0..4 {
            s.push(if i <= c.len() { T[((n >> (18 - 6 * i)) & 63) as usize] as char } else { '=' });
        }
    }
    s
}

/// The cached skin as a `data:` URL, so the window never needs a custom protocol for it.
pub async fn data_url(dirs: &Dirs, uuid: &str) -> Result<String> {
    let bytes = tokio::fs::read(skin_path(dirs, uuid)?).await?;
    if !is_skin_png(&bytes) {
        bail!("cached skin is not valid");
    }
    Ok(format!("data:image/png;base64,{}", base64_encode(&bytes)))
}

fn parse_textures(profile: &serde_json::Value) -> Option<(String, bool)> {
    let value = profile["properties"].as_array()?.iter().find(|p| p["name"] == "textures")?["value"].as_str()?;
    let json: serde_json::Value = serde_json::from_slice(&base64_decode(value)?).ok()?;
    let skin = &json["textures"]["SKIN"];
    Some((skin["url"].as_str()?.to_string(), skin["metadata"]["model"] == "slim"))
}

/// Mojang hands out `http://textures.minecraft.net/...`; the host is checked and the request is
/// always made over https.
fn skin_url(url: &str) -> Option<String> {
    let u = reqwest::Url::parse(url).ok()?;
    (matches!(u.scheme(), "http" | "https") && u.host_str() == Some("textures.minecraft.net") && u.port().is_none())
        .then(|| format!("https://textures.minecraft.net{}", u.path()))
}

/// A real skin: a PNG of 64x64 or 64x32 pixels, small enough to be a skin.
pub fn is_skin_png(b: &[u8]) -> bool {
    if b.len() < 33 || b.len() > MAX_SKIN_BYTES || &b[..8] != b"\x89PNG\r\n\x1a\n" || &b[12..16] != b"IHDR" {
        return false;
    }
    let w = u32::from_be_bytes([b[16], b[17], b[18], b[19]]);
    let h = u32::from_be_bytes([b[20], b[21], b[22], b[23]]);
    w == 64 && (h == 64 || h == 32)
}

fn dir(dirs: &Dirs) -> PathBuf {
    dirs.cache().join("skins")
}

pub fn skin_path(dirs: &Dirs, uuid: &str) -> Result<PathBuf> {
    let id = uuid.replace('-', "");
    if !valid_id(&id) || id.len() != 32 {
        bail!("bad uuid");
    }
    Ok(dir(dirs).join(format!("{id}.png")))
}

fn meta_path(dirs: &Dirs, uuid: &str) -> Result<PathBuf> {
    Ok(skin_path(dirs, uuid)?.with_extension("json"))
}

fn fresh(path: &std::path::Path) -> bool {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.elapsed().ok())
        .is_some_and(|age| age.as_secs() < MAX_AGE_SECS)
}

/// Whatever skin is already on disk, with no network and no freshness check, so the window can
/// show it immediately and refresh it in the background.
pub async fn cached(dirs: &Dirs, uuid: &str) -> Option<SkinInfo> {
    let png = skin_path(dirs, uuid).ok()?;
    if !tokio::fs::metadata(&png).await.ok()?.is_file() {
        return None;
    }
    serde_json::from_slice(&tokio::fs::read(png.with_extension("json")).await.ok()?).ok()
}

/// Makes sure the skin of a real account is cached locally; `None` when it has none.
pub async fn ensure(dirs: &Dirs, client: &reqwest::Client, uuid: &str) -> Result<Option<SkinInfo>> {
    let png = skin_path(dirs, uuid)?;
    let meta = meta_path(dirs, uuid)?;
    if fresh(&png) {
        if let Ok(b) = tokio::fs::read(&meta).await {
            if let Ok(info) = serde_json::from_slice(&b) {
                return Ok(Some(info));
            }
        }
    }
    let id = uuid.replace('-', "");
    let resp = client.get(format!("https://sessionserver.mojang.com/session/minecraft/profile/{id}")).send().await?;
    if resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
        bail!("Mojang limits skin requests, try again in a minute");
    }
    if !resp.status().is_success() {
        return Ok(None);
    }
    let profile: serde_json::Value = resp.json().await?;
    let Some((url, slim)) = parse_textures(&profile) else { return Ok(None) };
    let Some(url) = skin_url(&url) else { bail!("skin from unexpected host") };
    let bytes = client.get(&url).send().await?.error_for_status()?.bytes().await?;
    if !is_skin_png(&bytes) {
        bail!("not a valid skin image");
    }
    tokio::fs::create_dir_all(dir(dirs)).await?;
    tokio::fs::write(&png, &bytes).await?;
    let info = SkinInfo { slim };
    tokio::fs::write(&meta, serde_json::to_vec(&info)?).await.context("write skin meta")?;
    Ok(Some(info))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_base64_roundtrip() {
        for input in [&b""[..], b"a", b"ab", b"abc", b"abcd", b"\x89PNG\r\n\x1a\n\0\xff"] {
            assert_eq!(base64_decode(&base64_encode(input)).unwrap(), input);
        }
        assert_eq!(base64_encode(b"hello"), "aGVsbG8=");
    }

    #[test]
    fn decodes_base64() {
        assert_eq!(base64_decode("aGVsbG8=").unwrap(), b"hello");
        assert_eq!(base64_decode("aGk").unwrap(), b"hi");
        assert!(base64_decode("a$b").is_none());
    }

    #[test]
    fn reads_skin_url_and_model() {
        let tex = r#"{"textures":{"SKIN":{"url":"https://textures.minecraft.net/texture/abc","metadata":{"model":"slim"}}}}"#;
        let enc = {
            const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
            let b = tex.as_bytes();
            let mut s = String::new();
            for c in b.chunks(3) {
                let n = u32::from(c[0]) << 16 | u32::from(*c.get(1).unwrap_or(&0)) << 8 | u32::from(*c.get(2).unwrap_or(&0));
                for i in 0..4 {
                    if i <= c.len() {
                        s.push(T[((n >> (18 - 6 * i)) & 63) as usize] as char);
                    }
                }
            }
            s
        };
        let profile = serde_json::json!({ "properties": [{ "name": "textures", "value": enc }] });
        assert_eq!(parse_textures(&profile), Some(("https://textures.minecraft.net/texture/abc".into(), true)));
        assert_eq!(parse_textures(&serde_json::json!({ "properties": [] })), None);
    }

    #[test]
    fn host_and_png_checks() {
        assert_eq!(skin_url("https://textures.minecraft.net/texture/x").as_deref(), Some("https://textures.minecraft.net/texture/x"));
        assert_eq!(skin_url("http://textures.minecraft.net/texture/x").as_deref(), Some("https://textures.minecraft.net/texture/x"));
        assert_eq!(skin_url("https://evil.example/textures.minecraft.net"), None);
        assert_eq!(skin_url("https://textures.minecraft.net.evil.example/x"), None);
        assert_eq!(skin_url("ftp://textures.minecraft.net/x"), None);
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        png.extend([0, 0, 0, 64, 0, 0, 0, 64, 8, 6, 0, 0, 0, 0, 0, 0, 0]);
        assert!(is_skin_png(&png));
        png[23] = 100;
        assert!(!is_skin_png(&png));
        assert!(!is_skin_png(b"not a png at all, but long enough to pass the length check!!"));
    }

    #[test]
    fn skin_path_rejects_bad_uuids() {
        let dirs = Dirs::new(std::env::temp_dir());
        assert!(skin_path(&dirs, "../etc/passwd").is_err());
        assert!(skin_path(&dirs, "abc").is_err());
        assert!(skin_path(&dirs, "123e4567-e89b-12d3-a456-426614174000").is_ok());
    }

    #[tokio::test]
    #[ignore = "talks to Mojang's session server"]
    async fn live_skin_download() {
        let root = std::env::temp_dir().join(format!("gn-skin-{}", uuid::Uuid::new_v4()));
        let dirs = Dirs::new(&root);
        let client = crate::http_client();
        let t = std::time::Instant::now();
        let info = ensure(&dirs, &client, "069a79f4-44e9-4726-a5be-fca90e38aaf5").await.unwrap().expect("Notch has a skin");
        let network = t.elapsed();
        let t = std::time::Instant::now();
        assert!(cached(&dirs, "069a79f444e94726a5befca90e38aaf5").await.is_some());
        println!("skin: network fetch {network:?}, cached read {:?}", t.elapsed());
        let url = data_url(&dirs, "069a79f444e94726a5befca90e38aaf5").await.unwrap();
        assert!(url.starts_with("data:image/png;base64,") && url.len() > 200);
        println!("slim={} data_url_len={}", info.slim, url.len());
        let _ = std::fs::remove_dir_all(root);
    }
}
