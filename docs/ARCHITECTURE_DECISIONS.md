# FluxBar Desktop Architecture Decisions and Invariants

These are durable decisions. Do not violate them accidentally during
local feature work. They describe required product and architecture
invariants, not an implementation-status list; feature documents
identify which parts are currently available.

## Product Boundary

### Desktop is a news inbox, not a full RSS reader

FluxBar Desktop primarily supports discovery, scanning, triage, state
management, and quick access.

Do not grow the desktop application into a traditional
three-column/full-reader experience without an explicit product
decision.

### Web articles open in the browser

Normal web articles should open in the user's configured/default
browser.

Do not add an embedded browser or full article-reading surface as a
convenience implementation detail.

The browser preserves publisher login/paywall sessions, cookies,
extensions, password managers, accessibility configuration, comments,
interactive content, and the publisher's intended
monetization/presentation.

### Podcasts are an intentional exception

Playable podcast media may be consumed inside FluxBar. Audio playback is
not required to follow the browser-first article rule.

## Platform Boundaries

### Shared behavior belongs in the portable core

Platform-independent Miniflux, sync, state, filtering, and reusable
media/business behavior belongs in the portable core where practical.
Rust is the default implementation and future shared core. The existing Go
core is deprecated for future development but remains a behavioral reference
and explicit temporary fallback.

Do not duplicate business rules in SwiftUI merely because the UI needs
them. The language migration must preserve this boundary rather than
move business logic into native clients.

### Native UI is preferred over shared cross-platform UI

macOS should use native SwiftUI/AppKit and follow macOS conventions.

Future Windows/Linux support should reuse the product semantics and
portable core, not force a pixel-identical UI or cross-platform toolkit
onto macOS.

### Localization ownership follows the product boundary

The FluxBar compatibility surface keeps shared application strings and current
JSON catalog semantics behind the portable core; Rust and Go remain compatible
while Go is retained. Localization must not depend on Fyne or another UI
framework.

Future native FluxNews UI, widgets, automotive surfaces, and platform metadata
use native Swift/Kotlin catalogs. Rust owns stable typed/domain error identity
and non-presentational fallback text where required, not the complete native UI
catalog. This avoids coupling independent native products to FluxBar's legacy
`go-i18n` resource format.

New FluxBar compatibility/core-owned strings must use stable, descriptive keys
and provide an English caller fallback. Add each key to the English and
supported locale catalogs in the same change. Existing `fmt` placeholders
remain valid for parameterized messages; plural messages use `go-i18n` plural
forms and named template data. Native FluxNews UI strings follow their platform
catalogs instead.

### macOS is Menu Bar first

The primary macOS experience is a Menu Bar item opening a native
popover.

A conventional full Dock window is not required for normal use.

### macOS uses a narrow native core bridge

The native macOS target links the Rust core by default, with Go retained as an
explicit fallback, and exchanges serialized request/response payloads through a
small C ABI.
Keep platform-independent Miniflux and browse semantics behind this
boundary; do not grow a parallel Swift networking implementation.

Browse snapshots are explicitly versioned so the native client and portable
core can evolve their schema deliberately.

## Navigation and Layout

### Sidebar is hidden by default

Feed/category navigation is available on demand rather than permanently
consuming space.

### Navigation expands the popover

Opening the sidebar should expand the popover horizontally instead of
significantly shrinking the article content column.

The content column should remain approximately stable in width so
article rows do not reflow dramatically.

### Navigation state is independent from the view

Feed/category/filter selection must not be encoded only as sidebar UI
state. The same selection must be reusable by Spotlight/App Intents,
notifications, shortcuts, and future deep links.

Native entry points must converge on one route-to-selection translation.
Process-level integration may queue a route during cold launch, but must
not create an independent navigation state.

### Unread is a per-destination filter

All News, categories, and feeds default to unread-only and independently
remember whether they show unread or all matching articles. The native
sidebar does not require a separate Unread destination. Starred is a
distinct destination and includes read and unread starred articles.

## Article Presentation

### Rows optimize scanning, not reading

The macOS default is a compact desktop row with thumbnail left and
content right.

Avoid heavy mobile-style cards or large vertical hero images as the
default.

### Secondary actions use progressive disclosure

Read/star may appear as quick hover controls. Less frequent actions
belong in an overflow/context menu.

No important action may depend solely on swipe gestures.

## Local Data and Sync

### Local data renders first

Opening the popover must not require a successful Miniflux request.

Render current SQLite state immediately, then synchronize in the
background.

### Persistent core state is independent from presentation state

Synchronization may update the persistent core state — including entries,
counters, remote baselines, and reconciliation metadata — without replacing
the snapshot currently presented by a native client.

An active article list is presentation state owned by the client. It changes
only when the client explicitly requests and adopts a new snapshot.

This separation allows background synchronization to prepare current data
without disturbing a timeline the user is actively reading.

### Sync is not UI refresh

Remote synchronization and presentation refresh are separate operations.

A background sync may fetch and persist new remote data while the visible
timeline and displayed counters remain unchanged. A later user-initiated
refresh may adopt the already-current local state without requiring another
network request.

The client owns refresh policy. Depending on freshness and product context,
it may:

- adopt the latest local snapshot immediately;
- trigger remote synchronization and then adopt a snapshot; or
- keep the current presentation unchanged.

Do not couple successful synchronization, mutations, or persistence changes
to implicit replacement of an active presentation snapshot.

### Miniflux remains the remote source of truth

Local state exists for responsiveness, resilience, and synchronization.
It does not replace Miniflux as the authoritative remote service.

### Image cache is disposable

Images are cached separately from durable article/state data. Clearing
image cache must not destroy application state.

### Offline inbox does not imply offline full-reader

Locally stored metadata and cached images may keep the inbox useful
without connectivity. FluxBar does not need to persist/render complete
articles solely to become an offline reader.

## Automatic Read State

### Scrollover requires meaningful visibility

An article must not be marked read merely because it left the viewport.

It must first be meaningfully visible for a minimum amount of time/area
and then be intentionally scrolled past upward.

### Jumps do not imply reading

Scrollbar jumps, programmatic scrolling, feed/category changes,
deep-link navigation, restored scroll positions, and skipped rows must
not mass-mark unseen articles as read.

### Automatic read state should be undoable

Desktop scroll behavior is imprecise enough that a lightweight undo
affordance is desirable after automatic read mutations.

## Notifications

### Sync itself is never notification-worthy

A successful background sync is an implementation detail.

Only user-relevant new content from explicitly enabled feeds/categories
may generate notifications.

### Notifications are opt-in and selective

Default notification behavior should be off. Users opt into feeds that
matter enough to interrupt them.

Avoid notification storms; batch/group where appropriate.

## Podcasts

### Position synchronization is preserved

Desktop playback must preserve the existing synchronized
playback-position concept.

### Now Playing complements the app player

macOS Now Playing/system media controls are additional control surfaces.
They do not replace direct FluxBar player controls.

### Stop and Eject are distinct

Stop halts playback while the episode may remain loaded.

Eject removes the active episode/player state and allows the mini-player
to disappear.

### Chapters and speed are first-class controls

Chapter navigation and playback speed are important desktop podcast
functions, not obscure advanced settings.

## Core Language Migration Invariant

The Go-to-Rust migration is intentionally staged.

The existing C/JSON boundary and SQLite representation are compatibility
boundaries for the first migration. Rust should first become an
interchangeable implementation behind the same native contract.

``` text
macOS native client
        │
    C / JSON contract
      ┌─┴─┐
      │   │
     Go  Rust
```

The Go implementation remains the behavioral reference/fallback while it is
retained. The developer build supports both cores through `FLUX_CORE=rust`
(default) and `FLUX_CORE=go`, while Xcode consumes the same
`libfluxcore.a` / `libfluxcore.h` artifact contract regardless of
implementation. Go is deprecated for future feature development. The completed
compatibility implementation through the development-default phase must not be
retroactively combined with unrelated feature, database, UI, or product
redesign.

Rust persistence must use the existing unversioned SQLite schema rather than
introducing a Rust-specific database or migration version. The portable store
receives an explicit database path; platform path discovery remains outside
the persistence layer. SQLite row representations may contain remote baseline
and compatibility state that must not leak into pure domain models.

Sync compatibility requires separate persisted remote baselines and effective
local desired state. A pending mutation protects the effective value from a
stale remote refresh. Negative reconciliation is permitted only after the
remote adapter has produced a complete, stable selection. Pending rows and
Undo metadata remain account-scoped and durable across process restart.

Rust sync/flush uses blocking, account-bound orchestration without an async
runtime. Refresh and pending flush serialize on a per-service gate;
same-account reconfiguration and account round trips must reuse that gate while
its service remains alive. Retained account services share one runtime-wide
SQLite connection, while icons use independent
cache/single-flight synchronization. Never hold SQLite ownership or icon state
across Miniflux network I/O. Public operation deadlines include waits for the
refresh/flush gate or SQLite ownership. Synchronous SQLite/image calls cannot
be cancelled mid-call and require immediate post-call deadline checks;
committed pending work must be scheduled before such a check can return an
error. The Miniflux HTTP library timeout and delayed automatic-read timer remain
separate limits.
Delayed work may retain its original account service after reconfiguration,
but can never use the replacement account's store or remote client. Pending
revision acknowledgement must preserve a newer concurrent local value.

Core mutation, query, synchronization, and presentation responsibilities must
remain separable. Commands must not implicitly replace a client's active
presentation snapshot, and synchronization may advance persistent state without
forcing presentation state to advance with it. See
`CORE_COMMAND_QUERY_SEPARATION.md`.

After mobile runtime and API behavior are characterized, C/JSON, a typed C ABI,
and UniFFI may be evaluated as mobile adapters. That decision is independent
from the stable FluxBar C/JSON API and must not leak FFI concerns into the Rust
domain model. See `SHARED_RUST_CORE_ROADMAP.md`.
