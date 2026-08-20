import AppKit
import SwiftUI

enum PopoverLayout {
    static let contentWidth: CGFloat = 690
    static let sidebarWidth: CGFloat = 200
    static let height: CGFloat = 620

    static func width(sidebarVisible: Bool) -> CGFloat {
        contentWidth + (sidebarVisible ? sidebarWidth : 0)
    }
}

struct PopoverContentView: View {
    @ObservedObject var store: BrowserStore
    @ObservedObject var localization: Localization
    let sidebarChanged: (Bool) -> Void
    @State private var sidebarVisible = false

    var body: some View {
        HStack(spacing: 0) {
            if sidebarVisible {
                NavigationSidebar(store: store, localization: localization)
                    .frame(width: PopoverLayout.sidebarWidth)
                Divider()
            }
            ArticlePane(
                store: store,
                localization: localization,
                sidebarVisible: $sidebarVisible,
                sidebarChanged: sidebarChanged
            )
            .frame(width: PopoverLayout.contentWidth)
        }
        .frame(
            width: PopoverLayout.width(sidebarVisible: sidebarVisible),
            height: PopoverLayout.height
        )
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
    let sidebarChanged: (Bool) -> Void

    var body: some View {
        VStack(spacing: 0) {
            header
            Divider()
            content
        }
        .background(colorScheme == .light ? Color.white.opacity(0.72) : Color.clear)
    }

    private var header: some View {
        HStack(spacing: 8) {
            Button {
                withAnimation(.easeInOut(duration: 0.18)) {
                    sidebarVisible.toggle()
                }
                sidebarChanged(sidebarVisible)
            } label: {
                Image(systemName: "sidebar.left")
            }
            .buttonStyle(.borderless)
            .help(localization.text(sidebarVisible ? "navigation.hide" : "navigation.show", sidebarVisible ? "Hide navigation" : "Show navigation"))
            .accessibilityLabel(localization.text(sidebarVisible ? "navigation.hide" : "navigation.show", sidebarVisible ? "Hide navigation" : "Show navigation"))

            Text(selectionTitle)
                .font(.headline)
                .lineLimit(1)
            if store.selection.kind == "category" || store.selection.kind == "feed" {
                countBadge(store.snapshot.total)
            }
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
                Button { store.showingSettings = true } label: {
                    Label(localization.text("menu.settings", "Settings…"), systemImage: "gearshape")
                }
                Divider()
                Button { NSApplication.shared.terminate(nil) } label: {
                    Label(localization.text("menu.quit", "Quit FluxNews"), systemImage: "power")
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
            ScrollView {
                LazyVStack(spacing: 0) {
                    ForEach(store.snapshot.entries) { article in
                        ArticleRow(article: article, store: store, localization: localization)
                        Divider().padding(.leading, 268)
                    }
                }
            }
        }
    }

    private var selectionTitle: String {
        switch store.selection.kind {
        case "unread": return localization.text("navigation.unread", "Unread")
        case "starred": return localization.text("navigation.starred", "Starred")
        case "category":
            return store.snapshot.categories.first { $0.id == store.selection.id }?.title ?? "FluxNews"
        case "feed":
            return store.snapshot.categories.flatMap(\.feeds).first { $0.id == store.selection.id }?.title ?? "FluxNews"
        default: return localization.text("navigation.all", "All News")
        }
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
                    selectionButton(localization.text("navigation.all", "All News"), icon: "tray.full", count: store.snapshot.unreadTotal, selection: .all)
                    selectionButton(localization.text("navigation.starred", "Starred"), icon: "star.fill", count: store.snapshot.starredTotal, selection: .starred)
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

    private func selectionButton(_ title: String, icon: String, count: Int, selection: ArticleSelection) -> some View {
        Button { store.select(selection) } label: {
            HStack(spacing: 8) {
                Image(systemName: icon).frame(width: 16)
                Text(title).lineLimit(1)
                Spacer(minLength: 0)
                navigationCount(count)
            }
            .navigationRow(selected: store.selection.matchesRoute(selection))
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

            Button { store.select(.category(category.id)) } label: {
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
        Button { store.select(.feed(feed.id)) } label: {
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

private struct ArticleRow: View {
    private static let isoFormatter = ISO8601DateFormatter()
    private static let relativeFormatter: RelativeDateTimeFormatter = {
        let formatter = RelativeDateTimeFormatter()
        formatter.unitsStyle = .abbreviated
        return formatter
    }()

    let article: Article
    @ObservedObject var store: BrowserStore
    @ObservedObject var localization: Localization
    @State private var hovered = false

    var body: some View {
        HStack(alignment: .top, spacing: 12) {
            Button { store.open(article) } label: {
                HStack(alignment: .top, spacing: 12) {
                    ThumbnailView(
                        url: article.imageURL.flatMap(URL.init(string:)),
                        accessibilityLabel: localization.text("article.thumbnail", "Article thumbnail")
                    )
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
        .background(hovered ? Color.primary.opacity(0.055) : Color.clear)
        .onHover { hovered = $0 }
        .contextMenu { actionMenu }
        .disabled(store.isPending(article.id))
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

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text(localization.text("settings.title", "FluxNews Settings")).font(.title2.bold())
            Text(localization.text("settings.security_note", "Credentials are stored securely in the macOS Keychain."))
                .font(.caption)
                .foregroundStyle(.secondary)
            TextField(localization.text("settings.server", "Miniflux Server"), text: $server)
            SecureField(localization.text("settings.api_key", "API Key"), text: $apiKey)
            Toggle(localization.text("settings.launch_at_login", "Launch automatically at login"), isOn: $launchAtLogin)
            HStack {
                Spacer()
                Button(localization.text("settings.cancel", "Cancel")) { store.showingSettings = false }
                Button(localization.text("settings.save", "Save")) {
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
        }
    }
}
