use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use wispergo_core::llama_server::{LlamaServerCleanupProvider, DEFAULT_LLAMA_SERVER_MODEL};

use crate::inference::resources::InferenceResourcePaths;

const DEFAULT_UNAVAILABLE_MESSAGE: &str = "Offline punctuation is not ready.";
const MISSING_ASSETS_MESSAGE: &str = "Offline punctuation assets are missing. Reinstall Wispergo.";
const PREPARING_MESSAGE: &str = "Preparing offline punctuation.";
const START_FAILED_MESSAGE: &str = "Offline punctuation could not start.";
const TIMEOUT_MESSAGE: &str = "Offline punctuation did not become ready in time.";
const STOPPED_MESSAGE: &str = "Offline punctuation is stopped.";
const STOPPED_UNEXPECTEDLY_MESSAGE: &str = "Offline punctuation stopped unexpectedly.";
const LLAMA_SERVER_HOST: &str = "127.0.0.1";
const LLAMA_SERVER_FALLBACK_PORT: u16 = 41_173;
const READINESS_DEADLINE: Duration = Duration::from_secs(30);
const READINESS_ATTEMPT_TIMEOUT: Duration = Duration::from_millis(750);
const READINESS_RETRY_DELAY: Duration = Duration::from_millis(250);
const CHILD_MONITOR_INTERVAL: Duration = Duration::from_secs(2);

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
    inner: Arc<Mutex<CleanupRuntimeInner>>,
}

#[derive(Debug)]
struct CleanupRuntimeInner {
    status: CleanupRuntimeStatus,
    base_url: Option<String>,
    child: Option<Child>,
    generation: u64,
}

impl Default for CleanupRuntimeManager {
    fn default() -> Self {
        Self {
            inner: Arc::new(Mutex::new(CleanupRuntimeInner {
                status: CleanupRuntimeStatus {
                    state: CleanupRuntimeState::Unavailable,
                    message: Some(DEFAULT_UNAVAILABLE_MESSAGE.to_string()),
                },
                base_url: None,
                child: None,
                generation: 0,
            })),
        }
    }
}

impl CleanupRuntimeManager {
    pub fn status(&self) -> CleanupRuntimeStatus {
        self.inner
            .lock()
            .expect("cleanup runtime lock")
            .status
            .clone()
    }

    pub fn provider(&self) -> Option<LlamaServerCleanupProvider> {
        let inner = self.inner.lock().expect("cleanup runtime lock");
        if inner.status.state != CleanupRuntimeState::Ready {
            return None;
        }

        inner.base_url.as_ref().map(|base_url| {
            LlamaServerCleanupProvider::new(
                base_url.clone(),
                DEFAULT_LLAMA_SERVER_MODEL.to_string(),
            )
        })
    }

    pub fn start_background(&self, resources: InferenceResourcePaths) {
        let manager = self.clone();
        tauri::async_runtime::spawn(async move {
            manager.start(resources).await;
        });
    }

    fn start_background_if_generation(&self, resources: InferenceResourcePaths, generation: u64) {
        let manager = self.clone();
        tauri::async_runtime::spawn(async move {
            if manager.should_restart_for_generation(generation) {
                manager.start(resources).await;
            }
        });
    }

    pub async fn start(&self, resources: InferenceResourcePaths) {
        if resources.validate_required_assets().is_err() {
            self.terminate_runtime_with_status(
                CleanupRuntimeState::Unavailable,
                Some(MISSING_ASSETS_MESSAGE.to_string()),
            );
            return;
        }

        let (generation, existing_child) = self.begin_start();
        terminate_child_if_present(existing_child);
        if !self.is_generation_current(generation) {
            return;
        }

        let port = choose_local_port();
        let base_url = format!("http://{LLAMA_SERVER_HOST}:{port}");
        let command = CleanupRuntimeCommand::new(&resources, port);
        let child = match command.spawn() {
            Ok(child) => child,
            Err(err) => {
                eprintln!("cleanup runtime process failed to start: {err}");
                self.set_status_if_generation(
                    generation,
                    CleanupRuntimeState::Failed,
                    Some(START_FAILED_MESSAGE.to_string()),
                );
                return;
            }
        };

        if !self.store_started_child(generation, child, base_url.clone()) {
            return;
        }

        self.start_child_monitor(resources.clone(), generation);

        let provider =
            LlamaServerCleanupProvider::new(base_url, DEFAULT_LLAMA_SERVER_MODEL.to_string());
        let deadline = Instant::now() + READINESS_DEADLINE;
        while Instant::now() < deadline {
            if !self.is_generation_current(generation) {
                return;
            }

            if provider.warm(READINESS_ATTEMPT_TIMEOUT).await.is_ok() {
                self.set_status_if_generation(generation, CleanupRuntimeState::Ready, None);
                return;
            }

            tokio::time::sleep(READINESS_RETRY_DELAY).await;
        }

        if self.is_generation_current(generation) {
            self.terminate_runtime_with_status(
                CleanupRuntimeState::Failed,
                Some(TIMEOUT_MESSAGE.to_string()),
            );
        }
    }

    async fn monitor_child(&self, resources: InferenceResourcePaths, generation: u64) {
        enum MonitorOutcome {
            Continue,
            Restart { generation: u64 },
            TerminateAndRestart { child: Child, generation: u64 },
        }

        loop {
            tokio::time::sleep(CHILD_MONITOR_INTERVAL).await;

            let outcome = {
                let mut inner = self.inner.lock().expect("cleanup runtime lock");
                if inner.generation != generation {
                    return;
                }

                let Some(child) = inner.child.as_mut() else {
                    return;
                };

                match child.try_wait() {
                    Ok(Some(_status)) => {
                        inner.child.take();
                        inner.base_url = None;
                        inner.status = CleanupRuntimeStatus {
                            state: CleanupRuntimeState::Failed,
                            message: Some(STOPPED_UNEXPECTEDLY_MESSAGE.to_string()),
                        };
                        inner.generation = inner.generation.wrapping_add(1);
                        let restart_generation = inner.generation;
                        MonitorOutcome::Restart {
                            generation: restart_generation,
                        }
                    }
                    Ok(None) => MonitorOutcome::Continue,
                    Err(err) => {
                        eprintln!("cleanup runtime process monitor failed: {err}");
                        let child = inner.child.take();
                        inner.base_url = None;
                        inner.status = CleanupRuntimeStatus {
                            state: CleanupRuntimeState::Failed,
                            message: Some(STOPPED_UNEXPECTEDLY_MESSAGE.to_string()),
                        };
                        inner.generation = inner.generation.wrapping_add(1);
                        let restart_generation = inner.generation;
                        match child {
                            Some(child) => MonitorOutcome::TerminateAndRestart {
                                child,
                                generation: restart_generation,
                            },
                            None => MonitorOutcome::Restart {
                                generation: restart_generation,
                            },
                        }
                    }
                }
            };

            match outcome {
                MonitorOutcome::Continue => {}
                MonitorOutcome::Restart { generation } => {
                    self.start_background_if_generation(resources.clone(), generation);
                    return;
                }
                MonitorOutcome::TerminateAndRestart { child, generation } => {
                    terminate_child_if_present(Some(child));
                    self.start_background_if_generation(resources.clone(), generation);
                    return;
                }
            }
        }
    }

    pub fn shutdown(&self) {
        self.terminate_runtime_with_status(
            CleanupRuntimeState::Unavailable,
            Some(STOPPED_MESSAGE.to_string()),
        );
    }

    fn begin_start(&self) -> (u64, Option<Child>) {
        let mut inner = self.inner.lock().expect("cleanup runtime lock");
        inner.generation = inner.generation.wrapping_add(1);
        let generation = inner.generation;
        let existing_child = inner.child.take();
        inner.base_url = None;
        inner.status = CleanupRuntimeStatus {
            state: CleanupRuntimeState::Starting,
            message: Some(PREPARING_MESSAGE.to_string()),
        };
        (generation, existing_child)
    }

    fn store_started_child(&self, generation: u64, child: Child, base_url: String) -> bool {
        let mut inner = self.inner.lock().expect("cleanup runtime lock");
        if inner.generation != generation {
            drop(inner);
            terminate_child_if_present(Some(child));
            return false;
        }

        inner.child = Some(child);
        inner.base_url = Some(base_url);
        true
    }

    fn start_child_monitor(&self, resources: InferenceResourcePaths, generation: u64) {
        let manager = self.clone();
        tauri::async_runtime::spawn(async move {
            manager.monitor_child(resources, generation).await;
        });
    }

    fn is_generation_current(&self, generation: u64) -> bool {
        self.inner.lock().expect("cleanup runtime lock").generation == generation
    }

    fn should_restart_for_generation(&self, generation: u64) -> bool {
        let inner = self.inner.lock().expect("cleanup runtime lock");
        inner.generation == generation && inner.status.state == CleanupRuntimeState::Failed
    }

    fn set_status_if_generation(
        &self,
        generation: u64,
        state: CleanupRuntimeState,
        message: Option<String>,
    ) {
        let mut inner = self.inner.lock().expect("cleanup runtime lock");
        if inner.generation == generation {
            inner.status = CleanupRuntimeStatus { state, message };
        }
    }

    fn terminate_runtime_with_status(&self, state: CleanupRuntimeState, message: Option<String>) {
        let (generation, child) = {
            let mut inner = self.inner.lock().expect("cleanup runtime lock");
            inner.generation = inner.generation.wrapping_add(1);
            let generation = inner.generation;
            let child = inner.child.take();
            inner.base_url = None;
            (generation, child)
        };

        terminate_child_if_present(child);

        let mut inner = self.inner.lock().expect("cleanup runtime lock");
        if inner.generation == generation {
            inner.status = CleanupRuntimeStatus { state, message };
        }
    }

    #[cfg(test)]
    fn mark_ready_for_test(&self, base_url: &str) {
        let mut inner = self.inner.lock().expect("cleanup runtime lock");
        inner.base_url = Some(base_url.to_string());
        inner.status = CleanupRuntimeStatus {
            state: CleanupRuntimeState::Ready,
            message: None,
        };
    }

    #[cfg(test)]
    fn mark_failed_for_test(&self, message: &str) {
        let mut inner = self.inner.lock().expect("cleanup runtime lock");
        inner.status = CleanupRuntimeStatus {
            state: CleanupRuntimeState::Failed,
            message: Some(message.to_string()),
        };
    }

    #[cfg(test)]
    fn generation_for_test(&self) -> u64 {
        self.inner.lock().expect("cleanup runtime lock").generation
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanupRuntimeCommand {
    pub program: PathBuf,
    pub args: Vec<String>,
}

impl CleanupRuntimeCommand {
    pub fn new(resources: &InferenceResourcePaths, port: u16) -> Self {
        Self {
            program: resources.llama_server_binary_path.clone(),
            args: vec![
                "-m".to_string(),
                resources.cleanup_model_path.display().to_string(),
                "--host".to_string(),
                LLAMA_SERVER_HOST.to_string(),
                "--port".to_string(),
                port.to_string(),
                "--ctx-size".to_string(),
                "2048".to_string(),
                "--n-gpu-layers".to_string(),
                "999".to_string(),
            ],
        }
    }

    fn spawn(&self) -> std::io::Result<Child> {
        Command::new(&self.program)
            .args(&self.args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
    }
}

pub fn choose_local_port() -> u16 {
    TcpListener::bind((LLAMA_SERVER_HOST, 0))
        .and_then(|listener| listener.local_addr())
        .map(|addr| addr.port())
        .unwrap_or(LLAMA_SERVER_FALLBACK_PORT)
}

fn terminate_child_if_present(child: Option<Child>) {
    if let Some(mut child) = child {
        match child.try_wait() {
            Ok(Some(_status)) => return,
            Ok(None) => {
                if let Err(err) = child.kill() {
                    eprintln!("cleanup runtime process kill failed: {err}");
                }
            }
            Err(err) => {
                eprintln!("cleanup runtime process status check failed: {err}");
                if let Err(err) = child.kill() {
                    eprintln!("cleanup runtime process kill failed: {err}");
                }
            }
        }

        if let Err(err) = child.wait() {
            eprintln!("cleanup runtime process wait failed: {err}");
        }
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
    fn ready_status_does_not_expose_port_or_model_details() {
        let manager = CleanupRuntimeManager::default();
        manager.mark_ready_for_test("http://127.0.0.1:43210");

        let status = manager.status();
        assert_eq!(status.state, CleanupRuntimeState::Ready);
        assert_eq!(status.message, None);
        assert!(manager.provider().is_some());
    }

    #[test]
    fn server_command_uses_bundled_binary_model_and_localhost() {
        let root = PathBuf::from("/Applications/Wispergo.app/Contents/Resources");
        let resources = InferenceResourcePaths::from_resource_root_for_arch(
            root.clone(),
            CpuArchitecture::Aarch64,
        );

        let command = CleanupRuntimeCommand::new(&resources, 43_210);

        assert_eq!(command.program, root.join("bin/macos-aarch64/llama-server"));
        assert_eq!(
            command.args,
            vec![
                "-m".to_string(),
                root.join("models/cleanup/qwen2.5-3b-instruct-q4_k_m.gguf")
                    .display()
                    .to_string(),
                "--host".to_string(),
                "127.0.0.1".to_string(),
                "--port".to_string(),
                "43210".to_string(),
                "--ctx-size".to_string(),
                "2048".to_string(),
                "--n-gpu-layers".to_string(),
                "999".to_string(),
            ]
        );
    }

    #[test]
    fn stopped_child_transitions_to_failed_status() {
        let manager = CleanupRuntimeManager::default();
        manager.mark_failed_for_test("Offline punctuation stopped unexpectedly.");

        let status = manager.status();
        assert_eq!(status.state, CleanupRuntimeState::Failed);
        assert_eq!(
            status.message.as_deref(),
            Some("Offline punctuation stopped unexpectedly.")
        );
    }

    #[test]
    fn restart_guard_is_invalidated_by_shutdown_generation() {
        let manager = CleanupRuntimeManager::default();
        manager.mark_failed_for_test("Offline punctuation stopped unexpectedly.");
        let generation = manager.generation_for_test();

        assert!(manager.should_restart_for_generation(generation));

        manager.shutdown();

        assert!(!manager.should_restart_for_generation(generation));
    }
}
