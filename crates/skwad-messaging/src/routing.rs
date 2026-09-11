use skwad_agents::{Agent, AgentState};
use uuid::Uuid;

use crate::error::{Result, SendError};
use crate::message::Message;
use crate::notify::DeliveryNotifier;
use crate::store::MessageStore;

/// The routing checks `send` and `broadcast` both apply per recipient:
/// shell agents can't receive, and companion ownership must match in both
/// directions.
fn eligible(sender: &Agent, recipient: &Agent) -> Result<()> {
    if recipient.is_shell() {
        return Err(SendError::ShellRecipient);
    }
    if recipient.is_companion && recipient.created_by != Some(sender.id) {
        return Err(SendError::NotCompanionOwner);
    }
    if sender.is_companion && Some(recipient.id) != sender.created_by {
        return Err(SendError::CompanionNotOwner);
    }
    Ok(())
}

fn notify_if_idle(notifier: &dyn DeliveryNotifier, recipient: &Agent, message_id: Uuid) {
    if recipient.state == AgentState::Idle {
        notifier.notify(recipient.id, message_id);
    }
}

/// Sends `content` from `sender` to `recipient_id`. `workspace_members` is
/// every agent (including `sender`) in the sender's workspace - the only
/// pool `recipient_id` may be found in, per the spec's workspace scoping.
/// Returns the new message's id on success.
pub fn send(
    store: &mut MessageStore,
    notifier: &dyn DeliveryNotifier,
    sender: &Agent,
    workspace_members: &[Agent],
    recipient_id: Uuid,
    content: impl Into<String>,
) -> Result<Uuid> {
    if !sender.is_registered {
        return Err(SendError::SenderNotRegistered);
    }

    let recipient = workspace_members
        .iter()
        .find(|a| a.id == recipient_id)
        .ok_or(SendError::RecipientNotFound)?;

    eligible(sender, recipient)?;

    let message = Message::new(sender.id, recipient_id, content);
    let message_id = message.id;
    notify_if_idle(notifier, recipient, message_id);
    store.add(message);

    Ok(message_id)
}

/// Fans `content` out from `sender` to every eligible recipient in
/// `workspace_members` (which includes `sender`). Returns the number of
/// messages created; 0 covers both an unregistered sender and a workspace
/// with no eligible recipient.
pub fn broadcast(
    store: &mut MessageStore,
    notifier: &dyn DeliveryNotifier,
    sender: &Agent,
    workspace_members: &[Agent],
    content: impl Into<String>,
) -> usize {
    if !sender.is_registered {
        return 0;
    }
    let content = content.into();

    let mut count = 0;
    for recipient in workspace_members {
        if recipient.id == sender.id || !recipient.is_registered {
            continue;
        }
        if eligible(sender, recipient).is_err() {
            continue;
        }

        let message = Message::new(sender.id, recipient.id, content.clone());
        notify_if_idle(notifier, recipient, message.id);
        store.add(message);
        count += 1;
    }

    count
}

/// Returns `agent_id`'s unread messages, marking them read unless
/// `mark_as_read` is false.
pub fn check(store: &mut MessageStore, agent_id: Uuid, mark_as_read: bool) -> Vec<Message> {
    let unread: Vec<Message> = store.unread_for(agent_id).into_iter().cloned().collect();
    if mark_as_read {
        store.mark_read(agent_id);
    }
    unread
}

#[cfg(test)]
mod tests {
    use skwad_agents::AgentState;

    use super::*;
    use crate::notify::RecordingNotifier;

    fn agent(name: &str) -> Agent {
        let mut a = test_agent_defaults();
        a.name = name.to_string();
        a
    }

    fn test_agent_defaults() -> Agent {
        // Every field is public on `Agent`; build directly rather than
        // going through `AgentStore::create`, which always yields an
        // unregistered, `Idle` agent and has no public mutator for either
        // field.
        Agent {
            id: Uuid::new_v4(),
            name: String::new(),
            avatar: String::new(),
            folder: "/tmp/a".to_string(),
            agent_type: "claude".to_string(),
            created_by: None,
            is_companion: false,
            shell_command: None,
            persona_id: None,
            state: AgentState::Idle,
            status_text: String::new(),
            is_registered: true,
            is_pending_start: false,
            terminal_title: String::new(),
            restart_token: Uuid::new_v4(),
            session_id: None,
            resume_session_id: None,
            fork_session: false,
            metadata: Default::default(),
        }
    }

    fn shell(owner: Uuid) -> Agent {
        let mut a = agent("shell");
        a.agent_type = "shell".to_string();
        a.is_companion = true;
        a.created_by = Some(owner);
        a
    }

    fn companion(owner: Uuid) -> Agent {
        let mut a = agent("companion");
        a.is_companion = true;
        a.created_by = Some(owner);
        a
    }

    #[test]
    fn unregistered_sender_is_rejected() {
        let mut a = agent("a");
        a.is_registered = false;
        let b = agent("b");
        let members = vec![a.clone(), b.clone()];
        let mut store = MessageStore::new();
        let notifier = RecordingNotifier::new();

        let result = send(&mut store, &notifier, &a, &members, b.id, "hi");

        assert_eq!(result, Err(SendError::SenderNotRegistered));
    }

    #[test]
    fn recipient_outside_workspace_is_not_found() {
        let a = agent("a");
        let b = agent("b"); // not in `members`
        let members = vec![a.clone()];
        let mut store = MessageStore::new();
        let notifier = RecordingNotifier::new();

        let result = send(&mut store, &notifier, &a, &members, b.id, "hi");

        assert_eq!(result, Err(SendError::RecipientNotFound));
    }

    #[test]
    fn direct_send_to_shell_agent_is_rejected() {
        let a = agent("a");
        let shell = shell(a.id);
        let members = vec![a.clone(), shell.clone()];
        let mut store = MessageStore::new();
        let notifier = RecordingNotifier::new();

        let result = send(&mut store, &notifier, &a, &members, shell.id, "hi");

        assert_eq!(result, Err(SendError::ShellRecipient));
    }

    #[test]
    fn owner_messages_its_companion() {
        let owner = agent("owner");
        let comp = companion(owner.id);
        let members = vec![owner.clone(), comp.clone()];
        let mut store = MessageStore::new();
        let notifier = RecordingNotifier::new();

        let result = send(&mut store, &notifier, &owner, &members, comp.id, "hi");

        assert!(result.is_ok());
    }

    #[test]
    fn third_party_messages_a_companion() {
        let owner = agent("owner");
        let other = agent("other");
        let comp = companion(owner.id);
        let members = vec![owner.clone(), other.clone(), comp.clone()];
        let mut store = MessageStore::new();
        let notifier = RecordingNotifier::new();

        let result = send(&mut store, &notifier, &other, &members, comp.id, "hi");

        assert_eq!(result, Err(SendError::NotCompanionOwner));
    }

    #[test]
    fn companion_can_only_message_its_owner() {
        let owner = agent("owner");
        let other = agent("other");
        let comp = companion(owner.id);
        let members = vec![owner.clone(), other.clone(), comp.clone()];
        let mut store = MessageStore::new();
        let notifier = RecordingNotifier::new();

        let result = send(&mut store, &notifier, &comp, &members, other.id, "hi");

        assert_eq!(result, Err(SendError::CompanionNotOwner));
    }

    #[test]
    fn idle_recipient_is_notified() {
        let a = agent("a");
        let b = agent("b");
        let members = vec![a.clone(), b.clone()];
        let mut store = MessageStore::new();
        let notifier = RecordingNotifier::new();

        let message_id = send(&mut store, &notifier, &a, &members, b.id, "hi").unwrap();

        assert_eq!(notifier.calls(), vec![(b.id, message_id)]);
    }

    #[test]
    fn busy_recipient_is_not_notified() {
        let a = agent("a");
        let mut b = agent("b");
        b.state = AgentState::Running;
        let members = vec![a.clone(), b.clone()];
        let mut store = MessageStore::new();
        let notifier = RecordingNotifier::new();

        send(&mut store, &notifier, &a, &members, b.id, "hi").unwrap();

        assert!(notifier.calls().is_empty());
        assert!(store.has_unread(b.id));
    }

    #[test]
    fn check_clears_unread_by_default() {
        let a = agent("a");
        let b = agent("b");
        let members = vec![a.clone(), b.clone()];
        let mut store = MessageStore::new();
        let notifier = RecordingNotifier::new();
        send(&mut store, &notifier, &a, &members, b.id, "hi").unwrap();

        let unread = check(&mut store, b.id, true);

        assert_eq!(unread.len(), 1);
        assert!(!store.has_unread(b.id));
    }

    #[test]
    fn non_destructive_check_leaves_unread_flags() {
        let a = agent("a");
        let b = agent("b");
        let members = vec![a.clone(), b.clone()];
        let mut store = MessageStore::new();
        let notifier = RecordingNotifier::new();
        send(&mut store, &notifier, &a, &members, b.id, "hi").unwrap();

        check(&mut store, b.id, false);

        assert!(store.has_unread(b.id));
    }

    #[test]
    fn broadcast_reaches_eligible_workspace_members_only() {
        let sender = agent("sender");
        let peer1 = agent("peer1");
        let peer2 = agent("peer2");
        let shell = shell(sender.id);
        let members = vec![sender.clone(), peer1.clone(), peer2.clone(), shell.clone()];
        let mut store = MessageStore::new();
        let notifier = RecordingNotifier::new();

        let count = broadcast(&mut store, &notifier, &sender, &members, "hi");

        assert_eq!(count, 2);
    }

    #[test]
    fn broadcast_to_a_busy_recipient_stores_without_notifying() {
        let sender = agent("sender");
        let mut peer = agent("peer");
        peer.state = AgentState::Running;
        let members = vec![sender.clone(), peer.clone()];
        let mut store = MessageStore::new();
        let notifier = RecordingNotifier::new();

        let count = broadcast(&mut store, &notifier, &sender, &members, "hi");

        assert_eq!(count, 1);
        assert!(notifier.calls().is_empty());
        assert!(store.has_unread(peer.id));
    }
}
