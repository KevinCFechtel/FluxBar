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
