#!/usr/bin/env bash
set -euo pipefail

# Phase 9.2 Go/Rust localization differential harness.

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPOSITORY_DIR="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
FIXTURE="${SCRIPT_DIR}/testdata/localization_fixtures.json"
WORK_DIR="$(mktemp -d)"

cleanup() {
  rm -rf "${WORK_DIR}"
}
trap cleanup EXIT

go -C "${REPOSITORY_DIR}/go-core" run -tags compat ./internal/localization-compat \
  "${FIXTURE}" > "${WORK_DIR}/go.json"
cargo run --quiet --manifest-path "${REPOSITORY_DIR}/rust-core/Cargo.toml" \
  --bin localization-compat -- "${FIXTURE}" > "${WORK_DIR}/rust.json"

if python3 - "${WORK_DIR}/go.json" "${WORK_DIR}/rust.json" <<'PYEOF'
import json, sys
go = json.load(open(sys.argv[1]))
rust = json.load(open(sys.argv[2]))
if go != rust:
    for idx, (g, r) in enumerate(zip(go, rust)):
        if g != r:
            print(f"MISMATCH case={g.get('name')}")
            print("  go:  ", json.dumps(g))
            print("  rust:", json.dumps(r))
            break
    else:
        print(f"length mismatch: go={len(go)} rust={len(rust)}")
    sys.exit(1)
PYEOF
then
  echo "Localization differential tests passed."
else
  echo "Localization differential tests failed."; exit 1
fi
