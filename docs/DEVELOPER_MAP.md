# FluxBar Desktop Developer Map

## Purpose

FluxBar Desktop is a lightweight desktop companion for Miniflux.

It is intentionally not a direct desktop port of the mobile FluxBar
application and not a traditional full-screen RSS reader. Its primary
job is to surface new content, let users quickly scan and triage it,
synchronize state with Miniflux, and hand normal web articles to the
user's browser.

Podcast media is a planned intentional exception: podcast audio may be
played directly in FluxBar with synchronized playback position.

## Current Implementation Status

The current macOS implementation provides a native Menu Bar item and popover
backed by Miniflux through the Rust core. Rust is the normal development core
and persists inbox state and desired read/star mutations in SQLite. Go remains
a deprecated reference/fallback. Native views load the selected local snapshot
first. While the popover is visible, Miniflux inbox refreshes are explicit user
actions.

Automatic Mark as Read on Scrollover, transient Undo,
startup/stale-close sync, a hidden-popover 15-minute sync interval,
keyboard navigation, a global shortcut, Spotlight navigation, and App
Intents are implemented. Podcast playback and content notifications
remain target functionality.

## Product Model

``` text
                         Miniflux
                            │
                            ▼
                        Rust Core
             ┌──────────────┼──────────────┐
             │              │              │
           Models          Sync         Storage
             │              │            SQLite
             └──────────────┼──────────────┘
                            │
                    Native platform bridge
                            │
             ┌──────────────┼──────────────┐
             │              │              │
           macOS          Windows         Linux
      SwiftUI/AppKit       future         future
             │
        Menu Bar Item
             │
          Popover
       ┌─────┼───────────────┐
       │     │               │
   Sidebar  Article List   Podcast Player
                 │
                 ▼
              Browser
          (web articles)
```

## Core Responsibilities

### Miniflux

Miniflux is the remote source of truth for categories, feeds, remote
articles, read/unread state, starred state, and relevant feed metadata.

### Portable Core

The current Rust core owns platform-independent behavior where practical,
including Miniflux communication, synchronization/business rules, shared
models, filtering semantics, and state operations. Go remains the behavioral
reference for the completed FluxBar compatibility migration.

The native macOS layer should not duplicate business rules merely for UI
convenience.

Application localization is shared core behavior implemented in both the Go
and Rust cores. Translation catalogs live under
`go-core/internal/localization/translations` and are embedded by the Rust
core via `include_str!`, so both implementations share a single source of
truth. Native interfaces provide their ordered locale preferences through the
platform bridge and do not maintain a duplicate application catalog.

### SQLite

SQLite is the local operational store used to make the desktop UI
immediate and resilient. It stores article/navigation snapshots, remote
state, effective local state, pending desired mutations, and automatic
read Undo receipts per Miniflux account.

The popover should be able to render useful local state before a network
synchronization completes.

### Image Cache

Article images are lazily loaded and cached separately from durable
application data. The first usable image in article HTML is preferred;
the first valid Miniflux `image/*` enclosure is used as a fallback.
Relative image references are resolved against the article URL, and only
HTTP(S) URLs are accepted. Cached images are disposable.

### macOS UI

The macOS application is Menu Bar first. A native popover is the primary
surface.

Normal state: - compact article inbox - sidebar hidden - podcast player
absent unless media is loaded

Progressive state: - sidebar expands on demand - podcast mini player
appears when needed - secondary actions appear through hover/context
menus

The podcast mini player is planned rather than implemented.

### Native Core Bridge

The macOS application links an implementation-neutral C archive and defaults to
the Rust implementation. The native layer sends JSON requests across a small C
ABI and explicitly releases core-allocated response strings. Browse data is
returned as a versioned JSON snapshot; the current snapshot schema is version
1. Explicit Go builds use the same artifact and ABI contract.

The bridge currently supports configuration, refresh, read/star
mutations, feed icons, and localization. Snapshot-version compatibility
is represented in the payload but is not yet enforced by the Swift
client.

### Browser

The browser is the primary long-form environment for web articles
because it already owns publisher cookies, paywall/login sessions,
extensions, password managers, accessibility settings, and the original
site experience.

### Podcasts

Podcast enclosures are planned to play directly inside FluxBar. The
future player should synchronize position and integrate with macOS Now
Playing.

## Shared Core Direction

The FluxBar compatibility migration is complete through the Rust development-
default phase. The stable macOS architecture remains:

``` text
                     Native macOS client
                            |
                     stable C/JSON bridge
                            |
                    Rust core (default)
                            |
                 FluxBar compatibility store
```

The Xcode target links `libfluxcore.a` / `libfluxcore.h`. `FLUX_CORE=rust` (or
unset) selects Rust; `FLUX_CORE=go` selects the deprecated reference/fallback.
The Rust core implements all 11 FluxBar operations. Its state-scoped
concurrency permits local snapshots and optimistic mutations while remote or
icon work is blocked.

Future native FluxNews clients will use additional mobile services around the
same Rust foundation without inheriting FluxBar's 200-row snapshot or
unversioned compatibility schema. The durable constraints are:

-   preserve the existing bridge and JSON behavior;
-   preserve FluxBar SQLite compatibility;
-   keep platform UI/OS integration native;
-   keep Go available as deprecated reference/fallback for now;
-   validate mobile runtime and API requirements before selecting UniFFI or
    another mobile adapter; and
-   use a separate versioned mobile persistence profile for FluxNews.

See `RUST_CORE_MIGRATION.md` for the completed compatibility phases,
`FLUXNEWS_CORE_GAP_ANALYSIS.md` for the source inventory, and
`SHARED_RUST_CORE_ROADMAP.md` for the post-FluxBar execution plan.

## Data Flow: App Open / Popover Open

``` text
User opens popover
        │
        ▼
Read local SQLite state
        │
        ▼
Render inbox immediately
        │
        ├──────────────► Lazy-load cached/missing thumbnails
        │
        └──────────────► Explicit refresh syncs with Miniflux
```

## Data Flow: Local State Change

``` text
User marks read / stars item
        │
        ▼
Update local state immediately
        │
        ▼
Persist pending/current sync state
        │
        ▼
Synchronize with Miniflux
```

## Main UI Architecture

``` text
Menu Bar Item
    │
    ▼
FluxBar Popover
    ├── Header
    │    ├── Sidebar toggle
    │    ├── current context
    │    ├── refresh
    │    └── settings entry
    │
    ├── Optional Sidebar
    │    ├── All
    │    ├── Starred
    │    └── Categories
    │          └── Feeds
    │
    ├── Scrollable Article List
    │    └── Article Row
    │          ├── Thumbnail
    │          ├── Feed metadata
    │          ├── Title
    │          ├── Teaser
    │          └── contextual actions
    │
    └── Planned Conditional Podcast Mini Player
```

## Navigation State

Navigation should be modeled independently of the sidebar so the same
state can later be activated by Spotlight, App Intents, Shortcuts,
notifications, or deep links.

Conceptually:

``` text
Selection
├── all + unread/all filter
├── starred
├── category(id) + unread/all filter
└── feed(id) + unread/all filter
```

All, category, and feed default to unread-only and remember their filter
independently. Starred includes both read and unread starred articles.
There is no separate Unread destination in the native sidebar.

The native `NavigationRoute` represents All, Starred, category, feed,
and an optional browser-oriented article destination. Sidebar actions,
article feed actions, Spotlight, and App Intents all call
`BrowserStore.route(to:)`, which translates browse routes to the
existing `ArticleSelection` and local SQLite snapshot flow. `AppRouter`
only bridges process-level external entry points and cold-launch
delivery to that store; it does not own a second selection state. This
is also the route entry point intended for future notification
destinations.

``` text
Sidebar / Spotlight / App Intent / future notification
                         │
                         ▼
                 NavigationRoute
                         │
                         ▼
              BrowserStore.route(to:)
                         │
                         ▼
                ArticleSelection
                         │
                         ▼
               local SQLite snapshot
```

## Where to Read Next

-   macOS UI work → `features/MACOS_UI.md`
-   sync/database/cache/scrollover → `features/SYNC_AND_DATA.md`
-   podcast/audio → `features/PODCASTS.md`
-   notifications → `features/NOTIFICATIONS.md`
-   architecture-sensitive work → `ARCHITECTURE_DECISIONS.md`
-   shared Rust core/native FluxNews roadmap → `SHARED_RUST_CORE_ROADMAP.md`
