import Foundation

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
        let encoded: Data
        do {
            encoded = try JSONEncoder().encode(request)
        } catch {
            throw GoCoreError.encoding(error)
        }
        guard let json = String(data: encoded, encoding: .utf8) else {
            throw GoCoreError.nullResponse
        }

        let responsePointer = json.withCString { pointer in
            FluxCoreRequest(UnsafeMutablePointer(mutating: pointer))
        }
        guard let responsePointer else {
            throw GoCoreError.nullResponse
        }
        defer { FluxCoreFree(responsePointer) }

        let data = Data(String(cString: responsePointer).utf8)
        let response: CoreResponse
        do {
            response = try JSONDecoder().decode(CoreResponse.self, from: data)
        } catch {
            throw GoCoreError.invalidResponse(error)
        }
        if !response.ok {
            throw GoCoreError.core(response.error ?? "Unknown Go core error")
        }
        return response
    }
}
