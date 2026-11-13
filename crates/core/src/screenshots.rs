use crate::instance;
use crate::paths::Dirs;
use anyhow::Result;
use serde::Serialize;

#[derive(Serialize, Clone, Debug)]
pub struct Shot {
    pub instance_id: String,
    pub file: String,
    pub modified: u64,
    pub size: u64,
}

pub async fn list(dirs: &Dirs) -> Result<Vec<Shot>> {
    let mut out = Vec::new();
    for inst in instance::list(dirs).await? {
        let dir = instance::game_dir(dirs, &inst.id).join("screenshots");
        let Ok(mut rd) = tokio::fs::read_dir(&dir).await else { continue };
        while let Some(e) = rd.next_entry().await? {
            let name = e.file_name().to_string_lossy().to_string();
            if !is_image(&name) {
                continue;
            }
            let Ok(m) = e.metadata().await else { continue };
            let modified = m.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs());
            out.push(Shot { instance_id: inst.id.clone(), file: name, modified, size: m.len() });
        }
    }
    out.sort_by_key(|x| std::cmp::Reverse(x.modified));
    Ok(out)
}

pub fn is_image(name: &str) -> bool {
    let l = name.to_ascii_lowercase();
    crate::paths::file_name_ok(name) && (l.ends_with(".png") || l.ends_with(".webp") || l.ends_with(".jpg"))
}

/// Resolve a screenshot to an on-disk path, rejecting anything that is not a plain file name.
pub fn path_of(dirs: &Dirs, instance_id: &str, file: &str) -> Result<std::path::PathBuf> {
    if !crate::paths::valid_id(instance_id) || !is_image(file) {
        anyhow::bail!("bad screenshot reference");
    }
    Ok(instance::game_dir(dirs, instance_id).join("screenshots").join(file))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert!(is_image("2026-10-28_10.00.00.png"));
        assert!(!is_image("../x.png"));
        assert!(!is_image("a/b.png"));
        assert!(!is_image("x.exe"));
    }
}
