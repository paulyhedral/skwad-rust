use std::collections::BTreeMap;
use std::path::Path;

use skwad_core::{BenchAgent, SavedAgent, Workspace};
use uuid::Uuid;

use crate::agent::{Agent, AgentState};
use crate::convert::from_saved;
use crate::error::{AgentError, Result};

const DEFAULT_WORKSPACE_NAME: &str = "Skwad";
const DEFAULT_WORKSPACE_COLOR: &str = "#1B4FB2";

/// Fields a caller may override when creating an agent; everything else
/// (id, runtime state) is derived. `folder` is passed separately to
/// `AgentStore::create` since it is the one field every creation supplies.
#[derive(Debug, Clone, Default)]
pub struct CreateOptions {
    pub name: Option<String>,
    pub avatar: Option<String>,
    pub agent_type: Option<String>,
    pub shell_command: Option<String>,
    pub persona_id: Option<Uuid>,
    pub created_by: Option<Uuid>,
    pub is_companion: bool,
    pub insert_after: Option<Uuid>,
}

/// Fields an edit may change. `name`/`avatar` always apply and never trigger
/// a restart; `folder`/`agent_type`/persona changes do.
#[derive(Debug, Clone, Default)]
pub struct EditRequest {
    pub name: String,
    pub avatar: String,
    pub folder: Option<String>,
    pub agent_type: Option<String>,
    pub persona_id: Option<Uuid>,
    pub persona_changed: bool,
    pub relocate_companions: bool,
}

/// An agent removed by [`AgentStore::remove`], in cascade order (companions
/// before their owner).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RemovedAgent {
    pub id: Uuid,
    /// Whether the caller owes MCP an unregister call for this id.
    pub was_registered: bool,
}

/// Owns the agent list and the workspaces that place them, implementing
/// every operation `openspec/specs/agent-lifecycle/spec.md` names.
#[derive(Debug, Clone, Default)]
pub struct AgentStore {
    agents: Vec<Agent>,
    workspaces: Vec<Workspace>,
    current_workspace_id: Option<Uuid>,
}

impl AgentStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_saved(saved_agents: &[SavedAgent], workspaces: Vec<Workspace>) -> Self {
        let agents = saved_agents.iter().map(from_saved).collect::<Vec<_>>();
        let workspaces = if agents.is_empty() || !workspaces.is_empty() {
            workspaces
        } else {
            vec![default_workspace(
                agents.iter().map(|agent| agent.id).collect(),
            )]
        };
        let current_workspace_id = workspaces.first().map(|workspace| workspace.id);

        Self {
            agents,
            workspaces,
            current_workspace_id,
        }
    }

    pub fn agents(&self) -> &[Agent] {
        &self.agents
    }

    pub fn workspaces(&self) -> &[Workspace] {
        &self.workspaces
    }

    pub fn agent(&self, id: Uuid) -> Option<&Agent> {
        self.agents.iter().find(|a| a.id == id)
    }

    fn agent_mut(&mut self, id: Uuid) -> Option<&mut Agent> {
        self.agents.iter_mut().find(|a| a.id == id)
    }

    /// A no-op on an unknown `id`, matching the Swift reference's
    /// `AgentDataProvider.setRegistered`/`setSessionId`/`setAgentStatusText`,
    /// which silently ignore a stale caller id rather than erroring.
    pub fn set_registered(&mut self, id: Uuid, registered: bool) {
        if let Some(agent) = self.agent_mut(id) {
            agent.is_registered = registered;
        }
    }

    pub fn set_session_id(&mut self, id: Uuid, session_id: String) {
        if let Some(agent) = self.agent_mut(id) {
            agent.session_id = Some(session_id);
        }
    }

    pub fn set_state(&mut self, id: Uuid, state: AgentState) {
        if let Some(agent) = self.agent_mut(id) {
            agent.state = state;
        }
    }

    pub fn update_metadata(&mut self, id: Uuid, metadata: BTreeMap<String, String>) {
        if let Some(agent) = self.agent_mut(id) {
            agent.metadata.extend(metadata);
        }
    }

    /// Sets the agent-facing status text (`set-status`'s target); distinct
    /// from `AgentState`, the automatic state machine.
    pub fn set_status_text(&mut self, id: Uuid, status: String) {
        if let Some(agent) = self.agent_mut(id) {
            agent.status_text = status;
        }
    }

    pub fn add_workspace(&mut self, workspace: Workspace) {
        self.workspaces.push(workspace);
    }

    pub fn current_workspace_id(&self) -> Option<Uuid> {
        self.current_workspace_id
    }

    pub fn set_current_workspace(&mut self, id: Uuid) {
        self.current_workspace_id = Some(id);
    }

    /// The current workspace, creating the default "Skwad" workspace if none
    /// exists yet.
    fn ensure_current_workspace(&mut self) -> Uuid {
        if let Some(id) = self.current_workspace_id
            && self.workspaces.iter().any(|w| w.id == id)
        {
            return id;
        }
        let workspace = default_workspace(Vec::new());
        let id = workspace.id;
        self.workspaces.push(workspace);
        self.current_workspace_id = Some(id);
        id
    }

    fn workspace_of(&self, agent_id: Uuid) -> Option<Uuid> {
        self.workspaces
            .iter()
            .find(|w| w.agent_ids.contains(&agent_id))
            .map(|w| w.id)
    }

    // -- Creation ------------------------------------------------------

    pub fn create(&mut self, folder: impl Into<String>, opts: CreateOptions) -> Uuid {
        let folder = folder.into();
        let name = opts.name.unwrap_or_else(|| last_path_component(&folder));
        let agent = Agent {
            id: Uuid::new_v4(),
            name,
            avatar: opts.avatar.unwrap_or_default(),
            folder,
            agent_type: opts.agent_type.unwrap_or_else(|| "claude".to_string()),
            created_by: opts.created_by,
            is_companion: opts.is_companion,
            shell_command: opts.shell_command,
            persona_id: opts.persona_id,
            state: AgentState::Idle,
            status_text: String::new(),
            is_registered: false,
            is_pending_start: false,
            terminal_title: String::new(),
            restart_token: Uuid::new_v4(),
            session_id: None,
            resume_session_id: None,
            fork_session: false,
            metadata: BTreeMap::new(),
            markdown_file: None,
            markdown_maximized: false,
            markdown_history: Vec::new(),
            mermaid_source: None,
            mermaid_title: None,
        };
        let id = agent.id;

        match opts
            .insert_after
            .and_then(|sib| self.agents.iter().position(|a| a.id == sib))
        {
            Some(idx) => self.agents.insert(idx + 1, agent),
            None => self.agents.push(agent),
        }

        let source = opts.created_by.or(opts.insert_after);
        let workspace_id = source
            .and_then(|s| self.workspace_of(s))
            .unwrap_or_else(|| self.ensure_current_workspace());

        if let Some(ws) = self.workspaces.iter_mut().find(|w| w.id == workspace_id) {
            match opts
                .insert_after
                .and_then(|sib| ws.agent_ids.iter().position(|a| *a == sib))
            {
                Some(idx) => ws.agent_ids.insert(idx + 1, id),
                None => ws.agent_ids.push(id),
            }
            if ws.active_agent_ids.is_empty() {
                ws.active_agent_ids = vec![id];
            }
        }

        id
    }

    /// Create a shell companion bound to `owner`. Placed after `owner` in
    /// both the master list and its workspace.
    pub fn create_shell_companion(&mut self, owner: Uuid) -> Result<Uuid> {
        let owner_agent = self.agent(owner).ok_or(AgentError::NotFound(owner))?;
        if owner_agent.is_companion {
            return Err(AgentError::CompanionCannotOwn(owner));
        }
        let folder = owner_agent.folder.clone();
        Ok(self.create(
            folder,
            CreateOptions {
                name: Some("Shell".to_string()),
                agent_type: Some("shell".to_string()),
                created_by: Some(owner),
                is_companion: true,
                insert_after: Some(owner),
                ..Default::default()
            },
        ))
    }

    // -- Removal ---------------------------------------------------------

    pub fn companions(&self, owner: Uuid) -> Vec<Uuid> {
        self.agents
            .iter()
            .filter(|a| a.created_by == Some(owner) && a.is_companion)
            .map(|a| a.id)
            .collect()
    }

    /// Removes `id` and, recursively, every agent it owns. Companions are
    /// removed (and returned) before their owner.
    pub fn remove(&mut self, id: Uuid) -> Vec<RemovedAgent> {
        let mut removed = Vec::new();
        for companion_id in self.companions(id) {
            removed.extend(self.remove(companion_id));
        }

        if let Some(pos) = self.agents.iter().position(|a| a.id == id) {
            let was_registered = self.agents[pos].is_registered;
            self.agents.remove(pos);
            for ws in &mut self.workspaces {
                ws.agent_ids.retain(|a| *a != id);
                ws.active_agent_ids.retain(|a| *a != id);
            }
            removed.push(RemovedAgent { id, was_registered });
        }

        removed
    }

    // -- Restart / resume -------------------------------------------------

    /// Bumps the restart token and resets transient UI state, forcing the
    /// terminal session to be recreated. Does not touch session/resume/fork
    /// fields - `restart` and `resume_session` decide those independently.
    fn recreate_terminal(&mut self, id: Uuid) -> Result<()> {
        let agent = self
            .agents
            .iter_mut()
            .find(|a| a.id == id)
            .ok_or(AgentError::NotFound(id))?;
        agent.restart_token = Uuid::new_v4();
        agent.state = AgentState::Idle;
        agent.is_registered = false;
        agent.terminal_title = String::new();
        Ok(())
    }

    /// Preserves id, regenerates the restart token, and clears session
    /// identity: session id, resume-session id, fork flag.
    pub fn restart(&mut self, id: Uuid) -> Result<()> {
        {
            let agent = self
                .agents
                .iter_mut()
                .find(|a| a.id == id)
                .ok_or(AgentError::NotFound(id))?;
            agent.session_id = None;
            agent.resume_session_id = None;
            agent.fork_session = false;
        }
        self.recreate_terminal(id)
    }

    /// Targets `session_id` for resume (not fork), then recreates the
    /// terminal session.
    pub fn resume_session(&mut self, id: Uuid, session_id: impl Into<String>) -> Result<()> {
        let session_id = session_id.into();
        {
            let agent = self
                .agents
                .iter_mut()
                .find(|a| a.id == id)
                .ok_or(AgentError::NotFound(id))?;
            agent.resume_session_id = Some(session_id.clone());
            agent.session_id = Some(session_id);
            agent.fork_session = false;
        }
        self.recreate_terminal(id)
    }

    // -- Edit --------------------------------------------------------------

    /// Applies `req`. Name/avatar changes never restart; folder, agent type,
    /// or persona changes do. With `relocate_companions`, companions still at
    /// the old folder move with it and restart too.
    pub fn edit(&mut self, id: Uuid, req: EditRequest) -> Result<()> {
        let old_folder = self
            .agent(id)
            .ok_or(AgentError::NotFound(id))?
            .folder
            .clone();
        let mut needs_restart = false;

        {
            let agent = self.agents.iter_mut().find(|a| a.id == id).unwrap();
            agent.name = req.name;
            agent.avatar = req.avatar;

            if let Some(agent_type) = &req.agent_type
                && *agent_type != agent.agent_type
            {
                agent.agent_type = agent_type.clone();
                needs_restart = true;
            }
            if req.persona_changed {
                agent.persona_id = req.persona_id;
                needs_restart = true;
            }
            if let Some(folder) = &req.folder
                && *folder != old_folder
            {
                agent.folder = folder.clone();
                needs_restart = true;
            }
        }

        if let Some(new_folder) = req.folder.as_ref().filter(|f| **f != old_folder)
            && req.relocate_companions
        {
            let stale_companions: Vec<Uuid> = self
                .companions(id)
                .into_iter()
                .filter(|c| self.agent(*c).is_some_and(|a| a.folder == old_folder))
                .collect();
            for companion_id in stale_companions {
                if let Some(a) = self.agents.iter_mut().find(|a| a.id == companion_id) {
                    a.folder = new_folder.clone();
                }
                self.restart(companion_id)?;
            }
        }

        if needs_restart {
            self.restart(id)?;
        }
        Ok(())
    }

    // -- Panels ----------------------------------------------------------

    /// Sets the markdown panel target and pushes `file` to the front of the
    /// history, deduping so a re-shown file moves to the front instead of
    /// appearing twice.
    pub fn set_markdown_panel(
        &mut self,
        id: Uuid,
        file: std::path::PathBuf,
        maximized: bool,
    ) -> Result<()> {
        let agent = self
            .agents
            .iter_mut()
            .find(|a| a.id == id)
            .ok_or(AgentError::NotFound(id))?;
        agent.markdown_history.retain(|f| *f != file);
        agent.markdown_history.insert(0, file.clone());
        agent.markdown_file = Some(file);
        agent.markdown_maximized = maximized;
        Ok(())
    }

    pub fn set_mermaid_panel(
        &mut self,
        id: Uuid,
        source: String,
        title: Option<String>,
    ) -> Result<()> {
        let agent = self
            .agents
            .iter_mut()
            .find(|a| a.id == id)
            .ok_or(AgentError::NotFound(id))?;
        agent.mermaid_source = Some(source);
        agent.mermaid_title = title;
        Ok(())
    }

    // -- Bench ---------------------------------------------------------------

    /// Deploys a bench template if its folder still exists. Returns `None`
    /// (creating nothing) when it doesn't, so the caller can prune the stale
    /// entry; this crate stays filesystem-free, so existence is a predicate
    /// the caller supplies rather than an `std::fs` call here.
    pub fn deploy_bench(
        &mut self,
        bench: &BenchAgent,
        folder_exists: impl FnOnce(&Path) -> bool,
    ) -> Option<Uuid> {
        if !folder_exists(Path::new(&bench.folder)) {
            return None;
        }
        Some(self.create(
            bench.folder.clone(),
            CreateOptions {
                name: Some(bench.name.clone()),
                avatar: Some(bench.avatar.clone()),
                agent_type: Some(bench.agent_type.clone()),
                shell_command: bench.shell_command.clone(),
                persona_id: bench.persona_id,
                ..Default::default()
            },
        ))
    }

    // -- Ordering ------------------------------------------------------------

    pub fn reorder(&mut self, workspace_id: Uuid, from: usize, to: usize) {
        let Some(ws) = self.workspaces.iter_mut().find(|w| w.id == workspace_id) else {
            return;
        };
        if from >= ws.agent_ids.len() || to >= ws.agent_ids.len() {
            return;
        }
        let id = ws.agent_ids.remove(from);
        ws.agent_ids.insert(to, id);
    }

    pub fn move_to_workspace(&mut self, agent_id: Uuid, target_workspace_id: Uuid) {
        if let Some(source) = self.workspace_of(agent_id) {
            if source == target_workspace_id {
                return;
            }
            if let Some(ws) = self.workspaces.iter_mut().find(|w| w.id == source) {
                ws.agent_ids.retain(|a| *a != agent_id);
                ws.active_agent_ids.retain(|a| *a != agent_id);
            }
        }
        if let Some(ws) = self
            .workspaces
            .iter_mut()
            .find(|w| w.id == target_workspace_id)
        {
            ws.agent_ids.push(agent_id);
            if ws.active_agent_ids.is_empty() {
                ws.active_agent_ids = vec![agent_id];
            }
        }
    }
}

fn default_workspace(agent_ids: Vec<Uuid>) -> Workspace {
    Workspace {
        id: Uuid::new_v4(),
        name: DEFAULT_WORKSPACE_NAME.to_string(),
        color_hex: DEFAULT_WORKSPACE_COLOR.to_string(),
        active_agent_ids: agent_ids.first().copied().into_iter().collect(),
        agent_ids,
        layout_mode: "single".to_string(),
        focused_pane_index: 0,
        split_ratio: 0.5,
        split_ratio_secondary: None,
        show_dashboard: None,
        is_detached: None,
    }
}

fn last_path_component(folder: &str) -> String {
    Path::new(folder)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| folder.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> AgentStore {
        AgentStore::new()
    }

    #[test]
    fn restores_agents_and_workspaces_with_runtime_defaults() {
        let agent_id = Uuid::new_v4();
        let workspace_id = Uuid::new_v4();
        let saved = SavedAgent::new(agent_id, "proj", None, "/tmp/proj");
        let workspace = Workspace {
            id: workspace_id,
            name: "Work".to_string(),
            color_hex: "#123456".to_string(),
            agent_ids: vec![agent_id],
            layout_mode: "splitVertical".to_string(),
            active_agent_ids: vec![agent_id],
            focused_pane_index: 1,
            split_ratio: 0.6,
            split_ratio_secondary: Some(0.4),
            show_dashboard: Some(false),
            is_detached: Some(true),
        };

        let store = AgentStore::from_saved(&[saved], vec![workspace.clone()]);

        assert_eq!(store.current_workspace_id(), Some(workspace_id));
        assert_eq!(store.workspaces(), &[workspace]);
        let agent = store.agent(agent_id).unwrap();
        assert_eq!(agent.name, "proj");
        assert_eq!(agent.state, AgentState::Idle);
        assert!(!agent.is_registered);
        assert!(agent.session_id.is_none());
        assert!(agent.metadata.is_empty());
    }

    #[test]
    fn restores_agents_into_default_workspace_when_layout_is_missing() {
        let agent_id = Uuid::new_v4();
        let saved = SavedAgent::new(agent_id, "proj", None, "/tmp/proj");

        let store = AgentStore::from_saved(&[saved], Vec::new());

        assert_eq!(store.workspaces().len(), 1);
        assert_eq!(store.workspaces()[0].agent_ids, vec![agent_id]);
        assert_eq!(store.workspaces()[0].active_agent_ids, vec![agent_id]);
    }

    #[test]
    fn create_from_folder_with_defaults() {
        let mut s = store();
        let id = s.create("/Users/x/proj", CreateOptions::default());

        let agent = s.agent(id).unwrap();
        assert_eq!(agent.name, "proj");
        assert_eq!(agent.agent_type, "claude");

        let ws = &s.workspaces()[0];
        assert_eq!(ws.agent_ids, vec![id]);
        assert_eq!(ws.active_agent_ids, vec![id]);
    }

    #[test]
    fn insert_after_a_sibling() {
        let mut s = store();
        let a = s.create("/tmp/a", CreateOptions::default());
        let c = s.create("/tmp/c", CreateOptions::default());
        let b = s.create(
            "/tmp/b",
            CreateOptions {
                insert_after: Some(a),
                ..Default::default()
            },
        );

        assert_eq!(
            s.agents().iter().map(|a| a.id).collect::<Vec<_>>(),
            vec![a, b, c]
        );
        assert_eq!(s.workspaces()[0].agent_ids, vec![a, b, c]);
    }

    #[test]
    fn new_agent_inherits_source_workspace() {
        let mut s = store();
        let a = s.create("/tmp/a", CreateOptions::default());
        let other_ws = Uuid::new_v4();
        s.add_workspace(Workspace {
            id: other_ws,
            name: "Other".to_string(),
            color_hex: "#000000".to_string(),
            agent_ids: vec![],
            layout_mode: "single".to_string(),
            active_agent_ids: vec![],
            focused_pane_index: 0,
            split_ratio: 0.5,
            split_ratio_secondary: None,
            show_dashboard: None,
            is_detached: None,
        });
        s.set_current_workspace(other_ws);

        let b = s.create(
            "/tmp/b",
            CreateOptions {
                insert_after: Some(a),
                ..Default::default()
            },
        );

        let home_ws = s
            .workspaces()
            .iter()
            .find(|w| w.agent_ids.contains(&a))
            .unwrap();
        assert!(home_ws.agent_ids.contains(&b));
        let other = s.workspaces().iter().find(|w| w.id == other_ws).unwrap();
        assert!(!other.agent_ids.contains(&b));
    }

    #[test]
    fn companion_is_bound_to_its_owner() {
        let mut s = store();
        let owner = s.create("/tmp/a", CreateOptions::default());
        let companion = s.create_shell_companion(owner).unwrap();

        let agent = s.agent(companion).unwrap();
        assert_eq!(agent.created_by, Some(owner));
        assert!(agent.is_companion);
        assert_eq!(agent.agent_type, "shell");
        assert_eq!(s.workspaces()[0].agent_ids, vec![owner, companion]);
    }

    #[test]
    fn companion_cannot_own_companions() {
        let mut s = store();
        let owner = s.create("/tmp/a", CreateOptions::default());
        let companion = s.create_shell_companion(owner).unwrap();

        let err = s.create_shell_companion(companion).unwrap_err();
        assert_eq!(err, AgentError::CompanionCannotOwn(companion));
    }

    #[test]
    fn removing_an_owner_cascades_to_companions() {
        let mut s = store();
        let a = s.create("/tmp/a", CreateOptions::default());
        let b = s.create_shell_companion(a).unwrap();
        let c = s.create_shell_companion(a).unwrap();

        let removed = s.remove(a);

        // Companions before owner; order between siblings is unspecified.
        let ids: Vec<Uuid> = removed.iter().map(|r| r.id).collect();
        assert_eq!(ids.last(), Some(&a));
        assert!(ids.contains(&b) && ids.contains(&c));
        assert!(s.agents().is_empty());
        assert!(s.workspaces()[0].agent_ids.is_empty());
    }

    #[test]
    fn removing_a_registered_agent_flags_unregister() {
        let mut s = store();
        let a = s.create("/tmp/a", CreateOptions::default());
        s.agents
            .iter_mut()
            .find(|x| x.id == a)
            .unwrap()
            .is_registered = true;

        let removed = s.remove(a);

        assert_eq!(
            removed,
            vec![RemovedAgent {
                id: a,
                was_registered: true
            }]
        );
    }

    #[test]
    fn restart_keeps_identity_drops_session() {
        let mut s = store();
        let a = s.create("/tmp/a", CreateOptions::default());
        {
            let agent = s.agents.iter_mut().find(|x| x.id == a).unwrap();
            agent.session_id = Some("s1".to_string());
            agent.is_registered = true;
        }
        let old_token = s.agent(a).unwrap().restart_token;

        s.restart(a).unwrap();

        let agent = s.agent(a).unwrap();
        assert_eq!(agent.id, a);
        assert_ne!(agent.restart_token, old_token);
        assert_eq!(agent.session_id, None);
        assert!(!agent.is_registered);
        assert_eq!(agent.state, AgentState::Idle);
    }

    #[test]
    fn resume_targets_a_prior_session() {
        let mut s = store();
        let a = s.create("/tmp/a", CreateOptions::default());

        s.resume_session(a, "s2").unwrap();

        let agent = s.agent(a).unwrap();
        assert_eq!(agent.resume_session_id.as_deref(), Some("s2"));
        assert_eq!(agent.session_id.as_deref(), Some("s2"));
        assert!(!agent.fork_session);
    }

    #[test]
    fn rename_does_not_restart() {
        let mut s = store();
        let a = s.create("/tmp/a", CreateOptions::default());
        let old_token = s.agent(a).unwrap().restart_token;

        s.edit(
            a,
            EditRequest {
                name: "renamed".to_string(),
                avatar: "🦀".to_string(),
                ..Default::default()
            },
        )
        .unwrap();

        let agent = s.agent(a).unwrap();
        assert_eq!(agent.name, "renamed");
        assert_eq!(agent.restart_token, old_token);
    }

    #[test]
    fn folder_change_restarts_and_relocates_companions() {
        let mut s = store();
        let a = s.create("/tmp/old", CreateOptions::default());
        let b = s.create_shell_companion(a).unwrap();
        let a_token = s.agent(a).unwrap().restart_token;
        let b_token = s.agent(b).unwrap().restart_token;

        s.edit(
            a,
            EditRequest {
                name: s.agent(a).unwrap().name.clone(),
                avatar: s.agent(a).unwrap().avatar.clone(),
                folder: Some("/tmp/new".to_string()),
                relocate_companions: true,
                ..Default::default()
            },
        )
        .unwrap();

        assert_eq!(s.agent(a).unwrap().folder, "/tmp/new");
        assert_ne!(s.agent(a).unwrap().restart_token, a_token);
        assert_eq!(s.agent(b).unwrap().folder, "/tmp/new");
        assert_ne!(s.agent(b).unwrap().restart_token, b_token);
    }

    #[test]
    fn stale_bench_entry_is_pruned() {
        let mut s = store();
        let bench = BenchAgent::new(Uuid::new_v4(), "Ghost", None, "/does/not/exist");

        let id = s.deploy_bench(&bench, |_| false);

        assert_eq!(id, None);
        assert!(s.agents().is_empty());
    }

    #[test]
    fn bench_deploy_creates_agent_from_template() {
        let mut s = store();
        let bench = BenchAgent::new(Uuid::new_v4(), "Real", None, "/tmp/real");

        let id = s.deploy_bench(&bench, |_| true).unwrap();

        let agent = s.agent(id).unwrap();
        assert_eq!(agent.name, "Real");
        assert_eq!(agent.folder, "/tmp/real");
    }

    #[test]
    fn reorder_moves_within_workspace() {
        let mut s = store();
        let a = s.create("/tmp/a", CreateOptions::default());
        let b = s.create("/tmp/b", CreateOptions::default());
        let ws_id = s.workspaces()[0].id;

        s.reorder(ws_id, 0, 1);

        assert_eq!(s.workspaces()[0].agent_ids, vec![b, a]);
    }

    #[test]
    fn move_to_workspace_relocates_and_activates() {
        let mut s = store();
        let a = s.create("/tmp/a", CreateOptions::default());
        let target = Uuid::new_v4();
        s.add_workspace(Workspace {
            id: target,
            name: "Target".to_string(),
            color_hex: "#000000".to_string(),
            agent_ids: vec![],
            layout_mode: "single".to_string(),
            active_agent_ids: vec![],
            focused_pane_index: 0,
            split_ratio: 0.5,
            split_ratio_secondary: None,
            show_dashboard: None,
            is_detached: None,
        });

        s.move_to_workspace(a, target);

        assert!(!s.workspaces()[0].agent_ids.contains(&a));
        let target_ws = s.workspaces().iter().find(|w| w.id == target).unwrap();
        assert_eq!(target_ws.agent_ids, vec![a]);
        assert_eq!(target_ws.active_agent_ids, vec![a]);
    }

    #[test]
    fn markdown_panel_history_dedups_and_moves_to_front() {
        let mut s = store();
        let id = s.create("/tmp/a", CreateOptions::default());
        let file_a = std::path::PathBuf::from("/tmp/a/A.md");
        let file_b = std::path::PathBuf::from("/tmp/a/B.md");

        s.set_markdown_panel(id, file_a.clone(), false).unwrap();
        s.set_markdown_panel(id, file_b.clone(), false).unwrap();
        s.set_markdown_panel(id, file_a.clone(), true).unwrap();

        let agent = s.agent(id).unwrap();
        assert_eq!(agent.markdown_history, vec![file_a.clone(), file_b]);
        assert_eq!(agent.markdown_file, Some(file_a));
        assert!(agent.markdown_maximized);
    }

    #[test]
    fn set_registered_and_session_id_apply_to_the_agent() {
        let mut s = store();
        let id = s.create("/tmp/a", CreateOptions::default());

        s.set_registered(id, true);
        s.set_session_id(id, "sess-1".to_string());

        let agent = s.agent(id).unwrap();
        assert!(agent.is_registered);
        assert_eq!(agent.session_id.as_deref(), Some("sess-1"));
    }

    #[test]
    fn set_status_text_updates_status_not_state() {
        let mut s = store();
        let id = s.create("/tmp/a", CreateOptions::default());

        s.set_status_text(id, "Running tests".to_string());

        let agent = s.agent(id).unwrap();
        assert_eq!(agent.status_text, "Running tests");
        assert_eq!(agent.state, AgentState::Idle);
    }

    #[test]
    fn setters_on_unknown_agent_are_a_no_op() {
        let mut s = store();
        let stale = Uuid::new_v4();
        s.set_registered(stale, true);
        s.set_status_text(stale, "x".to_string());
        assert!(s.agent(stale).is_none());
    }

    #[test]
    fn markdown_panel_unknown_agent_errors() {
        let mut s = store();
        let result = s.set_markdown_panel(Uuid::new_v4(), std::path::PathBuf::from("/x.md"), false);
        assert!(result.is_err());
    }

    #[test]
    fn mermaid_panel_round_trips_source_and_title() {
        let mut s = store();
        let id = s.create("/tmp/a", CreateOptions::default());

        s.set_mermaid_panel(id, "graph TD; A-->B;".to_string(), Some("Flow".to_string()))
            .unwrap();

        let agent = s.agent(id).unwrap();
        assert_eq!(agent.mermaid_source.as_deref(), Some("graph TD; A-->B;"));
        assert_eq!(agent.mermaid_title.as_deref(), Some("Flow"));
    }
}
