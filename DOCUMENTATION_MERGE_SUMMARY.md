# Documentation Merge Summary

The existing FluxBar documentation was preserved as the base.

## Updated

-   `AGENTS.md`
    -   portable-core wording;
    -   active Go-to-Rust migration rules;
    -   links to migration-specific documentation;
    -   Rust validation routing.
-   `docs/ARCHITECTURE_DECISIONS.md`
    -   durable shared-core decision made language-neutral;
    -   Go identified as current/reference implementation;
    -   Rust parallel-migration invariant added;
    -   UniFFI explicitly deferred.
-   `docs/DEVELOPER_MAP.md`
    -   current Go state retained;
    -   target portable-core terminology added;
    -   parallel Go/Rust migration diagram added.
-   `docs/features/SYNC_AND_DATA.md`
    -   current Go feed-icon cache retained as an implementation fact
        while making its behavior a Rust compatibility requirement.
-   `docs/RELEASE_AND_DISTRIBUTION.md`
    -   release safety during the parallel migration added.
-   `docs/README.md`
    -   migration documents added to context-routing guidance.

## Added

-   `docs/CORE_COMPATIBILITY_CONTRACT.md`
-   `docs/RUST_CORE_MIGRATION.md`
-   `docs/RUST_CORE_TESTING.md`
-   `PROMPT_01_RUST_CORE_MIGRATION.md`

## Deliberately not rewritten

-   feature/product details unrelated to the language migration;
-   `LEGACY_PRODUCT_CONTEXT.md`, because it is historical/legacy
    context;
-   product backlog items;
-   macOS UI behavior;
-   podcast/notification plans.

The first migration prompt instructs the coding agent to audit actual Go
code before filling in concrete ABI/JSON details. This avoids encoding
assumptions from the planning conversation as repository facts.
