//
//  FluxBarMobileProofTests.swift
//  FluxBarMobileProofTests
//
//  XCTest coverage for the FluxBar Rust core on iOS.
//

import XCTest
@testable import FluxBarMobileProof

final class FluxBarMobileProofTests: XCTestCase {

    private var probeDir: URL!

    override func setUp() {
        super.setUp()
        let fm = FileManager.default
        let support = fm.urls(for: .applicationSupportDirectory, in: .userDomainMask).first!
        probeDir = support.appendingPathComponent("FluxBarMobileProofProbe", isDirectory: true)
        try? fm.createDirectory(at: probeDir, withIntermediateDirectories: true)
        var values = URLResourceValues()
        values.isExcludedFromBackup = true
        try? probeDir.setResourceValues(values)
    }

    // MARK: - FFI and invocation

    func test_runtime_info_on_ui_thread() throws {
        let response = try FluxCore.shared.request(operation: "mobile_runtime_probe", body: [
            "probeAction": "runtime_info"
        ])
        XCTAssertEqual(response["ok"] as? Bool, true)
        guard let data = response["data"] as? [String: Any] else {
            XCTFail("missing data")
            return
        }
        XCTAssertEqual(data["os"] as? String, "ios")
        XCTAssertEqual(data["mobileRuntimeProofEnabled"] as? Bool, true)
        XCTAssertEqual(data["panicStrategy"] as? String, "unwind")
        XCTAssertNotNil(data["arch"])
        XCTAssertNotNil(data["pointerWidth"])
    }

    func test_json_null_uses_defaults_and_returns_controlled_error() throws {
        // The Swift wrapper always passes a non-null C string; a JSON `null`
        // literal follows the existing Go-compatible default-request path.
        let response = try FluxCore.shared.request(json: "null")
        XCTAssertEqual(response["ok"] as? Bool, false)
        XCTAssertEqual(response["error"] as? String, "unsupported operation \"\"")
    }

    func test_malformed_json_returns_controlled_error() throws {
        let response = try FluxCore.shared.request(json: "{")
        XCTAssertEqual(response["ok"] as? Bool, false)
        let error = response["error"] as? String ?? ""
        XCTAssertTrue(error.contains("invalid request"), "unexpected error: \(error)")
    }

    func test_unknown_operation_returns_controlled_error() throws {
        let response = try FluxCore.shared.request(operation: "no_such_operation")
        XCTAssertEqual(response["ok"] as? Bool, false)
        let error = response["error"] as? String ?? ""
        XCTAssertTrue(error.contains("unsupported operation"), "unexpected error: \(error)")
    }

    // MARK: - Round-trip

    func test_round_trip_unicode_payload() throws {
        let payload = "héllo 🌍 \\n\\t\"quoted\""
        let response = try FluxCore.shared.request(operation: "mobile_runtime_probe", body: [
            "probeAction": "round_trip",
            "probePayload": payload
        ])
        XCTAssertEqual(response["ok"] as? Bool, true)
        let data = response["data"] as? [String: Any] ?? [:]
        XCTAssertEqual(data["receivedLength"] as? Int, payload.utf8.count)
        XCTAssertEqual(data["echoed"] as? String, payload)
    }

    func test_round_trip_concurrent_no_crossover() throws {
        let iterations = 8
        let expectations = (0..<iterations).map { index -> XCTestExpectation in
            let exp = expectation(description: "round-trip-\(index)")
            DispatchQueue.global(qos: .default).async {
                do {
                    let payload = "payload-\(index)"
                    let response = try FluxCore.shared.request(operation: "mobile_runtime_probe", body: [
                        "probeAction": "round_trip",
                        "probePayload": payload
                    ])
                    let data = response["data"] as? [String: Any] ?? [:]
                    if (data["echoed"] as? String) == payload {
                        exp.fulfill()
                    } else {
                        XCTFail("crossover at \(index)")
                    }
                } catch {
                    XCTFail("error at \(index): \(error)")
                }
            }
            return exp
        }
        wait(for: expectations, timeout: 30.0)
    }

    // MARK: - SQLite persistence

    func test_sqlite_open_write_read_close_reopen() throws {
        let open = try FluxCore.shared.request(operation: "mobile_runtime_probe", body: [
            "probeAction": "sqlite_open",
            "probeAllowedRoot": probeDir.path,
            "probeDbFilename": "probe.db"
        ])
        XCTAssertEqual(open["ok"] as? Bool, true, open["error"] as? String ?? "")
        let openData = open["data"] as? [String: Any] ?? [:]
        XCTAssertEqual(openData["journalModeAfter"] as? String, "wal")

        let write = try FluxCore.shared.request(operation: "mobile_runtime_probe", body: [
            "probeAction": "sqlite_write",
            "probeKey": "greeting",
            "probeValue": "héllo iOS"
        ])
        XCTAssertEqual(write["ok"] as? Bool, true, write["error"] as? String ?? "")

        let read = try FluxCore.shared.request(operation: "mobile_runtime_probe", body: [
            "probeAction": "sqlite_read",
            "probeKey": "greeting"
        ])
        XCTAssertEqual(read["ok"] as? Bool, true, read["error"] as? String ?? "")
        let readData = read["data"] as? [String: Any] ?? [:]
        XCTAssertEqual(readData["value"] as? String, "héllo iOS")

        let close = try FluxCore.shared.request(operation: "mobile_runtime_probe", body: [
            "probeAction": "sqlite_close"
        ])
        XCTAssertEqual(close["ok"] as? Bool, true, close["error"] as? String ?? "")

        let reopen = try FluxCore.shared.request(operation: "mobile_runtime_probe", body: [
            "probeAction": "sqlite_open",
            "probeAllowedRoot": probeDir.path,
            "probeDbFilename": "probe.db"
        ])
        XCTAssertEqual(reopen["ok"] as? Bool, true, reopen["error"] as? String ?? "")

        let reread = try FluxCore.shared.request(operation: "mobile_runtime_probe", body: [
            "probeAction": "sqlite_read",
            "probeKey": "greeting"
        ])
        XCTAssertEqual(reread["ok"] as? Bool, true, reread["error"] as? String ?? "")
        let rereadData = reread["data"] as? [String: Any] ?? [:]
        XCTAssertEqual(rereadData["value"] as? String, "héllo iOS")
    }

    func test_sqlite_path_containment_rejects_escape() throws {
        let response = try FluxCore.shared.request(operation: "mobile_runtime_probe", body: [
            "probeAction": "sqlite_open",
            "probeAllowedRoot": probeDir.path,
            "probeDbFilename": "../escape.db"
        ])
        XCTAssertEqual(response["ok"] as? Bool, false)
        let error = response["error"] as? String ?? ""
        XCTAssertTrue(error.contains("single path component"), "unexpected error: \(error)")
    }

    // MARK: - HTTPS/TLS

    func test_https_public_root_succeeds() throws {
        let response = try FluxCore.shared.request(operation: "mobile_runtime_probe", body: [
            "probeAction": "https_get",
            "probeUrl": "https://httpbin.org/get",
            "probeTimeoutMs": 15000
        ])
        XCTAssertEqual(response["ok"] as? Bool, true, response["error"] as? String ?? "")
        let data = response["data"] as? [String: Any] ?? [:]
        XCTAssertEqual(data["status"] as? Int, 200)
        XCTAssertNotNil(data["bodyDigest"])
    }

    func test_https_invalid_certificate_fails() throws {
        let response = try FluxCore.shared.request(operation: "mobile_runtime_probe", body: [
            "probeAction": "https_get",
            "probeUrl": "https://self-signed.badssl.com/",
            "probeTimeoutMs": 15000
        ])
        XCTAssertEqual(response["ok"] as? Bool, false)
        let data = response["data"] as? [String: Any] ?? [:]
        let category = data["category"] as? String ?? ""
        XCTAssertTrue(category == "transport" || category == "connection", "category: \(category)")
    }

    // MARK: - Threading

    func test_thread_probe_spawns_and_joins() throws {
        let response = try FluxCore.shared.request(operation: "mobile_runtime_probe", body: [
            "probeAction": "thread_probe",
            "probeIterations": 100
        ])
        XCTAssertEqual(response["ok"] as? Bool, true, response["error"] as? String ?? "")
        let data = response["data"] as? [String: Any] ?? [:]
        XCTAssertEqual(data["iterations"] as? Int, 100)
        XCTAssertEqual(data["finalCount"] as? Int, 100)
    }

    // MARK: - Panic boundary

    func test_intentional_panic_is_contained() throws {
        let response = try FluxCore.shared.request(operation: "mobile_runtime_probe", body: [
            "probeAction": "panic",
            "probeConfirmPanic": "confirm-intentional-probe-panic"
        ])
        XCTAssertEqual(response["ok"] as? Bool, false)
        XCTAssertEqual(response["error"] as? String, "internal error")

        // Core must remain usable after a contained panic.
        let followUp = try FluxCore.shared.request(operation: "mobile_runtime_probe", body: [
            "probeAction": "runtime_info"
        ])
        XCTAssertEqual(followUp["ok"] as? Bool, true)
    }
}
