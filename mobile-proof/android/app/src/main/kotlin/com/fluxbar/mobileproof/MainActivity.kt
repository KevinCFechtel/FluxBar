package com.fluxbar.mobileproof

import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.widget.Button
import android.widget.TextView
import androidx.appcompat.app.AppCompatActivity
import androidx.lifecycle.lifecycleScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.json.JSONObject
import java.io.File

/**
 * Minimal status-oriented Activity for the runtime proof. It exists only to host
 * the Android lifecycle; the deterministic coverage lives in the instrumentation
 * tests. No Compose architecture, ViewModel, or persistent product state is used.
 */
class MainActivity : AppCompatActivity() {

    private lateinit var statusText: TextView
    private val mainHandler = Handler(Looper.getMainLooper())

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_main)

        statusText = findViewById(R.id.statusText)
        findViewById<Button>(R.id.runProbesButton).setOnClickListener {
            runProbes()
        }
        findViewById<Button>(R.id.resetProbeButton).setOnClickListener {
            resetProbeDatabase()
        }

        // Initialize the TLS verifier as early as possible. This is idempotent.
        if (!FluxCoreBridge.initVerifier(applicationContext)) {
            appendLine("ERROR: TLS verifier initialization failed")
        }
    }

    private fun runProbes() {
        lifecycleScope.launch {
            statusText.text = "Running probes…"
            val output = withContext(Dispatchers.IO) {
                buildString {
                    appendLine("=== runtime_info ===")
                    appendResponse(callProbe("runtime_info"))

                    appendLine("=== round_trip ===")
                    appendResponse(
                        callProbe(
                            "round_trip",
                            mapOf("probePayload" to "héllo Android 🌍", "probeSize" to 0)
                        )
                    )

                    appendLine("=== sqlite ===")
                    val probeDir = File(applicationContext.noBackupFilesDir, "proof")
                    probeDir.mkdirs()
                    appendResponse(
                        callProbe(
                            "sqlite_open",
                            mapOf("probeAllowedRoot" to probeDir.canonicalPath, "probeDbFilename" to "probe.db")
                        )
                    )
                    appendResponse(callProbe("sqlite_write", mapOf("probeKey" to "greeting", "probeValue" to "héllo Android")))
                    appendResponse(callProbe("sqlite_read", mapOf("probeKey" to "greeting")))
                    appendResponse(callProbe("sqlite_close"))

                    appendLine("=== https_get ===")
                    appendResponse(
                        callProbe(
                            "https_get",
                            mapOf("probeUrl" to "https://httpbin.org/get", "probeTimeoutMs" to 15000)
                        )
                    )

                    appendLine("=== thread_probe ===")
                    appendResponse(callProbe("thread_probe", mapOf("probeIterations" to 100)))

                    appendLine("=== panic ===")
                    appendResponse(
                        callProbe(
                            "panic",
                            mapOf("probeConfirmPanic" to "confirm-intentional-probe-panic")
                        )
                    )

                    appendLine("=== runtime_info after panic ===")
                    appendResponse(callProbe("runtime_info"))
                }
            }
            statusText.text = output
        }
    }

    private fun resetProbeDatabase() {
        val probeDir = File(applicationContext.noBackupFilesDir, "proof")
        probeDir.listFiles()?.forEach { it.delete() }
        statusText.text = "Probe database reset."
    }

    private fun StringBuilder.appendResponse(response: String) {
        appendLine(response)
        appendLine()
    }

    private fun callProbe(action: String, params: Map<String, Any> = emptyMap()): String {
        val envelope = JSONObject()
        envelope.put("operation", "mobile_runtime_probe")
        envelope.put("probeAction", action)
        params.forEach { (key, value) -> envelope.put(key, value) }
        return try {
            FluxCoreBridge.request(envelope.toString())
        } catch (e: Exception) {
            JSONObject().put("ok", false).put("error", e.message).toString()
        }
    }

    private fun appendLine(text: String) {
        mainHandler.post {
            statusText.append("$text\n")
        }
    }
}
