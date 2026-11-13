use std::path::{Path, PathBuf};

/// All launcher data lives under one root. Libraries/assets are shared between instances.
#[derive(Clone, Debug)]
pub struct Dirs {
    pub root: PathBuf,
}

/// On Windows a child process would otherwise open its own console window next to the launcher.
pub fn hide_console(cmd: &mut tokio::process::Command) {
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000);
    #[cfg(not(windows))]
    let _ = cmd;
}

impl Dirs {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
    pub fn default_root() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(crate::APP_NAME)
    }
    pub fn instances(&self) -> PathBuf { self.root.join("instances") }
    pub fn instance(&self, id: &str) -> PathBuf { self.instances().join(id) }
    pub fn libraries(&self) -> PathBuf { self.root.join("libraries") }
    pub fn assets(&self) -> PathBuf { self.root.join("assets") }
    pub fn versions(&self) -> PathBuf { self.root.join("versions") }
    pub fn runtimes(&self) -> PathBuf { self.root.join("runtimes") }
    pub fn cache(&self) -> PathBuf { self.root.join("cache") }
    pub fn settings_file(&self) -> PathBuf { self.root.join("settings.json") }

    pub async fn ensure(&self) -> std::io::Result<()> {
        for d in [self.instances(), self.libraries(), self.assets(), self.versions(), self.runtimes(), self.cache()] {
            tokio::fs::create_dir_all(d).await?;
        }
        Ok(())
    }
}

/// A switched-off mod or pack keeps its name with this suffix, as other launchers do.
pub const DISABLED: &str = ".disabled";

/// The file name without the `.disabled` suffix.
pub fn base_name(file: &str) -> &str {
    file.strip_suffix(DISABLED).unwrap_or(file)
}

/// Whether `file` (switched off or not) ends in `ext`, in any letter case.
pub fn has_extension(file: &str, ext: &str) -> bool {
    std::path::Path::new(base_name(file)).extension().is_some_and(|e| e.eq_ignore_ascii_case(ext))
}

/// Instance ids are used as directory names, so they are restricted hard.
pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// Asset index ids are Minecraft version names like "1.12" or "legacy"; they end up in file names.
pub fn asset_id_ok(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && !id.starts_with('.')
        && !id.contains("..")
        && id.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

pub fn file_name_ok(name: &str) -> bool {
    let p = Path::new(name);
    !name.is_empty()
        && name.len() <= 200
        && p.file_name().is_some_and(|f| f == std::ffi::OsStr::new(name))
        && name != "."
        && name != ".."
}

#[cfg(test)]
mod asset_id_tests {
    use super::asset_id_ok;

    #[test]
    fn version_like_asset_ids_are_accepted() {
        for id in ["1.12", "1.7.10", "legacy", "pre-1.6", "26.1", "12"] {
            assert!(asset_id_ok(id), "{id}");
        }
    }

    #[test]
    fn path_tricks_are_rejected() {
        for id in ["", ".", "..", "../x", "a/b", "a\\b", ".hidden", "1..2", "a b"] {
            assert!(!asset_id_ok(id), "{id}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extensions_match_in_any_case_and_when_switched_off() {
        assert!(has_extension("Sodium.JAR", "jar"));
        assert!(has_extension("sodium.jar.disabled", "jar"));
        assert!(!has_extension("sodium.jar.zip", "jar"));
        assert!(!has_extension("jar", "jar"));
        assert_eq!(base_name("pack.zip.disabled"), "pack.zip");
    }
}
