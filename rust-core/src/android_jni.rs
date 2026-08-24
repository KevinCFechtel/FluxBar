//! Feature-gated Android JNI helper for the mobile runtime proof.
//!
//! Compiled only when both `target_os = "android"` and the
//! `mobile-runtime-proof` feature are enabled. This module provides one
//! non-production JNI entry point used by the proof-host C/C++ shim to
//! initialize `rustls-platform-verifier` with a JVM `Context` before any
//! HTTPS request runs.
//!
//! This symbol is **not** part of the stable two-symbol FluxBar C ABI and is
//! absent from default/production artifacts.

use std::ffi::c_void;

use jni::JNIEnv;
use jni::objects::JObject;

/// Initializes the Android platform TLS verifier from raw JNI pointers.
///
/// The proof-host JNI shim passes the current `JNIEnv` and application
/// `Context` as opaque pointers. This function reconstructs the `jni-rs`
/// types, calls `rustls_platform_verifier::android::init_hosted`, and returns
/// `0` on success or `1` on failure. On failure it also throws a Java
/// `RuntimeException` so Kotlin/Swift callers see a typed error rather than a
/// generic native crash.
///
/// # Safety
///
/// `raw_env` must be a valid `JNIEnv` pointer and `raw_context` must be a
/// valid Android `Context` jobject. Both must remain valid for the duration of
/// this call. This function is intended to be called exactly once per process
/// before any HTTPS-capable request; the underlying verifier initializer is
/// idempotent.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fluxbar_mobile_proof_init_android_verifier(
    raw_env: *mut c_void,
    raw_context: *mut c_void,
) -> i32 {
    // `from_raw` is unsafe because the pointers must be valid JNI handles.
    // The calling JNI method owns both handles for the duration of the call.
    let mut env = match unsafe { JNIEnv::from_raw(raw_env as *mut jni::sys::JNIEnv) } {
        Ok(env) => env,
        Err(_) => return 1,
    };
    let context = unsafe { JObject::from_raw(raw_context as jni::sys::jobject) };

    match rustls_platform_verifier::android::init_hosted(&mut env, context) {
        Ok(()) => 0,
        Err(error) => {
            let message = format!("rustls-platform-verifier initialization failed: {error:?}");
            let _ = env.throw_new("java/lang/RuntimeException", &message);
            1
        }
    }
}
