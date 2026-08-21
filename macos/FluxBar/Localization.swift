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

    func plural(_ key: String, one: String, other: String, count: Int) -> String {
        let cacheKey = "\(key).\(count)"
        if let cached = cache[cacheKey] {
            return cached
        }
        let request = CoreRequest(
            operation: "localize_plural",
            locales: locales,
            key: key,
            oneFallback: one,
            otherFallback: other,
            count: count
        )
        let fallback = count == 1
            ? one.replacingOccurrences(of: "{{.Count}}", with: "\(count)")
            : other.replacingOccurrences(of: "{{.Count}}", with: "\(count)")
        let value = (try? GoCore.request(request).text) ?? fallback
        cache[cacheKey] = value
        return value
    }
}
