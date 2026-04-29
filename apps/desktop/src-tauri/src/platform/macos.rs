#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessibilityStatus {
    pub granted: bool,
    pub can_prompt: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MicrophoneStatus {
    pub granted: bool,
    pub can_prompt: bool,
}

#[cfg(target_os = "macos")]
pub fn accessibility_status() -> AccessibilityStatus {
    let ax_trusted = macos_accessibility_client::accessibility::application_is_trusted();
    let focused_element_accessible = if ax_trusted {
        false
    } else {
        focused_element_accessibility_available()
    };
    let input_simulation_allowed = if ax_trusted || focused_element_accessible {
        false
    } else {
        input_simulation_permission(false)
    };

    accessibility_status_from_checks(
        ax_trusted,
        input_simulation_allowed,
        focused_element_accessible,
    )
}

#[cfg(not(target_os = "macos"))]
pub fn accessibility_status() -> AccessibilityStatus {
    AccessibilityStatus {
        granted: true,
        can_prompt: false,
    }
}

#[cfg(target_os = "macos")]
pub fn request_accessibility() -> AccessibilityStatus {
    let ax_trusted =
        macos_accessibility_client::accessibility::application_is_trusted_with_prompt();
    let focused_element_accessible = if ax_trusted {
        false
    } else {
        focused_element_accessibility_available()
    };
    let input_simulation_allowed = if ax_trusted || focused_element_accessible {
        false
    } else {
        input_simulation_permission(true)
    };

    accessibility_status_from_checks(
        ax_trusted,
        input_simulation_allowed,
        focused_element_accessible,
    )
}

#[cfg(not(target_os = "macos"))]
pub fn request_accessibility() -> AccessibilityStatus {
    accessibility_status()
}

#[cfg(target_os = "macos")]
pub fn microphone_status() -> MicrophoneStatus {
    microphone_status_from_authorization_status(av_capture_audio_authorization_status())
}

#[cfg(target_os = "macos")]
pub fn request_microphone_access() -> MicrophoneStatus {
    let status = microphone_status();
    if !status.can_prompt {
        return status;
    }

    av_capture_request_audio_access()
        .map(microphone_status_from_request_decision)
        .unwrap_or_else(microphone_status)
}

#[cfg(not(target_os = "macos"))]
pub fn microphone_status() -> MicrophoneStatus {
    MicrophoneStatus {
        granted: true,
        can_prompt: false,
    }
}

#[cfg(not(target_os = "macos"))]
pub fn request_microphone_access() -> MicrophoneStatus {
    microphone_status()
}

fn accessibility_status_from_checks(
    ax_trusted: bool,
    input_simulation_allowed: bool,
    focused_element_accessible: bool,
) -> AccessibilityStatus {
    AccessibilityStatus {
        granted: ax_trusted || input_simulation_allowed || focused_element_accessible,
        can_prompt: true,
    }
}

fn microphone_status_from_authorization_status(status: isize) -> MicrophoneStatus {
    const NOT_DETERMINED: isize = 0;
    const AUTHORIZED: isize = 3;

    MicrophoneStatus {
        granted: status == AUTHORIZED,
        can_prompt: status == NOT_DETERMINED,
    }
}

fn microphone_status_from_request_decision(granted: bool) -> MicrophoneStatus {
    MicrophoneStatus {
        granted,
        can_prompt: false,
    }
}

#[cfg(target_os = "macos")]
fn input_simulation_permission(open_prompt: bool) -> bool {
    let settings = enigo::Settings {
        open_prompt_to_get_permissions: open_prompt,
        ..enigo::Settings::default()
    };
    enigo::Enigo::new(&settings).is_ok()
}

#[cfg(target_os = "macos")]
fn focused_element_accessibility_available() -> bool {
    use std::ffi::c_void;
    use std::ptr;

    use core_foundation::base::TCFType;
    use core_foundation::string::{CFString, CFStringRef};
    use core_foundation_sys::base::{CFRelease, CFTypeRef};

    type AXError = i32;
    type AXUIElementRef = *const c_void;
    const AX_ERROR_SUCCESS: AXError = 0;
    const AX_ERROR_API_DISABLED: AXError = -25211;

    #[link(name = "ApplicationServices", kind = "framework")]
    unsafe extern "C" {
        fn AXUIElementCreateSystemWide() -> AXUIElementRef;
        fn AXUIElementCopyAttributeValue(
            element: AXUIElementRef,
            attribute: CFStringRef,
            value: *mut CFTypeRef,
        ) -> AXError;
    }

    let system = unsafe { AXUIElementCreateSystemWide() };
    if system.is_null() {
        return false;
    }

    let attribute = CFString::new("AXFocusedUIElement");
    let mut value: CFTypeRef = ptr::null();
    let error = unsafe {
        AXUIElementCopyAttributeValue(system, attribute.as_concrete_TypeRef(), &mut value)
    };
    unsafe { CFRelease(system.cast()) };

    if !value.is_null() {
        unsafe { CFRelease(value) };
    }

    error == AX_ERROR_SUCCESS || error != AX_ERROR_API_DISABLED
}

#[cfg(target_os = "macos")]
fn av_capture_audio_authorization_status() -> isize {
    use std::ffi::{c_char, c_void};

    #[link(name = "objc")]
    unsafe extern "C" {
        fn objc_getClass(name: *const c_char) -> *mut c_void;
        fn sel_registerName(name: *const c_char) -> *mut c_void;

        #[allow(clashing_extern_declarations)]
        #[link_name = "objc_msgSend"]
        fn objc_msg_send_authorization_status(
            receiver: *mut c_void,
            selector: *mut c_void,
            media_type: *const c_void,
        ) -> isize;
    }

    #[link(name = "AVFoundation", kind = "framework")]
    unsafe extern "C" {
        static AVMediaTypeAudio: *const c_void;
    }

    let class_name = b"AVCaptureDevice\0";
    let selector_name = b"authorizationStatusForMediaType:\0";

    unsafe {
        let class = objc_getClass(class_name.as_ptr().cast());
        let selector = sel_registerName(selector_name.as_ptr().cast());
        if class.is_null() || selector.is_null() || AVMediaTypeAudio.is_null() {
            return -1;
        }
        objc_msg_send_authorization_status(class, selector, AVMediaTypeAudio)
    }
}

#[cfg(target_os = "macos")]
fn av_capture_request_audio_access() -> Option<bool> {
    use std::ffi::{c_char, c_void};
    use std::sync::mpsc;
    use std::time::Duration;

    use block2::RcBlock;
    use objc2::runtime::Bool;

    #[link(name = "objc")]
    unsafe extern "C" {
        fn objc_getClass(name: *const c_char) -> *mut c_void;
        fn sel_registerName(name: *const c_char) -> *mut c_void;

        #[allow(clashing_extern_declarations)]
        #[link_name = "objc_msgSend"]
        fn objc_msg_send_request_access(
            receiver: *mut c_void,
            selector: *mut c_void,
            media_type: *const c_void,
            completion: *mut c_void,
        );
    }

    #[link(name = "AVFoundation", kind = "framework")]
    unsafe extern "C" {
        static AVMediaTypeAudio: *const c_void;
    }

    let class_name = b"AVCaptureDevice\0";
    let selector_name = b"requestAccessForMediaType:completionHandler:\0";
    let (tx, rx) = mpsc::channel();
    let completion = RcBlock::new(move |granted: Bool| {
        let _ = tx.send(granted.as_bool());
    });

    unsafe {
        let class = objc_getClass(class_name.as_ptr().cast());
        let selector = sel_registerName(selector_name.as_ptr().cast());
        if class.is_null() || selector.is_null() || AVMediaTypeAudio.is_null() {
            return None;
        }
        objc_msg_send_request_access(
            class,
            selector,
            AVMediaTypeAudio,
            RcBlock::as_ptr(&completion).cast(),
        );
    }

    rx.recv_timeout(Duration::from_secs(60)).ok()
}

#[cfg(test)]
mod tests {
    use super::{
        accessibility_status_from_checks, microphone_status_from_authorization_status,
        microphone_status_from_request_decision, AccessibilityStatus, MicrophoneStatus,
    };

    #[test]
    fn accessibility_status_accepts_trust_input_simulation_or_focused_element_capability() {
        assert_eq!(
            accessibility_status_from_checks(false, true, false),
            AccessibilityStatus {
                granted: true,
                can_prompt: true
            }
        );
        assert_eq!(
            accessibility_status_from_checks(true, false, false),
            AccessibilityStatus {
                granted: true,
                can_prompt: true
            }
        );
        assert_eq!(
            accessibility_status_from_checks(false, false, true),
            AccessibilityStatus {
                granted: true,
                can_prompt: true
            }
        );
        assert_eq!(
            accessibility_status_from_checks(false, false, false),
            AccessibilityStatus {
                granted: false,
                can_prompt: true
            }
        );
    }

    #[test]
    fn microphone_status_is_granted_only_for_authorized_av_status() {
        assert_eq!(
            microphone_status_from_authorization_status(3),
            MicrophoneStatus {
                granted: true,
                can_prompt: false
            }
        );
        assert_eq!(
            microphone_status_from_authorization_status(2),
            MicrophoneStatus {
                granted: false,
                can_prompt: false
            }
        );
    }

    #[test]
    fn microphone_status_can_prompt_only_when_not_determined() {
        assert_eq!(
            microphone_status_from_authorization_status(0),
            MicrophoneStatus {
                granted: false,
                can_prompt: true
            }
        );
    }

    #[test]
    fn microphone_status_after_request_is_not_promptable_anymore() {
        assert_eq!(
            microphone_status_from_request_decision(true),
            MicrophoneStatus {
                granted: true,
                can_prompt: false
            }
        );
        assert_eq!(
            microphone_status_from_request_decision(false),
            MicrophoneStatus {
                granted: false,
                can_prompt: false
            }
        );
    }
}
