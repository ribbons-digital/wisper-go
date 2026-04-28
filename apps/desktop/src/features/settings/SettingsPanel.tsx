import type { RecordingMode } from "../../types/pipeline";

type Props = {
  mode: RecordingMode;
  fallbackPolicy: string;
  onModeChange: (mode: RecordingMode) => void;
};

export function SettingsPanel({ mode, fallbackPolicy, onModeChange }: Props) {
  return (
    <section className="settings-panel" aria-label="Settings">
      <label>
        Recording mode
        <select
          value={mode}
          onChange={(event) => onModeChange(event.target.value as RecordingMode)}
        >
          <option value="press_and_hold">Press and hold</option>
          <option value="toggle">Toggle</option>
          <option value="floating_button">Floating button</option>
        </select>
      </label>
      <p>Fallback policy: {fallbackPolicy}</p>
    </section>
  );
}
