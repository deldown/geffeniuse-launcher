use anyhow::{bail, Context, Result};
use futures_util::{stream, StreamExt};
use sha1::{Digest, Sha1};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Clone, Debug)]
pub struct Job {
    pub url: String,
    pub dest: PathBuf,
    pub sha1: Option<String>,
    pub size: Option<u64>,
}

#[derive(Clone, Copy, Debug, serde::Serialize)]
pub struct Progress {
    pub done: usize,
    pub total: usize,
}

/// Join an untrusted relative path (from a modpack, zip or JSON) onto `base`.
/// Rejects `..`, absolute paths and drive prefixes so nothing escapes `base`.
pub fn safe_join(base: &Path, rel: &str) -> Result<PathBuf> {
    let rel = rel.replace('\\', "/");
    let mut out = base.to_path_buf();
    let mut any = false;
    for c in Path::new(&rel).components() {
        match c {
            Component::Normal(p) => {
                out.push(p);
                any = true;
            }
            Component::CurDir => {}
            _ => bail!("unsafe path: {rel}"),
        }
    }
    if !any {
        bail!("empty path");
    }
    Ok(out)
}

/// Hosts a modpack file is allowed to be downloaded from (Modrinth spec + CurseForge CDN).
pub fn pack_host_allowed(url: &str) -> bool {
    let Ok(u) = reqwest::Url::parse(url) else { return false };
    if u.scheme() != "https" {
        return false;
    }
    matches!(
        u.host_str().unwrap_or(""),
        "cdn.modrinth.com" | "github.com" | "raw.githubusercontent.com" | "gitlab.com"
            | "edge.forgecdn.net" | "mediafilez.forgecdn.net" | "media.forgecdn.net"
    )
}

pub async fn sha1_file(path: &Path) -> Result<String> {
    let mut f = tokio::fs::File::open(path).await?;
    let mut h = Sha1::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = f.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(hex::encode(h.finalize()))
}

async fn is_valid(job: &Job) -> bool {
    let Ok(meta) = tokio::fs::metadata(&job.dest).await else { return false };
    if job.size.is_some_and(|s| s != meta.len()) {
        return false;
    }
    // Existing files are only size-checked: Mojang's assets and libraries are content-addressed and
    // every fresh download is hashed, so re-hashing thousands of files on each launch only burns CPU
    // and disk. The hash is still used when no size is known.
    match (&job.sha1, job.size) {
        (Some(h), None) => sha1_file(&job.dest).await.is_ok_and(|x| x.eq_ignore_ascii_case(h)),
        _ => true,
    }
}

pub async fn download_one(client: &reqwest::Client, job: &Job) -> Result<()> {
    if !job.url.starts_with("https://") {
        bail!("refusing non-https url: {}", job.url);
    }
    if is_valid(job).await {
        return Ok(());
    }
    if let Some(parent) = job.dest.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let mut last = None;
    for attempt in 0..5u32 {
        match try_download(client, job).await {
            Ok(()) => return Ok(()),
            Err(e) => {
                last = Some(e);
                tokio::time::sleep(std::time::Duration::from_millis(500 << attempt)).await;
            }
        }
    }
    Err(last.unwrap()).with_context(|| format!("download {}", job.url))
}

async fn try_download(client: &reqwest::Client, job: &Job) -> Result<()> {
    let resp = client.get(&job.url).send().await?.error_for_status()?;
    let part = job.dest.with_extension("part");
    let mut file = tokio::fs::File::create(&part).await?;
    let mut hasher = Sha1::new();
    let mut total = 0u64;
    let mut body = resp.bytes_stream();
    while let Some(chunk) = body.next().await {
        let chunk = chunk?;
        total += chunk.len() as u64;
        hasher.update(&chunk);
        file.write_all(&chunk).await?;
    }
    file.flush().await?;
    drop(file);
    if job.size.is_some_and(|s| s != total) {
        let _ = tokio::fs::remove_file(&part).await;
        bail!("size mismatch");
    }
    if let Some(want) = &job.sha1 {
        if !hex::encode(hasher.finalize()).eq_ignore_ascii_case(want) {
            let _ = tokio::fs::remove_file(&part).await;
            bail!("sha1 mismatch");
        }
    }
    tokio::fs::rename(&part, &job.dest).await?;
    Ok(())
}

pub async fn download_all(
    client: &reqwest::Client,
    jobs: Vec<Job>,
    concurrency: usize,
    on_progress: impl Fn(Progress) + Send + Sync,
) -> Result<()> {
    let total = jobs.len();
    on_progress(Progress { done: 0, total });
    // Most launches find nearly everything already on disk. Those checks are cheap metadata
    // lookups, so they run 64 at a time instead of waiting behind the few real downloads.
    let checked: Vec<(Job, bool)> = stream::iter(jobs).map(|job| async move { let ok = is_valid(&job).await; (job, ok) }).buffer_unordered(64).collect().await;
    let done = Arc::new(AtomicUsize::new(checked.iter().filter(|(_, ok)| *ok).count()));
    let missing: Vec<Job> = checked.into_iter().filter(|(_, ok)| !ok).map(|(j, _)| j).collect();
    on_progress(Progress { done: done.load(Ordering::Relaxed), total });
    let mut results = stream::iter(missing)
        .map(|job| {
            let client = client.clone();
            async move { download_one(&client, &job).await }
        })
        .buffer_unordered(concurrency.max(1));
    while let Some(r) = results.next().await {
        r?;
        let d = done.fetch_add(1, Ordering::Relaxed) + 1;
        on_progress(Progress { done: d, total });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_join_blocks_escape() {
        let b = Path::new("/base");
        assert!(safe_join(b, "../x").is_err());
        assert!(safe_join(b, "a/../../x").is_err());
        assert!(safe_join(b, "/etc/passwd").is_err());
        assert!(safe_join(b, "..\\x").is_err());
        assert!(safe_join(b, "").is_err());
        assert_eq!(safe_join(b, "mods/a.jar").unwrap(), PathBuf::from("/base/mods/a.jar"));
        assert_eq!(safe_join(b, "mods\\a.jar").unwrap(), PathBuf::from("/base/mods/a.jar"));
    }

    #[test]
    fn host_allowlist() {
        assert!(pack_host_allowed("https://cdn.modrinth.com/data/x/y.jar"));
        assert!(!pack_host_allowed("http://cdn.modrinth.com/x"));
        assert!(!pack_host_allowed("https://evil.example/x"));
        assert!(!pack_host_allowed("https://cdn.modrinth.com.evil.example/x"));
    }

    #[tokio::test]
    async fn existing_files_are_skipped_without_any_request() {
        let dir = std::env::temp_dir().join(format!("gn-dl-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut jobs = Vec::new();
        for i in 0..400 {
            let dest = dir.join(format!("f{i}.bin"));
            std::fs::write(&dest, vec![7u8; 10]).unwrap();
            // The URL cannot be reached: if any job were downloaded, this test would fail.
            jobs.push(Job { url: "https://unreachable.invalid/x".into(), dest, sha1: Some("0".repeat(40)), size: Some(10) });
        }
        let last = std::sync::atomic::AtomicUsize::new(0);
        let client = reqwest::Client::new();
        download_all(&client, jobs, 8, |p| last.store(p.done * 1000 + p.total, std::sync::atomic::Ordering::Relaxed)).await.unwrap();
        assert_eq!(last.load(std::sync::atomic::Ordering::Relaxed), 400 * 1000 + 400, "progress must end at total/total");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn a_missing_file_is_still_fetched_and_a_wrong_size_counts_as_missing() {
        let dir = std::env::temp_dir().join(format!("gn-dl2-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let wrong = dir.join("wrong.bin");
        std::fs::write(&wrong, b"abc").unwrap();
        // http is refused instantly, which proves a download was attempted without waiting for retries
        let job = Job { url: "http://example.invalid/x".into(), dest: wrong, sha1: None, size: Some(10) };
        assert!(download_all(&reqwest::Client::new(), vec![job], 2, |_| {}).await.is_err(), "a wrong-sized file must be re-downloaded");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    #[ignore = "benchmark: prints timings"]
    async fn bench_prevalidation_old_vs_new() {
        let dir = std::env::temp_dir().join(format!("gn-bench-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let jobs: Vec<Job> = (0..4000)
            .map(|i| {
                let dest = dir.join(format!("f{i}.bin"));
                std::fs::write(&dest, vec![1u8; 2048]).unwrap();
                Job { url: "https://unreachable.invalid/x".into(), dest, sha1: Some("0".repeat(40)), size: Some(2048) }
            })
            .collect();
        let client = reqwest::Client::new();
        // old behaviour: every job goes through download_one at concurrency 8
        let t = std::time::Instant::now();
        let mut results = stream::iter(jobs.clone()).map(|j| { let c = client.clone(); async move { download_one(&c, &j).await } }).buffer_unordered(8);
        while let Some(r) = results.next().await {
            r.unwrap();
        }
        let old = t.elapsed();
        let t = std::time::Instant::now();
        download_all(&client, jobs, 8, |_| {}).await.unwrap();
        println!("4000 existing files: old path {old:?}, new path {:?}", t.elapsed());
        let _ = std::fs::remove_dir_all(dir);
    }
}
