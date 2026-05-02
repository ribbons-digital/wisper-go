# Floating UI Collapse Design

## Goal

Make Wispergo less obtrusive while the user is working in other apps. In the idle, unfocused state, the floating UI should collapse into a single centered bottom handle similar to an iPhone home indicator. When the user hovers that handle or starts dictation via the keyboard shortcut, the UI expands into the current two-control layout: language globe/toggle on the left and recorder status pill on the right.

## User-visible requirements

- Idle/unfocused state shows one centered minimized handle.
- The minimized handle represents both the language toggle and recorder pill; neither full control is visible while collapsed.
- Minimized handle size is approximately `96px × 10px`.
- The visible handle itself is the hover target. Do not add a larger invisible hover area.
- Expanded state keeps today's layout:
  - language globe/toggle on the left
  - recorder status pill on the right
- Recorder pill remains status-only and non-clickable.
- Expanded state appears when:
  - the mouse hovers the minimized handle
  - dictation is triggered by keyboard shortcut
  - recording is active
  - processing/insertion is active
  - the language menu is open
- After shortcut dictation finishes, keep the expanded UI visible for about `1.5s`, then collapse.
- Move both collapsed and expanded floating UI lower than today: position the bottom of the floating cluster `40px` above the bottom of the active screen.
- Use a short animation, roughly `150–200ms`, for the visual transition.

## Recommended architecture

Use the existing two-window architecture and add a small shared floating-chrome state machine.

Current architecture:

- `recorder` window: non-focusable, status-only recorder surface.
- `language` window: separate interactive language control surface.
- Native code positions both windows and already handles inactive hover for the language window.

New behavior:

- In collapsed state:
  - recorder window is resized/repositioned to the minimized handle bounds.
  - language window is hidden.
  - recorder window continues to avoid stealing focus or intercepting clicks.
- In expanded state:
  - recorder window is resized/repositioned to the current recorder pill bounds.
  - language window is shown and positioned to the left of the recorder pill.
  - language menu behavior remains unchanged.

This keeps the recorder pill non-clickable while preserving the existing language window as the only interactive control in expanded mode.

## Floating chrome state

Add an explicit frontend/backend concept of whether the floating chrome is expanded.

Expanded if any of these are true:

- `status === "recording"`
- `pending === true`
- recorder/handle native hover is true
- language native hover is true
- language menu is open
- post-insertion grace timer is active

Collapsed only when all are false.

The post-insertion grace timer should start after a successful or failed stop-recording operation settles. The timer duration should be `1500ms`. If the user hovers, records again, or opens the language menu during the grace period, the UI stays expanded and the collapse is deferred until all active reasons clear.

## Native window behavior

### Positioning

Replace the current floating bottom margin with `40px` logical pixels.

Collapsed recorder window:

- width: approximately `96px`
- height: enough to render a `10px` rounded handle without clipping, ideally `10–14px`
- centered horizontally on the active monitor
- bottom edge `40px` above monitor bottom

Expanded recorder window:

- current recorder pill dimensions remain approximately `304px × 48px`
- centered horizontally on the active monitor
- bottom edge `40px` above monitor bottom

Expanded language window:

- shown only while expanded or while the language menu is open
- positioned to the left of the recorder pill with the existing gap
- vertically aligned so the language toggle bar center matches the recorder pill center
- menu-open size behavior remains as today

### Hover detection

Because the app can be inactive and the recorder window should not steal focus, rely on native mouse movement monitoring rather than CSS `:hover` alone.

- Add recorder/handle hover tracking similar to the existing language inactive-hover monitor.
- Emit a frontend event such as `wispergo://recorder-hover-changed`.
- The hover bounds should match the visible collapsed handle in collapsed state and the visible recorder pill in expanded state.
- Do not create an invisible larger hover target.

### Pointer/focus behavior

- Recorder window remains non-focusable.
- Recorder window remains status-only and should not handle clicks.
- Language window remains focusable/interactive so the user can open and select the language menu.
- Expanded recorder hover should keep the UI open but should not block interaction with the foreground app beyond the visible floating UI.

## Frontend rendering

### Recorder surface

`FloatingRecorder` should support two visual modes:

- `collapsed`: render only the bottom handle.
- `expanded`: render current recorder pill content.

The recorder surface can derive the mode from the shared expanded state.

Collapsed handle styling:

- dark neutral background matching current pill (`#05070a` or similar)
- rounded full radius
- approximately `96px × 10px`
- no text, icon, or status dot

Expanded styling:

- preserve current recorder pill layout and copy.
- animate opacity/scale/width where practical.

### Language surface

When collapsed:

- language window should be hidden by native code, so no globe is visible.

When expanded:

- render current `LanguageToggle` unchanged except for animation polish.
- language toggle fades/slides in during expansion.
- menu-open state forces expanded state.

## Animation

Use CSS transitions for visual smoothness and native resize/reposition for actual window bounds.

Target behavior:

- collapsed handle grows/fades into recorder pill over `150–200ms`.
- language control fades/slides in over the same duration.
- collapse reverses the transition.

Native window resizing may be immediate; CSS should make the content transition feel intentional. Avoid long animations because this UI is used during fast dictation workflows.

## Testing plan

### Rust/native tests

- Positioning helper tests:
  - collapsed recorder bottom edge is `40px` above monitor bottom.
  - expanded recorder bottom edge is `40px` above monitor bottom.
  - language toggle center aligns with recorder center at the new bottom margin.
- Window mode tests:
  - collapsed mode uses minimized recorder dimensions.
  - expanded mode uses existing recorder dimensions.
  - language window is hidden/collapsed when floating chrome is collapsed.
- Source-level tests for native hover event wiring, matching the existing language hover tests.

### Frontend tests

- `FloatingRecorder` renders minimized handle when collapsed.
- `FloatingRecorder` renders current status pill when expanded.
- Keyboard shortcut/recording/pending states force expanded mode.
- Post-insertion grace state keeps expanded mode for the grace period.
- Language menu open forces expanded mode.
- Language toggle is absent/hidden while collapsed and visible while expanded.

### Manual validation

- Launch rebuilt app.
- Confirm idle UI is only a centered bottom handle.
- Hover handle and confirm current language + recorder controls appear.
- Move mouse away and confirm collapse.
- Trigger dictation by keyboard shortcut and confirm UI expands immediately.
- Release shortcut and confirm UI stays expanded during processing, then for about `1.5s`, then collapses.
- Open language menu and confirm UI stays expanded until the menu closes and hover ends.
- Confirm app remains usable in foreground apps and recorder pill is not clickable.

## Out of scope

- Redesigning the expanded recorder pill content.
- Combining language and recorder into one interactive window.
- Adding drag-to-reposition.
- Adding user settings for bottom offset, handle size, or animation duration.
