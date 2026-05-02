#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_PATH="${1:-$ROOT_DIR/target/release/bundle/macos/Wispergo.app}"
RESOURCE_DIR="$APP_PATH/Contents/Resources"

case "$(uname -m)" in
  arm64)
    current_arch="macos-aarch64"
    ;;
  x86_64)
    current_arch="macos-x86_64"
    ;;
  *)
    echo "Unsupported macOS architecture: $(uname -m)" >&2
    exit 1
    ;;
esac

required_dirs=(
  "bin/macos-aarch64"
  "bin/macos-x86_64"
  "models/asr"
  "models/cleanup"
)

required_files=(
  "bin/$current_arch/whisper-cli"
  "bin/$current_arch/llama-server"
  "models/asr/ggml-large-v3-turbo.bin"
  "models/cleanup/qwen2.5-3b-instruct-q4_k_m.gguf"
)

if [[ ! -d "$APP_PATH" ]]; then
  echo "Built app bundle not found: $APP_PATH" >&2
  exit 1
fi

missing_dirs=()
for relative in "${required_dirs[@]}"; do
  if [[ ! -d "$RESOURCE_DIR/$relative" ]]; then
    missing_dirs+=("$relative")
  fi
done

missing_files=()
for relative in "${required_files[@]}"; do
  if [[ ! -f "$RESOURCE_DIR/$relative" ]]; then
    missing_files+=("$relative")
  fi
done

if (( ${#missing_dirs[@]} > 0 )); then
  echo "Built app bundle is missing inference resource directories:" >&2
  printf '  - Contents/Resources/%s\n' "${missing_dirs[@]}" >&2
fi

if (( ${#missing_files[@]} > 0 )); then
  echo "Built app bundle is missing required inference files for $current_arch:" >&2
  printf '  - Contents/Resources/%s\n' "${missing_files[@]}" >&2
fi

if (( ${#missing_dirs[@]} > 0 || ${#missing_files[@]} > 0 )); then
  exit 1
fi

echo "Built app inference resource layout verified for $current_arch."
