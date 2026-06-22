#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InsertionResult {
    Inserted,
    CopiedOnly,
    NoEditableTarget,
    AccessibilityDenied,
    SecureField,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FocusedTextTarget {
    Editable { direct_insert: bool },
    NoEditableTarget,
    AccessibilityDenied,
    SecureField,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InsertionStepStatus {
    NotAttempted,
    Success,
    Failed { message: String },
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FocusedTargetMetadata {
    pub process_id: Option<i32>,
    pub role: Option<String>,
    pub subrole: Option<String>,
    pub selected_text_settable: Option<bool>,
    pub value_settable: Option<bool>,
    pub text_selection_available: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InsertionDiagnostics {
    pub target_status: FocusedTextTarget,
    pub target: FocusedTargetMetadata,
    pub clipboard: InsertionStepStatus,
    pub paste: InsertionStepStatus,
    pub direct_insert: InsertionStepStatus,
    pub final_result: InsertionResult,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InsertionOutcome {
    pub result: InsertionResult,
    pub diagnostics: InsertionDiagnostics,
}

pub trait Clipboard {
    fn set_text(&self, text: &str) -> Result<(), String>;
}

pub trait PasteSimulator {
    fn paste(&self) -> Result<(), String>;
}

pub trait TextTargetDetector {
    fn focused_text_target(&self) -> FocusedTextTarget;

    fn focused_target_metadata(&self) -> FocusedTargetMetadata {
        FocusedTargetMetadata::default()
    }

    fn insert_selected_text(&self, text: &str) -> Result<(), String>;
}

pub fn insert_via_clipboard<C: Clipboard>(
    clipboard: &C,
    text: &str,
) -> Result<InsertionResult, String> {
    clipboard.set_text(text)?;
    Ok(InsertionResult::CopiedOnly)
}

pub fn insert_via_clipboard_and_paste<C: Clipboard, P: PasteSimulator>(
    clipboard: &C,
    paste: &P,
    text: &str,
) -> Result<InsertionResult, String> {
    clipboard.set_text(text)?;
    Ok(paste_after_clipboard_set(paste))
}

fn paste_after_clipboard_set<P: PasteSimulator>(paste: &P) -> InsertionResult {
    match paste.paste() {
        Ok(()) => InsertionResult::Inserted,
        Err(_) => InsertionResult::CopiedOnly,
    }
}

pub fn insert_with_target_detection<C: Clipboard, P: PasteSimulator, T: TextTargetDetector>(
    clipboard: &C,
    paste: &P,
    target: &T,
    text: &str,
) -> Result<InsertionResult, String> {
    Ok(insert_with_target_detection_detailed(clipboard, paste, target, text)?.result)
}

pub fn insert_with_target_detection_detailed<
    C: Clipboard,
    P: PasteSimulator,
    T: TextTargetDetector,
>(
    clipboard: &C,
    paste: &P,
    target: &T,
    text: &str,
) -> Result<InsertionOutcome, String> {
    let target_status = target.focused_text_target();
    let mut diagnostics = InsertionDiagnostics {
        target_status,
        target: target.focused_target_metadata(),
        clipboard: InsertionStepStatus::NotAttempted,
        paste: InsertionStepStatus::NotAttempted,
        direct_insert: InsertionStepStatus::NotAttempted,
        final_result: InsertionResult::CopiedOnly,
    };

    let result = match target_status {
        FocusedTextTarget::SecureField => InsertionResult::SecureField,
        FocusedTextTarget::AccessibilityDenied => {
            set_clipboard_with_diagnostics(clipboard, text, &mut diagnostics)?;
            match paste.paste() {
                Ok(()) => {
                    diagnostics.paste = InsertionStepStatus::Success;
                    InsertionResult::Inserted
                }
                Err(message) => {
                    diagnostics.paste = InsertionStepStatus::Failed { message };
                    InsertionResult::AccessibilityDenied
                }
            }
        }
        FocusedTextTarget::NoEditableTarget => {
            set_clipboard_with_diagnostics(clipboard, text, &mut diagnostics)?;
            match paste.paste() {
                Ok(()) => {
                    diagnostics.paste = InsertionStepStatus::Success;
                    InsertionResult::Inserted
                }
                Err(message) => {
                    diagnostics.paste = InsertionStepStatus::Failed { message };
                    InsertionResult::NoEditableTarget
                }
            }
        }
        FocusedTextTarget::Editable { direct_insert } => {
            set_clipboard_with_diagnostics(clipboard, text, &mut diagnostics)?;
            match paste.paste() {
                Ok(()) => {
                    diagnostics.paste = InsertionStepStatus::Success;
                    InsertionResult::Inserted
                }
                Err(message) => {
                    diagnostics.paste = InsertionStepStatus::Failed { message };
                    if direct_insert {
                        match target.insert_selected_text(text) {
                            Ok(()) => {
                                diagnostics.direct_insert = InsertionStepStatus::Success;
                                InsertionResult::Inserted
                            }
                            Err(message) => {
                                diagnostics.direct_insert = InsertionStepStatus::Failed { message };
                                InsertionResult::CopiedOnly
                            }
                        }
                    } else {
                        InsertionResult::CopiedOnly
                    }
                }
            }
        }
    };

    diagnostics.final_result = result.clone();
    Ok(InsertionOutcome {
        result,
        diagnostics,
    })
}

fn set_clipboard_with_diagnostics<C: Clipboard>(
    clipboard: &C,
    text: &str,
    diagnostics: &mut InsertionDiagnostics,
) -> Result<(), String> {
    match clipboard.set_text(text) {
        Ok(()) => {
            diagnostics.clipboard = InsertionStepStatus::Success;
            Ok(())
        }
        Err(message) => {
            diagnostics.clipboard = InsertionStepStatus::Failed {
                message: message.clone(),
            };
            Err(message)
        }
    }
}

pub struct SystemClipboard;

impl Clipboard for SystemClipboard {
    fn set_text(&self, text: &str) -> Result<(), String> {
        let mut clipboard = arboard::Clipboard::new().map_err(|err| err.to_string())?;
        clipboard
            .set_text(text.to_string())
            .map_err(|err| err.to_string())
    }
}

pub struct SystemPaste;

impl PasteSimulator for SystemPaste {
    fn paste(&self) -> Result<(), String> {
        use enigo::{
            Direction::{Click, Press, Release},
            Enigo, Keyboard,
        };

        let settings = paste_settings();
        let mut enigo = Enigo::new(&settings).map_err(|err| err.to_string())?;
        let modifier = paste_modifier_key();
        let paste_key = paste_key();
        enigo.key(modifier, Press).map_err(|err| err.to_string())?;
        let paste_result = enigo.key(paste_key, Click).map_err(|err| err.to_string());
        let release_result = enigo.key(modifier, Release).map_err(|err| err.to_string());
        paste_result.and(release_result)
    }
}

fn paste_settings() -> enigo::Settings {
    enigo::Settings {
        open_prompt_to_get_permissions: false,
        ..enigo::Settings::default()
    }
}

#[cfg(target_os = "macos")]
fn paste_modifier_key() -> enigo::Key {
    enigo::Key::Meta
}

#[cfg(not(target_os = "macos"))]
fn paste_modifier_key() -> enigo::Key {
    enigo::Key::Control
}

#[cfg(target_os = "macos")]
fn paste_key() -> enigo::Key {
    enigo::Key::Other(0x09)
}

#[cfg(not(target_os = "macos"))]
fn paste_key() -> enigo::Key {
    enigo::Key::Unicode('v')
}

#[cfg(target_os = "macos")]
pub struct SystemTextTarget {
    focused_element: std::cell::RefCell<Option<macos_ax::AxElement>>,
    target_metadata: std::cell::RefCell<FocusedTargetMetadata>,
}

#[cfg(target_os = "macos")]
impl Default for SystemTextTarget {
    fn default() -> Self {
        Self {
            focused_element: std::cell::RefCell::new(None),
            target_metadata: std::cell::RefCell::new(FocusedTargetMetadata::default()),
        }
    }
}

#[cfg(target_os = "macos")]
impl TextTargetDetector for SystemTextTarget {
    fn focused_text_target(&self) -> FocusedTextTarget {
        let Some(element) = macos_ax::focused_element() else {
            self.focused_element.replace(None);
            self.target_metadata
                .replace(FocusedTargetMetadata::default());
            return macos_ax::last_target_status();
        };

        let inspection = macos_ax::inspect_text_target(&element);
        let status = inspection.status;
        self.target_metadata.replace(inspection.metadata);
        if matches!(
            status,
            FocusedTextTarget::Editable {
                direct_insert: true
            }
        ) {
            self.focused_element.replace(Some(element));
        } else {
            self.focused_element.replace(None);
        }
        status
    }

    fn focused_target_metadata(&self) -> FocusedTargetMetadata {
        self.target_metadata.borrow().clone()
    }

    fn insert_selected_text(&self, text: &str) -> Result<(), String> {
        let focused_element = self.focused_element.borrow();
        let Some(element) = focused_element.as_ref() else {
            return Err("no focused editable text target".to_string());
        };
        macos_ax::insert_selected_text(element, text)
    }
}

#[cfg(not(target_os = "macos"))]
#[derive(Default)]
pub struct SystemTextTarget;

#[cfg(not(target_os = "macos"))]
impl TextTargetDetector for SystemTextTarget {
    fn focused_text_target(&self) -> FocusedTextTarget {
        FocusedTextTarget::Editable {
            direct_insert: false,
        }
    }

    fn insert_selected_text(&self, _text: &str) -> Result<(), String> {
        Err("direct text insertion is only implemented on macOS".to_string())
    }
}

pub fn insert_text(text: &str) -> Result<InsertionResult, String> {
    Ok(insert_text_detailed(text)?.result)
}

pub fn insert_text_detailed(text: &str) -> Result<InsertionOutcome, String> {
    insert_with_target_detection_detailed(
        &SystemClipboard,
        &SystemPaste,
        &SystemTextTarget::default(),
        text,
    )
}

#[cfg(target_os = "macos")]
mod macos_ax {
    use std::cell::Cell;
    use std::ffi::c_void;
    use std::ptr;

    use core_foundation::base::TCFType;
    use core_foundation::string::CFString;
    use core_foundation_sys::base::{Boolean, CFRelease, CFTypeRef};

    use super::{FocusedTargetMetadata, FocusedTextTarget};

    type AXError = i32;
    type AXUIElementRef = *const c_void;

    const AX_ERROR_SUCCESS: AXError = 0;
    const AX_ERROR_API_DISABLED: AXError = -25211;

    thread_local! {
        static LAST_TARGET_STATUS: Cell<FocusedTextTarget> = const { Cell::new(FocusedTextTarget::NoEditableTarget) };
    }

    #[link(name = "ApplicationServices", kind = "framework")]
    unsafe extern "C" {
        fn AXUIElementCreateSystemWide() -> AXUIElementRef;
        fn AXUIElementCopyAttributeValue(
            element: AXUIElementRef,
            attribute: core_foundation::string::CFStringRef,
            value: *mut CFTypeRef,
        ) -> AXError;
        fn AXUIElementSetAttributeValue(
            element: AXUIElementRef,
            attribute: core_foundation::string::CFStringRef,
            value: CFTypeRef,
        ) -> AXError;
        fn AXUIElementIsAttributeSettable(
            element: AXUIElementRef,
            attribute: core_foundation::string::CFStringRef,
            settable: *mut Boolean,
        ) -> AXError;
        fn AXUIElementGetPid(element: AXUIElementRef, pid: *mut i32) -> AXError;
    }

    pub struct AxElement {
        ptr: AXUIElementRef,
    }

    impl Drop for AxElement {
        fn drop(&mut self) {
            if !self.ptr.is_null() {
                unsafe { CFRelease(self.ptr.cast()) };
            }
        }
    }

    pub fn focused_element() -> Option<AxElement> {
        LAST_TARGET_STATUS.with(|status| status.set(FocusedTextTarget::NoEditableTarget));
        let system = unsafe { AXUIElementCreateSystemWide() };
        if system.is_null() {
            LAST_TARGET_STATUS.with(|status| status.set(FocusedTextTarget::AccessibilityDenied));
            return None;
        }
        let system = AxElement { ptr: system };

        let mut value: CFTypeRef = ptr::null();
        let error = copy_attribute(system.ptr, "AXFocusedUIElement", &mut value);
        if error != AX_ERROR_SUCCESS || value.is_null() {
            let target_status = if error == AX_ERROR_API_DISABLED {
                FocusedTextTarget::AccessibilityDenied
            } else {
                FocusedTextTarget::NoEditableTarget
            };
            LAST_TARGET_STATUS.with(|status| status.set(target_status));
            return None;
        }

        Some(AxElement { ptr: value.cast() })
    }

    pub struct TargetInspection {
        pub status: FocusedTextTarget,
        pub metadata: FocusedTargetMetadata,
    }

    pub fn inspect_text_target(element: &AxElement) -> TargetInspection {
        let role = string_attribute(element.ptr, "AXRole");
        let subrole = string_attribute(element.ptr, "AXSubrole");
        let selected_text_settable = is_attribute_settable(element.ptr, "AXSelectedText");
        let value_settable = is_attribute_settable(element.ptr, "AXValue");
        let text_selection_available = attribute_exists(element.ptr, "AXSelectedText")
            || attribute_exists(element.ptr, "AXSelectedTextRange");
        let status = target_status_from_ax_attributes(
            role.as_deref().unwrap_or_default(),
            subrole.as_deref().unwrap_or_default(),
            selected_text_settable,
            value_settable,
            text_selection_available,
        );

        TargetInspection {
            status,
            metadata: FocusedTargetMetadata {
                process_id: process_id(element.ptr),
                role,
                subrole,
                selected_text_settable: Some(selected_text_settable),
                value_settable: Some(value_settable),
                text_selection_available: Some(text_selection_available),
            },
        }
    }

    pub(super) fn target_status_from_ax_attributes(
        role: &str,
        subrole: &str,
        selected_text_settable: bool,
        value_settable: bool,
        text_selection_available: bool,
    ) -> FocusedTextTarget {
        if role == "AXSecureTextField" || subrole == "AXSecureTextField" {
            return FocusedTextTarget::SecureField;
        }

        let editable_role = matches!(
            role,
            "AXTextArea" | "AXTextField" | "AXComboBox" | "AXSearchField"
        );
        let paste_candidate_role = matches!(role, "AXWebArea");

        if selected_text_settable || (editable_role && value_settable) {
            FocusedTextTarget::Editable {
                direct_insert: selected_text_settable,
            }
        } else if editable_role || paste_candidate_role || text_selection_available {
            FocusedTextTarget::Editable {
                direct_insert: false,
            }
        } else {
            FocusedTextTarget::NoEditableTarget
        }
    }

    pub fn insert_selected_text(element: &AxElement, text: &str) -> Result<(), String> {
        let attribute = CFString::new("AXSelectedText");
        let text = CFString::new(text);
        let error = unsafe {
            AXUIElementSetAttributeValue(
                element.ptr,
                attribute.as_concrete_TypeRef(),
                text.as_concrete_TypeRef().cast(),
            )
        };
        if error == AX_ERROR_SUCCESS {
            Ok(())
        } else {
            Err(format!(
                "AXSelectedText insertion failed with AXError {error}"
            ))
        }
    }

    pub fn last_target_status() -> FocusedTextTarget {
        LAST_TARGET_STATUS.with(Cell::get)
    }

    fn string_attribute(element: AXUIElementRef, attribute_name: &str) -> Option<String> {
        let mut value: CFTypeRef = ptr::null();
        if copy_attribute(element, attribute_name, &mut value) != AX_ERROR_SUCCESS
            || value.is_null()
        {
            return None;
        }
        let value = unsafe { CFString::wrap_under_create_rule(value.cast()) };
        Some(value.to_string())
    }

    fn is_attribute_settable(element: AXUIElementRef, attribute_name: &str) -> bool {
        let attribute = CFString::new(attribute_name);
        let mut settable: Boolean = 0;
        let error = unsafe {
            AXUIElementIsAttributeSettable(element, attribute.as_concrete_TypeRef(), &mut settable)
        };
        error == AX_ERROR_SUCCESS && settable != 0
    }

    fn process_id(element: AXUIElementRef) -> Option<i32> {
        let mut pid = 0;
        let error = unsafe { AXUIElementGetPid(element, &mut pid) };
        if error == AX_ERROR_SUCCESS {
            Some(pid)
        } else {
            None
        }
    }

    fn attribute_exists(element: AXUIElementRef, attribute_name: &str) -> bool {
        let mut value: CFTypeRef = ptr::null();
        if copy_attribute(element, attribute_name, &mut value) != AX_ERROR_SUCCESS
            || value.is_null()
        {
            return false;
        }
        unsafe { CFRelease(value) };
        true
    }

    fn copy_attribute(
        element: AXUIElementRef,
        attribute_name: &str,
        value: *mut CFTypeRef,
    ) -> AXError {
        let attribute = CFString::new(attribute_name);
        unsafe { AXUIElementCopyAttributeValue(element, attribute.as_concrete_TypeRef(), value) }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::{
        insert_via_clipboard, insert_via_clipboard_and_paste, insert_with_target_detection,
        insert_with_target_detection_detailed, Clipboard, FocusedTextTarget, InsertionResult,
        InsertionStepStatus, PasteSimulator, TextTargetDetector,
    };

    struct FakeClipboard;

    impl Clipboard for FakeClipboard {
        fn set_text(&self, _text: &str) -> Result<(), String> {
            Ok(())
        }
    }

    struct RecordingClipboard {
        calls: Cell<usize>,
    }

    impl RecordingClipboard {
        fn new() -> Self {
            Self {
                calls: Cell::new(0),
            }
        }
    }

    impl Clipboard for RecordingClipboard {
        fn set_text(&self, _text: &str) -> Result<(), String> {
            self.calls.set(self.calls.get() + 1);
            Ok(())
        }
    }

    struct FakePaste {
        result: Result<(), String>,
    }

    impl PasteSimulator for FakePaste {
        fn paste(&self) -> Result<(), String> {
            self.result.clone()
        }
    }

    struct RecordingPaste {
        calls: Cell<usize>,
        result: Result<(), String>,
    }

    impl RecordingPaste {
        fn new(result: Result<(), String>) -> Self {
            Self {
                calls: Cell::new(0),
                result,
            }
        }
    }

    impl PasteSimulator for RecordingPaste {
        fn paste(&self) -> Result<(), String> {
            self.calls.set(self.calls.get() + 1);
            self.result.clone()
        }
    }

    struct FakeTextTarget {
        focused: FocusedTextTarget,
        direct_calls: Cell<usize>,
        direct_result: Result<(), String>,
    }

    impl FakeTextTarget {
        fn new(focused: FocusedTextTarget) -> Self {
            Self {
                focused,
                direct_calls: Cell::new(0),
                direct_result: Ok(()),
            }
        }

        fn with_direct_result(mut self, result: Result<(), String>) -> Self {
            self.direct_result = result;
            self
        }
    }

    impl TextTargetDetector for FakeTextTarget {
        fn focused_text_target(&self) -> FocusedTextTarget {
            self.focused
        }

        fn insert_selected_text(&self, _text: &str) -> Result<(), String> {
            self.direct_calls.set(self.direct_calls.get() + 1);
            self.direct_result.clone()
        }
    }

    #[test]
    fn failed_native_paste_still_copies_text() {
        let result = insert_via_clipboard(&FakeClipboard, "hello").expect("insert");

        assert_eq!(result, InsertionResult::CopiedOnly);
    }

    #[test]
    fn successful_native_paste_reports_inserted() {
        let result =
            insert_via_clipboard_and_paste(&FakeClipboard, &FakePaste { result: Ok(()) }, "hello")
                .expect("insert");

        assert_eq!(result, InsertionResult::Inserted);
    }

    #[test]
    fn failed_native_paste_reports_copied_only() {
        let result = insert_via_clipboard_and_paste(
            &FakeClipboard,
            &FakePaste {
                result: Err("accessibility denied".to_string()),
            },
            "hello",
        )
        .expect("insert");

        assert_eq!(result, InsertionResult::CopiedOnly);
    }

    #[test]
    fn editable_target_copies_then_prefers_clipboard_paste() {
        let clipboard = RecordingClipboard::new();
        let paste = RecordingPaste::new(Ok(()));
        let target = FakeTextTarget::new(FocusedTextTarget::Editable {
            direct_insert: true,
        });

        let result =
            insert_with_target_detection(&clipboard, &paste, &target, "hello").expect("insert");

        assert_eq!(result, InsertionResult::Inserted);
        assert_eq!(target.direct_calls.get(), 0);
        assert_eq!(clipboard.calls.get(), 1);
        assert_eq!(paste.calls.get(), 1);
    }

    #[test]
    fn editable_target_falls_back_to_direct_accessibility_when_paste_fails() {
        let clipboard = RecordingClipboard::new();
        let paste = RecordingPaste::new(Err("accessibility paste failed".to_string()));
        let target = FakeTextTarget::new(FocusedTextTarget::Editable {
            direct_insert: true,
        });

        let result =
            insert_with_target_detection(&clipboard, &paste, &target, "hello").expect("insert");

        assert_eq!(result, InsertionResult::Inserted);
        assert_eq!(target.direct_calls.get(), 1);
        assert_eq!(clipboard.calls.get(), 1);
        assert_eq!(paste.calls.get(), 1);
    }

    #[test]
    fn diagnostics_record_target_and_step_failures() {
        let clipboard = RecordingClipboard::new();
        let paste = RecordingPaste::new(Err("accessibility paste failed".to_string()));
        let target = FakeTextTarget::new(FocusedTextTarget::Editable {
            direct_insert: true,
        });

        let outcome = insert_with_target_detection_detailed(&clipboard, &paste, &target, "hello")
            .expect("insert");

        assert_eq!(outcome.result, InsertionResult::Inserted);
        assert_eq!(
            outcome.diagnostics.target_status,
            FocusedTextTarget::Editable {
                direct_insert: true,
            }
        );
        assert_eq!(outcome.diagnostics.clipboard, InsertionStepStatus::Success);
        assert_eq!(
            outcome.diagnostics.paste,
            InsertionStepStatus::Failed {
                message: "accessibility paste failed".to_string()
            }
        );
        assert_eq!(
            outcome.diagnostics.direct_insert,
            InsertionStepStatus::Success
        );
    }

    #[test]
    fn non_editable_target_attempts_best_effort_paste() {
        let clipboard = RecordingClipboard::new();
        let paste = RecordingPaste::new(Ok(()));
        let target = FakeTextTarget::new(FocusedTextTarget::NoEditableTarget);

        let outcome = insert_with_target_detection_detailed(&clipboard, &paste, &target, "hello")
            .expect("insert");

        assert_eq!(outcome.result, InsertionResult::Inserted);
        assert_eq!(target.direct_calls.get(), 0);
        assert_eq!(clipboard.calls.get(), 1);
        assert_eq!(paste.calls.get(), 1);
        assert_eq!(outcome.diagnostics.clipboard, InsertionStepStatus::Success);
        assert_eq!(outcome.diagnostics.paste, InsertionStepStatus::Success);
    }

    #[test]
    fn non_editable_target_reports_no_editable_when_best_effort_paste_fails() {
        let clipboard = RecordingClipboard::new();
        let paste = RecordingPaste::new(Err("paste rejected".to_string()));
        let target = FakeTextTarget::new(FocusedTextTarget::NoEditableTarget);

        let outcome = insert_with_target_detection_detailed(&clipboard, &paste, &target, "hello")
            .expect("insert");

        assert_eq!(outcome.result, InsertionResult::NoEditableTarget);
        assert_eq!(clipboard.calls.get(), 1);
        assert_eq!(paste.calls.get(), 1);
        assert_eq!(
            outcome.diagnostics.paste,
            InsertionStepStatus::Failed {
                message: "paste rejected".to_string()
            }
        );
    }

    #[test]
    fn inaccessible_target_still_attempts_clipboard_paste_for_diagnostics_and_best_effort() {
        let clipboard = RecordingClipboard::new();
        let paste = RecordingPaste::new(Ok(()));
        let target = FakeTextTarget::new(FocusedTextTarget::AccessibilityDenied);

        let result =
            insert_with_target_detection(&clipboard, &paste, &target, "hello").expect("insert");

        assert_eq!(result, InsertionResult::Inserted);
        assert_eq!(target.direct_calls.get(), 0);
        assert_eq!(clipboard.calls.get(), 1);
        assert_eq!(paste.calls.get(), 1);
    }

    #[test]
    fn inaccessible_target_reports_accessibility_denied_when_best_effort_paste_fails() {
        let clipboard = RecordingClipboard::new();
        let paste = RecordingPaste::new(Err("no permission".to_string()));
        let target = FakeTextTarget::new(FocusedTextTarget::AccessibilityDenied);

        let outcome = insert_with_target_detection_detailed(&clipboard, &paste, &target, "hello")
            .expect("insert");

        assert_eq!(outcome.result, InsertionResult::AccessibilityDenied);
        assert_eq!(clipboard.calls.get(), 1);
        assert_eq!(paste.calls.get(), 1);
        assert_eq!(
            outcome.diagnostics.paste,
            InsertionStepStatus::Failed {
                message: "no permission".to_string()
            }
        );
    }

    #[test]
    fn secure_text_target_does_not_copy_or_paste() {
        let clipboard = RecordingClipboard::new();
        let paste = RecordingPaste::new(Ok(()));
        let target = FakeTextTarget::new(FocusedTextTarget::SecureField);

        let result =
            insert_with_target_detection(&clipboard, &paste, &target, "secret").expect("insert");

        assert_eq!(result, InsertionResult::SecureField);
        assert_eq!(target.direct_calls.get(), 0);
        assert_eq!(clipboard.calls.get(), 0);
        assert_eq!(paste.calls.get(), 0);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn ax_web_area_is_treated_as_a_clipboard_paste_candidate() {
        assert_eq!(
            super::macos_ax::target_status_from_ax_attributes("AXWebArea", "", false, false, false,),
            FocusedTextTarget::Editable {
                direct_insert: false,
            }
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn ax_text_selection_support_is_treated_as_a_clipboard_paste_candidate() {
        assert_eq!(
            super::macos_ax::target_status_from_ax_attributes("AXGroup", "", false, false, true,),
            FocusedTextTarget::Editable {
                direct_insert: false,
            }
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn system_paste_uses_layout_independent_v_key_on_macos() {
        assert_eq!(super::paste_key(), enigo::Key::Other(0x09));
    }

    #[test]
    fn system_paste_does_not_open_accessibility_prompt_during_dictation() {
        assert!(!super::paste_settings().open_prompt_to_get_permissions);
    }
}
