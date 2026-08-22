# Rust Core Migration Testing

## Goal

Compilation is not migration success. The primary risk is behavioral
drift between the existing Go core and the Rust replacement.

## Existing baseline

Keep existing Go validation green throughout the parallel migration.

Use the repository's established Go test/static-check commands and do
not weaken tests to accommodate Rust.

## Rust validation

For Rust work, run at minimum where applicable:

``` sh
cargo fmt --manifest-path rust-core/Cargo.toml --check
cargo check --manifest-path rust-core/Cargo.toml
cargo test --manifest-path rust-core/Cargo.toml
```

Never report a command as successful unless it was executed.

## Rust target prerequisites

The Rust core may require additional cross-compilation targets such as
`x86_64-apple-darwin`. `Build/build-rust-core.sh` checks that each
required target is installed and fails with the exact remediation command
if one is missing. It does not modify the developer or CI toolchain
automatically.

Install a missing target with:

``` sh
rustup target add x86_64-apple-darwin
```

## ABI tests

Validate the compiled boundary, not only Rust internals.

Cover:

-   exported symbols;
-   null request behavior;
-   C-string conversion;
-   malformed JSON;
-   valid UTF-8 response;
-   response ownership;
-   matching `FluxCoreFree`;
-   repeated allocation/free cycles.

Keep `unsafe` isolated and documented.

## Contract fixtures

Suggested structure:

``` text
tests/
└── core-compat/
    ├── requests/
    ├── databases/
    ├── remote-fixtures/
    └── expected/
```

Use sanitized deterministic fixtures.

## Differential testing

For deterministic operations:

``` text
same fixture + same request
          │
      ┌───┴───┐
      │       │
     Go      Rust
      │       │
    output  output
      └───┬───┘
          │
       normalize
          │
        compare
```

Normalize only genuinely nondeterministic values. Do not normalize away
meaningful differences.

## SQLite interoperability

During the parallel period test:

  Producer   Consumer   Required
  ---------- ---------- ----------
  Go         Go         baseline
  Go         Rust       yes
  Rust       Rust       yes
  Rust       Go         yes

Never use the developer's real FluxBar database in automated tests.

Phase 5 persistence validation uses:

``` sh
Build/test-sqlite-compat.sh
```

The script creates fresh databases under `mktemp`, has Go create/write a
database that Rust opens/reads, then has Rust create/write a database that Go
opens/reads. It also compares normalized `sqlite_master` definitions. Both
test helpers reject paths outside the operating system temporary directory.

Rust unit tests additionally verify exact table/column metadata, indexes,
defaults, constraints, absent foreign keys, `user_version=0`, connection
PRAGMAs, 0600 permissions, account isolation, rollback, partial-schema
initialization, Boolean/timestamp encodings, and lossless unknown-status
round trips.

Remote adapter validation (Phase 7) uses:

``` sh
Build/test-remote-compat.sh
```

A deterministic fake Miniflux server feeds identical selections to the
production Go Browse path and the Rust adapter; snapshot JSON must match
exactly for all/unread/starred/category/feed selections, and both sides
must fail on a truncated paginated sequence. Adapter unit tests use an
in-process fake TCP server for auth headers, query construction,
pagination edge cases, error taxonomy, counters/icon/mutation wire
formats, and never require internet access or real credentials.

Phase 6 local-snapshot validation uses:

``` sh
Build/test-snapshot-compat.sh
```

The harness builds deterministic fixture databases with the Go helper
(empty; basic; 205-row limit; two-account isolation), feeds identical
selection/retention inputs to both cores' snapshot implementations, and
compares the JSON semantically. Covered cases include every selection kind
and fallbacks, unreadOnly combinations, retained locally-read entries,
multi-account isolation, the 200-entry boundary with ordering in both
directions, retention of an ID outside the first page, and unknown persisted
status values. Rust unit tests mirror the same semantics directly.

Phase 8 sync/state-machine validation uses:

``` sh
Build/test-sync-compat.sh
```

Each implementation receives a fresh temporary database and fresh stateful
fake Miniflux server. The harness compares response JSON, normalized account/
navigation/selection/entry rows, pending revisions, Undo rows, exact HTTP
request sequences, and final snapshots. Scenarios cover initial/incremental
refresh, new/changed remote state, incomplete pagination, auth/5xx partial
refresh, read/star mutation and reversal, stale remote state with pending
desired values, Undo before/after delivery, discard, full first-mutation
failure, and successful-prefix partial failure.

Phase 9.1 article-processing validation uses:

``` sh
Build/test-article-compat.sh
```

A shared JSON fixture feeds identical article HTML/content, base URLs, and
enclosure lists to the Go and Rust article processors. The harness compares
the produced preview text and resolved image URL exactly. Fixtures cover
empty content, plain text, paragraphs, nested tags, line breaks, HTML
entities, Unicode, whitespace normalization, truncation, malformed HTML,
inline images, relative/absolute/invalid URLs, lazy/responsive attributes,
tiny-image skip, and image-enclosure fallback.

Phase 9.2 localization validation uses:

``` sh
Build/test-localization-compat.sh
```

A shared JSON fixture feeds identical locale preferences, keys, fallbacks,
and plural counts to the Go and Rust localization implementations. The
harness compares the returned text exactly. Fixtures cover English and German
text lookup, unsupported-locale fallback, unknown-key fallback, empty locale
preferences, English and German plural forms, and fallback plural rendering.

The SQLite suite also creates pending read/star and Undo state in each
implementation and has the other implementation continue it. Timer unit tests
use a narrowly injected short automatic delay instead of repeatedly sleeping
10 seconds; production always uses 10 seconds.

## Sync scenarios

Tests should reflect the documented local-first behavior, including:

-   empty/first sync;
-   incremental refresh;
-   remote pagination;
-   failed page leaves local snapshot intact;
-   duplicate/reordered/count-inconsistent page handling;
-   read/unread online;
-   star/unstar online;
-   offline desired mutation;
-   restart with pending mutation;
-   successful later flush;
-   partial flush failure;
-   automatic read delayed flush;
-   Undo before remote delivery;
-   compensating Undo after delivery where applicable.

## Snapshot scenarios

Include:

-   empty inbox;
-   one entry;
-   multiple feeds/categories;
-   unread/all/starred selections;
-   equal timestamps/order stability;
-   missing optional metadata;
-   more than 200 matching rows;
-   schema-version field;
-   current-presentation retention of locally read rows.

## Remote API tests

Most tests should not require a live Miniflux server.

Prefer a local fake/mock HTTP endpoint for:

-   authentication;
-   pagination;
-   non-2xx responses;
-   malformed payload;
-   timeout;
-   partial response/failure.

A live Miniflux smoke test may exist separately.

## Native application smoke tests

At major phases, build/run the native application against the selected
core and verify affected behavior.

When both cores are linkable, use the Go build to isolate suspected Rust
regressions.

## Transport compatibility tests

During the compatibility-api phase, verify that Rust parses and routes
requests the same way Go does at the transport level. Focus on:

- null request;
- invalid/non-UTF-8 input;
- malformed JSON;
- missing operation field;
- unknown operation;
- every supported operation name;
- operation-specific payload fields and defaults;
- response envelope shape (`ok`, `error`, `text`, `snapshot`, `icon`,
  `receipt`).

Domain results are not expected to match until the corresponding business
logic is ported.

A small shell-based smoke test (`Build/test-core-compat.sh`) builds both
cores and links a tiny C caller against each to compare transport-level
behavior for cases that should already match.

For pure domain logic that is not reachable through the C ABI (such as
selection normalization or account ID derivation), use mirrored test
vectors: a Go characterization test locks the reference behavior, and the
Rust side asserts the same expected values. Phase 4 introduced this
pattern for `Selection.Normalized()` (go-core/internal/model) and
`AccountID` (go-core/internal/inbox).

## Difference report

When Go and Rust disagree, record:

1.  fixture/request;
2.  Go behavior;
3.  Rust behavior;
4.  reference code/documentation;
5.  classification:
    -   Rust defect;
    -   undocumented Go behavior;
    -   stale documentation;
    -   legitimate nondeterminism;
    -   proposed intentional behavior change.

Do not silently change the compatibility contract.

## Rust-default gate

Rust must not become the normal core until:

-   relevant Rust tests pass;
-   existing Go tests pass;
-   ABI/contract tests pass;
-   relevant differential tests pass;
-   native macOS build succeeds;
-   manual smoke tests succeed.

Go must not be deleted merely because Rust becomes the default.
