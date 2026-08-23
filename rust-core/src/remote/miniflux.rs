//! Blocking Miniflux HTTP client reproducing the Go client's wire behavior.

use std::time::{Duration, Instant};

use crate::remote::RemoteInbox;
use crate::remote::dto::{
    EntriesFilter, EntryDto, EntryResultSetDto, FeedCountersDto, FeedDto, FeedIconDto,
};
use crate::remote::error::RemoteError;

/// Per-request library timeout mirroring the Go client's
/// `defaultTimeout = 80s` context deadline. Operation-level deadlines remain
/// the dispatcher's responsibility (unchanged).
const REQUEST_TIMEOUT: Duration = Duration::from_secs(80);

const USER_AGENT: &str = "Miniflux Client Library";
pub const BROWSE_PAGE_SIZE: i64 = 200;
pub const STATUS_UNREAD: &str = "unread";
pub const STATUS_READ: &str = "read";
pub const FILTER_ONLY_STARRED: &str = "1";

/// Blocking Miniflux API v1 client using the system trust store via
/// native-tls (Security.framework on macOS), matching Go's transport trust.
pub struct MinifluxClient {
    agent: ureq::Agent,
    endpoint: String,
    api_key: String,
    operation_deadline: std::sync::Mutex<Option<std::time::Instant>>,
}

impl MinifluxClient {
    /// Creates a client. Trailing slashes and a terminal `/v1` are stripped,
    /// matching the upstream Go client before it appends API paths.
    pub fn new(endpoint: &str, api_key: &str) -> Result<Self, RemoteError> {
        let trimmed = endpoint.trim_end_matches('/');
        let trimmed = trimmed.strip_suffix("/v1").unwrap_or(trimmed);
        if trimmed.is_empty() {
            return Err(RemoteError::Transport(
                "miniflux: empty endpoint provided".to_string(),
            ));
        }
        let agent = ureq::AgentBuilder::new()
            .timeout(REQUEST_TIMEOUT)
            // ureq counts the terminal response in this limit, so 10 matches
            // Go http.Client's default limit of ten consecutive requests.
            .redirects(10)
            .build();
        Ok(Self {
            agent,
            endpoint: trimmed.to_string(),
            api_key: api_key.to_string(),
            operation_deadline: std::sync::Mutex::new(None),
        })
    }

    fn execute(
        &self,
        method: &str,
        path: &str,
        body: Option<String>,
    ) -> Result<String, RemoteError> {
        let operation_deadline = *self
            .operation_deadline
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        self.execute_with_deadline(method, path, body, operation_deadline)
    }

    fn execute_with_deadline(
        &self,
        method: &str,
        path: &str,
        body: Option<String>,
        operation_deadline: Option<std::time::Instant>,
    ) -> Result<String, RemoteError> {
        log::info!(target: "http", "request started method={method} path={path}");
        let start = Instant::now();
        let mut request = self
            .agent
            .request(method, &format!("{}{path}", self.endpoint))
            .set("User-Agent", USER_AGENT)
            .set("Content-Type", "application/json")
            .set("Accept", "application/json");
        if let Some(deadline) = operation_deadline {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                log::warn!(target: "http", "request aborted before send: context deadline exceeded method={method} path={path}");
                return Err(RemoteError::Transport(
                    "context deadline exceeded".to_string(),
                ));
            }
            request = request.timeout(remaining.min(REQUEST_TIMEOUT));
        }
        if !self.api_key.is_empty() {
            request = request.set("X-Auth-Token", &self.api_key);
        }

        let result = match body {
            Some(payload) => request.send_string(&payload),
            None => request.call(),
        };
        let elapsed = start.elapsed().as_millis();
        let response = result.map_err(|error| {
            let remote_error = match error {
                ureq::Error::Status(_code, response) => map_error_response(response),
                _ if operation_deadline
                    .is_some_and(|deadline| std::time::Instant::now() >= deadline) =>
                {
                    RemoteError::Transport("context deadline exceeded".to_string())
                }
                other => RemoteError::Transport(other.to_string()),
            };
            log::warn!(
                target: "http",
                "request failed method={method} path={path} duration_ms={elapsed} category={} error={}",
                remote_error.category(),
                remote_error.log_safe_summary()
            );
            remote_error
        })?;

        let status = response.status();
        log::info!(
            target: "http",
            "request completed method={method} path={path} status={status} duration_ms={elapsed}"
        );
        let text = response
            .into_string()
            .map_err(|error| RemoteError::Json(format!("body read failed: {error}")))?;
        Ok(text)
    }

    pub fn entries(&self, filter: &EntriesFilter) -> Result<EntryResultSetDto, RemoteError> {
        let path = format!("/v1/entries{}", filter.to_query());
        let body = self.execute("GET", &path, None)?;
        serde_json::from_str(&body).map_err(|error| RemoteError::Json(error.to_string()))
    }

    /// Fetches one entry (`GET /v1/entries/{id}`).
    pub fn entry(&self, entry_id: i64) -> Result<EntryDto, RemoteError> {
        let body = self.execute("GET", &format!("/v1/entries/{entry_id}"), None)?;
        serde_json::from_str(&body).map_err(|error| RemoteError::Json(error.to_string()))
    }

    pub fn categories(&self) -> Result<Vec<crate::remote::dto::CategoryDto>, RemoteError> {
        let body = self.execute("GET", "/v1/categories", None)?;
        serde_json::from_str(&body).map_err(|error| RemoteError::Json(error.to_string()))
    }

    pub fn feeds(&self) -> Result<Vec<FeedDto>, RemoteError> {
        let body = self.execute("GET", "/v1/feeds", None)?;
        serde_json::from_str(&body).map_err(|error| RemoteError::Json(error.to_string()))
    }

    pub fn counters(&self) -> Result<FeedCountersDto, RemoteError> {
        let body = self.execute("GET", "/v1/feeds/counters", None)?;
        serde_json::from_str(&body).map_err(|error| RemoteError::Json(error.to_string()))
    }

    /// Raw icon data URL from `GET /v1/feeds/{id}/icon`. No decoding,
    /// variants, or caching happen here (Phase 9 concerns).
    pub fn icon(&self, feed_id: i64) -> Result<Option<FeedIconDto>, RemoteError> {
        let body = self.execute("GET", &format!("/v1/feeds/{feed_id}/icon"), None)?;
        if body.is_empty() {
            return Ok(None);
        }
        Ok(Some(
            serde_json::from_str(&body).map_err(|error| RemoteError::Json(error.to_string()))?,
        ))
    }

    /// Fully paginated ascending-ID selection fetch. Reproduces
    /// `fetchCompleteSelection`, including its strict stability checks and
    /// German compatibility error strings.
    pub fn fetch_complete_selection(
        &self,
        base: &EntriesFilter,
    ) -> Result<(Vec<EntryDto>, i64), RemoteError> {
        log::info!(target: "http", "fetch_complete_selection started");
        let start = Instant::now();
        let mut entries: Vec<EntryDto> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let mut expected_total: i64 = -1;
        let mut after_entry_id: i64 = 0;
        let mut page_count: u32 = 0;

        loop {
            let mut filter = base.clone();
            filter.after_entry_id = after_entry_id;
            let result = self.entries(&filter)?;
            page_count += 1;
            if expected_total < 0 {
                expected_total = result.total;
                entries.reserve(result.entries.len());
            }

            let page_len = result.entries.len();
            let mut last_id = after_entry_id;
            for entry in result.entries {
                if entry.id <= last_id {
                    return Err(RemoteError::Transport(format!(
                        "unstabile Seitensortierung nach Artikel {last_id}"
                    )));
                }
                if !seen.insert(entry.id) {
                    return Err(RemoteError::Transport(format!(
                        "doppelter Artikel {} in paginierter Antwort",
                        entry.id
                    )));
                }
                last_id = entry.id;
                entries.push(entry);
            }

            log::debug!(
                target: "http",
                "fetch_complete_selection page={page_count} collected={} expected={expected_total}",
                entries.len()
            );

            let collected = entries.len() as i64;
            if collected == expected_total {
                let elapsed = start.elapsed().as_millis();
                log::info!(
                    target: "http",
                    "fetch_complete_selection completed pages={page_count} entries={collected} total={expected_total} duration_ms={elapsed}"
                );
                return Ok((entries, expected_total));
            }
            if collected > expected_total {
                return Err(RemoteError::Transport(format!(
                    "Trefferzahl änderte sich während der Pagination: erwartet {expected_total}, erhalten {collected}"
                )));
            }
            if page_len < BROWSE_PAGE_SIZE as usize || last_id == after_entry_id {
                return Err(RemoteError::Transport(format!(
                    "unvollständige paginierte Antwort: erwartet {expected_total}, erhalten {collected}"
                )));
            }
            after_entry_id = last_id;
        }
    }

    /// Builds the Browse-style base filter used by sync for a normalized
    /// selection (mirrors `Browse`'s filter construction).
    pub fn browse_filter(
        status_unread_only: bool,
        starred: bool,
        category_feed_id: (i64, i64),
    ) -> EntriesFilter {
        let mut filter = EntriesFilter {
            limit: BROWSE_PAGE_SIZE,
            order: Some("id".into()),
            direction: Some("asc".into()),
            statuses: vec![STATUS_READ.into(), STATUS_UNREAD.into()],
            offset: 0,
            ..Default::default()
        };
        if status_unread_only {
            filter.statuses.clear();
            filter.status = Some(STATUS_UNREAD.into());
        }
        if starred {
            filter.starred = Some(FILTER_ONLY_STARRED.into());
        }
        let (category_id, feed_id) = category_feed_id;
        if category_id > 0 {
            filter.category_id = category_id;
        }
        if feed_id > 0 {
            filter.feed_id = feed_id;
        }
        filter
    }

    /// Starred total via a `limit=1` query, as Go does outside starred
    /// selections.
    pub fn starred_total(&self) -> Result<i64, RemoteError> {
        let filter = EntriesFilter {
            starred: Some(FILTER_ONLY_STARRED.into()),
            statuses: vec![STATUS_READ.into(), STATUS_UNREAD.into()],
            limit: 1,
            offset: 0,
            ..Default::default()
        };
        Ok(self.entries(&filter)?.total)
    }
}

fn map_error_response(response: ureq::Response) -> RemoteError {
    let status = response.status();
    let body_text = response.into_string().ok();
    match status {
        401 => RemoteError::NotAuthorized,
        403 => RemoteError::Forbidden,
        404 => RemoteError::NotFound,
        400 => {
            let message = body_text
                .as_deref()
                .and_then(|text| serde_json::from_str::<serde_json::Value>(text).ok())
                .and_then(|value| {
                    value
                        .get("error_message")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_string)
                });
            RemoteError::BadRequest(message)
        }
        500 => {
            let message = body_text
                .as_deref()
                .and_then(|text| serde_json::from_str::<serde_json::Value>(text).ok())
                .and_then(|value| {
                    value
                        .get("error_message")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_string)
                });
            RemoteError::ServerError(message)
        }
        _ => RemoteError::Status(status),
    }
}

impl RemoteInbox for MinifluxClient {
    fn set_operation_deadline(&self, deadline: Option<std::time::Instant>) {
        *self
            .operation_deadline
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = deadline;
    }

    fn fetch_complete_selection(
        &self,
        filter: &EntriesFilter,
    ) -> Result<(Vec<EntryDto>, i64), RemoteError> {
        MinifluxClient::fetch_complete_selection(self, filter)
    }

    fn categories(&self) -> Result<Vec<crate::remote::dto::CategoryDto>, RemoteError> {
        MinifluxClient::categories(self)
    }

    fn feeds(&self) -> Result<Vec<FeedDto>, RemoteError> {
        MinifluxClient::feeds(self)
    }

    fn unread_counters(&self) -> Result<FeedCountersDto, RemoteError> {
        MinifluxClient::counters(self)
    }

    fn starred_total(&self) -> Result<i64, RemoteError> {
        MinifluxClient::starred_total(self)
    }

    fn icon_data_url(&self, feed_id: i64) -> Result<Option<String>, RemoteError> {
        Ok(MinifluxClient::icon(self, feed_id)?.map(|icon| icon.data))
    }

    fn icon_data_url_with_deadline(
        &self,
        feed_id: i64,
        deadline: std::time::Instant,
    ) -> Result<Option<String>, RemoteError> {
        let body = self.execute_with_deadline(
            "GET",
            &format!("/v1/feeds/{feed_id}/icon"),
            None,
            Some(deadline),
        )?;
        if body.is_empty() {
            return Ok(None);
        }
        let icon: FeedIconDto =
            serde_json::from_str(&body).map_err(|error| RemoteError::Json(error.to_string()))?;
        Ok(Some(icon.data))
    }

    fn set_read_batch(&self, entry_ids: &[i64], read: bool) -> Result<(), RemoteError> {
        let status = if read { STATUS_READ } else { STATUS_UNREAD };
        let ids: Vec<String> = entry_ids.iter().map(ToString::to_string).collect();
        let payload = format!(r#"{{"entry_ids":[{}],"status":"{status}"}}"#, ids.join(","));
        // 204 responses decode to an empty string; both are success.
        self.execute("PUT", "/v1/entries", Some(payload))
            .map(|_| ())
    }

    fn entry_starred(&self, entry_id: i64) -> Result<bool, RemoteError> {
        Ok(self.entry(entry_id)?.starred)
    }

    fn toggle_starred(&self, entry_id: i64) -> Result<(), RemoteError> {
        self.execute("PUT", &format!("/v1/entries/{entry_id}/star"), None)
            .map(|_| ())
    }
}
