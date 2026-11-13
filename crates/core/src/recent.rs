//! The worlds and servers played last, across all modpacks, for the start page.
use crate::instance::{self, Instance};
use crate::paths::{file_name_ok, Dirs};
use anyhow::Result;
use serde::Serialize;

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct Recent {
    /// "world" or "server".
    pub kind: &'static str,
    pub instance_id: String,
    pub instance_name: String,
    pub mc_version: String,
    pub loader: instance::Loader,
    pub icon: Option<String>,
    /// World folder or server address.
    pub target: String,
    /// Seconds since 1970 of the last play.
    pub when: u64,
    /// The world's or server's own picture as a `data:` URL, when it has one.
    pub picture: Option<String>,
}

fn secs(path: &std::path::Path) -> u64 {
    std::fs::metadata(path).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs())
}

/// Servers seen in the game's own log ("Connecting to host, port"), newest first, kept per modpack.
pub fn server_from_log(line: &str) -> Option<String> {
    let rest = line.split("Connecting to ").nth(1)?;
    let (host, port) = rest.trim().split_once(", ")?;
    let port: String = port.chars().take_while(char::is_ascii_digit).collect();
    let addr = if port.is_empty() || port == "25565" { host.trim().to_string() } else { format!("{}:{}", host.trim(), port) };
    crate::launch::server_addr_ok(&addr).then_some(addr)
}

fn servers_file(dirs: &Dirs, id: &str) -> std::path::PathBuf {
    dirs.instance(id).join("servers-played.json")
}

pub fn remember_server(dirs: &Dirs, id: &str, addr: &str) {
    if !crate::paths::valid_id(id) || !crate::launch::server_addr_ok(addr) {
        return;
    }
    let path = servers_file(dirs, id);
    let mut list: Vec<(String, u64)> = std::fs::read(&path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    list.retain(|(a, _)| a != addr);
    list.insert(0, (addr.to_string(), crate::running::now()));
    list.truncate(5);
    if let Ok(b) = serde_json::to_vec(&list) {
        let _ = std::fs::write(path, b);
    }
}

/// The server the game joined last, as written in options.txt (`lastServer:host`).
fn last_server(options: &str) -> Option<String> {
    options.lines().find_map(|l| l.strip_prefix("lastServer:")).map(|v| v.trim().to_string()).filter(|v| crate::launch::server_addr_ok(v))
}

const MAX_PICTURE: u64 = 200 * 1024;

fn data_url(png: &[u8]) -> String {
    format!("data:image/png;base64,{}", crate::skin::base64_encode(png))
}

/// Server entries of a servers.dat (uncompressed NBT): (address, base64 icon). Anything unexpected ends the walk.
pub fn server_icons(nbt: &[u8]) -> Vec<(String, String)> {
    struct Rd<'a>(&'a [u8], usize);
    impl<'a> Rd<'a> {
        fn take(&mut self, n: usize) -> Option<&'a [u8]> {
            let end = self.1.checked_add(n).filter(|e| *e <= self.0.len())?;
            let out = &self.0[self.1..end];
            self.1 = end;
            Some(out)
        }
        fn u16(&mut self) -> Option<usize> {
            self.take(2).map(|b| u16::from_be_bytes([b[0], b[1]]) as usize)
        }
        fn i32(&mut self) -> Option<usize> {
            self.take(4).map(|b| i32::from_be_bytes([b[0], b[1], b[2], b[3]]).max(0) as usize)
        }
        fn string(&mut self) -> Option<String> {
            let n = self.u16()?;
            self.take(n).map(|b| String::from_utf8_lossy(b).to_string())
        }
        fn skip(&mut self, ty: u8, depth: u8) -> Option<()> {
            if depth > 12 {
                return None;
            }
            match ty {
                1 => self.take(1).map(|_| ()),
                2 => self.take(2).map(|_| ()),
                3 | 5 => self.take(4).map(|_| ()),
                4 | 6 => self.take(8).map(|_| ()),
                7 => { let n = self.i32()?; self.take(n).map(|_| ()) }
                8 => self.string().map(|_| ()),
                9 => {
                    let t = *self.take(1)?.first()?;
                    let n = self.i32()?;
                    for _ in 0..n.min(100_000) { self.skip(t, depth + 1)?; }
                    Some(())
                }
                10 => loop {
                    let t = *self.take(1)?.first()?;
                    if t == 0 { return Some(()); }
                    self.string()?;
                    self.skip(t, depth + 1)?;
                },
                11 => { let n = self.i32()?; self.take(n.checked_mul(4)?).map(|_| ()) }
                12 => { let n = self.i32()?; self.take(n.checked_mul(8)?).map(|_| ()) }
                _ => None,
            }
        }
    }
    let mut r = Rd(nbt, 0);
    let mut out = Vec::new();
    let _ = (|| -> Option<()> {
        if *r.take(1)?.first()? != 10 { return None; }
        r.string()?;
        loop {
            let t = *r.take(1)?.first()?;
            if t == 0 { return Some(()); }
            let name = r.string()?;
            if name == "servers" && t == 9 {
                let et = *r.take(1)?.first()?;
                let n = r.i32()?;
                for _ in 0..n.min(500) {
                    if et != 10 { r.skip(et, 1)?; continue; }
                    let (mut ip, mut icon) = (None, None);
                    loop {
                        let t = *r.take(1)?.first()?;
                        if t == 0 { break; }
                        let key = r.string()?;
                        match (t, key.as_str()) {
                            (8, "ip") => ip = Some(r.string()?),
                            (8, "icon") => icon = Some(r.string()?),
                            _ => r.skip(t, 2)?,
                        }
                    }
                    if let (Some(ip), Some(icon)) = (ip, icon) { out.push((ip, icon)); }
                }
            } else {
                r.skip(t, 1)?;
            }
        }
    })();
    out
}

fn same_server(a: &str, b: &str) -> bool {
    let norm = |x: &str| x.trim().to_lowercase().trim_end_matches(":25565").to_string();
    norm(a) == norm(b)
}

fn scan(dirs: &Dirs, inst: &Instance) -> Vec<Recent> {
    let game = instance::game_dir(dirs, &inst.id);
    let mut out = Vec::new();
    let item = |kind, target: String, when| Recent {
        kind,
        instance_id: inst.id.clone(),
        instance_name: inst.name.clone(),
        mc_version: inst.mc_version.clone(),
        loader: inst.loader,
        icon: inst.icon.clone(),
        target,
        when,
        picture: None,
    };
    if let Ok(rd) = std::fs::read_dir(game.join("saves")) {
        for e in rd.flatten().take(200) {
            let name = e.file_name().to_string_lossy().to_string();
            if file_name_ok(&name) && crate::worlds::is_world(&e.path()) {
                let mut r = item("world", name, secs(&e.path().join("level.dat")));
                let icon = e.path().join("icon.png");
                if std::fs::metadata(&icon).is_ok_and(|m| m.is_file() && m.len() <= MAX_PICTURE) {
                    r.picture = std::fs::read(icon).ok().map(|b| data_url(&b));
                }
                out.push(r);
            }
        }
    }
    let played: Vec<(String, u64)> = std::fs::read(servers_file(dirs, &inst.id)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    for (addr, when) in played {
        out.push(item("server", addr, when));
    }
    let options = game.join("options.txt");
    if let Some(addr) = std::fs::read_to_string(&options).ok().as_deref().and_then(last_server) {
        if !out.iter().any(|r| r.kind == "server" && r.target == addr) {
            out.push(item("server", addr, secs(&options)));
        }
    }
    out
}

/// Newest first; a world or server that was played in several modpacks shows up once per modpack.
pub async fn list(dirs: &Dirs, limit: usize) -> Result<Vec<Recent>> {
    let instances = instance::list(dirs).await?;
    let scan_dirs = dirs.clone();
    let mut all = tokio::task::spawn_blocking(move || instances.iter().flat_map(|i| scan(&scan_dirs, i)).collect::<Vec<_>>()).await?;
    all.sort_by_key(|r| std::cmp::Reverse(r.when));
    all.truncate(limit.min(12));
    let dirs = dirs.clone();
    // Server pictures live in servers.dat; only the few that are shown are looked up.
    let all = tokio::task::spawn_blocking(move || {
        for r in all.iter_mut().filter(|r| r.kind == "server") {
            let file = instance::game_dir(&dirs, &r.instance_id).join("servers.dat");
            if !std::fs::metadata(&file).is_ok_and(|m| m.len() <= 4 * 1024 * 1024) {
                continue;
            }
            let found = std::fs::read(file).ok().map(|b| server_icons(&b)).and_then(|l| l.into_iter().find(|(ip, _)| same_server(ip, &r.target)));
            if let Some((_, icon)) = found {
                if icon.len() <= 300_000 && icon.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '=')) {
                    r.picture = Some(format!("data:image/png;base64,{icon}"));
                }
            }
        }
        all
    })
    .await?;
    Ok(all)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn last_server_is_read_and_checked() {
        assert_eq!(last_server("fov:70\nlastServer:mc.example.net:25565\n").as_deref(), Some("mc.example.net:25565"));
        assert_eq!(last_server("lastServer:\n"), None);
        assert_eq!(last_server("lastServer:--demo\n"), None);
        assert_eq!(last_server("fov:70\n"), None);
    }

    #[test]
    fn server_pictures_are_found_in_servers_dat() {
        let mut n: Vec<u8> = vec![10, 0, 0, 9, 0, 7];
        n.extend(b"servers");
        n.extend([10, 0, 0, 0, 2]);
        for (ip, icon) in [("a.example.net", "QUJD"), ("b.example.net:25570", "REVG")] {
            for (key, val) in [("name", "x"), ("ip", ip), ("icon", icon)] {
                n.extend([8, 0, key.len() as u8]);
                n.extend(key.as_bytes());
                n.extend([0, val.len() as u8]);
                n.extend(val.as_bytes());
            }
            n.extend([1, 0, 6]);
            n.extend(b"hidden");
            n.push(1);
            n.push(0);
        }
        n.push(0);
        assert_eq!(server_icons(&n), [("a.example.net".to_string(), "QUJD".to_string()), ("b.example.net:25570".to_string(), "REVG".to_string())]);
        assert!(server_icons(&n[..20]).is_empty() || server_icons(&n[..20]).len() < 2);
        assert!(server_icons(b"garbage").is_empty());
        assert!(same_server("MC.Example.net:25565", "mc.example.net") && !same_server("a.net", "b.net"));
    }

    #[test]
    fn servers_are_read_from_the_game_log() {
        assert_eq!(server_from_log("[12:00:01] [Render thread/INFO]: Connecting to mc.example.net, 25565").as_deref(), Some("mc.example.net"));
        assert_eq!(server_from_log("Connecting to play.example.org, 25570").as_deref(), Some("play.example.org:25570"));
        assert_eq!(server_from_log("Connecting to --demo, 1"), None);
        assert_eq!(server_from_log("Loading world"), None);
    }

    #[tokio::test]
    async fn worlds_and_servers_come_back_newest_first() {
        let root = std::env::temp_dir().join(format!("gn-recent-{}", uuid::Uuid::new_v4()));
        let dirs = Dirs::new(&root);
        let inst = instance::create(&dirs, "Pack", "1.21.1", instance::Loader::Fabric, None, None).await.unwrap();
        let game = instance::game_dir(&dirs, &inst.id);
        std::fs::create_dir_all(game.join("saves/Alt")).unwrap();
        std::fs::write(game.join("saves/Alt/level.dat"), [0]).unwrap();
        std::fs::create_dir_all(game.join("saves/Kein Spielstand")).unwrap();
        std::fs::write(game.join("options.txt"), "lastServer:play.example.org\n").unwrap();
        let l = list(&dirs, 5).await.unwrap();
        let mut found: Vec<_> = l.iter().map(|r| (r.kind, r.target.as_str())).collect();
        found.sort();
        assert_eq!(found, [("server", "play.example.org"), ("world", "Alt")]);
        assert_eq!(list(&dirs, 1).await.unwrap().len(), 1);
        let _ = std::fs::remove_dir_all(root);
    }
}
