#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerEvent {
    PressAndHoldStart,
    PressAndHoldStop,
    Toggle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerAction {
    Start,
    Stop,
}

#[derive(Debug, Default)]
pub struct TriggerManager {
    toggle_active: bool,
}

impl TriggerManager {
    pub fn handle(&mut self, event: TriggerEvent) -> TriggerAction {
        match event {
            TriggerEvent::PressAndHoldStart => {
                self.toggle_active = true;
                TriggerAction::Start
            }
            TriggerEvent::PressAndHoldStop => {
                self.toggle_active = false;
                TriggerAction::Stop
            }
            TriggerEvent::Toggle => {
                self.toggle_active = !self.toggle_active;
                if self.toggle_active {
                    TriggerAction::Start
                } else {
                    TriggerAction::Stop
                }
            }
        }
    }

    pub fn sync_recording_active(&mut self, active: bool) {
        self.toggle_active = active;
    }
}

#[cfg(test)]
mod tests {
    use super::{TriggerAction, TriggerEvent, TriggerManager};

    #[test]
    fn toggle_alternates_start_and_stop() {
        let mut manager = TriggerManager::default();

        assert_eq!(manager.handle(TriggerEvent::Toggle), TriggerAction::Start);
        assert_eq!(manager.handle(TriggerEvent::Toggle), TriggerAction::Stop);
    }

    #[test]
    fn press_and_hold_maps_to_start_and_stop() {
        let mut manager = TriggerManager::default();

        assert_eq!(
            manager.handle(TriggerEvent::PressAndHoldStart),
            TriggerAction::Start
        );
        assert_eq!(
            manager.handle(TriggerEvent::PressAndHoldStop),
            TriggerAction::Stop
        );
    }

    #[test]
    fn external_status_sync_keeps_toggle_in_step() {
        let mut manager = TriggerManager::default();

        assert_eq!(manager.handle(TriggerEvent::Toggle), TriggerAction::Start);
        manager.sync_recording_active(false);
        assert_eq!(manager.handle(TriggerEvent::Toggle), TriggerAction::Start);
    }
}
