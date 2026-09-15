use std::path::PathBuf;

/// Combined line-change totals across staged, unstaged, and untracked changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DiffStats {
    pub insertions: u64,
    pub deletions: u64,
    pub files_changed: u64,
}

/// Parses `git diff --numstat` rows: `<added>\t<deleted>\t<path>`. A `-` in
/// either count marks a binary file and yields `0` for that count.
pub fn parse_numstat(output: &str) -> Vec<(u64, u64, PathBuf)> {
    output
        .lines()
        .filter_map(|line| {
            let mut fields = line.splitn(3, '\t');
            let added = parse_count(fields.next()?);
            let deleted = parse_count(fields.next()?);
            let path = fields.next()?;
            Some((added, deleted, PathBuf::from(path)))
        })
        .collect()
}

fn parse_count(field: &str) -> u64 {
    if field == "-" {
        0
    } else {
        field.parse().unwrap_or(0)
    }
}

/// Line count of an untracked file for stats purposes. `None` when the file is
/// binary (contains a NUL byte) or unreadable.
pub fn untracked_line_count(bytes: std::io::Result<Vec<u8>>) -> Option<u64> {
    let bytes = bytes.ok()?;
    if bytes.contains(&0) {
        return None;
    }

    let count = bytes.iter().filter(|&&b| b == b'\n').count() as u64;
    Some(count)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{parse_numstat, untracked_line_count};

    #[test]
    fn numstat_parses_text_and_binary_rows() {
        let rows = parse_numstat("3\t1\tsrc.rs\n-\t-\tlogo.png\n");

        assert_eq!(
            rows,
            vec![
                (3, 1, PathBuf::from("src.rs")),
                (0, 0, PathBuf::from("logo.png")),
            ]
        );
    }

    #[test]
    fn untracked_line_count_counts_newlines_and_rejects_binary() {
        let text = b"a\nb\nc\n".to_vec();
        assert_eq!(untracked_line_count(Ok(text)), Some(3));

        let binary = vec![b'a', 0u8, b'b'];
        assert_eq!(untracked_line_count(Ok(binary)), None);

        let err = Err(std::io::Error::other("nope"));
        assert_eq!(untracked_line_count(err), None);
    }
}
