use std::path::Path;

use skwad_git::{is_working_tree, GitError, Repository, Runner};

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

#[test]
fn create_worktree_makes_a_linked_working_tree() {
    let dir = tempfile::tempdir().unwrap();
    let repo_path = dir.path().join("repo");
    std::fs::create_dir(&repo_path).unwrap();
    init_repo(&repo_path);

    let dest = dir.path().join("repo-feature");
    Repository::open(&repo_path)
        .create_worktree("feature", &dest)
        .unwrap();

    assert!(is_working_tree(&dest));
    // A linked worktree's `.git` is a file, not a directory.
    assert!(dest.join(".git").is_file());
}

#[test]
fn existing_branch_name_fails_and_creates_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let repo_path = dir.path().join("repo");
    std::fs::create_dir(&repo_path).unwrap();
    init_repo(&repo_path);
    let repo = Repository::open(&repo_path);

    repo.create_worktree("dup", &dir.path().join("repo-dup-1"))
        .unwrap();

    let dest = dir.path().join("repo-dup-2");
    let err = repo.create_worktree("dup", &dest).unwrap_err();

    match err {
        GitError::Command { code, .. } => assert_ne!(code, 0),
        other => panic!("expected GitError::Command, got {other:?}"),
    }
    assert!(!dest.exists());
}
