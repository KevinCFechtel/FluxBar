//! Typed operation dispatcher.
//!
//! The dispatcher is the boundary between the JSON compatibility adapter and
//! the runtime/domain implementations. Every supported operation routes through
//! this typed boundary without exposing JSON details to domain code.

use std::time::Instant;

use crate::runtime::AppRuntime;
use crate::transport::{Operation, Response};

/// Dispatches a typed operation to its handler.
///
/// All 11 current public operations have real handlers.
pub fn dispatch(operation: Operation, runtime: &AppRuntime) -> Response {
    let name = operation_name(&operation);
    let is_localize = is_localize_operation(&operation);
    if is_localize {
        log::debug!(target: "ffi", "operation={name} dispatching");
    } else {
        log::info!(target: "ffi", "operation={name} dispatching");
    }
    let start = Instant::now();

    let response = match operation {
        Operation::Configure {
            server,
            api_key,
            newest_first,
            configuration_generation,
            locales,
        } => runtime.configure(
            &server,
            &api_key,
            newest_first,
            configuration_generation,
            &locales,
        ),
        Operation::LocalSnapshot {
            selection,
            retain_entry_ids,
        } => runtime.local_snapshot(
            &selection.kind,
            selection.id,
            selection.unread_only,
            &retain_entry_ids,
        ),
        Operation::Refresh {
            selection,
            retain_entry_ids,
        } => runtime.refresh(
            &selection.kind,
            selection.id,
            selection.unread_only,
            &retain_entry_ids,
        ),
        Operation::SetRead {
            selection,
            entry_id,
            entry_ids,
            retain_entry_ids,
            read,
            mutation_source,
        } => runtime.set_read(
            &selection.kind,
            selection.id,
            selection.unread_only,
            entry_id,
            &entry_ids,
            &retain_entry_ids,
            read,
            mutation_source == "automatic",
        ),
        Operation::SetStarred {
            selection,
            entry_id,
            retain_entry_ids,
            desired_starred,
        } => runtime.set_starred(
            &selection.kind,
            selection.id,
            selection.unread_only,
            entry_id,
            &retain_entry_ids,
            desired_starred,
        ),
        Operation::UndoRead {
            selection,
            mutation_id,
            retain_entry_ids,
        } => runtime.undo_read(
            &selection.kind,
            selection.id,
            selection.unread_only,
            &mutation_id,
            &retain_entry_ids,
        ),
        Operation::DiscardUndo { mutation_id } => runtime.discard_undo(&mutation_id),
        Operation::FlushPending {
            selection,
            retain_entry_ids,
        } => runtime.flush_pending(
            &selection.kind,
            selection.id,
            selection.unread_only,
            &retain_entry_ids,
        ),
        Operation::FeedIcon { feed_id, .. } => runtime.feed_icon(feed_id),
        Operation::Localize {
            locales,
            key,
            fallback,
        } => handlers::localize(&locales, &key, &fallback),
        Operation::LocalizePlural {
            locales,
            key,
            one_fallback,
            other_fallback,
            count,
        } => handlers::localize_plural(&locales, &key, &one_fallback, &other_fallback, count),
    };

    let elapsed = start.elapsed().as_millis();
    if response.ok {
        if is_localize {
            log::debug!(target: "ffi", "operation={name} completed duration_ms={elapsed}");
        } else {
            log::info!(target: "ffi", "operation={name} completed duration_ms={elapsed}");
        }
    } else {
        log::warn!(
            target: "ffi",
            "operation={name} failed duration_ms={elapsed} error={}",
            response.error
        );
    }
    response
}

fn is_localize_operation(operation: &Operation) -> bool {
    matches!(
        operation,
        Operation::Localize { .. } | Operation::LocalizePlural { .. }
    )
}

fn operation_name(operation: &Operation) -> &'static str {
    match operation {
        Operation::Configure { .. } => "configure",
        Operation::LocalSnapshot { .. } => "local_snapshot",
        Operation::Refresh { .. } => "refresh",
        Operation::SetRead { .. } => "set_read",
        Operation::SetStarred { .. } => "set_starred",
        Operation::UndoRead { .. } => "undo_read",
        Operation::DiscardUndo { .. } => "discard_undo",
        Operation::FlushPending { .. } => "flush_pending",
        Operation::FeedIcon { .. } => "feed_icon",
        Operation::Localize { .. } => "localize",
        Operation::LocalizePlural { .. } => "localize_plural",
    }
}

/// Operation-specific handler stubs.
mod handlers {
    use crate::transport::Response;

    pub fn localize(locales: &[String], key: &str, fallback: &str) -> Response {
        let localizer = crate::localization::Localizer::new(locales);
        Response {
            ok: true,
            text: localizer.text(key, fallback),
            ..Response::default()
        }
    }

    pub fn localize_plural(
        locales: &[String],
        key: &str,
        one_fallback: &str,
        other_fallback: &str,
        count: i64,
    ) -> Response {
        let localizer = crate::localization::Localizer::new(locales);
        Response {
            ok: true,
            text: localizer.plural(key, one_fallback, other_fallback, count),
            ..Response::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::Operation;

    #[test]
    fn feed_icon_requires_configuration() {
        let runtime = crate::runtime::AppRuntime::new();
        let resp = dispatch(
            Operation::FeedIcon {
                feed_id: 7,
                feed_name: String::new(),
            },
            &runtime,
        );
        assert!(!resp.ok);
        assert_eq!(resp.error, "Miniflux is not configured");
    }

    #[test]
    fn localize_returns_localized_text() {
        let runtime = crate::runtime::AppRuntime::new();
        let resp = dispatch(
            Operation::Localize {
                locales: vec!["de-DE".to_string()],
                key: "menu.refresh".to_string(),
                fallback: "fallback".to_string(),
            },
            &runtime,
        );
        assert!(resp.ok);
        assert_eq!(resp.text, "Aktualisieren");
    }

    #[test]
    fn localize_plural_returns_localized_text() {
        let runtime = crate::runtime::AppRuntime::new();
        let resp = dispatch(
            Operation::LocalizePlural {
                locales: vec!["de".to_string()],
                key: "status.unread_count".to_string(),
                one_fallback: "FluxBar — {{.Count}} unread article".to_string(),
                other_fallback: "FluxBar — {{.Count}} unread articles".to_string(),
                count: 2,
            },
            &runtime,
        );
        assert!(resp.ok);
        assert_eq!(resp.text, "FluxBar — 2 ungelesene Artikel");
    }

    #[test]
    fn implemented_local_handlers_route_correctly() {
        use crate::transport::request::Selection;

        let runtime = crate::runtime::AppRuntime::new();

        // configure routes into validation (empty credentials -> Go's
        // localized-fallback error, not "not implemented").
        let configure_response = dispatch(
            Operation::Configure {
                server: String::new(),
                api_key: String::new(),
                newest_first: false,
                configuration_generation: 0,
                locales: vec![],
            },
            &runtime,
        );
        assert!(!configure_response.ok);
        assert_eq!(
            configure_response.error,
            "The server URL must be a complete HTTP or HTTPS URL."
        );

        // local_snapshot routes into the runtime and reports unconfigured.
        let snapshot_response = dispatch(
            Operation::LocalSnapshot {
                selection: Selection::default(),
                retain_entry_ids: vec![],
            },
            &runtime,
        );
        assert!(!snapshot_response.ok);
        assert_eq!(snapshot_response.error, "Miniflux is not configured");
    }
}
