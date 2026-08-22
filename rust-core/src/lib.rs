//! Minimal FluxBar core skeleton.
//!
//! This crate exports the same C ABI as the existing Go core:
//!
//! ```c
//! extern char* FluxCoreRequest(char* request);
//! extern void FluxCoreFree(char* value);
//! ```
//!
//! It does not implement SQLite, Miniflux, snapshots, icons, localization,
//! mutations, or sync. For otherwise valid input it returns a deterministic
//! "not implemented" JSON response. All `unsafe` code is confined to the FFI
//! boundary below.

use serde::{Deserialize, Serialize};
use std::ffi::{CStr, CString, c_char};

/// Minimal response shape used to produce deterministic JSON.
#[derive(Debug, Serialize)]
struct Response<'a> {
    ok: bool,
    #[serde(skip_serializing_if = "str::is_empty", default)]
    error: &'a str,
}

/// The JSON error returned when the C request pointer is null.
const NULL_REQUEST_ERROR: &str = r#"{"ok":false,"error":"null request"}"#;

/// Builds an invalid-request error response, escaping `reason` via serde_json.
fn invalid_request_error(reason: &str) -> String {
    let message = format!("invalid request: {reason}");
    serde_json::to_string(&Response {
        ok: false,
        error: &message,
    })
    .unwrap_or_else(|_| r#"{"ok":false,"error":"invalid request"}"#.to_string())
}

/// Builds a deterministic "not implemented" response for the given operation.
fn not_implemented_response(operation: &str) -> String {
    let message = format!("not implemented: {operation}");
    serde_json::to_string(&Response {
        ok: false,
        error: &message,
    })
    .unwrap_or_else(|_| r#"{"ok":false,"error":"not implemented"}"#.to_string())
}

/// Minimal request shape used only to extract the operation name.
#[derive(Debug, Deserialize)]
struct Request<'a> {
    #[serde(borrow)]
    operation: &'a str,
}

/// Parses the operation name from a UTF-8 JSON request.
///
/// Returns the raw operation string on success, or an owned error JSON string
/// on failure. The caller is responsible for converting the result into a
/// core-owned C string.
fn handle_json(request: &str) -> Result<&str, String> {
    let parsed: Request =
        serde_json::from_str(request).map_err(|e| invalid_request_error(&e.to_string()))?;
    Ok(parsed.operation)
}

/// Converts a Rust string into a core-owned, null-terminated C string.
///
/// Returns a pointer suitable for returning across the FFI boundary. The
/// caller must later release it with `FluxCoreFree`.
fn into_owned_c_string(value: String) -> *mut c_char {
    match CString::new(value) {
        Ok(c_string) => c_string.into_raw(),
        // NUL bytes cannot appear in the deterministic JSON responses produced
        // by this skeleton, but we handle the edge case defensively.
        Err(_) => CString::new(r#"{"ok":false,"error":"encode response"}"#)
            .expect("static response contains no NUL")
            .into_raw(),
    }
}

/// Processes a JSON request and returns a core-owned C-string response.
///
/// # Safety
///
/// `request` must either be null or a valid pointer to a null-terminated
/// UTF-8 C string. The pointer is borrowed for the duration of this call and
/// is not retained by the core.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn FluxCoreRequest(request: *mut c_char) -> *mut c_char {
    if request.is_null() {
        return into_owned_c_string(NULL_REQUEST_ERROR.to_string());
    }

    // Safety: we just checked for null; the Swift caller passes a
    // null-terminated UTF-8 C string and does not modify it concurrently.
    let c_str = unsafe { CStr::from_ptr(request) };

    let request_str = match c_str.to_str() {
        Ok(s) => s,
        Err(_) => return into_owned_c_string(invalid_request_error("request is not valid UTF-8")),
    };

    let response = match handle_json(request_str) {
        Ok(operation) => not_implemented_response(operation),
        Err(err_json) => err_json,
    };

    into_owned_c_string(response)
}

/// Releases a response string previously returned by `FluxCoreRequest`.
///
/// # Safety
///
/// `value` must either be null or a pointer previously returned by
/// `FluxCoreRequest` (i.e. produced by `CString::into_raw`). Passing any
/// other pointer is undefined behavior.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn FluxCoreFree(value: *mut c_char) {
    if value.is_null() {
        return;
    }

    // Safety: by contract the caller only passes pointers returned from
    // `FluxCoreRequest`, which were created via `CString::into_raw`. We
    // reconstitute the `CString` here so it is dropped and freed.
    unsafe {
        drop(CString::from_raw(value));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_request_error_is_deterministic() {
        assert_eq!(NULL_REQUEST_ERROR, r#"{"ok":false,"error":"null request"}"#);
    }

    #[test]
    fn not_implemented_response_includes_operation() {
        let response = not_implemented_response("refresh");
        assert!(response.contains("\"ok\":false"));
        assert!(response.contains("\"error\""));
        assert!(response.contains("refresh"));
    }

    #[test]
    fn handle_json_extracts_operation() {
        assert_eq!(
            handle_json(r#"{"operation":"refresh"}"#).unwrap(),
            "refresh"
        );
    }

    #[test]
    fn handle_json_rejects_malformed_json() {
        assert!(handle_json("not-json").is_err());
        assert!(handle_json(r#"{"server":"x"}"#).is_err());
    }

    #[test]
    fn flux_core_request_null_returns_error() {
        // Safety: passing null is explicitly allowed by the contract.
        let ptr = unsafe { FluxCoreRequest(std::ptr::null_mut()) };
        assert!(!ptr.is_null());
        unsafe {
            let response = CStr::from_ptr(ptr).to_string_lossy();
            assert_eq!(response, NULL_REQUEST_ERROR);
            FluxCoreFree(ptr);
        }
    }

    #[test]
    fn flux_core_request_roundtrip() {
        let request = CString::new(r#"{"operation":"local_snapshot"}"#).unwrap();
        // Safety: request is a valid NUL-terminated C string owned by this test.
        let ptr = unsafe { FluxCoreRequest(request.into_raw()) };
        assert!(!ptr.is_null());
        unsafe {
            let response = CStr::from_ptr(ptr).to_string_lossy();
            assert!(response.contains("\"ok\":false"));
            assert!(response.contains("not implemented: local_snapshot"));
            FluxCoreFree(ptr);
        }
    }

    #[test]
    fn flux_core_request_invalid_utf8() {
        // Invalid UTF-8 sequence wrapped in a NUL-terminated C string.
        let bytes: Vec<u8> = vec![0x22, 0xc3, 0x28, 0x22, 0x00];
        let request = CString::from_vec_with_nul(bytes).unwrap();
        // Safety: request is a valid NUL-terminated byte sequence.
        let ptr = unsafe { FluxCoreRequest(request.into_raw()) };
        assert!(!ptr.is_null());
        unsafe {
            let response = CStr::from_ptr(ptr).to_string_lossy();
            assert!(response.contains("\"ok\":false"));
            assert!(response.contains("invalid request"));
            FluxCoreFree(ptr);
        }
    }

    #[test]
    fn flux_core_free_null_is_noop() {
        // Safety: FluxCoreFree explicitly documents null as a no-op.
        unsafe {
            FluxCoreFree(std::ptr::null_mut());
        }
    }
}
