# Phase 1 Mobile Runtime Proof Contract

## Purpose

This document defines the implementation and acceptance contract for Phase 1 of
`SHARED_RUST_CORE_ROADMAP.md`. It answers one question:

> What is the smallest useful experiment that proves the current Rust core can
> be packaged, loaded, invoked, persisted, and used safely on real iOS and
> Android runtimes before a final mobile binding or FluxNews data model is
> selected?

This is a planning contract, not a Phase 1 implementation. It introduces no
Rust feature, binding, mobile project, schema, dependency, or build script.

The document relationship is:

```text
FLUXNEWS_CORE_GAP_ANALYSIS.md    what is missing
              |
SHARED_RUST_CORE_ROADMAP.md      when to address it
              |
MOBILE_RUNTIME_PROOF_CONTRACT.md exactly how Phase 1 proves feasibility
```

## Audit basis

FluxBar source and documentation were audited from the current repository
worktree. The worktree also contains separate, in-progress diagnostic logging
changes for Phase 0. This contract treats those changes as read-only and does
not depend on the outcome of the signed-build sync investigation.

The FluxNews platform audit used revision
`8f8161787d99b6bedb3d17404bb370b53c869aae`, the revision recorded in
`FLUXNEWS_CORE_GAP_ANALYSIS.md:14-19`.

Important current facts:

- `rust-core` builds `staticlib` and `rlib`, not `cdylib`
  (`rust-core/Cargo.toml:6-8`).
- The stable boundary is only `FluxCoreRequest` and `FluxCoreFree`, with
  caller-owned input and Rust-owned output (`rust-core/libfluxcore.h:10-33`).
- Normal non-test configuration has no host path input. Non-macOS path
  discovery returns an error (`rust-core/src/runtime.rs:427-475`).
- Persistence itself accepts an explicit path and uses bundled SQLite, WAL,
  synchronous NORMAL, foreign keys ON, and a five-second busy timeout
  (`rust-core/src/persistence/store.rs:144-166`).
- The current build script supports only macOS targets and SDKs
  (`Build/build-rust-core.sh:9-29`, `Build/build-rust-core.sh:64-65`,
  `Build/build-rust-core.sh:118-146`).
- The current FFI catches panics during request processing, but reading the
  caller pointer occurs before `catch_unwind`; the documented valid-pointer and
  NUL-termination preconditions remain mandatory (`rust-core/src/ffi.rs:34-73`).
- Current in-progress logging is macOS-only; non-macOS builds install a no-op
  logger (`rust-core/src/logging/mod.rs:25-52`). Mobile logging is not a Phase 1
  requirement.

The audited local toolchain had Rust 1.98.0 on Apple Silicon, Xcode 26.6 with
iOS 26.5 SDKs, Android NDK `28.2.13676358`, and only
`aarch64-apple-darwin`, `x86_64-apple-darwin`, and
`armv7-linux-androideabi` Rust targets installed. This is environment evidence,
not a product toolchain mandate. Phase 1 scripts must detect missing targets and
print exact remediation without installing or changing toolchains.

## Phase 1 scope verification

### GAP coverage

Phase 1 addresses only `CORE-GAP-002`: mobile Rust artifacts, host path
injection evidence, TLS/runtime behavior, panic/thread behavior, and package
integration (`FLUXNEWS_CORE_GAP_ANALYSIS.md:332`).

It gathers evidence for, but does not begin:

- `CORE-GAP-001`, by measuring C/JSON host-wrapper and ownership costs;
- `CORE-GAP-003`, by proving only SQLite runtime feasibility and host-selected
  paths, not a mobile schema;
- `CORE-GAP-008`, by recording process/lifecycle behavior and the future
  cross-process test architecture, not implementing headless coordination; and
- `CORE-GAP-019`, by observing error transport ergonomics without defining the
  future typed error taxonomy.

### Prerequisites

Phase 1 implementation may begin after Phase 0's relevant FluxBar Rust tests
are green. It need not wait for a public release. The current signed-build sync
root cause is not a Phase 1 input unless Phase 0 concludes that a shared runtime,
SQLite, HTTP/TLS, FFI, or concurrency primitive is defective.

Before Phase 1 coding starts, the chosen integration branch must pass a locked
Rust dependency check and the relevant Rust/ABI tests. During this audit the
concurrent diagnostic work updated both the manifest and lockfile; the current
lock now includes `log` and `oslog` (`rust-core/Cargo.lock:701-709`,
`rust-core/Cargo.lock:941-959`). Phase 1 depends only on a clean, internally
consistent base revision, not on resolution of the signed-build sync problem or
completion of the diagnostic task.

### Intended outputs

Phase 1 produces:

- reproducible raw Rust archives for every required target;
- an iOS XCFramework containing the device and simulator variants;
- Android JNI probe libraries packaged for each required ABI;
- one disposable but maintained iOS runtime-proof host;
- one disposable but maintained Android runtime-proof host;
- automated ABI, persistence, error, memory, and threading tests;
- controlled HTTPS test evidence;
- physical-device lifecycle checklists for later product qualification;
- artifact size/build-time baselines;
- a binding-evidence report for Phase 5; and
- one binary PASS/NOT PASSED decision record.

### Explicitly deferred

Phase 1 does not implement a FluxNews feature, mobile schema, production mobile
service, final account identity, Miniflux synchronization, migration,
Keychain/Keystore, background scheduling, widgets, audio, downloads, native
FluxNews UI, typed C, or UniFFI.

### Roadmap consistency

No source contradiction requires changing Phase 1's GAP assignment or ordering.
This contract narrows two potentially ambiguous roadmap statements:

- "host paths" are proven with an application-sandbox probe database, not by
  adding the final mobile configuration API; and
- "foreground/background" is later product qualification, not
  BGTaskScheduler/WorkManager or simultaneous extension process execution in
  the Phase 1 feasibility gate.

## Proof decision

The proof uses the complete current Rust library, the existing two-symbol ABI,
and a non-default compile-time probe feature. It must establish facts on both
mobile platforms, not merely cross-compile.

The evidence chain is:

```text
native proof host
    |
    +--> link/load platform artifact
    +--> call FluxCoreRequest with JSON
    +--> copy response and call FluxCoreFree
    +--> run feature-gated runtime probes
            |-- target/runtime metadata
            |-- round trip and large output
            |-- SQLite open/write/read/reopen
            |-- HTTPS trust and timeout
            |-- Rust-created thread
            +-- intentional contained panic
    |
close probe database
    |
reopen and verify persisted SQLite value
```

The feature-gated probes must compile out of normal FluxBar artifacts. They are
test instrumentation, not a mobile product API.

## Proposed Rust target matrix

| Platform | Environment | Rust target | Phase 1 status | Runtime gate |
| --- | --- | --- | --- | --- |
| iOS | Physical iPhone/iPad | `aarch64-apple-ios` | Required artifact | Build and XCFramework link; physical execution is deferred product qualification |
| iOS | Apple Silicon simulator | `aarch64-apple-ios-sim` | Required | Build, XCFramework link, automated simulator tests |
| iOS | Intel simulator | `x86_64-apple-ios` | Optional/non-gating until an Intel CI/developer support requirement exists | Build when an Intel host/CI is available; may join the simulator XCFramework variant |
| Android | 64-bit ARM device | `aarch64-linux-android` | Required artifact | Build/package JNI `.so` and run on an arm64 emulator; physical execution is deferred product qualification |
| Android | 64-bit x86 emulator | `x86_64-linux-android` | Required | Build JNI `.so`, package, emulator instrumentation |
| Android | 32-bit ARM device | `armv7-linux-androideabi` | Required artifact | Build/link/package inspection; hardware runtime is strongly recommended when available but not a physical-device gate |

`i686-linux-android` is not included. The inspected FluxNews release source has
no x86 32-bit artifact. It does explicitly publish `armeabi-v7a`, `arm64-v8a`,
and `x86_64` through its ABI map (`FluxNews/android/app/build.gradle:88-97`)
and release automation, so 32-bit ARM cannot be silently omitted from this
proof. A later product decision may remove it, but Phase 1 does not make that
decision.

Intel iOS simulator support is not a current product commitment. FluxNews
declares simulator support but does not pin architectures; Xcode uses active
architecture for Debug (`FluxNews/ios/Runner.xcodeproj/project.pbxproj:496-500`).
The proof must not fail solely because no Intel execution host exists.

## Deployment targets and toolchains

### iOS

The proof deployment target is **iOS 17.0**, matching the current FluxNews app
and widget targets (`FluxNews/ios/Runner.xcodeproj/project.pbxproj:403-427`,
`FluxNews/ios/Runner.xcodeproj/project.pbxproj:641-713`). Phase 1 must not raise
that product minimum.

Required build environment:

- a supported Xcode version whose SDK can target iOS 17.0;
- Rust targets listed above;
- `cargo`, `rustup`, `xcrun`, `clang`, and `xcodebuild`; and
- an Apple development team/provisioning profile only for physical-device host
  execution.

Raw static archives and headers are not separately code signed. The proof host
application is signed by Xcode for device installation. The XCFramework is a
packaging container, not a separately notarized or App Store artifact.

### Android

The proof uses:

- minimum API **29**, matching `minSdkVersion 29`;
- compile/target API **36**, matching the current FluxNews configuration;
- NDK **28.2.13676358**, matching the pinned Flutter toolchain audit;
- Java/Kotlin bytecode target 17; and
- the three ABIs in the target matrix.

`compileSdk 36`, minimum API 29, and Java/Kotlin 17 are direct project settings
in `FluxNews/android/app/build.gradle:31-56`; the ABI map is at
`FluxNews/android/app/build.gradle:88-97`. Target API and NDK are delegated to
the pinned Flutter gitlink by lines 33 and 53. At the audited FluxNews revision,
that gitlink resolves to Flutter 3.47.0, whose `FlutterExtension` supplies target
API 36 and NDK `28.2.13676358`; release CI verifies that the gitlink resolves to
an exact Flutter release tag (`FluxNews/.github/workflows/build-and-release.yml:34-62`).
Phase 1A must re-resolve and record these inherited values rather than trusting
the prose here. The implementation uses NDK API 29 linkers for every ABI and
fails clearly if the required SDK, NDK, Rust target, Java, Gradle, CMake, or
optional pinned `cargo-ndk` tool is absent.

The build scripts must not invoke `rustup target add`, `sdkmanager`,
`cargo install`, or any package manager automatically. They print exact pinned
remediation commands and exit nonzero.

## Required mobile artifacts

### iOS artifacts

Per build profile, the implementation produces:

```text
.build/mobile-runtime-proof/ios/
    device-arm64/libfluxcore.a
    simulator-arm64/libfluxcore.a
    simulator-x86_64/libfluxcore.a       optional when built
    include/libfluxcore.h
    FluxCoreMobileRuntimeProof.xcframework/
    manifest.json
```

The XCFramework contains one device library variant and one simulator variant.
If x86_64 simulator is built, it is combined with the arm64 simulator slice
before creating the XCFramework. Device and simulator arm64 archives must never
be merged directly with `lipo`, because they target different platforms.

The manifest records Rust version, target triple, profile, feature set, iOS
deployment target, Xcode/SDK versions, archive SHA-256, archive size, and build
duration. It must not contain credentials or absolute user paths.

Required Apple linkage must be discovered and encoded by the proof packaging
instead of copied blindly from macOS. The current HTTP stack is expected to use
Apple Security framework on iOS, but the exact framework/link flags are a build
result to verify. A successful static archive alone is not sufficient; the host
must link and execute it.

### Android artifacts

The Rust crate remains `staticlib` for the proof. A minimal NDK C/C++ JNI shim
links `libfluxcore.a`, copies Java/Kotlin strings into native-owned UTF-8,
invokes the C ABI, copies the result into a Kotlin `String`, calls
`FluxCoreFree`, and exposes no raw pointer to Kotlin.

Per build profile, the implementation produces:

```text
.build/mobile-runtime-proof/android/
    arm64-v8a/libfluxcore.a
    x86_64/libfluxcore.a
    armeabi-v7a/libfluxcore.a
    jniLibs/arm64-v8a/libfluxcore_mobile_probe.so
    jniLibs/x86_64/libfluxcore_mobile_probe.so
    jniLibs/armeabi-v7a/libfluxcore_mobile_probe.so
    verifier/rustls-platform-verifier-0.1.1.aar
    include/libfluxcore.h
    manifest.json
```

The `.so` is the host-loadable artifact. Adding `cdylib` to the production
crate is not required merely to prove Android integration. The JNI shim is
probe-host infrastructure and must not be treated as the final Kotlin binding.
The proof host packages the exact AAR shipped by locked
`rustls-platform-verifier-android 0.1.1`, records its SHA-256, and pins the local
Gradle dependency rather than using `latest.release`.

The manifest records the same reproducibility information as iOS plus NDK
version, API level, ABI, ELF architecture, required shared libraries, SHA-256,
and size. `readelf`/NDK tooling must confirm no unsupported host library or
absolute build path is embedded.

## Minimal native hosts

### iOS host

Use a tiny SwiftUI lifecycle shell plus XCTest target:

- one screen showing target/runtime metadata and PASS/FAIL rows;
- one host wrapper that JSON-encodes requests, copies returned UTF-8, and frees
  every non-null response in `defer`;
- app-private path creation under `Library/Application Support` or an
  equivalent non-user-facing application-support directory;
- test controls for write/read, HTTPS, background-thread execution, and
  lifecycle markers;
- no Keychain, feed UI, navigation architecture, app group, widget target,
  background entitlement, or Miniflux account; and
- XCTest for deterministic ABI/probe cases.

The SwiftUI shell exists only because UIApplication/Scene lifecycle and device
installation cannot be proven by a command-line target. It must not seed the
future FluxNews view hierarchy or state architecture.

### Android host

Use a tiny Kotlin Android application plus instrumentation tests:

- one non-exported test-oriented Activity where practical, with a minimal
  status list and manual lifecycle controls;
- `System.loadLibrary("fluxcore_mobile_probe")`;
- one process-idempotent native initializer that passes the application context
  to `rustls-platform-verifier` before any request can perform HTTPS;
- one Kotlin wrapper over the JNI string-in/string-out function;
- app-private `noBackupFilesDir` for the probe database;
- INTERNET permission only;
- no Compose architecture requirement, Keystore, WorkManager, widget,
  foreground service, media component, FileProvider, or Miniflux account; and
- AndroidX instrumentation tests plus Activity recreation tests.

No Android component other than the launcher Activity should be exported. The
host must not copy FluxNews's existing exported media/widget components because
they add no runtime-proof evidence.

## Temporary binding strategy

### Decision

Reuse `FluxCoreRequest` and `FluxCoreFree`. Do not introduce typed C or UniFFI.
Do not add a third exported production symbol.

Android requires one feature-gated `extern "system"` JNI initialization method
in proof-host glue because the verifier needs `JNIEnv`, `JavaVM`, class loader,
and an application `Context`; those cannot be passed safely through JSON or
obtained from `JNI_OnLoad` alone. This method calls the verifier's idempotent
Android initializer before any HTTPS-capable request. It is not named
`FluxCore*`, is absent from default artifacts, and does not change the two-symbol
production C ABI. The host must surface initialization failure before enabling
HTTPS tests.

A non-default Cargo feature named conceptually `mobile-runtime-proof` adds one
reserved test-only JSON operation to the existing request path. The exact
feature name may change during implementation review, but these constraints do
not:

- normal/default and FluxBar release builds do not recognize or include it;
- the operation is dispatched inside the existing `catch_unwind` boundary;
- responses use the existing Rust allocation and `FluxCoreFree` path;
- probe code is isolated from domain and FluxBar compatibility behavior; and
- build/CI tests prove a normal artifact rejects the probe operation.

### Why not a separate probe C ABI

A new probe function would prove a different call path and allocator boundary.
It also creates symbols that could accidentally be mistaken for a future mobile
API. Feature-gating one operation through the current entry point tests more of
the existing proven interface with less production surface.

### Why not only existing operations

Existing pure operations can prove JSON round trips, but production `configure`
cannot receive a mobile database path, no operation safely injects a panic, and
real refresh would introduce credentials and product synchronization. Reusing
the FluxBar schema or pretending `HOME` is an iOS/Android path would distort the
mobile proof.

### Why not typed C or UniFFI

Either would turn a runtime experiment into the Phase 5 binding decision before
mobile packaging, lifecycle, cancellation, and API shapes are known. They are
explicit Phase 1 non-goals.

## Minimum probe API

One feature-gated operation, `mobile_runtime_probe`, accepts a `probeAction`
field. Exact wire field casing should follow the existing JSON style during
implementation. The minimum actions are:

| Action | Input | Evidence |
| --- | --- | --- |
| `runtime_info` | None | OS, architecture, pointer width, crate version, build profile, panic strategy, and enabled proof feature |
| `round_trip` | JSON string payload and requested output size | Structured input/output, Unicode, empty payload, embedded escapes, and bounded large response |
| `sqlite_open` | Canonical app-private allowed root plus one relative database filename | Bundled SQLite load, path containment, WAL/synchronous settings, and retained probe connection |
| `sqlite_write` | Key and value | Transactional write through the retained probe connection |
| `sqlite_read` | Key | Read during the same process and after reopen/relaunch |
| `sqlite_close` | None | Explicit connection close followed by reopen |
| `https_get` | Controlled URL and bounded timeout | DNS, TLS validation, status/error class, and timeout behavior with no response body logging |
| `thread_probe` | Iteration count within a fixed maximum | Rust thread creation, mutex/condvar coordination, join, and deterministic result |
| `panic` | Fixed confirmation token | Panic inside the existing FFI guard returns the deterministic internal-error JSON and does not unwind or abort |

The probe SQLite runtime uses a separate, minimal table such as key/value plus
schema marker in a dedicated probe database. It must not call or alter the
FluxBar schema. It is compiled only for proof hosts and is deleted with those
hosts.

Inputs are bounded. The signed native proof host is trusted to select its own
app-private root, but Rust still prevents accidental traversal: `sqlite_open`
accepts one canonical existing `allowedRoot` and one single-component relative
database filename, rejects absolute names, `..`, separators, and symlinks, and
verifies the resolved parent remains the canonical root. It does not accept an
arbitrary absolute database path. This is probe containment, not a security
boundary against a malicious signed host and not the final path API.

The proof also rejects response sizes above the fixed test maximum, arbitrary
HTTP schemes, and unapproved URLs in automated tests. The panic action is
unavailable without the feature and confirmation token.

Existing operations still provide control vectors such as null request,
malformed JSON, unsupported operation, and localization. The proof does not
expose all FluxBar operations to host UI.

## FluxBar reuse and non-commitments

### Reused unchanged

- C header and two exported symbols;
- caller/Rust ownership contract;
- C-string/JSON request-response shape;
- panic guard and deterministic FFI error response;
- serde JSON stack;
- current dependency graph as the portability subject; and
- normal FluxBar tests as regression controls.

### Temporary proof-only reuse

- the flat JSON envelope;
- synchronous calls;
- test-only probe operation;
- probe database and retained connection;
- handwritten Swift/JNI wrappers; and
- current HTTP stack for portability measurement.

### Not mobile architecture commitments

- FluxBar's 11 operations;
- FluxBar's unversioned SQLite schema;
- snapshot v1, retained IDs, or 200-row cap;
- credential-derived account identity;
- macOS path discovery;
- embedded FluxBar localization;
- current blocking API as the final cancellation design;
- C/JSON as the final Swift/Kotlin binding; or
- JNI shim shape as the final Android API.

The roadmap's host-supplied cache-path criterion is explicitly **not
applicable** to this runtime proof. The current Rust feed-icon cache is
process-memory-only (`docs/features/SYNC_AND_DATA.md:154-159`), while article
image filesystem caches remain native platform/application storage work. Phase
1 proves host-supplied database-root handling and must not invent a filesystem
cache API solely to satisfy that wording.

## Dependency portability audit

The classifications describe Phase 1 evidence needs, not replacement decisions.

| Dependency/area | Classification | Source-backed assessment and Phase 1 action |
| --- | --- | --- |
| `serde`, `serde_json` | `MOBILE SAFE` | Pure Rust serialization. Exercise Unicode, malformed input, and large bounded output. |
| `sha2`, `base64` | `MOBILE SAFE` | Pure Rust. Included in the complete artifact; no platform configuration expected. |
| `time` | `MOBILE SAFE` | Pure Rust over system time. Verify timestamp creation on both platforms indirectly through probe metadata or SQLite marker. |
| `html5ever`, `markup5ever_rcdom` | `MOBILE SAFE` | Pure Rust parser. No Phase 1 product parsing test is needed; artifact link and size baseline include it. |
| `url` and IDNA/ICU data | `MOBILE SAFE` | Pure Rust but contributes code/data size. HTTPS probe uses URL parsing and size measurement records impact. |
| `image` codecs | `MOBILE SAFE` | Rust codecs with synchronous CPU/memory work. Do not add image product tests; record linked/dead-stripped size and defer memory-pressure image workloads. |
| `resvg`/`usvg`/`tiny-skia` | `MOBILE SAFE` | Pure Rust/C-free configured graph with default text/font features disabled. Packaging and size evidence only. |
| `rusqlite` + bundled `libsqlite3-sys` | `REQUIRES CONFIGURATION` | Requires Apple SDK or NDK C toolchain/linker. Phase 1 proves all targets build and representative native hosts create WAL, persist, and close/reopen. Device relaunch is product qualification. |
| `ureq 2.12.1` + `rustls 0.23.43` | `REQUIRES CONFIGURATION` | The shared agent installs an explicit `rustls-platform-verifier`; it must run off UI threads. Phase 1 proves representative valid HTTPS and invalid-certificate rejection. DNS/error, TLS-version, hostname, timeout, and cancellation matrices are product/release qualification. |
| `rustls-platform-verifier 0.6.2` on iOS | `REQUIRES CONFIGURATION` | Source dispatch selects Apple's Security.framework verifier for Apple vendors. Phase 1 verifies final `Security`/`CoreFoundation` linkage, static package integration, and simulator OS trust; hostname and physical-device repeats are later qualification. |
| `rustls-platform-verifier 0.6.2` on Android | `REQUIRES CONFIGURATION` | Source dispatch uses Android `X509TrustManagerExtensions` through JNI. Package locked support AAR `0.1.1`, initialize once per process with application context before HTTPS, retain its classes in release/R8, and prove representative emulator system trust and invalid-certificate rejection. Device and hostname repeats are later qualification. No OpenSSL library or CA bundle should be required. |
| ureq `native-certs` / `rustls-native-certs 0.7.3` | `REDUNDANT / RELEASE QUALIFICATION` | The explicit platform verifier controls handshakes, but `AgentBuilder::new()` constructs ureq's default configuration first. On iOS/Android this transitive crate probes Unix certificate-file locations via `openssl-probe` (which is a file locator, not OpenSSL). Before release, measure startup behavior and either prove this discarded initialization harmless or remove the redundant feature with full desktop/mobile TLS regression evidence. |
| `log` facade | `REQUIRES CONFIGURATION` | Portable, process-global facade. Current non-macOS backend is no-op. Proof results belong in native test UI/files, not core production logs. |
| `oslog` | `MOBILE SAFE` by exclusion | Gated to macOS and should not enter iOS/Android artifacts. Verify dependency manifests/link maps. Do not broaden it during Phase 1. |
| Complete transitive graph | `UNKNOWN / MUST TEST` | Cargo metadata does not prove target linking. Build/link/run every required target and record native dependencies. |

No audited dependency is pre-classified `REQUIRES REPLACEMENT`. The selected
platform verifier is an explicit shared transport change, not proof-only code:
the proof must therefore test normal FluxBar HTTP behavior and must not claim
that the `mobile-runtime-proof` feature isolates the verifier dependency or
agent configuration. Android verifier initialization/packaging failure makes
Phase 1 NOT PASSED pending an explicit transport decision.

## Mobile SQLite runtime proof

### What Phase 1 proves

On each platform the proof must show:

1. Native code creates/canonicalizes an app-private parent directory and passes
   that allowed root plus a fixed relative database filename as structured
   probe input.
2. Rust opens a new database using bundled SQLite.
3. Rust creates only the temporary probe schema.
4. `journal_mode` reports WAL, `synchronous` reports the configured value, and
   the busy timeout is applied.
5. Rust writes and reads a Unicode key/value in one process.
6. Rust closes and reopens the connection and reads the same value.

Product qualification subsequently proves background/foreground reads,
terminate/relaunch persistence, WAL/SHM sidecars, and platform
permissions/file-protection behavior.

The host owns directory selection and platform backup/file-protection metadata.
Rust owns every SQLite connection. Native Swift/Kotlin code must not open,
query, checkpoint, migrate, or mutate the probe database.

### What Phase 1 does not prove

- final mobile schema or schema migrations;
- full article/enclosure persistence;
- account scope or API-key rotation;
- final transaction/reconciliation behavior;
- downloaded-media retention contracts;
- app-group/widget sharing;
- background-worker simultaneous access;
- native-to-Rust migration; or
- production backup/file-protection policy.

Phase 1 proves **SQLite runtime feasibility**, not **FluxNews persistence
design**. Phase 2 specifies the domain/schema contract and Phase 3 implements it.

### Platform paths

The iOS proof uses an app-private application-support path and marks the probe
directory excluded from backup. The native host records the effective file
protection attribute. It does not use the FluxNews app group.

The Android proof uses `noBackupFilesDir`, not external storage and not a
world-readable path. Android's application sandbox provides access control;
the proof records resulting mode/ownership without assuming Unix `0600` alone
defines Android security.

## Cross-process SQLite risk

Cross-process access remains `CRITICAL`, but Phase 1 does not retire it.

### iOS evidence

Current FluxNews includes a distinct WidgetKit extension and an app group shared
by app and widget (`FluxNews/ios/Runner/Runner.entitlements:5-8`,
`FluxNews/ios/FluxNewsWidgets/FluxNewsWidgets.entitlements:5-8`). The current
widget exchanges generated data through shared defaults rather than directly
opening the SQLite database. Future architecture must not assume the extension
can safely share a live Rust connection or mobile database.

### Android evidence

Current FluxNews declares widget receiver/service and WorkManager behavior, but
no inspected app component specifies `android:process`
(`FluxNews/android/app/src/main/AndroidManifest.xml:73-87`). Process recreation
still occurs, and merged dependency manifests may add behavior not visible in
source. The launcher process hosting RemoteViews is not proof that application
callbacks share one process lifetime forever.

### Phase 1 boundary

Phase 1 tests:

- multiple concurrent native threads calling one process;
- repeated SQLite open/close within one process; and
- WAL/busy behavior within one process.

Product qualification adds relaunch and abrupt process death after a committed
write.

It does not create a WidgetKit target, WorkManager job, second Android process,
or two-process writer test. It hands Phase 2 an ownership rule and Phase 7 a
test requirement:

> Only Rust services open the Rust-owned database. Extensions and platform
> workers consume explicit core/query projections or invoke a separately
> coordinated Rust service. Direct native SQLite access is prohibited.

Full risk retirement remains Phase 7, where foreground/background process
coordination, leases/transactions, expiration, and kill/restart are designed
against the stable mobile repository.

## Lifecycle qualification matrix

The simulator/emulator cases marked required below contribute to the Phase 1
feasibility gate. Physical-device and process-death cases are retained as the
qualification matrix that must pass before native product development relies on
this architecture; they do not block the re-derived Phase 1 gate below.

### iOS

| Scenario | XCTest/simulator | Physical iPhone | Qualification gate |
| --- | --- | --- | --- |
| Cold launch and first FFI call | Automated | Manual/recorded | Simulator required for Phase 1; device later |
| JSON round-trip and free | Automated stress | Recorded smoke | Simulator required for Phase 1; device later |
| SQLite open/write/read/close/reopen | Automated | Recorded | Simulator required for Phase 1; device later |
| Scene background/foreground with connection retained | UI/integration test where deterministic | Recorded | Product qualification |
| Force terminate and relaunch persistence | Script/UI-assisted | Recorded | Product qualification |
| Main-thread short `runtime_info` call | Automated | Smoke | Simulator required for Phase 1 |
| SQLite/HTTPS on host background queue | Automated | Recorded | Simulator required for Phase 1; device later |
| Bounded memory-stress workload | Automated allocation/large-response stress | Recorded Instruments/RSS workload | Bounded host stress required for Phase 1; device profiling later |
| Simulator arm64 artifact execution | Automated | Not applicable | Required for Phase 1 |
| Intel simulator execution | Only when Intel host exists | Not applicable | Non-gating |

iOS suspension duration is OS-controlled and not deterministic. Product
qualification records what occurs during ordinary background/foreground
transitions; it does not claim BGTask execution or guaranteed completion while
suspended.

The physical iPhone checklist also requires a locked-device observation after
first unlock: commit/read the probe value, background the app, lock the device,
wait at least 60 seconds, unlock/foreground, and read again. This proves the
chosen proof-file protection and ordinary suspension path, not Keychain access,
cold-boot-before-first-unlock behavior, or BGTask execution.

### Android

| Scenario | Instrumentation/emulator | Physical arm64 device | Qualification gate |
| --- | --- | --- | --- |
| Cold launch and `System.loadLibrary` | Automated | Recorded | Emulator required for Phase 1; device later |
| JSON/JNI round-trip and free | Automated stress | Recorded smoke | Emulator required for Phase 1; device later |
| SQLite open/write/read/close/reopen | Automated | Recorded | Emulator required for Phase 1; device later |
| Activity recreation | `ActivityScenario.recreate()` | Optional confirmation | Product qualification |
| Background/foreground | Instrumentation/UI-assisted | Recorded | Product qualification |
| `am force-stop`/process death and relaunch persistence | Scripted emulator | Recorded | Product qualification |
| Screen lock while backgrounded, then unlock/read | Optional emulator | Recorded | Product qualification |
| SQLite/HTTPS from background executor/coroutine dispatcher | Automated | Recorded | Emulator required for Phase 1; device later |
| x86_64 emulator execution | Automated | Not applicable | Runtime support qualification; build-only for Phase 1 |
| armeabi-v7a package/load | Build/link/package inspection | Runtime when hardware exists | Artifact gate; runtime residual risk may remain recorded |

No test claims WorkManager, widget, media service, or Android Auto lifecycle
parity.

### Repeated initialization mapping

Phase 1 repeated initialization means bounded probe SQLite
open/write/read/close/reopen cycles in one process and idempotent process-local
Android `System.loadLibrary` use. Native host UI/controller recreation and full
process terminate/relaunch/read cycles remain product qualification. The iOS
artifact is statically linked and the Rust runtime has no production shutdown
API, so neither gate claims dynamic unload/reload or reset of all process-global
Rust state.

## HTTP/TLS runtime proof

Phase 1 requires HTTPS from Rust on the iOS arm64 simulator and one Android
emulator. The same cases remain required on physical iPhone and Android arm64
before product development relies on the transport. Android x86_64 execution is
also required before claiming runtime support for that ABI.

The full product-qualification evidence matrix uses fixed paths for the cases
below. Phase 1 requires only the public-root and invalid-certificate rows;
timeout, DNS, header, hostname, and TLS-version expansion is later qualification.

| Case | Expected evidence |
| --- | --- |
| Publicly trusted certificate and valid hostname | DNS resolves, TLS succeeds, expected fixed status/body digest returns |
| Hostname mismatch or self-signed/untrusted certificate | Request fails certificate validation; no bypass is available |
| Delayed response beyond probe timeout | Request fails no earlier than 90 percent of the requested timeout and no later than the timeout plus the larger of two seconds or 25 percent; it does not hang indefinitely |
| Nonexistent DNS name | Typed/recorded transport failure without process crash |
| Fixed nonsecret request header | Server confirms transport can send a header without using Miniflux credentials |

The server must not log secrets because no real secrets are sent. A real
Miniflux smoke is optional and belongs primarily to Phase 4. It is not required
to prove DNS, TLS, roots, headers, or timeout.

The proof records, without requiring support:

- behavior behind the test environment's HTTP proxy;
- whether a device-installed user CA is visible;
- whether Android Network Security Config affects the platform Trust Manager;
  and
- any custom-CA packaging required by the current stack.

The Android host must not enable cleartext traffic. Current Flutter FluxNews
allows cleartext and trusts system/user certificates in its native network
configuration (`FluxNews/android/app/src/main/res/xml/network_security_config.xml:1-9`),
but that policy does not prove or dictate the future Rust transport policy.

The Android host must initialize the verifier from application context before
constructing the first HTTP agent. Phase 1 requires cold process startup,
repeated idempotent initialization, background-worker HTTPS, and a release/R8
build. Activity recreation and force-stop/relaunch remain product qualification.
The packaged verifier class must load in each executed case; a panic or generic
internal FFI error is not an acceptable substitute for an initialization error.

Phase 1 passes TLS if a public-root case succeeds and an invalid-certificate case
fails on each required simulator/emulator host without disabled verification.
The broader TLS-version/hostname matrix and physical-device repeats are product
qualification. The final iOS link map must include Security and
CoreFoundation. The Android package must include the pinned verifier AAR classes
and must not require an OpenSSL shared library, private CA bundle, or insecure
bypass. Failure is NOT PASSED pending an explicit transport decision.

## Concurrency and threading proof

The current FFI is synchronous. The host contract for Phase 1 is:

- a short pure `runtime_info` call may execute on the UI thread to prove basic
  invocation;
- SQLite, HTTPS, large round trips, and stress calls execute off the UI thread;
- Kotlin/Swift wrappers do not imply cancellation that Rust cannot honor; and
- host task cancellation only stops awaiting/displaying the result unless the
  underlying synchronous call has returned.

Phase 1 required tests:

1. Call `round_trip` from at least eight concurrent native worker tasks with
   deterministic unique payloads and verify no response crossover.
2. Execute `thread_probe` repeatedly so Rust creates, coordinates, joins, and
   cleans up native threads.

Product qualification adds these concurrency/lifecycle cases:

1. Interleave SQLite reads and serialized writes through the probe runtime and
   verify mutex/condvar behavior does not deadlock.
2. Run HTTPS concurrently with local round trips and SQLite reads; local calls
   must not wait on an unrelated network operation solely because of the host
   wrapper.
3. Background/foreground the host while a bounded delayed HTTPS call runs and
   record completion/timeout behavior.
4. Terminate only after a committed SQLite write for the persistence gate. A
   kill during a transaction is deferred to Phase 7 failure testing.

The proof does not redesign `SyncService`, invoke automatic-read scheduler
threads, or claim mobile scheduler correctness. Current process-local thread
behavior and blocking cancellation limits are evidence for Phase 5 and Phase 7.

## Panic and error boundary

Required cases through the actual mobile wrapper are:

| Case | Required result |
| --- | --- |
| Null request from native shim/test | Existing deterministic null-request JSON |
| Valid JSON and supported probe | Valid UTF-8 JSON response |
| Malformed JSON | Existing invalid-request JSON; host remains alive |
| Valid pointer with invalid UTF-8 | Existing invalid-request JSON through a native test helper; host remains alive |
| Unsupported operation | Existing unsupported-operation JSON |
| Probe-level Rust/SQLite/HTTP error | `ok:false` structured error without unwind/crash |
| Intentional probe panic | Exact existing internal-error JSON; subsequent call succeeds |
| Invalid raw pointer or missing NUL | Not tested; violates the C safety precondition and is undefined behavior |

The implementation must compile the proof artifact with unwind-capable panic
behavior. `runtime_info` reports the panic strategy, and the intentional panic
test is the executable gate. A `panic=abort` artifact cannot pass.

Allocation failure, invalid pointers, double free, and freeing through libc are
not safely injectable correctness tests. They remain prohibited caller
behavior, not cases that `catch_unwind` can guarantee.

## Memory ownership proof

Required ownership tests:

- native input storage remains caller-owned and is mutable/valid for the full
  synchronous call;
- Rust does not retain the input pointer after return;
- native code copies every response before calling `FluxCoreFree`;
- every non-null response is freed exactly once through `FluxCoreFree`;
- `FluxCoreFree(NULL)` remains a no-op;
- success, malformed/error, panic, empty, Unicode, and one bounded large
  response follow the same free path;
- a bounded repeated-call workload completes without crash or an observed
  tool-attributed definite leak;
- concurrent response pointers remain independent; and
- process remains usable after ownership and panic stress.

The bounded large response should be large enough to expose bridge copying and
JNI local-reference issues, initially 1 MiB, with a hard proof limit no larger
than necessary. It is not a product payload recommendation.

Use Xcode Address Sanitizer for simulator XCTest and Instruments Allocations/
Leaks on at least one device run where tool support permits. On Android, use a
debuggable native build with Android Studio native memory profiling and an NDK
sanitizer/HWASan configuration where supported by the selected emulator/device.
Tool incompatibility may be recorded, but ownership stress and no observed
leak/crash remain mandatory. The report must not claim absence of all memory
defects solely from these tools.

The Phase 1 retained-memory gate uses a documented warm-up and bounded repeated
call count on both native hosts. No tool may report definitely leaked
allocations attributable to the wrapper/core, and retained memory must settle
rather than grow monotonically over repeated batches. Exact RSS/native-heap
thresholds are device/performance qualification, not portable runtime evidence.

The physical-device stress workload additionally performs 100 sequential 1 MiB
responses, freeing each response before the next call, while Instruments or the
platform native-memory profiler records peak and post-quiescence memory. It must
not crash, terminate, grow monotonically across batches, or report an
attributable definite leak. Record an actual OS memory-warning/trim callback if
one occurs, but do not manufacture one or make nondeterministic callback
delivery a separate pass condition.

## Packaging reproducibility

### Proposed scripts

Names remain subject to implementation review, but responsibilities are fixed:

| Script | Responsibility |
| --- | --- |
| `Build/build-rust-ios.sh` | Validate tools/targets, build device/simulator archives with proof feature, copy header, create XCFramework, verify slices/symbols, write manifest |
| `Build/build-rust-android.sh` | Validate SDK/NDK/tools/targets, build three archives with proof feature, invoke host CMake/JNI linking, verify ELF ABIs/symbols/dependencies, write manifest |
| `Build/test-mobile-runtime-ios.sh` | Build proof host, run simulator XCTest, optionally select a configured device test destination |
| `Build/test-mobile-runtime-android.sh` | Build proof host, run x86_64 emulator instrumentation, optionally select a configured physical device |
| `Build/record-mobile-runtime-proof.sh` | Collect immutable command/tool/artifact/test measurements into a result directory without credentials |

These scripts are separate from `Build/build-rust-core.sh`,
`Build/build-core.sh`, and release scripts. They do not alter FluxBar defaults or
artifacts.

### Inputs

- explicit build profile (`debug` or `release`);
- explicit output directory or repository-local default;
- iOS deployment target 17.0;
- Android SDK/NDK locations and API 29;
- optional device/emulator identifier;
- controlled HTTPS endpoint allowlist; and
- no Miniflux API key.

### Toolchain policy

Scripts inspect and fail. They do not install.

Missing-target output must identify every absent target and show one command
such as `rustup target add <target>`. Missing Android tooling must identify the
pinned NDK/API/tool version. If `cargo-ndk` is selected, its required version is
pinned in documentation and checked rather than installed. Direct NDK linker
configuration remains acceptable if it is simpler and equally reproducible.

### Reproducibility meaning

Phase 1 reproducibility means two clean builds from the same locked source and
toolchain into fresh output directories both pass and report the same
target/profile/features/exported symbols/native dependencies. Manifests record
artifact hashes so differences are visible, but byte identity is not required
for runtime feasibility. Bit-for-bit artifacts and normalized, path-independent
manifests are later release/supply-chain qualification.

### Future CI

CI should eventually build all raw artifacts and run iOS simulator plus Android
x86_64 emulator tests. Physical-device gates remain recorded manual/lab checks
unless device-farm infrastructure is explicitly adopted. CI inclusion is not
required to write the first implementation, but Phase 1 cannot pass without
reproducible commands suitable for CI.

## Binary size and build-time baseline

Measurements are evidence, not optimization gates.

Record for debug and release where practical:

- each raw `libfluxcore.a` size;
- XCFramework total and per-slice size;
- each JNI `.so` unstripped and release-packaged size;
- symbols/debug data versus stripped/package size;
- empty proof host application size;
- host size after linking Rust;
- APK/installed size per Android ABI;
- simulator/device app product and archived IPA-equivalent size where
  reproducibly measurable;
- clean Rust build time per target;
- incremental no-op build time;
- incremental one-Rust-file build time;
- Xcode/Gradle host build time; and
- peak linker memory if readily available.

Use isolated target directories for clean measurements and record command,
tool versions, machine architecture, profile, and feature flags. Static archive
size alone overstates final app impact because link-time dead stripping matters;
both raw and final-host deltas are required.

No dependency is removed or feature-reduced in Phase 1 merely for size. A
surprising result becomes Phase 2/5 evidence.

## Security contract

### Credentials and network

- Use no real Miniflux credentials.
- Use only fixed nonsecret probe headers.
- Never disable certificate or hostname verification.
- Do not enable Android cleartext traffic.
- Restrict automated HTTPS probe URLs to the configured controlled host.
- Do not log full URLs with secrets/query tokens, response bodies, headers,
  database values, or absolute private paths.

### Files

- Use app-private application-support/no-backup directories.
- Native hosts create and validate the allowed parent directory.
- Probe input uses canonical `allowedRoot` plus a single-component relative
  filename and rejects traversal, separators, absolute paths, and symlinks.
- iOS host records backup exclusion and file protection.
- Android host uses internal `noBackupFilesDir` and no external storage.
- Proof reset deletes database, WAL, SHM, and result files owned by the host.

### FFI

- No raw pointer reaches Swift or Kotlin application code beyond the narrow
  wrapper/JNI layer.
- Kotlin never stores a Rust response pointer.
- Swift copies before `defer` frees.
- JNI validates allocation/conversion failures and clears local references.
- Probe input sizes and iteration counts are bounded.
- Undefined invalid-pointer/double-free tests are prohibited.

### Platform surface

- iOS host requests no app group, background, CarPlay, Keychain, or network
  extension entitlement.
- Android host exports only the launcher Activity required for launch, declares
  INTERNET only, and defines no provider/service/receiver.
- Proof artifacts are debug/development artifacts and are not distributed as
  FluxNews or FluxBar releases.

### Logging discrepancy

The current in-progress logger is no-op on iOS/Android. Phase 1 must not broaden
macOS `oslog`, add mobile logging dependencies, or make runtime success depend
on core logs. Native proof harnesses record sanitized outcomes. If concurrent
diagnostic work changes this before implementation, the security review must be
rerun against the chosen integration revision.

## What not to test yet

The following are explicitly deferred because they do not answer mobile runtime
feasibility or depend on later contracts:

- final mobile domain and versioned schema;
- account identity/API-key rotation;
- full article/feed/enclosure repository;
- Miniflux login, `/me`, capability discovery, custom production headers, sync,
  search, onboarding, or mutations;
- Keychain/Keystore and locked-secret behavior;
- BGTaskScheduler/WorkManager scheduling and expiration;
- simultaneous WidgetKit/app/worker database access;
- WidgetKit/Android widget UI or app-group/shared preference transport;
- automatic-read scheduler semantics and Undo UI;
- podcast progression conflict policy;
- audio playback/session/media controls;
- download files/queues/retention;
- CarPlay and Android Auto;
- Flutter SQLite/secure-storage/download migration;
- backup import/encryption compatibility;
- final error taxonomy and native UI localization;
- final C/JSON, typed C, or UniFFI choice; and
- production SwiftUI/Android application architecture.

## Phase 2 handoff

Phase 1 enables Phase 2 to decide:

- which target/packaging constraints the mobile API must accommodate;
- whether current bundled SQLite is viable on every supported ABI;
- how hosts supply app-private paths without macOS discovery;
- whether Android TLS can remain on the current stack;
- what synchronous-call/cancellation limits the API contract must expose;
- which account/runtime lifecycle events need explicit API operations;
- which binary-size dependencies deserve scrutiny;
- what cross-process SQLite architecture Phase 7 must eventually test; and
- what C/JSON costs the Phase 5 binding comparison must include.

The Phase 1 handoff contains:

- target/artifact manifests and reproducible commands;
- final link dependency reports;
- device/simulator/emulator test records;
- SQLite journal/path/protection results;
- TLS trust/error/timeout matrix;
- concurrency, panic, ownership, and leak-stress results;
- lifecycle/cancellation observations;
- size/build-time measurements;
- security review and residual risks; and
- a list of runtime-driven constraints, not proposed domain/schema types.

Phase 1 deliberately leaves Phase 2 account identity, mobile schema,
configuration API, semantic settings, and migration discovery unresolved.

## Relationship to the native iOS prototype

The runtime proof is intentionally disposable as a product but reusable as
engineering infrastructure.

Expected to survive:

- iOS XCFramework build/package script;
- Android Rust/NDK artifact script;
- target/toolchain manifests;
- C ownership wrappers as test references, not necessarily product wrappers;
- controlled HTTPS server fixtures;
- ABI/panic/memory/lifecycle tests;
- device validation checklists; and
- artifact-size/build-time baseline tooling.

Expected to be discarded or replaced:

- proof app UI and local state;
- probe JSON operation and probe feature;
- probe SQLite database/table;
- fixed JNI method and Swift proof wrapper if Phase 5 selects another binding;
  and
- manual probe navigation.

The proof hosts must be maintained enough to reproduce Phase 1 evidence but
must not evolve into an alternate FluxNews app before Phase 2-4 contracts exist.
Useful native iOS inbox development remains after Phase 4 as specified by the
roadmap.

## Binding decision evidence

Phase 1 records evidence for Phase 5 without selecting a winner:

| Measurement | Why it matters |
| --- | --- |
| Swift wrapper source lines and conversion sites | Quantifies C/JSON boilerplate and ownership burden |
| JNI C/C++ plus Kotlin wrapper source lines | Quantifies Android complexity hidden by a string API |
| Number of explicit allocation/copy/free steps | Compares lifetime risk with future generated/typed bindings |
| Error decoding branches | Shows current dynamic error ergonomics |
| Thread hops required per operation class | Exposes synchronous API burden |
| Cancellation experiment outcome | Establishes what a future binding can and cannot solve without service changes |
| Round-trip latency for small and 1 MiB payloads | Gives realistic serialization/copy baseline, not a synthetic final-API target |
| Startup/load time | Quantifies static-link/JNI loading cost |
| Package setup steps and generated/manual files | Compares future maintenance complexity |
| Binary/app size delta | Captures binding/package overhead baseline |
| Debugging stack traces across boundary | Records practical Swift/Kotlin/Rust diagnosis quality |

Measurements use the real proof hosts and payloads needed for runtime evidence.
Do not create artificial microbenchmarks to favor C/JSON, typed C, or UniFFI.

## Implementation decomposition

Each step is independently reviewable. Phase 1 coding must not begin from an
unlocked or failing FluxBar baseline.

### Phase 1A: Reproducible target and dependency preflight

**Objective:** Encode the target/toolchain matrix and prove locked dependency
resolution can start for each target without changing production behavior.

**Likely files:** New mobile build scripts under `Build/`, proof result schema,
and narrowly scoped Cargo feature declaration. Do not alter normal selectors.

**Platform:** Both.

**Acceptance test:** Scripts detect installed/missing Rust targets, Xcode SDK,
Android API/NDK/tooling, and lock consistency; they print exact remediation and
never mutate the toolchain.

**Model:** Kimi K2.7 Code implementation; GPT-5.6 Terra review.

### Phase 1B: Feature-gated Rust probe contract

**Objective:** Implement the one test-only JSON operation and minimal retained
probe runtime without changing normal ABI behavior.

**Likely files:** Feature-gated probe module, minimal FFI dispatch hook inside
the existing panic guard, Cargo feature, Rust unit tests.

**Platform:** Portable Rust.

**Acceptance test:** Default build rejects probe operation; proof build supports
all bounded actions; existing ABI/parity suite is unchanged; intentional panic
returns the existing internal error and a later call succeeds.

**Model:** GPT-5.6 Terra implementation; GPT-5.6 Sol reviews FFI/panic/path
isolation only.

### Phase 1C: iOS archive and XCFramework packaging

**Objective:** Build and verify device/simulator static artifacts and package
them with the unchanged header.

**Likely files:** iOS build script and artifact manifest tooling.

**Platform:** iOS.

**Acceptance test:** Required slices build with iOS 17 target, symbols and
platforms verify, XCFramework is reproducible, and a tiny linker smoke target
resolves required frameworks.

**Model:** Kimi K2.7 Code implementation; GPT-5.6 Terra review.

### Phase 1D: Minimal iOS proof host

**Objective:** Load the XCFramework and automate FFI, SQLite, threading, and
simulator lifecycle cases.

**Likely files:** Isolated iOS proof project, Swift wrapper, XCTest target.

**Platform:** iOS.

**Acceptance test:** Arm64 simulator suite passes; host uses app-private paths;
ASan ownership stress passes; no production FluxNews architecture is added.

**Model:** GPT-5.6 Luna for minimal host/XCTest ergonomics; GPT-5.6 Terra for
bridge review.

### Phase 1E: Android Rust/NDK and JNI packaging

**Objective:** Build three Rust archives and JNI `.so` artifacts with API 29.

**Likely files:** Android build script, JNI shim, CMake configuration, artifact
manifest tooling.

**Platform:** Android.

**Acceptance test:** All three ABI artifacts build/link; symbols, ELF ABI, and
shared dependencies verify; the pinned verifier AAR is packaged; JNI
initialization succeeds; and no OpenSSL native dependency or private CA bundle
is present.

**Model:** GPT-5.6 Terra implementation/review; Kimi K2.7 Code may implement
manifest verification after toolchain details are fixed.

### Phase 1F: Minimal Android proof host

**Objective:** Package/load JNI artifacts and automate representative ABI,
SQLite, and threading cases. Activity/process lifecycle is later qualification.

**Likely files:** Isolated Kotlin proof project, JNI wrapper, instrumentation
tests.

**Platform:** Android.

**Acceptance test:** One Android emulator host suite passes; all ABI `.so` files
package; app uses no-backup internal storage and minimal manifest surface.
x86_64 execution is required before claiming runtime support for that ABI.

**Model:** GPT-5.6 Luna or Kimi K2.7 Code for fixed host scaffolding;
GPT-5.6 Terra reviews JNI ownership and manifest security.

### Phase 1G: Controlled HTTPS and trust matrix

**Objective:** Execute representative valid/invalid cases using the current
HTTP/TLS stack and preserve the broader matrix for product qualification.

**Likely files:** Probe HTTPS action tests, controlled server fixture/config,
platform test scripts, result matrix.

**Platform:** Both.

**Acceptance test:** Required simulator/emulator cases meet the Phase 1 TLS
gate; Apple Security linkage and Android Trust Manager initialization,
AAR/R8 packaging, class loading, and trust have binary viable/not-viable
findings. Existing FluxBar HTTP/parity tests and one macOS public-root HTTPS
smoke cover the shared production agent. The existing HTTP/parity tests remain
Phase 1 regression evidence; the macOS public-root smoke is a FluxBar
release-qualification gate.

**Model:** GPT-5.6 Terra implementation; GPT-5.6 Sol reviews any TLS replacement
decision, but does not preemptively implement one.

### Phase 1H: Concurrency, panic, and ownership stress

**Objective:** Run the specified multi-thread, panic recovery, allocation/free,
large response, sanitizer, and cancellation-observation tests.

**Likely files:** Rust probe tests, Swift XCTest, Android instrumentation/native
tests, stress scripts.

**Platform:** Both.

**Acceptance test:** All deterministic cases pass, no deadlock/crash/crossover
or monotonic leak is observed, and blocking cancellation limits are recorded.

**Model:** GPT-5.6 Terra implementation; GPT-5.6 Sol reviews FFI/concurrency
failures or ambiguous evidence.

### Product qualification: Physical-device lifecycle validation

**Objective:** Before native product work relies on the proof, execute the
iPhone and arm64 Android device matrix and capture reproducible evidence.

**Likely files:** Device checklist/result template and optional orchestration
scripts; no product code.

**Platform:** Both physical platforms.

**Acceptance test:** Link/load, FFI, SQLite relaunch persistence, HTTPS,
background-thread call, foreground/background, and memory observation pass on
named OS/device classes without storing device identifiers unnecessarily.

**Model:** GPT-5.6 Terra coordinates and analyzes results; no Sol unless a
critical architecture ambiguity appears.

### Phase 1J: Evidence review and decision

**Objective:** Compare actual evidence with this acceptance gate and issue one
binary decision.

**Likely files:** Phase 1 result report, roadmap status cross-reference, risk
register updates. Remove no proof infrastructure yet.

**Platform:** Both.

**Acceptance test:** Every mandatory artifact/test/result is linked; exceptions
are either explicitly non-gating above or produce NOT PASSED. A reviewer who was
not the primary implementer or evidence-report author signs the decision and
confirms that every Phase-1-scope `CRITICAL` finding is closed; known critical
risks explicitly assigned to later phases do not invalidate claims Phase 1 does
not make.

**Model:** GPT-5.6 Sol performs the final risk review; GPT-5.6 Terra prepares the
evidence index.

## Model assignment summary

| Step | Primary | Review/escalation |
| --- | --- | --- |
| 1A target/build preflight | Kimi K2.7 Code | GPT-5.6 Terra |
| 1B feature-gated probe | GPT-5.6 Terra | GPT-5.6 Sol for FFI/panic/path boundaries |
| 1C iOS packaging | Kimi K2.7 Code | GPT-5.6 Terra |
| 1D iOS host/tests | GPT-5.6 Luna | GPT-5.6 Terra |
| 1E Android NDK/JNI | GPT-5.6 Terra | Sol only for unresolved ABI/TLS architecture |
| 1F Android host/tests | Kimi K2.7 Code or GPT-5.6 Luna | GPT-5.6 Terra |
| 1G HTTPS/TLS | GPT-5.6 Terra | GPT-5.6 Sol for replacement decision |
| 1H concurrency/FFI stress | GPT-5.6 Terra | GPT-5.6 Sol for ambiguous failures |
| 1I device validation | GPT-5.6 Terra | Sol only for critical interpretation |
| 1J final gate | GPT-5.6 Sol | Terra evidence preparation |

Sol defines/reviews high-risk boundaries and the final decision. It should not
write routine host scaffolding, target checks, manifests, or deterministic
fixtures.

## Phase 1 acceptance gate

Phase 1 answers whether the current Rust library can be a technically viable
shared foundation in native macOS, iOS, and Android processes. It does not prove
native FluxNews product readiness.

### Required for PASSED

#### Baseline and portability

- Current Rust, Go-reference, ABI/parity, and macOS Rust/Go builds pass from one
  coherent revision.
- Locked release archives build for iOS device/simulator and Android arm64,
  x86_64, and armv7.
- The iOS XCFramework and Android JNI libraries package through clean scripted
  commands; architecture, exported symbols, and native dependencies verify.
- The proof feature and Android verifier initializer are absent from default
  production artifacts.

#### Native boundary

- The unchanged `FluxCoreRequest` / `FluxCoreFree` path executes in an iOS
  simulator host and an Android emulator host.
- Swift and JNI bridges preserve standard UTF-8, including non-BMP Unicode,
  copy responses before exactly-once `FluxCoreFree`, and retain no Rust pointer.
- Null/malformed/unsupported input and intentional panic produce controlled
  errors; a call after panic succeeds.
- Bounded repeated and concurrent calls show no crash, crossover, double free,
  or tool-attributed definite leak.

#### Runtime dependencies

- Each mobile host creates a Rust-owned bundled-SQLite database in app-private
  storage, records WAL/configuration, and reads a committed value after explicit
  close/reopen. Native code does not open that database.
- Rust-created threads and concurrent native callers complete without deadlock;
  blocking SQLite/HTTPS work can run away from the UI thread and its
  non-cancellable nature is documented.
- The shared ureq/rustls/platform-verifier path succeeds against a public root
  and rejects an invalid certificate on iOS simulator and Android emulator.
- Android initializes the verifier before HTTPS and packages the lock-matched
  AAR through an R8-enabled release-style build.

#### Evidence integrity

- Build/test scripts select exactly one requested artifact root and fail rather
  than silently accepting stale or duplicate libraries.
- Repeated clean builds are procedurally reproducible as defined above; byte
  identity is not required.
- An independent review has no unresolved `CRITICAL` or `HIGH` defect inside
  these Phase 1 claims.

### Deferred without blocking Phase 1

- Physical iPhone and Android smoke/lifecycle testing is required before native
  product development depends on the proof architecture.
- Foreground/background, force-stop/terminate/relaunch, lock/unlock, memory
  pressure, and platform file-protection behavior are product-development and
  pre-release gates.
- Android x86_64 runtime and armv7 hardware runtime are required before claiming
  runtime support for those device classes; build-only evidence is sufficient
  for Phase 1 feasibility.
- Controlled timeout/DNS/hostname/TLS-version matrices, user-CA/Network Security
  Config policy, and signed/notarized live macOS synchronization are required
  before the corresponding public product release, not to prove basic runtime
  viability.
- Exact device memory budgets and byte-identical release artifacts belong to
  later performance and supply-chain qualification.

### NOT PASSED conditions

Any required item failing or lacking trustworthy evidence produces NOT PASSED.
A NOT PASSED result must identify the smallest corrective action and may be
re-reviewed without beginning Phase 2 implementation. It does not authorize
UniFFI, a typed production ABI, a mobile schema, Go removal, or transport
replacement.

### Current closure work

The independent review recorded in `MOBILE_RUNTIME_PROOF_STATUS.md` found Phase
1 not passed. Before rerunning this gate:

1. Correct the Android Modified-UTF-8/standard-UTF-8 bridge and add direct
   supplementary-Unicode boundary coverage.
2. Correct Android artifact verification so the clean proof build command
   completes for all ABIs.
3. Remove duplicate JNI artifact roots/`pickFirst` ambiguity and prove the test
   APK contains the requested current library.
4. Add a bounded repeated-call ownership test on both native hosts.
5. Add iOS simulator coverage that runs blocking SQLite and HTTPS probes from a
   background queue rather than the main thread.
6. Add native-shim coverage for null requests and valid pointers containing
   invalid UTF-8, while keeping invalid raw pointers outside the test contract.
7. Assert the reported post-open SQLite synchronous and busy-timeout values on
   both native hosts, not only WAL mode.
8. Add an explicit default-artifact check proving the probe operation and
   Android verifier initializer are absent without `mobile-runtime-proof`.

The obsolete ureq `native-certs` feature is redundant under the explicit
platform verifier. It may be removed with desktop/mobile TLS regression tests or
retained temporarily with its harmless discarded-initialization behavior
documented; it is not independently a Phase 1 blocker.
