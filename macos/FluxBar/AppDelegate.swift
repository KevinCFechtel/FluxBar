import AppKit
import Combine
import CoreSpotlight
import SwiftUI

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate, NSPopoverDelegate, NSWindowDelegate {
    private static let statusItemWidth: CGFloat = 62

    private var statusItem: NSStatusItem!
    private let popover = NSPopover()
    private let localization = Localization()
    private let store = BrowserStore()
    private let spotlightIndexer = SpotlightIndexer()
    private var unreadCountObservation: AnyCancellable?
    private var catalogObservation: AnyCancellable?
    private var shortcutObservation: AnyCancellable?
    private lazy var shortcutRegistrar = GlobalShortcutRegistrar { [weak self] in
        self?.showFluxBar()
    }
    private var sidebarVisible = false
    private var fallbackPanel: NSPanel?

    func applicationDidFinishLaunching(_ notification: Notification) {
        statusItem = NSStatusBar.system.statusItem(withLength: Self.statusItemWidth)
        if let button = statusItem.button {
            if let iconURL = Bundle.main.url(forResource: "FluxBarTemplate", withExtension: "svg") {
                button.image = NSImage(contentsOf: iconURL)
            }
            button.image?.size = NSSize(width: 18, height: 18)
            button.image?.isTemplate = true
            button.imagePosition = .imageLeading
            button.toolTip = "FluxBar"
            button.setAccessibilityLabel("FluxBar")
            button.target = self
            button.action = #selector(togglePopover)
        }
        unreadCountObservation = store.$snapshot
            .map(\.unreadTotal)
            .removeDuplicates()
            .sink { [weak self] count in
                self?.statusItem.button?.title = switch count {
                case ...0: ""
                case 1...999: "\(count)"
                default: "999+"
                }
            }

        popover.behavior = .transient
        popover.animates = true
        popover.delegate = self
        popover.contentSize = popoverSize(sidebarVisible: false)
        popover.contentViewController = NSHostingController(rootView: PopoverContentView(
            store: store,
            localization: localization,
            layoutChanged: { [weak self] visible in
                self?.resizePopover(sidebarVisible: visible, animated: true)
            },
            dismiss: { [weak self] in self?.dismissFluxBar() }
        ))

        AppRouter.shared.configure(
            open: { [weak self] route in
                self?.store.route(to: route)
                self?.showFluxBar()
            },
            refresh: { [weak self] in
                self?.showFluxBar()
                self?.store.refresh()
            }
        )
        catalogObservation = store.$snapshot
            .dropFirst()
            .map(NavigationCatalog.init)
            .removeDuplicates()
            .sink { [weak self] catalog in
                guard let self else { return }
                AppRouter.shared.updateCatalog(catalog)
                spotlightIndexer.update(catalog, localization: localization)
                FluxBarShortcuts.updateAppShortcutParameters()
            }
        shortcutObservation = store.$globalShortcut
            .sink { [weak self] shortcut in
                guard let self else { return }
                let status = shortcutRegistrar.register(shortcut)
                store.setGlobalShortcutRegistrationError(status == noErr ? nil : localization.text(
                    "error.global_shortcut_unavailable",
                    "The selected global shortcut is unavailable. Choose another shortcut."
                ))
            }

        if ProcessInfo.processInfo.arguments.contains("--ui-smoke-test") {
            DispatchQueue.main.async { [weak self] in self?.runUISmokeTest() }
            return
        }
        store.start()
    }

    @objc func togglePopover() {
        if popover.isShown || fallbackPanel?.isVisible == true {
            dismissFluxBar()
            return
        }
        showFluxBar()
    }

    func popoverDidClose(_ notification: Notification) {
        statusItem.button?.highlight(false)
        store.setPopoverVisible(false)
        store.resetListPresentation()
        store.refreshIfStale()
    }

    func popoverWillShow(_ notification: Notification) {
        store.setPopoverVisible(true)
        popover.contentSize = popoverSize(sidebarVisible: sidebarVisible)
    }

    func windowWillClose(_ notification: Notification) {
        guard notification.object as? NSPanel === fallbackPanel else { return }
        store.setPopoverVisible(false)
        store.resetListPresentation()
        store.refreshIfStale()
    }

    func application(
        _ application: NSApplication,
        continue userActivity: NSUserActivity,
        restorationHandler: @escaping ([NSUserActivityRestoring]) -> Void
    ) -> Bool {
        guard userActivity.activityType == CSSearchableItemActionType,
              let identifier = userActivity.userInfo?[CSSearchableItemActivityIdentifier] as? String,
              let route = SpotlightIndexer.route(from: identifier) else { return false }
        AppRouter.shared.open(route)
        return true
    }

    private func showFluxBar() {
        NSApplication.shared.activate(ignoringOtherApps: true)
        if popover.isShown {
            popover.contentViewController?.view.window?.makeKey()
            return
        }
        if let fallbackPanel, fallbackPanel.isVisible {
            fallbackPanel.makeKeyAndOrderFront(nil)
            return
        }
        if let button = usableStatusItemButton() {
            popover.show(relativeTo: button.bounds, of: button, preferredEdge: .minY)
            popover.contentViewController?.view.window?.makeKey()
            return
        }
        showFallbackPanel()
    }

    private func usableStatusItemButton() -> NSStatusBarButton? {
        guard let button = statusItem.button,
              let window = button.window,
              let screen = window.screen else { return nil }
        let frame = window.convertToScreen(button.convert(button.bounds, to: nil))
        return screen.frame.intersects(frame) ? button : nil
    }

    private func dismissFluxBar() {
        if popover.isShown { popover.performClose(nil) }
        fallbackPanel?.close()
    }

    private func showFallbackPanel() {
        let size = popoverSize(sidebarVisible: sidebarVisible)
        let panel = fallbackPanel ?? makeFallbackPanel(size: size)
        fallbackPanel = panel
        panel.setContentSize(size)
        store.setPopoverVisible(true)
        panel.center()
        panel.makeKeyAndOrderFront(nil)
    }

    private func makeFallbackPanel(size: NSSize) -> NSPanel {
        let panel = NSPanel(
            contentRect: NSRect(origin: .zero, size: size),
            styleMask: [.titled, .closable, .utilityWindow],
            backing: .buffered,
            defer: false
        )
        panel.title = "FluxBar"
        panel.isFloatingPanel = true
        panel.level = .floating
        panel.collectionBehavior = [.moveToActiveSpace, .transient]
        panel.isReleasedWhenClosed = false
        panel.delegate = self
        panel.contentViewController = NSHostingController(rootView: PopoverContentView(
            store: store,
            localization: localization,
            layoutChanged: { [weak self] visible in
                self?.resizePopover(sidebarVisible: visible, animated: true)
            },
            dismiss: { [weak self] in self?.dismissFluxBar() }
        ))
        return panel
    }

    private func resizePopover(
        sidebarVisible: Bool,
        animated: Bool = false
    ) {
        self.sidebarVisible = sidebarVisible
        let size = popoverSize(sidebarVisible: sidebarVisible)
        guard animated, !NSWorkspace.shared.accessibilityDisplayShouldReduceMotion else {
            popover.contentSize = size
            fallbackPanel?.setContentSize(size)
            return
        }

        if popover.isShown {
            NSAnimationContext.runAnimationGroup { context in
                context.duration = PopoverLayout.sidebarAnimationDuration
                context.allowsImplicitAnimation = true
                popover.contentSize = size
            }
            fallbackPanel?.setContentSize(size)
            return
        }

        popover.contentSize = size
        guard let panel = fallbackPanel else { return }
        guard panel.isVisible else {
            panel.setContentSize(size)
            return
        }
        let currentFrame = panel.frame
        var targetFrame = panel.frameRect(forContentRect: NSRect(origin: .zero, size: size))
        targetFrame.origin.x = currentFrame.midX - targetFrame.width / 2
        targetFrame.origin.y = currentFrame.maxY - targetFrame.height
        NSAnimationContext.runAnimationGroup { context in
            context.duration = PopoverLayout.sidebarAnimationDuration
            panel.animator().setFrame(targetFrame, display: true)
        }
    }

    private func popoverSize(sidebarVisible: Bool) -> NSSize {
        let visibleHeight = statusItem.button?.window?.screen?.visibleFrame.height
            ?? NSScreen.main?.visibleFrame.height
            ?? PopoverLayout.rowHeight + PopoverLayout.verticalScreenMargin
        let maximumHeight = max(320, visibleHeight - PopoverLayout.verticalScreenMargin)
        return NSSize(
            width: PopoverLayout.width(style: store.articleListStyle, sidebarVisible: sidebarVisible),
            height: PopoverLayout.height(for: store.articleListStyle, maximumHeight: maximumHeight)
        )
    }

    private func runUISmokeTest() {
        guard statusItem.button != nil else { exit(EXIT_FAILURE) }
        guard let resources = Bundle.main.resourceURL,
              FileManager.default.fileExists(atPath: resources.appendingPathComponent("Metadata.appintents").path),
              Bundle.main.path(forResource: "AppShortcuts", ofType: "strings", inDirectory: nil, forLocalization: "de") != nil else {
            exit(EXIT_FAILURE)
        }
        store.prepareUISmokeTest()
        guard statusItem.button?.title == "1",
              store.globalShortcutRegistrationError == nil else { exit(EXIT_FAILURE) }
        guard shortcutRegistrar.sendTestEventForSmokeTest() == noErr else { exit(EXIT_FAILURE) }
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) { [weak self] in
            guard let self, self.popover.isShown else { exit(EXIT_FAILURE) }
            self.continueUISmokeTest()
        }
    }

    private func continueUISmokeTest() {
        resizePopover(sidebarVisible: true)
        guard popover.contentSize.width == PopoverLayout.rowWidth + PopoverLayout.sidebarWidth else {
            exit(EXIT_FAILURE)
        }
        let rowHeight = popover.contentSize.height
        guard rowHeight <= PopoverLayout.rowHeight else { exit(EXIT_FAILURE) }
        store.prepareUICardSmokeTest()
        resizePopover(sidebarVisible: true)
        guard popover.contentSize.width == PopoverLayout.cardWidth + PopoverLayout.sidebarWidth else {
            exit(EXIT_FAILURE)
        }
        guard popover.contentSize.height >= rowHeight,
              popover.contentSize.height <= PopoverLayout.cardHeight else {
            exit(EXIT_FAILURE)
        }
        resizePopover(sidebarVisible: false)
        guard popover.contentSize.width == PopoverLayout.cardWidth else { exit(EXIT_FAILURE) }

        popover.performClose(nil)
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.2) { [weak self] in
            guard let self, !self.popover.isShown else { exit(EXIT_FAILURE) }
            NSApplication.shared.terminate(nil)
        }
    }
}
