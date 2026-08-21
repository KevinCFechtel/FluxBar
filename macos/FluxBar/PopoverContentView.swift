import AppKit
import SwiftUI

enum PopoverLayout {
    static let rowWidth: CGFloat = 620
    static let cardWidth: CGFloat = 390
    static let sidebarWidth: CGFloat = 200
    static let rowHeight: CGFloat = 620
    static let cardHeight: CGFloat = 760
    static let verticalScreenMargin: CGFloat = 32
    static let sidebarAnimationDuration: TimeInterval = 0.2

    static func contentWidth(for style: ArticleListStyle) -> CGFloat {
        style == .row ? rowWidth : cardWidth
    }

    static func width(style: ArticleListStyle, sidebarVisible: Bool) -> CGFloat {
        contentWidth(for: style) + (sidebarVisible ? sidebarWidth : 0)
    }

    static func height(for style: ArticleListStyle, maximumHeight: CGFloat) -> CGFloat {
        min(style == .row ? rowHeight : cardHeight, maximumHeight)
    }
}

private enum ArticleScrollCoordinateSpace {
    static let name = "FluxBar.ArticleScroll"
}

private struct ArticleFramePreferenceKey: PreferenceKey {
    static var defaultValue: [Int64: CGRect] = [:]

    static func reduce(value: inout [Int64: CGRect], nextValue: () -> [Int64: CGRect]) {
        value.merge(nextValue(), uniquingKeysWith: { _, new in new })
    }
}

struct PopoverContentView: View {
    @ObservedObject var store: BrowserStore
    @ObservedObject var localization: Localization
    let layoutChanged: (Bool) -> Void
    let dismiss: () -> Void
    @State private var sidebarVisible = false

    var body: some View {
        HStack(spacing: 0) {
            if sidebarVisible {
                NavigationSidebar(store: store, localization: localization)
                    .frame(width: PopoverLayout.sidebarWidth)
                    .overlay(alignment: .trailing) { Divider() }
                    .transition(.move(edge: .leading).combined(with: .opacity))
            }
            ArticlePane(
                store: store,
                localization: localization,
                sidebarVisible: $sidebarVisible,
                layoutChanged: layoutChanged,
                dismiss: dismiss
            )
            .frame(width: PopoverLayout.contentWidth(for: store.articleListStyle))
        }
        .frame(width: PopoverLayout.width(style: store.articleListStyle, sidebarVisible: sidebarVisible))
        .frame(maxHeight: .infinity)
        .background(.regularMaterial)
        .sheet(isPresented: $store.showingSettings) {
            SettingsView(store: store, localization: localization)
        }
    }
}

private struct ArticlePane: View {
    @Environment(\.colorScheme) private var colorScheme
    @ObservedObject var store: BrowserStore
    @ObservedObject var localization: Localization
    @Binding var sidebarVisible: Bool
    let layoutChanged: (Bool) -> Void
    let dismiss: () -> Void
    @State private var exposureTracker = ScrolloverExposureTracker()
    @State private var articleFrames: [Int64: CGRect] = [:]
    @State private var articleViewport = CGRect.zero
    @State private var trackerPresentationRevision: UInt64 = 0
    @State private var selectedArticleID: Int64?
    @State private var suppressScrolloverUntil: TimeInterval = 0
    private let exposureTimer = Timer.publish(every: 0.2, on: .main, in: .common).autoconnect()

    var body: some View {
        VStack(spacing: 0) {
            header
            Divider()
            content
        }
        .background(colorScheme == .light ? Color.white.opacity(0.72) : Color.clear)
        .background {
            if store.snapshot.entries.isEmpty {
                KeyboardCommandObserver { command in
                    if command == .refresh { store.refresh() }
                    if command == .dismiss { dismiss() }
                }
            }
        }
        .overlay(alignment: .bottom) {
            if let undo = store.automaticReadUndo {
                automaticReadUndo(undo)
                    .padding(.bottom, 12)
            }
        }
        .onChange(of: store.listPresentationRevision) { _ in
            exposureTracker.reset()
            trackerPresentationRevision = store.listPresentationRevision
            selectedArticleID = nil
            suppressScrolloverUntil = ProcessInfo.processInfo.systemUptime + 0.4
        }
        .onChange(of: store.isPopoverVisible) { visible in
            exposureTracker.reset()
            trackerPresentationRevision = store.listPresentationRevision
            if visible {
                observeExposure(at: Date.timeIntervalSinceReferenceDate)
            }
        }
        .onReceive(exposureTimer) { date in
            observeExposure(at: date.timeIntervalSinceReferenceDate)
        }
        .onChange(of: store.snapshot.entries.map(\.id)) { ids in
            if let selectedArticleID, !ids.contains(selectedArticleID) {
                self.selectedArticleID = nil
            }
        }
    }

    private var header: some View {
        HStack(spacing: 8) {
            Button {
                withAnimation(.easeInOut(duration: PopoverLayout.sidebarAnimationDuration)) {
                    sidebarVisible.toggle()
                }
                layoutChanged(sidebarVisible)
            } label: {
                Image(systemName: "sidebar.left")
            }
            .buttonStyle(.borderless)
            .help(localization.text(sidebarVisible ? "navigation.hide" : "navigation.show", sidebarVisible ? "Hide navigation" : "Show navigation"))
            .accessibilityLabel(localization.text(sidebarVisible ? "navigation.hide" : "navigation.show", sidebarVisible ? "Hide navigation" : "Show navigation"))

            Text(selectionTitle)
                .font(.headline)
                .lineLimit(1)
            countBadge(titleCount)
            Spacer()
            Button { store.refresh() } label: {
                if store.isLoading {
                    ProgressView()
                        .controlSize(.small)
                        .frame(width: 16, height: 16)
                } else {
                    Image(systemName: "arrow.clockwise")
                }
            }
            .buttonStyle(.borderless)
            .disabled(store.isLoading)
            .help(localization.text("menu.refresh.tooltip", "Refresh Miniflux now"))
            .accessibilityLabel(localization.text("menu.refresh", "Refresh"))
            Menu {
                if store.supportsUnreadFilter {
                    Button { store.setUnreadOnly(true) } label: {
                        Label(
                            localization.text("filter.show_unread_only", "Show Unread News Only"),
                            systemImage: store.showsUnreadOnly ? "checkmark.circle.fill" : "circle"
                        )
                    }
                    Button { store.setUnreadOnly(false) } label: {
                        Label(
                            localization.text("filter.show_all", "Show All News"),
                            systemImage: store.showsUnreadOnly ? "circle" : "checkmark.circle.fill"
                        )
                    }
                    Divider()
                }
                Menu {
                    Button { store.setNewestFirst(true) } label: {
                        Label(
                            localization.text("settings.sort.newest", "Newest First"),
                            systemImage: "arrow.down"
                        )
                    }
                    Button { store.setNewestFirst(false) } label: {
                        Label(
                            localization.text("settings.sort.oldest", "Oldest First"),
                            systemImage: "arrow.up"
                        )
                    }
                } label: {
                    Label(localization.text("settings.sort", "Sort Order"), systemImage: "arrow.up.arrow.down")
                }
                .disabled(store.credentials == nil)
                Menu {
                    Button { setArticleListStyle(.row) } label: {
                        Label(
                            localization.text("view.rows", "Rows"),
                            systemImage: store.articleListStyle == .row ? "checkmark" : "list.bullet"
                        )
                    }
                    Button { setArticleListStyle(.card) } label: {
                        Label(
                            localization.text("view.cards", "Cards"),
                            systemImage: store.articleListStyle == .card ? "checkmark" : "rectangle.grid.1x2"
                        )
                    }
                } label: {
                    Label(localization.text("view.layout", "Layout"), systemImage: "rectangle.3.group")
                }
                Button { store.showingSettings = true } label: {
                    Label(localization.text("menu.settings", "Settings…"), systemImage: "gearshape")
                }
                Divider()
                Button { NSApplication.shared.terminate(nil) } label: {
                    Label(localization.text("menu.quit", "Quit FluxBar"), systemImage: "power")
                }
            } label: {
                Image(systemName: "gearshape")
            }
            .menuStyle(.borderlessButton)
            .menuIndicator(.hidden)
            .frame(width: 24)
            .help(localization.text("menu.settings", "Settings…"))
            .accessibilityLabel(localization.text("menu.settings", "Settings…"))
        }
        .padding(.horizontal, 12)
        .frame(height: 44)
    }

    @ViewBuilder
    private var content: some View {
        if let error = store.errorMessage, store.snapshot.entries.isEmpty {
            EmptyState(
                title: localization.text("status.refresh_failed", "Refresh failed"),
                detail: error,
                systemImage: "exclamationmark.triangle"
            )
        } else if store.isLoading && store.snapshot.entries.isEmpty {
            VStack(spacing: 12) {
                ProgressView()
                Text(localization.text("status.loading", "Loading Miniflux…"))
                    .foregroundStyle(.secondary)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        } else if store.snapshot.entries.isEmpty {
            EmptyState(
                title: localization.text("status.no_articles", "No articles"),
                detail: localization.text("status.no_articles_detail", "There are no articles in this selection."),
                systemImage: "tray"
            )
        } else {
            GeometryReader { geometry in
                ScrollViewReader { proxy in
                    ScrollView {
                        LazyVStack(spacing: store.articleListStyle == .row ? 0 : 4) {
                            ScrollActivityObserver { activity in
                                DispatchQueue.main.async {
                                    processScroll(activity)
                                }
                            }
                            .frame(height: 0)
                            ForEach(store.snapshot.entries) { article in
                                ArticleItem(
                                    article: article,
                                    style: store.articleListStyle,
                                    selected: selectedArticleID == article.id,
                                    store: store,
                                    localization: localization,
                                    onSelect: { selectedArticleID = article.id }
                                )
                                .id(article.id)
                                .background {
                                    GeometryReader { rowGeometry in
                                        Color.clear.preference(
                                            key: ArticleFramePreferenceKey.self,
                                            value: [article.id: rowGeometry.frame(in: .named(ArticleScrollCoordinateSpace.name))]
                                        )
                                    }
                                }
                                if store.articleListStyle == .row {
                                    Divider().padding(.leading, 264)
                                }
                            }
                        }
                    }
                    .coordinateSpace(name: ArticleScrollCoordinateSpace.name)
                    .background {
                        KeyboardCommandObserver { command in
                            handleKeyboardCommand(command, proxy: proxy)
                        }
                    }
                    .onAppear {
                        articleViewport = CGRect(origin: .zero, size: geometry.size)
                        trackerPresentationRevision = store.listPresentationRevision
                        observeExposure(at: Date.timeIntervalSinceReferenceDate)
                    }
                    .onChange(of: geometry.size) { size in
                        articleViewport = CGRect(origin: .zero, size: size)
                        exposureTracker.reset()
                        observeExposure(at: Date.timeIntervalSinceReferenceDate)
                    }
                    .onPreferenceChange(ArticleFramePreferenceKey.self) { frames in
                        articleFrames = frames
                        observeExposure(at: Date.timeIntervalSinceReferenceDate)
                    }
                }
            }
        }
    }

    private func setArticleListStyle(_ style: ArticleListStyle) {
        store.setArticleListStyle(style)
        layoutChanged(sidebarVisible)
    }

    private func observeExposure(at time: TimeInterval) {
        guard store.isPopoverVisible,
              !store.isNavigating,
              store.markReadOnScrolloverEnabled,
              !articleViewport.isEmpty else { return }
        exposureTracker.observe(
            frames: articleFrames,
            viewport: articleViewport,
            unreadIDs: Set(store.snapshot.entries.filter { !$0.isRead }.map(\.id)),
            at: time
        )
    }

    private func processScroll(_ activity: ScrollActivity) {
        guard store.isPopoverVisible,
              !store.isNavigating,
              store.markReadOnScrolloverEnabled,
              !articleViewport.isEmpty else { return }
        guard ProcessInfo.processInfo.systemUptime >= suppressScrolloverUntil else {
            exposureTracker.reset()
            return
        }
        guard trackerPresentationRevision == store.listPresentationRevision else {
            exposureTracker.reset()
            trackerPresentationRevision = store.listPresentationRevision
            return
        }
        let marked = exposureTracker.processScroll(
            frames: articleFrames,
            viewport: articleViewport,
            unreadIDs: Set(store.snapshot.entries.filter { !$0.isRead }.map(\.id)),
            at: Date.timeIntervalSinceReferenceDate,
            offsetDelta: activity.offsetDelta,
            userInitiated: activity.userInitiated
        )
        if !marked.isEmpty {
            store.markReadAutomatically(marked)
        }
    }

    private func handleKeyboardCommand(_ command: ArticleKeyboardCommand, proxy: ScrollViewProxy) {
        switch command {
        case .moveUp:
            moveSelection(by: -1, proxy: proxy)
        case .moveDown:
            moveSelection(by: 1, proxy: proxy)
        case .open:
            if let article = selectedArticle { store.open(article) }
        case .toggleRead:
            if let article = selectedArticle { store.setRead(article, read: !article.isRead) }
        case .toggleStarred:
            if let article = selectedArticle { store.setStarred(article, starred: !article.starred) }
        case .refresh:
            store.refresh()
        case .dismiss:
            dismiss()
        }
    }

    private func moveSelection(by delta: Int, proxy: ScrollViewProxy) {
        let entries = store.snapshot.entries
        guard !entries.isEmpty else { return }
        let currentIndex = selectedArticleID.flatMap { id in entries.firstIndex { $0.id == id } }
        let nextIndex: Int
        if let currentIndex {
            nextIndex = min(max(0, currentIndex + delta), entries.count - 1)
        } else {
            nextIndex = delta < 0 ? entries.count - 1 : 0
        }
        let id = entries[nextIndex].id
        selectedArticleID = id
        suppressScrolloverUntil = ProcessInfo.processInfo.systemUptime + 0.4
        exposureTracker.reset()
        if articleFrames[id].map({ !articleViewport.contains($0) }) ?? true {
            proxy.scrollTo(id, anchor: .center)
        }
    }

    private var selectedArticle: Article? {
        guard let selectedArticleID else { return nil }
        return store.snapshot.entries.first { $0.id == selectedArticleID }
    }

    private func automaticReadUndo(_ undo: AutomaticReadUndo) -> some View {
        HStack(spacing: 10) {
            Text(localization.plural(
                "status.scrollover_marked_read",
                one: "{{.Count}} article marked as read",
                other: "{{.Count}} articles marked as read",
                count: undo.count
            ))
            Button(localization.text("edit.undo", "Undo")) {
                store.undoAutomaticRead()
            }
            .buttonStyle(.borderless)
        }
        .font(.callout)
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .background(.regularMaterial, in: Capsule())
        .shadow(radius: 4, y: 2)
    }

    private var selectionTitle: String {
        switch store.selection.kind {
        case "unread": return localization.text("navigation.unread", "Unread")
        case "starred": return localization.text("navigation.starred", "Starred")
        case "category":
            return store.snapshot.categories.first { $0.id == store.selection.id }?.title ?? "FluxBar"
        case "feed":
            return store.snapshot.categories.flatMap(\.feeds).first { $0.id == store.selection.id }?.title ?? "FluxBar"
        default: return localization.text("navigation.all", "All News")
        }
    }

    private var titleCount: Int {
        let selection = store.snapshot.selection
        if selection.unreadOnly == true || selection.kind == "unread" {
            return store.snapshot.total
        }
        return store.snapshot.entries.count
    }

    private func countBadge(_ count: Int) -> some View {
        Text("\(count)")
            .font(.caption.monospacedDigit())
            .foregroundStyle(.secondary)
            .padding(.horizontal, 6)
            .padding(.vertical, 2)
            .background(Color.secondary.opacity(0.12), in: Capsule())
    }
}

private struct EmptyState: View {
    let title: String
    let detail: String
    let systemImage: String

    var body: some View {
        VStack(spacing: 10) {
            Image(systemName: systemImage)
                .font(.system(size: 30))
                .foregroundStyle(.secondary)
            Text(title).font(.headline)
            Text(detail)
                .font(.subheadline)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
                .frame(maxWidth: 320)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .padding()
    }
}

private struct NavigationSidebar: View {
    @ObservedObject var store: BrowserStore
    @ObservedObject var localization: Localization
    @State private var expandedCategories: Set<Int64> = []

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text(localization.text("navigation.feeds", "Feeds"))
                .font(.headline)
                .padding(.horizontal, 12)
                .frame(height: 44)
            Divider()
            ScrollView {
                LazyVStack(spacing: 2) {
                    selectionButton(localization.text("navigation.all", "All News"), icon: "tray.full", count: store.snapshot.unreadTotal, route: .all)
                    selectionButton(localization.text("navigation.starred", "Starred"), icon: "star.fill", count: store.snapshot.starredTotal, route: .starred)
                    ForEach(store.snapshot.categories) { category in
                        categoryButton(category)
                        if expandedCategories.contains(category.id) {
                            ForEach(category.feeds) { feed in
                                feedButton(feed)
                            }
                        }
                    }
                }
                .padding(.horizontal, 8)
                .padding(.vertical, 6)
            }
        }
        .background(.bar)
    }

    private func selectionButton(_ title: String, icon: String, count: Int, route: NavigationRoute) -> some View {
        Button { store.route(to: route) } label: {
            HStack(spacing: 8) {
                Image(systemName: icon).frame(width: 16)
                Text(title).lineLimit(1)
                Spacer(minLength: 0)
                navigationCount(count)
            }
            .navigationRow(selected: route.selection.map(store.selection.matchesRoute) ?? false)
        }
        .buttonStyle(.plain)
    }

    private func categoryButton(_ category: FeedCategory) -> some View {
        HStack(spacing: 8) {
            Button {
                if expandedCategories.contains(category.id) {
                    expandedCategories.remove(category.id)
                } else {
                    expandedCategories.insert(category.id)
                }
            } label: {
                Image(systemName: "chevron.right")
                    .font(.caption.bold())
                    .rotationEffect(.degrees(expandedCategories.contains(category.id) ? 90 : 0))
                    .frame(width: 16)
            }
            .buttonStyle(.plain)
            .accessibilityLabel(category.title)

            Button { store.route(to: .category(category.id)) } label: {
                HStack(spacing: 8) {
                    Text(category.title).lineLimit(1)
                    Spacer(minLength: 0)
                    navigationCount(category.unreadCount)
                }
            }
            .buttonStyle(.plain)
        }
        .navigationRow(selected: store.selection.matchesRoute(.category(category.id)))
    }

    private func feedButton(_ feed: Feed) -> some View {
        Button { store.route(to: .feed(feed.id)) } label: {
            HStack(spacing: 8) {
                FeedIconView(
                    feedID: feed.id,
                    feedName: feed.title,
                    accessibilityLabel: localization.text("article.feed_icon", "Feed icon")
                )
                Text(feed.title).lineLimit(1)
                Spacer(minLength: 0)
                navigationCount(feed.unreadCount)
            }
            .padding(.leading, 24)
            .navigationRow(selected: store.selection.matchesRoute(.feed(feed.id)))
        }
        .buttonStyle(.plain)
    }

    private func navigationCount(_ count: Int) -> some View {
        Text("\(count)")
            .font(.caption.monospacedDigit())
            .foregroundStyle(.secondary)
    }
}

private extension View {
    func navigationRow(selected: Bool) -> some View {
        self
            .fontWeight(selected ? .semibold : .regular)
            .padding(.horizontal, 6)
            .frame(maxWidth: .infinity, minHeight: 28, alignment: .leading)
            .background(selected ? Color.accentColor.opacity(0.16) : Color.clear)
            .clipShape(RoundedRectangle(cornerRadius: 5))
    }
}

private struct ArticleItem: View {
    private static let isoFormatter = ISO8601DateFormatter()
    private static let relativeFormatter: RelativeDateTimeFormatter = {
        let formatter = RelativeDateTimeFormatter()
        formatter.unitsStyle = .abbreviated
        return formatter
    }()

    let article: Article
    let style: ArticleListStyle
    let selected: Bool
    @ObservedObject var store: BrowserStore
    @ObservedObject var localization: Localization
    let onSelect: () -> Void
    @State private var hovered = false

    var body: some View {
        switch style {
        case .row:
            row
        case .card:
            card
        }
    }

    private var row: some View {
        HStack(alignment: .top, spacing: 12) {
            Button {
                onSelect()
                store.open(article)
            } label: {
                HStack(alignment: .top, spacing: 12) {
                    ThumbnailView(
                        url: article.imageURL.flatMap(URL.init(string:)),
                        accessibilityLabel: localization.text("article.thumbnail", "Article thumbnail"),
                        width: 240,
                        height: 168
                    )
                    textComposition
                }
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)

            quickActions
                .opacity(hovered ? 1 : 0)
                .allowsHitTesting(hovered)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.horizontal, 12)
        .padding(.vertical, 10)
        .contentShape(Rectangle())
        .background(selected ? Color.accentColor.opacity(0.16) : (hovered ? Color.primary.opacity(0.055) : Color.clear))
        .onHover { hovered = $0 }
        .contextMenu { actionMenu }
        .disabled(store.isPending(article.id))
    }

    private var card: some View {
        VStack(alignment: .leading, spacing: 0) {
            Button {
                onSelect()
                store.open(article)
            } label: {
                ThumbnailView(
                    url: article.imageURL.flatMap(URL.init(string:)),
                    accessibilityLabel: localization.text("article.thumbnail", "Article thumbnail"),
                    width: PopoverLayout.cardWidth - 24,
                    height: 206,
                    cornerRadius: 10
                )
            }
            .buttonStyle(.plain)
            HStack(alignment: .top, spacing: 10) {
                Button {
                    onSelect()
                    store.open(article)
                } label: {
                    textComposition
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                quickActions
                    .opacity(hovered ? 1 : 0)
                    .allowsHitTesting(hovered)
            }
            .padding(12)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(selected ? Color.accentColor.opacity(0.16) : Color.primary.opacity(hovered ? 0.07 : 0.035))
        .clipShape(RoundedRectangle(cornerRadius: 10))
        .overlay {
            RoundedRectangle(cornerRadius: 10)
                .stroke(Color.secondary.opacity(0.16), lineWidth: 1)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 6)
        .contentShape(Rectangle())
        .onHover { hovered = $0 }
        .contextMenu { actionMenu }
        .disabled(store.isPending(article.id))
    }

    private var textComposition: some View {
        VStack(alignment: .leading, spacing: 5) {
            metadata
            Text(article.title)
                .font(.system(size: 14, weight: article.isRead ? .regular : .semibold))
                .foregroundStyle(article.isRead ? .secondary : .primary)
                .lineLimit(3)
                .multilineTextAlignment(.leading)
            if !article.preview.isEmpty {
                Text(article.preview)
                    .font(.system(size: 12.5))
                    .foregroundStyle(article.isRead ? .tertiary : .secondary)
                    .lineLimit(6)
                    .multilineTextAlignment(.leading)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private var metadata: some View {
        HStack(spacing: 5) {
            FeedIconView(
                feedID: article.feedID,
                feedName: article.feedName,
                accessibilityLabel: localization.text("article.feed_icon", "Feed icon")
            )
            Text(article.feedName).lineLimit(1)
            Text("·")
            Text(relativeDate)
            if article.commentsURL != nil {
                Image(systemName: "bubble.left")
            }
            if article.starred {
                Image(systemName: "star.fill")
                    .foregroundStyle(.yellow)
                    .accessibilityLabel(localization.text("action.unstar", "Unstar"))
            }
        }
        .font(.caption)
        .foregroundStyle(article.isRead ? .tertiary : .secondary)
    }

    private var quickActions: some View {
        VStack(spacing: 8) {
            iconButton(
                article.isRead ? "circle.fill" : "checkmark.circle",
                label: localization.text(article.isRead ? "action.mark_unread" : "action.mark_read", article.isRead ? "Mark as Unread" : "Mark as Read")
            ) { store.setRead(article, read: !article.isRead) }
            iconButton(
                article.starred ? "star.fill" : "star",
                label: localization.text(article.starred ? "action.unstar" : "action.star", article.starred ? "Unstar" : "Star")
            ) { store.setStarred(article, starred: !article.starred) }
            Menu { actionMenu } label: { Image(systemName: "ellipsis") }
                .menuStyle(.borderlessButton)
                .menuIndicator(.hidden)
                .frame(width: 22)
                .help(localization.text("action.more", "More"))
                .accessibilityLabel(localization.text("action.more", "More"))
        }
    }

    @ViewBuilder
    private var actionMenu: some View {
        Button { store.open(article) } label: {
            Label(localization.text("action.open_browser", "Open in Browser"), systemImage: "safari")
        }
        Button { store.setRead(article, read: !article.isRead) } label: {
            Label(
                localization.text(article.isRead ? "action.mark_unread" : "action.mark_read", article.isRead ? "Mark as Unread" : "Mark as Read"),
                systemImage: article.isRead ? "circle.fill" : "checkmark.circle"
            )
        }
        Button { store.setStarred(article, starred: !article.starred) } label: {
            Label(
                localization.text(article.starred ? "action.unstar" : "action.star", article.starred ? "Unstar" : "Star"),
                systemImage: article.starred ? "star.slash" : "star"
            )
        }
        Divider()
        Button { store.copyLink(article) } label: {
            Label(localization.text("action.copy_link", "Copy Link"), systemImage: "doc.on.doc")
        }
        Button { store.share(article) } label: {
            Label(localization.text("action.share", "Share…"), systemImage: "square.and.arrow.up")
        }
        Button { store.showFeed(article) } label: {
            Label(localization.text("action.filter_feed", "Show Feed"), systemImage: "line.3.horizontal.decrease.circle")
        }
        if article.commentsURL != nil {
            Button { store.openComments(article) } label: {
                Label(localization.text("action.open_comments", "Open Comments"), systemImage: "bubble.left")
            }
        }
    }

    private func iconButton(_ icon: String, label: String, action: @escaping () -> Void) -> some View {
        Button(action: action) { Image(systemName: icon) }
            .buttonStyle(.borderless)
            .help(label)
            .accessibilityLabel(label)
    }

    private var relativeDate: String {
        guard let date = Self.isoFormatter.date(from: article.publishedAt) else { return "" }
        return Self.relativeFormatter.localizedString(for: date, relativeTo: Date())
    }
}

private struct SettingsView: View {
    @ObservedObject var store: BrowserStore
    @ObservedObject var localization: Localization
    @State private var server = ""
    @State private var apiKey = ""
    @State private var launchAtLogin = false
    @State private var markReadOnScrolloverEnabled = true
    @State private var globalShortcut = GlobalShortcutChoice.optionCommandF

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text(localization.text("settings.title", "FluxBar Settings")).font(.title2.bold())
            Text(localization.text("settings.security_note", "Credentials are stored securely in the macOS Keychain."))
                .font(.caption)
                .foregroundStyle(.secondary)
            TextField(localization.text("settings.server", "Miniflux Server"), text: $server)
            SecureField(localization.text("settings.api_key", "API Key"), text: $apiKey)
            Toggle(localization.text("settings.launch_at_login", "Launch automatically at login"), isOn: $launchAtLogin)
            Toggle(
                localization.text("settings.mark_read_on_scrollover", "Mark articles as read when scrolling past"),
                isOn: $markReadOnScrolloverEnabled
            )
            Picker(
                localization.text("settings.global_shortcut", "Global shortcut"),
                selection: $globalShortcut
            ) {
                ForEach(GlobalShortcutChoice.allCases, id: \.self) { shortcut in
                    Text(shortcut.title(localization: localization)).tag(shortcut)
                }
            }
            if let error = store.globalShortcutRegistrationError {
                Text(error)
                    .font(.caption)
                    .foregroundStyle(.red)
            }
            HStack {
                Spacer()
                Button(localization.text("settings.cancel", "Cancel")) { store.showingSettings = false }
                Button(localization.text("settings.save", "Save")) {
                    store.setMarkReadOnScrolloverEnabled(markReadOnScrolloverEnabled)
                    store.setGlobalShortcut(globalShortcut)
                    store.save(credentials: MinifluxCredentials(
                        server: server.trimmingCharacters(in: .whitespacesAndNewlines),
                        apiKey: apiKey.trimmingCharacters(in: .whitespacesAndNewlines),
                        showSplash: store.credentials?.showSplash ?? true,
                        newestFirst: store.credentials?.newestFirst ?? false
                    ), launchAtLogin: launchAtLogin)
                }
                .keyboardShortcut(.defaultAction)
                .disabled(server.isEmpty || apiKey.isEmpty)
            }
        }
        .padding(20)
        .frame(width: 420)
        .onAppear {
            server = store.credentials?.server ?? ""
            apiKey = store.credentials?.apiKey ?? ""
            launchAtLogin = CredentialStore.launchAtLoginEnabled
            markReadOnScrolloverEnabled = store.markReadOnScrolloverEnabled
            globalShortcut = store.globalShortcut
        }
    }
}
