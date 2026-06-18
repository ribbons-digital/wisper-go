# Thin app with asset downloader supersedes fully-bundled offline inference

**Status**: accepted — supersedes the "fully bundled, no first-run downloads"
direction approved in
`docs/superpowers/specs/2026-05-01-offline-apple-inference-design.md`.

## Context

The 2026-05-01 design mandated bundling all inference runtimes and models
inside `Wispergo.app` so the app worked fully offline immediately after
install, explicitly listing "do not require first-run downloads" and "do not
optimize primarily for small app size" as non-goals-to-violate. After shipping
that build, the costs became concrete: a ~3.5 GB app (1.5 GB ASR + 2.0 GB
cleanup models, plus a dual-arch GGML dylib forest), slow notarization and
updates that re-ship the full payload every release, and a `whisper-cli`
sidecar cold-started on every utterance causing noticeable hold-to-dictate
latency.

## Decision

Reverse the bundled direction. Ship a **thin app** that downloads model weights
into app-support on first run and keeps them across versions. Move ASR and
cleanup **in-process** via statically-linked `whisper-rs` and `llama-cpp-2`
with Metal, retiring the `whisper-cli` and `llama-server` sidecars and the
entire `resources/bin/` dylib tree. Default to smaller models (`medium` ASR,
Qwen2.5-0.5B cleanup) with larger models (`large-v3-turbo`, Qwen2.5-3B) as
opt-in packs. Drop Intel Mac support; target Apple Silicon only.

## Why

- **Distribution**: app updates drop from gigabytes to megabytes; models dedupe
  across versions; notarization stops being size-painful.
- **Latency**: an in-process persistent `whisper-rs` context eliminates the
  per-utterance model load that dominated hold-to-dictate time.
- **Size/latency fit-for-purpose**: punctuation is a low-difficulty task that a
  0.5B model handles; bundling 2 GB of 3B for it was over-provisioning. The 3B
  remains available as a pack for the Full-cleanup case where its quality
  actually matters.
- **Simplification**: one statically-linked GGML stack replaces two sidecar
  binaries plus their per-arch dylib matrix; arm64-only removes the Intel
  binary/dylib dimension entirely.

## Trade-offs accepted

- First run (and any fresh install) requires network before dictation works.
  The fully-bundled `desktop:build:offline-release` variant is retired; a
  truly air-gapped user would need a one-off custom build, not a supported SKU.
- Intel Macs are no longer supported; existing Intel users stay on the last
  Intel-compatible release.
- `llama-cpp-2` is fast-moving and not semver-stable; we pin a version and
  isolate it behind the existing `CleanupProvider` trait.
- In-process GGML means a native crash in the engine can take down the app;
  we mitigate with dedicated threads, panic guards, pinned stable versions, and
  best-effort raw-ASR fallback on the cleanup path. We deliberately do **not**
  reintroduce a sidecar for isolation — that would undo the point of the
  migration.

## Consequences

- `docs/superpowers/specs/2026-05-01-offline-apple-inference-design.md` is
  superseded by
  `docs/superpowers/specs/2026-06-18-in-process-inference-and-asset-downloader-design.md`.
- `CleanupRuntimeManager` (process/port/HTTP-readiness/child-monitor) is
  replaced by an in-process `InferenceManager` (lazy load / idle unload /
  reload-on-failure).
- `resources/bin/` and `resources/models/` trees, the
  `verify-inference-assets.sh` and `check-macos-bundle-inference-layout.sh`
  scripts, and the `desktop:build:offline-release` script are retired.
- The Ollama developer override (env vars) is retained as an alternative
  cleanup backend; it is no longer the product default and never was required
  by users.
