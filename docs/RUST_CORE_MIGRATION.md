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

The Rust crate separates the external JSON compatibility layer from
future domain types:

``` text
rust-core/src/
├── lib.rs        # module root, re-exports C ABI
├── ffi.rs        # C-string / panic-safe FFI boundary
├── transport/    # external request/response DTOs
│   ├── request.rs
│   └── response.rs
└── dispatcher.rs # typed operation dispatch to stub handlers
```

The `transport` module owns the historical/awkward JSON envelope shared
by all operations. It deserializes the flat external request and converts
it into a typed `Operation` enum (configure, local_snapshot, refresh,
set_read, set_starred, undo_read, discard_undo, flush_pending, feed_icon,
localize, localize_plural). The dispatcher routes each variant to a stub
handler that returns a deterministic "not implemented" response.

`FluxCoreRequest` wraps JSON processing in `catch_unwind` so a Rust panic
never unwinds across the C ABI. Panics produce a deterministic JSON error
response.

Exit gate: Rust understands every supported operation, routes each to the
correct handler, and returns compatible error/null/malformed/unknown-
operation responses. Domain functionality (SQLite, Miniflux, snapshots,
mutations, icons, localization) remains unimplemented.

## Phase 4 --- Pure domain models/logic

Port models and deterministic transformations.

Use Rust enums, explicit optionality, and typed internal errors where
appropriate, while keeping JSON compatibility at the adapter.

The domain layer lives under `rust-core/src/domain/`:

-   `selection.rs`: typed `Selection` enum with `normalize()` preserving
    Go's observable rules, including the quirks that `"all"` echoes an
    incoming id and `kind=unread` is unread-only regardless of the flag.
-   `entry.rs`: `Entry` plus a validated `EntryStatus` (`read` / `unread`).
    Snapshot/UI-only fields (icon bytes) are deliberately absent.
-   `navigation.rs`: `Feed`, `Category`, and `build_navigation()`
    porting the pure part of the Miniflux adapter's navigation mapping:
    orphan-feed skipping, per-category count aggregation, and
    case-insensitive stable title sorting.
-   `account.rs`: SHA-256 account ID derivation, byte-compatible with Go
    (verified against a cross-language test vector).

The domain layer has no serde, JSON, FFI, SQLite, or Miniflux concerns.
Wire DTOs convert to domain types through an explicit conversion
(`transport::Selection::to_domain`). Deliberately not ported in this
phase: HTML preview extraction (needs an HTML-parser dependency),
icon processing, localization, snapshot assembly (persistence-coupled),
and store reconciliation logic.

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

The Phase 5 Rust adapter is synchronous and uses `rusqlite` with bundled
SQLite. Bundling matches Go's SQLite amalgamation model and provides a
consistent SQLite implementation across the planned portable targets without
introducing an async runtime or ORM. One `Connection` is retained directly,
matching Go's `MaxOpenConns(1)` assumption.

Implemented persistence scope:

-   explicit-path store open/create, exact idempotent schema bootstrap, WAL,
    foreign keys enabled, synchronous NORMAL, 5-second busy timeout, and 0600
    database-file permissions on Unix;
-   account upsert preserving counters/timestamps;
-   low-level account-scoped category, feed, selection-total, and exact entry
    row primitives;
-   separate `PersistedEntry` for remote baseline fields, keeping SQLite state
    out of the domain `Entry`;
-   lossless unknown entry-status round trips through `EntryStatus::Other`;
-   temporary Go-to-Rust and Rust-to-Go interoperability tests.

There is no historical migration/version mechanism to port: Go uses only
`CREATE TABLE/INDEX IF NOT EXISTS`, and `PRAGMA user_version` remains 0.
Pending/Undo tables are created and preserved, but their behavior is deferred.
Also deferred: `ApplySnapshot` and negative reconciliation (sync behavior),
`Snapshot`/navigation query assembly (Phase 6), and all read/star/pending/Undo
mutation operations (Phase 8).

## Phase 6 --- Local snapshots

Implement the local browse/snapshot path first.

Compare normalized Go and Rust outputs over representative database
fixtures, including the documented 200-row presentation cap and snapshot
schema version.

Implemented as `rust-core/src/snapshot.rs`, converting persistence results
(`SnapshotData`) into transport DTOs. Semantics ported from
`go-core/internal/inbox/store.go Snapshot`:

-   selection normalization happens in the store path; the normalized
    selection is echoed in the response (`categories` marshals as JSON
    `null` when empty, matching Go's nil slice);
-   entries are queried with `(selection clause) OR (account_id=? AND id IN
    retainEntryIDs)` before `ORDER BY published_at ASC|DESC LIMIT 200`;
    presentation retention is therefore caller-driven via retained IDs;
-   navigation reads categories/feeds ordered by SQLite `COLLATE NOCASE`
    (ASCII case-insensitive), skips orphan feeds, applies per-feed pending
    read deltas (query errors ignored), and clamps counts at zero;
-   totals use `max(COUNT(*), max(0, remote_total + pending_delta))` when a
    `selection_totals` row exists;
-   starred total = `remote_starred_total` + pending starred delta; a
    missing account row is an error, as in Go.

The public `local_snapshot` handler is real (local state only). `configure`
is implemented as a network-free local subset that is externally equivalent
to Go (Go configure performs no remote effects), except validation errors
are English-only until localization parity arrives in Phase 9.

## Phase 7 --- Miniflux adapter

Implement only the API surface FluxBar actually uses.

Keep remote DTOs separate from domain models and make remote access
replaceable by a test fake/mock.

`rust-core/src/remote/` implements the audited surface: entries listing
with Go-identical query strings (alphabetically sorted parameters,
repeated `status` keys), single entry fetch, categories, feeds,
unread counters, raw icon data URL, `PUT /v1/entries` read batches, and
`PUT /v1/entries/{id}/star`. Authentication uses the `X-Auth-Token`
header; requests carry the Go client's user agent. The blocking `ureq`
client with native-tls preserves OS trust-store semantics (no async
runtime); the 80-second per-request timeout mirrors the Go library
default while operation deadlines remain dispatcher-owned.

The pagination primitive reproduces `fetchCompleteSelection`: ascending
entry-ID cursor via `after_entry_id`, first response's total as
authoritative, strict per-page stability and cross-page uniqueness
checks, and short-page/cursor-stall/growing-total failures with Go's
German compatibility messages.

DTO-to-domain conversion (`remote::entry_to_domain`) preserves the
`mapEntries` feed fallback quirks; preview/image extraction stays
deferred to Phase 9. `RemoteInbox` lets Phase 8 orchestrate against a
fake without HTTP. Differential tests (`Build/test-remote-compat.sh`)
compare production Go Browse against the Rust adapter over a scripted
fake server for every selection kind plus truncated-pagination failure.
No persistence wiring and no public handler changes occur in this
phase: `refresh` remains a stub.

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

Implemented in `rust-core/src/sync.rs` and the existing persistence/runtime
adapters:

-   initial and incremental refresh through `RemoteInbox`, including the
    Go-compatible request order and partial-success local snapshot;
-   atomic `ApplySnapshot` behavior, positive reconciliation, scoped negative
    reconciliation only after strict complete pagination, and pending desired
    state preservation;
-   transactional effective read/star changes plus one durable, revisioned,
    replacing pending row per account/entry/field;
-   ordered flush with desired-state star re-read, per-mutation acknowledge,
    first-failure stop, and successful-prefix persistence;
-   durable Undo batches/items, compensating Undo after delivery, metadata-only
    discard, and Go-compatible manual-read Undo cleanup;
-   one resettable account-bound scheduler worker, 10-second automatic delay,
    immediate manual/star/Undo scheduling, and background panic containment;
-   absolute 45-second refresh and 30-second flush HTTP deadlines, separate
    from the 80-second library default and delayed timer;
-   real public `refresh`, `set_read`, `set_starred`, `undo_read`,
    `discard_undo`, and `flush_pending` handlers.

Refresh and pending flush are serialized per account service, matching Go's
`syncMu` ordering. SQLite ownership, delayed-worker state, and icon
single-flight state are separate, so local snapshots and mutations and
unrelated icon loads do not wait behind remote I/O. Delayed callbacks retain
their original account service after reconfiguration. No async runtime was
introduced.

`Build/test-sync-compat.sh` executes identical operation sequences against Go
and Rust with a stateful fake Miniflux server and compares response JSON,
normalized database rows (including pending and Undo state), exact remote
requests, and final snapshots. `Build/test-sqlite-compat.sh` additionally
continues Go-created mutation/Undo state in Rust and Rust-created state in Go.

## Phase 9 --- Supporting core services

Port remaining currently used core-owned behavior, including as
applicable:

-   feed icons/cache (Phase 9.3);
-   article processing (Phase 9.1, completed);
-   localization (Phase 9.2).

Preserve documented behavior first. Redesign later.

### Phase 9.1 --- Article processing

Implemented in `rust-core/src/article.rs`:

-   Go-compatible preview extraction from article HTML, including block-element
    line breaks, `<br>` handling, ignored `<script>`/`<style>`/`<head>` content,
    whitespace normalization, HTML entity decoding, and rune-based truncation
    with the `…` suffix.
-   First-usable image extraction from `<img>` and `<source>` elements, with
    attribute priority `data-src`, `data-original`, `src`, `data-srcset`,
    `srcset`, tiny-image skip (width/height both present and `<= 2`), and
    srcset reverse selection.
-   Relative URL resolution against the article URL using the `url` crate,
    rejecting `data:`, `javascript:`, `file:`, and other non-HTTP(S) schemes.
-   Image-enclosure fallback when no inline image resolves, using the first
    enclosure whose trimmed MIME type starts with `image/` and whose URL
    resolves to HTTP(S).
-   Integration point: `remote::entry_to_domain` applies article processing
    during refresh before `apply_snapshot`, matching Go's `mapEntries`
    lifecycle.
-   Malformed HTML is handled by html5ever's HTML5 error recovery; any
    unexpected panic during parsing is isolated so one bad article cannot abort
    a refresh.

Differential coverage:

-   `Build/test-article-compat.sh` compares Go and Rust outputs for a shared
    fixture set covering empty content, plain text, paragraphs, nested tags,
    line breaks, entities, Unicode, whitespace, truncation, malformed HTML,
    inline images, relative/absolute/invalid URLs, image-only content,
    lazy/responsive attributes, tiny images, and enclosure fallback.
-   `Build/test-sync-compat.sh` now includes article HTML and enclosures in its
    fake Miniflux entries and compares persisted `preview`/`image_url` values
    together with snapshots and request sequences.

### Phase 9.2 --- Localization

Implemented in `rust-core/src/localization.rs`:

-   Embeds the same English and German translation JSON files used by the Go
    core via `include_str!`, so Rust and Go share a single source of truth for
    catalog content.
-   BCP-47 locale negotiation with English fallback, matching Go's
    `localization.New` behavior for the supported locales. The primary language
    subtag is used; unsupported locales fall back to English.
-   Simple message lookup with caller-supplied fallback (`text`).
-   Plural message lookup using `one`/`other` forms and `{{.Count}}` template
    substitution, with caller fallbacks when the key is missing.
-   `configure` validation errors are now localized; `validation.server_invalid`
    and `validation.api_key_required` use the caller's preferred locale.
-   Public `localize` and `localize_plural` handlers return the localized string
    in the `text` response field, removing the last "not implemented" stubs.

Differential coverage:

-   `Build/test-localization-compat.sh` compares Go and Rust outputs for a
    shared fixture set covering English and German text lookup, unsupported
    locale fallback, unknown-key fallback, missing-locale fallback, English and
    German plural forms, and fallback plural rendering.

### Phase 9.3 --- Feed icons

Implemented in `rust-core/src/icons.rs` and the runtime/dispatcher adapters:

-   data-URL decoding plus raster/SVG normalization to a 32x32 PNG;
-   regular/dark variant generation with Go-compatible appearance thresholds;
-   process-local cache and same-feed single-flight load deduplication;
-   failed, missing, and malformed loads remain retryable rather than cached;
-   RAII cleanup removes stale load slots and wakes waiters during panic
    unwinding;
-   public icon bytes use Go's base64 JSON-string representation.

`Build/test-icon-compat.sh` compares decoded RGBA results and verifies that each
response field contains its implementation's processed PNG bytes. PNG
container bytes may differ between encoders.

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

### Phase 10 audit result (2026-08-22)

**Decision: NOT READY.** All 11 operations exist and broad differential,
bidirectional SQLite, clean multi-architecture artifact, native unit-test, and
Go/Rust UI-smoke validation passed. The audit also found and fixed transport,
icon, endpoint, URL-validation, article-template, and pluralization defects.

The development-default gate remains closed because Rust's single
`SyncService` inner mutex serializes local reads/mutations behind remote and
icon work, unlike Go. Neither implementation cancels every lock wait, but the
Rust lock covers substantially more work and can delay nominally local
operations behind remote I/O. A focused orchestration/deadline remediation
phase is required before Phase 11. Go remains production/reference and Rust
remains experimental.

Recorded engineering validation on 2026-08-22 included arm64/x86_64 universal
core builds, both 16-test native XCTest runs, and the built-in
`--ui-smoke-test` for Go- and Rust-backed apps. The smoke path validates native
layout with a synthetic snapshot; it does not exercise live core startup or
the manual product scenarios above. Live configuration, offline behavior,
startup scheduling, and restart persistence remain outstanding manual checks.

### Phase 10.1 concurrency result (2026-08-23)

**Decision: READY WITH RESERVATIONS for controlled Rust-backed development
evaluation.** The broad service mutex was replaced by state-scoped ownership:

-   a deadline-aware serial gate covers refresh and pending flush only;
-   a separate deadline-aware lock owns the runtime-wide SQLite connection and
    is shared by retained old and current account services;
-   icon cache/single-flight and delayed-worker state are independently
    synchronized;
-   account identity and remote client are immutable on each retained account
    service; same-account configuration updates only generation and the atomic
    sort preference so it cannot create a second flush gate;
-   a weak account-service registry reuses retained work across an A-to-B-to-A
    round trip, while allowing an inactive old service to be reclaimed;
-   runtime configuration publishes a replacement service without redirecting
    in-flight or delayed work from the old service.

Rust now matches the relevant Go concurrency semantics: remote refresh/flush
remain ordered, local snapshots and optimistic mutations can proceed while
remote or icon work is blocked, pending revisions protect superseding local
state, and unrelated icon and sync requests can overlap. Absolute operation
deadlines include waits for SQLite and refresh/flush ownership, with checks
around synchronous SQLite and image-processing calls. Those calls cannot be
interrupted mid-call; committed pending work is scheduled before a later
deadline check can report failure. Same-feed icon waiters use their own
deadline. No unsafe `Send`/`Sync`, async runtime, UniFFI, or bridge/schema
change was introduced.

Deterministic blocked-operation tests cover refresh plus local work, icon plus
local/sync work, flush plus superseding mutation, refresh plus flush,
refresh plus refresh, flush plus flush, automatic/manual scheduler races,
same-feed icon waiter expiry, configure during a blocked refresh, and retained
service reuse across direct and round-trip configuration. The full
Phase 10 parity suite, 2,073-response ABI differential, Go race checks, Rust
tests, universal core artifacts, all three app builds, and a Rust-linked launch
smoke passed on 2026-08-23.

This result removes the Phase 10 concurrency blocker but does not switch any
default. Go remains production/reference and release-pinned; Rust remains an
explicit experimental build. Live configuration, offline, startup scheduling,
and restart-persistence product checks listed above remain reservations before
any Phase 11 default change.

### Phase 10.2 development-default readiness re-check (2026-08-23)

**Decision: READY FOR DEVELOPMENT DEFAULT.** Independent review of the actual
Phase 10.1 code confirmed that no broad runtime/service lock remains across
remote or icon work. The runtime captures an account-bound service before work,
the shared SQLite owner serializes only database use, the per-service gate
serializes refresh and flush, and icon cache/single-flight state is independent.
The retained-service registry prevents duplicate gates for both direct
same-account configuration and A-to-B-to-A returns while older work is alive.

The current full Rust suite (122 tests), focused parallel concurrency suite,
Go tests/vet and ten inbox race repetitions, all eight parity suites,
2,073-response ABI differential, arm64/x86_64 core artifact smoke builds, and
default/explicit-Go/explicit-Rust application builds passed on 2026-08-23. A
Rust-linked application launch smoke also passed. The core-artifact build
scripts compile and invoke the required C ABI symbols; a separate `nm` listing
is not reliable with the installed Apple LLVM reader and Rust 1.98 object
attributes.

FluxBar has no public Go-backed installed base. A clean Rust-backed first
public installation is therefore sufficient: Go/Rust SQLite interoperability
remains compatibility and regression-oracle coverage, not an end-user upgrade
requirement. This statement applies to FluxBar only, not to any future FluxNews
migration.

Remaining work is not a development-default core blocker: live configuration,
offline behavior, startup scheduling, and restart persistence are manual
pre-1.0 product-hardening checks. The synchronous SQLite/image deadline limits
and rare scheduler-thread-creation retry behavior remain documented bounded
limitations. Phase 11 may switch the normal development build to Rust while
retaining Go as reference/fallback. This re-check does not perform that switch,
does not alter the Go-backed release path, and does not remove Go.

## Phase 11 --- Rust becomes development default

**Status: COMPLETE.** On 2026-08-23 the normal development core was switched
to Rust. `Build/build.sh` and `Build/build-core.sh` now default to Rust when
`FLUX_CORE` is unset; valid values remain `rust` and `go`, with an explicit
error for any other value. `FLUX_CORE=rust` and `FLUX_CORE=go` both work;
`Build/release-go.sh` remains explicitly Go-backed so signed releases stay
pinned to the proven core.

This switch affects only the default developer/local build path. Go remains
the production/reference implementation and the fallback for regression
isolation. No Go code was removed or deprecated, the SQLite schema was not
changed, and no new bridge/dependency was introduced.

## Phase 12 --- Proving releases

Ship Rust-backed releases while retaining Go for regression isolation.

A parallel signed/notarized Rust release-candidate script,
`Build/release-rust.sh`, is available for realistic local testing. It uses
the same signing identity, hardened runtime, notarization process,
versioning, packaging, and validation as `Build/release-go.sh`. The
artifact filename includes `-rust` to avoid overwriting the Go reference
release; the app bundle name and identifier remain unchanged.

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
