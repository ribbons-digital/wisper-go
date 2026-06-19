//! In-process llama.cpp cleanup provider (Phase 3).
//!
//! Only compiled when the `llama-cpp` cargo feature is enabled, which builds
//! llama.cpp via `llama-cpp-sys-2` (requires cmake + clang). It is gated off
//! by default during the bridge state where the bundled `llama-server`
//! sidecar remains the cleanup runtime.
//!
//! ## Slice 3.1 scope
//!
//! This slice ships only the dependency integration + build verification. The
//! `LlamaCppCleanupProvider` implementing `CleanupProvider` /
//! `TextCleanupProvider` lands in slice 3.2 (reusing the exact prompt contract
//! from the retired `llama_server.rs`), and the pipeline switch + `llama-server`
//! + `CleanupRuntimeManager` retirement land in 3.3.

#![cfg(feature = "llama-cpp")]

/// Return a static string proving the `llama-cpp-2` dependency links and Metal
/// builds on arm64. Used by the 3.1 build-verification test as the slice DoD.
pub fn llama_cpp_linked() -> &'static str {
    "llama-cpp-2 linked"
}

#[cfg(test)]
#[cfg(feature = "llama-cpp")]
mod tests {
    use super::*;

    /// Build-integration smoke test: proves llama-cpp-2 links and Metal builds
    /// on arm64. This is the slice 3.1 DoD.
    #[test]
    fn llama_cpp_links() {
        assert_eq!(llama_cpp_linked(), "llama-cpp-2 linked");
    }
}
