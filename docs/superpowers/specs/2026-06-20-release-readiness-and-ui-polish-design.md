# Release Readiness and UI Polish Design

## Status

Draft for user review. This is the next phase after the in-process inference re-architecture. Phase 7 streaming remains deferred because it is optional product expansion, not required for release readiness.

## Goals

- Prepare Wispergo for public GitHub Releases that serve both end users and contributors.
- Ship a trusted macOS binary release path: signed, notarized, and packaged as a user-installable artifact.
- Make first-run setup understandable for non-developer users, including permissions and required model downloads.
- Improve the core recording visual experience with a polished waveform state.
- Replace low-contrast app and menu bar icons with release-quality macOS assets.
- Keep the developer path simple: clone, install, test, build, release docs.

## Non-goals

- Live streaming partial transcripts.
- Changing the final dictation, cleanup, or insertion pipeline.
- Cloud inference.
- Intel Mac support.
- Building a marketing website.
- Adding a cleanup model picker.

## Release posture

The target public release path is a signed and notarized macOS artifact distributed through GitHub Releases.

Recommended artifact:

- Primary: `.dmg` containing `Wispergo.app`.
- Optional secondary: `.zip` for developers or automated checks.

Unsigned local builds remain supported for contributors through `pnpm desktop:build`, but GitHub Release binaries should be Developer ID signed and notarized. This likely requires an Apple Developer Program account and release secrets in GitHub Actions.

Expected secrets, exact names to be finalized during implementation:

- Apple Developer ID signing certificate, base64 encoded.
- Certificate password.
- Apple notarization credentials, preferably App Store Connect API key based credentials.
- Team ID or issuer/key IDs as required by the notarization flow.

## Current state summary

Already working:

- Thin app bundle with in-process `whisper-rs` and `llama-cpp-2` inference.
- Asset Manifest bundled in the app.
- Default and optional model Assets resolved from app-support storage.
- Download and repair commands for default Assets.
- Settings UI shows model download status and retries failures.
- Local build wrapper sets macOS deployment target and local signing identity.

Needs release hardening:

- The release artifact is currently an `.app` bundle target, not a DMG release flow.
- First-run model download is tied to the settings surface. A fresh end user should get guided setup rather than discovering hidden settings.
- Permission and model readiness should be shown as a setup checklist before dictation is expected to work.
- Dock and menu bar icons need separate assets optimized for their sizes and backgrounds.
- The recording pill is functional but not release-polished.
- GitHub Actions release workflow and contributor docs are missing.

## User experience design

### First-run setup

On first launch, if required setup is incomplete, Wispergo should show the settings/setup window automatically. The floating recorder can remain available, but dictation should clearly report why it is not ready if the user tries the shortcut before setup completes.

Setup checklist:

1. Microphone permission.
2. Accessibility permission.
3. Required model download:
   - ASR `medium`.
   - Punctuation cleanup default model.
4. Optional preferences:
   - Recognition language.
   - ASR Accuracy Pack.
   - Full-cleanup Pack.

The default path should be one obvious action at a time. The app should explain that model downloads are one-time and speech recognition runs locally after setup.

Model download states:

- Ready: hide download noise or show a compact success state in setup.
- Missing: show required model name and size when available.
- Downloading: show current model name and progress if the downloader can expose bytes; otherwise show indeterminate progress with model name.
- Failed: show plain error, retry button, and a note to check network/storage.
- Corrupt: offer repair/re-download.

Release requirement: a clean install with no app-support models must lead a user to a working default setup without manual file placement.

### Recording waveform

When the shortcut is held and recording is active, replace the text-heavy expanded pill with a compact waveform surface.

Behavior:

- Idle collapsed state remains minimal.
- Hover/ready state may continue to show the current pill and language controls.
- Recording state shows animated waveform and a concise listening state.
- Processing state shows a separate visual state, not the waveform, so users know recording stopped and transcription/cleanup is running.
- Reduced motion users get a static or gently fading level indicator.

Preferred implementation: drive the waveform from real microphone amplitude samples emitted by the audio capture layer. If that is too large for the first slice, use a deterministic CSS fallback only as an intermediate PR and document that real amplitude is the follow-up.

Testing:

- Component tests for recording, processing, idle, and reduced-motion class/state behavior.
- Rust tests for any audio level event payloads if emitted from backend.
- Manual smoke for hold-to-dictate visual behavior.

### Icons

Use separate assets for app icon and menu bar/tray icon.

Dock/app icon:

- Full-color macOS app icon.
- Legible at Dock sizes.
- Better contrast than the current icon.
- Exported as the required icon inputs for Tauri and `.icns` generation.

Menu bar/tray icon:

- Separate monochrome or template-style icon.
- Legible at small sizes.
- Works in light and dark menu bar appearances.
- Avoid relying on color contrast that disappears against system chrome.

Implementation should update Tauri tray setup to use the menu bar specific icon rather than the app icon if necessary.

### Settings and setup UI polish

Settings should be reshaped into a release-ready setup/preferences surface:

- Top status: Ready, Setup needed, Downloading, or Error.
- Setup checklist for permissions and model readiness.
- Model settings grouped separately from permissions.
- Optional packs clearly marked optional.
- Fewer repeated paragraphs, more state-specific copy.
- Keep native form controls unless a custom control improves clarity.

## GitHub Actions release flow

### CI workflow

For every pull request:

- Install Rust and Node/pnpm.
- Cache dependencies where safe.
- Run:
  - `cargo build --workspace`
  - `cargo test --workspace`
  - `cargo clippy -p wispergo-core --all-targets -- -D warnings`
  - `cargo clippy -p wispergo-core --all-targets --features llama-cpp -- -D warnings`
  - `cargo clippy -p wispergo-desktop --all-targets -- -D warnings`
  - `pnpm test:ts`

### Release workflow

Trigger on version tags such as `v0.1.0`.

Steps:

1. Checkout source.
2. Install Rust, Node, pnpm, Tauri prerequisites.
3. Import Developer ID signing certificate from secrets.
4. Build release artifact for Apple Silicon macOS.
5. Sign app and package DMG.
6. Notarize and staple the artifact.
7. Run thin-bundle check.
8. Upload DMG and optional ZIP to GitHub Release.
9. Publish release notes with install instructions, model download note, and troubleshooting links.

The workflow should fail closed if signing or notarization is requested but secrets are missing. A separate manual unsigned artifact workflow can be considered later for contributors, but it should not be the default public release.

## Documentation requirements

README should have clear split sections:

- Download and install for end users.
- First-run setup and model downloads.
- Privacy/local-first note.
- Developer setup.
- Release process for maintainers.
- Troubleshooting Gatekeeper, permissions, downloads, and insertion.

Add or update:

- `CONTRIBUTING.md` for developer workflow.
- Release checklist, either in README or `docs/release.md`.
- Screenshots or GIFs after the UI polish lands.

## Implementation slices

### Slice 1: Release readiness roadmap and product context

Deliverables:

- `PRODUCT.md`.
- This design spec.
- Roadmap/HANDOFF updates.

Verification:

- Docs self-review for consistency and scope.

### Slice 2: First-run setup and model readiness UX

Deliverables:

- Auto-show setup window when permissions or required default Assets are missing.
- Setup checklist in settings/main surface.
- Clear dictation-not-ready behavior if the shortcut is used before setup completes.
- Tests for readiness states and setup window behavior.

Verification:

- `pnpm test:ts`.
- `cargo test -p wispergo-desktop` if backend readiness helpers change.
- Manual clean-app-support smoke.

### Slice 3: Icon refresh

Deliverables:

- New app icon assets.
- Separate menu bar/tray icon assets.
- Tauri config/tray setup updated.
- Light and dark menu bar smoke.

Verification:

- `pnpm desktop:build`.
- Visual smoke of Dock and menu bar icons.

### Slice 4: Recording waveform UI

Deliverables:

- Recording state uses waveform surface instead of text-heavy pill.
- Processing and idle states remain distinct.
- Reduced-motion behavior.
- Component tests.

Verification:

- `pnpm test:ts`.
- Manual hold-to-dictate smoke.

### Slice 5: CI and release workflow

Deliverables:

- PR CI workflow.
- Tag-based release workflow.
- DMG target and release artifact naming.
- Signing/notarization secret documentation.
- Release checklist docs.

Verification:

- CI workflow passes on PR.
- Release workflow dry-run or manual tag test, depending on secret availability.

### Slice 6: Public README and contributor docs

Deliverables:

- End-user install instructions.
- First-run model download explanation.
- Developer setup and contribution flow.
- Release maintenance instructions.

Verification:

- Link/path review.
- Fresh clone command review.

## Open questions

- Exact final icon artwork source and ownership.
- Whether to expose byte-level download progress before release or keep model-name level progress for the first public release.
- Whether the first public release should include both DMG and ZIP.
- Which Apple notarization credential style to use in GitHub Actions.

## Recommendation

Start with Slice 2 after this spec is approved: first-run setup and model readiness UX. It is the highest-value release blocker because a user who downloads the app must be able to reach a working default setup without knowing anything about models or app-support folders.
