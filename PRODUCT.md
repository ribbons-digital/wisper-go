# Product

## Register

product

## Users

Wispergo serves two audiences:

- End users on Apple Silicon macOS who want fast, local dictation that inserts text into the app they are already using.
- Developers and contributors who want a clear, reproducible repository for building, testing, debugging, and improving the app.

End users are usually in the middle of another task: writing a message, drafting notes, editing text, or issuing a short command. The app should stay out of the way, confirm that it is listening, and recover clearly when permissions, downloads, or insertion fail.

Developers need predictable setup, documented architecture, automated verification, and release instructions that match the binaries users receive.

## Product Purpose

Wispergo is a local-first macOS dictation app. It provides a small global shortcut driven recording experience, runs speech recognition and cleanup locally after first setup, and inserts the result into the active app.

Success means a non-technical macOS user can download a trusted release, grant permissions, download required models, and dictate without installing developer tools or model files manually. Success for contributors means they can clone the repo, run tests, build the app, understand the inference architecture, and propose changes without reverse-engineering the release flow.

## Brand Personality

Native, calm, trustworthy.

The app should feel like a focused macOS utility, not a generic AI SaaS product. It should be quiet by default, explicit when it needs attention, and polished in the moments that prove it is working: first-run setup, recording feedback, model download, and insertion result.

## Anti-references

- Neon, cyberpunk, or high-saturation AI assistant visuals.
- Dense power-user preference panels as the first-run experience.
- Marketing-style UI patterns inside the product surface.
- Hidden setup work that leaves users confused when dictation is unavailable.
- Scary or unclear release artifacts that require terminal workarounds for ordinary users.

## Design Principles

1. Make setup explicit: permissions and model downloads should have clear progress, retry paths, and plain language.
2. Preserve flow: the recorder should confirm state without stealing focus or covering the user's work.
3. Prefer native trust: release artifacts, icons, menu bar behavior, and permission flows should feel at home on macOS.
4. Keep local-first visible: users should understand what downloads once, what runs offline, and when optional packs are needed.
5. Build for contributors too: CI, release docs, and architecture notes should reflect the app that users install.

## Accessibility & Inclusion

Target WCAG AA for visible UI text and controls. Recording animation must respect reduced-motion preferences. Menu bar and Dock icons must remain legible in light and dark macOS appearances. First-run and error states should not rely on color alone; use text and state labels for model downloads, permissions, and failures.
