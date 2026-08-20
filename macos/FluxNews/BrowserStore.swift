import AppKit
import Combine
import Foundation

@MainActor
final class BrowserStore: ObservableObject {
    @Published private(set) var snapshot = BrowseSnapshot.empty
    @Published private(set) var selection = ArticleSelection.all
    @Published private(set) var isLoading = false
    @Published var errorMessage: String?
    @Published var showingSettings = false

    private(set) var credentials: MinifluxCredentials?
    private var generation = 0
    @Published private var pendingEntries: Set<Int64> = []
    private let defaults = UserDefaults.standard
    private var sharingPicker: NSSharingServicePicker?

    func start() {
        do {
            selection = selectionWithStoredFilter(.all)
            guard let credentials = try CredentialStore.load() else {
                showingSettings = true
                return
            }
            self.credentials = credentials
            configure(credentials)
        } catch {
            errorMessage = error.localizedDescription
            showingSettings = true
        }
    }

    func save(credentials: MinifluxCredentials, launchAtLogin: Bool) {
        isLoading = true
        generation += 1
        let requestGeneration = generation
        Task {
            do {
                let request = CoreRequest(
                    operation: "configure",
                    server: credentials.server,
                    apiKey: credentials.apiKey,
                    newestFirst: credentials.newestFirst ?? false,
                    configurationGeneration: Int64(requestGeneration),
                    locales: Locale.preferredLanguages
                )
                _ = try await requestOffMain(request)
                guard requestGeneration == generation else { return }
                try CredentialStore.setLaunchAtLogin(launchAtLogin)
                try CredentialStore.save(credentials)
                self.credentials = credentials
                showingSettings = false
                let response = try await requestOffMain(CoreRequest(operation: "refresh", selection: selection))
                guard requestGeneration == generation else { return }
                apply(response)
            } catch {
                guard requestGeneration == generation else { return }
                errorMessage = error.localizedDescription
                isLoading = false
            }
        }
    }

    func refresh() {
        load(CoreRequest(operation: "refresh", selection: selection))
    }

    func select(_ selection: ArticleSelection) {
        let resolved = selectionWithStoredFilter(selection)
        self.selection = resolved
        load(CoreRequest(operation: "refresh", selection: resolved))
    }

    var supportsUnreadFilter: Bool {
        selection.kind == "all" || selection.kind == "category" || selection.kind == "feed"
    }

    var showsUnreadOnly: Bool {
        selection.unreadOnly ?? true
    }

    func setUnreadOnly(_ unreadOnly: Bool) {
        guard supportsUnreadFilter else { return }
        defaults.set(unreadOnly, forKey: unreadPreferenceKey(selection))
        let updated: ArticleSelection
        if selection.kind == "category", let id = selection.id {
            updated = .category(id, unreadOnly: unreadOnly)
        } else if selection.kind == "feed", let id = selection.id {
            updated = .feed(id, unreadOnly: unreadOnly)
        } else {
            updated = .all(unreadOnly: unreadOnly)
        }
        selection = updated
        load(CoreRequest(operation: "refresh", selection: updated))
    }

    var newestFirst: Bool {
        credentials?.newestFirst ?? false
    }

    func setNewestFirst(_ newestFirst: Bool) {
        guard var credentials else { return }
        credentials.newestFirst = newestFirst
        save(credentials: credentials, launchAtLogin: CredentialStore.launchAtLoginEnabled)
    }

    func open(_ article: Article) {
        guard let url = URL(string: article.url) else { return }
        NSWorkspace.shared.open(url)
        if !article.isRead {
            setRead(article, read: true)
        }
    }

    func openComments(_ article: Article) {
        guard let value = article.commentsURL, let url = URL(string: value) else { return }
        NSWorkspace.shared.open(url)
    }

    func copyLink(_ article: Article) {
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(article.url, forType: .string)
    }

    func share(_ article: Article) {
        guard let url = URL(string: article.url) else { return }
        DispatchQueue.main.async { [weak self] in
            guard let self, let view = NSApplication.shared.keyWindow?.contentView else { return }
            let picker = NSSharingServicePicker(items: [url])
            sharingPicker = picker
            let point = view.convert(view.window?.mouseLocationOutsideOfEventStream ?? .zero, from: nil)
            picker.show(relativeTo: NSRect(origin: point, size: NSSize(width: 1, height: 1)), of: view, preferredEdge: .minY)
        }
    }

    func setRead(_ article: Article, read: Bool) {
        let updated = article.replacing(status: read ? "read" : "unread")
        mutate(articleID: article.id, optimisticSnapshot: snapshot.replacing(updated, unreadDelta: read ? -1 : 1), request: CoreRequest(
            operation: "set_read",
            selection: selection,
            entryID: article.id,
            read: read
        ))
    }

    func setStarred(_ article: Article, starred: Bool) {
        let updated = article.replacing(starred: starred)
        mutate(articleID: article.id, optimisticSnapshot: snapshot.replacing(updated, starredDelta: starred ? 1 : -1), request: CoreRequest(
            operation: "set_starred",
            selection: selection,
            entryID: article.id,
            currentStarred: article.starred,
            desiredStarred: starred
        ))
    }

    func showFeed(_ article: Article) {
        select(.feed(article.feedID))
    }

    func isPending(_ articleID: Int64) -> Bool {
        pendingEntries.contains(articleID)
    }

    func prepareUISmokeTest() {
        let category = FeedCategory(
            id: 1,
            title: "Technology",
            unreadCount: 1,
            feeds: [Feed(id: 2, title: "Example Feed", categoryID: 1, unreadCount: 1)]
        )
        snapshot = BrowseSnapshot(
            version: 1,
            selection: .all,
            entries: [Article(
                id: 3,
                title: "Example article",
                url: "https://example.com/article",
                commentsURL: "https://example.com/comments",
                feedID: 2,
                feedName: "Example Feed",
                categoryID: 1,
                publishedAt: "2026-08-20T12:00:00Z",
                preview: "A compact article teaser used to validate native row rendering.",
                imageURL: nil,
                status: "unread",
                starred: false
            )],
            categories: [category],
            total: 1,
            unreadTotal: 1,
            starredTotal: 0
        )
        selection = .all
    }

    private func configure(_ credentials: MinifluxCredentials) {
        isLoading = true
        generation += 1
        let requestGeneration = generation
        Task {
            do {
                let request = CoreRequest(
                    operation: "configure",
                    server: credentials.server,
                    apiKey: credentials.apiKey,
                    newestFirst: credentials.newestFirst ?? false,
                    configurationGeneration: Int64(requestGeneration),
                    locales: Locale.preferredLanguages
                )
                _ = try await requestOffMain(request)
                guard requestGeneration == generation else { return }
                let response = try await requestOffMain(CoreRequest(operation: "refresh", selection: selection))
                guard requestGeneration == generation else { return }
                apply(response)
            } catch {
                guard requestGeneration == generation else { return }
                errorMessage = error.localizedDescription
                isLoading = false
            }
        }
    }

    private func load(_ request: CoreRequest) {
        isLoading = true
        generation += 1
        let requestGeneration = generation
        Task {
            do {
                let response = try await requestOffMain(request)
                guard requestGeneration == generation else { return }
                apply(response)
            } catch {
                guard requestGeneration == generation else { return }
                errorMessage = error.localizedDescription
                isLoading = false
            }
        }
    }

    private func mutate(articleID: Int64, optimisticSnapshot: BrowseSnapshot, request: CoreRequest) {
        guard !pendingEntries.contains(articleID) else { return }
        let previousSnapshot = snapshot
        snapshot = optimisticSnapshot
        pendingEntries.insert(articleID)
        generation += 1
        let requestGeneration = generation
        Task {
            defer { pendingEntries.remove(articleID) }
            do {
                _ = try await requestOffMain(request)
                guard requestGeneration == generation else { return }
                errorMessage = nil
            } catch {
                guard requestGeneration == generation else { return }
                snapshot = previousSnapshot
                errorMessage = error.localizedDescription
            }
        }
    }

    private func requestOffMain(_ request: CoreRequest) async throws -> CoreResponse {
        try await Task.detached(priority: .userInitiated) {
            try GoCore.request(request)
        }.value
    }

    private func selectionWithStoredFilter(_ selection: ArticleSelection) -> ArticleSelection {
        guard selection.kind == "all" || selection.kind == "category" || selection.kind == "feed" else { return selection }
        let key = unreadPreferenceKey(selection)
        let unreadOnly = defaults.object(forKey: key) as? Bool ?? true
        if selection.kind == "category", let id = selection.id {
            return .category(id, unreadOnly: unreadOnly)
        }
        if selection.kind == "feed", let id = selection.id {
            return .feed(id, unreadOnly: unreadOnly)
        }
        return .all(unreadOnly: unreadOnly)
    }

    private func unreadPreferenceKey(_ selection: ArticleSelection) -> String {
        if selection.kind == "category", let id = selection.id {
            return "FluxNews.unreadOnly.category.\(id)"
        }
        if selection.kind == "feed", let id = selection.id {
            return "FluxNews.unreadOnly.feed.\(id)"
        }
        return "FluxNews.unreadOnly.all"
    }

    private func apply(_ response: CoreResponse) {
        if let snapshot = response.snapshot {
            self.snapshot = snapshot
            selection = snapshot.selection
        }
        errorMessage = nil
        isLoading = false
    }
}
