use std::fmt;
use std::marker::PhantomData;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use wispergo_core::cleanup_inprocess::{LlamaCppCleanupConfig, LlamaCppCleanupProvider};
use wispergo_core::domain::{PipelineResult, ProviderSource};
use wispergo_core::providers::{
    AsrProvider, CleanupInput, CleanupProvider, ProviderError, TextCleanupProvider,
};
use wispergo_core::whisper_rs_provider::WhisperRsProvider;

const DEFAULT_UNAVAILABLE_MESSAGE: &str = "Inference engine is not configured.";
const ASR_TIMEOUT: Duration = Duration::from_secs(30);
const PUNCTUATION_CLEANUP_TIMEOUT: Duration = Duration::from_millis(1200);
const FULL_CLEANUP_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InferenceRuntimeState {
    Disabled,
    Starting,
    Ready,
    Unavailable,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InferenceRuntimeStatus {
    pub state: InferenceRuntimeState,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InferenceRuntimeSnapshot {
    pub status: InferenceRuntimeStatus,
    pub generation: u64,
    pub loaded: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InferenceManagerError {
    Unavailable { engine: String, message: String },
    Failed { engine: String, message: String },
    RuntimeStopped { engine: String },
}

impl fmt::Display for InferenceManagerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable { engine, message } => {
                write!(f, "{engine} unavailable: {message}")
            }
            Self::Failed { engine, message } => write!(f, "{engine} failed: {message}"),
            Self::RuntimeStopped { engine } => write!(f, "{engine} runtime stopped"),
        }
    }
}

impl std::error::Error for InferenceManagerError {}

pub trait ManagedInferenceEngine<P, R>: Send + 'static {
    fn infer(&mut self, payload: P) -> Result<R, InferenceManagerError>;
}

type EngineFactory<C, P, R> = Arc<
    dyn Fn(&C) -> Result<Box<dyn ManagedInferenceEngine<P, R>>, InferenceManagerError>
        + Send
        + Sync,
>;

pub struct EngineRuntime<C, P, R>
where
    C: Send + 'static,
    P: Send + 'static,
    R: Send + 'static,
{
    name: String,
    sender: mpsc::Sender<EngineCommand<C, P, R>>,
    snapshot: Arc<Mutex<InferenceRuntimeSnapshot>>,
    handle: Mutex<Option<thread::JoinHandle<()>>>,
    _marker: PhantomData<(C, P, R)>,
}

impl<C, P, R> EngineRuntime<C, P, R>
where
    C: Send + 'static,
    P: Send + 'static,
    R: Send + 'static,
{
    pub fn new<F>(name: impl Into<String>, idle_timeout: Duration, factory: F) -> Self
    where
        F: Fn(&C) -> Result<Box<dyn ManagedInferenceEngine<P, R>>, InferenceManagerError>
            + Send
            + Sync
            + 'static,
    {
        let name = name.into();
        let (sender, receiver) = mpsc::channel();
        let snapshot = Arc::new(Mutex::new(InferenceRuntimeSnapshot {
            status: InferenceRuntimeStatus {
                state: InferenceRuntimeState::Unavailable,
                message: Some(DEFAULT_UNAVAILABLE_MESSAGE.to_string()),
            },
            generation: 0,
            loaded: false,
        }));
        let worker_snapshot = Arc::clone(&snapshot);
        let worker_sender = sender.clone();
        let worker_name = name.clone();
        let factory: EngineFactory<C, P, R> = Arc::new(factory);
        let handle = thread::Builder::new()
            .name(format!("wispergo-{worker_name}-inference"))
            .spawn(move || {
                run_worker(
                    worker_name,
                    idle_timeout,
                    factory,
                    receiver,
                    worker_sender,
                    worker_snapshot,
                );
            })
            .expect("spawn inference worker");

        Self {
            name,
            sender,
            snapshot,
            handle: Mutex::new(Some(handle)),
            _marker: PhantomData,
        }
    }

    pub fn arm(&self, config: C) -> Result<(), InferenceManagerError> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.send(EngineCommand::Arm { config, reply_tx })?;
        recv_unit(&self.name, reply_rx)
    }

    pub fn disable(&self) -> Result<(), InferenceManagerError> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.send(EngineCommand::Disable { reply_tx })?;
        recv_unit(&self.name, reply_rx)
    }

    pub fn mark_unavailable(
        &self,
        message: impl Into<String>,
    ) -> Result<(), InferenceManagerError> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.send(EngineCommand::MarkUnavailable {
            message: message.into(),
            reply_tx,
        })?;
        recv_unit(&self.name, reply_rx)
    }

    pub fn request(&self, payload: P) -> Result<R, InferenceManagerError> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.send(EngineCommand::Request { payload, reply_tx })?;
        reply_rx
            .recv()
            .map_err(|_| InferenceManagerError::RuntimeStopped {
                engine: self.name.clone(),
            })?
    }

    pub fn status(&self) -> InferenceRuntimeStatus {
        self.snapshot
            .lock()
            .expect("inference snapshot lock")
            .status
            .clone()
    }

    pub fn snapshot(&self) -> InferenceRuntimeSnapshot {
        self.snapshot
            .lock()
            .expect("inference snapshot lock")
            .clone()
    }

    pub fn shutdown(&self) -> Result<(), InferenceManagerError> {
        let handle = self.handle.lock().expect("inference handle lock").take();
        let Some(handle) = handle else {
            return Ok(());
        };

        let (reply_tx, reply_rx) = mpsc::channel();
        self.send(EngineCommand::Shutdown { reply_tx })?;
        recv_unit(&self.name, reply_rx)?;
        handle
            .join()
            .map_err(|_| InferenceManagerError::RuntimeStopped {
                engine: self.name.clone(),
            })?;
        Ok(())
    }

    #[cfg(test)]
    fn request_idle_unload_for_generation(
        &self,
        generation: u64,
    ) -> Result<(), InferenceManagerError> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.send(EngineCommand::UnloadIfIdle {
            generation,
            reply_tx: Some(reply_tx),
        })?;
        recv_unit(&self.name, reply_rx)
    }

    fn send(&self, command: EngineCommand<C, P, R>) -> Result<(), InferenceManagerError> {
        self.sender
            .send(command)
            .map_err(|_| InferenceManagerError::RuntimeStopped {
                engine: self.name.clone(),
            })
    }
}

impl<C, P, R> Drop for EngineRuntime<C, P, R>
where
    C: Send + 'static,
    P: Send + 'static,
    R: Send + 'static,
{
    fn drop(&mut self) {
        if let Ok(mut guard) = self.handle.lock() {
            if let Some(handle) = guard.take() {
                let (reply_tx, _reply_rx) = mpsc::channel();
                let _ = self.sender.send(EngineCommand::Shutdown { reply_tx });
                let _ = handle.join();
            }
        }
    }
}

pub struct InferenceManager {
    asr: EngineRuntime<AsrEngineConfig, AsrInferenceRequest, AsrInferenceOutput>,
    cleanup: EngineRuntime<CleanupEngineConfig, CleanupInferenceRequest, CleanupInferenceOutput>,
}

impl InferenceManager {
    pub fn product() -> Self {
        Self::new(product_asr_engine, product_cleanup_engine)
    }

    pub fn new<AsrFactory, CleanupFactory>(
        asr_factory: AsrFactory,
        cleanup_factory: CleanupFactory,
    ) -> Self
    where
        AsrFactory: Fn(
                &AsrEngineConfig,
            ) -> Result<
                Box<dyn ManagedInferenceEngine<AsrInferenceRequest, AsrInferenceOutput>>,
                InferenceManagerError,
            > + Send
            + Sync
            + 'static,
        CleanupFactory: Fn(
                &CleanupEngineConfig,
            ) -> Result<
                Box<dyn ManagedInferenceEngine<CleanupInferenceRequest, CleanupInferenceOutput>>,
                InferenceManagerError,
            > + Send
            + Sync
            + 'static,
    {
        Self::new_with_idle_timeouts(
            Duration::from_secs(30 * 60),
            Duration::from_secs(5 * 60),
            asr_factory,
            cleanup_factory,
        )
    }

    pub fn new_with_idle_timeouts<AsrFactory, CleanupFactory>(
        asr_idle_timeout: Duration,
        cleanup_idle_timeout: Duration,
        asr_factory: AsrFactory,
        cleanup_factory: CleanupFactory,
    ) -> Self
    where
        AsrFactory: Fn(
                &AsrEngineConfig,
            ) -> Result<
                Box<dyn ManagedInferenceEngine<AsrInferenceRequest, AsrInferenceOutput>>,
                InferenceManagerError,
            > + Send
            + Sync
            + 'static,
        CleanupFactory: Fn(
                &CleanupEngineConfig,
            ) -> Result<
                Box<dyn ManagedInferenceEngine<CleanupInferenceRequest, CleanupInferenceOutput>>,
                InferenceManagerError,
            > + Send
            + Sync
            + 'static,
    {
        Self {
            asr: EngineRuntime::new("asr", asr_idle_timeout, asr_factory),
            cleanup: EngineRuntime::new("cleanup", cleanup_idle_timeout, cleanup_factory),
        }
    }

    pub fn asr(&self) -> &EngineRuntime<AsrEngineConfig, AsrInferenceRequest, AsrInferenceOutput> {
        &self.asr
    }

    pub fn cleanup(
        &self,
    ) -> &EngineRuntime<CleanupEngineConfig, CleanupInferenceRequest, CleanupInferenceOutput> {
        &self.cleanup
    }

    pub fn shutdown(&self) -> Result<(), InferenceManagerError> {
        self.asr.shutdown()?;
        self.cleanup.shutdown()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AsrEngineConfig {
    pub model_path: PathBuf,
    pub language: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AsrInferenceRequest {
    pub audio: Vec<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AsrInferenceOutput {
    pub transcript: String,
    pub confidence: Option<f32>,
    pub source: ProviderSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanupEngineConfig {
    pub model_path: PathBuf,
    pub mode: CleanupInferenceMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CleanupInferenceMode {
    PunctuationOnly,
    FullCleanup,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanupInferenceRequest {
    pub transcript: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CleanupInferenceOutput {
    pub result: PipelineResult,
}

enum EngineCommand<C, P, R> {
    Arm {
        config: C,
        reply_tx: mpsc::Sender<Result<(), InferenceManagerError>>,
    },
    Disable {
        reply_tx: mpsc::Sender<Result<(), InferenceManagerError>>,
    },
    MarkUnavailable {
        message: String,
        reply_tx: mpsc::Sender<Result<(), InferenceManagerError>>,
    },
    Request {
        payload: P,
        reply_tx: mpsc::Sender<Result<R, InferenceManagerError>>,
    },
    UnloadIfIdle {
        generation: u64,
        reply_tx: Option<mpsc::Sender<Result<(), InferenceManagerError>>>,
    },
    Shutdown {
        reply_tx: mpsc::Sender<Result<(), InferenceManagerError>>,
    },
}

fn run_worker<C, P, R>(
    name: String,
    idle_timeout: Duration,
    factory: EngineFactory<C, P, R>,
    receiver: mpsc::Receiver<EngineCommand<C, P, R>>,
    sender: mpsc::Sender<EngineCommand<C, P, R>>,
    snapshot: Arc<Mutex<InferenceRuntimeSnapshot>>,
) where
    C: Send + 'static,
    P: Send + 'static,
    R: Send + 'static,
{
    let mut config: Option<C> = None;
    let mut engine: Option<Box<dyn ManagedInferenceEngine<P, R>>> = None;
    let mut generation: u64 = 0;
    let mut last_used: Option<Instant> = None;

    while let Ok(command) = receiver.recv() {
        match command {
            EngineCommand::Arm {
                config: new_config,
                reply_tx,
            } => {
                generation = generation.wrapping_add(1);
                config = Some(new_config);
                engine = None;
                last_used = None;
                write_snapshot(
                    &snapshot,
                    InferenceRuntimeSnapshot {
                        status: InferenceRuntimeStatus {
                            state: InferenceRuntimeState::Ready,
                            message: None,
                        },
                        generation,
                        loaded: false,
                    },
                );
                let _ = reply_tx.send(Ok(()));
            }
            EngineCommand::Disable { reply_tx } => {
                generation = generation.wrapping_add(1);
                config = None;
                engine = None;
                last_used = None;
                write_snapshot(
                    &snapshot,
                    InferenceRuntimeSnapshot {
                        status: InferenceRuntimeStatus {
                            state: InferenceRuntimeState::Disabled,
                            message: None,
                        },
                        generation,
                        loaded: false,
                    },
                );
                let _ = reply_tx.send(Ok(()));
            }
            EngineCommand::MarkUnavailable { message, reply_tx } => {
                generation = generation.wrapping_add(1);
                config = None;
                engine = None;
                last_used = None;
                write_snapshot(
                    &snapshot,
                    InferenceRuntimeSnapshot {
                        status: InferenceRuntimeStatus {
                            state: InferenceRuntimeState::Unavailable,
                            message: Some(message),
                        },
                        generation,
                        loaded: false,
                    },
                );
                let _ = reply_tx.send(Ok(()));
            }
            EngineCommand::Request { payload, reply_tx } => {
                let result = handle_request(
                    &name,
                    &factory,
                    &mut config,
                    &mut engine,
                    &mut generation,
                    &mut last_used,
                    idle_timeout,
                    &sender,
                    &snapshot,
                    payload,
                );
                let _ = reply_tx.send(result);
            }
            EngineCommand::UnloadIfIdle {
                generation: unload_generation,
                reply_tx,
            } => {
                if unload_generation == generation {
                    if let Some(used_at) = last_used {
                        if engine.is_some() && used_at.elapsed() >= idle_timeout {
                            engine = None;
                            write_snapshot(
                                &snapshot,
                                InferenceRuntimeSnapshot {
                                    status: InferenceRuntimeStatus {
                                        state: InferenceRuntimeState::Ready,
                                        message: None,
                                    },
                                    generation,
                                    loaded: false,
                                },
                            );
                        }
                    }
                }
                if let Some(reply_tx) = reply_tx {
                    let _ = reply_tx.send(Ok(()));
                }
            }
            EngineCommand::Shutdown { reply_tx } => {
                generation = generation.wrapping_add(1);
                write_snapshot(
                    &snapshot,
                    InferenceRuntimeSnapshot {
                        status: InferenceRuntimeStatus {
                            state: InferenceRuntimeState::Disabled,
                            message: None,
                        },
                        generation,
                        loaded: false,
                    },
                );
                let _ = reply_tx.send(Ok(()));
                break;
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_request<C, P, R>(
    name: &str,
    factory: &EngineFactory<C, P, R>,
    config: &mut Option<C>,
    engine: &mut Option<Box<dyn ManagedInferenceEngine<P, R>>>,
    generation: &mut u64,
    last_used: &mut Option<Instant>,
    idle_timeout: Duration,
    sender: &mpsc::Sender<EngineCommand<C, P, R>>,
    snapshot: &Arc<Mutex<InferenceRuntimeSnapshot>>,
    payload: P,
) -> Result<R, InferenceManagerError>
where
    C: Send + 'static,
    P: Send + 'static,
    R: Send + 'static,
{
    let Some(current_config) = config.as_ref() else {
        write_snapshot(
            snapshot,
            InferenceRuntimeSnapshot {
                status: InferenceRuntimeStatus {
                    state: InferenceRuntimeState::Unavailable,
                    message: Some(DEFAULT_UNAVAILABLE_MESSAGE.to_string()),
                },
                generation: *generation,
                loaded: false,
            },
        );
        return Err(InferenceManagerError::Unavailable {
            engine: name.to_string(),
            message: DEFAULT_UNAVAILABLE_MESSAGE.to_string(),
        });
    };

    if engine.is_none() {
        write_snapshot(
            snapshot,
            InferenceRuntimeSnapshot {
                status: InferenceRuntimeStatus {
                    state: InferenceRuntimeState::Starting,
                    message: None,
                },
                generation: *generation,
                loaded: false,
            },
        );

        match catch_unwind(AssertUnwindSafe(|| factory(current_config))) {
            Ok(Ok(loaded_engine)) => {
                *engine = Some(loaded_engine);
                write_snapshot(
                    snapshot,
                    InferenceRuntimeSnapshot {
                        status: InferenceRuntimeStatus {
                            state: InferenceRuntimeState::Ready,
                            message: None,
                        },
                        generation: *generation,
                        loaded: true,
                    },
                );
            }
            Ok(Err(err)) => {
                *engine = None;
                mark_failed(snapshot, *generation, &err);
                return Err(err);
            }
            Err(_) => {
                *engine = None;
                let err = InferenceManagerError::Failed {
                    engine: name.to_string(),
                    message: "inference engine panicked while loading".to_string(),
                };
                mark_failed(snapshot, *generation, &err);
                return Err(err);
            }
        }
    }

    let result = match engine.as_mut() {
        Some(loaded_engine) => catch_unwind(AssertUnwindSafe(|| loaded_engine.infer(payload))),
        None => unreachable!("engine was loaded above"),
    };

    match result {
        Ok(Ok(output)) => {
            *last_used = Some(Instant::now());
            write_snapshot(
                snapshot,
                InferenceRuntimeSnapshot {
                    status: InferenceRuntimeStatus {
                        state: InferenceRuntimeState::Ready,
                        message: None,
                    },
                    generation: *generation,
                    loaded: true,
                },
            );
            schedule_idle_unload(sender.clone(), idle_timeout, *generation);
            Ok(output)
        }
        Ok(Err(err)) => {
            *engine = None;
            *last_used = None;
            mark_failed(snapshot, *generation, &err);
            Err(err)
        }
        Err(_) => {
            *engine = None;
            *last_used = None;
            let err = InferenceManagerError::Failed {
                engine: name.to_string(),
                message: "inference engine panicked while handling request".to_string(),
            };
            mark_failed(snapshot, *generation, &err);
            Err(err)
        }
    }
}

fn schedule_idle_unload<C, P, R>(
    sender: mpsc::Sender<EngineCommand<C, P, R>>,
    idle_timeout: Duration,
    generation: u64,
) where
    C: Send + 'static,
    P: Send + 'static,
    R: Send + 'static,
{
    thread::spawn(move || {
        thread::sleep(idle_timeout);
        let _ = sender.send(EngineCommand::UnloadIfIdle {
            generation,
            reply_tx: None,
        });
    });
}

fn write_snapshot(snapshot: &Arc<Mutex<InferenceRuntimeSnapshot>>, next: InferenceRuntimeSnapshot) {
    *snapshot.lock().expect("inference snapshot lock") = next;
}

fn mark_failed(
    snapshot: &Arc<Mutex<InferenceRuntimeSnapshot>>,
    generation: u64,
    err: &InferenceManagerError,
) {
    write_snapshot(
        snapshot,
        InferenceRuntimeSnapshot {
            status: InferenceRuntimeStatus {
                state: InferenceRuntimeState::Failed,
                message: Some(sanitize_error_message(err)),
            },
            generation,
            loaded: false,
        },
    );
}

fn sanitize_error_message(err: &InferenceManagerError) -> String {
    match err {
        InferenceManagerError::Unavailable { message, .. }
        | InferenceManagerError::Failed { message, .. } => message.clone(),
        InferenceManagerError::RuntimeStopped { .. } => "inference runtime stopped".to_string(),
    }
}

fn recv_unit(
    engine: &str,
    reply_rx: mpsc::Receiver<Result<(), InferenceManagerError>>,
) -> Result<(), InferenceManagerError> {
    reply_rx
        .recv()
        .map_err(|_| InferenceManagerError::RuntimeStopped {
            engine: engine.to_string(),
        })?
}

fn product_asr_engine(
    config: &AsrEngineConfig,
) -> Result<
    Box<dyn ManagedInferenceEngine<AsrInferenceRequest, AsrInferenceOutput>>,
    InferenceManagerError,
> {
    Ok(Box::new(WhisperRsManagedEngine {
        provider: WhisperRsProvider::new(config.model_path.clone())
            .with_language(config.language.clone())
            .with_timeout(ASR_TIMEOUT),
    }))
}

struct WhisperRsManagedEngine {
    provider: WhisperRsProvider,
}

impl ManagedInferenceEngine<AsrInferenceRequest, AsrInferenceOutput> for WhisperRsManagedEngine {
    fn infer(
        &mut self,
        payload: AsrInferenceRequest,
    ) -> Result<AsrInferenceOutput, InferenceManagerError> {
        tauri::async_runtime::block_on(self.provider.transcribe(payload.audio))
            .map(|output| AsrInferenceOutput {
                transcript: output.transcript,
                confidence: output.confidence,
                source: output.source,
            })
            .map_err(|err| provider_error_to_manager("asr", err))
    }
}

fn product_cleanup_engine(
    config: &CleanupEngineConfig,
) -> Result<
    Box<dyn ManagedInferenceEngine<CleanupInferenceRequest, CleanupInferenceOutput>>,
    InferenceManagerError,
> {
    Ok(Box::new(LlamaCppManagedCleanupEngine {
        provider: LlamaCppCleanupProvider::new(LlamaCppCleanupConfig::new(
            config.model_path.clone(),
        )),
        mode: config.mode,
    }))
}

struct LlamaCppManagedCleanupEngine {
    provider: LlamaCppCleanupProvider,
    mode: CleanupInferenceMode,
}

impl ManagedInferenceEngine<CleanupInferenceRequest, CleanupInferenceOutput>
    for LlamaCppManagedCleanupEngine
{
    fn infer(
        &mut self,
        payload: CleanupInferenceRequest,
    ) -> Result<CleanupInferenceOutput, InferenceManagerError> {
        let input = CleanupInput {
            transcript: payload.transcript,
            selected_text: None,
            timeout: match self.mode {
                CleanupInferenceMode::PunctuationOnly => PUNCTUATION_CLEANUP_TIMEOUT,
                CleanupInferenceMode::FullCleanup => FULL_CLEANUP_TIMEOUT,
            },
        };

        match self.mode {
            CleanupInferenceMode::PunctuationOnly => {
                tauri::async_runtime::block_on(self.provider.clean_punctuation_only(input))
                    .map(|text| CleanupInferenceOutput {
                        result: PipelineResult::InsertText {
                            text,
                            source: ProviderSource::Local,
                            confidence: None,
                        },
                    })
                    .map_err(|err| provider_error_to_manager("cleanup", err))
            }
            CleanupInferenceMode::FullCleanup => {
                tauri::async_runtime::block_on(self.provider.clean(input))
                    .map(|output| CleanupInferenceOutput {
                        result: output.result,
                    })
                    .map_err(|err| provider_error_to_manager("cleanup", err))
            }
        }
    }
}

fn provider_error_to_manager(engine: &str, err: ProviderError) -> InferenceManagerError {
    match err {
        ProviderError::Unavailable { message, .. } => InferenceManagerError::Unavailable {
            engine: engine.to_string(),
            message: message.unwrap_or_else(|| "provider unavailable".to_string()),
        },
        ProviderError::Timeout { .. }
        | ProviderError::Failed { .. }
        | ProviderError::InvalidOutput { .. } => InferenceManagerError::Failed {
            engine: engine.to_string(),
            message: err.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    #[derive(Debug, Clone)]
    struct FakeConfig {
        response_prefix: String,
    }

    struct FakeEngine {
        response_prefix: String,
        infer_count: Arc<AtomicUsize>,
        panic_on_infer: Arc<AtomicBool>,
        fail_on_infer: Arc<AtomicBool>,
    }

    impl ManagedInferenceEngine<String, String> for FakeEngine {
        fn infer(&mut self, payload: String) -> Result<String, InferenceManagerError> {
            self.infer_count.fetch_add(1, Ordering::SeqCst);
            if self.panic_on_infer.load(Ordering::SeqCst) {
                panic!("fake inference panic");
            }
            if self.fail_on_infer.load(Ordering::SeqCst) {
                return Err(InferenceManagerError::Failed {
                    engine: "fake".to_string(),
                    message: "fake inference failed".to_string(),
                });
            }
            Ok(format!("{}:{payload}", self.response_prefix))
        }
    }

    #[derive(Clone)]
    struct FakeHarness {
        loads: Arc<AtomicUsize>,
        infers: Arc<AtomicUsize>,
        panic_on_load: Arc<AtomicBool>,
        panic_on_infer: Arc<AtomicBool>,
        fail_on_load: Arc<AtomicBool>,
        fail_on_infer: Arc<AtomicBool>,
    }

    impl FakeHarness {
        fn new() -> Self {
            Self {
                loads: Arc::new(AtomicUsize::new(0)),
                infers: Arc::new(AtomicUsize::new(0)),
                panic_on_load: Arc::new(AtomicBool::new(false)),
                panic_on_infer: Arc::new(AtomicBool::new(false)),
                fail_on_load: Arc::new(AtomicBool::new(false)),
                fail_on_infer: Arc::new(AtomicBool::new(false)),
            }
        }

        fn runtime(&self, idle_timeout: Duration) -> EngineRuntime<FakeConfig, String, String> {
            let harness = self.clone();
            EngineRuntime::new("fake", idle_timeout, move |config: &FakeConfig| {
                harness.loads.fetch_add(1, Ordering::SeqCst);
                if harness.panic_on_load.load(Ordering::SeqCst) {
                    panic!("fake load panic");
                }
                if harness.fail_on_load.load(Ordering::SeqCst) {
                    return Err(InferenceManagerError::Unavailable {
                        engine: "fake".to_string(),
                        message: "fake load failed".to_string(),
                    });
                }
                Ok(Box::new(FakeEngine {
                    response_prefix: config.response_prefix.clone(),
                    infer_count: Arc::clone(&harness.infers),
                    panic_on_infer: Arc::clone(&harness.panic_on_infer),
                    fail_on_infer: Arc::clone(&harness.fail_on_infer),
                }))
            })
        }
    }

    #[test]
    fn default_status_is_unavailable_and_unloaded() {
        let harness = FakeHarness::new();
        let runtime = harness.runtime(Duration::from_secs(60));

        let snapshot = runtime.snapshot();
        assert_eq!(snapshot.status.state, InferenceRuntimeState::Unavailable);
        assert_eq!(
            snapshot.status.message.as_deref(),
            Some("Inference engine is not configured.")
        );
        assert!(!snapshot.loaded);

        runtime.shutdown().expect("shutdown");
    }

    #[test]
    fn arm_marks_ready_without_loading() {
        let harness = FakeHarness::new();
        let runtime = harness.runtime(Duration::from_secs(60));

        runtime
            .arm(FakeConfig {
                response_prefix: "ok".to_string(),
            })
            .expect("arm");

        let snapshot = runtime.snapshot();
        assert_eq!(snapshot.status.state, InferenceRuntimeState::Ready);
        assert!(!snapshot.loaded);
        assert_eq!(harness.loads.load(Ordering::SeqCst), 0);

        runtime.shutdown().expect("shutdown");
    }

    #[test]
    fn first_request_loads_and_reuses_engine_before_idle_deadline() {
        let harness = FakeHarness::new();
        let runtime = harness.runtime(Duration::from_secs(60));
        runtime
            .arm(FakeConfig {
                response_prefix: "loaded".to_string(),
            })
            .expect("arm");

        assert_eq!(
            runtime.request("one".to_string()).expect("request"),
            "loaded:one"
        );
        assert_eq!(
            runtime.request("two".to_string()).expect("request"),
            "loaded:two"
        );

        let snapshot = runtime.snapshot();
        assert_eq!(snapshot.status.state, InferenceRuntimeState::Ready);
        assert!(snapshot.loaded);
        assert_eq!(harness.loads.load(Ordering::SeqCst), 1);
        assert_eq!(harness.infers.load(Ordering::SeqCst), 2);

        runtime.shutdown().expect("shutdown");
    }

    #[test]
    fn idle_unload_drops_loaded_engine_but_remains_ready() {
        let harness = FakeHarness::new();
        let runtime = harness.runtime(Duration::ZERO);
        runtime
            .arm(FakeConfig {
                response_prefix: "loaded".to_string(),
            })
            .expect("arm");
        runtime.request("one".to_string()).expect("request");
        let generation = runtime.snapshot().generation;

        runtime
            .request_idle_unload_for_generation(generation)
            .expect("idle unload");

        let snapshot = runtime.snapshot();
        assert_eq!(snapshot.status.state, InferenceRuntimeState::Ready);
        assert!(!snapshot.loaded);

        assert_eq!(
            runtime.request("two".to_string()).expect("request"),
            "loaded:two"
        );
        assert_eq!(harness.loads.load(Ordering::SeqCst), 2);

        runtime.shutdown().expect("shutdown");
    }

    #[test]
    fn shutdown_invalidates_stale_idle_unload_generation() {
        let harness = FakeHarness::new();
        let runtime = harness.runtime(Duration::from_secs(60));
        runtime
            .arm(FakeConfig {
                response_prefix: "first".to_string(),
            })
            .expect("arm");
        runtime.request("one".to_string()).expect("request");
        let stale_generation = runtime.snapshot().generation;

        runtime.disable().expect("disable");
        runtime
            .arm(FakeConfig {
                response_prefix: "second".to_string(),
            })
            .expect("re-arm");
        runtime.request("two".to_string()).expect("request");
        runtime
            .request_idle_unload_for_generation(stale_generation)
            .expect("stale idle unload");

        let snapshot = runtime.snapshot();
        assert_eq!(snapshot.status.state, InferenceRuntimeState::Ready);
        assert!(snapshot.loaded);
        assert_eq!(snapshot.generation, stale_generation + 2);

        runtime.shutdown().expect("shutdown");
    }

    #[test]
    fn load_panic_is_caught_and_next_request_retries() {
        let harness = FakeHarness::new();
        let runtime = harness.runtime(Duration::from_secs(60));
        runtime
            .arm(FakeConfig {
                response_prefix: "ok".to_string(),
            })
            .expect("arm");
        harness.panic_on_load.store(true, Ordering::SeqCst);

        let err = runtime.request("one".to_string()).expect_err("panic error");
        assert!(err.to_string().contains("panicked while loading"));
        assert_eq!(runtime.status().state, InferenceRuntimeState::Failed);
        assert!(!runtime.snapshot().loaded);

        harness.panic_on_load.store(false, Ordering::SeqCst);
        assert_eq!(runtime.request("two".to_string()).expect("retry"), "ok:two");
        assert_eq!(harness.loads.load(Ordering::SeqCst), 2);

        runtime.shutdown().expect("shutdown");
    }

    #[test]
    fn inference_panic_is_caught_unloads_and_next_request_reloads() {
        let harness = FakeHarness::new();
        let runtime = harness.runtime(Duration::from_secs(60));
        runtime
            .arm(FakeConfig {
                response_prefix: "ok".to_string(),
            })
            .expect("arm");
        harness.panic_on_infer.store(true, Ordering::SeqCst);

        let err = runtime.request("one".to_string()).expect_err("panic error");
        assert!(err.to_string().contains("panicked while handling request"));
        assert_eq!(runtime.status().state, InferenceRuntimeState::Failed);
        assert!(!runtime.snapshot().loaded);

        harness.panic_on_infer.store(false, Ordering::SeqCst);
        assert_eq!(runtime.request("two".to_string()).expect("retry"), "ok:two");
        assert_eq!(harness.loads.load(Ordering::SeqCst), 2);

        runtime.shutdown().expect("shutdown");
    }

    #[test]
    fn inference_failure_unloads_and_next_request_reloads() {
        let harness = FakeHarness::new();
        let runtime = harness.runtime(Duration::from_secs(60));
        runtime
            .arm(FakeConfig {
                response_prefix: "ok".to_string(),
            })
            .expect("arm");
        harness.fail_on_infer.store(true, Ordering::SeqCst);

        let err = runtime
            .request("one".to_string())
            .expect_err("inference error");
        assert!(err.to_string().contains("fake inference failed"));
        assert_eq!(runtime.status().state, InferenceRuntimeState::Failed);
        assert!(!runtime.snapshot().loaded);

        harness.fail_on_infer.store(false, Ordering::SeqCst);
        assert_eq!(runtime.request("two".to_string()).expect("retry"), "ok:two");
        assert_eq!(harness.loads.load(Ordering::SeqCst), 2);

        runtime.shutdown().expect("shutdown");
    }

    #[test]
    fn manager_exposes_asr_and_cleanup_slots() {
        struct EchoEngine;

        impl ManagedInferenceEngine<AsrInferenceRequest, AsrInferenceOutput> for EchoEngine {
            fn infer(
                &mut self,
                payload: AsrInferenceRequest,
            ) -> Result<AsrInferenceOutput, InferenceManagerError> {
                Ok(AsrInferenceOutput {
                    transcript: format!("{} samples", payload.audio.len()),
                    confidence: None,
                    source: ProviderSource::Local,
                })
            }
        }

        struct CleanupEchoEngine;

        impl ManagedInferenceEngine<CleanupInferenceRequest, CleanupInferenceOutput> for CleanupEchoEngine {
            fn infer(
                &mut self,
                payload: CleanupInferenceRequest,
            ) -> Result<CleanupInferenceOutput, InferenceManagerError> {
                Ok(CleanupInferenceOutput {
                    result: PipelineResult::InsertText {
                        text: format!("{}!", payload.transcript),
                        source: ProviderSource::Local,
                        confidence: None,
                    },
                })
            }
        }

        let manager = InferenceManager::new_with_idle_timeouts(
            Duration::from_secs(60),
            Duration::from_secs(60),
            |_config| Ok(Box::new(EchoEngine)),
            |_config| Ok(Box::new(CleanupEchoEngine)),
        );

        manager
            .asr()
            .arm(AsrEngineConfig {
                model_path: PathBuf::from("asr.gguf"),
                language: Some("en".to_string()),
            })
            .expect("arm asr");
        manager
            .cleanup()
            .arm(CleanupEngineConfig {
                model_path: PathBuf::from("cleanup.gguf"),
                mode: CleanupInferenceMode::PunctuationOnly,
            })
            .expect("arm cleanup");

        let asr = manager
            .asr()
            .request(AsrInferenceRequest {
                audio: vec![0.0, 1.0],
            })
            .expect("asr request");
        let cleanup = manager
            .cleanup()
            .request(CleanupInferenceRequest {
                transcript: "hello".to_string(),
            })
            .expect("cleanup request");

        assert_eq!(asr.transcript, "2 samples");
        assert_eq!(
            cleanup.result,
            PipelineResult::InsertText {
                text: "hello!".to_string(),
                source: ProviderSource::Local,
                confidence: None,
            }
        );
        assert_eq!(manager.asr().status().state, InferenceRuntimeState::Ready);
        assert_eq!(
            manager.cleanup().status().state,
            InferenceRuntimeState::Ready
        );

        manager.shutdown().expect("shutdown");
    }
}
