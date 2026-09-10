use std::path::{Path, PathBuf};

/// True when `path` holds a `.git` entry, whether that entry is a directory
/// (a primary clone) or a file (a linked worktree).
pub fn is_working_tree(path: &Path) -> bool {
    path.join(".git").exists()
}

/// The suggested destination for a new worktree of `repo` on `branch`: a
/// sibling directory named `<repo-name>-<sanitized-branch>`, where sanitizing
/// replaces `/` and space with `-`.
pub fn suggest_worktree_path(repo: &Path, branch: &str) -> PathBuf {
    let repo_name = repo
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let sanitized: String = branch
        .chars()
        .map(|c| match c {
            '/' | ' ' => '-',
            other => other,
        })
        .collect();
    let sibling = format!("{repo_name}-{sanitized}");

    match repo.parent() {
        Some(parent) => parent.join(sibling),
        None => PathBuf::from(sibling),
    }
}

#[cfg(test)]
mod tests {
    use super::{is_working_tree, suggest_worktree_path};
    use std::path::{Path, PathBuf};

    #[test]
    fn detects_git_dir_and_rejects_plain_dir() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!is_working_tree(dir.path()));

        std::fs::create_dir(dir.path().join(".git")).unwrap();
        assert!(is_working_tree(dir.path()));
    }

    #[test]
    fn slash_in_branch_name_is_sanitized() {
        assert_eq!(
            suggest_worktree_path(Path::new("/src/app"), "feat/login"),
            PathBuf::from("/src/app-feat-login"),
        );
    }

    #[test]
    fn space_in_branch_name_is_sanitized() {
        assert_eq!(
            suggest_worktree_path(Path::new("/src/app"), "quick fix"),
            PathBuf::from("/src/app-quick-fix"),
        );
    }
}
