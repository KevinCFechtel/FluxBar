# FluxBar

FluxBar is an open-source macOS menu bar app for [Miniflux](https://miniflux.app/). It provides a compact view of unread articles in the menu bar, helping you keep track of new posts without opening a separate browser window.

The project is released under the BSD 3-Clause License. Contributions, bug reports, and suggestions for improvement are welcome.

## Features

- Unread Miniflux articles directly in the macOS menu bar
- Article previews with title, feed, publication date, text, and image
- Sorting by newest or oldest articles
- Open articles in the default browser and mark them as read
- Support for macOS light and dark appearances
- English and German localization based on the system or per-app language
- Optional launch at login
- Optional startup notification
- Native settings for the server URL and API key
- Secure, persistent credential storage in the macOS Keychain

## Requirements

- macOS 15 or later
- Go 1.25.1 or later
- Rust (for experimental Rust core builds)
- Xcode Command Line Tools
- An accessible Miniflux instance with an API key

The Rust core may require additional targets for cross-compilation, for
example `x86_64-apple-darwin`. If `Build/build-rust-core.sh` reports a
missing target, install it with `rustup target add <target>`.

## Build the App

The default developer build uses the Go core, which remains the production/reference implementation:

```bash
./Build/build.sh              # default: Go core
FLUX_CORE=go ./Build/build.sh # explicit Go core
```

An experimental Rust core build is available for development and compatibility testing:

```bash
FLUX_CORE=rust ./Build/build.sh
```

The Rust core implements the current public operation surface but remains experimental. Phase 10 found unresolved service-lock/deadline parity risks, so Go remains the default and production/reference implementation.

Both build paths produce `dist/FluxBar.app`. On first launch, enter the Miniflux URL and API key through “Settings…” in the menu. Credentials do not need to be stored in the source code or any build files.

## Create a Release

A signed and notarized release requires an Apple Developer ID and a `notarytool` profile stored locally in the Keychain:

```bash
xcrun notarytool store-credentials FluxBar-notary
cp Build/.env.example Build/.env
./Build/release-go.sh
```

`Build/.env` is ignored by Git. The completed archive is written to `dist/release/`.

## Development

Go core:

```bash
cd go-core
go test ./...
go vet ./...
```

Rust core:

```bash
cargo fmt --manifest-path rust-core/Cargo.toml --check
cargo check --manifest-path rust-core/Cargo.toml
cargo test --manifest-path rust-core/Cargo.toml
```

Changes should be covered by appropriate tests. Pull requests should focus on a clearly described problem or feature.

## License

FluxBar is released under the [BSD 3-Clause License](LICENSE).
