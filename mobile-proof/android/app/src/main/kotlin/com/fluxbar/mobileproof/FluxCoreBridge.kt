package com.fluxbar.mobileproof

import android.content.Context
import android.util.Log

/**
 * Kotlin wrapper around the JNI shim that exposes the Rust two-symbol C ABI.
 *
 * The native library is loaded once per process. [initVerifier] must be called
 * with an application [Context] before any HTTPS-capable request, because
 * `rustls-platform-verifier` needs the JVM Trust Manager on Android.
 *
 * No raw Rust pointer is stored or exposed to Kotlin callers.
 */
object FluxCoreBridge {

    private const val TAG = "FluxCoreBridge"
    private var verifierInitialized = false

    init {
        System.loadLibrary("fluxcore_mobile_probe")
        Log.i(TAG, "Loaded libfluxcore_mobile_probe")
    }

    /**
     * Idempotently initialize the platform TLS verifier. Must be called with an
     * application context before [request] is used for HTTPS. Returns true if the
     * verifier is ready (including if it was already initialized).
     */
    @JvmStatic
    fun initVerifier(context: Context): Boolean {
        if (verifierInitialized) {
            return true
        }
        val ok = initVerifierNative(context)
        if (ok) {
            verifierInitialized = true
        }
        return ok
    }

    /**
     * Sends a JSON request to the Rust core and returns the JSON response string.
     * The caller is responsible for parsing the returned JSON.
     */
    @JvmStatic
    external fun request(json: String): String

    @JvmStatic
    private external fun initVerifierNative(context: Context): Boolean
}
