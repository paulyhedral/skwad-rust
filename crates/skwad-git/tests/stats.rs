use std::fs;
use std::path::Path;

use skwad_git::{Repository, Runner};

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
fn untracked_file_contributes_its_lines_and_one_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path();
    init_repo(path);
    Runner::new(path)
        .run(&["commit", "-qm", "empty", "--allow-empty"])
        .unwrap();

    let body: String = (0..12).map(|n| format!("line {n}\n")).collect();
    fs::write(path.join("notes.txt"), body).unwrap();

    let stats = Repository::open(path).diff_stats().unwrap();

    assert_eq!(stats.insertions, 12);
    assert_eq!(stats.deletions, 0);
    assert_eq!(stats.files_changed, 1);
}

#[test]
fn staged_and_unstaged_changes_sum() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path();
    init_repo(path);
    let run = |args: &[&str]| {
        Runner::new(path).run(args).unwrap();
    };

    fs::write(path.join("a.txt"), "1\n2\n3\n").unwrap();
    fs::write(path.join("b.txt"), "x\n").unwrap();
    run(&["add", "-A"]);
    run(&["commit", "-qm", "init"]);

    fs::write(path.join("a.txt"), "1\n2\n3\n4\n5\n").unwrap();
    run(&["add", "a.txt"]);
    fs::write(path.join("b.txt"), "x\ny\n").unwrap();

    let stats = Repository::open(path).diff_stats().unwrap();

    assert_eq!(stats.insertions, 3);
    assert_eq!(stats.deletions, 0);
    assert_eq!(stats.files_changed, 2);
}
