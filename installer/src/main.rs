#![cfg_attr(windows, windows_subsystem = "windows")]
//! Per-user installer for Geffeniuse Launcher: no admin rights, no dependencies.
//! Without arguments it installs; `--uninstall` (used by the entry in Windows Settings) removes it.

#[cfg(windows)]
mod win {
    use std::ffi::{c_void, OsStr};
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::process::CommandExt;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::{env, fs, io};

    const EXE: &[u8] = include_bytes!("../payload/geffeniuse.exe");
    const LOADER: &[u8] = include_bytes!("../payload/WebView2Loader.dll");
    const VERSION: &str = match option_env!("GEFFENIUSE_VERSION") { Some(v) => v, None => "0" };
    const APP: &str = "Geffeniuse Launcher";
    const UNINSTALL_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\GeffeniuseLauncher";
    const NO_WINDOW: u32 = 0x0800_0000;

    #[link(name = "user32")]
    extern "system" {
        fn MessageBoxW(owner: *mut c_void, text: *const u16, caption: *const u16, kind: u32) -> i32;
    }

    fn wide(s: &str) -> Vec<u16> {
        OsStr::new(s).encode_wide().chain(Some(0)).collect()
    }

    fn ask(text: &str) -> bool {
        unsafe { MessageBoxW(std::ptr::null_mut(), wide(text).as_ptr(), wide(APP).as_ptr(), 0x4 | 0x20) == 6 }
    }

    fn tell(text: &str, error: bool) {
        unsafe { MessageBoxW(std::ptr::null_mut(), wide(text).as_ptr(), wide(APP).as_ptr(), if error { 0x10 } else { 0x40 }); }
    }

    fn quiet(program: &str) -> Command {
        let mut c = Command::new(program);
        c.creation_flags(NO_WINDOW);
        c
    }

    fn install_dir() -> io::Result<PathBuf> {
        let base = env::var_os("LOCALAPPDATA").ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "LOCALAPPDATA fehlt"))?;
        Ok(PathBuf::from(base).join("Programs").join(APP))
    }

    fn shortcut_path() -> io::Result<PathBuf> {
        let base = env::var_os("APPDATA").ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "APPDATA fehlt"))?;
        Ok(PathBuf::from(base).join(r"Microsoft\Windows\Start Menu\Programs").join(format!("{APP}.lnk")))
    }

    // A running launcher cannot be overwritten but can be renamed; the leftover is removed next time.
    fn replace(path: &Path, bytes: &[u8]) -> io::Result<()> {
        let old = path.with_extension("old");
        let _ = fs::remove_file(&old);
        if path.exists() {
            fs::rename(path, &old)?;
        }
        if let Err(e) = fs::write(path, bytes) {
            let _ = fs::rename(&old, path);
            return Err(e);
        }
        let _ = fs::remove_file(&old);
        Ok(())
    }

    fn ps_quote(p: &Path) -> String {
        format!("'{}'", p.to_string_lossy().replace('\'', "''"))
    }

    fn make_shortcut(link: &Path, target: &Path, dir: &Path) -> io::Result<()> {
        if let Some(parent) = link.parent() {
            fs::create_dir_all(parent)?;
        }
        let script = format!(
            "$s=(New-Object -ComObject WScript.Shell).CreateShortcut({}); $s.TargetPath={}; $s.WorkingDirectory={}; $s.Save()",
            ps_quote(link), ps_quote(target), ps_quote(dir));
        let ok = quiet("powershell").args(["-NoProfile", "-NonInteractive", "-Command", &script]).status()?.success();
        if ok { Ok(()) } else { Err(io::Error::new(io::ErrorKind::Other, "Verknüpfung konnte nicht angelegt werden")) }
    }

    fn reg_add(name: &str, kind: &str, value: &str) -> io::Result<()> {
        quiet("reg").args(["add", UNINSTALL_KEY, "/v", name, "/t", kind, "/d", value, "/f"]).status().map(|_| ())
    }

    fn install() -> io::Result<()> {
        let dir = install_dir()?;
        fs::create_dir_all(&dir)?;
        let exe = dir.join("geffeniuse.exe");
        replace(&exe, EXE)?;
        replace(&dir.join("WebView2Loader.dll"), LOADER)?;
        let remover = dir.join("uninstall.exe");
        if env::current_exe()? != remover {
            replace(&remover, &fs::read(env::current_exe()?)?)?;
        }
        make_shortcut(&shortcut_path()?, &exe, &dir)?;
        reg_add("DisplayName", "REG_SZ", APP)?;
        reg_add("DisplayVersion", "REG_SZ", VERSION)?;
        reg_add("Publisher", "REG_SZ", "deldown")?;
        reg_add("InstallLocation", "REG_SZ", &dir.to_string_lossy())?;
        reg_add("DisplayIcon", "REG_SZ", &exe.to_string_lossy())?;
        reg_add("UninstallString", "REG_SZ", &format!("\"{}\" --uninstall", remover.display()))?;
        reg_add("NoModify", "REG_DWORD", "1")?;
        reg_add("NoRepair", "REG_DWORD", "1")?;
        Ok(())
    }

    fn uninstall() -> io::Result<()> {
        let dir = install_dir()?;
        let _ = fs::remove_file(shortcut_path()?);
        let _ = quiet("reg").args(["delete", UNINSTALL_KEY, "/f"]).status();
        for name in ["geffeniuse.exe", "WebView2Loader.dll", "geffeniuse.old", "WebView2Loader.old"] {
            let _ = fs::remove_file(dir.join(name));
        }
        // This very file is still running, so the folder is removed a moment after it exits.
        let cmd = format!("ping -n 3 127.0.0.1 >nul & rmdir /s /q \"{}\"", dir.display());
        quiet("cmd").args(["/c", &cmd]).spawn().map(|_| ())
    }

    pub fn run() {
        if env::args().nth(1).as_deref() == Some("--uninstall") {
            if !ask(&format!("{APP} entfernen?\n\nDeine Instanzen und Einstellungen bleiben erhalten.")) { return; }
            match uninstall() {
                Ok(()) => tell(&format!("{APP} wurde entfernt."), false),
                Err(e) => tell(&format!("Entfernen fehlgeschlagen: {e}"), true),
            }
            return;
        }
        let Ok(dir) = install_dir() else { return tell("Der Installationsordner wurde nicht gefunden.", true) };
        if !ask(&format!("{APP} {VERSION} installieren?\n\nZiel: {}", dir.display())) { return; }
        match install() {
            Ok(()) => {
                if ask(&format!("{APP} ist installiert.\n\nJetzt starten?")) {
                    let _ = Command::new(dir.join("geffeniuse.exe")).current_dir(&dir).spawn();
                }
            }
            Err(e) => tell(&format!("Installation fehlgeschlagen: {e}\n\nSchließe den Launcher und versuche es noch einmal."), true),
        }
    }
}

#[cfg(windows)]
fn main() { win::run() }

#[cfg(not(windows))]
fn main() { eprintln!("This installer is for Windows."); }
