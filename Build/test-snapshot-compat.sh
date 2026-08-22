#!/usr/bin/env bash
set -euo pipefail

# Differential local-snapshot compatibility harness (Phase 6).
#
# Builds deterministic temporary fixture databases with the Go helper, then
# feeds identical selection/retention inputs to the Go and Rust snapshot
# implementations and compares the resulting JSON semantically.

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPOSITORY_DIR="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
WORK_DIR="$(mktemp -d)"

cleanup() {
  rm -rf "${WORK_DIR}"
}
trap cleanup EXIT

GO_CORE="${REPOSITORY_DIR}/go-core"
RUST_MANIFEST="${REPOSITORY_DIR}/rust-core/Cargo.toml"

go_helper() {
  go -C "${GO_CORE}" run ./cmd/sqlite-compat "$@"
}

rust_snapshot() {
  cargo run --quiet --manifest-path "${RUST_MANIFEST}" \
    --bin sqlite-compat -- snapshot "$@"
}

compare_case() {
  local label="$1" database="$2" kind="$3" id="$4" unread_only="$5" retain="$6"
  go_helper snapshot "${database}" "${kind}" "${id}" "${unread_only}" "${retain}" \
    > "${WORK_DIR}/go.json"
  rust_snapshot "${database}" "${kind}" "${id}" "${unread_only}" "${retain}" \
    > "${WORK_DIR}/rust.json"
  if python3 - "${WORK_DIR}/go.json" "${WORK_DIR}/rust.json" <<'PYEOF'
import json, sys
go = json.load(open(sys.argv[1]))
rust = json.load(open(sys.argv[2]))
if go != rust:
    for key in sorted(set(go) | set(rust)):
        if go.get(key) != rust.get(key):
            print(f"MISMATCH key={key}")
            print("  go:  ", json.dumps(go.get(key))[:400])
            print("  rust:", json.dumps(rust.get(key))[:400])
    sys.exit(1)
PYEOF
  then
    echo "PASS  ${label}"
  else
    echo "FAIL  ${label}"
    exit 1
  fi
}

echo "Building fixtures"
BASIC="${WORK_DIR}/basic.sqlite3"
LARGE="${WORK_DIR}/large.sqlite3"
MULTI="${WORK_DIR}/multi.sqlite3"
EMPTY="${WORK_DIR}/empty.sqlite3"
go_helper fixture-basic  "${BASIC}"
go_helper fixture-large  "${LARGE}"
go_helper fixture-multi  "${MULTI}"
go_helper fixture-empty  "${EMPTY}"

echo "Selection kinds on basic fixture"
compare_case "empty-database/all"          "${EMPTY}" "all"       0 false ""
compare_case "all"                         "${BASIC}" "all"       0 false ""
compare_case "unread-kind"                 "${BASIC}" "unread"    0 false ""
compare_case "starred"                     "${BASIC}" "starred"   0 false ""
compare_case "category"                    "${BASIC}" "category" 10 true  ""
compare_case "feed"                        "${BASIC}" "feed"    100 false ""
compare_case "missing-operation-selection" "${BASIC}" ""          0 false ""
compare_case "unknown-kind-fallback"       "${BASIC}" "bogus"     4 false ""
compare_case "category-zero-id-fallback"   "${BASIC}" "category"  0 false ""

echo "unreadOnly combinations"
compare_case "all+unreadOnly"              "${BASIC}" "all"      0 true ""
compare_case "starred+unreadOnly"          "${BASIC}" "starred"  0 true ""
compare_case "feed+unreadOnly"             "${BASIC}" "feed"   100 true ""

echo "Presentation retention"
# Entry 2 is locally read; retaining it must keep it in the unread view.
compare_case "retain-read-entry"           "${BASIC}" "unread" 0 true "2,3"

echo "Account isolation"
compare_case "multi-account/all"           "${MULTI}" "all"      0 false ""
compare_case "multi-account/category"      "${MULTI}" "category" 10 false ""

echo "200-entry presentation limit"
compare_case "large/below-limit"           "${LARGE}" "unread" 0 false ""
compare_case "large/exact-201-rows"        "${LARGE}" "all"    0 false ""
compare_case "large/retained-outside-page" "${LARGE}" "unread" 0 true "204"

echo "Unknown persisted EntryStatus"
compare_case "unknown-status-row"          "${BASIC}" "all"    0 false ""

echo "Snapshot differential tests passed."
