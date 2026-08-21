import XCTest

final class RoutingTests: XCTestCase {
    func testBrowseRoutesMapToExistingSelectionModel() {
        XCTAssertEqual(NavigationRoute.all.selection, .all)
        XCTAssertEqual(NavigationRoute.starred.selection, .starred)
        XCTAssertEqual(NavigationRoute.category(7).selection, .category(7))
        XCTAssertEqual(NavigationRoute.feed(9).selection, .feed(9))
        XCTAssertNil(NavigationRoute.article(id: 11, url: nil).selection)
    }

    func testSpotlightIdentifiersResolveToSharedRoutes() {
        XCTAssertEqual(NavigationRoute(searchableIdentifier: "feed:42"), .feed(42))
        XCTAssertEqual(NavigationRoute(searchableIdentifier: "category:7"), .category(7))
        XCTAssertNil(NavigationRoute(searchableIdentifier: "article:11"))
        XCTAssertNil(NavigationRoute(searchableIdentifier: "feed:not-a-number"))
    }
}
