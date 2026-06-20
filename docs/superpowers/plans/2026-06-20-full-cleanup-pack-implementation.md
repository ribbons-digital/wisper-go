# Phase 5.3 Full-cleanup Pack Implementation Plan

**Date:** 2026-06-20  
**Branch:** `phase-5-3-full-cleanup-pack`  
**Parent design:** `docs/superpowers/specs/2026-06-19-model-tiering-phase-5-design.md`  
**Roadmap slice:** Phase 5.3 — Full-cleanup Pack (3B) opt-in

## Goal

Make **Cleanup Mode = Full cleanup and commands** an opt-in Model Pack backed by
the manifest-driven Asset downloader. Selecting Full cleanup downloads and
verifies the 3B `cleanup_full` Asset before settings are persisted. If download
or verification fails, previous settings remain active. Punctuation-only remains
independent and must not download or require the 3B pack.

## Non-goals

- Do not redesign Full cleanup prompts, JSON parsing, command behavior, or the
  Phase 5.2 punctuation safety gate.
- Do not add a visible cleanup model picker.
- Do not make the 3B Full-cleanup Pack part of first-run/default downloads.
- Do not remove bundled resource paths; that remains Phase 6.
- Do not change Ollama dev override behavior.

## Asset metadata

Add this manifest entry:

```json
{
  "id": "qwen2.5-3b-instruct",
  "role": "cleanup_full",
  "displayName": "Qwen2.5 3B Full Cleanup",
  "url": "https://huggingface.co/Qwen/Qwen2.5-3B-Instruct-GGUF/resolve/main/qwen2.5-3b-instruct-q4_k_m.gguf",
  "size": 2104932768,
  "sha256": "626b4a6678b86442240e33df819e00132d3ba7dddfe1cdc4fbb18e0a9615c62d",
  "default": false
}
```

`default: false` is required so the Full-cleanup Pack is not downloaded during
first-run/default asset repair. The pack is downloaded only when the user selects
Full cleanup.

## Current code facts

- `AssetRole::CleanupFull` already exists in `wispergo-core`.
- `resolve_cleanup_model_path_for_settings` already maps
  `CleanupMode::FullCleanup` to `AssetRole::CleanupFull`.
- Today `selected_cleanup_asset` requires `default: true` for all cleanup roles;
  this must change for `cleanup_full` because the Full-cleanup Pack is opt-in and
  non-default.
- `ensure_assets_for_settings` currently ensures only the selected ASR Asset.
- `managed_cleanup_runtime_enabled_for_backend` disables local cleanup when
  `WISPERGO_CLEANUP_BACKEND=ollama`; download-before-activate must preserve that
  dev override and must not require local cleanup Assets in Ollama mode.
- `set_local_model_settings` already calls `ensure_assets_for_settings` before
  saving/persisting settings, so adding cleanup-full ensure there preserves the
  previous-setting-on-failure guarantee.

## Desired behavior

| Setting change | Required downloads before save | Failure behavior |
|---|---|---|
| ASR tier only | selected ASR | save fails, previous settings remain active |
| Punctuation-only cleanup | selected ASR only | missing/corrupt punctuation asset later falls back to raw ASR |
| Full cleanup | selected ASR + Full-cleanup Pack, unless `WISPERGO_CLEANUP_BACKEND=ollama` | save fails, previous cleanup mode remains active |
| Cleanup off | selected ASR only | save fails only for ASR failure |

Runtime resolution after successful activation:

- Punctuation-only resolves verified app-support `cleanup_punctuation` Asset.
- Full cleanup resolves verified app-support `cleanup_full` Asset.
- Empty manifest dev bridge may still use bundled cleanup path until Phase 6.
- Full cleanup must not use the punctuation Asset.

## Task 1 — Verify artifact metadata, then manifest and selector tests first

Files:

- `apps/desktop/src-tauri/resources/models.manifest.json`
- `apps/desktop/src-tauri/src/commands/settings.rs`
- `/tmp/wispergo-phase-5-3-artifact-verification.txt` (local evidence, not committed)

Before editing the manifest, verify the 3B artifact metadata from a local file or
fresh download. Preferred commands if the eval/downloaded file is available:

```bash
MODEL=/path/to/qwen2.5-3b-instruct-q4_k_m.gguf
stat -f '%z' "$MODEL"
shasum -a 256 "$MODEL"
```

If no local copy exists, download to `/tmp/wispergo-model-eval/` and verify:

```bash
mkdir -p /tmp/wispergo-model-eval
curl -L --fail --retry 3 \
  -o /tmp/wispergo-model-eval/qwen2.5-3b-instruct-q4_k_m.gguf \
  https://huggingface.co/Qwen/Qwen2.5-3B-Instruct-GGUF/resolve/main/qwen2.5-3b-instruct-q4_k_m.gguf
stat -f '%z' /tmp/wispergo-model-eval/qwen2.5-3b-instruct-q4_k_m.gguf
shasum -a 256 /tmp/wispergo-model-eval/qwen2.5-3b-instruct-q4_k_m.gguf
```

Expected:

- size: `2104932768`
- SHA-256: `626b4a6678b86442240e33df819e00132d3ba7dddfe1cdc4fbb18e0a9615c62d`

Save the verification commands/output to
`/tmp/wispergo-phase-5-3-artifact-verification.txt` and cite it in the PR body.
Do not commit the model file or the `/tmp` evidence file.

Then add failing tests in `settings.rs`:

1. `full_cleanup_uses_verified_app_support_full_asset_when_manifest_populated`
   - Build a manifest containing one `cleanup_full` Asset with `default: false`.
   - Create its app-support file.
   - Set `cleanup_mode: FullCleanup`.
   - Assert `resolve_cleanup_model_path_for_settings` returns the app-support
     `cleanup_full` path.

2. `full_cleanup_missing_full_asset_reports_unavailable_path_error`
   - Same manifest but no file.
   - Assert error contains
     `full cleanup asset 'qwen2.5-3b-instruct' is not downloaded yet`.

3. Adjust or replace the existing
   `full_cleanup_does_not_use_punctuation_asset_when_manifest_populated` test so
   it proves Full cleanup does not fall back to the punctuation Asset when only a
   punctuation Asset exists.

Expected first run:

```bash
cargo test -p wispergo-desktop --lib full_cleanup_
```

Expected: FAIL because `selected_cleanup_asset` still requires `default: true`
for `cleanup_full`.

Implementation:

- Add the manifest entry above.
- Replace `selected_cleanup_asset(manifest, role)` with role-specific selection:
  - `CleanupPunctuation`: continue requiring the role default.
  - `CleanupFull`: select the sole/first `cleanup_full` pack by role even when
    `default: false`.
  - Error for no `cleanup_full` remains
    `no default full cleanup asset is configured` or is updated to a clearer
    `no full cleanup asset is configured`; tests should match the chosen copy.
- Keep `default` invariant unchanged: at most one default per role.

Verification:

```bash
cargo test -p wispergo-desktop --lib full_cleanup_
cargo test -p wispergo-core asset_manifest
```

Commit:

```bash
git commit -m "feat(desktop): add full cleanup asset selection"
```

## Task 2 — Download-before-activate for Full cleanup

Files:

- `apps/desktop/src-tauri/src/commands/settings.rs`

Refactor `ensure_assets_for_settings` to ensure all Assets required by the
candidate settings before saving:

- Always ensure selected ASR.
- If managed local cleanup is enabled and
  `settings.cleanup_mode == CleanupMode::FullCleanup`, also ensure the selected
  `cleanup_full` Asset.
- If `WISPERGO_CLEANUP_BACKEND=ollama`, preserve the dev override: require ASR
  only, even when the requested cleanup mode is Full cleanup.
- Do not ensure `cleanup_punctuation` for Punctuation-only in this slice; Phase
  5.2 intentionally allows raw fallback if punctuation is missing/corrupt.
- Keep emitting the existing `ASSET_DOWNLOAD_EVENT` statuses around each repair.

Suggested helper shape:

```rust
fn required_assets_for_settings<'a>(
    manifest: &'a AssetManifest,
    settings: &LocalModelSettings,
    cleanup_backend: Option<&str>,
) -> Result<Vec<&'a AssetEntry>, String>
```

or equivalent. Avoid over-abstraction if a small helper is clearer.

Tests:

1. Add a focused unit test that required-asset selection includes ASR only for
   Punctuation-only and ASR + cleanup_full for normal Full cleanup.
2. Add a focused unit test that Full cleanup with `cleanup_backend = Some("ollama")`
   requires ASR only and does not require/download the local `cleanup_full` Asset.
3. If practical without a full Tauri app harness, add unit tests around the
   selection helper rather than network download plumbing.
4. Existing `set_local_model_settings` ordering already gives previous-setting
   retention because `ensure_assets_for_settings` runs before
   `state.set_local_model_settings`; do not move that line.

Verification:

```bash
cargo test -p wispergo-desktop --lib required_assets
cargo test -p wispergo-desktop --lib cleanup_
```

Commit:

```bash
git commit -m "feat(desktop): download full cleanup pack before activation"
```

## Task 3 — Frontend copy/tests for Full cleanup opt-in pack

Files:

- `apps/desktop/src/features/settings/SettingsPanel.tsx`
- `apps/desktop/src/features/settings/SettingsPanel.test.tsx`

Current UI already exposes `Full cleanup and commands` and save delegates to the
Tauri settings command. Only change UI copy if it helps users understand the
large opt-in pack.

Preferred minimal change:

- Add a short note near Cleanup Mode, such as:
  `Full cleanup downloads the optional 3B Full-cleanup Pack before activation.`

Tests:

- Add/adjust one SettingsPanel test asserting the note appears.
- Preserve existing save behavior test for `cleanupMode: "full_cleanup"`.

Verification:

```bash
pnpm test:ts -- SettingsPanel.test.tsx
```

After any `pnpm` command, check and revert the out-of-scope Corepack edit:

```bash
git diff -- package.json
git checkout -- package.json # only if packageManager was added
```

Commit:

```bash
git commit -m "docs(ui): clarify full cleanup pack activation"
```

If no UI copy change is needed after inspection, skip this task and document why
in the final PR body.

## Task 4 — Roadmap and handoff refresh

Files:

- `docs/superpowers/plans/2026-06-18-in-process-inference-roadmap.md`
- `HANDOFF.md`

Updates:

- Mark Phase 5.2 as merged via PR #15 / complete, not locally complete.
- Mark Phase 5.3 as locally complete once implementation and verification pass
  but before PR merge.
- Record Full-cleanup Pack behavior:
  - 3B `cleanup_full` manifest entry, `default: false`.
  - Full cleanup downloads/verifies the pack before activation.
  - Previous settings remain active on failure.
  - Punctuation-only unaffected.
- Preserve standing warnings:
  - Do not use `librarian` for this project.
  - Desktop clippy is part of the PR gate.
  - Revert `package.json` `packageManager` edits after `pnpm test:ts`.

Verification:

```bash
rg -n "Phase 5\.2 is implemented locally|PR needed|phase-5-2-cleanup-punctuation-default" HANDOFF.md docs/superpowers/plans/2026-06-18-in-process-inference-roadmap.md
```

Expected after edits: no stale matches except historical references only if they
are explicitly described as already merged.

Commit:

```bash
git commit -m "docs: update full cleanup pack status"
```

## Task 5 — Full verification and PR

Run the full gate:

```bash
cargo build --workspace
cargo test --workspace
cargo clippy -p wispergo-core --all-targets -- -D warnings
cargo clippy -p wispergo-core --all-targets --features llama-cpp -- -D warnings
cargo clippy -p wispergo-desktop --all-targets -- -D warnings
pnpm test:ts
git diff -- package.json
```

If `package.json` only has an out-of-scope `packageManager` edit, revert it:

```bash
git checkout -- package.json
```

Then push and open PR:

```bash
git push -u origin phase-5-3-full-cleanup-pack
cat > /tmp/wispergo-phase-5-3-pr.md <<'PR'
# Summary
- Add the Qwen2.5 3B Full-cleanup Pack as a manifest `cleanup_full` Asset.
- Download/verify Full-cleanup Pack before activating Full cleanup.
- Keep previous settings active on failure; Punctuation-only remains unaffected.

# Verification
- [ ] cargo build --workspace
- [ ] cargo test --workspace
- [ ] cargo clippy -p wispergo-core --all-targets -- -D warnings
- [ ] cargo clippy -p wispergo-core --all-targets --features llama-cpp -- -D warnings
- [ ] cargo clippy -p wispergo-desktop --all-targets -- -D warnings
- [ ] pnpm test:ts
PR
gh pr create --title "feat(desktop): add full cleanup pack" --body-file /tmp/wispergo-phase-5-3-pr.md
```

Do not merge the PR. Wait for user merge, then sync `main` and clean the branch.

## Review checkpoints

Use subagents for implementation slices. After each task:

- Run the task-specific verification.
- Request a review against this plan and the Phase 5 design.
- Fix review findings before moving to the next task.

Final acceptance before PR:

- 3B artifact size/SHA are verified and cited in PR evidence.
- `models.manifest.json` has one `cleanup_full` 3B entry with `default: false`.
- Full cleanup resolves verified app-support 3B Asset.
- Settings activation downloads/verifies Full-cleanup Pack before saving.
- `WISPERGO_CLEANUP_BACKEND=ollama` preserves ASR-only requirements and does not
  require local cleanup Assets.
- Punctuation-only does not download or require Full-cleanup Pack.
- Full verification gate passes.
- Working tree is clean except committed branch changes.
