#!/usr/bin/env bash
set -euo pipefail

# Phase 8 Go/Rust differential sync and mutation state-machine harness.

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPOSITORY_DIR="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
WORK_DIR="$(mktemp -d)"
PORT=""
BASE=""
SERVER_PID=""

cleanup() {
  [[ -n "${SERVER_PID}" ]] && kill "${SERVER_PID}" 2>/dev/null || true
  rm -rf "${WORK_DIR}"
}
trap cleanup EXIT

launch_server() {
  local mode="$1" log="$2"
  local port_file="${WORK_DIR}/server.port"
  : > "${log}"
  rm -f "${port_file}"
  python3 "${SCRIPT_DIR}/testdata/fake_sync_miniflux.py" 0 "${mode}" "${log}" "${port_file}" &
  SERVER_PID=$!
  for _ in $(seq 1 50); do
    if ! kill -0 "${SERVER_PID}" 2>/dev/null; then
      wait "${SERVER_PID}" || true
      echo "fake sync server exited during startup" >&2
      exit 1
    fi
    if [[ -s "${port_file}" ]]; then
      PORT="$(<"${port_file}")"
      BASE="http://127.0.0.1:${PORT}"
      if nc -z 127.0.0.1 "${PORT}" 2>/dev/null; then return; fi
    fi
    sleep 0.1
  done
  echo "fake sync server failed to start" >&2
  exit 1
}

stop_server() {
  kill "${SERVER_PID}" 2>/dev/null || true
  wait "${SERVER_PID}" 2>/dev/null || true
  SERVER_PID=""
}

dump_database() {
  local database="$1" output="$2"
  python3 - "${database}" > "${output}" <<'PYEOF'
import json, sqlite3, sys
db = sqlite3.connect(sys.argv[1])
queries = {
    "accounts": "SELECT remote_starred_total FROM accounts ORDER BY id",
    "categories": "SELECT id,title FROM categories ORDER BY id",
    "feeds": "SELECT id,category_id,title,remote_unread_count FROM feeds ORDER BY id",
    "totals": "SELECT kind,selection_id,unread_only,total FROM selection_totals ORDER BY kind,selection_id,unread_only",
    "entries": "SELECT id,title,preview,image_url,remote_status,remote_starred,status,starred FROM entries ORDER BY id",
    "pending": "SELECT entry_id,field,desired,revision FROM pending_mutations ORDER BY updated_at,entry_id,field",
    "undo_batches": "SELECT COUNT(*) FROM undo_batches",
    "undo_items": "SELECT entry_id,prior_read FROM undo_items ORDER BY entry_id",
}
print(json.dumps({name: db.execute(query).fetchall() for name, query in queries.items()}, sort_keys=True))
PYEOF
}

run_implementation() {
  local implementation="$1" scenario="$2" mode="$3"
  local database="${WORK_DIR}/${implementation}-${scenario}.sqlite3"
  local response="${WORK_DIR}/${implementation}-${scenario}.json"
  local requests="${WORK_DIR}/${implementation}-${scenario}.requests"
  launch_server "${mode}" "${requests}"
  if [[ "${implementation}" == "go" ]]; then
    go -C "${REPOSITORY_DIR}/go-core" run ./cmd/sqlite-compat \
      sync-probe "${database}" "${BASE}" "${scenario}" > "${response}"
  else
    cargo run --quiet --manifest-path "${REPOSITORY_DIR}/rust-core/Cargo.toml" \
      --bin sqlite-compat -- sync-probe "${database}" "${BASE}" "${scenario}" > "${response}"
  fi
  stop_server
  dump_database "${database}" "${WORK_DIR}/${implementation}-${scenario}.db.json"
}

compare_case() {
  local scenario="$1" mode="$2"
  run_implementation go "${scenario}" "${mode}"
  run_implementation rust "${scenario}" "${mode}"
  python3 - "${WORK_DIR}" "${scenario}" <<'PYEOF'
import json, pathlib, sys
root, scenario = pathlib.Path(sys.argv[1]), sys.argv[2]
def load(prefix, suffix):
    return json.loads((root / f"{prefix}-{scenario}.{suffix}").read_text())
go_response, rust_response = load("go", "json"), load("rust", "json")
go_response.pop("error", None) if not go_response.get("error") else None
rust_response.pop("error", None) if not rust_response.get("error") else None
if scenario == "pagination-malformed":
    prefix = "Miniflux-Einträge laden:"
    if not all(response.get("error", "").startswith(prefix) for response in (go_response, rust_response)):
        raise SystemExit(f"malformed pagination error mismatch\ngo={go_response.get('error')}\nrust={rust_response.get('error')}")
    go_response["error"] = rust_response["error"] = prefix
for response in (go_response, rust_response):
    for entry in response.get("snapshot", {}).get("entries", []):
        entry.pop("icon", None); entry.pop("darkIcon", None)
if go_response != rust_response:
    raise SystemExit(f"response mismatch\ngo={go_response}\nrust={rust_response}")
go_db, rust_db = load("go", "db.json"), load("rust", "db.json")
if go_db != rust_db:
    raise SystemExit(f"database mismatch\ngo={go_db}\nrust={rust_db}")
go_requests = [json.loads(line) for line in (root / f"go-{scenario}.requests").read_text().splitlines()]
rust_requests = [json.loads(line) for line in (root / f"rust-{scenario}.requests").read_text().splitlines()]
if go_requests != rust_requests:
    raise SystemExit(f"request mismatch\ngo={go_requests}\nrust={rust_requests}")
PYEOF
  echo "PASS  ${scenario}"
}

compare_case initial happy
compare_case incremental incremental
compare_case incomplete incomplete
compare_case pagination-duplicate pagination-duplicate
compare_case pagination-reordered pagination-reordered
compare_case pagination-growing-total pagination-growing-total
compare_case pagination-shrinking-total pagination-shrinking-total
compare_case pagination-malformed pagination-malformed
compare_case refresh-5xx refresh-5xx
compare_case refresh-auth refresh-auth
compare_case read happy
compare_case read-reversal happy
compare_case read-cycle happy
compare_case read-identical happy
compare_case star happy
compare_case star-reversal happy
compare_case star-cycle happy
compare_case star-identical happy
compare_case pending-stale fail-first
compare_case restart-pending happy
compare_case undo-before-flush happy
compare_case undo-after-flush happy
compare_case discard-undo happy
compare_case full-failure fail-first
compare_case partial-failure fail-second
compare_case mixed-middle-retry fail-second

echo "Sync state-machine differential tests passed."
