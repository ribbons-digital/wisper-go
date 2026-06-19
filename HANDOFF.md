# Handoff — Wispergo In-Process Inference Migration

**Date:** 2026-06-19 (updated after Phase 4.1 local verification)
**Next session focus:** Commit/push/open PR for completed **Phase 4.1 `InferenceManager` lifecycle core** on branch `phase-4-1-inference-manager-lifecycle`, then wait for user merge. After merge, sync main/clean branch and scope Phase 4.2 recording/settings wiring. Do **not** use the `librarian` skill for this project unless its Pi prompt-interface issue is fixed.

> **Standing rule:** This file is tracked and is kept in sync with the roadmap whenever the roadmap changes. If the roadmap says phase X.Y is ✅, this file must reflect that. A fresh agent should be able to read this + the roadmap and continue without re-deriving state.

## Where things stand right now

- **Phase 2 (In-Process ASR) is complete and merged** (PR #7). The `whisper-cli` sidecar is deleted; `whisper-rs` is the default ASR.
- **Phase 3.1 is complete and merged** (PR #8): `llama-cpp-2` pinned at 0.1.146 as an optional, off-by-default `llama-cpp` cargo feature; Metal build verified on arm64.
- **Phase 3.2 is complete and merged** (PR #9): `LlamaCppCleanupProvider` exists behind the existing traits/feature, with shared prompt/parsing contract and approved fake-seam + ignored real-GGUF test strategy.
- **Phase 3.3 is complete and merged** (PR #10): cleanup sidecar path is deleted, `llama-cpp` is on by default, recording uses `LlamaCppCleanupProvider`, and `cleanup_runtime_status` is now a lightweight bridge until Phase 4.
- **Local `main` was in sync** with `origin/main` after PR #10; current work is on feature branch `phase-4-1-inference-manager-lifecycle`.

## The work, in one paragraph

Wispergo is being migrated from a fully-bundled, sidecar-based offline app (~3.5 GB, `whisper-cli` + `llama-server` sidecars, dual-arch GGML dylibs) to a thin app with in-process GGML engines (`whisper-rs` + `llama-cpp-2`, statically linked, Metal, arm64-only) and a first-run asset downloader. The original "fully bundled, no downloads" spec (2026-05-01) was **superseded**; the reversal is recorded in ADR-0001. Phases 0, 1, and 2 are merged. Phase 3 is next.

## Authoritative artifacts (read these, don't re-derive)

- **Roadmap (source of truth for what's next):** `docs/superpowers/plans/2026-06-18-in-process-inference-roadmap.md` — every slice has a ✅/🟡/⬜ status and DoD. Check statuses here before starting.
- **Phase 3.2 API research:** `docs/superpowers/research/2026-06-19-llama-cpp-2-api-research.md` — pinned `llama-cpp-2 = 0.1.146` API findings with exact permalinks. This replaces the old instruction to run `librarian`.
- **Phase 3.2 design:** `docs/superpowers/specs/2026-06-19-llama-cpp-cleanup-provider-3-2-design.md` — approved and implemented in PR #9.
- **Phase 3.3 design:** `docs/superpowers/specs/2026-06-19-cleanup-sidecar-retirement-3-3-design.md` — approved and implemented in PR #10.
- **Phase 4 design:** `docs/superpowers/specs/2026-06-19-inference-manager-lifecycle-phase-4-design.md` — approved by user; Phase 4.1 implemented locally.
- **Design spec:** `docs/superpowers/specs/2026-06-18-in-process-inference-and-asset-downloader-design.md` — includes the reversal table vs. the superseded 2026-05-01 spec.
- **ADR-0001 (the reversal):** `docs/adr/0001-thin-app-downloader-supersedes-bundled-inference.md`
- **Superseded spec (do not follow, but read for context):** `docs/superpowers/specs/2026-05-01-offline-apple-inference-design.md`
- **Glossary:** `CONTEXT.md` — canonical terms (Asset, Asset Manifest, Model Pack, Inference Manager, Inference Engine, "offline-after-setup").
- **README** — already updated for in-process ASR; full refresh is Phase 6.

## Phase/slice status snapshot

| Phase | Status | Merged via |
|---|---|---|
| 0 Foundations (manifest + storage) | ✅ | PRs #1 |
| 1 Asset Downloader (core + command + integrity) | ✅ | PRs #2, #3, #4 |
| 2 In-Process ASR (build + provider + switchover) | ✅ | PRs #5, #6, #7 |
| 3 In-Process Cleanup | ✅ | PRs #8, #9, #10 |
| 4 InferenceManager lifecycle | ⬜ | — |
| 5 Model tiering + readiness gate | ⬜ | — |
| 6 Retire bundled path + Intel + README | ⬜ | — |
| 7 Streaming (follow-on) | ⬜ | — |

## How this project runs (standing conventions — follow these)

From `AGENTS.md` and the user's documented workflow:

1. **Review-gated loop, slice by slice.** Each slice: define DoD up front → implement on a **fresh feature branch** → verify (build + tests + clippy) → push → open PR via `gh` → **wait for user to merge** → sync main → clean up branch → recommend next slice → wait for go-ahead.
2. **PR convention:** standard GitHub merge title (`gh pr merge <n> --merge --delete-branch`). User confirmed this.
3. **Bridge state discipline:** every merge keeps main shippable. Additive/switch slices land before removal slices. "Remove old" only after "new is default and verified."
4. **`gh` CLI is used for push + PR + merge** (SSH key isn't loaded; remote is HTTPS with gh's token — already configured via `gh auth setup-git`). Active account: `ribbons-digital`.
5. **Package installs:** `sfw pnpm install <pkg>` (JS only). Rust deps go in `Cargo.toml` directly. System build deps via `brew` (cmake already installed).
6. **Shippability gate before every PR:** `cargo build --workspace` (0 warnings) + `cargo test --workspace` + `cargo clippy -p wispergo-core --all-targets -- -D warnings` + `pnpm test:ts` (64 TS tests). Run all four; report results.
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

## Current slice: Phase 4.1 — `InferenceManager` lifecycle core

**Design status:** Approved in `docs/superpowers/specs/2026-06-19-inference-manager-lifecycle-phase-4-design.md`.

**Implementation status:** Implemented and verified locally on branch `phase-4-1-inference-manager-lifecycle`. Added `apps/desktop/src-tauri/src/inference/manager.rs` with dedicated per-engine worker threads, command channels, lazy-load-on-request, idle unload, generation-guarded stale unload protection, failure/panic unload, reload-on-next-request, fake-engine tests, and ASR + cleanup slots. No recording/settings wiring yet; Phase 4.2 owns live integration.

## Key gotchas learned this run (save yourself the time)

1. **serde `rename_all` on an internally-tagged enum does NOT propagate to variant fields.** Add `#[serde(rename_all = "camelCase")]` per variant. Bit me in 1.2 (`AssetDownloadStatus`) — a unit test caught it.
2. **`gh pr create` body with backticks breaks shell heredocs.** Write the PR/commit body to `/tmp/*.md` and use `--body-file` / `git commit -F`. (Mangled one commit message before learning this.)
3. **`pnpm test:ts` auto-adds `packageManager` to root `package.json`.** Always `git checkout -- package.json` before staging.
4. **httpmock 0.7** uses `.hits()` (not `times_called()`) and `.body(impl AsRef<[u8]>)` (pass slices directly, not `.to_vec()`). No easy sequential/dynamic bodies — prove retry-once via `assert hits == 2` on a failing test.
5. **`whisper-rs` / `llama-cpp-2` need cmake + clang at build time.** Both installed on this machine. README documents it.
6. **Vitest mock paths:** when two test files mock the same resolved module (`../lib/tauriApi` vs `../../lib/tauriApi`), keep both mocks complete — an incomplete mock in one file can break the other when run together. Happened in 1.2.
7. **`git pull` on this machine needed `git config --global pull.rebase false`** set this session — already done, won't recur.

## Deferred items (explicitly punted, documented in roadmap)

- **Dictation-readiness gate → Phase 5.** The "block dictation until ASR asset downloaded" state can't wire until the manifest has real assets (Phase 5). The bundled model is still the in-process provider's model source today. Gating now would break all dictation.
- **Real manifest entries → Phase 5.** `apps/desktop/src-tauri/resources/models.manifest.json` is an empty placeholder. Real model URLs/sizes/SHA-256s land in Phase 5 (model tiering), gated on the `offline-cleanup-eval.md` fixture for the 0.5B-vs-1.5B cleanup decision.
- **`LocalModelSettings.whisper_binary_path`** is now unused (sidecar gone) but kept to avoid a settings-schema migration; cleanup in Phase 6.
- **ASR idle-unload / lazy-load lifecycle → Phase 4** (`InferenceManager`). The `WhisperRsProvider` loads its context on first `transcribe` and holds it; no idle unload yet.
- **Streaming partial transcripts → Phase 7** (follow-on, separate spec).

## One thing the automated gate can't cover

PR #7 changed the **live runtime ASR path**. CI/tests pass but no model binary runs in CI. A manual smoke test (`pnpm desktop:dev` + hold-to-dictate, with `ggml-large-v3-turbo.bin` staged under `apps/desktop/src-tauri/resources/models/asr/`) is the only real proof that in-process ASR works end-to-end. **Recommend the user do this before starting Phase 3** — if it's broken, fixing it is higher priority than 3.1.

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
gh pr list --state merged --limit 10                                               # what's landed (8 PRs so far)
cargo build --workspace && cargo test --workspace && pnpm test:ts                  # baseline green check
```

Baseline state (as of this handoff): 226 Rust tests + 64 TS tests pass, 0 build warnings, clippy clean on core. cmake + clang installed and required (the `whisper-rs` feature is on by default since 2.3).
