import AppKit
import SwiftUI

struct ScrollActivity {
    let offsetDelta: CGFloat
    let userInitiated: Bool
}

struct ScrollActivityObserver: NSViewRepresentable {
    let onActivity: (ScrollActivity) -> Void

    func makeCoordinator() -> Coordinator {
        Coordinator(onActivity: onActivity)
    }

    func makeNSView(context: Context) -> NSView {
        let view = NSView(frame: .zero)
        DispatchQueue.main.async { context.coordinator.attach(to: view.enclosingScrollView) }
        return view
    }

    func updateNSView(_ view: NSView, context: Context) {
        context.coordinator.onActivity = onActivity
        DispatchQueue.main.async { context.coordinator.attach(to: view.enclosingScrollView) }
    }

    static func dismantleNSView(_ view: NSView, coordinator: Coordinator) {
        coordinator.detach()
    }

    final class Coordinator {
        var onActivity: (ScrollActivity) -> Void
        private weak var scrollView: NSScrollView?
        private var boundsObserver: NSObjectProtocol?
        private var eventMonitor: Any?
        private var lastOffsetY: CGFloat?
        private var lastVerticalWheelTime: TimeInterval = -.infinity

        init(onActivity: @escaping (ScrollActivity) -> Void) {
            self.onActivity = onActivity
        }

        func attach(to scrollView: NSScrollView?) {
            guard let scrollView, self.scrollView !== scrollView else { return }
            detach()
            self.scrollView = scrollView
            let clipView = scrollView.contentView
            clipView.postsBoundsChangedNotifications = true
            lastOffsetY = clipView.bounds.origin.y
            boundsObserver = NotificationCenter.default.addObserver(
                forName: NSView.boundsDidChangeNotification,
                object: clipView,
                queue: .main
            ) { [weak self] _ in
                self?.boundsChanged()
            }
            eventMonitor = NSEvent.addLocalMonitorForEvents(matching: .scrollWheel) { [weak self] event in
                self?.record(event)
                return event
            }
        }

        func detach() {
            if let boundsObserver {
                NotificationCenter.default.removeObserver(boundsObserver)
            }
            if let eventMonitor {
                NSEvent.removeMonitor(eventMonitor)
            }
            boundsObserver = nil
            eventMonitor = nil
            scrollView = nil
            lastOffsetY = nil
            lastVerticalWheelTime = -.infinity
        }

        private func record(_ event: NSEvent) {
            guard let scrollView,
                  event.window === scrollView.window,
                  abs(event.scrollingDeltaY) > abs(event.scrollingDeltaX) else { return }
            let point = scrollView.convert(event.locationInWindow, from: nil)
            guard scrollView.bounds.contains(point) else { return }
            lastVerticalWheelTime = ProcessInfo.processInfo.systemUptime
        }

        private func boundsChanged() {
            guard let scrollView else { return }
            let offsetY = scrollView.contentView.bounds.origin.y
            guard let previous = lastOffsetY else {
                lastOffsetY = offsetY
                return
            }
            lastOffsetY = offsetY
            let recentWheel = ProcessInfo.processInfo.systemUptime - lastVerticalWheelTime < 0.25
            onActivity(ScrollActivity(offsetDelta: offsetY - previous, userInitiated: recentWheel))
        }
    }
}
