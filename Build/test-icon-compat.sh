#!/usr/bin/env bash
set -euo pipefail

# Phase 10 Go/Rust icon-processing and response-wire differential harness.

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPOSITORY_DIR="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
FIXTURE="${SCRIPT_DIR}/testdata/icon_fixtures.json"
WORK_DIR="$(mktemp -d)"

cleanup() {
  rm -rf "${WORK_DIR}"
}
trap cleanup EXIT

go -C "${REPOSITORY_DIR}/go-core" run -tags compat ./internal/icon-compat \
  "${FIXTURE}" > "${WORK_DIR}/go.json"
cargo run --quiet --manifest-path "${REPOSITORY_DIR}/rust-core/Cargo.toml" \
  --bin icon-compat -- "${FIXTURE}" > "${WORK_DIR}/rust.json"

python3 - "${WORK_DIR}/go.json" "${WORK_DIR}/rust.json" <<'PYEOF'
import base64
import json
import sys

go = json.load(open(sys.argv[1]))
rust = json.load(open(sys.argv[2]))

if len(go) != len(rust):
    raise SystemExit(f"length mismatch: go={len(go)} rust={len(rust)}")

for left, right in zip(go, rust):
    if left["name"] != right["name"]:
        raise SystemExit(f"case mismatch: go={left['name']} rust={right['name']}")
    for variant in ("regular", "dark"):
        if (variant in left) != (variant in right):
            raise SystemExit(f"MISMATCH {left['name']} {variant} presence")
        if variant not in left:
            continue
        if (left[variant]["width"], left[variant]["height"]) != (right[variant]["width"], right[variant]["height"]):
            raise SystemExit(f"MISMATCH {left['name']} {variant} dimensions")
        go_pixels = base64.b64decode(left[variant]["rgba"])
        rust_pixels = base64.b64decode(right[variant]["rgba"])
        maximum_delta = max((abs(a - b) for a, b in zip(go_pixels, rust_pixels)), default=0)
        if len(go_pixels) != len(rust_pixels) or maximum_delta > 2:
            raise SystemExit(
                f"MISMATCH decoded image case={left['name']} variant={variant} "
                f"bytes={len(go_pixels)}/{len(rust_pixels)} max_channel_delta={maximum_delta}"
            )

for implementation, cases in (("go", go), ("rust", rust)):
    for case in cases:
        response = case["response"]
        if set(response) != {"ok", "icon"} or response["ok"] is not True:
            raise SystemExit(f"{implementation} invalid response envelope for {case['name']}: {response}")
        icon = response["icon"]
        expected = {key for key in ("regular", "dark") if key in case}
        if set(icon) != expected:
            raise SystemExit(f"{implementation} invalid icon omission for {case['name']}: {icon}")
        for key, value in icon.items():
            if not isinstance(value, str):
                raise SystemExit(f"{implementation} {case['name']} {key} is not a base64 string")
            try:
                base64.b64decode(value, validate=True)
            except Exception as error:
                raise SystemExit(f"{implementation} {case['name']} {key} invalid base64: {error}")
            if value != case[key]["png"]:
                raise SystemExit(f"{implementation} {case['name']} {key} response bytes differ from processed icon")

print("Icon differential tests passed.")
PYEOF
