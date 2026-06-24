export type ProviderSource = "local" | "cloud";
export type CommandSource = "rules" | "local_llm" | "cloud_llm";
export type RecordingMode = "press_and_hold" | "toggle" | "floating_button";
export type RewriteStyle = "casual" | "professional" | "shorter" | "longer";
export type InsertionResult =
  | "inserted"
  | "copied_only"
  | "no_editable_target"
  | "accessibility_denied"
  | "secure_field";

export type AudioInputDevice = {
  id: string;
  name: string;
  isDefault: boolean;
};

export type AccessibilityStatus = {
  granted: boolean;
  canPrompt: boolean;
};

export type MicrophoneStatus = {
  granted: boolean;
  canPrompt: boolean;
};

export type RecognitionLanguage = "auto" | "en" | "zh";
export type CleanupMode = "off" | "punctuation_only" | "full_cleanup";
export type AsrModelId = "medium" | "large-v3-turbo" | string;

export type LocalModelSettings = {
  asrModelId: AsrModelId;
  recognitionLanguage: RecognitionLanguage;
  cleanupMode: CleanupMode;
};

export type ShortcutMode = "combo" | "modifier_hold";

export type ModifierHoldKey =
  | "left_command"
  | "right_command"
  | "left_option"
  | "right_option"
  | "left_control"
  | "right_control"
  | "left_shift"
  | "right_shift";

export type ModifierHoldSettings = {
  key: ModifierHoldKey;
  holdThresholdMs: number;
};

export type ShortcutKey =
  | "space"
  | "enter"
  | "escape"
  | "tab"
  | "backquote"
  | "minus"
  | "equal"
  | "bracketLeft"
  | "bracketRight"
  | "backslash"
  | "semicolon"
  | "quote"
  | "comma"
  | "period"
  | "slash"
  | "arrowUp"
  | "arrowDown"
  | "arrowLeft"
  | "arrowRight"
  | "digit0"
  | "digit1"
  | "digit2"
  | "digit3"
  | "digit4"
  | "digit5"
  | "digit6"
  | "digit7"
  | "digit8"
  | "digit9"
  | "keyA"
  | "keyB"
  | "keyC"
  | "keyD"
  | "keyE"
  | "keyF"
  | "keyG"
  | "keyH"
  | "keyI"
  | "keyJ"
  | "keyK"
  | "keyL"
  | "keyM"
  | "keyN"
  | "keyO"
  | "keyP"
  | "keyQ"
  | "keyR"
  | "keyS"
  | "keyT"
  | "keyU"
  | "keyV"
  | "keyW"
  | "keyX"
  | "keyY"
  | "keyZ";

export type ShortcutModifiers = {
  command: boolean;
  shift: boolean;
  option: boolean;
  control: boolean;
};

export type ShortcutCombo = {
  modifiers: ShortcutModifiers;
  key: ShortcutKey;
};

export type ShortcutSettings = {
  mode: ShortcutMode;
  combo: ShortcutCombo;
  modifierHold: ModifierHoldSettings;
};

export type ShortcutSettingsView = {
  settings: ShortcutSettings;
  displayLabel: string;
};

export type CleanupRuntimeState = "disabled" | "starting" | "ready" | "unavailable" | "failed";

export type CleanupRuntimeStatus = {
  state: CleanupRuntimeState;
  message?: string | null;
};

export type AssetDownloadStatus =
  | { state: "ready" }
  | { state: "missing"; assetId: string; displayName: string }
  | { state: "downloading"; assetId: string; displayName: string }
  | { state: "failed"; message: string };

export type CommandAction =
  | { kind: "new_line" }
  | { kind: "new_paragraph" }
  | { kind: "cancel" }
  | { kind: "literal_dictation"; text: string }
  | { kind: "delete_previous_phrase" }
  | { kind: "replace_selection"; text: string }
  | { kind: "rewrite_selection"; style: RewriteStyle }
  | { kind: "format_selection_bullets" }
  | { kind: "format_selection_numbered" }
  | { kind: "unsupported"; reason: string };

export type PipelineResult =
  | {
      kind: "insert_text";
      text: string;
      source: ProviderSource;
      confidence?: number | null;
    }
  | {
      kind: "command";
      command: CommandAction;
      requires_confirmation: boolean;
      source: CommandSource;
    }
  | { kind: "cancelled"; reason: string }
  | { kind: "error"; recoverable: boolean; message: string };

export type StopRecordingOutput = {
  result: PipelineResult;
  insertion: InsertionResult;
};
