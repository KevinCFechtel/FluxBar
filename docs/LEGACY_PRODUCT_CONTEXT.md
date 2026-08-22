# FluxBar Desktop --- Product Context

## Purpose

This document captures the current product direction, UX decisions,
architectural principles, and open considerations for a desktop
companion to FluxBar.

It is intended as implementation context for Codex and other
contributors. It describes **why** the desktop application exists and
which product constraints should guide technical decisions.

## Product Vision

FluxBar Desktop should **not** be a traditional full-size RSS reader
ported from mobile.

The desktop experience should instead act as a lightweight,
always-accessible **news inbox**:

1.  Surface new articles from Miniflux.
2.  Let the user quickly scan and triage them.
3.  Provide enough preview information to decide whether an article is
    interesting.
4.  Allow lightweight state management such as read/unread and starred.
5.  Open the original article in the user's browser for actual reading.

The application should feel like a native desktop utility rather than a
mobile application enlarged for desktop.

### Core workflow

``` text
Miniflux
   ↓
FluxBar Desktop
   ↓
Scan / Preview / Triage
   ↓
Original article in browser
```

FluxBar helps the user answer:

> "Which of my new articles are worth opening?"

It does not need to replace the browser as the primary environment for
consuming web content.

## Relationship to FluxBar Mobile

FluxBar on iOS and Android can remain a complete Miniflux feed reader.

The desktop application has a different role:

``` text
                    Miniflux
                   /        \
                  /          \
        FluxBar Mobile    FluxBar Desktop
             │                  │
       Full feed reader     News inbox /
                            quick triage
                                  │
                                  ↓
                               Browser
```

Both clients share Miniflux state, so actions performed on one device
should naturally be reflected on the others.

Examples:

-   Reading an article on the Mac marks it read for mobile clients.
-   Starring an article on the Mac makes it available later on mobile.
-   Read/unread state remains synchronized through Miniflux.

## Why Articles Open in the Browser

Opening the original article in the user's browser is an intentional
product decision.

The browser already provides:

-   publisher login sessions
-   paywall authentication
-   cookies
-   password managers
-   browser extensions
-   content blockers
-   accessibility configuration
-   site-specific functionality
-   comments and interactive content
-   the publisher's intended presentation

An embedded browser or full article renderer would duplicate
functionality while potentially providing an inferior experience.

There is also a product philosophy behind this choice: publishers have a
legitimate interest in users visiting the original site where articles
can be monetized through subscriptions, advertising, or other
mechanisms.

FluxBar should help users **discover and select content**, not
unnecessarily replace the publisher's website.

This principle should guide future feature decisions, but should not be
presented as a moral requirement to users.

## Article Preview

A preview exists to help the user decide whether an article is
interesting.

It is **not intended to become a complete article reader**.

Useful preview content may include:

-   title
-   feed/source
-   publication time
-   thumbnail or lead image
-   short excerpt
-   potentially the first paragraph(s)

Avoid gradually turning the preview into a full reader with:

-   complete long-form article rendering
-   complex HTML/CSS compatibility
-   embedded browsing
-   reader-mode replacement
-   extensive typography controls

If a feature primarily improves long-form reading rather than discovery
or triage, reconsider whether it belongs in FluxBar Desktop.

## macOS Experience

### Menu Bar First

The primary macOS interface should be a **Menu Bar application**.

The application should not require a conventional Dock window for normal
usage.

Primary interaction:

``` text
Menu Bar icon
     ↓
Native popover
     ↓
News inbox
```

A native SwiftUI/AppKit implementation is preferred so the application
follows current and future macOS behavior and appearance, including
changes to system materials and Liquid Glass.

Do not compromise native macOS UX merely to reuse UI code on other
platforms.

### Popover, Not NSMenu

Use a real popover/custom view rather than treating the article list as
a conventional `NSMenu`.

The popover should support a richer interface with:

-   fixed header
-   scrollable article area
-   fixed navigation/actions where appropriate
-   images
-   multiple lines of article metadata
-   hover states
-   context menus
-   keyboard interaction
-   filtering
-   potentially gestures

Conceptually:

``` text
╭────────────────────────────────────╮
│ FluxBar                      ↻  ⚙  │
│ 23 unread                          │
├────────────────────────────────────┤
│ ┌──────┐  Article title...         │
│ │ IMG  │  Source · 12 min ago     │
│ └──────┘  Short preview...         │
│                                    │
│ ┌──────┐  Another article...       │
│ │ IMG  │  Source · 35 min ago     │
│ └──────┘  Short preview...         │
│                                    │
│             ↕ scroll               │
├────────────────────────────────────┤
│ All   Unread   Starred   Feeds     │
╰────────────────────────────────────╯
```

The popover may be somewhat larger than a conventional menu. It should
provide enough room for comfortable scanning without turning into a
disguised full-size application window.

Exact dimensions are a design decision and should remain adaptable.

## Article Interaction

### Primary action

Clicking an article should open the **original article URL in the user's
browser**.

### Secondary actions

Common article actions may include:

-   mark read/unread
-   star/unstar
-   open in browser
-   copy link
-   open/filter by feed
-   additional Miniflux-supported actions where useful

### Hover Actions

Small action buttons may appear when hovering an article.

Example:

``` text
┌─────────────────────────────────────────┐
│ ┌──────┐ Article title                 │
│ │ IMG  │ Source · 20 min ago           │
│ └──────┘                    ✓   ★   ⋯  │
└─────────────────────────────────────────┘
```

Avoid permanently displaying too many controls on every row.

The normal visual hierarchy should prioritize the content. Management
controls can become visible when the user interacts with an item.

### Context Menus

Context menus should be a primary desktop interaction mechanism.

Right-clicking an article should expose the relevant actions.

The `…` action button may expose the same or a closely related menu.

### Swipe Gestures

Swipe gestures are optional convenience shortcuts, particularly useful
for Mac trackpad users.

They must **never be the only way to access an action**.

Possible mappings:

``` text
Swipe left  → Mark read/unread
Swipe right → Star/unstar
```

Exact gesture mappings remain a UX decision.

Mouse users must have equivalent controls through hover buttons and/or
context menus.

## Keyboard Interaction

Keyboard navigation should be treated as an important desktop
capability.

Potential workflow:

``` text
Global shortcut
      ↓
Open FluxBar
      ↓
Arrow keys navigate articles
      ↓
Enter
      ↓
Open selected article in browser
```

Other useful keyboard actions may include:

-   mark read/unread
-   star/unstar
-   change filter
-   close popover
-   refresh

Do not require keyboard usage, but make common workflows efficient for
power users.

## Global Shortcut

A configurable global keyboard shortcut is desirable.

It should allow the user to quickly show or hide FluxBar even if the
Menu Bar item is hidden because of:

-   limited Menu Bar space
-   Menu Bar management utilities
-   user preference

The UI should have a sensible fallback position if the Menu Bar item
cannot be used as the popover anchor.

## Spotlight and App Intents

Deep macOS integration is part of the product direction.

Feeds and potentially categories should be accessible through
Spotlight/App Intents.

Example:

``` text
⌘ Space
"Heise"
Enter
    ↓
FluxBar opens directly filtered to the Heise feed
```

Potential Spotlight/App Intent concepts:

``` text
FluxBar
  → Open normal inbox

Heise Online
  → Open FluxBar filtered to Heise

Development
  → Open category

Refresh FluxBar
  → Trigger synchronization

Show Starred
  → Open starred articles
```

Feeds can potentially be represented as App Entities.

Avoid indexing every transient news article into Spotlight by default.
Doing so may create excessive noise in the user's system-wide search.

Feeds and categories are more stable and useful system-level entities.

## Navigation and Filtering

Likely useful top-level views include:

-   All
-   Unread
-   Starred
-   Feeds
-   Categories

The exact navigation presentation is not fixed.

The application should support opening directly into a specific
feed/category, including from external system integrations such as
Spotlight or Shortcuts.

## Notifications

Notifications may be useful for new articles, but FluxBar should avoid
becoming noisy.

Notification behavior should remain configurable and may eventually
support per-feed or per-category preferences.

This is not required for the initial product concept.

## Architecture

### Shared Go Core

The existing Go core should remain the platform-independent foundation.

Responsibilities suitable for the Go layer include:

-   Miniflux API communication
-   authentication/session-related API logic
-   synchronization
-   article/feed/category models
-   read/unread state operations
-   starred state operations
-   filtering/business rules
-   shared configuration where appropriate

Platform-specific UI concerns should not leak unnecessarily into the Go
core.

Conceptually:

``` text
                       Go Core
                          │
              Miniflux / Sync / State
                          │
          ┌───────────────┼───────────────┐
          │               │               │
       macOS           Windows          Linux
          │               │               │
   SwiftUI/AppKit     Native UI      Native UI
          │               │               │
     Menu Bar         System Tray     Status Item
     Popover            Popup           Popup
```

The boundary between Go and the native UI should be kept clear and
deliberately designed.

## Cross-Platform Strategy

FluxBar Desktop should be **conceptually cross-platform but visually
platform-native**.

Do not aim for pixel-identical interfaces across operating systems.

The shared concept is:

> A lightweight, quickly accessible news inbox that uses Miniflux for
> synchronization and hands actual reading to the browser.

Each operating system should implement that concept according to its
native desktop conventions.

### macOS

Preferred approach:

-   SwiftUI
-   AppKit where required
-   Menu Bar item
-   native popover
-   Spotlight
-   App Intents / Shortcuts
-   trackpad gestures as optional enhancements

### Windows

Potential approach:

-   System Tray / Notification Area
-   native popup/borderless window
-   native Windows UI technology such as WinUI where appropriate

The interaction model can remain similar:

``` text
Tray icon
   ↓
News popup
   ↓
Scan / triage
   ↓
Browser
```

Mouse/hover/context-menu interactions should take precedence over
gesture-specific behavior.

### Linux

Linux can follow the same product concept using available desktop
integration such as StatusNotifierItem/AppIndicator and an appropriate
native popup/window.

However, desktop environments differ substantially.

Do not assume:

-   a tray is always available
-   identical positioning behavior across GNOME/KDE/etc.
-   identical interaction conventions

Linux may require additional fallback entry points or adaptations.

## Platform-Native UI Over Shared UI

A cross-platform UI toolkit may reduce implementation effort, but native
desktop integration is particularly important for this product because
the application lives in highly platform-specific system UI.

Therefore:

-   share business logic
-   share models and product semantics
-   share behavior where sensible
-   **do not require sharing the UI implementation**

A native macOS experience is more important than minimizing all
platform-specific UI code.

The same principle should apply to Windows and Linux if those versions
are implemented.

## Product Principles

### 1. Fast access

FluxBar should be reachable with minimal interruption to the user's
current work.

### 2. Triage before reading

The application helps decide what deserves attention.

### 3. Browser for web content

The original browser is the default environment for consuming full
articles.

### 4. Native desktop behavior

Use the conventions and capabilities of each operating system.

### 5. Miniflux is the source of truth

Avoid duplicating feed synchronization infrastructure that Miniflux
already provides.

### 6. Progressive disclosure

Keep the article list visually focused. Show secondary controls on
hover, through context menus, shortcuts, or gestures.

### 7. Multiple input methods

Mouse, trackpad, and keyboard users should all have efficient workflows.

### 8. No gesture-only functionality

Gestures are accelerators, not requirements.

### 9. Keep the desktop client lightweight

Do not gradually turn FluxBar Desktop into a conventional full-screen
RSS reader unless the product direction is intentionally changed.

## Distribution Direction

For macOS, the intended distribution options may include:

-   Mac App Store
-   signed/notarized GitHub releases
-   Homebrew Cask

The Mac App Store and direct/Homebrew distributions may require
separately signed builds.

The application should be designed with App Sandbox compatibility in
mind if Mac App Store distribution is pursued.

A Homebrew Cask can initially live in a project-owned tap and
potentially move to the official Homebrew Cask repository once
eligibility/notability requirements are met.

Distribution details are implementation/release concerns and should not
dictate the core UX.

## Non-Goals

Unless the product direction explicitly changes, do **not**:

-   turn FluxBar Desktop into a traditional three-column RSS reader
-   build a complete embedded web browser
-   make the popover a full-size window disguised as a popover
-   require swipe gestures for important functionality
-   compromise native macOS UX to achieve identical cross-platform UI
-   move platform-independent Miniflux/business logic into
    SwiftUI/AppKit
-   index every news article into Spotlight without a clear user benefit
-   duplicate Miniflux's role as the synchronization source of truth
-   assume Linux desktop integration behaves identically across
    environments

## Decisions vs. Open Questions

### Current Product Decisions

These should be treated as the default direction unless explicitly
reconsidered:

-   Desktop is a lightweight news inbox rather than a full mobile-style
    reader.
-   Miniflux remains the synchronization/source-of-truth backend.
-   Full articles open in the user's browser.
-   macOS uses a Menu Bar-first experience.
-   macOS should use a rich native popover rather than a conventional
    `NSMenu`.
-   The list may contain thumbnails and richer metadata.
-   Context menus and visible/hover actions are primary interaction
    mechanisms.
-   Swipe gestures are optional accelerators.
-   The Go core remains shared and platform-independent.
-   Platform UIs should be native rather than forced into a single
    shared UI.
-   Spotlight/App Intents integration is desirable, particularly for
    feeds/categories.

### Open Design Questions

These should not be treated as fixed requirements:

-   exact popover dimensions
-   exact article row layout
-   amount/length of preview text
-   whether a separate hover preview is still useful when rows become
    richer
-   exact swipe gesture mappings
-   exact global keyboard shortcut
-   which actions receive dedicated hover buttons
-   final navigation structure
-   notification behavior
-   whether categories should be indexed in Spotlight alongside feeds
-   Windows UI framework
-   Linux UI framework and fallback behavior

## Guidance for Codex

When implementing features, preserve the intent of this document.

Before adding significant UI or architectural behavior, ask:

1.  Does this help users discover, triage, or manage news quickly?
2.  Is this functionality better handled by the user's browser?
3.  Is the implementation native to the target platform?
4.  Does platform-independent logic belong in the Go core?
5.  Is an interaction discoverable without relying on gestures?
6.  Does this keep FluxBar lightweight rather than turning it into a
    conventional desktop reader?

When an implementation choice conflicts with these principles, prefer
the product principles over incidental existing code unless instructed
otherwise.

# UI and Behavior Specification Addendum

This section captures the more detailed UI and behavior decisions made
after the initial product context was created. These details are
concrete enough to guide a first macOS UI prototype. Unless explicitly
marked as an open question, they should be treated as the preferred
direction for the first design/implementation pass.

## Feed Navigation

### Collapsible Sidebar

The preferred macOS navigation model is a **collapsed-by-default
sidebar** that can be revealed on demand.

The normal popover remains focused on the article inbox:

``` text
╭──────────────────────────────────────────╮
│ ☰  FluxBar                        ↻  ⚙  │
├──────────────────────────────────────────┤
│                                          │
│              Article list                │
│                                          │
├──────────────────────────────────────────┤
│ All          Unread          Starred     │
╰──────────────────────────────────────────╯
```

When the user opens navigation, the popover expands and reveals a
sidebar:

``` text
╭───────────────────┬──────────────────────────────────────────╮
│ Feeds             │ ☰  FluxBar                        ↻  ⚙ │
│                   │                                          │
│ ● All             │              Article list                │
│ ○ Unread          │                                          │
│ ★ Starred         │                                          │
│                   │                                          │
│ ▾ Technology      │                                          │
│    Heise          │                                          │
│    Ars Technica   │                                          │
│    The Verge      │                                          │
│                   │                                          │
│ ▸ Development     │                                          │
│ ▸ Gaming          │                                          │
╰───────────────────┴──────────────────────────────────────────╯
```

The existing Miniflux hierarchy maps naturally to this sidebar:

``` text
Category
└── Feed
```

Categories should be expandable/collapsible. Selecting a category
filters the article list to all feeds in that category. Selecting a feed
filters to that individual feed.

### Preserve Content Width

A key design rule is:

> **Navigation expands the popover; it should not significantly shrink
> the article content column.**

For example:

``` text
Sidebar closed:
[            ~450–500 px article content             ]

Sidebar open:
[ ~180–220 px sidebar ][ ~450–500 px article content ]
```

The popover may therefore grow horizontally when the sidebar is opened
and shrink again when it is closed.

This avoids responsive layout changes in article rows and preserves
readability.

The expansion should feel like revealing additional navigation rather
than transforming the entire layout.

### NavigationSplitView

SwiftUI `NavigationSplitView` is a natural conceptual fit for this
structure, with the sidebar hidden by default.

However, the exact implementation is not fixed. If `NavigationSplitView`
makes popover resizing or layout control awkward, a custom SwiftUI
sidebar/content composition is acceptable.

The desired UX is more important than using a specific container.

### Direct Navigation

The same selection model should be reusable from:

-   sidebar navigation
-   Spotlight
-   App Intents
-   Shortcuts
-   future deep links

Conceptually:

``` text
Selection
├── all
├── unread
├── starred
├── category(id)
└── feed(id)
```

Opening a feed through Spotlight should produce the same application
state as selecting that feed in the sidebar.

## macOS Article List Design

### Row View as the Default

The preferred macOS article presentation is based on the existing
FluxBar mobile **landscape Row View**, rather than the vertically
stacked mobile card.

The basic layout is:

``` text
╭──────────────────────────────────────────────────╮
│ ┌───────────┐  Feed icon · Feed · 18 min · 💬 12│
│ │           │                                    │
│ │ Thumbnail │  Article title                     │
│ │           │                                    │
│ │           │  Short teaser over a few lines...  │
│ └───────────┘                         ✓   ★   ⋯   │
╰──────────────────────────────────────────────────╯
```

This layout is preferred because the desktop popover has enough
horizontal space and because it allows more articles to remain visible
simultaneously.

The list is optimized for **scanning and triage**, not long-form
reading.

### Information Hierarchy

A row should generally contain:

1.  thumbnail/lead image
2.  feed icon and feed name
3.  publication date/relative time
4.  optional comment count
5.  article title
6.  short teaser
7.  contextual actions when appropriate

The exact typography and spacing remain design decisions.

### Images

Thumbnails should normally appear on the left with text on the right.

A rough starting point could be around 110--130 px thumbnail width in a
\~450--500 px content column, but these are prototype values rather than
hard requirements.

The existing large-image mobile card can remain relevant to mobile or
potentially a future alternative display mode, but should not be the
default macOS presentation.

### Cards and Row Styling

Do not make every article look like a heavy, permanently elevated mobile
card.

Prefer a native macOS row/card hybrid:

-   generous spacing
-   subtle separation
-   minimal permanent decoration
-   stronger background/material on hover
-   clear native selection state
-   system colors/materials
-   avoid unnecessary custom shadows and hard-coded card styling

The interface should follow current macOS system appearance, including
future system material/Liquid Glass changes where native components
provide them.

## Article Actions

### Primary Interaction

Clicking the main article row opens the original article in the user's
configured/default browser.

### Quick Actions

The most useful frequent actions are:

-   mark read/unread
-   star/unstar

These may appear as small icon buttons on hover.

Example:

``` text
Normal:
[IMG]  Feed · Time
       Article title
       Teaser

Hover:
[IMG]  Feed · Time
       Article title
       Teaser                       ✓  ★  ⋯
```

Avoid permanently showing a large action toolbar under every article.

### Context Menu

Right-clicking an article should expose a full context menu.

The `…` button may expose the same or a closely related menu.

Candidate actions include:

-   Open in Browser
-   Mark as Read / Mark as Unread
-   Star / Unstar
-   Share...
-   Open Comments
-   Copy Link
-   Open/Filter by Feed

### Comments

If Miniflux/article metadata provides a comment count, it may be shown
as metadata rather than as a permanently visible action button:

``` text
Heise · 18 min · 💬 12
```

Clicking the comment indicator can open the comments destination.

### Swipe Gestures

Swipe gestures are optional accelerators, not primary controls.

Potential actions:

``` text
Swipe left  → Mark read/unread
Swipe right → Star/unstar
```

No important action may be available exclusively through a swipe
gesture.

Context menus and visible/hover controls remain the primary cross-input
desktop interaction model.

## Local Persistence and Sync

### SQLite Remains Appropriate

FluxBar Desktop should maintain a local SQLite database for news and
synchronization state, similar to the mobile clients.

The purpose is not primarily full offline article reading. The major
benefits are:

-   immediate popover opening
-   responsive local filtering/navigation
-   resilience during temporary connectivity loss
-   local state changes without waiting for the server
-   efficient background synchronization

Preferred flow:

``` text
Open popover
    ↓
Read local SQLite state immediately
    ↓
Render current inbox
    ↓
Background Miniflux sync
    ↓
Update local database
    ↓
UI updates
```

The application should not need to wait for a Miniflux network request
before showing useful content.

### Offline State Changes

Actions such as read/unread and starred should be able to update local
state immediately.

If the server cannot be reached, changes can remain pending and
synchronize later.

Conceptually:

``` text
User action
    ↓
Update local SQLite
    ↓
Queue/persist pending synchronization state
    ↓
Synchronize with Miniflux when possible
```

The existing mobile synchronization behavior can be reused conceptually
where appropriate.

### Browser Still Requires Connectivity

The inbox can remain useful offline using locally stored metadata and
cached images.

Opening the actual publisher page in the browser naturally requires
network access unless the browser itself has cached it.

This does not require FluxBar Desktop to become a full offline article
reader.

## Image Loading and Cache

Images should continue to be lazy loaded and cached locally.

The image cache should be treated separately from persistent article
data.

Properties of the image cache:

-   disk backed
-   disposable
-   bounded in size
-   automatically cleaned
-   potentially LRU or age based
-   safe to clear without losing application state

Because the macOS row uses relatively small thumbnails, it may be
beneficial to store resized thumbnail variants rather than retaining
unnecessarily large source images.

The UI should display a suitable placeholder while an uncached image is
loading.

## Mark as Read on Scrollover

The existing mobile **Mark as Read on Scrollover** feature is valuable
and should be considered part of the desktop experience, but desktop
scrolling requires additional safeguards.

### Goal

The workflow should remain:

> As the user intentionally works through the inbox, articles they have
> passed can automatically become read.

### Do Not Mark Based Only on Position

Do not simply mark an article read because its frame moved outside the
visible scroll area.

Desktop trackpads and mouse wheels allow rapid momentum scrolling, and
users may accidentally scroll through many articles.

Instead, an article should first qualify as genuinely seen.

A starting heuristic for prototyping:

-   approximately 50--70% of the row was visible
-   it remained meaningfully visible for roughly 0.5--1 second
-   it then leaves the viewport upward through normal scrolling

Only then should it become a candidate for automatic read marking.

Exact thresholds should be tuned through UX testing.

### Visibility State

Conceptually:

``` text
unseen
  ↓
visible
  ↓
qualified-as-seen
  ↓
scrolled past upward
  ↓
mark read locally
```

### Cases That Should Not Mass-Mark Articles

Do not treat these as normal reading progression:

-   dragging the scrollbar to jump far down
-   programmatic scrolling
-   changing feeds/categories
-   opening a feed from Spotlight
-   restoring a saved scroll position
-   rows skipped without meaningful viewport exposure

### Sync Behavior

Automatic read changes do not need to be immediately synchronized with
Miniflux.

The existing delayed/batched sync approach remains appropriate.
Immediate sync can remain an optional behavior if already supported by
the product.

### Undo

A transient undo affordance is desirable after one or more automatic
read changes, for example:

``` text
5 articles marked as read · Undo
```

This reduces the cost of accidental desktop scrolling.

## Notifications

### Sync Is Not Notification-Worthy

Never notify merely because a background synchronization occurred.

A sync is an implementation detail. Notifications should represent
user-relevant new content.

### Opt-In Per Feed

Notifications should default to off and be explicitly enabled for feeds
the user considers important.

Conceptually:

``` text
Notifications
├── Off by default
└── Selected feeds
    ├── Heise          ✓
    ├── Flutter Blog   ✓
    └── Ars Technica   ✕
```

Category-level notification preferences may be considered later.

### Notification Grouping

If one synchronization finds multiple new entries for the same feed,
avoid creating excessive individual notifications.

Prefer sensible grouping/batching by feed where possible.

Example:

``` text
Heise Online
3 new articles

• Article A
• Article B
• Article C
```

A single article can use a normal single-article notification.

### Notification Navigation

Selecting a notification should deep-link into the relevant FluxBar
state:

-   open the specific article where appropriate, or
-   open the relevant feed filtered to the new content

### Interruption Level

Normal RSS/news notifications should remain normal/passive system
notifications.

Do not treat ordinary feed updates as critical or time-sensitive
notifications that aggressively bypass Focus settings.

## Podcast Support

### Podcasts Belong Inside FluxBar

The browser-first principle applies to web articles, but not necessarily
to playable media.

Podcast audio should be playable directly inside FluxBar Desktop.

Conceptually:

``` text
Web article → Original browser
Podcast     → FluxBar audio player
```

This is appropriate because the browser provides substantial value for
publisher web pages, while FluxBar can provide a better integrated
experience for a podcast enclosure/audio file.

### Cross-Device Playback Position

The existing mobile functionality already synchronizes podcast playback
position.

The desktop implementation should preserve this capability.

A key workflow is:

``` text
Listen on iPhone
Stop at 37:24
      ↓
Open FluxBar on Mac
      ↓
Resume at 37:24
```

This is an important desktop feature.

## Podcast Mini Player

### Own Controls Plus Now Playing

Do not rely exclusively on macOS Now Playing.

FluxBar should integrate with system Now Playing/remote media controls
**and** provide its own compact controls while the popover is open.

System Now Playing is useful when FluxBar is closed/hidden. The
FluxBar mini player is useful while the user is actively interacting
with the application.

### Required/Important Controls

The player should support:

-   Play / Pause
-   30 seconds backward
-   30 seconds forward
-   Stop
-   Eject/remove current episode from player
-   seek/progress control
-   playback speed
-   chapter selection

### Stop vs. Eject

Treat Stop and Eject as distinct concepts.

**Stop:** - stop playback - current episode may remain loaded in the
player

**Eject:** - stop playback as needed - remove the episode from the
active player - allow the mini-player area to disappear/reset

### Compact Player Concept

A compact state could look approximately like:

``` text
╭────────────────────────────────────────────╮
│ [Art] Podcast / Episode                    │
│       Current chapter                      │
│                                            │
│       ━━━━━━━━━●━━━━━━━━━━━━               │
│       42:18                    1:37:22      │
│                                            │
│       ↶30      ❚❚      ↷30       1.5×     │
╰────────────────────────────────────────────╯
```

The exact layout is open to prototyping.

The player should only consume persistent popover space when an episode
is loaded/active.

### Chapters

Chapter support is considered important, not merely an optional advanced
feature.

The current chapter may be displayed in the mini player and be
selectable.

A chapter selector can expose the complete list and allow direct
navigation.

### Playback Speed

Playback speed should be directly accessible from the player, for
example through a compact `1.5×` control/menu.

### Now Playing

FluxBar should publish appropriate metadata and playback state to macOS
Now Playing and respond to supported system media commands.

The system controls complement rather than replace the in-app mini
player.

## Sleep Timer

A sleep timer is considered a useful but lower-priority desktop feature.

It should not block the initial player implementation.

If implemented, useful options include:

``` text
15 minutes
30 minutes
45 minutes
60 minutes
────────────
End of chapter
End of episode
```

`End of chapter` is especially interesting because it can be useful even
outside a traditional bedtime scenario.

## First UI Prototype Scope

The current product discussion is sufficiently concrete to create an
initial macOS UI prototype.

A first prototype should focus on the following structure:

``` text
Menu Bar
   │
   ▼
FluxBar Popover
   │
   ├── Header
   │     ├── Sidebar toggle
   │     ├── current context/title
   │     ├── refresh
   │     └── settings
   │
   ├── Optional expandable Sidebar
   │     ├── All
   │     ├── Unread
   │     ├── Starred
   │     └── Categories
   │           └── Feeds
   │
   ├── Scrollable Article List
   │     └── Article Row
   │           ├── Thumbnail
   │           ├── Feed icon/name
   │           ├── date/time
   │           ├── optional comments
   │           ├── title
   │           ├── teaser
   │           └── hover/context actions
   │
   └── Conditional Podcast Mini Player
         ├── artwork/title/chapter
         ├── progress
         ├── ±30 seconds
         ├── play/pause
         ├── stop/eject
         ├── speed
         └── chapter selection
```

The main article list remains the dominant surface.

The sidebar is normally hidden.

The podcast player is normally absent unless media is loaded.

The popover therefore remains lightweight in the common case and
progressively reveals additional UI only when needed.

## Additional Design Principles

The following principles have emerged from the detailed UI discussion:

> **Navigation expands the popover; it does not shrink the content.**

> **Article rows are optimized for scanning and triage, not long-form
> reading.**

> **Local persistence exists primarily for responsiveness, resilience,
> and synchronization---not to turn FluxBar Desktop into an offline
> full-article reader.**

> **Web content belongs in the browser; playable media can remain inside
> FluxBar.**

> **Secondary controls should use progressive disclosure: hover, context
> menus, and compact controls rather than permanent visual clutter.**

> **Desktop scrolling is powerful and imprecise enough that automatic
> read-state behavior needs stronger safeguards than on mobile.**

> **Notifications represent explicitly important content, never
> synchronization activity itself.**

> **Now Playing complements the FluxBar podcast player; it does not
> replace it.**

## Remaining Open UI Questions

The following details are intentionally still open for prototype
evaluation:

-   exact closed/open popover dimensions
-   exact sidebar width
-   exact thumbnail dimensions/aspect ratio
-   precise typography and spacing
-   whether a separate hover preview is still needed once rows contain
    richer teaser content
-   exact location of All/Unread/Starred navigation
-   exact hover button set
-   exact context menu contents
-   exact mark-as-read visibility/time thresholds
-   undo presentation and duration
-   notification batching details
-   podcast mini-player height/layout
-   whether Stop is permanently visible or placed in an overflow menu
-   whether Eject receives a dedicated button
-   chapter selector presentation
-   Sleep Timer priority after V1
