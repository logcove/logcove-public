use serde::Serialize;
use std::fmt;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Serialize)]
pub struct Error {
    pub code: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

impl Error {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            request_id: None,
        }
    }

    pub fn invalid_response() -> Self {
        Self::new(
            "INVALID_RESPONSE",
            "The API returned an unexpected response. Check the API address and server version.",
        )
    }

    pub fn unauthenticated() -> Self {
        Self::new(
            "UNAUTHENTICATED",
            "No valid CLI session. Run logcove login for this API environment.",
        )
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for Error {}
