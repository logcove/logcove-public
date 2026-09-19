use crate::{
    client::Api,
    config::safe_transport,
    credentials::CredentialStore,
    error::{Error, Result},
};
use reqwest::{Method, Url};
use serde::Deserialize;
use serde_json::json;
use std::time::{Duration, Instant};

const CLIENT_ID: &str = "logcove-cli";

#[derive(Deserialize)]
pub struct Authorization {
    device_code: String,
    pub user_code: String,
    pub verification_uri_complete: String,
    pub expires_in: u64,
    pub interval: u64,
}

pub trait Clock {
    fn elapsed(&self) -> Duration;
    fn sleep(&mut self, duration: Duration);
}

pub struct SystemClock(Instant);

impl Default for SystemClock {
    fn default() -> Self {
        Self(Instant::now())
    }
}

impl Clock for SystemClock {
    fn elapsed(&self) -> Duration {
        self.0.elapsed()
    }
    fn sleep(&mut self, duration: Duration) {
        std::thread::sleep(duration);
    }
}

impl<S: CredentialStore> Api<S> {
    pub fn begin_login(&self) -> Result<Authorization> {
        self.require_session_mode()?;
        let authorization = self
            .request(
                Method::POST,
                "/api/auth/device/code",
                &[],
                Some(&json!({"client_id": CLIENT_ID})),
                false,
                None,
            )?
            .decode::<Authorization>()?;
        let url = Url::parse(&authorization.verification_uri_complete)
            .map_err(|_| Error::invalid_response())?;
        if !safe_transport(&url)
            || url.fragment().is_some()
            || url.path() != "/cli/login"
            || !url
                .query_pairs()
                .any(|(name, value)| name == "user_code" && value == authorization.user_code)
            || authorization.device_code.is_empty()
            || authorization.user_code.is_empty()
            || authorization.interval == 0
            || authorization.expires_in == 0
        {
            return Err(Error::invalid_response());
        }
        Ok(authorization)
    }

    pub fn complete_login(
        &mut self,
        authorization: &Authorization,
        clock: &mut impl Clock,
    ) -> Result<()> {
        self.require_session_mode()?;
        let expiry = Duration::from_secs(authorization.expires_in);
        let mut interval = Duration::from_secs(authorization.interval);
        loop {
            let remaining = expiry.saturating_sub(clock.elapsed());
            if remaining <= interval {
                clock.sleep(remaining);
                return Err(Error::new(
                    "AUTHORIZATION_EXPIRED",
                    "Browser authorization expired. Run logcove login again.",
                ));
            }
            clock.sleep(interval);
            let remaining = expiry.saturating_sub(clock.elapsed());
            if remaining.is_zero() {
                return Err(Error::new(
                    "AUTHORIZATION_EXPIRED",
                    "Browser authorization expired. Run logcove login again.",
                ));
            }
            let reply = self.request(
                Method::POST,
                "/api/auth/device/token",
                &[],
                Some(&json!({
                    "client_id": CLIENT_ID, "device_code": authorization.device_code,
                    "grant_type": "urn:ietf:params:oauth:grant-type:device_code"
                })),
                false,
                Some(remaining),
            )?;
            if reply.success() {
                #[derive(Deserialize)]
                struct Token {
                    access_token: String,
                    token_type: String,
                }
                let token = reply.decode::<Token>()?;
                if !token.token_type.eq_ignore_ascii_case("bearer")
                    || !token.access_token.contains('.')
                    || token.access_token.chars().any(char::is_whitespace)
                {
                    return Err(Error::invalid_response());
                }
                self.token = Some(token.access_token.clone());
                if self.store.save(&token.access_token).is_err() {
                    let revoked = self.logout().is_ok();
                    return Err(Error::new(
                        "CREDENTIAL_SAVE_FAILED",
                        if revoked {
                            "Could not save the CLI session. The new server session was revoked. Unlock your system credential store and log in again."
                        } else {
                            "Could not save the CLI session or confirm its revocation. No persistent login was completed. Unlock your system credential store before retrying."
                        },
                    ));
                }
                return Ok(());
            }
            if reply.status == 429 {
                interval =
                    interval.max(Duration::from_secs(reply.retry_after.unwrap_or(60).max(1)));
                continue;
            }
            match (
                reply.status,
                reply.body.get("error").and_then(|e| e.as_str()),
            ) {
                (400, Some("authorization_pending")) => {}
                (400, Some("slow_down")) => {
                    interval = interval.saturating_add(Duration::from_secs(5))
                }
                (400, Some("access_denied")) => {
                    return Err(Error::new(
                        "AUTHORIZATION_DENIED",
                        "Browser authorization was declined.",
                    ))
                }
                (400, Some("expired_token")) => {
                    return Err(Error::new(
                        "AUTHORIZATION_EXPIRED",
                        "Browser authorization expired. Run logcove login again.",
                    ))
                }
                (400, Some("invalid_grant")) => {
                    return Err(Error::new(
                        "AUTHORIZATION_INVALID",
                        "This authorization request is no longer valid. Run logcove login again.",
                    ))
                }
                _ => return Err(reply.error()),
            }
        }
    }
}
