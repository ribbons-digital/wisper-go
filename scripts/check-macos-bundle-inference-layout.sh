#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_PATH="${1:-$ROOT_DIR/target/release/bundle/macos/Wispergo.app}"
RESOURCE_DIR="$APP_PATH/Contents/Resources"

required_dirs=(
  "bin/macos-aarch64"
  "bin/macos-x86_64"
  "models/asr"
  "models/cleanup"
)

if [[ ! -d "$APP_PATH" ]]; then
  echo "Built app bundle not found: $APP_PATH" >&2
  exit 1
fi

missing=()
for relative in "${required_dirs[@]}"; do
  if [[ ! -d "$RESOURCE_DIR/$relative" ]]; then
    missing+=("$relative")
  fi
done

if (( ${#missing[@]} > 0 )); then
  echo "Built app bundle is missing inference resource directories:" >&2
  printf '  - Contents/Resources/%s\n' "${missing[@]}" >&2
  exit 1
fi

echo "Built app inference resource layout verified."
