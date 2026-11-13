//! Remembers which game processes this launcher started, so a restarted launcher still knows
//! that Minecraft is running, can stop it, and can finish the bookkeeping when it ends.
use crate::instance;
use crate::paths::{valid_id, Dirs};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct RunState {
    pub pid: u32,
    /// Unix seconds when the game started.
    pub started: u64,
    /// Unix seconds up to which the play time has been saved.
    pub last_saved: u64,
}

pub fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn file(dirs: &Dirs, id: &str) -> PathBuf {
    dirs.instance(id).join("running.json")
}

pub async fn write(dirs: &Dirs, id: &str, state: &RunState) -> Result<()> {
    anyhow::ensure!(valid_id(id), "bad instance id");
    let path = file(dirs, id);
    let tmp = path.with_extension("json.tmp");
    tokio::fs::write(&tmp, serde_json::to_vec(state)?).await?;
    tokio::fs::rename(&tmp, &path).await?;
    Ok(())
}

pub async fn read(dirs: &Dirs, id: &str) -> Option<RunState> {
    if !valid_id(id) {
        return None;
    }
    serde_json::from_slice(&tokio::fs::read(file(dirs, id)).await.ok()?).ok()
}

pub async fn clear(dirs: &Dirs, id: &str) {
    if valid_id(id) {
        let _ = tokio::fs::remove_file(file(dirs, id)).await;
    }
}

/// The launcher saves `started` right after spawning, so a process that began much earlier or later
/// is a stranger that got the same number, typically after a reboot.
const START_TOLERANCE: u64 = 120;

/// Is `pid` still the game of this instance, started at `started` (Unix seconds)?
pub fn alive(pid: u32, game_dir: &Path, started: u64) -> bool {
    same_program(pid, game_dir) && process_start(pid).is_some_and(|t| t.abs_diff(started) <= START_TOLERANCE)
}

/// On Linux the command line must mention the game folder; on Windows it must be a Java process.
fn same_program(pid: u32, game_dir: &Path) -> bool {
    #[cfg(target_os = "linux")]
    {
        let Some(dir) = game_dir.to_str() else { return false };
        std::fs::read(format!("/proc/{pid}/cmdline")).is_ok_and(|b| String::from_utf8_lossy(&b).replace('\0', " ").contains(dir))
    }
    #[cfg(target_os = "windows")]
    {
        let _ = game_dir;
        windows::is_java_running(pid)
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        let _ = (pid, game_dir);
        false
    }
}

#[cfg(target_os = "linux")]
fn process_start(pid: u32) -> Option<u64> {
    // Field 22 of /proc/<pid>/stat counts clock ticks since boot; USER_HZ is 100 on Linux.
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let ticks: u64 = stat.rsplit_once(')')?.1.split_whitespace().nth(19)?.parse().ok()?;
    let boot: u64 = std::fs::read_to_string("/proc/stat").ok()?.lines().find_map(|l| l.strip_prefix("btime "))?.trim().parse().ok()?;
    Some(boot + ticks / 100)
}

#[cfg(target_os = "windows")]
fn process_start(pid: u32) -> Option<u64> {
    windows::creation_time(pid)
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
fn process_start(_pid: u32) -> Option<u64> {
    None
}

/// Asks the game process to end (and, on Windows, its children); `force` does not let it save first.
pub fn kill(pid: u32, force: bool) {
    #[cfg(target_os = "windows")]
    let _ = force;
    #[cfg(target_os = "windows")]
    let mut cmd = {
        use std::os::windows::process::CommandExt;
        let mut c = std::process::Command::new("taskkill");
        c.args(["/PID", &pid.to_string(), "/T", "/F"]).creation_flags(0x0800_0000);
        c
    };
    #[cfg(not(target_os = "windows"))]
    let mut cmd = {
        let mut c = std::process::Command::new("kill");
        c.args([if force { "-KILL" } else { "-TERM" }, &pid.to_string()]);
        c
    };
    let _ = cmd.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status();
}

#[cfg(target_os = "windows")]
mod windows {
    #[link(name = "kernel32")]
    extern "system" {
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut core::ffi::c_void;
        fn GetExitCodeProcess(handle: *mut core::ffi::c_void, code: *mut u32) -> i32;
        fn QueryFullProcessImageNameW(handle: *mut core::ffi::c_void, flags: u32, name: *mut u16, len: *mut u32) -> i32;
        fn CloseHandle(handle: *mut core::ffi::c_void) -> i32;
        fn GetProcessTimes(handle: *mut core::ffi::c_void, creation: *mut u64, exit: *mut u64, kernel: *mut u64, user: *mut u64) -> i32;
    }

    /// Unix seconds at which the process was created.
    pub fn creation_time(pid: u32) -> Option<u64> {
        const QUERY_LIMITED: u32 = 0x1000;
        // FILETIME counts 100 ns steps since 1601-01-01.
        const EPOCH_DIFF: u64 = 11_644_473_600;
        unsafe {
            let h = OpenProcess(QUERY_LIMITED, 0, pid);
            if h.is_null() {
                return None;
            }
            let (mut creation, mut exit, mut kernel, mut user) = (0u64, 0u64, 0u64, 0u64);
            let ok = GetProcessTimes(h, &mut creation, &mut exit, &mut kernel, &mut user) != 0;
            CloseHandle(h);
            (ok && creation > 0).then(|| (creation / 10_000_000).saturating_sub(EPOCH_DIFF))
        }
    }

    pub fn is_java_running(pid: u32) -> bool {
        const QUERY_LIMITED: u32 = 0x1000;
        const STILL_ACTIVE: u32 = 259;
        unsafe {
            let h = OpenProcess(QUERY_LIMITED, 0, pid);
            if h.is_null() {
                return false;
            }
            let mut code = 0u32;
            let mut buf = [0u16; 520];
            let mut len = buf.len() as u32;
            let ok = GetExitCodeProcess(h, &mut code) != 0 && code == STILL_ACTIVE && QueryFullProcessImageNameW(h, 0, buf.as_mut_ptr(), &mut len) != 0;
            let name = String::from_utf16_lossy(&buf[..len as usize]).to_lowercase();
            CloseHandle(h);
            ok && name.contains("java")
        }
    }
}

#[derive(Clone, Debug)]
pub struct Found {
    pub id: String,
    pub state: RunState,
    /// False when the game ended while the launcher was closed.
    pub alive: bool,
}

/// Everything the previous launcher session left behind.
pub async fn scan(dirs: &Dirs) -> Vec<Found> {
    let mut out = Vec::new();
    for inst in instance::list(dirs).await.unwrap_or_default() {
        if let Some(state) = read(dirs, &inst.id).await {
            let game = instance::game_dir(dirs, &inst.id);
            out.push(Found { alive: alive(state.pid, &game, state.started), id: inst.id, state });
        }
    }
    out
}

/// For a game that ended unseen: when it probably ended. Minecraft writes options.txt on exit.
pub fn ended_at(dirs: &Dirs, id: &str, state: &RunState) -> u64 {
    let options = instance::game_dir(dirs, id).join("options.txt");
    let modified = std::fs::metadata(options).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs());
    modified.clamp(state.last_saved, now())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn the_state_file_roundtrips_and_clears() {
        let root = std::env::temp_dir().join(format!("gn-run-{}", uuid::Uuid::new_v4()));
        let dirs = Dirs::new(&root);
        let inst = instance::create(&dirs, "R", "1.21.1", instance::Loader::Fabric, None, None).await.unwrap();
        assert_eq!(read(&dirs, &inst.id).await, None);
        let s = RunState { pid: 4242, started: 100, last_saved: 160 };
        write(&dirs, &inst.id, &s).await.unwrap();
        assert_eq!(read(&dirs, &inst.id).await, Some(s));
        let found = scan(&dirs).await;
        assert_eq!((found.len(), found[0].alive), (1, false), "pid 4242 is not this instance's game");
        clear(&dirs, &inst.id).await;
        assert_eq!(read(&dirs, &inst.id).await, None);
        assert!(write(&dirs, "../x", &s).await.is_err());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn a_game_end_is_never_dated_before_the_last_save_or_after_now() {
        let root = std::env::temp_dir().join(format!("gn-end-{}", uuid::Uuid::new_v4()));
        let dirs = Dirs::new(&root);
        let s = RunState { pid: 1, started: 10, last_saved: 50 };
        let t = ended_at(&dirs, "none", &s);
        assert!(t >= 50 && t <= now());
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn a_live_process_is_recognised_by_its_game_folder_and_a_stranger_is_not() {
        let dir = std::env::temp_dir().join(format!("gn-game-{}", uuid::Uuid::new_v4()));
        let mut child = std::process::Command::new("sh").args(["-c", "sleep 20", "--"]).arg(&dir).spawn().unwrap();
        let started = now();
        for _ in 0..100 {
            if alive(child.id(), &dir, started) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(alive(child.id(), &dir, started));
        assert!(!alive(child.id(), Path::new("/somewhere/else"), started));
        child.kill().unwrap();
        child.wait().unwrap();
        assert!(!alive(child.id(), &dir, started));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_process_from_before_a_reboot_is_not_the_game_even_with_the_same_number() {
        let dir = std::env::temp_dir().join(format!("gn-reboot-{}", uuid::Uuid::new_v4()));
        let mut child = std::process::Command::new("sh").args(["-c", "sleep 20", "--"]).arg(&dir).spawn().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(100));
        assert!(!alive(child.id(), &dir, now() - 86_400), "saved start lies a day before this process");
        assert!(process_start(child.id()).is_some_and(|t| t.abs_diff(now()) <= 5));
        child.kill().unwrap();
        child.wait().unwrap();
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_game_that_ignores_the_stop_request_is_ended_by_force() {
        let mut child = std::process::Command::new("sh").args(["-c", "trap '' TERM; while true; do sleep 0.05; done"]).spawn().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(200));
        kill(child.id(), false);
        std::thread::sleep(std::time::Duration::from_millis(200));
        assert!(child.try_wait().unwrap().is_none(), "TERM is ignored");
        kill(child.id(), true);
        let status = child.wait().unwrap();
        assert!(!status.success());
    }
}
