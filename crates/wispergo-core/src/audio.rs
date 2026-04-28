#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VadConfig {
    pub silence_threshold: f32,
}

impl Default for VadConfig {
    fn default() -> Self {
        Self {
            silence_threshold: 0.02,
        }
    }
}

pub fn trim_silence(samples: &[f32], config: VadConfig) -> Vec<f32> {
    let first = samples
        .iter()
        .position(|sample| sample.abs() >= config.silence_threshold);
    let last = samples
        .iter()
        .rposition(|sample| sample.abs() >= config.silence_threshold);

    match (first, last) {
        (Some(start), Some(end)) if start <= end => samples[start..=end].to_vec(),
        _ => samples.to_vec(),
    }
}
