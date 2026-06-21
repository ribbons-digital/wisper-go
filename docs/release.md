# Wispergo Release Process

## Release artifact

Public releases ship a signed and notarized Apple Silicon macOS DMG. The app is thin: model Assets are downloaded on first run into app-support storage, not bundled into the release artifact.

## Required GitHub secrets

Configure these repository secrets before running the release workflow:

- `APPLE_CERTIFICATE`: base64 encoded Developer ID Application `.p12` certificate.
- `APPLE_CERTIFICATE_PASSWORD`: password for the `.p12` certificate.
- `KEYCHAIN_PASSWORD`: temporary CI keychain password.
- `APPLE_API_KEY`: App Store Connect API key id.
- `APPLE_API_ISSUER`: App Store Connect issuer UUID.
- `APPLE_API_KEY_PRIVATE_KEY`: contents of the `AuthKey_<APPLE_API_KEY>.p8` private key.

The release workflow fails before building if any of these secrets are missing.

## Apple Developer prerequisites

A public notarized DMG requires an Apple Developer Program account.

At a high level:

1. Create or use an Apple Developer Program team.
2. Create a **Developer ID Application** certificate.
3. Export the certificate and private key as a password-protected `.p12` file.
4. Base64 encode the `.p12` file and store it as `APPLE_CERTIFICATE`.
5. Create an App Store Connect API key with access to notarization.
6. Store the key id, issuer UUID, and `.p8` private-key contents in the GitHub secrets above.

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

The workflow fails before building if any signing/notarization secret is missing. Add the missing secret in GitHub repository settings and rerun the workflow.

### Certificate import fails

Confirm the `.p12` is a Developer ID Application certificate, not an Apple Development certificate, and that `APPLE_CERTIFICATE_PASSWORD` matches the export password.

### No Developer ID identity is found

Confirm the exported `.p12` includes the private key and that the certificate name includes `Developer ID Application`.

### Notarization fails

Check the App Store Connect API key id, issuer UUID, and private key content. The key must have access to notarization for the Apple Developer team.

### DMG is missing

On a machine with release signing and notarization environment configured, run:

```bash
pnpm --dir apps/desktop tauri build --target aarch64-apple-darwin --bundles app,dmg
```

If Tauri changes the emitted DMG path, update `scripts/desktop-release-build.sh` and `scripts/check-github-workflows.sh`.

### Thin-bundle check fails

The release artifact must not bundle model binaries, GGUF files, retired sidecars, or GGML dylibs in `Contents/Resources`. Re-run:

```bash
pnpm check:macos-thin-bundle
```

and remove any accidental bundled model/sidecar assets before tagging.
