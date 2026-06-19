# Phase 4 Design Draft: `InferenceManager` Lifecycle

**Date:** 2026-06-19  
**Status:** Approved by user on 2026-06-19 for Phase 4.1 implementation.  
**Roadmap slices:** Phase 4.1 and 4.2 in `docs/superpowers/plans/2026-06-18-in-process-inference-roadmap.md`.

## Entry gate

This is the design/scoping gate for Phase 4. It replaces the temporary Phase 3.3
`cleanup_runtime_status` bridge with the real in-process **Inference Manager**
lifecycle for ASR and cleanup. No code should be implemented until this draft is
reviewed and approved.

## Sources

- Roadmap: `docs/superpowers/plans/2026-06-18-in-process-inference-roadmap.md`
- Approved architecture: `docs/superpowers/specs/2026-06-18-in-process-inference-and-asset-downloader-design.md`
- Phase 3.2 cleanup provider design: `docs/superpowers/specs/2026-06-19-llama-cpp-cleanup-provider-3-2-design.md`
- Phase 3.3 sidecar retirement design: `docs/superpowers/specs/2026-06-19-cleanup-sidecar-retirement-3-3-design.md`
- Current bridge: `apps/desktop/src-tauri/src/inference/cleanup_runtime.rs`
- Recording pipeline: `apps/desktop/src-tauri/src/commands/recording.rs`
- In-process ASR provider: `crates/wispergo-core/src/whisper_rs_provider.rs`
- In-process cleanup provider: `crates/wispergo-core/src/cleanup_inprocess.rs`
- Glossary: `CONTEXT.md`

## Slice goals

### Phase 4.1 — lifecycle/worker core

Introduce an `InferenceManager` that owns lifecycle state for ASR and cleanup:
armed-but-not-loaded, loading, ready, unavailable, failed, idle unload, and
failure-triggered unload/reload-on-next-request. The manager should be testable
with fake engines before wiring it into dictation.

### Phase 4.2 — recording/settings wiring

Route recording through `InferenceManager` and make settings sync arm/disable the
manager instead of constructing providers directly. Startup should arm configured
engines without loading models. First dictation should trigger load.

## Non-goals for Phase 4

- Do not populate real model manifest entries or switch from bundled model paths
  to app-support asset paths; Phase 5 owns model tiering/readiness.
- Do not remove bundled asset trees/scripts beyond what Phase 3.3 already did;
  Phase 6 owns bundled-path retirement.
- Do not add live partial transcripts; Phase 7 owns streaming.
- Do not remove the Ollama dev override; it remains an alternative cleanup
  backend and bypasses local cleanup manager routing.
- Do not solve multi-user/concurrent dictation; one in-flight request per engine
  is the intended single-user product shape.

## Recommended decisions

### 1. Keep Phase 4 split into two PR-sized slices

**Decision:** Implement Phase 4 as two review-gated PRs:

1. **4.1 lifecycle core** — new manager/worker/state-machine with fake-engine
   tests and no recording-path switch.
2. **4.2 wiring** — connect recording/settings/app setup to the manager.

**Why:** The hard part is lifecycle correctness. Keeping it isolated lets tests
prove idle unload, generation guards, and panic handling before the live
dictation path changes.

### 2. Place `InferenceManager` in the desktop inference layer

**Decision:** Create the manager under `apps/desktop/src-tauri/src/inference/`,
for example:

- `apps/desktop/src-tauri/src/inference/manager.rs`
- optional submodules if needed: `manager/state.rs`, `manager/worker.rs`

**Why:** The manager owns desktop runtime concerns: app resource paths, settings
sync, Tauri state, background timers, and frontend status serialization. Core
provider traits remain in `wispergo-core`.

### 3. Preserve frontend state shape while renaming internals

**Decision:** Keep the serialized frontend states compatible with the existing
settings UI:

- `disabled`
- `starting`
- `ready`
- `unavailable`
- `failed`

But introduce clearer internal names:

- `InferenceRuntimeState`
- `InferenceRuntimeStatus`
- `InferenceManager`

For compatibility, `cleanup_runtime_status` can remain as a command alias in
4.2, returning the cleanup engine's `InferenceRuntimeStatus`. The frontend rename
can wait until there is user value in changing UI/API names.

**Interpretation:** `ready` means the engine is configured/armed and available
for lazy load. It does **not** necessarily mean the model is currently resident
in memory. `starting` is only used while an actual load/request is in progress.

### 4. Use dedicated per-engine workers with command channels

**Decision:** Give ASR and cleanup their own worker loops. Each worker owns its
engine state and receives commands over channels:

- `Arm(config)` — store configuration; do not load.
- `Disable` — unload if loaded and mark disabled.
- `Request(payload)` — load if needed, run inference, update last-used time.
- `UnloadIfIdle(generation)` — unload only if generation still matches and idle
  deadline has passed.
- `Shutdown` — unload and stop.

**Why:** This satisfies the roadmap's dedicated-thread/panic-guard direction and
keeps non-`Send` / borrowing-sensitive model state inside one owner. It also
avoids a self-referential `LlamaModel` + `LlamaContext<'_>` struct.

### 5. Cleanup worker owns backend + model; creates context per request

**Decision:** For cleanup, the worker should persist the expensive
`LlamaBackend` + `LlamaModel` while loaded. It should create a fresh
`LlamaContext` for each request unless a safe KV-cache reset path is proven.

**Why:** `LlamaContext<'a>` borrows `LlamaModel`. Persisting backend+model in the
worker and creating context per request avoids unsafe self-referential structs
while still eliminating repeated model loads. Context creation per utterance is
acceptable until profiling says otherwise.

**Implementation implication:** Refactor `cleanup_inprocess.rs` so the llama
completion logic has reusable pieces:

- model/backend load
- chat template prompt construction
- per-request context/decode

The public `LlamaCppCleanupProvider` can remain for tests/compatibility, but the
manager's cleanup worker may call lower-level helpers to reuse the loaded model.

### 6. ASR worker owns `WhisperContext`; creates state per request

**Decision:** For ASR, the worker should persist `WhisperContext` while loaded
and create a new whisper state per transcription. This matches the current
`WhisperRsProvider` behavior but lets the manager unload after idle.

**Implementation implication:** Refactor `whisper_rs_provider.rs` enough to let
the worker reuse loading/transcription helpers without duplicating the whisper
pipeline. Keep public `WhisperRsProvider` behavior unchanged unless 4.2 fully
replaces direct construction in recording.

### 7. Idle windows: cleanup 5 minutes, ASR 30 minutes

**Decision:** Use:

- cleanup idle unload: 5 minutes
- ASR idle unload: 30 minutes

**Why:** The approved spec explicitly sets cleanup to 5 minutes and says ASR
should have a longer window because load is expensive and users dictate in
bursts. 30 minutes is a reasonable first default; tuning can use
`recording-timings.log` later.

### 8. Generation guard applies to arm/disable/unload/reload

**Decision:** Every engine has a monotonic generation token. Arm/disable/settings
changes increment generation. Any delayed idle unload or failure-triggered reload
must check the generation before changing state.

**Why:** This preserves the old runtime-manager safety property without process
restart complexity. It prevents stale background work from unloading or marking
failed after the user changed mode/model.

### 9. Panic handling unloads and marks failed, reload happens on next request

**Decision:** Worker request execution is wrapped in `catch_unwind`. On panic or
provider failure classified as hard failure:

1. unload current engine state;
2. mark engine `failed` with sanitized message;
3. return an error to the caller;
4. next request attempts to load again if still armed and generation matches.

**Why:** This follows the approved architecture: no timer-driven respawn, no
process isolation, reload on next request.

### 10. Failure semantics remain asymmetric

**Decision:** Preserve current product behavior:

- ASR unavailable/failure means dictation fails with clear error.
- Cleanup unavailable/failure means raw ASR fallback.

**Why:** This is a documented domain decision in the approved design. Phase 4
changes lifecycle ownership, not product failure semantics.

## Proposed implementation files

### Phase 4.1

- Create: `apps/desktop/src-tauri/src/inference/manager.rs`
  - `InferenceManager`
  - per-engine status/state types
  - worker command abstraction
  - fake-engine test harness
- Modify: `apps/desktop/src-tauri/src/inference/mod.rs`
  - export manager module.
- Possibly modify: `crates/wispergo-core/src/cleanup_inprocess.rs`
  - expose lower-level reusable cleanup engine helpers for worker-owned model.
- Possibly modify: `crates/wispergo-core/src/whisper_rs_provider.rs`
  - expose lower-level reusable ASR helpers for worker-owned context.
- Tests: manager state machine tests with fake engines.

### Phase 4.2

- Modify: `apps/desktop/src-tauri/src/lib.rs`
  - manage `InferenceManager` instead of or alongside temporary cleanup bridge.
- Modify: `apps/desktop/src-tauri/src/commands/settings.rs`
  - sync settings to arm/disable manager without model load.
  - keep `cleanup_runtime_status` command alias if frontend remains unchanged.
- Modify: `apps/desktop/src-tauri/src/commands/recording.rs`
  - route ASR and cleanup requests through manager.
  - preserve Ollama cleanup override.
- Modify/delete: `apps/desktop/src-tauri/src/inference/cleanup_runtime.rs`
  - remove temporary bridge once manager status is wired.
- Tests: no-load-at-launch, first-dictation-loads, idle unload, settings change
  generation guard, cleanup raw-ASR fallback.

## Proposed Phase 4.1 definition of done

- `InferenceManager` lifecycle core exists with ASR and cleanup engine slots.
- Engines can be armed without loading.
- First request transitions through `starting` and loads fake engine once.
- Loaded engine is reused across requests before idle deadline.
- Idle unload drops loaded fake engine after configured window.
- Shutdown/disable invalidates pending idle unload via generation guard.
- Panic in fake engine is caught, status becomes `failed`, engine is unloaded,
  and next request attempts reload.
- Tests cover state transitions with fake engines; no real model files required.
- No live recording/settings wiring yet except module exports if needed.

## Proposed Phase 4.2 definition of done

- App setup arms the manager but does not load ASR or cleanup models.
- First dictation loads ASR; cleanup loads only when Cleanup Mode is not Off and
  local cleanup backend is selected.
- Cleanup Mode Off disables/keeps cleanup unloaded.
- Ollama override bypasses local cleanup manager as today.
- Missing ASR asset remains a hard dictation error.
- Missing cleanup asset/provider remains best-effort raw-ASR fallback.
- `cleanup_runtime_status` remains frontend-compatible or is replaced with a
  tested equivalent frontend contract.
- Roadmap/handoff updated.

## Verification before each PR

- `cargo build --workspace`
- `cargo test --workspace`
- `cargo clippy -p wispergo-core --all-targets -- -D warnings`
- `cargo clippy -p wispergo-core --all-targets --features llama-cpp -- -D warnings`
- `pnpm test:ts`
- For 4.2: a targeted test proving app setup does not load models and first
  dictation does.

## Review questions for approval

1. Do you approve keeping Phase 4 split into 4.1 lifecycle core and 4.2 live
   wiring, rather than doing both in one large PR?
2. Do you approve the worker ownership approach: ASR worker persists
   `WhisperContext`, cleanup worker persists `LlamaBackend` + `LlamaModel` and
   creates a fresh `LlamaContext` per request?
3. Do you approve the first idle defaults: cleanup 5 minutes, ASR 30 minutes?
4. Do you approve keeping `cleanup_runtime_status` as a frontend-compatible alias
   until 4.2 decides whether a rename has enough user value?
