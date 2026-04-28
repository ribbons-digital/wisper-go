use std::fs;
use std::time::{Duration, Instant};

use tempfile::tempdir;
use wispergo_core::domain::ProviderSource;
use wispergo_core::providers::{
    AsrProvider, ProviderError, ASR_INPUT_CHANNELS, ASR_INPUT_SAMPLE_RATE_HZ,
};
use wispergo_core::whisper_sidecar::{
    parse_whisper_output, WhisperSidecarProvider, WHISPER_SIDECAR_BITS_PER_SAMPLE,
};

#[test]
fn parses_plain_whisper_output() {
    let transcript = parse_whisper_output(" hello world \n").expect("parse");
    assert_eq!(transcript, "hello world");
}

#[test]
fn rejects_empty_whisper_output() {
    assert!(matches!(
        parse_whisper_output(" \n"),
        Err(ProviderError::InvalidOutput { provider, .. }) if provider == "whisper_sidecar"
    ));
}

#[tokio::test]
async fn sidecar_provider_invokes_configured_binary() {
    let dir = tempdir().expect("tempdir");
    let script = dir.path().join("fake-whisper.sh");
    let marker = dir.path().join("args.txt");
    let captured_wav = dir.path().join("captured.wav");
    let model = dir.path().join("model.bin");
    fs::write(&model, "fake model").expect("write model");
    fs::write(
        &script,
        format!(
            "#!/bin/sh\n\
             while [ \"$#\" -gt 0 ]; do\n\
             case \"$1\" in\n\
             --file)\n\
             shift\n\
             test -s \"$1\" || exit 7\n\
             cp \"$1\" \"{}\"\n\
             printf 'file=%s\\n' \"$1\" >> \"{}\"\n\
             ;;\n\
             --model)\n\
             shift\n\
             printf 'model=%s\\n' \"$1\" >> \"{}\"\n\
             ;;\n\
             esac\n\
             shift\n\
             done\n\
             printf 'sidecar transcript\\n'\n",
            captured_wav.display(),
            marker.display(),
            marker.display()
        ),
    )
    .expect("write script");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(&script).expect("metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&script, permissions).expect("chmod");
    }

    let provider = WhisperSidecarProvider::new(script, Some(model.clone()));
    let output = provider
        .transcribe(vec![0.1, 0.2])
        .await
        .expect("transcribe");

    assert_eq!(output.transcript, "sidecar transcript");
    assert_eq!(output.source, ProviderSource::Local);
    let args = fs::read_to_string(&marker).expect("read sidecar marker");
    assert!(
        args.contains("file="),
        "sidecar should receive a file argument"
    );
    assert!(
        args.contains(&format!("model={}", model.display())),
        "sidecar should receive the configured model path"
    );
    assert_valid_16khz_mono_wav(&fs::read(&captured_wav).expect("read captured wav"), 2);
}

#[tokio::test]
async fn sidecar_timeout_returns_timeout_error() {
    let dir = tempdir().expect("tempdir");
    let script = dir.path().join("slow-whisper.sh");
    fs::write(&script, "#!/bin/sh\nsleep 1\nprintf 'too late\\n'\n").expect("write script");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(&script).expect("metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&script, permissions).expect("chmod");
    }

    let provider =
        WhisperSidecarProvider::new(script, None).with_timeout(Duration::from_millis(20));

    let started = Instant::now();
    let result = provider.transcribe(vec![0.1, 0.2]).await;

    assert!(
        started.elapsed() < Duration::from_millis(500),
        "timeout should not wait for the sidecar sleep to finish"
    );
    assert!(matches!(
        result,
        Err(ProviderError::Timeout { provider }) if provider == "whisper_sidecar"
    ));
}

fn assert_valid_16khz_mono_wav(bytes: &[u8], sample_count: u32) {
    assert_eq!(&bytes[0..4], b"RIFF");
    assert_eq!(&bytes[8..12], b"WAVE");
    assert_eq!(&bytes[12..16], b"fmt ");
    assert_eq!(u16_le(bytes, 20), 1, "audio format should be PCM");
    assert_eq!(u16_le(bytes, 22), ASR_INPUT_CHANNELS);
    assert_eq!(u32_le(bytes, 24), ASR_INPUT_SAMPLE_RATE_HZ);
    assert_eq!(u16_le(bytes, 34), WHISPER_SIDECAR_BITS_PER_SAMPLE);
    assert_eq!(&bytes[36..40], b"data");
    assert_eq!(
        u32_le(bytes, 40),
        sample_count * u32::from(WHISPER_SIDECAR_BITS_PER_SAMPLE / 8)
    );
}

fn u16_le(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().expect("u16"))
}

fn u32_le(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("u32"))
}
