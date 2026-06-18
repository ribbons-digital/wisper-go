//! Manifest-driven asset path resolution over a models root directory.
//!
//! Pure path logic — no filesystem access, no network, no Tauri. The desktop
//! layer supplies the base models directory (resolved from the app handle's
//! app-config dir); this module turns an asset id (looked up in an
//! [`AssetManifest`]) into a concrete file path under that root.
//!
//! Layout, matching the design doc:
//!
//! ```text
//! {models_root}/
//!   asr/
//!     {id}.bin
//!   cleanup/
//!     {id}.gguf
//! ```
//!
//! Both cleanup roles (`cleanup_punctuation`, `cleanup_full`) share the
//! `cleanup/` directory; their ids disambiguate the files.

use std::path::{Path, PathBuf};

use crate::asset_manifest::{AssetManifest, AssetRole};
use thiserror::Error;

/// Subdirectory under the models root for ASR assets.
pub const ASR_SUBDIR: &str = "asr";
/// Subdirectory under the models root for cleanup assets (both roles).
pub const CLEANUP_SUBDIR: &str = "cleanup";

/// File extension for ASR assets (whisper.cpp ggml format).
pub const ASR_EXTENSION: &str = ".bin";
/// File extension for cleanup assets (llama.cpp GGUF format).
pub const CLEANUP_EXTENSION: &str = ".gguf";

/// Suffix appended to an in-progress download.
pub const PART_SUFFIX: &str = ".part";

/// Resolves asset file paths under a fixed models root directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetStorage {
    models_root: PathBuf,
}

/// Errors raised when resolving an asset path from a manifest.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum AssetPathError {
    /// No asset in the manifest carried the requested id.
    #[error("asset not found in manifest: {0}")]
    AssetNotFound(String),
}

impl AssetStorage {
    /// Create a resolver rooted at `models_root`.
    pub fn new<P: Into<PathBuf>>(models_root: P) -> Self {
        Self {
            models_root: models_root.into(),
        }
    }

    /// The configured models root directory.
    pub fn models_root(&self) -> &Path {
        &self.models_root
    }

    /// The directory holding assets of `role`.
    pub fn role_dir(&self, role: AssetRole) -> PathBuf {
        self.models_root.join(role_subdir(role))
    }

    /// The concrete file path for asset `id` of `role`.
    pub fn asset_path(&self, id: &str, role: AssetRole) -> PathBuf {
        self.role_dir(role)
            .join(format!("{id}{}", role_extension(role)))
    }

    /// The `.part` temp path used while `id` of `role` is downloading.
    pub fn part_path(&self, id: &str, role: AssetRole) -> PathBuf {
        let mut path = self.asset_path(id, role);
        let mut file_name = path
            .file_name()
            .expect("asset path always has a file name")
            .to_os_string();
        file_name.push(PART_SUFFIX);
        path.set_file_name(file_name);
        path
    }

    /// Look up `id` in `manifest` and resolve its concrete file path.
    pub fn path_for(&self, manifest: &AssetManifest, id: &str) -> Result<PathBuf, AssetPathError> {
        let asset = manifest
            .find(id)
            .ok_or_else(|| AssetPathError::AssetNotFound(id.to_string()))?;
        Ok(self.asset_path(&asset.id, asset.role))
    }
}

/// Subdirectory name for a role.
pub fn role_subdir(role: AssetRole) -> &'static str {
    match role {
        AssetRole::Asr => ASR_SUBDIR,
        AssetRole::CleanupPunctuation | AssetRole::CleanupFull => CLEANUP_SUBDIR,
    }
}

/// File extension (with leading dot) for a role.
pub fn role_extension(role: AssetRole) -> &'static str {
    match role {
        AssetRole::Asr => ASR_EXTENSION,
        AssetRole::CleanupPunctuation | AssetRole::CleanupFull => CLEANUP_EXTENSION,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asset_manifest::AssetEntry;

    fn manifest_with(assets: &[(&str, AssetRole)]) -> AssetManifest {
        let mut sha = 0u8;
        AssetManifest {
            schema_version: 1,
            assets: assets
                .iter()
                .map(|(id, role)| {
                    sha = sha.wrapping_add(1);
                    AssetEntry {
                        id: id.to_string(),
                        role: *role,
                        display_name: (*id).to_string(),
                        url: format!("https://example.org/{id}"),
                        size: 100,
                        sha256: std::iter::repeat_n(hex_char(sha), 64).collect(),
                    }
                })
                .collect(),
        }
    }

    fn hex_char(b: u8) -> char {
        let n = b % 16;
        if n < 10 {
            (b'0' + n) as char
        } else {
            (b'a' + (n - 10)) as char
        }
    }

    #[test]
    fn role_subdir_and_extension_mapping() {
        assert_eq!(role_subdir(AssetRole::Asr), "asr");
        assert_eq!(role_extension(AssetRole::Asr), ".bin");

        assert_eq!(role_subdir(AssetRole::CleanupPunctuation), "cleanup");
        assert_eq!(role_extension(AssetRole::CleanupPunctuation), ".gguf");

        assert_eq!(role_subdir(AssetRole::CleanupFull), "cleanup");
        assert_eq!(role_extension(AssetRole::CleanupFull), ".gguf");
    }

    #[test]
    fn asr_path_uses_asr_subdir_and_bin_extension() {
        let storage = AssetStorage::new("/var/lib/wispergo/models");
        assert_eq!(
            storage.asset_path("medium", AssetRole::Asr),
            PathBuf::from("/var/lib/wispergo/models/asr/medium.bin")
        );
    }

    #[test]
    fn cleanup_punctuation_and_full_share_cleanup_subdir() {
        let storage = AssetStorage::new("/models");
        assert_eq!(
            storage.asset_path("qwen2.5-0.5b", AssetRole::CleanupPunctuation),
            PathBuf::from("/models/cleanup/qwen2.5-0.5b.gguf")
        );
        assert_eq!(
            storage.asset_path("qwen2.5-3b", AssetRole::CleanupFull),
            PathBuf::from("/models/cleanup/qwen2.5-3b.gguf")
        );
    }

    #[test]
    fn part_path_appends_part_suffix_to_asset_path() {
        let storage = AssetStorage::new("/models");
        assert_eq!(
            storage.part_path("medium", AssetRole::Asr),
            PathBuf::from("/models/asr/medium.bin.part")
        );
        assert_eq!(
            storage.part_path("qwen2.5-3b", AssetRole::CleanupFull),
            PathBuf::from("/models/cleanup/qwen2.5-3b.gguf.part")
        );
    }

    #[test]
    fn role_dir_resolves_role_subdir_under_root() {
        let storage = AssetStorage::new("/models");
        assert_eq!(storage.role_dir(AssetRole::Asr), PathBuf::from("/models/asr"));
        assert_eq!(
            storage.role_dir(AssetRole::CleanupFull),
            PathBuf::from("/models/cleanup")
        );
    }

    #[test]
    fn models_root_accessor_returns_configured_root() {
        let storage = AssetStorage::new("/models");
        assert_eq!(storage.models_root(), Path::new("/models"));
    }

    #[test]
    fn path_for_resolves_via_manifest_lookup() {
        let storage = AssetStorage::new("/models");
        let manifest = manifest_with(&[
            ("medium", AssetRole::Asr),
            ("qwen2.5-0.5b", AssetRole::CleanupPunctuation),
        ]);

        assert_eq!(
            storage.path_for(&manifest, "medium").unwrap(),
            PathBuf::from("/models/asr/medium.bin")
        );
        assert_eq!(
            storage.path_for(&manifest, "qwen2.5-0.5b").unwrap(),
            PathBuf::from("/models/cleanup/qwen2.5-0.5b.gguf")
        );
    }

    #[test]
    fn path_for_errors_on_unknown_id() {
        let storage = AssetStorage::new("/models");
        let manifest = manifest_with(&[("medium", AssetRole::Asr)]);

        let err = storage
            .path_for(&manifest, "missing")
            .expect_err("unknown id");
        assert!(matches!(err, AssetPathError::AssetNotFound(id) if id == "missing"));
    }

    #[test]
    fn asset_path_handles_ids_with_dots_and_dashes() {
        let storage = AssetStorage::new("/models");
        // The id is the full stem; only the role-derived extension is appended.
        assert_eq!(
            storage.asset_path("large-v3-turbo", AssetRole::Asr),
            PathBuf::from("/models/asr/large-v3-turbo.bin")
        );
        assert_eq!(
            storage.asset_path("qwen2.5-3b-instruct", AssetRole::CleanupFull),
            PathBuf::from("/models/cleanup/qwen2.5-3b-instruct.gguf")
        );
    }
}
