# FluxBar Core Compatibility Contract

## Purpose

This is the migration contract between the current Go core and the
parallel Rust implementation.

The repository's existing Go implementation is authoritative where this
document is incomplete. Update this document from inspected code before
porting an operation. Do not infer missing behavior.

This contract supplements, rather than replaces:

-   `ARCHITECTURE_DECISIONS.md`
-   `DEVELOPER_MAP.md`
-   `features/SYNC_AND_DATA.md`

## Current native boundary

FluxBar Desktop currently links the Go core as a C archive. The native
client exchanges JSON through a narrow C ABI and explicitly releases
core-allocated response strings.

The initial Rust core must preserve the callable behavior of:

``` c
FluxCoreRequest(...)
FluxCoreFree(...)
```

The exact generated C signature and nullability must be copied from the
current Go export/header during the contract audit.

## Memory ownership

The migration must explicitly verify and document:

1.  ownership of the request buffer;
2.  whether the core retains any input pointer;
3.  allocation ownership of the response;
4.  the required matching free operation;
5.  null-pointer behavior;
6.  invalid UTF-8/C-string behavior where relevant.

Rust `unsafe` code must remain confined to the FFI adapter and every
unsafe assumption must be documented.

## JSON compatibility

The Rust compatibility adapter must initially preserve:

-   operation names;
-   field names;
-   optional/null behavior;
-   defaults;
-   response shape;
-   error shape;
-   snapshot version behavior;
-   partial-success semantics.

Rust-internal naming may differ using serialization attributes.

## Known current bridge capabilities

Repository documentation currently states that the bridge supports:

-   configuration;
-   refresh;
-   read/star mutations;
-   feed icons;
-   localization;
-   versioned browse snapshots (current documented schema version: 1).

The exact operation inventory and request/response schemas must be
audited from `internal/coreapi` before implementation. Do not treat this
summary as exhaustive.

## Data/sync compatibility that must be preserved

From the current product documentation:

-   SQLite is account-scoped operational state.
-   Local snapshots render before network synchronization.
-   Effective read/star state and desired mutations are committed
    transactionally.
-   Pending changes survive connectivity failure.
-   Rows marked read locally remain visible in the current unread
    presentation until an explicit presentation refresh/context change.
-   Automatic-read changes use delayed flush semantics so Undo normally
    precedes remote delivery.
-   Remote selection sync uses ascending entry-ID cursor pagination.
-   Remote selections are fully paginated in 200-entry pages before
    negative reconciliation.
-   Only an exact fully loaded remote ID set is treated as complete
    absence information.
-   Failed/duplicated/reordered/count-inconsistent pages leave the last
    local snapshot intact.
-   The local popover snapshot is capped at 200 rows.
-   Feed-icon bytes are not part of browse snapshots and are not
    currently persisted to disk.
-   Browse snapshot compatibility is versioned; the documented current
    schema version is 1.

These are compatibility requirements unless code inspection shows the
documentation is stale. If code and docs differ, report the discrepancy
before deciding which behavior Rust should reproduce.

## Operation audit template

Complete one section per actual operation discovered in the Go
dispatcher.

### `<operation>`

**Input fields**

``` json
{}
```

**Success response**

``` json
{}
```

**Failure response**

``` json
{}
```

**Timeout/deadline**

-   TBD from Go implementation.

**Local side effects**

-   TBD.

**SQLite effects**

-   TBD.

**Remote effects**

-   TBD.

**Partial-success behavior**

-   TBD.

**Reference implementation/tests**

-   TBD.

## Error cases to audit

At minimum inspect:

-   null request;
-   malformed JSON;
-   unknown operation;
-   missing/invalid configuration;
-   authentication failure;
-   network failure;
-   timeout;
-   database failure;
-   malformed Miniflux response;
-   partial sync failure.

Do not improve error semantics during the compatibility port.

## Database interoperability

The initial Rust implementation must use the existing SQLite
representation.

Required directionality during the parallel period:

``` text
Go writes   -> Rust reads
Rust writes -> Go reads
```

Do not redesign schema, migration metadata, pending-mutation encoding,
or account scoping as part of the language migration.

## Contract change rule

During the compatibility migration:

1.  discover behavior;
2.  document it;
3.  test it;
4.  reproduce it in Rust.

Any deliberate product/contract change is a separate task requiring an
explicit decision.
