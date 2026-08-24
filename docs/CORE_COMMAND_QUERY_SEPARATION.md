# Core Command--Query Separation

**Status:** Adopted architecture invariant; implementation deferred\
**Scope:** FluxBar Rust core and future shared/mobile core API\
**Origin:** UI behavior observed during FluxBar timeline testing\
**Implementation:** Deferred until current Phase 1 runtime work is
complete

## Context

FluxBar's article list should behave as a stable reading timeline while
the user has the popover open and scrolls through existing entries.

The current behavior couples mutations such as `set_read` to retrieval
of a complete updated snapshot. In practice, automatic **Mark as read on
Scrollover** can therefore cause the client to receive and apply a newly
assembled list while the user is actively scrolling.

This is undesirable for FluxBar's product model. The list is not
intended to behave purely as a dynamically re-queried inbox. It also
behaves like a timeline: while the user is reading it, the spatial
position and ordering of the currently presented entries should remain
stable unless the client deliberately requests new state.

A mutation and a snapshot query are separate responsibilities and should
be represented as such in the core API.

## Decision

Adopt explicit command/query separation for core state operations.

General principle:

> **Commands mutate state; queries return state. A command must not
> implicitly replace the caller's presentation snapshot.**

### `set_read`

`set_read(entry_ids, read)` changes the read state of the supplied
entries.

Its responsibilities are limited to:

-   validating the request;
-   persisting the requested read/unread state;
-   maintaining pending remote mutations where required;
-   maintaining Undo metadata where required;
-   scheduling or otherwise enabling remote delivery according to the
    existing synchronization contract;
-   returning success/failure and only mutation-specific metadata that
    is actually required by the caller.

`set_read` must **not implicitly return a complete entry snapshot merely
because state changed**.

The same principle should be evaluated for other state-changing commands
such as `set_starred` and `undo_read`.

### `get_snapshot`

A dedicated snapshot query returns the current snapshot for a requested
selection/scope.

Conceptually:

``` text
get_snapshot(selection, ...)
    -> current snapshot
```

This operation is explicit and side-effect free from the caller's
perspective.

The existing `local_snapshot` operation may already represent this
responsibility. Whether it should be retained, renamed, or evolved into
`get_snapshot` is an implementation/API-design decision and is
intentionally not fixed by this document.

## Client-controlled behavior

Separating the operations preserves both possible client behaviors.

A client that deliberately wants to rebuild its presentation after a
mutation can perform:

``` text
set_read(...)
get_snapshot(...)
```

A timeline-oriented client such as FluxBar can instead perform:

``` text
set_read(...)
-> update only the affected entry's local presentation state
-> keep the currently presented list stable
```

The core therefore no longer dictates presentation refresh behavior.

## FluxBar timeline stability

For FluxBar, the intended behavior is:

``` text
Popover opens
    |
    v
Client obtains snapshot
    |
    v
Visible timeline is established
    |
    +--> Scrollover marks individual entries read
    |       |
    |       +--> set_read(...)
    |       +--> affected UI state changes locally
    |       +--> no implicit timeline replacement
    |
    +--> User/defined refresh event
            |
            +--> refresh/synchronization
            +--> client deliberately obtains/applies new snapshot
```

Newly synchronized or newly ordered entries should not unexpectedly
appear inside a timeline the user is currently traversing merely because
an unrelated entry was marked read.

The client owns the decision to replace its presented snapshot.

## Why the previous contract is problematic

A setter returning a complete snapshot combines two independent
concerns:

1.  **Command:** change persistent/domain state.
2.  **Query:** retrieve a complete representation of current state.

This coupling:

-   makes mutation behavior unnecessarily expensive;
-   forces presentation refresh semantics onto every client;
-   can destabilize an actively scrolling list;
-   makes it harder to distinguish mutation effects from
    query/reconciliation effects;
-   increases the amount of data crossing the FFI boundary;
-   makes future typed bindings less clear;
-   reduces flexibility for native iOS and Android clients.

The fact that the Go reference implementation may currently behave this
way does not make this behavior a required compatibility property.

## Compatibility decision

If the Go core also couples `set_read` or related mutations to complete
snapshot responses, this specific behavior is **not considered a
compatibility requirement to preserve**.

Go remains a behavioral reference for valid domain, synchronization,
persistence, remote, and compatibility semantics, but known or
intentionally superseded product/API behavior should not constrain the
future shared Rust core.

Compatibility tests affected by this decision should eventually
distinguish between:

-   mutation semantics that must remain compatible; and
-   legacy response-shape/presentation coupling that is intentionally
    removed.

No compatibility tests should be changed until the implementation phase
explicitly adopts this decision.

## Undo

Undo should follow the same separation principle.

An operation such as `undo_read` should perform the requested state
reversal and return only information required to describe that mutation
result.

It should not require a complete snapshot response.

If the UI needs to reconstruct all current state after Undo, it can
explicitly call `get_snapshot()`.

The exact mutation receipt / Undo result type is not defined here.

## `set_starred` and future commands

The same rule should be evaluated consistently for:

-   `set_starred`;
-   `undo_read`;
-   future progression mutations;
-   future feed/category mutations;
-   future download-state commands;
-   other mobile/shared-core commands.

This does not require every command to return nothing. Commands may
return command-specific results, identifiers, receipts, conflict
information, or errors.

They should not implicitly perform unrelated state queries.

## `refresh`

`refresh` belongs to synchronization rather than simple command/query state
access. Future mobile synchronization should return synchronization-specific
completion, change, freshness, or retry metadata; current repository state is
obtained through an explicit query. A legacy compatibility operation may still
transport a snapshot, but the client must choose whether to adopt it and the
future API must not make snapshot replacement an implicit consequence of
synchronization.

The important requirements are that **ordinary local mutations must not
implicitly replace the caller's presentation snapshot** and **synchronization
must not force presentation adoption**.

## Persistent core state and presentation state

Command/query separation also applies to synchronization and presentation.

The persistent core state may advance independently from the state currently
presented by a client. Background synchronization may fetch remote entries,
update counters, reconcile remote baselines, and persist other durable state
without implicitly replacing an active UI snapshot.

Conceptually:

```text
Remote State
     |
     | synchronization
     v
Persistent Core State (SQLite)
     |
     | explicit snapshot query and client adoption
     v
Presentation State (Swift / Kotlin)
```

This enables a client to keep a stable timeline while background work prepares
new data. A later user-initiated refresh may simply adopt the latest local
snapshot when that state is sufficiently fresh, without necessarily performing
another network request.

The exact freshness policy is intentionally not defined here. A client may
choose to adopt local state immediately, synchronize first, or keep its current
presentation. The important invariant is:

> **Sync is not UI refresh. Persistent core state is not presentation state.**

Counters follow the same presentation rule. Synchronization may update durable
counter values without forcing counters already displayed by an active client
to change until that client deliberately adopts a newer snapshot/state.

This separation is particularly relevant to future native mobile clients,
background tasks, widgets, and other independent presentation surfaces. Each
surface may consume the same persistent core state on its own lifecycle without
requiring all surfaces to advance their presentation simultaneously.

The names and final contracts of synchronization/query operations are not fixed
by this decision. In particular, this document does not require an operation to
be named `sync_remote`, `get_snapshot`, or `refresh_ui`.

## FFI and future bindings

This decision should be considered when designing the future mobile API
and evaluating:

-   existing C ABI + JSON;
-   typed C ABI;
-   UniFFI.

No binding architecture is selected here.

A future typed API should ideally expose the semantic distinction
directly, for example:

``` text
Commands
    set_read(...)
    set_starred(...)
    undo_read(...)

Queries
    get_snapshot(...)
    get_feed_icon(...)

Synchronization
    refresh(...)
    flush_pending(...)
```

This separation should make Swift and Kotlin integration clearer
regardless of the selected binding technology.

## Mobile relevance

The decision is intended to apply beyond FluxBar.

Future native FluxNews clients should be able to:

-   maintain stable presentation state;
-   apply optimistic/local mutations without receiving an unrelated full
    dataset;
-   decide independently when to query current state;
-   minimize unnecessary FFI payloads;
-   express command/query semantics naturally through typed bindings.

The final mobile schema and binding architecture remain separate
decisions.

## Relationship to the observed FluxBar UI issue

During manual FluxBar testing, entries were observed occasionally
jumping relative to neighboring entries while **Mark as read on
Scrollover** was active.

Runtime logging showed that automatic `set_read` operations were
followed by complete snapshot queries and complete entry-list responses.

This architecture is considered undesirable regardless of whether it is
ultimately proven to be the direct cause of the observed visual jump.

Therefore:

-   fixing command/query separation should not depend on proving that it
    is the sole cause of the UI bug;
-   ordering and SwiftUI reconciliation should still be investigated if
    jumping remains after the separation is implemented.

## Non-goals

This decision does **not** currently:

-   change the existing C/JSON ABI;
-   select UniFFI;
-   define the final mobile API;
-   define the final mobile schema;
-   change synchronization semantics;
-   change pending mutation semantics;
-   change Undo semantics;
-   change database schema;
-   remove Go;
-   change the legacy FluxBar `refresh` response contract;
-   implement the FluxBar UI fix.

Those changes require their own implementation/review phases.

## Implementation timing

Record this decision now so that ongoing mobile runtime, schema,
synchronization, and binding work does not accidentally treat
mutation-to-snapshot coupling as a required architectural property.

Actual implementation should wait until Phase 1 runtime closure. Phase 2 owns
the durable command/query/synchronization and counter contracts; Phase 3
implements the local repository and query side of the separation; Phase 4
implements durable commands and sync-only outcomes plus explicit query/adoption. Any FluxBar
response-contract change remains a separate desktop compatibility task and must
complete before a Rust-backed public release if the active-timeline issue is
still present.

Before implementation:

1.  Audit current Rust mutation handlers and response types.
2.  Audit equivalent Go behavior.
3.  Audit all Swift callers and identify where complete mutation
    responses are consumed.
4.  Identify Undo and pending-mutation dependencies.
5.  Define the smallest safe response-contract change.
6.  Update compatibility tests to reflect intentional behavior changes
    rather than blindly preserving legacy coupling.
7.  Preserve the ability for clients to explicitly reproduce the old
    behavior through `set_*()` followed by `get_snapshot()`.

## Acceptance criteria for the future implementation

The eventual implementation should satisfy all of the following:

-   `set_read` correctly persists read and unread changes.
-   Pending remote mutation behavior remains correct.
-   Undo remains correct.
-   `set_read` does not implicitly require or return a complete
    presentation snapshot.
-   Snapshot retrieval remains available through an explicit query.
-   FluxBar can update the affected entry locally without replacing the
    active timeline.
-   A client can deliberately reproduce the previous behavior by issuing
    a snapshot query after the mutation.
-   `set_starred` and related commands are reviewed for the same
    separation.
-   Existing synchronization correctness is preserved.
-   No unnecessary Go behavior is retained solely for compatibility when
    it conflicts with this decision.
-   The decision is incorporated into later mobile binding/API
    architecture work.
