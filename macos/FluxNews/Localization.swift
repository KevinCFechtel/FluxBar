import Combine
import Foundation

@MainActor
final class Localization: ObservableObject {
    private let locales = Locale.preferredLanguages
    private var cache: [String: String] = [:]

    func text(_ key: String, _ fallback: String) -> String {
        if let cached = cache[key] {
            return cached
        }
        let request = CoreRequest(
            operation: "localize",
            locales: locales,
            key: key,
            fallback: fallback
        )
        let value = (try? GoCore.request(request).text) ?? fallback
        cache[key] = value
        return value
    }
}
