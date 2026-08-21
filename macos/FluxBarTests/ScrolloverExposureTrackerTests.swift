import Foundation
import XCTest

final class ScrolloverExposureTrackerTests: XCTestCase {
    private let viewport = CGRect(x: 0, y: 0, width: 600, height: 100)

    func testSufficientVisibilityAndDurationQualify() {
        var tracker = ScrolloverExposureTracker()
        tracker.observe(frames: [1: row(y: 0)], viewport: viewport, unreadIDs: [1], at: 0)
        tracker.observe(frames: [1: row(y: 0)], viewport: viewport, unreadIDs: [1], at: 0.8)

        let marked = tracker.processScroll(
            frames: [1: row(y: -101)], viewport: viewport, unreadIDs: [1],
            at: 1, offsetDelta: 50, userInitiated: true
        )

        XCTAssertEqual(marked, [1])
    }

    func testInsufficientVisibilityDoesNotQualify() {
        var tracker = ScrolloverExposureTracker()
        tracker.observe(frames: [1: row(y: 50)], viewport: viewport, unreadIDs: [1], at: 0)
        tracker.observe(frames: [1: row(y: 50)], viewport: viewport, unreadIDs: [1], at: 2)

        XCTAssertTrue(tracker.processScroll(
            frames: [:], viewport: viewport, unreadIDs: [1],
            at: 2.1, offsetDelta: 40, userInitiated: true
        ).isEmpty)
    }

    func testFastPassDoesNotQualify() {
        var tracker = ScrolloverExposureTracker()
        tracker.observe(frames: [1: row(y: 0)], viewport: viewport, unreadIDs: [1], at: 0)

        XCTAssertTrue(tracker.processScroll(
            frames: [1: row(y: -101)], viewport: viewport, unreadIDs: [1],
            at: 0.2, offsetDelta: 50, userInitiated: true
        ).isEmpty)
    }

    func testQualifiedArticleOnlyMarksWhenScrollingUpwardPastTop() {
        var tracker = qualifiedTracker()
        XCTAssertTrue(tracker.processScroll(
            frames: [1: row(y: 101)], viewport: viewport, unreadIDs: [1],
            at: 1, offsetDelta: -40, userInitiated: true
        ).isEmpty)

        tracker = qualifiedTracker()
        XCTAssertEqual(tracker.processScroll(
            frames: [1: row(y: -101)], viewport: viewport, unreadIDs: [1],
            at: 1, offsetDelta: 40, userInitiated: true
        ), [1])
    }

    func testFramePreferenceUpdateBeforeScrollCallbackStillMarks() {
        var tracker = qualifiedTracker()
        let moved = [Int64(1): row(y: -101)]
        tracker.observe(frames: moved, viewport: viewport, unreadIDs: [1], at: 1)

        XCTAssertEqual(tracker.processScroll(
            frames: moved, viewport: viewport, unreadIDs: [1],
            at: 1, offsetDelta: 40, userInitiated: true
        ), [1])
    }

    func testHorizontalLayoutRebasePreservesQualifiedExposure() {
        var tracker = qualifiedTracker()
        tracker.rebase(
            frames: [1: CGRect(x: 200, y: 0, width: 600, height: 100)],
            unreadIDs: [1]
        )

        XCTAssertEqual(tracker.processScroll(
            frames: [1: CGRect(x: 200, y: -101, width: 600, height: 100)],
            viewport: viewport, unreadIDs: [1],
            at: 1, offsetDelta: 40, userInitiated: true
        ), [1])
    }

    func testLayoutRebaseDropsReadExposure() {
        var tracker = qualifiedTracker()
        tracker.rebase(frames: [1: row(y: 0)], unreadIDs: [])

        XCTAssertTrue(tracker.processScroll(
            frames: [1: row(y: -101)], viewport: viewport, unreadIDs: [],
            at: 1, offsetDelta: 40, userInitiated: true
        ).isEmpty)
    }

    func testLayoutRebaseDoesNotTreatVerticalCorrectionAsScrollover() {
        var tracker = qualifiedTracker()
        tracker.rebase(frames: [1: row(y: -101)], unreadIDs: [1])

        XCTAssertTrue(tracker.processScroll(
            frames: [1: row(y: -110)], viewport: viewport, unreadIDs: [1],
            at: 1, offsetDelta: 9, userInitiated: true
        ).isEmpty)
    }

    func testRepeatedSidebarLayoutRebasesResumeNormalScrollover() {
        var tracker = qualifiedTracker()

        tracker.rebase(frames: [1: CGRect(x: 200, y: -10, width: 600, height: 100)], unreadIDs: [1])
        tracker.rebase(frames: [1: row(y: -10)], unreadIDs: [1])

        XCTAssertEqual(tracker.processScroll(
            frames: [1: row(y: -101)], viewport: viewport, unreadIDs: [1],
            at: 1, offsetDelta: 50, userInitiated: true
        ), [1])
    }

    func testProgrammaticAndScrollbarChangesDoNotMark() {
        var tracker = qualifiedTracker()
        XCTAssertTrue(tracker.processScroll(
            frames: [:], viewport: viewport, unreadIDs: [1],
            at: 1, offsetDelta: 40, userInitiated: false
        ).isEmpty)

        tracker = qualifiedTracker()
        XCTAssertTrue(tracker.processScroll(
            frames: [:], viewport: viewport, unreadIDs: [1],
            at: 1, offsetDelta: 90, userInitiated: true
        ).isEmpty)
    }

    func testNavigationAndExternalSelectionResetExposure() {
        var tracker = qualifiedTracker()
        tracker.reset() // Feed or filter change.
        XCTAssertTrue(tracker.processScroll(
            frames: [:], viewport: viewport, unreadIDs: [1],
            at: 1, offsetDelta: 40, userInitiated: true
        ).isEmpty)

        tracker = qualifiedTracker()
        tracker.reset() // External navigation, including the same route.
        XCTAssertTrue(tracker.processScroll(
            frames: [:], viewport: viewport, unreadIDs: [1],
            at: 1, offsetDelta: 40, userInitiated: true
        ).isEmpty)
    }

    func testReopeningCanQualifyUsingRetainedFramesBeforeFirstScroll() {
        var tracker = ScrolloverExposureTracker()
        let visibleFrames = [Int64(1): row(y: 0)]
        tracker.observe(frames: visibleFrames, viewport: viewport, unreadIDs: [1], at: 0)
        tracker.reset() // Popover closed.

        tracker.observe(frames: visibleFrames, viewport: viewport, unreadIDs: [1], at: 1)
        tracker.observe(frames: visibleFrames, viewport: viewport, unreadIDs: [1], at: 1.8)

        XCTAssertEqual(tracker.processScroll(
            frames: [1: row(y: -101)], viewport: viewport, unreadIDs: [1],
            at: 2, offsetDelta: 50, userInitiated: true
        ), [1])
    }

    @MainActor
    func testScrolloverPreferencePersistsAndDefaultsToEnabled() throws {
        let suiteName = "FluxBarTests.ScrolloverPreference.\(UUID().uuidString)"
        let defaults = try XCTUnwrap(UserDefaults(suiteName: suiteName))
        defer { defaults.removePersistentDomain(forName: suiteName) }

        XCTAssertTrue(ScrolloverPreferences.isEnabled(in: defaults))

        ScrolloverPreferences.setEnabled(false, in: defaults)

        XCTAssertFalse(ScrolloverPreferences.isEnabled(in: defaults))
    }

    private func qualifiedTracker() -> ScrolloverExposureTracker {
        var tracker = ScrolloverExposureTracker()
        tracker.observe(frames: [1: row(y: 0)], viewport: viewport, unreadIDs: [1], at: 0)
        tracker.observe(frames: [1: row(y: 0)], viewport: viewport, unreadIDs: [1], at: 0.8)
        return tracker
    }

    private func row(y: CGFloat) -> CGRect {
        CGRect(x: 0, y: y, width: 600, height: 100)
    }
}
