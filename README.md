# Wispergo

Wispergo is a local-first macOS dictation app built with Tauri, React, Rust, whisper.cpp, and llama.cpp. It provides a small floating recorder, a separate floating language toggle, and text insertion into the active app.

## Features

- Hold `Command + Shift + Space` to dictate.
- In-process offline speech recognition with whisper.cpp after first-run model download.
- In-process offline cleanup for punctuation-only cleanup or full cleanup/classification after model download, without translating original language.
- Recognition language modes: Auto, English, and Chinese / Mixed Chinese-English.
- Floating status-only recorder pill.
- Separate floating language control that cycles Auto → EN → ZH.
- macOS microphone and accessibility permission handling.
- Clipboard/accessibility-based insertion with diagnostics when direct insertion is unavailable.

## Using the app

1. Build or open `Wispergo.app`.
2. On first launch, Wispergo opens setup if permissions or required local models are missing.
3. Grant microphone permission.
4. Grant accessibility permission so Wispergo can insert text into other apps.
5. Let Wispergo download the required default local models. This is a one-time setup step.
6. Choose a recognition language:
   - **Auto**: let Whisper detect the language. Auto can bias toward the first spoken language.
   - **English**: passes `--language en`.
   - **Chinese / Mixed Chinese-English**: passes `--language zh`; recommended for Chinese or mixed Chinese/English dictation.
7. Choose a cleanup mode:
   - **Off**: insert the raw Whisper transcript.
   - **Punctuation only**: default; uses local cleanup to add punctuation/capitalization only.
   - **Full cleanup and commands**: downloads the optional Full-cleanup Pack before enabling the cleanup/classification flow.
8. Hold `Command + Shift + Space`, speak, then release to transcribe and insert.

Dictation requires microphone permission and the required default local models. If you press the shortcut before setup is complete, Wispergo opens setup and shows a concise setup-needed message. Accessibility permission is required for direct insertion into other apps; without it, Wispergo may fall back to copying text.

### Offline inference

Product builds are thin: they bundle the app, in-process GGML engines, and the Asset Manifest, but not model files. On first run, Wispergo opens setup when required model Assets are missing, downloads default Assets into app support storage, and verifies them before use.

Default setup downloads:

- ASR `medium` (`ggml-medium-q5_0.bin`)
- Punctuation cleanup (`Qwen2.5-0.5B-Instruct` GGUF)

Optional settings downloads:

- ASR Accuracy Pack (`large-v3-turbo`)
- Full-cleanup Pack (`Qwen2.5-3B-Instruct` GGUF)

Normal users should not install Ollama, whisper.cpp, llama.cpp, or model files separately. After setup, speech recognition and cleanup run locally/offline. If punctuation cleanup is unavailable, unsafe, or too slow, Wispergo falls back to inserting the raw ASR transcript.

Developer overrides are available for debugging and experimentation:

```bash
export WISPERGO_WHISPER_MODEL=/path/to/ggml-model.bin
export WISPERGO_CLEANUP_BACKEND=ollama
export WISPERGO_OLLAMA_BASE_URL=http://127.0.0.1:11434
export WISPERGO_OLLAMA_MODEL=qwen2.5:3b-instruct
```

## Development

### Prerequisites

- macOS (Apple Silicon; Intel is no longer supported as of the in-process inference migration — see ADR-0001)
- Rust toolchain
- Node.js + pnpm
- Tauri v2 dependencies
- `cmake` and `clang` (required to build the in-process whisper.cpp ASR provider via `whisper-rs` and cleanup provider via `llama-cpp-2`; install with `brew install cmake`)

### Install dependencies

```bash
pnpm install
```

### Run in development

```bash
pnpm desktop:dev
```

### Build the macOS app

```bash
pnpm desktop:build
```

The build script creates/uses a local code-signing identity and signs the `.app` bundle with a stable local designated requirement.

Built app location:

```text
target/release/bundle/macos/Wispergo.app
```

If macOS rejects the local certificate, run:

```bash
pnpm desktop:trust-cert
```

### Test

Run everything:

```bash
pnpm test
```

Frontend tests only:

```bash
pnpm test:ts
```

Rust tests only:

```bash
pnpm test:rust
```

Useful direct commands:

```bash
pnpm --dir apps/desktop test
cargo test -p wispergo-desktop --lib
cargo test --workspace
```

## Project structure

```text
apps/desktop/                 Tauri + React desktop app
apps/desktop/src/             React UI, surfaces, settings, Tauri API wrapper
apps/desktop/src-tauri/       Tauri commands, macOS integration, recording, insertion
crates/wispergo-core/         Core ASR/cleanup pipeline and provider abstractions
scripts/                      Local macOS signing helper scripts
```

Main UI surfaces:

- `main`: settings window.
- `recorder`: non-clickable floating recorder/status pill.
- `language`: clickable floating language toggle.

Core runtime flow:

1. Global shortcut starts/stops recording.
2. Desktop app captures microphone audio.
3. Audio is trimmed and transcribed in-process via whisper.cpp (linked via `whisper-rs`).
4. Cleanup mode decides whether to skip cleanup, run punctuation-only local cleanup, or run full local cleanup/classification.
5. Result is inserted into the focused app or copied when insertion is unavailable.

## Troubleshooting

### Offline inference assets are unavailable

Open settings and let Wispergo download or repair model Assets. On a fresh install, Wispergo should open setup automatically when required default Assets are missing. If the Asset Manifest itself is missing, reinstall the app. Developers can also set `WISPERGO_WHISPER_MODEL` to a local GGML ASR model for ASR debugging.

### No audio or no speech detected

- Confirm microphone permission is granted.
- Check the selected microphone in settings.
- Verify the input level in macOS Sound settings.

### Text is copied but not inserted

Wispergo falls back to copying when the active target cannot be pasted into or accessibility access is unavailable. Confirm Accessibility permission is granted for Wispergo in macOS Settings.

Insertion diagnostics are written to the app data directory as `insertion-diagnostics.log`.

Recording performance timings are written to the app data directory as `recording-timings.log` and also printed to stderr when the app is launched from Terminal. On macOS this directory is typically under `~/Library/Application Support/com.ribbonsdigital.wispergo/`.

### Hover behavior for floating language control

The language toggle is a separate Tauri window. On macOS, Wispergo uses native mouse tracking so the chevron can reveal even when another app is active. Hovering the language control may activate Wispergo by design.

## Notes

- Wispergo is local-first: speech recognition and cleanup run offline by default.
- Chinese / Mixed recognition uses Whisper’s generic `zh` language code; Wispergo does not convert between Simplified and Traditional Chinese.
