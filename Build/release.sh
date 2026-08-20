#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPOSITORY_DIR="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
APP_DIR="${REPOSITORY_DIR}/dist/FluxBar.app"
RELEASE_DIR="${REPOSITORY_DIR}/dist/release"
INFO_PLIST="${SCRIPT_DIR}/Info.plist"
RELEASE_ENV_FILE="${FLUXBAR_RELEASE_ENV_FILE:-${SCRIPT_DIR}/.env}"

if [[ -f "${RELEASE_ENV_FILE}" ]]; then
  set -a
  # shellcheck disable=SC1090
  source "${RELEASE_ENV_FILE}"
  set +a
fi

SIGNING_IDENTITY="${SIGNING_IDENTITY:-}"
NOTARY_PROFILE="${NOTARY_PROFILE:-}"
SIGNING_TIMESTAMP_URL="${SIGNING_TIMESTAMP_URL:-}"
NOTARY_TIMEOUT="${NOTARY_TIMEOUT:-30m}"
RELEASE_ARCH="${GOARCH:-$(go env GOARCH)}"
RELEASE_VERSION="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "${INFO_PLIST}")"

if [[ -z "${SIGNING_IDENTITY}" ]]; then
  echo "SIGNING_IDENTITY fehlt (Developer ID Application)." >&2
  exit 1
fi

if [[ -z "${NOTARY_PROFILE}" ]]; then
  echo "NOTARY_PROFILE fehlt (Name eines notarytool-Keychain-Profils)." >&2
  exit 1
fi

if [[ ! "${RELEASE_VERSION}" =~ ^[0-9A-Za-z][0-9A-Za-z._-]*$ ]]; then
  echo "Ungültige Release-Version: ${RELEASE_VERSION}" >&2
  exit 1
fi

if [[ ! "${RELEASE_ARCH}" =~ ^[0-9A-Za-z_-]+$ ]]; then
  echo "Ungültige Architektur: ${RELEASE_ARCH}" >&2
  exit 1
fi

for command_name in codesign dscacheutil ditto go security spctl xcrun; do
  if ! command -v "${command_name}" >/dev/null 2>&1; then
    echo "Benötigtes Programm fehlt: ${command_name}" >&2
    exit 1
  fi
done

if ! security find-identity -v -p codesigning | grep -F -- "${SIGNING_IDENTITY}" >/dev/null; then
  echo "SIGNING_IDENTITY wurde nicht als gültige Codesignatur-Identität gefunden." >&2
  exit 1
fi

if [[ -z "${SIGNING_TIMESTAMP_URL}" ]]; then
  timestamp_ipv4="$({
    dscacheutil -q host -a name timestamp.apple.com || true
  } | awk '/ip_address:/ && $2 ~ /^[0-9.]+$/ {print $2; exit}')"

  if [[ -z "${timestamp_ipv4}" ]]; then
    echo "Keine IPv4-Adresse für timestamp.apple.com gefunden." >&2
    echo "Alternativ SIGNING_TIMESTAMP_URL explizit setzen." >&2
    exit 1
  fi

  SIGNING_TIMESTAMP_URL="http://${timestamp_ipv4}/ts01"
fi

SUBMISSION_ARCHIVE="${RELEASE_DIR}/FluxBar-${RELEASE_VERSION}-notarization.zip"
FINAL_ARCHIVE="${RELEASE_DIR}/FluxBar-${RELEASE_VERSION}-macos-${RELEASE_ARCH}.zip"
CHECK_DIR="$(mktemp -d /tmp/fluxbar-release-check.XXXXXX)"

cleanup() {
  rm -rf -- "${CHECK_DIR}"
}
trap cleanup EXIT

echo "1/8 FluxBar-App bauen"
CONFIGURATION=Release GOARCH="${RELEASE_ARCH}" "${SCRIPT_DIR}/build.sh"

echo "2/8 Mit Developer ID und Hardened Runtime signieren"
codesign \
  --force \
  --options runtime \
  --timestamp="${SIGNING_TIMESTAMP_URL}" \
  --sign "${SIGNING_IDENTITY}" \
  "${APP_DIR}"

codesign --verify --deep --strict --verbose=4 "${APP_DIR}"

mkdir -p "${RELEASE_DIR}"

echo "3/8 Archiv zur Notarisierung erstellen"
rm -f -- "${SUBMISSION_ARCHIVE}"
COPYFILE_DISABLE=1 ditto \
  -c -k \
  --keepParent \
  --norsrc \
  --noextattr \
  "${APP_DIR}" \
  "${SUBMISSION_ARCHIVE}"

echo "4/8 Bei Apple einreichen und Ergebnis abwarten"
xcrun notarytool submit \
  "${SUBMISSION_ARCHIVE}" \
  --keychain-profile "${NOTARY_PROFILE}" \
  --wait \
  --timeout "${NOTARY_TIMEOUT}"

echo "5/8 Notarisierungsticket an die App heften"
xcrun stapler staple "${APP_DIR}"
xcrun stapler validate "${APP_DIR}"

echo "6/8 Signatur und Gatekeeper-Freigabe prüfen"
codesign --verify --deep --strict --verbose=4 "${APP_DIR}"
spctl --assess --type execute --verbose=4 "${APP_DIR}"

echo "7/8 Sauberes Release-ZIP ohne AppleDouble-Dateien erstellen"
rm -f -- "${FINAL_ARCHIVE}"
COPYFILE_DISABLE=1 ditto \
  -c -k \
  --keepParent \
  --norsrc \
  --noextattr \
  "${APP_DIR}" \
  "${FINAL_ARCHIVE}"

echo "8/8 Release-ZIP erneut extrahieren und vollständig prüfen"
ditto -x -k "${FINAL_ARCHIVE}" "${CHECK_DIR}"
xcrun stapler validate "${CHECK_DIR}/FluxBar.app"
codesign --verify --deep --strict --verbose=4 "${CHECK_DIR}/FluxBar.app"
spctl --assess --type execute --verbose=4 "${CHECK_DIR}/FluxBar.app"

echo "Release erstellt: ${FINAL_ARCHIVE}"
