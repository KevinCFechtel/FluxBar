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
    var selection: ArticleSelection? = nil
    var entryID: Int64? = nil
    var read: Bool? = nil
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
