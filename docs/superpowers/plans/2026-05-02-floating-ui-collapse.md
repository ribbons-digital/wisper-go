# Collapsed Floating UI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Collapse Wispergo's idle floating UI into one centered bottom handle and expand it into the existing language-toggle + recorder-pill layout on hover, shortcut use, recording, processing, menu open, or post-insertion grace.

**Architecture:** Keep the existing two-window Tauri architecture. Add a small native floating-chrome state machine that owns window sizing, visibility, positioning, and cross-window expansion events; add frontend state/listeners only for rendering the recorder surface and updating native expansion reasons from shortcut/processing/post-insertion state.

**Tech Stack:** Tauri 2, Rust, macOS AppKit private APIs already used by the app, React, TypeScript, Vitest, CSS transitions.

---

## Files and responsibilities

- `apps/desktop/src-tauri/src/lib.rs`
  - Add native floating chrome constants, reason state, Tauri command, window mode sizing/positioning, and recorder hover monitoring.
  - Update startup so the app launches collapsed and the language window starts hidden.
  - Update tests for the new bottom margin, collapsed recorder dimensions, command registration, and hover event wiring.
- `apps/desktop/src-tauri/tauri.conf.json`
  - Change initial recorder window to collapsed handle dimensions.
  - Change language window to initially hidden.
- `apps/desktop/src/lib/tauriApi.ts`
  - Add `setFloatingChromeReason(reason, active)` wrapper.
- `apps/desktop/src/app/App.tsx`
  - Listen for native floating expansion and recorder hover events.
  - Drive native expansion reasons for recording, processing, language hover, and post-insertion grace.
  - Pass expanded/collapsed mode into `FloatingRecorder`.
- `apps/desktop/src/app/App.test.tsx`
  - Add tests for recorder collapsed default, shortcut expansion, and post-insertion grace collapse.
- `apps/desktop/src/features/recorder/FloatingRecorder.tsx`
  - Add `expanded` prop and render collapsed handle when false.
- `apps/desktop/src/features/recorder/FloatingRecorder.test.tsx`
  - Add collapsed handle tests and update existing tests to render expanded mode.
- `apps/desktop/src/styles.css`
  - Style collapsed handle, lower/center surfaces, and animate recorder/language transitions.

## Constants and names

Use these exact values unless implementation constraints force a tiny window-size adjustment:

```rust
const FLOATING_BOTTOM_MARGIN: f64 = 40.0;
const RECORDER_COLLAPSED_WIDTH: f64 = 96.0;
const RECORDER_COLLAPSED_HEIGHT: f64 = 10.0;
const RECORDER_EXPANDED_WIDTH: f64 = 320.0;
const RECORDER_EXPANDED_HEIGHT: f64 = 62.0;
const LANGUAGE_CLOSED_WIDTH: f64 = 74.0;
const LANGUAGE_CLOSED_HEIGHT: f64 = 52.0;
const LANGUAGE_OPEN_WIDTH: f64 = 260.0;
const LANGUAGE_OPEN_HEIGHT: f64 = 190.0;
const LANGUAGE_TOGGLE_BAR_HEIGHT: f64 = 40.0;
```

Use these event and command names:

```text
wispergo://floating-chrome-expanded-changed
wispergo://recorder-hover-changed
set_floating_chrome_reason
```

Use these reason strings from TypeScript:

```ts
"recording" | "processing" | "post_insert" | "language_hover"
```

Native-only reason:

```rust
FloatingChromeReason::RecorderHover
```

---

### Task 1: Native sizing constants and positioning helpers

**Files:**
- Modify: `apps/desktop/src-tauri/src/lib.rs`

- [ ] **Step 1: Write failing Rust tests for collapsed and expanded positioning**

Add these imports/items inside the existing `#[cfg(test)] mod tests` import block in `apps/desktop/src-tauri/src/lib.rs`:

```rust
use super::{
    floating_chrome_expanded, language_window_top_for_aligned_toggle_bar,
    recorder_window_ignores_cursor_events, recorder_window_size_for_mode,
    recorder_window_top_for_bottom_margin, should_hide_window_on_close, FloatingChromeReason,
    FloatingChromeReasonState, FloatingRecorderMode, FLOATING_BOTTOM_MARGIN,
};
```

Replace the current `language_window_top_aligns_toggle_bar_center_with_recorder_center` test body so it uses the new `FLOATING_BOTTOM_MARGIN` and expanded recorder size:

```rust
#[test]
fn language_window_top_aligns_toggle_bar_center_with_recorder_center() {
    let monitor_top = 0;
    let monitor_height = 900;
    let bottom_margin = FLOATING_BOTTOM_MARGIN as i32;
    let (_, recorder_height) = recorder_window_size_for_mode(FloatingRecorderMode::Expanded);
    let recorder_height = recorder_height as u32;
    let toggle_bar_height = 40;
    let monitor_bottom = monitor_top as f64 + monitor_height as f64;
    let recorder_center = monitor_bottom - bottom_margin as f64 - recorder_height as f64 / 2.0;

    for language_height in [52, 190] {
        let language_y = language_window_top_for_aligned_toggle_bar(
            monitor_top,
            monitor_height,
            bottom_margin,
            recorder_height,
            language_height,
            toggle_bar_height,
        );
        let language_bar_center =
            language_y as f64 + language_height as f64 - toggle_bar_height as f64 / 2.0;

        assert_eq!(language_bar_center, recorder_center);
    }
}
```

Add these new tests below it:

```rust
#[test]
fn floating_bottom_margin_is_forty_logical_pixels() {
    assert_eq!(FLOATING_BOTTOM_MARGIN, 40.0);
}

#[test]
fn recorder_window_size_switches_between_collapsed_handle_and_expanded_pill() {
    assert_eq!(
        recorder_window_size_for_mode(FloatingRecorderMode::Collapsed),
        (96.0, 10.0)
    );
    assert_eq!(
        recorder_window_size_for_mode(FloatingRecorderMode::Expanded),
        (320.0, 62.0)
    );
}

#[test]
fn recorder_window_top_uses_configured_bottom_margin() {
    let monitor_top = 0;
    let monitor_height = 900;
    let collapsed_y = recorder_window_top_for_bottom_margin(
        monitor_top,
        monitor_height,
        10,
        FLOATING_BOTTOM_MARGIN as i32,
    );
    let expanded_y = recorder_window_top_for_bottom_margin(
        monitor_top,
        monitor_height,
        62,
        FLOATING_BOTTOM_MARGIN as i32,
    );

    assert_eq!(collapsed_y, 850);
    assert_eq!(expanded_y, 798);
}

#[test]
fn floating_chrome_expands_when_any_reason_is_active() {
    assert!(!floating_chrome_expanded(&FloatingChromeReasonState::default()));

    let mut state = FloatingChromeReasonState::default();
    state.set(FloatingChromeReason::RecorderHover, true);
    assert!(floating_chrome_expanded(&state));

    state.set(FloatingChromeReason::RecorderHover, false);
    assert!(!floating_chrome_expanded(&state));
}
```

- [ ] **Step 2: Run the tests and verify they fail**

Run:

```bash
cargo test -p wispergo-desktop --lib floating_bottom_margin_is_forty_logical_pixels recorder_window_size_switches_between_collapsed_handle_and_expanded_pill recorder_window_top_uses_configured_bottom_margin floating_chrome_expands_when_any_reason_is_active
```

`cargo test` only accepts one name filter, so if the command rejects multiple filters, run these instead:

```bash
cargo test -p wispergo-desktop --lib floating_bottom_margin_is_forty_logical_pixels
cargo test -p wispergo-desktop --lib recorder_window_size_switches_between_collapsed_handle_and_expanded_pill
cargo test -p wispergo-desktop --lib recorder_window_top_uses_configured_bottom_margin
cargo test -p wispergo-desktop --lib floating_chrome_expands_when_any_reason_is_active
```

Expected: compile/test failure because `FloatingRecorderMode`, `FloatingChromeReasonState`, helper functions, and the `40.0` margin are not implemented yet.

- [ ] **Step 3: Implement constants, helper types, and pure helper functions**

In `apps/desktop/src-tauri/src/lib.rs`, replace the existing floating constants block with:

```rust
const FLOATING_BOTTOM_MARGIN: f64 = 40.0;
const FLOATING_GAP: f64 = 8.0;
const RECORDER_COLLAPSED_WIDTH: f64 = 96.0;
const RECORDER_COLLAPSED_HEIGHT: f64 = 10.0;
const RECORDER_EXPANDED_WIDTH: f64 = 320.0;
const RECORDER_EXPANDED_HEIGHT: f64 = 62.0;
const LANGUAGE_CLOSED_WIDTH: f64 = 74.0;
const LANGUAGE_CLOSED_HEIGHT: f64 = 52.0;
const LANGUAGE_OPEN_WIDTH: f64 = 260.0;
const LANGUAGE_OPEN_HEIGHT: f64 = 190.0;
const LANGUAGE_TOGGLE_BAR_HEIGHT: f64 = 40.0;
```

Add these types and helpers near the constants:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FloatingRecorderMode {
    Collapsed,
    Expanded,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FloatingChromeReason {
    RecorderHover,
    LanguageHover,
    LanguageMenu,
    Recording,
    Processing,
    PostInsert,
}

#[derive(Default)]
struct FloatingChromeReasonState {
    recorder_hover: bool,
    language_hover: bool,
    language_menu: bool,
    recording: bool,
    processing: bool,
    post_insert: bool,
}

impl FloatingChromeReasonState {
    fn set(&mut self, reason: FloatingChromeReason, active: bool) {
        match reason {
            FloatingChromeReason::RecorderHover => self.recorder_hover = active,
            FloatingChromeReason::LanguageHover => self.language_hover = active,
            FloatingChromeReason::LanguageMenu => self.language_menu = active,
            FloatingChromeReason::Recording => self.recording = active,
            FloatingChromeReason::Processing => self.processing = active,
            FloatingChromeReason::PostInsert => self.post_insert = active,
        }
    }
}

fn floating_chrome_expanded(state: &FloatingChromeReasonState) -> bool {
    state.recorder_hover
        || state.language_hover
        || state.language_menu
        || state.recording
        || state.processing
        || state.post_insert
}

fn recorder_window_size_for_mode(mode: FloatingRecorderMode) -> (f64, f64) {
    match mode {
        FloatingRecorderMode::Collapsed => (RECORDER_COLLAPSED_WIDTH, RECORDER_COLLAPSED_HEIGHT),
        FloatingRecorderMode::Expanded => (RECORDER_EXPANDED_WIDTH, RECORDER_EXPANDED_HEIGHT),
    }
}

fn recorder_window_top_for_bottom_margin(
    monitor_top: i32,
    monitor_height: u32,
    window_height: i32,
    bottom_margin: i32,
) -> i32 {
    monitor_top + monitor_height as i32 - window_height - bottom_margin
}
```

Update `position_recorder_window` to accept a mode and set the recorder size before positioning:

```rust
fn position_recorder_window(app: &tauri::AppHandle, mode: FloatingRecorderMode) -> tauri::Result<()> {
    let Some(window) = app.get_webview_window("recorder") else {
        return Ok(());
    };
    let monitor = window
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| window.primary_monitor().ok().flatten());
    let Some(monitor) = monitor else {
        return Ok(());
    };

    let (logical_width, logical_height) = recorder_window_size_for_mode(mode);
    window.set_size(tauri::Size::Logical(tauri::LogicalSize::new(
        logical_width,
        logical_height,
    )))?;

    let monitor_position = monitor.position();
    let monitor_size = monitor.size();
    let scale_factor = monitor.scale_factor();
    let physical_width = logical_to_physical_u32(logical_width, scale_factor);
    let physical_height = logical_to_physical_i32(logical_height, scale_factor);
    let bottom_margin = logical_to_physical_i32(FLOATING_BOTTOM_MARGIN, scale_factor);
    let x = centered_window_left(monitor_position.x, monitor_size.width, physical_width);
    let y = recorder_window_top_for_bottom_margin(
        monitor_position.y,
        monitor_size.height,
        physical_height,
        bottom_margin,
    );
    window.set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(
        x, y,
    )))?;
    Ok(())
}
```

Update all current call sites from `position_recorder_window(app.handle())` to `let _ = position_recorder_window(app.handle(), FloatingRecorderMode::Collapsed);` for startup. Later tasks will route this through the shared state.

- [ ] **Step 4: Run the focused Rust tests and verify they pass**

Run:

```bash
cargo test -p wispergo-desktop --lib floating_bottom_margin_is_forty_logical_pixels
cargo test -p wispergo-desktop --lib recorder_window_size_switches_between_collapsed_handle_and_expanded_pill
cargo test -p wispergo-desktop --lib recorder_window_top_uses_configured_bottom_margin
cargo test -p wispergo-desktop --lib floating_chrome_expands_when_any_reason_is_active
```

Expected: all pass.

- [ ] **Step 5: Run full desktop lib tests**

Run:

```bash
cargo test -p wispergo-desktop --lib
```

Expected: all tests pass. If existing tests fail because they still assert the old `88` bottom margin or old `position_recorder_window` signature, update those tests to use the new constants/helpers from this task.

- [ ] **Step 6: Commit Task 1**

```bash
git add apps/desktop/src-tauri/src/lib.rs
git commit -m "feat: add floating chrome sizing helpers"
```

---

### Task 2: Native floating chrome command and window visibility

**Files:**
- Modify: `apps/desktop/src-tauri/src/lib.rs`
- Modify: `apps/desktop/src-tauri/tauri.conf.json`

- [ ] **Step 1: Write failing Rust/source tests for native state, command registration, and initial config**

In `apps/desktop/src-tauri/src/lib.rs` tests, add:

```rust
#[test]
fn floating_chrome_command_is_registered() {
    let source = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs"))
        .expect("lib source");
    let production_source = source
        .split("\n#[cfg(test)]")
        .next()
        .expect("production lib source before tests");
    let generate_handler_block = production_source
        .split(".invoke_handler(tauri::generate_handler![")
        .nth(1)
        .and_then(|source| source.split("])").next())
        .expect("tauri generate_handler block");

    let registered_commands: Vec<&str> = generate_handler_block
        .lines()
        .map(|line| line.trim().trim_end_matches(','))
        .filter(|line| !line.is_empty())
        .collect();

    assert!(registered_commands.contains(&"set_floating_chrome_reason"));
}

#[test]
fn floating_windows_start_collapsed_in_tauri_config() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let config = fs::read_to_string(manifest_dir.join("tauri.conf.json")).expect("tauri config");
    let config: Value = serde_json::from_str(&config).expect("valid tauri config json");
    let recorder = config["app"]["windows"]
        .as_array()
        .expect("windows array")
        .iter()
        .find(|window| window["label"].as_str() == Some("recorder"))
        .expect("recorder window configured");
    let language = config["app"]["windows"]
        .as_array()
        .expect("windows array")
        .iter()
        .find(|window| window["label"].as_str() == Some("language"))
        .expect("language window configured");

    assert_eq!(recorder["width"].as_f64(), Some(96.0));
    assert_eq!(recorder["height"].as_f64(), Some(10.0));
    assert_eq!(recorder["visible"].as_bool(), Some(true));
    assert_eq!(language["visible"].as_bool(), Some(false));
}

#[test]
fn native_floating_chrome_hides_language_when_collapsed() {
    let source = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs"))
        .expect("lib source");
    let production_source = source
        .split("\n#[cfg(test)]")
        .next()
        .expect("production lib source before tests");

    assert!(production_source.contains("apply_floating_chrome_windows"));
    assert!(production_source.contains("language_window.hide()"));
    assert!(production_source.contains("language_window.show()"));
    assert!(production_source.contains("wispergo://floating-chrome-expanded-changed"));
}
```

- [ ] **Step 2: Run tests and verify they fail**

Run:

```bash
cargo test -p wispergo-desktop --lib floating_chrome_command_is_registered
cargo test -p wispergo-desktop --lib floating_windows_start_collapsed_in_tauri_config
cargo test -p wispergo-desktop --lib native_floating_chrome_hides_language_when_collapsed
```

Expected: failures because command/config/window apply behavior is not implemented.

- [ ] **Step 3: Implement managed floating chrome state and command**

In `apps/desktop/src-tauri/src/lib.rs`, add imports near existing imports:

```rust
use std::sync::Mutex;
```

Add this managed state near `FloatingChromeReasonState`:

```rust
#[derive(Default)]
struct FloatingChromeState {
    reasons: Mutex<FloatingChromeReasonState>,
}
```

Add reason parsing:

```rust
fn floating_reason_from_str(reason: &str) -> Result<FloatingChromeReason, String> {
    match reason {
        "language_hover" => Ok(FloatingChromeReason::LanguageHover),
        "language_menu" => Ok(FloatingChromeReason::LanguageMenu),
        "recording" => Ok(FloatingChromeReason::Recording),
        "processing" => Ok(FloatingChromeReason::Processing),
        "post_insert" => Ok(FloatingChromeReason::PostInsert),
        _ => Err("unknown floating chrome reason".to_string()),
    }
}
```

Add state mutation helpers:

```rust
fn set_floating_chrome_reason_state(
    app: &tauri::AppHandle,
    state: &FloatingChromeState,
    reason: FloatingChromeReason,
    active: bool,
) -> Result<bool, String> {
    let expanded = {
        let mut reasons = state
            .reasons
            .lock()
            .map_err(|_| "floating chrome state poisoned".to_string())?;
        reasons.set(reason, active);
        floating_chrome_expanded(&reasons)
    };
    apply_floating_chrome_windows(app, expanded, false).map_err(|err| err.to_string())?;
    Ok(expanded)
}

#[tauri::command]
fn set_floating_chrome_reason(
    app: tauri::AppHandle,
    state: tauri::State<'_, FloatingChromeState>,
    reason: String,
    active: bool,
) -> Result<bool, String> {
    let reason = floating_reason_from_str(&reason)?;
    set_floating_chrome_reason_state(&app, state.inner(), reason, active)
}
```

Do not add new dependencies for this task.

Add window application helper:

```rust
fn apply_floating_chrome_windows(
    app: &tauri::AppHandle,
    expanded: bool,
    language_menu_open: bool,
) -> tauri::Result<()> {
    let recorder_mode = if expanded {
        FloatingRecorderMode::Expanded
    } else {
        FloatingRecorderMode::Collapsed
    };
    position_recorder_window(app, recorder_mode)?;

    if let Some(language_window) = app.get_webview_window("language") {
        if expanded {
            language_window.show()?;
            position_language_window(app, language_menu_open)?;
        } else {
            language_window.hide()?;
        }
    }

    let _ = app.emit("wispergo://floating-chrome-expanded-changed", expanded);
    Ok(())
}
```

Change `set_language_menu_open` to take `FloatingChromeState`, update the `language_menu` reason, and pass the real menu-open value into positioning:

```rust
#[tauri::command]
fn set_language_menu_open(
    app: tauri::AppHandle,
    state: tauri::State<'_, FloatingChromeState>,
    open: bool,
) -> Result<(), String> {
    {
        let mut reasons = state
            .reasons
            .lock()
            .map_err(|_| "floating chrome state poisoned".to_string())?;
        reasons.set(FloatingChromeReason::LanguageMenu, open);
    }

    let expanded = {
        let reasons = state
            .reasons
            .lock()
            .map_err(|_| "floating chrome state poisoned".to_string())?;
        floating_chrome_expanded(&reasons)
    };
    apply_floating_chrome_windows(&app, expanded, open).map_err(|err| err.to_string())
}
```

Register the state and command in `run()`:

```rust
.manage(FloatingChromeState::default())
```

and add `set_floating_chrome_reason` to `tauri::generate_handler![...]`.

In setup, replace direct positioning calls with:

```rust
let floating_chrome_state = app.state::<FloatingChromeState>();
apply_floating_chrome_windows(app.handle(), false, false)?;
```

Keep the `floating_chrome_state` binding only if needed by later code. If unused, do not introduce it.

- [ ] **Step 4: Update `tauri.conf.json` initial window state**

Set the recorder window to collapsed dimensions:

```json
"width": 96,
"height": 10,
```

Set the language window initial visibility to false:

```json
"visible": false,
```

Do not change the language window dimensions; it should stay `74 × 52` so native code can show it in the existing closed size.

- [ ] **Step 5: Run focused tests and fix compile errors**

Run:

```bash
cargo test -p wispergo-desktop --lib floating_chrome_command_is_registered
cargo test -p wispergo-desktop --lib floating_windows_start_collapsed_in_tauri_config
cargo test -p wispergo-desktop --lib native_floating_chrome_hides_language_when_collapsed
cargo test -p wispergo-desktop --lib
```

Expected: all pass.

- [ ] **Step 6: Commit Task 2**

```bash
git add apps/desktop/src-tauri/src/lib.rs apps/desktop/src-tauri/tauri.conf.json
git commit -m "feat: manage floating chrome window state"
```

---

### Task 3: Native recorder hover monitoring

**Files:**
- Modify: `apps/desktop/src-tauri/src/lib.rs`

- [ ] **Step 1: Write failing source tests for recorder hover monitor wiring**

Add these tests to `apps/desktop/src-tauri/src/lib.rs` tests:

```rust
#[test]
fn recorder_window_enables_macos_mouse_moved_events_for_hover_tracking() {
    let source = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs"))
        .expect("lib source");
    let production_source = source
        .split("\n#[cfg(test)]")
        .next()
        .expect("production lib source before tests");

    assert!(production_source.contains("configure_recorder_window_for_hover_tracking(app.handle())"));
    assert!(production_source.contains("setAcceptsMouseMovedEvents:"));
}

#[test]
fn recorder_window_reports_hover_while_app_is_inactive() {
    let source = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs"))
        .expect("lib source");
    let production_source = source
        .split("\n#[cfg(test)]")
        .next()
        .expect("production lib source before tests");

    assert!(production_source.contains("install_recorder_inactive_hover_monitor(app.handle())"));
    assert!(production_source.contains("wispergo://recorder-hover-changed"));
    assert!(production_source.contains("FloatingChromeReason::RecorderHover"));
}
```

- [ ] **Step 2: Run tests and verify they fail**

Run:

```bash
cargo test -p wispergo-desktop --lib recorder_window_enables_macos_mouse_moved_events_for_hover_tracking
cargo test -p wispergo-desktop --lib recorder_window_reports_hover_while_app_is_inactive
```

Expected: failure because recorder hover functions are not wired yet.

- [ ] **Step 3: Implement recorder hover setup and monitor**

Add this setup function beside the existing language setup function:

```rust
fn configure_recorder_window_for_hover_tracking(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window("recorder") else {
        return;
    };
    enable_mouse_moved_events(&window);
}
```

In `run()` setup, call it before installing monitors:

```rust
configure_recorder_window_for_hover_tracking(app.handle());
configure_language_window_for_hover_tracking(app.handle());
install_recorder_inactive_hover_monitor(app.handle());
install_language_inactive_hover_monitor(app.handle());
```

Implement macOS recorder monitor by copying the language monitor pattern but targeting the recorder window and updating native state directly:

```rust
#[cfg(target_os = "macos")]
fn install_recorder_inactive_hover_monitor(app: &tauri::AppHandle) {
    use std::ffi::{c_char, c_void};
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };

    use block2::RcBlock;

    const NS_MOUSE_MOVED_MASK: usize = 1 << 5;

    #[link(name = "objc")]
    unsafe extern "C" {
        fn objc_getClass(name: *const c_char) -> *mut c_void;
        fn sel_registerName(name: *const c_char) -> *mut c_void;

        #[allow(clashing_extern_declarations)]
        #[link_name = "objc_msgSend"]
        fn objc_msg_send_add_global_monitor(
            receiver: *mut c_void,
            selector: *mut c_void,
            mask: usize,
            handler: *mut c_void,
        ) -> *mut c_void;
    }

    let Some(window) = app.get_webview_window("recorder") else {
        return;
    };
    let Ok(ns_window) = window.ns_window() else {
        return;
    };

    let app = app.clone();
    let recorder_window = window.clone();
    let hover_inside = Arc::new(AtomicBool::new(false));
    let ns_window = ns_window as usize;
    let handler = RcBlock::new(move |_event: *mut c_void| {
        let ns_window = ns_window as *mut c_void;
        let inside = unsafe { cursor_is_inside_window(ns_window, &recorder_window) };
        let was_inside = hover_inside.swap(inside, Ordering::SeqCst);
        if was_inside == inside {
            return;
        }

        if let Some(state) = app.try_state::<FloatingChromeState>() {
            let _ = set_floating_chrome_reason_state(
                &app,
                state.inner(),
                FloatingChromeReason::RecorderHover,
                inside,
            );
        }
        let _ = app.emit("wispergo://recorder-hover-changed", inside);
    });

    let handler = RcBlock::into_raw(handler);

    unsafe {
        let event_class = objc_getClass(b"NSEvent\0".as_ptr().cast());
        let selector = sel_registerName(
            b"addGlobalMonitorForEventsMatchingMask:handler:\0"
                .as_ptr()
                .cast(),
        );
        if event_class.is_null() || selector.is_null() {
            return;
        }
        let _monitor = objc_msg_send_add_global_monitor(
            event_class,
            selector,
            NS_MOUSE_MOVED_MASK,
            handler.cast(),
        );
    }
}

#[cfg(not(target_os = "macos"))]
fn install_recorder_inactive_hover_monitor(_app: &tauri::AppHandle) {}
```

If `AppHandle::try_state` is unavailable in the Tauri version, use:

```rust
let state = app.state::<FloatingChromeState>();
let _ = set_floating_chrome_reason_state(
    &app,
    state.inner(),
    FloatingChromeReason::RecorderHover,
    inside,
);
```

Update the language monitor to update native floating state too, not only frontend class state:

```rust
if let Some(state) = app.try_state::<FloatingChromeState>() {
    let _ = set_floating_chrome_reason_state(
        &app,
        state.inner(),
        FloatingChromeReason::LanguageHover,
        inside,
    );
}
let _ = app.emit("wispergo://language-hover-changed", inside);
```

Use the `app.state::<FloatingChromeState>()` fallback if `try_state` is unavailable.

- [ ] **Step 4: Run focused and full native tests**

Run:

```bash
cargo test -p wispergo-desktop --lib recorder_window_enables_macos_mouse_moved_events_for_hover_tracking
cargo test -p wispergo-desktop --lib recorder_window_reports_hover_while_app_is_inactive
cargo test -p wispergo-desktop --lib
```

Expected: all pass.

- [ ] **Step 5: Commit Task 3**

```bash
git add apps/desktop/src-tauri/src/lib.rs
git commit -m "feat: expand floating UI on recorder hover"
```

---

### Task 4: Frontend floating chrome API and recorder state machine

**Files:**
- Modify: `apps/desktop/src/lib/tauriApi.ts`
- Modify: `apps/desktop/src/app/App.tsx`
- Modify: `apps/desktop/src/app/App.test.tsx`

- [ ] **Step 1: Write failing App tests for collapsed default and post-insertion grace**

In `apps/desktop/src/app/App.test.tsx`, add `setFloatingChromeReason` to the imported Tauri API symbols:

```ts
  setFloatingChromeReason,
```

Add it to the mock module:

```ts
  setFloatingChromeReason: vi.fn().mockResolvedValue(false),
```

Add reset/default setup in `beforeEach`:

```ts
    vi.mocked(setFloatingChromeReason).mockReset();
```

and:

```ts
    vi.mocked(setFloatingChromeReason).mockResolvedValue(false);
```

Add helper near `emitLanguageHover`:

```ts
async function emitFloatingChromeExpanded(payload: boolean) {
  await waitFor(() => {
    expect(eventListeners.has("wispergo://floating-chrome-expanded-changed")).toBe(true);
  });
  await act(async () => {
    eventListeners.get("wispergo://floating-chrome-expanded-changed")?.({ payload });
  });
}
```

Add these tests in the recorder/language surface area:

```ts
it("renders the recorder surface collapsed until native floating chrome expands", async () => {
  window.history.pushState({}, "", "/?surface=recorder");

  render(<App />);

  expect(screen.getByLabelText("Wispergo idle handle")).toBeInTheDocument();
  expect(screen.queryByText("Ready")).not.toBeInTheDocument();

  await emitFloatingChromeExpanded(true);

  expect(await screen.findByText("Ready")).toBeInTheDocument();
  expect(screen.queryByLabelText("Wispergo idle handle")).not.toBeInTheDocument();
});

it("keeps recorder expanded briefly after insertion then clears post-insert reason", async () => {
  vi.useFakeTimers();
  window.history.pushState({}, "", "/?surface=recorder");

  render(<App />);
  await emitFloatingChromeExpanded(true);
  await emitRecordShortcut("Pressed");
  await emitRecordShortcut("Released");

  expect(await screen.findByText("Inserted: hello from voice")).toBeInTheDocument();
  expect(setFloatingChromeReason).toHaveBeenCalledWith("post_insert", true);

  await act(async () => {
    await vi.advanceTimersByTimeAsync(1499);
  });
  expect(setFloatingChromeReason).not.toHaveBeenCalledWith("post_insert", false);

  await act(async () => {
    await vi.advanceTimersByTimeAsync(1);
  });
  expect(setFloatingChromeReason).toHaveBeenCalledWith("post_insert", false);
});
```

- [ ] **Step 2: Run tests and verify they fail**

Run:

```bash
pnpm --dir apps/desktop test -- App.test.tsx
```

Expected: failure because `setFloatingChromeReason`, native expanded listener, collapsed rendering, and post-insert reason management do not exist yet.

- [ ] **Step 3: Add Tauri API wrapper**

In `apps/desktop/src/lib/tauriApi.ts`, add:

```ts
export type FloatingChromeReason =
  | "language_hover"
  | "language_menu"
  | "recording"
  | "processing"
  | "post_insert";

export async function setFloatingChromeReason(
  reason: FloatingChromeReason,
  active: boolean,
): Promise<boolean> {
  return invoke<boolean>("set_floating_chrome_reason", { reason, active });
}
```

- [ ] **Step 4: Implement frontend floating chrome state in `App.tsx`**

Update imports from `../lib/tauriApi` to include `setFloatingChromeReason`.

Add constant near refresh intervals:

```ts
const POST_INSERT_EXPANDED_MS = 1500;
```

Add state/refs near existing language hover state:

```ts
  const [floatingChromeExpanded, setFloatingChromeExpanded] = useState(false);
  const postInsertTimerRef = useRef<number | null>(null);
```

Add cleanup effect:

```ts
  useEffect(() => {
    return () => {
      if (postInsertTimerRef.current !== null) {
        window.clearTimeout(postInsertTimerRef.current);
      }
    };
  }, []);
```

Add listener for native expansion on recorder and language surfaces:

```ts
  useEffect(() => {
    if (!isRecorderSurface && !isLanguageSurface) {
      return;
    }

    let mounted = true;
    const unlisten = listen<boolean>("wispergo://floating-chrome-expanded-changed", (event) => {
      if (mounted) {
        setFloatingChromeExpanded(event.payload);
      }
    });

    return () => {
      mounted = false;
      void unlisten.then((unsubscribe) => unsubscribe());
    };
  }, [isRecorderSurface, isLanguageSurface]);
```

Add listener for recorder hover event on recorder surface. This mostly keeps the frontend in sync for tests/debugging; native already owns expansion:

```ts
  useEffect(() => {
    if (!isRecorderSurface) {
      return;
    }

    let mounted = true;
    const unlisten = listen<boolean>("wispergo://recorder-hover-changed", (event) => {
      if (mounted) {
        setFloatingChromeExpanded(event.payload || statusRef.current === "recording" || pendingRef.current);
      }
    });

    return () => {
      mounted = false;
      void unlisten.then((unsubscribe) => unsubscribe());
    };
  }, [isRecorderSurface]);
```

Add status/pending native reason effect:

```ts
  useEffect(() => {
    if (!isRecorderSurface) {
      return;
    }

    void setFloatingChromeReason("recording", status === "recording").catch((err: unknown) => {
      setError(errorMessage(err));
    });
  }, [isRecorderSurface, status]);

  useEffect(() => {
    if (!isRecorderSurface) {
      return;
    }

    void setFloatingChromeReason("processing", pending).catch((err: unknown) => {
      setError(errorMessage(err));
    });
  }, [isRecorderSurface, pending]);
```

In the language hover listener, after `setLanguageNativeHovered(event.payload);`, add:

```ts
      void setFloatingChromeReason("language_hover", event.payload).catch((err: unknown) => {
        setError(errorMessage(err));
      });
```

Add helper inside `App` before `stopActiveRecording`:

```ts
  function startPostInsertExpandedGrace() {
    if (!isRecorderSurface) {
      return;
    }
    if (postInsertTimerRef.current !== null) {
      window.clearTimeout(postInsertTimerRef.current);
    }

    void setFloatingChromeReason("post_insert", true).catch((err: unknown) => {
      setError(errorMessage(err));
    });
    postInsertTimerRef.current = window.setTimeout(() => {
      postInsertTimerRef.current = null;
      void setFloatingChromeReason("post_insert", false).catch((err: unknown) => {
        setError(errorMessage(err));
      });
    }, POST_INSERT_EXPANDED_MS);
  }
```

Extend `runRecordingCommand` options with `onSettled?: () => void;` and call it in success and error branches after state updates:

```ts
    options: {
      errorStatus?: RecordingStatus;
      onSettledSuccess?: () => void;
      onSettled?: () => void;
    } = {},
```

In the success branch, after `options.onSettledSuccess?.();`, add:

```ts
          options.onSettled?.();
```

In the catch branch, after `applyPending(false);`, add:

```ts
          options.onSettled?.();
```

Update `stopActiveRecording` command options:

```ts
      { errorStatus: "idle", onSettled: startPostInsertExpandedGrace },
```

Pass `expanded={floatingChromeExpanded}` to `FloatingRecorder`:

```tsx
        <FloatingRecorder status={status} busy={pending} expanded={floatingChromeExpanded} />
```

- [ ] **Step 5: Run App tests**

Run:

```bash
pnpm --dir apps/desktop test -- App.test.tsx
```

Expected: tests still fail until Task 5 updates `FloatingRecorder` to accept `expanded` and render collapsed mode. If TypeScript fails only because `FloatingRecorder` lacks the prop, proceed to Task 5 before committing. If other App state tests fail, fix them in this task.

- [ ] **Step 6: Commit Task 4 after Task 5 makes frontend tests pass**

Do not commit with failing TypeScript. After Task 5 is complete and frontend tests pass, commit Task 4 and Task 5 together if they cannot compile independently:

```bash
git add apps/desktop/src/lib/tauriApi.ts apps/desktop/src/app/App.tsx apps/desktop/src/app/App.test.tsx
git commit -m "feat: drive floating chrome expansion from frontend"
```

If Task 4 compiles independently after adding a temporary default prop in Task 5, commit it separately.

---

### Task 5: Collapsed recorder rendering, styles, and frontend tests

**Files:**
- Modify: `apps/desktop/src/features/recorder/FloatingRecorder.tsx`
- Modify: `apps/desktop/src/features/recorder/FloatingRecorder.test.tsx`
- Modify: `apps/desktop/src/styles.css`
- Potentially include uncommitted files from Task 4 if they compile together.

- [ ] **Step 1: Write failing `FloatingRecorder` tests for collapsed and expanded modes**

Replace `apps/desktop/src/features/recorder/FloatingRecorder.test.tsx` tests with these mode-aware tests:

```tsx
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { FloatingRecorder } from "./FloatingRecorder";

describe("FloatingRecorder", () => {
  it("renders only the minimized handle while collapsed", () => {
    render(<FloatingRecorder status="idle" expanded={false} />);

    expect(screen.getByRole("region", { name: "Recorder" })).toBeInTheDocument();
    expect(screen.getByLabelText("Wispergo idle handle")).toBeInTheDocument();
    expect(screen.queryByText("Ready")).not.toBeInTheDocument();
    expect(screen.queryByText("hold Command + Shift + Space")).not.toBeInTheDocument();
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
  });

  it("renders a keyboard-only shortcut prompt while expanded and idle", () => {
    render(<FloatingRecorder status="idle" expanded />);

    expect(screen.getByRole("region", { name: "Recorder" })).toHaveTextContent("Ready");
    expect(screen.getByText("hold Command + Shift + Space")).toBeInTheDocument();
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
  });

  it("renders a concise recording prompt without controls while expanded", () => {
    render(<FloatingRecorder status="recording" expanded />);

    expect(screen.getByRole("region", { name: "Recorder" })).toHaveTextContent("Recording");
    expect(screen.getByText("release to insert")).toBeInTheDocument();
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
  });

  it("renders processing without exposing controls while expanded", () => {
    render(<FloatingRecorder status="idle" busy expanded />);

    expect(screen.getByRole("region", { name: "Recorder" })).toHaveTextContent("Processing");
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run tests and verify they fail**

Run:

```bash
pnpm --dir apps/desktop test -- FloatingRecorder.test.tsx
```

Expected: failure because `expanded` is not implemented yet.

- [ ] **Step 3: Implement collapsed rendering**

Replace `FloatingRecorder` props and component in `apps/desktop/src/features/recorder/FloatingRecorder.tsx` with:

```tsx
type RecordingStatus = "idle" | "recording";

type Props = {
  status: RecordingStatus;
  busy?: boolean;
  expanded?: boolean;
};

export function FloatingRecorder({ status, busy = false, expanded = true }: Props) {
  const isRecording = status === "recording";
  const className = ["floating-recorder", expanded ? "is-expanded" : "is-collapsed"].join(" ");

  if (!expanded) {
    return (
      <section className={className} aria-label="Recorder">
        <div className="recorder-idle-handle" aria-label="Wispergo idle handle" />
      </section>
    );
  }

  return (
    <section className={className} aria-label="Recorder">
      <div className="recording-dot" aria-hidden="true" />
      <div className="recording-copy">
        <div className="recording-status">
          {busy && !isRecording ? "Processing" : isRecording ? "Recording" : "Ready"}
        </div>
        <div className="recording-hint">
          {isRecording ? "release to insert" : "hold Command + Shift + Space"}
        </div>
      </div>
    </section>
  );
}
```

- [ ] **Step 4: Update CSS for collapsed handle, expanded pill, and lower surfaces**

In `apps/desktop/src/styles.css`, change `.recorder-surface` to remove padding by default and let each mode set its own dimensions:

```css
.recorder-surface {
  width: 100vw;
  min-height: 100vh;
  margin: 0;
  padding: 0;
  align-content: center;
  justify-content: center;
  overflow: hidden;
}
```

Replace `.floating-recorder` block with mode-aware styles:

```css
.floating-recorder {
  display: inline-grid;
  align-items: center;
  max-width: 100%;
  background: #05070a;
  color: #ffffff;
  box-shadow: none;
  transition:
    width 180ms ease,
    height 180ms ease,
    opacity 160ms ease,
    transform 180ms ease;
}

.floating-recorder.is-collapsed {
  width: 96px;
  height: 10px;
  grid-template-columns: 1fr;
  padding: 0;
  border-radius: 999px;
}

.floating-recorder.is-expanded {
  grid-template-columns: auto 1fr;
  gap: 10px;
  width: 304px;
  height: 48px;
  padding: 7px 14px;
  border-radius: 24px;
}

.recorder-idle-handle {
  width: 96px;
  height: 10px;
  border-radius: 999px;
  background: #05070a;
}
```

Update mobile override so collapsed recorder is not stretched on small screens:

```css
  .floating-recorder.is-expanded {
    min-width: 0;
    width: calc(100vw - 16px);
  }
```

Update recorder surface overrides after the media query:

```css
html[data-surface="recorder"] .app-shell {
  width: 100vw;
  min-height: 100vh;
  padding: 0;
  align-content: center;
}

html[data-surface="recorder"] .floating-recorder.is-expanded {
  width: 304px;
  height: 48px;
}

html[data-surface="recorder"] .floating-recorder.is-collapsed {
  width: 96px;
  height: 10px;
}
```

Add language transition polish:

```css
.language-toggle {
  display: grid;
  justify-items: end;
  gap: 6px;
  color: #ffffff;
  transition:
    opacity 160ms ease,
    transform 180ms ease;
}
```

If `.language-toggle` already exists, merge only the transition declarations into the existing block.

- [ ] **Step 5: Run frontend tests and build**

Run:

```bash
pnpm --dir apps/desktop test -- FloatingRecorder.test.tsx
pnpm --dir apps/desktop test -- App.test.tsx
pnpm --dir apps/desktop build
```

Expected: all pass.

- [ ] **Step 6: Run Rust lib tests because CSS/source tests inspect styles**

Run:

```bash
cargo test -p wispergo-desktop --lib
```

Expected: all pass. Update style-inspection tests in `apps/desktop/src-tauri/src/lib.rs` if they still assert the old unmodeled `.floating-recorder` width/height or old `padding: 7px 8px;` behavior. Keep the intent: recorder surface transparent, expanded pill fixed, collapsed handle fixed.

- [ ] **Step 7: Commit Task 4/5 frontend changes**

If Task 4 was not committed separately, include its files here:

```bash
git add apps/desktop/src/lib/tauriApi.ts apps/desktop/src/app/App.tsx apps/desktop/src/app/App.test.tsx apps/desktop/src/features/recorder/FloatingRecorder.tsx apps/desktop/src/features/recorder/FloatingRecorder.test.tsx apps/desktop/src/styles.css apps/desktop/src-tauri/src/lib.rs
git commit -m "feat: collapse idle floating recorder UI"
```

If Task 4 was already committed, use:

```bash
git add apps/desktop/src/features/recorder/FloatingRecorder.tsx apps/desktop/src/features/recorder/FloatingRecorder.test.tsx apps/desktop/src/styles.css apps/desktop/src-tauri/src/lib.rs
git commit -m "feat: render collapsed floating recorder handle"
```

---

### Task 6: End-to-end validation and final polish

**Files:**
- Modify only if validation uncovers a bug in files already touched by Tasks 1–5.

- [ ] **Step 1: Run full automated verification**

Run:

```bash
cargo fmt --check
cargo test --workspace
pnpm --dir apps/desktop test
pnpm --dir apps/desktop build
```

Expected: all pass.

- [ ] **Step 2: Build the app bundle**

Run:

```bash
pnpm desktop:build
./scripts/check-macos-bundle-inference-layout.sh
```

Expected: build succeeds, app bundle exists at:

```text
target/release/bundle/macos/Wispergo.app
```

and bundle layout check passes.

- [ ] **Step 3: Manual validation**

Launch:

```bash
open target/release/bundle/macos/Wispergo.app
```

Verify manually:

- Idle UI shows only a centered bottom handle.
- Handle is around `96px × 10px`.
- Handle bottom is about `40px` above screen bottom.
- Hovering the handle expands to the existing globe + recorder pill layout.
- Moving away collapses once no menu/recording/processing/post-insert reason is active.
- Pressing the keyboard shortcut expands immediately.
- Releasing the shortcut keeps the UI expanded during processing, then about `1.5s` after insertion, then collapses.
- Language menu keeps the UI expanded until menu closes and hover ends.
- Recorder pill remains non-clickable and does not steal focus.

- [ ] **Step 4: Commit validation fixes if any**

If validation required fixes:

```bash
git add <fixed files>
git commit -m "fix: polish collapsed floating UI behavior"
```

If no fixes were needed, do not create an empty commit.

- [ ] **Step 5: Final status**

Report:

- commits created
- verification commands and results
- whether manual validation was performed or still needs user validation
- rebuilt app path
