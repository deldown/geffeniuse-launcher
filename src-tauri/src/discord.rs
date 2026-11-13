use geffeniuse_core::settings::{valid_discord_application_id, Settings};
use serde_json::{json, Value};
use std::{collections::BTreeSet, io, time::Duration};
use tokio::{io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt}, sync::watch};

const IO_TIMEOUT: Duration = Duration::from_secs(3);
const RETRY: Duration = Duration::from_secs(15);
const MAX_FRAME: usize = 64 * 1024;

#[derive(Clone, Default)]
struct Presence {
    enabled: bool,
    application_id: String,
    games: BTreeSet<String>,
}

pub(crate) struct Discord(watch::Sender<Presence>);

impl Discord {
    pub(crate) fn new(settings: &Settings) -> Self {
        let (tx, rx) = watch::channel(Presence::default());
        tokio::spawn(run(rx));
        let controller = Self(tx);
        controller.configure(settings);
        controller
    }

    pub(crate) fn configure(&self, settings: &Settings) {
        self.0.send_modify(|p| {
            p.enabled = settings.discord_presence && valid_discord_application_id(&settings.discord_application_id);
            p.application_id.clone_from(&settings.discord_application_id);
        });
    }

    pub(crate) fn game(&self, key: &str, running: bool) {
        self.0.send_modify(|p| {
            if running { p.games.insert(key.to_owned()); } else { p.games.remove(key); }
        });
    }
}

async fn run(mut rx: watch::Receiver<Presence>) {
    loop {
        let config = rx.borrow_and_update().clone();
        if !config.enabled {
            if rx.changed().await.is_err() { return; }
            continue;
        }
        // Settings can cancel connection/handshake immediately, including an opt-out.
        tokio::select! {
            changed = rx.changed() => { if changed.is_err() { return; } continue; }
            connection = tokio::time::timeout(IO_TIMEOUT, connect()) => {
                if let Ok(Ok(stream)) = connection {
                    let _ = session(stream, &mut rx, &config.application_id).await;
                }
            }
        }
        if !rx.borrow().enabled { continue; }
        tokio::select! {
            () = tokio::time::sleep(RETRY) => {},
            changed = rx.changed() => if changed.is_err() { return; },
        }
    }
}

#[cfg(unix)]
async fn connect() -> io::Result<tokio::net::UnixStream> {
    use std::path::PathBuf;
    let mut dirs: Vec<PathBuf> = ["XDG_RUNTIME_DIR", "TMPDIR", "TMP", "TEMP"]
        .into_iter().filter_map(std::env::var_os).map(PathBuf::from).filter(|p| p.is_absolute()).collect();
    dirs.push(PathBuf::from("/tmp"));
    for dir in dirs {
        for prefix in [dir.clone(), dir.join("app/com.discordapp.Discord")] {
            for n in 0..10 {
                if let Ok(stream) = tokio::net::UnixStream::connect(prefix.join(format!("discord-ipc-{n}"))).await {
                    return Ok(stream);
                }
            }
        }
    }
    Err(io::ErrorKind::NotFound.into())
}

#[cfg(windows)]
async fn connect() -> io::Result<tokio::net::windows::named_pipe::NamedPipeClient> {
    for n in 0..10 {
        if let Ok(pipe) = tokio::net::windows::named_pipe::ClientOptions::new()
            .open(format!(r"\\?\pipe\discord-ipc-{n}")) {
            return Ok(pipe);
        }
    }
    Err(io::ErrorKind::NotFound.into())
}

async fn write_frame<W: AsyncWrite + Unpin>(out: &mut W, opcode: u32, body: &[u8]) -> io::Result<()> {
    if body.len() > MAX_FRAME { return Err(io::ErrorKind::InvalidData.into()); }
    let mut bytes = Vec::with_capacity(8 + body.len());
    bytes.extend(opcode.to_le_bytes());
    bytes.extend((body.len() as u32).to_le_bytes());
    bytes.extend(body);
    tokio::time::timeout(IO_TIMEOUT, out.write_all(&bytes)).await??;
    Ok(())
}

async fn read_frame<R: AsyncRead + Unpin>(input: &mut R) -> io::Result<(u32, Vec<u8>)> {
    let opcode = input.read_u32_le().await?;
    let size = input.read_u32_le().await? as usize;
    if size > MAX_FRAME { return Err(io::ErrorKind::InvalidData.into()); }
    let mut body = vec![0; size];
    input.read_exact(&mut body).await?;
    Ok((opcode, body))
}

fn activity(playing: Option<bool>, nonce: u64) -> Value {
    json!({"cmd": "SET_ACTIVITY", "args": {
        "pid": std::process::id(),
        "activity": playing.map(|playing| json!({"type": 0,
            "details": if playing { "Spielt Minecraft" } else { "Im Launcher" }
        }))
    }, "nonce": nonce.to_string()})
}

async fn session<S: AsyncRead + AsyncWrite + Unpin>(stream: S, rx: &mut watch::Receiver<Presence>, id: &str) -> io::Result<()> {
    let (mut input, mut out) = tokio::io::split(stream);
    write_frame(&mut out, 0, &serde_json::to_vec(&json!({"v": 1, "client_id": id}))?).await?;
    let (opcode, body) = tokio::select! {
        result = tokio::time::timeout(IO_TIMEOUT, read_frame(&mut input)) => result??,
        _ = rx.changed() => return Ok(()),
    };
    let ready: Value = serde_json::from_slice(&body)?;
    if opcode != 1 || ready["evt"] != "READY" { return Err(io::ErrorKind::InvalidData.into()); }
    let mut nonce = 0;
    let mut last = None;
    loop {
        // Keep the pending read alive across state changes: read_exact is not cancellation-safe.
        let frame = read_frame(&mut input);
        tokio::pin!(frame);
        loop {
            let current = rx.borrow_and_update().clone();
            let active = current.enabled && current.application_id == id;
            let playing = active.then_some(!current.games.is_empty());
            if playing != last || nonce == 0 {
                nonce += 1;
                write_frame(&mut out, 1, &serde_json::to_vec(&activity(playing, nonce))?).await?;
                last = playing;
            }
            if !active { return Ok(()); }
            tokio::select! {
                changed = rx.changed() => if changed.is_err() {
                    write_frame(&mut out, 1, &serde_json::to_vec(&activity(None, nonce + 1))?).await?;
                    return Ok(());
                },
                result = &mut frame => {
                    let (opcode, body) = result?;
                    match opcode {
                        1 => {
                            let response: Value = serde_json::from_slice(&body)?;
                            if response["evt"] == "ERROR" { return Err(io::ErrorKind::InvalidData.into()); }
                        }
                        3 => write_frame(&mut out, 4, &body).await?,
                        4 => {},
                        _ => return Err(io::ErrorKind::ConnectionAborted.into()),
                    }
                    break;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_contains_only_generic_activity_and_protocol_fields() {
        for playing in [false, true] {
            let value = activity(Some(playing), 7);
            assert_eq!(value["args"]["activity"], json!({"type": 0, "details": if playing { "Spielt Minecraft" } else { "Im Launcher" }}));
            assert_eq!(value["nonce"], "7");
        }
        assert!(activity(None, 8)["args"]["activity"].is_null());
    }

    #[tokio::test]
    async fn framing_rejects_oversize_and_truncated_messages() {
        let mut oversized = [&1u32.to_le_bytes()[..], &((MAX_FRAME + 1) as u32).to_le_bytes()[..]].concat();
        assert!(read_frame(&mut oversized.as_slice()).await.is_err());
        oversized.truncate(3);
        assert!(read_frame(&mut oversized.as_slice()).await.is_err());
        assert!(write_frame(&mut tokio::io::sink(), 1, &vec![0; MAX_FRAME + 1]).await.is_err());
    }

    #[tokio::test]
    async fn session_updates_handles_ping_and_clears_on_opt_out() {
        let (client, mut server) = tokio::io::duplex(4096);
        let (tx, mut rx) = watch::channel(Presence { enabled: true, application_id: "123456789012345678".into(), ..Presence::default() });
        let task = tokio::spawn(async move { session(client, &mut rx, "123456789012345678").await });
        let (opcode, body) = read_frame(&mut server).await.unwrap();
        assert_eq!(opcode, 0);
        assert_eq!(serde_json::from_slice::<Value>(&body).unwrap()["client_id"], "123456789012345678");
        write_frame(&mut server, 1, br#"{"evt":"READY"}"#).await.unwrap();
        let (_, body) = read_frame(&mut server).await.unwrap();
        assert_eq!(serde_json::from_slice::<Value>(&body).unwrap()["args"]["activity"]["details"], "Im Launcher");
        tx.send_modify(|p| { p.games.insert("private-pack-name".into()); });
        let (_, body) = read_frame(&mut server).await.unwrap();
        assert_eq!(serde_json::from_slice::<Value>(&body).unwrap()["args"]["activity"]["details"], "Spielt Minecraft");
        write_frame(&mut server, 3, b"ping").await.unwrap();
        assert_eq!(read_frame(&mut server).await.unwrap(), (4, b"ping".to_vec()));
        tx.send_modify(|p| { p.games.clear(); });
        let (_, body) = read_frame(&mut server).await.unwrap();
        assert_eq!(serde_json::from_slice::<Value>(&body).unwrap()["args"]["activity"]["details"], "Im Launcher");
        tx.send_modify(|p| p.enabled = false);
        let (_, body) = read_frame(&mut server).await.unwrap();
        assert!(serde_json::from_slice::<Value>(&body).unwrap()["args"]["activity"].is_null());
        task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn failed_handshake_never_sends_activity() {
        let (client, mut server) = tokio::io::duplex(4096);
        let (_tx, mut rx) = watch::channel(Presence { enabled: true, application_id: "123456789012345678".into(), ..Presence::default() });
        let task = tokio::spawn(async move { session(client, &mut rx, "123456789012345678").await });
        read_frame(&mut server).await.unwrap();
        write_frame(&mut server, 1, br#"{"evt":"ERROR"}"#).await.unwrap();
        assert!(task.await.unwrap().is_err());
        assert!(read_frame(&mut server).await.is_err());
    }

    #[tokio::test]
    async fn changing_application_clears_the_old_activity_even_during_a_partial_read() {
        let (client, mut server) = tokio::io::duplex(4096);
        let (tx, mut rx) = watch::channel(Presence { enabled: true, application_id: "123456789012345678".into(), ..Presence::default() });
        let task = tokio::spawn(async move { session(client, &mut rx, "123456789012345678").await });
        read_frame(&mut server).await.unwrap();
        write_frame(&mut server, 1, br#"{"evt":"READY"}"#).await.unwrap();
        read_frame(&mut server).await.unwrap();
        server.write_all(&[1, 0]).await.unwrap();
        tx.send_modify(|p| p.application_id = "234567890123456789".into());
        let (_, body) = tokio::time::timeout(IO_TIMEOUT, read_frame(&mut server)).await.unwrap().unwrap();
        assert!(serde_json::from_slice::<Value>(&body).unwrap()["args"]["activity"].is_null());
        task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn controller_tracks_multiple_games_and_validates_configuration() {
        let controller = Discord::new(&Settings::default());
        controller.game("one", true);
        controller.game("two", true);
        controller.game("one", false);
        assert_eq!(controller.0.borrow().games.len(), 1);
        assert!(!controller.0.borrow().enabled);
        controller.configure(&Settings { discord_presence: true, discord_application_id: "invalid".into(), ..Settings::default() });
        assert!(!controller.0.borrow().enabled);
    }
}
