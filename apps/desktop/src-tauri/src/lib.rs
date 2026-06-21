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

use commands::assets::{
    asset_integrity, asset_readiness, ensure_model_assets, repair_asset_by_id, AssetClient,
    AssetDownloadStatus,
};
use commands::recording::{cancel_recording, recording_status, start_recording, stop_recording};
use commands::settings::{
    accessibility_status, apply_local_model_settings, cleanup_runtime_status, ensure_ollama_setup,
    fallback_policy_label, list_microphones, load_persisted_settings, local_model_settings,
    microphone_status, recognition_language, request_accessibility, request_microphone_access,
    selected_microphone_id, set_local_model_settings, set_microphone_device,
    set_recognition_language, sync_inference_manager_for_settings,
};
use inference::manager::InferenceManager;
use state::{AppState, CleanupMode, RecognitionLanguage};
use tauri::{include_image, Emitter, Manager};
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
        .manage(InferenceManager::product())
        .manage(AssetClient::default())
        .manage(FloatingChromeState::default())
        .setup(move |app| {
            if let Err(err) = load_persisted_settings(app.handle(), app.state::<AppState>().inner())
            {
                eprintln!("settings load failed: {err}");
            }
            sync_inference_manager_for_settings(
                app.handle(),
                app.state::<InferenceManager>().inner(),
                &app.state::<AppState>().inner().local_model_settings(),
            );
            setup_global_shortcut(app.handle())?;
            setup_menu_bar(app)?;
            show_settings_if_setup_required(app.handle());
            apply_floating_chrome_windows(app.handle(), false, false, false)?;
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
            ensure_model_assets,
            asset_integrity,
            repair_asset_by_id,
            list_microphones,
            selected_microphone_id,
            set_microphone_device,
            microphone_status,
            request_microphone_access,
            accessibility_status,
            asset_readiness,
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
                let _ = app_handle.state::<InferenceManager>().shutdown();
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

const WISPERGO_TRAY_ID: &str = "wispergo-tray";

fn setup_menu_bar(app: &mut tauri::App) -> tauri::Result<()> {
    #[cfg(target_os = "macos")]
    app.set_activation_policy(tauri::ActivationPolicy::Accessory);

    let menu = build_tray_menu(app)?;

    let tray_icon = include_image!("./icons/tray-template.png");
    let tray = tauri::tray::TrayIconBuilder::with_id(WISPERGO_TRAY_ID)
        .icon(tray_icon)
        .icon_as_template(true)
        .tooltip("Wispergo")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| handle_tray_menu_event(app, event.id().as_ref()));

    tray.build(app)?;
    Ok(())
}

fn refresh_tray_menu(app: &tauri::AppHandle) {
    let Some(tray) = app.tray_by_id(WISPERGO_TRAY_ID) else {
        return;
    };
    match build_tray_menu(app) {
        Ok(menu) => {
            let _ = tray.set_menu(Some(menu));
        }
        Err(err) => eprintln!("tray menu refresh failed: {err}"),
    }
}

fn build_tray_menu<R: tauri::Runtime, M: Manager<R>>(
    manager: &M,
) -> tauri::Result<tauri::menu::Menu<R>> {
    let settings = manager.state::<AppState>().local_model_settings();
    let selected_microphone_id = manager.state::<AppState>().selected_microphone_id();

    let language_auto = tauri::menu::CheckMenuItem::with_id(
        manager,
        tray_menu_language_id(RecognitionLanguage::Auto),
        "Auto",
        true,
        settings.recognition_language == RecognitionLanguage::Auto,
        None::<&str>,
    )?;
    let language_en = tauri::menu::CheckMenuItem::with_id(
        manager,
        tray_menu_language_id(RecognitionLanguage::En),
        "English",
        true,
        settings.recognition_language == RecognitionLanguage::En,
        None::<&str>,
    )?;
    let language_zh = tauri::menu::CheckMenuItem::with_id(
        manager,
        tray_menu_language_id(RecognitionLanguage::Zh),
        "Chinese / Mixed",
        true,
        settings.recognition_language == RecognitionLanguage::Zh,
        None::<&str>,
    )?;
    let language = tauri::menu::Submenu::with_id_and_items(
        manager,
        "submenu_language",
        "Language",
        true,
        &[&language_auto, &language_en, &language_zh],
    )?;

    let model_medium = tauri::menu::CheckMenuItem::with_id(
        manager,
        tray_menu_asr_model_id("medium"),
        "Medium",
        true,
        settings.asr_model_id == "medium",
        None::<&str>,
    )?;
    let model_accuracy = tauri::menu::CheckMenuItem::with_id(
        manager,
        tray_menu_asr_model_id("large-v3-turbo"),
        "Accuracy Pack",
        true,
        settings.asr_model_id == "large-v3-turbo",
        None::<&str>,
    )?;
    let model = tauri::menu::Submenu::with_id_and_items(
        manager,
        "submenu_asr_model",
        "Dictation model",
        true,
        &[&model_medium, &model_accuracy],
    )?;

    let cleanup_off = tauri::menu::CheckMenuItem::with_id(
        manager,
        tray_menu_cleanup_id(CleanupMode::Off),
        "Off",
        true,
        settings.cleanup_mode == CleanupMode::Off,
        None::<&str>,
    )?;
    let cleanup_punctuation = tauri::menu::CheckMenuItem::with_id(
        manager,
        tray_menu_cleanup_id(CleanupMode::PunctuationOnly),
        "Punctuation only",
        true,
        settings.cleanup_mode == CleanupMode::PunctuationOnly,
        None::<&str>,
    )?;
    let cleanup_full = tauri::menu::CheckMenuItem::with_id(
        manager,
        tray_menu_cleanup_id(CleanupMode::FullCleanup),
        "Full cleanup",
        true,
        settings.cleanup_mode == CleanupMode::FullCleanup,
        None::<&str>,
    )?;
    let cleanup = tauri::menu::Submenu::with_id_and_items(
        manager,
        "submenu_cleanup",
        "Cleanup",
        true,
        &[&cleanup_off, &cleanup_punctuation, &cleanup_full],
    )?;

    let microphones = list_microphones().unwrap_or_default();
    let microphone =
        tauri::menu::Submenu::with_id(manager, "submenu_microphone", "Microphone", true)?;
    if microphones.is_empty() {
        let empty = tauri::menu::MenuItem::with_id(
            manager,
            "microphone:none",
            "No microphones found",
            false,
            None::<&str>,
        )?;
        microphone.append(&empty)?;
    } else {
        let has_system_default_device = microphones.iter().any(is_system_default_microphone);
        for device in microphones {
            let checked = tray_microphone_menu_item_checked(
                selected_microphone_id.as_deref(),
                &device.id,
                &device.name,
                device.is_default,
                has_system_default_device,
            );
            let item = tauri::menu::CheckMenuItem::with_id(
                manager,
                tray_menu_microphone_id(&device.id),
                device.name,
                true,
                checked,
                None::<&str>,
            )?;
            microphone.append(&item)?;
        }
    }

    let separator = tauri::menu::PredefinedMenuItem::separator(manager)?;
    let open_settings = tauri::menu::MenuItem::with_id(
        manager,
        "open_settings",
        "Open Settings",
        true,
        None::<&str>,
    )?;
    let quit = tauri::menu::MenuItem::with_id(manager, "quit", "Quit", true, None::<&str>)?;

    tauri::menu::Menu::with_items(
        manager,
        &[
            &language,
            &model,
            &cleanup,
            &microphone,
            &separator,
            &open_settings,
            &quit,
        ],
    )
}

fn handle_tray_menu_event(app: &tauri::AppHandle, id: &str) {
    match id {
        "open_settings" => {
            let _ = show_settings(app);
        }
        "quit" => {
            app.exit(0);
        }
        _ => handle_tray_settings_menu_event(app, id),
    }
}

fn handle_tray_settings_menu_event(app: &tauri::AppHandle, id: &str) {
    if let Some(language) = tray_menu_language_from_id(id) {
        let app = app.clone();
        if let Err(err) = set_recognition_language(
            app.clone(),
            app.state::<AppState>(),
            app.state::<InferenceManager>(),
            language,
        ) {
            eprintln!("tray language update failed: {err}");
        }
        refresh_tray_menu(&app);
        return;
    }

    if let Some(asr_model_id) = tray_menu_asr_model_from_id(id) {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            let mut settings = app.state::<AppState>().local_model_settings();
            settings.asr_model_id = asr_model_id;
            if let Err(err) = apply_local_model_settings(app.clone(), settings).await {
                eprintln!("tray dictation model update failed: {err}");
            }
            refresh_tray_menu(&app);
        });
        return;
    }

    if let Some(cleanup_mode) = tray_menu_cleanup_from_id(id) {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            let mut settings = app.state::<AppState>().local_model_settings();
            settings.cleanup_mode = cleanup_mode;
            if let Err(err) = apply_local_model_settings(app.clone(), settings).await {
                eprintln!("tray cleanup mode update failed: {err}");
            }
            refresh_tray_menu(&app);
        });
        return;
    }

    if let Some(device_id) = tray_menu_microphone_from_id(id) {
        if let Err(err) = set_microphone_device(app.state::<AppState>(), Some(device_id)) {
            eprintln!("tray microphone update failed: {err}");
        }
        refresh_tray_menu(app);
    }
}

fn tray_menu_language_id(language: RecognitionLanguage) -> String {
    match language {
        RecognitionLanguage::Auto => "language:auto".to_string(),
        RecognitionLanguage::En => "language:en".to_string(),
        RecognitionLanguage::Zh => "language:zh".to_string(),
    }
}

fn tray_menu_language_from_id(id: &str) -> Option<RecognitionLanguage> {
    match id {
        "language:auto" => Some(RecognitionLanguage::Auto),
        "language:en" => Some(RecognitionLanguage::En),
        "language:zh" => Some(RecognitionLanguage::Zh),
        _ => None,
    }
}

fn tray_menu_asr_model_id(model_id: &str) -> String {
    format!("asr_model:{model_id}")
}

fn tray_menu_asr_model_from_id(id: &str) -> Option<String> {
    id.strip_prefix("asr_model:").map(str::to_string)
}

fn tray_menu_cleanup_id(mode: CleanupMode) -> String {
    match mode {
        CleanupMode::Off => "cleanup:off".to_string(),
        CleanupMode::PunctuationOnly => "cleanup:punctuation_only".to_string(),
        CleanupMode::FullCleanup => "cleanup:full_cleanup".to_string(),
    }
}

fn tray_menu_cleanup_from_id(id: &str) -> Option<CleanupMode> {
    match id {
        "cleanup:off" => Some(CleanupMode::Off),
        "cleanup:punctuation_only" => Some(CleanupMode::PunctuationOnly),
        "cleanup:full_cleanup" => Some(CleanupMode::FullCleanup),
        _ => None,
    }
}

fn tray_microphone_menu_item_checked(
    selected_microphone_id: Option<&str>,
    device_id: &str,
    device_name: &str,
    is_default: bool,
    has_system_default_device: bool,
) -> bool {
    if let Some(selected_microphone_id) = selected_microphone_id {
        return selected_microphone_id == device_id;
    }

    if has_system_default_device {
        return is_system_default_microphone_parts(device_id, device_name);
    }

    is_default
}

fn is_system_default_microphone(device: &audio::AudioInputDevice) -> bool {
    is_system_default_microphone_parts(&device.id, &device.name)
}

fn is_system_default_microphone_parts(device_id: &str, device_name: &str) -> bool {
    device_id == "default" || device_name == "System Default"
}

fn tray_menu_microphone_id(device_id: &str) -> String {
    format!("microphone:{device_id}")
}

fn tray_menu_microphone_from_id(id: &str) -> Option<String> {
    id.strip_prefix("microphone:").map(str::to_string)
}

fn setup_window_should_open(
    microphone_granted: bool,
    accessibility_granted: bool,
    required_assets_ready: bool,
) -> bool {
    !microphone_granted || !accessibility_granted || !required_assets_ready
}

fn show_settings_if_setup_required(app: &tauri::AppHandle) {
    let microphone_ready = microphone_status().granted;
    let accessibility_ready = accessibility_status().granted;
    let assets_ready = matches!(asset_readiness(app.clone()), Ok(AssetDownloadStatus::Ready));

    if setup_window_should_open(microphone_ready, accessibility_ready, assets_ready) {
        let _ = show_settings(app);
    }
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
        let event_class = objc_getClass(c"NSEvent".as_ptr());
        let selector = sel_registerName(c"addGlobalMonitorForEventsMatchingMask:handler:".as_ptr());
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
        let state = app.state::<FloatingChromeState>();
        let recorder_mode = if current_floating_chrome_expanded(state.inner()).unwrap_or(false) {
            FloatingRecorderMode::Expanded
        } else {
            FloatingRecorderMode::Collapsed
        };
        let inside =
            unsafe { cursor_is_inside_recorder_window(ns_window, &recorder_window, recorder_mode) };
        let was_inside = hover_inside.swap(inside, Ordering::SeqCst);
        if was_inside == inside {
            return;
        }

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
        let event_class = objc_getClass(c"NSEvent".as_ptr());
        let selector = sel_registerName(c"addGlobalMonitorForEventsMatchingMask:handler:".as_ptr());
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
unsafe fn cursor_position_and_window_size(
    ns_window: *mut std::ffi::c_void,
    window: &tauri::WebviewWindow,
) -> Option<(f64, f64, f64, f64)> {
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
        unsafe { sel_registerName(c"mouseLocationOutsideOfEventStream".as_ptr()) };
    if mouse_location_selector.is_null() {
        return None;
    }

    let mouse = unsafe { objc_msg_send_mouse_location(ns_window, mouse_location_selector) };
    let Ok(size) = window.outer_size() else {
        return None;
    };
    let scale_factor = window.scale_factor().unwrap_or(1.0).max(1.0);
    let width = size.width as f64 / scale_factor;
    let height = size.height as f64 / scale_factor;

    Some((mouse.x, mouse.y, width, height))
}

#[cfg(target_os = "macos")]
unsafe fn cursor_is_inside_window(
    ns_window: *mut std::ffi::c_void,
    window: &tauri::WebviewWindow,
) -> bool {
    let Some((x, y, width, height)) =
        (unsafe { cursor_position_and_window_size(ns_window, window) })
    else {
        return false;
    };
    x >= 0.0 && x <= width && y >= 0.0 && y <= height
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

    let app_class = unsafe { objc_getClass(c"NSApplication".as_ptr()) };
    let shared_selector = unsafe { sel_registerName(c"sharedApplication".as_ptr()) };
    let activate_selector = unsafe { sel_registerName(c"activateIgnoringOtherApps:".as_ptr()) };
    let make_key_selector = unsafe { sel_registerName(c"makeKeyWindow".as_ptr()) };
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
const RECORDER_EXPANDED_WIDTH: f64 = 304.0;
const RECORDER_EXPANDED_HEIGHT: f64 = 48.0;
const LANGUAGE_CLOSED_WIDTH: f64 = 74.0;
const LANGUAGE_CLOSED_HEIGHT: f64 = 52.0;
const LANGUAGE_OPEN_WIDTH: f64 = 260.0;
const LANGUAGE_OPEN_HEIGHT: f64 = 190.0;
const LANGUAGE_TOGGLE_BAR_HEIGHT: f64 = 40.0;
const HOVER_COLLAPSE_GRACE_MS: u64 = 200;

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
struct HoverClearGeneration {
    recorder_hover: u64,
    language_hover: u64,
}

#[derive(Default)]
struct FloatingChromeState {
    reasons: Mutex<FloatingChromeReasonState>,
    hover_clear_generation: Mutex<HoverClearGeneration>,
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

fn floating_chrome_window_state(state: &FloatingChromeReasonState) -> (bool, bool, bool) {
    let expanded = floating_chrome_expanded(state);
    let language_visible = language_window_visible_for_floating_chrome(expanded, state.recording);
    (expanded, state.language_menu, language_visible)
}

fn is_hover_reason(reason: FloatingChromeReason) -> bool {
    matches!(
        reason,
        FloatingChromeReason::RecorderHover | FloatingChromeReason::LanguageHover
    )
}

fn bump_hover_clear_generation(
    state: &FloatingChromeState,
    reason: FloatingChromeReason,
) -> Result<Option<u64>, String> {
    let mut generation = state
        .hover_clear_generation
        .lock()
        .map_err(|_| "floating chrome hover state unavailable".to_string())?;
    match reason {
        FloatingChromeReason::RecorderHover => {
            generation.recorder_hover = generation.recorder_hover.wrapping_add(1);
            Ok(Some(generation.recorder_hover))
        }
        FloatingChromeReason::LanguageHover => {
            generation.language_hover = generation.language_hover.wrapping_add(1);
            Ok(Some(generation.language_hover))
        }
        _ => Ok(None),
    }
}

fn hover_clear_generation_matches(
    state: &FloatingChromeState,
    reason: FloatingChromeReason,
    expected: u64,
) -> bool {
    let Ok(generation) = state.hover_clear_generation.lock() else {
        return false;
    };
    match reason {
        FloatingChromeReason::RecorderHover => generation.recorder_hover == expected,
        FloatingChromeReason::LanguageHover => generation.language_hover == expected,
        _ => false,
    }
}

fn current_floating_chrome_expanded(state: &FloatingChromeState) -> Result<bool, String> {
    let (expanded, _, _) = current_floating_chrome_window_state(state)?;
    Ok(expanded)
}

fn current_floating_chrome_window_state(
    state: &FloatingChromeState,
) -> Result<(bool, bool, bool), String> {
    let reasons = state
        .reasons
        .lock()
        .map_err(|_| "floating chrome state unavailable".to_string())?;
    Ok(floating_chrome_window_state(&reasons))
}

fn set_floating_chrome_reason_active(
    app: &tauri::AppHandle,
    state: &FloatingChromeState,
    reason: FloatingChromeReason,
    active: bool,
) -> Result<bool, String> {
    if is_hover_reason(reason) {
        if active {
            let _ = bump_hover_clear_generation(state, reason)?;
        } else {
            return set_floating_chrome_hover_reason_inactive_after_grace(app, state, reason);
        }
    }

    set_floating_chrome_reason_active_immediate(app, state, reason, active)
}

fn set_floating_chrome_hover_reason_inactive_after_grace(
    app: &tauri::AppHandle,
    state: &FloatingChromeState,
    reason: FloatingChromeReason,
) -> Result<bool, String> {
    let Some(generation) = bump_hover_clear_generation(state, reason)? else {
        return set_floating_chrome_reason_active_immediate(app, state, reason, false);
    };
    let expanded = current_floating_chrome_expanded(state)?;
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(HOVER_COLLAPSE_GRACE_MS));
        let state = app.state::<FloatingChromeState>();
        if !hover_clear_generation_matches(state.inner(), reason, generation) {
            return;
        }
        let _ = set_floating_chrome_reason_active_immediate(&app, state.inner(), reason, false);
    });
    Ok(expanded)
}

fn set_floating_chrome_reason_active_immediate(
    app: &tauri::AppHandle,
    state: &FloatingChromeState,
    reason: FloatingChromeReason,
    active: bool,
) -> Result<bool, String> {
    let (expanded, language_menu_open, language_visible) = {
        let mut reasons = state
            .reasons
            .lock()
            .map_err(|_| "floating chrome state unavailable".to_string())?;
        reasons.set(reason, active);
        floating_chrome_window_state(&reasons)
    };

    apply_floating_chrome_windows(app, expanded, language_menu_open, language_visible)
        .map_err(|err| err.to_string())?;
    app.emit("wispergo://floating-chrome-expanded-changed", expanded)
        .map_err(|err| err.to_string())?;
    Ok(expanded)
}

fn language_window_visible_for_floating_chrome(expanded: bool, recording: bool) -> bool {
    expanded && !recording
}

fn recorder_window_size_for_mode(mode: FloatingRecorderMode) -> (f64, f64) {
    match mode {
        FloatingRecorderMode::Collapsed => (RECORDER_COLLAPSED_WIDTH, RECORDER_COLLAPSED_HEIGHT),
        FloatingRecorderMode::Expanded => (RECORDER_EXPANDED_WIDTH, RECORDER_EXPANDED_HEIGHT),
    }
}

fn recorder_native_window_size_for_mode(_mode: FloatingRecorderMode) -> (f64, f64) {
    (RECORDER_EXPANDED_WIDTH, RECORDER_EXPANDED_HEIGHT)
}

fn cursor_is_inside_recorder_visible_area(
    x: f64,
    y: f64,
    window_width: f64,
    window_height: f64,
    mode: FloatingRecorderMode,
) -> bool {
    match mode {
        FloatingRecorderMode::Collapsed => {
            let (handle_width, handle_height) = recorder_window_size_for_mode(mode);
            let left = (window_width - handle_width) / 2.0;
            let right = left + handle_width;
            x >= left && x <= right && y >= 0.0 && y <= handle_height
        }
        FloatingRecorderMode::Expanded => {
            x >= 0.0 && x <= window_width && y >= 0.0 && y <= window_height
        }
    }
}

#[cfg(target_os = "macos")]
unsafe fn cursor_is_inside_recorder_window(
    ns_window: *mut std::ffi::c_void,
    window: &tauri::WebviewWindow,
    mode: FloatingRecorderMode,
) -> bool {
    let Some((x, y, width, height)) =
        (unsafe { cursor_position_and_window_size(ns_window, window) })
    else {
        return false;
    };
    cursor_is_inside_recorder_visible_area(x, y, width, height, mode)
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

    let (logical_width, logical_height) = recorder_native_window_size_for_mode(mode);
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
    language_visible: bool,
) -> tauri::Result<()> {
    let recorder_mode = if expanded {
        FloatingRecorderMode::Expanded
    } else {
        FloatingRecorderMode::Collapsed
    };
    position_recorder_window(app, recorder_mode)?;

    if language_visible {
        position_language_window(app, language_menu_open, recorder_mode)?;
        if let Some(window) = app.get_webview_window("language") {
            window.show()?;
        }
    } else if let Some(window) = app.get_webview_window("language") {
        window.hide()?;
    }

    Ok(())
}

fn position_language_window(
    app: &tauri::AppHandle,
    open: bool,
    recorder_mode: FloatingRecorderMode,
) -> tauri::Result<()> {
    let Some(window) = app.get_webview_window("language") else {
        return Ok(());
    };
    let recorder_window = app.get_webview_window("recorder");
    let recorder_monitor = recorder_window.as_ref().and_then(|window| {
        window
            .current_monitor()
            .ok()
            .flatten()
            .or_else(|| window.primary_monitor().ok().flatten())
    });
    let monitor = recorder_monitor.or_else(|| {
        window
            .current_monitor()
            .ok()
            .flatten()
            .or_else(|| window.primary_monitor().ok().flatten())
    });
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
    let (recorder_logical_width, recorder_logical_height) =
        recorder_window_size_for_mode(recorder_mode);
    let recorder_width = logical_to_physical_u32(recorder_logical_width, scale_factor);
    let recorder_height = logical_to_physical_u32(recorder_logical_height, scale_factor);
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
        cursor_is_inside_recorder_visible_area, floating_chrome_expanded,
        floating_chrome_window_state, language_window_top_for_aligned_toggle_bar,
        language_window_visible_for_floating_chrome, parse_floating_chrome_reason,
        recorder_native_window_size_for_mode, recorder_window_ignores_cursor_events,
        recorder_window_size_for_mode, recorder_window_top_for_bottom_margin,
        setup_window_should_open, should_hide_window_on_close, tray_menu_asr_model_from_id,
        tray_menu_asr_model_id, tray_menu_cleanup_from_id, tray_menu_cleanup_id,
        tray_menu_language_from_id, tray_menu_language_id, tray_menu_microphone_from_id,
        tray_menu_microphone_id, tray_microphone_menu_item_checked, CleanupMode,
        FloatingChromeReason, FloatingChromeReasonState, FloatingRecorderMode, RecognitionLanguage,
        FLOATING_BOTTOM_MARGIN, HOVER_COLLAPSE_GRACE_MS,
    };

    #[test]
    fn setup_window_opens_when_required_setup_is_incomplete() {
        assert!(setup_window_should_open(false, true, true));
        assert!(setup_window_should_open(true, false, true));
        assert!(setup_window_should_open(true, true, false));
    }

    #[test]
    fn setup_window_stays_hidden_when_required_setup_is_complete() {
        assert!(!setup_window_should_open(true, true, true));
    }

    #[test]
    fn settings_dashboard_styles_use_custom_controls_and_window_fit() {
        let styles =
            fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/styles.css"))
                .expect("desktop styles");

        assert!(styles.contains("body[data-surface=\"settings\"]"));
        assert!(styles.contains("overflow: hidden"));
        assert!(styles.contains("appearance: none"));
        assert!(styles.contains("background-image: url(\"data:image/svg+xml"));
        assert!(!styles.contains(".square-glyph"));
        assert!(styles.contains(".settings-icon"));
        assert!(styles.contains("padding: 8px 8px 32px"));
        assert!(styles.contains(".settings-panel button"));
        assert!(styles.contains(".settings-primary-action"));
    }

    #[test]
    fn settings_window_is_large_enough_for_compact_dashboard() {
        let config: Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).expect("tauri config");
        let windows = config["app"]["windows"].as_array().expect("windows config");
        let main = windows
            .iter()
            .find(|window| window["label"] == "main")
            .expect("main settings window");

        assert!(main["width"].as_u64().unwrap() >= 920);
        assert!(main["height"].as_u64().unwrap() >= 1080);
    }

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
            (304.0, 48.0)
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
            48,
            FLOATING_BOTTOM_MARGIN as i32,
        );

        assert_eq!(collapsed_y, 850);
        assert_eq!(expanded_y, 812);
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
    fn floating_chrome_window_state_hides_language_surface_while_recording() {
        let mut state = FloatingChromeReasonState::default();
        state.set(FloatingChromeReason::LanguageHover, true);
        state.set(FloatingChromeReason::LanguageMenu, true);
        state.set(FloatingChromeReason::Recording, true);

        assert_eq!(floating_chrome_window_state(&state), (true, true, false));
        assert!(!language_window_visible_for_floating_chrome(true, true));

        state.set(FloatingChromeReason::Recording, false);
        assert_eq!(floating_chrome_window_state(&state), (true, true, true));
        assert!(language_window_visible_for_floating_chrome(true, false));
    }

    #[test]
    fn floating_chrome_window_state_preserves_language_menu_open_reason() {
        let mut state = FloatingChromeReasonState::default();
        state.set(FloatingChromeReason::LanguageMenu, true);
        state.set(FloatingChromeReason::Processing, true);

        assert_eq!(floating_chrome_window_state(&state), (true, true, true));

        state.set(FloatingChromeReason::LanguageMenu, false);
        assert_eq!(floating_chrome_window_state(&state), (true, false, true));
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
    fn tray_uses_separate_template_icon_for_light_and_dark_menu_bars() {
        let production_source = include_str!("lib.rs");
        let setup_menu_bar = production_source
            .split("fn setup_menu_bar(")
            .nth(1)
            .and_then(|source| source.split("\nfn should_hide_window_on_close").next())
            .expect("setup_menu_bar function body");

        assert!(setup_menu_bar.contains("include_image!(\"./icons/tray-template.png\")"));
        assert!(setup_menu_bar.contains(".icon_as_template(true)"));
        assert!(!setup_menu_bar.contains("app.default_window_icon()"));
    }

    #[test]
    fn tray_left_click_shows_nested_menu_instead_of_opening_settings() {
        let production_source = include_str!("lib.rs");
        let setup_menu_bar = production_source
            .split("fn setup_menu_bar(")
            .nth(1)
            .and_then(|source| source.split("\nfn build_tray_menu").next())
            .expect("setup_menu_bar function body");
        let build_tray_menu = production_source
            .split("fn build_tray_menu")
            .nth(1)
            .and_then(|source| source.split("\nfn handle_tray_menu_event").next())
            .expect("build_tray_menu function body");

        assert!(setup_menu_bar.contains(".menu(&menu)"));
        assert!(setup_menu_bar.contains(".show_menu_on_left_click(true)"));
        assert!(!setup_menu_bar.contains("on_tray_icon_event"));
        assert!(!setup_menu_bar.contains("MouseButton::Left"));
        assert!(build_tray_menu.contains("\"Language\""));
        assert!(build_tray_menu.contains("\"Dictation model\""));
        assert!(build_tray_menu.contains("\"Cleanup\""));
        assert!(build_tray_menu.contains("\"Microphone\""));
        assert!(build_tray_menu.contains("PredefinedMenuItem::separator"));
        assert!(
            build_tray_menu.find("\"Language\"").unwrap()
                < build_tray_menu.find("\"Open Settings\"").unwrap()
        );
        assert!(
            build_tray_menu.find("\"Dictation model\"").unwrap()
                < build_tray_menu.find("\"Open Settings\"").unwrap()
        );
        assert!(
            build_tray_menu.find("\"Cleanup\"").unwrap()
                < build_tray_menu.find("\"Open Settings\"").unwrap()
        );
        assert!(
            build_tray_menu.find("\"Microphone\"").unwrap()
                < build_tray_menu.find("\"Open Settings\"").unwrap()
        );
    }

    #[test]
    fn tray_microphone_menu_checks_only_one_default_choice() {
        assert!(tray_microphone_menu_item_checked(
            None,
            "default",
            "System Default",
            true,
            true,
        ));
        assert!(!tray_microphone_menu_item_checked(
            None,
            "airpods-pro",
            "Ryan’s AirPods Pro",
            true,
            true,
        ));
        assert!(tray_microphone_menu_item_checked(
            Some("airpods-pro"),
            "airpods-pro",
            "Ryan’s AirPods Pro",
            true,
            true,
        ));
        assert!(!tray_microphone_menu_item_checked(
            Some("airpods-pro"),
            "default",
            "System Default",
            true,
            true,
        ));
        assert!(tray_microphone_menu_item_checked(
            None,
            "only-device",
            "Only Device",
            true,
            false,
        ));
    }

    #[test]
    fn tray_menu_ids_round_trip_for_quick_settings() {
        assert_eq!(
            tray_menu_language_from_id(&tray_menu_language_id(RecognitionLanguage::Auto)),
            Some(RecognitionLanguage::Auto)
        );
        assert_eq!(
            tray_menu_language_from_id(&tray_menu_language_id(RecognitionLanguage::En)),
            Some(RecognitionLanguage::En)
        );
        assert_eq!(
            tray_menu_language_from_id(&tray_menu_language_id(RecognitionLanguage::Zh)),
            Some(RecognitionLanguage::Zh)
        );
        assert_eq!(
            tray_menu_asr_model_from_id(&tray_menu_asr_model_id("large-v3-turbo")),
            Some("large-v3-turbo".to_string())
        );
        assert_eq!(
            tray_menu_cleanup_from_id(&tray_menu_cleanup_id(CleanupMode::FullCleanup)),
            Some(CleanupMode::FullCleanup)
        );
        assert_eq!(
            tray_menu_microphone_from_id(&tray_menu_microphone_id("device-1")),
            Some("device-1".to_string())
        );
        assert_eq!(tray_menu_language_from_id("open_settings"), None);
        assert_eq!(tray_menu_cleanup_from_id("quit"), None);
    }

    #[test]
    fn release_icon_assets_exist_for_app_and_tray() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));

        assert!(manifest_dir.join("icons/icon.png").exists());
        assert!(manifest_dir.join("icons/tray-template.png").exists());
    }

    #[test]
    fn app_registers_recognition_language_and_ollama_setup_commands() {
        let registered_commands = registered_tauri_commands();

        assert!(registered_commands.contains(&"recognition_language".to_string()));
        assert!(registered_commands.contains(&"set_recognition_language".to_string()));
        assert!(registered_commands.contains(&"set_language_menu_open".to_string()));
        assert!(registered_commands.contains(&"ensure_ollama_setup".to_string()));
        assert!(registered_commands.contains(&"asset_readiness".to_string()));
        assert!(registered_commands.contains(&"ensure_model_assets".to_string()));
        assert!(registered_commands.contains(&"asset_integrity".to_string()));
        assert!(registered_commands.contains(&"repair_asset_by_id".to_string()));
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
            .split("fn set_floating_chrome_reason_active_immediate(")
            .nth(1)
            .and_then(|source| {
                source
                    .split("\nfn apply_floating_chrome_windows_after_collapse_delay")
                    .next()
            })
            .expect("floating chrome native state update function");

        assert!(floating_chrome_update
            .contains("app.emit(\"wispergo://floating-chrome-expanded-changed\", expanded)"));
        assert!(floating_chrome_update.contains("if expanded"));
        assert!(floating_chrome_update.contains(
            "apply_floating_chrome_windows(app, expanded, language_menu_open, language_visible)"
        ));
    }

    #[test]
    fn native_floating_chrome_applies_stable_host_geometry_without_collapse_delay() {
        let source = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs"))
            .expect("lib source");
        let production_source = source
            .split("\n#[cfg(test)]")
            .next()
            .expect("production lib source before tests");
        let floating_chrome_update = production_source
            .split("fn set_floating_chrome_reason_active_immediate(")
            .nth(1)
            .and_then(|source| {
                source
                    .split("\nfn language_window_visible_for_floating_chrome")
                    .next()
            })
            .expect("floating chrome native state update source");

        assert!(!production_source.contains("FLOATING_COLLAPSE_APPLY_DELAY_MS"));
        assert!(!production_source.contains("apply_floating_chrome_windows_after_collapse_delay"));
        assert!(floating_chrome_update.contains(
            "apply_floating_chrome_windows(app, expanded, language_menu_open, language_visible)"
        ));
        assert!(floating_chrome_update
            .contains("app.emit(\"wispergo://floating-chrome-expanded-changed\", expanded)"));
    }

    #[test]
    fn recorder_native_window_stays_expanded_sized_to_avoid_collapse_clipping() {
        assert_eq!(
            recorder_native_window_size_for_mode(FloatingRecorderMode::Collapsed),
            (304.0, 48.0)
        );
        assert_eq!(
            recorder_native_window_size_for_mode(FloatingRecorderMode::Expanded),
            (304.0, 48.0)
        );
    }

    #[test]
    fn collapsed_recorder_hover_uses_visible_handle_not_full_host_window() {
        assert!(cursor_is_inside_recorder_visible_area(
            152.0,
            5.0,
            304.0,
            48.0,
            FloatingRecorderMode::Collapsed,
        ));
        assert!(!cursor_is_inside_recorder_visible_area(
            152.0,
            24.0,
            304.0,
            48.0,
            FloatingRecorderMode::Collapsed,
        ));
        assert!(!cursor_is_inside_recorder_visible_area(
            30.0,
            5.0,
            304.0,
            48.0,
            FloatingRecorderMode::Collapsed,
        ));
        assert!(cursor_is_inside_recorder_visible_area(
            30.0,
            24.0,
            304.0,
            48.0,
            FloatingRecorderMode::Expanded,
        ));
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

        assert_eq!(recorder["width"].as_u64(), Some(304));
        assert_eq!(recorder["height"].as_u64(), Some(48));
        assert_eq!(recorder["visible"].as_bool(), Some(true));
        assert_eq!(language["visible"].as_bool(), Some(false));
    }

    #[test]
    fn native_floating_chrome_hides_language_when_collapsed() {
        assert!(!language_window_visible_for_floating_chrome(false, false));
        assert!(language_window_visible_for_floating_chrome(true, false));
        assert!(!language_window_visible_for_floating_chrome(true, true));

        let source = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs"))
            .expect("lib source");
        let production_source = source
            .split("\n#[cfg(test)]")
            .next()
            .expect("production lib source before tests");

        assert!(production_source
            .contains("apply_floating_chrome_windows(app.handle(), false, false, false)?"));
        assert!(production_source.contains("window.hide()?;"));
    }

    #[test]
    fn language_window_position_uses_intended_recorder_mode_not_stale_outer_size() {
        let source = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs"))
            .expect("lib source");
        let production_source = source
            .split("\n#[cfg(test)]")
            .next()
            .expect("production lib source before tests");
        let apply_windows = production_source
            .split("fn apply_floating_chrome_windows(")
            .nth(1)
            .and_then(|source| source.split("\nfn position_language_window").next())
            .expect("floating chrome window application function");
        let position_language = production_source
            .split("fn position_language_window(")
            .nth(1)
            .and_then(|source| source.split("\n#[cfg(test)]").next())
            .expect("language window positioning function");

        assert!(apply_windows
            .contains("position_language_window(app, language_menu_open, recorder_mode)?"));
        assert!(production_source.contains(
            "fn position_language_window(\n    app: &tauri::AppHandle,\n    open: bool,\n    recorder_mode: FloatingRecorderMode,\n)"
        ));
        assert!(position_language.contains("recorder_window_size_for_mode(recorder_mode)"));
        assert!(!position_language.contains("recorder_window_physical_width"));
        assert!(!position_language.contains("recorder_window_physical_height"));
        assert!(!position_language.contains("outer_size()"));
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
    fn native_hover_collapse_waits_for_grace_before_clearing_reason() {
        assert_eq!(HOVER_COLLAPSE_GRACE_MS, 200);

        let source = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs"))
            .expect("lib source");
        let production_source = source
            .split("\n#[cfg(test)]")
            .next()
            .expect("production lib source before tests");
        let hover_update = production_source
            .split("fn set_floating_chrome_reason_active(")
            .nth(1)
            .and_then(|source| {
                source
                    .split("\nfn language_window_visible_for_floating_chrome")
                    .next()
            })
            .expect("floating chrome hover update source");

        assert!(production_source.contains("const HOVER_COLLAPSE_GRACE_MS: u64 = 200;"));
        assert!(hover_update.contains("set_floating_chrome_hover_reason_inactive_after_grace"));
        assert!(hover_update.contains("std::thread::sleep"));
        assert!(hover_update.contains("Duration::from_millis(HOVER_COLLAPSE_GRACE_MS)"));
        assert!(hover_update.contains("set_floating_chrome_reason_active_immediate"));
        assert!(hover_update.contains("state.inner(), reason, false"));
        assert!(hover_update.contains("hover_clear_generation_matches"));
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
        let collapsed_recorder_surface_styles = styles
            .split("html[data-surface=\"recorder\"] .recorder-surface.is-floating-collapsed {")
            .nth(1)
            .and_then(|styles| styles.split('}').next())
            .expect("collapsed recorder surface styles exist");
        let expanded_recorder_surface_styles = styles
            .split("html[data-surface=\"recorder\"] .recorder-surface.is-floating-expanded {")
            .nth(1)
            .and_then(|styles| styles.split('}').next())
            .expect("expanded recorder surface styles exist");

        assert!(recorder_surface_styles.contains("padding: 0;"));
        assert!(collapsed_recorder_surface_styles.contains("align-content: end;"));
        assert!(expanded_recorder_surface_styles.contains("align-content: center;"));
        assert!(collapsed_recorder_styles.contains("width: 96px;"));
        assert!(collapsed_recorder_styles.contains("height: 10px;"));
        assert!(collapsed_recorder_styles.contains("align-self: end;"));
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
    fn recorder_waveform_styles_are_standalone_and_reduced_motion_safe() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let styles =
            fs::read_to_string(manifest_dir.join("../src/styles.css")).expect("frontend styles");

        let waveform_surface_styles = styles
            .split(".recording-waveform-surface {")
            .nth(1)
            .and_then(|styles| styles.split('}').next())
            .expect("recording waveform surface styles exist");
        let waveform_bar_styles = styles
            .split(".recording-waveform-bar {")
            .nth(1)
            .and_then(|styles| styles.split('}').next())
            .expect("recording waveform bar styles exist");
        let reduced_motion_styles = styles
            .split("@media (prefers-reduced-motion: reduce)")
            .nth(1)
            .expect("reduced motion media query exists");

        assert!(waveform_surface_styles.contains("display: inline-grid;"));
        assert!(waveform_surface_styles.contains("place-items: center;"));
        assert!(waveform_bar_styles.contains("animation: recording-waveform-pulse"));
        assert!(reduced_motion_styles.contains(".recording-waveform-bar"));
        assert!(reduced_motion_styles.contains("animation: none;"));
    }

    #[test]
    fn recorder_styles_avoid_size_transition_jank_and_keep_rounded_clip() {
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
        let transition = floating_recorder_styles
            .split("transition:")
            .nth(1)
            .and_then(|styles| styles.split(';').next())
            .expect("floating recorder transition exists");

        assert!(!transition.contains("width"));
        assert!(!transition.contains("height"));
        assert!(transition.contains("opacity"));
        assert!(transition.contains("transform"));
        assert!(recorder_surface_styles.contains("overflow: hidden;"));
        assert!(recorder_surface_styles.contains("border-radius: 999px;"));
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
        let build_script = fs::read_to_string(root_dir.join("scripts/desktop-build.sh"))
            .expect("desktop build wrapper");
        let sign_script = fs::read_to_string(root_dir.join("scripts/sign-macos-app.sh"))
            .expect("stable macOS signing script");
        let ensure_script =
            fs::read_to_string(root_dir.join("scripts/ensure-local-codesign-cert.sh"))
                .expect("local macOS code-signing identity script");
        let trust_script =
            fs::read_to_string(root_dir.join("scripts/trust-local-codesign-cert.sh"))
                .expect("local macOS code-signing trust script");

        assert!(package.contains("scripts/desktop-build.sh"));
        assert!(package.contains("scripts/trust-local-codesign-cert.sh"));
        assert!(build_script.contains("scripts/ensure-local-codesign-cert.sh"));
        assert!(build_script.contains("scripts/sign-macos-app.sh"));
        assert!(sign_script.contains("--requirements"));
        assert!(sign_script.contains("IDENTIFIER=\"com.ribbonsdigital.wispergo\""));
        assert!(sign_script.contains("Wispergo Local Code Signing"));
        assert!(sign_script.contains("designated => identifier"));
        assert!(ensure_script.contains("extendedKeyUsage=codeSigning"));
        assert!(trust_script.contains("security add-trusted-cert"));
        assert!(trust_script.contains("/Library/Keychains/System.keychain"));
    }

    #[test]
    fn macos_bundle_resources_include_only_asset_manifest() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let config =
            fs::read_to_string(manifest_dir.join("tauri.conf.json")).expect("tauri config");
        let config: Value = serde_json::from_str(&config).expect("valid tauri config json");
        let resources = config["bundle"]["resources"]
            .as_object()
            .expect("bundle resources configured as object");

        assert_eq!(resources.len(), 1);
        assert_eq!(
            resources
                .get("resources/models.manifest.json")
                .and_then(Value::as_str),
            Some("resources/models.manifest.json")
        );
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
