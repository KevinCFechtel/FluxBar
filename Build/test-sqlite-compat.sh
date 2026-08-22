#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPOSITORY_DIR="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
WORK_DIR="$(mktemp -d)"

cleanup() {
  rm -rf "${WORK_DIR}"
}
trap cleanup EXIT

GO_DATABASE="${WORK_DIR}/go.sqlite3"
RUST_DATABASE="${WORK_DIR}/rust.sqlite3"
GO_MUTATION_DATABASE="${WORK_DIR}/go-mutations.sqlite3"
RUST_MUTATION_DATABASE="${WORK_DIR}/rust-mutations.sqlite3"

echo "Go creates and writes database"
go -C "${REPOSITORY_DIR}/go-core" run ./cmd/sqlite-compat create "${GO_DATABASE}"

echo "Rust opens and reads Go database"
cargo run --quiet --manifest-path "${REPOSITORY_DIR}/rust-core/Cargo.toml" \
  --bin sqlite-compat -- read-go "${GO_DATABASE}"

echo "Rust creates and writes database"
cargo run --quiet --manifest-path "${REPOSITORY_DIR}/rust-core/Cargo.toml" \
  --bin sqlite-compat -- create "${RUST_DATABASE}"

echo "Go opens and reads Rust database"
go -C "${REPOSITORY_DIR}/go-core" run ./cmd/sqlite-compat read-rust "${RUST_DATABASE}"

echo "Go creates pending and Undo state; Rust continues it"
go -C "${REPOSITORY_DIR}/go-core" run ./cmd/sqlite-compat create-mutations "${GO_MUTATION_DATABASE}"
cargo run --quiet --manifest-path "${REPOSITORY_DIR}/rust-core/Cargo.toml" \
  --bin sqlite-compat -- continue-go-mutations "${GO_MUTATION_DATABASE}"

echo "Rust creates pending and Undo state; Go continues it"
cargo run --quiet --manifest-path "${REPOSITORY_DIR}/rust-core/Cargo.toml" \
  --bin sqlite-compat -- create-mutations "${RUST_MUTATION_DATABASE}"
go -C "${REPOSITORY_DIR}/go-core" run ./cmd/sqlite-compat continue-rust-mutations "${RUST_MUTATION_DATABASE}"

normalize_state() {
  local database="$1" output="$2"
  python3 - "${database}" > "${output}" <<'PYEOF'
import json, sqlite3, sys

db = sqlite3.connect(sys.argv[1])
accounts = {row[0]: row[1] for row in db.execute("SELECT id,server FROM accounts")}

state = {
    "accounts": sorted([[server, starred] for _, server, starred in db.execute(
        "SELECT id,server,remote_starred_total FROM accounts")]),
    "categories": sorted([[accounts[account], id, title] for account, id, title in db.execute(
        "SELECT account_id,id,title FROM categories")]),
    "feeds": sorted([[accounts[account], id, category, title, unread] for account, id, category, title, unread in db.execute(
        "SELECT account_id,id,category_id,title,remote_unread_count FROM feeds")]),
    "totals": sorted([[accounts[account], kind, id, unread, total] for account, kind, id, unread, total in db.execute(
        "SELECT account_id,kind,selection_id,unread_only,total FROM selection_totals")]),
    "entries": sorted([[accounts[row[0]], *row[1:]] for row in db.execute(
        "SELECT account_id,id,title,url,comments_url,feed_id,feed_name,category_id,published_at,preview,image_url,remote_status,remote_starred,status,starred FROM entries")]),
    "pending": sorted([[accounts[account], entry, field, desired, revision] for account, entry, field, desired, revision in db.execute(
        "SELECT account_id,entry_id,field,desired,revision FROM pending_mutations")]),
    "undo": sorted([[accounts[account], entry, prior] for account, entry, prior in db.execute(
        "SELECT account_id,entry_id,prior_read FROM undo_items")]),
}
print(json.dumps(state, sort_keys=True, separators=(",", ":")))
PYEOF
}

normalize_state "${GO_MUTATION_DATABASE}" "${WORK_DIR}/go-rust-state.json"
normalize_state "${RUST_MUTATION_DATABASE}" "${WORK_DIR}/rust-go-state.json"
diff -u "${WORK_DIR}/go-rust-state.json" "${WORK_DIR}/rust-go-state.json"

echo "Compare normalized schema definitions"
sqlite3 -separator '|' "${GO_DATABASE}" \
  "SELECT type,name,tbl_name,replace(replace(sql,char(10),' '),char(9),' ') FROM sqlite_master WHERE name NOT LIKE 'sqlite_autoindex_%' ORDER BY type,name" \
  > "${WORK_DIR}/go-schema.txt"
sqlite3 -separator '|' "${RUST_DATABASE}" \
  "SELECT type,name,tbl_name,replace(replace(sql,char(10),' '),char(9),' ') FROM sqlite_master WHERE name NOT LIKE 'sqlite_autoindex_%' ORDER BY type,name" \
  > "${WORK_DIR}/rust-schema.txt"
diff -u "${WORK_DIR}/go-schema.txt" "${WORK_DIR}/rust-schema.txt"

echo "SQLite interoperability passed: Go -> Rust and Rust -> Go"
