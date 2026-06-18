//! In-process whisper.cpp ASR provider (Phase 2).
//!
//! This module is only compiled when the `whisper-rs` cargo feature is enabled,
//! which builds whisper.cpp via `whisper-rs-sys` (requires cmake + clang). It
//! is gated off by default during the bridge state where the bundled
//! `whisper-cli` sidecar remains the ASR provider.
//!
//! Slice 2.1 ships only the dependency integration + build verification. The
//! `WhisperRsProvider` implementing `AsrProvider` lands in slice 2.2, and the
//! pipeline switch + sidecar retirement land in 2.3.

#[cfg(feature = "whisper-rs")]
use whisper_rs::get_whisper_version;

/// Return the linked whisper.cpp version, for build-integration smoke checks.
///
/// Available only when the `whisper-rs` feature is enabled. Used by the 2.1
/// build-verification test to prove the dependency links and Metal builds on
/// arm64.
#[cfg(feature = "whisper-rs")]
pub fn linked_whisper_version() -> &'static str {
    get_whisper_version()
}

#[cfg(test)]
#[cfg(feature = "whisper-rs")]
mod tests {
    use super::*;

    /// Build-integration smoke test: proves whisper-rs links, Metal builds on
    /// arm64, and the version symbol is reachable. This is the slice 2.1 DoD.
    #[test]
    fn whisper_rs_links_and_reports_version() {
        let version = linked_whisper_version();
        assert!(!version.is_empty(), "whisper.cpp version string must be non-empty: {version:?}");
    }
}
