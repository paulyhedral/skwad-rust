use std::path::{Path, PathBuf};
use std::sync::Mutex;

use notify::{RecommendedWatcher, RecursiveMode, Watcher, recommended_watcher};
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;
use tokio::time::{Instant, sleep_until};

use crate::consts::{DEBOUNCE, GIT_DIR};
use crate::error::Result;
use crate::scan::{RepoInfo, scan};

/// Coordinates repository discovery for a single source folder: an initial
/// scan, a non-recursive watch on the folder, and a debounced rescan that
/// coalesces bursts. Results are published on a [`watch`] channel.
pub struct Discovery {
    tx: watch::Sender<Vec<RepoInfo>>,
    active: Mutex<Option<ActiveWatch>>,
}

/// The running watch for the current source folder. `watcher` is held here
/// rather than inside the spawned task so replacing it is a plain, synchronous
/// drop: the OS-level watch closes before this function returns, instead of
/// waiting on `JoinHandle::abort`, which only requests cancellation and gives
/// no guarantee the old watch is gone before the caller's next filesystem
/// write - a real, not just CI-timing, "stale event after switch" race.
struct ActiveWatch {
    watcher: RecommendedWatcher,
    task: JoinHandle<()>,
}

impl Discovery {
    /// Create a discovery coordinator and the receiver its results are
    /// published on. The receiver starts holding an empty list.
    pub fn new() -> (Self, watch::Receiver<Vec<RepoInfo>>) {
        let (tx, rx) = watch::channel(Vec::new());
        let discovery = Self {
            tx,
            active: Mutex::new(None),
        };
        (discovery, rx)
    }

    /// Point discovery at `path`, replacing any current watch. `None`, an empty
    /// path, a missing path, or a non-directory publishes an empty list and
    /// starts no watch. Otherwise the folder is scanned once, the result
    /// published, and a debounced watch started.
    pub fn set_source_folder(&self, path: Option<PathBuf>) -> Result<()> {
        if let Some(old) = self.active.lock().unwrap().take() {
            old.task.abort();
            drop(old.watcher);
        }

        let base = match path {
            Some(p) if !p.as_os_str().is_empty() && p.is_dir() => p,
            _ => {
                let _ = self.tx.send(Vec::new());
                return Ok(());
            }
        };
        let base = base.canonicalize().unwrap_or(base);

        let _ = self.tx.send(scan(&base));

        let (evt_tx, evt_rx) = mpsc::unbounded_channel();
        let mut watcher: RecommendedWatcher = recommended_watcher(move |res| {
            if let Ok(event) = res {
                let _ = evt_tx.send(event);
            }
        })?;
        watcher.watch(&base, RecursiveMode::NonRecursive)?;

        let task = tokio::spawn(watch_loop(evt_rx, base, self.tx.clone()));
        *self.active.lock().unwrap() = Some(ActiveWatch { watcher, task });
        Ok(())
    }
}

/// Debounces relevant filesystem events into rescans. `EventKind::Access` is
/// filtered out before path-relevance is even checked: Linux's inotify
/// backend watches `WatchMask::OPEN` by default, so every rescan's own
/// `scan()` opening `base` to read it back generates an Access event on
/// `base` - which `is_relevant` would otherwise treat as always-relevant,
/// restarting the debounce forever. macOS's FSEvents backend doesn't report
/// plain opens this way, so this feedback loop is Linux-only.
async fn watch_loop(
    mut events: mpsc::UnboundedReceiver<notify::Event>,
    base: PathBuf,
    tx: watch::Sender<Vec<RepoInfo>>,
) {
    let mut deadline: Option<Instant> = None;

    loop {
        tokio::select! {
            event = events.recv() => {
                match event {
                    Some(event) => {
                        if !matches!(event.kind, notify::EventKind::Access(_))
                            && event.paths.iter().any(|p| is_relevant(p, &base))
                        {
                            deadline = Some(Instant::now() + DEBOUNCE);
                        }
                    }
                    None => return,
                }
            }
            _ = async { sleep_until(deadline.unwrap()).await }, if deadline.is_some() => {
                deadline = None;
                let base = base.clone();
                let Ok(repos) = tokio::task::spawn_blocking(move || scan(&base)).await else {
                    return;
                };
                if tx.send(repos).is_err() {
                    return;
                }
            }
        }
    }
}

/// A change is relevant when it lands on the base folder itself, a direct child
/// (a repo folder appearing or disappearing), or a child's `.git` entry.
/// Deeper working-tree churn is ignored.
fn is_relevant(path: &Path, base: &Path) -> bool {
    let Ok(rel) = path.strip_prefix(base) else {
        return false;
    };
    let mut components = rel.components();
    let Some(first) = components.next() else {
        return true;
    };
    match components.next() {
        None => true,
        Some(second) => {
            components.next().is_none()
                && second.as_os_str() == GIT_DIR
                && first.as_os_str() != GIT_DIR
        }
    }
}

#[cfg(test)]
mod tests {
    use super::is_relevant;
    use std::path::Path;

    #[test]
    fn base_and_direct_child_and_git_are_relevant() {
        let base = Path::new("/src");
        assert!(is_relevant(Path::new("/src"), base));
        assert!(is_relevant(Path::new("/src/app"), base));
        assert!(is_relevant(Path::new("/src/app/.git"), base));
    }

    #[test]
    fn deep_worktree_change_is_not_relevant() {
        let base = Path::new("/src");
        assert!(!is_relevant(Path::new("/src/app/src/main.rs"), base));
        assert!(!is_relevant(Path::new("/src/app/.git/index"), base));
        assert!(!is_relevant(Path::new("/other/app"), base));
    }
}
