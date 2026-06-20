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
  whisperBinaryPath: string;
  whisperModelPath: string;
  asrModelId: AsrModelId;
  recognitionLanguage: RecognitionLanguage;
  cleanupMode: CleanupMode;
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
