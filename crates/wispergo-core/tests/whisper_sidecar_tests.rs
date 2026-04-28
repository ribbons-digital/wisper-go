use std::fs;

use tempfile::tempdir;
use wispergo_core::domain::ProviderSource;
use wispergo_core::providers::AsrProvider;
use wispergo_core::whisper_sidecar::{parse_whisper_output, WhisperSidecarProvider};

#[test]
fn parses_plain_whisper_output() {
    let transcript = parse_whisper_output(" hello world \n").expect("parse");
    assert_eq!(transcript, "hello world");
}

#[tokio::test]
async fn sidecar_provider_invokes_configured_binary() {
    let dir = tempdir().expect("tempdir");
    let script = dir.path().join("fake-whisper.sh");
    let marker = dir.path().join("args.txt");
    fs::write(
        &script,
        format!(
            "#!/bin/sh\n\
             while [ \"$#\" -gt 0 ]; do\n\
             if [ \"$1\" = \"--file\" ]; then\n\
             test -s \"$2\" || exit 7\n\
             printf '%s' \"$2\" > \"{}\"\n\
             fi\n\
             shift\n\
             done\n\
             printf 'sidecar transcript\\n'\n",
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

    let provider = WhisperSidecarProvider::new(script, None);
    let output = provider
        .transcribe(vec![0.1, 0.2])
        .await
        .expect("transcribe");

    assert_eq!(output.transcript, "sidecar transcript");
    assert_eq!(output.source, ProviderSource::Local);
    assert!(
        fs::read_to_string(&marker)
            .expect("read sidecar marker")
            .contains(".wav"),
        "sidecar should receive a temporary wav path"
    );
}
