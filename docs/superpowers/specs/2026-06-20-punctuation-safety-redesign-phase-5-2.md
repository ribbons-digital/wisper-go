# Phase 5.2 Redesign: Safe Punctuation Cleanup

**Date:** 2026-06-20  
**Status:** Approved by user on 2026-06-20.  
**Roadmap slice:** Phase 5.2 in `docs/superpowers/plans/2026-06-18-in-process-inference-roadmap.md`.

## Entry gate

Phase 5.2 originally planned to choose a cleanup-punctuation default by finding a
small Qwen2.5 model whose raw output passed `docs/manual/offline-cleanup-eval.md`.
That gate failed for 0.5B, 1.5B, and 3B candidates. This redesign replaces
"trust the model to punctuate only" with "treat model output as a suggestion and
accept it only if a deterministic safety gate proves it did not rewrite the
transcript."

No runtime implementation should start until this spec is reviewed and approved.

## Sources

- Roadmap: `docs/superpowers/plans/2026-06-18-in-process-inference-roadmap.md`
- Phase 5 design: `docs/superpowers/specs/2026-06-19-model-tiering-phase-5-design.md`
- Manual eval fixture: `docs/manual/offline-cleanup-eval.md`
- Cleanup prompt/parser: `crates/wispergo-core/src/cleanup_prompt.rs`
- In-process cleanup provider: `crates/wispergo-core/src/cleanup_inprocess.rs`
- Recording fallback behavior: `apps/desktop/src-tauri/src/commands/recording.rs`
- Glossary: `CONTEXT.md`

## Historical context

The pre-redesign product used Ollama for cleanup with default model
`qwen2.5:0.5b`, overridable via `WISPERGO_OLLAMA_MODEL`. The old bundled-sidecar
plan later named `qwen2.5-3b-instruct-q4_k_m.gguf` for llama.cpp cleanup, but
that sidecar direction has been superseded by the thin app + downloadable Assets
architecture.

The failed Phase 5.2 evals should be read as surfacing a pre-existing product
risk rather than a regression caused by the thin-app redesign: small chat models
can translate, romanize, or remove words even when prompted to add punctuation
only.

## Eval evidence that triggered redesign

Manual fixture: `docs/manual/offline-cleanup-eval.md`.

Local artifacts and results:

| Candidate | Size | SHA-256 | Outcome |
| --- | ---: | --- | --- |
| Qwen2.5-0.5B-Instruct Q4_K_M | `491400032` | `74a4da8c9fdbcd15bd1f6d01d621410d31c6fc00986f5eb687824e7b93d7a9db` | Failed: translated Chinese question; changed/romanized mixed `小王`; inconsistent Chinese punctuation. |
| Qwen2.5-1.5B-Instruct Q4_K_M | `1117315456` | `6a1a2eb6d15622bf3c96857206351ba97e1af16c30d7a74ee38970e434e9407e` | Failed: translated Chinese question or omitted Chinese punctuation; changed `小王` to `王`. |
| Qwen2.5-3B-Instruct Q4_K_M | `2104932768` | `626b4a6678b86442240e33df819e00132d3ba7dddfe1cdc4fbb18e0a9615c62d` | Failed: omitted Chinese question punctuation and changed `小王` to `王`; stricter prompt still changed `小王` and mis-punctuated the Chinese sentence. |

Saved local evidence files:

- `/tmp/wispergo-model-eval/qwen-0.5b-eval.tsv`
- `/tmp/wispergo-model-eval/qwen-0.5b-eval-hardened.tsv`
- `/tmp/wispergo-model-eval/qwen-1.5b-eval.tsv`
- `/tmp/wispergo-model-eval/qwen-1.5b-eval-hardened.tsv`
- `/tmp/wispergo-model-eval/qwen-1.5b-eval-user-strict.tsv`
- `/tmp/wispergo-model-eval/qwen-3b-eval.tsv`
- `/tmp/wispergo-model-eval/qwen-3b-eval-user-strict.tsv`

## Problem statement

Punctuation-only cleanup has a stricter safety requirement than Full cleanup.
Users choose punctuation-only expecting the same words in the same language and
script, with only punctuation and minimal capitalization changed. A model output
that silently translates text or mutates a name is worse than raw ASR.

The model prompt cannot be the safety boundary. The product needs a deterministic
accept/reject boundary after model generation and before insertion.

## Goals

1. Make Punctuation-only non-rewriting by construction.
2. Preserve raw ASR fallback when punctuation output is unsafe, missing, corrupt,
   downloading, or unavailable.
3. Allow a small cleanup-punctuation Asset to provide value where it behaves
   safely, without risking transcript corruption.
4. Keep Full cleanup separate: it remains the mode where LLM rewriting and intent
   classification are expected and pack-gated.
5. Update manual eval semantics to distinguish safety from punctuation quality.

## Non-goals

- Do not redesign Full cleanup in Phase 5.2.
- Do not add a visible cleanup model picker.
- Do not introduce cloud inference.
- Do not require a 3B cleanup Asset for Punctuation-only.
- Do not solve every language's punctuation perfectly in this slice.
- Do not optimize persistent llama model ownership unless required for
  correctness; lifecycle/performance remains separate from punctuation safety.

## Decision: model suggestion + deterministic safety gate

Punctuation-only cleanup will run as:

1. Get raw transcript from ASR.
2. If Cleanup Mode is Off, insert raw transcript.
3. If Punctuation-only is enabled and a cleanup provider is available, request a
   punctuation suggestion.
4. Run the suggestion through a deterministic punctuation safety gate.
5. If the suggestion is safe, insert it.
6. If the suggestion is unsafe or the provider fails, insert raw ASR.

The safety gate is the trust boundary. The model is only a suggestion source.

## Safety gate requirements

The safety gate compares raw transcript and suggested output after removing only
formatting differences that Punctuation-only is allowed to change.

Required behavior:

- Chinese/CJK characters must be identical and in the same order.
- Latin words must be identical ignoring ASCII capitalization.
- Digits must be preserved.
- Non-punctuation symbols that are part of the transcript must be preserved.
- Punctuation may be added, removed, or changed.
- Whitespace may change only as a formatting side effect around punctuation.
- ASCII capitalization may change, primarily sentence-initial capitalization.
- Any translation, romanization, paraphrase, added word, removed word, or changed
  CJK character rejects the suggestion.

Examples:

| Raw | Suggestion | Decision | Reason |
| --- | --- | --- | --- |
| `can you send the updated notes before the meeting starts` | `Can you send the updated notes before the meeting starts?` | Accept | Same Latin words, only capitalization/punctuation changed. |
| `你明天可以帮我检查这个离线版本吗` | `你明天可以帮我检查这个离线版本吗？` | Accept | Same Chinese characters, punctuation added. |
| `你明天可以帮我检查这个离线版本吗` | `Can you check this offline version for me tomorrow?` | Reject | Translation. |
| `please remind 小王 to review the offline build tonight` | `Please remind 王 to review the offline build tonight.` | Reject | Removed `小`. |
| `today we reviewed the release checklist` | `Today, we reviewed the release checklist.` | Accept | Same words, punctuation/capitalization only. |
| `today we reviewed the release checklist` | `Today, we reviewed our release checklist.` | Reject | Added/replaced word. |

## Default cleanup-punctuation Asset choice

With the safety gate in place, the default model choice no longer needs to prove
that every raw model output is safe. It must prove that the final inserted output
is safe after validation/fallback.

Recommendation for implementation:

- Use Qwen2.5-0.5B-Instruct Q4_K_M as the first cleanup-punctuation default
  candidate because it is the smallest tested candidate and can provide English
  punctuation value.
- Wrap it with the safety gate before activation.
- Re-run the manual fixture in terms of final inserted output:
  - unsafe model output must fall back to raw ASR;
  - safe model output may be accepted;
  - the final output must never translate, romanize, add, remove, or rewrite
    content.
- Track punctuation quality separately from safety. A case that falls back to raw
  ASR may pass safety while being marked as "no punctuation improvement."

This keeps first-run download size low while preventing the unsafe behaviors
observed in 0.5B/1.5B/3B evals from reaching the user.

## Manual eval update

`docs/manual/offline-cleanup-eval.md` should be updated to record:

- raw ASR;
- model suggestion;
- safety decision: `accepted` or `fallback_raw`;
- final inserted output;
- safety notes;
- punctuation-quality notes;
- latency.

Pass criteria should become:

- **Safety pass:** final inserted output does not translate, romanize, add,
  remove, or rewrite content. Safe fallback to raw ASR counts as a safety pass.
- **Quality observation:** accepted model output should improve punctuation where
  possible, but lack of punctuation improvement is not a safety failure.
- **Default-asset acceptance:** a cleanup-punctuation default may be accepted if
  every fixture case passes safety and the model gives enough useful improvement
  to justify the default download. The current 0.5B candidate is expected to
  improve English cases and fall back on unsafe mixed/Chinese cases.

## Implementation slices

### 5.2a — punctuation safety gate

Likely files:

- `crates/wispergo-core/src/cleanup_safety.rs` or equivalent new pure module
- `crates/wispergo-core/src/lib.rs`
- `crates/wispergo-core/tests/cleanup_safety_tests.rs`
- `apps/desktop/src-tauri/src/commands/recording.rs`

Definition of done:

- Pure safety gate accepts punctuation/capitalization-only changes.
- Pure safety gate rejects translation, romanization, added words, removed words,
  changed CJK characters, and mixed-language name corruption.
- Recording path applies the gate to Punctuation-only model output before
  insertion.
- Unsafe punctuation output falls back to raw ASR, preserving existing fallback
  behavior.
- Full cleanup path is unchanged.

### 5.2b — cleanup-punctuation default Asset with safety eval

Likely files:

- `apps/desktop/src-tauri/resources/models.manifest.json`
- `apps/desktop/src-tauri/src/commands/settings.rs`
- `docs/manual/offline-cleanup-eval.md`
- tests for app-support cleanup-punctuation path resolution and missing/corrupt
  cleanup fallback

Definition of done:

- Manifest contains one `default: true` `cleanup_punctuation` Asset for the
  safety-wrapped default candidate.
- Default cleanup Asset is downloaded with defaults.
- Punctuation-only uses verified app-support cleanup Asset when present.
- Missing/corrupt/downloading cleanup punctuation falls back to raw ASR.
- Manual eval documents model suggestion, safety decision, final inserted output,
  and latency.
- Every final inserted fixture output passes safety.

## Verification before PR

Use the normal expanded gate:

- `cargo build --workspace`
- `cargo test --workspace`
- `cargo clippy -p wispergo-core --all-targets -- -D warnings`
- `cargo clippy -p wispergo-core --all-targets --features llama-cpp -- -D warnings`
- `cargo clippy -p wispergo-desktop --all-targets -- -D warnings`
- `pnpm test:ts`

For 5.2b also run the manual cleanup eval with the safety gate enabled and record
the final inserted outputs in `docs/manual/offline-cleanup-eval.md`.

## Risks and mitigations

- **Risk: validator is too permissive.** Mitigate with explicit tests for the
  failure modes already observed: Chinese translation, `小王` → `王`, romanization,
  added words, removed words, and English paraphrase.
- **Risk: validator is too strict and rejects useful punctuation.** This is
  acceptable for Punctuation-only; raw ASR fallback is safer than unsafe rewrite.
- **Risk: 0.5B provides limited punctuation value outside English.** Track quality
  separately. Full multilingual punctuation quality can be revisited with a
  dedicated punctuation model later.
- **Risk: latency measurements from eval include model load cost.** Phase 5.2 is
  a safety slice. Persistent llama ownership remains a separate lifecycle/perf
  improvement if user value justifies it.

## Open questions resolved

- **Should safe raw-ASR fallback count as passing the eval?** Yes, for safety.
  Punctuation quality is recorded separately.
- **Should Punctuation-only remain LLM-based?** Yes, but only as an untrusted
  suggestion source behind a deterministic safety gate.
- **Should Full cleanup use the same safety gate?** No. Full cleanup is a
  different mode where structured intent classification and rewriting are part
  of the expected behavior.
