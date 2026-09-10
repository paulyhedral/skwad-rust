use std::fs;
use std::path::Path;
use std::time::Duration;

use skwad_discovery::{scan, Discovery};
use tokio::time::timeout;

fn mk_repo(base: &Path, name: &str) {
    let git = base.join(name).join(".git");
    fs::create_dir_all(&git).unwrap();
    fs::write(git.join("HEAD"), "ref: refs/heads/main\n").unwrap();
}

async fn no_update_within(
    rx: &mut tokio::sync::watch::Receiver<Vec<skwad_discovery::RepoInfo>>,
    secs: u64,
) {
    assert!(
        timeout(Duration::from_secs(secs), rx.changed())
            .await
            .is_err(),
        "expected no further update"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn missing_path_yields_empty_and_no_watch() {
    let tmp = tempfile::tempdir().unwrap();
    let missing = tmp.path().join("does-not-exist");

    let (discovery, mut rx) = Discovery::new();
    discovery.set_source_folder(Some(missing)).unwrap();

    assert!(rx.borrow_and_update().is_empty());

    // Activity elsewhere must not trigger a rescan - nothing is watched.
    mk_repo(tmp.path(), "app");
    no_update_within(&mut rx, 2).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn rapid_child_creation_coalesces_to_one_rescan() {
    let base = tempfile::tempdir().unwrap();

    let (discovery, mut rx) = Discovery::new();
    discovery
        .set_source_folder(Some(base.path().to_path_buf()))
        .unwrap();
    assert!(rx.borrow_and_update().is_empty());

    for name in ["alpha", "beta", "gamma"] {
        mk_repo(base.path(), name);
    }

    timeout(Duration::from_secs(5), rx.changed())
        .await
        .expect("a rescan should fire")
        .unwrap();
    let repos = rx.borrow_and_update();
    let mut names: Vec<_> = repos.iter().map(|r| r.name.clone()).collect();
    names.sort();
    assert_eq!(names, ["alpha", "beta", "gamma"]);
    drop(repos);

    no_update_within(&mut rx, 2).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn switching_source_folder_settles_on_the_last() {
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let c = tempfile::tempdir().unwrap();
    mk_repo(c.path(), "gamma");

    let (discovery, mut rx) = Discovery::new();
    discovery
        .set_source_folder(Some(a.path().to_path_buf()))
        .unwrap();
    discovery
        .set_source_folder(Some(b.path().to_path_buf()))
        .unwrap();
    discovery
        .set_source_folder(Some(c.path().to_path_buf()))
        .unwrap();

    let settled = rx.borrow_and_update().clone();
    let expected = scan(&c.path().canonicalize().unwrap());
    assert_eq!(settled, expected);
    assert_eq!(settled.len(), 1);
    assert_eq!(settled[0].name, "gamma");

    // Watchers for a and b were aborted; changes there are ignored.
    mk_repo(a.path(), "stale-a");
    mk_repo(b.path(), "stale-b");
    no_update_within(&mut rx, 2).await;
}
