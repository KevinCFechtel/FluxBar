package com.fluxbar.mobileproof

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.async
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withContext
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import java.io.File

/**
 * Instrumentation tests for the FluxBar Rust core on Android.
 *
 * These tests exercise the same probe actions as the iOS proof host:
 * FFI invocation, JSON round-trip, SQLite persistence, HTTPS/TLS trust store,
 * threading, panic containment, and lifecycle/ownership stress.
 */
@RunWith(AndroidJUnit4::class)
class FluxCoreInstrumentedTest {

    private lateinit var context: Context
    private lateinit var probeDir: File

    @Before
    fun setUp() {
        context = ApplicationProvider.getApplicationContext()
        probeDir = File(context.noBackupFilesDir, "proof")
        probeDir.mkdirs()
        // Initialize the TLS verifier before any HTTPS-capable request. This
        // only needs to happen once per process, but the call is idempotent.
        val verifierOk = FluxCoreBridge.initVerifier(context)
        assertTrue("TLS verifier initialization failed", verifierOk)
    }

    @After
    fun tearDown() {
        if (::probeDir.isInitialized) {
            probeDir.listFiles()?.forEach { it.deleteRecursively() }
        }
    }

    // MARK: - FFI and invocation

    @Test
    fun runtime_info_reports_android_and_proof_enabled() {
        val response = callProbe("runtime_info")
        assertProbeOk(response)
        val data = response.getJSONObject("data")
        assertEquals("android", data.getString("os"))
        assertEquals(true, data.getBoolean("mobileRuntimeProofEnabled"))
        assertEquals("unwind", data.getString("panicStrategy"))
        assertTrue(data.getInt("pointerWidth") == 32 || data.getInt("pointerWidth") == 64)
    }

    @Test
    fun malformed_json_returns_controlled_error() {
        val response = JSONObject(FluxCoreBridge.request("{"))
        assertFalse(response.getBoolean("ok"))
        assertTrue(response.getString("error").contains("invalid request"))
    }

    @Test
    fun unknown_operation_returns_controlled_error() {
        val response = JSONObject(
            FluxCoreBridge.request("""{"operation":"no_such_operation"}""")
        )
        assertFalse(response.getBoolean("ok"))
        assertTrue(response.getString("error").contains("unsupported operation"))
    }

    // MARK: - Round-trip

    @Test
    fun round_trip_unicode_payload() {
        val payload = "héllo Android 🌍 \\n\\t\"quoted\""
        val response = callProbe(
            "round_trip",
            mapOf("probePayload" to payload, "probeSize" to 0)
        )
        assertProbeOk(response)
        val data = response.getJSONObject("data")
        assertEquals(payload, data.getString("echoed"))
    }

    @Test
    fun round_trip_concurrent_no_crossover() = runBlocking {
        val iterations = 8
        val results = (0 until iterations).map { index ->
            async(Dispatchers.IO) {
                val payload = "payload-$index"
                val response = callProbe(
                    "round_trip",
                    mapOf("probePayload" to payload, "probeSize" to 0)
                )
                assertProbeOk(response)
                response.getJSONObject("data").getString("echoed")
            }
        }.map { it.await() }

        results.forEachIndexed { index, echoed ->
            assertEquals("crossover at $index", "payload-$index", echoed)
        }
    }

    // MARK: - SQLite persistence

    @Test
    fun sqlite_open_write_read_close_reopen() {
        val open = callProbe(
            "sqlite_open",
            mapOf("probeAllowedRoot" to probeDir.canonicalPath, "probeDbFilename" to "probe.db")
        )
        assertProbeOk(open)
        assertEquals("wal", open.getJSONObject("data").getString("journalModeAfter"))

        val write = callProbe(
            "sqlite_write",
            mapOf("probeKey" to "greeting", "probeValue" to "héllo Android")
        )
        assertProbeOk(write)

        val read = callProbe("sqlite_read", mapOf("probeKey" to "greeting"))
        assertProbeOk(read)
        assertEquals("héllo Android", read.getJSONObject("data").getString("value"))

        val close = callProbe("sqlite_close")
        assertProbeOk(close)

        val reopen = callProbe(
            "sqlite_open",
            mapOf("probeAllowedRoot" to probeDir.canonicalPath, "probeDbFilename" to "probe.db")
        )
        assertProbeOk(reopen)

        val reread = callProbe("sqlite_read", mapOf("probeKey" to "greeting"))
        assertProbeOk(reread)
        assertEquals("héllo Android", reread.getJSONObject("data").getString("value"))
    }

    @Test
    fun sqlite_path_containment_rejects_escape() {
        val response = callProbe(
            "sqlite_open",
            mapOf("probeAllowedRoot" to probeDir.canonicalPath, "probeDbFilename" to "../escape.db")
        )
        assertFalse(response.getBoolean("ok"))
        assertTrue(response.getString("error").contains("single path component"))
    }

    // MARK: - HTTPS/TLS

    @Test
    fun https_public_root_succeeds() {
        val response = callProbe(
            "https_get",
            mapOf("probeUrl" to "https://httpbin.org/get", "probeTimeoutMs" to 15000)
        )
        assertProbeOk(response)
        val data = response.getJSONObject("data")
        assertEquals(200, data.getInt("status"))
        assertNotNull(data.getString("bodyDigest"))
    }

    @Test
    fun https_invalid_certificate_fails() {
        val response = callProbe(
            "https_get",
            mapOf("probeUrl" to "https://self-signed.badssl.com/", "probeTimeoutMs" to 15000)
        )
        assertFalse(response.getBoolean("ok"))
        val category = response.getJSONObject("data").getString("category")
        assertTrue(
            "expected transport/connection error, got $category",
            category == "transport" || category == "connection"
        )
    }

    // MARK: - Threading

    @Test
    fun thread_probe_spawns_and_joins() {
        val response = callProbe("thread_probe", mapOf("probeIterations" to 100))
        assertProbeOk(response)
        val data = response.getJSONObject("data")
        assertEquals(100, data.getInt("iterations"))
        assertEquals(100, data.getInt("finalCount"))
    }

    // MARK: - Panic boundary

    @Test
    fun intentional_panic_is_contained() {
        val response = callProbe(
            "panic",
            mapOf("probeConfirmPanic" to "confirm-intentional-probe-panic")
        )
        assertFalse(response.getBoolean("ok"))
        assertEquals("internal error", response.getString("error"))

        val followUp = callProbe("runtime_info")
        assertProbeOk(followUp)
    }

    // MARK: - Lifecycle

    @Test
    fun process_can_load_library_and_run_on_background_thread() = runBlocking {
        withContext(Dispatchers.IO) {
            val response = callProbe("runtime_info")
            assertProbeOk(response)
        }
    }

    // MARK: - Helpers

    private fun callProbe(action: String, params: Map<String, Any> = emptyMap()): JSONObject {
        val envelope = JSONObject()
        envelope.put("operation", "mobile_runtime_probe")
        envelope.put("probeAction", action)
        params.forEach { (key, value) -> envelope.put(key, value) }
        val raw = FluxCoreBridge.request(envelope.toString())
        return JSONObject(raw)
    }

    private fun assertProbeOk(response: JSONObject) {
        assertTrue(
            response.optString("error", "(no error field)"),
            response.getBoolean("ok")
        )
    }
}
