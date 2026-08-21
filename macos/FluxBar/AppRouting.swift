@preconcurrency import CoreSpotlight
import Foundation
import UniformTypeIdentifiers

@MainActor
final class AppRouter {
    static let shared = AppRouter()

    private static let catalogKey = "FluxBar.navigationCatalog"
    private var openHandler: ((NavigationRoute) -> Void)?
    private var refreshHandler: (() -> Void)?
    private var pendingActions: [Action] = []
    private(set) var catalog: NavigationCatalog

    private enum Action {
        case open(NavigationRoute)
        case refresh
    }

    private init(defaults: UserDefaults = .standard) {
        if let data = defaults.data(forKey: Self.catalogKey),
           let catalog = try? JSONDecoder().decode(NavigationCatalog.self, from: data) {
            self.catalog = catalog
        } else {
            catalog = .empty
        }
    }

    func configure(open: @escaping (NavigationRoute) -> Void, refresh: @escaping () -> Void) {
        openHandler = open
        refreshHandler = refresh
        let actions = pendingActions
        pendingActions.removeAll()
        for action in actions { perform(action) }
    }

    func open(_ route: NavigationRoute) {
        perform(.open(route))
    }

    func refresh() {
        perform(.refresh)
    }

    func updateCatalog(_ catalog: NavigationCatalog, defaults: UserDefaults = .standard) {
        self.catalog = catalog
        if let data = try? JSONEncoder().encode(catalog) {
            defaults.set(data, forKey: Self.catalogKey)
        }
    }

    private func perform(_ action: Action) {
        switch action {
        case .open(let route):
            guard let openHandler else {
                pendingActions.append(action)
                return
            }
            openHandler(route)
        case .refresh:
            guard let refreshHandler else {
                pendingActions.append(action)
                return
            }
            refreshHandler()
        }
    }
}

@MainActor
final class SpotlightIndexer {
    private let index = CSSearchableIndex.default()
    private let domainIdentifier = "dev.kevincfechtel.FluxBar.navigation"
    private var pendingItems: [CSSearchableItem]?
    private var isUpdating = false

    func update(_ catalog: NavigationCatalog, localization: Localization) {
        let feedDescription = localization.text("spotlight.feed.description", "Feed in FluxBar")
        let categoryDescription = localization.text("spotlight.category.description", "Category in FluxBar")
        let items = catalog.feeds.map { feed in
            searchableItem(
                identifier: "feed:\(feed.id)",
                title: feed.title,
                description: feedDescription,
                keywords: [feed.categoryTitle]
            )
        } + catalog.categories.map { category in
            searchableItem(
                identifier: "category:\(category.id)",
                title: category.title,
                description: categoryDescription,
                keywords: []
            )
        }
        pendingItems = items
        processNextUpdate()
    }

    static func route(from searchableIdentifier: String) -> NavigationRoute? {
        NavigationRoute(searchableIdentifier: searchableIdentifier)
    }

    private func searchableItem(
        identifier: String,
        title: String,
        description: String,
        keywords: [String]
    ) -> CSSearchableItem {
        let attributes = CSSearchableItemAttributeSet(contentType: .content)
        attributes.title = title
        attributes.contentDescription = description
        attributes.keywords = ["FluxBar"] + keywords
        return CSSearchableItem(
            uniqueIdentifier: identifier,
            domainIdentifier: domainIdentifier,
            attributeSet: attributes
        )
    }

    private func processNextUpdate() {
        guard !isUpdating, let items = pendingItems else { return }
        pendingItems = nil
        isUpdating = true
        index.deleteSearchableItems(withDomainIdentifiers: [domainIdentifier]) { [weak self] _ in
            DispatchQueue.main.async {
                guard let self else { return }
                guard !items.isEmpty else {
                    self.finishUpdate()
                    return
                }
                self.index.indexSearchableItems(items) { _ in
                    DispatchQueue.main.async { [weak self] in self?.finishUpdate() }
                }
            }
        }
    }

    private func finishUpdate() {
        isUpdating = false
        processNextUpdate()
    }
}
