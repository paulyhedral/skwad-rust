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
    run(&["commit", "-qm", "root", "--allow-empty"]);
}

#[test]
fn current_branch_reports_name_then_none_when_detached() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path();
    init_repo(path);
    let repo = Repository::open(path);

    assert_eq!(repo.current_branch().unwrap().as_deref(), Some("main"));

    Runner::new(path)
        .run(&["checkout", "--detach", "-q"])
        .unwrap();
    assert_eq!(repo.current_branch().unwrap(), None);
}

#[test]
fn no_upstream_means_zero_counts_and_not_unpushed() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path();
    init_repo(path);
    let repo = Repository::open(path);

    assert_eq!(repo.ahead_behind().unwrap(), (0, 0));
    assert!(!repo.has_unpushed().unwrap());
}

#[test]
fn ahead_of_upstream_is_counted_and_flagged() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path();
    init_repo(path);
    let run = |args: &[&str]| Runner::new(path).run(args).unwrap();
    run(&["branch", "base"]);
    run(&["branch", "--set-upstream-to=base"]);
    run(&["commit", "-qm", "ahead", "--allow-empty"]);

    let repo = Repository::open(path);

    assert_eq!(repo.ahead_behind().unwrap(), (1, 0));
    assert!(repo.has_unpushed().unwrap());
}
