## 1. Module and consts

- [ ] 1.1 Add `WORKTREE_ADD: &[&str] = &["worktree", "add", "-b"]` to `crates/skwad-git/src/consts.rs`; verify `cargo build -p skwad-git`.
- [ ] 1.2 Create `crates/skwad-git/src/worktree.rs` and declare `pub mod worktree;` in `lib.rs`; verify `cargo build -p skwad-git`.

## 2. Detection and path suggestion (spec: Working-tree detection, Suggested destination path)

- [ ] 2.1 Implement `is_working_tree(path: &Path) -> bool` as `path.join(".git").exists()`; verify a unit test: a temp dir with a `.git` dir is true, a bare temp dir is false.
- [ ] 2.2 Implement `suggest_worktree_path(repo: &Path, branch: &str) -> PathBuf` — sibling `<repo-name>-<sanitized>`, sanitize `/` and space to `-`; verify unit tests for the spec scenario (`/src/app` + `feat/login` -> `/src/app-feat-login`) and a space-in-branch case.

## 3. Create worktree (spec: Create worktree on a new branch)

- [ ] 3.1 Implement `Repository::create_worktree(&self, branch: &str, destination: &Path) -> Result<()>` running `consts::WORKTREE_ADD` + branch + destination through the runner; verify an integration test in `tests/worktree.rs`: create a worktree from a temp repo, then `is_working_tree(destination)` is true and its `.git` is a file (linked worktree).
- [ ] 3.2 Verify branch-already-exists propagates: integration test calling `create_worktree` twice with the same branch returns `GitError::Command` with `code != 0` and no second worktree on disk.

## 4. Exports and verification

- [ ] 4.1 Re-export `is_working_tree` and `suggest_worktree_path` from `lib.rs` and mention worktree ops in the crate doc comment; verify `cargo doc -p skwad-git` builds with no warnings.
- [ ] 4.2 Run `make rust-fmt rust-lint rust-test` — all pass, new tests included; `openspec validate worktree-management-port --strict` -> valid.
- [ ] 4.3 Cross-check every `worktree-management` spec scenario against a test (linked worktree detected; branch already exists; slash in branch name) in a table in this file.
