## Context

`knot-activity`'s state machine already emits `Effect::AwaitingInput(message)`
on the correct transition (`openspec/specs/activity-detection/spec.md`); `main.rs`
already wires that effect to an `awaiting_input` queue and drains it once per
frame into an in-window `AwaitingNotice` toast, deduped by comparing to
`agent_selection` and the last message per agent
(`should_show_awaiting_notice`). See proposal.md - Why for what's missing:
nothing raises an actual OS notification, so a backgrounded/menu-bar-hidden
app gives no signal. `Settings.desktop_notifications_enabled`
(`settings-persistence` spec) exists and is already decoded/persisted but has
no reader anywhere in the Rust port yet.

`gpui` (via `gpui-kit`, already the UI toolkit for this port) turns out to
already wrap this exact feature: `App::show_system_notification`,
`App::dismiss_system_notification`, and `App::on_system_notification_response`
(`gpui`'s `platform.rs`/`app.rs`). Its macOS backend
(`gpui-pre-macos::system_notifications`) is a full `UNUserNotificationCenter`
implementation — lazy authorization (nothing touches the notification center
until the first `show_system_notification` call, avoiding the
not-in-a-bundle abort in dev builds), a retained delegate handling
`willPresent`/`didReceive`, and category/action-button registration. This
was discovered by inspecting the vendored source for
`objc2-user-notifications` while implementing the originally-proposed direct
binding, and changes the design below substantially from an earlier draft
that proposed binding `UNUserNotificationCenter` directly via `objc2`.

## Goals / Non-Goals

**Goals:**
- Raise a real OS notification on the same event the in-window toast already
  reacts to, reusing its existing dedup signal where possible instead of
  inventing a second one.
- Click-to-navigate parity with the Swift reference.

**Non-Goals:**
- Changing or removing the existing in-window `AwaitingNotice` toast — it
  serves the foreground case (app visible but a different agent selected)
  and this change doesn't touch it.
- Any notification type beyond Awaiting-input (Swift's `NotificationService`
  itself only has the one call site: `notifyAwaitingInput`).
- A new crate, or any direct `objc2`/`UNUserNotificationCenter` binding —
  `gpui::App` already exposes this as a platform-neutral capability; adding
  either would duplicate code `gpui-kit` already ships and links.

## Decisions

- **Use `gpui::App::show_system_notification` / `on_system_notification_response`
  / `dismiss_system_notification` directly, not a new `knot-notifications`
  crate or a direct `objc2` binding.** `gpui-pre-macos`'s
  `system_notifications` module is already a complete, lazily-initialized
  `UNUserNotificationCenter` wrapper (delegate, authorization, category/action
  registration) linked into every `knot` build via `gpui-kit`. Reimplementing
  any part of that — a custom `objc2` delegate class, a second authorization
  request, a second permission-prompt UX — would be dead weight duplicating
  code already in the dependency tree, and would risk two competing
  `UNUserNotificationCenter` delegates on the same process-wide singleton
  (`UNUserNotificationCenter.current()` has exactly one `delegate` slot; gpui
  already claims it). This also makes the port more correct than a
  macOS-only `objc2` binding would have been: `gpui::SystemNotification` is
  cross-platform by construction, so this code needs no `#[cfg(target_os =
  "macos")]` and keeps working if the port ever gains another target.

- **No new crate.** With the mechanism reduced to two `cx.app()` calls (show
  on entering Awaiting input, and a response callback registered once at
  startup) plus small pure suppression/body-text functions, there's nothing
  here that warrants a crate boundary — it's a few functions alongside
  `should_show_awaiting_notice`, `delivery_notice`, and the other pure
  helpers `main.rs` already keeps next to its GPUI wiring.

- **Reuse `should_show_awaiting_notice`'s suppression logic for the OS
  notification's visible-agent check, rather than a separate predicate.**
  Both the in-window toast and the OS notification suppress on the identical
  condition (agent not currently selected). The new `should_notify` function
  composes that existing check with the setting flag and the
  already-Awaiting-input repeat-event check, rather than duplicating the
  selection comparison.

- **Tag each `SystemNotification` with the agent's id (as a string) and
  match on that in the response callback**, rather than trying to smuggle
  richer state through gpui's `tag: SharedString`. `SystemNotificationResponse`
  only round-trips the tag and an optional `action_id`; the agent id is all
  that's needed to look the agent up in the store and select it, and reusing
  it as the tag also gives "posting a new notification with the same tag
  replaces the previous one" (gpui's documented behavior) for free — a
  second Awaiting-input notification for the same still-unanswered agent
  replaces rather than stacks.

## Risks / Trade-offs

- [`gpui::App::show_system_notification` is a no-op on any platform/config
  where delivery is unavailable (e.g. authorization denied, or a platform
  gpui doesn't yet support) — silently, per its own doc comment] →
  Mitigation: this matches the Swift reference's own behavior exactly (no
  in-app fallback there either); no additional handling proposed.
- [The response callback registered via `on_system_notification_response`
  is process-global (one callback slot on `App`), so a later, unrelated
  system-notification feature would need to compose with this one rather
  than register its own] → Mitigation: not a concern for this change (the
  only system notification `knot` raises is this one), noted here so a
  future addition doesn't silently clobber this callback.
