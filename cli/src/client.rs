use crate::{
    config,
    credentials::CredentialStore,
    error::{Error, Result},
    models::{Data, Page, Project, User},
};
use reqwest::{blocking::Client, redirect::Policy, Method, Url};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use std::{collections::HashSet, io::Read, time::Duration};

const RESPONSE_LIMIT: u64 = 8 * 1024 * 1024;

pub(crate) struct Reply {
    pub status: u16,
    pub body: Value,
    pub renewed: Option<String>,
    pub request_id: Option<String>,
    pub retry_after: Option<u64>,
}

impl Reply {
    pub fn success(&self) -> bool {
        (200..300).contains(&self.status)
    }

    pub fn error(&self) -> Error {
        let (code, message) = match self.status {
            401 => (
                "UNAUTHENTICATED",
                "Your CLI session is no longer valid. Run logcove login.",
            ),
            403 => (
                "ACCESS_DENIED",
                "This account cannot access the requested resource.",
            ),
            404 => ("NOT_FOUND", "The resource or API endpoint was not found."),
            409 => match self.body.pointer("/error/code").and_then(Value::as_str) {
                Some("PROJECT_LIMIT_REACHED") => (
                    "PROJECT_LIMIT_REACHED",
                    "Your plan's active Project limit has been reached. Archive a Project or upgrade.",
                ),
                Some("CHART_CONFLICT") => (
                    "CHART_CONFLICT",
                    "The Chart definition changed. Read its latest revision and reconcile before retrying.",
                ),
                Some("WRITE_KEY_CONFLICT") => (
                    "WRITE_KEY_CONFLICT",
                    "A Project already has another write Key. Inspect its binding before choosing an explicit replacement.",
                ),
                Some("KEY_REVOKED") => (
                    "KEY_REVOKED",
                    "The Key has been revoked and cannot be modified. Select an active Key.",
                ),
                Some("INVALID_KEY_BINDING") => (
                    "INVALID_KEY_BINDING",
                    "The Key or Project changed. Read their current state before retrying the binding.",
                ),
                _ => (
                    "CONFLICT",
                    "The resource changed. Read its current state before retrying.",
                ),
            },
            429 => (
                "RATE_LIMITED",
                "The API rate limit was reached. Wait before retrying.",
            ),
            400 => (
                "INVALID_INPUT",
                "The API rejected the input. Check field values and file formats.",
            ),
            413 => (
                "PAYLOAD_TOO_LARGE",
                "The request exceeds the API size limit.",
            ),
            300..=399 => (
                "REDIRECT_REFUSED",
                "The API returned a redirect. Configure its final API origin instead.",
            ),
            500..=599 => (
                "SERVER_ERROR",
                "The API could not complete the request. Try again later.",
            ),
            _ => (
                "API_ERROR",
                "The API rejected the request. Check the command and server version.",
            ),
        };
        Error {
            code,
            message: format!("{message} (HTTP {})", self.status),
            request_id: self.request_id.clone(),
        }
    }

    pub fn decode<T: DeserializeOwned>(self) -> Result<T> {
        if !self.success() {
            return Err(self.error());
        }
        serde_json::from_value(self.body).map_err(|_| Error::invalid_response())
    }
}

pub struct Api<S: CredentialStore> {
    pub(crate) origin: Url,
    pub(crate) store: S,
    pub(crate) token: Option<String>,
    environment_token: bool,
    http: Client,
}

impl<S: CredentialStore> Api<S> {
    pub fn new(origin: Url, store: S) -> Result<Self> {
        Self::with_environment_token(origin, store, None)
    }

    pub fn with_environment_token(origin: Url, store: S, token: Option<String>) -> Result<Self> {
        if let Some(value) = &token {
            if !value.strip_prefix("lc_pat_").is_some_and(|secret| {
                secret.len() == 64
                    && secret
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            }) {
                return Err(Error::new("INVALID_TOKEN", "LOGCOVE_TOKEN must be a personal access token (lc_pat_), not a write key or Session. No local login fallback was used."));
            }
        }
        let environment_token = token.is_some();
        let origin = config::origin(origin.as_str())?;
        let mut builder = Client::builder()
            .redirect(Policy::none())
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .user_agent(concat!("logcove/", env!("CARGO_PKG_VERSION")));
        if config::is_loopback(&origin) {
            builder = builder.no_proxy();
        }
        let http = builder.build().map_err(|_| {
            Error::new("HTTP_CLIENT_ERROR", "Could not initialize the HTTP client.")
        })?;
        let token = if environment_token {
            token
        } else {
            store.read()?
        };
        Ok(Self {
            origin,
            store,
            token,
            environment_token,
            http,
        })
    }

    pub fn has_session(&self) -> bool {
        self.token.is_some()
    }

    pub(crate) fn require_session_mode(&self) -> Result<()> {
        if self.environment_token {
            return Err(Error::new("ENV_TOKEN_ACTIVE", "LOGCOVE_TOKEN is active. Unset it before browser login or session logout. To revoke this token, use Personal tokens in the Logcove app."));
        }
        Ok(())
    }

    pub(crate) fn request(
        &self,
        method: Method,
        path: &str,
        query: &[(String, String)],
        body: Option<&Value>,
        authenticated: bool,
        timeout: Option<Duration>,
    ) -> Result<Reply> {
        let url = self
            .origin
            .join(path)
            .map_err(|_| Error::new("INVALID_REQUEST", "Invalid API path."))?;
        if url.origin() != self.origin.origin() {
            return Err(Error::new(
                "INVALID_REQUEST",
                "Requests must stay on the configured API origin.",
            ));
        }
        let mut request = self.http.request(method, url).query(query);
        if authenticated {
            request =
                request.bearer_auth(self.token.as_deref().ok_or_else(Error::unauthenticated)?);
        }
        if let Some(body) = body {
            request = request.json(body);
        }
        if let Some(timeout) = timeout {
            request = request.timeout(timeout.min(Duration::from_secs(30)));
        }
        // Never display reqwest errors: they may contain a credential-bearing URL.
        let response = request.send().map_err(|_| {
            Error::new(
                "NETWORK_ERROR",
                "Could not complete the API request. Check your connection and API address.",
            )
        })?;
        let status = response.status().as_u16();
        let header = |name| {
            response
                .headers()
                .get(name)
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned)
        };
        let renewed = header("set-auth-token");
        let request_id = header("x-request-id");
        let retry_after = header("retry-after").and_then(|v| v.parse().ok());
        let mut raw = Vec::new();
        response
            .take(RESPONSE_LIMIT + 1)
            .read_to_end(&mut raw)
            .map_err(|_| Error::new("NETWORK_ERROR", "The API response was interrupted."))?;
        if raw.len() as u64 > RESPONSE_LIMIT {
            return Err(Error::invalid_response());
        }
        let body = if raw.is_empty() {
            Value::Null
        } else {
            match serde_json::from_slice(&raw) {
                Ok(value) => value,
                Err(_) if !(200..300).contains(&status) => Value::Null,
                Err(_) => return Err(Error::invalid_response()),
            }
        };
        Ok(Reply {
            status,
            body,
            renewed,
            request_id,
            retry_after,
        })
    }

    pub(crate) fn authenticated(
        &mut self,
        method: Method,
        path: &str,
        query: &[(String, String)],
        body: Option<&Value>,
    ) -> Result<Reply> {
        let reply = self.request(method, path, query, body, true, None)?;
        if self.environment_token {
            if reply.status == 401 {
                let mut error = Error::new("UNAUTHENTICATED", "LOGCOVE_TOKEN was rejected. Check or replace it in your secret store. No local login fallback was used.");
                error.request_id = reply.request_id;
                return Err(error);
            }
            return Ok(reply);
        }
        if reply.status == 401 {
            if let Some(token) = self.token.take() {
                if let Err(error) = self.store.delete_if_matches(&token) {
                    eprintln!("Warning: {error}");
                }
            }
        } else if reply.success() {
            if let Some(token) = &reply.renewed {
                if self.token.as_ref() != Some(token) {
                    self.token = Some(token.clone());
                    // A persistence warning must not turn a successful operation into a retry.
                    if let Err(error) = self.store.save(token) {
                        eprintln!("Warning: {error}");
                    }
                }
            }
        }
        Ok(reply)
    }

    pub fn whoami(&mut self) -> Result<User> {
        let reply = self.authenticated(Method::GET, "/api/v1/me", &[], None)?;
        Ok(reply.decode::<Data<User>>()?.data)
    }

    pub fn projects(&mut self) -> Result<Vec<Project>> {
        let mut projects = Vec::new();
        let mut cursor = None;
        let mut seen = HashSet::new();
        loop {
            let mut query = vec![("limit".into(), "100".into())];
            if let Some(cursor) = cursor {
                query.push(("cursor".into(), cursor));
            }
            let page = self
                .authenticated(Method::GET, "/data/v1/projects", &query, None)?
                .decode::<Page<Project>>()?;
            projects.extend(page.data);
            cursor = page.pagination.next_cursor;
            match &cursor {
                Some(value) if !seen.insert(value.clone()) => {
                    return Err(Error::new(
                        "INVALID_PAGINATION",
                        "The API repeated a cursor; listing could not complete.",
                    ))
                }
                Some(_) => {}
                None => return Ok(projects),
            }
        }
    }

    pub fn project(&mut self, id: &str) -> Result<Project> {
        if !valid_project_id(id) {
            return Err(Error::new(
                "INVALID_PROJECT_ID",
                "Expected prj_ followed by a UUID.",
            ));
        }
        let reply =
            self.authenticated(Method::GET, &format!("/api/v1/projects/{id}"), &[], None)?;
        Ok(reply.decode::<Data<Project>>()?.data)
    }

    pub fn logout(&mut self) -> Result<()> {
        self.require_session_mode()?;
        if self.token.is_none() {
            return Ok(());
        }
        let reply = self.request(
            Method::POST,
            "/api/auth/sign-out",
            &[],
            Some(&json!({})),
            true,
            None,
        )?;
        if reply.status != 401 {
            #[derive(serde::Deserialize)]
            struct SignedOut {
                success: bool,
            }
            if !reply.decode::<SignedOut>()?.success {
                return Err(Error::invalid_response());
            }
        }
        let token = self.token.take().unwrap();
        self.store.delete_if_matches(&token).map_err(|_| Error::new("CREDENTIAL_DELETE_FAILED", "The server session is no longer active, but local credential removal failed. Unlock the store and run logout again."))
    }
}

pub(crate) fn valid_project_id(id: &str) -> bool {
    valid_id(id, "prj_")
}

pub(crate) fn valid_id(id: &str, prefix: &str) -> bool {
    let Some(uuid) = id.strip_prefix(prefix) else {
        return false;
    };
    uuid.len() == 36
        && uuid.bytes().enumerate().all(|(i, c)| {
            if [8, 13, 18, 23].contains(&i) {
                c == b'-'
            } else {
                c.is_ascii_hexdigit()
            }
        })
}
