//! Typed remote errors mirroring the Go Miniflux client taxonomy.

use std::fmt;

/// Structured so Phase 8 sync orchestration can distinguish auth failures
/// (abort) from transient server/transport failures (leave snapshot intact).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoteError {
    /// HTTP 401.
    NotAuthorized,
    /// HTTP 403.
    Forbidden,
    /// HTTP 404.
    NotFound,
    /// HTTP 400 with the server-provided message when present.
    BadRequest(Option<String>),
    /// HTTP 500, optionally carrying the server error message.
    ServerError(Option<String>),
    /// Any other non-2xx status.
    Status(u16),
    /// Connection/DNS/TLS/timeout-level failure.
    Transport(String),
    /// Response body could not be parsed as expected JSON.
    Json(String),
}

impl fmt::Display for RemoteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RemoteError::NotAuthorized => {
                write!(formatter, "miniflux: unauthorized (bad credentials)")
            }
            RemoteError::Forbidden => write!(formatter, "miniflux: access forbidden"),
            RemoteError::NotFound => write!(formatter, "miniflux: resource not found"),
            RemoteError::BadRequest(Some(message)) => {
                write!(formatter, "miniflux: bad request ({message})")
            }
            RemoteError::BadRequest(None) => write!(formatter, "miniflux: bad request"),
            RemoteError::ServerError(Some(message)) => {
                write!(formatter, "miniflux: internal server error: {message}")
            }
            RemoteError::ServerError(None) => write!(formatter, "miniflux: internal server error"),
            RemoteError::Status(code) => write!(formatter, "miniflux: status code={code}"),
            RemoteError::Transport(message) => write!(formatter, "{message}"),
            RemoteError::Json(message) => write!(formatter, "miniflux: response error ({message})"),
        }
    }
}

impl std::error::Error for RemoteError {}
