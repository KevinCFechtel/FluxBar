import AppKit
import Combine
import SwiftUI

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate, NSPopoverDelegate {
    private static let statusItemWidth: CGFloat = 62

    private var statusItem: NSStatusItem!
    private let popover = NSPopover()
    private let localization = Localization()
    private let store = BrowserStore()
    private var unreadCountObservation: AnyCancellable?

    func applicationDidFinishLaunching(_ notification: Notification) {
        statusItem = NSStatusBar.system.statusItem(withLength: Self.statusItemWidth)
        if let button = statusItem.button {
            if let iconURL = Bundle.main.url(forResource: "FluxBarTemplate", withExtension: "svg") {
                button.image = NSImage(contentsOf: iconURL)
            }
            button.image?.size = NSSize(width: 18, height: 18)
            button.image?.isTemplate = true
            button.imagePosition = .imageLeading
            button.toolTip = "FluxNews"
            button.setAccessibilityLabel("FluxNews")
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
        popover.contentSize = NSSize(width: PopoverLayout.contentWidth, height: PopoverLayout.height)
        popover.contentViewController = NSHostingController(rootView: PopoverContentView(
            store: store,
            localization: localization,
            sidebarChanged: { [weak self] visible in self?.resizePopover(sidebarVisible: visible) }
        ))

        if ProcessInfo.processInfo.arguments.contains("--ui-smoke-test") {
            DispatchQueue.main.async { [weak self] in self?.runUISmokeTest() }
            return
        }
        store.start()
    }

    @objc func togglePopover() {
        if popover.isShown {
            popover.performClose(nil)
            return
        }
        guard let button = statusItem.button else { return }
        popover.show(relativeTo: button.bounds, of: button, preferredEdge: .minY)
        popover.contentViewController?.view.window?.makeKey()
    }

    func popoverDidClose(_ notification: Notification) {
        statusItem.button?.highlight(false)
    }

    private func resizePopover(sidebarVisible: Bool) {
        popover.contentSize = NSSize(
            width: PopoverLayout.width(sidebarVisible: sidebarVisible),
            height: PopoverLayout.height
        )
    }

    private func runUISmokeTest() {
        guard statusItem.button != nil else { exit(EXIT_FAILURE) }
        store.prepareUISmokeTest()
        guard statusItem.button?.title == "1" else { exit(EXIT_FAILURE) }
        togglePopover()
        guard popover.isShown else { exit(EXIT_FAILURE) }

        resizePopover(sidebarVisible: true)
        guard popover.contentSize.width == PopoverLayout.contentWidth + PopoverLayout.sidebarWidth else {
            exit(EXIT_FAILURE)
        }
        resizePopover(sidebarVisible: false)
        guard popover.contentSize.width == PopoverLayout.contentWidth else { exit(EXIT_FAILURE) }

        popover.performClose(nil)
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.2) { [weak self] in
            guard let self, !self.popover.isShown else { exit(EXIT_FAILURE) }
            NSApplication.shared.terminate(nil)
        }
    }
}
