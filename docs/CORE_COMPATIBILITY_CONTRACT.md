# FluxBar Core Compatibility Contract

## Purpose

This is the migration contract between the current Go core and the parallel
Rust implementation.

The repository's existing Go implementation is authoritative where this
document is incomplete. Update this document from inspected code before
porting an operation. Do not infer missing behavior.

This contract supplements, rather than replaces:

-   ARCHITECTURE_DECISIONS.md
-   DEVELOPER_MAP.md
-   features/SYNC_AND_DATA.md
-   RUST_CORE_MIGRATION.md
-   RUST_CORE_TESTING.md

## Current native boundary

FluxBar Desktop currently links the Go core as a C archive. The native
client exchanges JSON through a narrow C ABI and explicitly releases
core-allocated response strings.

The initial Rust core must preserve the callable behavior of:

``` c
extern char* FluxCoreRequest(char* request);
extern void FluxCoreFree(char* value);
```

These signatures are taken from the C header produced by
go-core/cmd/fluxcore/main.go (go build -buildmode=c-archive). The pointer types
are mutable char* even though the request buffer is not modified by the
core.

The Swift caller (macos/FluxBar/GoCore.swift) passes a UTF-8 C string via
json.withCString, decodes the returned pointer with String(cString:), and
always calls FluxCoreFree in a defer.

## Memory ownership

Facts taken from go-core/cmd/fluxcore/main.go:

1.  **Request buffer ownership:** the native caller owns the request buffer.
    The core reads it synchronously and does not retain the pointer.
2.  **Core retains no input pointer:** C.GoString copies the C string into
    Go memory; the input *C.char is not kept.
3.  **Response allocation ownership:** every successful and error response
    is allocated by C.CString from the C heap in the core's address space.
4.  **Required matching free operation:** the caller must release the
    response pointer by calling FluxCoreFree, which executes C.free.
5.  **Null-pointer behavior:**
    -   FluxCoreRequest(nil) returns a core-owned string containing
        {"ok":false,"error":"null request"}.
    -   FluxCoreFree(nil) is a no-op.
6.  **Invalid UTF-8 / non-C-string behavior:** not explicitly handled. The
    Go implementation uses C.GoString, which expects a null-terminated
    byte sequence. Rust must define and document its own behavior for
    non-UTF-8 input; a safe choice is to treat it like malformed JSON and
    return an invalid-request error.

Rust unsafe code must remain confined to the FFI adapter and every
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

## Operation inventory

The dispatcher in go-core/internal/coreapi/api.go handles the following operations.
All requests share a single JSON envelope (Request) and all responses share
a single JSON envelope (Response). Unknown operations return an error of
form unsupported operation "op".

Operations that require an engine (i.e. a successful prior configure)
return Miniflux is not configured when no engine exists:
local_snapshot, refresh, set_read, set_starred, undo_read,
discard_undo, flush_pending, feed_icon.

### Common request envelope

go-core/internal/coreapi/api.go defines Request with these JSON fields:

-   operation (string, required)
-   server (string)
-   apiKey (string)
-   newestFirst (bool)
-   configurationGeneration (int64)
-   locales (array of strings)
-   key (string)
-   fallback (string)
-   oneFallback (string)
-   otherFallback (string)
-   count (int)
-   selection (object: kind, id, unreadOnly)
-   entryID (int64)
-   entryIDs (array of int64)
-   retainEntryIDs (array of int64)
-   read (bool)
-   mutationSource (string)
-   mutationID (string)
-   currentStarred (bool)
-   desiredStarred (bool)
-   feedID (int64)
-   feedName (string)

currentStarred is present in the envelope but is not used by the Go
dispatcher. The Swift client does not send it for set_starred; the core
re-reads remote state before deciding whether to toggle.

### Common response envelope

go-core/internal/coreapi/api.go defines Response with these JSON fields:

-   ok (bool, always present)
-   error (string, omitted when empty)
-   text (string, omitted when empty)
-   snapshot (object)
-   icon (object with regular and dark byte arrays encoded as base64 JSON
    strings, following Go's `[]byte` encoding; empty variants are omitted)
-   receipt (object with id and count)

### configure

**Input fields**

-   operation: "configure"
-   server: complete HTTP or HTTPS URL
-   apiKey: non-empty API key
-   newestFirst: publication sort order (default false / oldest first)
-   configurationGeneration: monotonic configuration generation
-   locales: ordered BCP-47 locale preferences

Validation:

-   server is trimmed, trailing slashes are removed, and the result must
    parse as http or https with a non-empty host.
-   apiKey is trimmed and must be non-empty after trimming.
-   configurationGeneration below the active generation prevents replacement
    of the active service. Validation, store opening, and account upsert happen
    before that check and remain observable.
-   Validation error messages are localized using the supplied locales.

**Success response**

ok set to true.

**Failure response**

ok set to false and error contains a localized validation message.

**Timeout/deadline:** no explicit timeout is applied.

**Local side effects:**

-   Lazily creates the SQLite store under the user config directory at
    UserConfigDir/FluxBar/inbox.sqlite3.
-   Ensures the account row exists via EnsureAccount.
-   Creates an inbox.Service with a Miniflux remote client if the
    generation is current.

**SQLite effects:** INSERT into accounts(id, server) with
`ON CONFLICT(id) DO UPDATE SET server=excluded.server`, preserving existing
counters/timestamps. The account ID is SHA256(server + NUL byte + apiKey),
hex-encoded.

**Remote effects:** none during configure.

**Partial-success behavior:** none.

**Reference implementation/tests:**

-   go-core/internal/coreapi/api.go case "configure"
-   go-core/internal/coreapi/api_test.go TestConfigureValidatesCredentials

### local_snapshot

**Input fields**

-   operation: "local_snapshot"
-   selection: ArticleSelection-like object
-   retainEntryIDs: array of entry IDs to keep visible

**Success response**

ok true and snapshot present. The snapshot version is always 1.

**Failure response**

ok false with error "Miniflux is not configured" or a database error.

**Timeout/deadline:** 5 seconds.

**Local side effects:** reads SQLite only.

**SQLite effects:** queries entries, categories, feeds, selection_totals,
and pending_mutations for the configured account.

**Remote effects:** none.

**Partial-success behavior:** none.

**Reference implementation/tests:**

-   go-core/internal/coreapi/api.go case "local_snapshot"
-   go-core/internal/inbox/store.go Snapshot
-   go-core/internal/inbox/store_test.go

### refresh

**Input fields**

-   operation: "refresh"
-   selection: ArticleSelection-like object
-   retainEntryIDs: array of entry IDs to keep visible

**Success response**

ok true and snapshot present. Snapshot version is always 1.

**Failure response**

ok false and error present.

Partial success: ok true, error present, and snapshot present. This occurs
when remote sync fails but a usable local snapshot with version greater
than 0 can still be produced.

**Timeout/deadline:** 45 seconds.

**Local side effects:** flushes pending mutations, applies the remote
snapshot if available, and returns the resulting local snapshot.

**SQLite effects:** may update accounts, categories, feeds, entries,
selection_totals, and remove acknowledged pending_mutations.

**Remote effects:** fetches counters, categories, feeds, and paginated
entries via Miniflux. Remote selection uses ascending entry-ID cursor
pagination in 200-entry pages. A failed, duplicated, reordered, or
count-inconsistent page causes the whole refresh to fail while leaving the
last local snapshot intact.

**Partial-success behavior:** yes; see above.

**Reference implementation/tests:**

-   go-core/internal/coreapi/api.go case "refresh"
-   go-core/internal/inbox/service.go Sync
-   go-core/internal/miniflux/service.go Browse, fetchCompleteSelection
-   go-core/internal/inbox/service_test.go TestSyncWritesAndRetainsLocalSnapshotOffline

### set_read

**Input fields**

-   operation: "set_read"
-   selection: ArticleSelection-like object
-   entryID: single entry ID (fallback when entryIDs is empty)
-   entryIDs: array of entry IDs to mutate
-   retainEntryIDs: array of entry IDs to keep visible
-   read: desired read state
-   mutationSource: "automatic" for scrollover reads, otherwise manual

entryIDs is preferred. If entryIDs is empty and entryID is greater than
zero, a single-ID list is used. At least one ID is required.

**Success response**

ok true and snapshot present. For undoable (automatic) batches that
actually change at least one row, a receipt object is included with id and
count fields. Manual reads and zero-count automatic batches do not include
a receipt.

**Failure response**

ok false with an error message.

**Timeout/deadline:** 5 seconds.

**Local side effects:** updates local read state, creates or updates
pending_mutations for read, and creates undo_batches / undo_items when
undoable.

**SQLite effects:** updates entries.status, inserts/updates
pending_mutations, and inserts undo_batches/undo_items when undoable.

**Remote effects:** none during the request; pending mutations are flushed
asynchronously via ScheduleFlush (immediate for manual, 10-second delay for
automatic).

**Partial-success behavior:** none; the operation fails atomically if the
database transaction cannot be committed.

**Reference implementation/tests:**

-   go-core/internal/coreapi/api.go case "set_read"
-   go-core/internal/inbox/service.go MarkRead
-   go-core/internal/inbox/store.go SetRead
-   go-core/internal/inbox/service_test.go TestSequentialAutomaticReadBatchesRetainEarlierRows

### set_starred

**Input fields**

-   operation: "set_starred"
-   selection: ArticleSelection-like object
-   entryID: entry ID to mutate
-   retainEntryIDs: array of entry IDs to keep visible
-   desiredStarred: desired starred state

**Success response**

ok true and snapshot present.

**Failure response**

ok false with an error message.

**Timeout/deadline:** 5 seconds.

**Local side effects:** updates local starred state, creates or updates a
pending mutation for starred, and schedules an immediate flush.

**SQLite effects:** updates entries.starred and inserts/updates
pending_mutations.

**Remote effects:** none during the request; flush re-reads remote state
before toggling to implement desired-state semantics.

**Partial-success behavior:** none.

**Reference implementation/tests:**

-   go-core/internal/coreapi/api.go case "set_starred"
-   go-core/internal/inbox/service.go SetStarred
-   go-core/internal/inbox/store.go SetStarred
-   go-core/internal/inbox/service_test.go TestStarredReconciliationUsesRemoteDesiredState

### undo_read

**Input fields**

-   operation: "undo_read"
-   selection: ArticleSelection-like object
-   mutationID: undo batch ID from a previous automatic read receipt
-   retainEntryIDs: array of entry IDs to keep visible

**Success response**

ok true and snapshot present.

**Failure response**

ok false with an error message.

**Timeout/deadline:** 5 seconds.

**Local side effects:** restores the prior read state recorded in the undo
batch, updates pending_mutations, and schedules an immediate flush.

**SQLite effects:** updates entries.status, inserts/updates
pending_mutations, and deletes the undo batch and its items.

**Remote effects:** none during the request.

**Partial-success behavior:** none.

**Reference implementation/tests:**

-   go-core/internal/coreapi/api.go case "undo_read"
-   go-core/internal/inbox/service.go Undo
-   go-core/internal/inbox/store.go Undo

### discard_undo

**Input fields**

-   operation: "discard_undo"
-   mutationID: undo batch ID to discard

**Success response**

ok true.

**Failure response**

ok false with an error message.

**Timeout/deadline:** 5 seconds.

**Local side effects:** removes the undo batch and its items without
mutating entry read state.

**SQLite effects:** deletes from undo_batches and undo_items.

**Remote effects:** none.

**Partial-success behavior:** none.

**Reference implementation/tests:**

-   go-core/internal/coreapi/api.go case "discard_undo"
-   go-core/internal/inbox/service.go DiscardUndo
-   go-core/internal/inbox/store.go DiscardUndo

### flush_pending

**Input fields**

-   operation: "flush_pending"
-   selection: ArticleSelection-like object
-   retainEntryIDs: array of entry IDs to keep visible

**Success response**

ok true and snapshot present (from local_snapshot after flush).

**Failure response**

ok false with an error message.

**Timeout/deadline:** 30 seconds.

**Local side effects:** flushes all pending read/star mutations to
Miniflux, acknowledges successful mutations, and returns a local snapshot.

**SQLite effects:** updates entries.remote_status and entries.remote_starred,
updates feeds.remote_unread_count, accounts.remote_starred_total, and
selection_totals, and deletes acknowledged pending_mutations.

**Remote effects:** sends SetReadBatch, EntryState, and ToggleStarred calls.

**Partial-success behavior:** successful mutations are acknowledged one by
one. If a later mutation fails, that mutation and all unattempted suffix
mutations remain pending, but previously successful mutations have already
been acknowledged and removed. There is no transaction spanning the full
flush or its remote calls.

**Reference implementation/tests:**

-   go-core/internal/coreapi/api.go case "flush_pending"
-   go-core/internal/inbox/service.go Flush, flushPending
-   go-core/internal/inbox/store.go Acknowledge

### feed_icon

**Input fields**

-   operation: "feed_icon"
-   feedID: feed identifier
-   feedName: feed name used for diagnostics

**Success response**

ok true and icon present. The optional regular and dark fields are base64 JSON
strings and are omitted when the corresponding variant is empty.

**Failure response**

ok false with "Miniflux is not configured". Once configured, remote, missing,
and processing failures are collapsed into a successful empty icon payload.

**Timeout/deadline:** 15 seconds.

**Local side effects:** reads from an in-memory icon cache and deduplicates
concurrent loads for the same feed ID.

**SQLite effects:** none; feed icons are not persisted.

**Remote effects:** fetches the feed icon from Miniflux and normalizes it
to a square PNG. A dark-mode variant is generated when the icon is dark
and transparent.

**Partial-success behavior:** a missing or unprocessable icon returns an empty
icon object rather than an error.

**Reference implementation/tests:**

-   go-core/internal/coreapi/api.go case "feed_icon"
-   go-core/internal/inbox/service.go FeedIcon
-   go-core/internal/miniflux/service.go FeedIcon, icon
-   go-core/internal/icons/icons.go

### localize

**Input fields**

-   operation: "localize"
-   locales: ordered BCP-47 preferences
-   key: translation key
-   fallback: fallback text

**Success response**

ok true and text present with the localized string.

**Failure response**

ok false if the localization bundle cannot be loaded.

**Timeout/deadline:** none.

**Local side effects:** loads the shared go-i18n bundle from embedded JSON
files in go-core/internal/localization/translations.

**SQLite effects:** none.

**Remote effects:** none.

**Partial-success behavior:** falls back to the supplied fallback text when
no catalog match exists.

**Reference implementation/tests:**

-   go-core/internal/coreapi/api.go case "localize"
-   go-core/internal/localization/localization.go
-   go-core/internal/coreapi/api_test.go TestLocalizeRequest
-   Build/test-localization-compat.sh (Phase 9.2 differential harness)

### localize_plural

**Input fields**

-   operation: "localize_plural"
-   locales: ordered BCP-47 preferences
-   key: translation key
-   oneFallback: fallback for count of one
-   otherFallback: fallback for other counts
-   count: integer count

**Success response**

ok true and text present with the pluralized localized string.

**Failure response**

ok false if the localization bundle cannot be loaded.

**Timeout/deadline:** none.

**Local side effects:** same as localize.

**SQLite effects:** none.

**Remote effects:** none.

**Partial-success behavior:** falls back to oneFallback / otherFallback when
no catalog match exists.

**Reference implementation/tests:**

-   go-core/internal/coreapi/api.go case "localize_plural"
-   go-core/internal/localization/localization.go
-   Build/test-localization-compat.sh (Phase 9.2 differential harness)

### Unknown operations

Any operation value other than the ones above returns:

-   ok: false
-   error: unsupported operation "op"

This is produced by the default branch of the dispatcher.

If the `operation` field is absent from the request JSON, the Go core
 deserializes it as an empty string and returns
`unsupported operation ""`. The Rust compatibility layer matches this
behavior.

## Snapshot schema

model.BrowseSnapshot is serialized with these JSON fields:

-   version (int): currently always 1
-   selection (Selection: kind, id, unreadOnly)
-   entries (array of Entry)
-   categories (array of Category)
-   total (int)
-   unreadTotal (int)
-   starredTotal (int)

Entry fields:

-   id (int64)
-   title (string)
-   url (string)
-   commentsURL (string)
-   feedID (int64)
-   feedName (string)
-   categoryID (int64)
-   publishedAt (RFC3339Nano timestamp)
-   preview (string)
-   imageURL (string)
-   status (string, "read" or "unread")
-   starred (bool)
-   icon (byte array)
-   darkIcon (byte array)

Category fields:

-   id (int64)
-   title (string)
-   unreadCount (int)
-   feeds (array of Feed)

Feed fields:

-   id (int64)
-   title (string)
-   categoryID (int64)
-   unreadCount (int)

Selection normalization (model.Selection.Normalized):

-   "all" keeps id and unreadOnly.
-   "unread" and "starred" drop id.
-   "category" and "feed" require id greater than 0; otherwise fall back to
    all with unreadOnly true.
-   Any other kind falls back to all with unreadOnly true.

The local snapshot limits returned entries to 200 rows, but total may
reflect the full remote count for the selection.

## Article processing compatibility

Observed from `go-core/internal/article/text.go` and
`go-core/internal/miniflux/service.go`:

-   Preview extraction happens during `mapEntries`, before the entry is written
    to SQLite. The Go core calls `article.Extract(source.Content, source.URL,
    article.PreviewLimit)`.
-   `PreviewLimit` is 600 runes.
-   Preview text is built by walking the HTML tree depth-first:
    -   `<script>`, `<style>`, `<head>`, `<noscript>`, `<template>` and their
        descendants are ignored.
    -   Text nodes are appended verbatim.
    -   `<br>` emits a newline.
    -   `<img alt="...">` emits the alt text surrounded by single spaces.
    -   Block elements emit a newline before and after their children.
-   After the walk, the result is split on newlines; each line is collapsed to
    a single space between whitespace-separated tokens; empty lines are
    removed; remaining lines are joined with `\n`.
-   The final text is truncated to `limit - 1` runes with `strings.TrimSpace`,
    then suffixed with the Unicode ellipsis `…` (U+2026) when longer than the
    limit.
-   Image extraction also walks the tree depth-first and considers `<img>` and
    `<source>` elements. An element is skipped when both `width` and `height`
    attributes are present, parseable as non-negative integers, and `<= 2`.
-   Image attribute priority: `data-src`, `data-original`, `src`,
    `data-srcset`, `srcset`.
-   For `srcset`/`data-srcset` values, the candidate is the first whitespace-
    separated token of the last comma-separated part that has any token.
-   URL resolution trims whitespace, rejects `data:` URLs case-insensitively,
    resolves relative references against the article URL, and accepts only
    `http` and `https` schemes.
-   If no inline image resolves, the first Miniflux enclosure whose trimmed,
    lowercased MIME type (part before `;`) starts with `image/` and whose URL
    resolves to HTTP(S) is used.
-   Processed `preview` and `image_url` values are persisted in the `entries`
    table and round-trip through both Go and Rust stores.

## Data/sync compatibility that must be preserved

Observed from the implementation:

-   SQLite is account-scoped operational state.
-   Local snapshots render before network synchronization.
-   Effective read/star state and desired mutations are committed
    transactionally.
-   Pending changes survive connectivity failure.
-   Rows marked read locally remain visible in the current unread
    presentation until an explicit presentation refresh or context change.
-   Automatic-read changes use a 10-second delayed flush so Undo normally
    precedes remote delivery.
-   Remote selection sync uses ascending entry-ID cursor pagination.
-   Remote selections are fully paginated in 200-entry pages before
    negative reconciliation.
-   Only an exact fully loaded remote ID set is treated as complete
    absence information.
-   Failed/duplicated/reordered/count-inconsistent pages leave the last
    local snapshot intact.
-   The local popover snapshot is capped at 200 rows.
-   Presentation retention is caller-driven: `retainEntryIDs` are ORed into
    the entry query together with the selection clause before the 200-row
    limit, so a locally read entry stays visible while its ID is retained by
    the native client.
-   Snapshot navigation ordering uses SQLite `COLLATE NOCASE` (ASCII-only),
    not full Unicode case folding.
-   An empty category list marshals as JSON `null` (Go nil slice).
-   Feed-icon bytes are not part of browse snapshots and are not persisted
    to disk.
-   Browse snapshot compatibility is versioned; the current schema version
    is 1.

## Error cases

Observed behavior for the audited error cases:

-   **Null request:** FluxCoreRequest(nil) returns
    {"ok":false,"error":"null request"}.
-   **Malformed JSON:** returns {"ok":false,"error":"invalid request: ..."}
    with the JSON parser error appended.
-   **Unknown operation:** returns unsupported operation "op".
-   **Missing/invalid configuration:** configure returns a localized error
    for invalid server URL or empty API key.
-   **Not configured:** operations requiring an engine return
    "Miniflux is not configured".
-   **Authentication failure:** propagated from the Miniflux client as an
    error string during refresh/flush.
-   **Network failure:** refresh returns partial success with a local
    snapshot when one exists; flush returns a hard error.
-   **Timeout:** enforced by context.WithTimeout per operation.
-   **Database failure:** returned as an error string, typically in German
    because the Go code uses German error prefixes.
-   **Malformed Miniflux response:** treated as a hard error by the
    pagination verifier.
-   **Partial sync failure:** flush stops at the first failed mutation;
    refresh may return partial success with a local snapshot.

## Operation timeouts

Deadlines observed in go-core/internal/coreapi/api.go:

-   configure: none
-   local_snapshot: 5 seconds
-   refresh: 45 seconds
-   set_read: 5 seconds
-   set_starred: 5 seconds
-   undo_read: 5 seconds
-   discard_undo: 5 seconds
-   flush_pending: 30 seconds
-   feed_icon: 15 seconds
-   localize / localize_plural: none

## Database initialization and location

The SQLite store is opened lazily by Runtime.currentStore:

-   Path: UserConfigDir/FluxBar/inbox.sqlite3
-   Open flags include _busy_timeout=5000, _foreign_keys=on,
    _journal_mode=WAL, _synchronous=NORMAL.
-   MaxOpenConns is set to 1.
-   The parent directory is created with 0700 permissions.
-   The database file is chmod 0600 after opening.
-   Schema is created via CREATE TABLE IF NOT EXISTS in
    go-core/internal/inbox/store.go migrate().
-   There is no migration table and no database schema version. SQLite
    `PRAGMA user_version` remains 0. The snapshot JSON version is unrelated.
-   The schema declares no foreign keys. Foreign-key enforcement is still
    enabled as part of the connection contract.

Tables created by the Go core:

-   accounts (id, server, remote_starred_total, last_sync_at)
-   categories (account_id, id, title)
-   feeds (account_id, id, category_id, title, remote_unread_count)
-   selection_totals (account_id, kind, selection_id, unread_only, total)
-   entries (account_id, id, title, url, comments_url, feed_id, feed_name,
    category_id, published_at, preview, image_url, remote_status,
    remote_starred, status, starred)
-   pending_mutations (account_id, entry_id, field, desired, revision,
    updated_at)
-   undo_batches (account_id, id, created_at)
-   undo_items (account_id, batch_id, entry_id, prior_read)

Declared secondary indexes:

-   entries_account_published (account_id, published_at)
-   entries_account_feed (account_id, feed_id, published_at)
-   entries_account_category (account_id, category_id, published_at)

The only explicit CHECK constraint is pending_mutations.field IN
('read', 'starred'). Boolean values are stored as SQLite INTEGER values.
Timestamps are UTC RFC3339Nano strings.

The intended entry-status domain is read/unread, but the physical Go/SQLite
representation is open: status columns have no CHECK constraint and Go scans
arbitrary strings without validation. Rust therefore preserves unknown status
values losslessly instead of rejecting or normalizing databases accepted by
Go.

Account ID derivation:

-   sha256(server + "\x00" + apiKey) rendered as lowercase hex.

During the parallel migration the schema, migration metadata, pending-
mutation encoding, and account scoping must not be redesigned.

## Database interoperability

The initial Rust implementation must use the existing SQLite
representation.

Required directionality during the parallel period:

-   Go writes -> Rust reads
-   Rust writes -> Go reads

Do not redesign schema, migration metadata, pending-mutation encoding,
or account scoping as part of the language migration.

Phase 5 uses the same unversioned schema through a synchronous, single-
connection Rust adapter. The adapter receives an explicit path; production
path discovery remains outside portable persistence. Interoperability tests
use temporary databases only and cover both Go-write/Rust-read and
Rust-write/Go-read directions.

## Phase 8 synchronization state model

The Go implementation does not store one abstract entry state. The persisted
relationship is:

-   `entries.remote_status` / `remote_starred`: last observed or acknowledged
    remote baseline;
-   `entries.status` / `starred`: effective local/presentation value;
-   `pending_mutations`: one desired Boolean per account/entry/field, with a
    replacement revision and update timestamp;
-   `undo_batches` / `undo_items`: an automatic-read batch and each changed
    entry's prior read Boolean;
-   snapshot membership: effective state plus caller-supplied retained IDs,
    independent from Undo membership.

Applying a remote snapshot always advances remote baselines for returned
entries. It replaces effective read/star values only when no pending row for
that field exists. A complete unread result negatively reconciles missing
remote-unread rows to read; a complete starred result similarly clears remote
starred state. Category/feed scope is preserved. Incomplete or failed
pagination is never passed to persistence and therefore cannot trigger this
negative reconciliation.

Local read/star mutations update effective state and upsert their pending row
in one transaction. Replacement changes `desired`, increments `revision`, and
updates `updated_at`; rows are ordered by `updated_at` for flush. Acknowledge
updates the remote baseline and counters, then deletes only the exact observed
revision, protecting a superseding concurrent value.

Automatic reads create Undo rows only for entries whose effective status
actually changes. Undo restores each recorded prior Boolean, upserts the
corresponding desired read mutation (including a compensating mutation after
remote delivery), and deletes the batch/items atomically. Discard deletes only
Undo metadata. Manual reads remove Undo membership for affected entries and
clean empty batches.

One per-service delayed flush schedule is reset by every scheduled mutation.
Automatic reads schedule 10 seconds; manual reads, stars, and Undo schedule
immediately. Explicit flush does not cancel a scheduled callback. The native
Undo affordance remains 8 seconds, preserving the intentional two-second
buffer. A reconfigured service remains account-bound, so an already scheduled
callback can only flush the account that created it.

## Existing test coverage

Go tests that exercise compatibility-relevant behavior:

-   go-core/internal/coreapi/api_test.go
    -   TestLocalizeRequest
    -   TestInvalidAndUnconfiguredRequests
    -   TestConfigureValidatesCredentials
-   go-core/internal/inbox/service_test.go
    -   TestSyncWritesAndRetainsLocalSnapshotOffline
    -   TestStarredReconciliationUsesRemoteDesiredState
    -   TestSequentialAutomaticReadBatchesRetainEarlierRows
-   go-core/internal/inbox/store_test.go
    -   TestStorePersistsLocalSnapshotAndUndo
    -   TestApplySnapshotPreservesPendingDesiredState
    -   TestAcknowledgementKeepsNavigationCountsStable
    -   TestSnapshotPreservesRemoteTotalBeyondCachedRows
    -   TestCompleteUnreadSnapshotReconcilesExternallyReadEntry
    -   TestCompleteEmptyUnreadSnapshotReconcilesAllEntries
    -   TestIncompleteUnreadSnapshotDoesNotReconcileMissingEntry
    -   TestCompleteFilteredSnapshotReconcilesOnlyItsScope
    -   TestCompleteLargeSnapshotReconcilesWithoutSQLiteParameterLimit

These tests cover: localization, JSON validation, configuration
validation, offline snapshot retention, starred desired-state semantics,
automatic-read batch retention, persistence, undo, pending preservation,
acknowledgement accounting, remote totals beyond cached rows, complete and
incomplete reconciliation, filtered reconciliation scope, and large
snapshot handling.

## Code/documentation discrepancies

The following differences were found between the implementation and
product documentation. The formal compatibility decisions below treat
these as reference behavior to reproduce rather than defects to fix.

1.  **Selection kind "unread":** model.SelectionUnread exists in
    go-core/internal/model/browse.go and the Swift ArticleSelection type
    has an `.unread` constant, but DEVELOPER_MAP.md states there is no
    separate Unread destination in the native sidebar. The core treats
    SelectionUnread similarly to all with unreadOnly true.
2.  **currentStarred field:** CoreRequest includes currentStarred, but the
    Go dispatcher ignores it. The Swift client does not send it; star
    mutations rely on local state and a remote re-read.
3.  **Snapshot schema version:** the documented current schema version is 1,
    and the Go core hardcodes Version: 1 everywhere. The Swift client does
    not enforce it yet.
4.  **Automatic read delayed flush:** SYNC_AND_DATA.md describes an
    eight-second Undo visibility window and a short delayed flush. The Go
    code uses a 10-second flush delay for automatic reads in
    MarkRead(service.go), while the Undo UI timer in BrowserStore.swift is
    eight seconds.
5.  **Error language:** many low-level Go store errors are prefixed in
    German (e.g. "SQLite-Konto anlegen"). Rust should preserve the same
    visible strings during compatibility testing, even though the language
    is not a product requirement.

## Formal compatibility decisions

These decisions fix the contract for the duration of the compatibility
migration. Deliberate product changes require a separate explicit decision.

### 8-second Undo window and 10-second automatic flush

The current timers are intentional and must not be changed during the
migration. The intended relationship is:

``` text
automatic read
     │
     ├──── Undo available (~8 s)
     │
     └──────── remote automatic flush (~10 s)
```

The ~2-second buffer ensures the Undo affordance normally disappears
before the automatic remote flush is attempted. Rust must reproduce the
same effective ordering (Undo window shorter than the delayed flush).

### currentStarred

The `currentStarred` field in the request envelope is currently unused
compatibility surface. The Swift client does not populate it and the Go
core does not read it. Star mutations rely on local state and a remote
re-read before toggling.

Do not remove `currentStarred` during migration. It may be removed or
activated later as a separate cleanup decision.

### German error strings

Low-level Go/store error strings are part of the externally observable
reference behavior for compatibility testing. Rust must reproduce the
same visible error semantics closely enough for differential testing.

Translating, normalizing, or redesigning these errors is a separate
architectural change that may only happen after Rust parity exists.

### SelectionUnread

`SelectionUnread` is a valid internal selection kind. It must remain
supported by the core even though the native sidebar does not expose a
separate Unread destination. The distinction is:

-   Internal/core selection capability: all, unread, starred, category,
    feed.
-   Visible sidebar destinations: All, Starred, Categories/Feeds.

Do not remove `SelectionUnread` merely because there is no dedicated
sidebar item.

### Snapshot version

Snapshot version `1` is the current compatibility contract. The Go core
hardcodes `Version: 1` in every browse snapshot and Swift currently does
not strictly enforce the version field. Rust must produce version `1`
and should tolerate the same lack of strict client enforcement until a
separate version-policy decision is made.

### Invalid/non-UTF-8 FFI input

The Go implementation uses `C.GoString`, which expects a null-terminated
byte sequence and silently interprets non-UTF-8 bytes according to Go's
UTF-8 handling. Exact byte-for-byte equivalence for malformed foreign
input is neither possible to guarantee nor meaningful to the product.

The Rust compatibility candidate defines deterministic safe behavior:

-   `null` request -> `{"ok":false,"error":"null request"}`.
-   Non-UTF-8 request -> `{"ok":false,"error":"invalid request: ..."}`.
-   Malformed JSON -> `{"ok":false,"error":"invalid request: ..."}`.

This matches the observable error shape of the Go core for the cases
that matter to the Swift caller. Rust must not read past a NUL byte,
must not retain the input pointer, and must not panic.

## Resolved compatibility questions

-   Invalid non-UTF-8 input returns a deterministic invalid-request response in
    Rust; exact parser text is an accepted implementation-specific difference.
-   Configure for an existing account ID updates only the server column via
    `ON CONFLICT DO UPDATE`, preserving counters and timestamps.
-   Missing or empty `selection.kind` falls back to all/unreadOnly true through
    normal selection normalization.

## Contract change rule

During the compatibility migration:

1.  discover behavior;
2.  document it;
3.  test it;
4.  reproduce it in Rust.

Any deliberate product/contract change is a separate task requiring an
explicit decision.

## Phase 10 parity findings

Phase 10 re-audited the Go implementation rather than treating earlier phase
documents as proof. Differential coverage now verifies transport null/default
semantics, icon base64 wire encoding and decoded processing output,
bidirectional mutation and Undo continuation, colliding numeric feed/entry IDs
across accounts, 26 sync/failure sequences, snapshot boundaries at 0, 1, 199,
200, 201, and 205 rows, malformed HTML/template traversal, ordered locale
fallback, and negative and 64-bit plural counts. Rust unit tests and Go source
characterization cover icon retry behavior. Universal core builds were also
validated separately from the differential suites.

Compatibility defects fixed in Rust during this audit include:

- explicit JSON `null` fields now use Go zero values;
- icon bytes serialize as base64 strings and failed icons are not cached;
- icon single-flight cleanup wakes waiters even during panic unwinding;
- terminal Miniflux `/v1` endpoints and redirect limits match the Go client;
- malformed configure authorities rejected by Go are rejected by Rust;
- `<template>` image traversal and negative plural forms match Go;
- localization counts accept the 64-bit range used by Go on supported macOS.

The audit result is **NOT READY** for a development-default transition. The
remaining material difference is orchestration: Go serializes refresh/flush but
allows local snapshots and mutations during remote work, while Rust currently
holds one service mutex across remote, SQLite, and icon operations. Go contexts
bound subsequent HTTP/SQLite calls but do not cancel a wait on Go's `syncMu`;
Rust similarly lacks cancellation while waiting for its broader service mutex.
Until Rust's broader blocking scope is remediated and tested, the 5-second local
operation behavior and local-first responsiveness are not proven equivalent.

Accepted bounded differences remain parser-specific malformed-JSON detail,
Rust's safer FFI panic containment, and simplified locale matching for the
actual English/German Apple locale lists. Arbitrary Accept-Language syntax and
every malformed URL form are not claimed equivalent.

## Phase 10.1 concurrency findings

The Phase 10 orchestration blocker was remediated on 2026-08-23. The observable
concurrency contract is:

- refresh and pending flush are serialized for one retained account service;
- local snapshots and optimistic read/star/Undo/discard mutations do not wait
  for remote refresh, remote flush, or icon network work;
- one runtime-wide SQLite connection is shared by retained account services and
  used by one operation at a time; its ownership wait is deadline-aware and
  deadline checks surround subsequent synchronous database calls;
- a pending mutation is acknowledged only at its delivered revision, so a
  concurrent superseding local value survives an older remote completion;
- icon cache and same-feed single-flight coordination are separate from sync
  and store ownership; each waiter has its own deadline;
- reconfiguration publishes a new service, while work already holding the old
  service remains bound to its original account, database, and remote client;
- same-account reconfiguration retains its existing service/sync gate and
  updates only generation and sort preference, preventing duplicate delivery;
- returning to an account reuses any service still retained by in-flight or
  delayed work, so an A-to-B-to-A sequence cannot create concurrent A gates;
- delayed flush scheduling is resettable and account-bound; an immediate
  manual mutation advances earlier automatic work without duplicate delivery.

Rust's refresh/flush serial-gate wait is deadline-aware. Go's `syncMu` wait is
not context-cancellable, so this is an accepted bounded internal safety
difference: timed-out Rust work returns an operation error and does not execute
later, while successful call ordering and all wire/persistence semantics remain
compatible.

`rusqlite` statements/transactions and icon decoding/rasterization are
synchronous library calls and cannot be interrupted in the middle. Rust checks
the absolute deadline immediately after them and does not cache an icon whose
processing completed late. Local mutation scheduling occurs immediately after
transaction commit, before snapshot/deadline checks, so a caller-visible
timeout cannot leave committed pending work unscheduled. If the operating
system refuses worker-thread creation, the deadline remains queued and a later
scheduling event retries worker creation; this resource-exhaustion case cannot
guarantee immediate delivery.

## Phase 10.2 development-default conclusion

The 2026-08-23 readiness re-check found no high- or medium-severity concurrency
or contract regression in the Phase 10.1 implementation. The state-scoped
synchronization, account-bound retained services, shared SQLite ownership,
deadline behavior, C/JSON ABI, and Go/Rust persistence contract are sufficient
for Rust to become the normal FluxBar development core in Phase 11. Go remains
the behavioral reference/fallback until a later explicit phase changes that
decision.

FluxBar has no public Go-backed installed base. A clean Rust-backed first
release does not require a Go-to-Rust end-user data migration path. Existing
bidirectional SQLite tests remain required compatibility and regression-oracle
coverage; this FluxBar-only conclusion does not imply a policy for FluxNews.
