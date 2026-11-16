use std::future::Future;
use std::time::{Duration, SystemTime};

use workos::user_management::AuthenticateWithRefreshTokenParams;
use workos::{AuthenticateResponse, Client};

use crate::config::Config;
use crate::error::AuthError;
use crate::session::{Account, Session};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);

pub(crate) fn client(config: &Config) -> Client {
    Client::builder()
        .client_id(config.client_id.clone())
        .base_url(config.base_url.clone())
        .timeout(REQUEST_TIMEOUT)
        .build()
}

pub(crate) fn block_on<T>(future: impl Future<Output = T>) -> Result<T, AuthError> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| AuthError::Failed(error.to_string()))?;
    Ok(runtime.block_on(future))
}

pub(crate) fn session_from(response: &AuthenticateResponse) -> Session {
    Session::verified(
        Account::from_user(&response.user),
        response.refresh_token.expose().to_owned(),
        SystemTime::now(),
    )
    .with_access_token(response.access_token.expose().to_owned())
}

pub fn refresh(config: &Config, session: &Session) -> Result<Session, AuthError> {
    let client = client(config);
    let params = AuthenticateWithRefreshTokenParams::new(session.refresh_token().to_owned());
    let response = block_on(
        client
            .user_management()
            .authenticate_with_refresh_token(params),
    )?
    .map_err(AuthError::from_workos)?;
    Ok(session_from(&response))
}
