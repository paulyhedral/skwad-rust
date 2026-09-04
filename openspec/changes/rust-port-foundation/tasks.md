## 1. Workspace scaffold

- [x] 1.1 Add root virtual `Cargo.toml` with `members = ["crates/*"]` and a shared `[workspace.package]` (edition 2021, license, repo); verify `cargo metadata` succeeds. Done: `AGPL-3.0-only`, repo `Kochava-Studios/skwad`, `[workspace.dependencies]` for thiserror/tempfile/insta.
- [x] 1.2 Add `rust-toolchain.toml` pinning a stable channel plus the `rustfmt`/`clippy` components; verify `rustc --version` matches. Done: pinned `1.98.0`.
- [x] 1.3 Add `/target/` and `**/*.rs.bk` to `.gitignore`; verify `git status` stays clean after `cargo build`. Done: `git check-ignore target` passes, no stray files.
- [x] 1.4 Create `crates/skwad-core` (lib) with `error.rs` (`Error` via `thiserror`, `Result<T>` alias), `consts.rs`, and `t(key) -> String` (in `l10n.rs`) backed by a static match; verify `cargo test -p skwad-core`. Done: 2 tests pass (`app.name` -> `Skwad`, unknown -> key).
- [ ] 1.5 Wire gpui: add it as a git dependency pinned to an exact rev, `main` opens an empty window and exits on close; verify `cargo run -p skwad --features gui` shows a window. DEFERRED — no network to resolve a known-good Zed rev; bin currently builds with a feature-gated `unimplemented!()` stub. Follow-up issue to be opened.
- [x] 1.6 Feature-gate the window behind `--features gui` with a no-op default `main`; verify `cargo build -p skwad` (default, no features) passes. Done: default `main` prints `Skwad: gui feature not built`; build + clippy clean.

## 2. skwad-git crate skeleton

- [x] 2.1 Create `crates/skwad-git` (lib), no tokio/gpui/axum deps; `thiserror` + dev-deps `insta`/`tempfile`; verify `cargo tree` shows no banned runtime crates. Done: `cargo tree -e normal | grep -iE 'tokio|gpui|axum|hyper|mio'` -> none.
- [x] 2.2 Add `consts.rs` with `DEFAULT_TIMEOUT: Duration` (30s) and argv arrays for every git subcommand in the spec; verify `cargo build -p skwad-git`. Done: 15 argv consts + `DIFF_STAGED_FLAG`.
- [x] 2.3 Define `GitError` (`Timeout { command }`, `Command { command, output, code }`, `Io`, `Parse`) + `Result<T>` alias with `thiserror`; verify unit tests assert `Display` carries the command string. Done: 2 tests pass (Timeout + Command Display).

## 3. Command runner (spec: Command runner)

- [x] 3.1 Implement `Runner { cwd, timeout, program }` with `run(&[&str]) -> Result<String>`: spawn `git`, drain stdout/stderr on worker threads, poll `try_wait` to a deadline, `kill()`+`wait()` on elapse, trim stdout; verify integration test `run(consts::VERSION)` returns trimmed `git version ...` in a temp dir. Done. Note: poll-loop instead of design's `recv_timeout` channel — same behavior, simpler; drain threads prevent pipe-buffer wedging.
- [x] 3.2 Map non-zero exit to `GitError::Command` (stderr, or stdout when stderr empty; + exit code); verify `run(["rev-parse","--bogus"])` -> `Command`, `code != 0`, non-empty `output`. Done.
- [x] 3.3 Enforce the timeout via a `#[cfg(test)]` `with_program` seam: verify `with_program("sleep").with_timeout(50ms).run(["5"])` returns `GitError::Timeout { command: "5" }` and aborts in < 2s (child killed). Done.

## 4. Status parsing (spec: Repository status)

- [x] 4.1 Model `RepoStatus`, `FileEntry`, `ChangeType {Modified,Added,Deleted,Renamed,Copied,Untracked,Unmerged,Ignored}`, staged + unstaged change types, `orig_path` for rename/copy; verify build. Done (`status.rs`; `T` type-changed maps to `Modified`).
- [x] 4.2 Implement `parse_status(&str) -> RepoStatus` for `porcelain=v2 --branch`: `# branch.head/upstream/ab` + `1`/`2`/`u`/`?`/`!` entries via `splitn`; verify `insta` snapshot for "Mixed working tree" (1 staged / 1 modified / 1 untracked, `is_clean == false`). Done: `mixed_working_tree` snapshot.
- [x] 4.3 Add `staged()`/`modified()`/`untracked()`/`conflicted()`/`is_clean()` and rename handling (`\t`-split new/orig); verify "Rename keeps original path" snapshot (`Renamed`, `orig_path` recorded). Done: `rename_keeps_original_path` snapshot.
- [x] 4.4 Add `Repository::open`/`with_runner` + `status()`; verify integration tests against temp repos: staged+unstaged+untracked counts, `git mv` rename with original path, clean repo -> `is_clean()`. Done: 3 tests in `tests/status.rs`.

## 5. Diff parsing (spec: Diff parsing)

- [x] 5.1 Model `FileDiff { path, old_path, binary, hunks }`, `Hunk { header, old_start, old_count, new_start, new_count, lines }`, `DiffLine { kind, text, old_lineno, new_lineno }`, `LineKind {Context,Addition,Deletion,Header,HunkHeader}`; verify build. Done (`diff.rs`).
- [x] 5.2 Implement `parse_diff(&str) -> Vec<FileDiff>` for `git diff --no-color` (also `--staged` / single-path shapes); `@@ -a,b +c,d @@` counts default to 1; running old/new line numbers per line. Verify "Hunk header without counts" snapshot (`@@ -10 +10 @@` -> counts 1). Done.
- [x] 5.3 Detect `Binary files ... differ` -> `binary = true`, no hunks; verify "Binary file" snapshot. Done.
- [x] 5.4 `FileDiff::additions()`/`deletions()` count classified hunk lines; public `classify(&str) -> LineKind`. Verify multi-hunk count test (3 add / 2 del) + `classify_covers_every_kind`. Done.

## 6. Combined diff stats (spec: Combined diff stats)

- [x] 6.1 Implement `parse_numstat(&str) -> Vec<(u64,u64,PathBuf)>` (`-` -> 0, binary) + `untracked_line_count(io::Result<Vec<u8>>)` (None on NUL/unreadable); verify unit tests: text+binary rows, newline count + binary/err rejection. Done (`stats.rs`).
- [x] 6.2 Implement `Repository::diff_stats() -> DiffStats`: `diff --numstat` + `diff --staged --numstat` summed, then each untracked file as +1 file with its line count as insertions (binary/unreadable -> +1 file, +0); verify "Untracked file contributes lines" (12-line -> 12 ins / 1 file) + staged+unstaged sum test. Done (`tests/stats.rs`).

## 7. Staging, commit, discard (spec: Staging and commit operations)

- [x] 7.1 `stage`/`unstage`/`discard` (path-scoped), `stage_all`/`unstage_all`, `commit(message)` as runner wrappers with the spec's exact argv; verify `stage_then_commit_moves_head` (status staged as `Added`, HEAD changes, clean after) + `unstage_and_discard_round_trip`. Done.
- [x] 7.2 Path-scoped ops return `Ok(())` on an empty slice without spawning; verify unit test with `with_program("false")` (a spawn would error) + `discard_with_empty_paths_is_a_no_op` (dirty file untouched). Done.
- [x] 7.3 Runner errors propagate unchanged; verify `commit_failure_propagates` (nothing staged -> `GitError::Command`, `command == "commit -m nothing staged"`, `code != 0`). Done.

## 8. Branch and ahead/behind (spec: Branch and ahead/behind queries)

- [ ] 8.1 Implement `current_branch() -> Option<String>` from `branch --show-current` (empty -> `None`); verify integration test on a temp repo and on a detached HEAD.
- [ ] 8.2 Implement `has_unpushed()` from `log @{u}.. --oneline` and `ahead_behind() -> (u32,u32)` from `rev-list --left-right --count @{u}...HEAD`; verify test: no upstream -> `(0,0)` and `has_unpushed() == false` for the "No upstream" scenario.

## 9. Build, lint, CI

- [ ] 9.1 Add `Makefile` targets `rust-fmt` (`cargo +nightly fmt --check`), `rust-lint` (`cargo clippy --all-targets -- -D warnings`), `rust-test` (`cargo test --workspace`), `rust-build`; verify each runs green locally.
- [ ] 9.2 Add a GitHub Actions workflow running those four targets on `macos-latest` and `ubuntu-latest` with cargo caching, pinning a `git` version; verify the workflow passes on the change branch.
- [ ] 9.3 Add `crates/skwad-git/README.md` noting the `git` binary requirement and minimum tested version; verify it renders and is referenced from the crate root doc comment.

## 10. Verification

- [ ] 10.1 Cross-check every scenario in `openspec/specs/git-operations/spec.md` against a test in `skwad-git`; verify a checklist in the PR maps each scenario name to its test function.
- [ ] 10.2 Run `openspec validate rust-port-foundation` and `make rust-fmt rust-lint rust-test`; verify all pass.
