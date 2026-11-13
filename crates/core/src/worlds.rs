use crate::instance;
use crate::paths::Dirs;
use anyhow::Result;
use serde::Serialize;

#[derive(Serialize, Clone, Debug)]
pub struct World {
    pub instance_id: String,
    pub folder: String,
    pub modified: u64,
    pub size: u64,
}

/// Size of a directory tree, capped on depth and entry count so a hostile save cannot stall the UI.
fn dir_size(path: &std::path::Path, depth: u8, budget: &mut u32) -> u64 {
    if depth > 8 {
        return 0;
    }
    let Ok(rd) = std::fs::read_dir(path) else { return 0 };
    let mut total = 0;
    for e in rd.flatten() {
        if *budget == 0 {
            break;
        }
        *budget -= 1;
        let Ok(ft) = e.file_type() else { continue };
        if ft.is_symlink() {
            continue;
        }
        if ft.is_dir() {
            total += dir_size(&e.path(), depth + 1, budget);
        } else if let Ok(m) = e.metadata() {
            total += m.len();
        }
    }
    total
}

pub fn is_world(dir: &std::path::Path) -> bool {
    dir.join("level.dat").is_file()
}

pub async fn list_instance(dirs: &Dirs, id: &str) -> Result<Vec<World>> {
    if !crate::paths::valid_id(id) {
        anyhow::bail!("bad instance id");
    }
    let saves = instance::game_dir(dirs, id).join("saves");
    let id = id.to_string();
    let mut found = tokio::task::spawn_blocking(move || {
        let mut v = Vec::new();
        let Ok(rd) = std::fs::read_dir(&saves) else { return v };
        for e in rd.flatten().take(500) {
            let p = e.path();
            if !e.file_type().is_ok_and(|t| t.is_dir()) || !is_world(&p) {
                continue;
            }
            let modified = std::fs::metadata(p.join("level.dat"))
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_secs());
            let mut budget = 50_000;
            v.push(World { instance_id: id.clone(), folder: e.file_name().to_string_lossy().to_string(), modified, size: dir_size(&p, 0, &mut budget) });
        }
        v
    })
    .await?;
    found.sort_by_key(|x| std::cmp::Reverse(x.modified));
    Ok(found)
}

pub async fn list(dirs: &Dirs) -> Result<Vec<World>> {
    let mut out = Vec::new();
    for inst in instance::list(dirs).await? {
        out.extend(list_instance(dirs, &inst.id).await?);
    }
    out.sort_by_key(|x| std::cmp::Reverse(x.modified));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_detection_and_size() {
        let d = std::env::temp_dir().join(format!("geff-world-{}", std::process::id()));
        std::fs::create_dir_all(d.join("region")).unwrap();
        assert!(!is_world(&d));
        std::fs::write(d.join("level.dat"), [0u8; 10]).unwrap();
        std::fs::write(d.join("region/r.0.0.mca"), [0u8; 90]).unwrap();
        assert!(is_world(&d));
        let mut budget = 100;
        assert_eq!(dir_size(&d, 0, &mut budget), 100);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn size_budget_stops() {
        let d = std::env::temp_dir().join(format!("geff-budget-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        for i in 0..5 {
            std::fs::write(d.join(format!("f{i}")), [0u8; 1]).unwrap();
        }
        let mut budget = 2;
        assert_eq!(dir_size(&d, 0, &mut budget), 2);
        let _ = std::fs::remove_dir_all(&d);
    }
}
