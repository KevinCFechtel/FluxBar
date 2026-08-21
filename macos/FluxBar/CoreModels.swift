import Foundation

struct CoreRequest: Encodable, Sendable {
    let operation: String
    var server: String? = nil
    var apiKey: String? = nil
    var newestFirst: Bool? = nil
    var configurationGeneration: Int64? = nil
    var locales: [String]? = nil
    var key: String? = nil
    var fallback: String? = nil
    var oneFallback: String? = nil
    var otherFallback: String? = nil
    var count: Int? = nil
    var selection: ArticleSelection? = nil
    var entryID: Int64? = nil
    var entryIDs: [Int64]? = nil
    var retainEntryIDs: [Int64]? = nil
    var read: Bool? = nil
    var mutationSource: String? = nil
    var mutationID: String? = nil
    var currentStarred: Bool? = nil
    var desiredStarred: Bool? = nil
    var feedID: Int64? = nil
    var feedName: String? = nil
}

struct CoreResponse: Decodable, Sendable {
    let ok: Bool
    let error: String?
    let text: String?
    let snapshot: BrowseSnapshot?
    let icon: FeedIconPayload?
    let receipt: MutationReceipt?
}

struct MutationReceipt: Decodable, Sendable {
    let id: String
    let count: Int
}

struct ArticleSelection: Codable, Hashable, Sendable {
    let kind: String
    let id: Int64?
    let unreadOnly: Bool?

    static let all = ArticleSelection(kind: "all", id: nil, unreadOnly: true)
    static let unread = ArticleSelection(kind: "unread", id: nil, unreadOnly: true)
    static let starred = ArticleSelection(kind: "starred", id: nil, unreadOnly: false)

    static func all(unreadOnly: Bool) -> ArticleSelection {
        ArticleSelection(kind: "all", id: nil, unreadOnly: unreadOnly)
    }

    static func category(_ id: Int64, unreadOnly: Bool = true) -> ArticleSelection {
        ArticleSelection(kind: "category", id: id, unreadOnly: unreadOnly)
    }

    static func feed(_ id: Int64, unreadOnly: Bool = true) -> ArticleSelection {
        ArticleSelection(kind: "feed", id: id, unreadOnly: unreadOnly)
    }

    func matchesRoute(_ other: ArticleSelection) -> Bool {
        kind == other.kind && id == other.id
    }

    var navigationRoute: NavigationRoute? {
        switch kind {
        case "all", "unread": return .all
        case "starred": return .starred
        case "category": return id.map(NavigationRoute.category)
        case "feed": return id.map(NavigationRoute.feed)
        default: return nil
        }
    }
}

enum NavigationRoute: Hashable, Sendable {
    case all
    case starred
    case category(Int64)
    case feed(Int64)
    case article(id: Int64, url: String?)

    var selection: ArticleSelection? {
        switch self {
        case .all: return .all
        case .starred: return .starred
        case .category(let id): return .category(id)
        case .feed(let id): return .feed(id)
        case .article: return nil
        }
    }

    init?(searchableIdentifier: String) {
        let parts = searchableIdentifier.split(separator: ":", maxSplits: 1)
        guard parts.count == 2, let id = Int64(parts[1]) else { return nil }
        switch parts[0] {
        case "feed": self = .feed(id)
        case "category": self = .category(id)
        default: return nil
        }
    }
}

struct NavigationCatalog: Codable, Equatable, Sendable {
    struct CategoryDestination: Codable, Equatable, Identifiable, Sendable {
        let id: Int64
        let title: String
    }

    struct FeedDestination: Codable, Equatable, Identifiable, Sendable {
        let id: Int64
        let title: String
        let categoryID: Int64
        let categoryTitle: String
    }

    let categories: [CategoryDestination]
    let feeds: [FeedDestination]

    static let empty = NavigationCatalog(categories: [], feeds: [])

    init(snapshot: BrowseSnapshot) {
        categories = snapshot.categories.map { .init(id: $0.id, title: $0.title) }
        feeds = snapshot.categories.flatMap { category in
            category.feeds.map {
                .init(id: $0.id, title: $0.title, categoryID: category.id, categoryTitle: category.title)
            }
        }
    }

    init(categories: [CategoryDestination], feeds: [FeedDestination]) {
        self.categories = categories
        self.feeds = feeds
    }
}

struct BrowseSnapshot: Decodable, Sendable {
    let version: Int
    let selection: ArticleSelection
    let entries: [Article]
    let categories: [FeedCategory]
    let total: Int
    let unreadTotal: Int
    let starredTotal: Int

    static let empty = BrowseSnapshot(
        version: 1,
        selection: .all,
        entries: [],
        categories: [],
        total: 0,
        unreadTotal: 0,
        starredTotal: 0
    )

    func replacing(_ article: Article, unreadDelta: Int = 0, starredDelta: Int = 0) -> BrowseSnapshot {
        let visibleTotalDelta = selection.unreadOnly == true ? unreadDelta : 0
        return BrowseSnapshot(
            version: version,
            selection: selection,
            entries: entries.map { $0.id == article.id ? article : $0 },
            categories: categories.map { $0.adjustingUnread(feedID: article.feedID, by: unreadDelta) },
            total: max(0, total + visibleTotalDelta),
            unreadTotal: max(0, unreadTotal + unreadDelta),
            starredTotal: max(0, starredTotal + starredDelta)
        )
    }
}

struct Article: Decodable, Identifiable, Sendable {
    let id: Int64
    let title: String
    let url: String
    let commentsURL: String?
    let feedID: Int64
    let feedName: String
    let categoryID: Int64?
    let publishedAt: String
    let preview: String
    let imageURL: String?
    let status: String
    let starred: Bool

    var isRead: Bool { status == "read" }

    func replacing(status: String? = nil, starred: Bool? = nil) -> Article {
        Article(
            id: id,
            title: title,
            url: url,
            commentsURL: commentsURL,
            feedID: feedID,
            feedName: feedName,
            categoryID: categoryID,
            publishedAt: publishedAt,
            preview: preview,
            imageURL: imageURL,
            status: status ?? self.status,
            starred: starred ?? self.starred
        )
    }
}

struct FeedCategory: Decodable, Identifiable, Sendable {
    let id: Int64
    let title: String
    let unreadCount: Int
    let feeds: [Feed]

    func adjustingUnread(feedID: Int64, by delta: Int) -> FeedCategory {
        guard delta != 0, feeds.contains(where: { $0.id == feedID }) else { return self }
        return FeedCategory(
            id: id,
            title: title,
            unreadCount: max(0, unreadCount + delta),
            feeds: feeds.map { $0.adjustingUnread(feedID: feedID, by: delta) }
        )
    }
}

struct Feed: Decodable, Identifiable, Sendable {
    let id: Int64
    let title: String
    let categoryID: Int64
    let unreadCount: Int

    func adjustingUnread(feedID: Int64, by delta: Int) -> Feed {
        guard id == feedID else { return self }
        return Feed(id: id, title: title, categoryID: categoryID, unreadCount: max(0, unreadCount + delta))
    }
}

struct FeedIconPayload: Decodable, Sendable {
    let regular: Data?
    let dark: Data?
}
