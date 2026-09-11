use std::path::Path;

use skwad_discovery::RepoInfo;
use skwad_git::{Repository, worktree::is_working_tree};
use skwad_mcp::ToolCallResult;

use crate::args::require_str;
use crate::responses::{
    CreateWorktreeResponse, ListReposResponse, ListWorktreesResponse, RepoInfoResponse,
    WorktreeInfoResponse, success,
};

fn to_worktree_responses(repo: &RepoInfo) -> Vec<WorktreeInfoResponse> {
    repo.worktrees
        .iter()
        .map(|w| WorktreeInfoResponse {
            name: w.name.clone(),
            path: w.path.display().to_string(),
        })
        .collect()
}

pub fn list_repos(repos: &[RepoInfo]) -> ToolCallResult {
    let repos = repos
        .iter()
        .map(|r| RepoInfoResponse {
            name: r.name.clone(),
            worktrees: to_worktree_responses(r),
        })
        .collect();
    success(&ListReposResponse { repos })
}

pub fn list_worktrees(repos: &[RepoInfo], arguments: &serde_json::Value) -> ToolCallResult {
    let repo_path = match require_str(arguments, "repoPath") {
        Ok(v) => v,
        Err(err) => return err,
    };

    // The primary clone (index 0, per `skwad_discovery::scan`'s grouping) is
    // the repo's identity; a `repoPath` matching neither the primary nor a
    // worktree path yields an empty list rather than an error.
    let worktrees = repos
        .iter()
        .find(|r| {
            r.worktrees
                .iter()
                .any(|w| w.path.to_string_lossy() == repo_path)
        })
        .map(to_worktree_responses)
        .unwrap_or_default();

    success(&ListWorktreesResponse {
        repo_path: repo_path.to_string(),
        worktrees,
    })
}

pub fn create_worktree(arguments: &serde_json::Value) -> ToolCallResult {
    let repo_path = match require_str(arguments, "repoPath") {
        Ok(v) => v,
        Err(err) => return err,
    };
    let branch_name = match require_str(arguments, "branchName") {
        Ok(v) if !v.is_empty() => v,
        _ => return ToolCallResult::error("Missing required parameter: branchName"),
    };

    let repo_path_ref = Path::new(repo_path);
    if !is_working_tree(repo_path_ref) {
        return ToolCallResult::error(format!("Not a git repository: {repo_path}"));
    }

    let destination = skwad_git::suggest_worktree_path(repo_path_ref, branch_name);
    match Repository::open(repo_path_ref).create_worktree(branch_name, &destination) {
        Ok(()) => {
            let path = destination.display().to_string();
            success(&CreateWorktreeResponse {
                success: true,
                path: Some(path.clone()),
                message: format!("Worktree created at {path}"),
            })
        }
        Err(err) => ToolCallResult::error(format!("Failed to create worktree: {err}")),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde_json::json;
    use skwad_discovery::WorktreeInfo;
    use skwad_git::Runner;

    use super::*;

    fn init_repo(dir: &Path) {
        let run = |args: &[&str]| {
            Runner::new(dir).run(args).unwrap();
        };
        run(&["init", "-q", "-b", "main"]);
        run(&["config", "user.email", "test@example.com"]);
        run(&["config", "user.name", "Test"]);
        run(&["config", "commit.gpgsign", "false"]);
        std::fs::write(dir.join("seed.txt"), "seed\n").unwrap();
        run(&["add", "-A"]);
        run(&["commit", "-qm", "init"]);
    }

    fn repo_info(name: &str, primary: &Path) -> RepoInfo {
        RepoInfo {
            name: name.to_string(),
            worktrees: vec![WorktreeInfo {
                name: name.to_string(),
                path: primary.to_path_buf(),
            }],
        }
    }

    #[test]
    fn list_repos_maps_names_and_worktrees() {
        let repos = vec![repo_info("proj", &PathBuf::from("/tmp/proj"))];

        let result = list_repos(&repos);

        assert_eq!(result.is_error, None);
        assert!(result.content[0].text.contains("proj"));
    }

    #[test]
    fn list_worktrees_matches_by_path() {
        let repos = vec![repo_info("proj", &PathBuf::from("/tmp/proj"))];

        let result = list_worktrees(&repos, &json!({"repoPath": "/tmp/proj"}));

        assert_eq!(result.is_error, None);
        assert!(result.content[0].text.contains("\"name\": \"proj\""));
    }

    #[test]
    fn list_worktrees_unknown_path_returns_empty_list() {
        let repos = vec![repo_info("proj", &PathBuf::from("/tmp/proj"))];

        let result = list_worktrees(&repos, &json!({"repoPath": "/tmp/other"}));

        assert_eq!(result.is_error, None);
        assert!(result.content[0].text.contains("\"worktrees\": []"));
    }

    #[test]
    fn create_worktree_empty_branch_name_errors() {
        let result = create_worktree(&json!({"repoPath": "/tmp/x", "branchName": ""}));
        assert_eq!(result.is_error, Some(true));
        assert!(result.content[0].text.contains("branchName"));
    }

    #[test]
    fn create_worktree_not_a_repo_errors() {
        let dir = tempfile::tempdir().unwrap();
        let result = create_worktree(&json!({
            "repoPath": dir.path().to_str().unwrap(),
            "branchName": "feature",
        }));
        assert_eq!(result.is_error, Some(true));
        assert!(result.content[0].text.contains("Not a git repository"));
    }

    #[test]
    fn create_worktree_success_returns_new_path() {
        let dir = tempfile::tempdir().unwrap();
        let repo_path = dir.path().join("repo");
        std::fs::create_dir(&repo_path).unwrap();
        init_repo(&repo_path);

        let result = create_worktree(&json!({
            "repoPath": repo_path.to_str().unwrap(),
            "branchName": "feature",
        }));

        assert_eq!(result.is_error, None);
        let expected = dir.path().join("repo-feature");
        assert!(is_working_tree(&expected));
        assert!(result.content[0].text.contains("repo-feature"));
    }
}
