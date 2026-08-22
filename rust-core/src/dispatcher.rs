//! Typed operation dispatcher.
//!
//! The dispatcher is the boundary between the JSON compatibility adapter and
//! future domain implementations. In Phase 3 every supported operation is
//! routed to a handler that returns a deterministic "not implemented"
//! response. Later phases can replace individual handlers with real behavior
//! without changing the FFI or transport layers.

use crate::transport::{Operation, Response};

/// Dispatches a typed operation to its handler.
///
/// Phase 3 handlers are intentionally stubs. The value of this function is
/// ensuring that the external operation string is correctly recognized and
/// routed to the correct typed handler.
pub fn dispatch(operation: Operation) -> Response {
    match operation {
        Operation::Configure { .. } => handlers::configure(),
        Operation::LocalSnapshot { .. } => handlers::local_snapshot(),
        Operation::Refresh { .. } => handlers::refresh(),
        Operation::SetRead { .. } => handlers::set_read(),
        Operation::SetStarred { .. } => handlers::set_starred(),
        Operation::UndoRead { .. } => handlers::undo_read(),
        Operation::DiscardUndo { .. } => handlers::discard_undo(),
        Operation::FlushPending { .. } => handlers::flush_pending(),
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

    not_implemented!(configure, "configure");
    not_implemented!(local_snapshot, "local_snapshot");
    not_implemented!(refresh, "refresh");
    not_implemented!(set_read, "set_read");
    not_implemented!(set_starred, "set_starred");
    not_implemented!(undo_read, "undo_read");
    not_implemented!(discard_undo, "discard_undo");
    not_implemented!(flush_pending, "flush_pending");
    not_implemented!(feed_icon, "feed_icon");
    not_implemented!(localize, "localize");
    not_implemented!(localize_plural, "localize_plural");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::Operation;
    use crate::transport::request::Selection;

    #[test]
    fn dispatch_routes_to_correct_handler() {
        let cases = vec![
            (
                Operation::Configure {
                    server: String::new(),
                    api_key: String::new(),
                    newest_first: false,
                    configuration_generation: 0,
                    locales: vec![],
                },
                "configure",
            ),
            (
                Operation::LocalSnapshot {
                    selection: Selection::default(),
                    retain_entry_ids: vec![],
                },
                "local_snapshot",
            ),
            (
                Operation::Refresh {
                    selection: Selection::default(),
                    retain_entry_ids: vec![],
                },
                "refresh",
            ),
            (
                Operation::SetRead {
                    selection: Selection::default(),
                    entry_id: 0,
                    entry_ids: vec![],
                    retain_entry_ids: vec![],
                    read: false,
                    mutation_source: String::new(),
                },
                "set_read",
            ),
            (
                Operation::SetStarred {
                    selection: Selection::default(),
                    entry_id: 0,
                    retain_entry_ids: vec![],
                    desired_starred: false,
                },
                "set_starred",
            ),
            (
                Operation::UndoRead {
                    selection: Selection::default(),
                    mutation_id: String::new(),
                    retain_entry_ids: vec![],
                },
                "undo_read",
            ),
            (
                Operation::DiscardUndo {
                    mutation_id: String::new(),
                },
                "discard_undo",
            ),
            (
                Operation::FlushPending {
                    selection: Selection::default(),
                    retain_entry_ids: vec![],
                },
                "flush_pending",
            ),
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
        for (op, expected_name) in cases {
            let resp = dispatch(op);
            assert!(!resp.ok);
            assert!(
                resp.error
                    .contains(&format!("not implemented: {expected_name}"))
            );
        }
    }
}
