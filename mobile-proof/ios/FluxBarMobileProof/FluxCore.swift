//
//  FluxCore.swift
//  FluxBarMobileProof
//
//  Swift wrapper around the Rust C ABI. The core is loaded as a static
//  dependency via the packaged FluxCore.xcframework.
//

import Foundation
import FluxCore

enum FluxCoreError: Error, LocalizedError {
    case nullRequest
    case invalidResponse
    case requestFailed(String)

    var errorDescription: String? {
        switch self {
        case .nullRequest:
            return "Core rejected a null request"
        case .invalidResponse:
            return "Core returned a non-UTF-8 response"
        case .requestFailed(let message):
            return message
        }
    }
}

final class FluxCore {
    static let shared = FluxCore()

    private init() {}

    /// Sends a JSON request and returns the parsed JSON response.
    ///
    /// - Parameters:
    ///   - operation: Top-level operation name (e.g. `mobile_runtime_probe`).
    ///   - body: Additional fields merged into the request envelope.
    /// - Returns: Parsed JSON response object.
    func request(operation: String, body: [String: Any] = [:]) throws -> [String: Any] {
        var envelope = body
        envelope["operation"] = operation
        let data = try JSONSerialization.data(withJSONObject: envelope, options: [.sortedKeys])
        guard let json = String(data: data, encoding: .utf8) else {
            throw FluxCoreError.invalidResponse
        }
        return try request(json: json)
    }

    /// Sends a raw JSON string and returns the parsed JSON response.
    func request(json: String) throws -> [String: Any] {
        let responseString = try callFluxCoreRequest(json)
        guard let data = responseString.data(using: .utf8) else {
            throw FluxCoreError.invalidResponse
        }
        let object = try JSONSerialization.jsonObject(with: data, options: [])
        guard let dictionary = object as? [String: Any] else {
            throw FluxCoreError.requestFailed("Response is not a JSON object")
        }
        return dictionary
    }
}

/// Imported C ABI. Memory ownership follows the Rust contract: every returned
/// pointer must be released with `FluxCoreFree`.
private func callFluxCoreRequest(_ json: String) throws -> String {
    guard let cString = json.cString(using: .utf8) else {
        throw FluxCoreError.invalidResponse
    }
    return try cString.withUnsafeBufferPointer { buffer in
        let mutableCopy = strdup(buffer.baseAddress)
        defer { free(mutableCopy) }
        guard let responsePtr = FluxCoreRequest(mutableCopy) else {
            throw FluxCoreError.nullRequest
        }
        defer { FluxCoreFree(responsePtr) }
        guard let response = String(cString: responsePtr, encoding: .utf8) else {
            throw FluxCoreError.invalidResponse
        }
        return response
    }
}
