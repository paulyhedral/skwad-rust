//! Activity-tracking presets: which terminal signals drive the state machine.

/// A set of terminal activity sources that drive status changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActivityTracking(u8);

impl ActivityTracking {
    /// Set when terminal output is observed.
    pub const TERMINAL_OUTPUT: Self = Self(1 << 0);

    /// Set when a user keystroke is observed.
    pub const USER_INPUT: Self = Self(1 << 1);

    /// No sources tracked; the agent stays Idle (shell agents).
    pub const NONE: Self = Self(0);

    /// Both sources tracked.
    pub const ALL: Self = Self(0b11);

    /// Whether `other` is a subset of this set.
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Whether no sources are tracked.
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Add `other` to this set.
    pub const fn insert(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Remove `other` from this set.
    pub const fn remove(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }
}

impl Default for ActivityTracking {
    fn default() -> Self {
        Self::ALL
    }
}

/// Tracking preset for `agent_type`: shell agents track neither terminal
/// output nor user input; all other agents track both.
pub fn tracking_for(agent_type: &str) -> ActivityTracking {
    if agent_type == "shell" {
        ActivityTracking::NONE
    } else {
        ActivityTracking::ALL
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_tracks_nothing() {
        assert_eq!(tracking_for("shell"), ActivityTracking::NONE);
        assert!(tracking_for("shell").is_empty());
    }

    #[test]
    fn non_shell_tracks_both() {
        for t in ["claude", "codex", "opencode", "gemini"] {
            assert_eq!(tracking_for(t), ActivityTracking::ALL);
        }
    }

    #[test]
    fn unknown_agent_type_tracks_both() {
        assert_eq!(tracking_for("unknown-type"), ActivityTracking::ALL);
    }

    #[test]
    fn contains_and_bits() {
        let user_only = ActivityTracking::ALL.remove(ActivityTracking::TERMINAL_OUTPUT);
        assert_eq!(user_only, ActivityTracking::USER_INPUT);
        assert!(!user_only.contains(ActivityTracking::TERMINAL_OUTPUT));
        assert!(user_only.contains(ActivityTracking::USER_INPUT));
        assert_eq!(
            user_only.insert(ActivityTracking::TERMINAL_OUTPUT),
            ActivityTracking::ALL
        );
    }
}
