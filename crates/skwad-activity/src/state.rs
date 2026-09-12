//! The automatic status state machine: pure, clock-injected rules over
//! `tokio::time::Instant`. The tokio actor in [`crate::tracker`] owns the
//! timers and calls these methods with `Instant::now()`.

use std::time::Duration;

use skwad_agents::AgentState;
use tokio::time::Instant;

use crate::consts::{
    DEFAULT_IDLE_TIMEOUT, HOOK_FALLBACK_IDLE_TIMEOUT, REGISTRATION_FIRST_IDLE_DELAY_LONG,
    REGISTRATION_FIRST_IDLE_DELAY_SHORT, REGISTRATION_SUBSEQUENT_IDLE_DELAY,
    USER_INPUT_IDLE_TIMEOUT,
};
use crate::events::{ActivitySource, Effect, KeyEvent, StatusEvent};
use crate::tracking::ActivityTracking;

/// Configuration for one tracker, fixed for the agent's lifetime.
#[derive(Debug, Clone)]
pub struct TrackerConfig {
    /// Agent type this tracker is attached to.
    pub agent_type: String,
    /// Idle timeout after terminal output for non-hook agents.
    pub idle_timeout: Duration,
    /// Idle timeout after a user keystroke, and the guard window.
    pub user_input_idle_timeout: Duration,
    /// Terminal-output idle timeout for hook-managed agents.
    pub hook_fallback_timeout: Duration,
    /// Hook-managed agent: keystrokes do not drive the state machine.
    pub is_hook_based: bool,
    /// MCP server enabled. When false, no registration gating.
    pub mcp_enabled: bool,
    /// Agent registers through CLI arguments; skip deferred registration.
    pub inline_registration: bool,
    /// Slow-starting agent: use the long first-idle registration delay.
    pub slow_startup: bool,
    /// Registration delay after the first idle for fast-starting agents.
    pub first_idle_delay_short: Duration,
    /// Registration delay after the first idle for slow-starting agents.
    pub first_idle_delay_long: Duration,
    /// Registration delay after subsequent idles.
    pub subsequent_idle_delay: Duration,
}

impl Default for TrackerConfig {
    fn default() -> Self {
        Self {
            agent_type: String::new(),
            idle_timeout: DEFAULT_IDLE_TIMEOUT,
            user_input_idle_timeout: USER_INPUT_IDLE_TIMEOUT,
            hook_fallback_timeout: HOOK_FALLBACK_IDLE_TIMEOUT,
            is_hook_based: false,
            mcp_enabled: true,
            inline_registration: false,
            slow_startup: false,
            first_idle_delay_short: REGISTRATION_FIRST_IDLE_DELAY_SHORT,
            first_idle_delay_long: REGISTRATION_FIRST_IDLE_DELAY_LONG,
            subsequent_idle_delay: REGISTRATION_SUBSEQUENT_IDLE_DELAY,
        }
    }
}

impl TrackerConfig {
    /// Defaults for `agent_type`. Hook flags and MCP wiring are set by the
    /// caller once they are known.
    pub fn for_agent_type(agent_type: impl Into<String>) -> Self {
        Self {
            agent_type: agent_type.into(),
            ..Self::default()
        }
    }
}

/// An armed idle timer: when to fire and the window it was armed with.
#[derive(Debug, Clone)]
struct IdleTimer {
    at: Instant,
    window: Duration,
}

/// The automatic status model for one agent.
#[derive(Debug, Clone)]
pub struct ActivityState {
    cfg: TrackerConfig,
    tracking: ActivityTracking,
    status: AgentState,
    changed_at: Instant,
    last_activity_at: Option<Instant>,
    last_activity_source: ActivitySource,
    idle_due: Option<IdleTimer>,
    guard_until: Option<Instant>,
    has_become_idle: bool,
    idle_count: u64,
    registration_ready_at: Option<Instant>,
    registration_attempted: bool,
    registration_prompt: Option<String>,
    did_inject_registration: bool,
}

impl ActivityState {
    /// Starts in Idle with nothing tracked.
    pub fn new(cfg: TrackerConfig, tracking: ActivityTracking) -> Self {
        Self {
            cfg,
            tracking,
            status: AgentState::Idle,
            changed_at: Instant::now(),
            last_activity_at: None,
            last_activity_source: ActivitySource::Terminal,
            idle_due: None,
            guard_until: None,
            has_become_idle: false,
            idle_count: 0,
            registration_ready_at: None,
            registration_attempted: false,
            registration_prompt: None,
            did_inject_registration: false,
        }
    }

    /// The current status.
    pub fn status(&self) -> AgentState {
        self.status
    }

    /// Time of the last status change, for dashboard sorting.
    pub fn changed_at(&self) -> Instant {
        self.changed_at
    }

    /// When the armed idle timer fires, if any.
    pub fn idle_deadline(&self) -> Option<Instant> {
        self.idle_due.as_ref().map(|t| t.at)
    }

    /// When the input-protection guard expires, if armed.
    pub fn guard_deadline(&self) -> Option<Instant> {
        self.guard_until
    }

    /// When the registration prompt may next be evaluated, if scheduled and
    /// not yet consumed.
    pub fn registration_deadline(&self) -> Option<Instant> {
        (!self.registration_attempted)
            .then_some(self.registration_ready_at)
            .flatten()
    }

    /// Schedule deferred registration, gated on MCP being enabled and the
    /// agent not registering inline. No-op otherwise.
    pub fn start(&mut self, now: Instant) {
        self.schedule_registration(self.first_idle_delay(), now);
    }

    /// Downgrade (or otherwise change) the tracked activity sources at runtime.
    pub fn set_tracking(&mut self, tracking: ActivityTracking) {
        self.tracking = tracking;
    }

    /// Set the registration prompt text to inject once conditions are met.
    pub fn set_registration_prompt(&mut self, prompt: String) {
        self.registration_prompt = Some(prompt);
    }

    /// Terminal output observed.
    pub fn on_terminal_activity(&mut self, now: Instant) -> Vec<Effect> {
        // Always cancel a pending idle timer, even when terminal output is not
        // being tracked or the agent is awaiting input.
        self.idle_due = None;
        if self.status == AgentState::Input
            || !self.tracking.contains(ActivityTracking::TERMINAL_OUTPUT)
        {
            return Vec::new();
        }
        self.last_activity_at = Some(now);
        self.last_activity_source = ActivitySource::Terminal;
        let window = self.terminal_idle_timeout();
        self.idle_due = Some(IdleTimer {
            at: now + window,
            window,
        });
        let mut effects = Vec::new();
        self.set_status(
            AgentState::Running,
            ActivitySource::Terminal,
            now,
            &mut effects,
        );
        effects
    }

    /// A significant user keystroke observed.
    pub fn on_user_input(&mut self, now: Instant, key: KeyEvent) -> Vec<Effect> {
        if !self.tracking.contains(ActivityTracking::USER_INPUT) {
            return Vec::new();
        }
        self.last_activity_at = Some(now);
        self.last_activity_source = ActivitySource::UserInput;
        self.guard_until = Some(now + self.cfg.user_input_idle_timeout);

        let mut effects = Vec::new();
        if self.status == AgentState::Input {
            // Awaiting-input is exited only by a keystroke.
            match key {
                KeyEvent::Return => {
                    self.idle_due = None;
                    self.set_status(
                        AgentState::Running,
                        ActivitySource::UserInput,
                        now,
                        &mut effects,
                    );
                }
                KeyEvent::Escape => {
                    self.idle_due = None;
                    self.mark_idle(now, ActivitySource::UserInput, &mut effects);
                }
                KeyEvent::Other => {}
            }
        } else if !self.cfg.is_hook_based {
            let window = self.cfg.user_input_idle_timeout;
            self.idle_due = Some(IdleTimer {
                at: now + window,
                window,
            });
            self.set_status(
                AgentState::Running,
                ActivitySource::UserInput,
                now,
                &mut effects,
            );
        }
        effects
    }

    /// A hook reported a status authoritatively, carrying an optional message
    /// for the awaiting-input notification.
    pub fn apply_hook_status(
        &mut self,
        now: Instant,
        status: AgentState,
        message: Option<String>,
    ) -> Vec<Effect> {
        // Any hook status cancels the input-protection guard.
        self.guard_until = None;
        let mut effects = Vec::new();
        if status == AgentState::Idle {
            self.mark_idle(now, ActivitySource::Hook, &mut effects);
        } else {
            self.set_status(status, ActivitySource::Hook, now, &mut effects);
            if status == AgentState::Input {
                effects.push(Effect::AwaitingInput(message));
            }
        }
        effects
    }

    /// The terminal process exited.
    pub fn on_process_exit(&mut self, now: Instant, exit_code: Option<i32>) -> Vec<Effect> {
        self.idle_due = None;
        self.guard_until = None;
        let mut effects = Vec::new();
        let status = match exit_code {
            Some(code) if code != 0 => AgentState::Error,
            _ => AgentState::Idle,
        };
        self.set_status(status, ActivitySource::Terminal, now, &mut effects);
        if status == AgentState::Idle {
            self.mark_idle(now, ActivitySource::Terminal, &mut effects);
        }
        effects
    }

    /// The armed idle timer fired.
    pub fn idle_timer_fired(&mut self, now: Instant) -> Vec<Effect> {
        if self.status == AgentState::Input {
            // Idle timers never move an awaiting-input agent.
            self.idle_due = None;
            return Vec::new();
        }
        let Some(IdleTimer { at, window }) = self.idle_due.take() else {
            return Vec::new();
        };
        let since = self.last_activity_at.unwrap_or(at);
        if now.saturating_duration_since(since) >= window {
            self.idle_due = None;
            let mut effects = Vec::new();
            self.mark_idle(now, self.last_activity_source, &mut effects);
            effects
        } else {
            // Newer activity exists; reschedule for the remaining interval.
            self.idle_due = Some(IdleTimer {
                at: since + window,
                window,
            });
            Vec::new()
        }
    }

    /// The input-protection guard expired.
    pub fn guard_expired(&mut self, now: Instant) -> Vec<Effect> {
        let Some(until) = self.guard_until else {
            return Vec::new();
        };
        if now < until {
            return Vec::new();
        }
        self.guard_until = None;
        let mut effects = Vec::new();
        if let Some(inject) = self.maybe_inject_registration(now) {
            effects.push(inject);
        }
        effects.push(Effect::CheckMessages);
        effects
    }

    /// The scheduled registration time arrived.
    pub fn registration_ready_fired(&mut self, now: Instant) -> Vec<Effect> {
        let Some(ready) = self.registration_ready_at else {
            return Vec::new();
        };
        if now < ready || self.registration_attempted {
            return Vec::new();
        }
        self.registration_attempted = true;
        self.maybe_inject_registration(now).into_iter().collect()
    }

    fn terminal_idle_timeout(&self) -> Duration {
        if self.cfg.is_hook_based {
            self.cfg.hook_fallback_timeout
        } else {
            self.cfg.idle_timeout
        }
    }

    fn first_idle_delay(&self) -> Duration {
        if self.cfg.slow_startup {
            self.cfg.first_idle_delay_long
        } else {
            self.cfg.first_idle_delay_short
        }
    }

    fn registration_enabled(&self) -> bool {
        self.cfg.mcp_enabled && !self.cfg.inline_registration
    }

    fn schedule_registration(&mut self, delay: Duration, now: Instant) {
        if self.registration_enabled() && !self.did_inject_registration {
            self.registration_ready_at = Some(now + delay);
            self.registration_attempted = false;
        }
    }

    fn maybe_inject_registration(&mut self, now: Instant) -> Option<Effect> {
        if self.did_inject_registration || !self.registration_enabled() {
            return None;
        }
        if !self.has_become_idle {
            return None;
        }
        let ready = self.registration_ready_at?;
        if now < ready {
            return None;
        }
        if self.guard_until.is_some() {
            return None;
        }
        let prompt = self.registration_prompt.take()?;
        self.did_inject_registration = true;
        self.registration_ready_at = None;
        Some(Effect::InjectRegistration(prompt))
    }

    /// Transition to Idle: checks unread messages and schedules the next
    /// registration evaluation only on a real transition out of another state.
    fn mark_idle(&mut self, now: Instant, source: ActivitySource, effects: &mut Vec<Effect>) {
        let was_idle = self.status == AgentState::Idle;
        self.set_status(AgentState::Idle, source, now, effects);
        if was_idle {
            return;
        }
        self.has_become_idle = true;
        if !self.did_inject_registration && self.registration_enabled() {
            self.idle_count += 1;
            let delay = if self.idle_count == 1 {
                self.first_idle_delay()
            } else {
                self.cfg.subsequent_idle_delay
            };
            self.schedule_registration(delay, now);
        }
        effects.push(Effect::CheckMessages);
    }

    fn set_status(
        &mut self,
        status: AgentState,
        source: ActivitySource,
        now: Instant,
        effects: &mut Vec<Effect>,
    ) {
        if self.status == status {
            return;
        }
        self.status = status;
        self.changed_at = now;
        effects.push(Effect::Status(StatusEvent {
            status,
            source,
            changed_at: now,
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tracking::tracking_for;
    use std::time::Instant as StdInstant;

    fn at(offset: Duration) -> Instant {
        Instant::from_std(StdInstant::now()) + offset
    }

    fn config(agent_type: &str) -> TrackerConfig {
        TrackerConfig::for_agent_type(agent_type)
    }

    fn status_effects(effects: &[Effect]) -> Vec<AgentState> {
        effects
            .iter()
            .filter_map(|e| match e {
                Effect::Status(event) => Some(event.status),
                _ => None,
            })
            .collect()
    }

    fn has_effect(effects: &[Effect], kind: EffectKind) -> bool {
        effects.iter().any(|e| e.kind() == kind)
    }

    // Local mirror of Effect discriminants for assertions.
    #[derive(PartialEq)]
    enum EffectKind {
        Status,
        AwaitingInput,
        CheckMessages,
        InjectRegistration,
    }

    trait EffectTag {
        fn kind(&self) -> EffectKind;
    }
    impl EffectTag for Effect {
        fn kind(&self) -> EffectKind {
            match self {
                Effect::Status(_) => EffectKind::Status,
                Effect::AwaitingInput(_) => EffectKind::AwaitingInput,
                Effect::CheckMessages => EffectKind::CheckMessages,
                Effect::InjectRegistration(_) => EffectKind::InjectRegistration,
            }
        }
    }

    #[test]
    fn shell_agent_never_leaves_idle() {
        let mut state = ActivityState::new(config("shell"), tracking_for("shell"));
        let effects = state.on_terminal_activity(at(Duration::ZERO));
        assert!(status_effects(&effects).is_empty());
        assert_eq!(state.status(), AgentState::Idle);
        assert!(state.idle_due.is_none());
    }

    #[test]
    fn terminal_output_sets_working_and_arms_idle_timer() {
        let mut state = ActivityState::new(config("claude"), tracking_for("claude"));
        let t0 = at(Duration::ZERO);
        let effects = state.on_terminal_activity(t0);
        assert_eq!(state.status(), AgentState::Running);
        assert_eq!(status_effects(&effects), vec![AgentState::Running]);
        assert_eq!(state.idle_deadline(), Some(t0 + Duration::from_secs(3)));
    }

    #[test]
    fn idle_fires_after_quiet_period() {
        let mut state = ActivityState::new(config("claude"), tracking_for("claude"));
        let t0 = at(Duration::ZERO);
        state.on_terminal_activity(t0);
        let effects = state.idle_timer_fired(t0 + Duration::from_secs(3));
        assert_eq!(state.status(), AgentState::Idle);
        assert!(has_effect(&effects, EffectKind::CheckMessages));
    }

    #[test]
    fn late_activity_defers_idle_for_remaining_interval() {
        let mut state = ActivityState::new(config("claude"), tracking_for("claude"));
        let t0 = at(Duration::ZERO);
        state.on_terminal_activity(t0);
        state.on_terminal_activity(t0 + Duration::from_secs(2));
        let effects = state.idle_timer_fired(t0 + Duration::from_secs(3));
        assert!(status_effects(&effects).is_empty());
        assert_eq!(state.status(), AgentState::Running);
        assert_eq!(state.idle_deadline(), Some(t0 + Duration::from_secs(5)));
    }

    #[test]
    fn runtime_downgrade_stops_terminal_output_transitions() {
        let mut state = ActivityState::new(config("claude"), tracking_for("claude"));
        let t0 = at(Duration::ZERO);
        state.on_terminal_activity(t0);
        assert_eq!(state.status(), AgentState::Running);
        state.set_tracking(ActivityTracking::USER_INPUT);
        let effects = state.on_terminal_activity(t0 + Duration::from_secs(1));
        assert!(status_effects(&effects).is_empty());
        assert_eq!(state.status(), AgentState::Running);
        assert!(
            state.idle_due.is_none(),
            "output with tracking removed cancels idle timer"
        );
    }

    #[test]
    fn user_input_in_plain_agent_shows_working_with_ten_second_idle() {
        let mut state = ActivityState::new(config("claude"), tracking_for("claude"));
        let t0 = at(Duration::ZERO);
        state.on_user_input(t0, KeyEvent::Other);
        assert_eq!(state.status(), AgentState::Running);
        assert_eq!(state.idle_deadline(), Some(t0 + Duration::from_secs(10)));
        assert_eq!(state.guard_deadline(), Some(t0 + Duration::from_secs(10)));
    }

    #[test]
    fn user_input_in_hook_agent_does_not_flip_status() {
        let mut cfg = config("claude");
        cfg.is_hook_based = true;
        let mut state = ActivityState::new(cfg, tracking_for("claude"));
        let t0 = at(Duration::ZERO);
        state.on_user_input(t0, KeyEvent::Other);
        assert_eq!(state.status(), AgentState::Idle);
        assert_eq!(state.guard_deadline(), Some(t0 + Duration::from_secs(10)));
    }

    #[test]
    fn guard_expiry_triggers_message_check() {
        let mut state = ActivityState::new(config("claude"), tracking_for("claude"));
        let t0 = at(Duration::ZERO);
        state.on_user_input(t0, KeyEvent::Other);
        let effects = state.guard_expired(t0 + Duration::from_secs(10));
        assert_eq!(state.guard_deadline(), None);
        assert!(has_effect(&effects, EffectKind::CheckMessages));
    }

    #[test]
    fn hook_awaiting_input_raises_notification() {
        let mut state = ActivityState::new(config("claude"), tracking_for("claude"));
        let t0 = at(Duration::ZERO);
        let effects = state.apply_hook_status(t0, AgentState::Input, Some("grant access?".into()));
        assert_eq!(state.status(), AgentState::Input);
        assert_eq!(
            effects,
            vec![
                Effect::Status(StatusEvent {
                    status: AgentState::Input,
                    source: ActivitySource::Hook,
                    changed_at: t0,
                }),
                Effect::AwaitingInput(Some("grant access?".into())),
            ]
        );
    }

    #[test]
    fn return_answers_prompt_and_escape_dismisses() {
        let mut state = ActivityState::new(config("claude"), tracking_for("claude"));
        let t0 = at(Duration::ZERO);
        state.apply_hook_status(t0, AgentState::Input, None);
        state.on_user_input(t0 + Duration::from_secs(1), KeyEvent::Return);
        assert_eq!(state.status(), AgentState::Running);

        let mut state = ActivityState::new(config("claude"), tracking_for("claude"));
        let t0 = at(Duration::ZERO);
        state.apply_hook_status(t0, AgentState::Input, None);
        state.on_user_input(t0 + Duration::from_secs(1), KeyEvent::Escape);
        assert_eq!(state.status(), AgentState::Idle);
    }

    #[test]
    fn idle_timers_do_not_move_awaiting_input_agent() {
        let mut state = ActivityState::new(config("claude"), tracking_for("claude"));
        let t0 = at(Duration::ZERO);
        state.apply_hook_status(t0, AgentState::Input, None);
        let effects = state.idle_timer_fired(t0 + Duration::from_secs(60));
        assert!(effects.is_empty());
        assert_eq!(state.status(), AgentState::Input);
    }

    #[test]
    fn hook_idle_overrides_local_working_and_cancels_guard() {
        let mut state = ActivityState::new(config("claude"), tracking_for("claude"));
        let t0 = at(Duration::ZERO);
        state.on_user_input(t0, KeyEvent::Other);
        assert_eq!(state.status(), AgentState::Running);
        let effects = state.apply_hook_status(t0 + Duration::from_secs(1), AgentState::Idle, None);
        assert_eq!(status_effects(&effects), vec![AgentState::Idle]);
        assert_eq!(state.guard_deadline(), None);
    }

    #[test]
    fn process_exit_sets_error_or_idle() {
        let mut state = ActivityState::new(config("claude"), tracking_for("claude"));
        let t0 = at(Duration::ZERO);
        state.on_process_exit(t0, Some(1));
        assert_eq!(state.status(), AgentState::Error);
        assert!(state.idle_due.is_none());

        let mut state = ActivityState::new(config("claude"), tracking_for("claude"));
        let t0 = at(Duration::ZERO);
        state.on_process_exit(t0, Some(0));
        assert_eq!(state.status(), AgentState::Idle);

        let mut state = ActivityState::new(config("claude"), tracking_for("claude"));
        let t0 = at(Duration::ZERO);
        state.on_process_exit(t0, None);
        assert_eq!(state.status(), AgentState::Idle);
    }

    #[test]
    fn registration_waits_for_first_idle() {
        let mut state = ActivityState::new(config("claude"), tracking_for("claude"));
        let t0 = at(Duration::ZERO);
        state.start(t0);
        state.set_registration_prompt("Register with the skwad".into());
        let effects = state.registration_ready_fired(t0 + Duration::from_secs(2));
        assert!(effects.is_empty(), "cannot inject before first idle");
        assert_eq!(state.status(), AgentState::Idle);
    }

    #[test]
    fn registration_injected_once_after_idle() {
        let mut state = ActivityState::new(config("claude"), tracking_for("claude"));
        let t0 = at(Duration::ZERO);
        state.start(t0);
        state.set_registration_prompt("Register with the skwad".into());
        state.on_terminal_activity(t0 + Duration::from_millis(100));
        let t_idle = t0 + Duration::from_secs(4);
        state.idle_timer_fired(t_idle);
        let effects = state.registration_ready_fired(t_idle + Duration::from_secs(2));
        assert_eq!(
            effects,
            vec![Effect::InjectRegistration("Register with the skwad".into())]
        );
        let effects = state.registration_ready_fired(t_idle + Duration::from_secs(3));
        assert!(effects.is_empty(), "injected only once");
    }

    #[test]
    fn registration_was_not_injected_when_guard_blocked() {
        let mut state = ActivityState::new(config("claude"), tracking_for("claude"));
        let t0 = at(Duration::ZERO);
        state.start(t0);
        state.set_registration_prompt("Register with the skwad".into());
        // Reach Idle so the ready gate opens, then type to arm the guard.
        state.on_terminal_activity(t0 + Duration::from_millis(100));
        state.idle_timer_fired(t0 + Duration::from_secs(4));
        state.on_user_input(t0 + Duration::from_secs(5), KeyEvent::Other);
        let effects = state.registration_ready_fired(t0 + Duration::from_secs(6));
        assert!(effects.is_empty(), "guard blocks injection");
        // Guard expiry re-evaluates and injects.
        let effects = state.guard_expired(t0 + Duration::from_secs(15));
        assert!(has_effect(&effects, EffectKind::InjectRegistration));
    }

    #[test]
    fn hook_fallback_timeout_used_for_hook_agents() {
        let mut cfg = config("claude");
        cfg.is_hook_based = true;
        let mut state = ActivityState::new(cfg, tracking_for("claude"));
        let t0 = at(Duration::ZERO);
        state.on_terminal_activity(t0);
        assert_eq!(state.idle_deadline(), Some(t0 + Duration::from_secs(5)));
    }

    #[test]
    fn idle_transition_records_change_time() {
        let mut state = ActivityState::new(config("claude"), tracking_for("claude"));
        let t0 = at(Duration::ZERO);
        state.on_terminal_activity(t0);
        let changed = state.changed_at();
        assert_eq!(changed, t0);
        state.mark_idle_for_test(t0 + Duration::from_secs(3));
        assert_eq!(state.changed_at(), t0 + Duration::from_secs(3));
    }

    impl ActivityState {
        fn mark_idle_for_test(&mut self, now: Instant) {
            let mut effects = Vec::new();
            self.mark_idle(now, ActivitySource::Terminal, &mut effects);
        }
    }

    #[test]
    fn shell_config_disables_registration() {
        // Shell registers inline (like the launch crate's capability table),
        // so no deferred registration is scheduled.
        let mut cfg = config("shell");
        cfg.inline_registration = true;
        let mut state = ActivityState::new(cfg, tracking_for("shell"));
        let t0 = at(Duration::ZERO);
        state.start(t0);
        assert!(state.registration_deadline().is_none());
    }
}
