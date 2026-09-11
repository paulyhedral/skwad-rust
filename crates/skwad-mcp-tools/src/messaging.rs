use skwad_agents::AgentStore;
use skwad_mcp::ToolCallResult;
use skwad_messaging::{DeliveryNotifier, MessageStore, broadcast, check, send};

use crate::args::{optional_bool, require_str};
use crate::lookup::{agent_not_found, find_in_workspace, workspace_members};
use crate::responses::{
    BroadcastResponse, CheckMessagesResponse, MessageInfo, SendMessageResponse, success,
};

pub fn send_message(
    agents: &AgentStore,
    messages: &mut MessageStore,
    notifier: &dyn DeliveryNotifier,
    arguments: &serde_json::Value,
) -> ToolCallResult {
    let from = match require_str(arguments, "from") {
        Ok(v) => v,
        Err(err) => return err,
    };
    let to = match require_str(arguments, "to") {
        Ok(v) => v,
        Err(err) => return err,
    };
    let content = match require_str(arguments, "content") {
        Ok(v) => v,
        Err(err) => return err,
    };

    let Some(sender) = crate::lookup::find_by_name_or_id(agents, from) else {
        return agent_not_found(agents, from);
    };
    let sender = sender.clone();
    let members = workspace_members(agents, sender.id);

    let Some(recipient) = find_in_workspace(agents, sender.id, to) else {
        return ToolCallResult::error("Failed to send message: Recipient not found");
    };

    match send(messages, notifier, &sender, &members, recipient.id, content) {
        Ok(_) => success(&SendMessageResponse {
            success: true,
            message: "Message sent successfully. Don't check for a response right away - you will be notified when the other agent responds.".to_string(),
        }),
        Err(err) => ToolCallResult::error(err.to_string()),
    }
}

pub fn check_messages(
    agents: &AgentStore,
    messages: &mut MessageStore,
    arguments: &serde_json::Value,
) -> ToolCallResult {
    let agent_id_str = match require_str(arguments, "agentId") {
        Ok(v) => v,
        Err(err) => return err,
    };
    let Some(agent) = crate::lookup::find_by_name_or_id(agents, agent_id_str) else {
        return agent_not_found(agents, agent_id_str);
    };
    let mark_as_read = optional_bool(arguments, "markAsRead").unwrap_or(true);

    let unread = check(messages, agent.id, mark_as_read);
    let infos = unread
        .into_iter()
        .map(|m| {
            let sender_name = agents
                .agent(m.from)
                .map(|a| a.name.clone())
                .unwrap_or_else(|| m.from.to_string());
            MessageInfo {
                id: m.id.to_string(),
                from: sender_name,
                content: m.content,
                timestamp: rfc3339(m.timestamp),
            }
        })
        .collect();

    success(&CheckMessagesResponse { messages: infos })
}

pub fn broadcast_message(
    agents: &AgentStore,
    messages: &mut MessageStore,
    notifier: &dyn DeliveryNotifier,
    arguments: &serde_json::Value,
) -> ToolCallResult {
    let from = match require_str(arguments, "from") {
        Ok(v) => v,
        Err(err) => return err,
    };
    let content = match require_str(arguments, "content") {
        Ok(v) => v,
        Err(err) => return err,
    };

    let Some(sender) = crate::lookup::find_by_name_or_id(agents, from) else {
        return agent_not_found(agents, from);
    };
    let sender = sender.clone();
    let members = workspace_members(agents, sender.id);

    let count = broadcast(messages, notifier, &sender, &members, content);
    success(&BroadcastResponse {
        success: count > 0,
        recipient_count: count,
    })
}

/// RFC 3339, matching the Swift reference's `ISO8601DateFormatter`.
fn rfc3339(time: std::time::SystemTime) -> String {
    time::OffsetDateTime::from(time)
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use skwad_agents::CreateOptions;
    use skwad_messaging::NoopNotifier;

    use super::*;

    fn registered(store: &mut AgentStore, folder: &str) -> uuid::Uuid {
        let id = store.create(folder, CreateOptions::default());
        store.set_registered(id, true);
        id
    }

    #[test]
    fn send_message_succeeds_between_registered_workspace_members() {
        let mut agents = AgentStore::new();
        let from = registered(&mut agents, "/tmp/a");
        let to = registered(&mut agents, "/tmp/b");
        let mut messages = MessageStore::new();

        let result = send_message(
            &agents,
            &mut messages,
            &NoopNotifier,
            &json!({"from": from.to_string(), "to": to.to_string(), "content": "hi"}),
        );

        assert_eq!(result.is_error, None);
        assert!(messages.has_unread(to));
    }

    #[test]
    fn send_message_to_shell_agent_surfaces_rejection_text() {
        let mut agents = AgentStore::new();
        let from = registered(&mut agents, "/tmp/a");
        let to = agents.create(
            "/tmp/shell",
            CreateOptions {
                agent_type: Some("shell".to_string()),
                insert_after: Some(from),
                ..Default::default()
            },
        );
        agents.set_registered(to, true);
        let mut messages = MessageStore::new();

        let result = send_message(
            &agents,
            &mut messages,
            &NoopNotifier,
            &json!({"from": from.to_string(), "to": to.to_string(), "content": "hi"}),
        );

        assert_eq!(result.is_error, Some(true));
        assert_eq!(
            result.content[0].text,
            "Cannot send messages to shell agents"
        );
    }

    #[test]
    fn check_messages_marks_read_by_default() {
        let mut agents = AgentStore::new();
        let from = registered(&mut agents, "/tmp/a");
        let to = registered(&mut agents, "/tmp/b");
        let mut messages = MessageStore::new();
        send_message(
            &agents,
            &mut messages,
            &NoopNotifier,
            &json!({"from": from.to_string(), "to": to.to_string(), "content": "hi"}),
        );

        let result = check_messages(&agents, &mut messages, &json!({"agentId": to.to_string()}));

        assert_eq!(result.is_error, None);
        assert!(!messages.has_unread(to));
    }

    #[test]
    fn check_messages_without_mark_as_read_leaves_unread() {
        let mut agents = AgentStore::new();
        let from = registered(&mut agents, "/tmp/a");
        let to = registered(&mut agents, "/tmp/b");
        let mut messages = MessageStore::new();
        send_message(
            &agents,
            &mut messages,
            &NoopNotifier,
            &json!({"from": from.to_string(), "to": to.to_string(), "content": "hi"}),
        );

        check_messages(
            &agents,
            &mut messages,
            &json!({"agentId": to.to_string(), "markAsRead": false}),
        );

        assert!(messages.has_unread(to));
    }

    #[test]
    fn broadcast_reports_recipient_count() {
        let mut agents = AgentStore::new();
        let sender = registered(&mut agents, "/tmp/a");
        registered(&mut agents, "/tmp/b");
        registered(&mut agents, "/tmp/c");
        let mut messages = MessageStore::new();

        let result = broadcast_message(
            &agents,
            &mut messages,
            &NoopNotifier,
            &json!({"from": sender.to_string(), "content": "hi all"}),
        );

        assert_eq!(result.is_error, None);
        assert!(result.content[0].text.contains("\"recipientCount\": 2"));
    }
}
