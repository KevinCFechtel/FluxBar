//! Feature-gated mobile runtime proof probes.
//!
//! Compiled only when the `mobile-runtime-proof` feature is enabled. These
//! operations exercise FFI ownership, SQLite persistence, HTTPS/TLS, threading,
//! and panic containment without touching the production FluxBar schema or
//! operations.

use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::Duration;

use rusqlite::Connection;
use sha2::{Digest, Sha256};

use crate::remote::miniflux::build_http_agent;
use crate::transport::Response;

/// Fixed confirmation token required for the intentional `panic` probe.
/// Chosen to be obvious in test code and unlikely to be typed accidentally.
const PANIC_CONFIRMATION: &str = "confirm-intentional-probe-panic";

/// Maximum payload size accepted by `round_trip` to avoid unbounded allocation.
const MAX_ROUND_TRIP_SIZE: i64 = 4 * 1024 * 1024;

/// Maximum number of iterations the `thread_probe` will honor.
const MAX_THREAD_ITERATIONS: i64 = 10_000;

/// Default HTTPS timeout when the host does not specify one.
const DEFAULT_HTTPS_TIMEOUT_MS: i64 = 15_000;

/// Maximum HTTPS timeout to prevent tests from hanging.
const MAX_HTTPS_TIMEOUT_MS: i64 = 60_000;

/// Retained probe SQLite connection. The host is responsible for calling
/// `sqlite_close` before process death when it wants a clean close; abrupt
/// termination is also acceptable because SQLite WAL recovery handles it.
static PROBE_CONNECTION: Mutex<Option<Connection>> = Mutex::new(None);

/// Parameters for a single `mobile_runtime_probe` action.
#[derive(Debug, Clone, Default)]
pub struct ProbeParams {
    pub action: String,
    pub payload: String,
    pub size: i64,
    pub allowed_root: String,
    pub db_filename: String,
    pub key: String,
    pub value: String,
    pub url: String,
    pub timeout_ms: i64,
    pub iterations: i64,
    pub confirm_panic: String,
}

/// Dispatches a probe action to its handler.
pub fn dispatch(params: ProbeParams) -> Response {
    match params.action.as_str() {
        "runtime_info" => runtime_info(),
        "round_trip" => round_trip(&params.payload, params.size),
        "sqlite_open" => sqlite_open(&params.allowed_root, &params.db_filename),
        "sqlite_write" => sqlite_write(&params.key, &params.value),
        "sqlite_read" => sqlite_read(&params.key),
        "sqlite_close" => sqlite_close(),
        "https_get" => https_get(&params.url, params.timeout_ms),
        "thread_probe" => thread_probe(params.iterations),
        "panic" => panic_probe(&params.confirm_panic),
        "" => Response {
            ok: false,
            error: "missing probeAction".to_string(),
            ..Response::default()
        },
        other => Response {
            ok: false,
            error: format!("unsupported probeAction \"{other}\""),
            ..Response::default()
        },
    }
}

fn runtime_info() -> Response {
    let info = serde_json::json!({
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "pointerWidth": usize::BITS,
        "crateVersion": env!("CARGO_PKG_VERSION"),
        "buildProfile": if cfg!(debug_assertions) { "debug" } else { "release" },
        "panicStrategy": if cfg!(panic = "abort") { "abort" } else { "unwind" },
        "mobileRuntimeProofEnabled": true,
    });
    Response {
        ok: true,
        data: Some(info),
        ..Response::default()
    }
}

fn round_trip(payload: &str, requested_size: i64) -> Response {
    let requested_size = requested_size.clamp(0, MAX_ROUND_TRIP_SIZE) as usize;
    let received_len = payload.len();

    // Echo the payload and, if requested, pad the echoed field to a bounded
    // size so the host can exercise large response allocation. A requested size
    // of zero means "echo the payload unchanged".
    let echoed = if requested_size == 0 || requested_size == payload.len() {
        payload.to_string()
    } else if requested_size < payload.len() {
        payload.chars().take(requested_size).collect::<String>()
    } else {
        let mut buffer = String::with_capacity(requested_size);
        buffer.push_str(payload);
        while buffer.len() < requested_size {
            buffer.push('.'); // ASCII filler so truncation is always on a char boundary
        }
        // Ensure exact byte length if the placeholder pushed us over.
        buffer.truncate(requested_size);
        buffer
    };

    Response {
        ok: true,
        data: Some(serde_json::json!({
            "receivedLength": received_len,
            "requestedSize": requested_size,
            "echoed": echoed,
        })),
        ..Response::default()
    }
}

fn sqlite_open(allowed_root: &str, db_filename: &str) -> Response {
    if allowed_root.is_empty() {
        return error_response("probeAllowedRoot is empty");
    }
    if db_filename.is_empty() {
        return error_response("probeDbFilename is empty");
    }

    let resolved = match resolve_probe_db_path(allowed_root, db_filename) {
        Ok(path) => path,
        Err(error) => return error_response(&error),
    };

    let mut guard = probe_connection();
    if guard.is_some() {
        return error_response("probe connection already open");
    }

    let conn = match Connection::open(&resolved) {
        Ok(conn) => conn,
        Err(error) => {
            return error_response(&format!("sqlite open failed: {error}"));
        }
    };

    if let Err(error) = conn
        .execute_batch("CREATE TABLE IF NOT EXISTS probe_kv (key TEXT PRIMARY KEY, value TEXT);")
    {
        return error_response(&format!("sqlite schema failed: {error}"));
    }

    let journal_mode: String = conn
        .query_row("PRAGMA journal_mode;", [], |row| row.get(0))
        .unwrap_or_default();
    let synchronous: i64 = conn
        .query_row("PRAGMA synchronous;", [], |row| row.get(0))
        .unwrap_or_default();
    let busy_timeout: i64 = conn
        .query_row("PRAGMA busy_timeout;", [], |row| row.get(0))
        .unwrap_or_default();

    // Configure WAL and a short busy timeout for the proof. These match the
    // production intent and are surfaced in the response for evidence.
    let _ = conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA busy_timeout = 5000;",
    );

    let journal_mode_after: String = conn
        .query_row("PRAGMA journal_mode;", [], |row| row.get(0))
        .unwrap_or_default();
    let synchronous_after: i64 = conn
        .query_row("PRAGMA synchronous;", [], |row| row.get(0))
        .unwrap_or_default();
    let busy_timeout_after: i64 = conn
        .query_row("PRAGMA busy_timeout;", [], |row| row.get(0))
        .unwrap_or_default();

    *guard = Some(conn);

    Response {
        ok: true,
        data: Some(serde_json::json!({
            "path": resolved.to_string_lossy(),
            "journalMode": journal_mode,
            "synchronous": synchronous,
            "busyTimeout": busy_timeout,
            "journalModeAfter": journal_mode_after,
            "synchronousAfter": synchronous_after,
            "busyTimeoutAfter": busy_timeout_after,
        })),
        ..Response::default()
    }
}

fn sqlite_write(key: &str, value: &str) -> Response {
    if key.is_empty() {
        return error_response("probeKey is empty");
    }
    let mut guard = probe_connection();
    let conn = match guard.as_mut() {
        Some(conn) => conn,
        None => return error_response("probe connection is not open"),
    };
    match conn.execute(
        "INSERT OR REPLACE INTO probe_kv (key, value) VALUES (?1, ?2)",
        [key, value],
    ) {
        Ok(rows) => Response {
            ok: true,
            data: Some(serde_json::json!({"rowsAffected": rows})),
            ..Response::default()
        },
        Err(error) => error_response(&format!("sqlite write failed: {error}")),
    }
}

fn sqlite_read(key: &str) -> Response {
    if key.is_empty() {
        return error_response("probeKey is empty");
    }
    let mut guard = probe_connection();
    let conn = match guard.as_mut() {
        Some(conn) => conn,
        None => return error_response("probe connection is not open"),
    };
    match conn.query_row("SELECT value FROM probe_kv WHERE key = ?1", [key], |row| {
        row.get::<_, Option<String>>(0)
    }) {
        Ok(Some(value)) => Response {
            ok: true,
            data: Some(serde_json::json!({"value": value, "found": true})),
            ..Response::default()
        },
        Ok(None) => Response {
            ok: true,
            data: Some(serde_json::json!({"value": serde_json::Value::Null, "found": false})),
            ..Response::default()
        },
        Err(error) => error_response(&format!("sqlite read failed: {error}")),
    }
}

fn sqlite_close() -> Response {
    let mut guard = probe_connection();
    match guard.take() {
        Some(conn) => match conn.close() {
            Ok(_) => Response {
                ok: true,
                data: Some(serde_json::json!({"closed": true})),
                ..Response::default()
            },
            Err((_conn, error)) => error_response(&format!("sqlite close failed: {error}")),
        },
        None => Response {
            ok: true,
            data: Some(serde_json::json!({"closed": false, "note": "no open connection"})),
            ..Response::default()
        },
    }
}

fn https_get(url: &str, timeout_ms: i64) -> Response {
    if url.is_empty() {
        return error_response("probeUrl is empty");
    }
    let parsed = match url::Url::parse(url) {
        Ok(url) => url,
        Err(error) => return error_response(&format!("invalid URL: {error}")),
    };
    if parsed.scheme() != "https" {
        return error_response("only https URLs are allowed");
    }

    let timeout_ms = timeout_ms
        .clamp(1, MAX_HTTPS_TIMEOUT_MS)
        .max(DEFAULT_HTTPS_TIMEOUT_MS);
    let timeout = Duration::from_millis(timeout_ms as u64);

    let agent = match build_http_agent() {
        Ok(agent) => agent,
        Err(error) => return error_response(&error.to_string()),
    };

    let request = agent
        .request("GET", url)
        .set("User-Agent", "FluxBarMobileRuntimeProof/1.0")
        .timeout(timeout);

    let start = std::time::Instant::now();
    match request.call() {
        Ok(response) => {
            let status = response.status();
            let content_length: Option<i64> = response
                .header("Content-Length")
                .and_then(|v| v.parse().ok());
            let elapsed_ms = start.elapsed().as_millis();

            // Read enough of the body to produce a deterministic digest without
            // logging the full response content.
            let body_digest = match response.into_string() {
                Ok(body) => {
                    let mut hasher = Sha256::new();
                    hasher.update(body.as_bytes());
                    format!("{:x}", hasher.finalize())
                }
                Err(error) => {
                    return error_response(&format!("body read failed: {error}"));
                }
            };

            Response {
                ok: true,
                data: Some(serde_json::json!({
                    "status": status,
                    "contentLength": content_length,
                    "bodyDigest": body_digest,
                    "elapsedMs": elapsed_ms,
                })),
                ..Response::default()
            }
        }
        Err(error) => {
            let elapsed_ms = start.elapsed().as_millis();
            let (category, detail) = categorize_ureq_error(error);
            Response {
                ok: false,
                error: format!("{category}: {detail}"),
                data: Some(serde_json::json!({
                    "category": category,
                    "detail": detail,
                    "elapsedMs": elapsed_ms,
                })),
                ..Response::default()
            }
        }
    }
}

fn thread_probe(iterations: i64) -> Response {
    let iterations = iterations.clamp(0, MAX_THREAD_ITERATIONS) as usize;
    let counter = Arc::new(Mutex::new(0usize));
    let mut handles = Vec::with_capacity(iterations);

    for _ in 0..iterations {
        let counter = Arc::clone(&counter);
        handles.push(thread::spawn(move || {
            let mut value = counter.lock().expect("probe mutex poisoned");
            *value += 1;
        }));
    }

    for handle in handles {
        if let Err(error) = handle.join() {
            return error_response(&format!("thread join failed: {error:?}"));
        }
    }

    let final_count = *counter.lock().expect("probe mutex poisoned");
    Response {
        ok: true,
        data: Some(serde_json::json!({
            "iterations": iterations,
            "finalCount": final_count,
        })),
        ..Response::default()
    }
}

fn panic_probe(confirm_panic: &str) -> Response {
    if confirm_panic != PANIC_CONFIRMATION {
        return Response {
            ok: false,
            error: format!("panic probe rejected: confirm_panic must be \"{PANIC_CONFIRMATION}\""),
            ..Response::default()
        };
    }
    panic!("intentional mobile runtime proof panic");
}

fn error_response(message: &str) -> Response {
    Response {
        ok: false,
        error: message.to_string(),
        ..Response::default()
    }
}

fn probe_connection() -> MutexGuard<'static, Option<Connection>> {
    PROBE_CONNECTION
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Resolves `allowed_root / db_filename` and verifies the result remains inside
/// the canonical allowed root. Rejects absolute names, parent references,
/// separators, and symlinks.
fn resolve_probe_db_path(allowed_root: &str, db_filename: &str) -> Result<PathBuf, String> {
    let root = Path::new(allowed_root);
    if !root.is_absolute() {
        return Err("probeAllowedRoot must be absolute".to_string());
    }

    let file = Path::new(db_filename);
    if file.is_absolute() {
        return Err("probeDbFilename must be relative".to_string());
    }
    if file.components().count() != 1 {
        return Err("probeDbFilename must be a single path component".to_string());
    }
    match file.components().next() {
        Some(Component::Normal(_)) => {}
        _ => return Err("probeDbFilename is not a plain filename".to_string()),
    }
    if db_filename.contains("..") {
        return Err("probeDbFilename may not contain parent references".to_string());
    }

    let canonical_root =
        fs::canonicalize(root).map_err(|e| format!("failed to canonicalize allowed root: {e}"))?;
    let resolved = canonical_root.join(file);
    let canonical_resolved = fs::canonicalize(&resolved).unwrap_or_else(|_| resolved.clone());

    let parent = canonical_resolved
        .parent()
        .ok_or_else(|| "resolved database path has no parent".to_string())?;
    if parent != canonical_root {
        return Err("resolved database path escapes allowed root".to_string());
    }

    // Detect symlinks in the final component. fs::canonicalize already resolves
    // symlinks, so if the canonical parent differs from the expected root we
    // have already rejected it; this explicit check records intent.
    if resolved != canonical_resolved {
        return Err("database path resolves through a symlink".to_string());
    }

    Ok(resolved)
}

fn categorize_ureq_error(error: ureq::Error) -> (String, String) {
    match error {
        ureq::Error::Status(code, response) => {
            let body = response
                .into_string()
                .unwrap_or_else(|_| String::from("<unreadable body>"));
            (format!("http_{code}"), body)
        }
        ureq::Error::Transport(transport) => {
            let detail = transport
                .message()
                .map(|s| s.to_string())
                .unwrap_or_default();
            let kind = transport.kind();
            let category = if kind == ureq::ErrorKind::Dns {
                "dns"
            } else if kind == ureq::ErrorKind::ConnectionFailed {
                "connection"
            } else if kind == ureq::ErrorKind::InvalidUrl {
                "invalid_url"
            } else if kind == ureq::ErrorKind::Io {
                "io"
            } else {
                "transport"
            };
            (category.to_string(), detail)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_info_reports_proof_enabled() {
        let resp = runtime_info();
        assert!(resp.ok);
        let data = resp
            .data
            .as_ref()
            .unwrap()
            .as_object()
            .expect("data object");
        assert_eq!(
            data.get("mobileRuntimeProofEnabled"),
            Some(&serde_json::json!(true))
        );
        assert_eq!(
            data.get("panicStrategy"),
            Some(&serde_json::json!("unwind"))
        );
    }

    #[test]
    fn round_trip_echoes_payload() {
        let resp = round_trip("héllo 🌍", 0);
        assert!(resp.ok);
        assert_eq!(resp.data.as_ref().unwrap()["receivedLength"], 11);
        assert_eq!(resp.data.as_ref().unwrap()["echoed"], "héllo 🌍");
    }

    #[test]
    fn round_trip_bounds_size() {
        let resp = round_trip("", MAX_ROUND_TRIP_SIZE + 1);
        assert!(resp.ok);
        assert_eq!(
            resp.data.as_ref().unwrap()["requestedSize"],
            MAX_ROUND_TRIP_SIZE
        );
    }

    #[test]
    fn sqlite_round_trip_persists() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().to_str().unwrap();

        let open = sqlite_open(root, "probe.db");
        assert!(open.ok, "{}", open.error);
        assert_eq!(open.data.as_ref().unwrap()["journalModeAfter"], "wal");

        let write = sqlite_write("greeting", "héllo");
        assert!(write.ok, "{}", write.error);

        let read = sqlite_read("greeting");
        assert!(read.ok, "{}", read.error);
        assert_eq!(read.data.as_ref().unwrap()["value"], "héllo");

        let close = sqlite_close();
        assert!(close.ok, "{}", close.error);
    }

    #[test]
    fn path_containment_rejects_escape() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().to_str().unwrap();
        let err = resolve_probe_db_path(root, "../escape.db").unwrap_err();
        assert!(err.contains("single path component"), "{err}");
    }

    #[test]
    fn path_containment_rejects_absolute() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().to_str().unwrap();
        let err = resolve_probe_db_path(root, "/etc/passwd").unwrap_err();
        assert!(err.contains("relative"), "{err}");
    }

    #[test]
    fn thread_probe_counts_correctly() {
        let resp = thread_probe(100);
        assert!(resp.ok, "{}", resp.error);
        assert_eq!(resp.data.as_ref().unwrap()["iterations"], 100);
        assert_eq!(resp.data.as_ref().unwrap()["finalCount"], 100);
    }

    #[test]
    fn panic_probe_rejects_bad_token() {
        let resp = panic_probe("wrong");
        assert!(!resp.ok);
        assert!(resp.error.contains("panic probe rejected"));
    }
}
