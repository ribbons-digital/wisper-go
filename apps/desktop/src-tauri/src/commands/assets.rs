//! Asset management commands: load the bundled manifest, report readiness,
//! and trigger first-run download of default assets with frontend events.
//!
//! The bundled `models.manifest.json` is the source of truth for downloadable
//! model Assets. Live dictation resolves verified model files from app-support
//! storage; the app bundle intentionally does not include model binaries.

use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Manager, State};
use wispergo_core::asset_manifest::AssetManifest;
use wispergo_core::downloader::{
    download_defaults, integrity_sweep, missing_defaults, repair_asset, AssetIntegrity,
};

use crate::commands::settings::sync_inference_manager_for_settings;
use crate::inference::app_support_asset_storage;
use crate::inference::manager::InferenceManager;
use crate::state::AppState;

const MANIFEST_RESOURCE_PATH: &str = "resources/models.manifest.json";
pub(crate) const ASSET_DOWNLOAD_EVENT: &str = "wispergo://asset-download";

/// Frontend-facing download status for the default-asset set.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase", tag = "state")]
pub enum AssetDownloadStatus {
    /// No download needed; all default assets are present (or the manifest has
    /// no defaults).
    Ready,
    /// One or more default assets are missing and should be downloaded.
    #[serde(rename_all = "camelCase")]
    Missing {
        asset_id: String,
        display_name: String,
    },
    /// One or more default assets are being downloaded.
    #[serde(rename_all = "camelCase")]
    Downloading {
        asset_id: String,
        display_name: String,
    },
    /// A download attempt failed.
    Failed { message: String },
}

/// Lazily-shared HTTP client for asset downloads.
#[derive(Debug, Default)]
pub struct AssetClient(Mutex<Option<reqwest::Client>>);

impl AssetClient {
    pub(crate) fn get(&self) -> reqwest::Client {
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
        // Report the first missing asset as the next needed download target.
        let next = missing[0];
        Ok(AssetDownloadStatus::Missing {
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
    app_state: State<'_, AppState>,
    inference_manager: State<'_, InferenceManager>,
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
        sync_inference_manager_for_settings(
            &app,
            inference_manager.inner(),
            &app_state.local_model_settings(),
        );
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

/// Integrity state of a single asset on disk, mirrored to the frontend.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetIntegrityStatus {
    Valid,
    Corrupt,
    Missing,
}

impl From<AssetIntegrity> for AssetIntegrityStatus {
    fn from(value: AssetIntegrity) -> Self {
        match value {
            AssetIntegrity::Valid => Self::Valid,
            AssetIntegrity::Corrupt => Self::Corrupt,
            AssetIntegrity::Missing => Self::Missing,
        }
    }
}

/// Result of an integrity sweep over the default assets.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrityReport {
    /// All default assets are valid; nothing needs repair.
    pub all_valid: bool,
    /// Per-asset integrity for any non-valid default asset.
    pub problems: Vec<IntegrityProblem>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrityProblem {
    pub asset_id: String,
    pub display_name: String,
    pub integrity: AssetIntegrityStatus,
}

/// Sweep all default assets and report any that are corrupt or missing.
///
/// Intended as a startup integrity check. The actual repair-on-load wiring
/// (re-download corrupt assets before a provider loads them) lands in Phase 2
/// (ASR) and Phase 3 (cleanup), when in-process providers replace the sidecars.
/// Today's sidecars do not load via `AssetStorage`, so this command is
/// informational + tested in isolation.
#[tauri::command]
pub fn asset_integrity(app: AppHandle) -> Result<IntegrityReport, String> {
    let manifest = load_bundled_manifest(&app);
    let storage = app_support_asset_storage(&app)?;
    let problems = integrity_sweep(&manifest, &storage)
        .into_iter()
        .map(|(asset, integrity)| IntegrityProblem {
            asset_id: asset.id.clone(),
            display_name: asset.display_name.clone(),
            integrity: integrity.into(),
        })
        .collect::<Vec<_>>();
    let all_valid = problems.is_empty();
    Ok(IntegrityReport {
        all_valid,
        problems,
    })
}

/// Repair (re-download) a single asset by id. Used by the frontend's retry
/// path or by Phase 2/3 load-path hooks when an asset is found corrupt.
#[tauri::command]
pub async fn repair_asset_by_id(
    app: AppHandle,
    client_state: State<'_, AssetClient>,
    app_state: State<'_, AppState>,
    inference_manager: State<'_, InferenceManager>,
    asset_id: String,
) -> Result<AssetDownloadStatus, String> {
    let manifest = load_bundled_manifest(&app);
    let storage = app_support_asset_storage(&app)?;
    let Some(asset) = manifest.find(&asset_id).cloned() else {
        return Err(format!("unknown asset id: {asset_id}"));
    };
    let client = client_state.get();
    match repair_asset(&asset, &storage, &client).await {
        Ok(_) => {
            sync_inference_manager_for_settings(
                &app,
                inference_manager.inner(),
                &app_state.local_model_settings(),
            );
            let _ = app.emit(ASSET_DOWNLOAD_EVENT, AssetDownloadStatus::Ready);
            Ok(AssetDownloadStatus::Ready)
        }
        Err(err) => {
            let message = format!("failed to repair {asset_id}: {err}");
            let _ = app.emit(
                ASSET_DOWNLOAD_EVENT,
                AssetDownloadStatus::Failed {
                    message: message.clone(),
                },
            );
            Ok(AssetDownloadStatus::Failed { message })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_manifest_declares_asr_default_and_accuracy_pack() {
        let manifest_path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/models.manifest.json");
        let text = std::fs::read_to_string(manifest_path).expect("read manifest");
        let manifest = AssetManifest::from_json(&text).expect("valid manifest");

        let medium = manifest.find("medium").expect("medium asset");
        assert_eq!(medium.role, wispergo_core::asset_manifest::AssetRole::Asr);
        assert_eq!(medium.size, 539_212_467);
        assert_eq!(
            medium.sha256,
            "19fea4b380c3a618ec4723c3eef2eb785ffba0d0538cf43f8f235e7b3b34220f"
        );
        assert!(medium.default);
        assert!(medium.url.ends_with("/ggml-medium-q5_0.bin"));

        let accuracy = manifest
            .find("large-v3-turbo")
            .expect("large-v3-turbo asset");
        assert_eq!(accuracy.role, wispergo_core::asset_manifest::AssetRole::Asr);
        assert_eq!(accuracy.size, 1_624_555_275);
        assert!(!accuracy.default);
        assert!(accuracy.url.ends_with("/ggml-large-v3-turbo.bin"));
    }

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
    fn asset_download_status_serializes_missing() {
        let v = serde_json::to_value(AssetDownloadStatus::Missing {
            asset_id: "medium".to_string(),
            display_name: "Whisper medium".to_string(),
        })
        .unwrap();
        assert_eq!(v["state"], "missing");
        assert_eq!(v["assetId"], "medium", "serialized as: {v}");
        assert_eq!(v["displayName"], "Whisper medium");
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

    #[test]
    fn asset_integrity_status_serializes_snake_case() {
        assert_eq!(
            serde_json::to_value(AssetIntegrityStatus::Valid).unwrap(),
            "valid"
        );
        assert_eq!(
            serde_json::to_value(AssetIntegrityStatus::Corrupt).unwrap(),
            "corrupt"
        );
        assert_eq!(
            serde_json::to_value(AssetIntegrityStatus::Missing).unwrap(),
            "missing"
        );
    }

    #[test]
    fn integrity_report_serializes_camel_case() {
        let report = IntegrityReport {
            all_valid: false,
            problems: vec![IntegrityProblem {
                asset_id: "medium".to_string(),
                display_name: "Whisper medium".to_string(),
                integrity: AssetIntegrityStatus::Corrupt,
            }],
        };
        let v = serde_json::to_value(report).unwrap();
        assert_eq!(v["allValid"], false);
        assert_eq!(v["problems"][0]["assetId"], "medium");
        assert_eq!(v["problems"][0]["displayName"], "Whisper medium");
        assert_eq!(v["problems"][0]["integrity"], "corrupt");
    }

    #[test]
    fn asset_integrity_status_from_core_round_trips() {
        assert_eq!(
            AssetIntegrityStatus::from(AssetIntegrity::Valid),
            AssetIntegrityStatus::Valid
        );
        assert_eq!(
            AssetIntegrityStatus::from(AssetIntegrity::Corrupt),
            AssetIntegrityStatus::Corrupt
        );
        assert_eq!(
            AssetIntegrityStatus::from(AssetIntegrity::Missing),
            AssetIntegrityStatus::Missing
        );
    }
}
