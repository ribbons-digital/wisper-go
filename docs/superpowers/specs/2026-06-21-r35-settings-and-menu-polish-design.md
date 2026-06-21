# R3.5 Settings and Menu Polish Design

## Status

Approved for implementation on 2026-06-21.

## Goal

Make Wispergo feel like a finished macOS product before release-workflow implementation by polishing the Settings window and menu bar interaction.

## Scope

In scope:

- Reshape the Settings window into a compact product dashboard based on approved Option A.
- Remove visible engineering/test copy such as `Fallback policy: ...`.
- Keep existing settings behavior: permissions, model settings, microphone selection, asset download/repair, cleanup runtime notices.
- Reduce routine scrolling in the ready state by grouping setup status and primary preferences into a compact layout.
- Change menu bar behavior so left-click opens the native menu rather than opening Settings directly.
- Add native nested menu groups above Open Settings:
  - Language: Auto, English, Chinese / Mixed.
  - Dictation model: Medium, Accuracy Pack.
  - Cleanup: Off, Punctuation only, Full cleanup.
  - Microphone: available input devices when they can be enumerated.
- Keep Open Settings and Quit below a separator.

Out of scope:

- Changing dictation, ASR, cleanup, insertion, or model download semantics.
- Adding a separate custom popover window for the menu.
- Adding new model picker concepts beyond the existing ASR model and cleanup mode settings.
- Changing release/signing workflow.

## UX design

### Settings window

The Settings window should read as a calm utility dashboard:

- Header/status area communicates readiness in user language.
- Checklist is compact and visual but still text-based for accessibility.
- Shortcut is shown as a product affordance, not a debug field.
- Preferences are grouped as "Dictation" and "Input" instead of one long generic form.
- Save action says "Save changes".
- Advanced/runtime details are de-emphasized or omitted from the ready path.

The fallback policy remains an internal setting/status, but should not be visible as `Fallback policy: prefer_local_ask_before_cloud`.

### Menu bar menu

Left-clicking the menu bar icon opens the native menu. The app no longer uses left-click as a Settings shortcut.

Menu order:

1. Language submenu.
2. Dictation model submenu.
3. Cleanup submenu.
4. Microphone submenu.
5. Separator.
6. Open Settings.
7. Quit.

Submenu item activation should reuse existing settings paths when possible. Selecting ASR model or cleanup mode may trigger existing download/verification behavior, matching Settings save semantics.

## Testing

- Frontend tests verify Settings hides fallback-policy copy, uses product copy, and keeps settings save behavior.
- Source-level Rust tests verify tray uses left-click menu behavior, removes the left-click Settings shortcut, and includes nested submenus before Open Settings.
- Rust tests cover tray menu id parsing/handling helpers for language/model/cleanup/microphone selections.
- Existing frontend/Rust gates should remain green.
