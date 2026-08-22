//! Application runtime state: local configuration and the open store.
//!
//! Mirrors `go-core/internal/coreapi` Runtime behavior.
//! Go's `configure` has no remote effects (validation, account upsert, engine
//! creation only), so a network-free implementation reproduces its observable
//! contract. Validation errors use the shared localization catalogs.

use std::path::PathBuf;
use std::sync::Mutex;

use crate::domain::account::account_id;
use crate::domain::selection::Selection;
use crate::persistence::Store;
use crate::remote::MinifluxClient;
use crate::sync::{SyncResult, SyncService};
use crate::transport::response::{BrowseSnapshot, Icon, Response};

struct Config {
    #[cfg_attr(not(test), allow(dead_code))]
    account_id: String,
    generation: i64,
}

struct Session {
    config: Config,
    engine: std::sync::Arc<SyncService>,
}

pub struct AppRuntime {
    session: Mutex<Option<Session>>,
    database_path_override: Option<PathBuf>,
}

impl Default for AppRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl AppRuntime {
    pub const fn new() -> Self {
        Self {
            session: Mutex::new(None),
            database_path_override: None,
        }
    }

    #[cfg(test)]
    pub fn with_database_path(path: PathBuf) -> Self {
        Self {
            session: Mutex::new(None),
            database_path_override: Some(path),
        }
    }

    /// Validates credentials, derives the account ID, opens/creates the local
    /// store, and upserts the account. Never contacts Miniflux.
    ///
    /// Stale generations are accepted with `ok:true` while leaving the
    /// existing session untouched, matching Go's race protection.
    pub fn configure(
        &self,
        server: &str,
        api_key: &str,
        newest_first: bool,
        generation: i64,
        locales: &[String],
    ) -> Response {
        let normalized_server = match validate_configuration(server, api_key, locales) {
            Ok(server) => server,
            Err(message) => return Response::error(message),
        };
        let trimmed_key = api_key.trim();
        let derived_account = account_id(&normalized_server, trimmed_key);

        let path = match &self.database_path_override {
            Some(path) => path.clone(),
            None => match default_database_path() {
                Ok(path) => path,
                Err(error) => return Response::error(error),
            },
        };
        let store = match Store::open(&path) {
            Ok(store) => store,
            Err(error) => return Response::error(format!("SQLite öffnen: {error}")),
        };
        if let Err(error) = store.ensure_account(&derived_account, &normalized_server) {
            return Response::error(error.to_string());
        }

        let remote = match MinifluxClient::new(&normalized_server, trimmed_key) {
            Ok(remote) => remote,
            Err(error) => return Response::error(error.to_string()),
        };
        let engine = SyncService::new(
            store,
            Box::new(remote),
            derived_account.clone(),
            newest_first,
        );

        let mut guard = locked(&self.session);
        if guard
            .as_ref()
            .is_none_or(|existing| generation >= existing.config.generation)
        {
            *guard = Some(Session {
                config: Config {
                    account_id: derived_account,
                    generation,
                },
                engine,
            });
        }
        Response::ok()
    }

    /// Builds a snapshot purely from local SQLite state.
    pub fn local_snapshot(
        &self,
        kind: &str,
        id: i64,
        unread_only: bool,
        retain_entry_ids: &[i64],
    ) -> Response {
        let guard = locked(&self.session);
        let Some(session) = guard.as_ref() else {
            return Response::not_configured();
        };

        let selection = Selection::normalize(kind, id, unread_only);
        match session.engine.local_snapshot(&selection, retain_entry_ids) {
            Ok(data) => snapshot_response(data),
            Err(error) => Response::error(error.to_string()),
        }
    }

    pub fn refresh(
        &self,
        kind: &str,
        id: i64,
        unread_only: bool,
        retain_entry_ids: &[i64],
    ) -> Response {
        let Some(engine) = self.current_engine() else {
            return Response::not_configured();
        };
        let selection = Selection::normalize(kind, id, unread_only);
        match engine.sync(&selection, retain_entry_ids) {
            Ok(SyncResult::Success(data)) => snapshot_response(data),
            Ok(SyncResult::Partial(data, error)) => {
                let mut response = snapshot_response(data);
                response.error = error;
                response
            }
            Err(error) => Response::error(error),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn set_read(
        &self,
        kind: &str,
        id: i64,
        unread_only: bool,
        entry_id: i64,
        entry_ids: &[i64],
        retain_entry_ids: &[i64],
        read: bool,
        automatic: bool,
    ) -> Response {
        let Some(engine) = self.current_engine() else {
            return Response::not_configured();
        };
        let ids = if entry_ids.is_empty() && entry_id > 0 {
            vec![entry_id]
        } else {
            entry_ids.to_vec()
        };
        if ids.is_empty() {
            return Response::error("missing entry IDs");
        }
        let selection = Selection::normalize(kind, id, unread_only);
        match engine.mark_read(&selection, &ids, retain_entry_ids, read, automatic) {
            Ok((data, receipt)) => {
                let mut response = snapshot_response(data);
                response.receipt = receipt.map(|receipt| crate::transport::response::Receipt {
                    id: receipt.id,
                    count: receipt.count,
                });
                response
            }
            Err(error) => Response::error(error),
        }
    }

    pub fn set_starred(
        &self,
        kind: &str,
        id: i64,
        unread_only: bool,
        entry_id: i64,
        retain_entry_ids: &[i64],
        starred: bool,
    ) -> Response {
        let Some(engine) = self.current_engine() else {
            return Response::not_configured();
        };
        let selection = Selection::normalize(kind, id, unread_only);
        match engine.set_starred(&selection, entry_id, starred, retain_entry_ids) {
            Ok(data) => snapshot_response(data),
            Err(error) => Response::error(error),
        }
    }

    pub fn undo_read(
        &self,
        kind: &str,
        id: i64,
        unread_only: bool,
        mutation_id: &str,
        retain_entry_ids: &[i64],
    ) -> Response {
        let Some(engine) = self.current_engine() else {
            return Response::not_configured();
        };
        let selection = Selection::normalize(kind, id, unread_only);
        match engine.undo(&selection, mutation_id, retain_entry_ids) {
            Ok(data) => snapshot_response(data),
            Err(error) => Response::error(error),
        }
    }

    pub fn discard_undo(&self, mutation_id: &str) -> Response {
        let Some(engine) = self.current_engine() else {
            return Response::not_configured();
        };
        match engine.discard_undo(mutation_id) {
            Ok(()) => Response::ok(),
            Err(error) => Response::error(error),
        }
    }

    pub fn flush_pending(
        &self,
        kind: &str,
        id: i64,
        unread_only: bool,
        retain_entry_ids: &[i64],
    ) -> Response {
        let Some(engine) = self.current_engine() else {
            return Response::not_configured();
        };
        if let Err(error) = engine.flush() {
            return Response::error(error);
        }
        let selection = Selection::normalize(kind, id, unread_only);
        match engine.local_snapshot(&selection, retain_entry_ids) {
            Ok(data) => snapshot_response(data),
            Err(error) => Response::error(error),
        }
    }

    pub fn feed_icon(&self, feed_id: i64) -> Response {
        let Some(engine) = self.current_engine() else {
            return Response::not_configured();
        };
        let icon = engine.feed_icon(feed_id);
        Response {
            ok: true,
            icon: Some(Icon {
                regular: icon.regular,
                dark: icon.dark,
            }),
            ..Response::default()
        }
    }

    fn current_engine(&self) -> Option<std::sync::Arc<SyncService>> {
        locked(&self.session)
            .as_ref()
            .map(|session| std::sync::Arc::clone(&session.engine))
    }
}

fn snapshot_response(data: crate::persistence::SnapshotData) -> Response {
    let snapshot: BrowseSnapshot = crate::snapshot::assemble(&data);
    Response {
        ok: true,
        snapshot: Some(snapshot),
        ..Response::default()
    }
}

fn locked<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    // A poisoned lock still contains valid data; recovering avoids turning an
    // already-contained panic into a second failure at the FFI boundary.
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Same validation rules as Go's `validateConfiguration`. Error messages are
/// localized using the caller's preferred locales, falling back to English
/// when no supported locale matches.
fn validate_configuration(
    server: &str,
    api_key: &str,
    locales: &[String],
) -> Result<String, String> {
    let localizer = crate::localization::Localizer::new(locales);
    let trimmed = server.trim().trim_end_matches('/');
    if !has_http_host(trimmed) {
        return Err(localizer.text(
            "validation.server_invalid",
            "The server URL must be a complete HTTP or HTTPS URL.",
        ));
    }
    if api_key.trim().is_empty() {
        return Err(localizer.text(
            "validation.api_key_required",
            "Please enter a Miniflux API key.",
        ));
    }
    Ok(trimmed.to_string())
}

fn has_http_host(candidate: &str) -> bool {
    let parsed = url::Url::parse(candidate).or_else(|error| {
        if error != url::ParseError::InvalidPort {
            return Err(error);
        }
        // Go accepts an all-numeric port above 65535 during URL parsing. Use a
        // valid placeholder only for validation and preserve the original URL.
        let scheme_end = candidate.find("://").ok_or(error)? + 3;
        let authority_end = candidate[scheme_end..]
            .find(['/', '?', '#'])
            .map_or(candidate.len(), |index| scheme_end + index);
        let authority = &candidate[scheme_end..authority_end];
        let colon = authority.rfind(':').ok_or(error)?;
        let port = &authority[colon + 1..];
        if port.is_empty() || !port.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(error);
        }
        let port_start = scheme_end + colon + 1;
        let replacement = format!(
            "{}1{}",
            &candidate[..port_start],
            &candidate[authority_end..]
        );
        url::Url::parse(&replacement)
    });
    parsed.is_ok_and(|url| {
        matches!(url.scheme(), "http" | "https")
            && url.host_str().is_some_and(|host| !host.is_empty())
    })
}

/// Production database location. macOS-only resolution for now, mirroring Go's
/// UserConfigDir behavior; kept outside portable persistence. Tests always
/// supply an explicit override path.
#[cfg(not(test))]
fn default_database_path() -> Result<PathBuf, String> {
    #[cfg(target_os = "macos")]
    {
        let home =
            std::env::var("HOME").map_err(|_| "Benutzerverzeichnis bestimmen".to_string())?;
        let directory = PathBuf::from(home)
            .join("Library")
            .join("Application Support")
            .join("FluxBar");
        // Mirror Go's os.MkdirAll(dir, 0700): create every missing component
        // with private permissions; existing directories stay untouched.
        let mut missing = Vec::new();
        let mut probe: &std::path::Path = directory.as_path();
        while !probe.exists() {
            missing.push(probe.to_path_buf());
            match probe.parent() {
                Some(parent) => probe = parent,
                None => break,
            }
        }
        for path in missing.iter().rev() {
            std::fs::create_dir(path)
                .map_err(|error| format!("Anwendungsdaten-Verzeichnis anlegen: {error}"))?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut permissions = std::fs::metadata(path)
                    .map_err(|e| e.to_string())?
                    .permissions();
                permissions.set_mode(0o700);
                std::fs::set_permissions(path, permissions).map_err(|e| e.to_string())?;
            }
        }
        Ok(directory.join("inbox.sqlite3"))
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("Plattform-Datenbankpfad noch nicht implementiert".to_string())
    }
}

/// Test builds never call this because they always supply an explicit path.
#[cfg(test)]
fn default_database_path() -> Result<PathBuf, String> {
    Err("database path override missing".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation_matches_go_fallback_messages() {
        assert_eq!(
            validate_configuration("not-a-url", "secret", &[]),
            Err("The server URL must be a complete HTTP or HTTPS URL.".to_string())
        );
        assert_eq!(
            validate_configuration("https://miniflux.example", "  ", &[]),
            Err("Please enter a Miniflux API key.".to_string())
        );
        assert_eq!(
            validate_configuration(" https://m.example/ ", " k ", &[]).as_deref(),
            Ok("https://m.example")
        );
        assert!(validate_configuration("HTTPS://M.EXAMPLE", "k", &[]).is_ok());
        assert!(validate_configuration("http://", "k", &[]).is_err());
        assert!(validate_configuration("ftp://m.example", "k", &[]).is_err());
        assert!(validate_configuration("http://exa mple.com", "k", &[]).is_err());
        assert!(validate_configuration("http://[::1", "k", &[]).is_err());
        assert!(validate_configuration("http://example.com:bad", "k", &[]).is_err());
        assert!(validate_configuration("http://example.com:65536", "k", &[]).is_ok());
    }

    #[test]
    fn validation_uses_localized_errors() {
        assert_eq!(
            validate_configuration("not-a-url", "secret", &["de-DE".to_string()]),
            Err("Die Server-URL muss eine vollständige HTTP- oder HTTPS-URL sein.".to_string())
        );
        assert_eq!(
            validate_configuration("https://miniflux.example", "  ", &["de-DE".to_string()]),
            Err("Bitte einen Miniflux-API-Key eingeben.".to_string())
        );
    }

    #[test]
    fn stale_generation_is_accepted_but_ignored() {
        let directory = test_directory();
        let runtime = AppRuntime::with_database_path(directory.path().join("inbox.sqlite3"));

        assert!(
            runtime
                .configure("https://a.example", "k1", false, 5, &[])
                .ok
        );
        let first_account = {
            let guard = locked(&runtime.session);
            guard.as_ref().unwrap().config.account_id.clone()
        };

        // Older generation must not replace the configured account.
        assert!(
            runtime
                .configure("https://b.example", "k2", true, 4, &[])
                .ok
        );
        let unchanged = {
            let guard = locked(&runtime.session);
            guard.as_ref().unwrap().config.account_id.clone()
        };
        assert_eq!(first_account, unchanged);

        // Equal or newer generations replace it (Go uses >=).
        assert!(
            runtime
                .configure("https://c.example", "k3", false, 5, &[])
                .ok
        );
        let replaced = {
            let guard = locked(&runtime.session);
            guard.as_ref().unwrap().config.account_id.clone()
        };
        assert_ne!(first_account, replaced);
    }

    #[test]
    fn account_switching_isolated_and_stale_configuration_still_upserts() {
        let directory = test_directory();
        let database_path = directory.path().join("inbox.sqlite3");
        let runtime = AppRuntime::with_database_path(database_path.clone());

        assert!(
            runtime
                .configure("https://a.example", "a", false, 1, &[])
                .ok
        );
        let account_a = locked(&runtime.session)
            .as_ref()
            .unwrap()
            .config
            .account_id
            .clone();
        assert!(
            runtime
                .configure("https://b.example", "b", false, 2, &[])
                .ok
        );
        let account_b = locked(&runtime.session)
            .as_ref()
            .unwrap()
            .config
            .account_id
            .clone();
        assert_ne!(account_a, account_b);

        assert!(
            runtime
                .configure("https://stale.example", "stale", false, 1, &[])
                .ok
        );
        assert_eq!(
            locked(&runtime.session).as_ref().unwrap().config.account_id,
            account_b
        );

        assert!(
            runtime
                .configure("https://a.example", "a", false, 3, &[])
                .ok
        );
        assert_eq!(
            locked(&runtime.session).as_ref().unwrap().config.account_id,
            account_a
        );

        let database = rusqlite::Connection::open(database_path).unwrap();
        let account_count: i64 = database
            .query_row("SELECT COUNT(*) FROM accounts", [], |row| row.get(0))
            .unwrap();
        assert_eq!(account_count, 3);
    }

    #[test]
    fn local_snapshot_requires_configuration() {
        let directory = test_directory();
        let runtime = AppRuntime::with_database_path(directory.path().join("inbox.sqlite3"));
        let response = runtime.local_snapshot("all", 0, true, &[]);
        assert!(!response.ok);
        assert_eq!(response.error, "Miniflux is not configured");
    }

    struct TestDirectory(std::path::PathBuf);

    impl TestDirectory {
        fn path(&self) -> &std::path::Path {
            &self.0
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn test_directory() -> TestDirectory {
        use std::sync::atomic::{AtomicUsize, Ordering};

        static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "fluxbar-runtime-test-{}-{}",
            std::process::id(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        TestDirectory(path)
    }
}
