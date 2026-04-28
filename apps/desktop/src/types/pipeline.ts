export type ProviderSource = "local" | "cloud";
export type CommandSource = "rules" | "local_llm" | "cloud_llm";
export type RecordingMode = "press_and_hold" | "toggle" | "floating_button";
export type RewriteStyle = "casual" | "professional" | "shorter" | "longer";

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
