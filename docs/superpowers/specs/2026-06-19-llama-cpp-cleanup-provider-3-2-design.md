# Phase 3.2 Design Draft: `LlamaCppCleanupProvider`

**Date:** 2026-06-19  
**Status:** Approved by user on 2026-06-19.  
**Roadmap slice:** Phase 3.2 in `docs/superpowers/plans/2026-06-18-in-process-inference-roadmap.md`.

## Entry gate

This was the design/scoping gate for Phase 3.2. It resolves the `llama-cpp-2`
API findings into an implementation shape and records the approved DoD change
for test fixtures. The user approved this draft and recommendations on
2026-06-19.

## Sources

- Roadmap: `docs/superpowers/plans/2026-06-18-in-process-inference-roadmap.md`
- Approved architecture: `docs/superpowers/specs/2026-06-18-in-process-inference-and-asset-downloader-design.md`
- API research: `docs/superpowers/research/2026-06-19-llama-cpp-2-api-research.md`
- Current HTTP cleanup provider: `crates/wispergo-core/src/llama_server.rs`
- Current in-process placeholder: `crates/wispergo-core/src/cleanup_inprocess.rs`
- Existing provider traits: `crates/wispergo-core/src/providers.rs`

## Slice goal

Add an in-process cleanup provider behind the existing `TextCleanupProvider` and
`CleanupProvider` traits, using `llama-cpp-2` behind the existing optional
`llama-cpp` feature. The prompt contract and output parsing must match the
current `llama-server` provider; only the transport changes from OpenAI HTTP to
in-process llama.cpp completion.

## Non-goals for 3.2

- Do not wire the provider into the live desktop pipeline yet; that is Phase 3.3.
- Do not remove `llama_server.rs`, `CleanupRuntimeManager`, process spawning,
  or HTTP readiness checks; that is Phase 3.3.
- Do not build the long-lived `InferenceManager` lifecycle, idle unload, or
  generation-guarded reload; that is Phase 4.
- Do not populate real cleanup model manifest entries or decide 0.5B vs 1.5B;
  that remains Phase 5.2's eval gate.

## Recommended decisions

### 1. Extract shared cleanup prompt/parsing contract before adding the provider

**Decision:** Move the prompt builders and parsers currently embedded in
`llama_server.rs` into a shared core module, for example
`crates/wispergo-core/src/cleanup_prompt.rs`.

Move/reuse these functions verbatim:

- `parse_punctuation_cleanup_text`
- `parse_cleanup_json`
- `cleanup_system_prompt`
- `cleanup_user_prompt`
- `punctuation_system_prompt`
- `punctuation_user_prompt`

**Why:** Phase 3.2 must prove prompt-output parity while Phase 3.3 will delete
`llama_server.rs`. Keeping the shared contract outside the soon-to-be-retired
HTTP provider prevents the in-process provider from depending on a module whose
name is already legacy.

**DoD impact:** Tests should assert both providers use the same shared functions,
not duplicate prompt strings.

### 2. Use a per-request local engine in 3.2 to avoid unsafe/self-referential ownership

**Decision:** For 3.2, construct `LlamaBackend`, `LlamaModel`, and
`LlamaContext` inside the blocking completion path for each request. The provider
stores only configuration: GGUF path, context size, thread counts, max generated
tokens, sampler policy, and optional chat-template fallback.

**Why:** `LlamaContext<'a>` borrows `LlamaModel`, which makes a persistent
provider-owned `backend + model + context` self-referential. The safer 3.2 slice
is a correctness-first bridge that avoids `unsafe`, avoids adding a
self-referential helper crate, and leaves lifecycle/performance ownership to the
already-planned Phase 4 `InferenceManager`.

**Trade-off:** Per-request model/context load is slower. That is acceptable for
3.2 because the provider is not wired live until 3.3, and Phase 4 is the planned
place for lazy-load + idle-unload lifecycle. If 3.3 would make this temporary
latency unacceptable, the alternative is a dedicated worker thread that owns the
model/context in stack order and handles requests over channels; that is more
implementation surface and overlaps Phase 4.

**Implementation implication:** Keep the completion engine as a small internal
unit, e.g. `complete_once(config, messages, timeout) -> Result<String,
ProviderError>`, so Phase 4 can replace the per-request load with a persistent
engine without changing prompt/parsing behavior.

### 3. Preserve chat prompt semantics with `apply_chat_template`

**Decision:** Build `LlamaChatMessage` values with:

- role `system`, content from `punctuation_system_prompt()` or
  `cleanup_system_prompt()`
- role `user`, content from `punctuation_user_prompt(&input)` or
  `cleanup_user_prompt(&input)`

Then call `model.chat_template(None)` and
`model.apply_chat_template(&template, &messages, true)`.

**Fallback:** If the GGUF has no embedded chat template, fall back to an explicit
ChatML template only if `llama-cpp-2` exposes the documented
`LlamaChatTemplate::new("chatml")` path on the pinned version. Otherwise return
`ProviderError::Unavailable` with a diagnostic message saying the model lacks a
chat template.

**Why:** Qwen GGUFs should carry the correct template. Reusing the model's own
template avoids hand-formatting ChatML and keeps Phase 3.2 scoped to transport
replacement.

### 4. Use deterministic greedy decoding

**Decision:** Use greedy sampling only for Phase 3.2.

**Why:** Cleanup is not creative generation. Deterministic output makes provider
behavior easier to test and reduces variance before the Phase 5 cleanup model
eval gate.

**Stop condition:** Stop on `model.is_eog_token(token)` and also enforce a
max-generated-token cap. A fixed default such as 512 generated tokens is enough
for the first implementation; the provider config can expose it for tests.

### 5. Keep timeout/error semantics aligned with current providers

**Decision:** Wrap the blocking completion work in `tokio::task::spawn_blocking`
and `tokio::time::timeout(input.timeout, ...)`, matching the established
provider style.

Map errors to the existing `ProviderError` variants:

- model path missing or template missing: `Unavailable`
- timeout: `Timeout`
- decode / llama.cpp failure: `Failed`
- empty punctuation output or invalid cleanup JSON: `InvalidOutput`

**Note:** As with existing blocking provider work, a timed-out blocking task may
continue in the background until llama.cpp returns. Phase 4's dedicated
`InferenceManager` is the right place to improve cancellation semantics if it
becomes necessary.

### 6. Adjust the test-fixture DoD instead of committing a large GGUF

**Roadmap says:** provider tests with a tiny GGUF fixture.

**Recommended adjustment:** Do not commit a cleanup GGUF fixture in 3.2. Instead:

1. CI/unit tests use a small fake completion seam to verify:
   - `TextCleanupProvider::clean_punctuation_only` uses the shared punctuation
     prompt builders and parser.
   - `CleanupProvider::clean` uses the shared cleanup prompt builders and JSON
     parser.
   - invalid/empty outputs map to existing `ProviderError` variants.
   - the `llama-cpp` feature compiles.
2. Add an ignored integration test gated by `WISPERGO_LLAMA_TEST_GGUF=/path/to/model.gguf`
   that runs the real `llama-cpp-2` path against a local GGUF.
3. Document that the real model quality gate remains Phase 5.2 via
   `docs/manual/offline-cleanup-eval.md`.

**Why:** The real candidate cleanup models are hundreds of MB. A vocab-only GGUF
is not a meaningful text-generation provider test, and committing a model binary
would conflict with the thin-app/downloader direction. The ignored test still
proves the real path locally when a fixture is staged, while CI stays fast and
small.

**Approved DoD refinement:** This is a deliberate DoD refinement from "provider
tests with a tiny GGUF fixture" to "CI prompt/parser/provider tests + ignored
real GGUF integration test." The user approved this recommendation on
2026-06-19.

## Proposed implementation files for Phase 3.2

- Modify: `crates/wispergo-core/src/lib.rs`
  - Add `pub mod cleanup_prompt;`.
- Create: `crates/wispergo-core/src/cleanup_prompt.rs`
  - Shared prompt builders and parsers moved from `llama_server.rs`.
- Modify: `crates/wispergo-core/src/llama_server.rs`
  - Import shared prompt/parsing functions; keep HTTP behavior unchanged.
- Modify: `crates/wispergo-core/src/cleanup_inprocess.rs`
  - Replace the 3.1 link-smoke placeholder with `LlamaCppCleanupProvider`,
    config types, completion helper, and tests behind `#[cfg(feature =
    "llama-cpp")]`.
- Possibly modify: `crates/wispergo-core/Cargo.toml`
  - Only if additional `tokio` features or tiny test helpers are required. Do
    not add a self-referential ownership crate for 3.2.

## Proposed Phase 3.2 definition of done

- `LlamaCppCleanupProvider` implements both `TextCleanupProvider` and
  `CleanupProvider` behind the existing `llama-cpp` feature.
- The provider uses the same shared prompt builders and parsers as
  `LlamaServerCleanupProvider`.
- The provider applies the model's chat template with `add_ass = true`.
- Generated text is accumulated via llama.cpp token pieces, stopped by EOG or
  max-generated-token cap, then parsed by the shared parser.
- CI tests cover prompt/parsing/provider behavior without real network and
  without a committed model binary.
- An ignored real-GGUF integration test exists and is documented.
- No live pipeline wiring and no `llama-server` removal happen in 3.2.
- Verification before PR includes the standard shippability gate plus at least:
  - `cargo test -p wispergo-core --features llama-cpp cleanup_inprocess`
  - `cargo clippy -p wispergo-core --all-targets --features llama-cpp -- -D warnings`

## Approved implementation direction

1. Use the per-request local engine for 3.2, with lifecycle/perf deferred to
   Phase 4.
2. Use the refined test DoD: no committed GGUF; add an ignored
   `WISPERGO_LLAMA_TEST_GGUF` integration test plus CI fake-seam tests.
3. Keep Phase 3.2 as one implementation branch/PR unless implementation reveals
   a smaller review split is necessary.
