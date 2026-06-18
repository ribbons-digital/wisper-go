# In-Process Inference and Asset Downloader Design

## Status

Approved. Supersedes `docs/superpowers/specs/2026-05-01-offline-apple-inference-design.md`.
Reversal recorded in `docs/adr/0001-thin-app-downloader-supersedes-bundled-inference.md`.

## Goals

- Ship a thin `Wispergo.app` (megabytes, not gigabytes) that downloads model
  weights on first run and runs fully offline thereafter.
- Move ASR and cleanup **in-process** via statically-linked `whisper-rs` and
  `llama-cpp-2` with Metal — no sidecar binaries, no shipped GGML dylibs.
- Eliminate per-utterance ASR model load (the dominant hold-to-dictate
  latency) by keeping a persistent in-process Whisper context.
- Default to smaller, fit-for-purpose models; offer larger models as opt-in
  packs.
- Keep the existing user-facing feature set intact: Auto/EN/ZH recognition,
  Off/Punctuation/Full cleanup modes, floating recorder + language toggle,
  clipboard/accessibility insertion, privacy/intent pipeline, raw-ASR fallback.
- Apple Silicon only.

## Non-goals (this roadmap)

- Cloud inference.
- Intel Mac support (clean break).
- Live streaming partial transcripts — deferred to a follow-on phase after the
  engine is stable. The in-process Whisper context built here is
  streaming-capable later; only the frontend half is deferred.
- A fully-bundled offline installer variant (retired).
- Changing the privacy/intent pipeline, insertion logic, or floating UI.

## Reversals from the 2026-05-01 spec

| 2026-05-01 decision | New direction |
| --- | --- |
| Bundle all models inside `.app` | Thin app; weights downloaded to app-support on first run |
| "No first-run downloads" non-goal | First run requires network; offline thereafter |
| "Do not optimize for small app size" | Small app size is a primary goal |
| `whisper-cli` sidecar per utterance | In-process `whisper-rs`, persistent context |
| `llama-server` sidecar, warm | In-process `llama-cpp-2`, lazy load / idle unload |
| `ggml-large-v3-turbo` default ASR | `medium` default; `large-v3-turbo` = Accuracy Pack |
| Qwen2.5-3B default cleanup | Qwen2.5-0.5B default (gated on eval); 3B = Full-cleanup Pack |
| Apple Silicon + Intel | Apple Silicon only |

## Architecture

### Asset layer

```
~/Library/Application Support/com.ribbonsdigital.wispergo/
  models/
    asr/
      medium.bin                       # default
      large-v3-turbo.bin               # Accuracy Pack (opt-in)
    cleanup/
      qwen2.5-0.5b-instruct-q4_k_m.gguf  # default (punctuation)
      qwen2.5-3b-instruct-q4_k_m.gguf    # Full-cleanup Pack (opt-in)
  settings.json
  insertion-diagnostics.log
  recording-timings.log
```

Nothing model-related ships inside the `.app` except a small **Asset Manifest**
(`models.manifest.json`) listing each asset: id, role, display name, HuggingFace
download URL, size, SHA-256. The downloader reads the manifest; no asset path or
URL is hardcoded in source.

Roles:
- `asr` — transcription model; exactly one active at a time, selected by
  setting.
- `cleanup-punctuation` — punctuation-only cleanup; the default cleanup asset.
- `cleanup-full` — Full-cleanup-mode asset; required only when the user sets
  Cleanup Mode = Full cleanup.

### Inference engine

Static linkage into `wispergo-desktop`, Metal feature on:

- **ASR**: `whisper-rs` (bindings to whisper.cpp). Receives `f32` PCM directly
  per the existing `AsrProvider` contract (`ASR_INPUT_SAMPLE_RATE_HZ`,
  `ASR_INPUT_CHANNELS`). No temp WAV, no process spawn. The Whisper context is
  loaded once and reused across utterances; language (Auto/EN/ZH) is a context
  parameter, not a different model.
- **Cleanup**: `llama-cpp-2` (bindings to llama.cpp) behind the existing
  `TextCleanupProvider` / `CleanupProvider` traits. The prompt contract from
  `crates/wispergo-core/src/llama_server.rs` is reused verbatim — only the
  transport changes (in-process completion, not HTTP to `llama-server`).

Both crates are pinned to a specific version; `llama-cpp-2`'s fast-moving,
non-semver nature is contained behind the provider traits.

### InferenceManager (lifecycle)

Replaces `CleanupRuntimeManager`. Same frontend-facing states
(`Disabled / Starting / Ready / Unavailable / Failed`) and the same status
events, so the React settings UI needs no state-machine changes.

Transitions:
- **Lazy load.** Models are not loaded at app launch. The manager is "armed" at
  setup. The ASR context loads on the first dictation; the cleanup model loads
  on the first dictation whose Cleanup Mode ≠ Off.
- **Idle unload.** Cleanup model unloads after 5 minutes with no cleanup
  request. ASR context unloads after a longer idle window (ASR load is the
  expensive one; users dictate in bursts). Next use reloads.
- **Failure → reload on next request.** No timer-driven respawn. A panic or
  GGML error at the FFI boundary is caught (`catch_unwind` + `Result`), the
  manager transitions to `Failed`, the model is unloaded, and it reloads on the
  next request. The generation-token guard is retained to prevent a stale
  reload racing a user-initiated unload.
- **Dedicated threads.** ASR and cleanup each run on a dedicated thread with a
  panic guard. No out-of-process isolation — that would reintroduce the sidecar
  we retired.

Removed concepts: `Child`, `TcpListener`, `choose_local_port`,
`CleanupRuntimeCommand`, `monitor_child`, HTTP readiness polling,
`LLAMA_SERVER_HOST`. Kept: the generation-guard pattern, the status state
machine, best-effort raw-ASR fallback on cleanup failure.

### Downloader

A new desktop component, shaped on the existing `ensure_ollama_setup` command
(detect → fetch → verify → ready), emitting status events to the frontend:

- Reads the bundled Asset Manifest.
- Downloads to a `.part` temp file with **resume** (HTTP range requests against
  HuggingFace direct URLs).
- Verifies **SHA-256**; on mismatch, deletes and retries once, then reports
  failure.
- Atomically renames `.part` → final path on success.
- Re-verifies SHA-256 on load (cheap) to detect post-download corruption; a
  corrupt asset triggers re-download or, for cleanup, raw-ASR fallback.

### Failure semantics (deliberate asymmetry)

- **ASR model = hard prerequisite.** If the default ASR asset is absent or
  corrupt, dictation is unavailable with a clear "downloading models" state —
  not silent raw-ASR. There is nothing to transcribe without it.
- **Cleanup model = best-effort.** If the cleanup asset is absent, still
  downloading, or fails, the existing raw-ASR fallback applies. This matches
  current behavior (`ProviderError::Timeout` → `InsertText`).

## Model selection

### ASR

- **Default**: Whisper `medium` (~480 MB, multilingual). Preserves Auto/EN/ZH
  without per-language model swaps.
- **Accuracy Pack (opt-in)**: `ggml-large-v3-turbo` (~1.5 GB). Users who want
  maximum accuracy at the cost of size/latency.

### Cleanup

- **Punctuation default**: Qwen2.5-0.5B-Instruct Q4_K_M (~400 MB).
  **Provisional — gated on `docs/manual/offline-cleanup-eval.md`.** The 0.5B
  must pass the existing eval fixture (English, Chinese, mixed, already-punctuated
  cases) without regressing punctuation quality versus the current 3B. If it
  regresses — especially Chinese punctuation — the punctuation default bumps to
  Qwen2.5-1.5B-Instruct (~900 MB).
- **Full-cleanup Pack (opt-in)**: Qwen2.5-3B-Instruct Q4_K_M (~2.0 GB).
  Required only for Cleanup Mode = Full cleanup. Preserves the 2026-05-01
  quality bar exactly where it matters (structured-JSON intent
  classification).

### Net default download

~0.9 GB (`medium` ~480 MB + 0.5B ~400 MB) versus the prior 3.5 GB bundle.
Accuracy- and Full-cleanup-tuned users can opt into ~3.5 GB more.

## Settings surface

`LocalModelSettings` gains:
- `asr_model_id` — which ASR asset is active (`"medium"` default,
  `"large-v3-turbo"` opt-in).
- `cleanup_model_id` — implicit from Cleanup Mode for now (punctuation default
  vs. Full-cleanup pack); reserved for future explicit selection.

Persisted via the existing `settings.json` path. `set_local_model_settings`
becomes the switch point: selecting a model whose asset is not present triggers
its download before activation; until verified, the previous selection remains
active.

A new `ensure_model_assets` command mirrors `ensure_ollama_setup`: detect →
download → verify → ready, with frontend status events. The Ollama dev override
(env vars `WISPERGO_CLEANUP_BACKEND=ollama` etc.) is retained unchanged as an
alternative backend and is not the product default.

## Testing strategy

Boundary tests (existing pattern), not model quality:
- Asset Manifest parsing and asset-id → path resolution.
- Downloader: resume, SHA-256 mismatch → retry → fail, atomic rename, corrupt
  re-verify on load.
- InferenceManager state transitions: lazy load, idle unload, generation-guarded
  reload-on-failure.
- ASR provider: `f32` PCM → transcript via `whisper-rs` (mocked context in unit
  tests; real context behind an integration test gate).
- Cleanup provider: in-process completion behind the existing traits; prompt
  contract parity with the retired `llama_server` provider.
- Raw-ASR fallback when cleanup model is absent/unavailable.

Model quality (manual gate, not CI):
- `docs/manual/offline-cleanup-eval.md` fixture run against 0.5B **before
  locking the punctuation default**. This is a stop-rule on the cleanup-model
  slice: 0.5B is not accepted until the fixture passes.

## Migration plan (sliced, see roadmap)

1. Asset manifest + downloader + app-support storage; default assets download
   on first run. ASR/cleanup still sidecar-based behind it as a bridge.
2. In-process ASR via `whisper-rs`; retire `whisper-cli` sidecar and temp WAV.
3. In-process cleanup via `llama-cpp-2`; retire `llama-server` sidecar and
   `CleanupRuntimeManager` process lifecycle.
4. `InferenceManager` lifecycle rewrite (lazy load / idle unload / reload).
5. Model tiering: `medium` ASR default + Accuracy Pack; 0.5B cleanup default
   (gated on eval) + Full-cleanup Pack.
6. Retire bundled-asset path: remove `resources/bin`, `resources/models`,
   `verify-inference-assets.sh`, `check-macos-bundle-inference-layout.sh`,
   `desktop:build:offline-release`. Drop Intel targets.
7. (Follow-on phase, separate spec) Live streaming partial transcripts on the
   persistent Whisper context.

## Open evaluation items

- Final punctuation-default model (0.5B vs. 1.5B) — resolved by the eval gate.
- Exact ASR idle-unload window (start longer than cleanup's 5 min; tune from
  `recording-timings.log`).
- Pinned `whisper-rs` and `llama-cpp-2` versions once build integration is
  proven.
