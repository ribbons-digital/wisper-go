import type { CSSProperties } from "react";

type RecordingStatus = "idle" | "recording";

type Props = {
  status: RecordingStatus;
  busy?: boolean;
  expanded?: boolean;
  setupNeeded?: boolean;
};

export function FloatingRecorder({ status, busy = false, expanded = true, setupNeeded = false }: Props) {
  const isRecording = status === "recording";
  const showWaveform = expanded && isRecording && !busy && !setupNeeded;
  const className = ["floating-recorder", expanded ? "is-expanded" : "is-collapsed"].join(" ");

  if (!expanded) {
    return (
      <section className={className} aria-label="Recorder">
        <div className="recorder-idle-handle" aria-label="Wispergo idle handle" />
      </section>
    );
  }

  if (showWaveform) {
    return (
      <section className="recording-waveform-surface" aria-label="Recorder">
        <RecordingWaveform />
      </section>
    );
  }

  return (
    <section className={className} aria-label="Recorder">
      <div className="recording-dot" aria-hidden="true" />
      <div className="recording-copy">
        <div className="recording-status">{setupNeeded ? "Setup needed" : busy ? "Processing" : "Ready"}</div>
        <div className="recording-hint">{setupNeeded ? "open settings to finish" : "hold Command + Shift + Space"}</div>
      </div>
    </section>
  );
}

function RecordingWaveform() {
  const bars = [14, 24, 18, 32, 22, 38, 16, 28, 20, 34, 18, 26];

  return (
    <div className="recording-waveform" aria-label="Recording waveform">
      {bars.map((height, index) => (
        <span
          className="recording-waveform-bar"
          aria-hidden="true"
          key={`${height}-${index}`}
          style={{ "--waveform-height": `${height}px`, "--waveform-index": index } as CSSProperties}
        />
      ))}
    </div>
  );
}
