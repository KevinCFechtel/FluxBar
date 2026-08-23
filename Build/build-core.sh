#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
OUTPUT_DIR="${1:?output directory required}"
shift
ARCHS=("$@")

case "${FLUX_CORE:-rust}" in
  rust)
    exec "${SCRIPT_DIR}/build-rust-core.sh" "${OUTPUT_DIR}" "${ARCHS[@]}"
    ;;
  go)
    exec "${SCRIPT_DIR}/build-go-core.sh" "${OUTPUT_DIR}" "${ARCHS[@]}"
    ;;
  *)
    echo "Nicht unterstützter FLUX_CORE-Wert: ${FLUX_CORE}" >&2
    echo "Erlaubte Werte: go, rust" >&2
    exit 1
    ;;
esac
