use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

pub const MANIFEST_URL: &str = "https://deldown-downloads.s3.de.io.cloud.ovh.net/geffeniuse/latest.json";
const MAX_BYTES: u64 = 200 * 1024 * 1024;

/// The manifest location; `GEFFENIUSE_UPDATE_URL` overrides it for testing a release before it is published.
pub fn manifest_url() -> String {
    std::env::var("GEFFENIUSE_UPDATE_URL").ok().filter(|u| !u.is_empty()).unwrap_or_else(|| MANIFEST_URL.to_string())
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Manifest {
    pub version: String,
    pub appimage_url: Option<String>,
    pub appimage_sha256: Option<String>,
    pub deb_url: Option<String>,
    /// Windows package: a zip with `geffeniuse.exe` and `WebView2Loader.dll`.
    pub windows_url: Option<String>,
    pub windows_sha256: Option<String>,
    pub notes: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
pub struct Available {
    pub current: String,
    pub version: String,
    pub notes: Option<String>,
    /// True when this process can replace itself (an AppImage, or the Windows exe).
    pub can_apply: bool,
    pub download_url: Option<String>,
}

fn parts(v: &str) -> Option<Vec<u32>> {
    v.split('.').map(|p| p.parse().ok()).collect()
}

pub fn is_newer(candidate: &str, current: &str) -> bool {
    match (parts(candidate), parts(current)) {
        (Some(a), Some(b)) => a > b,
        _ => false,
    }
}

/// The app's shared client refuses anything but https. Only a manifest on 127.0.0.1 (a release
/// being tried locally) gets a client that also speaks plain http; every real update stays https-only.
pub fn client_for(manifest: &str, shared: &reqwest::Client) -> reqwest::Client {
    let loopback = reqwest::Url::parse(manifest).is_ok_and(|u| u.scheme() == "http" && u.host_str() == Some("127.0.0.1"));
    if !loopback {
        return shared.clone();
    }
    reqwest::Client::builder().timeout(std::time::Duration::from_secs(60)).build().unwrap_or_else(|_| shared.clone())
}

/// Files must come from the same https host as the manifest, so a manifest can never point
/// the updater at a third party. Plain http is accepted for 127.0.0.1 only, which is how a
/// release is tried out locally (`GEFFENIUSE_UPDATE_URL`); it is unreachable from other machines.
fn same_origin(manifest: &str, asset: &str) -> bool {
    let (Ok(m), Ok(a)) = (reqwest::Url::parse(manifest), reqwest::Url::parse(asset)) else { return false };
    let local_test = a.scheme() == "http" && a.host_str() == Some("127.0.0.1") && m.host_str() == a.host_str();
    (a.scheme() == "https" || local_test) && m.host_str().is_some() && m.host_str() == a.host_str() && m.port() == a.port()
}

pub fn running_appimage() -> Option<PathBuf> {
    std::env::var_os("APPIMAGE").map(PathBuf::from).filter(|p| p.is_file())
}

/// How this running copy of the launcher can replace itself, if at all.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    AppImage,
    Windows,
}

pub fn self_update_kind() -> Option<Kind> {
    if cfg!(windows) {
        Some(Kind::Windows)
    } else {
        running_appimage().map(|_| Kind::AppImage)
    }
}

fn valid_sha(h: &Option<String>) -> bool {
    h.as_deref().is_some_and(|h| h.len() == 64 && h.bytes().all(|b| b.is_ascii_hexdigit()))
}

pub fn evaluate(m: &Manifest, current: &str, kind: Option<Kind>, manifest: &str) -> Option<Available> {
    if !is_newer(&m.version, current) {
        return None;
    }
    let ok = |u: &Option<String>| u.clone().filter(|u| same_origin(manifest, u));
    let appimage = ok(&m.appimage_url).is_some() && valid_sha(&m.appimage_sha256);
    let windows = ok(&m.windows_url).is_some() && valid_sha(&m.windows_sha256);
    let on_windows = kind == Some(Kind::Windows) || (kind.is_none() && cfg!(windows));
    Some(Available {
        current: current.to_string(),
        version: m.version.clone(),
        notes: m.notes.clone().map(|n| n.chars().take(500).collect()),
        can_apply: match kind {
            Some(Kind::AppImage) => appimage,
            Some(Kind::Windows) => windows,
            None => false,
        },
        download_url: if on_windows { ok(&m.windows_url) } else { ok(&m.deb_url).or_else(|| ok(&m.appimage_url)) },
    })
}

async fn fetch_manifest(client: &reqwest::Client, url: &str) -> Result<Manifest> {
    let resp = client.get(url).send().await?;
    if matches!(resp.status().as_u16(), 403 | 404) {
        bail!("no release has been published yet");
    }
    Ok(resp.error_for_status()?.json().await?)
}

pub async fn check(client: &reqwest::Client, current: &str) -> Result<Option<Available>> {
    let url = manifest_url();
    let m = fetch_manifest(client, &url).await?;
    Ok(evaluate(&m, current, self_update_kind(), &url))
}

/// Streams `url` into `tmp`, enforcing the size cap and the published SHA-256. A bad file is deleted.
async fn download_verified(client: &reqwest::Client, manifest_url: &str, url: &str, want: &str, tmp: &Path) -> Result<()> {
    if !same_origin(manifest_url, url) {
        bail!("update from unexpected host");
    }
    let mut resp = client.get(url).send().await?.error_for_status()?;
    if resp.content_length().is_some_and(|l| l > MAX_BYTES) {
        bail!("update too large");
    }
    let mut hasher = Sha256::new();
    let mut total = 0u64;
    {
        use tokio::io::AsyncWriteExt;
        let mut f = tokio::fs::File::create(tmp).await?;
        while let Some(chunk) = resp.chunk().await? {
            total += chunk.len() as u64;
            if total > MAX_BYTES {
                let _ = tokio::fs::remove_file(tmp).await;
                bail!("update too large");
            }
            hasher.update(&chunk);
            f.write_all(&chunk).await?;
        }
        f.flush().await?;
    }
    if !hex::encode(hasher.finalize()).eq_ignore_ascii_case(want) {
        let _ = tokio::fs::remove_file(tmp).await;
        bail!("checksum mismatch, update discarded");
    }
    Ok(())
}

/// Downloads the new AppImage next to the running one, checks its SHA-256 and swaps it in.
/// The running process keeps its old file (Linux keeps the inode); the new one starts on relaunch.
pub async fn apply_to(client: &reqwest::Client, manifest_url: &str, current: &str, target: &Path) -> Result<String> {
    let m = fetch_manifest(client, manifest_url).await?;
    if !is_newer(&m.version, current) {
        bail!("already up to date");
    }
    let (url, want) = (m.appimage_url.context("no AppImage in this release")?, m.appimage_sha256.context("no checksum in this release")?);
    let tmp = target.with_extension("update");
    download_verified(client, manifest_url, &url, &want, &tmp).await?;
    set_executable(&tmp).await?;
    tokio::fs::rename(&tmp, target).await.context("replace AppImage")?;
    Ok(m.version)
}

/// Files a Windows package may contain. Anything else in the zip is ignored.
const WINDOWS_FILES: [&str; 2] = ["geffeniuse.exe", "WebView2Loader.dll"];
const MAX_ENTRY_BYTES: u64 = 120 * 1024 * 1024;

/// Unpacks the Windows zip next to `exe` and swaps the files in. A running exe or loaded DLL
/// cannot be overwritten on Windows but can be renamed, so each current file becomes `<name>.old`
/// (deleted on the next start) and the new one takes its name. On any failure everything is put back.
pub fn install_windows_zip(zip_path: &Path, exe: &Path) -> Result<()> {
    let dir = exe.parent().context("exe has no folder")?;
    let exe_name = exe.file_name().context("exe has no name")?.to_os_string();
    let mut zip = zip::ZipArchive::new(std::fs::File::open(zip_path)?)?;
    let mut staged: Vec<(PathBuf, PathBuf)> = Vec::new(); // (new file, final path)
    for entry in WINDOWS_FILES {
        let Ok(mut f) = zip.by_name(entry) else { continue };
        if f.size() > MAX_ENTRY_BYTES {
            bail!("file in update is too large");
        }
        let final_path = if entry == "geffeniuse.exe" { dir.join(&exe_name) } else { dir.join(entry) };
        let new_path = final_path.with_file_name(format!("{}.new", final_path.file_name().unwrap().to_string_lossy()));
        std::io::copy(&mut std::io::Read::take(&mut f, MAX_ENTRY_BYTES), &mut std::fs::File::create(&new_path)?)?;
        staged.push((new_path, final_path));
    }
    if !staged.iter().any(|(_, f)| f.file_name() == Some(exe_name.as_os_str())) {
        for (n, _) in &staged {
            let _ = std::fs::remove_file(n);
        }
        bail!("the update does not contain the launcher");
    }
    let mut done: Vec<(PathBuf, PathBuf)> = Vec::new(); // (final, old)
    let result = (|| -> Result<()> {
        for (new_path, final_path) in &staged {
            let old = final_path.with_file_name(format!("{}.old", final_path.file_name().unwrap().to_string_lossy()));
            let _ = std::fs::remove_file(&old);
            let had = final_path.exists();
            if had {
                std::fs::rename(final_path, &old).context("move the running file aside")?;
                done.push((final_path.clone(), old.clone()));
            }
            std::fs::rename(new_path, final_path).context("put the new file in place")?;
        }
        Ok(())
    })();
    if result.is_err() {
        for (final_path, old) in done.iter().rev() {
            let _ = std::fs::remove_file(final_path);
            let _ = std::fs::rename(old, final_path);
        }
        for (n, _) in &staged {
            let _ = std::fs::remove_file(n);
        }
    }
    result
}

pub async fn apply_windows_to(client: &reqwest::Client, manifest_url: &str, current: &str, exe: &Path) -> Result<String> {
    let m = fetch_manifest(client, manifest_url).await?;
    if !is_newer(&m.version, current) {
        bail!("already up to date");
    }
    let (url, want) = (m.windows_url.context("no Windows package in this release")?, m.windows_sha256.context("no checksum in this release")?);
    let tmp = exe.with_file_name(format!("{}.update.zip", exe.file_name().context("exe has no name")?.to_string_lossy()));
    download_verified(client, manifest_url, &url, &want, &tmp).await?;
    let (zip, exe) = (tmp.clone(), exe.to_path_buf());
    let r = tokio::task::spawn_blocking(move || install_windows_zip(&zip, &exe)).await?;
    let _ = tokio::fs::remove_file(&tmp).await;
    r?;
    Ok(m.version)
}

static START_EXE: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

/// Remembers where the launcher was started from. After a Windows update renamed the running
/// file, `current_exe()` may report the `.old` name, so restart and updates use this instead.
pub fn remember_exe() {
    if let Ok(p) = std::env::current_exe() {
        let _ = START_EXE.set(p);
    }
}

pub fn exe_path() -> Option<PathBuf> {
    START_EXE.get().cloned().or_else(|| std::env::current_exe().ok())
}

/// Removes the files a previous Windows update moved aside. Called once at start.
pub fn cleanup_leftovers() {
    let Some(exe) = exe_path() else { return };
    let Some(dir) = exe.parent() else { return };
    let mut names: Vec<String> = WINDOWS_FILES.iter().map(|n| format!("{n}.old")).collect();
    if let Some(n) = exe.file_name() {
        names.push(format!("{}.old", n.to_string_lossy()));
    }
    for n in names {
        let _ = std::fs::remove_file(dir.join(n));
    }
}

pub async fn apply(client: &reqwest::Client, current: &str) -> Result<String> {
    match self_update_kind() {
        Some(Kind::AppImage) => {
            let target = running_appimage().context("not running as an AppImage")?;
            apply_to(client, &manifest_url(), current, &target).await
        }
        Some(Kind::Windows) => apply_windows_to(client, &manifest_url(), current, &exe_path().context("cannot find the launcher file")?).await,
        None => bail!("this copy of the launcher cannot update itself"),
    }
}

async fn set_executable(p: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(p, std::fs::Permissions::from_mode(0o755)).await?;
    }
    let _ = p;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    const MANIFEST: &str = "https://downloads.example/geffeniuse/latest.json";

    fn m(v: &str) -> Manifest {
        Manifest {
            version: v.into(),
            appimage_url: Some("https://downloads.example/geffeniuse/a.AppImage".into()),
            appimage_sha256: Some("a".repeat(64)),
            deb_url: Some("https://evil.example/x.deb".into()),
            windows_url: Some("https://downloads.example/geffeniuse/w.zip".into()),
            windows_sha256: Some("b".repeat(64)),
            notes: None,
        }
    }

    #[test]
    fn only_loopback_manifests_get_the_plain_http_client() {
        let shared = crate::http_client();
        let plain = client_for("http://127.0.0.1:8791/latest.json", &shared);
        let strict = client_for("https://downloads.example/latest.json", &shared);
        assert!(plain.get("http://127.0.0.1:1/").build().is_ok());
        // the shared client was built with https_only, so a plain-http request cannot even be sent
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        assert!(rt.block_on(strict.get("http://example.invalid/").send()).is_err());
        assert!(!same_origin("http://example.com/latest.json", "http://example.com/a"), "http is never accepted off loopback");
    }

    #[test]
    fn version_order_is_numeric() {
        assert!(is_newer("0.1.10", "0.1.9"));
        assert!(is_newer("0.3.0", "0.2.9"));
        assert!(!is_newer("0.1.4", "0.1.4"));
        assert!(!is_newer("0.1.3", "0.1.4"));
        assert!(!is_newer("garbage", "0.1.4"));
    }

    #[test]
    fn evaluate_filters_hosts_and_needs_checksum() {
        let a = evaluate(&m("0.2.0"), "0.1.4", Some(Kind::AppImage), MANIFEST).unwrap();
        assert!(a.can_apply && a.current == "0.1.4");
        assert!(a.download_url.unwrap().starts_with("https://downloads.example/"), "foreign deb url must be dropped");
        assert!(!evaluate(&m("0.2.0"), "0.1.4", None, MANIFEST).unwrap().can_apply);
        let mut bad = m("0.2.0");
        bad.appimage_sha256 = Some("zz".repeat(32));
        assert!(!evaluate(&bad, "0.1.4", Some(Kind::AppImage), MANIFEST).unwrap().can_apply);
        assert!(evaluate(&m("0.1.4"), "0.1.4", Some(Kind::AppImage), MANIFEST).is_none());
        assert!(!same_origin(MANIFEST, "http://downloads.example/x"), "plain http is refused");
        assert!(!same_origin(MANIFEST, "https://downloads.example.evil/x"));
    }

    /// A server on a fixed local port: `latest.json` returns the manifest, everything else the payload.
    async fn serve(port: u16, manifest: Vec<u8>, payload: Vec<u8>) -> tokio::task::JoinHandle<()> {
        let l = tokio::net::TcpListener::bind(("127.0.0.1", port)).await.unwrap();
        tokio::spawn(async move {
            loop {
                let Ok((mut sock, _)) = l.accept().await else { return };
                let (m, p) = (manifest.clone(), payload.clone());
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 4096];
                    let n = sock.read(&mut buf).await.unwrap_or(0);
                    let body = if String::from_utf8_lossy(&buf[..n]).contains("latest.json") { m } else { p };
                    let head = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
                    let _ = sock.write_all(head.as_bytes()).await;
                    let _ = sock.write_all(&body).await;
                });
            }
        })
    }

    async fn free_port() -> u16 {
        tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap().local_addr().unwrap().port()
    }

    fn manifest_json(file_base: &str, version: &str, sha: &str) -> Vec<u8> {
        format!(r#"{{"version":"{version}","appimage_url":"{file_base}/new.AppImage","appimage_sha256":"{sha}"}}"#).into_bytes()
    }

    fn temp_target(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("gn-upd-{tag}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let t = dir.join("Geffeniuse.AppImage");
        std::fs::write(&t, b"OLD").unwrap();
        t
    }

    async fn run(manifest_for: impl Fn(&str, &str) -> Vec<u8>, current: &str) -> (anyhow::Result<String>, PathBuf) {
        let payload = b"NEWBINARY".to_vec();
        let sha = hex::encode(Sha256::digest(&payload));
        let port = free_port().await;
        let base = format!("http://127.0.0.1:{port}");
        let server = serve(port, manifest_for(&base, &sha), payload).await;
        let target = temp_target("run");
        let r = apply_to(&reqwest::Client::new(), &format!("{base}/latest.json"), current, &target).await;
        server.abort();
        (r, target)
    }

    #[tokio::test]
    async fn applies_a_verified_update_over_the_old_file() {
        let (r, target) = run(|base, sha| manifest_json(base, "9.9.9", sha), "0.3.0").await;
        assert_eq!(r.unwrap(), "9.9.9");
        assert_eq!(std::fs::read(&target).unwrap(), b"NEWBINARY");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&target).unwrap().permissions().mode() & 0o111, 0o111, "AppImage must be executable");
        }
        assert!(!target.with_extension("update").exists());
        let _ = std::fs::remove_dir_all(target.parent().unwrap());
    }

    #[tokio::test]
    async fn rejects_a_wrong_checksum_and_keeps_the_old_file() {
        let (r, target) = run(|base, _| manifest_json(base, "9.9.9", &"0".repeat(64)), "0.3.0").await;
        assert!(r.unwrap_err().to_string().contains("checksum"));
        assert_eq!(std::fs::read(&target).unwrap(), b"OLD");
        assert!(!target.with_extension("update").exists(), "a rejected download must not stay behind");
        let _ = std::fs::remove_dir_all(target.parent().unwrap());
    }

    #[tokio::test]
    async fn refuses_files_from_another_host_and_older_versions() {
        let (r, target) = run(|_, sha| manifest_json("http://127.0.0.1:1", "9.9.9", sha), "0.3.0").await;
        assert!(r.unwrap_err().to_string().contains("unexpected host"));
        assert_eq!(std::fs::read(&target).unwrap(), b"OLD");
        let _ = std::fs::remove_dir_all(target.parent().unwrap());
        let (r, target) = run(|base, sha| manifest_json(base, "0.1.0", sha), "0.3.0").await;
        assert!(r.unwrap_err().to_string().contains("up to date"));
        let _ = std::fs::remove_dir_all(target.parent().unwrap());
    }

    fn make_zip(entries: &[(&str, &[u8])]) -> PathBuf {
        use std::io::Write;
        let p = std::env::temp_dir().join(format!("gn-upd-{}.zip", uuid::Uuid::new_v4()));
        let mut w = zip::ZipWriter::new(std::fs::File::create(&p).unwrap());
        for (n, d) in entries {
            w.start_file(*n, zip::write::SimpleFileOptions::default()).unwrap();
            w.write_all(d).unwrap();
        }
        w.finish().unwrap();
        p
    }

    fn windows_dir() -> (PathBuf, PathBuf) {
        let dir = std::env::temp_dir().join(format!("gn-win-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("geffeniuse.exe"), b"OLD EXE").unwrap();
        std::fs::write(dir.join("WebView2Loader.dll"), b"OLD DLL").unwrap();
        (dir.join("geffeniuse.exe"), dir)
    }

    #[test]
    fn windows_zip_swaps_both_files_and_keeps_the_old_ones_aside() {
        let (exe, dir) = windows_dir();
        let zip = make_zip(&[("geffeniuse.exe", b"NEW EXE"), ("WebView2Loader.dll", b"NEW DLL"), ("evil.bat", b"del *")]);
        install_windows_zip(&zip, &exe).unwrap();
        assert_eq!(std::fs::read(&exe).unwrap(), b"NEW EXE");
        assert_eq!(std::fs::read(dir.join("WebView2Loader.dll")).unwrap(), b"NEW DLL");
        assert_eq!(std::fs::read(dir.join("geffeniuse.exe.old")).unwrap(), b"OLD EXE");
        assert!(!dir.join("evil.bat").exists(), "files outside the allow-list are never written");
        assert!(!dir.join("geffeniuse.exe.new").exists());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn windows_zip_without_the_launcher_changes_nothing() {
        let (exe, dir) = windows_dir();
        let zip = make_zip(&[("WebView2Loader.dll", b"NEW DLL"), ("readme.txt", b"x")]);
        assert!(install_windows_zip(&zip, &exe).is_err());
        assert_eq!(std::fs::read(&exe).unwrap(), b"OLD EXE");
        assert_eq!(std::fs::read(dir.join("WebView2Loader.dll")).unwrap(), b"OLD DLL");
        assert!(!dir.join("WebView2Loader.dll.new").exists(), "staged files are cleaned up");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn evaluate_picks_the_windows_package_on_windows() {
        let a = evaluate(&m("0.2.0"), "0.1.4", Some(Kind::Windows), MANIFEST).unwrap();
        assert!(a.can_apply);
        assert_eq!(a.download_url.as_deref(), Some("https://downloads.example/geffeniuse/w.zip"));
        let mut no_win = m("0.2.0");
        no_win.windows_sha256 = None;
        assert!(!evaluate(&no_win, "0.1.4", Some(Kind::Windows), MANIFEST).unwrap().can_apply);
    }

    #[tokio::test]
    #[ignore = "downloads the published release from the public update server"]
    async fn live_public_update_end_to_end() {
        let client = crate::http_client();
        let latest = fetch_manifest(&client, MANIFEST_URL).await.expect("a published manifest");
        let target = temp_target("live");
        let version = apply_to(&client, MANIFEST_URL, "0.0.1", &target).await.unwrap();
        assert_eq!(version, latest.version);
        let bytes = std::fs::read(&target).unwrap();
        assert!(bytes.len() > 1_000_000 && bytes.starts_with(b"\x7fELF"), "a real AppImage (ELF runtime), not the placeholder");
        assert_eq!(hex::encode(Sha256::digest(&bytes)), latest.appimage_sha256.unwrap());
        assert!(apply_to(&client, MANIFEST_URL, &latest.version, &target).await.unwrap_err().to_string().contains("up to date"));
        let _ = std::fs::remove_dir_all(target.parent().unwrap());
    }
}
