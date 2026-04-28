use wispergo_core::audio::{trim_silence, VadConfig};

#[test]
fn trims_leading_and_trailing_silence() {
    let input = vec![0.0, 0.001, 0.08, 0.12, 0.002, 0.0];
    let output = trim_silence(&input, VadConfig::default());

    assert_eq!(output, vec![0.08, 0.12]);
}

#[test]
fn returns_empty_audio_when_every_sample_is_below_threshold() {
    let input = vec![0.0, 0.001, 0.002];
    let output = trim_silence(&input, VadConfig::default());

    assert!(output.is_empty());
}

#[test]
fn custom_threshold_changes_trim_boundary() {
    let input = vec![0.03, 0.06, 0.02];
    let output = trim_silence(
        &input,
        VadConfig {
            silence_threshold: 0.05,
        },
    );

    assert_eq!(output, vec![0.06]);
}
