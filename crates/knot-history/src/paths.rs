use std::path::PathBuf;

/// The current user's home directory, or `None` when it can't be resolved
/// (in which case every provider degrades to an empty result).
pub fn home_dir() -> Option<PathBuf> {
    directories::UserDirs::new().map(|dirs| dirs.home_dir().to_path_buf())
}
