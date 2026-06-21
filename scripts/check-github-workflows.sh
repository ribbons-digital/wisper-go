#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CI_WORKFLOW="$ROOT_DIR/.github/workflows/ci.yml"
RELEASE_WORKFLOW="$ROOT_DIR/.github/workflows/release.yml"
RELEASE_BUILD_SCRIPT="$ROOT_DIR/scripts/desktop-release-build.sh"

require_file() {
  local path="$1"
  if [[ ! -f "$path" ]]; then
    echo "Missing required file: ${path#$ROOT_DIR/}" >&2
    exit 1
  fi
}

require_contains() {
  local path="$1"
  local text="$2"
  if ! grep -Fq -- "$text" "$path"; then
    echo "Missing expected text in ${path#$ROOT_DIR/}: $text" >&2
    exit 1
  fi
}

require_file "$CI_WORKFLOW"
require_file "$RELEASE_WORKFLOW"
require_file "$RELEASE_BUILD_SCRIPT"

for command in \
  "cargo build --workspace" \
  "cargo test --workspace" \
  "cargo clippy -p wispergo-core --all-targets -- -D warnings" \
  "cargo clippy -p wispergo-core --all-targets --features llama-cpp -- -D warnings" \
  "cargo clippy -p wispergo-desktop --all-targets -- -D warnings" \
  "pnpm test:ts"; do
  require_contains "$CI_WORKFLOW" "$command"
done

require_contains "$RELEASE_BUILD_SCRIPT" "--target aarch64-apple-darwin"
require_contains "$RELEASE_BUILD_SCRIPT" "--bundles app,dmg"
require_contains "$RELEASE_BUILD_SCRIPT" "check-macos-thin-bundle.sh"
require_contains "$RELEASE_BUILD_SCRIPT" "xcrun stapler validate"

for secret in \
  "APPLE_CERTIFICATE" \
  "APPLE_CERTIFICATE_PASSWORD" \
  "KEYCHAIN_PASSWORD" \
  "APPLE_API_KEY" \
  "APPLE_API_ISSUER" \
  "APPLE_API_KEY_PRIVATE_KEY"; do
  require_contains "$RELEASE_WORKFLOW" "$secret"
done

require_contains "$RELEASE_WORKFLOW" "v*.*.*"
require_contains "$RELEASE_WORKFLOW" "scripts/desktop-release-build.sh"
require_contains "$RELEASE_WORKFLOW" "softprops/action-gh-release@v2"
require_contains "$RELEASE_WORKFLOW" "*.dmg"
require_contains "$RELEASE_WORKFLOW" "draft: true"
require_contains "$RELEASE_WORKFLOW" "Developer ID Application"

echo "GitHub workflow configuration verified."
