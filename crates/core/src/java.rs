use anyhow::{bail, Result};
use std::path::PathBuf;
use tokio::process::Command;

/// Parse `java -version` output: `"1.8.0_392"` -> 8, `"21.0.1"` -> 21.
pub fn parse_major(out: &str) -> Option<u32> {
    let start = out.find('"')? + 1;
    let rest = &out[start..];
    let ver = &rest[..rest.find('"')?];
    let mut parts = ver.split(['.', '_', '-', '+']);
    let first: u32 = parts.next()?.parse().ok()?;
    if first == 1 {
        parts.next()?.parse().ok()
    } else {
        Some(first)
    }
}

/// Java homes carry a `release` file with the version, so most candidates are read without
/// starting a JVM; starting one per candidate cost up to a second before every launch.
fn major_from_release(java: &std::path::Path) -> Option<u32> {
    if !java.is_file() {
        return None;
    }
    let home = java.parent()?.parent()?;
    let text = std::fs::read_to_string(home.join("release")).ok()?;
    let line = text.lines().find(|l| l.starts_with("JAVA_VERSION="))?;
    parse_major(line)
}

async fn major_of(path: &PathBuf) -> Option<u32> {
    if let Some(m) = major_from_release(path) {
        return Some(m);
    }
    let mut cmd = Command::new(path);
    cmd.arg("-version");
    crate::paths::hide_console(&mut cmd);
    let out = cmd.output().await.ok()?;
    parse_major(&String::from_utf8_lossy(&out.stderr))
}

fn candidates(preferred: Option<&str>, runtimes: &std::path::Path) -> Vec<PathBuf> {
    let exe = if cfg!(windows) { "java.exe" } else { "java" };
    let mut c = Vec::new();
    if let Some(p) = preferred.filter(|p| !p.is_empty()) {
        c.push(PathBuf::from(p));
    }
    if let Ok(rd) = std::fs::read_dir(runtimes) {
        for e in rd.flatten() {
            c.push(e.path().join("bin").join(exe));
        }
    }
    if let Ok(rd) = std::fs::read_dir(runtimes) {
        for e in rd.flatten() {
            c.push(e.path().join("jre.bundle/Contents/Home/bin").join(exe));
        }
    }
    if let Ok(h) = std::env::var("JAVA_HOME") {
        c.push(PathBuf::from(h).join("bin").join(exe));
    }
    if let Ok(rd) = std::fs::read_dir("/usr/lib/jvm") {
        for e in rd.flatten() {
            c.push(e.path().join("bin").join(exe));
        }
    }
    c.push(PathBuf::from(exe));
    c
}

/// Games up to Java 8 only run on Java 8: LaunchWrapper (Minecraft 1.12 and older, Forge up to 1.16) casts the
/// system class loader to URLClassLoader, which Java 9 and newer no longer allow. Newer games accept newer Java.
fn acceptable(required: u32, found: u32) -> bool {
    found == required || (required > 8 && found > required)
}

/// Find a Java for `required`, preferring an exact match. A Java the user chose by path is used as long as
/// `acceptable` (games that need Java 8 never get a newer one).
pub async fn find(required: u32, preferred: Option<&str>, runtimes: &std::path::Path) -> Result<PathBuf> {
    let chosen = preferred.filter(|p| !p.is_empty()).map(PathBuf::from);
    let mut best: Option<(u32, PathBuf)> = None;
    for p in candidates(preferred, runtimes) {
        let Some(m) = major_of(&p).await else { continue };
        if m == required || (chosen.as_ref() == Some(&p) && acceptable(required, m)) {
            return Ok(p);
        }
        if acceptable(required, m) && best.as_ref().is_none_or(|(bm, _)| m < *bm) {
            best = Some((m, p));
        }
    }
    match best {
        Some((_, p)) => Ok(p),
        None if required <= 8 => bail!("Java 8 not found. Install it or set a Java path in the settings."),
        None => bail!("Java {required} or newer not found. Install it or set a Java path in the settings."),
    }
}

const RUNTIME_INDEX: &str = "https://launchermeta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json";

fn runtime_platform() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => Some("linux"),
        ("linux", "x86") => Some("linux-i386"),
        ("windows", "x86_64") => Some("windows-x64"),
        ("windows", "x86") => Some("windows-x86"),
        ("windows", "aarch64") => Some("windows-arm64"),
        ("macos", "x86_64") => Some("mac-os"),
        ("macos", "aarch64") => Some("mac-os-arm64"),
        _ => None,
    }
}

#[derive(serde::Deserialize)]
struct Pointer {
    manifest: Ref,
}
#[derive(serde::Deserialize)]
struct Ref {
    sha1: String,
    url: String,
}
#[derive(serde::Deserialize)]
struct RtManifest {
    files: std::collections::HashMap<String, RtFile>,
}
#[derive(serde::Deserialize)]
struct RtFile {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    executable: bool,
    downloads: Option<RtDownloads>,
}
#[derive(serde::Deserialize)]
struct RtDownloads {
    raw: RtRaw,
}
#[derive(serde::Deserialize)]
struct RtRaw {
    sha1: String,
    size: u64,
    url: String,
}

fn mojang_host(url: &str) -> bool {
    reqwest::Url::parse(url)
        .is_ok_and(|u| u.scheme() == "https" && matches!(u.host_str(), Some("piston-meta.mojang.com" | "piston-data.mojang.com" | "launchermeta.mojang.com" | "launcher.mojang.com")))
}

/// Download Mojang's own Java runtime `component` (e.g. `java-runtime-delta`) into
/// `runtimes/<component>` and return its `java` binary. Every file is SHA-1 checked.
pub async fn install_runtime(
    client: &reqwest::Client,
    runtimes: &std::path::Path,
    component: &str,
    on_progress: impl Fn(crate::download::Progress) + Send + Sync,
) -> Result<PathBuf> {
    if !crate::paths::file_name_ok(component) {
        bail!("bad runtime name");
    }
    let platform = runtime_platform().ok_or_else(|| anyhow::anyhow!("no Mojang Java runtime for this platform; set a Java path in the settings"))?;
    let index: std::collections::HashMap<String, std::collections::HashMap<String, Vec<Pointer>>> =
        client.get(RUNTIME_INDEX).send().await?.error_for_status()?.json().await?;
    let ptr = index
        .get(platform)
        .and_then(|m| m.get(component))
        .and_then(|v| v.first())
        .ok_or_else(|| anyhow::anyhow!("Mojang has no runtime {component} for {platform}"))?;
    if !mojang_host(&ptr.manifest.url) {
        bail!("runtime manifest from unexpected host");
    }
    let text = client.get(&ptr.manifest.url).send().await?.error_for_status()?.bytes().await?;
    {
        use sha1::{Digest, Sha1};
        let got: String = Sha1::digest(&text).iter().map(|b| format!("{b:02x}")).collect();
        if !got.eq_ignore_ascii_case(&ptr.manifest.sha1) {
            bail!("runtime manifest checksum mismatch");
        }
    }
    let manifest: RtManifest = serde_json::from_slice(&text)?;
    let root = runtimes.join(component);
    let mut jobs = Vec::new();
    let mut exec = Vec::new();
    for (rel, f) in &manifest.files {
        let dest = crate::download::safe_join(&root, rel)?;
        match f.kind.as_str() {
            "directory" => tokio::fs::create_dir_all(&dest).await?,
            "file" => {
                let Some(d) = &f.downloads else { continue };
                if !mojang_host(&d.raw.url) {
                    bail!("runtime file from unexpected host");
                }
                if f.executable {
                    exec.push(dest.clone());
                }
                jobs.push(crate::download::Job { url: d.raw.url.clone(), dest, sha1: Some(d.raw.sha1.clone()), size: Some(d.raw.size) });
            }
            _ => {} // links are not needed to run java
        }
    }
    crate::download::download_all(client, jobs, 8, on_progress).await?;
    #[cfg(unix)]
    for p in exec {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).await?;
    }
    #[cfg(not(unix))]
    let _ = exec;
    for rel in ["bin/java", "bin/java.exe", "jre.bundle/Contents/Home/bin/java"] {
        let p = root.join(rel);
        if p.exists() {
            return Ok(p);
        }
    }
    bail!("runtime installed but no java binary found")
}

/// Like `find`, but fetches Mojang's runtime when no suitable Java is installed.
pub async fn find_or_install(
    client: &reqwest::Client,
    required: u32,
    component: Option<&str>,
    preferred: Option<&str>,
    runtimes: &std::path::Path,
    on_progress: impl Fn(crate::download::Progress) + Send + Sync,
) -> Result<PathBuf> {
    match find(required, preferred, runtimes).await {
        Ok(p) => Ok(p),
        Err(e) => match component {
            Some(c) => install_runtime(client, runtimes, c, on_progress).await,
            None => Err(e),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_version_is_read_from_the_release_file() {
        let home = std::env::temp_dir().join(format!("gn-release-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(home.join("bin")).unwrap();
        let java = home.join("bin").join("java");
        assert_eq!(major_from_release(&java), None, "no java binary, no answer");
        std::fs::write(&java, "").unwrap();
        assert_eq!(major_from_release(&java), None, "no release file, no answer");
        std::fs::write(home.join("release"), "IMPLEMENTOR=\"Eclipse\"\nJAVA_VERSION=\"21.0.3\"\n").unwrap();
        assert_eq!(major_from_release(&java), Some(21));
        std::fs::write(home.join("release"), "JAVA_VERSION=\"1.8.0_392\"\n").unwrap();
        assert_eq!(major_from_release(&java), Some(8));
        let _ = std::fs::remove_dir_all(home);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn preferred_java_must_be_compatible() {
        use std::os::unix::fs::PermissionsExt;

        let root = std::env::temp_dir().join(format!("geff-java-selection-{}", uuid::Uuid::new_v4()));
        let runtimes = root.join("runtimes");
        let java8 = runtimes.join("jre-legacy/bin/java");
        std::fs::create_dir_all(java8.parent().unwrap()).unwrap();
        std::fs::write(&java8, "#!/bin/sh\necho 'java version \"1.8.0_392\"' >&2\n").unwrap();
        std::fs::set_permissions(&java8, std::fs::Permissions::from_mode(0o755)).unwrap();
        let preferred = root.join("preferred-java");
        for major in [8, 17, 21] {
            std::fs::write(&preferred, format!("#!/bin/sh\necho 'openjdk version \"{major}.0.1\"' >&2\n")).unwrap();
            std::fs::set_permissions(&preferred, std::fs::Permissions::from_mode(0o755)).unwrap();
            let selected = find(8, preferred.to_str(), &runtimes).await.unwrap();
            assert_eq!(selected, if major == 8 { preferred.clone() } else { java8.clone() });
            if major >= 17 {
                assert_eq!(find(17, preferred.to_str(), &runtimes).await.unwrap(), preferred);
            }
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    #[ignore = "downloads Mojang's Java runtime"]
    async fn live_runtime_install() {
        let dir = std::env::temp_dir().join(format!("geff-rt-{}", std::process::id()));
        let client = reqwest::Client::new();
        let java = install_runtime(&client, &dir, "java-runtime-delta", |_| {}).await.unwrap();
        assert_eq!(major_of(&java).await, Some(21));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn platform_known() {
        assert!(runtime_platform().is_some());
        assert!(mojang_host("https://piston-meta.mojang.com/v1/x"));
        assert!(!mojang_host("http://piston-meta.mojang.com/x"));
        assert!(!mojang_host("https://evil.example/piston-meta.mojang.com"));
    }

    #[test]
    fn old_games_only_run_on_java_8() {
        assert!(acceptable(8, 8));
        assert!(!acceptable(8, 17));
        assert!(!acceptable(8, 21));
        assert!(!acceptable(8, 7));
        assert!(acceptable(17, 17));
        assert!(acceptable(17, 21));
        assert!(!acceptable(17, 8));
    }

    #[test]
    fn majors() {
        assert_eq!(parse_major("openjdk version \"21.0.1\" 2023-10-17"), Some(21));
        assert_eq!(parse_major("java version \"1.8.0_392\""), Some(8));
        assert_eq!(parse_major("openjdk version \"17\" 2021"), Some(17));
        assert_eq!(parse_major("garbage"), None);
    }
}
