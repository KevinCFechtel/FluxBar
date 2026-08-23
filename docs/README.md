# FluxBar Desktop Context Documentation

The project context is split by purpose so humans and coding agents can
load only the information relevant to a task.

This documentation describes **FluxBar Desktop**. It intentionally does
not inherit mobile FluxBar features unless they are explicitly
documented here.

## Always Relevant

-   `../AGENTS.md` --- coding-agent instructions and repository-wide
    working rules.
-   `DEVELOPER_MAP.md` --- compact product/system model.
-   `ARCHITECTURE_DECISIONS.md` --- durable product and architecture
    invariants.

## Feature-Specific

-   `features/MACOS_UI.md` --- Menu Bar popover, sidebar, article rows,
    actions, keyboard and Spotlight direction.
-   `features/SYNC_AND_DATA.md` --- SQLite, local-first behavior, image
    cache, sync semantics, mark-as-read-on-scrollover.
-   `features/PODCASTS.md` --- desktop podcast playback and Now Playing.
-   `features/NOTIFICATIONS.md` --- selective notification behavior.

## Core Migration and Shared-Core Planning

-   `RUST_CORE_MIGRATION.md` --- staged Go-to-Rust migration, scope
    gates, and rollout/removal criteria.
-   `CORE_COMPATIBILITY_CONTRACT.md` --- external ABI/JSON/persistence
    behavior Rust must reproduce.
-   `RUST_CORE_TESTING.md` --- Rust, ABI, differential, database, and
    integration testing strategy.
-   `FLUXNEWS_CORE_GAP_ANALYSIS.md` --- source-backed inventory of current
    Rust coverage and FluxNews responsibilities.
-   `SHARED_RUST_CORE_ROADMAP.md` --- post-FluxBar shared-core, native-client,
    binding, persistence, and Flutter migration execution plan.
-   `MOBILE_RUNTIME_PROOF_CONTRACT.md` --- exact Phase 1 iOS/Android artifact,
    lifecycle, SQLite, TLS, FFI, and physical-device proof contract.

Read these only for core migration work or when changing the shared core
boundary.

## Other Context

-   `RELEASE_AND_DISTRIBUTION.md` --- intended Mac App Store, GitHub,
    notarization, and Homebrew direction.
-   `PRODUCT_BACKLOG.md` --- open questions and future ideas.

## Suggested Agent Workflow

1.  Read `AGENTS.md`.
2.  Read `DEVELOPER_MAP.md` when architectural orientation is needed.
3.  Read the relevant feature document(s).
4.  Read `ARCHITECTURE_DECISIONS.md` before changing core/platform
    boundaries, sync, persistence, automatic state mutation, or media
    behavior.
5.  For Go-to-Rust work, read the compatibility migration documents. For
    shared-core or native FluxNews work, also read the GAP analysis and roadmap.
6.  Read release/backlog documents only when the task involves them.

Do not load all context files by default.
