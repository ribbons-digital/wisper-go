import type { RecordingMode } from "../../types/pipeline";

type RecordingStatus = "idle" | "recording";

type Props = {
  status: RecordingStatus;
  mode: RecordingMode;
  onStart: (mode: RecordingMode) => void;
  onStop: (reason: string) => void;
  onCancel: (reason: string) => void;
};

export function FloatingRecorder({
  status,
  mode,
  onStart,
  onStop,
  onCancel,
}: Props) {
  const isRecording = status === "recording";

  return (
    <section className="floating-recorder" aria-label="Recorder">
      <div className="recording-status">{isRecording ? "Recording" : "Ready"}</div>
      <button
        type="button"
        className="record-button"
        aria-label={isRecording ? "Stop recording" : "Start recording"}
        onClick={() => {
          if (isRecording) {
            onStop("floating_button");
          } else {
            onStart(mode);
          }
        }}
      >
        {isRecording ? "Stop" : "Record"}
      </button>
      <button type="button" onClick={() => onCancel("user_cancelled")}>
        Cancel
      </button>
    </section>
  );
}
