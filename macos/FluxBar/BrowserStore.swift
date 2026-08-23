import AppKit
import Combine
import Foundation
import OSLog

struct AutomaticReadUndo: Equatable {
    let id: String
    let count: Int
}

struct ManualRefreshScrollRequest: Equatable {
    let id: UInt64
    let presentationRevision: UInt64
    let firstEntryID: Int64?
}

enum ArticleListStyle: String {
    case row
    case card
}

@MainActor
final class BrowserStore: ObservableObject {
    private enum RefreshOrigin {
        case manual
        case startup
        case background
    }

    private static let automaticReadUndoMinimumCount = 3
    private static let articleListStyleKey = "FluxBar.articleListStyle"

    @Published private(set) var snapshot = BrowseSnapshot.empty
    @Published private(set) var selection = ArticleSelection.all
    @Published private(set) var isLoading = false
    @Published private(set) var listPresentationRevision: UInt64 = 0
    @Published private(set) var manualRefreshScrollRequest: ManualRefreshScrollRequest?
    @Published private(set) var automaticReadUndo: AutomaticReadUndo?
    @Published private(set) var markReadOnScrolloverEnabled: Bool
    @Published private(set) var articleListStyle: ArticleListStyle
    @Published private(set) var isPopoverVisible = false
    @Published private(set) var isNavigating = false
    @Published private(set) var globalShortcut: GlobalShortcutChoice
    @Published private(set) var globalShortcutRegistrationError: String?
    @Published var errorMessage: String?
    @Published var showingSettings = false

    private(set) var credentials: MinifluxCredentials?
    private var configurationGeneration = 0
    private var routeGeneration = 0
    private var mutationGeneration = 0
    private var refreshGeneration = 0
    private var manualRefreshScrollRequestID: UInt64 = 0
    private var retainedEntryIDs: Set<Int64> = []
    private var lastRefresh: Date?
    private var undoExpirationTask: Task<Void, Never>?
    private var backgroundSync: AnyCancellable?
    private let defaults: UserDefaults
    private var sharingPicker: NSSharingServicePicker?
    private var coreConfigured = false
    private var pendingSelection: ArticleSelection?
    private var refreshWhenConfigured = false

    init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
        markReadOnScrolloverEnabled = ScrolloverPreferences.isEnabled(in: defaults)
        articleListStyle = defaults.string(forKey: Self.articleListStyleKey)
            .flatMap(ArticleListStyle.init(rawValue:)) ?? .row
        globalShortcut = GlobalShortcutChoice.stored(in: defaults)
        globalShortcutRegistrationError = nil
    }

    func start() {
        Logger.fluxBar.info("store starting")
        do {
            selection = selectionWithStoredFilter(.all)
            guard let credentials = try CredentialStore.load() else {
                Logger.fluxBar.info("no credentials stored; opening settings")
                showingSettings = true
                return
            }
            self.credentials = credentials
            configure(credentials)
        } catch {
            Logger.fluxBar.error("store startup failed: \(error.localizedDescription)")
            errorMessage = error.localizedDescription
            showingSettings = true
        }
    }

    func save(credentials: MinifluxCredentials, launchAtLogin: Bool) {
        Logger.fluxBar.info("configure requested")
        resetListPresentation()
        retainedEntryIDs.removeAll()
        coreConfigured = false
        configurationGeneration += 1
        let configuration = configurationGeneration
        isLoading = true
        Task {
            do {
                try await configureCore(credentials, generation: configuration)
                guard configuration == configurationGeneration else { return }
                Logger.fluxBar.info("configure completed")
                try CredentialStore.setLaunchAtLogin(launchAtLogin)
                try CredentialStore.save(credentials)
                self.credentials = credentials
                coreConfigured = true
                showingSettings = false
                startBackgroundSync()
                resumeAfterConfiguration()
            } catch {
                guard configuration == configurationGeneration else { return }
                Logger.fluxBar.error("configure failed: \(error.localizedDescription)")
                errorMessage = error.localizedDescription
                isLoading = false
            }
        }
    }

    func refresh() {
        Logger.fluxBar.info("manual refresh requested")
        guard coreConfigured else {
            refreshWhenConfigured = true
            return
        }
        mutationGeneration += 1
        retainedEntryIDs.removeAll()
        refresh(selection: selection, route: routeGeneration, retainEntryIDs: [], origin: .manual)
    }

    func refreshIfStale() {
        resetListPresentation()
        guard credentials != nil, !isPopoverVisible else { return }
        if let lastRefresh, Date().timeIntervalSince(lastRefresh) <= 60 { return }
        Logger.fluxBar.info("background refresh requested")
        refreshInBackground()
    }

    func setPopoverVisible(_ visible: Bool) {
        isPopoverVisible = visible
    }

    func setMarkReadOnScrolloverEnabled(_ enabled: Bool) {
        guard enabled != markReadOnScrolloverEnabled else { return }
        markReadOnScrolloverEnabled = enabled
        ScrolloverPreferences.setEnabled(enabled, in: defaults)
        resetListPresentation()
        if !enabled, let undo = automaticReadUndo {
            automaticReadUndo = nil
            undoExpirationTask?.cancel()
            discardUndo(undo.id)
        }
    }

    func setArticleListStyle(_ style: ArticleListStyle) {
        guard style != articleListStyle else { return }
        articleListStyle = style
        defaults.set(style.rawValue, forKey: Self.articleListStyleKey)
        resetListPresentation()
    }

    func setGlobalShortcut(_ shortcut: GlobalShortcutChoice) {
        guard shortcut != globalShortcut else { return }
        globalShortcut = shortcut
        shortcut.store(in: defaults)
    }

    func setGlobalShortcutRegistrationError(_ message: String?) {
        globalShortcutRegistrationError = message
    }

    func route(to route: NavigationRoute) {
        if case let .article(id, url) = route {
            if let article = snapshot.entries.first(where: { $0.id == id }) {
                open(article)
            } else if let url, let destination = URL(string: url) {
                NSWorkspace.shared.open(destination)
            }
            return
        }
        guard let selection = route.selection else { return }
        let resolved = selectionWithStoredFilter(selection)
        resetListPresentation()
        retainedEntryIDs.removeAll()
        self.selection = resolved
        isNavigating = true
        guard coreConfigured else {
            pendingSelection = resolved
            return
        }
        navigate(to: resolved, resetPresentation: false)
    }

    func resetListPresentation() {
        listPresentationRevision &+= 1
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
        navigate(to: updated)
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
        if read {
            retainedEntryIDs.insert(article.id)
        } else {
            retainedEntryIDs.remove(article.id)
        }
        mutate(CoreRequest(
            operation: "set_read",
            selection: selection,
            entryIDs: [article.id],
            retainEntryIDs: Array(retainedEntryIDs),
            read: read,
            mutationSource: "manual"
        ))
    }

    func markReadAutomatically(_ entryIDs: [Int64]) {
        guard markReadOnScrolloverEnabled else { return }
        let unreadIDs = Set(snapshot.entries.filter { !$0.isRead }.map(\.id))
        let ids = Array(Set(entryIDs).intersection(unreadIDs))
        guard !ids.isEmpty else { return }
        retainedEntryIDs.formUnion(ids)
        mutationGeneration += 1
        let mutation = mutationGeneration
        let route = routeGeneration
        Task {
            do {
                let response = try await requestOffMain(CoreRequest(
                    operation: "set_read",
                    selection: selection,
                    entryIDs: ids,
                    retainEntryIDs: Array(retainedEntryIDs),
                    read: true,
                    mutationSource: "automatic"
                ))
                guard route == routeGeneration, mutation == mutationGeneration else {
                    if let receipt = response.receipt { discardUndo(receipt.id) }
                    if route == routeGeneration { reloadLocal(route: route) }
                    return
                }
                apply(response, structural: false)
                if let receipt = response.receipt {
                    if receipt.count >= Self.automaticReadUndoMinimumCount {
                        showUndo(receipt)
                    } else {
                        discardUndo(receipt.id)
                    }
                }
            } catch {
                guard route == routeGeneration, mutation == mutationGeneration else { return }
                errorMessage = error.localizedDescription
            }
        }
    }

    func undoAutomaticRead() {
        guard let undo = automaticReadUndo else { return }
        automaticReadUndo = nil
        undoExpirationTask?.cancel()
        mutationGeneration += 1
        let mutation = mutationGeneration
        let route = routeGeneration
        Task {
            do {
                let response = try await requestOffMain(CoreRequest(
                    operation: "undo_read",
                    selection: selection,
                    retainEntryIDs: Array(retainedEntryIDs),
                    mutationID: undo.id
                ))
                guard route == routeGeneration, mutation == mutationGeneration else { return }
                apply(response, structural: false)
            } catch {
                guard route == routeGeneration, mutation == mutationGeneration else { return }
                errorMessage = error.localizedDescription
            }
        }
    }

    func setStarred(_ article: Article, starred: Bool) {
        mutate(CoreRequest(
            operation: "set_starred",
            selection: selection,
            entryID: article.id,
            retainEntryIDs: Array(retainedEntryIDs),
            desiredStarred: starred
        ))
    }

    func showFeed(_ article: Article) {
        route(to: .feed(article.feedID))
    }

    func isPending(_ articleID: Int64) -> Bool {
        false
    }

    func prepareUISmokeTest() {
        articleListStyle = .row
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

    func prepareUICardSmokeTest() {
        articleListStyle = .card
        resetListPresentation()
    }

    private func configure(_ credentials: MinifluxCredentials) {
        Logger.fluxBar.info("configure requested at startup")
        configurationGeneration += 1
        let configuration = configurationGeneration
        isLoading = true
        Task {
            do {
                try await configureCore(credentials, generation: configuration)
                guard configuration == configurationGeneration else { return }
                Logger.fluxBar.info("configure completed at startup")
                coreConfigured = true
                startBackgroundSync()
                resumeAfterConfiguration()
            } catch {
                guard configuration == configurationGeneration else { return }
                Logger.fluxBar.error("configure failed at startup: \(error.localizedDescription)")
                errorMessage = error.localizedDescription
                isLoading = false
            }
        }
    }

    private func configureCore(_ credentials: MinifluxCredentials, generation: Int) async throws {
        _ = try await requestOffMain(CoreRequest(
            operation: "configure",
            server: credentials.server,
            apiKey: credentials.apiKey,
            newestFirst: credentials.newestFirst ?? false,
            configurationGeneration: Int64(generation),
            locales: Locale.preferredLanguages
        ))
    }

    private func navigate(to selection: ArticleSelection, resetPresentation: Bool = true) {
        if resetPresentation {
            resetListPresentation()
        }
        retainedEntryIDs.removeAll()
        self.selection = selection
        isNavigating = true
        loadLocalThenRefresh()
    }

    private func loadLocalThenRefresh() {
        routeGeneration += 1
        let route = routeGeneration
        let selected = selection
        isLoading = true
        Task {
            do {
                let local = try await requestOffMain(CoreRequest(
                    operation: "local_snapshot",
                    selection: selected,
                    retainEntryIDs: Array(retainedEntryIDs)
                ))
                guard route == routeGeneration else { return }
                apply(local, structural: false)
            } catch {
                guard route == routeGeneration else { return }
                errorMessage = error.localizedDescription
                isNavigating = false
            }
            guard !isPopoverVisible else { return }
            refresh(selection: selected, route: route, retainEntryIDs: Array(retainedEntryIDs), origin: .startup)
        }
    }

    private func refresh(
        selection: ArticleSelection,
        route: Int,
        retainEntryIDs: [Int64],
        origin: RefreshOrigin
    ) {
        let reason = switch origin {
        case .manual: "manual"
        case .startup: "startup"
        case .background: "background"
        }
        Logger.fluxBar.info("refresh requested reason=\(reason)")
        refreshGeneration += 1
        let refresh = refreshGeneration
        isLoading = true
        Task {
            do {
                let response = try await requestOffMain(CoreRequest(
                    operation: "refresh",
                    selection: selection,
                    retainEntryIDs: retainEntryIDs
                ))
                guard route == routeGeneration, refresh == refreshGeneration else { return }
                apply(response, structural: true)
                if response.error == nil {
                    lastRefresh = Date()
                    if let snapshot = response.snapshot {
                        Logger.fluxBar.info("refresh completed entries=\(snapshot.entries.count) total=\(snapshot.total) unread=\(snapshot.unreadTotal) starred=\(snapshot.starredTotal)")
                    } else {
                        Logger.fluxBar.info("refresh completed without snapshot")
                    }
                    if origin == .manual, isPopoverVisible {
                        manualRefreshScrollRequestID &+= 1
                        manualRefreshScrollRequest = ManualRefreshScrollRequest(
                            id: manualRefreshScrollRequestID,
                            presentationRevision: listPresentationRevision,
                            firstEntryID: snapshot.entries.first?.id
                        )
                    }
                } else {
                    Logger.fluxBar.warning("refresh partial/error: \(response.error ?? "unknown")")
                }
            } catch {
                guard route == routeGeneration, refresh == refreshGeneration else { return }
                Logger.fluxBar.error("refresh failed: \(error.localizedDescription)")
                errorMessage = error.localizedDescription
                isLoading = false
            }
        }
    }

    private func mutate(_ request: CoreRequest) {
        mutationGeneration += 1
        let mutation = mutationGeneration
        let route = routeGeneration
        Task {
            do {
                let response = try await requestOffMain(request)
                guard route == routeGeneration, mutation == mutationGeneration else {
                    if route == routeGeneration { reloadLocal(route: route) }
                    return
                }
                apply(response, structural: false)
            } catch {
                guard route == routeGeneration, mutation == mutationGeneration else { return }
                errorMessage = error.localizedDescription
            }
        }
    }

    private func showUndo(_ receipt: MutationReceipt) {
        if let previous = automaticReadUndo, previous.id != receipt.id {
            discardUndo(previous.id)
        }
        automaticReadUndo = AutomaticReadUndo(id: receipt.id, count: receipt.count)
        undoExpirationTask?.cancel()
        undoExpirationTask = Task {
            try? await Task.sleep(for: .seconds(8))
            guard !Task.isCancelled, automaticReadUndo?.id == receipt.id else { return }
            automaticReadUndo = nil
            discardUndo(receipt.id)
        }
    }

    private func discardUndo(_ id: String) {
        Task {
            _ = try? await requestOffMain(CoreRequest(operation: "discard_undo", mutationID: id))
        }
    }

    private func reloadLocal(route: Int) {
        let selected = selection
        Task {
            guard let response = try? await requestOffMain(CoreRequest(
                operation: "local_snapshot",
                selection: selected,
                retainEntryIDs: Array(retainedEntryIDs)
            )),
                  route == routeGeneration else { return }
            apply(response, structural: false)
        }
    }

    private func startBackgroundSync() {
        guard backgroundSync == nil else { return }
        backgroundSync = Timer.publish(every: 15 * 60, on: .main, in: .common)
            .autoconnect()
            .sink { [weak self] _ in self?.refreshInBackground() }
    }

    private func refreshInBackground() {
        guard !isPopoverVisible else { return }
        refresh(
            selection: selection,
            route: routeGeneration,
            retainEntryIDs: Array(retainedEntryIDs),
            origin: .background
        )
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

    private func resumeAfterConfiguration() {
        if let pendingSelection {
            self.pendingSelection = nil
            navigate(to: pendingSelection, resetPresentation: false)
        } else {
            loadLocalThenRefresh()
        }
        if refreshWhenConfigured {
            refreshWhenConfigured = false
            refresh()
        }
    }

    private func unreadPreferenceKey(_ selection: ArticleSelection) -> String {
        if selection.kind == "category", let id = selection.id {
            return "FluxBar.unreadOnly.category.\(id)"
        }
        if selection.kind == "feed", let id = selection.id {
            return "FluxBar.unreadOnly.feed.\(id)"
        }
        return "FluxBar.unreadOnly.all"
    }

    private func apply(_ response: CoreResponse, structural: Bool) {
        if let snapshot = response.snapshot {
            if structural {
                resetListPresentation()
            }
            self.snapshot = snapshot
            selection = snapshot.selection
            isNavigating = false
        }
        errorMessage = response.error
        isLoading = false
    }
}
