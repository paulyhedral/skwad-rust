## 1. Module and consts

- [x] 1.1 Add `WORKTREE_ADD: &[&str] = &["worktree", "add", "-b"]` to `crates/skwad-git/src/consts.rs`; verify `cargo build -p skwad-git`.
- [x] 1.2 Create `crates/skwad-git/src/worktree.rs` and declare `pub mod worktree;` in `lib.rs`; verify `cargo build -p skwad-git`.

## 2. Detection and path suggestion (spec: Working-tree detection, Suggested destination path)

- [x] 2.1 Implement `is_working_tree(path: &Path) -> bool` as `path.join(".git").exists()`; verify a unit test: a temp dir with a `.git` dir is true, a bare temp dir is false. Done: `worktree::tests::detects_git_dir_and_rejects_plain_dir`.
- [x] 2.2 Implement `suggest_worktree_path(repo: &Path, branch: &str) -> PathBuf` — sibling `<repo-name>-<sanitized>`, sanitize `/` and space to `-`; verify unit tests for the spec scenario (`/src/app` + `feat/login` -> `/src/app-feat-login`) and a space-in-branch case. Done: `slash_in_branch_name_is_sanitized`, `space_in_branch_name_is_sanitized`.

## 3. Create worktree (spec: Create worktree on a new branch)

- [x] 3.1 Implement `Repository::create_worktree(&self, branch: &str, destination: &Path) -> Result<()>` running `consts::WORKTREE_ADD` + branch + destination through the runner; verify an integration test in `tests/worktree.rs`: create a worktree from a temp repo, then `is_working_tree(destination)` is true and its `.git` is a file (linked worktree). Done: `tests/worktree.rs::create_worktree_makes_a_linked_working_tree`. Non-UTF-8 destination -> `GitError::Parse`.
- [x] 3.2 Verify branch-already-exists propagates: integration test calling `create_worktree` twice with the same branch returns `GitError::Command` with `code != 0` and no second worktree on disk. Done: `tests/worktree.rs::existing_branch_name_fails_and_creates_nothing`.

## 4. Exports and verification

- [x] 4.1 Re-export `is_working_tree` and `suggest_worktree_path` from `lib.rs` and mention worktree ops in the crate doc comment; verify `cargo doc -p skwad-git` builds with no warnings. Done. README updated too.
- [x] 4.2 Run `make rust-fmt rust-lint rust-test` — all pass, new tests included; `openspec validate worktree-management-port --strict` -> valid. Done: exit 0; 33 tests (2 skwad-core + 31 skwad-git).
- [x] 4.3 Cross-check every `worktree-management` spec scenario against a test:

  | Requirement | Scenario | Test |
  |---|---|---|
  | Working-tree detection | Linked worktree is detected | `tests/worktree.rs::create_worktree_makes_a_linked_working_tree` (asserts `.git` is a file), `worktree::tests::detects_git_dir_and_rejects_plain_dir` |
  | Create worktree on a new branch | Branch already exists | `tests/worktree.rs::existing_branch_name_fails_and_creates_nothing` |
  | Suggested destination path | Slash in branch name | `worktree::tests::slash_in_branch_name_is_sanitized` |
