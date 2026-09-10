use std::fs;
use std::path::Path;

use skwad_git::{ChangeType, GitError, Repository, Runner};

fn init_repo(dir: &Path) {
    let run = |args: &[&str]| {
        Runner::new(dir).run(args).unwrap();
    };
    run(&["init", "-q", "-b", "main"]);
    run(&["config", "user.email", "test@example.com"]);
    run(&["config", "user.name", "Test"]);
    run(&["config", "commit.gpgsign", "false"]);
}

#[test]
fn stage_then_commit_moves_head() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path();
    init_repo(path);
    let run = |args: &[&str]| Runner::new(path).run(args).unwrap();
    run(&["commit", "-qm", "root", "--allow-empty"]);
    let before = run(&["rev-parse", "HEAD"]);

    fs::write(path.join("f.txt"), "hi\n").unwrap();
    let repo = Repository::open(path);

    repo.stage(&["f.txt"]).unwrap();
    let staged = repo.status().unwrap();
    assert_eq!(staged.staged().count(), 1);
    assert_eq!(
        staged.staged().next().unwrap().staged,
        Some(ChangeType::Added)
    );

    repo.commit("add f").unwrap();
    let after = Runner::new(path).run(&["rev-parse", "HEAD"]).unwrap();
    assert_ne!(before, after);
    assert!(repo.status().unwrap().is_clean());
}

#[test]
fn unstage_and_discard_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path();
    init_repo(path);
    let run = |args: &[&str]| Runner::new(path).run(args).unwrap();
    fs::write(path.join("f.txt"), "one\n").unwrap();
    run(&["add", "-A"]);
    run(&["commit", "-qm", "init"]);

    fs::write(path.join("f.txt"), "one\ntwo\n").unwrap();
    let repo = Repository::open(path);

    repo.stage(&["f.txt"]).unwrap();
    repo.unstage(&["f.txt"]).unwrap();
    assert_eq!(repo.status().unwrap().staged().count(), 0);
    assert_eq!(repo.status().unwrap().modified().count(), 1);

    repo.discard(&["f.txt"]).unwrap();
    assert!(repo.status().unwrap().is_clean());
    assert_eq!(fs::read_to_string(path.join("f.txt")).unwrap(), "one\n");
}

#[test]
fn discard_with_empty_paths_is_a_no_op() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path();
    init_repo(path);
    let run = |args: &[&str]| Runner::new(path).run(args).unwrap();
    fs::write(path.join("f.txt"), "one\n").unwrap();
    run(&["add", "-A"]);
    run(&["commit", "-qm", "init"]);
    fs::write(path.join("f.txt"), "dirty\n").unwrap();

    Repository::open(path).discard(&[]).unwrap();

    assert_eq!(fs::read_to_string(path.join("f.txt")).unwrap(), "dirty\n");
}

#[test]
fn commit_failure_propagates() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path();
    init_repo(path);
    Runner::new(path)
        .run(&["commit", "-qm", "init", "--allow-empty"])
        .unwrap();

    let err = Repository::open(path).commit("nothing staged").unwrap_err();

    match err {
        GitError::Command { command, code, .. } => {
            assert_eq!(command, "commit -m nothing staged");
            assert_ne!(code, 0);
        }
        other => panic!("expected Command, got {other:?}"),
    }
}
