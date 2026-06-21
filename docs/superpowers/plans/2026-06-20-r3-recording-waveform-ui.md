# R3 Recording Waveform UI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Show only a standalone animated waveform during active shortcut-held recording, then return to the existing pill and language controls for processing, setup-needed, ready, and idle states.

**Architecture:** Keep the feature frontend-only for this slice. `FloatingRecorder` owns the visual state switch, while `App` ensures release transitions the recorder surface out of recording immediately when processing begins.

**Tech Stack:** React, TypeScript, Vitest, Testing Library, CSS media queries for reduced motion.

---

## Files

- Modify: `apps/desktop/src/features/recorder/FloatingRecorder.tsx`
- Modify: `apps/desktop/src/features/recorder/FloatingRecorder.test.tsx`
- Modify: `apps/desktop/src/app/App.tsx`
- Modify: `apps/desktop/src/app/App.test.tsx` if app-level release/processing behavior needs coverage.
- Modify: `apps/desktop/src/styles.css`
- Modify: `docs/superpowers/plans/2026-06-18-in-process-inference-roadmap.md`
- Modify: `HANDOFF.md`

## Task 1: Component state tests

- [ ] Add a failing test proving active recording renders a waveform and no visible status or hint copy.
- [ ] Add a failing test proving processing uses the pill, not the waveform.
- [ ] Add a failing test proving setup-needed and idle still use the pill.
- [ ] Run `pnpm --dir apps/desktop test -- FloatingRecorder.test.tsx` and confirm the new recording test fails before implementation.

## Task 2: Component implementation

- [ ] Update `FloatingRecorder.tsx` so `status === "recording" && !busy && !setupNeeded` renders a standalone waveform component.
- [ ] Keep the existing pill markup for idle, setup-needed, and processing.
- [ ] Add CSS for the standalone waveform surface and bar animation.
- [ ] Add reduced-motion CSS that disables bar animation and shows static bars.
- [ ] Run `pnpm --dir apps/desktop test -- FloatingRecorder.test.tsx` and confirm PASS.

## Task 3: Release-to-processing transition

- [ ] Inspect `App.tsx` shortcut stop flow.
- [ ] If release currently keeps `status="recording"` while `busy=true`, change `stopActiveRecording` to apply `idle` before starting the stop/transcription command so the waveform hides immediately and the processing pill appears.
- [ ] Add or update `App.test.tsx` if existing coverage does not prove processing rendering after release.
- [ ] Add a backend floating-chrome state test proving the language surface is hidden while recording.
- [ ] Run targeted app/backend tests.

## Task 4: Docs and verification

- [ ] Mark R3 in progress/done in the roadmap as appropriate.
- [ ] Update `HANDOFF.md` with the R3 branch/status.
- [ ] Run `pnpm test:ts`.
- [ ] Run `pnpm desktop:build` for release-polish confidence.
- [ ] Manually launch the app and smoke the recorder visual flow if feasible.
