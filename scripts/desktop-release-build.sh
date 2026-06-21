#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${REPO_ROOT}"

source "${SCRIPT_DIR}/macos-deployment-env.sh"

: "${APPLE_SIGNING_IDENTITY:?APPLE_SIGNING_IDENTITY must be set to a Developer ID Application identity}"
: "${APPLE_API_KEY:?APPLE_API_KEY must be set for notarization}"
: "${APPLE_API_ISSUER:?APPLE_API_ISSUER must be set for notarization}"
: "${APPLE_API_KEY_PATH:?APPLE_API_KEY_PATH must point to the App Store Connect API private key}"

pnpm --dir apps/desktop tauri build --target aarch64-apple-darwin --bundles app,dmg

APP_PATH="target/aarch64-apple-darwin/release/bundle/macos/Wispergo.app"
DMG_DIR="target/aarch64-apple-darwin/release/bundle/dmg"

./scripts/check-macos-thin-bundle.sh "$APP_PATH"

shopt -s nullglob
DMGS=("$DMG_DIR"/*.dmg)
if [[ "${#DMGS[@]}" -ne 1 ]]; then
  echo "Expected exactly one DMG in $DMG_DIR, found ${#DMGS[@]}" >&2
  find target/aarch64-apple-darwin/release/bundle -maxdepth 3 -type f >&2 || true
  exit 1
fi

xcrun stapler validate "${DMGS[0]}"

echo "Release DMG verified: ${DMGS[0]}"
