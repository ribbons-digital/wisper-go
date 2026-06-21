# Handoff — Wispergo In-Process Inference Migration

**Date:** 2026-06-20 (updated during R3 recording waveform UI)
**Next session focus:** Finish verifying R3 recording waveform UI, open PR, wait for merge, then consider R4 CI/release workflow before public release. Do **not** use the `librarian` skill for this project unless its Pi prompt-interface issue is fixed.

> **Standing rule:** This file is tracked and is kept in sync with the roadmap whenever the roadmap changes. If the roadmap says phase X.Y is ✅, this file must reflect that. A fresh agent should be able to read this + the roadmap and continue without re-deriving state.

## Where things stand right now

- **Phase 2 (In-Process ASR) is complete and merged** (PR #7). The `whisper-cli` sidecar is deleted; `whisper-rs` is the default ASR.
- **Phase 3.1 is complete and merged** (PR #8): `llama-cpp-2` pinned at 0.1.146 as an optional, off-by-default `llama-cpp` cargo feature; Metal build verified on arm64.
- **Phase 3.2 is complete and merged** (PR #9): `LlamaCppCleanupProvider` exists behind the existing traits/feature, with shared prompt/parsing contract and approved fake-seam + ignored real-GGUF test strategy.
- **Phase 3.3 is complete and merged** (PR #10): cleanup sidecar path is deleted, `llama-cpp` is on by default, recording uses `LlamaCppCleanupProvider`, and `cleanup_runtime_status` is now a lightweight bridge until Phase 4.
- **Phase 4.1 is complete and merged** (PR #11): desktop `InferenceManager` lifecycle core exists with dedicated worker threads, fake-engine tests, lazy-load, idle unload, generation guard, and panic/failure reload-on-next-request behavior.
- **Phase 4.2 is complete and merged** (PR #12): recording/settings now use `InferenceManager`, the temporary `CleanupRuntimeManager` bridge is removed, `cleanup_runtime_status` remains frontend-compatible, and tests cover the manager wiring behavior.
- **Desktop clippy cleanup is complete and merged** (PR #13): `cargo clippy -p wispergo-desktop --all-targets -- -D warnings` now passes by moving `recording.rs` tests to the end of the file and replacing manual Objective-C nul strings with C string literals in `lib.rs`.
- **Phase 5.1 is complete and merged** (PR #14): ASR manifest entries are populated, `asrModelId` setting/UI exists, selected ASR Assets resolve from app-support storage, and settings activation downloads/verifies the selected ASR Asset first.
- **Phase 5.2 is complete and merged** (PR #15): raw-model eval failed for Qwen2.5 0.5B, 1.5B, and 3B, so Punctuation-only now treats LLM output as an untrusted suggestion. A deterministic safety gate accepts only punctuation/capitalization-only changes and falls back to raw ASR for unsafe suggestions. The safety-wrapped Qwen2.5-0.5B cleanup-punctuation default Asset is in the manifest, and `docs/manual/offline-cleanup-eval.md` records model suggestion, safety decision, final inserted output, safety/quality notes, and latency.
- **Phase 5.3 is complete and merged** (PR #16): adds the Qwen2.5-3B-Instruct `cleanup_full` manifest Asset with `default: false`, resolves Full cleanup from the verified app-support 3B Asset, downloads/verifies the Full-cleanup Pack before activation, leaves previous settings active on failure, keeps Punctuation-only unaffected by missing 3B, and preserves the `WISPERGO_CLEANUP_BACKEND=ollama` dev override without requiring local `cleanup_full` Assets.
- **macOS deployment-target build fix is complete and merged** (PR #17): plain `pnpm desktop:build` no longer requires manual deployment-target env prefixes.
- **Phase 6 is complete and merged** (PR #18): retired bundled sidecar/model paths, kept only the Asset Manifest in the app bundle, removed `InferenceResourcePaths` / `CpuArchitecture` and legacy settings path fields, added a thin-bundle check, and refreshed README/docs for thin-app + first-run downloads.
- **Language UX follow-up is complete and merged** (PR #19): language-only switching re-arms ASR without re-hashing the selected Asset, while normal settings/model activation still verifies integrity; Chinese mode is documented as Chinese / Mixed for mixed Chinese-English dictation.
- **Compact ZH label follow-up is complete and merged** (PR #20): the floating badge is back to `ZH` while expanded UI/help copy remains Chinese / Mixed.
- **Release readiness design is complete and merged** (PR #21): added `PRODUCT.md`, release-readiness spec, and release-readiness roadmap track.
- **R1 first-run setup readiness is complete and merged** (PR #22): settings shows a setup checklist, setup auto-opens when readiness is incomplete, and dictation start reports setup-needed when microphone permission or required models are missing.

## The work, in one paragraph

Wispergo is being migrated from a fully-bundled, sidecar-based offline app (~3.5 GB, `whisper-cli` + `llama-server` sidecars, dual-arch GGML dylibs) to a thin app with in-process GGML engines (`whisper-rs` + `llama-cpp-2`, statically linked, Metal, arm64-only) and a first-run asset downloader. The original "fully bundled, no downloads" spec (2026-05-01) was **superseded**; the reversal is recorded in ADR-0001. Phases 0-6, the macOS deployment-target build fix, the language UX follow-up, and the compact ZH label follow-up are merged. The next track is release readiness and UI polish for public GitHub Releases.

## Authoritative artifacts (read these, don't re-derive)

- **Roadmap (source of truth for what's next):** `docs/superpowers/plans/2026-06-18-in-process-inference-roadmap.md` — every slice has a ✅/🟡/⬜ status and DoD. Check statuses here before starting.
- **Phase 3.2 API research:** `docs/superpowers/research/2026-06-19-llama-cpp-2-api-research.md` — pinned `llama-cpp-2 = 0.1.146` API findings with exact permalinks. This replaces the old instruction to run `librarian`.
- **Phase 3.2 design:** `docs/superpowers/specs/2026-06-19-llama-cpp-cleanup-provider-3-2-design.md` — approved and implemented in PR #9.
- **Phase 3.3 design:** `docs/superpowers/specs/2026-06-19-cleanup-sidecar-retirement-3-3-design.md` — approved and implemented in PR #10.
- **Phase 4 design:** `docs/superpowers/specs/2026-06-19-inference-manager-lifecycle-phase-4-design.md` — approved by user; Phase 4.1 implemented in PR #11.
- **Phase 4.2 design:** `docs/superpowers/specs/2026-06-19-inference-manager-wiring-phase-4-2-design.md` — approved by user; Phase 4.2 implemented in PR #12.
- **Phase 5 design:** `docs/superpowers/specs/2026-06-19-model-tiering-phase-5-design.md` — approved by user; Phase 5.1 merged in PR #14.
- **Phase 5.2 redesign:** `docs/superpowers/specs/2026-06-20-punctuation-safety-redesign-phase-5-2.md` — approved by user and merged in PR #15. Punctuation-only LLM output is untrusted and safety-gated before insertion.
- **Phase 5.2 implementation plan:** `docs/superpowers/plans/2026-06-20-punctuation-safety-redesign-implementation.md` — implemented and merged in PR #15.
- **Phase 5.3 implementation plan:** `docs/superpowers/plans/2026-06-20-full-cleanup-pack-implementation.md` — implemented and merged in PR #16.
- **Design spec:** `docs/superpowers/specs/2026-06-18-in-process-inference-and-asset-downloader-design.md` — includes the reversal table vs. the superseded 2026-05-01 spec.
- **ADR-0001 (the reversal):** `docs/adr/0001-thin-app-downloader-supersedes-bundled-inference.md`
- **Superseded spec (do not follow, but read for context):** `docs/superpowers/specs/2026-05-01-offline-apple-inference-design.md`
- **Glossary:** `CONTEXT.md` — canonical terms (Asset, Asset Manifest, Model Pack, Inference Manager, Inference Engine, "offline-after-setup").
- **PRODUCT.md** — strategic product context for UI/release polish.
- **Release-readiness spec:** `docs/superpowers/specs/2026-06-20-release-readiness-and-ui-polish-design.md`.
- **R1 implementation plan:** `docs/superpowers/plans/2026-06-20-r1-first-run-setup-readiness.md`.
- **README** — updated through Phase 6 and the language UX follow-up.

## Phase/slice status snapshot

| Phase | Status | Merged via |
|---|---|---|
| 0 Foundations (manifest + storage) | ✅ | PRs #1 |
| 1 Asset Downloader (core + command + integrity) | ✅ | PRs #2, #3, #4 |
| 2 In-Process ASR (build + provider + switchover) | ✅ | PRs #5, #6, #7 |
| 3 In-Process Cleanup | ✅ | PRs #8, #9, #10 |
| 4 InferenceManager lifecycle | ✅ | PRs #11, #12 |
| 5 Model tiering + readiness gate | ✅ | PRs #14, #15, #16 |
| 6 Retire bundled path + Intel + README | ✅ | PR #18 |
| Language UX follow-up | ✅ | PR #19 |
| Compact ZH label follow-up | ✅ | PR #20 |
| Release readiness and UI polish | 🟡 R3 implementation | — |
| 7 Streaming (optional follow-on) | ⬜ deferred | — |

## How this project runs (standing conventions — follow these)

From `AGENTS.md` and the user's documented workflow:

1. **Review-gated loop, slice by slice.** Each slice: define DoD up front → implement on a **fresh feature branch** → verify (build + tests + clippy) → push → open PR via `gh` → **wait for user to merge** → sync main → clean up branch → recommend next slice → wait for go-ahead.
2. **PR convention:** standard GitHub merge title (`gh pr merge <n> --merge --delete-branch`). User confirmed this.
3. **Bridge state discipline:** every merge keeps main shippable. Additive/switch slices land before removal slices. "Remove old" only after "new is default and verified."
4. **`gh` CLI is used for push + PR + merge** (SSH key isn't loaded; remote is HTTPS with gh's token — already configured via `gh auth setup-git`). Active account: `ribbons-digital`.
5. **Package installs:** `sfw pnpm install <pkg>` (JS only). Rust deps go in `Cargo.toml` directly. System build deps via `brew` (cmake already installed).
6. **Shippability gate before every PR:** `cargo build --workspace` (0 warnings) + `cargo test --workspace` + `cargo clippy -p wispergo-core --all-targets -- -D warnings` + `cargo clippy -p wispergo-core --all-targets --features llama-cpp -- -D warnings` + `cargo clippy -p wispergo-desktop --all-targets -- -D warnings` + `pnpm test:ts`. Run all six; report results.
7. **Revert the stray `package.json` `packageManager` field** that `pnpm test:ts` auto-adds — it's out of scope for every slice. `git checkout -- package.json` before committing.
8. **Sole maintainer/user = the user (shiang).** This justifies aggressive simplifications: no need to keep dark fallbacks, feature can flip on by default, no multi-user concerns. The user explicitly chose "1a + 2a" (delete sidecar outright, feature on by default) over the conservative "keep dark fallback" options.

## Recently completed slice: Phase 3.2 — `LlamaCppCleanupProvider` behind existing traits

**DoD (from roadmap):** New provider implementing `TextCleanupProvider` + `CleanupProvider` using the **same prompt contract** as `crates/wispergo-core/src/llama_server.rs` (reuse `punctuation_system_prompt`, `cleanup_system_prompt`, `parse_punctuation_cleanup_text`, `parse_cleanup_json` verbatim — only transport changes from HTTP to in-process completion). Provider tests with a tiny GGUF fixture; prompt-output parsing reuses the existing parsers.

**Research status:** Complete and saved in `docs/superpowers/research/2026-06-19-llama-cpp-2-api-research.md`. Key findings: no high-level completion helper; use backend → model → context → chat template → tokenization → `LlamaBatch` decode loop → greedy sampling → `token_to_piece`; stop on `model.is_eog_token(token)`. `LlamaContext<'a>` borrows `LlamaModel`, which is the main ownership-design risk.

**Design status:** Approved by the user on 2026-06-19 in `docs/superpowers/specs/2026-06-19-llama-cpp-cleanup-provider-3-2-design.md`. Approved choices: extract shared prompt/parsing contract to a new module, use a per-request local llama engine for 3.2 to avoid unsafe/self-referential ownership, defer persistent lifecycle/perf to Phase 4, and refine the tiny-GGUF test DoD to CI fake-seam tests plus an ignored `WISPERGO_LLAMA_TEST_GGUF` integration test.

**Implementation status:** Merged in PR #9. Shared `cleanup_prompt` extraction is complete; `llama_server.rs` and `ollama.rs` reuse it; `LlamaCppCleanupProvider` implements `TextCleanupProvider` + `CleanupProvider` behind `llama-cpp`; the real local llama.cpp engine constructor compiles; and the ignored `WISPERGO_LLAMA_TEST_GGUF` integration test exists.

## Recently completed slice: Phase 3.3 — retire cleanup sidecar + process runtime

**Design status:** Approved in `docs/superpowers/specs/2026-06-19-cleanup-sidecar-retirement-3-3-design.md`.

**Implementation status:** Merged in PR #10. Deleted the retired HTTP cleanup provider and tests; flipped `llama-cpp` on by default; changed recording to use `LlamaCppCleanupProvider` for local cleanup while preserving the Ollama dev override; replaced process runtime internals with a lightweight `cleanup_runtime_status` bridge; removed cleanup sidecar binary checks from scripts/README.

## Recently completed slice: Phase 4.1 — `InferenceManager` lifecycle core

**Design status:** Approved in `docs/superpowers/specs/2026-06-19-inference-manager-lifecycle-phase-4-design.md`.

**Implementation status:** Merged in PR #11. Added `apps/desktop/src-tauri/src/inference/manager.rs` with dedicated per-engine worker threads, command channels, lazy-load-on-request, idle unload, generation-guarded stale unload protection, failure/panic unload, reload-on-next-request, fake-engine tests, and ASR + cleanup slots.

## Recently completed slice: Phase 4.2 — `InferenceManager` recording/settings wiring

**Design status:** Approved in `docs/superpowers/specs/2026-06-19-inference-manager-wiring-phase-4-2-design.md`.

**Implementation status:** Merged in PR #12. Removed `apps/desktop/src-tauri/src/inference/cleanup_runtime.rs`; app setup now manages `InferenceManager::product()` and arms it after settings load; settings sync arms ASR/cleanup and re-arms ASR on recognition-language changes; recording routes ASR and local cleanup through the manager; Ollama override still bypasses local cleanup; cleanup manager errors still fall back to raw ASR; frontend `cleanup_runtime_status` command remains stable.

## Recently completed cleanup slice: desktop clippy gate

**Implementation status:** Merged in PR #13. Moved the `recording.rs` test module below runtime items to satisfy `items-after-test-module`; replaced manual Objective-C nul-terminated byte strings in `lib.rs` with Rust C string literals. `cargo clippy -p wispergo-desktop --all-targets -- -D warnings` now passes.

## Recently completed slice: Phase 5.1 — ASR model tiering

**Design status:** Phase 5 design approved in `docs/superpowers/specs/2026-06-19-model-tiering-phase-5-design.md`.

**Implementation status:** Merged in PR #14. Populated `models.manifest.json` with verified ASR entries for `medium` (`ggml-medium-q5_0.bin`) and `large-v3-turbo`; added `asrModelId` to Rust/TS settings schema and settings UI; changed ASR live resolution to use verified app-support Assets when the manifest is populated; added download-before-activation for selected ASR model settings; asset readiness now distinguishes `missing` from active `downloading`; successful default/repair downloads resync `InferenceManager`.

## Recently completed slice: Phase 5.2 — cleanup punctuation safety redesign

**Design status:** Approved in `docs/superpowers/specs/2026-06-20-punctuation-safety-redesign-phase-5-2.md` after raw-model eval showed Qwen2.5 0.5B, 1.5B, and 3B can translate, omit punctuation, or rewrite mixed-language content such as `小王` → `王`.

**Implementation status:** Merged in PR #15. Added `crates/wispergo-core/src/cleanup_safety.rs` with a deterministic punctuation safety gate; Punctuation-only output from both Ollama override and local `InferenceManager` cleanup is accepted only when it preserves transcript content with punctuation/capitalization-only changes; unsafe suggestions fall back to raw ASR. Added a safety-wrapped Qwen2.5-0.5B cleanup-punctuation default Asset to `models.manifest.json`; cleanup settings resolution now uses verified app-support cleanup Assets when the manifest is populated. Updated `docs/manual/offline-cleanup-eval.md` to record model suggestion, safety decision, final inserted output, safety notes, quality notes, and latency. Safety-gated eval passes safety for all fixture rows: unsafe Chinese/mixed suggestions fall back to raw ASR, while safe English/already-punctuated suggestions are accepted.

## Current slice: R3 recording waveform UI

**Issue:** The recording state should feel polished and immediate without showing a text-heavy pill while the shortcut is held.

**Implementation status:** In progress on branch `r3-recording-waveform-ui`. User approved a standalone waveform flow: holding the shortcut hides the pill and shows a waveform-only component with no visible labels; releasing hides the waveform and returns to the pill for processing/ready states. Added R3 spec and implementation plan, component/app tests, CSS waveform animation, and reduced-motion handling.

**Next step:** Finish full verification, run a manual visual smoke, open PR, and wait for user merge. Recommended next slice after merge is R4 CI and release workflow because release-polish UI slices R1-R3 are complete.

## Key gotchas learned this run (save yourself the time)

1. **serde `rename_all` on an internally-tagged enum does NOT propagate to variant fields.** Add `#[serde(rename_all = "camelCase")]` per variant. Bit me in 1.2 (`AssetDownloadStatus`) — a unit test caught it.
2. **`gh pr create` body with backticks breaks shell heredocs.** Write the PR/commit body to `/tmp/*.md` and use `--body-file` / `git commit -F`. (Mangled one commit message before learning this.)
3. **`pnpm test:ts` auto-adds `packageManager` to root `package.json`.** Always `git checkout -- package.json` before staging.
4. **httpmock 0.7** uses `.hits()` (not `times_called()`) and `.body(impl AsRef<[u8]>)` (pass slices directly, not `.to_vec()`). No easy sequential/dynamic bodies — prove retry-once via `assert hits == 2` on a failing test.
5. **`whisper-rs` / `llama-cpp-2` need cmake + clang at build time.** Both installed on this machine. README documents it.
6. **Vitest mock paths:** when two test files mock the same resolved module (`../lib/tauriApi` vs `../../lib/tauriApi`), keep both mocks complete — an incomplete mock in one file can break the other when run together. Happened in 1.2.
7. **`git pull` on this machine needed `git config --global pull.rebase false`** set this session — already done, won't recur.

## Deferred items (explicitly punted, documented in roadmap)

- **Cleanup model selector remains deferred.** Phase 5 keeps cleanup model choice implicit from Cleanup Mode and manifest role/pack selection; no visible cleanup model picker exists yet.
- **Lower-level persistent llama model optimization.** Phase 4.2 intentionally wires `InferenceManager` through existing `WhisperRsProvider` and `LlamaCppCleanupProvider`; a lower-level persistent `LlamaBackend` + `LlamaModel` cleanup engine remains a focused performance follow-up if needed.
- **Streaming partial transcripts → Phase 7** remains optional/deferred. Start only if real-use validation shows clear user value.

## One thing the automated gate can't cover

PR #7 changed the **live runtime ASR path**. CI/tests pass but no model binary runs in CI. A manual smoke test (`pnpm desktop:dev` + hold-to-dictate, with a verified ASR Asset available) remains the only real proof that in-process ASR works end-to-end if runtime ASR behavior is in question.

## Suggested skills (invoke these)

- **`grill-with-docs`** — if Phase 3 surfaces a design fork worth stress-testing against the glossary/spec (the 7-fork grill at the start of this run produced ADR-0001 and the spec; the same pattern applies if 3.2/3.3 reveal ambiguities).
- **`spec-driven-coding-pair`** — general slice execution against the roadmap/spec.
- **`writing-plans`** — only if a slice grows beyond what the roadmap's DoD captures; the roadmap is already detailed.
- **`verification-before-completion`** — before every PR; the shippability gate is non-negotiable.
- **Do not use `librarian` for now** — its web approval/curator flow broke Pi's prompt interface in this project. The needed `llama-cpp-2` research is already saved in `docs/superpowers/research/2026-06-19-llama-cpp-2-api-research.md`; use that doc plus direct repo/source inspection instead.
- **`handoff`** — at the end of the next session, write the next one of these.

## Quick orientation commands for a fresh agent

```bash
git checkout main && git pull origin main                # sync (clean state as of this update)
cat docs/superpowers/plans/2026-06-18-in-process-inference-roadmap.md              # current status
cat docs/superpowers/specs/2026-06-18-in-process-inference-and-asset-downloader-design.md
cat CONTEXT.md                                                                     # glossary
cat docs/adr/0001-thin-app-downloader-supersedes-bundled-inference.md              # the reversal
gh pr list --state merged --limit 20                                               # what's landed recently
cargo build --workspace && cargo test --workspace && pnpm test:ts                  # baseline green check
```

Baseline state (as of this handoff): Phase 6 is merged via PR #18, language UX is merged via PR #19, compact ZH label is merged via PR #20, release-readiness design is merged via PR #21, R1 setup readiness is merged via PR #22, and R2 icon refresh is merged via PR #23. R3 recording waveform UI is in progress on `r3-recording-waveform-ui`. Phase 5.3 full PR gate passed before opening PR #16: `cargo build --workspace`, `cargo test --workspace`, core clippy with and without `llama-cpp`, desktop clippy, and `pnpm test:ts`. cmake + clang installed and required (the `whisper-rs` and `llama-cpp` features are on by default).
