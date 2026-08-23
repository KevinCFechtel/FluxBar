import Foundation
import OSLog

extension Logger {
    static let fluxBar = Logger(subsystem: "dev.kevincfechtel.FluxBar", category: "ui")
}

enum GoCoreError: LocalizedError {
    case encoding(Error)
    case nullResponse
    case invalidResponse(Error)
    case core(String)

    var errorDescription: String? {
        switch self {
        case .encoding(let error), .invalidResponse(let error):
            return error.localizedDescription
        case .nullResponse:
            return "The Go core returned no response."
        case .core(let message):
            return message
        }
    }
}

enum GoCore {
    static func request(_ request: CoreRequest) throws -> CoreResponse {
        let operation = request.operation
        Logger.fluxBar.info("core request started operation=\(operation)")
        let encoded: Data
        do {
            encoded = try JSONEncoder().encode(request)
        } catch {
            Logger.fluxBar.error("core request encoding failed operation=\(operation): \(error.localizedDescription)")
            throw GoCoreError.encoding(error)
        }
        guard let json = String(data: encoded, encoding: .utf8) else {
            Logger.fluxBar.error("core request produced invalid UTF-8 operation=\(operation)")
            throw GoCoreError.nullResponse
        }

        let responsePointer = json.withCString { pointer in
            FluxCoreRequest(UnsafeMutablePointer(mutating: pointer))
        }
        guard let responsePointer else {
            Logger.fluxBar.error("core request returned null pointer operation=\(operation)")
            throw GoCoreError.nullResponse
        }
        defer { FluxCoreFree(responsePointer) }

        let data = Data(String(cString: responsePointer).utf8)
        let response: CoreResponse
        do {
            response = try JSONDecoder().decode(CoreResponse.self, from: data)
        } catch {
            Logger.fluxBar.error("core response decode failed operation=\(operation): \(error.localizedDescription)")
            throw GoCoreError.invalidResponse(error)
        }
        if !response.ok {
            Logger.fluxBar.warning("core request failed operation=\(operation): \(response.error ?? "unknown")")
            throw GoCoreError.core(response.error ?? "Unknown Go core error")
        }
        Logger.fluxBar.info("core request completed operation=\(operation)")
        return response
    }
}
