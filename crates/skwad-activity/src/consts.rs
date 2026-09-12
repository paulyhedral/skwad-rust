//! Timing constants for the activity tracker. Values match the Swift
//! reference (`Skwad/Utilities/TimingConstants.swift`).

use std::time::Duration;

/// Idle timeout after terminal output for non-hook agents.
pub const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_secs(3);

/// Idle timeout after a user keystroke, and the input-protection window.
pub const USER_INPUT_IDLE_TIMEOUT: Duration = Duration::from_secs(10);

/// Terminal-output idle timeout for hook-managed agents.
pub const HOOK_FALLBACK_IDLE_TIMEOUT: Duration = Duration::from_secs(5);

/// Registration delay after the first idle for fast-starting agents.
pub const REGISTRATION_FIRST_IDLE_DELAY_SHORT: Duration = Duration::from_millis(1500);

/// Registration delay after the first idle for slow-starting agents.
pub const REGISTRATION_FIRST_IDLE_DELAY_LONG: Duration = Duration::from_secs(5);

/// Registration delay after subsequent idles.
pub const REGISTRATION_SUBSEQUENT_IDLE_DELAY: Duration = Duration::from_millis(500);
