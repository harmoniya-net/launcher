use anyhow::{Context, Result, anyhow};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};
use tiny_http::{Header, Response, Server};

pub struct CallbackListener {
    server: Server,
    port: u16,
}

pub struct Captured {
    pub code: Option<String>,
    pub state: String,
}

const SUCCESS_HTML: &str = include_str!("../../assets/callback_success.html");

pub fn start() -> Result<CallbackListener> {
    let server = Server::http("127.0.0.1:0").map_err(|e| anyhow!("bind loopback: {e}"))?;
    let port = server.server_addr().to_ip().context("no addr")?.port();
    Ok(CallbackListener { server, port })
}

impl CallbackListener {
    pub fn port(&self) -> u16 { self.port }

    /// Block (with 5-minute timeout) until the browser hits `/callback?code=...&state=...`.
    /// Responds with the success HTML page, then shuts the server down.
    pub fn wait_for_callback(self) -> Result<Captured> {
        let deadline = Instant::now() + Duration::from_secs(5 * 60);
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(anyhow!("oauth callback timed out"));
            }
            let req = match self.server.recv_timeout(remaining) {
                Ok(Some(r)) => r,
                Ok(None) => continue,
                Err(e) => return Err(anyhow!("recv: {e}")),
            };

            let url = req.url().to_string();
            // Accept either /callback (clean) or /api/auth/callback (Nuxt-registered path).
            if !(url.starts_with("/callback") || url.starts_with("/api/auth/callback")) {
                let _ = req.respond(Response::from_string("not found").with_status_code(404));
                continue;
            }

            let query = url.split_once('?').map(|(_, q)| q).unwrap_or("");
            let params: HashMap<String, String> = url::form_urlencoded::parse(query.as_bytes())
                .into_owned()
                .collect();

            let code = params.get("code").cloned();
            let state = params.get("state").cloned().unwrap_or_default();

            let resp = Response::from_string(SUCCESS_HTML)
                .with_header(
                    Header::from_bytes(b"Content-Type".as_slice(), b"text/html; charset=utf-8".as_slice())
                        .unwrap(),
                );
            let _ = req.respond(resp);

            return Ok(Captured { code, state });
        }
    }
}
