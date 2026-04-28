#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerEvent {
    PressAndHoldStart,
    PressAndHoldStop,
    Toggle,
}

#[derive(Debug, Default)]
pub struct TriggerManager {
    toggle_active: bool,
}

impl TriggerManager {
    pub fn handle(&mut self, event: TriggerEvent) -> &'static str {
        match event {
            TriggerEvent::PressAndHoldStart => "start",
            TriggerEvent::PressAndHoldStop => "stop",
            TriggerEvent::Toggle => {
                self.toggle_active = !self.toggle_active;
                if self.toggle_active {
                    "start"
                } else {
                    "stop"
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{TriggerEvent, TriggerManager};

    #[test]
    fn toggle_alternates_start_and_stop() {
        let mut manager = TriggerManager::default();

        assert_eq!(manager.handle(TriggerEvent::Toggle), "start");
        assert_eq!(manager.handle(TriggerEvent::Toggle), "stop");
    }

    #[test]
    fn press_and_hold_maps_to_start_and_stop() {
        let mut manager = TriggerManager::default();

        assert_eq!(manager.handle(TriggerEvent::PressAndHoldStart), "start");
        assert_eq!(manager.handle(TriggerEvent::PressAndHoldStop), "stop");
    }
}
