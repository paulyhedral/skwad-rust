use std::fs;
use std::path::Path;

use skwad_git::{ChangeType, Repository, Runner};

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
fn status_reports_staged_unstaged_and_untracked() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path();
    init_repo(path);
    let run = |args: &[&str]| {
        Runner::new(path).run(args).unwrap();
    };

    fs::write(path.join("tracked.txt"), "one\n").unwrap();
    fs::write(path.join("mod.txt"), "a\n").unwrap();
    run(&["add", "-A"]);
    run(&["commit", "-qm", "init"]);

    fs::write(path.join("tracked.txt"), "one\ntwo\n").unwrap();
    run(&["add", "tracked.txt"]);
    fs::write(path.join("mod.txt"), "a\nb\n").unwrap();
    fs::write(path.join("new.txt"), "x\n").unwrap();

    let status = Repository::open(path).status().unwrap();

    assert_eq!(status.head.as_deref(), Some("main"));
    assert_eq!(status.staged().count(), 1);
    assert_eq!(status.modified().count(), 1);
    assert_eq!(status.untracked().count(), 1);
    assert!(!status.is_clean());
}

#[test]
fn status_reports_staged_rename_with_original_path() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path();
    init_repo(path);
    let run = |args: &[&str]| {
        Runner::new(path).run(args).unwrap();
    };

    fs::write(path.join("old.txt"), "content\n").unwrap();
    run(&["add", "-A"]);
    run(&["commit", "-qm", "init"]);
    run(&["mv", "old.txt", "new.txt"]);

    let status = Repository::open(path).status().unwrap();
    let entry = status
        .entries
        .iter()
        .find(|e| e.staged == Some(ChangeType::Renamed))
        .expect("a staged rename entry");

    assert_eq!(entry.path, Path::new("new.txt"));
    assert_eq!(entry.orig_path.as_deref(), Some(Path::new("old.txt")));
}

#[test]
fn clean_repo_is_clean() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path();
    init_repo(path);
    Runner::new(path)
        .run(&["commit", "-qm", "empty", "--allow-empty"])
        .unwrap();

    let status = Repository::open(path).status().unwrap();

    assert!(status.is_clean());
    assert_eq!(status.entries.len(), 0);
}
