# Roadmap: In-Process Inference & Asset Downloader

Tracking doc for the work approved in
`docs/superpowers/specs/2026-06-18-in-process-inference-and-asset-downloader-design.md`
(reversal recorded in `docs/adr/0001`).

Each slice follows the project's review-gated loop: design→approve→implement
slice with definition-of-done→verify→merge. Do not start a slice until the
previous one is merged unless explicitly parallelizable.

Legend: ⬜ not started · 🟡 in progress · ✅ done · ⛔ blocked

## Phase 0 — Foundations ✅

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

## Phase 2 — In-Process ASR ✅

- **2.1 Integrate `whisper-rs`, Metal feature, build pipeline** ✅
  - Add dependency, pin version, get a clean arm64 release build with Metal.
  - DoD: `cargo build --release` succeeds; CI builds arm64. Stop-rule: if
    `whisper-rs` Metal build is broken on the pinned version, block and
    re-pin before continuing.
  - Done: pinned `whisper-rs = "0.16"` (resolves to 0.16.0, 2026-03-12) as an
    **optional** cargo feature in `wispergo-core`, with `metal` enabled via
    target-cfg on `cfg(all(target_os = "macos", target_arch = "aarch64"))` (Intel
    Macs and non-macOS fall back to CPU). Placeholder `whisper_rs_provider`
    module + build-integration smoke test (`linked_whisper_version()`). Verified:
    clean arm64 release build with Metal (28s), smoke test passes, clippy clean
    both default and feature-on. Feature is **off by default** (bridge state —
    the `whisper-cli` sidecar is still ASR). README prerequisites updated to
    note cmake+clang required when the feature is on. Build prereq `cmake`
    installed via Homebrew on the dev machine.
  - Stop-rule outcome: pinned 0.16.0 built cleanly first try; no re-pin needed.

- **2.2 `WhisperRsProvider` implementing `AsrProvider`** ✅
  - New provider in `crates/wispergo-core` (or desktop) taking `f32` PCM
    directly — no temp WAV. Persistent context; language (Auto/EN/ZH) as a
    context parameter.
  - DoD: provider trait tests; language-arg parity with the retired
    `WhisperSidecarProvider`; temp-WAV code path removed.
  - Done: `WhisperRsProvider` in `crates/wispergo-core/src/whisper_rs_provider.rs`
    (feature-gated). Holds a persistent `WhisperContext` in `Arc<Mutex<Option<_>>>`,
    lazily loaded on first `transcribe` and reused across calls (the latency
    win over the sidecar's per-utterance reload). Takes `f32` PCM directly —
    no temp WAV. Language (Auto/EN/ZH) maps to `FullParams::set_language`.
    Transcription runs on `spawn_blocking` with a timeout, matching the
    sidecar's `ProviderError` shape (Timeout / Failed / InvalidOutput /
    Unavailable). 7 unit tests (normalize_language, no-speech sentinels,
    builder storage, defaults, lowercase normalization).
  - **Intentional difference from sidecar**: explicit language codes are
    lowercased (whisper.cpp documents lowercase; the CLI was case-insensitive).
    Caught by a test before merge; documented as a safe normalization.
  - **Still not wired into the pipeline** (that's 2.3). Feature stays off by
    default; the `whisper-cli` sidecar remains the live ASR. Temp-WAV code in
    the sidecar is left in place until 2.3 retires the sidecar.
  - Concurrency: a `transcribe` call holds the context lock for its full
    duration, serializing transcriptions — intentional for single-user
    dictation; Phase 4 `InferenceManager` owns idle-unload, not parallelism.

- **2.3 Retire `whisper-cli` sidecar** ✅
  - Remove `WhisperSidecarProvider` and `whisper-cli` from the manifest/resource
    paths. Keep the `AsrProvider` trait and `WISPERGO_WHISPER_*` env overrides
    as a dev escape hatch only.
  - DoD: no `whisper-cli` references in non-test source; tests green.
  - Done: deleted `crates/wispergo-core/src/whisper_sidecar.rs` and
    `tests/whisper_sidecar_tests.rs` entirely (sole maintainer/user — no dark
    fallback kept). Flipped the `whisper-rs` cargo feature ON by default in
    `wispergo-core` (`default = ["whisper-rs"]`), so every build (incl. core
    tests) now requires cmake + clang. Removed the `#![cfg(feature)]` gate from
    `whisper_rs_provider.rs`. Rewrote `commands/recording.rs`: `local_asr_provider`
    now returns `WhisperRsProvider`; `AsrPaths { binary_path, model_path }`
    collapsed to a single model-path resolver (`resolve_asr_model_path_*`);
    `find_in_path` + `WISPERGO_WHISPER_BIN` removed. Rewrote 7 sidecar path
    tests as 6 model-path-only tests. README: dropped `whisper-cli` from
    staging and env-override docs; noted ASR runs in-process.
  - **Deferred to Phase 5 (documented)**: the dictation-readiness gate (block
    dictation until the ASR default asset is downloaded). The in-process
    provider still reads the **bundled** model path; the gate + app-support
    path swap lands in Phase 5 when the manifest is populated with real assets.
    Gating now (empty manifest) would break all dictation.
  - `LocalModelSettings.whisper_binary_path` field left in place (unused by
    ASR now); full settings-shape cleanup is Phase 6.

## Phase 3 — In-Process Cleanup ✅

- **3.1 Integrate `llama-cpp-2`, pinned version, Metal build** ✅
  - Add dependency, pin, clean arm64 release build.
  - DoD: build succeeds; version pinned in `Cargo.toml`.
  - Done: pinned `llama-cpp-2 = "0.1.146"` (latest, 2026-04-30, not yanked, 628k
    downloads) as an **optional** cargo feature `llama-cpp` in `wispergo-core`,
    OFF by default (cleanup is still sidecar-based via `llama-server` until 3.3).
    `metal` via target-cfg on Apple Silicon; CPU elsewhere. Placeholder
    `cleanup_inprocess` module + smoke test. Probed in an isolated temp crate
    first (mirroring 2.1). Verified: clean arm64 release build with Metal (40s),
    smoke test passes, clippy clean both ways, default build unchanged.
  - Stop-rule outcome: 0.1.146 built cleanly first try; no re-pin needed.
    (`llama-cpp-sys-2` resolved to 0.1.150 transitively — the safe crate's
    `^0.1.146` permits it; normal for this fast-moving crate.)

- **3.2 `LlamaCppCleanupProvider` behind existing traits** ✅
  - New provider implementing `TextCleanupProvider` + `CleanupProvider` using
    the **same prompt contract** as `crates/wispergo-core/src/llama_server.rs`
    (reuse the prompts verbatim; only transport changes from HTTP to
    in-process completion).
  - DoD: provider tests with a tiny GGUF fixture; prompt-output parsing reuses
    `parse_punctuation_cleanup_text` / `parse_cleanup_json`.
  - Research complete: `docs/superpowers/research/2026-06-19-llama-cpp-2-api-research.md`.
  - Done: shared cleanup prompt/parsing contract extracted to
    `crates/wispergo-core/src/cleanup_prompt.rs`; `llama_server.rs` and
    `ollama.rs` now reuse it while preserving provider-specific errors.
    `LlamaCppCleanupProvider` in `cleanup_inprocess.rs` implements
    `TextCleanupProvider` + `CleanupProvider` behind the existing `llama-cpp`
    feature, using model chat templates + greedy decode loop + existing parsers.
    Tests: shared prompt/parser integration tests, fake completion-seam provider
    tests, and ignored `WISPERGO_LLAMA_TEST_GGUF` real-GGUF integration test.
    Approved DoD refinement: no committed tiny GGUF fixture.

- **3.3 Retire cleanup sidecar + `CleanupRuntimeManager` process layer** ✅
  - Remove retired HTTP provider, sidecar process command, local-port selection,
    child monitor, and HTTP readiness polling.
  - DoD: no retired cleanup sidecar / local-port / child-process references in
    active app/core/README/scripts.
  - Done: deleted `crates/wispergo-core/src/llama_server.rs` and its tests;
    flipped `llama-cpp` on by default; changed recording to use
    `LlamaCppCleanupProvider` for local cleanup while preserving the Ollama dev
    override; replaced process runtime internals with a lightweight
    `cleanup_runtime_status` bridge until Phase 4 `InferenceManager`; removed
    cleanup sidecar binary requirements from bundle verification scripts/docs.

## Phase 4 — InferenceManager Lifecycle ✅

- **4.1 Lazy-load + idle-unload state machine** ✅
  - New `InferenceManager` lifecycle core for ASR and cleanup slots. Same
    frontend-compatible states. Lazy load on first request; idle unload;
    generation-guarded stale unload protection; reload-on-next-request after
    failure.
  - Done in PR #11: added `apps/desktop/src-tauri/src/inference/manager.rs`
    with dedicated per-engine worker threads, command channels, fake-engine
    state transition tests, `catch_unwind` panic guards, idle unload tests, and
    ASR + cleanup manager slots. No recording/settings wiring yet; 4.2 owns
    that.
  - Design approved: `docs/superpowers/specs/2026-06-19-inference-manager-lifecycle-phase-4-design.md`.

- **4.2 Wire recording/settings → `InferenceManager`** ✅
  - "Arm, don't load" at setup; first dictation triggers ASR load. Settings
    changes route through the manager. Local cleanup loads only when Cleanup
    Mode is not Off and Ollama override is absent.
  - Done in PR #12: removed the temporary `CleanupRuntimeManager` bridge; kept
    the frontend `cleanup_runtime_status` command stable; app setup and settings
    sync now arm/disable `InferenceManager`; recording requests ASR and local
    cleanup through the manager; Ollama override still bypasses local cleanup;
    cleanup errors still fall back to raw ASR.
  - Tests added/updated for no-load-at-sync, first-request-loads, Cleanup Mode
    Off disables cleanup, cleanup fallback, Ollama override, and recognition
    language re-arm.
  - Design approved: `docs/superpowers/specs/2026-06-19-inference-manager-wiring-phase-4-2-design.md`.

## Phase 5 — Model Tiering ✅

- **5.1 ASR: `medium` default + Accuracy Pack toggle** ✅
  - Manifest entries for `medium` and `large-v3-turbo`; setting selects active
    ASR asset; switching to an absent asset triggers download.
  - Done in PR #14: populated ASR manifest entries; user-facing/default id
    `medium` points to quantized `ggml-medium-q5_0.bin`; added `asrModelId` to
    local model settings and UI; app-support ASR Asset paths are used when the
    manifest is populated; settings save downloads/verifies selected ASR before
    activation; asset readiness distinguishes missing vs active downloading;
    default downloads resync `InferenceManager` on success.
  - Design approved: `docs/superpowers/specs/2026-06-19-model-tiering-phase-5-design.md`.

- **5.2 Cleanup punctuation safety redesign** ✅
  - Done in PR #15: raw-model eval failed for Qwen2.5 0.5B, 1.5B, and 3B, so
    Punctuation-only now treats LLM output as an untrusted suggestion; a
    deterministic safety gate accepts only punctuation/capitalization-only
    changes and falls back to raw ASR for unsafe suggestions.
  - Added the safety-wrapped Qwen2.5-0.5B cleanup-punctuation default Asset.
  - Manual eval records model suggestion, safety decision, final inserted output,
    safety notes, quality notes, and latency.
  - Design approved: `docs/superpowers/specs/2026-06-20-punctuation-safety-redesign-phase-5-2.md`.

- **5.3 Full-cleanup Pack (3B) opt-in** ✅
  - Done in PR #16: added the Qwen2.5-3B-Instruct `cleanup_full` manifest Asset
    with `default: false`, so it is not part of first-run/default downloads.
  - Selecting Cleanup Mode = Full cleanup downloads/verifies the Full-cleanup
    Pack before activation; if download/verification fails, previous settings
    remain active.
  - Punctuation-only remains unaffected by a missing Full-cleanup Pack.
  - `WISPERGO_CLEANUP_BACKEND=ollama` remains a dev override and does not require
    local `cleanup_full` Assets.
  - Full verification gate passed before PR #16.

## Build-fix slice — macOS deployment target ✅

- **Plain desktop build after in-process GGML dependencies** ✅
  - Done in PR #17: set Tauri `bundle.macOS.minimumSystemVersion` to `10.15`
    and route root `desktop:build` / `desktop:dev` through wrappers that export
    aligned Cargo and CMake deployment-target variables.
  - Verified before PR: cold native build dirs + plain `pnpm desktop:build`
    passed without manual env prefixes; desktop clippy passed.

## Phase 6 — Retire Bundled Path ✅

- **6.1 Remove bundled-asset trees and scripts** ✅
  - Delete `apps/desktop/src-tauri/resources/bin/`,
    `apps/desktop/src-tauri/resources/models/`,
    `scripts/verify-inference-assets.sh`,
    `scripts/check-macos-bundle-inference-layout.sh`,
    `desktop:build:offline-release` script + docs.
  - Keep `apps/desktop/src-tauri/resources/models.manifest.json` and bundle only
    that manifest at `resources/models.manifest.json`.
  - DoD: `pnpm desktop:build` produces a thin app; bundle contains no `bin/`,
    no `models/`, no `.bin`/`.gguf`/`.dylib`/sidecar artifacts, and remains
    under the thin-app size budget; README updated; no dead references.

- **6.2 Drop Intel targets and bundled fallback paths** ✅
  - Remove `macos-x86_64` paths, `CpuArchitecture`, and
    `InferenceResourcePaths` bundled-path resolution.
  - Remove legacy `LocalModelSettings` path fields (`whisperBinaryPath` /
    `whisperModelPath`) while keeping old saved keys serde-compatible.
  - DoD: live model resolution is manifest/app-support based, with
    `WISPERGO_WHISPER_MODEL` retained as the ASR dev override; tests updated.

- **6.3 README + docs refresh** ✅
  - Update README to describe thin-app + first-run download; mark 2026-05-01
    spec as superseded (already done in doc header).
  - DoD: README matches reality; stale instructions removed; runtime ASR smoke
    result documented before PR.

## Language UX follow-up ✅

- **Fast language switch + mixed Chinese/English labeling** ✅
  - Issue: language-only switching re-resolved the ASR Asset through the normal
    integrity path, which can hash the selected model file and make switching
    feel delayed.
  - Issue: Whisper Auto can bias toward the first spoken language; observed
    mixed Chinese/English works better when forcing `zh` if Chinese content is
    expected.
  - DoD: language-only switches re-arm ASR from the present selected Asset
    without re-hashing; normal model/settings resolution still verifies
    integrity; UI/README present Chinese mode as Chinese / Mixed while the
    compact floating badge remains `ZH`; tests updated; manual switch smoke
    confirms the control feels immediate.

## Release readiness and UI polish 🟡

- **R0 Product context + release-readiness spec** ✅
  - Spec: `docs/superpowers/specs/2026-06-20-release-readiness-and-ui-polish-design.md`.
  - DoD: `PRODUCT.md` exists; release-readiness spec covers first-run setup,
    model downloads, icons, recording waveform, GitHub Actions, signing,
    notarization, docs, and implementation slices.

- **R1 First-run setup and model readiness UX** ✅
  - Plan: `docs/superpowers/plans/2026-06-20-r1-first-run-setup-readiness.md`.
  - Auto-show setup when permissions or required default Assets are missing.
  - Make model download status, retry, and dictation readiness obvious to
    non-developer users.
  - DoD: clean-app-support smoke reaches a working default setup without manual
    model placement; tests cover setup/readiness states.

- **R2 Icon refresh** ✅
  - Replace app/Dock icon and add a separate menu bar/tray icon optimized for
    small light/dark system chrome.
  - DoD: `pnpm desktop:build` includes release-quality assets; manual smoke
    verifies Dock and menu bar contrast.
  - Implementation: app icon now uses a high-contrast full-color tile;
    menu-bar/tray icon uses separate `tray-template.png` wired with macOS
    template rendering for light/dark appearances.

- **R3 Recording waveform UI** ✅
  - Replace the text-heavy recording pill with a compact waveform while holding
    the shortcut; keep processing and idle states distinct; respect reduced
    motion.
  - DoD: frontend tests cover state rendering; manual hold-to-dictate smoke
    verifies recording and processing visuals.
  - Implementation: active recording now renders a standalone waveform-only
    surface with no visible labels; idle/setup/processing keep the existing pill;
    reduced-motion disables waveform animation.

- **R3.5 Settings and menu polish** ✅
  - Reshape Settings into a compact product dashboard and replace engineering
    copy with user-facing release polish.
  - Change the menu bar icon so left-click opens a nested native menu with quick
    Language, Dictation model, Cleanup, and Microphone choices above Open Settings.
  - DoD: ready-state settings fit without routine scrolling; fallback-policy
    diagnostic copy is hidden; menu behavior/tests cover left-click nested menu.

- **R4 CI and release workflow** ⬜
  - Add PR CI and tag-based release workflow for signed/notarized macOS release
    artifacts.
  - DoD: PR gates run in GitHub Actions; release docs describe required Apple
    Developer credentials and release steps.

- **R5 Public README and contributor docs** ⬜
  - Split end-user install/setup docs from developer/contributor workflow.
  - DoD: README and release docs are usable by non-developer downloaders and
    source contributors.

## Phase 7 — Streaming (optional follow-on) ⬜

- **7.x Live partial transcripts on the persistent Whisper context** ⬜
  - Deferred. The re-architecture is complete without this phase.
  - Only start if real-use validation shows live partial transcripts would have
    clear user value.
