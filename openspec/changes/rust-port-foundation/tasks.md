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

- [ ] 4.1 Model `RepoStatus`, `FileEntry`, `ChangeType {Modified,Added,Deleted,Renamed,Copied,Untracked,Unmerged,Ignored}`, with staged + unstaged change types and retained original path for rename/copy; verify `cargo build -p skwad-git`.
- [ ] 4.2 Implement `parse_status(&str) -> RepoStatus` for `porcelain=v2 --branch`: `# branch.head/upstream/ab` lines and `1`/`2`/`?`/`u` entries; verify `insta` snapshot over a fixture covering the "Mixed working tree" scenario (one each staged/modified/untracked, `is_clean == false`).
- [ ] 4.3 Add derived groupings (`staged`, `modified`, `untracked`, `conflicted`, `is_clean`) and rename handling; verify snapshot test for the "Rename keeps original path" scenario (change type `Renamed`, original path recorded).
- [ ] 4.4 Wire `Repository::status()` to run the command then `parse_status`; verify integration test against a temp repo with a staged, an unstaged, and an untracked file matches the scenario.

## 5. Diff parsing (spec: Diff parsing)

- [ ] 5.1 Model `FileDiff { path, old_path, binary, hunks }` and `Hunk { header, old_start, old_count, new_start, new_count, lines }` with `LineKind {Context,Addition,Deletion,Header,HunkHeader}` and old/new line numbers; verify build.
- [ ] 5.2 Implement `parse_diff(&str) -> Vec<FileDiff>` for `git diff --no-color` incl. `--staged` and single-path output; parse `@@ -a,b +c,d @@` with counts defaulting to 1; verify snapshot for "Hunk header without counts" (`@@ -10 +10 @@` -> counts 1).
- [ ] 5.3 Handle binary file diffs (set `binary`, no hunks); verify snapshot for the "Binary file" scenario.
- [ ] 5.4 Expose per-file additions/deletions derived by counting classified lines; verify unit test on a multi-hunk fixture.

## 6. Combined diff stats (spec: Combined diff stats)

- [ ] 6.1 Implement `parse_numstat(&str) -> Vec<(u64,u64,PathBuf)>` treating `-` as binary (no line delta); verify unit test over a fixture with one text and one binary row.
- [ ] 6.2 Implement `Repository::diff_stats()`: one `diff --numstat` + one `diff --staged --numstat`, then add untracked files by counting their lines (unreadable/binary untracked = 1 file, 0 delta); verify integration test for "Untracked file contributes lines" (12-line file -> +12 insertions, +1 file).

## 7. Staging, commit, discard (spec: Staging and commit operations)

- [ ] 7.1 Implement `stage(paths)`, `unstage(paths)`, `stage_all()`, `unstage_all()`, `discard(paths)`, `commit(message)` as runner wrappers with the exact argv from the spec; verify integration test: stage a file then `status()` shows it staged, `commit` produces a new HEAD.
- [ ] 7.2 Make path-scoped ops a no-op on an empty slice (no process spawned); verify test for "Discard is path-scoped" (spy/temp-repo shows no `git` invocation).
- [ ] 7.3 Propagate runner errors unchanged; verify test for "Commit failure propagates" (commit with nothing staged returns the `GitError::Command`).

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
