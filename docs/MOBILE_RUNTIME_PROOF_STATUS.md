# Mobile Runtime Proof — Phase 1A/1B/1C Status

This document records the result of **Phase 1A: Reproducible target and dependency
preflight**, **Phase 1B: iOS runtime host proof**, and **Phase 1C: Android runtime
host proof** from `docs/MOBILE_RUNTIME_PROOF_CONTRACT.md`. It is an evidence
report, not a product decision. No native FluxNews application, mobile schema,
binding technology, or production API has been created.

## Scope of this document

Phase 1A answers:

1. Which mobile Rust targets are required.
2. Which targets build successfully today.
3. Which current Rust dependencies are mobile-compatible.
4. Which dependencies require configuration or later replacement.
5. What build artifacts are produced for iOS and Android.
6. What minimal build scripts should exist for later Phase 1B/1C host integration.
7. Which remaining blockers must be resolved before the runtime proof can continue.

Phase 1B answers:

1. Whether the existing C/JSON ABI works on iOS.
2. Whether the Rust core can be packaged as an iOS XCFramework.
3. Whether the core's SQLite, threading, panic boundary, and HTTPS/TLS behavior
   function on the iOS simulator.
4. Whether the iOS trust-store blocker identified in Phase 1A can be reproduced
   and safely fixed.

Phase 1C answers:

1. Whether the existing C/JSON ABI works on Android through a JNI shim.
2. Whether the Rust core can be packaged as loadable Android `.so` libraries.
3. Whether `rustls-platform-verifier` can be initialized from an Android
   application `Context` and used for system-trust TLS.
4. Whether the core's SQLite, threading, panic boundary, and HTTPS/TLS behavior
   function on an Android runtime.

Physical-device lifecycle validation remains intentionally deferred to Phase 1I.

---

## Mobile Rust target matrix

| Platform | Environment | Rust target | Required now? | Reason |
| -------- | ----------- | ----------- | ------------- | ------ |
| iOS | Physical device | `aarch64-apple-ios` | Yes | FluxNews `IPHONEOS_DEPLOYMENT_TARGET = 17.0`; `SUPPORTED_PLATFORMS = iphoneos iphonesimulator`; modern iPhones/iPads are arm64. |
| iOS | Apple Silicon simulator | `aarch64-apple-ios-sim` | Yes | Same project settings; current development host and CI are Apple Silicon. |
| iOS | Intel simulator | `x86_64-apple-ios` | No | FluxNews does not pin architectures; Xcode Debug uses `ONLY_ACTIVE_ARCH = YES`. Intel execution host is not a current product commitment. Optional with `BUILD_INTEL_SIMULATOR=1`. |
| Android | 64-bit ARM device | `aarch64-linux-android` | Yes | FluxNews `abiCodes` includes `arm64-v8a`; physical Android arm64 is primary device target. |
| Android | 64-bit x86 emulator | `x86_64-linux-android` | Yes | FluxNews `abiCodes` includes `x86_64`; emulator instrumentation is required. |
| Android | 32-bit ARM device | `armv7-linux-androideabi` | Yes (artifact) | FluxNews `abiCodes` includes `armeabi-v7a` and releases F-Droid APKs for it. Hardware runtime is recommended but not a Phase 1A gate. |

Source evidence:

- FluxNews iOS: `IPHONEOS_DEPLOYMENT_TARGET = 17.0`, `SUPPORTED_PLATFORMS = "iphonesimulator iphoneos"`, `ONLY_ACTIVE_ARCH = YES` in Debug (`FluxNews/ios/Runner.xcodeproj/project.pbxproj`).
- FluxNews Android: `minSdkVersion 29`, `compileSdk 36`, `ndkVersion = flutter.ndkVersion`, `abiCodes = ["x86_64": 1, "armeabi-v7a": 2, "arm64-v8a": 3]` (`FluxNews/android/app/build.gradle`).

`i686-linux-android` is not included because FluxNews releases no 32-bit x86 ABI.

---

## Toolchain prerequisites

### Common

- Rust 1.98.0 (stable-aarch64-apple-darwin toolchain used here).
- `cargo`, `rustup`.

### iOS

- Xcode 26.6 or later with iOS 26.5 SDK (or any SDK that can target iOS 17.0).
- `xcrun`, `xcodebuild`, `lipo`.
- Rust targets: `aarch64-apple-ios`, `aarch64-apple-ios-sim`. Optional: `x86_64-apple-ios`.

Installed and verified on this machine:

```text
Xcode 26.6 / Build version 17F113
iOS 26.5 SDK (iphoneos26.5, iphonesimulator26.5)
rustup target list --installed includes aarch64-apple-ios, aarch64-apple-ios-sim
```

### Android

- Android SDK with API 29+.
- Android NDK 28.2.13676358 (the version pinned by the audited FluxNews Flutter gitlink) or compatible.
- NDK LLVM toolchain including `llvm-ar` and `*-linux-android29-clang` wrappers.
- Rust targets: `aarch64-linux-android`, `x86_64-linux-android`, `armv7-linux-androideabi`.

Installed and verified on this machine:

```text
ANDROID_HOME=/Users/kevinfechtel/Library/Android/sdk
NDK: /Users/kevinfechtel/Library/Android/sdk/ndk/28.2.13676358
Clang wrappers present for aarch64-linux-android29, x86_64-linux-android29, armv7a-linux-androideabi29
```

`sdkmanager` is not required by the build scripts and is not installed on this
machine.

---

## Build scripts

Two new scripts were added under `Build/`:

- `Build/build-rust-ios.sh [output-dir] [profile]` — validates Xcode SDKs and Rust targets, builds required iOS archives, verifies architectures and exported symbols, copies the unchanged header, and writes `manifest.json`.
- `Build/build-rust-android.sh [output-dir] [profile]` — validates NDK, Rust targets, and clang/AR tools, builds required Android archives, verifies ELF architectures and exported symbols, copies the unchanged header, and writes `manifest.json`.

Both scripts:

- fail clearly and print exact remediation commands;
- do **not** run `rustup target add`, `sdkmanager`, or any package manager;
- do **not** sign, package a full application, or create bindings;
- do **not** depend on the FluxBar macOS Xcode project.

Environment variables:

- iOS: `IPHONEOS_DEPLOYMENT_TARGET` (default 17.0); `BUILD_INTEL_SIMULATOR=1` to include `x86_64-apple-ios`.
- Android: `ANDROID_HOME`, `FLUX_ANDROID_NDK`, `FLUX_ANDROID_API` (default 29).

---

## iOS build result

Commands run:

```sh
./Build/build-rust-ios.sh .build/mobile/ios release
```

Artifacts:

```text
.build/mobile/ios/
    device-arm64/libfluxcore.a        aarch64-apple-ios
    simulator-arm64/libfluxcore.a     aarch64-apple-ios-sim
    libfluxcore.h                     unchanged C header
    manifest.json                     build metadata
```

| Target | Size | SHA-256 |
| ------ | ---- | ------- |
| `aarch64-apple-ios` | 82,415,032 bytes | `f7b3052fa4ea622f106ce3c630fdc91b334638fdb81558d78e8623f3288f0d39` |
| `aarch64-apple-ios-sim` | 82,364,576 bytes | `b2cb1f886be272a543720f484d45e0f03e74ae542a2bc9d2290810ca6b8e38ba` |

Verification:

- `lipo -verify_arch arm64` passes for both slices.
- Rust `llvm-nm` confirms exported symbols `_FluxCoreRequest` and `_FluxCoreFree`.
- No AppKit, Carbon, Metal, or other macOS-only framework symbols are present in undefined symbols.
- A minimal iOS simulator smoke C program links successfully against the arm64 simulator archive with `-framework CoreFoundation -framework Security`.
- Clean build time for the first iOS target was approximately 1 minute 40 seconds; subsequent targets reuse dependencies and complete in under one second.

XCFramework packaging is intentionally not created in Phase 1A; it belongs to
Phase 1C.

---

## Android build result

Commands run:

```sh
./Build/build-rust-android.sh .build/mobile/android release
```

Artifacts:

```text
.build/mobile/android/
    arm64-v8a/libfluxcore.a            aarch64-linux-android
    x86_64/libfluxcore.a               x86_64-linux-android
    armeabi-v7a/libfluxcore.a          armv7-linux-androideabi
    libfluxcore.h                      unchanged C header
    manifest.json                      build metadata
```

| Target | Size | SHA-256 |
| ------ | ---- | ------- |
| `aarch64-linux-android` | 93,615,878 bytes | `1f37f0acc24e092188a73c21d1edaa63003c9977101bf03c1d81b6123548690e` |
| `x86_64-linux-android` | 97,622,350 bytes | `10418b6281c69dab7487b005d43cdbf640b04ee78e63358ab89e95520f30dbe4` |
| `armv7-linux-androideabi` | 81,955,540 bytes | `de879791fea6fbc6149a14db49587f3a9cc0124eb8ddc5a2eb3a762b3fbaa717` |

Verification:

- NDK `llvm-readelf` confirms correct ELF machines: AArch64, X86-64, ARM.
- Rust `llvm-nm` confirms exported symbols `FluxCoreRequest` and `FluxCoreFree` for every ABI.
- Bundled SQLite (`libsqlite3-sys`) compiled successfully for all three ABIs.
- No unexpected host-library dependencies are present in the static archives (static archives have no `DT_NEEDED` entries).
- Clean build time for the first Android target was approximately 1 minute 40 seconds; subsequent targets reuse dependencies and complete in under one second.

JNI `.so` packaging is intentionally not created in Phase 1A; it belongs to
Phase 1E.

---

## Dependency portability audit

Direct and relevant transitive dependencies were classified for Phase 1A
(build/link/package portability), not for final runtime behavior.

| Dependency/area | iOS | Android | Classification | Notes |
| --------------- | --- | ------- | -------------- | ----- |
| `serde`, `serde_json` | MOBILE_SAFE | MOBILE_SAFE | MOBILE_SAFE | Pure Rust; builds on all targets. |
| `sha2`, `base64` | MOBILE_SAFE | MOBILE_SAFE | MOBILE_SAFE | Pure Rust; no platform configuration. |
| `time` | MOBILE_SAFE | MOBILE_SAFE | MOBILE_SAFE | Pure Rust over system time. |
| `html5ever`, `markup5ever_rcdom` | MOBILE_SAFE | MOBILE_SAFE | MOBILE_SAFE | Pure Rust parser; linked in all archives. |
| `url`, IDNA/ICU data | MOBILE_SAFE | MOBILE_SAFE | MOBILE_SAFE | Pure Rust; contributes code/data size. |
| `image` | MOBILE_SAFE | MOBILE_SAFE | MOBILE_SAFE | Rust codecs only; compiled for every target. |
| `resvg`/`usvg`/`tiny-skia` | MOBILE_SAFE | MOBILE_SAFE | MOBILE_SAFE | C-free configuration; linked successfully. |
| `log` facade | MOBILE_SAFE | MOBILE_SAFE | MOBILE_SAFE | Portable facade. Non-macOS builds use the no-op backend in `rust-core/src/logging/mod.rs`. |
| `oslog` | Not compiled | Not compiled | MOBILE_SAFE | Gated to `cfg(target_os = "macos")`; does not enter iOS/Android artifacts. |
| `rusqlite` + bundled `libsqlite3-sys` | REQUIRES_CONFIGURATION | REQUIRES_CONFIGURATION | REQUIRES_CONFIGURATION | Requires Apple/NDK C toolchain. All targets built successfully; runtime SQLite/WAL behavior deferred to Phase 1C–F. |
| `ureq` blocking client | REQUIRES_CONFIGURATION | REQUIRES_CONFIGURATION | REQUIRES_CONFIGURATION | Compiles; host threading and cancellation limits are runtime questions. |
| `rustls` + `rustls-native-certs` | REQUIRES_CONFIGURATION | REQUIRES_CONFIGURATION | REQUIRES_CONFIGURATION | Compiles and links. **iOS trust-store behavior is a deferred blocker:** `rustls-native-certs` 0.7.3 uses the Unix backend on iOS (not Security.framework), which reads `/etc/ssl/cert.pem` paths that do not exist on iOS. Runtime TLS viability must be resolved in Phase 1G. Android uses `openssl-probe` to read `/system/etc/security/cacerts`; runtime test required. |
| `ring` | REQUIRES_CONFIGURATION | REQUIRES_CONFIGURATION | REQUIRES_CONFIGURATION | Cross-compiles for all targets; uses NDK clang on Android. |
| Complete transitive graph | Unknown until runtime | Unknown until runtime | UNKNOWN_NEEDS_RUNTIME_TEST | Cargo metadata alone does not prove mobile linking. All required archives linked, but TLS, SQLite, and thread behavior need host execution. |

### Resolved contract discrepancy

The initial Phase 1A audit found that the contract described `native-tls` while
the implementation selected rustls. Phase 1B subsequently configured the shared
agent with `rustls-platform-verifier`, and
`docs/MOBILE_RUNTIME_PROOF_CONTRACT.md` now audits that exact stack. The
remaining `native-certs` feature constructs ureq's discarded default TLS
configuration before the explicit verifier is installed; its startup effect or
removal remains a Phase 1 evidence item, not the active handshake verifier.

---

## Platform-specific code audit

Findings in `rust-core/src/**`:

| Location | Finding | Classification |
| -------- | ------- | -------------- |
| `rust-core/src/logging/mod.rs` | `oslog` backend gated to `cfg(target_os = "macos")`; non-macOS uses no-op logger. | BUILD_SAFE |
| `rust-core/src/runtime.rs:459-502` | `default_database_path()` resolves `~/Library/Application Support/FluxBar` on macOS and returns an explicit error on `cfg(not(target_os = "macos"))`. | RUNTIME_PLATFORM_ASSUMPTION / MUST_BE_ABSTRACTED_BEFORE_MOBILE_RUNTIME |
| `rust-core/src/runtime.rs:486` | `std::os::unix::fs::PermissionsExt` used inside `cfg(unix)`. iOS and Android are Unix-like and compile this; Android file-mode semantics are platform-specific. | BUILD_SAFE / RUNTIME_PLATFORM_ASSUMPTION |
| `rust-core/src/persistence/store.rs:1066,1795` | `std::os::unix::fs::PermissionsExt` in tests only. | BUILD_SAFE |
| `rust-core/Cargo.toml:48-50` | `oslog` dependency gated to `cfg(target_os = "macos")`. | BUILD_SAFE |

No macOS-only Security.framework, AppKit, or other Apple framework assumptions
were found in domain code. TLS framework linkage is pulled in by the `security-framework`
crate only on macOS and does not appear in the iOS device archive.

### Database path conclusion

Mobile builds compile cleanly despite the macOS path logic because
`default_database_path()` returns an error on non-macOS. The mobile runtime will
need an explicit database path supplied by the native host. `Store::open` already
accepts an explicit path, so no persistence primitive change is required for
Phase 1A. Final path abstraction design is deferred to Phase 2/3.

---

## Release profile

`rust-core/Cargo.toml`:

```toml
[profile.release]
opt-level = 3
lto = true
```

Assessment: reasonable for mobile proof builds. `lto = true` produces large
static archives (~80–97 MB) because all object code is included; final app size
after link-time dead stripping will be smaller. Panic mode is the default
(`unwind`), which satisfies the Phase 1 panic-containment requirement. No
profile changes were made.

---

## Security review

Phase 1A did **not** introduce:

- disabled TLS verification;
- HTTP fallback;
- embedded credentials;
- production mobile database paths;
- excessive file permissions;
- exported Android components;
- iOS entitlement changes;
- new unsafe FFI symbols beyond `FluxCoreRequest`/`FluxCoreFree`.

The existing panic guard, owned-response memory contract, and `FluxCoreFree`
semantics are preserved unchanged in mobile artifacts.

---

## Phase 1B — iOS runtime host proof

### What was implemented

1. **`mobile-runtime-proof` Cargo feature** in `rust-core/Cargo.toml`.
   - Adds a feature-gated `mobile_runtime_probe` operation to the existing
     C/JSON ABI without changing the production FluxBar operations.
   - Probe actions: `runtime_info`, `round_trip`, `sqlite_open`, `sqlite_write`,
     `sqlite_read`, `sqlite_close`, `https_get`, `thread_probe`, `panic`.
   - The probe uses its own minimal SQLite schema and never touches the FluxBar
     production schema.

2. **iOS TLS trust-store fix** (`rust-core/src/remote/miniflux.rs`).
   - Replaced `rustls-native-certs` with `rustls-platform-verifier` 0.6 for the
     shared ureq HTTP agent.
   - On iOS/macOS this uses `Security.framework`; on Android it will use the
     system Trust Manager once the JVM component is initialized; on Linux it
     falls back to `rustls-native-certs`.
   - This is the smallest change that keeps the existing ureq-based HTTP client
     while moving trust semantics to the platform verifier.

3. **iOS build/packaging scripts**.
   - `Build/build-rust-ios.sh` now accepts `CARGO_FEATURES` for feature-gated
     proof builds.
   - `Build/package-rust-ios-proof.sh` creates `FluxCore.xcframework` with a
     proper `Headers/` directory and a hand-written `module.modulemap` so Swift
     can import the C ABI.

4. **Minimal iOS proof host** under `mobile-proof/ios/`.
   - `project.yml` drives XcodeGen for reproducible project generation.
   - `FluxBarMobileProof` app target with a simple SwiftUI status view.
   - `FluxBarMobileProofTests` XCTest target covering:
     - FFI invocation and error boundaries (`runtime_info`, malformed JSON,
       unknown operation);
     - JSON round-trip and concurrent round-trip with no response crossover;
     - SQLite open/write/read/close/reopen persistence and path containment;
     - HTTPS public-root success and invalid-certificate failure;
     - Thread spawn/join coordination;
     - Intentional contained panic and process usability afterward.

### iOS simulator test results

All 12 XCTests pass on an iOS 26.5 arm64 simulator:

```sh
cd mobile-proof/ios
xcodegen generate
xcodebuild -project FluxBarMobileProof.xcodeproj -scheme FluxBarMobileProof \
  -destination 'platform=iOS Simulator,name=iPhone 11 Pro Max,OS=26.5' \
  -configuration Debug test CODE_SIGNING_ALLOWED=NO
```

Result: `Executed 12 tests, with 0 failures`.

Key observations:

- `runtime_info` reports `os: ios`, `arch: arm64`, `panicStrategy: unwind`, and
  `mobileRuntimeProofEnabled: true`.
- `https_get` against `https://httpbin.org/get` returns HTTP 200.
- `https_get` against `https://self-signed.badssl.com/` fails with a transport
  error, proving certificate validation is active and cannot be bypassed.
- The intentional `panic` probe prints a Rust backtrace to the device log but
  returns the deterministic `{"ok":false,"error":"internal error"}` JSON and
  the core remains usable for the next call.

### iOS TLS blocker resolution

The Phase 1A concern that `rustls-native-certs` does not use
Security.framework on iOS is **resolved** for the shared HTTP agent by adopting
`rustls-platform-verifier`. The iOS simulator test suite now passes real
public-root HTTPS requests and rejects invalid certificates, so the blocker is
no longer a Phase 1B/1G risk.

Android trust-store behavior remains unverified and is still a Phase 1E/1G
concern.

### Phase 1B artifact layout

```text
.build/mobile/ios-proof/
    device-arm64/libfluxcore.a
    simulator-arm64/libfluxcore.a
    Headers/libfluxcore.h
    Headers/module.modulemap
    manifest.json
    FluxCore.xcframework

mobile-proof/ios/
    project.yml
    FluxBarMobileProof/
        FluxBarMobileProofApp.swift
        ContentView.swift
        FluxCore.swift
        Info.plist
    FluxBarMobileProofTests/
        FluxBarMobileProofTests.swift
    FluxBarMobileProof.xcodeproj   # generated by XcodeGen, gitignored
```

The `.build/mobile/` tree is gitignored. The Xcode project is generated from
`project.yml` with XcodeGen. The generated `.xcodeproj` is gitignored; run
`xcodegen generate` in `mobile-proof/ios` after checkout to open it in Xcode.

---

## Known blockers and deferred questions

### Phase 1A blockers (must be resolved before 1B)

None. All required targets build and the build scripts are functional.

### Phase 1B blockers (must be resolved before 1C/1D acceptance)

None for the iOS simulator scope. Physical-device validation is deferred to
Phase 1I and is recorded as a residual risk, not a blocker.

### Phase 1G blockers (runtime TLS)

- **iOS trust store:** RESOLVED for the shared HTTP agent by
  `rustls-platform-verifier`. The iOS simulator passes public-root HTTPS and
  invalid-certificate rejection tests.
- **Android trust store:** RESOLVED for the shared HTTP agent by wiring
  `rustls-platform-verifier` JVM initialization through the feature-gated JNI
  initializer. The Android emulator passes public-root HTTPS and
  invalid-certificate rejection tests using the pinned verifier AAR.

### Intentionally deferred to later phases

- Controlled HTTPS trust matrix on physical iOS and Android (Phase 1G).
- Concurrency, panic, and ownership stress tests beyond the baseline coverage
  added here (Phase 1H).
- Physical-device lifecycle validation (Phase 1I).
- Final PASS/NOT PASSED decision (Phase 1J).
- Binding decision (C/JSON, typed C, UniFFI) — Phase 5.
- Mobile schema and persistence API — Phase 2/3.
- Mobile logging backend decisions — after Phase 1B.

---

## Phase 1A artifact layout

```text
.build/mobile/
    ios/
        device-arm64/libfluxcore.a
        simulator-arm64/libfluxcore.a
        Headers/libfluxcore.h
        Headers/module.modulemap
        manifest.json
    android/
        arm64-v8a/libfluxcore.a
        x86_64/libfluxcore.a
        armeabi-v7a/libfluxcore.a
        Headers/libfluxcore.h
        Headers/module.modulemap
        manifest.json
```

The header directory now includes a `module.modulemap` for Swift import. The
iOS proof build additionally produces `FluxCore.xcframework` under
`.build/mobile/ios-proof/` (see Phase 1B section above).

This layout is separate from the macOS FluxBar release directories (`dist/`,
`.build/DerivedData/`). It is gitignored via the existing `/.build/` rule.

---

## Regression validation

All commands succeeded:

```sh
cargo fmt --manifest-path rust-core/Cargo.toml --check
cargo check --manifest-path rust-core/Cargo.toml
cargo check --manifest-path rust-core/Cargo.toml --features mobile-runtime-proof
cargo test --manifest-path rust-core/Cargo.toml
cargo test --manifest-path rust-core/Cargo.toml --features mobile-runtime-proof
cd go-core && go test ./... && go vet ./...
./Build/test-core-parity.sh
./Build/build.sh
FLUX_CORE=rust ./Build/build.sh
FLUX_CORE=go ./Build/build.sh
git diff --check
```

iOS proof host validation:

```sh
CARGO_FEATURES=mobile-runtime-proof Build/build-rust-ios.sh .build/mobile/ios-proof release
Build/package-rust-ios-proof.sh .build/mobile/ios-proof .build/mobile/ios-proof/FluxCore.xcframework
cd mobile-proof/ios
xcodegen generate
xcodebuild -project FluxBarMobileProof.xcodeproj -scheme FluxBarMobileProof \
  -destination 'platform=iOS Simulator,name=iPhone 11 Pro Max,OS=26.5' \
  -configuration Debug test CODE_SIGNING_ALLOWED=NO
```

Result: `Executed 12 tests, with 0 failures`.

No existing tests were weakened.

---

## Phase 1C — Android runtime host proof

### What was implemented

1. **Android-specific Rust verifier initializer** (`rust-core/src/android_jni.rs`).
   - Feature-gated helper `fluxbar_mobile_proof_init_android_verifier` that calls
     `rustls_platform_verifier::android::init_hosted` with a JVM `Context`.
   - Absent from default/production artifacts; does not add a third exported
     production symbol.

2. **NDK C/C++ JNI shim** (`mobile-proof/android/app/src/main/cpp/jni-bridge.cpp`).
   - Converts Kotlin strings to UTF-8, invokes `FluxCoreRequest`/`FluxCoreFree`,
     and copies the response back to Kotlin.
   - Forwards the application `Context` to the Rust verifier initializer.
   - Links the prebuilt `libfluxcore.a` as a static dependency.

3. **CMake packaging** integrated into `Build/build-rust-android.sh`.
   - When `CARGO_FEATURES=mobile-runtime-proof`, the script builds
     `libfluxcore_mobile_probe.so` for all three ABIs using the NDK toolchain.
   - Locates the pinned `rustls-platform-verifier-android 0.1.1` AAR via
     `cargo metadata`, copies it into the output tree, and records its SHA-256.
   - Verifies ELF machines, exported JNI symbols, and `DT_NEEDED` entries.
   - C++ STL is statically linked so the APK does not depend on
     `libc++_shared.so`.

4. **Minimal Kotlin proof host** (`mobile-proof/android/`).
   - `FluxCoreBridge` loads `fluxcore_mobile_probe` and initializes the verifier.
   - `MainActivity` hosts a simple status UI with manual probe controls.
   - Uses `noBackupFilesDir` for the probe database and declares only the
     `INTERNET` permission.
   - ProGuard/R8 keep rules preserve the verifier Kotlin component.

5. **Instrumentation tests** (`FluxCoreInstrumentedTest`).
   - 12 tests covering FFI invocation, JSON round-trip, concurrent round-trip
     with no crossover, SQLite open/write/read/close/reopen, path containment,
     HTTPS public-root success, invalid-certificate rejection, thread spawn/join,
     contained panic, and background-thread invocation.

### Android build result

Command run:

```sh
CARGO_FEATURES=mobile-runtime-proof ./Build/build-rust-android.sh .build/mobile/android-proof release
```

Artifacts:

```text
.build/mobile/android-proof/
    arm64-v8a/libfluxcore.a            aarch64-linux-android
    x86_64/libfluxcore.a               x86_64-linux-android
    armeabi-v7a/libfluxcore.a          armv7-linux-androideabi
    jniLibs/arm64-v8a/libfluxcore_mobile_probe.so
    jniLibs/x86_64/libfluxcore_mobile_probe.so
    jniLibs/armeabi-v7a/libfluxcore_mobile_probe.so
    verifier/rustls-platform-verifier-0.1.1.aar
    Headers/libfluxcore.h
    Headers/module.modulemap
    manifest.json
```

| Target | `libfluxcore.a` size | `libfluxcore.a` SHA-256 | `.so` size |
| ------ | -------------------- | ----------------------- | ---------- |
| `aarch64-linux-android` | 96,891,910 bytes | `fc26453dccf7acabd66669bbaef1c003fc65afc212cb7d493279af0fcb2c3d54` | 23,208,208 bytes |
| `x86_64-linux-android` | 100,924,550 bytes | `eeafb7c37d3a3bd2161e9b882ec6ac0de52b7c550675273fbf37ae06e9092897` | 23,561,016 bytes |
| `armv7-linux-androideabi` | 84,977,576 bytes | `bb037442dbe403cdab7eb78b82dea5e00a5d8a7a7f0bc2fb8a58e1c683a57195` | 19,028,680 bytes |

| Component | Value |
| --------- | ----- |
| rustls-platform-verifier AAR | 9,287 bytes, SHA-256 `667292cadd8fa589229dd0f716541236a761f29b774930868d218175633830fd` |
| NDK | `28.2.13676358` |
| API level | 29 |

Verification:

- NDK `llvm-readelf` confirms correct ELF machines: AArch64, X86-64/AMD64, ARM.
- Rust `llvm-nm` confirms exported `FluxCoreRequest` and `FluxCoreFree` for every ABI.
- JNI `.so` exports `Java_com_fluxbar_mobileproof_FluxCoreBridge_request` and
  `Java_com_fluxbar_mobileproof_FluxCoreBridge_initVerifierNative`.
- `DT_NEEDED` checks confirm only expected Android system libraries; no OpenSSL,
  `libc++_shared.so`, or host-specific libraries.
- Release build with R8 (`:app:assembleRelease` and `:app:assembleReleaseTest`)
  succeeds; the releaseTest APK launches and loads the native library.

### Android emulator test results

All 12 instrumentation tests pass on an arm64 Android Virtual Device
(`NormalPhone`, API 37.1):

```sh
CARGO_FEATURES=mobile-runtime-proof ./Build/build-rust-android.sh .build/mobile/android-proof release
./Build/test-mobile-runtime-android.sh /Users/kevinfechtel/GitHub/FluxBar/.build/mobile/android-proof
```

Result: `Finished 12 tests on NormalPhone(AVD) - 17` with 0 failures.

Key observations:

- `runtime_info` reports `os: android`, `arch: aarch64`,
  `panicStrategy: unwind`, and `mobileRuntimeProofEnabled: true`.
- `https_get` against `https://httpbin.org/get` returns HTTP 200.
- `https_get` against `https://self-signed.badssl.com/` fails with a transport
  error, proving certificate validation is active and cannot be bypassed.
- The intentional `panic` probe returns the deterministic
  `{"ok":false,"error":"internal error"}` JSON and the core remains usable.

### x86_64 emulator and physical device notes

- Only arm64 emulator system images are installed on this machine; no x86_64
  Android emulator is available without downloading additional SDK components.
  The x86_64 ABI artifact builds, packages, and passes ELF/symbol verification.
- No physical Android arm64 device was connected during this session. The arm64
  emulator uses the same `arm64-v8a` ABI as a physical arm64 device and
  exercises the same native code path, but physical-device lifecycle behavior
  remains a recorded residual risk per the contract.

### Android TLS blocker resolution

The Phase 1A/1B concern that Android TLS behavior was unverified is **resolved**
for the shared HTTP agent by wiring `rustls-platform-verifier` JVM
initialization through the feature-gated JNI initializer. The Android emulator
now passes public-root HTTPS requests and rejects invalid certificates, so
Android TLS is no longer a Phase 1 residual risk for the current stack.

---

## Regression validation

All commands succeeded:

```sh
cargo fmt --manifest-path rust-core/Cargo.toml --check
cargo check --manifest-path rust-core/Cargo.toml
cargo check --manifest-path rust-core/Cargo.toml --features mobile-runtime-proof
cargo test --manifest-path rust-core/Cargo.toml
cargo test --manifest-path rust-core/Cargo.toml --features mobile-runtime-proof
cd go-core && go test ./... && go vet ./...
./Build/test-core-parity.sh
./Build/build.sh
FLUX_CORE=rust ./Build/build.sh
FLUX_CORE=go ./Build/build.sh
git diff --check
```

iOS proof host validation:

```sh
CARGO_FEATURES=mobile-runtime-proof ./Build/build-rust-ios.sh .build/mobile/ios-proof release
./Build/package-rust-ios-proof.sh .build/mobile/ios-proof .build/mobile/ios-proof/FluxCore.xcframework
```

Result: iOS Rust archives and XCFramework build successfully. iOS simulator
XCTest execution was not repeated in this session because `xcodegen` is not
currently on PATH; the existing 12-test pass from Phase 1B remains valid and no
iOS source was changed.

Android proof host validation:

```sh
CARGO_FEATURES=mobile-runtime-proof ./Build/build-rust-android.sh .build/mobile/android-proof release
./Build/test-mobile-runtime-android.sh /Users/kevinfechtel/GitHub/FluxBar/.build/mobile/android-proof
```

Result: `Finished 12 tests on NormalPhone(AVD) - 17` with 0 failures.

No existing tests were weakened.

---

## Next step

**PHASE 1D — iOS/Android artifact reproducibility and size baselines** is the
recommended next step, with physical-device lifecycle validation deferred to
Phase 1I as a recorded residual risk.

---

## Final independent review (2026-08-24)

This section supersedes the incremental "Next step" above while preserving the
earlier Phase 1A-1C evidence as history. The requirements were re-derived from
the implementation and runtime-feasibility objective rather than accepted from
the planning contract unchanged.

### Phase 1 decision

```text
PHASE 1 NOT PASSED
```

Build portability and representative iOS/Android execution are established,
but the current Android proof cannot provide trustworthy closure evidence:

1. `GetStringUTFChars`/`NewStringUTF` pass JNI Modified UTF-8 directly to a
   standard-UTF-8 C ABI (`mobile-proof/android/app/src/main/cpp/jni-bridge.cpp:36-61`).
2. A clean Android proof build fails its arm64 ELF check even though `file` and
   NDK `llvm-readelf` confirm a valid AArch64 `.so`; `set -o pipefail` combines
   with early-exiting `grep -q` at `Build/build-rust-android.sh:292`.
3. Gradle always registers the default JNI root and then an optional override,
   using `pickFirsts` to hide duplicates (`mobile-proof/android/app/build.gradle.kts:51-68`).
4. Neither native host has the bounded repeated-call ownership coverage required
   to close allocator/free-path evidence.
5. iOS does not yet execute the blocking SQLite and HTTPS probes from a native
   background queue; the current XCTest calls the synchronous wrapper directly.
6. The native hosts do not directly cover null request pointers or valid raw
   pointers containing invalid UTF-8 at their shim boundaries.
7. Native SQLite tests assert WAL but not the reported `synchronous` and
   `busyTimeout` configuration values.
8. Source gating is clear, but no recorded default-artifact inspection proves
   both the probe operation and Android verifier initializer are absent.

These are narrow Phase 1 closure tasks. They do not require a new production
API, schema, binding technology, transport, or Go removal.

### Contract review

| Disposition | Requirements |
| --- | --- |
| Retained | Locked mobile builds; native host execution; unchanged two-symbol C ABI; standard UTF-8; copy/free ownership; panic containment; concurrency; bundled SQLite close/reopen; sandbox paths; public-root success; invalid-certificate rejection; Android verifier initialization/AAR/R8; macOS regression suites. |
| Corrected | Reproducibility now means locked inputs and repeatable semantically equivalent builds; memory requires bounded ownership/leak evidence rather than an exact RSS formula; lifecycle requires enough initialization evidence for feasibility rather than production scheduler behavior. |
| Removed from Phase 1 | Byte-identical mobile artifacts and manifests; mandatory physical-device execution; exact device memory thresholds; lock/background and process-death product scenarios. |
| Deferred | Physical-device lifecycle/trust/file-protection; x86_64/armv7 runtime support claims; controlled TLS/timeout/DNS matrix; secure storage; background workers; widgets; product schema; migration; final bindings; supply-chain reproducible releases. |
| Added | Explicit standard-versus-Modified-UTF-8 validation; one unambiguous artifact root; clean build-script success as evidence provenance; shared-production TLS regression ownership. |

Byte-identical output is not necessary to prove runtime feasibility. This review
did not retain a complete paired clean-build hash record suitable for an
auditable reproducibility claim. Bit-identical release artifacts may be adopted
later as a supply-chain requirement, but are not a Phase 1 gate.

### Evidence matrix

| Capability | macOS | iOS Simulator | iOS Device | Android Emulator | Android Device |
| --- | --- | --- | --- | --- | --- |
| Build | PROVEN | PROVEN | BUILD ONLY | PARTIAL: arm64 ran; clean all-ABI script fails; x86_64 BUILD ONLY | BUILD ONLY arm64/armv7 |
| FFI | PROVEN | PARTIAL: null/invalid-byte shim cases absent | NOT TESTED | PARTIAL: Modified UTF-8 and boundary-test gaps | NOT TESTED |
| Memory ownership | PARTIAL | PARTIAL | NOT TESTED | PARTIAL | NOT TESTED |
| Panic containment | PROVEN | PROVEN | NOT TESTED | PROVEN | NOT TESTED |
| Threading | PROVEN | PARTIAL: no blocking background-queue test | NOT TESTED | PROVEN | NOT TESTED |
| SQLite | PROVEN | PARTIAL: configuration assertions incomplete | NOT TESTED | PARTIAL: configuration assertions incomplete | NOT TESTED |
| Persistence | PROVEN | PROVEN close/reopen | NOT TESTED | PROVEN close/reopen | NOT TESTED |
| TLS public-root | PARTIAL for current verifier | PROVEN | NOT TESTED | PROVEN arm64 | NOT TESTED |
| Invalid certificate rejection | NOT TESTED for current verifier | PROVEN | NOT TESTED | PROVEN arm64 | NOT TESTED |
| Lifecycle/reinit | PARTIAL | PARTIAL | NOT TESTED | PARTIAL | NOT TESTED |
| Release-style packaging | PARTIAL current unsigned build | PROVEN XCFramework | BUILD ONLY | PROVEN R8 host; clean artifact script currently fails | NOT TESTED |

`Android Emulator` runtime evidence is arm64 only. Neither an x86_64 emulator
nor physical Android hardware was executed. Simulator/emulator evidence is not
device evidence. For this review run, the requested JNI library and the copy in
both the debug and R8 `releaseTest` APKs had SHA-256
`9edb1cb391dd2e94fa0d83c6b40253ac50a569a9fb4cec9521bb003cd8081a8b`.
That proves this execution used the requested library, but the Gradle source-set
configuration remains unsafe when both roots exist.

### TLS architecture

```text
shared build_http_agent
          |
        ureq 2.12.1
          |
      rustls 0.23.43
          |
rustls-platform-verifier 0.6.2
       /                 \
Apple Security.framework  Android Trust Manager
 macOS / iOS               JNI Context initialization
                           + locked support AAR 0.1.1
```

`rust-core/src/remote/miniflux.rs:26-50` installs the explicit verifier used by
both production Miniflux requests and the probe. iOS simulator and Android arm64
emulator prove public-root success and self-signed rejection. Android packages
the lock-matched AAR and R8 keep rule and initializes from application context.
The host currently calls deprecated alias `init_hosted`; `init_with_env` is the
equivalent current crate API and should be used during closure cleanup.

ureq still enables `native-certs` (`rust-core/Cargo.toml:24-35`). Its default
configuration is constructed and then replaced by the explicit verifier. It is
redundant rather than the active handshake verifier; remove it with regression
tests or document its discarded initialization before release.

The current macOS compile/link and parity evidence is sufficient for Phase 1
feasibility because iOS executes the same Apple verifier path. A live public-root
request through `build_http_agent`, followed by signed/notarized live Miniflux
sync, is required before the first Rust-backed FluxBar public release, not before
Phase 2 architecture work.

### SQLite runtime assessment

Bundled SQLite runtime feasibility is sufficiently proven to design mobile
schema v1. Both representative mobile hosts create a Rust-owned database under
native-selected app-private roots, enable WAL, commit Unicode values, close,
reopen, and read them. This does not prove the mobile repository/schema,
migrations, retention, multi-process access, BGTask/WorkManager coordination,
or existing-user import. Those remain Phase 2, Phase 3, Phase 7, and Phase 10
work respectively.

### Binding assessment

The C/JSON ABI is viable for continued proof and interim prototype work after
the Android UTF-8 bridge is corrected. Swift ownership is direct; Kotlin needs a
manual JNI adapter, verifier initializer, string conversion, and artifact/AAR
packaging. Dynamic error decoding, synchronous blocking calls, no true
cancellation, serialization/copy overhead, and JNI complexity remain evidence
for the later C/JSON versus typed C versus UniFFI decision. No hard
impossibility justifies selecting or implementing a final binding now.

### Command/query and background-sync architecture

Accept this as a durable invariant:

```text
Commands mutate.
Queries return state.
Persistent Core State != Presentation State.
Synchronization != UI Refresh.
```

Phase 2 owns command/query/synchronization categories, durable/presented counter
meanings, and adoption contracts. Phase 3 implements the local repository and
queries. Phase 4 implements durable commands and sync-only outcomes plus
explicit client query and snapshot adoption. Background sync may insert or
update repository entries, remote baselines, reconciliation metadata,
completeness, and durable counters without forcing those rows or counters into
an active presented timeline. Manual refresh chooses whether
to adopt already-current local state or synchronize first; final freshness
policy remains a native product decision.

The existing FluxBar operation responses remain compatibility behavior until a
separate desktop task updates Rust, Swift callers, Go fallback policy, and
fixtures. The active-timeline stability issue should be addressed before a
Rust-backed public FluxBar release, but does not block Phase 2 design.

### Roadmap changes

| Action | Change | Justification |
| --- | --- | --- |
| KEEP | Phase 1 runtime proof objective | Correctly avoids designing a binding around an unproven runtime. |
| SHORTEN | Phase 1 physical/memory/reproducibility gate | Product and supply-chain qualification was incorrectly promoted into feasibility. |
| ADD | Narrow Phase 1 closure task | Correct UTF-8, build verification, artifact provenance, ownership stress, iOS worker execution, native edge cases, SQLite configuration assertions, and default-artifact exclusion evidence. |
| MOVE LATER | Physical lifecycle and full TLS matrix | Required during product development/release, not schema architecture. |
| ADD TO PHASE 2 | Command/query/sync categories, sync metadata, durable counter semantics | These constrain schema and interim APIs. |
| MOVE EARLIER | Source-only Flutter historical characterization | It does not depend on the target schema; mappings and import still do. |
| KEEP | Phase 3 local repository and Phase 4 sync/mutations | Correct order after stronger Phase 2 contracts. |
| SPLIT | Phase 4 synchronization from query/adoption | Required for timeline stability and background sync. |
| KEEP | Phase 5 binding decision | Useful native API experience should precede selection. |
| KEEP | Go oracle/fallback | Removal evidence is not yet sufficient. |

### Phase 2 scope after Phase 1 passes

**Objective:** Define a versioned, account-scoped mobile repository/schema and
service contract that separates durable state, commands, synchronization,
queries, and presentation before production implementation.

Phase 2 acceptance requires reviewed decisions for:

- account identity and API-key rotation;
- schema versioning, transactional migrations, future-version rejection,
  account isolation, keys, constraints, indexes, and reset semantics;
- entry/feed/category/enclosure/full-content ownership;
- effective, desired, remote-baseline, pending-revision, acknowledgement, and
  progression extensibility;
- synchronization run identity, scope/configuration fingerprint, main/starred
  completeness, capped/incomplete evidence, attempt/success/failure metadata,
  and restart behavior;
- repository-effective, remote-observed, completeness/freshness, and
  presentation-adopted counter meanings;
- deterministic cursor/query contracts and command-specific receipts;
- semantic-setting validation, storage, and migration ownership;
- transaction and in-process/concurrent-access assumptions;
- destructive reset coordination; and
- provenance-labeled, read-only characterization of historical Flutter SQLite,
  secure storage, preferences, progression, downloads, overrides, pending
  intent, and backups.

Phase 2 does not implement the production repository, live synchronization,
native clients, Flutter import/cutover, final bindings, or migration UX.

The Flutter database is an import source, not the target physical schema. Target
identity, schema, mutation/progression, settings, download, secret, and backup
contracts must stabilize before integrated migration implementation. Import must
write a new database and never mutate deployed Flutter data in place.

### Go status

Go remains deprecated for new features but retained as behavioral reference,
differential oracle, and explicit fallback. It is not an installed-base
migration source. Removal requires a reviewed fixture manifest replacing every
valuable differential suite, implementation-independent expected outputs,
standalone concurrency/deadline coverage, an agreed Rust public-release
observation criterion, zero unresolved Rust-specific correctness blockers, and
a Rust-only rollback/release plan.

### Flux monorepo recommendation

Repository consolidation is not a Phase 1 runtime or Phase 2 contract
prerequisite. After Phase 2 and before Phase 3, separately approve an
infrastructure-only transition and do not combine it with importing the
production Flutter tree. Move toward:

```text
Flux/
    apps/
        fluxbar/macos/
        fluxnews/ios/       # later native client
        fluxnews/android/   # later native client
    core/
        rust/
        go-reference/
    proofs/mobile-runtime/
        ios/
        android/
    docs/
    tooling/
```

Use the FluxBar Git history as the Flux base because it owns the shared core,
macOS client, proofs, and architecture. If `Flux` is the destination name for
this repository, a GitHub transfer/rename preserves repository metadata and
commit identity. If `Flux` already exists as a distinct repository, integrating
history is not a transfer/rename and requires a separate reviewed plan for
commits, issues, pull requests, releases, settings, and automation. Keep the existing
FluxNews repository authoritative and writable for Flutter production,
maintenance, store releases, translations, tags, issues, and existing-user
migration evidence until native replacement readiness. Do not import Flutter
history into Flux now.

Flux uses product-scoped tags/workflows/changelogs and path-scoped CI so one
issue/PR can span UI, binding, core, persistence, and sync without coupling
FluxBar and FluxNews release versions.

### Residual validation

#### Before Phase 2

- Correct the eight Phase 1 closure findings and rerun clean Android build,
  arm64 instrumentation, iOS simulator tests, full Rust/Go/parity/macOS builds,
  and `git diff --check`.

#### During native product development

- Physical iPhone and Android load/FFI/SQLite/TLS smoke.
- Foreground/background, terminate/force-stop/relaunch, lock/unlock, file
  protection, memory profiling, and verifier initialization races.
- Controlled timeout/DNS/TLS-version/hostname/user-CA policy and x86_64 emulator
  execution if that ABI remains supported.

#### Before first native mobile release

- Phase 7 cross-process/background expiration and database coordination.
- Secure storage, production schema migrations, backup policy, full TLS policy,
  release signing/R8, device performance, and existing-user migration/cutover.

#### Before first Rust-backed FluxBar public release

- Live public-root request through the shared `build_http_agent`.
- Signed/notarized Rust FluxBar live Miniflux sync, offline/restart behavior, and
  regression review of command/query timeline stability.

### Findings

#### CRITICAL

None.

#### HIGH

1. **Android artifact provenance can be ambiguous.** Evidence:
   `mobile-proof/android/app/build.gradle.kts:51-68` registers default and
   override JNI roots and suppresses duplicates with `pickFirsts`. Impact: tests
   may package a stale library while reporting the requested artifact root. The
   current APK hashes were verified, but the configuration can invalidate a
   future run. Blocks trustworthy Phase 1 evidence. Owner: Phase 1 closure.

#### MEDIUM

1. **Android string bridge violates the C ABI encoding contract.** Evidence:
   `GetStringUTFChars`, `strlen`, and `NewStringUTF` at
   `mobile-proof/android/app/src/main/cpp/jni-bridge.cpp:36-61` use Modified
   UTF-8, while `rust-core/src/ffi.rs:34-73` requires standard NUL-terminated
   UTF-8. Impact: supplementary characters and embedded NUL are not reliably
   transported; JSON escaping in existing tests masks the raw boundary. Blocks
   Phase 1. Owner: Phase 1 closure.
2. **The clean Android artifact command currently fails.** Evidence: the final
   review command stopped at `Build/build-rust-android.sh:292` although NDK
   `llvm-readelf` reported `Machine: AArch64`; `pipefail` observes the producer's
   SIGPIPE after `grep -q` exits. Impact: required all-ABI JNI artifacts cannot
   be reproduced by the supported command. Blocks Phase 1. Owner: Phase 1
   closure.
3. **Ownership stress is incomplete.** Existing tests prove basic/concurrent
   calls but not bounded repeated allocation/free behavior. Impact: allocator
   feasibility is only partial. Blocks Phase 1 until a focused automated loop
   passes. Owner: Phase 1 closure.
4. **iOS blocking-worker evidence is missing.** SQLite and HTTPS XCTest calls
   invoke the synchronous wrapper directly rather than dispatching from a native
   background queue. Impact: the wrapper's intended off-main-thread use is not
   yet executable evidence. Blocks Phase 1. Owner: Phase 1 closure.
5. **Native boundary edge cases are incomplete.** The native hosts do not
   directly exercise a null request pointer or valid invalid-UTF-8 bytes through
   their shims. Impact: controlled C-boundary error behavior is inferred from
   Rust tests rather than proven in both hosts. Blocks Phase 1. Owner: Phase 1
   closure.
6. **SQLite configuration assertions are incomplete.** Both native hosts assert
   WAL after open but do not assert the reported synchronous and busy-timeout
   values. Impact: required configuration evidence is partial. Blocks Phase 1.
   Owner: Phase 1 closure.
7. **Default-artifact exclusion is not explicit evidence.** Source `cfg` gates
   isolate the proof operation and Android initializer, but no recorded normal
   artifact inspection closes that claim. Blocks Phase 1. Owner: Phase 1
   closure.
8. **TLS coverage is representative, not a production matrix.** Only public-root
   and self-signed endpoints execute; controlled timeout/DNS/hostname/TLS
   versions and current macOS shared-agent live trust are absent. Impact: does
   not block feasibility, but blocks corresponding public-release claims.
   Owner: native product phases and FluxBar release hardening.
9. **Android verifier initialization is convention-based.** The unsynchronized
   flag and callable `request` at
   `mobile-proof/android/app/src/main/kotlin/com/fluxbar/mobileproof/FluxCoreBridge.kt:18-50`
   permit races or HTTPS before initialization. Impact: proof setup passes, but
   production initialization must enforce ordering. Does not block Phase 1.
   Owner: final Android host/binding phase.
10. **Lifecycle evidence is same-process only.** The Android "lifecycle" test is
   a background-thread call (`FluxCoreInstrumentedTest.kt:215-223`); neither
   host proves process death/relaunch or physical-device behavior. Impact:
   product lifecycle remains open. Does not block Phase 1 after contract
   correction. Owner: native development and Phase 7.

#### LOW

- `native-certs` is redundant under the explicit verifier.
- Build manifests include timestamps and an absolute NDK root; useful local
  evidence, but unsuitable as normalized release manifests.
- Historical Phase 1 subphase labels differ between the contract and status;
  this final section is authoritative.

#### INFORMATIONAL

- The final review rebuilt iOS archives/XCFramework and reran all 12 simulator
  tests successfully despite `xcodegen` being unavailable; a previously
  generated ignored local Xcode project was sufficient. This does not prove a
  clean checkout can regenerate the project without installing `xcodegen`.
- Android arm64 instrumentation and R8 release/releaseTest builds passed from
  the existing current-revision artifact set, while the clean artifact script
  defect prevented regenerating the complete set under a new output root.

### Recommended next step

```text
PHASE 1 CLOSURE - FIX AND RE-RUN THE EIGHT RUNTIME EVIDENCE BLOCKERS
```

After independent PASS:

```text
DEFINE PHASE 2 MOBILE CONTRACTS
```

Then:

```text
PHASE 2 — VERSIONED MOBILE REPOSITORY / SCHEMA ARCHITECTURE
```
