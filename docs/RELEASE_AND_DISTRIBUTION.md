# FluxBar Desktop Release and Distribution

## Intended macOS Channels

The macOS application may be distributed through:

-   Mac App Store
-   signed/notarized GitHub releases
-   Homebrew Cask

These channels can coexist.

## Current Native Build

`Build/build.sh` builds the SwiftUI/AppKit target through Xcode,
compiles and links the Go core as a C archive, copies `FluxBar.app` to
`dist`, and applies an ad-hoc signature for local execution. The current
top-level build is architecture-specific even though the lower-level
Go-core build script can combine multiple architectures.

`Build/release.sh` implements the direct-distribution path as a
Developer ID-signed, hardened, notarized ZIP. It verifies the signature,
notarization ticket, Gatekeeper assessment, and re-extracted final
artifact. DMG packaging, universal release artifacts, Mac App Store
packaging, Homebrew publication, and release CI remain future work.

## Identity and Compatibility

The user-facing name is FluxBar. The app bundle/executable name
`FluxBar.app`, bundle identifier `dev.kevincfechtel.FluxBar`, and
current Keychain service retain the FluxBar identity for compatibility.
The native minimum system version is macOS 13.

Native app metadata lives in `macos/FluxBar/Info.plist`, while the
release script currently reads its version from `Build/Info.plist`.
Keeping these two version sources synchronized is an outstanding release
maintenance concern.

## Mac App Store

The App Store build must follow Apple's sandbox/signing/review
requirements.

The product should remain useful enough as a native Miniflux Menu Bar
companion to stand on its own rather than behaving as a trivial list of
links.

The native popover, synchronized read/star state, filtering/navigation,
previews/metadata, background sync, global shortcuts, Spotlight, and App
Intents provide the current standalone utility. Planned notifications
and podcast capabilities will strengthen it further.

## Direct Distribution

Direct GitHub releases should use the appropriate Developer ID signing
and Apple notarization process.

ZIP or DMG packaging can be used as appropriate.

## Homebrew

A Homebrew Cask can point to the official direct release artifact.

Initially, a project-owned tap may be used.

If the project later satisfies current Homebrew acceptance/notability
requirements, it may be proposed for the official Homebrew Cask
repository so users can install without a custom tap.

Always revalidate current Homebrew and Apple requirements before release
work because external policies change.

## Build Separation

Mac App Store and direct/Homebrew distribution may require separately
signed builds.

Do not assume one signed artifact can be used interchangeably across all
channels.

## Release Safety

Signing/notarization credentials and CI workflows are
security-sensitive.

Prefer minimal dependencies and narrowly scoped credentials in critical
release workflows.

The current direct ZIP path is documented above. App Store, Homebrew,
and CI details should be added only when established in the repository.

## Core Migration and Release Safety

During the Go-to-Rust compatibility migration, the established
production release path remains Go-backed unless a migration phase
explicitly changes the default.

Rust build artifacts may be produced for development and compatibility
testing, but adding Rust must not silently alter signing, notarization,
archive layout, release automation, or distribution channels.

When Rust eventually becomes the default core, keep an explicit
Go-backed fallback build for the proving period described in
`RUST_CORE_MIGRATION.md`.
