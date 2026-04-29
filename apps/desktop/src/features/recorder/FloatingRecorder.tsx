type RecordingStatus = "idle" | "recording";

type Props = {
  status: RecordingStatus;
  busy?: boolean;
};

export function FloatingRecorder({ status, busy = false }: Props) {
  const isRecording = status === "recording";

  return (
    <section className="floating-recorder" aria-label="Recorder">
      <div className="recording-dot" aria-hidden="true" />
      <div className="recording-copy">
        <div className="recording-status">
          {busy && !isRecording ? "Processing" : isRecording ? "Recording" : "Ready"}
        </div>
        <div className="recording-hint">
          {isRecording ? "release to insert" : "hold Command + Shift + Space"}
        </div>
      </div>
    </section>
  );
}
