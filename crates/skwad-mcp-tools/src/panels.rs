use std::path::Path;

use skwad_agents::AgentStore;
use skwad_mcp::ToolCallResult;

use crate::args::{optional_bool, optional_str, require_str};
use crate::lookup::{agent_not_found, find_by_name_or_id};
use crate::responses::{ShowMarkdownResponse, ShowMermaidResponse, success};

pub fn display_markdown(store: &mut AgentStore, arguments: &serde_json::Value) -> ToolCallResult {
    let agent_id_str = match require_str(arguments, "agentId") {
        Ok(v) => v,
        Err(err) => return err,
    };
    let file_path = match require_str(arguments, "filePath") {
        Ok(v) => v,
        Err(err) => return err,
    };

    let Some(agent) = find_by_name_or_id(store, agent_id_str) else {
        return agent_not_found(store, agent_id_str);
    };
    let id = agent.id;

    if !Path::new(file_path).exists() {
        return ToolCallResult::error(format!("File not found: {file_path}"));
    }

    let maximized = optional_bool(arguments, "maximized").unwrap_or(false);
    store
        .set_markdown_panel(id, std::path::PathBuf::from(file_path), maximized)
        .ok();

    success(&ShowMarkdownResponse {
        success: true,
        message: format!(
            "Markdown panel opened for: {file_path}. Inform the user they can highlight text in the preview to make comments, then click 'Submit Review' to send them to you."
        ),
    })
}

pub fn view_mermaid(store: &mut AgentStore, arguments: &serde_json::Value) -> ToolCallResult {
    let agent_id_str = match require_str(arguments, "agentId") {
        Ok(v) => v,
        Err(err) => return err,
    };
    let source = match require_str(arguments, "source") {
        Ok(v) => v,
        Err(err) => return err,
    };

    let Some(agent) = find_by_name_or_id(store, agent_id_str) else {
        return agent_not_found(store, agent_id_str);
    };
    let id = agent.id;

    let title = optional_str(arguments, "title").map(str::to_string);
    store.set_mermaid_panel(id, source.to_string(), title).ok();

    success(&ShowMermaidResponse {
        success: true,
        message: "Mermaid diagram panel opened. Supported diagram types: flowcharts, state, sequence, class, and ER diagrams.".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use skwad_agents::CreateOptions;

    use super::*;

    #[test]
    fn display_markdown_missing_file_errors() {
        let mut store = AgentStore::new();
        let id = store.create("/tmp/a", CreateOptions::default());

        let result = display_markdown(
            &mut store,
            &json!({"agentId": id.to_string(), "filePath": "/definitely/missing.md"}),
        );

        assert_eq!(result.is_error, Some(true));
        assert!(result.content[0].text.contains("File not found"));
    }

    #[test]
    fn display_markdown_missing_agent_errors() {
        let mut store = AgentStore::new();
        let file = tempfile::NamedTempFile::new().unwrap();

        let result = display_markdown(
            &mut store,
            &json!({"agentId": "nope", "filePath": file.path().to_str().unwrap()}),
        );

        assert_eq!(result.is_error, Some(true));
    }

    #[test]
    fn display_markdown_success_updates_panel_state() {
        let mut store = AgentStore::new();
        let id = store.create("/tmp/a", CreateOptions::default());
        let file = tempfile::NamedTempFile::new().unwrap();
        let path = file.path().to_str().unwrap().to_string();

        let result = display_markdown(
            &mut store,
            &json!({"agentId": id.to_string(), "filePath": path, "maximized": true}),
        );

        assert_eq!(result.is_error, None);
        let agent = store.agent(id).unwrap();
        assert_eq!(agent.markdown_file, Some(std::path::PathBuf::from(&path)));
        assert!(agent.markdown_maximized);
    }

    #[test]
    fn view_mermaid_success_updates_panel_state() {
        let mut store = AgentStore::new();
        let id = store.create("/tmp/a", CreateOptions::default());

        let result = view_mermaid(
            &mut store,
            &json!({"agentId": id.to_string(), "source": "graph TD; A-->B;", "title": "Flow"}),
        );

        assert_eq!(result.is_error, None);
        let agent = store.agent(id).unwrap();
        assert_eq!(agent.mermaid_source.as_deref(), Some("graph TD; A-->B;"));
        assert_eq!(agent.mermaid_title.as_deref(), Some("Flow"));
    }

    #[test]
    fn view_mermaid_missing_agent_errors() {
        let mut store = AgentStore::new();
        let result = view_mermaid(
            &mut store,
            &json!({"agentId": "nope", "source": "graph TD;"}),
        );
        assert_eq!(result.is_error, Some(true));
    }
}
