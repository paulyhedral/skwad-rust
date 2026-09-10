use std::path::PathBuf;

/// A single change type from git's porcelain v2 XY codes plus the
/// untracked / ignored pseudo-states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeType {
    Modified,
    Added,
    Deleted,
    Renamed,
    Copied,
    Untracked,
    Unmerged,
    Ignored,
}

impl ChangeType {
    /// Maps one half of an XY code. `.` (unmodified) yields `None`.
    fn from_xy(code: char) -> Option<Self> {
        let change = match code {
            'M' | 'T' => Self::Modified,
            'A' => Self::Added,
            'D' => Self::Deleted,
            'R' => Self::Renamed,
            'C' => Self::Copied,
            'U' => Self::Unmerged,
            _ => return None,
        };

        Some(change)
    }
}

/// One path's status, with separate staged (index vs HEAD) and unstaged
/// (worktree vs index) change types. Rename and copy entries keep the
/// original path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEntry {
    pub path: PathBuf,
    pub orig_path: Option<PathBuf>,
    pub staged: Option<ChangeType>,
    pub unstaged: Option<ChangeType>,
}

/// Parsed `git status --porcelain=v2 --branch` output.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RepoStatus {
    /// Branch name, or `None` when HEAD is detached.
    pub head: Option<String>,
    pub upstream: Option<String>,
    pub ahead: i64,
    pub behind: i64,
    pub entries: Vec<FileEntry>,
}

impl RepoStatus {
    pub fn staged(&self) -> impl Iterator<Item = &FileEntry> {
        self.entries.iter().filter(|e| e.staged.is_some())
    }

    /// Tracked entries with a worktree change (excludes untracked, ignored,
    /// and conflicted).
    pub fn modified(&self) -> impl Iterator<Item = &FileEntry> {
        self.entries.iter().filter(|e| {
            matches!(
                e.unstaged,
                Some(
                    ChangeType::Modified
                        | ChangeType::Added
                        | ChangeType::Deleted
                        | ChangeType::Renamed
                        | ChangeType::Copied
                )
            )
        })
    }

    pub fn untracked(&self) -> impl Iterator<Item = &FileEntry> {
        self.entries
            .iter()
            .filter(|e| e.unstaged == Some(ChangeType::Untracked))
    }

    pub fn conflicted(&self) -> impl Iterator<Item = &FileEntry> {
        self.entries.iter().filter(|e| {
            e.staged == Some(ChangeType::Unmerged) || e.unstaged == Some(ChangeType::Unmerged)
        })
    }

    /// True when no entry carries a staged or unstaged change. Ignored files
    /// are never listed (the status call omits `--ignored`).
    pub fn is_clean(&self) -> bool {
        self.entries
            .iter()
            .all(|e| e.staged.is_none() && e.unstaged.is_none())
    }
}

pub fn parse_status(output: &str) -> RepoStatus {
    let mut status = RepoStatus::default();

    for line in output.lines() {
        if let Some(rest) = line.strip_prefix("# branch.") {
            parse_branch_header(rest, &mut status);
        } else if let Some(entry) = parse_entry(line) {
            status.entries.push(entry);
        }
    }

    status
}

fn parse_branch_header(rest: &str, status: &mut RepoStatus) {
    let (key, value) = match rest.split_once(' ') {
        Some(pair) => pair,
        None => return,
    };

    match key {
        "head" => {
            status.head = (value != "(detached)").then(|| value.to_owned());
        }
        "upstream" => status.upstream = Some(value.to_owned()),
        "ab" => {
            let (ahead, behind) = parse_ab(value);
            status.ahead = ahead;
            status.behind = behind;
        }
        _ => {}
    }
}

fn parse_ab(value: &str) -> (i64, i64) {
    let mut ahead = 0;
    let mut behind = 0;

    for token in value.split_whitespace() {
        match token.split_at(1) {
            ("+", n) => ahead = n.parse().unwrap_or(0),
            ("-", n) => behind = n.parse().unwrap_or(0),
            _ => {}
        }
    }

    (ahead, behind)
}

fn parse_entry(line: &str) -> Option<FileEntry> {
    let (kind, rest) = line.split_once(' ')?;

    match kind {
        "1" => parse_ordinary(rest),
        "2" => parse_rename(rest),
        "u" => parse_unmerged(rest),
        "?" => Some(untracked_like(rest, ChangeType::Untracked)),
        "!" => Some(untracked_like(rest, ChangeType::Ignored)),
        _ => None,
    }
}

fn xy(field: &str) -> (Option<ChangeType>, Option<ChangeType>) {
    let mut chars = field.chars();
    let x = chars.next().and_then(ChangeType::from_xy);
    let y = chars.next().and_then(ChangeType::from_xy);
    (x, y)
}

/// `<XY> <sub> <mH> <mI> <mW> <hH> <hI> <path>`
fn parse_ordinary(rest: &str) -> Option<FileEntry> {
    let mut fields = rest.splitn(8, ' ');
    let (staged, unstaged) = xy(fields.next()?);
    let path = fields.nth(6)?;

    Some(FileEntry {
        path: PathBuf::from(path),
        orig_path: None,
        staged,
        unstaged,
    })
}

/// `<XY> <sub> <mH> <mI> <mW> <hH> <hI> <Xscore> <path>\t<origPath>`
fn parse_rename(rest: &str) -> Option<FileEntry> {
    let mut fields = rest.splitn(9, ' ');
    let (staged, unstaged) = xy(fields.next()?);
    let paths = fields.nth(7)?;
    let (path, orig) = paths.split_once('\t')?;

    Some(FileEntry {
        path: PathBuf::from(path),
        orig_path: Some(PathBuf::from(orig)),
        staged,
        unstaged,
    })
}

/// `<XY> <sub> <m1> <m2> <m3> <mW> <h1> <h2> <h3> <path>`
fn parse_unmerged(rest: &str) -> Option<FileEntry> {
    let mut fields = rest.splitn(10, ' ');
    fields.next()?; // XY is always "UU"-like for conflicts; force Unmerged
    let path = fields.nth(8)?;

    Some(FileEntry {
        path: PathBuf::from(path),
        orig_path: None,
        staged: Some(ChangeType::Unmerged),
        unstaged: Some(ChangeType::Unmerged),
    })
}

fn untracked_like(path: &str, change: ChangeType) -> FileEntry {
    FileEntry {
        path: PathBuf::from(path),
        orig_path: None,
        staged: None,
        unstaged: Some(change),
    }
}

#[cfg(test)]
mod tests {
    use super::parse_status;

    const MIXED: &str = "\
# branch.oid 1111111111111111111111111111111111111111
# branch.head main
1 M. N... 100644 100644 100644 aaaaaaa bbbbbbb staged.txt
1 .M N... 100644 100644 100644 ccccccc ccccccc unstaged.txt
? untracked.txt
";

    const RENAME: &str = "\
# branch.oid 1111111111111111111111111111111111111111
# branch.head main
2 R. N... 100644 100644 100644 aaaaaaa aaaaaaa R100 new_name.txt\told_name.txt
";

    #[test]
    fn mixed_working_tree() {
        let status = parse_status(MIXED);

        assert_eq!(status.staged().count(), 1);
        assert_eq!(status.modified().count(), 1);
        assert_eq!(status.untracked().count(), 1);
        assert!(!status.is_clean());
        insta::assert_debug_snapshot!(status);
    }

    #[test]
    fn rename_keeps_original_path() {
        let status = parse_status(RENAME);
        let entry = &status.entries[0];

        assert_eq!(entry.staged, Some(super::ChangeType::Renamed));
        assert_eq!(
            entry.orig_path.as_deref(),
            Some(std::path::Path::new("old_name.txt"))
        );
        insta::assert_debug_snapshot!(status);
    }
}
