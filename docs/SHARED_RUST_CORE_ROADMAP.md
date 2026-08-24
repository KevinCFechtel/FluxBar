# Shared Rust Core and Native FluxNews Roadmap

## Purpose

This document is the execution plan derived from
`FLUXNEWS_CORE_GAP_ANALYSIS.md`. The GAP analysis remains the source-backed
inventory; this roadmap orders that work into independently reviewable phases.
It does not implement a GAP, choose a final mobile binding, or define a final
Flutter migration procedure.

The target architecture is:

```text
                         Shared Rust Core
                              |
                 +------------+------------+
                 |            |            |
              FluxBar      Native iOS   Native Android
              macOS         FluxNews       FluxNews
```

FluxBar is the first proving consumer, not the product template for FluxNews.
The existing Flutter FluxNews application is the feature and migration source,
not the target architecture. Rust owns portable domain, persistence, remote,
sync, mutation, and progression semantics where sharing is useful. Native hosts
continue to own secrets, scheduling, UI, UI localization, widgets, audio
sessions, media controls, files, download queues, and OS lifecycle.

## Current baseline

The repository already provides the complete FluxBar compatibility surface:

- the two-function C ABI and typed internal dispatcher;
- portable FluxBar domain types;
- the Go-compatible unversioned SQLite store and snapshots;
- Miniflux transport, refresh, reconciliation, mutations, pending delivery,
  automatic-read Undo, article processing, localization, and feed icons;
- state-scoped concurrency with local work independent from remote work;
- Rust-default developer and Xcode builds;
- explicit Go fallback builds; and
- separate signed/notarized Go and Rust release scripts.

The implementation evidence is summarized in `RUST_CORE_MIGRATION.md:137-580`,
`CORE_COMPATIBILITY_CONTRACT.md:1058-1111`, and
`FLUXNEWS_CORE_GAP_ANALYSIS.md:229-254`. `Build/build.sh` and
`Build/build-core.sh` default to Rust; `Build/release-rust.sh` and
`Build/release-go.sh` explicitly pin their respective cores.

FluxBar has no public Go-backed installed base. A first public FluxBar release
may therefore start from a clean Rust-created database. Go/Rust SQLite
interoperability remains valuable regression-oracle coverage, but is not an
end-user migration requirement (`CORE_COMPATIBILITY_CONTRACT.md:1108-1111`).
The migration that eventually matters is from deployed Flutter FluxNews state
to the Rust-backed native FluxNews architecture.

## Roadmap principles

1. Freeze working FluxBar behavior. New mobile services may reuse internals but
   must not enlarge or reinterpret the FluxBar snapshot, schema, or wire API.
2. Validate the current C ABI on mobile before selecting a replacement or
   companion binding.
3. Use a separate, versioned mobile persistence profile. Do not make either
   existing schema the unreviewed common schema.
4. Build vertical slices. A useful iOS inbox should precede podcasts, widgets,
   backup compatibility, and existing-user migration.
5. Start migration characterization early, but do not make migration
   implementation block clean-start prototypes.
6. Preserve stronger Rust mutation and reconciliation semantics instead of
   copying known Flutter weaknesses.
7. Add an abstraction only when a second real consumer or a tested portability
   requirement exists.
8. Keep native responsibilities native. The Rust crate must not become a
   cross-platform application framework.

## Re-evaluation of the old FluxBar phases

| Previous work | Decision | Replacement and reason |
| --- | --- | --- |
| Phases 0-9, compatibility implementation | `KEEP` | They are completed historical records and define the proven FluxBar compatibility baseline. |
| Phase 10, full application validation | `SHORTEN` | Keep the outstanding live configuration, offline, startup scheduling, and restart-persistence checks as first-release hardening. Do not repeat the already-passed language-port campaign. |
| Phase 10.1/10.2, concurrency and default readiness | `KEEP` | These results establish the reusable concurrency baseline and justify Rust-default development. |
| Phase 11, Rust development default | `KEEP` | Complete and verified. Go remains an explicit fallback. |
| Phase 12, multiple proving releases | `MERGE` | Merge useful product/release checks into Phase 0 below. A long Go-to-Rust rollout is not justified without a public Go installed base. |
| Mandatory two-Rust-release Go migration gate | `REMOVE` | There are no Go-backed users to migrate. A post-release observation period may still inform Go removal, but is maintenance policy rather than data-migration safety. |
| Phase 13, remove Go immediately after proving | `MOVE LATER` | Go is deprecated for development but remains useful as a differential oracle and fallback until frozen fixtures can replace it. |
| Phase 14, evaluate UniFFI after Go removal | `MOVE LATER` | Binding evaluation moves to Phase 5, after mobile runtime proof and a working inbox API. It is independent of Go removal. |
| Phase 15, one broad reusable-core expansion | `MERGE` | Replaced by Phases 1-10 below, derived from the 22 source-backed GAPs. |
| Strict `12 -> 13 -> 14 -> 15` ordering | `REMOVE` | Mobile runtime, schema, and vertical-slice work can proceed while Go remains available. |

Meaningful FluxBar testing is not removed. Phase 0 retains first-release product
and distribution checks; every later shared-module change must continue relevant
FluxBar Rust tests and selected Go differential tests.

## Tracks and phase gates

Two tracks share contracts but have different gates.

### Track A: clean-start development

Track A covers FluxBar, native prototypes, and development/test installations.
It creates a new mobile database and does not import Flutter state. It can begin
immediately and reaches a useful iOS inbox after Phase 4.

### Track B: existing Flutter FluxNews migration

Track B begins with read-only characterization in Phase 2 and completes in
Phase 10. It must not constrain early prototype storage with accidental legacy
shapes, but Track A must preserve stable identities, explicit schema versions,
and importable domain concepts so migration remains feasible.

### Phase dependency summary

```text
Phase 0  FluxBar first-release hardening (parallel release gate)
Phase 1 does not claim Phase 0 release completion.
Phase 1  mobile runtime proof
    |
Phase 2  mobile contracts + schema + migration discovery
    |
Phase 3  local mobile repository + interim API
    |
Phase 4  live sync + durable mutations
    |                  |
    |                  +--> useful native iOS inbox prototype
    |
Phase 5  binding decision + stable mobile API
    |                  |
    |                  +--> native Android product work may begin
    |
    +--> Phase 6 reader/service parity
    +--> Phase 7 headless/offline safety
    +--> Phase 8 media state/download contract
                       |
                       +--> Phase 9 native platform parity
                                      |
Phase 2 migration discovery ----------+--> Phase 10 existing-user cutover
```

Phases 6-8 may overlap after Phase 5 where their dependencies and repository
areas do not conflict. Phase 10 cannot complete until its must-preserve target
models from Phases 4, 6, and 8 are stable.

## Phase 0: FluxBar Rust first-release hardening

**Objective:** Complete the useful remainder of old Phase 10/12 without a
lengthy Go-to-Rust installed-base rollout.

**GAP IDs:** None. This proves the existing FluxBar product and release path.

**Why grouped:** Live product behavior and release artifact validation concern
the same already-implemented FluxBar binary. They should not delay unrelated
mobile architecture analysis once their gate is explicit.

**Prerequisites:** Completed old Phases 10.2, 11, and Rust release tooling.

**Expected repository areas:** FluxBar test plans, `Build/release-rust.sh`,
macOS smoke automation where practical, and release documentation. Product
defects may touch existing code only in a separately approved implementation
task.

**Non-goals:** No FluxNews capability, mobile artifact, schema change, Go
removal, ABI redesign, or binding evaluation.

**Acceptance criteria:**

- live Miniflux configuration and initial refresh pass in the Rust app;
- offline launch, local render, pending mutation, reconnect, and flush pass;
- startup/hidden-popover scheduling and restart persistence pass;
- signed/notarized Rust artifact passes signature, stapling, Gatekeeper,
  linkage, extraction, and launch checks; and
- release notes explicitly describe Rust as the shipped core and Go as a
  deprecated engineering fallback.

**Compatibility tests:** Existing Rust suite, native tests, all relevant parity
suites, ABI differential, Go tests/vet/race checks, explicit Go/Rust builds, and
Rust release validation.

**FluxBar behavior change:** No, except separately reviewed defect fixes.

**FluxNews fixtures:** No.

**Physical-device validation:** A real Mac and live Miniflux account are
required; signing/notarization requires the actual release environment.

**Recommended model:** GPT-5.6 Terra for test/release execution; GPT-5.6 Sol
only for a data, sync, concurrency, or signing failure with ambiguous cause.

## Phase 1: Mobile runtime proof

**Objective:** Prove that the current Rust library and unchanged C ABI can run
safely on iOS and Android before designing a final binding.

**GAP IDs:** `CORE-GAP-002`.

**Why grouped:** Artifact production, host path injection, native TLS, allocator
ownership, panic containment, threading, and lifecycle form one go/no-go proof.
No product API should be designed around an unproven runtime.

**Prerequisites:** Phase 0 need not be publicly released, but current FluxBar
tests must be green. The mobile spike must use the current ABI first.

**Expected repository areas:** `rust-core/Cargo.toml`, narrowly scoped runtime
path/config injection, mobile build scripts or packaging spikes, tiny Swift and
Kotlin test hosts, and mobile-specific test documentation.

**Non-goals:** No production app, mobile schema, UniFFI, full query API,
background scheduler, Keychain/Keystore wrapper, or FluxNews feature.

**Acceptance criteria:**

- device and simulator/emulator artifacts build for supported targets;
- hosts provide database paths without macOS path discovery; file-cache path
  injection is explicitly not applicable because the current Rust icon cache is
  memory-only and filesystem image caches remain native platform work;
- repeated request/response/free cycles prove allocator ownership;
- panic and malformed-input behavior remains contained;
- native TLS trust, authentication, timeout, and proxy/custom-certificate
  assumptions are recorded;
- load, foreground/background, termination, probe connection close/reopen, and
  process relaunch behavior is characterized; and
- cancellation limitations of blocking calls are measured rather than hidden.

**Compatibility tests:** Existing ABI corpus on macOS plus mobile ABI smoke
vectors, C-string ownership tests, repeated probe open/close and host process
relaunch, and FluxBar parity tests for any shared runtime change. Dynamic unload
of a statically linked core is not a Phase 1 claim.

**FluxBar behavior change:** No.

**FluxNews fixtures:** No; a tiny fake server is sufficient.

**Physical-device validation:** Deferred from the runtime-feasibility gate but
required before native product development depends on the proof architecture.
Simulators/emulators do not prove realistic memory, process lifecycle, file
protection, or locked/background behavior; those residual claims stay explicit
in `MOBILE_RUNTIME_PROOF_CONTRACT.md`.

**Recommended model:** GPT-5.6 Terra. Escalate ABI ownership or lifecycle
ambiguity to GPT-5.6 Sol.

The exact implementation and evidence gate for this phase are defined in
`MOBILE_RUNTIME_PROOF_CONTRACT.md`.

## Phase 2: Mobile contracts, schema decision, and migration discovery

**Objective:** Freeze the minimum mobile domain and persistence invariants,
resolve identity policy, characterize required Miniflux configuration, and
start read-only historical FluxNews discovery.

**GAP IDs:** Start `CORE-GAP-001`, `CORE-GAP-003`, and `CORE-GAP-009`. Collect
preparatory settings requirements for future `CORE-GAP-018` and historical
fixtures for future `CORE-GAP-004` and `CORE-GAP-021`, but do not begin those
GAPs: their declared dependencies remain authoritative.

**Why grouped:** Account identity, schema keys, semantic settings, custom
headers, server capabilities, and importability constrain one another. Choosing
them independently would create avoidable migrations or API churn.

**Prerequisites:** Phase 1 runtime evidence and the GAP analysis source
revision. Obtain real or generated historical Flutter fixtures where possible.

**Expected repository areas:** Architecture records, mobile domain/schema
specification, version/migration test harness design, remote configuration
contract, account identity decision, and fixture manifests. Minimal remote
capability code may be implemented only in the later implementation task for
this phase.

**Non-goals:** No production migration, no replacement of the FluxBar schema,
no final binding selection, no native UI, and no copying Flutter tables
unchanged.

**Acceptance criteria:**

- a reviewed account identity policy survives API-key rotation;
- mobile schema v1 is specified with explicit migration/version rules;
- full articles, feeds, categories, enclosures, progression, effective/remote/
  desired state, pending revisions, retention, and indexes have owners;
- command receipts, query/pagination behavior, and synchronization outcome
  categories are specified without coupling mutations or synchronization to
  presentation adoption;
- synchronization run identity, scope/completeness/freshness evidence, durable
  versus presented counter meanings, and restart semantics are specified;
- destructive reset coordination and in-process concurrency assumptions are
  explicit;
- custom-header allow/override policy and `/me`/version capability behavior are
  characterized;
- semantic versus presentation settings are separated;
- all obtainable Flutter schema versions and non-SQLite state locations are
  inventoried; and
- a provisional migration inventory identifies `MUST_PRESERVE`,
  `SHOULD_PRESERVE`, `CAN_REBUILD`, `CAN_DROP`, and `UNCLEAR` state without
  claiming final `CORE-GAP-004` characterization or implementing import.

**Compatibility tests:** Schema design review against FluxBar fixtures,
fake-Miniflux header/capability vectors, account rotation vectors, and read-only
Flutter fixture opening/inspection tests. Existing FluxBar configure must remain
network-free.

**FluxBar behavior change:** No.

**FluxNews fixtures:** Required for schema characterization; synthetic fixtures
must be labeled when real deployed examples are unavailable.

**Physical-device validation:** Not required for the design gate. Secure-storage
location and accessibility assumptions discovered here must be validated later.

**Recommended model:** GPT-5.6 Sol for schema, identity, migration inventory,
and API architecture. Kimi K2.7 Code may implement fixed fake-server vectors
for `CORE-GAP-009` after the contract is approved. GPT-5.6 Terra may define
settings DTO fixtures after ownership is fixed.

## Phase 3: Local mobile repository and interim API

**Objective:** Implement a clean-start, local-first mobile repository that can
render real FluxNews-shaped article data without networking.

**GAP IDs:** Complete `CORE-GAP-003`; only after that acceptance gate, begin the
local-query portion of `CORE-GAP-006` and the prototype slices of
`CORE-GAP-012` and `CORE-GAP-018`. Continue the prototype slice of
`CORE-GAP-001`. Those broader gaps remain open; scope mutations complete
`CORE-GAP-006` in Phase 4.

**Why grouped:** Persistence is useful only with deterministic queries and a
host-callable vertical slice. Article derivation and semantic defaults must be
applied consistently when fixture records enter the store.

**Prerequisites:** Phase 2 schema, identity, ownership, and API-category
decisions.

**Expected repository areas:** New mobile domain and persistence modules,
versioned migrations, local query service, mobile article policy, semantic
settings validation, and a versioned interim C/JSON mobile namespace with a
small Swift wrapper.

**Non-goals:** No network refresh, mutation delivery, final binding technology,
Flutter import, widgets, audio engine, downloads, or background execution.

**Acceptance criteria:**

- a clean database opens, upgrades, and rejects unsupported future versions;
- every durable domain row and query is isolated by the approved stable account
  identity, including API-key rotation fixtures;
- full HTML, categories, feeds, enclosures, progression baselines/desired
  metadata, retention metadata, and read/star state baselines round-trip;
- source-demonstrated article hash, created/published times, reading time, share
  code, comments URL, and feed title round-trip;
- richer feed metadata required by sync, article policy, icons, and local
  overrides, including site URL, icon ID/MIME, and crawler behavior, round-trips
  without embedding presentation-only UI state;
- cursor queries cover all/unread/starred/category/feed scopes and counts;
- equal publication times have deterministic secondary ordering;
- full-content lookup and fixture-driven preview/image policy work;
- queries are not capped by FluxBar's 200-row snapshot contract; and
- a SwiftUI spike renders an offline article list and content from fixtures.

**Compatibility tests:** Mobile schema migration/rollback fixtures,
multi-account/API-key-rotation isolation, richer-feed round trips, query
goldens, transaction failure injection, article differential fixtures, and the
full relevant FluxBar Rust/Go article, snapshot, SQLite, and ABI suites when
shared primitives change.

**FluxBar behavior change:** No. The FluxBar schema and snapshot remain frozen.

**FluxNews fixtures:** Required for full article/feed/enclosure mapping and
article policy output.

**Physical-device validation:** Not required for the repository gate; simulator
execution is sufficient after Phase 1 proved the runtime. A device smoke is
recommended for database path and storage-protection behavior.

**Recommended model:** GPT-5.6 Sol defines and reviews persistence transactions
and API ownership. GPT-5.6 Terra implements queries/settings from the contract.
Kimi K2.7 Code implements article-policy fixtures and mechanical DTO mapping.

## Phase 4: Live mobile sync and durable mutations

**Objective:** Deliver the first live, useful native iOS inbox vertical slice.

**GAP IDs:** Complete `CORE-GAP-005`, `CORE-GAP-006`, `CORE-GAP-007`, and
`CORE-GAP-009`. Provide only the bounded interim host calls from
`CORE-GAP-001` needed by the prototype.

**Why grouped:** Refresh, complete/incomplete reconciliation, local queries,
and desired mutations are one observable inbox state machine. Splitting them
across untestable intermediate releases would hide data-loss behavior.

**Prerequisites:** Phases 1-3; stable custom-header/capability policy; fake
Miniflux scenarios; `/star` versus `/bookmark` characterization.

**Expected repository areas:** Mobile sync profile, remote DTO extensions,
staged reconciliation, retention entry points, mutation repository integration,
bulk/scope operations, retry/acknowledgement, and interim mobile C/JSON calls.

**Non-goals:** No final binding decision, podcasts, downloads, widgets,
CarPlay, background scheduling, backup/restore, onboarding, or Flutter import.

**Acceptance criteria:**

- native credentials and host paths configure one clean-start account;
- configured custom headers follow the approved allow/override policy on every
  applicable request;
- `/me` validates the configured account, version discovery records supported
  server behavior, and capability gates prevent unsupported operations;
- local queries render before refresh;
- synchronization persists entries, baselines, completeness, and durable
  counters and returns only synchronization outcome/change metadata; it does not
  implicitly replace an active client snapshot;
- clients adopt current repository rows and presentation counters through an
  explicit query at a client-chosen refresh point; freshness policy remains a
  native product decision;
- initial and incremental main/starred sync handle configured scope/windows;
- capped or unstable results never authorize destructive absence cleanup;
- remote field and attachment changes refresh without dropping local intent;
- proven-complete feed/category absence applies the characterized deletion
  policy, while incomplete results preserve existing rows;
- configured retention is enforced transactionally after a complete sync;
- read, unread, star/bookmark, and scope mutations update locally first;
- cursor queries and mark-all/current-scope mutations cover all supported
  all/unread/starred/category/feed scopes;
- batch/scope calls return typed receipts and remote delivery uses characterized
  bulk behavior where Miniflux supports it;
- durable desired state survives restart and deterministic retry;
- newer local revisions survive older remote acknowledgements;
- pending desired values win over stale refresh baselines; and
- automatic-read mutations retain durable Undo receipts while native code owns
  visibility detection and Undo timing;
- a SwiftUI article list performs configure, local query, synchronization,
  explicit snapshot adoption, read/unread, and star/bookmark against a live or
  controlled Miniflux server without destabilizing an active timeline.

**Compatibility tests:** Stateful fake-server sync sequences, custom-header and
`/me`/version/capability matrices, incomplete and changing pagination, process
restart, concurrent local mutation, partial flush, retention boundaries,
mark-scope behavior, equal timestamps, endpoint-version fixtures, and all
relevant FluxBar sync/SQLite/remote/ABI parity suites.

**FluxBar behavior change:** No. Reused algorithms may be extracted only with
unchanged FluxBar golden behavior.

**FluxNews fixtures:** Required for scope, pagination, retention, and mutation
characterization.

**Physical-device validation:** Required on iPhone for live TLS, Keychain-hosted
credentials, app lifecycle, restart, and offline/reconnect behavior.

**Recommended model:** GPT-5.6 Sol for reconciliation, pending-state semantics,
and endpoint ambiguity. Kimi K2.7 Code may implement encoded requests and fixed
DTOs after Sol freezes the state-machine contract. GPT-5.6 Luna may build the
disposable SwiftUI prototype around the interim adapter.

## Native iOS prototype gate

Native iOS product work should begin immediately after Phase 4. The GAPs that
must be complete are `CORE-GAP-002`, `CORE-GAP-003`, `CORE-GAP-005`,
`CORE-GAP-006`, `CORE-GAP-007`, and `CORE-GAP-009`.
The prototype also requires the explicitly bounded interim portions of
`CORE-GAP-001`, `CORE-GAP-012`, and `CORE-GAP-018`, but final binding
ergonomics, the complete article policy, and the complete settings surface
remain open.

The prototype proves this architecture:

```text
SwiftUI host
    |-- Keychain credentials and host paths
    |-- interim versioned mobile adapter
    v
Rust mobile service
    |-- account/configuration
    |-- local cursor queries
    |-- refresh/reconciliation
    |-- durable read/star desired state
    v
versioned mobile SQLite database
```

It deliberately excludes podcasts, downloads, widgets, CarPlay, backup/restore,
existing Flutter migration, advanced onboarding/save commands, background
scheduling, and the final binding layer. It is architecture proof, not a public
replacement candidate.

## Phase 5: Binding decision and stable mobile API

**Objective:** Select and stabilize the mobile host boundary using evidence from
both mobile runtimes and a real inbox vertical slice.

**GAP IDs:** Complete `CORE-GAP-001` and `CORE-GAP-019`.

**Why grouped:** Error typing, cancellation, ownership, async shape, generated
code, and API evolution are boundary decisions. They should be solved once for
the now-characterized query/sync/mutation services rather than guessed earlier.

**Prerequisites:** Phases 1-4; an iOS prototype; an Android runtime host from
Phase 1; representative operation timing and payload measurements.

**Expected repository areas:** Mobile API contract, binding spikes, typed error
taxonomy, generated-wrapper policy if selected, packaging tests, and coexistence
tests with the unchanged FluxBar ABI.

**Non-goals:** No modification or removal of `FluxCoreRequest`/
`FluxCoreFree`, no domain dependency on FFI generators, and no product feature
added merely to exercise a binding.

**Acceptance criteria:**

- Options A-C below are measured with the same query/refresh/mutation slice;
- cancellation and background-expiration semantics are explicit;
- structured errors can be localized by Swift and Kotlin;
- memory ownership and threading rules are documented and tested;
- Swift and Kotlin package integration is reproducible;
- the chosen mobile API has a version/evolution policy; and
- FluxBar continues to use its existing ABI without source or behavior change.

**Compatibility tests:** Cross-adapter semantic tests, malformed/error cases,
ownership/leak stress, cancellation/timeout tests, generated-code reproducibility
where applicable, Swift/Kotlin package smoke tests, and unchanged FluxBar ABI
and differential suites.

**FluxBar behavior change:** No.

**FluxNews fixtures:** Representative query/sync/mutation payload fixtures are
required; historical migration fixtures are not.

**Physical-device validation:** Required on iOS and Android for packaging,
threading, memory, cancellation, and lifecycle behavior.

**Recommended model:** GPT-5.6 Sol owns the decision and API/ABI review.
GPT-5.6 Terra implements measured packaging spikes. Kimi K2.7 Code may generate
repetitive contract tests after the operation shapes are frozen.

## Binding decision gate

The existing FluxBar API remains stable under every option. A mobile API may
coexist beside it.

| Criterion | Option A: C ABI + JSON | Option B: typed C ABI | Option C: UniFFI |
| --- | --- | --- | --- |
| Swift ergonomics | Lowest-level, but a small handwritten typed Swift wrapper is straightforward and already proven conceptually by FluxBar. | Better scalar/handle calls; complex records still need wrappers and conversion. | Usually strongest generated domain-facing ergonomics. |
| Kotlin ergonomics | JNI/JNA wrapper work is explicit and relatively verbose. | JNI wrapper and C record mapping remain substantial. | Generated Kotlin can reduce repetitive mapping. |
| Binary/package complexity | Lowest incremental Rust complexity; mobile packaging still needs host integration. | Moderate; headers, symbol/version discipline, and wrapper packaging grow with the API. | Highest toolchain and generated-package complexity; platform support must be proven. |
| Async/cancellation | Must be designed with operation IDs, callbacks, polling, or host threads; synchronous calls alone are insufficient for background expiration. | Can expose explicit callback/cancel handles, but ownership/state machinery is manual. | Generated async support may help, but actual cancellation of blocking SQLite/HTTP still requires service design. |
| Error typing | JSON codes/data are stable but decoded at runtime. | Tagged result structs can be typed, with ABI evolution costs. | Generated enums/records are ergonomic if version evolution remains controlled. |
| ABI stability | Excellent with one request/free pair and versioned envelopes. | Every exported struct/function increases ABI surface and layout risk. | Rust-facing API changes regenerate host code; package versions must move together. |
| Generated code | None required; wrappers may be handwritten or locally generated. | Optional header/wrapper generation. | Required generated Swift/Kotlin scaffolding. |
| Ownership/lifetimes | Simple copy-in/copy-out strings, at the cost of serialization. | Explicit handles, buffers, callbacks, and free functions are easy to misuse. | Generated ownership helps, but callback/object lifetime behavior still needs device tests. |
| FluxBar impact | None when mobile operations use a separate namespace/envelope. | None if added as a separate API. | None if UniFFI remains a separate adapter target. |
| Migration cost | Lowest for prototype; serialization cost and wrapper maintenance grow with use. | Highest manual API design and bridge maintenance for rich records. | Up-front integration/tooling cost, then potentially lower host boilerplate. |

No option wins by modernity. The final decision requires:

- successful iOS and Android artifact/TLS/lifecycle evidence from Phase 1;
- stable mobile repository, query, sync, and mutation types from Phases 3-4;
- measured payload sizes and latency for list/full-content operations;
- a demonstrated cancellation and background-expiration design;
- comparable Swift and Kotlin package spikes;
- typed error and partial-outcome examples;
- allocator/lifetime stress results;
- generated-code review and reproducibility results; and
- a credible API evolution story independent of FluxBar compatibility.

Option A is the default temporary choice through Phase 4 because it minimizes
premature work. Phase 5 may choose A, B, C, or a narrow combination. UniFFI is
acceptable only if measured mobile benefits exceed package, generated-code, and
lifecycle costs.

## Phase 6: Reader and service parity

**Objective:** Expand the stable mobile service from inbox proof to normal RSS
and platform-data parity without introducing media playback.

**GAP IDs:** Complete `CORE-GAP-010`, `CORE-GAP-011`, `CORE-GAP-012`,
`CORE-GAP-013`, `CORE-GAP-017`, and `CORE-GAP-018`.

**Why grouped:** Search, demonstrated onboarding/save commands, article policy,
icon assets, widget projections, and semantic settings build on stable local
queries but do not alter the central sync state machine.

**Prerequisites:** Phase 5 stable mobile API; Phase 3 repository; Phase 4 remote
configuration.

**Expected repository areas:** Search DTOs, optional Miniflux commands,
FluxNews article policy, raw/MIME icon utilities and host cache contract,
deterministic widget projections, and complete semantic settings validation.

**Non-goals:** No arbitrary feed/category administration, new-article
notifications, UI rendering in Rust, filesystem image cache in Rust, native
widgets, or podcast playback.

**Acceptance criteria:**

- server search correctly encodes queries, pagination, caps, and sort behavior,
  and preserves result identity for subsequent actions;
- only source-demonstrated category/feed onboarding and save commands exist;
- full article content and FluxNews-specific preview/image policy are stable;
- hosts can persist raw icon assets and apply explicit contrast policy;
- widget query DTOs are deterministic for supported filters/limits; and
- sync/retention/feed/widget/media semantic settings have stable defaults,
  validation, and ownership.

**Compatibility tests:** Fake-server search encoding/pagination/cap/sort and
command traces, article and icon goldens, settings round trips/default
migrations, widget projection fixtures, and relevant unchanged FluxBar
article/icon/localization/ABI suites.

**FluxBar behavior change:** No. FluxBar article/icon output remains its own
compatibility policy.

**FluxNews fixtures:** Required for article policy, search parameters,
onboarding commands, icons, settings, and widget projections.

**Physical-device validation:** Not required for core acceptance. Native reader,
image cache, WidgetKit, and Android widget work require later device validation.

**Recommended model:** Kimi K2.7 Code for well-specified search, commands,
article, and icon work; GPT-5.6 Terra for settings and widget DTO integration;
GPT-5.6 Luna for SwiftUI/Android UI and native localization.

## Phase 7: Headless and cross-process safety

**Objective:** Make Rust operations safe for host-scheduled background launches
without moving scheduling policy into Rust.

**GAP IDs:** `CORE-GAP-008`.

**Why grouped:** Headless initialization, cross-process database coordination,
expiration, and foreground/background exclusion are one concurrency contract
and deserve focused failure testing.

**Prerequisites:** Phases 1, 3, 4, and the stable API from Phase 5.

**Expected repository areas:** Headless service initialization, database lease
or transaction strategy, idempotent operation entry points, cancellation/
expiration outcomes, process-contention tests, and native BGTaskScheduler/
WorkManager integration tests.

**Non-goals:** Rust does not register jobs, choose cadence, inspect OS power
policy, access Keychain/Keystore directly, or refresh native widgets itself.

**Acceptance criteria:**

- foreground and background processes cannot corrupt or double-deliver state;
- process death leaves durable pending work recoverable;
- expired operations return retry-meaningful typed outcomes;
- host-supplied paths and secrets initialize a headless core safely;
- SQLite busy/lease behavior is bounded and characterized; and
- native schedulers can rerun idempotent refresh/flush/retention operations.

**Compatibility tests:** Deterministic two-process database scenarios,
kill/restart recovery, overlapping refresh/flush/mutation, lock expiry, partial
delivery, and existing FluxBar in-process concurrency/parity suites.

**FluxBar behavior change:** No unless an independently reviewed safety fix is
shared without changing observable semantics.

**FluxNews fixtures:** Sync and pending-state fixtures are required.

**Physical-device validation:** Required on iOS for BGTask expiration and locked
Keychain behavior, and on Android for WorkManager process recreation,
constraints, and termination.

**Recommended model:** GPT-5.6 Sol for concurrency and failure semantics;
GPT-5.6 Terra for native scheduling harnesses after the contract is fixed.

## Phase 8: Media state, download contract, and destructive reset

**Objective:** Add portable podcast state while preserving native ownership of
playback and files.

**GAP IDs:** Complete `CORE-GAP-014`, `CORE-GAP-015`, and `CORE-GAP-022`.

**Why grouped:** Progression, downloaded-media retention protection,
user-skipped intent, and reset behavior share enclosure identity and data-loss
boundaries. They must be reviewed together even though native services perform
playback and deletion.

**Prerequisites:** Phases 3-5; stable enclosure identity; characterized Miniflux
progression capabilities; explicit download metadata ownership.

**Expected repository areas:** Progression baselines/desired state, conflict
and retry logic, portable download catalog metadata, retention protection,
user-skipped flags, repository-reset transaction, and native coordination
contracts.

**Non-goals:** No AVFoundation/Media3 engine, audio session/focus, Now Playing,
media notification, download queue, filesystem policy, CarPlay/Android Auto UI,
or shared ID3 parser.

**Acceptance criteria:**

- explicit zero, completion, local-ahead, remote-ahead, and stale outcomes have
  characterized conflict rules;
- server capability gates skip or reject unsupported progression operations
  without losing local state;
- progression delivery is durable and revision-safe;
- downloaded and user-skipped metadata survive restart;
- retention cannot remove articles required by registered native downloads;
- native deletion and core metadata acknowledgement have a recoverable order;
- repository reset is transactional for core state and coordinates native audio
  stop, player-state reset, download/icon/image-cache deletion, and widget/UI
  projection refresh with a recoverable partial-failure protocol; and
- Stop, Eject, delete, and completion remain distinct host actions.

**Compatibility tests:** Fake-server progression conflicts and unsupported-
capability cases, process death, explicit-zero fixtures, retention/download
interaction, skipped redownload, reset failure injection, and native audio/
player/file/icon/cache/widget/catalog coordination tests.

**FluxBar behavior change:** No. Future FluxBar podcast use may consume these
services separately after its product contract is defined.

**FluxNews fixtures:** Required for progression, downloaded metadata,
user-skipped state, retention, and reset behavior.

**Physical-device validation:** Required on iOS and Android with real audio
files, storage locations, interrupted downloads, background termination, and
media sessions, although those native systems remain outside Rust.

**Recommended model:** GPT-5.6 Sol for progression conflicts and destructive
semantics; GPT-5.6 Terra for download metadata/native contracts and reset
orchestration.

## Phase 9: Native platform parity and optional shared media utility

**Objective:** Build source-demonstrated native platform surfaces around stable
core contracts and decide whether shared ID3 parsing is justified.

**GAP IDs:** Decide and, only if justified, complete `CORE-GAP-016`. This phase
consumes `CORE-GAP-013`, `CORE-GAP-017`, and media outputs but does not reopen
them.

**Why grouped:** Widgets, automotive surfaces, media controls, image caches,
downloads, and UI localization are native parity work. ID3 parsing is the only
candidate shared utility and should exist only if both clients will consume it.

**Prerequisites:** Phase 5 API; Phase 6 reader/widget/icon data; Phase 7
headless safety; Phase 8 media contracts for media surfaces.

**Expected repository areas:** Native iOS and Android repositories when they
exist, platform integration tests, and optionally a small Rust
`media_metadata` utility isolated from inbox/domain state.

**Non-goals:** No platform scheduler, widget renderer, audio engine, file queue,
automotive UI, or UI catalog in Rust. No Dynamic Island parity claim without a
separate proven product decision.

**Acceptance criteria:**

- native reader/navigation/action flows cover source-demonstrated behavior;
- native diagnostics provide source-demonstrated log viewing/search, clear,
  export/share, and debug-setting behavior while native code owns files and
  sharing;
- BGTaskScheduler/WorkManager, WidgetKit/Android widgets, native localization,
  image caches, audio sessions, media controls, and automotive surfaces use the
  established ownership split;
- platform failure and lifecycle tests pass; and
- `CORE-GAP-016` is either implemented for two real consumers with shared
  fixtures or explicitly closed as native duplication by decision record.

**Compatibility tests:** Native UI/unit/integration suites, widget snapshot and
deep-link fixtures, media-session interruption tests, automotive browse/action
tests, and shared ID3 fixture parity only if the Rust utility is selected.

**FluxBar behavior change:** No.

**FluxNews fixtures:** Required for every claimed parity surface.

**Physical-device validation:** Required on both platforms, including widgets,
background work, secure storage, audio interruption/routes, downloads, and
CarPlay/Android Auto where available.

**Recommended model:** GPT-5.6 Luna for native UI/localization; GPT-5.6 Terra
for platform integration; Kimi K2.7 Code for a fully specified shared ID3 parser
with fixed differential fixtures.

## Phase 10: Existing Flutter user migration and backup decision

**Objective:** Make native FluxNews a safe replacement for existing Flutter
installations.

**GAP IDs:** Complete `CORE-GAP-004`, then `CORE-GAP-020` and
`CORE-GAP-021`, after all target-model dependencies are stable.

**Why grouped:** Historical SQLite, secure storage, progression, downloads,
settings, pending intent, backup artifacts, verification, and rollback form one
cutover safety boundary. They must not be implemented as unrelated importers.

**Prerequisites:** Phase 2 discovery; completed `CORE-GAP-003`, `CORE-GAP-007`,
`CORE-GAP-009`, `CORE-GAP-014`, `CORE-GAP-015`, and `CORE-GAP-018`; stable
native storage/file locations; product decision on supported backup artifacts.

**Expected repository areas:** Versioned migration library/runner, native secret
and file adapters, historical fixtures, import verification, retry/resume,
rollback/failure UI, backup parser/crypto only if retained, and release gating.

**Non-goals:** No in-place mutation of the only Flutter database, no assumption
that ambiguous `syncStatus` or bookmark rows prove user intent, no automatic
credential export from Rust, and no compatibility promise for uncharacterized
app versions.

**Acceptance criteria:**

- Gate 10A completes `CORE-GAP-004`: every supported historical schema and
  non-SQLite state source is characterized, must-preserve mappings are signed
  off, ambiguities have explicit policies, and target prerequisites are stable;
- no `CORE-GAP-021` migrator design or implementation begins before Gate 10A;
- Gate 10B designs and implements `CORE-GAP-021` against the approved mapping;
- every supported historical schema reaches a verified target projection;
- credentials/custom headers, explicit-zero progression, per-feed semantic
  overrides, and user-skipped download intent are preserved;
- ambiguous read/bookmark intent has an explicit conservative policy;
- downloaded files are copied/re-indexed/in-place-associated by a tested policy;
- import is idempotent, resumable, and leaves source state intact until success;
- counts, identities, state projections, and file references verify before
  cutover;
- interruption and insufficient-storage behavior is recoverable; and
- backup ZIP/JSON/Argon2id/AES-GCM compatibility is either tested and retained
  or deliberately declined with a user-facing transition plan.

**Compatibility tests:** Every supported historical SQLite fixture, secure-
storage and preference fixtures, progression/download/settings mappings,
interrupted/resumed imports, duplicate imports, rollback, low-storage cases,
backup known-answer crypto vectors, and physical-device upgrade rehearsals.

**FluxBar behavior change:** No. FluxBar Go/Rust interoperability is unrelated
and must not be presented as evidence for this migration.

**FluxNews fixtures:** Required. Existing-user release is blocked without
representative historical and corrupted/partial fixtures.

**Physical-device validation:** Required on supported iOS and Android upgrade
paths with real secure storage, application containers, files, and backups.

**Recommended model:** GPT-5.6 Sol for migration architecture, ambiguity policy,
verification, and rollback; GPT-5.6 Terra for fixed adapters and fixture
harnesses; Kimi K2.7 Code only for mechanical mappings after contracts and
goldens are frozen.

## Shared-core architecture boundary

The crate should evolve by adding explicit service/profile boundaries, not by a
large directory rename before a second consumer exists. A likely eventual shape
is:

```text
rust-core/src/
    domain/              reusable identities and state concepts
    persistence/
        fluxbar/         current unversioned compatibility store
        mobile/          versioned FluxNews-capable repository
    remote/              Miniflux transport primitives and profile operations
    sync/
        common/          completeness/revision algorithms after extraction
        fluxbar/         current selected-snapshot orchestration
        mobile/          offline repository synchronization
    mutations/           reusable desired-state/revision logic when extracted
    article/             parser/URL primitives plus separate output policies
    media_metadata/      progression; optional shared ID3 utility
    mobile/              mobile application services, not OS integration
    fluxbar_compat/      current dispatcher/snapshot/localization semantics
    ffi/                 FluxBar ABI and any separate mobile adapters
```

This is a destination sketch, not a refactor checklist.

### Likely reusable with focused extraction

- selection predicates and normalized domain concepts;
- Miniflux request/error primitives;
- strict complete-set verification;
- transaction patterns;
- desired-state revisions and exact-revision acknowledgement;
- article HTML/URL/enclosure parsing primitives;
- icon decoding/raster/SVG/single-flight primitives;
- state-scoped concurrency patterns; and
- FFI panic and allocation containment.

The caveats in `FLUXNEWS_CORE_GAP_ANALYSIS.md:651-666` remain binding. In
particular, account identity, remote DTOs, pending storage, and public service
shapes are not reusable unchanged.

### FluxBar compatibility layer

The following remain frozen unless a demonstrated FluxBar defect requires a
separate change:

- `FluxCoreRequest` and `FluxCoreFree`;
- the 11-operation flat FluxBar JSON envelope;
- snapshot v1, retained IDs, and the 200-row presentation limit;
- the unversioned Go-compatible SQLite schema;
- selected-scope refresh and its exact remote traces;
- 600-code-point article output and icon response policy;
- embedded English/German compatibility localization; and
- automatic-read timing assumptions exposed to the macOS host.

### Mobile/FluxNews services

Mobile services own full-content repository queries, mobile sync profile,
semantic settings, progression, widget projections, repository reset, and
typed mobile errors. They may use the same crate and internal primitives without
sharing FluxBar wire or storage contracts.

### Native platform responsibilities

Swift/Kotlin own Keychain/Keystore, host paths, network/lifecycle invocation,
BGTaskScheduler/WorkManager, UI, UI localization, WidgetKit/Android widgets,
image/audio files and caches, download queues, playback/audio sessions, media
controls, CarPlay/Android Auto, file pickers, sharing, and migration UX.

No raw C pointer, Swift/Kotlin type, AppKit/SwiftUI/Compose type, scheduler, or
audio-session concept belongs in Rust domain modules.

## Persistence decision

### Recommendation: Option B, a separate mobile schema

Create a separate, explicitly versioned mobile database and persistence module.
Keep FluxBar on its current unversioned compatibility database. Share algorithms
and domain concepts where proven, not physical tables.

### Why not extend the FluxBar schema

The FluxBar database is an audited compatibility artifact optimized for a
selected 200-row presentation snapshot. Adding full HTML, enclosures,
progression, retention, mobile indexes, and historical migrations would increase
FluxBar regression risk and confuse its no-migration first-release contract.

### Why not copy the Flutter schema

Flutter schema v12 is single-account, couples presentation overrides to feeds,
has weak/ambiguous mutation intent, and has historical upgrade paths requiring
characterization (`FLUXNEWS_CORE_GAP_ANALYSIS.md:97-119`). Copying it would
preserve current limitations and still require a future redesign.

### Why not adapt FluxBar to a new common schema now

FluxBar has a proven schema and no demonstrated product need for full mobile
storage. Moving it would spend migration and compatibility risk before a second
consumer proves the common shape. A future convergence may be evaluated only
after the mobile repository is stable and benefits exceed migration cost.

### Mobile schema requirements

The design must include:

- stable account scope independent from API-key rotation;
- explicit schema version and ordered migrations;
- categories and richer feed metadata;
- full article HTML and required metadata;
- enclosures and stable article/enclosure identities;
- remote, effective, and desired read state;
- remote, effective, and desired star/bookmark state;
- revisioned pending mutations and acknowledgement metadata;
- progression remote/local/desired state, including explicit zero;
- retention metadata and native downloaded-media protection references;
- deterministic cursor indexes for scope, publication time, and ID;
- foreign-key/delete behavior justified by retention and download contracts;
- transactional staging or equivalent complete-set application; and
- migration audit/verification metadata without embedding native secrets.

Credentials, sensitive custom headers, UI preferences, scheduler metadata,
download paths, image/audio bytes, widget transport, and logs remain outside the
core database unless a later source-backed requirement proves otherwise.

### Migration consequences

FluxBar requires no data migration. Clean-start native clients create mobile
schema v1 and exercise normal version upgrades from then on. Existing Flutter
users require a one-time, separately tested import into the mobile database;
their database is not opened as if it were the new store. This supports
verification, rollback, and leaving source data intact until cutover succeeds.

## Clean-start versus existing-user decisions

Track A must not wait for historical migration code, old backup parsing,
download relocation, or ambiguous pending-state policy. It must nevertheless
avoid choices that make Track B needlessly hard:

- never use an API key as the only durable account identity;
- preserve remote Miniflux IDs alongside local keys;
- version the schema from its first commit;
- distinguish missing progression from explicit zero;
- retain remote/effective/desired state separately;
- use stable enclosure identity suitable for download re-indexing;
- keep semantic feed overrides importable and separate from UI styling;
- record enough migration metadata to make import idempotent; and
- never require migration to infer state from rendered snapshots.

Track B characterizes historical SQLite, Keychain/secure storage,
SharedPreferences/equivalent progression, downloads and paths, per-feed
overrides, pending local intent, and backup compatibility. Unknowns remain
unknown until fixtures or source execution prove them.

## Intentional behavior improvements

| Concern | Existing Flutter behavior | Current Rust behavior | Recommended native FluxNews behavior | Migration consequence |
| --- | --- | --- | --- | --- |
| Unread delivery | No symmetric durable unread queue. | Read and unread desired values persist with revisions. | Keep durable bidirectional desired state. | Legacy rows may not prove pending unread intent; import only characterized intent. |
| Bookmark/star delivery | Optimistic `/bookmark` call with no durable outbox. | Durable desired star state, retry, and exact-revision acknowledgement using `/star`. | Keep durable desired state; characterize endpoint by Miniflux capability. | Import effective star; do not invent unresolved intent when legacy evidence is absent. |
| Superseding actions | Immediate/fire-and-forget paths can lose ordering. | Newer revisions survive acknowledgement of older delivery. | Preserve monotonic revisions and exact-revision acknowledgement. | Imported state starts with a deliberate baseline revision. |
| Refresh versus local intent | Weak `syncStatus` behavior can overwrite or omit intent. | Pending desired values win over refreshed remote baselines. | Pending intent always protects effective local state. | Ambiguous legacy values need a conservative migration policy. |
| Partial mutation failure | Selected read chunks may acknowledge; no general durable suffix model. | Successful prefix is removed and failed suffix remains ordered. | Keep deterministic prefix acknowledgement and retry. | No need to reproduce weaker failure behavior. |
| Incomplete synchronization | Flutter marks capped data incomplete; behavior is spread through staging logic. | Negative reconciliation requires a proven complete stable set. | Generalize strict completeness to the mobile profile; never delete on capped/unstable data. | Migration and first refresh must not treat partial remote data as authority. |
| Automatic-read Undo | Scrollover marks rows after scroll; no durable mutation Undo. | Automatic reads produce durable receipts and compensating Undo. | Keep durable receipt semantics; native UI owns visibility detection and Undo timing. | No legacy Undo data exists to import. |
| Account scope | One global Flutter database. | Every durable row is account-scoped, but identity includes API key. | Keep account-scoped state with rotation-safe identity. | Import must associate the legacy database with a verified account. |
| Concurrency | Flutter uses foreground/background exclusion and a file lease. | State-scoped gates allow local work during remote I/O. | Keep responsive local operations and add tested cross-process coordination. | Migration must run under exclusive target-store ownership. |

Flutter defects or limitations are not compatibility requirements. User-visible
data and intentional workflows are the compatibility target.

## FluxNews parity milestones

### Milestone 0: Runtime proof

Phase 1. Rust loads and behaves safely in representative native iOS and Android
hosts. Physical-device behavior remains product qualification before native
development relies on the architecture.

### Milestone 1: Core prototype parity

Phases 3-4. A native iOS host stores full articles, renders local lists, refreshes
from Miniflux, and performs durable read/star actions. Podcasts, background work,
widgets, migration, and final bindings are absent.

### Milestone 2: Reader parity

Phase 6 plus native UI work. Normal RSS use includes scopes, counts, full
content, search, source-demonstrated onboarding/save, article/image policy,
settings, icons, and native navigation/rendering.

### Milestone 3: Offline/background parity

Phase 7 plus native schedulers. Local-first state, retention, pending delivery,
headless refresh, process death, and widget query generation are safe. Native
WidgetKit/Android widget rendering may continue toward Milestone 5.

### Milestone 4: Media parity

Phase 8 plus native audio/download work. Enclosures, progression, download
metadata, retention protection, playback, speed, sleep timer, chapters, and
files behave correctly. Rust does not own the audio engine or queue.

### Milestone 5: Platform parity

Phase 9. Widgets, background lifecycle, native localization, media controls,
CarPlay, and Android Auto match source-demonstrated FluxNews behavior on the
relevant platform. No unsupported notification or general feed-management
feature is added.

### Milestone 6: Migration parity

Phase 10. Existing Flutter users can move with verified must-preserve state,
recoverable failure, and an explicit backup policy.

### Milestone 7: Full replacement readiness

All applicable prior milestones pass supported-device testing, unresolved
critical risks are retired, observability/support procedures exist, and a
release decision confirms the Flutter app can be retired. iOS may reach this
milestone before Android; retirement policy must account for both deployed
populations.

## Model and cost strategy

| Phase | Primary model | Lower-cost delegation after contract freeze |
| --- | --- | --- |
| 0 | GPT-5.6 Terra | Kimi for deterministic script/test maintenance; Sol only for ambiguous critical failures. |
| 1 | GPT-5.6 Terra | Kimi for fixed build matrix scripts; Sol for ABI/lifetime decisions. |
| 2 | GPT-5.6 Sol | Terra for settings/fixture schemas; Kimi for fake-server cases. |
| 3 | GPT-5.6 Sol | Terra for queries/settings; Kimi for DTOs and article fixtures. |
| 4 | GPT-5.6 Sol | Kimi for encoded requests and generated state fixtures after semantics freeze. |
| 5 | GPT-5.6 Sol | Terra for packaging spikes; Kimi for repetitive cross-adapter tests. |
| 6 | GPT-5.6 Terra | Kimi for remote commands, parsing, icons, and goldens; Luna for native UI. |
| 7 | GPT-5.6 Sol | Terra for native scheduler harnesses. |
| 8 | GPT-5.6 Sol | Terra for download/native coordination; Kimi for fixed fixtures. |
| 9 | GPT-5.6 Luna and Terra | Kimi for an approved shared ID3 parser and deterministic native fixtures. |
| 10 | GPT-5.6 Sol | Terra for fixed adapters/harnesses; Kimi for mechanical mappings only. |

Sol should produce short decision records, invariants, failure matrices, and
acceptance fixtures before implementation is delegated. Terra or Kimi can then
implement bounded work against those artifacts. Luna should consume stable
native-facing contracts rather than invent core behavior from UI needs. This
reduces repeated high-cost architecture reasoning without moving risk into
cheaper implementation passes.

## Go deprecation plan

### Current status

```text
Rust
  = default development core
  = first-public-release candidate
  = future shared and production core

Go
  = deprecated for future development
  = behavioral reference
  = differential-test oracle
  = explicit temporary fallback
```

Deprecation means no new product capability should be implemented in Go merely
to maintain future parity. Go may receive narrow fixes required to keep the
reference trustworthy or to diagnose a Rust regression. This task does not
delete Go or change build/release selectors.

### Documentation and CI

- Current-state documents must say Rust is the default and Go is deprecated,
  reference-only/fallback rather than an established production baseline.
- Go unit tests, vet, race checks, ABI differential tests, SQLite
  interoperability, and relevant parity suites remain while Go is present.
- `Build/release-go.sh` remains an explicit engineering fallback, but Go-backed
  release artifacts need not be the normal first-public-release path.
- New mobile capabilities are Rust-only and do not expand the Go oracle.

### Removal gate

Go removal is a later explicit decision, independent from the mobile binding.
It may occur when:

- FluxBar has shipped publicly with Rust and completed an agreed observation
  period;
- Phase 0 has no unresolved Rust-specific data, sync, concurrency, or release
  defect;
- no active debugging workflow still requires running Go;
- all valuable differential cases have durable, implementation-independent
  fixtures and expected outputs;
- build, release, and current-state documentation no longer relies on Go; and
- removal cost is lower than continued reference maintenance value.

Before removal, freeze the ABI corpus, normalized SQLite fixtures, snapshot
goldens, remote request traces, sync/mutation state-machine sequences,
article/icon/localization outputs, error cases, and concurrency invariants.
Those fixtures survive after Go source and live differential jobs are removed.

## Risk review

| Risk | Rating | Why | Earliest retirement phase |
| --- | --- | --- | --- |
| Mobile schema design | `CRITICAL` | A wrong identity/state/version model creates data loss and repeated migrations. | Phase 2 design, proven by Phase 3 implementation. |
| Account identity and API-key rotation | `HIGH` | Current hash changes when the key rotates and can orphan account-scoped state. | Phase 2. |
| Synchronization completeness | `CRITICAL` | Capped or unstable sets can cause destructive reconciliation. | Phase 4. |
| Cross-process SQLite access | `CRITICAL` | Foreground/background overlap can corrupt ordering or double-deliver mutations. | Phase 7. |
| iOS background lifecycle | `HIGH` | BGTask expiration, locked secrets, and process death affect durability. | Phase 7 on physical devices. |
| Android process lifecycle | `HIGH` | WorkManager recreation and multi-process timing differ from desktop assumptions. | Native product smoke before development; fully Phase 7. |
| Progression conflict semantics | `HIGH` | Explicit zero/completion and stale remote values can resume incorrectly. | Phase 8. |
| Binding choice | `HIGH` | Premature choice creates package/API churn across Swift and Kotlin. | Phase 5. |
| Flutter data migration | `CRITICAL` | Historical schemas and ambiguous intent can lose user state. | Discovery starts Phase 2; retired Phase 10. |
| Download/file ownership | `HIGH` | Retention, deletion, migration, and native queues cross transaction boundaries. | Phase 8 contract; fully Phase 10 migration. |
| Backup encryption compatibility | `HIGH` | Backups may contain credentials and use versioned Argon2id/AES-GCM formats. | Phase 10. |
| FluxBar regression from shared extraction | `HIGH` | Reusing code can accidentally change proven desktop behavior. | Every phase through unchanged parity/golden tests. |
| Optional shared ID3 parser | `LOW` | Native duplication may be cheaper than a premature shared utility. | Phase 9 decision. |

## Unresolved decisions

The following must remain explicit gates rather than assumptions:

- stable mobile account identity and account switching policy;
- supported Miniflux, iOS, and Android versions;
- custom-header override/security policy;
- `/star` versus `/bookmark` behavior across supported servers;
- mobile pagination order/cursor under concurrent remote changes;
- core versus native persistence of semantic settings;
- progression conflict and completion policy;
- database-only, advisory-file, or host-assisted cross-process coordination;
- final mobile binding and cancellation model;
- historical FluxNews versions that actually shipped;
- conservative handling of ambiguous legacy read/bookmark intent;
- downloaded-file copy, move, or re-index policy;
- unencrypted backup security policy and compatibility promise; and
- whether two native consumers justify shared ID3 parsing.

## Flux repository transition gate

After Phase 2 contracts stabilize and before Phase 3 implementation, separately
approve a repository consolidation. If `Flux` is a rename/transfer destination,
rename or transfer the current FluxBar repository without simultaneously
restructuring source or importing the production Flutter tree. This preserves
the history that created the shared Rust core, macOS client, compatibility
oracle, proofs, and architecture documents while keeping the existing FluxNews
repository authoritative for Flutter maintenance and releases.

If `Flux` is already a distinct populated repository, do not describe copying
commits into it as a transfer/rename: choose and review an explicit history
integration method and separately preserve issues, pull requests, releases,
settings, and automation. In either case, tag/freeze the pre-transition revision,
mirror both repositories, preserve product release automation, and reproduce
the full Rust/Go/macOS/mobile-proof baseline
after the transfer. Directory restructuring is a separate infrastructure-only
gate after Phase 2 contracts are stable and before Phase 3 implementation. The
eventual structure should be product-oriented (`apps/fluxbar/macos`, future
`apps/fluxnews/ios` and `apps/fluxnews/android`), with `core/rust`,
`core/go-reference`, `proofs/mobile-runtime`, `docs`, and `tooling`; do not move
the Flutter production app into Flux during native replacement development.

Use product-scoped tags, changelogs, and workflows so FluxBar, FluxNews, and any
future independently packaged core can release separately. The original
FluxNews issue/PR/tag history remains in its repository; cross-layer native/core
work uses one issue and PR in Flux.

## Concrete recommended sequence

```text
mobile C ABI/runtime proof on iOS and Android (close review findings)
        |
        v
mobile identity/domain/schema + command/query/sync contract
        |\
        | +--> begin read-only Flutter migration characterization
        v
separately approve repository consolidation; no source restructure
        |
        v
versioned local mobile repository + cursor queries
        |
        v
mobile sync + durable read/star mutations
        |
        +--> START NATIVE iOS INBOX DEVELOPMENT
        |
        v
C/JSON vs typed C vs UniFFI evidence gate
        |
        +--> stabilize selected mobile API
        +--> START NATIVE ANDROID PRODUCT DEVELOPMENT
        |
        +--> reader/search/settings/widget-data parity
        +--> headless/cross-process safety
        +--> progression/download/reset contracts
                         |
                         v
native iOS/Android platform and media parity
                         |
                         v
existing Flutter data migration + backup decision
                         |
                         v
full replacement readiness
                         |
                         +--> Go removal only if its oracle value is exhausted
```

FluxBar Rust first-release hardening remains a parallel release track. It does
not become complete merely because the mobile runtime proof or Phase 2 contracts
advance.

The single next implementation task is the narrow Phase 1 closure work listed
in `MOBILE_RUNTIME_PROOF_CONTRACT.md`. Do not begin Phase 2 or transfer the
repository until the coherent proof build and independent gate pass.
