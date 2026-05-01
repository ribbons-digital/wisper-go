# Wispergo

Wispergo is a local-first macOS dictation app built with Tauri, React, Rust, whisper.cpp, and llama.cpp. It provides a small floating recorder, a separate floating language toggle, and text insertion into the active app.

## Features

- Hold `Command + Shift + Space` to dictate.
- Bundled offline speech recognition with whisper.cpp.
- Bundled offline cleanup for punctuation-only cleanup or full cleanup/classification without translating original language.
- Recognition language modes: Auto, English, and Chinese.
- Floating status-only recorder pill.
- Separate floating language control that cycles Auto → EN → ZH.
- macOS microphone and accessibility permission handling.
- Clipboard/accessibility-based insertion with diagnostics when direct insertion is unavailable.

## Using the app

1. Build or open `Wispergo.app`.
2. Open settings from the menu bar/tray item.
3. Grant microphone permission.
4. Grant accessibility permission so Wispergo can insert text into other apps.
5. Choose a recognition language:
   - **Auto**: let Whisper detect the language.
   - **English**: passes `--language en`.
   - **Chinese**: passes `--language zh`.
6. Choose a cleanup mode:
   - **Off**: insert the raw Whisper transcript.
   - **Punctuation only**: default; uses bundled local cleanup to add punctuation/capitalization only.
   - **Full cleanup and commands**: uses bundled local cleanup for the existing cleanup/classification flow.
7. Hold `Command + Shift + Space`, speak, then release to transcribe and insert.

### Offline inference

Product builds bundle the complete offline inference stack inside `Wispergo.app`:

- `whisper.cpp`
- `ggml-large-v3-turbo` for ASR
- `llama.cpp` `llama-server` for cleanup
- Qwen2.5-3B-Instruct GGUF cleanup model

Normal users should not install Ollama, whisper.cpp, llama.cpp, or model files separately. If bundled cleanup is unavailable or too slow, Wispergo falls back to inserting the raw ASR transcript.

Developer overrides are available for debugging and experimentation:

```bash
export WISPERGO_WHISPER_BIN=/path/to/whisper-cli
export WISPERGO_WHISPER_MODEL=/path/to/ggml-large-v3-turbo.bin
export WISPERGO_CLEANUP_BACKEND=ollama
export WISPERGO_OLLAMA_BASE_URL=http://127.0.0.1:11434
export WISPERGO_OLLAMA_MODEL=qwen2.5:3b-instruct
```

Bundled binaries and model files are **not committed to git**. Only the resource directories are tracked. Before building a fully offline bundle, stage the assets yourself:

```text
apps/desktop/src-tauri/resources/
  bin/
    macos-aarch64/
      whisper-cli
      llama-server
      # llama.cpp dylibs required by llama-server
    macos-x86_64/
      whisper-cli
      llama-server
      # llama.cpp dylibs required by llama-server
  models/
    asr/
      ggml-large-v3-turbo.bin
    cleanup/
      qwen2.5-3b-instruct-q4_k_m.gguf
```

Download sources:

- `llama-server`: llama.cpp macOS release archives from <https://github.com/ggml-org/llama.cpp/releases>
- `whisper-cli`: build/download a whisper.cpp CLI binary for each target architecture
- ASR model: <https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo.bin>
- Cleanup model: <https://huggingface.co/Qwen/Qwen2.5-3B-Instruct-GGUF/resolve/main/qwen2.5-3b-instruct-q4_k_m.gguf>

For local Apple Silicon testing, staging only `bin/macos-aarch64/` plus both model files is enough. For release verification, stage both `macos-aarch64` and `macos-x86_64` binaries.

Build an offline release bundle with:

```bash
pnpm desktop:build:offline-release
```

## Development

### Prerequisites

- macOS
- Rust toolchain
- Node.js + pnpm
- Tauri v2 dependencies

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
3. Audio is trimmed and sent to the bundled Whisper sidecar.
4. Cleanup mode decides whether to skip cleanup, run punctuation-only bundled cleanup, or run full bundled cleanup/classification.
5. Result is inserted into the focused app or copied when insertion is unavailable.

## Troubleshooting

### Offline inference assets are unavailable

Product builds should include the bundled ASR and cleanup binaries/models. If Wispergo reports missing offline assets, rebuild with `pnpm desktop:build:offline-release` or reinstall the app.

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
- Chinese recognition uses Whisper’s generic `zh` language code; Wispergo does not convert between Simplified and Traditional Chinese.
