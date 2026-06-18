//! Asset management commands: load the bundled manifest, report readiness,
//! and trigger first-run download of default assets with frontend events.
//!
//! Bridge state (Phase 1.2): this is plumbing only. The live dictation path
//! still uses the bundled sidecar and does NOT consume downloaded assets yet.
//! The dictation-readiness gate ("downloading models" blocking dictation) is
//! deferred to Phase 2, when the in-process ASR provider replaces the sidecar.
//! Gating now would block dictation that currently works via the bundle.
//!
//! The bundled `models.manifest.json` ships as a structural placeholder
//! (empty assets) in this phase; real model entries land in Phase 5 (model
//! tiering) once models are locked and SHA-256s computed.

use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Manager, State};
use wispergo_core::asset_manifest::AssetManifest;
use wispergo_core::downloader::{download_defaults, missing_defaults};

use crate::inference::app_support_asset_storage;

const MANIFEST_RESOURCE_PATH: &str = "resources/models.manifest.json";
const ASSET_DOWNLOAD_EVENT: &str = "wispergo://asset-download";

/// Frontend-facing download status for the default-asset set.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase", tag = "state")]
pub enum AssetDownloadStatus {
    /// No download needed; all default assets are present (or the manifest has
    /// no defaults).
    Ready,
    /// One or more default assets are being downloaded.
    #[serde(rename_all = "camelCase")]
    Downloading { asset_id: String, display_name: String },
    /// A download attempt failed.
    Failed { message: String },
}

/// Lazily-shared HTTP client for asset downloads.
#[derive(Debug, Default)]
pub struct AssetClient(Mutex<Option<reqwest::Client>>);

impl AssetClient {
    fn get(&self) -> reqwest::Client {
        self.0
            .lock()
            .expect("asset client lock")
            .get_or_insert_with(reqwest::Client::new)
            .clone()
    }
}

/// Load the bundled asset manifest. Returns an empty manifest if the resource
/// is absent or unreadable (e.g. dev builds without staged assets), so callers
/// can treat "no manifest" as "nothing to download yet" rather than an error.
pub fn load_bundled_manifest(app: &AppHandle) -> AssetManifest {
    let Some(resource_root) = app.path().resource_dir().ok() else {
        return empty_manifest();
    };
    let path = resource_root.join(MANIFEST_RESOURCE_PATH);
    let Ok(text) = std::fs::read_to_string(&path) else {
        return empty_manifest();
    };
    AssetManifest::from_json(&text).unwrap_or_else(|_| empty_manifest())
}

fn empty_manifest() -> AssetManifest {
    AssetManifest {
        schema_version: 1,
        assets: Vec::new(),
    }
}

/// Report whether all default assets are present.
#[tauri::command]
pub fn asset_readiness(app: AppHandle) -> Result<AssetDownloadStatus, String> {
    let manifest = load_bundled_manifest(&app);
    let storage = app_support_asset_storage(&app)?;
    let missing = missing_defaults(&manifest, &storage);
    if missing.is_empty() {
        Ok(AssetDownloadStatus::Ready)
    } else {
        // Report the first missing asset as the current download target.
        let next = missing[0];
        Ok(AssetDownloadStatus::Downloading {
            asset_id: next.id.clone(),
            display_name: next.display_name.clone(),
        })
    }
}

/// Download every missing default asset, emitting `ASSET_DOWNLOAD_EVENT` for
/// each. Idempotent: assets already present are skipped. Returns the final
/// status. Failures are reported per-asset; the command itself succeeds and
/// returns `Failed` if any asset could not be downloaded.
#[tauri::command]
pub async fn ensure_model_assets(
    app: AppHandle,
    client_state: State<'_, AssetClient>,
) -> Result<AssetDownloadStatus, String> {
    let manifest = load_bundled_manifest(&app);
    let storage = app_support_asset_storage(&app)?;
    let client = client_state.get();

    let results = download_defaults(&manifest, &storage, &client, |asset| {
        let _ = app.emit(
            ASSET_DOWNLOAD_EVENT,
            AssetDownloadStatus::Downloading {
                asset_id: asset.id.clone(),
                display_name: asset.display_name.clone(),
            },
        );
    })
    .await;

    let failed: Vec<_> = results
        .iter()
        .filter(|r| r.outcome.is_err())
        .map(|r| r.asset_id.clone())
        .collect();

    if failed.is_empty() {
        let _ = app.emit(ASSET_DOWNLOAD_EVENT, AssetDownloadStatus::Ready);
        Ok(AssetDownloadStatus::Ready)
    } else {
        let message = format!("failed to download assets: {}", failed.join(", "));
        let _ = app.emit(
            ASSET_DOWNLOAD_EVENT,
            AssetDownloadStatus::Failed {
                message: message.clone(),
            },
        );
        Ok(AssetDownloadStatus::Failed { message })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_manifest_has_no_assets() {
        let m = empty_manifest();
        assert!(m.assets.is_empty());
        assert_eq!(m.schema_version, 1);
    }

    #[test]
    fn asset_download_status_serializes_ready() {
        let v = serde_json::to_value(AssetDownloadStatus::Ready).unwrap();
        assert_eq!(v["state"], "ready");
    }

    #[test]
    fn asset_download_status_serializes_downloading() {
        let v = serde_json::to_value(AssetDownloadStatus::Downloading {
            asset_id: "medium".to_string(),
            display_name: "Whisper medium".to_string(),
        })
        .unwrap();
        assert_eq!(v["state"], "downloading");
        assert_eq!(v["assetId"], "medium", "serialized as: {v}");
        assert_eq!(v["displayName"], "Whisper medium");
    }

    #[test]
    fn asset_download_status_serializes_failed() {
        let v = serde_json::to_value(AssetDownloadStatus::Failed {
            message: "boom".to_string(),
        })
        .unwrap();
        assert_eq!(v["state"], "failed");
        assert_eq!(v["message"], "boom");
    }
}
