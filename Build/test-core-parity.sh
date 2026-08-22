#!/usr/bin/env bash
set -euo pipefail

# Runs every focused Go/Rust compatibility suite in dependency order.
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

SUITES=(
  test-core-compat.sh
  test-sqlite-compat.sh
  test-snapshot-compat.sh
  test-remote-compat.sh
  test-sync-compat.sh
  test-article-compat.sh
  test-localization-compat.sh
  test-icon-compat.sh
)

for suite in "${SUITES[@]}"; do
  echo "=== ${suite} ==="
  "${SCRIPT_DIR}/${suite}"
done

echo "All Go/Rust core parity suites passed."
