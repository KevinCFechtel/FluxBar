import AppKit
import ImageIO
import SwiftUI

private final class LoadedImage: @unchecked Sendable {
    let value: NSImage

    init(_ value: NSImage) {
        self.value = value
    }
}

actor ImagePipeline {
    static let shared = ImagePipeline()
    private let session: URLSession
    private let images = NSCache<NSURL, NSImage>()
    private var loads: [URL: Task<LoadedImage, Error>] = [:]

    private init() {
        let configuration = URLSessionConfiguration.default
        configuration.urlCache = URLCache(
            memoryCapacity: 32 * 1024 * 1024,
            diskCapacity: 256 * 1024 * 1024,
            diskPath: "FluxBarThumbnails"
        )
        configuration.requestCachePolicy = .returnCacheDataElseLoad
        configuration.timeoutIntervalForRequest = 12
        session = URLSession(configuration: configuration)
        images.totalCostLimit = 64 * 1024 * 1024
    }

    func image(at url: URL) async throws -> NSImage {
        if let image = images.object(forKey: url as NSURL) {
            return image
        }
        if let load = loads[url] {
            return try await load.value.value
        }

        let load = Task { [session] in
            let (data, response) = try await session.data(from: url)
            guard data.count <= 10 * 1024 * 1024 else { throw URLError(.dataLengthExceedsMaximum) }
            if let response = response as? HTTPURLResponse,
               let mime = response.mimeType,
               !mime.hasPrefix("image/") {
                throw URLError(.cannotDecodeContentData)
            }
            guard let source = CGImageSourceCreateWithData(data as CFData, nil),
                  let thumbnail = CGImageSourceCreateThumbnailAtIndex(source, 0, [
                      kCGImageSourceCreateThumbnailFromImageAlways: true,
                      kCGImageSourceCreateThumbnailWithTransform: true,
                      kCGImageSourceShouldCacheImmediately: true,
                      kCGImageSourceThumbnailMaxPixelSize: 520,
                  ] as CFDictionary)
            else { throw URLError(.cannotDecodeContentData) }
            return LoadedImage(NSImage(
                cgImage: thumbnail,
                size: NSSize(width: thumbnail.width, height: thumbnail.height)
            ))
        }
        loads[url] = load
        defer { loads[url] = nil }

        let image = try await load.value.value
        if let representation = image.representations.first {
            images.setObject(
                image,
                forKey: url as NSURL,
                cost: representation.pixelsWide * representation.pixelsHigh * 4
            )
        } else {
            images.setObject(image, forKey: url as NSURL)
        }
        return image
    }
}

struct ThumbnailView: View {
    let url: URL?
    let accessibilityLabel: String
    var width: CGFloat = 120
    var height: CGFloat = 84
    var cornerRadius: CGFloat = 7
    @State private var image: NSImage?

    var body: some View {
        ZStack {
            RoundedRectangle(cornerRadius: cornerRadius)
                .fill(Color.secondary.opacity(0.10))
            if let image {
                Image(nsImage: image)
                    .resizable()
                    .scaledToFill()
            } else {
                Image(systemName: "photo")
                    .font(.title2)
                    .foregroundStyle(.tertiary)
            }
        }
        .frame(width: width, height: height)
        .clipShape(RoundedRectangle(cornerRadius: cornerRadius))
        .accessibilityLabel(accessibilityLabel)
        .task(id: url) {
            image = nil
            guard let url else { return }
            image = try? await ImagePipeline.shared.image(at: url)
        }
    }
}

@MainActor
final class FeedIconLoader: ObservableObject {
    @Published private(set) var regular: NSImage?
    @Published private(set) var dark: NSImage?
    private var loaded = false

    func load(feedID: Int64, feedName: String) async {
        guard !loaded else { return }
        loaded = true
        let request = CoreRequest(operation: "feed_icon", feedID: feedID, feedName: feedName)
        let response = try? await Task.detached(priority: .utility) {
            try GoCore.request(request)
        }.value
        if let data = response?.icon?.regular { regular = NSImage(data: data) }
        if let data = response?.icon?.dark { dark = NSImage(data: data) }
    }
}

struct FeedIconView: View {
    let feedID: Int64
    let feedName: String
    let accessibilityLabel: String
    @Environment(\.colorScheme) private var colorScheme
    @StateObject private var loader = FeedIconLoader()

    var body: some View {
        Group {
            if let image = colorScheme == .dark ? (loader.dark ?? loader.regular) : loader.regular {
                Image(nsImage: image).resizable().scaledToFit()
            } else {
                Image(systemName: "dot.radiowaves.left.and.right")
                    .foregroundStyle(.secondary)
            }
        }
        .frame(width: 16, height: 16)
        .accessibilityLabel(accessibilityLabel)
        .task { await loader.load(feedID: feedID, feedName: feedName) }
    }
}
