use std::time::Duration;

/// Debounce for the git-status watch (`GitFileWatcher.gitFileWatcherDebounce`).
pub const GIT_STATUS_DEBOUNCE: Duration = Duration::from_secs(1);

/// Debounce for a generic single-file watch (`FileWatcher.fileWatcherDebounce`).
pub const GENERIC_DEBOUNCE: Duration = Duration::from_millis(300);

/// Settle delay after `resume()` before events are honored again
/// (`TimingConstants.gitFileWatcherResume`).
pub const RESUME_SETTLE: Duration = Duration::from_millis(500);
