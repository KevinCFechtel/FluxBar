#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

case "${FLUX_CORE:-go}" in
  go)
    exec "${SCRIPT_DIR}/build-go.sh" "$@"
    ;;
  rust)
    exec "${SCRIPT_DIR}/build-rust.sh" "$@"
    ;;
  *)
    echo "Nicht unterstützter FLUX_CORE-Wert: ${FLUX_CORE}" >&2
    echo "Erlaubte Werte: go, rust" >&2
    exit 1
    ;;
esac
