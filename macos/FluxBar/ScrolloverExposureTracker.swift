import CoreGraphics
import Foundation

struct ScrolloverPreferences {
    private static let enabledKey = "FluxBar.markReadOnScrollover"

    static func isEnabled(in defaults: UserDefaults = .standard) -> Bool {
        defaults.object(forKey: enabledKey) as? Bool ?? true
    }

    static func setEnabled(_ enabled: Bool, in defaults: UserDefaults = .standard) {
        defaults.set(enabled, forKey: enabledKey)
    }
}

struct ScrolloverConfiguration: Equatable {
    var minimumVisibleFraction: CGFloat = 0.60
    var minimumVisibleDuration: TimeInterval = 0.70
    var maximumViewportJumpFraction: CGFloat = 0.85

    static let `default` = ScrolloverConfiguration()
}

struct ScrolloverExposureTracker {
    private struct Exposure {
        var visibleSince: TimeInterval?
        var qualified = false
        var processedFrame: CGRect
        var currentFrame: CGRect
    }

    private let configuration: ScrolloverConfiguration
    private var exposures: [Int64: Exposure] = [:]

    init(configuration: ScrolloverConfiguration = .default) {
        self.configuration = configuration
    }

    mutating func reset() {
        exposures.removeAll()
    }

    mutating func rebase(frames: [Int64: CGRect], unreadIDs: Set<Int64>) {
        exposures = exposures.filter { unreadIDs.contains($0.key) }
        for (id, frame) in frames {
            guard unreadIDs.contains(id), var exposure = exposures[id] else { continue }
            exposure.processedFrame = frame
            exposure.currentFrame = frame
            exposures[id] = exposure
        }
    }

    mutating func observe(
        frames: [Int64: CGRect],
        viewport: CGRect,
        unreadIDs: Set<Int64>,
        at time: TimeInterval
    ) {
        for (id, frame) in frames where unreadIDs.contains(id) {
            let fraction = visibleFraction(of: frame, in: viewport)
            var exposure = exposures[id] ?? Exposure(processedFrame: frame, currentFrame: frame)
            if fraction >= configuration.minimumVisibleFraction {
                if exposure.visibleSince == nil {
                    exposure.visibleSince = time
                }
                if let visibleSince = exposure.visibleSince,
                   time - visibleSince >= configuration.minimumVisibleDuration {
                    exposure.qualified = true
                }
            } else if !exposure.qualified {
                exposure.visibleSince = nil
            }
            exposure.currentFrame = frame
            exposures[id] = exposure
        }
        exposures = exposures.filter { unreadIDs.contains($0.key) }
    }

    mutating func processScroll(
        frames: [Int64: CGRect],
        viewport: CGRect,
        unreadIDs: Set<Int64>,
        at time: TimeInterval,
        offsetDelta: CGFloat,
        userInitiated: Bool
    ) -> [Int64] {
        let maximumJump = viewport.height * configuration.maximumViewportJumpFraction
        guard userInitiated, offsetDelta > 0, offsetDelta <= maximumJump else {
            reset()
            observe(frames: frames, viewport: viewport, unreadIDs: unreadIDs, at: time)
            return []
        }

        var marked: [Int64] = []
        for (id, exposure) in exposures where exposure.qualified && unreadIDs.contains(id) {
            if let currentFrame = frames[id] {
                if exposure.processedFrame.maxY > viewport.minY && currentFrame.maxY <= viewport.minY {
                    marked.append(id)
                }
            } else if exposure.currentFrame.midY < viewport.midY {
                marked.append(id)
            }
        }
        for id in marked {
            exposures.removeValue(forKey: id)
        }
        observe(frames: frames, viewport: viewport, unreadIDs: unreadIDs, at: time)
        for id in Array(exposures.keys) {
            guard var exposure = exposures[id] else { continue }
            exposure.processedFrame = exposure.currentFrame
            exposures[id] = exposure
        }
        return marked.sorted()
    }

    private func visibleFraction(of frame: CGRect, in viewport: CGRect) -> CGFloat {
        guard frame.height > 0 else { return 0 }
        return max(0, frame.intersection(viewport).height / frame.height)
    }
}
