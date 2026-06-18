# Roadmap: In-Process Inference & Asset Downloader

Tracking doc for the work approved in
`docs/superpowers/specs/2026-06-18-in-process-inference-and-asset-downloader-design.md`
(reversal recorded in `docs/adr/0001`).

Each slice follows the project's review-gated loop: design→approve→implement
slice with definition-of-done→verify→merge. Do not start a slice until the
previous one is merged unless explicitly parallelizable.

Legend: ⬜ not started · 🟡 in progress · ✅ done · ⛔ blocked

## Phase 0 — Foundations 🟡

- **0.1 Asset manifest format + parser** ✅
  - Define `models.manifest.json` schema (id, role, displayName, url, size,
    sha256). Add to `crates/wispergo-core` as a pure data type with unit tests.
  - DoD: parser unit-tested for valid/missing/malformed; no network.
  - Done: `crates/wispergo-core/src/asset_manifest.rs` — `AssetRole`,
    `AssetEntry`, `AssetManifest` with `from_json` + `validate` + `find`/`by_role`.
    12 unit tests, clippy-clean (`--lib`). Added `schemaVersion` field for
    forward-compat. SHA-256 validation is case-insensitive (downloader
    normalizes). No `default` flag yet — deferred to Phase 1.1 when the
    downloader needs first-run selection.

- **0.2 App-support asset storage + path resolution** ✅
  - Resolve `~/Library/Application Support/com.ribbonsdigital.wispergo/models/{asr,cleanup}/`
    from app handle. Replace `InferenceResourcePaths` bundled-path resolution
    with manifest-driven asset paths.
  - DoD: path resolution unit tests; existing `resources.rs` tests updated.
  - Done: `crates/wispergo-core/src/asset_storage.rs` — pure `AssetStorage`
    (role→subdir/extension, `asset_path`/`part_path`/`path_for` via manifest
    lookup), 9 unit tests. Desktop glue `app_support_asset_storage` in
    `inference/mod.rs` (bridge: `#[allow(dead_code)]`, not wired until Phase 1).
    Existing `InferenceResourcePaths` left untouched — removal is a later
    slice gated on the downloader.

## Phase 1 — Asset Downloader ✅

- **1.1 Downloader core (resume + SHA-256 + atomic rename)** ✅
  - New component in `apps/desktop/src-tauri/src/inference/`. HTTP range resume
    to `.part`, verify SHA-256, retry once on mismatch, atomic rename. No UI.
  - DoD: unit/integration tests for resume, mismatch→retry, corrupt→re-download,
    success path; uses a local file:// fixture, no real network in CI.
  - Done: `crates/wispergo-core/src/downloader.rs` — `Downloader` with resumable
    Range fetch (206-append vs 200-restart), SHA-256 verify, retry-once on
    mismatch, atomic rename, cached-final / cached-part short-circuits, corrupt-
    final re-download. 8 `httpmock` integration tests in
    `tests/downloader_tests.rs` (fresh, resume, range-ignored, mismatch→retry→
    fail, cached-final, corrupt-final, http-error, cleanup-gguf-path). No real
    network. Added deps: `sha2`, `futures-util`, reqwest `stream` feature.
  - Scope note: `allow_cached_part` flag added beyond DoD for re-run safety; the
    manifest's `default` field (first-run selection) is deferred to 1.2 where
    the `ensure_model_assets` command needs it.

- **1.2 `ensure_model_assets` command + frontend status events** ✅
  - Mirror `ensure_ollama_setup`: detect → download → verify → ready, emit
    status events. First-run flow downloads Default Assets only.
  - DoD: command tested; frontend shows download state; ASR-unavailable-while-
    downloading state shown (not silent raw-ASR).
  - Done: manifest `default` field + `defaults()` accessor + one-default-per-role
    validation (3 new core tests). Core `missing_defaults` + `download_defaults`
    orchestration with progress callback (5 new downloader tests). Desktop
    `commands/assets.rs` with `asset_readiness` + `ensure_model_assets` commands
    emitting `wispergo://asset-download` events, `AssetClient` state, bundled
    `resources/models.manifest.json` placeholder. Frontend: `AssetDownloadStatus`
    type, `tauriApi` wrappers, `AssetDownloadNotice` in SettingsPanel with
    live event listener + retry control (2 new TS tests).
  - **Deferred to Phase 2 (documented)**: the dictation-readiness gate
    ("downloading models" blocking dictation). Gating now would block dictation
    that currently works via the bundled sidecar, since ASR does not yet
    consume downloaded assets. The gate lands with the in-process ASR provider.
  - **Deferred to Phase 5 (documented)**: real model entries in the bundled
    manifest (URLs/sizes/SHA-256s). 1.2 ships a structural placeholder; the
    downloader is exercised by tests against a mock manifest, not real first-
    run. Frontend hides the download affordance when status is Ready (so the
    empty placeholder produces no confusing UI).
  - Bug found & fixed during slice: serde `rename_all` on an internally-tagged
    enum does not propagate to variant fields; added per-variant `rename_all`.

- **1.3 Re-verify-on-load** ✅
  - Cheap SHA-256 check before loading any asset; corrupt → re-download or
    raw-ASR fallback (cleanup only).
  - DoD: tests for corrupt-asset detection on both ASR and cleanup paths.
  - Done: core `AssetIntegrity` enum + `verify_asset` / `integrity_sweep` /
    `repair_asset` (re-downloads corrupt or missing) — 6 new downloader tests.
    Desktop `asset_integrity` (sweep) + `repair_asset_by_id` commands with
    `IntegrityReport` / `IntegrityProblem` / `AssetIntegrityStatus` types
    (3 new serialization/round-trip tests). 227 Rust tests total, 64 TS.
  - **Deferred to Phase 2/3 (documented)**: the actual load-path wiring (call
    `verify_asset` before an in-process provider loads, auto-`repair_asset` on
    corrupt). Today's sidecars don't load via `AssetStorage`, so wiring now
    would be dead code. The hook exists; the call sites land with the in-
    process providers. Phase 1 deliverable is the primitive + command, tested
    in isolation — matches the roadmap note.

> **Bridge state after Phase 1**: downloader works, but ASR/cleanup still run
> via the old sidecars reading assets from app-support instead of the bundle.
> This keeps the app functional while the engine migrates.

## Phase 2 — In-Process ASR ⬜

- **2.1 Integrate `whisper-rs`, Metal feature, build pipeline** ⬜
  - Add dependency, pin version, get a clean arm64 release build with Metal.
  - DoD: `cargo build --release` succeeds; CI builds arm64. Stop-rule: if
    `whisper-rs` Metal build is broken on the pinned version, block and
    re-pin before continuing.

- **2.2 `WhisperRsProvider` implementing `AsrProvider`** ⬜
  - New provider in `crates/wispergo-core` (or desktop) taking `f32` PCM
    directly — no temp WAV. Persistent context; language (Auto/EN/ZH) as a
    context parameter.
  - DoD: provider trait tests; language-arg parity with the retired
    `WhisperSidecarProvider`; temp-WAV code path removed.

- **2.3 Retire `whisper-cli` sidecar** ⬜
  - Remove `WhisperSidecarProvider` and `whisper-cli` from the manifest/resource
    paths. Keep the `AsrProvider` trait and `WISPERGO_WHISPER_*` env overrides
    as a dev escape hatch only.
  - DoD: no `whisper-cli` references in non-test source; tests green.

## Phase 3 — In-Process Cleanup ⬜

- **3.1 Integrate `llama-cpp-2`, pinned version, Metal build** ⬜
  - Add dependency, pin, clean arm64 release build.
  - DoD: build succeeds; version pinned in `Cargo.toml`.

- **3.2 `LlamaCppCleanupProvider` behind existing traits** ⬜
  - New provider implementing `TextCleanupProvider` + `CleanupProvider` using
    the **same prompt contract** as `crates/wispergo-core/src/llama_server.rs`
    (reuse the prompts verbatim; only transport changes from HTTP to
    in-process completion).
  - DoD: provider tests with a tiny GGUF fixture; prompt-output parsing reuses
    `parse_punctuation_cleanup_text` / `parse_cleanup_json`.

- **3.3 Retire `llama-server` sidecar + `CleanupRuntimeManager` process layer** ⬜
  - Remove `llama_server.rs` HTTP provider, `CleanupRuntimeCommand`,
    `choose_local_port`, child monitor, HTTP readiness polling.
  - DoD: no `llama-server`/`TcpListener`/`Child` references in non-test source.

## Phase 4 — InferenceManager Lifecycle ⬜

- **4.1 Lazy-load + idle-unload state machine** ⬜
  - New `InferenceManager` (replaces `CleanupRuntimeManager`) for both ASR and
    cleanup. Same frontend states/events. Lazy load on first use; idle unload
    (cleanup 5 min, ASR longer window). Generation-guarded reload-on-failure.
  - DoD: state-transition unit tests mirroring the existing
    `cleanup_runtime.rs` tests (default-unavailable, ready, failed, restart
    guard invalidated by shutdown, etc.); dedicated-thread + `catch_unwind`
    panic guards tested.

- **4.2 Wire `sync_cleanup_runtime_for_settings` → `InferenceManager`** ⬜
  - "Arm, don't load" at setup; first dictation triggers load. Settings change
    (Cleanup Mode, model id) routes through the manager.
  - DoD: integration test that no model loads at launch; loads on first
    dictation; unloads after idle.

## Phase 5 — Model Tiering ⬜

- **5.1 ASR: `medium` default + Accuracy Pack toggle** ⬜
  - Manifest entries for `medium` and `large-v3-turbo`; setting selects active
    ASR asset; switching to an absent asset triggers download.
  - DoD: settings round-trip; download-before-activate; Auto/EN/ZH all work on
    `medium`.

- **5.2 Cleanup: 0.5B default — EVAL GATE ⛔ until eval passes** ⬜
  - Run `docs/manual/offline-cleanup-eval.md` against Qwen2.5-0.5B.
  - **Stop-rule**: 0.5B is not accepted as the punctuation default until the
    fixture passes (English, Chinese, mixed, already-punctuated). On failure,
    bump to Qwen2.5-1.5B and re-run.
  - DoD: eval table filled in for the chosen model; no regression vs. current
    3B on the fixture cases.

- **5.3 Full-cleanup Pack (3B) opt-in** ⬜
  - Manifest entry; selecting Cleanup Mode = Full cleanup triggers 3B download
    if absent. Punctuation mode never requires it.
  - DoD: Full cleanup blocked-with-clear-state until 3B verified; Punctuation
    mode unaffected.

## Phase 6 — Retire Bundled Path ⬜

- **6.1 Remove bundled-asset trees and scripts** ⬜
  - Delete `apps/desktop/src-tauri/resources/bin/`,
    `apps/desktop/src-tauri/resources/models/`,
    `scripts/verify-inference-assets.sh`,
    `scripts/check-macos-bundle-inference-layout.sh`,
    `desktop:build:offline-release` script + docs.
  - DoD: `pnpm desktop:build` produces a thin app; README updated; no dead
    references.

- **6.2 Drop Intel targets** ⬜
  - Remove `macos-x86_64` paths and arch-selection logic; arm64-only build.
  - DoD: build is arm64-only; `CpuArchitecture` logic simplified or removed;
    tests updated.

- **6.3 README + docs refresh** ⬜
  - Update README to describe thin-app + first-run download; mark 2026-05-01
    spec as superseded (already done in doc header).
  - DoD: README matches reality; stale instructions removed.

## Phase 7 — Streaming (follow-on, separate spec) ⬜

- **7.x Live partial transcripts on the persistent Whisper context** ⬜
  - Out of scope for this roadmap. Spawn a new spec when Phase 6 is merged.
  - Not started until engine migration is verified stable in real use.
