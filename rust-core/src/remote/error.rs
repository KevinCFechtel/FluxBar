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

impl RemoteError {
    /// Returns a stable diagnostic category for logging. The category is safe
    /// to emit in Release logs and avoids relying on localized or verbose
    /// error strings.
    pub fn category(&self) -> &'static str {
        match self {
            RemoteError::NotAuthorized
            | RemoteError::Forbidden
            | RemoteError::NotFound
            | RemoteError::BadRequest(_)
            | RemoteError::ServerError(_)
            | RemoteError::Status(_) => "http_status",
            RemoteError::Transport(message) => classify_transport(message),
            RemoteError::Json(_) => "decode",
        }
    }

    /// A log-safe summary that never exposes full URLs, raw HTML, or request
    /// bodies. Transport errors are reduced to their category; typed HTTP and
    /// JSON errors keep their safe Display text.
    pub fn log_safe_summary(&self) -> String {
        match self {
            RemoteError::Transport(_) => self.category().to_string(),
            other => other.to_string(),
        }
    }
}

fn classify_transport(message: &str) -> &'static str {
    let lower = message.to_ascii_lowercase();
    if lower.contains("timed out") || lower.contains("timeout") || lower.contains("time limit") {
        return "timeout";
    }
    if lower.contains("dns") || lower.contains("resolve") || lower.contains("lookup") {
        return "dns";
    }
    if lower.contains("tls") || lower.contains("certificate") || lower.contains("cert") {
        return "tls";
    }
    if lower.contains("network is unreachable") || lower.contains("offline") {
        return "network_unavailable";
    }
    if lower.contains("connection refused") {
        return "connection_refused";
    }
    "other"
}

impl std::error::Error for RemoteError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn category_maps_typed_errors_to_safe_names() {
        assert_eq!(RemoteError::NotAuthorized.category(), "http_status");
        assert_eq!(RemoteError::Status(429).category(), "http_status");
        assert_eq!(RemoteError::Json("bad json".into()).category(), "decode");
    }

    #[test]
    fn transport_classification_recognizes_common_failures() {
        let cases = [
            ("timed out waiting for connection", "timeout"),
            ("dns error: failed to resolve", "dns"),
            ("tls certificate invalid", "tls"),
            ("Network is unreachable", "network_unavailable"),
            ("connection refused", "connection_refused"),
            ("something unexpected", "other"),
        ];
        for (message, expected) in cases {
            assert_eq!(
                RemoteError::Transport(message.to_string()).category(),
                expected,
                "message: {message}"
            );
        }
    }

    #[test]
    fn log_safe_summary_drops_transport_detail() {
        let error = RemoteError::Transport("may contain a url or host".to_string());
        assert_eq!(error.log_safe_summary(), "other");
    }

    #[test]
    fn log_safe_summary_keeps_typed_error_detail() {
        let error = RemoteError::Status(500);
        assert_eq!(error.log_safe_summary(), "miniflux: status code=500");
    }
}
