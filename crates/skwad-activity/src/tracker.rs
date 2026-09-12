//! A tokio actor that owns the idle, input-protection, and registration
//! timers for one [`ActivityState`] and dispatches its effects to an
//! [`EventSink`]. The public methods are fire-and-forget sends; the state
//! machine rules themselves live in [`ActivityState`] and stay clock-injected
//! and directly unit-testable.

use skwad_agents::AgentState;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::time::{Instant, sleep_until};

use crate::TrackerConfig;
use crate::events::{Effect, EventSink, KeyEvent};
use crate::state::ActivityState;
use crate::tracking::ActivityTracking;

enum Command {
    TerminalActivity,
    UserInput(KeyEvent),
    ProcessExit(Option<i32>),
    HookStatus(AgentState, Option<String>),
    SetTracking(ActivityTracking),
    SetRegistrationPrompt(String),
    Shutdown,
}

/// Actor handle to a single agent's activity tracker.
pub struct Tracker {
    tx: UnboundedSender<Command>,
    handle: tokio::task::JoinHandle<()>,
}

impl Tracker {
    /// Spawn the tracker task. Registration is scheduled for the start; call
    /// [`Tracker::set_registration_prompt`] before it fires to get the prompt
    /// injected.
    pub fn spawn(cfg: TrackerConfig, tracking: ActivityTracking, sink: EventSink) -> Self {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let handle = tokio::spawn(async move {
            let mut state = ActivityState::new(cfg, tracking);
            state.start(Instant::now());
            run(rx, state, sink).await;
        });
        Self { tx, handle }
    }

    /// Terminal output was observed in the agent's terminal.
    pub fn on_terminal_activity(&self) {
        let _ = self.tx.send(Command::TerminalActivity);
    }

    /// A significant user keystroke was observed in the agent's terminal.
    pub fn on_user_input(&self, key: KeyEvent) {
        let _ = self.tx.send(Command::UserInput(key));
    }

    /// The terminal process exited with the given code.
    pub fn on_process_exit(&self, exit_code: Option<i32>) {
        let _ = self.tx.send(Command::ProcessExit(exit_code));
    }

    /// A hook reported a status, with an optional attention message.
    pub fn apply_hook_status(&self, status: AgentState, message: Option<String>) {
        let _ = self.tx.send(Command::HookStatus(status, message));
    }

    /// Change the tracked activity sources at runtime.
    pub fn set_tracking(&self, tracking: ActivityTracking) {
        let _ = self.tx.send(Command::SetTracking(tracking));
    }

    /// Set the registration prompt text to inject once the gate opens.
    pub fn set_registration_prompt(&self, prompt: String) {
        let _ = self.tx.send(Command::SetRegistrationPrompt(prompt));
    }

    /// Ask the tracker to stop. The task also ends when the handle is dropped.
    pub fn shutdown(&self) {
        let _ = self.tx.send(Command::Shutdown);
    }

    /// Wait for the tracker task to finish.
    pub async fn join(self) {
        let _ = self.handle.await;
    }
}

async fn run(mut rx: UnboundedReceiver<Command>, mut state: ActivityState, mut sink: EventSink) {
    loop {
        let idle = state.idle_deadline();
        let guard = state.guard_deadline();
        let registration = state.registration_deadline();
        tokio::select! {
            cmd = rx.recv() => {
                let Some(cmd) = cmd else { break };
                match cmd {
                    Command::TerminalActivity => {
                        apply(&mut sink, state.on_terminal_activity(Instant::now()));
                    }
                    Command::UserInput(key) => {
                        apply(&mut sink, state.on_user_input(Instant::now(), key));
                    }
                    Command::ProcessExit(code) => {
                        apply(&mut sink, state.on_process_exit(Instant::now(), code));
                    }
                    Command::HookStatus(status, message) => {
                        apply(&mut sink, state.apply_hook_status(Instant::now(), status, message));
                    }
                    Command::SetTracking(tracking) => state.set_tracking(tracking),
                    Command::SetRegistrationPrompt(prompt) => state.set_registration_prompt(prompt),
                    Command::Shutdown => break,
                }
            }
            _ = sleep_until(idle.unwrap_or(Instant::now())), if idle.is_some() => {
                apply(&mut sink, state.idle_timer_fired(Instant::now()));
            }
            _ = sleep_until(guard.unwrap_or(Instant::now())), if guard.is_some() => {
                apply(&mut sink, state.guard_expired(Instant::now()));
            }
            _ = sleep_until(registration.unwrap_or(Instant::now())), if registration.is_some() => {
                apply(&mut sink, state.registration_ready_fired(Instant::now()));
            }
        }
    }
}

fn apply(sink: &mut EventSink, effects: Vec<Effect>) {
    for effect in effects {
        sink.apply(effect);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tracking_for;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    fn log_sink(log: &Arc<Mutex<Vec<String>>>) -> EventSink {
        let mut sink = EventSink::default();
        let status_log = Arc::clone(log);
        sink.on_status = Some(Box::new(move |event| {
            status_log.lock().unwrap().push(format!("status:{event:?}"));
        }));
        let check_log = Arc::clone(log);
        sink.on_check_messages = Some(Box::new(move || {
            check_log.lock().unwrap().push("check-messages".into());
        }));
        let inject_log = Arc::clone(log);
        sink.on_inject_registration = Some(Box::new(move |prompt| {
            inject_log.lock().unwrap().push(format!("inject:{prompt}"));
        }));
        let input_log = Arc::clone(log);
        sink.on_awaiting_input = Some(Box::new(move |message| {
            input_log
                .lock()
                .unwrap()
                .push(format!("awaiting:{message:?}"));
        }));
        sink
    }

    async fn tracker(is_hook_based: bool) -> (Tracker, Arc<Mutex<Vec<String>>>) {
        let log = Arc::new(Mutex::new(Vec::new()));
        let cfg = TrackerConfig {
            is_hook_based,
            ..TrackerConfig::for_agent_type("claude")
        };
        let tracker = Tracker::spawn(cfg, tracking_for("claude"), log_sink(&log));
        tokio::task::yield_now().await;
        (tracker, log)
    }

    fn snap(log: &Arc<Mutex<Vec<String>>>) -> Vec<String> {
        log.lock().unwrap().clone()
    }

    /// Under the paused clock, `advance` moves time but woken timer tasks only
    /// run on a later poll; yield once so the actor actually fires its timers.
    async fn settle(d: Duration) {
        tokio::time::advance(d).await;
        tokio::task::yield_now().await;
    }

    #[tokio::test(start_paused = true)]
    async fn goes_idle_after_quiet_period() {
        let (tracker, log) = tracker(false).await;
        tracker.on_terminal_activity();
        settle(Duration::from_millis(50)).await;
        assert!(
            snap(&log).iter().any(|e| e.contains("status:")),
            "working status recorded"
        );
        settle(Duration::from_secs(4)).await;
        let entries = snap(&log);
        assert!(
            entries
                .iter()
                .any(|e| e.contains("status:") && e.contains("Idle"))
        );
        assert!(entries.contains(&"check-messages".into()));
    }

    #[tokio::test(start_paused = true)]
    async fn typing_drives_working_then_idle_in_plain_agent() {
        let (tracker, log) = tracker(false).await;
        tracker.on_user_input(KeyEvent::Other);
        settle(Duration::from_millis(50)).await;
        settle(Duration::from_secs(11)).await;
        let entries = snap(&log);
        assert!(
            entries
                .iter()
                .any(|e| e.contains("status:") && e.contains("Running"))
        );
        assert!(
            entries
                .iter()
                .any(|e| e.contains("status:") && e.contains("Idle"))
        );
    }

    #[tokio::test(start_paused = true)]
    async fn hook_idle_overrides_working_and_cancels_guard() {
        let (tracker, log) = tracker(false).await;
        tracker.on_user_input(KeyEvent::Other);
        settle(Duration::from_millis(50)).await;
        assert!(
            snap(&log)
                .iter()
                .any(|e| e.contains("status:") && e.contains("Running")),
            "local detection sees Working"
        );
        tracker.apply_hook_status(AgentState::Idle, None);
        settle(Duration::from_millis(50)).await;
        // Guard cancelled: no message check at the 10s mark.
        settle(Duration::from_secs(11)).await;
        let entries = snap(&log);
        assert!(
            entries
                .iter()
                .any(|e| e.contains("status:") && e.contains("Idle"))
        );
        assert_eq!(
            entries
                .iter()
                .filter(|e| e.as_str() == "check-messages")
                .count(),
            1,
            "guard-expiry check was suppressed by the hook"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn hook_awaiting_input_notifies_and_return_moves_to_working() {
        let (tracker, log) = tracker(true).await;
        tracker.apply_hook_status(AgentState::Input, Some("grant access?".into()));
        settle(Duration::from_millis(50)).await;
        let entries = snap(&log);
        assert!(
            entries
                .iter()
                .any(|e| e.contains("status:") && e.contains("Input"))
        );
        assert!(
            entries
                .iter()
                .any(|e| e.starts_with("awaiting:Some(\"grant access?\")"))
        );

        tracker.on_user_input(KeyEvent::Return);
        settle(Duration::from_millis(50)).await;
        let entries = snap(&log);
        assert!(
            entries
                .iter()
                .any(|e| e.contains("status:") && e.contains("Running"))
        );
    }

    #[tokio::test(start_paused = true)]
    async fn registration_injects_once_after_first_idle() {
        let (tracker, log) = tracker(true).await;
        tracker.set_registration_prompt("Register with the skwad".into());
        tracker.apply_hook_status(AgentState::Running, None);
        settle(Duration::from_millis(50)).await;
        tracker.apply_hook_status(AgentState::Idle, None);
        settle(Duration::from_millis(50)).await;
        settle(Duration::from_secs(2)).await;
        let entries = snap(&log);
        assert_eq!(
            entries.iter().filter(|e| e.starts_with("inject:")).count(),
            1,
            "registration injected exactly once"
        );
        // Lodge another idle; no second injection.
        tracker.apply_hook_status(AgentState::Running, None);
        settle(Duration::from_millis(50)).await;
        tracker.apply_hook_status(AgentState::Idle, None);
        settle(Duration::from_secs(3)).await;
        let entries = snap(&log);
        assert_eq!(
            entries.iter().filter(|e| e.starts_with("inject:")).count(),
            1,
            "no second injection after later idles"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn dropping_handle_stops_the_task() {
        let (tracker, _) = tracker(true).await;
        drop(tracker);
        settle(Duration::from_millis(50)).await;
    }
}
