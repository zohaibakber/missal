use std::io::{BufRead, BufReader, ErrorKind, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use workos::AuthKitAuthorizationUrlParams;
use workos::user_management::AuthenticateWithCodeParams;

use crate::config::Config;
use crate::error::AuthError;
use crate::session::Session;
use crate::workos_api::{block_on, client, session_from};

const CALLBACK_PATH: &str = "/callback";
pub const CALLBACK_PORT: u16 = 42155;
const WAIT_LIMIT: Duration = Duration::from_secs(10 * 60);
const POLL: Duration = Duration::from_millis(100);
const READ_LIMIT: Duration = Duration::from_secs(5);

pub struct PendingSignIn {
    config: Config,
    listener: TcpListener,
    redirect_uri: String,
    url: String,
    code_verifier: String,
    state: String,
    cancel: Arc<AtomicBool>,
}

enum Callback {
    Code(String),
    Denied(String),
    Ignored,
}

pub fn begin_sign_in(config: &Config) -> Result<PendingSignIn, AuthError> {
    let failed = |error: std::io::Error| AuthError::Failed(error.to_string());
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, CALLBACK_PORT)).map_err(|error| {
        if error.kind() == ErrorKind::AddrInUse {
            AuthError::Failed(format!(
                "port {CALLBACK_PORT} is in use by another program; close it and try again"
            ))
        } else {
            AuthError::Failed(error.to_string())
        }
    })?;
    listener.set_nonblocking(true).map_err(failed)?;
    let port = listener.local_addr().map_err(failed)?.port();
    let redirect_uri = format!("http://127.0.0.1:{port}{CALLBACK_PATH}");
    let authorization = client(config)
        .authkit()
        .pkce_authorization_url(AuthKitAuthorizationUrlParams {
            redirect_uri: redirect_uri.clone(),
            provider: Some("authkit".to_owned()),
            ..Default::default()
        })
        .map_err(AuthError::from_workos)?;
    Ok(PendingSignIn {
        config: config.clone(),
        listener,
        redirect_uri,
        url: authorization.url,
        code_verifier: authorization.code_verifier,
        state: authorization.state,
        cancel: Arc::new(AtomicBool::new(false)),
    })
}

impl PendingSignIn {
    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn redirect_uri(&self) -> &str {
        &self.redirect_uri
    }

    pub fn canceller(&self) -> Arc<AtomicBool> {
        self.cancel.clone()
    }

    pub fn finish(self) -> Result<Session, AuthError> {
        let code = self.wait_for_code()?;
        let mut exchange = AuthenticateWithCodeParams::new(code);
        exchange.code_verifier = Some(self.code_verifier.clone().into());
        let client = client(&self.config);
        let response = block_on(client.user_management().authenticate_with_code(exchange))?
            .map_err(AuthError::from_workos)?;
        Ok(session_from(&response))
    }

    fn wait_for_code(&self) -> Result<String, AuthError> {
        let deadline = Instant::now() + WAIT_LIMIT;
        loop {
            if self.cancel.load(Ordering::Relaxed) {
                return Err(AuthError::Cancelled);
            }
            if Instant::now() >= deadline {
                return Err(AuthError::TimedOut);
            }
            match self.listener.accept() {
                Ok((stream, _)) => match self.answer(stream) {
                    Callback::Code(code) => return Ok(code),
                    Callback::Denied(reason) => return Err(AuthError::Rejected(reason)),
                    Callback::Ignored => {}
                },
                Err(error) if error.kind() == ErrorKind::WouldBlock => std::thread::sleep(POLL),
                Err(error) => return Err(AuthError::Failed(error.to_string())),
            }
        }
    }

    fn answer(&self, mut stream: TcpStream) -> Callback {
        let callback = self.read_callback(&stream);
        let (status, body) = match &callback {
            Callback::Code(_) => ("200 OK", "Signed in to Missal. You can close this tab."),
            Callback::Denied(_) => (
                "200 OK",
                "Sign-in did not finish. Return to Missal to try again.",
            ),
            Callback::Ignored => ("404 Not Found", "Not found."),
        };
        let page = format!(
            "<!doctype html><meta charset=\"utf-8\"><title>Missal</title><body style=\"font-family:system-ui;padding:48px\"><p>{body}</p></body>"
        );
        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\n\r\n{page}",
            page.len()
        );
        stream.write_all(response.as_bytes()).ok();
        stream.flush().ok();
        callback
    }

    fn read_callback(&self, stream: &TcpStream) -> Callback {
        if stream.set_nonblocking(false).is_err()
            || stream.set_read_timeout(Some(READ_LIMIT)).is_err()
        {
            return Callback::Ignored;
        }
        let mut line = String::new();
        if BufReader::new(stream).read_line(&mut line).is_err() {
            return Callback::Ignored;
        }
        let mut parts = line.split_whitespace();
        let (Some("GET"), Some(target)) = (parts.next(), parts.next()) else {
            return Callback::Ignored;
        };
        let Ok(url) = url::Url::parse(&format!("http://127.0.0.1{target}")) else {
            return Callback::Ignored;
        };
        if url.path() != CALLBACK_PATH {
            return Callback::Ignored;
        }
        let query = |name: &str| {
            url.query_pairs()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.into_owned())
        };
        if query("state").as_deref() != Some(self.state.as_str()) {
            return Callback::Ignored;
        }
        if let Some(code) = query("code") {
            return Callback::Code(code);
        }
        let reason = query("error_description")
            .or_else(|| query("error"))
            .unwrap_or_else(|| "no authorization code was returned".to_owned());
        Callback::Denied(reason)
    }
}
