# Phase 4.2 Design Draft: Wire Recording/Settings to `InferenceManager`

**Date:** 2026-06-19  
**Status:** Approved by user on 2026-06-19 for Phase 4.2 implementation.  
**Roadmap slice:** Phase 4.2 in `docs/superpowers/plans/2026-06-18-in-process-inference-roadmap.md`.

## Entry gate

This is the design/scoping gate for Phase 4.2. Phase 4.1 added the desktop
`InferenceManager` lifecycle core and fake-engine state-machine tests. Phase 4.2
wires that manager into live app setup, settings sync, and recording while
preserving current user-visible behavior.

No implementation should start until this draft is reviewed and approved.

## Sources

- Roadmap: `docs/superpowers/plans/2026-06-18-in-process-inference-roadmap.md`
- Phase 4 design: `docs/superpowers/specs/2026-06-19-inference-manager-lifecycle-phase-4-design.md`
- Manager core: `apps/desktop/src-tauri/src/inference/manager.rs`
- Temporary cleanup bridge: `apps/desktop/src-tauri/src/inference/cleanup_runtime.rs`
- Recording pipeline: `apps/desktop/src-tauri/src/commands/recording.rs`
- Settings commands: `apps/desktop/src-tauri/src/commands/settings.rs`
- App setup/state wiring: `apps/desktop/src-tauri/src/lib.rs`
- In-process ASR provider: `crates/wispergo-core/src/whisper_rs_provider.rs`
- In-process cleanup provider: `crates/wispergo-core/src/cleanup_inprocess.rs`

## Slice goal

Replace live recording/settings usage of `CleanupRuntimeManager` and direct
per-recording local provider construction with `InferenceManager`.

The product behavior should remain the same:

- app launch arms configured local engines but does not load model contexts;
- first dictation loads ASR;
- local cleanup loads only when Cleanup Mode is not Off and Ollama override is
  not selected;
- Ollama override keeps bypassing local cleanup manager;
- cleanup failure/missing cleanup assets fall back to raw ASR;
- ASR failure/missing ASR assets remains a hard dictation error;
- frontend `cleanup_runtime_status` contract remains stable.

## Non-goals

- Do not change frontend UI/API names unless needed for compatibility; keep
  `cleanup_runtime_status` as the stable command.
- Do not move bundled model paths to app-support assets; Phase 5/6 own model
  tiering and thin-app asset migration.
- Do not add real manifest-selected model IDs yet.
- Do not remove the Ollama dev override.
- Do not optimize persistent llama context/KV-cache reuse beyond the approved
  backend+model lifecycle.
- Do not fix unrelated desktop clippy warnings unless they block the PR.

## Recommended decisions

### 1. Delete the temporary `CleanupRuntimeManager` bridge in this slice

**Decision:** Remove `apps/desktop/src-tauri/src/inference/cleanup_runtime.rs`
and replace its Tauri state with `InferenceManager`.

**Why:** Phase 3.3 explicitly kept `CleanupRuntimeManager` only as a temporary
frontend status bridge. Phase 4.1 now provides the real lifecycle state, and
Phase 4.2 is the planned handoff point.

**Compatibility:** Keep the `cleanup_runtime_status` Tauri command name. It will
read `inference_manager.cleanup().status()` and serialize into the same
`{ state, message }` shape.

### 2. Add concrete ASR and cleanup engine adapters for the manager

**Decision:** Implement manager-owned engine adapters in the desktop inference
layer, probably in `apps/desktop/src-tauri/src/inference/manager.rs` initially:

- `WhisperRsManagedEngine`
  - config: `AsrEngineConfig { model_path, language }`
  - on factory/load: construct `WhisperRsProvider::new(model_path).with_language(language).with_timeout(30s)`
  - on request: call `AsrProvider::transcribe`
- `LlamaCppManagedCleanupEngine`
  - config: `CleanupEngineConfig { model_path, mode }`
  - on factory/load: construct `LlamaCppCleanupProvider::new(LlamaCppCleanupConfig::new(model_path))`
  - on request: call `clean_punctuation_only` or `clean` based on mode

**Why:** This keeps 4.2 small and safe. The manager will own a loaded provider
instance and therefore stop rebuilding the provider for every recording. A deeper
refactor to persist lower-level `LlamaBackend` + `LlamaModel` can be a follow-up
if needed, but the live wiring should first prove manager ownership and lazy
load semantics end-to-end.

**Accepted deviation from Phase 4 ideal:** The Phase 4 design preferred cleanup
worker persistence at `LlamaBackend` + `LlamaModel` granularity. In current core,
`LlamaCppCleanupProvider` loads inside each completion call, so simply storing
the provider does not yet persist the model. There are two options:

- **Option A (recommended for 4.2):** wire through existing providers now, prove
  launch/request/status/settings behavior, then add lower-level persistent llama
  internals as a dedicated performance follow-up before or during Phase 5.
- **Option B:** include lower-level llama/whisper helper refactors in 4.2.

Recommendation: choose **Option A** to keep 4.2 focused on live wiring and avoid
mixing lifecycle integration with unsafe/self-referential llama internals.

### 3. Make manager requests callable from async recording without blocking Tokio

**Decision:** Keep `EngineRuntime::request` synchronous internally, but call it
from recording through `tauri::async_runtime::spawn_blocking` or an equivalent
blocking boundary.

**Why:** The manager workers are synchronous dedicated threads. Calling a sync
request from async command code should not block the async runtime worker thread
while waiting for ASR/cleanup.

### 4. Introduce a single settings-to-manager sync function

**Decision:** Replace `sync_cleanup_runtime_for_settings` with a broader sync
function, for example:

```rust
pub fn sync_inference_manager_for_settings(
    app: &AppHandle,
    inference_manager: &InferenceManager,
    settings: &LocalModelSettings,
)
```

Responsibilities:

- resolve bundled resource paths;
- arm ASR when an ASR model path can be resolved;
- disable or leave unavailable ASR with a clear message if missing;
- arm cleanup when `cleanup_mode != Off` and `WISPERGO_CLEANUP_BACKEND != ollama`;
- disable cleanup when Cleanup Mode is Off or Ollama override is selected;
- preserve status messages compatible with settings UI.

**Why:** Phase 4.2 should route both ASR language/model changes and cleanup mode
through one lifecycle owner. Current code only syncs cleanup on cleanup-mode
change; Phase 4.2 should also re-arm ASR when recognition language changes.

### 5. App setup arms, does not load

**Decision:** In `lib.rs`, manage `InferenceManager::new(...)` instead of
`CleanupRuntimeManager::default()`, then call `sync_inference_manager_for_settings`
after persisted settings load.

**Test expectation:** app/setup-level test should prove sync/arm creates ready
status with `loaded == false` and fake engine load count remains 0.

### 6. Recording routes ASR and local cleanup through manager

**Decision:** Change `stop_recording` / `process_recording` so ASR comes from
`InferenceManager.asr().request(...)`, not `local_asr_provider(...).transcribe`.

For cleanup:

- Cleanup Mode Off: no cleanup request.
- Ollama override: build/use `OllamaCleanupProvider` as today.
- Local punctuation/full cleanup: request `InferenceManager.cleanup()`.
- Cleanup manager error: fall back to raw ASR exactly as provider failures do
  today.

**Why:** This is the first live use of Phase 4.1 manager lifecycle.

### 7. Keep `cleanup_runtime_status` frontend shape stable

**Decision:** `cleanup_runtime_status` should continue returning camelCase
status with snake_case state values. The backing type can be `InferenceRuntimeStatus`
or a type alias/adapter.

Current frontend does not need to know whether status comes from the temporary
cleanup bridge or the manager.

### 8. Tests should use fake manager engines where possible

**Decision:** Add/adjust tests around manager wiring without requiring real GGUF
or Whisper model files.

Proposed test seams:

- manager fake-engine factories with load counters;
- helper functions that accept `&InferenceManager` and injected resource paths;
- recording pipeline tests that pass fake manager outputs/errors instead of real
  model providers where feasible.

## Proposed implementation files

- Modify: `apps/desktop/src-tauri/src/inference/manager.rs`
  - add concrete provider-backed engine adapters;
  - add async/blocking helper methods if useful;
  - optionally add status setter/arm-unavailable helper if missing resources need
    explicit unavailable status without request.
- Delete or stop exporting: `apps/desktop/src-tauri/src/inference/cleanup_runtime.rs`
- Modify: `apps/desktop/src-tauri/src/inference/mod.rs`
  - remove cleanup bridge export.
- Modify: `apps/desktop/src-tauri/src/lib.rs`
  - manage `InferenceManager`;
  - sync manager on setup;
  - shutdown manager on exit.
- Modify: `apps/desktop/src-tauri/src/commands/settings.rs`
  - replace cleanup runtime status/sync with inference manager status/sync;
  - re-arm on cleanup mode and recognition language/model settings changes.
- Modify: `apps/desktop/src-tauri/src/commands/recording.rs`
  - route ASR/local cleanup through manager;
  - preserve Ollama provider path.
- Update docs:
  - roadmap/handoff status.

## Proposed definition of done

- `CleanupRuntimeManager` temporary bridge is removed or no longer used in live
  app state.
- App setup arms ASR/cleanup according to persisted settings without loading
  fake engines/models.
- First dictation/request loads ASR via `InferenceManager`.
- Local cleanup request loads cleanup via `InferenceManager` only when Cleanup
  Mode is not Off and Ollama override is absent.
- Cleanup Mode Off disables/keeps cleanup unloaded.
- Ollama override bypasses local cleanup manager as today.
- Recognition language changes re-arm ASR with the new language and do not load
  until next dictation.
- Cleanup failures/missing cleanup assets still return raw ASR text.
- Missing ASR assets remain hard dictation errors.
- `cleanup_runtime_status` command response remains frontend-compatible.
- Tests cover no-load-at-sync, first-request-loads, cleanup-off disables,
  Ollama override bypass, cleanup fallback, and ASR language re-arm.

## Verification before PR

- `cargo build --workspace`
- `cargo test --workspace`
- `cargo clippy -p wispergo-core --all-targets -- -D warnings`
- `cargo clippy -p wispergo-core --all-targets --features llama-cpp -- -D warnings`
- `pnpm test:ts`
- Targeted desktop manager/wiring tests added in this slice.

Known note: `cargo clippy -p wispergo-desktop --all-targets -- -D warnings`
currently fails on pre-existing unrelated lints documented in PR #11. Do not
expand Phase 4.2 solely to fix those unless they block merged CI.

## Review questions for approval

1. Do you approve deleting/replacing the temporary `CleanupRuntimeManager` bridge
   in Phase 4.2 while keeping the `cleanup_runtime_status` command name stable?
2. Do you approve Option A: wire manager through existing `WhisperRsProvider` and
   `LlamaCppCleanupProvider` first, deferring lower-level persistent
   `LlamaBackend` + `LlamaModel` optimization to a focused follow-up?
3. Do you approve broadening settings sync so recognition-language changes
   re-arm ASR, while cleanup-mode/backend changes arm/disable cleanup?
4. Do you approve keeping Phase 4.2 limited to live manager wiring and tests,
   leaving model manifest/tiering/downloader behavior to Phase 5?
