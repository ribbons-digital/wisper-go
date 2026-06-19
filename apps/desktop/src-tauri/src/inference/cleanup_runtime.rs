use std::sync::{Arc, Mutex};

use crate::inference::resources::InferenceResourcePaths;

const DEFAULT_UNAVAILABLE_MESSAGE: &str = "Offline punctuation is not ready.";
const MISSING_ASSETS_MESSAGE: &str = "Offline punctuation assets are missing. Reinstall Wispergo.";

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CleanupRuntimeState {
    Disabled,
    Starting,
    Ready,
    Unavailable,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupRuntimeStatus {
    pub state: CleanupRuntimeState,
    pub message: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CleanupRuntimeManager {
    inner: Arc<Mutex<CleanupRuntimeStatus>>,
}

impl Default for CleanupRuntimeManager {
    fn default() -> Self {
        Self {
            inner: Arc::new(Mutex::new(CleanupRuntimeStatus {
                state: CleanupRuntimeState::Unavailable,
                message: Some(DEFAULT_UNAVAILABLE_MESSAGE.to_string()),
            })),
        }
    }
}

impl CleanupRuntimeManager {
    pub fn status(&self) -> CleanupRuntimeStatus {
        self.inner.lock().expect("cleanup runtime lock").clone()
    }

    /// Phase 3.3 bridge: this no longer starts a sidecar process. It only keeps
    /// the frontend-facing readiness status in sync with the bundled cleanup
    /// GGUF path until Phase 4 replaces it with the real `InferenceManager`.
    pub fn start_background(&self, resources: InferenceResourcePaths) {
        let status = if resources.cleanup_model_path.exists() {
            CleanupRuntimeStatus {
                state: CleanupRuntimeState::Ready,
                message: None,
            }
        } else {
            CleanupRuntimeStatus {
                state: CleanupRuntimeState::Unavailable,
                message: Some(MISSING_ASSETS_MESSAGE.to_string()),
            }
        };

        *self.inner.lock().expect("cleanup runtime lock") = status;
    }

    pub fn shutdown(&self) {
        *self.inner.lock().expect("cleanup runtime lock") = CleanupRuntimeStatus {
            state: CleanupRuntimeState::Disabled,
            message: None,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inference::resources::{CpuArchitecture, InferenceResourcePaths};
    use std::path::PathBuf;

    #[test]
    fn default_status_is_sanitized_unavailable() {
        let manager = CleanupRuntimeManager::default();

        assert_eq!(manager.status().state, CleanupRuntimeState::Unavailable);
        assert_eq!(
            manager.status().message.as_deref(),
            Some("Offline punctuation is not ready.")
        );
    }

    #[test]
    fn shutdown_marks_runtime_disabled_without_process_cleanup() {
        let manager = CleanupRuntimeManager::default();

        manager.shutdown();

        let status = manager.status();
        assert_eq!(status.state, CleanupRuntimeState::Disabled);
        assert_eq!(status.message, None);
    }

    #[test]
    fn sync_marks_ready_when_bundled_cleanup_model_exists() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("wispergo-cleanup-runtime-ready-{unique}"));
        let resources = InferenceResourcePaths::from_resource_root_for_arch(
            root.clone(),
            CpuArchitecture::Aarch64,
        );
        std::fs::create_dir_all(
            resources
                .cleanup_model_path
                .parent()
                .expect("cleanup parent"),
        )
        .expect("create cleanup model parent");
        std::fs::write(&resources.cleanup_model_path, b"gguf placeholder").expect("write model");
        let manager = CleanupRuntimeManager::default();

        manager.start_background(resources);

        let status = manager.status();
        assert_eq!(status.state, CleanupRuntimeState::Ready);
        assert_eq!(status.message, None);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn sync_marks_unavailable_when_bundled_cleanup_model_is_missing() {
        let root = PathBuf::from("/Applications/Wispergo.app/Contents/Resources");
        let resources =
            InferenceResourcePaths::from_resource_root_for_arch(root, CpuArchitecture::Aarch64);
        let manager = CleanupRuntimeManager::default();

        manager.start_background(resources);

        let status = manager.status();
        assert_eq!(status.state, CleanupRuntimeState::Unavailable);
        assert_eq!(
            status.message.as_deref(),
            Some("Offline punctuation assets are missing. Reinstall Wispergo.")
        );
    }
}
