mod audio;
mod commands;
mod inference;
#[allow(dead_code)]
mod insertion;
mod platform;
mod state;
#[allow(dead_code)]
mod trigger;

use std::sync::Mutex;

use commands::recording::{cancel_recording, recording_status, start_recording, stop_recording};
use commands::settings::{
    accessibility_status, cleanup_runtime_status, ensure_ollama_setup, fallback_policy_label,
    list_microphones, load_persisted_settings, local_model_settings, microphone_status,
    recognition_language, request_accessibility, request_microphone_access, selected_microphone_id,
    set_local_model_settings, set_microphone_device, set_recognition_language,
    sync_cleanup_runtime_for_settings,
};
use inference::cleanup_runtime::CleanupRuntimeManager;
use state::AppState;
use tauri::{Emitter, Manager};
use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut, ShortcutState};

#[tauri::command]
fn app_health() -> &'static str {
    "ok"
}

#[tauri::command]
fn set_language_menu_open(
    app: tauri::AppHandle,
    state: tauri::State<'_, FloatingChromeState>,
    open: bool,
) -> Result<(), String> {
    set_floating_chrome_reason_active(
        &app,
        state.inner(),
        FloatingChromeReason::LanguageMenu,
        open,
    )
    .map(|_| ())
}

#[tauri::command]
fn set_floating_chrome_reason(
    app: tauri::AppHandle,
    state: tauri::State<'_, FloatingChromeState>,
    reason: String,
    active: bool,
) -> Result<bool, String> {
    let reason = parse_floating_chrome_reason(&reason)?;
    set_floating_chrome_reason_active(&app, state.inner(), reason, active)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::default())
        .manage(CleanupRuntimeManager::default())
        .manage(FloatingChromeState::default())
        .setup(move |app| {
            if let Err(err) = load_persisted_settings(app.handle(), app.state::<AppState>().inner())
            {
                eprintln!("settings load failed: {err}");
            }
            sync_cleanup_runtime_for_settings(
                app.handle(),
                app.state::<CleanupRuntimeManager>().inner(),
                &app.state::<AppState>().inner().local_model_settings(),
            );
            setup_global_shortcut(app.handle())?;
            setup_menu_bar(app)?;
            apply_floating_chrome_windows(app.handle(), false, false)?;
            configure_recorder_window_for_hover_tracking(app.handle());
            configure_language_window_for_hover_tracking(app.handle());
            install_recorder_inactive_hover_monitor(app.handle());
            install_language_inactive_hover_monitor(app.handle());
            if recorder_window_ignores_cursor_events() {
                if let Some(window) = app.get_webview_window("recorder") {
                    let _ = window.set_ignore_cursor_events(true);
                }
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if should_hide_window_on_close(window.label()) {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            app_health,
            start_recording,
            stop_recording,
            cancel_recording,
            recording_status,
            fallback_policy_label,
            cleanup_runtime_status,
            ensure_ollama_setup,
            list_microphones,
            selected_microphone_id,
            set_microphone_device,
            microphone_status,
            request_microphone_access,
            accessibility_status,
            request_accessibility,
            local_model_settings,
            set_local_model_settings,
            recognition_language,
            set_recognition_language,
            set_language_menu_open,
            set_floating_chrome_reason
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            if let tauri::RunEvent::Exit = event {
                app_handle.state::<CleanupRuntimeManager>().shutdown();
            }
        });
}

fn setup_global_shortcut(app: &tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let shortcut = Shortcut::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::Space);

    app.plugin(
        tauri_plugin_global_shortcut::Builder::new()
            .with_shortcut(shortcut)?
            .with_handler(|app, shortcut, event| {
                if !shortcut.matches(Modifiers::SUPER | Modifiers::SHIFT, Code::Space) {
                    return;
                }

                let payload = match event.state {
                    ShortcutState::Pressed => "Pressed",
                    ShortcutState::Released => "Released",
                };
                let _ = app.emit("wispergo://record-shortcut", payload);
            })
            .build(),
    )?;
    Ok(())
}

fn setup_menu_bar(app: &mut tauri::App) -> tauri::Result<()> {
    #[cfg(target_os = "macos")]
    app.set_activation_policy(tauri::ActivationPolicy::Accessory);

    let open_settings =
        tauri::menu::MenuItem::with_id(app, "open_settings", "Open Settings", true, None::<&str>)?;
    let quit = tauri::menu::MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = tauri::menu::Menu::with_items(app, &[&open_settings, &quit])?;

    let mut tray = tauri::tray::TrayIconBuilder::new()
        .tooltip("Wispergo")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open_settings" => {
                let _ = show_settings(app);
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let tauri::tray::TrayIconEvent::Click {
                button: tauri::tray::MouseButton::Left,
                button_state: tauri::tray::MouseButtonState::Up,
                ..
            } = event
            {
                let _ = show_settings(tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }

    tray.build(app)?;
    Ok(())
}

fn should_hide_window_on_close(label: &str) -> bool {
    label == "main"
}

fn recorder_window_ignores_cursor_events() -> bool {
    true
}

fn show_settings(app: &tauri::AppHandle) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window("main") {
        window.show()?;
        window.unminimize()?;
        window.set_focus()?;
    }
    Ok(())
}

fn configure_recorder_window_for_hover_tracking(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window("recorder") else {
        return;
    };
    enable_mouse_moved_events(&window);
}

fn configure_language_window_for_hover_tracking(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window("language") else {
        return;
    };
    enable_mouse_moved_events(&window);
}

#[cfg(target_os = "macos")]
fn enable_mouse_moved_events(window: &tauri::WebviewWindow) {
    use objc2::{
        msg_send,
        runtime::{AnyObject, Bool},
    };

    let Ok(ns_window) = window.ns_window() else {
        return;
    };
    let ns_window = ns_window.cast::<AnyObject>();

    unsafe {
        let _: () = msg_send![ns_window, setAcceptsMouseMovedEvents: Bool::YES];
    }
}

#[cfg(not(target_os = "macos"))]
fn enable_mouse_moved_events(_window: &tauri::WebviewWindow) {}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy)]
#[repr(C)]
struct MacosPoint {
    x: f64,
    y: f64,
}

#[cfg(target_os = "macos")]
fn install_language_inactive_hover_monitor(app: &tauri::AppHandle) {
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

    let Some(window) = app.get_webview_window("language") else {
        return;
    };
    let Ok(ns_window) = window.ns_window() else {
        return;
    };

    let app = app.clone();
    let language_window = window.clone();
    let hover_inside = Arc::new(AtomicBool::new(false));
    let ns_window = ns_window as usize;
    let handler = RcBlock::new(move |_event: *mut c_void| {
        let ns_window = ns_window as *mut c_void;
        let inside = language_window.is_visible().unwrap_or(false)
            && unsafe { cursor_is_inside_window(ns_window, &language_window) };
        let was_inside = hover_inside.swap(inside, Ordering::SeqCst);
        if was_inside == inside {
            return;
        }

        if inside {
            unsafe { activate_language_window_on_hover(ns_window) };
        }
        let state = app.state::<FloatingChromeState>();
        let _ = set_floating_chrome_reason_active(
            &app,
            state.inner(),
            FloatingChromeReason::LanguageHover,
            inside,
        );
        let _ = app.emit("wispergo://language-hover-changed", inside);
    });

    // The monitor is app-lifetime. Leak our retain so the block remains valid even if AppKit
    // does not synchronously copy it before this setup function returns.
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
fn install_language_inactive_hover_monitor(_app: &tauri::AppHandle) {}

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

        let state = app.state::<FloatingChromeState>();
        let _ = set_floating_chrome_reason_active(
            &app,
            state.inner(),
            FloatingChromeReason::RecorderHover,
            inside,
        );
        let _ = app.emit("wispergo://recorder-hover-changed", inside);
    });

    // The monitor is app-lifetime. Leak our retain so the block remains valid even if AppKit
    // does not synchronously copy it before this setup function returns.
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

#[cfg(target_os = "macos")]
unsafe fn cursor_is_inside_window(
    ns_window: *mut std::ffi::c_void,
    window: &tauri::WebviewWindow,
) -> bool {
    use std::ffi::{c_char, c_void};

    #[link(name = "objc")]
    unsafe extern "C" {
        fn sel_registerName(name: *const c_char) -> *mut c_void;

        #[allow(clashing_extern_declarations)]
        #[link_name = "objc_msgSend"]
        fn objc_msg_send_mouse_location(receiver: *mut c_void, selector: *mut c_void)
            -> MacosPoint;
    }

    let mouse_location_selector =
        unsafe { sel_registerName(b"mouseLocationOutsideOfEventStream\0".as_ptr().cast()) };
    if mouse_location_selector.is_null() {
        return false;
    }

    let mouse = unsafe { objc_msg_send_mouse_location(ns_window, mouse_location_selector) };
    let Ok(size) = window.outer_size() else {
        return false;
    };
    let scale_factor = window.scale_factor().unwrap_or(1.0).max(1.0);
    let width = size.width as f64 / scale_factor;
    let height = size.height as f64 / scale_factor;

    mouse.x >= 0.0 && mouse.x <= width && mouse.y >= 0.0 && mouse.y <= height
}

#[cfg(target_os = "macos")]
unsafe fn activate_language_window_on_hover(ns_window: *mut std::ffi::c_void) {
    use std::ffi::{c_char, c_void};

    use objc2::runtime::Bool;

    #[link(name = "objc")]
    unsafe extern "C" {
        fn objc_getClass(name: *const c_char) -> *mut c_void;
        fn sel_registerName(name: *const c_char) -> *mut c_void;

        #[allow(clashing_extern_declarations)]
        #[link_name = "objc_msgSend"]
        fn objc_msg_send_shared_application(
            receiver: *mut c_void,
            selector: *mut c_void,
        ) -> *mut c_void;

        #[allow(clashing_extern_declarations)]
        #[link_name = "objc_msgSend"]
        fn objc_msg_send_activate_ignoring_other_apps(
            receiver: *mut c_void,
            selector: *mut c_void,
            ignore_other_apps: Bool,
        );

        #[allow(clashing_extern_declarations)]
        #[link_name = "objc_msgSend"]
        fn objc_msg_send_make_key_window(receiver: *mut c_void, selector: *mut c_void);
    }

    let app_class = unsafe { objc_getClass(b"NSApplication\0".as_ptr().cast()) };
    let shared_selector = unsafe { sel_registerName(b"sharedApplication\0".as_ptr().cast()) };
    let activate_selector =
        unsafe { sel_registerName(b"activateIgnoringOtherApps:\0".as_ptr().cast()) };
    let make_key_selector = unsafe { sel_registerName(b"makeKeyWindow\0".as_ptr().cast()) };
    if app_class.is_null()
        || shared_selector.is_null()
        || activate_selector.is_null()
        || make_key_selector.is_null()
    {
        return;
    }

    let shared_app = unsafe { objc_msg_send_shared_application(app_class, shared_selector) };
    if shared_app.is_null() {
        return;
    }
    unsafe {
        objc_msg_send_activate_ignoring_other_apps(shared_app, activate_selector, Bool::YES);
        objc_msg_send_make_key_window(ns_window, make_key_selector);
    }
}

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

#[derive(Default)]
struct FloatingChromeState {
    reasons: Mutex<FloatingChromeReasonState>,
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

fn parse_floating_chrome_reason(reason: &str) -> Result<FloatingChromeReason, String> {
    match reason {
        "language_hover" => Ok(FloatingChromeReason::LanguageHover),
        "language_menu" => Ok(FloatingChromeReason::LanguageMenu),
        "recording" => Ok(FloatingChromeReason::Recording),
        "processing" => Ok(FloatingChromeReason::Processing),
        "post_insert" => Ok(FloatingChromeReason::PostInsert),
        _ => Err("unknown floating chrome reason".to_string()),
    }
}

fn floating_chrome_window_state(state: &FloatingChromeReasonState) -> (bool, bool) {
    (floating_chrome_expanded(state), state.language_menu)
}

fn set_floating_chrome_reason_active(
    app: &tauri::AppHandle,
    state: &FloatingChromeState,
    reason: FloatingChromeReason,
    active: bool,
) -> Result<bool, String> {
    let (expanded, language_menu_open) = {
        let mut reasons = state
            .reasons
            .lock()
            .map_err(|_| "floating chrome state unavailable".to_string())?;
        reasons.set(reason, active);
        floating_chrome_window_state(&reasons)
    };

    apply_floating_chrome_windows(app, expanded, language_menu_open)
        .map_err(|err| err.to_string())?;
    app.emit("wispergo://floating-chrome-expanded-changed", expanded)
        .map_err(|err| err.to_string())?;
    Ok(expanded)
}

fn language_window_visible_for_floating_chrome(expanded: bool) -> bool {
    expanded
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

fn logical_to_physical_i32(logical: f64, scale_factor: f64) -> i32 {
    (logical * scale_factor).round() as i32
}

fn logical_to_physical_u32(logical: f64, scale_factor: f64) -> u32 {
    logical_to_physical_i32(logical, scale_factor).max(0) as u32
}

fn centered_window_left(monitor_left: i32, monitor_width: u32, window_width: u32) -> i32 {
    monitor_left + (monitor_width as i32 - window_width as i32) / 2
}

fn language_window_top_for_aligned_toggle_bar(
    monitor_top: i32,
    monitor_height: u32,
    bottom_margin: i32,
    recorder_height: u32,
    language_height: i32,
    toggle_bar_height: i32,
) -> i32 {
    let monitor_bottom = monitor_top as f64 + monitor_height as f64;
    let recorder_center_y = monitor_bottom - bottom_margin as f64 - recorder_height as f64 / 2.0;
    (recorder_center_y - language_height as f64 + toggle_bar_height as f64 / 2.0).round() as i32
}

fn configured_window_physical_width(
    app: &tauri::AppHandle,
    label: &str,
    scale_factor: f64,
) -> Option<u32> {
    app.config()
        .app
        .windows
        .iter()
        .find(|window| window.label.as_str() == label)
        .map(|window| logical_to_physical_u32(window.width, scale_factor))
}

fn configured_window_physical_height(
    app: &tauri::AppHandle,
    label: &str,
    scale_factor: f64,
) -> Option<u32> {
    app.config()
        .app
        .windows
        .iter()
        .find(|window| window.label.as_str() == label)
        .map(|window| logical_to_physical_u32(window.height, scale_factor))
}

fn recorder_window_physical_width(app: &tauri::AppHandle, scale_factor: f64) -> Option<u32> {
    app.get_webview_window("recorder")
        .and_then(|window| window.outer_size().ok())
        .map(|size| size.width)
        .or_else(|| configured_window_physical_width(app, "recorder", scale_factor))
}

fn recorder_window_physical_height(app: &tauri::AppHandle, scale_factor: f64) -> Option<u32> {
    app.get_webview_window("recorder")
        .and_then(|window| window.outer_size().ok())
        .map(|size| size.height)
        .or_else(|| configured_window_physical_height(app, "recorder", scale_factor))
}

fn position_recorder_window(
    app: &tauri::AppHandle,
    mode: FloatingRecorderMode,
) -> tauri::Result<()> {
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

    let language_visible = language_window_visible_for_floating_chrome(expanded);
    if language_visible {
        position_language_window(app, language_menu_open)?;
        if let Some(window) = app.get_webview_window("language") {
            window.show()?;
        }
    } else if let Some(window) = app.get_webview_window("language") {
        window.hide()?;
    }

    Ok(())
}

fn position_language_window(app: &tauri::AppHandle, open: bool) -> tauri::Result<()> {
    let Some(window) = app.get_webview_window("language") else {
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

    let (width, height) = if open {
        (LANGUAGE_OPEN_WIDTH, LANGUAGE_OPEN_HEIGHT)
    } else {
        (LANGUAGE_CLOSED_WIDTH, LANGUAGE_CLOSED_HEIGHT)
    };

    window.set_size(tauri::Size::Logical(tauri::LogicalSize::new(width, height)))?;

    let monitor_position = monitor.position();
    let monitor_size = monitor.size();
    let scale_factor = monitor.scale_factor();
    let physical_width = logical_to_physical_i32(width, scale_factor);
    let physical_height = logical_to_physical_i32(height, scale_factor);
    let physical_gap = logical_to_physical_i32(FLOATING_GAP, scale_factor);
    let physical_toggle_bar_height =
        logical_to_physical_i32(LANGUAGE_TOGGLE_BAR_HEIGHT, scale_factor);
    let bottom_margin = logical_to_physical_i32(FLOATING_BOTTOM_MARGIN, scale_factor);
    let Some(recorder_width) = recorder_window_physical_width(app, scale_factor) else {
        return Ok(());
    };
    let Some(recorder_height) = recorder_window_physical_height(app, scale_factor) else {
        return Ok(());
    };
    let recorder_x = centered_window_left(monitor_position.x, monitor_size.width, recorder_width);
    let x = recorder_x - physical_gap - physical_width;
    let y = language_window_top_for_aligned_toggle_bar(
        monitor_position.y,
        monitor_size.height,
        bottom_margin,
        recorder_height,
        physical_height,
        physical_toggle_bar_height,
    );

    window.set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(
        x, y,
    )))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use serde_json::Value;

    use super::{
        floating_chrome_expanded, floating_chrome_window_state,
        language_window_top_for_aligned_toggle_bar, language_window_visible_for_floating_chrome,
        parse_floating_chrome_reason, recorder_window_ignores_cursor_events,
        recorder_window_size_for_mode, recorder_window_top_for_bottom_margin,
        should_hide_window_on_close, FloatingChromeReason, FloatingChromeReasonState,
        FloatingRecorderMode, FLOATING_BOTTOM_MARGIN,
    };

    #[test]
    fn settings_window_close_hides_instead_of_destroying_window() {
        assert!(should_hide_window_on_close("main"));
        assert!(!should_hide_window_on_close("recorder"));
    }

    #[test]
    fn recorder_window_ignores_cursor_events_because_it_is_keyboard_only() {
        assert!(recorder_window_ignores_cursor_events());
    }

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
        assert!(!floating_chrome_expanded(
            &FloatingChromeReasonState::default()
        ));

        for reason in [
            FloatingChromeReason::RecorderHover,
            FloatingChromeReason::LanguageHover,
            FloatingChromeReason::LanguageMenu,
            FloatingChromeReason::Recording,
            FloatingChromeReason::Processing,
            FloatingChromeReason::PostInsert,
        ] {
            let mut state = FloatingChromeReasonState::default();
            state.set(reason, true);
            assert!(floating_chrome_expanded(&state));

            state.set(reason, false);
            assert!(!floating_chrome_expanded(&state));
        }
    }

    #[test]
    fn floating_chrome_window_state_preserves_language_menu_open_reason() {
        let mut state = FloatingChromeReasonState::default();
        state.set(FloatingChromeReason::LanguageMenu, true);
        state.set(FloatingChromeReason::Processing, true);

        assert_eq!(floating_chrome_window_state(&state), (true, true));

        state.set(FloatingChromeReason::LanguageMenu, false);
        assert_eq!(floating_chrome_window_state(&state), (true, false));
    }

    #[test]
    fn floating_chrome_reason_parser_accepts_frontend_reasons() {
        assert_eq!(
            parse_floating_chrome_reason("language_hover"),
            Ok(FloatingChromeReason::LanguageHover)
        );
        assert_eq!(
            parse_floating_chrome_reason("language_menu"),
            Ok(FloatingChromeReason::LanguageMenu)
        );
        assert_eq!(
            parse_floating_chrome_reason("recording"),
            Ok(FloatingChromeReason::Recording)
        );
        assert_eq!(
            parse_floating_chrome_reason("processing"),
            Ok(FloatingChromeReason::Processing)
        );
        assert_eq!(
            parse_floating_chrome_reason("post_insert"),
            Ok(FloatingChromeReason::PostInsert)
        );
        assert_eq!(
            parse_floating_chrome_reason("<script>unexpected</script>"),
            Err("unknown floating chrome reason".to_string())
        );
    }

    fn registered_tauri_commands() -> Vec<String> {
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

        generate_handler_block
            .lines()
            .map(|line| line.trim().trim_end_matches(',').to_string())
            .filter(|line| !line.is_empty())
            .collect()
    }

    #[test]
    fn app_registers_recognition_language_and_ollama_setup_commands() {
        let registered_commands = registered_tauri_commands();

        assert!(registered_commands.contains(&"recognition_language".to_string()));
        assert!(registered_commands.contains(&"set_recognition_language".to_string()));
        assert!(registered_commands.contains(&"set_language_menu_open".to_string()));
        assert!(registered_commands.contains(&"ensure_ollama_setup".to_string()));
    }

    #[test]
    fn floating_chrome_command_is_registered() {
        let registered_commands = registered_tauri_commands();

        assert!(registered_commands.contains(&"set_floating_chrome_reason".to_string()));
    }

    #[test]
    fn native_floating_chrome_emits_expanded_changed_event() {
        let source = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs"))
            .expect("lib source");
        let production_source = source
            .split("\n#[cfg(test)]")
            .next()
            .expect("production lib source before tests");
        let floating_chrome_update = production_source
            .split("fn set_floating_chrome_reason_active(")
            .nth(1)
            .and_then(|source| {
                source
                    .split("\nfn language_window_visible_for_floating_chrome")
                    .next()
            })
            .expect("floating chrome native state update function");

        assert!(floating_chrome_update
            .contains("apply_floating_chrome_windows(app, expanded, language_menu_open)"));
        assert!(floating_chrome_update
            .contains("app.emit(\"wispergo://floating-chrome-expanded-changed\", expanded)"));
    }

    #[test]
    fn floating_windows_start_collapsed_in_tauri_config() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let config =
            fs::read_to_string(manifest_dir.join("tauri.conf.json")).expect("tauri config");
        let config: Value = serde_json::from_str(&config).expect("valid tauri config json");
        let windows = config["app"]["windows"].as_array().expect("windows array");
        let recorder = windows
            .iter()
            .find(|window| window["label"].as_str() == Some("recorder"))
            .expect("recorder window configured");
        let language = windows
            .iter()
            .find(|window| window["label"].as_str() == Some("language"))
            .expect("language window configured");

        assert_eq!(recorder["width"].as_u64(), Some(96));
        assert_eq!(recorder["height"].as_u64(), Some(10));
        assert_eq!(recorder["visible"].as_bool(), Some(true));
        assert_eq!(language["visible"].as_bool(), Some(false));
    }

    #[test]
    fn native_floating_chrome_hides_language_when_collapsed() {
        assert!(!language_window_visible_for_floating_chrome(false));
        assert!(language_window_visible_for_floating_chrome(true));

        let source = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs"))
            .expect("lib source");
        let production_source = source
            .split("\n#[cfg(test)]")
            .next()
            .expect("production lib source before tests");

        assert!(production_source
            .contains("apply_floating_chrome_windows(app.handle(), false, false)?"));
        assert!(production_source.contains("window.hide()?;"));
    }

    #[test]
    fn recorder_window_does_not_steal_target_app_focus() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let config =
            fs::read_to_string(manifest_dir.join("tauri.conf.json")).expect("tauri config");
        let config: Value = serde_json::from_str(&config).expect("valid tauri config json");
        let recorder = config["app"]["windows"]
            .as_array()
            .expect("windows array")
            .iter()
            .find(|window| window["label"].as_str() == Some("recorder"))
            .expect("recorder window configured");

        assert_eq!(recorder["focus"].as_bool(), Some(false));
        assert_eq!(recorder["focusable"].as_bool(), Some(false));
    }

    #[test]
    fn language_window_enables_macos_mouse_moved_events_for_hover_tracking() {
        let source = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs"))
            .expect("lib source");
        let production_source = source
            .split("\n#[cfg(test)]")
            .next()
            .expect("production lib source before tests");

        assert!(production_source
            .contains("configure_language_window_for_hover_tracking(app.handle())"));
        assert!(production_source.contains("setAcceptsMouseMovedEvents:"));
    }

    #[test]
    fn language_window_reports_hover_while_app_is_inactive() {
        let source = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs"))
            .expect("lib source");
        let production_source = source
            .split("\n#[cfg(test)]")
            .next()
            .expect("production lib source before tests");

        assert!(production_source.contains("install_language_inactive_hover_monitor(app.handle())"));
        assert!(production_source.contains("addGlobalMonitorForEventsMatchingMask:handler:"));
        assert!(production_source.contains("mouseLocationOutsideOfEventStream"));
        assert!(production_source.contains("activateIgnoringOtherApps:"));
        assert!(production_source.contains("wispergo://language-hover-changed"));
        assert!(!production_source.contains("objc_msg_send_frame"));
    }

    #[test]
    fn language_hover_monitor_ignores_hidden_language_window() {
        let source = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs"))
            .expect("lib source");
        let production_source = source
            .split("\n#[cfg(test)]")
            .next()
            .expect("production lib source before tests");
        let language_hover_monitor = production_source
            .split("fn install_language_inactive_hover_monitor(")
            .nth(1)
            .and_then(|source| source.split("\n#[cfg(not(target_os = \"macos\"))]").next())
            .expect("language inactive hover monitor source");

        let visibility_check = language_hover_monitor
            .find("language_window.is_visible().unwrap_or(false)")
            .expect("language hover monitor checks window visibility");
        let cursor_check = language_hover_monitor
            .find("cursor_is_inside_window(ns_window, &language_window)")
            .expect("language hover monitor checks cursor position");
        let hover_state_update = language_hover_monitor
            .find("FloatingChromeReason::LanguageHover")
            .expect("language hover monitor updates language hover reason");

        assert!(visibility_check < cursor_check);
        assert!(visibility_check < hover_state_update);
    }

    #[test]
    fn recorder_window_enables_macos_mouse_moved_events_for_hover_tracking() {
        let source = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs"))
            .expect("lib source");
        let production_source = source
            .split("\n#[cfg(test)]")
            .next()
            .expect("production lib source before tests");

        assert!(production_source
            .contains("configure_recorder_window_for_hover_tracking(app.handle())"));
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

    #[test]
    fn language_window_is_configured_as_separate_interactive_surface() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let config =
            fs::read_to_string(manifest_dir.join("tauri.conf.json")).expect("tauri config");
        let config: Value = serde_json::from_str(&config).expect("valid tauri config json");
        let language = config["app"]["windows"]
            .as_array()
            .expect("windows array")
            .iter()
            .find(|window| window["label"].as_str() == Some("language"))
            .expect("language window configured");

        assert_eq!(language["url"].as_str(), Some("/?surface=language"));
        assert_eq!(language["transparent"].as_bool(), Some(true));
        assert_eq!(language["backgroundColor"].as_str(), Some("#00000000"));
        assert_eq!(language["decorations"].as_bool(), Some(false));
        assert_eq!(language["alwaysOnTop"].as_bool(), Some(true));
        assert_eq!(language["focus"].as_bool(), Some(false));
        assert_eq!(language["focusable"].as_bool(), Some(true));
        assert_eq!(language["acceptFirstMouse"].as_bool(), Some(true));
    }

    #[test]
    fn default_capability_includes_language_window() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let capability = fs::read_to_string(manifest_dir.join("capabilities/default.json"))
            .expect("default capability");
        let capability: Value = serde_json::from_str(&capability).expect("valid capability json");
        let windows = capability["windows"]
            .as_array()
            .expect("capability windows array");

        assert!(windows.iter().any(|window| window.as_str() == Some("main")));
        assert!(windows
            .iter()
            .any(|window| window.as_str() == Some("recorder")));
        assert!(windows
            .iter()
            .any(|window| window.as_str() == Some("language")));
    }

    #[test]
    fn recorder_window_disables_opaque_background_and_shadow() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let config =
            fs::read_to_string(manifest_dir.join("tauri.conf.json")).expect("tauri config");
        let config: Value = serde_json::from_str(&config).expect("valid tauri config json");
        let recorder = config["app"]["windows"]
            .as_array()
            .expect("windows array")
            .iter()
            .find(|window| window["label"].as_str() == Some("recorder"))
            .expect("recorder window configured");

        assert_eq!(config["app"]["macOSPrivateApi"].as_bool(), Some(true));
        assert_eq!(recorder["transparent"].as_bool(), Some(true));
        assert_eq!(recorder["backgroundColor"].as_str(), Some("#00000000"));
        assert_eq!(recorder["shadow"].as_bool(), Some(false));
    }

    #[test]
    fn recorder_surface_css_clears_html_body_and_root_backgrounds() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let styles =
            fs::read_to_string(manifest_dir.join("../src/styles.css")).expect("frontend styles");
        let index = fs::read_to_string(manifest_dir.join("../index.html")).expect("index html");

        assert!(styles.contains("html[data-surface=\"recorder\"]"));
        assert!(styles.contains("body[data-surface=\"recorder\"]"));
        assert!(styles.contains("#root"));
        assert!(index.contains("requestedSurface === \"recorder\""));
        assert!(index.contains("requestedSurface === \"language\""));
        assert!(index.contains("document.documentElement.dataset.surface = surface"));
        assert!(index.contains("document.body.dataset.surface = surface"));
    }

    #[test]
    fn language_surface_css_stays_transparent_and_anchored_after_mobile_overrides() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let styles =
            fs::read_to_string(manifest_dir.join("../src/styles.css")).expect("frontend styles");

        assert!(styles.contains("html[data-surface=\"language\"]"));
        assert!(styles.contains("html[data-surface=\"language\"] body"));
        assert!(styles.contains("html[data-surface=\"language\"] #root"));
        assert!(styles.contains("body[data-surface=\"language\"]"));

        let mobile_media_index = styles
            .find("@media (max-width: 560px)")
            .expect("mobile media query exists");
        let language_shell_index = styles
            .find("html[data-surface=\"language\"] .app-shell")
            .expect("language shell override exists");
        assert!(
            language_shell_index > mobile_media_index,
            "language shell override must come after mobile media rules"
        );

        let language_shell_styles = styles[language_shell_index..]
            .split('{')
            .nth(1)
            .and_then(|styles| styles.split('}').next())
            .expect("language shell styles exist");
        assert!(language_shell_styles.contains("width: 100vw;"));
        assert!(language_shell_styles.contains("min-height: 100vh;"));
        assert!(language_shell_styles.contains("padding: 0;"));
        assert!(language_shell_styles.contains("align-content: end;"));
        assert!(language_shell_styles.contains("justify-content: end;"));

        let post_media_styles = &styles[mobile_media_index..];
        assert!(post_media_styles.contains("html[data-surface=\"language\"] .language-current"));
        assert!(post_media_styles.contains("width: 40px;"));
        assert!(post_media_styles.contains("html[data-surface=\"language\"] .language-chevron"));
        assert!(post_media_styles.contains("width: 0;"));
        assert!(
            post_media_styles.contains("html[data-surface=\"language\"] .language-current:hover")
        );
        assert!(post_media_styles.contains("background: transparent;"));
    }

    #[test]
    fn language_chevron_reveals_on_hover_without_sticking_after_focus() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let styles =
            fs::read_to_string(manifest_dir.join("../src/styles.css")).expect("frontend styles");

        assert!(styles.contains(".language-toggle:hover .language-chevron"));
        assert!(styles.contains(".language-toggle.is-native-hovered .language-chevron"));
        assert!(styles.contains(".language-toggle.is-open .language-chevron"));
        assert!(
            !styles.contains(".language-toggle:focus-within .language-chevron"),
            "click focus must not keep the hover-only chevron visible after the pointer leaves"
        );
    }

    #[test]
    fn recorder_styles_size_collapsed_handle_without_extra_surface_padding() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let styles =
            fs::read_to_string(manifest_dir.join("../src/styles.css")).expect("frontend styles");

        let recorder_surface_styles = styles
            .split(".recorder-surface {")
            .nth(1)
            .and_then(|styles| styles.split('}').next())
            .expect("recorder surface styles exist");
        let floating_recorder_styles = styles
            .split(".floating-recorder {")
            .nth(1)
            .and_then(|styles| styles.split('}').next())
            .expect("floating recorder styles exist");
        let collapsed_recorder_styles = styles
            .split(".floating-recorder.is-collapsed {")
            .nth(1)
            .and_then(|styles| styles.split('}').next())
            .expect("collapsed recorder styles exist");
        let expanded_recorder_styles = styles
            .split(".floating-recorder.is-expanded {")
            .nth(1)
            .and_then(|styles| styles.split('}').next())
            .expect("expanded recorder styles exist");

        assert!(recorder_surface_styles.contains("padding: 0;"));
        assert!(collapsed_recorder_styles.contains("width: 96px;"));
        assert!(collapsed_recorder_styles.contains("height: 10px;"));
        assert!(expanded_recorder_styles.contains("height: 48px;"));
        assert!(expanded_recorder_styles.contains("border-radius: 24px;"));
        assert!(styles.contains("html[data-surface=\"recorder\"] .app-shell"));
        assert!(styles.contains("html[data-surface=\"recorder\"] .floating-recorder.is-collapsed"));
        assert!(styles.contains("html[data-surface=\"recorder\"] .floating-recorder.is-expanded"));
        assert!(floating_recorder_styles.contains("box-shadow: none;"));
        assert!(floating_recorder_styles.contains("transition:"));
        assert!(!floating_recorder_styles.contains("box-shadow: 0"));
    }

    #[test]
    fn desktop_build_runs_stable_macos_signing_script() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let root_dir = manifest_dir
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
            .expect("repo root");
        let package = fs::read_to_string(root_dir.join("package.json")).expect("package json");
        let sign_script = fs::read_to_string(root_dir.join("scripts/sign-macos-app.sh"))
            .expect("stable macOS signing script");
        let ensure_script =
            fs::read_to_string(root_dir.join("scripts/ensure-local-codesign-cert.sh"))
                .expect("local macOS code-signing identity script");
        let trust_script =
            fs::read_to_string(root_dir.join("scripts/trust-local-codesign-cert.sh"))
                .expect("local macOS code-signing trust script");

        assert!(package.contains("scripts/ensure-local-codesign-cert.sh"));
        assert!(package.contains("scripts/sign-macos-app.sh"));
        assert!(package.contains("scripts/trust-local-codesign-cert.sh"));
        assert!(sign_script.contains("--requirements"));
        assert!(sign_script.contains("IDENTIFIER=\"com.ribbonsdigital.wispergo\""));
        assert!(sign_script.contains("Wispergo Local Code Signing"));
        assert!(sign_script.contains("designated => identifier"));
        assert!(ensure_script.contains("extendedKeyUsage=codeSigning"));
        assert!(trust_script.contains("security add-trusted-cert"));
        assert!(trust_script.contains("/Library/Keychains/System.keychain"));
    }

    #[test]
    fn macos_bundle_declares_microphone_entitlement() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let config =
            fs::read_to_string(manifest_dir.join("tauri.conf.json")).expect("tauri config");
        let config: Value = serde_json::from_str(&config).expect("valid tauri config json");
        let entitlement_path = config["bundle"]["macOS"]["entitlements"]
            .as_str()
            .expect("macOS entitlements path configured");

        let entitlements = fs::read_to_string(manifest_dir.join(entitlement_path))
            .expect("configured macOS entitlements file exists");

        assert!(
            entitlements.contains("com.apple.security.device.audio-input"),
            "microphone access requires the macOS audio-input entitlement"
        );
        assert!(
            entitlements.contains("<true/>"),
            "audio-input entitlement must be enabled"
        );
    }
}
