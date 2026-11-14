use crate::instance::{self, Instance, Loader};
use crate::mojang::{self, expand_args, Features};
use crate::paths::Dirs;
use crate::settings::Settings;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Account {
    pub name: String,
    pub uuid: String,
    pub access_token: String,
    pub user_type: String,
}

#[cfg(test)]
impl Account {
    /// Stand-in account for the live tests that start the game without a real login.
    fn for_tests(name: &str) -> Self {
        let id = uuid::Uuid::new_v3(&uuid::Uuid::NAMESPACE_OID, format!("OfflinePlayer:{name}").as_bytes());
        Self { name: name.into(), uuid: id.simple().to_string(), access_token: "0".into(), user_type: "legacy".into() }
    }
}

/// What to open as soon as the game is up.
#[derive(serde::Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(tag = "kind", content = "target", rename_all = "lowercase")]
pub enum Join {
    World(String),
    Server(String),
}

/// Quick Play exists since Minecraft 1.20; the year-based versions after 1.21 have it too.
pub fn supports_quick_play(mc_version: &str) -> bool {
    let mut parts = mc_version.split('.');
    match (parts.next().and_then(|m| m.parse::<u32>().ok()), parts.next().and_then(|m| m.parse::<u32>().ok())) {
        (Some(1), Some(minor)) => minor >= 20,
        (Some(major), _) => major > 1,
        _ => false,
    }
}

/// Server addresses are passed to the game, so only host and port characters are allowed.
pub fn server_addr_ok(addr: &str) -> bool {
    // Never start with '-': the game would read it as an option of its own.
    addr.len() <= 255
        && addr.chars().next().is_some_and(|c| c.is_ascii_alphanumeric() || c == '[')
        && addr.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':' | '_' | '[' | ']'))
}

/// The game arguments for `join`, or None when this Minecraft version cannot do it.
pub fn join_args(mc_version: &str, join: &Join) -> Option<Vec<String>> {
    let quick = supports_quick_play(mc_version);
    match join {
        Join::World(name) if quick && crate::paths::file_name_ok(name) && !name.starts_with('-') => Some(vec!["--quickPlaySingleplayer".into(), name.clone()]),
        Join::Server(addr) if server_addr_ok(addr) => Some(if quick {
            vec!["--quickPlayMultiplayer".into(), addr.clone()]
        } else {
            match addr.rsplit_once(':').filter(|(h, p)| !h.ends_with(']') || p.chars().all(|c| c.is_ascii_digit())) {
                Some((host, port)) if port.chars().all(|c| c.is_ascii_digit()) && !port.is_empty() => vec!["--server".into(), host.into(), "--port".into(), port.into()],
                _ => vec!["--server".into(), addr.clone()],
            }
        }),
        _ => None,
    }
}

pub struct Prepared {
    pub java: std::path::PathBuf,
    pub args: Vec<String>,
    pub game_dir: std::path::PathBuf,
}

fn sub(s: &str, vars: &[(&str, String)]) -> String {
    let mut out = s.to_string();
    for (k, v) in vars {
        out = out.replace(&format!("${{{k}}}"), v);
    }
    out
}

/// Folder for the native libraries of a game window; every window of a modpack needs its own.
pub fn natives_dir(dirs: &Dirs, id: &str, slot: u32) -> std::path::PathBuf {
    dirs.instance(id).join(if slot == 0 { "natives".to_string() } else { format!("natives-{slot}") })
}

/// Installs everything the instance needs and builds the java command line.
pub async fn prepare(
    dirs: &Dirs,
    client: &reqwest::Client,
    settings: &Settings,
    inst: &Instance,
    account: &Account,
    on_progress: impl Fn(&str, crate::download::Progress) + Send + Sync,
) -> Result<Prepared> {
    prepare_slot(dirs, client, settings, inst, std::future::ready(Ok(account.clone())), 0, on_progress).await
}

/// Like `prepare`, for the `slot`-th window of the same modpack (0 is the first).
pub async fn prepare_slot(
    dirs: &Dirs,
    client: &reqwest::Client,
    settings: &Settings,
    inst: &Instance,
    account: impl std::future::Future<Output = Result<Account>>,
    slot: u32,
    on_progress: impl Fn(&str, crate::download::Progress) + Send + Sync,
) -> Result<Prepared> {
    let on_progress = &on_progress;
    // Signing in takes several requests to Microsoft; the files are checked at the same time.
    let files = async {
        let vid = match inst.loader {
            Loader::Vanilla => inst.mc_version.clone(),
            Loader::Fabric | Loader::Quilt => {
                let lv = match &inst.loader_version {
                    Some(v) => v.clone(),
                    None => crate::loaders::latest_loader(client, inst.loader, &inst.mc_version).await?,
                };
                crate::loaders::install_profile(dirs, client, inst.loader, &inst.mc_version, &lv).await?
            }
            Loader::Forge | Loader::Neoforge => {
                let lv = match &inst.loader_version {
                    Some(v) => v.clone(),
                    None => crate::loaders::latest_loader(client, inst.loader, &inst.mc_version).await?,
                };
                // The installer needs the same Java the game will use.
                let vanilla = mojang::resolve_version(dirs, client, &inst.mc_version).await?;
                let major = vanilla.java_version.as_ref().map_or(8, |j| j.major_version);
                let preferred = inst.java_path.as_deref().or(settings.java_path.as_deref());
                let comp = vanilla.java_version.as_ref().and_then(|j| j.component.clone());
                let java = crate::java::find_or_install(client, major, comp.as_deref(), preferred, &dirs.runtimes(), |p| on_progress("java", p)).await?;
                crate::loaders::install_forge_like(dirs, client, inst.loader, &inst.mc_version, &lv, &java).await?
            }
        };
        let v = mojang::resolve_version(dirs, client, &vid).await?;
        let game_dir = instance::game_dir(dirs, &inst.id);
        tokio::fs::create_dir_all(&game_dir).await?;
        let libs = mojang::install_version(dirs, client, &v, &game_dir, settings.parallel_downloads as usize, on_progress).await?;

        let natives_dir = natives_dir(dirs, &inst.id, slot);
        let _ = tokio::fs::remove_dir_all(&natives_dir).await;
        mojang::extract_natives(&libs, &natives_dir).await?;

        let major = v.java_version.as_ref().map_or(8, |j| j.major_version);
        let preferred = inst.java_path.as_deref().or(settings.java_path.as_deref());
        let comp = v.java_version.as_ref().and_then(|j| j.component.clone());
        let java = crate::java::find_or_install(client, major, comp.as_deref(), preferred, &dirs.runtimes(), |p| on_progress("java", p)).await?;
        Ok((v, game_dir, libs, natives_dir, java))
    };
    let ((v, game_dir, libs, natives_dir, java), account) = tokio::try_join!(files, account)?;

    let sep = if cfg!(windows) { ";" } else { ":" };
    let jar = mojang::client_jar(dirs, &v)?;
    let mut cp: Vec<String> = libs.iter().filter(|l| l.on_classpath).map(|l| l.job.dest.display().to_string()).collect();
    cp.push(jar.display().to_string());

    let assets_id = v.asset_index.as_ref().map(|a| a.id.clone()).or(v.assets.clone()).unwrap_or_else(|| "legacy".into());
    let game_assets = if assets_id == "legacy" || assets_id == "pre-1.6" {
        dirs.assets().join("virtual").join(&assets_id)
    } else {
        dirs.assets()
    };
    let vars: Vec<(&str, String)> = vec![
        ("auth_player_name", account.name.clone()),
        ("version_name", v.id.clone()),
        ("game_directory", game_dir.display().to_string()),
        ("assets_root", dirs.assets().display().to_string()),
        ("game_assets", game_assets.display().to_string()),
        ("assets_index_name", assets_id),
        ("auth_uuid", account.uuid.clone()),
        ("auth_access_token", account.access_token.clone()),
        ("auth_xuid", "0".into()),
        ("clientid", "0".into()),
        ("user_type", account.user_type.clone()),
        ("user_properties", "{}".into()),
        ("version_type", v.kind.clone()),
        ("natives_directory", natives_dir.display().to_string()),
        ("launcher_name", crate::APP_NAME.into()),
        ("launcher_version", crate::APP_VERSION.into()),
        ("classpath", cp.join(sep)),
        ("classpath_separator", sep.into()),
        ("library_directory", dirs.libraries().display().to_string()),
    ];

    let f = Features::default();
    let mut jvm: Vec<String> = Vec::new();
    let mem = inst.memory_mb.unwrap_or(settings.default_memory_mb);
    jvm.push(format!("-Xmx{mem}M"));
    jvm.push(format!("-Xms{}M", (mem / 4).max(256)));
    for extra in [settings.extra_jvm_args.as_str(), inst.jvm_args.as_deref().unwrap_or("")] {
        jvm.extend(extra.split_whitespace().map(String::from));
    }
    let (jvm_def, game_def) = match &v.arguments {
        Some(a) => (expand_args(&a.jvm, &f), expand_args(&a.game, &f)),
        None => (
            vec!["-Djava.library.path=${natives_directory}".into(), "-cp".into(), "${classpath}".into()],
            v.minecraft_arguments.as_deref().unwrap_or("").split_whitespace().map(String::from).collect(),
        ),
    };
    jvm.extend(jvm_def);
    let mut args: Vec<String> = jvm.into_iter().map(|a| sub(&a, &vars)).collect();
    args.push(v.main_class.clone());
    args.extend(game_def.into_iter().map(|a| sub(&a, &vars)));
    Ok(Prepared { java, args, game_dir })
}

pub async fn spawn(p: &Prepared) -> Result<tokio::process::Child> {
    let mut cmd = tokio::process::Command::new(&p.java);
    cmd.args(&p.args)
        .current_dir(&p.game_dir)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(false);
    // The game must not inherit the launcher's desktop identity, or the taskbar groups its window under the launcher's icon.
    for var in ["DESKTOP_STARTUP_ID", "XDG_ACTIVATION_TOKEN", "APPIMAGE", "APPDIR", "ARGV0", "OWD", "GIO_LAUNCHED_DESKTOP_FILE", "GIO_LAUNCHED_DESKTOP_FILE_PID", "BAMF_DESKTOP_FILE_HINT"] {
        cmd.env_remove(var);
    }
    crate::paths::hide_console(&mut cmd);
    let child = cmd.spawn().context("failed to start java")?;
    Ok(child)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_window_gets_its_own_natives_folder() {
        let d = Dirs::new("/r");
        assert_ne!(natives_dir(&d, "a", 0), natives_dir(&d, "a", 1));
        assert_ne!(natives_dir(&d, "a", 1), natives_dir(&d, "a", 2));
        assert!(natives_dir(&d, "a", 0).ends_with("a/natives"));
    }

    #[test]
    fn substitution() {
        let v = vec![("a", "1".to_string())];
        assert_eq!(sub("x${a}y${b}", &v), "x1y${b}");
    }

    #[tokio::test]
    #[ignore = "downloads Minecraft, assets and Java (~600 MB)"]
    async fn live_vanilla_reaches_game_code() {
        let root = std::env::temp_dir().join(format!("geff-launch-{}", std::process::id()));
        let dirs = Dirs::new(&root);
        dirs.ensure().await.unwrap();
        let client = crate::http_client();
        let settings = Settings::default();
        let inst = crate::instance::create(&dirs, "t", "1.21.1", Loader::Vanilla, None, None).await.unwrap();
        let p = prepare(&dirs, &client, &settings, &inst, &Account::for_tests("Tester"), |_, _| {}).await.unwrap();
        let child = spawn(&p).await.unwrap();
        let out = tokio::time::timeout(std::time::Duration::from_secs(120), child.wait_with_output()).await.unwrap().unwrap();
        let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
        println!("{}", text.lines().take(25).collect::<Vec<_>>().join("\n"));
        assert!(!text.contains("ClassNotFoundException") && !text.contains("Could not find or load main class"), "{text}");
        assert!(text.contains("Setting user: Tester") || text.contains("Minecraft") || text.contains("LWJGL"), "{text}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    #[ignore = "downloads Minecraft 1.12.2, Forge and Java 8 (~300 MB)"]
    async fn live_forge_1_12_2_prepares() {
        let root = std::env::temp_dir().join("geff-launch-forge112-cache");
        let dirs = Dirs::new(&root);
        dirs.ensure().await.unwrap();
        let client = crate::http_client();
        let inst = crate::instance::create(&dirs, "t", "1.12.2", Loader::Forge, None, None).await.unwrap();
        let p = prepare(&dirs, &client, &Settings::default(), &inst, &Account::for_tests("Tester"), |_, _| {}).await.unwrap();
        assert!(p.args.iter().any(|a| a == "1.12"), "asset index name must be passed: {:?}", p.args);
    }

    #[tokio::test]
    #[ignore = "downloads Minecraft, NeoForge, assets and Java (~700 MB)"]
    async fn live_neoforge_args_are_consistent() {
        let root = std::env::temp_dir().join("geff-launch-neo-cache");
        let dirs = Dirs::new(&root);
        dirs.ensure().await.unwrap();
        let client = crate::http_client();
        let inst = crate::instance::create(&dirs, "t", "1.21.1", Loader::Neoforge, None, None).await.unwrap();
        let p = prepare(&dirs, &client, &Settings::default(), &inst, &Account::for_tests("Tester"), |_, _| {}).await.unwrap();
        let lists: Vec<Vec<&str>> = p.args.iter().map(|a| a.split(':').filter(|x| x.starts_with('/') && x.ends_with(".jar")).collect()).collect();
        assert!(lists.iter().any(|l| l.len() > 50), "expected a real classpath");
        for l in &lists {
            let unique: std::collections::HashSet<_> = l.iter().collect();
            assert_eq!(unique.len(), l.len(), "duplicate jar within one classpath-style argument");
            let missing: Vec<_> = l.iter().filter(|j| !std::path::Path::new(j).is_file()).collect();
            assert!(missing.is_empty(), "missing on disk: {missing:?}");
        }
        assert!(p.args.iter().any(|a| a.contains("bootstraplauncher")), "NeoForge main class missing");
        let _ = std::fs::remove_dir_all(&root);
    }
}

#[cfg(test)]
mod join_tests {
    use super::*;

    #[test]
    fn quick_play_starts_with_1_20() {
        for v in ["1.20", "1.20.4", "1.21.1", "26.3"] {
            assert!(supports_quick_play(v), "{v}");
        }
        for v in ["1.19.4", "1.12.2", "1.8.9", "abc"] {
            assert!(!supports_quick_play(v), "{v}");
        }
    }

    #[test]
    fn joins_become_the_right_arguments() {
        let world = Join::World("Neue Welt".into());
        assert_eq!(join_args("1.21.1", &world).unwrap(), ["--quickPlaySingleplayer", "Neue Welt"]);
        assert!(join_args("1.19.4", &world).is_none(), "worlds cannot be joined before 1.20");
        let server = Join::Server("mc.example.net:25570".into());
        assert_eq!(join_args("1.21.1", &server).unwrap(), ["--quickPlayMultiplayer", "mc.example.net:25570"]);
        assert_eq!(join_args("1.16.5", &server).unwrap(), ["--server", "mc.example.net", "--port", "25570"]);
        assert_eq!(join_args("1.16.5", &Join::Server("mc.example.net".into())).unwrap(), ["--server", "mc.example.net"]);
    }

    #[test]
    fn unsafe_targets_are_refused() {
        for bad in ["", "a b", "x;rm", "--demo", "-x", "host\nport", &"a".repeat(300)] {
            assert!(!server_addr_ok(bad), "{bad}");
        }
        assert!(join_args("1.21.1", &Join::World("../evil".into())).is_none());
        assert!(join_args("1.21.1", &Join::World("--demo".into())).is_none());
        assert!(join_args("1.21.1", &Join::Server("a b".into())).is_none());
    }
}
