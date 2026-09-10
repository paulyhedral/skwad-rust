use std::fs;
use std::path::PathBuf;

use crate::consts;
use crate::error::{GitError, Result};
use crate::runner::Runner;
use crate::stats::{parse_numstat, untracked_line_count, DiffStats};
use crate::status::{parse_status, RepoStatus};

/// A git repository addressed by working directory. All operations run through
/// a [`Runner`], so they share its timeout and working directory.
#[derive(Debug, Clone)]
pub struct Repository {
    runner: Runner,
}

impl Repository {
    pub fn open(path: impl Into<PathBuf>) -> Self {
        Self {
            runner: Runner::new(path),
        }
    }

    pub fn with_runner(runner: Runner) -> Self {
        Self { runner }
    }

    pub fn runner(&self) -> &Runner {
        &self.runner
    }

    /// Parsed `git status --porcelain=v2 --branch`.
    pub fn status(&self) -> Result<RepoStatus> {
        let output = self.runner.run(consts::STATUS)?;
        Ok(parse_status(&output))
    }

    /// Combined line changes: `git diff --numstat` plus `git diff --staged
    /// --numstat`, then each untracked file as one changed file with its line
    /// count as insertions (binary or unreadable untracked files add a file
    /// with no line delta).
    pub fn diff_stats(&self) -> Result<DiffStats> {
        let unstaged = parse_numstat(&self.runner.run(consts::NUMSTAT)?);
        let staged = parse_numstat(&self.runner.run(consts::NUMSTAT_STAGED)?);

        let mut stats = DiffStats::default();
        for (added, deleted, _) in unstaged.iter().chain(&staged) {
            stats.insertions += added;
            stats.deletions += deleted;
            stats.files_changed += 1;
        }

        for entry in self.status()?.untracked() {
            let lines = untracked_line_count(fs::read(self.runner.cwd().join(&entry.path)));
            stats.insertions += lines.unwrap_or(0);
            stats.files_changed += 1;
        }

        Ok(stats)
    }

    /// `git add <paths>`. Empty slice is a no-op (no process spawned).
    pub fn stage(&self, paths: &[&str]) -> Result<()> {
        self.run_scoped(consts::ADD, paths)
    }

    /// `git restore --staged <paths>`. Empty slice is a no-op.
    pub fn unstage(&self, paths: &[&str]) -> Result<()> {
        self.run_scoped(consts::RESTORE_STAGED, paths)
    }

    /// `git restore <paths>`. Empty slice is a no-op.
    pub fn discard(&self, paths: &[&str]) -> Result<()> {
        self.run_scoped(consts::RESTORE, paths)
    }

    /// `git add -A`.
    pub fn stage_all(&self) -> Result<()> {
        self.runner.run(consts::ADD_ALL).map(drop)
    }

    /// `git reset HEAD`.
    pub fn unstage_all(&self) -> Result<()> {
        self.runner.run(consts::UNSTAGE_ALL).map(drop)
    }

    /// `git commit -m <message>`.
    pub fn commit(&self, message: &str) -> Result<()> {
        let argv = [consts::COMMIT, &[message]].concat();
        self.runner.run(&argv).map(drop)
    }

    /// Current branch from `git branch --show-current`. `None` when HEAD is
    /// detached (empty output).
    pub fn current_branch(&self) -> Result<Option<String>> {
        let branch = self.runner.run(consts::BRANCH_SHOW_CURRENT)?;
        Ok((!branch.is_empty()).then_some(branch))
    }

    /// Whether the branch has commits its upstream lacks
    /// (`git log @{u}.. --oneline`). `false` when there is no upstream.
    pub fn has_unpushed(&self) -> Result<bool> {
        match self.runner.run(consts::LOG_UNPUSHED) {
            Ok(output) => Ok(!output.is_empty()),
            Err(GitError::Command { .. }) => Ok(false),
            Err(err) => Err(err),
        }
    }

    /// `(ahead, behind)` relative to the upstream from
    /// `git rev-list --left-right --count @{u}...HEAD`. `(0, 0)` when there is
    /// no upstream.
    pub fn ahead_behind(&self) -> Result<(u32, u32)> {
        let output = match self.runner.run(consts::AHEAD_BEHIND) {
            Ok(output) => output,
            Err(GitError::Command { .. }) => return Ok((0, 0)),
            Err(err) => return Err(err),
        };

        // left-right count prints "<behind>\t<ahead>": left = @{u}, right = HEAD.
        let mut counts = output.split_whitespace();
        let behind = counts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
        let ahead = counts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
        Ok((ahead, behind))
    }

    fn run_scoped(&self, base: &[&str], paths: &[&str]) -> Result<()> {
        if paths.is_empty() {
            return Ok(());
        }

        let argv = [base, paths].concat();
        self.runner.run(&argv).map(drop)
    }
}

#[cfg(test)]
mod tests {
    use super::Repository;
    use crate::runner::Runner;

    /// A path-scoped op with no paths must not spawn git. `false` always exits
    /// non-zero, so a spawn would surface as an error.
    #[test]
    fn empty_path_scoped_ops_do_not_spawn() {
        let dir = tempfile::tempdir().unwrap();
        let repo = Repository::with_runner(Runner::new(dir.path()).with_program("false"));

        assert!(repo.stage(&[]).is_ok());
        assert!(repo.unstage(&[]).is_ok());
        assert!(repo.discard(&[]).is_ok());
    }
}
