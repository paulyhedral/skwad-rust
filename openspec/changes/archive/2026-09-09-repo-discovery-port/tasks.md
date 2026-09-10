## 1. Crate scaffold and dependencies

- [x] 1.1 Add `notify` and `tokio` (features `rt`, `sync`, `time`, `macros`, `rt-multi-thread`) to `[workspace.dependencies]` in the root `Cargo.toml`; verify `cargo metadata` resolves.
- [x] 1.2 Create `crates/skwad-discovery/` with `Cargo.toml` (deps: `thiserror`, `notify`, `tokio`; dev-deps: `tempfile`, `tokio` test macros), add it to workspace `members`; `src/lib.rs` with module decls and a crate doc comment naming `openspec/specs/repo-discovery/spec.md` as the contract; verify `cargo build -p skwad-discovery`.
- [x] 1.3 Add `src/consts.rs` with `DEBOUNCE: Duration` (1s) and the parse literals (`GIT_DIR`, `HEAD`, `HEAD_REF_PREFIX`, `GITDIR_PREFIX`, `WORKTREES_MARKER`); add `src/error.rs` with `DiscoveryError` (`Watch(#[from] notify::Error)`) and `type Result<T, E = DiscoveryError>`; verify `cargo build -p skwad-discovery`.

## 2. Types (spec: Worktree naming, Result ordering)

- [x] 2.1 Define `RepoInfo { name: String, worktrees: Vec<WorktreeInfo> }` and `WorktreeInfo { name: String, path: PathBuf }` in `src/scan.rs`, `#[derive(Debug, Clone, PartialEq, Eq)]`; re-export both from `lib.rs`; verify `cargo build -p skwad-discovery`.

## 3. Filesystem scan (spec: Filesystem-only scan, Worktree naming, Result ordering)

- [x] 3.1 Implement `parse_branch_from_head(git_dir: &Path) -> Option<String>` reading `<git_dir>/HEAD` and returning the value after `ref: refs/heads/`; unit test: a `ref:` HEAD returns the branch, a detached (raw SHA) HEAD returns `None`.
- [x] 3.2 Implement `parse_worktree_gitfile(git_file: &Path) -> Option<PathBuf>` reading a `.git` file, taking the `gitdir: ` value and cutting at `/.git/worktrees/`; unit test: a real linked-worktree `.git` file resolves to the owning repo path; a file without the marker returns `None`.
- [x] 3.3 Implement `scan(base: &Path) -> Vec<RepoInfo>`: `read_dir` failure -> empty; `.git` dir -> repo (branch via 3.1 else folder name, primary worktree at index 0); `.git` file -> resolve via 3.2 and attach as a worktree of the matching repo with the `<repo>-` prefix stripped; drop worktrees whose owning repo is not in the scanned set; sort repos case-insensitively by name. Verify with unit tests in 3.4-3.7.
- [x] 3.4 Unit test - clone + linked worktree group: temp base with `app/` (`.git` dir) and `app-feat/` (`.git` file pointing into `app/.git/worktrees/`); `scan` returns one repo `app` whose worktrees are `[app, feat]` in that order.
- [x] 3.5 Unit test - detached HEAD: a clone whose `.git/HEAD` holds a raw SHA yields a primary worktree named after the folder.
- [x] 3.6 Unit test - prefix stripping: repo `app` with worktree folder `app-hotfix` yields a worktree named `hotfix`.
- [x] 3.7 Unit test - ordering: base with `Zebra/` and `apple/` returns `apple` before `Zebra`.

## 4. Debounced coordinator (spec: Debounced refresh)

- [x] 4.1 Implement `Discovery::new() -> (Discovery, watch::Receiver<Vec<RepoInfo>>)` and internal state (current base + current watcher `JoinHandle`) behind a `Mutex`; verify `cargo build -p skwad-discovery`.
- [x] 4.2 Implement `Discovery::set_source_folder(&self, path: Option<PathBuf>) -> Result<()>`: abort the previous watcher task; for `None` / empty / missing / non-directory send `Vec::new()` and return `Ok(())` with no watch; otherwise `spawn_blocking(scan)`, send the result, then spawn the watcher task (task creation errors from `notify` propagate as `DiscoveryError::Watch`).
- [x] 4.3 Implement the relevance check `is_relevant(event_path, base) -> bool` - true only when the path is within `base` and touches `base`, a direct child, or `<child>/.git`; unit test: a path several levels inside a working tree is not relevant; a new direct child folder is relevant.
- [x] 4.4 Implement the watcher task: `notify::recommended_watcher` on `base` (non-recursive) feeding a `tokio::sync::mpsc`; loop arming a `tokio::time::Sleep(DEBOUNCE)` on each relevant event, resetting it on the next relevant event, and on fire `spawn_blocking(scan)` + send. Verify with the integration tests in section 5.

## 5. Integration tests (spec: Debounced refresh)

- [x] 5.1 `tests/discovery.rs` - missing path: `set_source_folder(Some(<nonexistent>))` leaves the receiver at `[]` and starts no watch (a later file create under a sibling path produces no send within 2s).
- [x] 5.2 `tests/discovery.rs` - rapid changes coalesce: point `set_source_folder` at a temp dir, then create three child repo folders within the debounce window; assert the receiver observes exactly one post-initial update and its value reflects all three.
- [x] 5.3 `tests/discovery.rs` - source-folder switch: call `set_source_folder` three times in quick succession at three different temp dirs; the final settled result matches `scan` of the last dir, and events in the first two dirs after the switch produce no further sends.

## 6. Exports and verification

- [x] 6.1 Re-export `scan`, `RepoInfo`, `WorktreeInfo`, `Discovery`, `DiscoveryError` from `lib.rs`; verify `cargo doc -p skwad-discovery` builds with no warnings.
- [x] 6.2 Run `make rust-fmt rust-lint rust-test` - all pass with the new crate included; `openspec validate repo-discovery-port --strict` -> valid.
- [x] 6.3 Cross-check every `repo-discovery` spec scenario against a test:

  | Requirement | Scenario | Test |
  |---|---|---|
  | Filesystem-only scan | Clone and its linked worktree group together | 3.4 |
  | Filesystem-only scan | Detached HEAD falls back to folder name | 3.5 |
  | Worktree naming | Prefix stripped | 3.6 |
  | Result ordering | Alphabetical repos | 3.7 |
  | Debounced refresh | Rapid source-folder changes coalesce | 5.2, 5.3 |
  | Debounced refresh | Missing / non-directory path yields empty, no watch | 5.1 |
