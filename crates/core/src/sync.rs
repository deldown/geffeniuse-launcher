//! Shares Minecraft's own settings between modpacks, so FOV, controls and the like are set once.
//! Before a modpack starts it takes over the shared values; when it ends, its values flow back.
//! Per-modpack things (the resource pack list, the game version, the last server) are never shared.
//! Old versions (before 1.13) write controls differently, so they keep a separate shared file.
use crate::instance::{self, Instance};
use crate::paths::Dirs;
use anyhow::Result;
use std::path::{Path, PathBuf};

const MAX_BYTES: u64 = 1024 * 1024;
const PER_MODPACK: [&str; 4] = ["resourcePacks", "incompatibleResourcePacks", "version", "lastServer"];

/// "legacy" for 1.12 and older, "modern" for everything since (including the new year-based versions).
pub fn era(mc_version: &str) -> &'static str {
    let mut parts = mc_version.split('.');
    match (parts.next(), parts.next().and_then(|m| m.parse::<u32>().ok())) {
        (Some("1"), Some(minor)) if minor < 13 => "legacy",
        _ => "modern",
    }
}

fn key_of(line: &str) -> Option<&str> {
    line.split_once(':').map(|(k, _)| k)
}

/// `base` with every non-excluded value of `overlay` applied; keys only the overlay has are appended.
pub fn merge(base: &str, overlay: &str) -> String {
    let skipped = |k: &str| PER_MODPACK.contains(&k);
    let wanted: std::collections::HashMap<&str, &str> = overlay.lines().filter_map(|l| key_of(l).filter(|k| !skipped(k)).map(|k| (k, l))).collect();
    let base_keys: std::collections::HashSet<&str> = base.lines().filter_map(key_of).collect();
    let mut out: Vec<&str> = base.lines().map(|line| key_of(line).and_then(|k| wanted.get(k).copied()).unwrap_or(line)).collect();
    out.extend(overlay.lines().filter(|l| key_of(l).is_some_and(|k| !skipped(k) && !base_keys.contains(k))));
    let mut s = out.join("\n");
    s.push('\n');
    s
}

fn shared_dir(dirs: &Dirs) -> PathBuf {
    dirs.root.join("shared")
}

struct Pair {
    instance: PathBuf,
    shared: PathBuf,
    text: bool,
}

fn pairs(dirs: &Dirs, inst: &Instance) -> Vec<Pair> {
    let game = instance::game_dir(dirs, &inst.id);
    let shared = shared_dir(dirs);
    vec![
        Pair { instance: game.join("options.txt"), shared: shared.join(format!("options-{}.txt", era(&inst.mc_version))), text: true },
        Pair { instance: game.join("optionsof.txt"), shared: shared.join("optionsof.txt"), text: true },
        Pair { instance: game.join("servers.dat"), shared: shared.join("servers.dat"), text: false },
    ]
}

async fn small(path: &Path) -> Option<Vec<u8>> {
    let meta = tokio::fs::metadata(path).await.ok()?;
    if !meta.is_file() || meta.len() > MAX_BYTES {
        return None;
    }
    tokio::fs::read(path).await.ok()
}

async fn write(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(dir) = path.parent() {
        tokio::fs::create_dir_all(dir).await?;
    }
    let tmp = path.with_extension("tmp");
    tokio::fs::write(&tmp, bytes).await?;
    tokio::fs::rename(&tmp, path).await?;
    Ok(())
}

/// Shared settings into the modpack, before it starts. Returns the file names that changed.
pub async fn push_to_instance(dirs: &Dirs, inst: &Instance) -> Result<Vec<String>> {
    let mut changed = Vec::new();
    for p in pairs(dirs, inst) {
        let Some(shared) = small(&p.shared).await else { continue };
        let current = small(&p.instance).await;
        let next = if p.text {
            let base = current.as_deref().map(String::from_utf8_lossy).unwrap_or_default();
            merge(&base, &String::from_utf8_lossy(&shared)).into_bytes()
        } else {
            shared
        };
        if current.as_deref() == Some(next.as_slice()) {
            continue;
        }
        if let Some(old) = &current {
            let backup = p.instance.with_extension("geffeniuse-backup");
            if tokio::fs::metadata(&backup).await.is_err() {
                let _ = tokio::fs::write(&backup, old).await;
            }
        }
        write(&p.instance, &next).await?;
        changed.push(p.instance.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default());
    }
    Ok(changed)
}

/// The modpack's settings into the shared ones, after it ended.
pub async fn pull_from_instance(dirs: &Dirs, inst: &Instance) -> Result<Vec<String>> {
    let mut changed = Vec::new();
    for p in pairs(dirs, inst) {
        let Some(mine) = small(&p.instance).await else { continue };
        let shared = small(&p.shared).await;
        let next = if p.text {
            let base = shared.as_deref().map(String::from_utf8_lossy).unwrap_or_default();
            merge(&base, &String::from_utf8_lossy(&mine)).into_bytes()
        } else {
            mine
        };
        if shared.as_deref() != Some(next.as_slice()) {
            write(&p.shared, &next).await?;
            changed.push(p.shared.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default());
        }
    }
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inst(id: &str, mc: &str) -> Instance {
        serde_json::from_value(serde_json::json!({
            "id": id, "name": id, "mc_version": mc, "loader": "fabric", "loader_version": null, "memory_mb": null,
            "java_path": null, "jvm_args": null, "created": 0, "source": null
        }))
        .unwrap()
    }

    #[test]
    fn versions_before_1_13_are_legacy() {
        assert_eq!(era("1.12.2"), "legacy");
        assert_eq!(era("1.8.9"), "legacy");
        for v in ["1.13", "1.21.1", "26.3", "25w14a", "1.16.5"] {
            assert_eq!(era(v), "modern", "{v}");
        }
    }

    #[test]
    fn merge_takes_shared_values_but_keeps_per_modpack_keys() {
        let mine = "fov:70.0\nresourcePacks:[\"mine\"]\nversion:3955\nguiScale:2\n";
        let shared = "fov:90.0\nresourcePacks:[\"theirs\"]\nversion:1\nkey_key.jump:key.keyboard.space\n";
        assert_eq!(merge(mine, shared), "fov:90.0\nresourcePacks:[\"mine\"]\nversion:3955\nguiScale:2\nkey_key.jump:key.keyboard.space\n");
        assert_eq!(merge("", "fov:90.0\nlastServer:x\n"), "fov:90.0\n");
    }

    #[tokio::test]
    async fn settings_flow_from_one_modpack_through_the_shared_copy_into_another() {
        let root = std::env::temp_dir().join(format!("gn-sync-{}", uuid::Uuid::new_v4()));
        let dirs = Dirs::new(&root);
        let (a, b, old) = (inst("a", "1.21.1"), inst("b", "1.20.1"), inst("old", "1.8.9"));
        for i in [&a, &b, &old] {
            std::fs::create_dir_all(instance::game_dir(&dirs, &i.id)).unwrap();
        }
        let file = |i: &Instance, n: &str| instance::game_dir(&dirs, &i.id).join(n);
        std::fs::write(file(&a, "options.txt"), "fov:90.0\nresourcePacks:[\"a\"]\n").unwrap();
        std::fs::write(file(&a, "servers.dat"), b"servers-of-a").unwrap();
        std::fs::write(file(&b, "options.txt"), "fov:60.0\nresourcePacks:[\"b\"]\nguiScale:3\n").unwrap();
        std::fs::write(file(&old, "options.txt"), "fov:50.0\n").unwrap();

        assert_eq!(pull_from_instance(&dirs, &a).await.unwrap().len(), 2);
        let changed = push_to_instance(&dirs, &b).await.unwrap();
        assert_eq!(changed, ["options.txt", "servers.dat"]);
        assert_eq!(std::fs::read_to_string(file(&b, "options.txt")).unwrap(), "fov:90.0\nresourcePacks:[\"b\"]\nguiScale:3\n");
        assert_eq!(std::fs::read(file(&b, "servers.dat")).unwrap(), b"servers-of-a");
        assert!(file(&b, "options.geffeniuse-backup").is_file(), "the old file is kept once");
        assert!(push_to_instance(&dirs, &b).await.unwrap().is_empty(), "a second push changes nothing");
        // an old version never receives the modern controls
        push_to_instance(&dirs, &old).await.unwrap();
        assert_eq!(std::fs::read_to_string(file(&old, "options.txt")).unwrap(), "fov:50.0\n");
        let _ = std::fs::remove_dir_all(root);
    }
}
