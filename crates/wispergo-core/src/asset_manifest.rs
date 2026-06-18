//! Asset manifest: the machine-readable catalog of downloadable model weights.
//!
//! The manifest is a small JSON file bundled inside the app
//! (`models.manifest.json`). It lists every downloadable model asset — its id,
//! functional role, display name, download URL, byte size, and SHA-256. The
//! downloader (Phase 1) reads the manifest; no asset path or URL is hardcoded
//! in source.
//!
//! This module is pure data: parsing, validation, and lookup. It performs no
//! network access and no filesystem access.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Functional role an asset plays in the inference pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetRole {
    /// Speech recognition model (transcription). Exactly one is active at a
    /// time, selected by setting.
    Asr,
    /// Punctuation-only cleanup model; the default cleanup asset.
    CleanupPunctuation,
    /// Full-cleanup-mode model (structured-JSON intent classification).
    /// Required only when Cleanup Mode = Full cleanup.
    CleanupFull,
}

/// A single downloadable model weight entry in the manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetEntry {
    /// Stable identifier used in settings and downloader commands
    /// (e.g. `"medium"`, `"qwen2.5-0.5b-instruct"`).
    pub id: String,
    /// Functional role; determines where the asset plugs into the pipeline.
    pub role: AssetRole,
    /// Human-readable name surfaced in download/selection UI.
    pub display_name: String,
    /// HuggingFace direct download URL. Range-request supported for resume.
    pub url: String,
    /// Exact byte size of the downloaded file, for progress and resume.
    pub size: u64,
    /// Lowercase hex SHA-256 of the downloaded file, for verification.
    pub sha256: String,
}

/// The complete asset manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetManifest {
    /// Manifest schema version. Increment on breaking format changes.
    pub schema_version: u32,
    /// Every downloadable asset. Order is not significant.
    pub assets: Vec<AssetEntry>,
}

/// Errors raised when parsing or validating a manifest.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ManifestError {
    /// The JSON could not be deserialized into the manifest shape.
    #[error("asset manifest is not valid JSON: {0}")]
    InvalidJson(String),
    /// A required field was missing or empty.
    #[error("asset entry is missing required field: {0}")]
    MissingField(&'static str),
    /// A role value did not match any known [`AssetRole`].
    #[error("unknown asset role: {0}")]
    UnknownRole(String),
    /// An asset had a structurally invalid value (e.g. zero size, bad sha256).
    #[error("asset {id} has invalid {field}: {reason}")]
    InvalidField {
        id: String,
        field: &'static str,
        reason: String,
    },
    /// Two assets shared the same id, which would make lookup ambiguous.
    #[error("duplicate asset id: {0}")]
    DuplicateId(String),
}

impl AssetManifest {
    /// Parse a manifest JSON string into a validated [`AssetManifest`].
    ///
    /// Performs both structural deserialization and semantic validation
    /// (non-empty required fields, positive size, well-formed sha256, unique
    /// ids). Returns the first error encountered.
    pub fn from_json(json: &str) -> Result<Self, ManifestError> {
        let manifest: AssetManifest = serde_json::from_str(json)
            .map_err(|err| ManifestError::InvalidJson(err.to_string()))?;
        manifest.validate()?;
        Ok(manifest)
    }

    /// Validate semantic invariants after deserialization.
    pub fn validate(&self) -> Result<(), ManifestError> {
        let mut seen_ids = std::collections::HashSet::new();
        for asset in &self.assets {
            if asset.id.trim().is_empty() {
                return Err(ManifestError::MissingField("id"));
            }
            if asset.display_name.trim().is_empty() {
                return Err(ManifestError::MissingField("displayName"));
            }
            if asset.url.trim().is_empty() {
                return Err(ManifestError::MissingField("url"));
            }
            if asset.sha256.trim().is_empty() {
                return Err(ManifestError::MissingField("sha256"));
            }
            if !is_valid_sha256_hex(&asset.sha256) {
                return Err(ManifestError::InvalidField {
                    id: asset.id.clone(),
                    field: "sha256",
                    reason: "expected 64 hex characters".to_string(),
                });
            }
            if asset.size == 0 {
                return Err(ManifestError::InvalidField {
                    id: asset.id.clone(),
                    field: "size",
                    reason: "must be greater than zero".to_string(),
                });
            }
            if !seen_ids.insert(asset.id.clone()) {
                return Err(ManifestError::DuplicateId(asset.id.clone()));
            }
        }
        Ok(())
    }

    /// Look up an asset by id.
    pub fn find(&self, id: &str) -> Option<&AssetEntry> {
        self.assets.iter().find(|asset| asset.id == id)
    }

    /// All assets matching a role, in manifest order.
    pub fn by_role(&self, role: AssetRole) -> impl Iterator<Item = &AssetEntry> {
        self.assets.iter().filter(move |asset| asset.role == role)
    }
}

fn is_valid_sha256_hex(value: &str) -> bool {
    // Accept any-case hex; the downloader normalizes before comparing.
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a manifest JSON string with the given asset blocks.
    fn manifest_json(assets: &str) -> String {
        format!(
            r#"{{
            "schemaVersion": 1,
            "assets": [{assets}]
        }}"#
        )
    }

    #[test]
    fn parses_valid_manifest_with_multiple_roles() {
        let json = manifest_json(&format!(
            r#"
            {{
                "id": "medium", "role": "asr",
                "displayName": "Whisper medium",
                "url": "https://example.org/medium.bin",
                "size": 480000000,
                "sha256": "{}"
            }},
            {{
                "id": "qwen2.5-0.5b", "role": "cleanup_punctuation",
                "displayName": "Qwen2.5 0.5B",
                "url": "https://example.org/qwen.gguf",
                "size": 400000000,
                "sha256": "{}"
            }},
            {{
                "id": "qwen2.5-3b", "role": "cleanup_full",
                "displayName": "Qwen2.5 3B",
                "url": "https://example.org/qwen3b.gguf",
                "size": 2000000000,
                "sha256": "{}"
            }}
        "#,
            "b".repeat(64),
            "c".repeat(64),
            "d".repeat(64)
        ));

        let manifest = AssetManifest::from_json(&json).expect("valid manifest");

        assert_eq!(manifest.schema_version, 1);
        assert_eq!(manifest.assets.len(), 3);
        assert_eq!(manifest.assets[0].role, AssetRole::Asr);
        assert_eq!(
            manifest.assets[1].role,
            AssetRole::CleanupPunctuation
        );
        assert_eq!(manifest.assets[2].role, AssetRole::CleanupFull);
    }

    #[test]
    fn parses_empty_assets_array() {
        let json = r#"{"schemaVersion": 1, "assets": []}"#;
        let manifest = AssetManifest::from_json(json).expect("empty manifest is valid");
        assert!(manifest.assets.is_empty());
        assert!(manifest.find("anything").is_none());
    }

    #[test]
    fn rejects_malformed_json() {
        let json = r#"{"schemaVersion": 1, "assets": [ OOPS }"#;
        let err = AssetManifest::from_json(json).expect_err("malformed json");
        assert!(matches!(err, ManifestError::InvalidJson(_)));
    }

    #[test]
    fn rejects_unknown_role_variant() {
        let json = manifest_json(
            r#"{
                "id": "x", "role": "transcription",
                "displayName": "X", "url": "https://e.org/x",
                "size": 10, "sha256": "a"
            }"#,
        );
        let err = AssetManifest::from_json(&json).expect_err("unknown role");
        assert!(matches!(err, ManifestError::InvalidJson(_)));
    }

    #[test]
    fn rejects_missing_id() {
        let json = manifest_json(
            r#"{
                "role": "asr",
                "displayName": "X", "url": "https://e.org/x",
                "size": 10, "sha256": "deadbeef"
            }"#,
        );
        let err = AssetManifest::from_json(&json).expect_err("missing id");
        assert!(matches!(err, ManifestError::InvalidJson(_)));
    }

    #[test]
    fn rejects_empty_string_fields_after_parse() {
        let sha = "f".repeat(64);
        for (field, _value) in [
            ("id", ""),
            ("displayName", ""),
            ("url", ""),
            ("sha256", ""),
        ] {
            let entry = format!(
                r#"{{
                    "id": "medium", "role": "asr",
                    "displayName": "Whisper medium",
                    "url": "https://example.org/m.bin",
                    "size": 100,
                    "sha256": "{sha}"
                }}"#
            );
            // Inject the empty field by replacing the known-good value.
            let replacement = match field {
                "id" => entry.replacen("\"medium\"", "\"\"", 1),
                "displayName" => entry.replacen("Whisper medium", "", 1),
                "url" => entry.replacen("https://example.org/m.bin", "", 1),
                "sha256" => entry.replacen(&sha, "", 1),
                _ => entry,
            };
            let json = manifest_json(&replacement);
            let err = AssetManifest::from_json(&json)
                .expect_err(&format!("field {field} should error"));
            assert!(
                matches!(err, ManifestError::MissingField(_)),
                "field {field}: {err:?}"
            );
        }
    }

    #[test]
    fn rejects_malformed_sha256() {
        let json = manifest_json(&format!(
            r#"{{
                "id": "medium", "role": "asr",
                "displayName": "Whisper medium",
                "url": "https://example.org/m.bin",
                "size": 100,
                "sha256": "not-hex-nope"
            }}"#
        ));
        let err = AssetManifest::from_json(&json).expect_err("bad sha256");
        assert!(matches!(
            err,
            ManifestError::InvalidField { id, field: "sha256", .. } if id == "medium"
        ));
    }

    #[test]
    fn accepts_uppercase_sha256_hex() {
        // SHA-256 is conventionally lowercase, but the validator is case-
        // insensitive so a contributor-pasted uppercase hash is not rejected.
        // The downloader normalizes before comparing.
        let json = manifest_json(&format!(
            r#"{{
                "id": "medium", "role": "asr",
                "displayName": "Whisper medium",
                "url": "https://example.org/m.bin",
                "size": 100,
                "sha256": "{}"
            }}"#,
            "A".repeat(64)
        ));
        let manifest = AssetManifest::from_json(&json).expect("uppercase sha256 accepted");
        assert_eq!(manifest.assets[0].sha256.len(), 64);
    }

    #[test]
    fn rejects_zero_size() {
        let json = manifest_json(&format!(
            r#"{{
                "id": "medium", "role": "asr",
                "displayName": "Whisper medium",
                "url": "https://example.org/m.bin",
                "size": 0,
                "sha256": "{}"
            }}"#,
            "a".repeat(64)
        ));
        let err = AssetManifest::from_json(&json).expect_err("zero size");
        assert!(matches!(
            err,
            ManifestError::InvalidField { id, field: "size", .. } if id == "medium"
        ));
    }

    #[test]
    fn rejects_duplicate_ids() {
        let sha = "a".repeat(64);
        let entry = format!(
            r#"{{
                "id": "medium", "role": "asr",
                "displayName": "Whisper medium",
                "url": "https://example.org/m.bin",
                "size": 100,
                "sha256": "{sha}"
            }}"#
        );
        let json = manifest_json(&format!("{entry}, {entry}"));
        let err = AssetManifest::from_json(&json).expect_err("duplicate id");
        assert!(matches!(err, ManifestError::DuplicateId(id) if id == "medium"));
    }

    #[test]
    fn find_and_by_role_lookup_work() {
        let json = manifest_json(&format!(
            r#"
            {{
                "id": "medium", "role": "asr",
                "displayName": "Whisper medium",
                "url": "https://e.org/m", "size": 100, "sha256": "{}"
            }},
            {{
                "id": "large-v3-turbo", "role": "asr",
                "displayName": "Whisper large v3 turbo",
                "url": "https://e.org/l", "size": 1500, "sha256": "{}"
            }},
            {{
                "id": "qwen0.5b", "role": "cleanup_punctuation",
                "displayName": "Qwen 0.5B",
                "url": "https://e.org/q", "size": 400, "sha256": "{}"
            }}
        "#,
            "a".repeat(64),
            "b".repeat(64),
            "c".repeat(64),
        ));
        let manifest = AssetManifest::from_json(&json).expect("valid");

        assert_eq!(manifest.find("large-v3-turbo").unwrap().size, 1500);
        assert!(manifest.find("missing").is_none());

        let asr: Vec<_> = manifest.by_role(AssetRole::Asr).collect();
        assert_eq!(asr.len(), 2);
        assert_eq!(asr[0].id, "medium");
        assert_eq!(asr[1].id, "large-v3-turbo");

        assert_eq!(
            manifest.by_role(AssetRole::CleanupFull).count(),
            0
        );
    }

    #[test]
    fn round_trips_through_serialize() {
        let sha = "a".repeat(64);
        let json = manifest_json(&format!(
            r#"{{
                "id": "medium", "role": "asr",
                "displayName": "Whisper medium",
                "url": "https://e.org/m", "size": 100, "sha256": "{sha}"
            }}"#
        ));
        let manifest = AssetManifest::from_json(&json).expect("valid");
        let reserialized = serde_json::to_string(&manifest).expect("serialize");
        let reparsed = AssetManifest::from_json(&reserialized).expect("reparse");
        assert_eq!(manifest, reparsed);
    }
}
