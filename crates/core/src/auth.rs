//! Microsoft -> Xbox Live -> XSTS -> Minecraft services login (authorization-code flow).
//! The client id is a setting so it can be swapped for the project's own id later.

use crate::launch::Account;
use crate::paths::Dirs;
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::time::{Duration, Instant};

pub const REDIRECT: &str = "https://login.live.com/oauth20_desktop.srf";
const SCOPE: &str = "XboxLive.signin offline_access";

/// Stored per account. The refresh token never leaves the data directory.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Stored {
    pub uuid: String,
    pub name: String,
    pub refresh_token: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct Public {
    pub uuid: String,
    pub name: String,
}

pub fn random_state() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

pub fn authorize_url(client_id: &str, state: &str) -> String {
    let mut u = reqwest::Url::parse("https://login.live.com/oauth20_authorize.srf").unwrap();
    u.query_pairs_mut()
        .append_pair("client_id", client_id)
        .append_pair("response_type", "code")
        .append_pair("redirect_uri", REDIRECT)
        .append_pair("scope", SCOPE)
        .append_pair("prompt", "select_account")
        .append_pair("state", state);
    u.to_string()
}

/// Pull `code` out of the redirect, checking `state`. Returns Err for denied/foreign redirects.
pub fn code_from_redirect(url: &str, state: &str) -> Result<String> {
    let u = reqwest::Url::parse(url)?;
    let get = |k: &str| u.query_pairs().find(|(a, _)| a == k).map(|(_, v)| v.to_string());
    if get("state").as_deref() != Some(state) {
        bail!("login state mismatch");
    }
    if let Some(e) = get("error") {
        bail!("login denied: {e}");
    }
    get("code").context("no code in redirect")
}

#[derive(Deserialize)]
struct Tokens {
    access_token: String,
    refresh_token: Option<String>,
}

async fn token_request(c: &reqwest::Client, form: &[(&str, &str)]) -> Result<Tokens> {
    let r = c.post("https://login.live.com/oauth20_token.srf").form(form).send().await?;
    if !r.status().is_success() {
        bail!("Microsoft token request failed ({})", r.status());
    }
    Ok(r.json().await?)
}

#[derive(Deserialize)]
struct XblResp {
    #[serde(rename = "Token")]
    token: String,
    #[serde(rename = "DisplayClaims")]
    claims: Claims,
}
#[derive(Deserialize)]
struct Claims {
    xui: Vec<std::collections::HashMap<String, String>>,
}

fn xerr_text(code: u64) -> &'static str {
    match code {
        2148916233 => "This Microsoft account has no Xbox account. Create one at xbox.com.",
        2148916235 => "Xbox Live is not available in your country.",
        2148916238 => "Child account: it must be added to a family by an adult.",
        _ => "Xbox login failed.",
    }
}

struct McLogin {
    token: String,
    uuid: String,
    name: String,
    valid_for: Duration,
}

async fn minecraft_login(c: &reqwest::Client, ms_access: &str) -> Result<McLogin> {
    let xbl: XblResp = c
        .post("https://user.auth.xboxlive.com/user/authenticate")
        .json(&json!({"Properties":{"AuthMethod":"RPS","SiteName":"user.auth.xboxlive.com","RpsTicket":format!("d={ms_access}")},"RelyingParty":"http://auth.xboxlive.com","TokenType":"JWT"}))
        .send().await?.error_for_status()?.json().await?;
    let uhs = xbl.claims.xui.first().and_then(|m| m.get("uhs")).context("no uhs")?.clone();

    let r = c
        .post("https://xsts.auth.xboxlive.com/xsts/authorize")
        .json(&json!({"Properties":{"SandboxId":"RETAIL","UserTokens":[xbl.token]},"RelyingParty":"rp://api.minecraftservices.com/","TokenType":"JWT"}))
        .send().await?;
    if r.status() == 401 {
        let v: serde_json::Value = r.json().await.unwrap_or_default();
        bail!("{}", xerr_text(v["XErr"].as_u64().unwrap_or(0)));
    }
    let xsts: XblResp = r.error_for_status()?.json().await?;

    #[derive(Deserialize)]
    struct Mc { access_token: String, #[serde(default)] expires_in: u64 }
    let mc: Mc = c
        .post("https://api.minecraftservices.com/authentication/login_with_xbox")
        .json(&json!({"identityToken": format!("XBL3.0 x={uhs};{}", xsts.token)}))
        .send().await?.error_for_status()?.json().await?;

    #[derive(Deserialize)]
    struct Profile { id: String, name: String }
    let p = c.get("https://api.minecraftservices.com/minecraft/profile").bearer_auth(&mc.access_token).send().await?;
    if p.status() == 404 {
        bail!("This account does not own Minecraft Java Edition.");
    }
    let p: Profile = p.error_for_status()?.json().await?;
    Ok(McLogin { token: mc.access_token, uuid: p.id, name: p.name, valid_for: Duration::from_secs(mc.expires_in) })
}

/// A finished sign-in: the game account, what is stored on disk, and how long the token lasts.
pub struct Login {
    pub account: Account,
    pub stored: Stored,
    pub valid_for: Duration,
}

fn finish(stored_refresh: Option<String>, t: Tokens, mc: McLogin) -> Result<Login> {
    let refresh = t.refresh_token.or(stored_refresh).context("no refresh token")?;
    Ok(Login {
        account: Account { name: mc.name.clone(), uuid: mc.uuid.clone(), access_token: mc.token, user_type: "msa".into() },
        stored: Stored { uuid: mc.uuid, name: mc.name, refresh_token: refresh },
        valid_for: mc.valid_for,
    })
}

pub async fn login_with_code(c: &reqwest::Client, client_id: &str, code: &str) -> Result<Login> {
    let t = token_request(c, &[("client_id", client_id), ("code", code), ("grant_type", "authorization_code"), ("redirect_uri", REDIRECT), ("scope", SCOPE)]).await?;
    let mc = minecraft_login(c, &t.access_token).await?;
    finish(None, t, mc)
}

pub async fn refresh(c: &reqwest::Client, client_id: &str, s: &Stored) -> Result<Login> {
    let t = token_request(c, &[("client_id", client_id), ("refresh_token", &s.refresh_token), ("grant_type", "refresh_token"), ("redirect_uri", REDIRECT), ("scope", SCOPE)]).await?;
    let mc = minecraft_login(c, &t.access_token).await?;
    finish(Some(s.refresh_token.clone()), t, mc)
}

/// A token is dropped this long before Mojang's expiry, so a game never starts with one about to lapse.
const SESSION_MARGIN: Duration = Duration::from_secs(10 * 60);
const SESSION_MAX: Duration = Duration::from_secs(24 * 60 * 60);

/// Minecraft tokens last about a day. Launches inside that time reuse the token instead of
/// repeating the five requests to Microsoft, Xbox and Mojang. Kept in memory only, never on disk.
#[derive(Default)]
pub struct Sessions(tokio::sync::Mutex<HashMap<String, (Account, Instant)>>);

impl Sessions {
    pub async fn account(&self, c: &reqwest::Client, client_id: &str, dirs: &Dirs, stored: &Stored) -> Result<Account> {
        // Held across the refresh: two launches at once must not both spend the same refresh token.
        let mut map = self.0.lock().await;
        if let Some((acc, until)) = map.get(&stored.uuid) {
            if Instant::now() < *until {
                return Ok(acc.clone());
            }
        }
        let login = refresh(c, client_id, stored).await?;
        upsert(dirs, login.stored.clone()).await?;
        map.insert(stored.uuid.clone(), (login.account.clone(), session_end(Instant::now(), login.valid_for)));
        Ok(login.account)
    }

    pub async fn remember(&self, login: &Login) {
        self.0.lock().await.insert(login.stored.uuid.clone(), (login.account.clone(), session_end(Instant::now(), login.valid_for)));
    }

    pub async fn forget(&self, uuid: &str) {
        self.0.lock().await.remove(uuid);
    }
}

fn session_end(now: Instant, valid_for: Duration) -> Instant {
    now + valid_for.min(SESSION_MAX).saturating_sub(SESSION_MARGIN)
}

fn file(dirs: &Dirs) -> std::path::PathBuf { dirs.root.join("accounts.json") }

pub async fn load(dirs: &Dirs) -> Vec<Stored> {
    match tokio::fs::read(file(dirs)).await {
        Ok(b) => serde_json::from_slice(&b).unwrap_or_default(),
        Err(_) => vec![],
    }
}

pub async fn save(dirs: &Dirs, list: &[Stored]) -> Result<()> {
    let tmp = dirs.root.join("accounts.json.tmp");
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let mut o = std::fs::OpenOptions::new();
        o.write(true).create(true).truncate(true).mode(0o600);
        let data = serde_json::to_vec(list)?;
        let tmp2 = tmp.clone();
        tokio::task::spawn_blocking(move || -> Result<()> {
            use std::io::Write;
            o.open(&tmp2)?.write_all(&data)?;
            Ok(())
        })
        .await??;
    }
    #[cfg(not(unix))]
    tokio::fs::write(&tmp, serde_json::to_vec(list)?).await?;
    tokio::fs::rename(tmp, file(dirs)).await?;
    Ok(())
}

pub async fn upsert(dirs: &Dirs, s: Stored) -> Result<()> {
    let mut l = load(dirs).await;
    l.retain(|x| x.uuid != s.uuid);
    l.push(s);
    save(dirs, &l).await
}

pub async fn remove(dirs: &Dirs, uuid: &str) -> Result<()> {
    let mut l = load(dirs).await;
    l.retain(|x| x.uuid != uuid);
    save(dirs, &l).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn account(uuid: &str) -> Account {
        Account { name: "A".into(), uuid: uuid.into(), access_token: "t".into(), user_type: "msa".into() }
    }

    #[tokio::test]
    async fn a_remembered_session_is_reused_without_any_request() {
        let sessions = Sessions::default();
        let stored = Stored { uuid: "u1".into(), name: "A".into(), refresh_token: "r".into() };
        sessions.remember(&Login { account: account("u1"), stored: stored.clone(), valid_for: Duration::from_secs(86400) }).await;
        // A client without TLS roots cannot reach Microsoft, so success means nothing was sent.
        let offline = reqwest::Client::builder().tls_built_in_root_certs(false).build().unwrap();
        let dirs = Dirs::new(std::env::temp_dir().join(format!("gn-sessions-{}", uuid::Uuid::new_v4())));
        let got = sessions.account(&offline, "id", &dirs, &stored).await.unwrap();
        assert_eq!(got.access_token, "t");
        sessions.forget("u1").await;
        assert!(sessions.0.lock().await.is_empty());
    }

    #[test]
    fn sessions_end_before_the_token_and_never_last_longer_than_a_day() {
        let now = Instant::now();
        assert_eq!(session_end(now, Duration::from_secs(86400)), now + Duration::from_secs(86400 - 600));
        assert_eq!(session_end(now, Duration::from_secs(10 * 86400)), now + Duration::from_secs(86400 - 600));
        assert!(session_end(now, Duration::ZERO) <= now, "a token without expiry is never reused");
    }

    #[test]
    fn redirect_parsing() {
        let ok = format!("{REDIRECT}?code=abc&state=xyz");
        assert_eq!(code_from_redirect(&ok, "xyz").unwrap(), "abc");
        assert!(code_from_redirect(&ok, "other").is_err());
        assert!(code_from_redirect(&format!("{REDIRECT}?error=access_denied&state=xyz"), "xyz").is_err());
        assert!(code_from_redirect(&format!("{REDIRECT}?state=xyz"), "xyz").is_err());
    }

    #[test]
    fn url_has_state_and_redirect() {
        let u = authorize_url("cid", "st");
        assert!(u.contains("client_id=cid") && u.contains("state=st") && u.contains("oauth20_desktop.srf"));
    }
}
