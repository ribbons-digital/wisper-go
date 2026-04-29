#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VadConfig {
    pub silence_threshold: f32,
    pub padding_samples: usize,
}

impl Default for VadConfig {
    fn default() -> Self {
        Self {
            silence_threshold: 0.02,
            padding_samples: 0,
        }
    }
}

impl VadConfig {
    pub fn dictation() -> Self {
        Self {
            silence_threshold: 0.01,
            padding_samples: 4_000,
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
        (Some(start), Some(end)) if start <= end => {
            let start = start.saturating_sub(config.padding_samples);
            let end = end
                .saturating_add(config.padding_samples)
                .min(samples.len().saturating_sub(1));
            samples[start..=end].to_vec()
        }
        _ => Vec::new(),
    }
}
