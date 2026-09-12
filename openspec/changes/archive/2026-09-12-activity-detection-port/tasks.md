## 1. Crate scaffold

- [x] 1.1 Create `crates/skwad-activity` (`Cargo.toml` depending on `tokio`,
      `skwad-agents`), add it to the root workspace `[workspace.members]`, and
      verify `cargo build -p skwad-activity` succeeds. `[workspace.members]`
      uses `crates/*`, so no membership edit was needed. `skwad-agent-launch`
      and `skwad-core` are deliberately not depended on: hook capability and
      registration-prompt wiring happen at the integration boundary
      (`TrackerConfig` fields + `Tracker::set_registration_prompt`).
- [x] 1.2 Add `consts.rs` for the timing values (idle 3s, user-input idle 10s,
      hook fallback 5s, registration 1.5s/5s/0.5s), and verify the crate still
      builds.

## 2. Status and tracking presets

- [x] 2.1 Implement the tracking bitfield (`user-input`, `terminal-output`,
      both, neither) and the `tracking_for(agent_type)` preset lookup (shell ->
      none, everything else -> both), and verify unit tests for each named
      type plus an unknown type.
- [x] 2.2 Implement status-change recording: emit an `(AgentState, source,
      Instant)` event on every change, and verify a unit test asserts the
      timestamp advances and the source is recorded.

## 3. Activity and idle timer

- [x] 3.1 Implement `on_terminal_activity`: cancels a pending idle timer (even
      when terminal-output tracking is off, per the spec's runtime-downgrade
      requirement), stamps activity time, and - when tracked - sets Working
      and (re)arms a single idle timer with the idle timeout.
- [x] 3.2 Implement `on_user_input`: arms the input-protection guard for 10s;
      for non-hook agents sets Working with the 10s user-input timeout; for
      hook agents does not drive the state machine (spec: "typing in a hook
      agent does not flip status").
- [x] 3.3 Implement the idle-timer fire logic: when the timer fires, if no
      newer activity occurred within the window, go Idle; otherwise reschedule
      for the remaining interval. Verify unit tests for go-idle-after-quiet,
      late-activity-defers-idle, and the Awaiting-input guard (idle
      timers must NOT move the agent while Awaiting input).

## 4. Input-protection guard

- [x] 4.1 Implement the guard: while active, injection is suppressed and the
      injection stays queued; on guard expiry the queue is re-checked. Verify
      unit tests for message-held-while-typing and
      guard-expiry-triggers-check.
- [x] 4.2 Any hook status (Working/Idle/Awaiting input) cancels the guard
      immediately. Verify a unit test for hook-idle-cancels-guard.

## 5. Awaiting-input state and hook statuses

- [x] 5.1 Implement entering Awaiting input: an `AwaitingInput` event (with the
      hook-supplied message) and Awaiting input state; the state leaves only
      via `KeyEvent::Return` (-> Working) or `KeyEvent::Escape` (-> Idle).
      Verify unit tests for the notification signal,
      Return-answers-prompt, and Escape-dismisses-prompt.
- [x] 5.2 Implement `apply_hook_status`: applies Working/Idle/Awaiting-input
      directly as an authoritative source and cancels the guard. Verify a unit
      test for hook-idle-overrides-local-Working.

## 6. Process exit

- [x] 6.1 Implement `on_process_exit(code)`: cancels the idle timer; non-zero
      exit -> Error, zero/absent -> Idle. Verify unit tests for both cases.

## 7. Deferred registration gating

- [x] 7.1 Implement the registration schedule: a delay timer plus
      `has_become_idle` tracking; injection happens exactly once only when the
      delay has elapsed AND the agent has been Idle at least once; first-idle
      delay is long/short per `slow_startup`, subsequent idles use the short
      delay. Verify unit tests for waits-for-first-idle and
      injected-once-after-idle.
- [x] 7.2 Registration injection respects the input-protection guard, and the
      registration prompt text is supplied via `Tracker::set_registration_prompt`
      (the integration wires this to `skwad_agent_launch::registration_prompt`).
      Verify a unit test that the guard blocks the prompt and it is not lost.

## 8. Idle triggers message check

- [x] 8.1 Implement the on-idle message-check event (fires on every transition
      to Idle). Verify a unit test that becoming Idle emits the
      check-messages event.
- [x] 8.2 Cross-check every scenario in
      `openspec/specs/activity-detection/spec.md` against a test name in the
      crate and verify none are missing, then run `cargo +nightly fmt`,
      `cargo clippy -p skwad-activity --all-targets -- -D warnings`, and
      `cargo test -p skwad-activity` with all three green. Nightly fmt is not
      installed here; stable `cargo fmt` ran with no diffs.