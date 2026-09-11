## 1. Crate scaffold

- [x] 1.1 Create `crates/skwad-watch` (`Cargo.toml` with `notify`, `tokio`,
      `thiserror` workspace deps; `src/lib.rs`, `src/error.rs`,
      `src/consts.rs`) and add it to the workspace `members` list; verify
      `cargo build -p skwad-watch` succeeds.
- [x] 1.2 Add default debounce and settle-delay constants to `consts.rs`
      (git-status ~1s, generic ~0.3s, resume settle ~0.5s) per
      `Skwad/Utilities/TimingConstants.swift`; verify the module compiles
      and values are referenced from tests, not hardcoded twice.

## 2. Watch primitive

- [x] 2.1 Implement `Watch::new` taking a path, debounce `Duration`, and a
      relevance predicate `Fn(&Path) -> bool + Send + Sync`; `start()`
      spawns the `notify` watcher + debounce task, is a no-op if already
      running; verify with a unit test that a second `start()` call does
      not spawn a second task (e.g. by checking the task handle is
      unchanged).
- [x] 2.2 Implement `stop()` cancelling the debounce task and dropping the
      `notify` watcher; verify a test that events after `stop()` never
      invoke the callback.
- [x] 2.3 Implement debounce coalescing: relevant events restart the
      deadline, callback fires once after the debounce window elapses;
      verify with a real-filesystem test (`skwad-discovery`'s existing
      tests establish that `tokio::time::pause()` doesn't hold for real
      `notify` events - a real burst plus polling the callback count until
      it settles) that twenty rapid writes produce exactly one callback
      invocation, per spec scenario "Burst collapses to one callback".

## 3. Pause and resume

- [x] 3.1 Implement `pause()`/`resume()`: events observed while paused are
      dropped before the relevance check; `resume()` sets a settle-delay
      gate so events observed within the settle window after resume are
      also dropped; verify with a real-filesystem test matching spec
      scenario "Own commit does not self-trigger" (pause, write, resume,
      sleep past the settle + debounce window, confirm no callback fired),
      plus a pure unit test of the gating logic in isolation.
- [x] 3.2 Verify `pause()`/`resume()` are safe to call when the watch isn't
      running (no panic, no-op) with a unit test.

## 4. Verification

- [x] 4.1 Run `cargo +nightly fmt`, `cargo clippy -p skwad-watch --all-targets
      -- -D warnings`, `cargo test -p skwad-watch`; all pass.
- [x] 4.2 Update `crates/skwad-watch/src/lib.rs` module doc to link back to
      `openspec/specs/file-watching/spec.md` per this repo's contract-doc
      convention; verify the doc comment is present and the crate builds.
