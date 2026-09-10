use skwad_git::consts;
use skwad_git::{GitError, Runner};

#[test]
fn version_returns_trimmed_stdout() {
    let dir = tempfile::tempdir().unwrap();
    let out = Runner::new(dir.path()).run(consts::VERSION).unwrap();

    assert!(out.starts_with("git version"), "unexpected: {out:?}");
    assert_eq!(out, out.trim(), "output not trimmed");
}

#[test]
fn non_zero_exit_carries_output_and_code() {
    let dir = tempfile::tempdir().unwrap();
    let err = Runner::new(dir.path())
        .run(&["rev-parse", "--bogus"])
        .unwrap_err();

    match err {
        GitError::Command {
            command,
            output,
            code,
        } => {
            assert_eq!(command, "rev-parse --bogus");
            assert_ne!(code, 0);
            assert!(!output.is_empty());
        }
        other => panic!("expected Command, got {other:?}"),
    }
}
