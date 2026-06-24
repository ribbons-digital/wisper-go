# R5 Shortcut Customization Design

**Status:** Draft for user review. Planning/spec only; no implementation in this slice.

## Goal

Let users change how Wispergo starts dictation without breaking the current default. The spec covers both normal shortcut-combo customization and single-modifier hold-to-dictate, implemented as separate PRs.

Default behavior remains:

```text
Command + Shift + Space, press and hold to dictate
```

## Motivation

Wispergo currently hardcodes `Command + Shift + Space` in `apps/desktop/src-tauri/src/lib.rs` via `tauri-plugin-global-shortcut`. The frontend listens for `wispergo://record-shortcut` `Pressed`/`Released` events and maps them to hold-to-dictate.

That default is reliable, but users may already have conflicts or may prefer a more dictation-native trigger such as holding a single physical modifier key. OpenSuperWhisper shows that modifier-only hold can feel good, but Wispergo should keep a calmer, safer implementation: explicit settings, clear conflict handling, and no active key interception.

## Reviewed alternatives

This design was reviewed with:

```bash
claude -p --model claude-opus-4-8
```

Consensus recommendation:

1. Build combo customization first.
2. Add single modifier-key hold second.
3. Defer arbitrary single-key hold.

## Non-goals

- No arbitrary letter/number single-key hold, e.g. hold `J` to dictate. A listen-only monitor cannot suppress the typed character, and suppressing it would require an active event tap with higher risk.
- No cloud ASR or new ASR engine work.
- No streaming partial transcript work.
- No large redesign of Settings beyond the focused shortcut controls.
- No breaking change to the current default shortcut.

## User-facing behavior

Settings gains a Shortcut section with two modes:

1. **Key combination**
   - Default: `⌘ ⇧ Space`.
   - User can record/select another combination.
   - Saving validates registration.
   - If the combination is unavailable, Wispergo shows an inline error and keeps the previous working shortcut active.

2. **Hold one modifier key**
   - User chooses a physical modifier key:
     - Left Command
     - Right Command
     - Left Option
     - Right Option, when present/reliably detectable
     - Left Control
     - Right Control
     - Left Shift
     - Right Shift
   - Right Command is explicitly supported for keyboards that do not have Right Option.
   - Fn is out of scope for R5.2 because reliability varies across keyboards and macOS input paths.
   - Holding the selected modifier alone starts recording after a short threshold.
   - Releasing the modifier stops recording.
   - Pressing another key before the threshold cancels the trigger, so normal shortcuts keep working.

The Settings hero and recorder hint use the saved shortcut label instead of hardcoded `Command + Shift + Space`.

## Data model

Add a persisted shortcut settings model separate from `LocalModelSettings`. `LocalModelSettings` remains focused on ASR/language/cleanup choices:

```text
ShortcutSettings
  mode: combo | modifier_hold
  combo:
    modifiers: command/shift/option/control bitset
    code: key code enum/string
  modifierHold:
    key: left_command | right_command | left_option | right_option | left_control | right_control | left_shift | right_shift
    holdThresholdMs: default 200
```

Migration/defaults:

- Missing settings deserialize to `combo Command+Shift+Space`.
- Unknown/invalid saved settings normalize back to the default combo.
- Frontend receives a display label, e.g. `⌘ ⇧ Space` or `Hold Right ⌘`.

## Native trigger architecture

### Existing path

Current code registers one global shortcut at startup and emits:

```text
wispergo://record-shortcut: Pressed | Released
```

The frontend already handles these events as hold-to-dictate.

### Target architecture

Introduce a native shortcut controller responsible for:

- loading shortcut settings at startup;
- registering exactly one active trigger mode;
- applying new settings when the user saves;
- rolling back to the previous active trigger if registration fails;
- emitting the same frontend event contract where possible.

The frontend recording flow should not need to know whether the source was a combo or modifier hold. It continues to receive `Pressed` and `Released` events.

## PR 1: key-combination customization

Scope:

- Add `ShortcutSettings` persistence and commands to get/set/apply settings.
- Replace hardcoded shortcut registration with dynamic combo registration.
- Add Settings UI for combo mode and the current shortcut label.
- Add conflict-safe save behavior:
  - unregister/try-register new combo;
  - if registration fails, restore previous combo;
  - return a user-facing error.
- Update recorder and Settings copy to use the selected shortcut label.

Conflict handling:

- True conflict detection is available for combo shortcuts because `tauri-plugin-global-shortcut` registration can fail.
- Failed save should not leave Wispergo without a working shortcut.
- The old shortcut remains active after a failed save.

Testing:

- Rust tests for serialization/default normalization.
- Rust tests for controller apply/rollback behavior using fake shortcut registry.
- Frontend tests for shortcut label rendering and inline conflict error.
- Existing `starts on shortcut press and stops on shortcut release` tests remain valid.

## PR 2: single modifier-key hold

Scope:

- Add `modifier_hold` mode to `ShortcutSettings` UI and persistence.
- Add a macOS listen-only monitor for modifier state changes and key-down cancellation.
- Route modifier hold through the same `Pressed`/`Released` frontend event contract.
- Add watchdog/force-release behavior for missed key-up cases.
- Keep combo shortcut mode available and unchanged.

Detection model:

- Use listen-only macOS global event monitoring, not active event interception.
- Monitor `flagsChanged` to detect selected physical modifier down/up.
- Monitor key-down events while a modifier trigger is pending/active so normal chords cancel instead of starting dictation.
- Distinguish left/right modifiers using macOS physical modifier flags/key information where available.

Hold state machine:

```text
idle
  selected modifier down -> pending
pending
  threshold elapsed, no other key/modifier joined -> emit Pressed, active
  selected modifier up before threshold -> idle, no event
  other key/modifier joins before threshold -> cancelled_until_release
active
  selected modifier up -> emit Released, idle
  other key joins -> cancel/stop recording, cancelled_until_release
cancelled_until_release
  selected modifier up -> idle
```

Threshold:

- Default `200ms`.
- Stored in the model for future tuning, but not necessarily exposed as a user control in the first UI.

Watchdog:

- If Wispergo believes modifier-hold recording is active but no key-up arrives after a conservative maximum hold duration or app lifecycle transition, emit `Released`/stop to avoid stuck recording.
- Implementation should prefer state repair over user-visible errors.

Permissions:

- Modifier-hold mode requires Accessibility permission.
- Wispergo already requires Accessibility for insertion; Settings should reuse the existing setup checklist and permission copy.
- If Accessibility is missing, modifier-hold mode may be saved, but Settings must show that it will not work until permission is granted. This matches the existing setup checklist pattern and avoids making preferences feel broken while the user is still completing setup.

Conflict/interference policy:

- Single modifier hold has no OS-level registration conflict to detect.
- Safety comes from listen-only monitoring, hold threshold, and chord cancellation.
- Wispergo must not suppress, swallow, or rewrite user keystrokes.
- Arbitrary non-modifier single-key hold remains out of scope.

Testing:

- Rust unit tests for modifier-hold state machine:
  - tap selected modifier does not start;
  - hold past threshold starts;
  - release after active stops;
  - second key before threshold cancels;
  - second key while active stops/cancels;
  - missed release watchdog stops.
- Rust tests for settings serialization/defaults.
- Frontend tests for mode selection and labels.
- Manual smoke on a keyboard with left/right Command and no Right Option:
  - Right Command hold starts/stops dictation;
  - normal Command shortcuts still work;
  - quick Command+key chord does not start recording.

## UI design notes

The Settings section should stay compact and native-feeling.

Proposed structure inside the existing Input/Dictation area or a new Shortcut card:

```text
Shortcut
  Mode: [Key combination] [Hold one key]

  Key combination mode:
    Current: ⌘ ⇧ Space
    [Record shortcut]
    Inline error if unavailable

  Hold one key mode:
    Key: [Right Command]
    Helper: Starts when held by itself. Normal shortcuts are ignored.
```

Copy principles:

- Be explicit, not technical.
- Use physical names: `Left Command`, `Right Command`, not only symbols.
- Keep advanced caveats out of the main UI unless an error occurs.

## Manual QA checklist

For PR 1:

- Default shortcut still works on a fresh profile.
- Changing combo updates Settings and recorder hint.
- Invalid/conflicting combo shows inline error and old combo still works.
- Restart preserves selected combo.

For PR 2:

- Right Command works on the user's keyboard.
- Left Command works if selected.
- Quick Command+Tab, Command+C, Command+V, Command+Space style chords do not start recording.
- Holding selected key starts after threshold and release stops.
- Releasing after app switch or focus change does not leave recording stuck.
- Accessibility missing state is understandable.

## Risks and mitigations

| Risk | Mitigation |
|---|---|
| Shortcut save leaves no active trigger | Roll back to previous trigger on registration failure |
| Single modifier starts during normal shortcut use | 200ms threshold plus cancel-on-chord |
| Modifier key-up missed | Watchdog/force-release and lifecycle repair |
| Left/right modifier detection differs by keyboard | Manual smoke on target keyboard; keep fallback copy honest |
| Settings becomes too dense | One compact Shortcut card; default path remains simple |
| User expects arbitrary single keys | Explicitly limit first version to modifiers |

## Implementation sequencing

This umbrella spec should produce separate PRs:

1. `R5.1 Shortcut combo customization`
2. `R5.2 Single modifier hold-to-dictate`

Each PR must pass the existing desktop gate, including:

```bash
pnpm test:ts
cargo test -p wispergo-desktop
cargo clippy -p wispergo-desktop --all-targets -- -D warnings
pnpm desktop:build
pnpm check:macos-thin-bundle
```

Broader workspace/core gates may be run when changes touch shared crates or release workflow.
