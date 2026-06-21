# R3 Recording Waveform UI Design

## Status

Approved for implementation on 2026-06-20.

## Goal

During active shortcut-held recording, replace the expanded pill with a separate waveform-only component. The waveform should feel like live listening feedback without adding labels or controls.

## User-approved flow

1. User holds down the shortcut key.
2. The existing pill hides and a standalone waveform component appears.
3. The language-switching surface also hides while dictation is active, leaving only the waveform visible.
4. The waveform animates during dictation.
5. User releases the shortcut keys.
6. The waveform hides immediately and the existing pill returns for processing or ready states.

## Scope

In scope:

- Add a waveform-only visual for the active recording state.
- Keep the existing pill for idle, hover/ready, setup-needed, and processing states.
- Do not place the waveform inside the pill.
- Do not show visible labels or helper copy inside the waveform component.
- Do not show the language-switching button next to the waveform during active recording.
- Respect reduced-motion preferences with a static or gently faded indicator.
- Add frontend component tests for idle, recording, processing, and setup-needed rendering.

Out of scope for this slice:

- Real microphone amplitude events from the backend.
- Live partial transcripts.
- Audio pipeline changes.
- Settings UI changes.

## UX decisions

- The waveform uses compact vertical bars, closest to the approved Option A direction, but it is rendered as its own surface rather than inside the pill.
- The recording surface may keep accessible labels for screen readers, but no visible text appears while recording.
- Release should transition away from the waveform immediately. Processing must be visually distinct from recording by returning to the pill and showing the existing Processing state.

## Technical approach

- Extend `FloatingRecorder` with a `processing` or `busy` precedence that renders waveform only when `status === "recording"` and processing is not active.
- Add a `RecordingWaveform` component or local helper in `FloatingRecorder.tsx` to keep the markup focused.
- Update `App.tsx` stop handling so releasing the shortcut can switch visible status to processing immediately instead of leaving the UI in recording while stop/transcription runs.
- Add CSS for `.recording-waveform` and bars in `apps/desktop/src/styles.css`, including `@media (prefers-reduced-motion: reduce)`.

## Verification

- `pnpm --dir apps/desktop test -- FloatingRecorder.test.tsx`
- `pnpm test:ts`
- `cargo test -p wispergo-desktop` if recorder window tests are affected.
- Manual smoke: hold shortcut, waveform appears; release shortcut, pill returns for processing; idle pill remains unchanged.
