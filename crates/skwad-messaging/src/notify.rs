use std::sync::Mutex;

use uuid::Uuid;

/// Called by [`crate::routing::send`]/[`crate::routing::broadcast`] once per
/// stored message whose recipient is currently `AgentState::Idle`. The
/// actual "surface this in the terminal" side effect belongs to whichever
/// crate owns the terminal; `skwad-messaging` only reports that a message
/// landed for an idle agent.
pub trait DeliveryNotifier {
    fn notify(&self, agent_id: Uuid, message_id: Uuid);
}

/// Records every call for assertions in tests.
#[derive(Debug, Default)]
pub struct RecordingNotifier {
    calls: Mutex<Vec<(Uuid, Uuid)>>,
}

impl RecordingNotifier {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn calls(&self) -> Vec<(Uuid, Uuid)> {
        self.calls.lock().unwrap().clone()
    }
}

impl DeliveryNotifier for RecordingNotifier {
    fn notify(&self, agent_id: Uuid, message_id: Uuid) {
        self.calls.lock().unwrap().push((agent_id, message_id));
    }
}

/// A `DeliveryNotifier` for callers with no delivery-side-effect need yet.
#[derive(Debug, Default)]
pub struct NoopNotifier;

impl DeliveryNotifier for NoopNotifier {
    fn notify(&self, _agent_id: Uuid, _message_id: Uuid) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recording_notifier_records_calls() {
        let notifier = RecordingNotifier::new();
        let agent = Uuid::new_v4();
        let message = Uuid::new_v4();

        notifier.notify(agent, message);

        assert_eq!(notifier.calls(), vec![(agent, message)]);
    }
}
