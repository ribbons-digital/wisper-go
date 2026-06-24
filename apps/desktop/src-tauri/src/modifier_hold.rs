use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter};

use crate::shortcut::{
    ModifierHoldAction, ModifierHoldInput, ModifierHoldKey, ModifierHoldSettings,
    ModifierHoldStateMachine, RECORD_SHORTCUT_EVENT,
};

const NX_DEVICELCTLKEYMASK: u64 = 0x0000_0001;
const NX_DEVICELSHIFTKEYMASK: u64 = 0x0000_0002;
const NX_DEVICERSHIFTKEYMASK: u64 = 0x0000_0004;
const NX_DEVICELCMDKEYMASK: u64 = 0x0000_0008;
const NX_DEVICERCMDKEYMASK: u64 = 0x0000_0010;
const NX_DEVICELALTKEYMASK: u64 = 0x0000_0020;
const NX_DEVICERALTKEYMASK: u64 = 0x0000_0040;
const NX_DEVICERCTLKEYMASK: u64 = 0x0000_2000;

fn modifier_mask(key: ModifierHoldKey) -> u64 {
    match key {
        ModifierHoldKey::LeftCommand => NX_DEVICELCMDKEYMASK,
        ModifierHoldKey::RightCommand => NX_DEVICERCMDKEYMASK,
        ModifierHoldKey::LeftOption => NX_DEVICELALTKEYMASK,
        ModifierHoldKey::RightOption => NX_DEVICERALTKEYMASK,
        ModifierHoldKey::LeftControl => NX_DEVICELCTLKEYMASK,
        ModifierHoldKey::RightControl => NX_DEVICERCTLKEYMASK,
        ModifierHoldKey::LeftShift => NX_DEVICELSHIFTKEYMASK,
        ModifierHoldKey::RightShift => NX_DEVICERSHIFTKEYMASK,
    }
}

fn all_supported_modifier_masks() -> u64 {
    NX_DEVICELCMDKEYMASK
        | NX_DEVICERCMDKEYMASK
        | NX_DEVICELALTKEYMASK
        | NX_DEVICERALTKEYMASK
        | NX_DEVICELCTLKEYMASK
        | NX_DEVICERCTLKEYMASK
        | NX_DEVICELSHIFTKEYMASK
        | NX_DEVICERSHIFTKEYMASK
}

fn modifier_is_down(key: ModifierHoldKey, flags: u64) -> bool {
    flags & modifier_mask(key) != 0
}

fn has_other_modifier(key: ModifierHoldKey, flags: u64) -> bool {
    flags & (all_supported_modifier_masks() & !modifier_mask(key)) != 0
}

enum TimerCommand {
    ScheduleThreshold { generation: u64, delay_ms: u64 },
    ScheduleWatchdog { generation: u64, delay_ms: u64 },
    Stop,
}

struct ScheduledTimer {
    deadline: Instant,
    input: ModifierHoldInput,
}

#[cfg(target_os = "macos")]
fn spawn_timer_worker(
    app: AppHandle,
    machine: Arc<Mutex<ModifierHoldStateMachine>>,
) -> (mpsc::Sender<TimerCommand>, thread::JoinHandle<()>) {
    let (tx, rx) = mpsc::channel::<TimerCommand>();
    let join = thread::Builder::new()
        .name("wispergo-modifier-hold-timer".to_string())
        .spawn(move || {
            let mut timers: Vec<ScheduledTimer> = Vec::new();
            loop {
                timers.sort_by_key(|timer| timer.deadline);
                let timeout = timers
                    .first()
                    .map(|timer| timer.deadline.saturating_duration_since(Instant::now()))
                    .unwrap_or(Duration::from_secs(60));

                match rx.recv_timeout(timeout) {
                    Ok(TimerCommand::ScheduleThreshold {
                        generation,
                        delay_ms,
                    }) => {
                        timers.push(ScheduledTimer {
                            deadline: Instant::now() + Duration::from_millis(delay_ms),
                            input: ModifierHoldInput::ThresholdElapsed { generation },
                        });
                    }
                    Ok(TimerCommand::ScheduleWatchdog {
                        generation,
                        delay_ms,
                    }) => {
                        timers.push(ScheduledTimer {
                            deadline: Instant::now() + Duration::from_millis(delay_ms),
                            input: ModifierHoldInput::WatchdogElapsed { generation },
                        });
                    }
                    Ok(TimerCommand::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                }

                let now = Instant::now();
                let mut pending = Vec::new();
                for timer in timers.drain(..) {
                    if timer.deadline <= now {
                        dispatch_input(&app, &machine, timer.input);
                    } else {
                        pending.push(timer);
                    }
                }
                timers = pending;
            }
        })
        .expect("spawn modifier hold timer worker");
    (tx, join)
}

pub struct ModifierHoldMonitor {
    stop: mpsc::Sender<()>,
    stopped: mpsc::Receiver<()>,
    join: Option<thread::JoinHandle<()>>,
}

impl ModifierHoldMonitor {
    #[cfg(target_os = "macos")]
    pub fn start(app: AppHandle, settings: ModifierHoldSettings) -> Result<Self, String> {
        use core_foundation::runloop::{kCFRunLoopDefaultMode, CFRunLoop, CFRunLoopRunResult};
        use core_graphics::event::{
            CallbackResult, CGEventTap, CGEventTapLocation, CGEventTapOptions,
            CGEventTapPlacement, CGEventType,
        };

        let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();
        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let (stopped_tx, stopped_rx) = mpsc::channel::<()>();
        let join = thread::Builder::new()
            .name("wispergo-modifier-hold-monitor".to_string())
            .spawn(move || {
                let run_loop = CFRunLoop::get_current();
                let machine = Arc::new(Mutex::new(ModifierHoldStateMachine::new(settings.clone())));
                let (timer_tx, timer_join) = spawn_timer_worker(app.clone(), Arc::clone(&machine));
                let selected_key = settings.key;
                let app_for_callback = app.clone();
                let machine_for_callback = Arc::clone(&machine);
                let timer_for_callback = timer_tx.clone();

                let tap_result = CGEventTap::new(
                    CGEventTapLocation::Session,
                    CGEventTapPlacement::HeadInsertEventTap,
                    CGEventTapOptions::ListenOnly,
                    vec![CGEventType::FlagsChanged, CGEventType::KeyDown],
                    move |_proxy, event_type, event| {
                        handle_cg_event(
                            &app_for_callback,
                            &machine_for_callback,
                            &timer_for_callback,
                            selected_key,
                            event_type,
                            event,
                        );
                        CallbackResult::Keep
                    },
                );

                let Ok(tap) = tap_result else {
                    let _ = timer_tx.send(TimerCommand::Stop);
                    let _ = timer_join.join();
                    let _ = ready_tx.send(Err(
                        "Modifier-hold shortcuts require macOS Accessibility permission."
                            .to_string(),
                    ));
                    return;
                };

                let source = match tap.mach_port().create_runloop_source(0) {
                    Ok(source) => source,
                    Err(()) => {
                        let _ = timer_tx.send(TimerCommand::Stop);
                        let _ = timer_join.join();
                        let _ = ready_tx.send(Err(
                            "Modifier-hold event monitor could not start.".to_string(),
                        ));
                        return;
                    }
                };

                run_loop.add_source(&source, unsafe { kCFRunLoopDefaultMode });
                tap.enable();
                let _ = ready_tx.send(Ok(()));

                loop {
                    match stop_rx.try_recv() {
                        Ok(()) | Err(mpsc::TryRecvError::Disconnected) => break,
                        Err(mpsc::TryRecvError::Empty) => {}
                    }
                    match CFRunLoop::run_in_mode(
                        unsafe { kCFRunLoopDefaultMode },
                        Duration::from_millis(100),
                        true,
                    ) {
                        CFRunLoopRunResult::Stopped
                        | CFRunLoopRunResult::Finished
                        | CFRunLoopRunResult::TimedOut
                        | CFRunLoopRunResult::HandledSource => {}
                    }
                }

                let _ = timer_tx.send(TimerCommand::Stop);
                let _ = timer_join.join();
                drop(tap);
                let _ = stopped_tx.send(());
            })
            .map_err(|err| err.to_string())?;

        ready_rx
            .recv_timeout(Duration::from_secs(2))
            .map_err(|_| "Modifier-hold event monitor did not start in time.".to_string())??;

        Ok(Self {
            stop: stop_tx,
            stopped: stopped_rx,
            join: Some(join),
        })
    }

    #[cfg(not(target_os = "macos"))]
    pub fn start(_app: AppHandle, _settings: ModifierHoldSettings) -> Result<Self, String> {
        Err("Modifier-hold shortcuts are only supported on macOS.".to_string())
    }

    pub fn stop(mut self) {
        let _ = self.stop.send(());
        if self.stopped.recv_timeout(Duration::from_secs(2)).is_ok() {
            if let Some(join) = self.join.take() {
                let _ = join.join();
            }
        }
    }
}

impl Drop for ModifierHoldMonitor {
    fn drop(&mut self) {
        let _ = self.stop.send(());
    }
}

#[cfg(target_os = "macos")]
fn handle_cg_event(
    app: &AppHandle,
    machine: &Arc<Mutex<ModifierHoldStateMachine>>,
    timer: &mpsc::Sender<TimerCommand>,
    selected_key: ModifierHoldKey,
    event_type: core_graphics::event::CGEventType,
    event: &core_graphics::event::CGEvent,
) {
    use core_graphics::event::CGEventType;

    match event_type {
        CGEventType::FlagsChanged => {
            let flags = event.get_flags().bits();
            let input = if modifier_is_down(selected_key, flags) {
                if has_other_modifier(selected_key, flags) {
                    ModifierHoldInput::OtherModifierJoined
                } else {
                    ModifierHoldInput::SelectedModifierDown
                }
            } else {
                ModifierHoldInput::SelectedModifierUp
            };
            dispatch_input_with_timer(app, machine, timer, input);
        }
        CGEventType::KeyDown => {
            dispatch_input_with_timer(app, machine, timer, ModifierHoldInput::OtherKeyDown);
        }
        _ => {}
    }
}

#[cfg(target_os = "macos")]
fn dispatch_input_with_timer(
    app: &AppHandle,
    machine: &Arc<Mutex<ModifierHoldStateMachine>>,
    timer: &mpsc::Sender<TimerCommand>,
    input: ModifierHoldInput,
) {
    let actions = machine
        .lock()
        .expect("modifier hold state lock")
        .handle_event(input);
    run_actions(app, timer, actions);
}

#[cfg(target_os = "macos")]
fn dispatch_input(
    app: &AppHandle,
    machine: &Arc<Mutex<ModifierHoldStateMachine>>,
    input: ModifierHoldInput,
) {
    let actions = machine
        .lock()
        .expect("modifier hold state lock")
        .handle_event(input);
    for action in actions {
        match action {
            ModifierHoldAction::EmitPressed => {
                let _ = app.emit(RECORD_SHORTCUT_EVENT, "Pressed");
            }
            ModifierHoldAction::EmitReleased => {
                let _ = app.emit(RECORD_SHORTCUT_EVENT, "Released");
            }
            ModifierHoldAction::ScheduleThreshold { .. }
            | ModifierHoldAction::ScheduleWatchdog { .. } => {}
        }
    }
}

#[cfg(target_os = "macos")]
fn run_actions(
    app: &AppHandle,
    timer: &mpsc::Sender<TimerCommand>,
    actions: Vec<ModifierHoldAction>,
) {
    for action in actions {
        match action {
            ModifierHoldAction::EmitPressed => {
                let _ = app.emit(RECORD_SHORTCUT_EVENT, "Pressed");
            }
            ModifierHoldAction::EmitReleased => {
                let _ = app.emit(RECORD_SHORTCUT_EVENT, "Released");
            }
            ModifierHoldAction::ScheduleThreshold {
                generation,
                delay_ms,
            } => {
                let _ = timer.send(TimerCommand::ScheduleThreshold {
                    generation,
                    delay_ms,
                });
            }
            ModifierHoldAction::ScheduleWatchdog {
                generation,
                delay_ms,
            } => {
                let _ = timer.send(TimerCommand::ScheduleWatchdog {
                    generation,
                    delay_ms,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_modifier_detects_right_command_from_device_flag() {
        assert!(modifier_is_down(
            ModifierHoldKey::RightCommand,
            NX_DEVICERCMDKEYMASK
        ));
        assert!(!modifier_is_down(
            ModifierHoldKey::RightCommand,
            NX_DEVICELCMDKEYMASK
        ));
    }

    #[test]
    fn selected_modifier_detects_all_supported_physical_keys() {
        assert!(modifier_is_down(
            ModifierHoldKey::LeftCommand,
            NX_DEVICELCMDKEYMASK
        ));
        assert!(modifier_is_down(
            ModifierHoldKey::RightCommand,
            NX_DEVICERCMDKEYMASK
        ));
        assert!(modifier_is_down(
            ModifierHoldKey::LeftOption,
            NX_DEVICELALTKEYMASK
        ));
        assert!(modifier_is_down(
            ModifierHoldKey::RightOption,
            NX_DEVICERALTKEYMASK
        ));
        assert!(modifier_is_down(
            ModifierHoldKey::LeftControl,
            NX_DEVICELCTLKEYMASK
        ));
        assert!(modifier_is_down(
            ModifierHoldKey::RightControl,
            NX_DEVICERCTLKEYMASK
        ));
        assert!(modifier_is_down(
            ModifierHoldKey::LeftShift,
            NX_DEVICELSHIFTKEYMASK
        ));
        assert!(modifier_is_down(
            ModifierHoldKey::RightShift,
            NX_DEVICERSHIFTKEYMASK
        ));
    }

    #[test]
    fn only_selected_modifier_detects_no_other_modifier() {
        assert!(!has_other_modifier(
            ModifierHoldKey::RightCommand,
            NX_DEVICERCMDKEYMASK
        ));
        assert!(has_other_modifier(
            ModifierHoldKey::RightCommand,
            NX_DEVICERCMDKEYMASK | NX_DEVICELSHIFTKEYMASK,
        ));
    }

    #[test]
    fn selected_modifier_reasserted_without_other_modifier_is_not_other_modifier() {
        let flags = NX_DEVICERCMDKEYMASK;

        assert!(modifier_is_down(ModifierHoldKey::RightCommand, flags));
        assert!(!has_other_modifier(ModifierHoldKey::RightCommand, flags));
    }

    #[test]
    fn monitor_source_stays_listen_only_and_never_drops_events() {
        let source = include_str!("modifier_hold.rs");
        let drop_variant = ["CallbackResult", "::", "Drop"].concat();
        let replace_variant = ["CallbackResult", "::", "Replace"].concat();

        assert!(source.contains("CGEventTapOptions::ListenOnly"));
        assert!(source.contains("CallbackResult::Keep"));
        assert!(!source.contains(&drop_variant));
        assert!(!source.contains(&replace_variant));
    }
}
