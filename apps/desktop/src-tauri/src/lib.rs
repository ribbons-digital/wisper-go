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
    local_model_settings, microphone_status, request_accessibility, request_microphone_access,
    selected_microphone_id, set_local_model_settings, set_microphone_device,
};
use state::AppState;
use tauri::{Emitter, Manager};
use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut, ShortcutState};

#[tauri::command]
fn app_health() -> &'static str {
    "ok"
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
            set_local_model_settings
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
    let x = monitor_position.x + (monitor_size.width as i32 - window_size.width as i32) / 2;
    let y = monitor_position.y + monitor_size.height as i32 - window_size.height as i32 - 88;
    let _ = window.set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(
        x, y,
    )));
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use serde_json::Value;

    use super::{recorder_window_ignores_cursor_events, should_hide_window_on_close};

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
        assert!(index.contains("document.documentElement.dataset.surface"));
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
