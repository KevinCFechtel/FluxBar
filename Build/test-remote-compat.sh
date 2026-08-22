#!/usr/bin/env bash
set -euo pipefail

# Differential Miniflux remote-adapter harness (Phase 7).
#
# Starts one deterministic fake Miniflux server on a fixed port, points the
# production Go Browse path and the Rust remote adapter at it, and compares
# the resulting snapshot JSON semantically. Also verifies both sides reject
# a truncated paginated sequence (second server mode).

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPOSITORY_DIR="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
WORK_DIR="$(mktemp -d)"
KEY="diff-fake-key"
PORT=18477
BASE="http://127.0.0.1:${PORT}"

SERVER_PID=""
cleanup() {
  [[ -n "${SERVER_PID}" ]] && kill "${SERVER_PID}" 2>/dev/null || true
  rm -rf "${WORK_DIR}"
}
trap cleanup EXIT


# Replace __DIR__ placeholder at runtime.
run_case() {
  local label="$1" kind="$2" id="$3" unread_only="$4"
  go -C "${REPOSITORY_DIR}/go-core" run ./cmd/sqlite-compat \
      remote-browse "${BASE}" "${KEY}" "${kind}" "${id}" "${unread_only}" \
      > "${WORK_DIR}/go.json"
  cargo run --quiet --manifest-path "${REPOSITORY_DIR}/rust-core/Cargo.toml" \
      --bin sqlite-compat -- remote-browse "${BASE}" "${KEY}" "${kind}" "${id}" "${unread_only}" \
      > "${WORK_DIR}/rust.json"
  if python3 - "${WORK_DIR}/go.json" "${WORK_DIR}/rust.json" <<'PYEOF'
import json, sys
go = json.load(open(sys.argv[1]))
rust = json.load(open(sys.argv[2]))
if go != rust:
    for key in sorted(set(go) | set(rust)):
        if go.get(key) != rust.get(key):
            print(f"MISMATCH key={key}")
            print("  go:  ", json.dumps(go.get(key))[:300])
            print("  rust:", json.dumps(rust.get(key))[:300])
    sys.exit(1)
PYEOF
  then
    echo "PASS  ${label}"
  else
    echo "FAIL  ${label}"; exit 1
  fi
}

echo "Starting fake Miniflux server on ${PORT}"
cat > "${WORK_DIR}/fake_server.py" <<'PY'
import importlib.util, json, re, sys
port = int(sys.argv[1]); mode = sys.argv[2]
spec = importlib.util.spec_from_file_location("fake", sys.argv[3])
mod = importlib.util.module_from_spec(spec)
spec.loader.exec_module(mod)
from http.server import HTTPServer

if mode == "truncate":
    def paged(ids, query):
        m = re.search(r"after_entry_id=(\d+)", query)
        after = int(m.group(1)) if m else 0
        page = [e for e in ids if e > after][:200]
        if after == 0 and len(page) == 200:
            page = page[:-1]  # short page while total claims more
        return json.dumps({"total": len(ids), "entries": [
            mod.entry(e, mod.STATES[e], e in mod.STARRED) for e in page]}).encode()
    mod.paged = paged

HTTPServer(("127.0.0.1", port), mod.Handler).serve_forever()
PY

launch() {
  python3 "${WORK_DIR}/fake_server.py" "${PORT}" "$1" "${SCRIPT_DIR}/testdata/fake_miniflux.py" &
  SERVER_PID=$!
  for _ in $(seq 1 50); do
    if nc -z 127.0.0.1 "${PORT}" 2>/dev/null; then return; fi
    sleep 0.1
  done
  echo "server failed to start"; exit 1
}

stop() {
  kill "${SERVER_PID}" 2>/dev/null || true; wait "${SERVER_PID}" 2>/dev/null || true; SERVER_PID=""
}

launch happy

echo "Selection parity against fake server"
run_case "browse/all"          all      0 false
run_case "browse/unread-kind"  unread   0 false
run_case "browse/starred"      starred  0 false
run_case "browse/category"     category 2 true
run_case "browse/feed"         feed     3 false

stop

echo "Truncated pagination must fail on both sides"
launch truncate

GO_STATUS=0
go -C "${REPOSITORY_DIR}/go-core" run ./cmd/sqlite-compat \
    remote-browse "${BASE}" "${KEY}" all 0 false \
    > "${WORK_DIR}/go-truncated.json" 2>"${WORK_DIR}/go-truncated.err" || GO_STATUS=$?
RUST_STATUS=0
cargo run --quiet --manifest-path "${REPOSITORY_DIR}/rust-core/Cargo.toml" \
    --bin sqlite-compat -- remote-browse "${BASE}" "${KEY}" all 0 false \
    > "${WORK_DIR}/rust-truncated.json" 2>"${WORK_DIR}/rust-truncated.err" || RUST_STATUS=$?

if [[ "${GO_STATUS}" -ne 0 && "${RUST_STATUS}" -ne 0 ]]; then
  echo "PASS  truncated-pagination/both-fail"
else
  echo "FAIL  truncated-pagination (go=${GO_STATUS}, rust=${RUST_STATUS})"; exit 1
fi

echo "Remote differential tests passed."
