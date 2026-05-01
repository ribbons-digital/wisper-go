#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RESOURCE_DIR="$ROOT_DIR/apps/desktop/src-tauri/resources"

required=(
  "bin/macos-aarch64/whisper-cli"
  "bin/macos-aarch64/llama-server"
  "bin/macos-x86_64/whisper-cli"
  "bin/macos-x86_64/llama-server"
  "models/asr/ggml-large-v3-turbo.bin"
  "models/cleanup/qwen2.5-3b-instruct-q4_k_m.gguf"
)

missing=()
for relative in "${required[@]}"; do
  path="$RESOURCE_DIR/$relative"
  if [[ ! -e "$path" ]]; then
    missing+=("$relative")
  fi
  if [[ "$relative" == bin/* && -e "$path" && ! -x "$path" ]]; then
    echo "Inference binary is not executable: $relative" >&2
    exit 1
  fi
done

if (( ${#missing[@]} > 0 )); then
  echo "Missing bundled inference assets:" >&2
  printf '  - %s\n' "${missing[@]}" >&2
  echo "Stage whisper.cpp, llama.cpp, ggml-large-v3-turbo, and Qwen2.5-3B GGUF assets before release packaging." >&2
  exit 1
fi

echo "Bundled inference assets verified."
