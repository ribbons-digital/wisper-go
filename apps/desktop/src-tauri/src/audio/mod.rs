use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioInputDevice {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

pub const ASR_SAMPLE_RATE_HZ: u32 = 16_000;

pub struct AudioInputSession {
    stream: cpal::Stream,
    samples: Arc<Mutex<Vec<f32>>>,
    sample_rate: u32,
    channels: u16,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AudioCaptureStats {
    pub sample_count: usize,
    pub duration_ms: u64,
    pub peak: f32,
    pub rms: f32,
}

impl AudioInputSession {
    pub fn stop(self) -> Vec<f32> {
        drop(self.stream);
        let samples = Arc::try_unwrap(self.samples)
            .map(|samples| samples.into_inner().unwrap_or_default())
            .unwrap_or_else(|samples| {
                samples
                    .lock()
                    .map(|samples| samples.clone())
                    .unwrap_or_default()
            });
        normalize_to_asr_input(&samples, self.sample_rate, self.channels)
    }
}

pub fn capture_stats(samples: &[f32]) -> AudioCaptureStats {
    let sample_count = samples.len();
    let duration_ms = (sample_count as u64 * 1_000) / u64::from(ASR_SAMPLE_RATE_HZ);
    if samples.is_empty() {
        return AudioCaptureStats {
            sample_count,
            duration_ms,
            peak: 0.0,
            rms: 0.0,
        };
    }

    let mut peak = 0.0_f32;
    let mut sum_squares = 0.0_f32;
    for sample in samples {
        let amplitude = sample.abs();
        peak = peak.max(amplitude);
        sum_squares += sample * sample;
    }

    AudioCaptureStats {
        sample_count,
        duration_ms,
        peak,
        rms: (sum_squares / sample_count as f32).sqrt(),
    }
}

pub fn list_input_devices() -> Result<Vec<AudioInputDevice>, String> {
    let host = cpal::default_host();
    let default_name = host
        .default_input_device()
        .and_then(|device| device.description().ok())
        .map(|description| description.name().to_string());
    let mut devices = Vec::new();

    if let Some(default_name) = &default_name {
        devices.push(AudioInputDevice {
            id: "default".to_string(),
            name: format!("System Default ({default_name})"),
            is_default: true,
        });
    }

    let input_devices = host.input_devices().map_err(|err| err.to_string())?;
    for (index, device) in input_devices.enumerate() {
        let name = device
            .description()
            .map(|description| description.name().to_string())
            .unwrap_or_else(|_| format!("Input device {}", index + 1));
        devices.push(AudioInputDevice {
            id: name.clone(),
            is_default: default_name.as_deref() == Some(name.as_str()),
            name,
        });
    }

    Ok(devices)
}

pub fn start_input_session(device_id: Option<&str>) -> Result<AudioInputSession, String> {
    let host = cpal::default_host();
    let device = input_device_for_id(&host, device_id)?;
    let supported_config = device
        .default_input_config()
        .map_err(|err| err.to_string())?;
    let sample_format = supported_config.sample_format();
    let sample_rate = supported_config.sample_rate();
    let channels = supported_config.channels();
    let config = cpal::StreamConfig::from(supported_config);
    let samples = Arc::new(Mutex::new(Vec::new()));

    let stream = match sample_format {
        cpal::SampleFormat::F32 => {
            build_input_stream::<f32, _>(&device, &config, samples.clone(), |sample| sample)?
        }
        cpal::SampleFormat::F64 => {
            build_input_stream::<f64, _>(&device, &config, samples.clone(), |sample| sample as f32)?
        }
        cpal::SampleFormat::I8 => {
            build_input_stream::<i8, _>(&device, &config, samples.clone(), |sample| {
                sample as f32 / i8::MAX as f32
            })?
        }
        cpal::SampleFormat::I16 => {
            build_input_stream::<i16, _>(&device, &config, samples.clone(), |sample| {
                sample as f32 / i16::MAX as f32
            })?
        }
        cpal::SampleFormat::I32 => {
            build_input_stream::<i32, _>(&device, &config, samples.clone(), |sample| {
                sample as f32 / i32::MAX as f32
            })?
        }
        cpal::SampleFormat::U8 => {
            build_input_stream::<u8, _>(&device, &config, samples.clone(), |sample| {
                (sample as f32 - 128.0) / 128.0
            })?
        }
        cpal::SampleFormat::U16 => {
            build_input_stream::<u16, _>(&device, &config, samples.clone(), |sample| {
                (sample as f32 - 32_768.0) / 32_768.0
            })?
        }
        cpal::SampleFormat::U32 => {
            build_input_stream::<u32, _>(&device, &config, samples.clone(), |sample| {
                (sample as f32 - 2_147_483_648.0) / 2_147_483_648.0
            })?
        }
        other => return Err(format!("unsupported microphone sample format: {other:?}")),
    };

    stream.play().map_err(|err| err.to_string())?;

    Ok(AudioInputSession {
        stream,
        samples,
        sample_rate,
        channels,
    })
}

fn input_device_for_id(host: &cpal::Host, device_id: Option<&str>) -> Result<cpal::Device, String> {
    match device_id {
        None | Some("") | Some("default") => host
            .default_input_device()
            .ok_or_else(|| "no default microphone input is available".to_string()),
        Some(device_id) => host
            .input_devices()
            .map_err(|err| err.to_string())?
            .find(|device| {
                device
                    .description()
                    .map(|description| description.name() == device_id)
                    .unwrap_or(false)
            })
            .ok_or_else(|| "selected microphone input is unavailable".to_string()),
    }
}

fn build_input_stream<T, F>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    samples: Arc<Mutex<Vec<f32>>>,
    convert: F,
) -> Result<cpal::Stream, String>
where
    T: cpal::SizedSample + Copy + Send + 'static,
    F: Fn(T) -> f32 + Send + Copy + 'static,
{
    device
        .build_input_stream(
            config,
            move |data: &[T], _| {
                if let Ok(mut samples) = samples.lock() {
                    samples.extend(data.iter().copied().map(convert));
                }
            },
            |_err| {},
            None,
        )
        .map_err(|err| err.to_string())
}

pub fn normalize_to_asr_input(samples: &[f32], sample_rate: u32, channels: u16) -> Vec<f32> {
    if samples.is_empty() || sample_rate == 0 || channels == 0 {
        return Vec::new();
    }

    let channel_count = usize::from(channels);
    let mono = samples
        .chunks_exact(channel_count)
        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
        .collect::<Vec<_>>();

    if sample_rate == ASR_SAMPLE_RATE_HZ {
        return mono;
    }

    let target_len = ((mono.len() as u64 * u64::from(ASR_SAMPLE_RATE_HZ)) / u64::from(sample_rate))
        .max(1) as usize;
    let rate_ratio = sample_rate as f32 / ASR_SAMPLE_RATE_HZ as f32;

    (0..target_len)
        .map(|index| {
            let source_index = index as f32 * rate_ratio;
            let left = source_index.floor() as usize;
            let right = (left + 1).min(mono.len().saturating_sub(1));
            let fraction = source_index - left as f32;
            let left_sample = mono.get(left).copied().unwrap_or(0.0);
            let right_sample = mono.get(right).copied().unwrap_or(left_sample);
            left_sample + (right_sample - left_sample) * fraction
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{capture_stats, normalize_to_asr_input};

    #[test]
    fn downmixes_stereo_to_mono() {
        let mono = normalize_to_asr_input(&[1.0, -1.0, 0.5, 0.25], 16_000, 2);

        assert_eq!(mono, vec![0.0, 0.375]);
    }

    #[test]
    fn resamples_to_sixteen_kilohertz() {
        let mono = normalize_to_asr_input(&[0.0, 0.5, 1.0, 0.5], 32_000, 1);

        assert_eq!(mono, vec![0.0, 1.0]);
    }

    #[test]
    fn capture_stats_describe_recorded_asr_audio() {
        let stats = capture_stats(&[0.0, 0.5, -1.0, 0.5]);

        assert_eq!(stats.sample_count, 4);
        assert_eq!(stats.duration_ms, 0);
        assert_eq!(stats.peak, 1.0);
        assert!((stats.rms - 0.61237246).abs() < f32::EPSILON);
    }

    #[test]
    fn capture_stats_handle_empty_audio() {
        let stats = capture_stats(&[]);

        assert_eq!(stats.sample_count, 0);
        assert_eq!(stats.duration_ms, 0);
        assert_eq!(stats.peak, 0.0);
        assert_eq!(stats.rms, 0.0);
    }
}
