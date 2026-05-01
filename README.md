# Wispergo

Wispergo is a local-first macOS dictation app built with Tauri, React, Rust, and whisper.cpp. It provides a small floating recorder, a separate floating language toggle, and text insertion into the active app.

## Features

- Hold `Command + Shift + Space` to dictate.
- Local speech recognition through a whisper.cpp-compatible binary (`whisper-cli` or `whisper-cpp`).
- Recognition language modes: Auto, English, and Chinese.
- Optional local Ollama cleanup for punctuation-only cleanup or full cleanup/classification without translating original language.
- Floating status-only recorder pill.
- Separate floating language control that cycles Auto → EN → ZH.
- macOS microphone and accessibility permission handling.
- Clipboard/accessibility-based insertion with diagnostics when direct insertion is unavailable.

## Using the app

1. Build or open `Wispergo.app`.
2. Open settings from the menu bar/tray item.
3. Grant microphone permission.
4. Grant accessibility permission so Wispergo can insert text into other apps.
5. Configure local ASR:
   - **Whisper binary path**: path to `whisper-cli` or `whisper-cpp`.
   - **Whisper model path**: path to a local whisper.cpp model file.
6. Choose a recognition language:
   - **Auto**: let Whisper detect the language.
   - **English**: passes `--language en`.
   - **Chinese**: passes `--language zh`.
7. Choose a cleanup mode:
   - **Off**: insert the raw Whisper transcript.
   - **Punctuation only**: default; uses local Ollama to add punctuation/capitalization only.
   - **Full cleanup and commands**: uses local Ollama for the existing cleanup/classification flow.
8. Hold `Command + Shift + Space`, speak, then release to transcribe and insert.

You can also configure ASR through environment variables:

```bash
export WISPERGO_WHISPER_BIN=/path/to/whisper-cli
export WISPERGO_WHISPER_MODEL=/path/to/model.bin
```

Local Ollama cleanup:

```bash
export WISPERGO_OLLAMA_MODEL=qwen2.5:0.5b
export WISPERGO_OLLAMA_BASE_URL=http://127.0.0.1:11434
```

Wispergo checks for the Ollama CLI at startup. If it is installed, Wispergo attempts to start `ollama serve` when needed and pull the configured cleanup model when it is missing. If Ollama is unavailable or cleanup fails, dictation falls back to the raw Whisper transcript.

## Development

### Prerequisites

- macOS
- Rust toolchain
- Node.js + pnpm
- Tauri v2 dependencies
- A whisper.cpp-compatible binary and local model for runtime transcription
- Optional: Ollama for punctuation/full cleanup (`qwen2.5:0.5b` is the default model)

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
3. Audio is trimmed and sent to local Whisper sidecar.
4. Cleanup mode decides whether to skip cleanup, run punctuation-only local Ollama cleanup, or run full local Ollama cleanup/classification.
5. Result is inserted into the focused app or copied when insertion is unavailable.

## Troubleshooting

### “Local ASR is not configured”

Set the Whisper binary/model in settings or through:

```bash
export WISPERGO_WHISPER_BIN=/path/to/whisper-cli
export WISPERGO_WHISPER_MODEL=/path/to/model.bin
```

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

- Wispergo is local-first: speech recognition uses local Whisper configuration.
- Ollama cleanup is local-only. The default cleanup mode is punctuation-only with `qwen2.5:0.5b`.
- Chinese recognition uses Whisper’s generic `zh` language code; Wispergo does not convert between Simplified and Traditional Chinese.
