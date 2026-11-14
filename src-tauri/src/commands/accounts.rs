use crate::*;

pub(crate) async fn resolve_account(app: &App, settings: &Settings) -> anyhow::Result<Account> {
    let list = core::auth::load(&app.dirs).await;
    let Some(stored) = settings.active_account.as_ref().and_then(|u| list.iter().find(|a| &a.uuid == u)) else {
        anyhow::bail!("login required");
    };
    app.sessions.account(&app.http, &settings.ms_client_id, &app.dirs, stored).await
}

#[derive(Serialize)]
pub(crate) struct AccountEntry {
    pub(crate) uuid: String,
    pub(crate) name: String,
    /// `data:` URL of the cached skin, so the menu can show each account's head.
    pub(crate) skin: Option<String>,
}

pub(crate) async fn account_entries(dirs: &Dirs) -> Vec<AccountEntry> {
    let mut out = Vec::new();
    for a in core::auth::load(dirs).await {
        let skin = if core::skin::cached(dirs, &a.uuid).await.is_some() { core::skin::data_url(dirs, &a.uuid).await.ok() } else { None };
        out.push(AccountEntry { uuid: a.uuid, name: a.name, skin });
    }
    out
}

#[tauri::command]
pub(crate) async fn list_accounts(app: State<'_, App>) -> R<Vec<AccountEntry>> {
    Ok(account_entries(&app.dirs).await)
}

/// Fetches missing or stale skins for every stored account (one request each, cached six hours).
#[tauri::command]
pub(crate) async fn refresh_account_skins(app: State<'_, App>) -> R<Vec<AccountEntry>> {
    for a in core::auth::load(&app.dirs).await {
        let _ = core::skin::ensure(&app.dirs, &app.http, &a.uuid).await;
    }
    Ok(account_entries(&app.dirs).await)
}

#[tauri::command]
pub(crate) async fn remove_account(app: State<'_, App>, uuid: String) -> R<()> {
    core::auth::remove(&app.dirs, &uuid).await.map_err(err)?;
    app.sessions.forget(&uuid).await;
    let mut s = app.settings.lock().await;
    if s.active_account.as_deref() == Some(uuid.as_str()) {
        s.active_account = None;
        s.save(&app.dirs).await.map_err(err)?;
    }
    Ok(())
}

#[tauri::command]
pub(crate) async fn ms_login(app: State<'_, App>, handle: AppHandle) -> R<core::auth::Public> {
    use tauri::{WebviewUrl, WebviewWindowBuilder};
    let client_id = app.settings.lock().await.ms_client_id.clone();
    let state = uuid_state();
    let url = core::auth::authorize_url(&client_id, &state);
    let (tx, rx) = tokio::sync::oneshot::channel::<anyhow::Result<String>>();
    let tx = std::sync::Arc::new(std::sync::Mutex::new(Some(tx)));
    let st = state.clone();
    let txc = tx.clone();
    // Label "msa" has no capability file, so this window can not call any app command.
    let win = WebviewWindowBuilder::new(&handle, "msa", WebviewUrl::External(url.parse().map_err(err)?))
        .title("Microsoft-Anmeldung")
        .inner_size(520.0, 700.0)
        .on_navigation(move |u| {
            if u.as_str().starts_with(core::auth::REDIRECT) {
                if let Some(t) = txc.lock().unwrap().take() {
                    let _ = t.send(core::auth::code_from_redirect(u.as_str(), &st));
                }
                return false;
            }
            u.scheme() == "https" && u.host_str().is_some_and(login_host_ok)
        })
        .build()
        .map_err(err)?;
    let closed = tx.clone();
    win.on_window_event(move |e| {
        if matches!(e, tauri::WindowEvent::Destroyed) {
            if let Some(t) = closed.lock().unwrap().take() {
                let _ = t.send(Err(anyhow::anyhow!("login cancelled")));
            }
        }
    });
    let code = rx.await.map_err(err)?;
    let _ = win.close();
    let code = code.map_err(err)?;
    let login = core::auth::login_with_code(&app.http, &client_id, &code).await.map_err(err)?;
    let public = core::auth::Public { uuid: login.stored.uuid.clone(), name: login.stored.name.clone() };
    core::auth::upsert(&app.dirs, login.stored.clone()).await.map_err(err)?;
    app.sessions.remember(&login).await;
    let mut s = app.settings.lock().await;
    s.active_account = Some(public.uuid.clone());
    s.save(&app.dirs).await.map_err(err)?;
    Ok(public)
}

const LOGIN_DOMAINS: &[&str] = &["live.com", "microsoft.com", "microsoftonline.com", "msauth.net", "msftauth.net", "xboxlive.com"];

/// The login window may only show Microsoft's own pages: the domain itself or a subdomain of it.
pub(crate) fn login_host_ok(host: &str) -> bool {
    LOGIN_DOMAINS.iter().any(|d| host == *d || host.strip_suffix(d).is_some_and(|rest| rest.ends_with('.')))
}

pub(crate) fn uuid_state() -> String {
    core::auth::random_state()
}

#[cfg(test)]
mod tests {
    use super::login_host_ok;

    #[test]
    fn the_login_window_only_accepts_microsoft_domains() {
        for ok in ["live.com", "login.live.com", "account.microsoft.com", "login.microsoftonline.com", "user.auth.xboxlive.com"] {
            assert!(login_host_ok(ok), "{ok}");
        }
        for bad in ["evillive.com", "notmicrosoft.com", "live.com.evil.example", "xboxlive.com.attacker.net", ""] {
            assert!(!login_host_ok(bad), "{bad}");
        }
    }
}
