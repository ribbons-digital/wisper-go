# Phase 5 Design Draft: Model Tiering + Readiness

**Date:** 2026-06-19  
**Status:** Approved by user on 2026-06-19 for Phase 5.1 implementation.  
**Roadmap slice:** Phase 5 in `docs/superpowers/plans/2026-06-18-in-process-inference-roadmap.md`.

## Entry gate

This is the design/scoping gate for Phase 5. Phases 2–4 moved inference
in-process and added `InferenceManager`; Phase 5 chooses real downloadable model
Assets, wires settings to model tier selection, and enforces readiness/download
before activation. No implementation should start until this draft is reviewed
and approved.

## Sources

- Roadmap: `docs/superpowers/plans/2026-06-18-in-process-inference-roadmap.md`
- Approved architecture: `docs/superpowers/specs/2026-06-18-in-process-inference-and-asset-downloader-design.md`
- Glossary: `CONTEXT.md`
- Asset manifest: `crates/wispergo-core/src/asset_manifest.rs`
- Asset storage: `crates/wispergo-core/src/asset_storage.rs`
- Downloader: `crates/wispergo-core/src/downloader.rs`
- Desktop asset commands: `apps/desktop/src-tauri/src/commands/assets.rs`
- Settings/state: `apps/desktop/src-tauri/src/commands/settings.rs`, `apps/desktop/src-tauri/src/state.rs`
- Settings UI/types: `apps/desktop/src/types/pipeline.ts`, `apps/desktop/src/features/settings/SettingsPanel.tsx`
- Cleanup eval gate: `docs/manual/offline-cleanup-eval.md`
- Public model-source check: HuggingFace search confirmed `ggerganov/whisper.cpp` provides `ggml-medium-q5_0.bin`, `ggml-medium.bin`, and `ggml-large-v3-turbo.bin`; Qwen/Qwen2.5 GGUF repos exist for 0.5B/1.5B and community single-quant Q4_K_M repos also exist. Exact URLs, byte sizes, and SHA-256 values must be pinned by implementation-time artifact verification, not copied from search snippets.

## Current state

- `models.manifest.json` is still an empty placeholder.
- Downloader/storage support manifest-driven Assets under app-support:
  - ASR: `{models_root}/asr/{id}.bin`
  - cleanup: `{models_root}/cleanup/{id}.gguf`
- `ensure_model_assets` downloads manifest defaults, but with the current empty
  manifest it is a no-op.
- Live `InferenceManager` still resolves from bundled resource paths unless env
  overrides are set.
- Frontend settings expose recognition language and cleanup mode, but not ASR
  model tier selection.

## Phase 5 goals

1. Populate the Asset Manifest with real downloadable model Assets.
2. Make app-support Asset paths the live source for selected ASR/cleanup models.
3. Add ASR tier selection: default medium-quality asset plus Accuracy Pack.
4. Decide the cleanup punctuation default through the eval gate.
5. Make Full cleanup require the Full-cleanup Pack.
6. Download before activation: failed or missing pack downloads do not silently
   switch active settings.

## Non-goals

- Do not delete bundled resource trees/scripts yet; Phase 6 owns that.
- Do not drop Intel-target path logic yet; Phase 6 owns that.
- Do not redesign the settings UI beyond the model-tier controls/readiness text
  needed for Phase 5.
- Do not remove Ollama dev override.
- Do not implement streaming/partial transcripts.
- Do not optimize lower-level persistent llama model ownership unless required
  for correctness; Phase 5 is about model tiering/readiness.

## Recommended decisions

### 1. Split Phase 5 into three review-gated PRs

**Decision:** Keep the roadmap's split:

1. **5.1 ASR tiering** — add ASR manifest entries, `asrModelId`, Accuracy Pack
   selection, app-support ASR resolution, and ASR readiness/download-before-
   activate.
2. **5.2 Cleanup punctuation default eval** — run/evaluate 0.5B; choose 0.5B or
   1.5B as the cleanup-punctuation default; add manifest entry and live
   cleanup-punctuation app-support resolution.
3. **5.3 Full-cleanup Pack** — add 3B cleanup-full entry and make Full cleanup
   download-before-activate / unavailable until verified.

**Why:** The ASR tiering path is mostly plumbing; cleanup default selection has a
quality stop-rule; Full cleanup is a separate pack activation path. Keeping them
separate avoids burying model-quality decisions inside UI/storage work.

### 2. Use quantized `medium` as the ASR default

**Decision:** Keep the user-facing/default ASR tier id as `medium`, but point it
to the quantized whisper.cpp medium artifact:

- id: `medium`
- role: `asr`
- candidate artifact: `ggml-medium-q5_0.bin`
- source repo candidate: `https://huggingface.co/ggerganov/whisper.cpp`
- default: `true`

**Why:** The approved design said `medium` should be ~480 MB. Public model-source
checks show full `ggml-medium.bin` is ~1.5 GiB, while `ggml-medium-q5_0.bin` is
~514 MiB. The quantized artifact preserves the intended default download budget
and multilingual Auto/EN/ZH behavior.

**Implementation rule:** The manifest stores the stable id `medium`; the URL can
point to `.../resolve/main/ggml-medium-q5_0.bin`. The local file path remains
`asr/medium.bin` because storage derives filename from id + role extension.

### 3. Use full `large-v3-turbo` as the Accuracy Pack

**Decision:** Add the Accuracy Pack ASR Asset:

- id: `large-v3-turbo`
- role: `asr`
- candidate artifact: `ggml-large-v3-turbo.bin`
- source repo candidate: `https://huggingface.co/ggerganov/whisper.cpp`
- default: `false`

**Why:** This matches the roadmap's “maximum accuracy at cost of size/latency”
intent. A quantized turbo can be considered later, but Phase 5 should keep the
first tiering decision simple: compact default, best-available opt-in.

### 4. Pin byte sizes and SHA-256 by local artifact verification

**Decision:** Implementation must not paste approximate sizes or search-result
hashes into the manifest. It should download each candidate artifact, compute:

- exact byte size
- exact SHA-256 of downloaded bytes
- final direct URL used by `reqwest`

Then commit those exact values to `models.manifest.json`.

**Why:** Existing downloader verifies SHA-256. HuggingFace/Xet pages can show
metadata that is not the final file checksum expected by our downloader, and
README tables may use non-SHA-256 hashes. The manifest is an executable trust
boundary; values must be computed from the exact bytes Wispergo downloads.

**Recommended implementation helper:** add a small local-only script, for example
`scripts/pin-model-asset.sh <id> <role> <url>`, that downloads to a temp file and
prints JSON fields (`size`, `sha256`). The script can be used during development
without becoming runtime code.

### 5. After manifest population, live resolution uses app-support Assets first

**Decision:** Once a real manifest entry exists for a selected model, the live
`InferenceManager` config should resolve to app-support `AssetStorage` paths,
not bundled resource paths.

Resolution priority:

1. explicit env override (`WISPERGO_WHISPER_MODEL`, `WISPERGO_CLEANUP_BACKEND=ollama`);
2. selected/role default Asset in app-support storage if present and verified;
3. unavailable/downloading state with clear message.

Bundled resource fallback is allowed only when the manifest is empty (dev bridge)
until Phase 6 removes bundled assets.

**Why:** If bundled resources silently win after manifest population, the app
will appear to work while the thin-app/downloader path is broken. Phase 5 must
make downloaded Assets the live source.

### 6. Add `asrModelId`; keep cleanup model id implicit for now

**Decision:** Extend `LocalModelSettings` with:

```rust
asr_model_id: String // default "medium"
```

Frontend equivalent:

```ts
asrModelId: "medium" | "large-v3-turbo" | string
```

Do **not** add a visible cleanup model selector yet. Cleanup model selection is
implicit:

- Cleanup Mode = Punctuation-only → role default `cleanup_punctuation` Asset.
- Cleanup Mode = Full cleanup → role default/pack Asset for `cleanup_full`.

**Why:** ASR tiering is a real user preference. Cleanup punctuation model choice
is an implementation/eval decision; Full cleanup is already represented by
Cleanup Mode.

### 7. Download before activation for settings changes

**Decision:** `set_local_model_settings` should become the activation boundary.
When new settings require an absent/corrupt Asset:

1. emit asset-download status events;
2. download/repair the required Asset;
3. verify it;
4. only then persist settings and re-arm `InferenceManager`;
5. on failure, return an error and keep previous settings active.

This applies to:

- switching ASR to Accuracy Pack;
- switching back to medium if medium somehow missing/corrupt;
- selecting Full cleanup when the Full-cleanup Pack is absent.

**Why:** The approved design says previous selection remains active until the new
selection is verified. This avoids saving settings that point to missing Assets.

### 8. Auto-download default Assets on first run

**Decision:** With a populated manifest, default Assets should start downloading
without requiring a manual settings click.

Recommended path:

- app setup or early frontend boot calls `ensure_model_assets` for defaults;
- progress continues to use `wispergo://asset-download` events;
- `asset_readiness` remains the read-only status command;
- `ensure_model_assets` remains idempotent and safe to call repeatedly.

**Important UI correction:** Current `AssetDownloadNotice` treats `downloading`
status from `asset_readiness` as if a download is already happening. Phase 5.1
must either:

- make the frontend call `ensureModelAssets()` when readiness reports missing/
  downloading, or
- introduce a distinct `missing`/`needed` status before download begins.

Recommendation: add an explicit `missing` status for readiness clarity, while
keeping `downloading` for active transfer events.

### 9. ASR missing/corrupt is hard-blocking; cleanup missing/corrupt is best-effort

**Decision:** Preserve documented failure asymmetry:

- ASR selected Asset missing/corrupt/downloading → dictation unavailable with a
  clear message.
- Cleanup punctuation Asset missing/corrupt/downloading → raw ASR fallback.
- Full cleanup Asset missing/corrupt/downloading → Full cleanup unavailable with
  clear message; Punctuation-only remains unaffected.

**Why:** This matches existing domain language and prevents cleanup pack issues
from blocking basic dictation.

### 10. Run cleanup 0.5B eval before accepting the cleanup default

**Decision:** Phase 5.2 must run `docs/manual/offline-cleanup-eval.md` against
Qwen2.5-0.5B-Instruct Q4_K_M before it becomes the punctuation default.

Accepted outcomes:

- If 0.5B passes all cases without material regression, set it as the
  `cleanup_punctuation` default.
- If 0.5B fails, evaluate Qwen2.5-1.5B-Instruct Q4_K_M and set 1.5B as default
  if it passes.
- Do not accept a cleanup punctuation default until the eval table is filled.

**Why:** This is an explicit roadmap stop-rule, especially for Chinese and mixed
English/Chinese punctuation quality.

## Proposed implementation details by slice

### Phase 5.1 — ASR medium default + Accuracy Pack

Files likely touched:

- `apps/desktop/src-tauri/resources/models.manifest.json`
- `crates/wispergo-core/src/asset_manifest.rs` tests if any defaults/role helper
  behavior needs tightening
- `apps/desktop/src-tauri/src/commands/assets.rs`
  - add clearer readiness status and/or targeted download helper
- `apps/desktop/src-tauri/src/commands/settings.rs`
  - add `asr_model_id`, download-before-activate, app-support path resolution
- `apps/desktop/src-tauri/src/state.rs`
  - settings schema/default
- `apps/desktop/src/types/pipeline.ts`
- `apps/desktop/src/features/settings/SettingsPanel.tsx`
  - ASR model select: `Medium` and `Accuracy Pack (large-v3-turbo)`
- tests for Rust settings/download behavior and TS settings UI

Definition of done:

- Manifest contains verified ASR entries for `medium` and `large-v3-turbo`.
- `medium` is the default ASR Asset.
- First-run/default download includes `medium`.
- App-support `asr/medium.bin` is the live ASR path after manifest population.
- `LocalModelSettings` round-trips `asrModelId` with default `medium`.
- Switching ASR tier downloads/verifies before saving and leaves previous
  setting active on failure.
- Recognition language Auto/EN/ZH still maps into the selected ASR model.
- Empty-manifest dev bridge still works until Phase 6.

### Phase 5.2 — cleanup punctuation eval/default

Files likely touched:

- `docs/manual/offline-cleanup-eval.md` — filled eval table for 0.5B and, if
  needed, 1.5B
- `apps/desktop/src-tauri/resources/models.manifest.json`
- `apps/desktop/src-tauri/src/commands/settings.rs`
  - cleanup punctuation role default resolution via AssetStorage
- `apps/desktop/src-tauri/src/commands/recording.rs` / manager wiring only if
  status/fallback messaging needs tightening
- tests for missing cleanup fallback and app-support cleanup path selection

Definition of done:

- Eval table documents model tested, outputs, and pass/fail decision.
- Manifest contains exactly one `default: true` `cleanup_punctuation` Asset.
- Default cleanup Asset is downloaded with defaults after ASR 5.1 path exists.
- Punctuation-only cleanup uses app-support cleanup Asset when present.
- Missing/corrupt/downloading cleanup punctuation falls back to raw ASR.
- Punctuation-only does not require Full-cleanup Pack.

### Phase 5.3 — Full-cleanup Pack

Files likely touched:

- `apps/desktop/src-tauri/resources/models.manifest.json`
- `apps/desktop/src-tauri/src/commands/settings.rs`
- `apps/desktop/src/features/settings/SettingsPanel.tsx`
- tests for Full cleanup pack download-before-activate and blocked state

Definition of done:

- Manifest contains verified `cleanup_full` Asset for Qwen2.5-3B-Instruct Q4_K_M.
- Selecting Cleanup Mode = Full cleanup downloads/verifies Full-cleanup Pack
  before activation.
- If Full-cleanup Pack download/verification fails, previous cleanup mode remains
  active and user receives a clear error/status.
- Punctuation-only mode remains unaffected by missing Full-cleanup Pack.
- Full cleanup uses the app-support 3B Asset when verified.

## Verification before each PR

Now that PR #13 cleaned desktop clippy, use the expanded gate:

- `cargo build --workspace`
- `cargo test --workspace`
- `cargo clippy -p wispergo-core --all-targets -- -D warnings`
- `cargo clippy -p wispergo-core --all-targets --features llama-cpp -- -D warnings`
- `cargo clippy -p wispergo-desktop --all-targets -- -D warnings`
- `pnpm test:ts`

For 5.2 additionally require manual eval evidence in
`docs/manual/offline-cleanup-eval.md` before PR.

## Review questions for approval

1. Do you approve keeping Phase 5 split into 5.1 ASR tiering, 5.2 cleanup eval
   default, and 5.3 Full-cleanup Pack?
2. Do you approve using the quantized `ggml-medium-q5_0.bin` artifact behind the
   stable user-facing/default id `medium` to preserve the intended ~0.5 GB
   default ASR size?
3. Do you approve download-before-activate semantics for ASR tier changes and
   Full cleanup selection, keeping previous settings active on failure?
4. Do you approve adding a clearer asset readiness state (`missing`/`needed`) or
   equivalent auto-download behavior so first-run defaults do not show a fake
   in-progress “downloading” state before a download has actually started?
5. Do you approve deferring visible cleanup model selection and making cleanup
   model choice implicit from Cleanup Mode and manifest role defaults?
