#[allow(dead_code)]
pub mod cleanup_runtime;
pub mod resources;

use tauri::{AppHandle, Manager};
use wispergo_core::asset_storage::AssetStorage;

/// Desktop glue: resolves the app-support models directory from the Tauri app
/// handle and constructs an [`AssetStorage`] over it.
///
/// This is the manifest-driven path layer that will replace the bundled
/// `resources::InferenceResourcePaths` once the downloader routes through it.
/// During the bridge state both exist; nothing in the live runtime path uses
/// this yet.
#[allow(dead_code)]
pub fn app_support_asset_storage(app: &AppHandle) -> Result<AssetStorage, String> {
    let base = app
        .path()
        .app_config_dir()
        .map_err(|err| err.to_string())?;
    Ok(AssetStorage::new(base.join("models")))
}
