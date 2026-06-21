# R4 CI and Release Workflow Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add PR CI, signed/notarized macOS DMG release automation, and release documentation for Wispergo.

**Architecture:** Keep local builds unchanged and add a separate release-build path used by GitHub Actions. Use static workflow validation to make the CI/release contract testable without Apple secrets. Document maintainer release steps in `docs/release.md` and keep roadmap/HANDOFF in sync.

**Tech Stack:** GitHub Actions, Tauri v2 CLI, pnpm, Rust/Cargo, macOS codesign/notarytool/stapler, shell scripts.

---

## File map

- Create `.github/workflows/ci.yml`: PR and main push CI gate.
- Create `.github/workflows/release.yml`: tag/manual release workflow for signed/notarized Apple Silicon DMG.
- Create `scripts/desktop-release-build.sh`: release-only Tauri build wrapper that builds app + DMG for `aarch64-apple-darwin`.
- Create `scripts/check-github-workflows.sh`: static validation for workflow and release script requirements.
- Modify `package.json`: add `check:release-workflows` script. Do not commit a `packageManager` field.
- Create `docs/release.md`: maintainer release process and secret setup docs.
- Modify `README.md`: point maintainers to `docs/release.md`.
- Modify `docs/superpowers/plans/2026-06-18-in-process-inference-roadmap.md`: mark R4 in progress/done as implementation progresses.
- Modify `HANDOFF.md`: reflect R4 as current/completed slice.

---

## Task 1: Add workflow validation harness

**Files:**
- Create: `scripts/check-github-workflows.sh`
- Modify: `package.json`

- [ ] **Step 1: Create failing validation script skeleton**

Create `scripts/check-github-workflows.sh` with strict shell and checks for files that do not exist yet:

```bash
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

echo "GitHub workflow configuration verified."
```

- [ ] **Step 2: Make script executable and run it to verify RED**

Run:

```bash
chmod +x scripts/check-github-workflows.sh
./scripts/check-github-workflows.sh
```

Expected: FAIL because workflow files and release script are not created yet.

- [ ] **Step 3: Add package script**

Modify root `package.json` scripts to include:

```json
"check:release-workflows": "./scripts/check-github-workflows.sh"
```

Keep existing scripts unchanged. Do not add `packageManager`.

---

## Task 2: Add PR CI workflow

**Files:**
- Create: `.github/workflows/ci.yml`
- Modify: `scripts/check-github-workflows.sh`

- [ ] **Step 1: Create CI workflow**

Create `.github/workflows/ci.yml`:

```yaml
name: CI

on:
  pull_request:
  push:
    branches:
      - main

concurrency:
  group: ci-${{ github.ref }}
  cancel-in-progress: true

jobs:
  test:
    name: Test and lint
    runs-on: macos-latest
    env:
      CARGO_TERM_COLOR: always
    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Setup pnpm
        uses: pnpm/action-setup@v4
        with:
          run_install: false

      - name: Setup Node
        uses: actions/setup-node@v4
        with:
          node-version: '22'
          cache: pnpm

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: aarch64-apple-darwin

      - name: Cache Rust
        uses: swatinem/rust-cache@v2

      - name: Install native build prerequisites
        run: brew install cmake

      - name: Install frontend dependencies
        run: pnpm install --frozen-lockfile

      - name: Cargo build workspace
        run: |
          source scripts/macos-deployment-env.sh
          cargo build --workspace

      - name: Cargo test workspace
        run: |
          source scripts/macos-deployment-env.sh
          cargo test --workspace

      - name: Clippy core
        run: |
          source scripts/macos-deployment-env.sh
          cargo clippy -p wispergo-core --all-targets -- -D warnings

      - name: Clippy core with llama-cpp
        run: |
          source scripts/macos-deployment-env.sh
          cargo clippy -p wispergo-core --all-targets --features llama-cpp -- -D warnings

      - name: Clippy desktop
        run: |
          source scripts/macos-deployment-env.sh
          cargo clippy -p wispergo-desktop --all-targets -- -D warnings

      - name: Frontend tests
        run: pnpm test:ts
```

- [ ] **Step 2: Extend static validation for CI gate commands**

Add after `require_file "$RELEASE_BUILD_SCRIPT"` in `scripts/check-github-workflows.sh`:

```bash
for command in \
  "cargo build --workspace" \
  "cargo test --workspace" \
  "cargo clippy -p wispergo-core --all-targets -- -D warnings" \
  "cargo clippy -p wispergo-core --all-targets --features llama-cpp -- -D warnings" \
  "cargo clippy -p wispergo-desktop --all-targets -- -D warnings" \
  "pnpm test:ts"; do
  require_contains "$CI_WORKFLOW" "$command"
done
```

- [ ] **Step 3: Run validation to verify still RED**

Run:

```bash
pnpm check:release-workflows
```

Expected: FAIL because release workflow/release build script are still missing.

---

## Task 3: Add release build script

**Files:**
- Create: `scripts/desktop-release-build.sh`
- Modify: `scripts/check-github-workflows.sh`

- [ ] **Step 1: Create release build wrapper**

Create `scripts/desktop-release-build.sh`:

```bash
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
DMG_PATH="target/aarch64-apple-darwin/release/bundle/dmg/Wispergo_0.1.0_aarch64.dmg"

./scripts/check-macos-thin-bundle.sh "$APP_PATH"

if [[ ! -f "$DMG_PATH" ]]; then
  echo "Expected DMG not found: $DMG_PATH" >&2
  find target/aarch64-apple-darwin/release/bundle -maxdepth 3 -type f >&2 || true
  exit 1
fi

xcrun stapler validate "$DMG_PATH"

echo "Release DMG verified: $DMG_PATH"
```

Implementation note: if Tauri emits a slightly different DMG filename, adjust `DMG_PATH` to discover exactly one `target/aarch64-apple-darwin/release/bundle/dmg/*.dmg` instead of hard-coding the versioned path.

- [ ] **Step 2: Make script executable**

Run:

```bash
chmod +x scripts/desktop-release-build.sh
```

- [ ] **Step 3: Extend static validation for release build script**

Add to `scripts/check-github-workflows.sh`:

```bash
require_contains "$RELEASE_BUILD_SCRIPT" "--target aarch64-apple-darwin"
require_contains "$RELEASE_BUILD_SCRIPT" "--bundles app,dmg"
require_contains "$RELEASE_BUILD_SCRIPT" "check-macos-thin-bundle.sh"
require_contains "$RELEASE_BUILD_SCRIPT" "xcrun stapler validate"
```

- [ ] **Step 4: Run validation to verify still RED**

Run:

```bash
pnpm check:release-workflows
```

Expected: FAIL because release workflow is still missing.

---

## Task 4: Add tag-based release workflow

**Files:**
- Create: `.github/workflows/release.yml`
- Modify: `scripts/check-github-workflows.sh`

- [ ] **Step 1: Create release workflow**

Create `.github/workflows/release.yml`:

```yaml
name: Release

on:
  push:
    tags:
      - 'v*.*.*'
  workflow_dispatch:
    inputs:
      tag:
        description: 'Release tag, for example v0.1.0'
        required: true
        type: string

permissions:
  contents: write

concurrency:
  group: release-${{ github.ref }}
  cancel-in-progress: false

jobs:
  macos:
    name: Build signed macOS DMG
    runs-on: macos-latest
    env:
      CARGO_TERM_COLOR: always
      RELEASE_TAG: ${{ github.event_name == 'workflow_dispatch' && inputs.tag || github.ref_name }}
    steps:
      - name: Checkout
        uses: actions/checkout@v4
        with:
          ref: ${{ env.RELEASE_TAG }}

      - name: Validate tag and required secrets
        env:
          APPLE_CERTIFICATE: ${{ secrets.APPLE_CERTIFICATE }}
          APPLE_CERTIFICATE_PASSWORD: ${{ secrets.APPLE_CERTIFICATE_PASSWORD }}
          KEYCHAIN_PASSWORD: ${{ secrets.KEYCHAIN_PASSWORD }}
          APPLE_API_KEY: ${{ secrets.APPLE_API_KEY }}
          APPLE_API_ISSUER: ${{ secrets.APPLE_API_ISSUER }}
          APPLE_API_KEY_PRIVATE_KEY: ${{ secrets.APPLE_API_KEY_PRIVATE_KEY }}
        run: |
          set -euo pipefail
          case "$RELEASE_TAG" in
            v*.*.*) ;;
            *) echo "Release tag must look like v0.1.0: $RELEASE_TAG" >&2; exit 1 ;;
          esac
          for name in APPLE_CERTIFICATE APPLE_CERTIFICATE_PASSWORD KEYCHAIN_PASSWORD APPLE_API_KEY APPLE_API_ISSUER APPLE_API_KEY_PRIVATE_KEY; do
            if [[ -z "${!name:-}" ]]; then
              echo "Missing required secret: $name" >&2
              exit 1
            fi
          done

      - name: Setup pnpm
        uses: pnpm/action-setup@v4
        with:
          run_install: false

      - name: Setup Node
        uses: actions/setup-node@v4
        with:
          node-version: '22'
          cache: pnpm

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: aarch64-apple-darwin

      - name: Cache Rust
        uses: swatinem/rust-cache@v2

      - name: Install native build prerequisites
        run: brew install cmake

      - name: Install frontend dependencies
        run: pnpm install --frozen-lockfile

      - name: Import Developer ID certificate
        env:
          APPLE_CERTIFICATE: ${{ secrets.APPLE_CERTIFICATE }}
          APPLE_CERTIFICATE_PASSWORD: ${{ secrets.APPLE_CERTIFICATE_PASSWORD }}
          KEYCHAIN_PASSWORD: ${{ secrets.KEYCHAIN_PASSWORD }}
        run: |
          set -euo pipefail
          CERT_PATH="$RUNNER_TEMP/developer-id.p12"
          KEYCHAIN_PATH="$RUNNER_TEMP/wispergo-release.keychain-db"
          printf '%s' "$APPLE_CERTIFICATE" | base64 --decode > "$CERT_PATH"
          security create-keychain -p "$KEYCHAIN_PASSWORD" "$KEYCHAIN_PATH"
          security set-keychain-settings -lut 21600 "$KEYCHAIN_PATH"
          security unlock-keychain -p "$KEYCHAIN_PASSWORD" "$KEYCHAIN_PATH"
          security import "$CERT_PATH" -k "$KEYCHAIN_PATH" -P "$APPLE_CERTIFICATE_PASSWORD" -T /usr/bin/codesign -T /usr/bin/security
          security set-key-partition-list -S apple-tool:,apple:,codesign: -s -k "$KEYCHAIN_PASSWORD" "$KEYCHAIN_PATH"
          security list-keychains -d user -s "$KEYCHAIN_PATH" $(security list-keychains -d user | tr -d '"')
          CERT_ID=$(security find-identity -v -p codesigning "$KEYCHAIN_PATH" | grep "Developer ID Application" | head -1 | sed -E 's/.*"([^"]+)".*/\1/')
          if [[ -z "$CERT_ID" ]]; then
            echo "No Developer ID Application identity found in imported certificate" >&2
            exit 1
          fi
          echo "APPLE_SIGNING_IDENTITY=$CERT_ID" >> "$GITHUB_ENV"
          echo "WISPERGO_CODESIGN_KEYCHAIN=$KEYCHAIN_PATH" >> "$GITHUB_ENV"

      - name: Configure App Store Connect API key
        env:
          APPLE_API_KEY: ${{ secrets.APPLE_API_KEY }}
          APPLE_API_KEY_PRIVATE_KEY: ${{ secrets.APPLE_API_KEY_PRIVATE_KEY }}
        run: |
          set -euo pipefail
          KEY_DIR="$RUNNER_TEMP/appstoreconnect/private_keys"
          KEY_PATH="$KEY_DIR/AuthKey_${APPLE_API_KEY}.p8"
          mkdir -p "$KEY_DIR"
          printf '%s' "$APPLE_API_KEY_PRIVATE_KEY" > "$KEY_PATH"
          chmod 600 "$KEY_PATH"
          echo "APPLE_API_KEY_PATH=$KEY_PATH" >> "$GITHUB_ENV"

      - name: Validate workflow configuration
        run: pnpm check:release-workflows

      - name: Build signed and notarized DMG
        env:
          APPLE_API_KEY: ${{ secrets.APPLE_API_KEY }}
          APPLE_API_ISSUER: ${{ secrets.APPLE_API_ISSUER }}
        run: ./scripts/desktop-release-build.sh

      - name: Upload draft GitHub Release
        uses: softprops/action-gh-release@v2
        with:
          tag_name: ${{ env.RELEASE_TAG }}
          name: Wispergo ${{ env.RELEASE_TAG }}
          draft: true
          prerelease: false
          files: |
            target/aarch64-apple-darwin/release/bundle/dmg/*.dmg
          body: |
            ## Wispergo ${{ env.RELEASE_TAG }}

            Download the DMG, open it, and move Wispergo to Applications.
            On first launch, Wispergo downloads required local model Assets.
```

- [ ] **Step 2: Extend static validation for release workflow**

Add to `scripts/check-github-workflows.sh`:

```bash
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
```

- [ ] **Step 3: Run validation to verify PASS**

Run:

```bash
pnpm check:release-workflows
```

Expected: PASS.

---

## Task 5: Add release documentation

**Files:**
- Create: `docs/release.md`
- Modify: `README.md`

- [ ] **Step 1: Create release docs**

Create `docs/release.md` with these sections:

```markdown
# Wispergo Release Process

## Release artifact

Public releases ship a signed and notarized Apple Silicon macOS DMG. The app is thin: model Assets are downloaded on first run into app-support storage.

## Required GitHub secrets

- `APPLE_CERTIFICATE`: base64 encoded Developer ID Application `.p12` certificate.
- `APPLE_CERTIFICATE_PASSWORD`: password for the `.p12` certificate.
- `KEYCHAIN_PASSWORD`: temporary CI keychain password.
- `APPLE_API_KEY`: App Store Connect API key id.
- `APPLE_API_ISSUER`: App Store Connect issuer UUID.
- `APPLE_API_KEY_PRIVATE_KEY`: contents of the `AuthKey_<APPLE_API_KEY>.p8` private key.

## Before tagging

1. Confirm `main` is green.
2. Confirm app version in `package.json` and `apps/desktop/src-tauri/tauri.conf.json` is the intended release version.
3. Run local confidence checks:

```bash
pnpm check:release-workflows
pnpm test:ts
cargo test -p wispergo-desktop
cargo clippy -p wispergo-desktop --all-targets -- -D warnings
pnpm desktop:build
pnpm check:macos-thin-bundle
```

## Create a release

```bash
git checkout main
git pull --ff-only origin main
git tag v0.1.0
git push origin v0.1.0
```

The release workflow creates a draft GitHub Release. Review the artifact and release notes before publishing.

## Draft release review checklist

- DMG is present.
- Workflow completed signing, notarization, stapling, and thin-bundle verification.
- Release notes mention first-run model downloads.
- Release notes mention required macOS permissions.
- Downloaded app opens without Gatekeeper warning on a clean macOS machine.
- First-run setup opens if required models or permissions are missing.

## Troubleshooting

### Missing secrets

The workflow fails before building if any signing/notarization secret is missing.

### Certificate import fails

Confirm the `.p12` is a Developer ID Application certificate and the password matches `APPLE_CERTIFICATE_PASSWORD`.

### Notarization fails

Check the App Store Connect API key id, issuer, and private key content. The key must have access to notarization for the Apple Developer team.

### DMG is missing

Run `pnpm --dir apps/desktop tauri build --target aarch64-apple-darwin --bundles app,dmg` locally on macOS with release signing environment configured, or inspect Tauri output paths and update `scripts/desktop-release-build.sh`.
```

- [ ] **Step 2: Add README pointer**

Add under the development/build section:

```markdown
### Release process

Maintainer release instructions, required GitHub secrets, and the signed/notarized DMG workflow are documented in [`docs/release.md`](docs/release.md).
```

- [ ] **Step 3: Run documentation/static validation**

Run:

```bash
pnpm check:release-workflows
```

Expected: PASS.

---

## Task 6: Update roadmap and handoff

**Files:**
- Modify: `docs/superpowers/plans/2026-06-18-in-process-inference-roadmap.md`
- Modify: `HANDOFF.md`

- [ ] **Step 1: Update roadmap R4 status**

Change R4 from not-started to in-progress during implementation, then to done before PR if implementation and verification pass.

Final R4 entry should say:

```markdown
- **R4 CI and release workflow** ✅
  - Added PR CI workflow for the established shippability gate.
  - Added tag/manual release workflow for signed/notarized Apple Silicon DMG artifacts.
  - Added static workflow validation and maintainer release docs.
  - DoD: `pnpm check:release-workflows`, frontend tests, desktop tests/clippy, build, and thin-bundle check pass locally; release workflow remains secret-gated until credentials are configured in GitHub.
```

- [ ] **Step 2: Update HANDOFF**

Update the top date/focus and status snapshot to reflect R4 implementation. Mention any residual risk: the release workflow cannot be fully executed locally without Apple Developer ID and App Store Connect secrets.

---

## Task 7: Full local verification and PR

**Files:**
- No new files unless verification reveals needed fixes.

- [ ] **Step 1: Run static workflow validation**

Run:

```bash
pnpm check:release-workflows
```

Expected: PASS.

- [ ] **Step 2: Run frontend tests**

Run:

```bash
pnpm test:ts
```

Expected: PASS. If Corepack adds `packageManager`, remove it before commit.

- [ ] **Step 3: Run desktop Rust tests**

Run:

```bash
cargo test -p wispergo-desktop
```

Expected: PASS.

- [ ] **Step 4: Run desktop clippy gate**

Run:

```bash
cargo clippy -p wispergo-desktop --all-targets -- -D warnings
```

Expected: PASS.

- [ ] **Step 5: Run local app build and thin-bundle check**

Run:

```bash
pnpm desktop:build
pnpm check:macos-thin-bundle
```

Expected: PASS.

- [ ] **Step 6: Confirm packageManager was not committed**

Run:

```bash
python3 - <<'PY'
import json
from pathlib import Path
p = Path('package.json')
data = json.loads(p.read_text())
if 'packageManager' in data:
    raise SystemExit('packageManager field present; remove before commit')
print('packageManager absent')
PY
```

Expected: `packageManager absent`.

- [ ] **Step 7: Commit and open PR**

Run:

```bash
git status --short
git add .github/workflows/ci.yml .github/workflows/release.yml scripts/desktop-release-build.sh scripts/check-github-workflows.sh package.json docs/release.md README.md docs/superpowers/plans/2026-06-18-in-process-inference-roadmap.md HANDOFF.md docs/superpowers/specs/2026-06-21-r4-ci-release-workflow-design.md docs/superpowers/plans/2026-06-21-r4-ci-release-workflow.md
git commit -m "ci: add release workflow"
git push -u origin r4-ci-release-workflow
gh pr create --base main --head r4-ci-release-workflow --title "ci: add release workflow" --body-file /tmp/wispergo-r4-release-pr.md
```

PR body should include:

- Summary of CI workflow, release workflow, validation script, and docs.
- Verification commands run.
- Residual risk that notarized release workflow requires GitHub secrets and a tag/manual run.

---

## Self-review

- Spec coverage: PR CI, release workflow, DMG artifact, signing/notarization docs, release checklist, and validation are each mapped to tasks.
- Scope control: no runtime behavior, UI, model download, updater, Intel, or multi-platform work.
- Residual risk: the release workflow cannot be end-to-end proven without Apple Developer Program credentials and repository secrets; local verification covers syntax/static invariants and regular app build gates.
