//! Typed operation dispatcher.
//!
//! The dispatcher is the boundary between the JSON compatibility adapter and
//! future domain implementations. In Phase 3 every supported operation is
//! routed to a handler that returns a deterministic "not implemented"
//! response. Later phases can replace individual handlers with real behavior
//! without changing the FFI or transport layers.

use crate::runtime::AppRuntime;
use crate::transport::{Operation, Response};

/// Dispatches a typed operation to its handler.
///
/// Phase 8 implements configuration, local/remote snapshots, and the complete
/// read/star/pending/Undo mutation surface. Supporting services remain stubs.
pub fn dispatch(operation: Operation, runtime: &AppRuntime) -> Response {
    match operation {
        Operation::Configure {
            server,
            api_key,
            newest_first,
            configuration_generation,
            locales: _,
        } => runtime.configure(&server, &api_key, newest_first, configuration_generation),
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
        Operation::FeedIcon { .. } => handlers::feed_icon(),
        Operation::Localize { .. } => handlers::localize(),
        Operation::LocalizePlural { .. } => handlers::localize_plural(),
    }
}

/// Operation-specific handler stubs.
mod handlers {
    use crate::transport::Response;

    macro_rules! not_implemented {
        ($name:ident, $op:literal) => {
            pub fn $name() -> Response {
                Response::not_implemented($op)
            }
        };
    }

    not_implemented!(feed_icon, "feed_icon");
    not_implemented!(localize, "localize");
    not_implemented!(localize_plural, "localize_plural");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::Operation;

    #[test]
    fn dispatch_routes_to_correct_handler() {
        let cases = vec![
            (
                Operation::FeedIcon {
                    feed_id: 0,
                    feed_name: String::new(),
                },
                "feed_icon",
            ),
            (
                Operation::Localize {
                    locales: vec![],
                    key: String::new(),
                    fallback: String::new(),
                },
                "localize",
            ),
            (
                Operation::LocalizePlural {
                    locales: vec![],
                    key: String::new(),
                    one_fallback: String::new(),
                    other_fallback: String::new(),
                    count: 0,
                },
                "localize_plural",
            ),
        ];
        let runtime = crate::runtime::AppRuntime::new();
        for (op, expected_name) in cases {
            let resp = dispatch(op, &runtime);
            assert!(!resp.ok);
            assert!(
                resp.error
                    .contains(&format!("not implemented: {expected_name}"))
            );
        }
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
