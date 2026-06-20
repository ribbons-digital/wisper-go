#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${REPO_ROOT}"

source "${SCRIPT_DIR}/macos-deployment-env.sh"
./scripts/ensure-local-codesign-cert.sh
pnpm --dir apps/desktop tauri build
./scripts/sign-macos-app.sh
