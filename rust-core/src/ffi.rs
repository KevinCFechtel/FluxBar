//! C ABI / FFI boundary.
//!
//! All `unsafe` code is confined here. The exported functions are:
//!
//! ```c
//! extern char* FluxCoreRequest(char* request);
//! extern void FluxCoreFree(char* value);
//! ```
//!
//! Memory ownership:
//! - The request pointer is borrowed for the duration of the call.
//! - Every non-null response pointer is allocated by this crate and must be
//!   released with `FluxCoreFree`.
//! - `FluxCoreFree(null)` is a no-op.
//!
//! Panic safety:
//! `FluxCoreRequest` wraps processing in `catch_unwind`. A panic becomes a
//! deterministic JSON error response and never unwinds across the C ABI.

use std::ffi::{CStr, CString, c_char};
use std::panic::AssertUnwindSafe;

use crate::dispatcher::dispatch;
use crate::runtime::AppRuntime;
use crate::transport::{Request, Response};

/// Process-wide runtime shared by every request, mirroring Go's package-level
/// `runtime` variable.
static RUNTIME: AppRuntime = AppRuntime::new();

const PANIC_ERROR: &str = r#"{"ok":false,"error":"internal error"}"#;

/// Processes a JSON request and returns a core-owned null-terminated response.
///
/// # Safety
///
/// `request` must be null or a valid pointer to a null-terminated UTF-8 C
/// string. The pointer is borrowed and not retained.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn FluxCoreRequest(request: *mut c_char) -> *mut c_char {
    let input = parse_input(request);

    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        crate::logging::init();
        process(input, &RUNTIME)
    }));

    match result {
        Ok(response_json) => into_owned_c_string(response_json),
        Err(_) => {
            // A panic here means something in `process` violated an invariant.
            // We cannot rely on the logging facade being usable, but we can at
            // least record that the FFI boundary contained a panic.
            let _ = std::panic::catch_unwind(AssertUnwindSafe(|| {
                log::error!(target: "ffi", "panic contained at FFI boundary");
            }));
            into_owned_c_string(PANIC_ERROR.to_string())
        }
    }
}

fn parse_input(request: *mut c_char) -> Input {
    if request.is_null() {
        return Input::Null;
    }

    let c_str = unsafe { CStr::from_ptr(request) };

    match c_str.to_str() {
        Ok(s) => Input::Utf8(s.to_string()),
        Err(e) => Input::InvalidUtf8(e.to_string()),
    }
}

#[derive(Debug)]
enum Input {
    Null,
    Utf8(String),
    InvalidUtf8(String),
}

fn process(input: Input, runtime: &AppRuntime) -> String {
    match input {
        Input::Null => {
            log::error!(target: "ffi", "null request received");
            Response::null_request().to_json()
        }
        Input::InvalidUtf8(reason) => {
            log::error!(target: "ffi", "invalid UTF-8 request: {reason}");
            Response::invalid_request(&format!("request is not valid UTF-8: {reason}")).to_json()
        }
        Input::Utf8(json) => handle_json(&json, runtime),
    }
}

fn handle_json(json: &str, runtime: &AppRuntime) -> String {
    let request: Request = if json.trim() == "null" {
        Request::default()
    } else {
        match crate::transport::request::parse_request(json) {
            Ok(r) => r,
            Err(e) => {
                log::warn!(target: "ffi", "malformed JSON request: {e}");
                return Response::invalid_request(&e.to_string()).to_json();
            }
        }
    };

    let operation = match request.into_operation() {
        Ok(op) => op,
        Err(error) => {
            // The error string from `into_operation` is already formatted as
            // `unsupported operation "..."` to match Go. Preserve it directly.
            log::warn!(target: "ffi", "{error}");
            return Response {
                ok: false,
                error,
                ..Default::default()
            }
            .to_json();
        }
    };

    dispatch(operation, runtime).to_json()
}

/// Releases a response string previously returned by `FluxCoreRequest`.
///
/// # Safety
///
/// `value` must be null or a pointer previously returned by `FluxCoreRequest`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn FluxCoreFree(value: *mut c_char) {
    if value.is_null() {
        return;
    }

    unsafe {
        drop(CString::from_raw(value));
    }
}

fn into_owned_c_string(value: String) -> *mut c_char {
    match CString::new(value) {
        Ok(c_string) => c_string.into_raw(),
        Err(_) => CString::new(r#"{"ok":false,"error":"encode response"}"#)
            .expect("static response contains no NUL")
            .into_raw(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    #[test]
    fn null_request() {
        let ptr = unsafe { FluxCoreRequest(std::ptr::null_mut()) };
        assert!(!ptr.is_null());
        unsafe {
            let response = CStr::from_ptr(ptr).to_string_lossy();
            assert_eq!(response, r#"{"ok":false,"error":"null request"}"#);
            FluxCoreFree(ptr);
        }
    }

    #[test]
    fn malformed_json() {
        let request = CString::new("not-json").unwrap();
        let ptr = unsafe { FluxCoreRequest(request.into_raw()) };
        assert!(!ptr.is_null());
        unsafe {
            let response = CStr::from_ptr(ptr).to_string_lossy();
            assert!(response.contains(r#""ok":false"#));
            assert!(response.contains("invalid request"));
            FluxCoreFree(ptr);
        }
    }

    #[test]
    fn unknown_operation() {
        let request = CString::new(r#"{"operation":"unknown"}"#).unwrap();
        let ptr = unsafe { FluxCoreRequest(request.into_raw()) };
        assert!(!ptr.is_null());
        unsafe {
            let response = CStr::from_ptr(ptr).to_string_lossy();
            assert!(response.contains(r#""ok":false"#));
            assert!(response.contains("unsupported operation"));
            assert!(response.contains("unknown"));
            FluxCoreFree(ptr);
        }
    }

    #[test]
    fn root_json_null_uses_empty_request_defaults() {
        let response = handle_json("null", &AppRuntime::new());
        assert_eq!(
            response,
            r#"{"ok":false,"error":"unsupported operation \"\""}"#
        );
    }

    #[test]
    fn invalid_utf8() {
        let bytes: Vec<u8> = vec![0x22, 0xc3, 0x28, 0x22, 0x00];
        let request = CString::from_vec_with_nul(bytes).unwrap();
        let ptr = unsafe { FluxCoreRequest(request.into_raw()) };
        assert!(!ptr.is_null());
        unsafe {
            let response = CStr::from_ptr(ptr).to_string_lossy();
            assert!(response.contains(r#""ok":false"#));
            assert!(response.contains("invalid request"));
            FluxCoreFree(ptr);
        }
    }

    #[test]
    fn free_null_is_noop() {
        unsafe { FluxCoreFree(std::ptr::null_mut()) };
    }

    #[test]
    fn malformed_input_does_not_panic() {
        // serde_json does not panic on malformed input, so this does not
        // exercise the catch_unwind recovery path directly. It does prove
        // that the FFI entry point returns a valid JSON error for invalid
        // input instead of aborting.
        let request = CString::new("{").unwrap();
        let ptr = unsafe { FluxCoreRequest(request.into_raw()) };
        assert!(!ptr.is_null());
        unsafe {
            let response = CStr::from_ptr(ptr).to_string_lossy();
            assert!(response.contains(r#""ok":false"#));
            FluxCoreFree(ptr);
        }
    }

    #[test]
    fn catch_unwind_contains_panic() {
        // Directly verify the panic-containment mechanism used by
        // FluxCoreRequest. A panic in the closure becomes an Err, not an
        // abort.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            panic!("intentional test panic");
        }));
        assert!(result.is_err());
    }
}
