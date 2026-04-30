# Language Toggle Design

## Goal

Add language selection for Wispergo dictation with a floating control inspired by Wisper Flow. The control should default to automatic language detection and allow quick switching between Auto, English, and Chinese without making the existing recorder pill itself clickable.

## Scope

- Add a floating language control next to the recorder pill.
- Add language configuration in the Settings window.
- Persist the selected language.
- Pass the selected language to the local whisper.cpp sidecar.
- Preserve the original transcription language during cleanup.

Out of scope:

- Multiple simultaneous languages.
- Traditional/Simplified Chinese conversion.
- Adding more languages beyond Auto, English, and Chinese.
- Changing the existing global recording shortcut.

## Language Options

The supported recognition language options are:

- `auto`: default; do not pass a language flag to Whisper.
- `en`: English; pass `--language en`.
- `zh`: Chinese; pass `--language zh`.

The UI labels are:

- Auto: globe icon in the floating control, `Auto` in menus/settings.
- English: `EN` in the floating control, `English` in menus/settings.
- Chinese: `ZH` in the floating control, `Chinese` in menus/settings.

## Floating UX

The existing recorder pill remains status-only and should not become the clickable target.

A separate small language control appears next to the pill:

- In `auto`, the primary button shows a globe icon.
- In `en`, it shows `EN`.
- In `zh`, it shows `ZH`.
- Clicking the primary language button cycles `auto -> en -> zh -> auto`.
- Hovering the language control reveals a chevron affordance next to the current language indicator.
- Clicking the chevron opens a popover above the control.
- The popover offers exactly one active language selection:
  - Auto
  - English
  - Chinese
- The selected item shows a checkmark.
- Selecting an item updates the persisted language and closes the popover.

The language control may be interactive and may intercept clicks in its own bounds. The recorder pill should remain non-interactive/click-through.

## Settings UX

Add a `Recognition language` select to the Settings window with:

- Auto
- English
- Chinese

The setting should initialize from persisted settings, default to Auto, and save with the existing local model settings save flow.

## Data Flow

1. App startup loads persisted settings into `AppState`.
2. Settings surface loads `localModelSettings` and displays the selected recognition language.
3. Recorder surface also loads the selected recognition language so the floating control can display it.
4. Floating language control changes call a Tauri command to persist the new language immediately.
5. Stop recording reads current settings from `AppState`.
6. Whisper sidecar receives:
   - no language argument for Auto,
   - `--language en` for English,
   - `--language zh` for Chinese.
7. Optional cleanup receives the transcript with prompt guidance to preserve the original language and not translate.

## Implementation Boundaries

- Keep language values as a small typed enum/union shared conceptually between Rust and TypeScript.
- Extend `LocalModelSettings` with `recognition_language` / `recognitionLanguage`.
- Add Tauri commands if needed for immediate floating-control updates, while preserving the existing settings save path.
- Extend `WhisperSidecarProvider` to accept an optional language code.
- Update tests at each boundary:
  - settings persistence and normalization,
  - Whisper sidecar language arguments,
  - settings panel language selection,
  - floating control cycle/menu behavior,
  - recorder surface does not query settings-only permissions.

## Error Handling

- Invalid/missing persisted language values fall back to Auto.
- If saving a floating language change fails, show the existing app error status.
- Auto mode remains the safe default and should behave exactly like current recognition.

## Acceptance Criteria

- Default language is Auto.
- Settings window shows and saves Recognition language.
- Floating recorder shows a separate language control next to the status pill.
- The language control cycles Auto -> EN -> ZH -> Auto by clicking the main language button.
- Hover reveals the chevron affordance.
- Chevron opens a single-selection popover for Auto, English, Chinese.
- Whisper receives `--language en` or `--language zh` only when those modes are selected.
- Cleanup prompt preserves original language and does not translate.
- Existing recorder pill remains status-only.
- Build and tests pass.
