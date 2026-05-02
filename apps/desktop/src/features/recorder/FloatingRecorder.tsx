type RecordingStatus = "idle" | "recording";

type Props = {
  status: RecordingStatus;
  busy?: boolean;
  expanded?: boolean;
};

export function FloatingRecorder({ status, busy = false, expanded = true }: Props) {
  const isRecording = status === "recording";
  const className = ["floating-recorder", expanded ? "is-expanded" : "is-collapsed"].join(" ");

  if (!expanded) {
    return (
      <section className={className} aria-label="Recorder">
        <div className="recorder-idle-handle" aria-label="Wispergo idle handle" />
      </section>
    );
  }

  return (
    <section className={className} aria-label="Recorder">
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
