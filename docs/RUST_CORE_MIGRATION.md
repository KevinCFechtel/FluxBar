# FluxBar Go-to-Rust Core Migration

## Objective

Develop a Rust core beside the existing Go core and make it an
interchangeable implementation before considering removal of Go.

This is a compatibility migration first.

The existing FluxBar product architecture, native macOS UI, local-first
SQLite behavior, Miniflux semantics, scrollover behavior, release model,
and documented product boundaries remain authoritative.

## Why parallel migration

FluxBar already has a narrow native/core boundary, making it suitable
for a low-risk Rust evaluation.

The Go core provides:

-   a working behavioral reference;
-   a fast rollback;
-   a differential-test oracle;
-   a way to evaluate Rust without committing FluxBar to it prematurely.

## Target during migration

``` text
                        macOS SwiftUI/AppKit
                                │
                         existing C/JSON API
                         ┌───────┴───────┐
                         │               │
                       Go core        Rust core
                       reference      candidate
                         │               │
                         └───────┬───────┘
                                 │
                       equivalent behavior
```

## Rust layering

``` text
C ABI adapter
      │
JSON compatibility adapter / dispatcher
      │
domain + application services
      │
┌─────┴─────┐
│           │
SQLite    Miniflux
adapter    adapter
```

FFI and JSON are outer adapters, not domain concepts.

## Migration rules

-   Keep Go intact until the proving period is complete.
-   Do not combine migration with feature work.
-   Do not introduce UniFFI initially.
-   Do not redesign SQLite initially.
-   Do not change native macOS UX to accommodate Rust.
-   Do not move Keychain or other OS integration into the portable core
    without an explicit architecture decision.
-   Prefer idiomatic Rust internally while preserving external behavior.
-   Stop after each requested phase.

## Phase 0 --- Contract audit

Inspect the actual repository and complete
`CORE_COMPATIBILITY_CONTRACT.md`.

Audit:

-   exported C ABI;
-   memory ownership;
-   dispatcher operation inventory;
-   request/response schemas;
-   snapshot versioning;
-   errors;
-   deadlines/timeouts;
-   partial-success behavior;
-   SQLite schema/account scoping;
-   sync/pagination/mutation semantics;
-   existing tests.

Exit gate: enough observed behavior is documented to identify accidental
drift.

## Phase 1 --- Minimal Rust static library

Add `rust-core/` with the smallest viable Cargo project.

Initial requirements:

-   build as a static library;
-   export C-compatible `FluxCoreRequest` and `FluxCoreFree`;
-   safely accept/reject C-string input;
-   return core-owned UTF-8 JSON;
-   free Rust-owned responses through the matching exported function;
-   no Miniflux, SQLite, snapshots, icons, localization, or mutations
    yet;
-   no async runtime.

Go remains default.

## Phase 2 --- Parallel build selection

Add an isolated Rust-core build path that produces the same Xcode-facing
artifact contract as the Go core (`libfluxcore.a` / `libfluxcore.h`).

Desired developer UX:

``` sh
./Build/build.sh                  # default: Go core
FLUX_CORE=go ./Build/build.sh     # explicit Go core
FLUX_CORE=rust ./Build/build.sh   # experimental Rust core
```

`Build/build.sh` is a thin dispatcher that delegates to
`Build/build-go.sh` or `Build/build-rust.sh`. Both scripts invoke Xcode
with the same native target; the Xcode build phase calls
`Build/build-core.sh`, which dispatches to `Build/build-go-core.sh` or
`Build/build-rust-core.sh` based on `FLUX_CORE`.

Go remains the default and production/reference core. The Rust core is
still a skeleton: it links and returns deterministic "not implemented"
responses, but it does not implement SQLite, Miniflux, snapshots, icons,
localization, or mutations yet.

Exit gate: both variants build without Swift source edits and release
behavior has not silently changed.

## Phase 3 --- Compatibility API

Port JSON request/response DTOs and dispatcher structure.

Implement transport behavior before domain behavior. Operations may
remain explicitly unimplemented behind the correct dispatcher.

## Phase 4 --- Pure domain models/logic

Port models and deterministic transformations.

Use Rust enums, explicit optionality, and typed internal errors where
appropriate, while keeping JSON compatibility at the adapter.

## Phase 5 --- SQLite persistence

Port the store before networking.

No schema redesign.

Required compatibility:

``` text
Go DB -> Rust
Rust DB -> Go
```

Use fixture copies, never the developer's real database.

## Phase 6 --- Local snapshots

Implement the local browse/snapshot path first.

Compare normalized Go and Rust outputs over representative database
fixtures, including the documented 200-row presentation cap and snapshot
schema version.

## Phase 7 --- Miniflux adapter

Implement only the API surface FluxBar actually uses.

Keep remote DTOs separate from domain models and make remote access
replaceable by a test fake/mock.

## Phase 8 --- Sync and mutations

Port in controlled increments:

1.  initial refresh;
2.  incremental refresh;
3.  read/unread desired state;
4.  starred state;
5.  pending/offline mutations;
6.  pending flush;
7.  Undo/discard semantics;
8.  automatic-read source/delayed-flush behavior;
9.  full remote pagination/negative reconciliation.

This phase receives the highest compatibility scrutiny.

## Phase 9 --- Supporting core services

Port remaining currently used core-owned behavior, including as
applicable:

-   feed icons/cache;
-   article processing;
-   localization.

Preserve documented behavior first. Redesign later.

## Phase 10 --- Full Rust-backed application validation

Manually validate at least:

-   clean configuration;
-   existing account/database;
-   immediate local render;
-   explicit refresh;
-   startup/hidden-popover sync behavior;
-   read/unread;
-   star/unstar;
-   offline mutation;
-   pending flush;
-   automatic scrollover read;
-   Undo;
-   navigation/filter snapshot behavior;
-   feed icons;
-   localization;
-   restart persistence.

## Phase 11 --- Rust becomes development default

Switch the normal development core to Rust only after compatibility
gates pass.

Keep an explicit Go fallback.

## Phase 12 --- Proving releases

Ship Rust-backed releases while retaining Go for regression isolation.

Suggested removal gate:

-   at least two stable Rust-backed releases;
-   no known data compatibility regression;
-   no unresolved sync semantic difference;
-   signing/notarization/distribution remains stable.

## Phase 13 --- Remove Go

Only after the proving gate.

Then remove Go-specific source/build/module material that is no longer
needed and update current-state documentation.

## Phase 14 --- Evaluate UniFFI separately

After Rust is stable, prototype a typed adapter and compare:

-   Swift ergonomics;
-   future Kotlin/Android ergonomics;
-   typed errors;
-   async/callback support;
-   packaging complexity;
-   debugging;
-   binary/API stability.

C/JSON may remain a valid adapter even if UniFFI is added.

## Phase 15 --- Expand toward a reusable Flux core

Only after FluxBar proves the Rust architecture should the core gain
additional domain capabilities needed by FluxNews.

Do not add FluxNews UI concepts. Add portable domain capabilities.

Consider a separate core repository when the API is stable and a second
real consumer exists.

## Model/agent guidance

Lower-cost coding models are suitable for:

-   Cargo scaffolding;
-   mechanical DTO/model ports;
-   serde work;
-   focused tests;
-   straightforward build plumbing.

Escalate to a stronger reasoning/coding model for:

-   contract interpretation;
-   FFI ownership/allocator bugs;
-   SQLite transaction compatibility;
-   synchronization/reconciliation;
-   concurrency;
-   difficult cross-language integration regressions.
