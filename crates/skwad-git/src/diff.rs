use std::path::PathBuf;

/// Classification of a single line in `git diff` output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    Context,
    Addition,
    Deletion,
    Header,
    HunkHeader,
}

/// Classifies one raw diff line. `---`/`+++` file markers are headers, not
/// deletion/addition lines.
pub fn classify(line: &str) -> LineKind {
    if line.starts_with("@@") {
        LineKind::HunkHeader
    } else if line.starts_with("diff --git")
        || line.starts_with("index ")
        || line.starts_with("--- ")
        || line.starts_with("+++ ")
        || line.starts_with("new file")
        || line.starts_with("deleted file")
        || line.starts_with("old mode")
        || line.starts_with("new mode")
        || line.starts_with("similarity index")
        || line.starts_with("dissimilarity index")
        || line.starts_with("rename ")
        || line.starts_with("copy ")
        || line.starts_with("Binary files ")
    {
        LineKind::Header
    } else if line.starts_with('+') {
        LineKind::Addition
    } else if line.starts_with('-') {
        LineKind::Deletion
    } else {
        LineKind::Context
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffLine {
    pub kind: LineKind,
    pub text: String,
    pub old_lineno: Option<u32>,
    pub new_lineno: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hunk {
    pub header: String,
    pub old_start: u32,
    pub old_count: u32,
    pub new_start: u32,
    pub new_count: u32,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDiff {
    pub path: PathBuf,
    /// Original path for a rename or copy, otherwise `None`.
    pub old_path: Option<PathBuf>,
    pub binary: bool,
    pub hunks: Vec<Hunk>,
}

impl FileDiff {
    pub fn additions(&self) -> usize {
        self.count(LineKind::Addition)
    }

    pub fn deletions(&self) -> usize {
        self.count(LineKind::Deletion)
    }

    fn count(&self, kind: LineKind) -> usize {
        self.hunks
            .iter()
            .flat_map(|h| &h.lines)
            .filter(|l| l.kind == kind)
            .count()
    }
}

pub fn parse_diff(output: &str) -> Vec<FileDiff> {
    let mut files: Vec<FileDiff> = Vec::new();
    let mut old_lineno = 0u32;
    let mut new_lineno = 0u32;

    for line in output.lines() {
        if let Some(paths) = line.strip_prefix("diff --git ") {
            files.push(new_file_diff(paths));
            continue;
        }

        let Some(file) = files.last_mut() else {
            continue;
        };

        if let Some(rest) = line.strip_prefix("rename from ") {
            file.old_path = Some(PathBuf::from(rest));
        } else if let Some(rest) = line.strip_prefix("rename to ") {
            file.path = PathBuf::from(rest);
        } else if let Some(rest) = line.strip_prefix("copy from ") {
            file.old_path = Some(PathBuf::from(rest));
        } else if let Some(rest) = line.strip_prefix("copy to ") {
            file.path = PathBuf::from(rest);
        } else if line.starts_with("Binary files ") {
            file.binary = true;
        } else if line.starts_with("@@") {
            let hunk = parse_hunk_header(line);
            old_lineno = hunk.old_start;
            new_lineno = hunk.new_start;
            file.hunks.push(hunk);
        } else if let Some(hunk) = file.hunks.last_mut() {
            push_hunk_line(hunk, line, &mut old_lineno, &mut new_lineno);
        }
    }

    files
}

fn new_file_diff(paths: &str) -> FileDiff {
    let path = split_git_paths(paths)
        .map(|(_, b)| b)
        .unwrap_or_else(|| paths.to_owned());

    FileDiff {
        path: PathBuf::from(path),
        old_path: None,
        binary: false,
        hunks: Vec::new(),
    }
}

/// Splits `a/<x> b/<y>` into `(x, y)`. Uses the ` b/` boundary; paths with a
/// literal ` b/` substring are not handled (git quotes those).
fn split_git_paths(paths: &str) -> Option<(String, String)> {
    let (a, b) = paths.split_once(" b/")?;
    let a = a.strip_prefix("a/").unwrap_or(a);
    Some((a.to_owned(), b.to_owned()))
}

fn parse_hunk_header(line: &str) -> Hunk {
    let inner = line
        .strip_prefix("@@ ")
        .and_then(|rest| rest.split_once(" @@"))
        .map(|(ranges, _)| ranges)
        .unwrap_or("");

    let mut old_start = 0;
    let mut old_count = 1;
    let mut new_start = 0;
    let mut new_count = 1;

    for token in inner.split_whitespace() {
        if let Some(range) = token.strip_prefix('-') {
            (old_start, old_count) = parse_range(range);
        } else if let Some(range) = token.strip_prefix('+') {
            (new_start, new_count) = parse_range(range);
        }
    }

    Hunk {
        header: line.to_owned(),
        old_start,
        old_count,
        new_start,
        new_count,
        lines: Vec::new(),
    }
}

/// `<start>` or `<start>,<count>`. Count defaults to 1 when omitted.
fn parse_range(range: &str) -> (u32, u32) {
    match range.split_once(',') {
        Some((start, count)) => (start.parse().unwrap_or(0), count.parse().unwrap_or(1)),
        None => (range.parse().unwrap_or(0), 1),
    }
}

fn push_hunk_line(hunk: &mut Hunk, line: &str, old_lineno: &mut u32, new_lineno: &mut u32) {
    if line.starts_with('\\') {
        return; // "\ No newline at end of file"
    }

    // Diff markers are ASCII, so byte index 1 is always a char boundary.
    let (kind, text) = match line.as_bytes().first() {
        Some(b'+') => (LineKind::Addition, &line[1..]),
        Some(b'-') => (LineKind::Deletion, &line[1..]),
        Some(b' ') => (LineKind::Context, &line[1..]),
        _ => (LineKind::Context, line),
    };

    let (old, new) = match kind {
        LineKind::Addition => {
            let n = *new_lineno;
            *new_lineno += 1;
            (None, Some(n))
        }
        LineKind::Deletion => {
            let o = *old_lineno;
            *old_lineno += 1;
            (Some(o), None)
        }
        _ => {
            let (o, n) = (*old_lineno, *new_lineno);
            *old_lineno += 1;
            *new_lineno += 1;
            (Some(o), Some(n))
        }
    };

    hunk.lines.push(DiffLine {
        kind,
        text: text.to_owned(),
        old_lineno: old,
        new_lineno: new,
    });
}

#[cfg(test)]
mod tests {
    use super::{classify, parse_diff, LineKind};

    const NO_COUNTS: &str = "\
diff --git a/file.txt b/file.txt
index 1111111..2222222 100644
--- a/file.txt
+++ b/file.txt
@@ -10 +10 @@
-old line
+new line
";

    const BINARY: &str = "\
diff --git a/logo.png b/logo.png
index 1111111..2222222 100644
Binary files a/logo.png and b/logo.png differ
";

    const MULTI_HUNK: &str = "\
diff --git a/src.rs b/src.rs
index 1111111..2222222 100644
--- a/src.rs
+++ b/src.rs
@@ -1,3 +1,4 @@
 fn main() {
-    let x = 1;
+    let x = 2;
+    let y = 3;
 }
@@ -10,2 +11,2 @@ fn other()
 a
-b
+c
";

    #[test]
    fn hunk_header_without_counts_defaults_to_one() {
        let files = parse_diff(NO_COUNTS);
        let hunk = &files[0].hunks[0];

        assert_eq!((hunk.old_start, hunk.old_count), (10, 1));
        assert_eq!((hunk.new_start, hunk.new_count), (10, 1));
        insta::assert_debug_snapshot!(files);
    }

    #[test]
    fn binary_file_has_flag_and_no_hunks() {
        let files = parse_diff(BINARY);

        assert!(files[0].binary);
        assert!(files[0].hunks.is_empty());
        insta::assert_debug_snapshot!(files);
    }

    #[test]
    fn additions_and_deletions_counted_across_hunks() {
        let files = parse_diff(MULTI_HUNK);
        let file = &files[0];

        assert_eq!(file.additions(), 3);
        assert_eq!(file.deletions(), 2);
    }

    #[test]
    fn classify_covers_every_kind() {
        assert_eq!(classify("diff --git a/x b/x"), LineKind::Header);
        assert_eq!(classify("--- a/x"), LineKind::Header);
        assert_eq!(classify("+++ b/x"), LineKind::Header);
        assert_eq!(classify("@@ -1 +1 @@"), LineKind::HunkHeader);
        assert_eq!(classify("+added"), LineKind::Addition);
        assert_eq!(classify("-removed"), LineKind::Deletion);
        assert_eq!(classify(" context"), LineKind::Context);
    }
}
