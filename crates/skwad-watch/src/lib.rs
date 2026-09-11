//! A generic debounced directory watch with pause/resume, used to suppress
//! self-inflicted filesystem events (e.g. the app's own git writes) around a
//! watch's callback.
//!
//! Contract: `openspec/specs/file-watching/spec.md`.
//!
//! The relevance filter for the repo-discovery source-folder watch lives in
//! `skwad-discovery` and is unrelated to this crate; [`Watch`] takes its
//! relevance predicate from the caller instead.

pub mod consts;
pub mod error;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use notify::{RecommendedWatcher, RecursiveMode, Watcher as _, recommended_watcher};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::{Instant, sleep_until};

pub use error::{Result, WatchError};

type RelevantFn = dyn Fn(&Path) -> bool + Send + Sync;
type CallbackFn = dyn Fn() + Send + Sync;

/// A debounced watch on a directory tree, with pause/resume around the
/// caller's own writes to the watched tree.
///
/// Starting an already-running watch is a no-op; stopping cancels any
/// pending debounced callback.
pub struct Watch {
    path: PathBuf,
    debounce: Duration,
    relevant: Arc<RelevantFn>,
    callback: Arc<CallbackFn>,
    active: Mutex<Option<Active>>,
}

struct Active {
    watcher: RecommendedWatcher,
    task: JoinHandle<()>,
    pause: Arc<Mutex<PauseState>>,
}

#[derive(Default)]
struct PauseState {
    paused: bool,
    resume_at: Option<Instant>,
}

impl Watch {
    /// Create a watch. `relevant` decides whether a changed path should count
    /// toward the debounce; `callback` fires once after changes settle.
    pub fn new(
        path: impl Into<PathBuf>,
        debounce: Duration,
        relevant: impl Fn(&Path) -> bool + Send + Sync + 'static,
        callback: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        Self {
            path: path.into(),
            debounce,
            relevant: Arc::new(relevant),
            callback: Arc::new(callback),
            active: Mutex::new(None),
        }
    }

    /// Start watching. A no-op if already running.
    pub fn start(&self) -> Result<()> {
        let mut guard = self.active.lock().unwrap();
        if guard.is_some() {
            return Ok(());
        }

        let (evt_tx, evt_rx) = mpsc::unbounded_channel();
        let mut watcher: RecommendedWatcher = recommended_watcher(move |res| {
            if let Ok(event) = res {
                let _ = evt_tx.send(event);
            }
        })?;
        watcher.watch(&self.path, RecursiveMode::Recursive)?;

        let pause = Arc::new(Mutex::new(PauseState::default()));
        let task = tokio::spawn(watch_loop(
            evt_rx,
            self.debounce,
            self.relevant.clone(),
            self.callback.clone(),
            pause.clone(),
        ));
        *guard = Some(Active {
            watcher,
            task,
            pause,
        });
        Ok(())
    }

    /// Stop watching, cancelling any pending debounced callback.
    pub fn stop(&self) {
        if let Some(active) = self.active.lock().unwrap().take() {
            active.task.abort();
            drop(active.watcher);
        }
    }

    /// Suppress events until [`Self::resume`] is called. A no-op if not running.
    pub fn pause(&self) {
        if let Some(active) = self.active.lock().unwrap().as_ref() {
            let mut state = active.pause.lock().unwrap();
            state.paused = true;
            state.resume_at = None;
        }
    }

    /// Resume watching. Events within [`consts::RESUME_SETTLE`] of the resume
    /// call are still dropped, so writes still landing on disk from before the
    /// resume don't self-trigger the callback. A no-op if not running.
    pub fn resume(&self) {
        if let Some(active) = self.active.lock().unwrap().as_ref() {
            let mut state = active.pause.lock().unwrap();
            state.paused = false;
            state.resume_at = Some(Instant::now() + consts::RESUME_SETTLE);
        }
    }
}

async fn watch_loop(
    mut events: mpsc::UnboundedReceiver<notify::Event>,
    debounce: Duration,
    relevant: Arc<RelevantFn>,
    callback: Arc<CallbackFn>,
    pause: Arc<Mutex<PauseState>>,
) {
    let mut deadline: Option<Instant> = None;

    loop {
        tokio::select! {
            event = events.recv() => {
                match event {
                    Some(event) => {
                        if is_honored(&pause) && !matches!(event.kind, notify::EventKind::Access(_))
                            && event.paths.iter().any(|p| relevant(p))
                        {
                            deadline = Some(Instant::now() + debounce);
                        }
                    }
                    None => return,
                }
            }
            _ = async { sleep_until(deadline.unwrap()).await }, if deadline.is_some() => {
                deadline = None;
                callback();
            }
        }
    }
}

/// Whether an event should be considered at all: not paused, and past any
/// post-resume settle window.
fn is_honored(pause: &Arc<Mutex<PauseState>>) -> bool {
    let mut state = pause.lock().unwrap();
    if state.paused {
        return false;
    }
    match state.resume_at {
        Some(resume_at) if Instant::now() < resume_at => false,
        Some(_) => {
            state.resume_at = None;
            true
        }
        None => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration as StdDuration;
    use tokio::time::timeout;

    fn count_callback() -> (Arc<AtomicUsize>, impl Fn() + Send + Sync + 'static) {
        let count = Arc::new(AtomicUsize::new(0));
        let counted = count.clone();
        (count, move || {
            counted.fetch_add(1, Ordering::SeqCst);
        })
    }

    /// Poll `count` until it stops changing for `quiet`, bounded by an overall
    /// deadline so a watch that never settles fails fast rather than hanging.
    async fn settle(count: &AtomicUsize, quiet: StdDuration) -> usize {
        let overall = timeout(StdDuration::from_secs(10), async {
            loop {
                let before = count.load(Ordering::SeqCst);
                tokio::time::sleep(quiet).await;
                if count.load(Ordering::SeqCst) == before {
                    return before;
                }
            }
        })
        .await;
        overall.expect("settle() did not go quiet within 10s")
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn start_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let (_count, cb) = count_callback();
        let watch = Watch::new(dir.path(), StdDuration::from_millis(50), |_| true, cb);

        watch.start().unwrap();
        let first_pause = Arc::as_ptr(&watch.active.lock().unwrap().as_ref().unwrap().pause);
        watch.start().unwrap();
        let second_pause = Arc::as_ptr(&watch.active.lock().unwrap().as_ref().unwrap().pause);

        assert_eq!(
            first_pause, second_pause,
            "second start() replaced the running watch"
        );
        watch.stop();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn events_after_stop_never_invoke_callback() {
        let dir = tempfile::tempdir().unwrap();
        let (count, cb) = count_callback();
        let watch = Watch::new(dir.path(), StdDuration::from_millis(100), |_| true, cb);
        watch.start().unwrap();
        watch.stop();

        fs::write(dir.path().join("after-stop.txt"), "x").unwrap();
        tokio::time::sleep(StdDuration::from_millis(500)).await;
        assert_eq!(
            count.load(Ordering::SeqCst),
            0,
            "events after stop() must not invoke the callback"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn burst_collapses_to_one_callback() {
        let dir = tempfile::tempdir().unwrap();
        let (count, cb) = count_callback();
        let watch = Watch::new(dir.path(), StdDuration::from_millis(300), |_| true, cb);
        watch.start().unwrap();

        for i in 0..20 {
            fs::write(dir.path().join(format!("f{i}.txt")), "x").unwrap();
        }

        let fired = settle(&count, StdDuration::from_millis(500)).await;
        assert_eq!(fired, 1, "expected the burst to collapse to one callback");
        watch.stop();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn pause_resume_noop_when_not_running() {
        let dir = tempfile::tempdir().unwrap();
        let (_count, cb) = count_callback();
        let watch = Watch::new(dir.path(), StdDuration::from_millis(50), |_| true, cb);

        watch.pause();
        watch.resume();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn own_commit_does_not_self_trigger() {
        let dir = tempfile::tempdir().unwrap();
        let (count, cb) = count_callback();
        let watch = Watch::new(dir.path(), StdDuration::from_millis(100), |_| true, cb);
        watch.start().unwrap();

        watch.pause();
        fs::write(dir.path().join("index"), "own write").unwrap();
        watch.resume();

        // Past the settle window, past the debounce window: any event from the
        // paused write must not have survived to fire a callback.
        tokio::time::sleep(consts::RESUME_SETTLE + StdDuration::from_millis(300)).await;
        assert_eq!(
            count.load(Ordering::SeqCst),
            0,
            "own write during pause must not self-trigger after resume"
        );
        watch.stop();
    }

    #[test]
    fn is_honored_drops_paused_and_settling_events() {
        let pause = Arc::new(Mutex::new(PauseState::default()));
        {
            let mut state = pause.lock().unwrap();
            state.paused = true;
        }
        assert!(!is_honored(&pause), "events while paused must be dropped");

        {
            let mut state = pause.lock().unwrap();
            state.paused = false;
            state.resume_at = Some(Instant::now() + StdDuration::from_secs(60));
        }
        assert!(
            !is_honored(&pause),
            "events within the settle window after resume must be dropped"
        );
    }
}
