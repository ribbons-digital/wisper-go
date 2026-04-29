#!/usr/bin/env bash
set -euo pipefail

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "Skipping macOS app signing on non-Darwin host."
  exit 0
fi

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_PATH="${1:-$ROOT_DIR/target/release/bundle/macos/Wispergo.app}"
ENTITLEMENTS_PATH="$ROOT_DIR/apps/desktop/src-tauri/Entitlements.plist"
IDENTIFIER="com.ribbonsdigital.wispergo"
IDENTITY="${WISPERGO_CODESIGN_IDENTITY:-Wispergo Local Code Signing}"
KEYCHAIN="${WISPERGO_CODESIGN_KEYCHAIN:-$(security default-keychain -d user | sed 's/^ *//; s/"//g')}"
REQUIREMENT="=designated => identifier \"$IDENTIFIER\""

if [[ ! -d "$APP_PATH" ]]; then
  echo "macOS app bundle not found: $APP_PATH" >&2
  exit 1
fi

codesign \
  --force \
  --deep \
  --sign "$IDENTITY" \
  --keychain "$KEYCHAIN" \
  --options runtime \
  --entitlements "$ENTITLEMENTS_PATH" \
  --requirements "$REQUIREMENT" \
  "$APP_PATH"

codesign --verify --deep --strict "$APP_PATH"
codesign -dr - "$APP_PATH" 2>&1 | grep -F "designated => identifier \"$IDENTIFIER\"" >/dev/null

echo "Signed $APP_PATH with $IDENTITY and stable local designated requirement for $IDENTIFIER"
