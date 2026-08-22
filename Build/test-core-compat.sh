#!/usr/bin/env bash
set -euo pipefail

# C ABI and JSON transport differential test for the Go and Rust cores.

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPOSITORY_DIR="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
WORK_DIR="$(mktemp -d)"

cleanup() {
  rm -rf "${WORK_DIR}"
}
trap cleanup EXIT

GO_DIR="${WORK_DIR}/go"
RUST_DIR="${WORK_DIR}/rust"
HOST_ARCH="$(uname -m)"

case "${HOST_ARCH}" in
  arm64|x86_64) ;;
  *)
    echo "Unsupported macOS host architecture: ${HOST_ARCH}" >&2
    exit 1
    ;;
esac

mkdir -p "${GO_DIR}" "${RUST_DIR}"

echo "Building Go core..."
"${SCRIPT_DIR}/build-go-core.sh" "${GO_DIR}" "${HOST_ARCH}"

echo "Building Rust core..."
"${SCRIPT_DIR}/build-rust-core.sh" "${RUST_DIR}" "${HOST_ARCH}"

cat > "${WORK_DIR}/caller.c" <<'EOF'
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

extern char* FluxCoreRequest(char* request);
extern void FluxCoreFree(char* value);

static int valid_utf8(const unsigned char* s) {
    while (*s != 0) {
        unsigned int remaining;
        unsigned char c = *s++;
        if (c <= 0x7f) continue;
        if (c >= 0xc2 && c <= 0xdf) remaining = 1;
        else if (c >= 0xe0 && c <= 0xef) remaining = 2;
        else if (c >= 0xf0 && c <= 0xf4) remaining = 3;
        else return 0;
        for (unsigned int i = 0; i < remaining; i++) {
            if ((*s & 0xc0) != 0x80) return 0;
            if (i == 0 && c == 0xe0 && *s < 0xa0) return 0;
            if (i == 0 && c == 0xed && *s >= 0xa0) return 0;
            if (i == 0 && c == 0xf0 && *s < 0x90) return 0;
            if (i == 0 && c == 0xf4 && *s >= 0x90) return 0;
            s++;
        }
    }
    return 1;
}

static void call(const char* label, const char* request) {
    char* response = FluxCoreRequest((char*)request);
    if (response == NULL) {
        fprintf(stderr, "%s returned NULL\n", label);
        exit(1);
    }
    if (!valid_utf8((const unsigned char*)response)) {
        fprintf(stderr, "%s returned invalid UTF-8\n", label);
        FluxCoreFree(response);
        exit(1);
    }
    printf("%s\t%s\n", label, response);
    FluxCoreFree(response);
}

int main(void) {
    static char invalid_utf8[] = {(char)0xff, 0};

    FluxCoreFree(NULL);
    call("null", NULL);
    call("empty", "");
    call("malformed", "not-json");
    call("malformed_truncated", "{\"operation\":");
    call("invalid_utf8", invalid_utf8);
    call("missing_operation", "{}");
    call("unknown_operation", "{\"operation\":\"unknown\"}");
    call("root_null", "null");
    call("operation_null", "{\"operation\":null}");
    call("omitted_defaults", "{\"operation\":\"localize\"}");
    call("explicit_null_defaults", "{\"operation\":\"localize\",\"server\":null,\"apiKey\":null,\"newestFirst\":null,\"configurationGeneration\":null,\"locales\":null,\"key\":null,\"fallback\":null,\"oneFallback\":null,\"otherFallback\":null,\"count\":null,\"selection\":null,\"entryID\":null,\"entryIDs\":null,\"retainEntryIDs\":null,\"read\":null,\"mutationSource\":null,\"mutationID\":null,\"currentStarred\":null,\"desiredStarred\":null,\"feedID\":null,\"feedName\":null}");
    call("nested_null_defaults", "{\"operation\":\"local_snapshot\",\"selection\":{\"kind\":null,\"id\":null,\"unreadOnly\":null}}");
    call("case_insensitive_fields", "{\"OPERATION\":\"localize\",\"LOCALES\":[\"de-DE\"],\"KEY\":\"missing\",\"FALLBACK\":\"Groß\"}");
    call("case_duplicate_last_wins", "{\"operation\":\"unknown\",\"OPERATION\":\"localize\",\"fallback\":\"first\",\"FALLBACK\":\"last\"}");
    call("null_array_elements", "{\"OPERATION\":\"localize\",\"LOCALES\":[null],\"FALLBACK\":\"zero\",\"ENTRYIDS\":[null]}");
    call("configure_invalid", "{\"operation\":\"configure\",\"server\":\"not-a-url\",\"apiKey\":\"secret\"}");
    call("refresh_unconfigured", "{\"operation\":\"refresh\"}");
    call("set_read_unconfigured", "{\"operation\":\"set_read\",\"entryID\":1}");
    call("set_starred_unconfigured", "{\"operation\":\"set_starred\",\"entryID\":1}");
    call("undo_read_unconfigured", "{\"operation\":\"undo_read\",\"mutationID\":\"x\"}");
    call("discard_undo_unconfigured", "{\"operation\":\"discard_undo\",\"mutationID\":\"x\"}");
    call("flush_pending_unconfigured", "{\"operation\":\"flush_pending\"}");
    call("feed_icon_unconfigured", "{\"operation\":\"feed_icon\",\"feedID\":1}");
    call("unicode_localization", "{\"operation\":\"localize\",\"locales\":[\"de-DE\"],\"key\":\"missing.unicode\",\"fallback\":\"Grüße 東京 👋\"}");
    call("unicode_catalog", "{\"operation\":\"localize_plural\",\"locales\":[\"de-DE\"],\"key\":\"status.unread_count\",\"oneFallback\":\"fallback\",\"otherFallback\":\"fallback\",\"count\":2}");

    for (int i = 0; i < 512; i++) {
        char* response = FluxCoreRequest("{\"operation\":\"unknown-repeat\"}");
        if (response == NULL || !valid_utf8((const unsigned char*)response)) {
            fprintf(stderr, "repeated allocation %d failed\n", i);
            exit(1);
        }
        FluxCoreFree(response);
    }

    for (int i = 0; i < 2048; i++) {
        char label[32];
        snprintf(label, sizeof(label), "sequential_%04d", i);
        call(label, i % 2 == 0
            ? "{\"operation\":\"unknown-sequential\"}"
            : "{\"operation\":\"localize\",\"key\":\"missing\",\"fallback\":\"naïve café\"}");
    }
    return 0;
}
EOF

cc -arch "${HOST_ARCH}" -mmacosx-version-min=15.0 \
  -o "${WORK_DIR}/caller-go" "${WORK_DIR}/caller.c" "${GO_DIR}/libfluxcore.a" \
  -framework CoreFoundation -framework Security -lresolv
cc -arch "${HOST_ARCH}" -mmacosx-version-min=15.0 \
  -o "${WORK_DIR}/caller-rust" "${WORK_DIR}/caller.c" "${RUST_DIR}/libfluxcore.a" \
  -framework CoreFoundation -framework Security

"${WORK_DIR}/caller-go" > "${WORK_DIR}/go.jsonl"
"${WORK_DIR}/caller-rust" > "${WORK_DIR}/rust.jsonl"

python3 - "${WORK_DIR}/go.jsonl" "${WORK_DIR}/rust.jsonl" <<'PY'
import json
import sys
from pathlib import Path

PARSER_CASES = {"empty", "malformed", "malformed_truncated", "invalid_utf8"}


def load(path):
    try:
        text = Path(path).read_bytes().decode("utf-8", errors="strict")
    except UnicodeDecodeError as error:
        raise SystemExit(f"{path}: response stream is not valid UTF-8: {error}")

    responses = {}
    for line_number, line in enumerate(text.splitlines(), 1):
        try:
            label, payload = line.split("\t", 1)
            value = json.loads(payload)
        except (ValueError, json.JSONDecodeError) as error:
            raise SystemExit(f"{path}:{line_number}: invalid labeled JSON response: {error}")
        if label in responses:
            raise SystemExit(f"{path}:{line_number}: duplicate label {label!r}")
        if not isinstance(value, dict) or not isinstance(value.get("ok"), bool):
            raise SystemExit(f"{path}:{line_number}: invalid response envelope for {label!r}")
        responses[label] = value
    return responses


def normalized(label, response):
    if label not in PARSER_CASES:
        return response
    error = response.get("error")
    if response.get("ok") is not False or not isinstance(error, str) or not error.startswith("invalid request:"):
        raise SystemExit(f"{label}: expected an invalid-request parser classification, got {response!r}")
    return {"ok": False, "error": "invalid request: <parser detail>"}


go = load(sys.argv[1])
rust = load(sys.argv[2])
if go.keys() != rust.keys():
    raise SystemExit(f"response labels differ: Go-only={go.keys() - rust.keys()}, Rust-only={rust.keys() - go.keys()}")

expected = {
    "null": {"ok": False, "error": "null request"},
    "missing_operation": {"ok": False, "error": 'unsupported operation ""'},
    "unknown_operation": {"ok": False, "error": 'unsupported operation "unknown"'},
    "root_null": {"ok": False, "error": 'unsupported operation ""'},
    "operation_null": {"ok": False, "error": 'unsupported operation ""'},
    "omitted_defaults": {"ok": True},
    "explicit_null_defaults": {"ok": True},
    "nested_null_defaults": {"ok": False, "error": "Miniflux is not configured"},
    "case_insensitive_fields": {"ok": True, "text": "Groß"},
    "case_duplicate_last_wins": {"ok": True, "text": "last"},
    "null_array_elements": {"ok": True, "text": "zero"},
    "configure_invalid": {"ok": False, "error": "The server URL must be a complete HTTP or HTTPS URL."},
    "unicode_localization": {"ok": True, "text": "Grüße 東京 👋"},
    "unicode_catalog": {"ok": True, "text": "FluxBar — 2 ungelesene Artikel"},
}

for label in (
    "refresh_unconfigured",
    "set_read_unconfigured",
    "set_starred_unconfigured",
    "undo_read_unconfigured",
    "discard_undo_unconfigured",
    "flush_pending_unconfigured",
    "feed_icon_unconfigured",
):
    expected[label] = {"ok": False, "error": "Miniflux is not configured"}

for label, value in expected.items():
    if go.get(label) != value:
        raise SystemExit(f"{label}: unexpected shared semantics: {go.get(label)!r}, expected {value!r}")

for label in go:
    go_value = normalized(label, go[label])
    rust_value = normalized(label, rust[label])
    if go_value != rust_value:
        raise SystemExit(f"{label}: differential mismatch\nGo:  {go[label]!r}\nRust: {rust[label]!r}")

for i in range(2048):
    label = f"sequential_{i:04d}"
    expected_value = (
        {"ok": False, "error": 'unsupported operation "unknown-sequential"'}
        if i % 2 == 0
        else {"ok": True, "text": "naïve café"}
    )
    if go[label] != expected_value:
        raise SystemExit(f"{label}: unexpected sequential response {go[label]!r}")

print(f"Compared {len(go)} valid UTF-8 JSON responses per core; all transport and C ABI checks passed.")
PY
