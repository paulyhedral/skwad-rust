## Context

`skwad-activity`'s state machine already emits `Effect::AwaitingInput(message)`
on the correct transition (`openspec/specs/activity-detection/spec.md`); `main.rs`
already wires that effect to an `awaiting_input` queue and drains it once per
frame into an in-window `AwaitingNotice` toast, deduped by comparing to
`agent_selection` and the last message per agent
(`should_show_awaiting_notice`). See proposal.md - Why for what's missing:
nothing raises an actual OS notification, so a backgrounded/menu-bar-hidden
app gives no signal. `Settings.desktop_notifications_enabled`
(`settings-persistence` spec) exists and is already decoded/persisted but has
no reader anywhere in the Rust port yet.

## Goals / Non-Goals

**Goals:**
- Raise a real `UNUserNotificationCenter` notification on the same event the
  in-window toast already reacts to, reusing its existing dedup signal where
  possible instead of inventing a second one.
- Click-to-navigate parity with the Swift reference.

**Non-Goals:**
- Changing or removing the existing in-window `AwaitingNotice` toast — it
  serves the foreground case (app visible but a different agent selected)
  and this change doesn't touch it.
- Any notification type beyond Awaiting-input (Swift's `NotificationService`
  itself only has the one call site: `notifyAwaitingInput`).
- Linux/Windows notification backends — this port targets macOS only
  (`CLAUDE.md`: "NEVER build or develop for Windows unless explicitly
  instructed"; the workspace has no non-macOS target today).

## Decisions

- **Bind `UNUserNotificationCenter` directly via `objc2` + `objc2-user-notifications`
  (0.3.2), not a cross-platform notification crate.** Both crates are already
  present in `Cargo.lock` at this exact version as a transitive dependency of
  `gpui-pre-macos` (verified via `cargo tree -i objc2-user-notifications`), so
  adding them as a direct dependency of the new module changes no resolved
  version — zero new supply-chain surface, and it's the same framework the
  Swift reference uses, keeping behavior (including permission-prompt wording
  users have already seen from Skwad) identical. A cross-platform crate
  (`notify-rust` et al.) would add a new dependency for a platform this port
  doesn't target and typically uses the legacy `NSUserNotification` API on
  macOS, which Apple deprecated in favor of `UserNotifications`.

- **New `skwad-notifications` crate, not a module inside `skwad`.** The
  binding needs a delegate object (`UNUserNotificationCenterDelegate`
  equivalent) that must be `'static` and independently testable for its pure
  logic (dedup/suppression predicates), matching how `skwad-activity` and
  `skwad-messaging` already split platform-adjacent logic out of `main.rs`.
  The actual `objc2` calls are a thin, mostly-untestable shell around a
  small set of pure functions that carry the real behavior — those functions
  are what's unit tested, per this project's "no panics in library code, no
  untested branches" convention.

- **Reuse `should_show_awaiting_notice`'s suppression logic for the OS
  notification's visible-agent check, rather than a separate predicate.**
  Both the in-window toast and the OS notification are suppressing on the
  identical condition (agent not currently selected). The existing function
  already lives in `main.rs`; the new wiring calls it before raising the OS
  notification instead of duplicating its logic. The "already Awaiting
  input" dedup is a separate, narrower condition (repeat hook events for an
  agent already in that state) — checked against the live `AgentStore`
  state, not the toast's own last-message cache, so it stays correct even if
  the toast's dedup state is cleared or diverges for any reason.

- **Delegate lives for the process lifetime, held via the same `Arc`-based
  ownership pattern as `QueuedNotifier`.** `UNUserNotificationCenter.current()`
  is a process-wide singleton on the Apple side; the Rust delegate just needs
  to outlive it, which a static/leaked `Arc` (or GPUI's existing app-lifetime
  entity ownership) already guarantees for other long-lived singletons in
  this codebase.

## Risks / Trade-offs

- [`objc2-user-notifications`'s API surface could differ enough from the
  Swift `UNUserNotificationCenter` calls this design assumes] → Mitigation:
  confirm the exact method names/signatures against the crate's docs during
  implementation (task 1 in tasks.md) before writing the delegate; the crate
  is a near-1:1 binding of the same framework, so risk is low but not zero.
- [Click-to-navigate requires routing an async delegate callback back into
  GPUI's app/entity update cycle, which runs on GPUI's own executor, not an
  arbitrary background thread] → Mitigation: follow the same
  channel/queue-and-drain pattern already used for `awaiting_input` and
  `notifier` (a plain `Mutex<Vec<_>>` drained each frame) rather than trying
  to call into GPUI directly from the delegate callback.
- [Notification permission may be denied by the user, silently dropping all
  future notifications] → Mitigation: this is Apple's own UX, unchanged from
  the Swift app; no in-app fallback is proposed, matching the reference.
