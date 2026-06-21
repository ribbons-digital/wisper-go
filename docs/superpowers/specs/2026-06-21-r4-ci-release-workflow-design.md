# R4 CI and Release Workflow Design

## Status

Approved and implemented. This slice implements the CI/release workflow portion of the release-readiness track after R1, R2, R3, and R3.5 have merged.

## Goal

Make Wispergo ready for public GitHub Releases by adding repeatable PR gates, a tag-based signed/notarized macOS release workflow, and maintainer-facing release documentation.

## Scope

### In scope

- Add a pull-request CI workflow for the existing shippability gate.
- Add a tag-based release workflow for Apple Silicon macOS artifacts.
- Build a signed/notarized DMG for public releases.
- Keep local contributor builds working through `pnpm desktop:build`.
- Add a release-build script used by GitHub Actions rather than changing local build behavior.
- Add workflow/static validation so release workflow requirements are testable locally.
- Add `docs/release.md` with secrets, tagging, artifact, notarization, and troubleshooting steps.
- Add a README pointer to the release docs.
- Update roadmap and handoff status.

### Out of scope

- Auto-updater support.
- Multi-platform releases.
- Intel macOS artifacts.
- Unsigned public release artifacts.
- Changing inference/runtime behavior.
- Changing first-run UI or app visuals.
- Manual clean-app-support model download smoke unless explicitly requested.

## Existing constraints

- The app is Apple Silicon only.
- Local `pnpm desktop:build` currently creates/uses a local signing identity and produces a thin `.app` bundle.
- Release builds must not use the local self-signed identity.
- The app bundle must remain thin: only the Asset Manifest is bundled, no model binaries/sidecars.
- `pnpm test:ts` may auto-add `packageManager`; do not commit that change.
- Desktop clippy is a required gate: `cargo clippy -p wispergo-desktop --all-targets -- -D warnings`.

## Pull request CI design

Add `.github/workflows/ci.yml`.

Trigger:

- `pull_request`
- `push` to `main`

Runner:

- macOS runner, with Rust stable and Node 22.

Setup:

- Checkout repository.
- Setup pnpm through `pnpm/action-setup`.
- Setup Node with pnpm cache.
- Setup Rust stable with `aarch64-apple-darwin` target available.
- Install/cache Rust dependencies.
- Install frontend dependencies with `pnpm install --frozen-lockfile`.
- Source `scripts/macos-deployment-env.sh` for native builds.

Gate commands:

```bash
cargo build --workspace
cargo test --workspace
cargo clippy -p wispergo-core --all-targets -- -D warnings
cargo clippy -p wispergo-core --all-targets --features llama-cpp -- -D warnings
cargo clippy -p wispergo-desktop --all-targets -- -D warnings
pnpm test:ts
```

Rationale:

- This mirrors the local PR gate that has been used before every release-readiness PR.
- CI does not run model downloads or live dictation smoke.

## Release workflow design

Add `.github/workflows/release.yml`.

Trigger:

- `push` tags matching `v*.*.*`
- `workflow_dispatch` with a required tag input for manual reruns

Permissions:

- `contents: write` to create/update GitHub Releases and upload artifacts.

Required secrets:

- `APPLE_CERTIFICATE`: base64 encoded Developer ID Application `.p12` certificate.
- `APPLE_CERTIFICATE_PASSWORD`: password for the `.p12` certificate.
- `KEYCHAIN_PASSWORD`: temporary CI keychain password.
- `APPLE_API_KEY`: App Store Connect API key id.
- `APPLE_API_ISSUER`: App Store Connect issuer UUID.
- `APPLE_API_KEY_PRIVATE_KEY`: contents of `AuthKey_<APPLE_API_KEY>.p8`.

Release preflight:

- Validate all required secrets are non-empty.
- Validate the trigger tag starts with `v`.
- Import the signing certificate into a temporary keychain.
- Find a `Developer ID Application` identity and export it as `APPLE_SIGNING_IDENTITY`.
- Write the App Store Connect API private key to the expected file and export `APPLE_API_KEY_PATH`.

Build:

- Use a new script `scripts/desktop-release-build.sh`.
- Source `scripts/macos-deployment-env.sh`.
- Run Tauri build for Apple Silicon DMG:

```bash
pnpm --dir apps/desktop tauri build --target aarch64-apple-darwin --bundles app,dmg
```

- Run the thin-bundle check against the release target app path.
- Verify the produced DMG exists.
- Validate stapling on the DMG with `xcrun stapler validate`.

Upload:

- Use `softprops/action-gh-release` or `gh release upload` to attach the DMG and optional ZIP if produced.
- Use a draft release by default so the maintainer can review notes and artifacts before publishing.

Rationale:

- A separate release build script keeps local builds unchanged.
- App Store Connect API key credentials avoid Apple ID app-specific password handling.
- Fail-closed secret preflight prevents accidental unsigned public releases.

## Tauri bundle target design

Do not change local `bundle.targets` to `dmg` in `tauri.conf.json`. Keep local `desktop:build` producing the existing `.app` bundle quickly. The release script passes `--bundles app,dmg` explicitly so CI release builds produce the DMG.

If implementation proves Tauri requires config-level DMG targets for reliable naming, the implementation may set `bundle.targets` to include `dmg` only if `pnpm desktop:build` remains acceptable and tests are updated. Otherwise prefer script-level bundle selection.

## Workflow validation design

Add `scripts/check-github-workflows.sh` and root script `check:release-workflows`.

The script should assert:

- `.github/workflows/ci.yml` exists.
- `.github/workflows/release.yml` exists.
- CI workflow contains each required gate command.
- Release workflow references every required Apple secret.
- Release workflow runs `scripts/desktop-release-build.sh`.
- Release workflow uploads a `.dmg`.
- Release workflow validates notarization/stapling.
- Release build script uses `--target aarch64-apple-darwin` and `--bundles app,dmg`.

This is intentionally static. It catches accidental workflow drift locally without needing release secrets.

## Documentation design

Add `docs/release.md` with:

- Maintainer prerequisites.
- Required GitHub secrets.
- How to create the `.p12` certificate and API key at a high level.
- Version bump/tag flow.
- Expected artifacts.
- Draft release review checklist.
- Gatekeeper/notarization troubleshooting.
- First-run model download note for release notes.

Update README with a short “Release process” pointer to `docs/release.md`.

## Definition of done

- PR CI workflow exists and encodes the required local gate.
- Release workflow exists and is tag/manual-dispatch based.
- Release workflow fails closed on missing Apple signing/notarization secrets.
- Release workflow builds Apple Silicon DMG through a release-specific script.
- Static workflow validation exists and passes locally.
- Release docs explain secrets, tagging, artifacts, and troubleshooting.
- Roadmap and handoff reflect R4 progress/completion.
- Verification commands pass locally:
  - `pnpm check:release-workflows`
  - `pnpm test:ts`
  - `cargo test -p wispergo-desktop`
  - `cargo clippy -p wispergo-desktop --all-targets -- -D warnings`
  - `pnpm desktop:build`
  - `pnpm check:macos-thin-bundle`

## Open decisions for user approval

1. Should the release workflow create a draft release by default? Recommendation: yes.
2. Should the public release upload only DMG, or DMG plus ZIP? Recommendation: DMG only for first public release, keep ZIP out until there is a user need.
3. Should release docs use App Store Connect API key credentials as the only supported notarization path? Recommendation: yes, avoid Apple ID password flow.
