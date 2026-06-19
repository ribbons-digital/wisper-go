# Phase 3.3 Design Draft: Retire `llama-server` Sidecar and Process Runtime

**Date:** 2026-06-19  
**Status:** Approved by user on 2026-06-19.  
**Roadmap slice:** Phase 3.3 in `docs/superpowers/plans/2026-06-18-in-process-inference-roadmap.md`.

## Entry gate

This was the design/scoping gate for Phase 3.3. It turns the Phase 3.2
`LlamaCppCleanupProvider` into the product cleanup provider and removes the old
`llama-server` process layer. The user approved this draft and recommendations
on 2026-06-19.

## Sources

- Roadmap: `docs/superpowers/plans/2026-06-18-in-process-inference-roadmap.md`
- Approved architecture: `docs/superpowers/specs/2026-06-18-in-process-inference-and-asset-downloader-design.md`
- Phase 3.2 design: `docs/superpowers/specs/2026-06-19-llama-cpp-cleanup-provider-3-2-design.md`
- Current in-process provider: `crates/wispergo-core/src/cleanup_inprocess.rs`
- Current process runtime: `apps/desktop/src-tauri/src/inference/cleanup_runtime.rs`
- Recording pipeline cleanup selection: `apps/desktop/src-tauri/src/commands/recording.rs`
- Settings runtime sync/status command: `apps/desktop/src-tauri/src/commands/settings.rs`
- Bundled resource path resolver: `apps/desktop/src-tauri/src/inference/resources.rs`

## Slice goal

Retire the bundled `llama-server` sidecar and HTTP provider from the live cleanup
path. When cleanup is enabled and `WISPERGO_CLEANUP_BACKEND != ollama`, the
recording pipeline should use `LlamaCppCleanupProvider` directly through the
existing `TextCleanupProvider` / `CleanupProvider` traits.

## Non-goals for 3.3

- Do not build the full Phase 4 `InferenceManager` lifecycle yet.
- Do not implement persistent cleanup model ownership, idle unload, panic guard,
  or generation-guarded reload-on-failure; Phase 3.2's per-request local llama
  engine remains acceptable until Phase 4.
- Do not populate real asset-manifest cleanup entries or move cleanup loading to
  app-support assets; Phase 5 owns real model tiering/readiness.
- Do not remove bundled model directories, build scripts, Intel paths, or
  general bundle-layout tests beyond the `llama-server` binary dependency;
  Phase 6 owns the full bundled-path retirement.
- Do not remove the Ollama dev override (`WISPERGO_CLEANUP_BACKEND=ollama`).

## Recommended decisions

### 1. Mirror Phase 2.3's aggressive deletion approach

**Decision:** Delete the `llama-server` sidecar path outright rather than keep a
dark fallback.

Remove from non-test product code:

- `crates/wispergo-core/src/llama_server.rs`
- `pub mod llama_server`
- `LlamaServerCleanupProvider`
- `DEFAULT_LLAMA_SERVER_MODEL`
- process spawning for `llama-server`
- `CleanupRuntimeCommand`
- `choose_local_port`
- `TcpListener`
- `Child` monitor/restart mechanics
- HTTP readiness polling / warmup loop

**Why:** This matches the user's Phase 2.3 decision and the sole-maintainer
context. Keeping the sidecar as a hidden fallback would preserve dead complexity
and conflicts with the stated migration goal: in-process GGML engines, no
sidecars.

### 2. Flip `llama-cpp` on by default in `wispergo-core`

**Decision:** Update `crates/wispergo-core/Cargo.toml` so:

```toml
default = ["whisper-rs", "llama-cpp"]
```

**Why:** After 3.3, in-process cleanup is the product local cleanup backend.
Default builds should compile the code path the app uses. This mirrors Phase 2.3
where `whisper-rs` became a default feature when the sidecar was retired.

**Consequence:** Default builds now require cmake + clang for both
`whisper-rs` and `llama-cpp-2`, which is already documented as a prerequisite.

### 3. Keep the frontend `cleanup_runtime_status` contract as a bridge

**Decision:** Keep the existing frontend-facing command/type name for now:

- `cleanup_runtime_status`
- `CleanupRuntimeStatus`
- `CleanupRuntimeState`
- settings-panel notice wiring

But change the backend implementation from a process manager to a lightweight
in-process cleanup readiness/configuration state.

Recommended bridge semantics:

- Cleanup mode `Off` or Ollama backend override → `Disabled`.
- Cleanup enabled and bundled cleanup GGUF path exists → `Ready`.
- Cleanup enabled but bundled cleanup GGUF path missing → `Unavailable` with the
  existing sanitized offline punctuation message.
- `Starting` and `Failed` remain in the enum for frontend/API compatibility, but
  Phase 3.3 does not need to transition through `Starting` because no background
  process starts. Phase 4 can reuse or refine those states for real
  `InferenceManager` loading.

**Why:** The React UI already consumes this sanitized state. Deleting or renaming
it now would create unnecessary UI/API churn, while Phase 4 explicitly plans the
frontend-facing lifecycle replacement. Keeping the name temporarily is a bridge;
renaming to `InferenceManager` should happen in Phase 4 when the state machine is
real.

### 4. Use `LlamaCppCleanupProvider` from the recording pipeline

**Decision:** Replace `cleanup_runtime.provider()` in
`commands/recording.rs` with an in-process provider factory when local cleanup is
selected.

Recommended behavior:

- Preserve Ollama override first:
  - if `WISPERGO_CLEANUP_BACKEND=ollama`, use `OllamaCleanupProvider` exactly as
    today.
- Otherwise, use `LlamaCppCleanupProvider::new(LlamaCppCleanupConfig::new(path))`
  with the current bundled cleanup GGUF path.
- If the path is absent or invalid, return `None` so the existing raw-ASR
  fallback behavior for cleanup-unavailable cases remains intact.

**Why:** Failure semantics say cleanup is best-effort. Missing cleanup asset
should not block dictation. ASR remains the hard prerequisite.

### 5. Narrow `InferenceResourcePaths` without doing Phase 6

**Decision:** Remove `llama_server_binary_path` from `InferenceResourcePaths` and
from required asset validation. Keep:

- `whisper_binary_path` for now, even though it is already unused by the live ASR
  path, because its cleanup is explicitly deferred to Phase 6.
- `cleanup_model_path` because Phase 3.3 still loads the bundled cleanup GGUF
  until Phase 5 asset manifest/model tiering.

**Why:** Phase 3.3 should remove exactly the cleanup sidecar binary dependency
without broadening into full bundled-path cleanup.

### 6. Delete/replace sidecar-specific tests, add in-process selection tests

**Decision:** Remove tests that assert `llama-server` command shape, port
selection, child monitor restart, or HTTP provider behavior. Add tests for the
new boundary behavior:

- `llama-cpp` feature is on by default.
- `cleanup_runtime_status` reports `Ready` when cleanup is enabled and the
  cleanup GGUF exists.
- `cleanup_runtime_status` reports `Unavailable` with sanitized message when the
  cleanup GGUF is missing.
- `sync_cleanup_runtime_for_settings` disables local runtime state for Cleanup
  Off and Ollama override.
- Recording provider factory selects Ollama for the env override and local
  in-process cleanup otherwise.
- Repository grep/DoD check: no `llama-server`, `TcpListener`, `Child`,
  `CleanupRuntimeCommand`, or `LlamaServerCleanupProvider` references remain in
  non-test source.

## Proposed implementation files for Phase 3.3

- Delete: `crates/wispergo-core/src/llama_server.rs`
- Delete: `crates/wispergo-core/tests/llama_server_tests.rs`
- Modify: `crates/wispergo-core/src/lib.rs`
  - Remove `pub mod llama_server`.
- Modify: `crates/wispergo-core/Cargo.toml`
  - Add `llama-cpp` to default features; update feature comments.
- Modify: `apps/desktop/src-tauri/src/inference/cleanup_runtime.rs`
  - Replace process manager internals with lightweight status/config bridge.
  - Remove process/port/child/readiness code.
- Modify: `apps/desktop/src-tauri/src/commands/recording.rs`
  - Use `LlamaCppCleanupProvider` for local cleanup backend.
- Modify: `apps/desktop/src-tauri/src/commands/settings.rs`
  - Keep `cleanup_runtime_status` and `sync_cleanup_runtime_for_settings`, but
    make sync update lightweight status instead of spawning a process.
- Modify: `apps/desktop/src-tauri/src/inference/resources.rs`
  - Remove `llama_server_binary_path` and its validation requirement.
- Modify: `apps/desktop/src-tauri/src/lib.rs`
  - Keep manager registration if the lightweight bridge remains; remove shutdown
    child-kill semantics or make shutdown a simple state transition.
- Modify tests near the files above to assert the new behavior.
- Possibly modify: `README.md` only if it currently instructs users to stage
  `llama-server` specifically. Full README refresh remains Phase 6.

## Proposed Phase 3.3 definition of done

- Default local cleanup uses `LlamaCppCleanupProvider` behind the existing
  cleanup provider traits.
- `llama-cpp` feature is enabled by default in `wispergo-core`.
- `llama_server.rs` and `llama_server_tests.rs` are deleted.
- No `llama-server`, `TcpListener`, `Child`, `CleanupRuntimeCommand`,
  `choose_local_port`, `LLAMA_SERVER_*`, or `LlamaServerCleanupProvider`
  references remain in non-test source.
- Ollama dev override remains available and tested.
- Cleanup-unavailable behavior remains best-effort raw-ASR fallback; missing
  cleanup model does not block dictation.
- Frontend status contract remains stable for this slice.
- Roadmap and handoff are updated.
- Verification before PR includes:
  - `cargo build --workspace`
  - `cargo test --workspace`
  - `cargo clippy -p wispergo-core --all-targets -- -D warnings`
  - `cargo clippy -p wispergo-core --all-targets --features llama-cpp -- -D warnings`
  - `pnpm test:ts`
  - `rg -n "llama-server|LlamaServerCleanupProvider|CleanupRuntimeCommand|choose_local_port|TcpListener|std::process::Child|LLAMA_SERVER" apps crates README.md Cargo.toml`
    with only allowed docs/superseded-plan references, if any.

## Approved implementation direction

1. Mirror Phase 2.3: delete the `llama-server` sidecar path outright and flip
   `llama-cpp` on by default.
2. Keep `cleanup_runtime_status` / `CleanupRuntimeStatus` as a temporary
   frontend bridge until Phase 4, even though the backend no longer manages a
   process.
3. Keep Phase 3.3 focused on sidecar/runtime removal only, deferring persistent
   cleanup model lifecycle/perf to Phase 4 and real model assets/readiness to
   Phase 5.
