# Local Dictation App Design

Date: 2026-04-28

## Goal

Build a macOS-first desktop dictation app inspired by Wispr Flow, with an architecture that can later support Windows and Linux. The app should feel near-instant for normal dictation, support both press-and-hold and toggle recording, run local models by default, and allow optional cloud fallback while the local pipeline matures.

The MVP includes both cleaned dictation and a constrained voice command system. It should not silently send user content to cloud services.

## Product Scope

The first version targets macOS as the first-class platform while keeping cross-platform boundaries in place. It should provide:

- Global hotkey recording.
- Floating recorder button.
- Press-and-hold and toggle recording modes.
- Local ASR transcription.
- Context-aware cleanup and formatting.
- A small voice command set.
- Clipboard-based insertion as the default.
- Local history, corrections, dictionary, and style profiles.
- Explicit optional cloud fallback for ASR and cleanup.

The MVP does not include broad app-control automation such as clicking buttons, sending messages, submitting forms, opening apps, or controlling arbitrary UI elements.

## Recommended Approach

Use a local-first hybrid pipeline:

```text
whisper.cpp / WhisperKit small model
        |
        v
fast rule-based command pre-parser
        |
        v
Ollama cleanup for ambiguous/rich formatting
        |
        v
cloud fallback only by policy
```

This approach keeps privacy and latency under control while preserving an escape hatch for quality and reliability during early development. Provider interfaces should be designed so Ollama can later be replaced by an embedded `llama.cpp`, MLX, or native runtime.

## Technology Stack

Use Tauri, Rust, and TypeScript:

- Rust owns audio capture, hotkeys, provider orchestration, OS integration, and insertion adapters.
- TypeScript and React own settings, tray/menu UI, the floating recorder control, and status views.
- SQLite stores local history, corrections, dictionary terms, style profiles, settings, and provider telemetry.
- Local ASR starts with `whisper.cpp` or WhisperKit on macOS.
- Local LLM cleanup starts with Ollama through a `CleanupProvider` interface.
- Optional cloud providers are available behind explicit privacy/fallback policy controls.

Ollama is recommended for the first local cleanup provider because it exposes a local HTTP API, supports OpenAI-compatible endpoints, works on macOS with Apple Silicon acceleration, and handles model management during prototyping. It is not assumed to be the final packaging strategy.

## Architecture

```text
Desktop shell
+-- Tray/menu bar UI
+-- Floating recorder button
+-- Global hotkey manager
+-- Permissions/settings UI
+-- Pipeline controller

Core pipeline
+-- Audio session manager
+-- VAD + utterance segmenter
+-- ASR provider
|   +-- Local: whisper.cpp / WhisperKit
|   +-- Cloud fallback: optional, policy-gated
+-- Context collector
+-- Intent + cleanup engine
|   +-- Rule-based fast commands
|   +-- Local LLM: Ollama
|   +-- Cloud fallback: optional, policy-gated
+-- Insertion adapter
|   +-- Clipboard paste first
|   +-- Native insertion later
+-- Local data store
    +-- History
    +-- Corrections
    +-- Dictionary
    +-- Style profiles
    +-- Privacy/fallback policies
```

The UI technology and model runtime are intentionally separated. The app shell should call into core services through typed commands/events rather than coupling UI components directly to model implementations.

## Component Responsibilities

### TriggerManager

Handles press-and-hold hotkey, toggle hotkey, and floating button events. It does not record audio directly. It only sends `startRecording`, `stopRecording`, and `cancelRecording` commands to the pipeline.

### AudioSessionManager

Owns microphone permission, buffering, sample format conversion, and recording lifecycle. It exposes clean utterance audio to the ASR layer.

### VadSegmenter

Trims leading and trailing silence and can auto-stop in toggle mode. In press-and-hold mode, VAD should trim silence but avoid ending early unless the user enables that behavior.

### AsrProvider

Converts audio to raw transcript. The default provider is local. Cloud ASR is optional and must respect the user's fallback policy.

### ContextCollector

Builds a scoped context package from allowed sources:

- Active app name.
- Window title where available.
- Selected text or nearby text when permitted.
- Dictionary terms.
- Recent corrections.
- Current style profile.

Context collection is privacy-controlled and should be disabled per app when configured.

### IntentEngine

Decides whether the utterance is dictation or a command. Simple commands are handled by rules first. Ambiguous or richer transformations go through the LLM and return structured JSON.

### CleanupProvider

Rewrites raw ASR into final text or structured actions. Ollama is the first local implementation. Cloud LLM is a policy-gated fallback.

### InsertionAdapter

Inserts output into the active app. Clipboard paste is the reliable MVP default. Native accessibility/text insertion can be added per OS later.

### LocalStore

Stores history, corrections, dictionary terms, style profiles, settings, and provider health/latency telemetry in SQLite.

### PrivacyPolicyEngine

Decides whether active context, selected text, cloud fallback, and history storage are allowed for the current app/profile. This should be a real policy layer, not scattered conditionals.

## Recording Modes

```text
Input trigger
+-- Press-and-hold hotkey
|   +-- key down starts recording
|   +-- key up stops recording
+-- Toggle hotkey
|   +-- first press starts recording
|   +-- second press stops recording
+-- Floating button
    +-- click toggles
    +-- optional hold-to-record later
```

The recorder receives mode-agnostic lifecycle calls:

```text
startRecording(trigger, mode)
stopRecording(reason)
cancelRecording(reason)
```

## Pipeline Result Model

Each recording session returns one structured result:

```ts
type PipelineResult =
  | {
      kind: "insert_text";
      text: string;
      source: "local" | "cloud";
      confidence?: number;
    }
  | {
      kind: "command";
      command: CommandAction;
      requiresConfirmation: boolean;
      source: "rules" | "local_llm" | "cloud_llm";
    }
  | {
      kind: "cancelled";
      reason: string;
    }
  | {
      kind: "error";
      recoverable: boolean;
      message: string;
    };
```

The LLM should not directly execute behavior. It classifies or transforms into structured results, then the application validates and executes allowed actions.

## Command Handling

The command path is conservative:

1. Capture audio.
2. Transcribe with local ASR.
3. Run a fast rules pass for explicit commands.
4. If the utterance is not a clear command, send it to cleanup as dictation.
5. If the utterance references current selection or style, use the LLM to classify it into a structured command.
6. Execute only allowed commands.
7. Require confirmation for destructive or ambiguous actions.

Initial command set:

- Insert cleaned dictation.
- New line.
- New paragraph.
- Cancel.
- Literal dictation.
- Delete previous phrase.
- Replace selected text.
- Rewrite selected text casually, professionally, shorter, or longer.
- Format selected text as bullets.
- Format selected text as a numbered list.

Out of scope for MVP:

- Send or submit actions.
- Click actions.
- Opening apps.
- Arbitrary UI automation.
- Target-specific app workflows.

## Latency Targets

The app should feel near-instant:

- Rule commands resolve within 200 ms.
- Normal 5-10 second utterances insert within about 1 second after release on Apple Silicon.
- Complex rewrites complete within 1-3 seconds and show visible progress.
- Cloud fallback is explicitly surfaced according to policy.

If cleanup exceeds a timeout, the app may insert raw ASR text first and refine later only when it is safe and predictable.

## Cloud Fallback Policy

Cloud fallback must never be silent. Supported policy modes:

- Local only.
- Prefer local, ask before cloud.
- Prefer local, automatic cloud fallback.
- Cloud disabled for selected apps.

The same provider interface shape should support local and cloud ASR, local and cloud cleanup, and future embedded local providers.

## Error Handling

Predictable degradation:

```text
Microphone unavailable
-> show permission/error state, do not start recording

Local ASR unavailable or too slow
-> follow fallback policy: fail local-only, ask, or use cloud

Cleanup LLM unavailable or timeout
-> insert raw ASR text, or ask before cloud fallback depending on policy

Insertion fails
-> leave final text in clipboard and show a small copied state

Command is destructive or ambiguous
-> require confirmation, never auto-execute
```

## Privacy Requirements

Privacy controls are product requirements:

- Local-only mode.
- Prefer-local modes with clear cloud fallback behavior.
- Per-app cloud disable list.
- Per-app context disable list.
- History off/on.
- Do not store dictated audio by default.
- Optional transcript history with local retention controls.
- Clear UI signal when cloud is used.

## Testing Strategy

Focus tests on high-risk boundaries:

- Unit tests for command classification and fallback policy.
- Golden tests for cleanup prompts that require valid structured JSON.
- Audio fixture tests for VAD segmentation.
- Provider contract tests for ASR and cleanup providers.
- macOS integration tests for hotkeys, recording lifecycle, permissions, and clipboard insertion.
- Latency benchmarks for 5-10 second utterances.
- Manual end-to-end matrix across Notes, browser text fields, Slack/Discord, terminal, and code editor.

## MVP Acceptance Criteria

- Press-and-hold and toggle recording both work.
- Floating button can start and stop recording.
- A 5-10 second utterance inserts within about 1 second after release on Apple Silicon using the default local model.
- Basic commands work reliably.
- Destructive or ambiguous commands require confirmation.
- Cloud fallback is never silent.
- If insertion fails, the text remains available in the clipboard.
- History, corrections, and dictionary are stored locally.
- Dictated audio is not stored by default.

## References

- Wispr Flow official site: https://wisprflow.ai/
- TechCrunch coverage of Wispr Flow Android launch, published 2026-02-23: https://techcrunch.com/2026/02/23/wispr-flow-launches-an-android-app-for-ai-powered-dictation/
- Ollama API documentation: https://docs.ollama.com/api
- Ollama OpenAI compatibility documentation: https://docs.ollama.com/api/openai-compatibility
- Ollama macOS documentation: https://docs.ollama.com/macos
