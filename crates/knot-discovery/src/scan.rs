use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::consts::{GIT_DIR, GITDIR_PREFIX, HEAD, HEAD_REF_PREFIX, WORKTREES_MARKER};

/// A repository discovered under the source folder, with its primary clone and
/// any linked worktrees.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoInfo {
    pub name: String,
    pub worktrees: Vec<WorktreeInfo>,
}

/// One working tree: either a repository's primary clone or a linked worktree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeInfo {
    pub name: String,
    pub path: PathBuf,
}

/// Scan the immediate children of `base`, grouping each clone with its linked
/// worktrees. Pure filesystem reads, no `git` process. A `read_dir` failure
/// (missing path, not a directory, no permission) yields an empty result.
pub fn scan(base: &Path) -> Vec<RepoInfo> {
    let entries = match fs::read_dir(base) {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };

    // repo path -> folder name
    let mut repo_names: HashMap<PathBuf, String> = HashMap::new();
    // repo path -> worktrees, primary always at index 0
    let mut worktrees: HashMap<PathBuf, Vec<WorktreeInfo>> = HashMap::new();

    for entry in entries.flatten() {
        let item_path = entry.path();
        let Some(folder) = item_path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
        else {
            continue;
        };
        let git_path = item_path.join(GIT_DIR);
        let Ok(meta) = fs::symlink_metadata(&git_path) else {
            continue;
        };

        if meta.is_dir() {
            let branch = parse_branch_from_head(&git_path).unwrap_or_else(|| folder.clone());
            worktrees.entry(item_path.clone()).or_default().insert(
                0,
                WorktreeInfo {
                    name: branch,
                    path: item_path.clone(),
                },
            );
            repo_names.insert(item_path, folder);
        } else if let Some(repo_path) = parse_worktree_gitfile(&git_path) {
            let repo_name = repo_path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let wt_name = folder
                .strip_prefix(&format!("{repo_name}-"))
                .unwrap_or(&folder)
                .to_owned();
            worktrees.entry(repo_path).or_default().push(WorktreeInfo {
                name: wt_name,
                path: item_path,
            });
        }
    }

    let mut result: Vec<RepoInfo> = repo_names
        .into_iter()
        .map(|(repo_path, name)| {
            let mut wts = worktrees.remove(&repo_path).unwrap_or_default();
            // Primary stays at index 0; order the linked worktrees for stability.
            if wts.len() > 1 {
                wts[1..].sort_by_key(|w| w.name.to_lowercase());
            }
            RepoInfo {
                name,
                worktrees: wts,
            }
        })
        .collect();

    result.sort_by_key(|r| r.name.to_lowercase());
    result
}

/// The branch a `.git` directory's `HEAD` points at, when it is a
/// `ref: refs/heads/<name>` line. `None` for a detached HEAD or an unreadable
/// file.
pub fn parse_branch_from_head(git_dir: &Path) -> Option<String> {
    let content = fs::read_to_string(git_dir.join(HEAD)).ok()?;
    let trimmed = content.trim();
    trimmed
        .strip_prefix(HEAD_REF_PREFIX)
        .map(|name| name.to_owned())
}

/// The owning repository path for a linked worktree's `.git` file, read from its
/// `gitdir: .../.git/worktrees/<id>` pointer. `None` when the file is
/// unreadable or not a worktree pointer.
pub fn parse_worktree_gitfile(git_file: &Path) -> Option<PathBuf> {
    let content = fs::read_to_string(git_file).ok()?;
    let gitdir = content.trim().strip_prefix(GITDIR_PREFIX)?;
    let end = gitdir.find(WORKTREES_MARKER)?;
    Some(PathBuf::from(&gitdir[..end]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, contents: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    #[test]
    fn head_ref_returns_branch_detached_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let git_dir = dir.path().join(".git");
        fs::create_dir(&git_dir).unwrap();

        fs::write(git_dir.join("HEAD"), "ref: refs/heads/feature/login\n").unwrap();
        assert_eq!(
            parse_branch_from_head(&git_dir),
            Some("feature/login".to_owned())
        );

        fs::write(git_dir.join("HEAD"), "9c1f0a2b3c4d5e6f\n").unwrap();
        assert_eq!(parse_branch_from_head(&git_dir), None);
    }

    #[test]
    fn worktree_gitfile_resolves_owner_or_none() {
        let dir = tempfile::tempdir().unwrap();
        let git_file = dir.path().join(".git");

        fs::write(&git_file, "gitdir: /src/app/.git/worktrees/app-feat\n").unwrap();
        assert_eq!(
            parse_worktree_gitfile(&git_file),
            Some(PathBuf::from("/src/app"))
        );

        fs::write(&git_file, "gitdir: /src/app/.git\n").unwrap();
        assert_eq!(parse_worktree_gitfile(&git_file), None);
    }

    #[test]
    fn groups_clone_with_linked_worktree() {
        let base = tempfile::tempdir().unwrap();
        let app = base.path().join("app");
        write(&app.join(".git").join("HEAD"), "ref: refs/heads/main\n");

        let feat = base.path().join("app-feat");
        write(
            &feat.join(".git"),
            &format!("gitdir: {}/.git/worktrees/app-feat\n", app.display()),
        );

        let repos = scan(base.path());
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].name, "app");
        let names: Vec<_> = repos[0].worktrees.iter().map(|w| w.name.as_str()).collect();
        assert_eq!(names, ["main", "feat"]);
    }

    #[test]
    fn detached_head_primary_uses_folder_name() {
        let base = tempfile::tempdir().unwrap();
        write(
            &base.path().join("app").join(".git").join("HEAD"),
            "9c1f0a2b3c4d5e6f7081920a3b4c5d6e7f809012\n",
        );

        let repos = scan(base.path());
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].worktrees[0].name, "app");
    }

    #[test]
    fn linked_worktree_prefix_is_stripped() {
        let base = tempfile::tempdir().unwrap();
        let app = base.path().join("app");
        write(&app.join(".git").join("HEAD"), "ref: refs/heads/main\n");
        write(
            &base.path().join("app-hotfix").join(".git"),
            &format!("gitdir: {}/.git/worktrees/app-hotfix\n", app.display()),
        );

        let repos = scan(base.path());
        let feat = repos[0]
            .worktrees
            .iter()
            .find(|w| w.path.ends_with("app-hotfix"));
        assert_eq!(feat.unwrap().name, "hotfix");
    }

    #[test]
    fn repos_sorted_case_insensitively() {
        let base = tempfile::tempdir().unwrap();
        write(
            &base.path().join("Zebra").join(".git").join("HEAD"),
            "ref: refs/heads/main\n",
        );
        write(
            &base.path().join("apple").join(".git").join("HEAD"),
            "ref: refs/heads/main\n",
        );

        let repos = scan(base.path());
        let names: Vec<_> = repos.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["apple", "Zebra"]);
    }

    #[test]
    fn missing_base_yields_empty() {
        assert!(scan(Path::new("/no/such/path/xyzzy")).is_empty());
    }
}
