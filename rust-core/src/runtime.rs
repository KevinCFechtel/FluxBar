//! Application runtime state: local configuration and the open store.
//!
//! Mirrors `go-core/internal/coreapi` Runtime behavior.
//! Go's `configure` has no remote effects (validation, account upsert, engine
//! creation only), so a network-free implementation reproduces its observable
//! contract. Validation errors use the shared localization catalogs.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, Weak};
use std::time::Instant;

use crate::domain::account::account_id;
use crate::domain::selection::Selection;
use crate::persistence::Store;
use crate::remote::MinifluxClient;
use crate::sync::{SharedStore, SyncResult, SyncService};
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
    store: Mutex<Option<Arc<SharedStore>>>,
    services: Mutex<Vec<(String, Weak<SyncService>)>>,
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
            store: Mutex::new(None),
            services: Mutex::new(Vec::new()),
            database_path_override: None,
        }
    }

    #[cfg(test)]
    pub fn with_database_path(path: PathBuf) -> Self {
        Self {
            session: Mutex::new(None),
            store: Mutex::new(None),
            services: Mutex::new(Vec::new()),
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
        log::info!(target: "configure", "configure started");
        let start = Instant::now();

        let normalized_server = match validate_configuration(server, api_key, locales) {
            Ok(server) => server,
            Err(message) => {
                log::warn!(target: "configure", "configure failed: {message}");
                return Response::error(message);
            }
        };
        let trimmed_key = api_key.trim();
        let derived_account = account_id(&normalized_server, trimmed_key);
        let account_prefix = account_prefix(&derived_account);

        let path = match &self.database_path_override {
            Some(path) => path.clone(),
            None => match default_database_path() {
                Ok(path) => path,
                Err(error) => {
                    log::error!(target: "configure", "database path resolution failed: {error}");
                    return Response::error(error);
                }
            },
        };
        log::info!(
            target: "configure",
            "opening store account={account_prefix} generation={generation}"
        );
        let store = {
            let mut current = locked(&self.store);
            match current.as_ref() {
                Some(store) => Arc::clone(store),
                None => {
                    let opened = match Store::open(&path) {
                        Ok(store) => {
                            log::info!(target: "configure", "store opened account={account_prefix}");
                            SharedStore::new(store)
                        }
                        Err(error) => {
                            let summary = match &error {
                                crate::persistence::OpenError::Sqlite(e) => format!(
                                    "category={} error={}",
                                    crate::persistence::sqlite_error_category(e),
                                    crate::persistence::sqlite_error_summary(e)
                                ),
                                crate::persistence::OpenError::Filesystem(_) => {
                                    "database permissions".to_string()
                                }
                            };
                            log::error!(target: "configure", "store open failed: {summary}");
                            return Response::error(format!("SQLite öffnen: {error}"));
                        }
                    };
                    *current = Some(Arc::clone(&opened));
                    opened
                }
            }
        };
        if let Err(error) = store.ensure_account(&derived_account, &normalized_server) {
            log::error!(target: "configure", "account upsert failed: {error}");
            return Response::error(error);
        }
        match (
            store.pending_count(&derived_account),
            store.undo_count(&derived_account),
        ) {
            (Ok(pending), Ok(undo)) => {
                log::info!(
                    target: "configure",
                    "existing account state loaded account={account_prefix} pending_mutations={pending} undo_batches={undo}"
                );
                if pending > 0 {
                    log::info!(
                        target: "configure",
                        "pending mutations will be flushed by startup refresh pending_count={pending}"
                    );
                } else {
                    log::info!(target: "configure", "no pending mutations found");
                }
            }
            (Err(error), _) | (_, Err(error)) => {
                log::warn!(target: "configure", "startup recovery state query failed: {error}");
            }
        }

        let remote = match MinifluxClient::new(&normalized_server, trimmed_key) {
            Ok(remote) => remote,
            Err(error) => {
                log::error!(target: "configure", "remote client creation failed: {error}");
                return Response::error(error.to_string());
            }
        };
        let candidate = SyncService::with_shared_store(
            Arc::clone(&store),
            Box::new(remote),
            derived_account.clone(),
            newest_first,
        );

        let mut guard = locked(&self.session);
        let replaced = generation >= guard.as_ref().map_or(0, |s| s.config.generation);
        if guard
            .as_ref()
            .is_none_or(|existing| generation >= existing.config.generation)
        {
            let mut services = locked(&self.services);
            let retained = services
                .iter()
                .find(|(account_id, _)| account_id == &derived_account)
                .and_then(|(_, service)| service.upgrade());
            let engine = retained.unwrap_or(candidate);
            engine.set_newest_first(newest_first);
            services.retain(|(account_id, service)| {
                account_id == &derived_account || service.strong_count() > 0
            });
            if let Some((_, service)) = services
                .iter_mut()
                .find(|(account_id, _)| account_id == &derived_account)
            {
                *service = Arc::downgrade(&engine);
            } else {
                services.push((derived_account.clone(), Arc::downgrade(&engine)));
            }
            *guard = Some(Session {
                config: Config {
                    account_id: derived_account,
                    generation,
                },
                engine,
            });
        }
        let elapsed = start.elapsed().as_millis();
        log::info!(
            target: "configure",
            "configure completed account={account_prefix} generation={generation} replaced={replaced} duration_ms={elapsed}"
        );
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
        let Some(engine) = self.current_engine() else {
            return Response::not_configured();
        };

        let selection = Selection::normalize(kind, id, unread_only);
        match engine.local_snapshot(&selection, retain_entry_ids) {
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
        let selection = Selection::normalize(kind, id, unread_only);
        match engine.flush_and_snapshot(&selection, retain_entry_ids) {
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

/// Returns a short, non-secret correlation prefix for an account identifier.
/// The full account ID is a SHA-256 hex digest; only the first 8 characters
/// are logged so logs can be correlated without exposing the complete value.
fn account_prefix(account_id: &str) -> String {
    account_id.chars().take(8).collect()
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
    use crate::remote::{
        CategoryDto, EntriesFilter, EntryDto, FeedCountersDto, FeedDto, RemoteError, RemoteInbox,
    };

    struct BlockingRemote {
        entered: std::sync::mpsc::Sender<()>,
        release: Mutex<std::sync::mpsc::Receiver<()>>,
        entries: Vec<EntryDto>,
    }

    impl RemoteInbox for BlockingRemote {
        fn fetch_complete_selection(
            &self,
            _filter: &EntriesFilter,
        ) -> Result<(Vec<EntryDto>, i64), RemoteError> {
            Ok((self.entries.clone(), self.entries.len() as i64))
        }

        fn categories(&self) -> Result<Vec<CategoryDto>, RemoteError> {
            Ok(Vec::new())
        }

        fn feeds(&self) -> Result<Vec<FeedDto>, RemoteError> {
            Ok(Vec::new())
        }

        fn unread_counters(&self) -> Result<FeedCountersDto, RemoteError> {
            self.entered.send(()).unwrap();
            locked(&self.release).recv().unwrap();
            Ok(FeedCountersDto {
                unreads: std::collections::HashMap::new(),
            })
        }

        fn starred_total(&self) -> Result<i64, RemoteError> {
            Ok(0)
        }

        fn icon_data_url(&self, _feed_id: i64) -> Result<Option<String>, RemoteError> {
            Ok(None)
        }

        fn set_read_batch(&self, _entry_ids: &[i64], _read: bool) -> Result<(), RemoteError> {
            Ok(())
        }

        fn entry_starred(&self, _entry_id: i64) -> Result<bool, RemoteError> {
            Ok(false)
        }

        fn toggle_starred(&self, _entry_id: i64) -> Result<(), RemoteError> {
            Ok(())
        }
    }

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

    #[test]
    fn configure_during_blocked_refresh_keeps_work_account_bound() {
        let directory = test_directory();
        let database_path = directory.path().join("inbox.sqlite3");
        let runtime = std::sync::Arc::new(AppRuntime::with_database_path(database_path.clone()));
        let account_a = account_id("https://a.example", "a");
        let store = Store::open(&database_path).unwrap();
        store
            .ensure_account(&account_a, "https://a.example")
            .unwrap();
        let (entered_tx, entered_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let engine = SyncService::new(
            store,
            Box::new(BlockingRemote {
                entered: entered_tx,
                release: Mutex::new(release_rx),
                entries: vec![EntryDto {
                    id: 7,
                    feed_id: 20,
                    title: "Account A entry".to_string(),
                    url: "https://a.example/7".to_string(),
                    comments_url: String::new(),
                    status: "unread".to_string(),
                    starred: false,
                    published_at: "2026-08-23T10:00:00Z".to_string(),
                    content: String::new(),
                    enclosures: Vec::new(),
                    feed: None,
                }],
            }),
            account_a.clone(),
            false,
        );
        *locked(&runtime.store) = Some(engine.shared_store());
        *locked(&runtime.session) = Some(Session {
            config: Config {
                account_id: account_a.clone(),
                generation: 1,
            },
            engine,
        });

        let refreshing = std::sync::Arc::clone(&runtime);
        let refresh = std::thread::spawn(move || refreshing.refresh("all", 0, true, &[]));
        entered_rx
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();

        assert!(
            runtime
                .configure("https://b.example", "b", false, 2, &[])
                .ok
        );
        let account_b = account_id("https://b.example", "b");
        assert_eq!(
            locked(&runtime.session).as_ref().unwrap().config.account_id,
            account_b
        );
        assert!(runtime.local_snapshot("all", 0, true, &[]).ok);

        release_tx.send(()).unwrap();
        assert!(refresh.join().unwrap().ok);
        assert_eq!(
            locked(&runtime.session).as_ref().unwrap().config.account_id,
            account_b
        );

        let database = rusqlite::Connection::open(database_path).unwrap();
        let account_count: i64 = database
            .query_row("SELECT COUNT(*) FROM accounts", [], |row| row.get(0))
            .unwrap();
        assert_eq!(account_count, 2);
        let entry_accounts: Vec<String> = database
            .prepare("SELECT account_id FROM entries WHERE id=7 ORDER BY account_id")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(entry_accounts, vec![account_a]);
    }

    #[test]
    fn same_account_reconfigure_retains_serialization_service() {
        let directory = test_directory();
        let runtime = AppRuntime::with_database_path(directory.path().join("inbox.sqlite3"));
        assert!(
            runtime
                .configure("https://a.example", "key", false, 1, &[])
                .ok
        );
        let first = runtime.current_engine().unwrap();

        assert!(
            runtime
                .configure("https://a.example/", " key ", true, 2, &[])
                .ok
        );
        let second = runtime.current_engine().unwrap();

        assert!(Arc::ptr_eq(&first, &second));
    }

    #[test]
    fn account_round_trip_reuses_a_retained_service() {
        let directory = test_directory();
        let runtime = AppRuntime::with_database_path(directory.path().join("inbox.sqlite3"));
        assert!(
            runtime
                .configure("https://a.example", "a", false, 1, &[])
                .ok
        );
        let first_a = runtime.current_engine().unwrap();
        assert!(
            runtime
                .configure("https://b.example", "b", false, 2, &[])
                .ok
        );
        assert!(runtime.configure("https://a.example", "a", true, 3, &[]).ok);
        let second_a = runtime.current_engine().unwrap();

        assert!(Arc::ptr_eq(&first_a, &second_a));
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

    #[test]
    fn account_prefix_truncates_to_eight_chars() {
        assert_eq!(account_prefix("abcdef123456"), "abcdef12");
        assert_eq!(account_prefix("short"), "short");
        assert!(account_prefix("").is_empty());
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
