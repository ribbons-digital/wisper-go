#!/usr/bin/env bash
set -euo pipefail

APP_PATH="${1:-target/release/bundle/macos/Wispergo.app}"
RESOURCE_DIR="${APP_PATH}/Contents/Resources"
MAX_BYTES=$((200 * 1024 * 1024))

if [[ ! -d "${APP_PATH}" ]]; then
  echo "Missing app bundle: ${APP_PATH}" >&2
  exit 1
fi

if [[ ! -f "${RESOURCE_DIR}/resources/models.manifest.json" ]]; then
  echo "Missing bundled asset manifest at resources/models.manifest.json" >&2
  exit 1
fi

for retired in "${RESOURCE_DIR}/bin" "${RESOURCE_DIR}/models"; do
  if [[ -e "${retired}" ]]; then
    echo "Retired bundled asset path is present: ${retired}" >&2
    exit 1
  fi
done

if find "${RESOURCE_DIR}" -type f \( \
  -name '*.bin' -o \
  -name '*.gguf' -o \
  -name '*.dylib' -o \
  -name 'whisper-cli' -o \
  -name 'llama-server' \
\) | grep -q .; then
  echo "Found retired model/sidecar artifact in ${RESOURCE_DIR}:" >&2
  find "${RESOURCE_DIR}" -type f \( \
    -name '*.bin' -o \
    -name '*.gguf' -o \
    -name '*.dylib' -o \
    -name 'whisper-cli' -o \
    -name 'llama-server' \
  \) >&2
  exit 1
fi

size_bytes=$(du -sk "${APP_PATH}" | awk '{print $1 * 1024}')
if (( size_bytes > MAX_BYTES )); then
  echo "App bundle is too large for thin build: ${size_bytes} bytes > ${MAX_BYTES} bytes" >&2
  exit 1
fi

echo "Thin macOS bundle verified: ${APP_PATH} (${size_bytes} bytes)"
