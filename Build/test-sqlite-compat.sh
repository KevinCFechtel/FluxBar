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

echo "Compare normalized schema definitions"
sqlite3 -separator '|' "${GO_DATABASE}" \
  "SELECT type,name,tbl_name,replace(replace(sql,char(10),' '),char(9),' ') FROM sqlite_master WHERE name NOT LIKE 'sqlite_autoindex_%' ORDER BY type,name" \
  > "${WORK_DIR}/go-schema.txt"
sqlite3 -separator '|' "${RUST_DATABASE}" \
  "SELECT type,name,tbl_name,replace(replace(sql,char(10),' '),char(9),' ') FROM sqlite_master WHERE name NOT LIKE 'sqlite_autoindex_%' ORDER BY type,name" \
  > "${WORK_DIR}/rust-schema.txt"
diff -u "${WORK_DIR}/go-schema.txt" "${WORK_DIR}/rust-schema.txt"

echo "SQLite interoperability passed: Go -> Rust and Rust -> Go"
