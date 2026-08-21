import AppKit
import SwiftUI

struct ScrollActivity {
    let offsetDelta: CGFloat
    let userInitiated: Bool
    let epoch: UInt64
}

struct ScrollActivityObserver: NSViewRepresentable {
    let isPaused: Bool
    let epoch: UInt64
    let onActivity: (ScrollActivity) -> Void

    func makeCoordinator() -> Coordinator {
        Coordinator(isPaused: isPaused, epoch: epoch, onActivity: onActivity)
    }

    func makeNSView(context: Context) -> NSView {
        let view = NSView(frame: .zero)
        DispatchQueue.main.async { context.coordinator.attach(to: view.enclosingScrollView) }
        return view
    }

    func updateNSView(_ view: NSView, context: Context) {
        context.coordinator.onActivity = onActivity
        context.coordinator.update(isPaused: isPaused, epoch: epoch)
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
        private var isPaused: Bool
        private var epoch: UInt64

        init(isPaused: Bool, epoch: UInt64, onActivity: @escaping (ScrollActivity) -> Void) {
            self.isPaused = isPaused
            self.epoch = epoch
            self.onActivity = onActivity
        }

        func update(isPaused: Bool, epoch: UInt64) {
            guard self.isPaused != isPaused || self.epoch != epoch else { return }
            self.isPaused = isPaused
            self.epoch = epoch
            lastOffsetY = scrollView?.contentView.bounds.origin.y
            lastVerticalWheelTime = -.infinity
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
            guard !isPaused,
                  let scrollView,
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
            guard !isPaused else {
                lastVerticalWheelTime = -.infinity
                return
            }
            let offsetDelta = offsetY - previous
            guard offsetDelta != 0 else { return }
            let recentWheel = ProcessInfo.processInfo.systemUptime - lastVerticalWheelTime < 0.25
            onActivity(ScrollActivity(offsetDelta: offsetDelta, userInitiated: recentWheel, epoch: epoch))
        }
    }
}
