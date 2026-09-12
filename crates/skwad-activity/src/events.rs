//! Events the tracker emits and the sink that delivers them.

use skwad_agents::AgentState;
use tokio::time::Instant;

/// Where a status change came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivitySource {
    /// Terminal output drove the local detector.
    Terminal,
    /// A user keystroke drove the local detector.
    UserInput,
    /// A hook reported the status authoritatively.
    Hook,
}

/// A significant keystroke, mapped by the terminal integration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyEvent {
    /// Return/Enter.
    Return,
    /// Escape.
    Escape,
    /// Any other significantly printable key.
    Other,
}

/// A recorded status change, with the change time for dashboard sorting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusEvent {
    pub status: AgentState,
    pub source: ActivitySource,
    pub changed_at: Instant,
}

/// A side effect the tracker wants performed by its owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// Agent status changed.
    Status(StatusEvent),
    /// Entered (or is in) Awaiting input; the hook-supplied message is present
    /// when the hook provided one. Used to raise a desktop notification.
    AwaitingInput(Option<String>),
    /// Check for unread MCP messages and deliver them (subject to the guard).
    CheckMessages,
    /// Inject the deferred registration prompt (guard already honoured).
    InjectRegistration(String),
}

/// Callbacks the tracker owner wires up to consume effects. Each is optional.
#[derive(Default)]
pub struct EventSink {
    pub on_status: Option<Box<dyn FnMut(StatusEvent) + Send>>,
    pub on_awaiting_input: Option<Box<dyn FnMut(Option<String>) + Send>>,
    pub on_check_messages: Option<Box<dyn FnMut() + Send>>,
    pub on_inject_registration: Option<Box<dyn FnMut(String) + Send>>,
}

impl EventSink {
    /// Dispatch a single effect to the matching callback.
    pub fn apply(&mut self, effect: Effect) {
        match effect {
            Effect::Status(event) => {
                if let Some(cb) = self.on_status.as_mut() {
                    cb(event);
                }
            }
            Effect::AwaitingInput(message) => {
                if let Some(cb) = self.on_awaiting_input.as_mut() {
                    cb(message);
                }
            }
            Effect::CheckMessages => {
                if let Some(cb) = self.on_check_messages.as_mut() {
                    cb();
                }
            }
            Effect::InjectRegistration(prompt) => {
                if let Some(cb) = self.on_inject_registration.as_mut() {
                    cb(prompt);
                }
            }
        }
    }
}
