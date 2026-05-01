mod audio;
mod commands;
#[allow(dead_code)]
mod insertion;
mod platform;
mod state;
#[allow(dead_code)]
mod trigger;

use commands::recording::{cancel_recording, recording_status, start_recording, stop_recording};
use commands::settings::{
    accessibility_status, fallback_policy_label, list_microphones, load_persisted_settings,
    local_model_settings, microphone_status, recognition_language, request_accessibility,
    request_microphone_access, selected_microphone_id, set_local_model_settings,
    set_microphone_device, set_recognition_language,
};
use state::AppState;
use tauri::{Emitter, Manager};
use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut, ShortcutState};

#[tauri::command]
fn app_health() -> &'static str {
    "ok"
}

#[tauri::command]
fn set_language_menu_open(app: tauri::AppHandle, open: bool) -> Result<(), String> {
    position_language_window(&app, open).map_err(|err| err.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(move |app| {
            if let Err(err) = load_persisted_settings(app.handle(), app.state::<AppState>().inner())
            {
                eprintln!("settings load failed: {err}");
            }
            setup_global_shortcut(app.handle())?;
            setup_menu_bar(app)?;
            position_recorder_window(app.handle());
            position_language_window(app.handle(), false)?;
            configure_language_window_for_hover_tracking(app.handle());
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
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            app_health,
            start_recording,
            stop_recording,
            cancel_recording,
            recording_status,
            fallback_policy_label,
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
            set_language_menu_open
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
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
        let inside = unsafe { cursor_is_inside_window(ns_window, &language_window) };
        let was_inside = hover_inside.swap(inside, Ordering::SeqCst);
        if was_inside == inside {
            return;
        }

        if inside {
            unsafe { activate_language_window_on_hover(ns_window) };
        }
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
unsafe fn cursor_is_inside_window(
    ns_window: *mut std::ffi::c_void,
    language_window: &tauri::WebviewWindow,
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
    let Ok(size) = language_window.outer_size() else {
        return false;
    };
    let scale_factor = language_window.scale_factor().unwrap_or(1.0).max(1.0);
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

const FLOATING_BOTTOM_MARGIN: f64 = 88.0;
const FLOATING_GAP: f64 = 8.0;
const LANGUAGE_CLOSED_WIDTH: f64 = 74.0;
const LANGUAGE_CLOSED_HEIGHT: f64 = 52.0;
const LANGUAGE_OPEN_WIDTH: f64 = 260.0;
const LANGUAGE_OPEN_HEIGHT: f64 = 190.0;
const LANGUAGE_TOGGLE_BAR_HEIGHT: f64 = 40.0;

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

fn position_recorder_window(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window("recorder") else {
        return;
    };
    let monitor = window
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| window.primary_monitor().ok().flatten());
    let Some(monitor) = monitor else {
        return;
    };
    let Ok(window_size) = window.outer_size() else {
        return;
    };

    let monitor_position = monitor.position();
    let monitor_size = monitor.size();
    let bottom_margin = logical_to_physical_i32(FLOATING_BOTTOM_MARGIN, monitor.scale_factor());
    let x = centered_window_left(monitor_position.x, monitor_size.width, window_size.width);
    let y =
        monitor_position.y + monitor_size.height as i32 - window_size.height as i32 - bottom_margin;
    let _ = window.set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(
        x, y,
    )));
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
        language_window_top_for_aligned_toggle_bar, recorder_window_ignores_cursor_events,
        should_hide_window_on_close,
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
        let bottom_margin = 88;
        let recorder_height = 62;
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
    fn app_registers_recognition_language_commands() {
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

        assert!(registered_commands.contains(&"recognition_language"));
        assert!(registered_commands.contains(&"set_recognition_language"));
        assert!(registered_commands.contains(&"set_language_menu_open"));
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
    fn recorder_pill_has_transparent_padding_and_fixed_radius() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let styles =
            fs::read_to_string(manifest_dir.join("../src/styles.css")).expect("frontend styles");

        assert!(styles.contains("padding: 7px 8px;"));
        assert!(styles.contains("height: 48px;"));
        let floating_recorder_styles = styles
            .split(".floating-recorder {")
            .nth(1)
            .and_then(|styles| styles.split('}').next())
            .expect("floating recorder styles exist");

        assert!(styles.contains("border-radius: 24px;"));
        assert!(styles.contains("html[data-surface=\"recorder\"] .app-shell"));
        assert!(styles.contains("html[data-surface=\"recorder\"] .floating-recorder"));
        assert!(floating_recorder_styles.contains("box-shadow: none;"));
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
