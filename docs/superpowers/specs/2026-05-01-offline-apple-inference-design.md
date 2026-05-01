# Offline Apple Inference Architecture Design

## Status

Approved direction: Wispergo should move toward a fully offline Apple-first product that does not require Ollama, Homebrew, external CLIs, or first-run model downloads for core dictation and punctuation.

## Goals

- Ship a macOS app that works fully offline immediately after installation.
- Bundle ASR and cleanup runtimes/models inside the app package.
- Remove Ollama as a product dependency while keeping it available as an optional developer backend during transition.
- Keep the existing Auto / English / Chinese recognition controls.
- Improve punctuation quality beyond the current `qwen2.5:0.5b` Ollama setup.
- Support both Apple Silicon and Intel Macs, with Apple Silicon as the optimized experience.
- Fail gracefully: if cleanup is unavailable or too slow, insert raw ASR text rather than blocking dictation.

## Non-goals

- Do not add cloud inference.
- Do not require first-run downloads for the default product path.
- Do not expose model paths, ports, or cleanup timeouts to normal users.
- Do not replace punctuation cleanup with command execution work in this phase.
- Do not optimize primarily for small app size; product completeness and offline behavior take priority.

## Product experience

On first launch, a user should only need to grant macOS microphone and accessibility permissions. They should not be asked to install Ollama, install whisper.cpp, download models, run terminal commands, or configure model paths.

The app should start its local cleanup runtime in the background, warm the punctuation model, and be ready by the time the user finishes normal setup. If warmup is still in progress, dictation should still work and cleanup can fall back to raw ASR output when necessary.

Settings may show a simple cleanup quality mode such as Off / Punctuation / Full Cleanup, but should not expose implementation details such as model filenames, localhost ports, GGUF quantization names, or timeout values.

## Architecture overview

Wispergo will own the local inference stack:

```text
Wispergo.app/
  Contents/
    MacOS/
      wispergo-desktop
    Resources/
      bin/
        whisper-cli
        llama-server or wispergo-cleanup-sidecar
      models/
        asr/
          ggml-large-v3-turbo.bin
        cleanup/
          qwen2.5-3b-instruct-q4_k_m.gguf
```

At runtime:

1. App startup resolves bundled resource paths.
2. Cleanup runtime starts in the background and loads the punctuation model.
3. User records speech.
4. Wispergo runs bundled whisper.cpp against the captured audio using the selected language mode.
5. Wispergo sends the transcript to the warmed cleanup runtime for punctuation-only cleanup.
6. Wispergo inserts the cleaned text.
7. If cleanup fails, times out, or is not ready, Wispergo inserts raw ASR output.

## ASR design

### Runtime

Use a bundled `whisper.cpp` sidecar binary. The current `WhisperSidecarProvider` pattern remains a good boundary, but production defaults should resolve to bundled resources rather than environment variables or user settings.

### Model

Use `ggml-large-v3-turbo` as the default bundled ASR model.

### Language behavior

Keep the existing language mapping:

- Auto: no explicit Whisper language argument
- English: `--language en`
- Chinese: `--language zh`

Chinese remains generic Whisper `zh`; no Traditional/Simplified conversion is introduced.

### Error handling

If bundled ASR assets are missing or fail to execute, show a clear app-level error that the installation is damaged. For normal ASR runtime failures, keep the existing safe error path and do not attempt cloud fallback.

## Cleanup design

### Runtime

Use a Wispergo-managed `llama.cpp`-based local runtime instead of Ollama. The runtime should be persistent across recordings so the model stays warm in memory.

There are two viable implementation shapes:

1. Start a bundled `llama-server` process bound to localhost on an app-managed port.
2. Build a narrower `wispergo-cleanup-sidecar` around llama.cpp that exposes only the cleanup operation Wispergo needs.

The first implementation should prefer the `llama-server` sidecar because it is faster to build and closer to the current Ollama HTTP provider. A custom narrow sidecar can come later if packaging, security, or lifecycle control becomes difficult.

### Model

Use a larger multilingual text model than the current `qwen2.5:0.5b` default. Recommended initial bundled cleanup model:

- Qwen2.5-3B-Instruct GGUF
- Quantization: start with `Q4_K_M`; evaluate `Q5_K_M` if quality gain justifies size/latency

This is a balanced quality/speed choice for English and Chinese punctuation. It should provide better punctuation accuracy than the current 0.5B model without jumping to a model size that makes Intel Macs unusable.

### Prompt contract

For punctuation-only cleanup, keep the narrow plain-text contract:

- return only the corrected transcript as plain text
- add punctuation and capitalization only
- preserve exact words, language, and script
- do not translate
- do not paraphrase
- do not summarize
- do not add or remove words
- do not classify or execute commands

Full cleanup can continue using the structured JSON path, but punctuation-only remains the default product path.

### Fallback behavior

Cleanup is best-effort. If the cleanup runtime is not ready, returns invalid output, or exceeds its internal timeout, Wispergo inserts raw ASR text. This keeps dictation responsive and avoids making punctuation a hard dependency for basic typing.

## Runtime lifecycle

A new desktop-side runtime manager should own cleanup process lifecycle:

- resolve bundled runtime/model paths
- choose the correct binary for the current architecture if needed
- start the cleanup server in the background
- wait for readiness without blocking app launch
- warm the model with a short prompt
- restart the process if it crashes
- shut it down on app exit

The manager should expose simple state to the app:

- disabled
- starting
- ready
- unavailable
- failed

Normal users should not see raw process errors unless the installation is broken. Diagnostics can be logged for support.

## Apple Silicon and Intel support

Wispergo should support both Apple Silicon and Intel Macs.

Apple Silicon is the performance target and should use Metal acceleration where available. Intel support should be functional but may have slower ASR and cleanup. The app should keep short cleanup timeouts and fallback to raw ASR if cleanup is too slow.

Packaging may use either universal binaries or per-architecture binaries selected at runtime. The implementation should start with the simplest reliable packaging approach and verify both architectures before release.

## Configuration strategy

Current developer-oriented configuration paths should remain during transition:

- environment-variable overrides for ASR and cleanup backends
- Ollama provider for local experimentation
- explicit settings paths for debugging if already present

Product defaults should not depend on those paths. In the finished app, default resolution should prefer bundled assets first.

## Testing strategy

Add tests around boundaries rather than model quality:

- bundled resource path resolution
- architecture-specific binary selection
- ASR command construction with bundled default paths
- language argument mapping
- cleanup provider request/response parsing
- cleanup fallback on timeout, invalid output, and unavailable runtime
- runtime manager state transitions
- no transcript content in timing diagnostics

Model quality should be evaluated with a small manual fixture set containing English, Chinese, and mixed English/Chinese dictation examples. That fixture set should compare raw Whisper output against cleanup output and record latency.

## Migration plan

1. Keep current Ollama cleanup path as a working development baseline.
2. Add bundled-resource resolution for ASR and cleanup assets.
3. Package whisper.cpp and `ggml-large-v3-turbo` in the macOS bundle.
4. Add a llama.cpp cleanup provider behind the same cleanup interface.
5. Add cleanup runtime manager and warmup lifecycle.
6. Switch product default cleanup provider from Ollama to bundled llama.cpp.
7. Keep Ollama as a hidden developer override until the bundled path is stable.
8. Remove user-facing local model path setup once bundled assets are reliable.

## Open evaluation items

These are implementation evaluation items, not unresolved product requirements:

- exact Qwen2.5-3B-Instruct GGUF source and quantization after local latency/quality testing
- universal binary versus per-architecture sidecar packaging
- whether `llama-server` is sufficient or a custom cleanup sidecar is needed later
- exact internal cleanup timeout after measuring the bundled model on Apple Silicon and Intel

## Approval checkpoint

This design captures the approved direction: fully offline Apple-first packaging, bundled `ggml-large-v3-turbo` ASR, bundled llama.cpp cleanup, Qwen2.5-3B-Instruct-class punctuation model, no Ollama dependency for product users, and graceful fallback to raw ASR.
