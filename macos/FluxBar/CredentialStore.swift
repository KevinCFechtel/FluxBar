import Foundation
import Security
import ServiceManagement

struct MinifluxCredentials: Codable, Sendable {
    var server: String
    var apiKey: String
    var showSplash: Bool?
    var newestFirst: Bool?
}

enum CredentialStore {
    private static let service = "dev.kevincfechtel.FluxBar.miniflux"
    private static let legacyService = "com.kevinfechtel.FluxBar.miniflux"
    private static let account = "credentials"

    static func load() throws -> MinifluxCredentials? {
        if let credentials = try load(service: service) {
            return credentials
        }
        return try load(service: legacyService)
    }

    static func save(_ credentials: MinifluxCredentials) throws {
        let data = try JSONEncoder().encode(credentials)
        let json = String(decoding: data, as: UTF8.self)
        guard let stored = json.data(using: .utf8) else { throw GoCoreError.nullResponse }

        let key: [CFString: Any] = [
            kSecClass: kSecClassGenericPassword,
            kSecAttrService: service,
            kSecAttrAccount: account,
        ]
        let updateStatus = SecItemUpdate(
            key as CFDictionary,
            [kSecValueData: stored] as CFDictionary
        )
        if updateStatus == errSecSuccess { return }
        if updateStatus != errSecItemNotFound { throw statusError(updateStatus) }

        var insertion = key
        insertion[kSecValueData] = stored
        insertion[kSecAttrAccessible] = kSecAttrAccessibleWhenUnlocked
        let addStatus = SecItemAdd(insertion as CFDictionary, nil)
        if addStatus != errSecSuccess { throw statusError(addStatus) }
    }

    static var launchAtLoginEnabled: Bool {
        SMAppService.mainApp.status == .enabled
    }

    static func setLaunchAtLogin(_ enabled: Bool) throws {
        if enabled {
            if SMAppService.mainApp.status != .enabled {
                try SMAppService.mainApp.register()
            }
        } else if SMAppService.mainApp.status == .enabled {
            try SMAppService.mainApp.unregister()
        }
    }

    private static func load(service: String) throws -> MinifluxCredentials? {
        let query: [CFString: Any] = [
            kSecClass: kSecClassGenericPassword,
            kSecAttrService: service,
            kSecAttrAccount: account,
            kSecReturnData: true,
            kSecMatchLimit: kSecMatchLimitOne,
        ]
        var result: CFTypeRef?
        let status = SecItemCopyMatching(query as CFDictionary, &result)
        if status == errSecItemNotFound { return nil }
        if status != errSecSuccess { throw statusError(status) }
        guard let data = result as? Data,
              let jsonData = String(data: data, encoding: .utf8)?.data(using: .utf8)
        else { throw statusError(errSecDecode) }
        return try JSONDecoder().decode(MinifluxCredentials.self, from: jsonData)
    }

    private static func statusError(_ status: OSStatus) -> NSError {
        NSError(
            domain: NSOSStatusErrorDomain,
            code: Int(status),
            userInfo: [NSLocalizedDescriptionKey: SecCopyErrorMessageString(status, nil) as String? ?? "Keychain error \(status)"]
        )
    }
}
