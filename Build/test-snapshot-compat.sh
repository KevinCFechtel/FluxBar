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
  local newest_first="${7:-false}"
  go_helper snapshot "${database}" "${kind}" "${id}" "${unread_only}" "${retain}" "${newest_first}" \
    > "${WORK_DIR}/go.json"
  rust_snapshot "${database}" "${kind}" "${id}" "${unread_only}" "${retain}" "${newest_first}" \
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
COUNT_0="${WORK_DIR}/count-0.sqlite3"
COUNT_1="${WORK_DIR}/count-1.sqlite3"
COUNT_199="${WORK_DIR}/count-199.sqlite3"
COUNT_200="${WORK_DIR}/count-200.sqlite3"
COUNT_201="${WORK_DIR}/count-201.sqlite3"
COUNT_205="${WORK_DIR}/count-205.sqlite3"
go_helper fixture-basic  "${BASIC}"
go_helper fixture-large  "${LARGE}"
go_helper fixture-multi  "${MULTI}"
go_helper fixture-empty  "${EMPTY}"
go_helper fixture-count  "${COUNT_0}" 0
go_helper fixture-count  "${COUNT_1}" 1
go_helper fixture-count  "${COUNT_199}" 199
go_helper fixture-count  "${COUNT_200}" 200
go_helper fixture-count  "${COUNT_201}" 201
go_helper fixture-count  "${COUNT_205}" 205

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
compare_case "equal-timestamps/oldest-first" "${BASIC}" "all"     0 false "" false
compare_case "equal-timestamps/newest-first" "${BASIC}" "all"     0 false "" true

echo "unreadOnly combinations"
compare_case "all+unreadOnly"              "${BASIC}" "all"      0 true ""
compare_case "starred+unreadOnly"          "${BASIC}" "starred"  0 true ""
compare_case "feed+unreadOnly"             "${BASIC}" "feed"   100 true ""

echo "Presentation retention"
# Entries 2, 5, and 205 are locally read. Entry 205 sorts ahead of the normal
# newest-first unread page, while entries 2 and 5 exercise multiple retention.
compare_case "retain-read-entry"                     "${BASIC}" "unread" 0 true "2"
compare_case "large/retained-read-outside-page"      "${LARGE}" "unread" 0 true "205" true
compare_case "large/multiple-retained-read-ids"      "${LARGE}" "unread" 0 true "2,5"
compare_case "large/missing-retained-id"             "${LARGE}" "unread" 0 true "99999"

echo "Account isolation"
compare_case "multi-account/all"           "${MULTI}" "all"      0 false ""
compare_case "multi-account/category"      "${MULTI}" "category" 10 false ""
compare_case "multi-account/wrong-account-retained-id" "${MULTI}" "unread" 0 true "900"

echo "0/1/199/200/201/>200 presentation boundaries"
compare_case "boundary/exact-0"            "${COUNT_0}"   "all" 0 false ""
compare_case "boundary/exact-1"            "${COUNT_1}"   "all" 0 false ""
compare_case "boundary/exact-199"          "${COUNT_199}" "all" 0 false ""
compare_case "boundary/exact-200"          "${COUNT_200}" "all" 0 false ""
compare_case "boundary/exact-201"          "${COUNT_201}" "all" 0 false ""
compare_case "boundary/above-200-exact-205" "${COUNT_205}" "all" 0 false "" true
compare_case "large/exact-205-mixed-status-rows" "${LARGE}" "all" 0 false ""

echo "Unknown persisted EntryStatus"
compare_case "unknown-status-row"          "${BASIC}" "all"    0 false ""

echo "Snapshot differential tests passed."
