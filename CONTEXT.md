# Wispergo

A local-first macOS dictation app. Hold a global shortcut, speak, and cleaned
text is inserted into the focused app. Speech recognition and punctuation
cleanup run on-device; the app ships thin and downloads model weights on first
run.

## Language

**Asset**:
A single downloadable model weight file (`.bin` for ASR, `.gguf` for cleanup)
stored under the app-support models directory, addressed by id and verified by
SHA-256.
_Avoid_: model file, bundle resource, packaged binary.

**Asset Manifest**:
A small JSON file bundled inside the app that lists every Asset — id, role,
display name, download URL, size, and SHA-256. The downloader reads the
manifest; no asset path or URL is hardcoded in source.
_Avoid_: model list, config, resource map.

**Model Pack**:
An optional, user-elected bundle of one or more Assets downloaded on demand
beyond the defaults. Switching to a pack triggers its download before use.
_Avoid_: add-on, plugin, expansion.

**Default Assets**:
The ASR Asset and cleanup-punctuation Asset downloaded automatically on first
run. Required for dictation; the app is non-functional until the ASR default is
present.
_Avoid_: base model, starter model.

**Accuracy Pack**:
The opt-in Model Pack containing the larger ASR Asset (`large-v3-turbo`) for
users who want maximum transcription accuracy at the cost of size and latency.
_Avoid_: pro model, HD model.

**Full-cleanup Pack**:
The opt-in Model Pack containing the larger cleanup Asset (Qwen2.5-3B) required
to run Full Cleanup mode (structured-JSON intent classification). Punctuation
mode does not require it.
_Avoid_: advanced cleanup, command model.

**Inference Manager**:
The desktop-side component that owns in-process model lifecycle — lazy load on
first use, idle unload, generation-guarded reload on failure — for both ASR and
cleanup. Supersedes the former `CleanupRuntimeManager`, which managed an
out-of-process `llama-server` child.
_Avoid_: runtime manager, sidecar manager, server manager.

**Inference Engine**:
The statically-linked GGML stack (`whisper-rs` for ASR, `llama-cpp-2` for
cleanup) compiled into the app binary with Metal enabled. There are no sidecar
binaries or shared GGML dylibs shipped.
_Avoid_: MLX, sidecar, llama-server, whisper-cli (these are retired terms for
the engine layer).

**Cleanup Mode**:
A user-facing setting with three values — Off, Punctuation-only, Full cleanup —
determining whether the transcript is inserted raw, lightly punctuated, or run
through full intent classification. Punctuation is the default and uses the
default cleanup Asset; Full cleanup requires the Full-cleanup Pack.
_Avoid_: quality mode, post-processing level.

### Flagged ambiguities

- **"Offline"**: Wispergo remains offline-first, but "offline" now means
  "after first-run asset download," not "works the instant the .app is
  installed with no network." The fully-bundled offline installer variant has
  been retired. Use "offline-after-setup" when precision matters.

### Example dialogue

> **Dev**: The user enabled Full cleanup but the Inference Manager says
> Unavailable.
>
> **Domain expert**: Right — Full cleanup needs the Full-cleanup Pack. Did the
> Asset Manifest entry for the 3B model finish downloading and pass SHA-256?
>
> **Dev**: Not yet, it's still in `.part`. So Punctuation-only would still work
> off the Default Assets?
>
> **Domain expert**: Yes. Punctuation uses the default cleanup Asset which came
> down on first run. Full cleanup is gated on its pack. Until the pack Asset is
> verified, Full cleanup can't load and the Inference Manager reports
> Unavailable; dictation itself isn't blocked because ASR uses its own default
> Asset.
