# AGENTS.md

## Goal

Work efficiently and conservatively in FluxBar Desktop.

Prefer small, focused changes that preserve the existing architecture
and the product direction documented under `docs/`. Do not introduce
unnecessary abstractions, dependencies, migrations, cross-platform UI
frameworks, or unrelated refactors.

The human developer remains responsible for architectural and product
decisions. Make important design choices explicit instead of silently
introducing them.

## Repository Context

Before non-trivial work, read only the documentation relevant to the
task:

-   `docs/DEVELOPER_MAP.md` --- high-level model of FluxBar Desktop and
    its intended data/UI flows.
-   `docs/ARCHITECTURE_DECISIONS.md` --- durable product/architecture
    invariants that must not be accidentally violated.
-   `docs/features/MACOS_UI.md` --- macOS Menu Bar popover, navigation,
    article rows, interactions, Spotlight/shortcut direction.
-   `docs/features/SYNC_AND_DATA.md` --- SQLite, local-first behavior,
    Miniflux sync, image caching, mark-as-read-on-scrollover.
-   `docs/features/PODCASTS.md` --- podcast playback, position sync,
    mini player, chapters, speed, Now Playing.
-   `docs/features/NOTIFICATIONS.md` --- selective feed notifications
    and batching behavior.
-   `docs/RELEASE_AND_DISTRIBUTION.md` --- intended macOS distribution
    channels and constraints.
-   `docs/PRODUCT_BACKLOG.md` --- ideas and unresolved future work;
    backlog items are not necessarily implemented.
-   `docs/RUST_CORE_MIGRATION.md` --- active Go-to-Rust core migration
    plan; read for Rust-core migration work.
-   `docs/CORE_COMPATIBILITY_CONTRACT.md` --- audited external Go-core
    contract that Rust must preserve during compatibility migration.
-   `docs/RUST_CORE_TESTING.md` --- compatibility, differential, ABI,
    and database-interoperability testing rules for the migration.

Do not assume documentation describing a target design is already
implemented. Verify the repository state.

## Understand Before Changing

Before modifying code:

-   Inspect the relevant implementation and direct dependencies.
-   Reuse existing Go core behavior and native platform patterns where
    appropriate.
-   Identify the smallest reasonable scope for the requested change.
-   Distinguish current implementation from target product behavior.
-   Do not infer mobile FluxBar behavior unless it is explicitly
    documented for FluxBar Desktop.
-   Do not copy features from the mobile FluxBar project merely because
    they exist there.

For non-trivial changes, briefly state the intended approach before
implementation.

If a task requires a significant architectural decision not implied by
the code, documentation, or task description, explain the alternatives
before implementing it.

## Product Boundaries

FluxBar Desktop is primarily a lightweight Miniflux news inbox and
triage tool, not a traditional full desktop RSS reader.

-   Miniflux is the remote source of truth for feed/article state.
-   Web articles are normally opened in the user's browser.
-   Do not introduce a full embedded browser or full article-reading
    surface without an explicit product decision.
-   Podcast audio is an intentional exception and may be consumed
    directly inside FluxBar.
-   The macOS application is Menu Bar first and uses a native popover.
-   The UI should be native to each platform. Do not compromise macOS UX
    for future Windows/Linux UI reuse.
-   Platform-independent business logic belongs in the portable core.
    The existing Go core is the current production/reference
    implementation; the Rust core is being developed in parallel as its
    intended replacement.
-   Platform-specific presentation and OS integration belong in the
    native UI layer.
-   Swipe gestures are optional accelerators; no important action may
    depend on gestures alone.

See `docs/ARCHITECTURE_DECISIONS.md` for the full set of invariants.

## macOS UI Rules

-   Prefer SwiftUI and AppKit system behavior over custom imitation of
    macOS styling.
-   The sidebar is hidden by default.
-   Revealing navigation should expand the popover horizontally rather
    than significantly shrinking the article content column.
-   Article rows are optimized for scanning and triage.
-   Use progressive disclosure for secondary actions: hover controls,
    context menus, compact overflow controls.
-   Clicking a normal article opens its original URL in the
    configured/default browser.
-   Preserve future keyboard, Spotlight, App Intent, and deep-link
    routing by keeping selection/navigation state independent from
    individual views.

## Data and Sync Rules

-   Local SQLite data should allow the popover to render immediately
    without waiting for Miniflux.
-   Read/unread and starred actions should update local state
    immediately and synchronize through the established sync mechanism.
-   Never treat image cache data as durable application state.
-   Automatic mark-as-read-on-scrollover must require meaningful
    visibility and must not mass-mark content skipped by jumps or
    programmatic navigation.
-   Sync completion itself is not a user notification event.

## Podcasts

-   Reuse existing shared podcast logic where available.
-   Preserve synchronized playback position.
-   Distinguish Stop from Eject.
-   FluxBar should provide its own compact controls and also integrate
    with macOS Now Playing.
-   Chapters and playback speed are important player features.

## Implementation

-   Keep changes focused on the requested task.
-   Prefer modifying existing abstractions over creating parallel
    implementations.
-   Avoid unrelated cleanup or refactoring.
-   Avoid new dependencies unless clearly justified.
-   Preserve public interfaces unless changing them is part of the task.
-   Do not silently change behavior outside the requested scope.
-   If an unrelated issue is discovered, report it instead of fixing it
    automatically.

## Validation

Use validation appropriate to the actual repository and affected target.

For macOS UI changes, build the affected macOS target and run relevant
tests. For Go changes, run the relevant Go tests and static checks
already established by the repository. For Rust-core migration changes,
also run the Rust checks and compatibility validation defined in
`docs/RUST_CORE_TESTING.md`.

Never claim a build, test, or command succeeded unless it was actually
executed successfully.

## Risk-Based Review

Apply additional scrutiny when changes affect:

-   persistence/database migrations
-   Miniflux synchronization/reconciliation
-   concurrency/background execution
-   credentials/authentication/security/privacy
-   automatic read-state mutation
-   notifications
-   audio sessions/media state
-   playback-position synchronization
-   native platform bridges
-   signing/notarization/App Store/Homebrew distribution

Explicitly mention meaningful risks in the final review.

## Human Review Summary

After a non-trivial implementation, provide a concise review grouped by
logical change.

For each logical change include:

### `<short description>`

**What:** What behavior or implementation changed.

**Why:** Why it was necessary.

**Code:** Relevant files and symbols to inspect.

**Risk:** Only when there is a meaningful risk, tradeoff, compatibility
concern, or behavior deserving attention.

Then finish with:

### Validation

List only checks actually performed and their results.

### Review Focus

List the areas that deserve particular human attention.

## Context Efficiency

Read only the documentation relevant to the task. Avoid repeatedly
loading unchanged files or unrelated feature documents.

The goal is to spend agent context on understanding, implementing,
validating, and explaining the requested change.

## Active Go-to-Rust Core Migration

FluxBar is incrementally migrating the portable core from Go to Rust.

During this migration:

-   The existing Go core is the behavioral reference implementation.
-   Keep Go and Rust implementations side by side until Rust has proven
    compatible and stable.
-   Compatibility comes before redesign. Do not silently fix unusual Go
    behavior while porting it.
-   Preserve the current C/JSON bridge initially, including
    `FluxCoreRequest` / `FluxCoreFree`, JSON field semantics, snapshot
    compatibility, and SQLite interoperability.
-   Do not introduce UniFFI during the initial compatibility migration.
    UniFFI is a later, separately evaluated adapter.
-   Do not redesign the SQLite schema as part of the language migration.
-   Do not move platform-specific macOS behavior into Rust merely to
    share more code.
-   Keep FFI concerns at the outer adapter boundary. Domain code must
    not depend on raw C pointers, Swift, AppKit, or SwiftUI.
-   Rust is now the default core for normal development/local builds; the
    Go core remains the production/reference implementation and explicit
    fallback.
-   Do not advance into a later migration phase unless the task requests
    it.

For Rust migration tasks, read `docs/RUST_CORE_MIGRATION.md`,
`docs/CORE_COMPATIBILITY_CONTRACT.md`, and `docs/RUST_CORE_TESTING.md`
in addition to the normal task-specific documentation.
