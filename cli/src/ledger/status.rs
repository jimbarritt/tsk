//! What a ledger store holds that origin does not, for `tsk ledger status` and
//! `tsk ledger discard`. The types and the output are shared by both backends. The git
//! helpers serve every store that keeps files in a worktree: the file ledger, and the
//! documents of a Dolt ledger.

use super::git::{split_nul, Git};

/// How the local committed state stands against origin, counted in commits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoteState {
    UpToDate,
    Ahead(usize),
    Behind(usize),
    Diverged {
        ahead: usize,
        behind: usize,
    },
    /// Origin could not be reached, so nothing can be said.
    Unknown,
    /// Origin holds no such branch yet: nothing of this side is pushed.
    Unpublished,
}

impl RemoteState {
    /// The state for `ahead` local commits that origin lacks and `behind` origin commits
    /// the local state lacks.
    pub fn from_counts(ahead: usize, behind: usize) -> RemoteState {
        match (ahead, behind) {
            (0, 0) => RemoteState::UpToDate,
            (ahead, 0) => RemoteState::Ahead(ahead),
            (0, behind) => RemoteState::Behind(behind),
            (ahead, behind) => RemoteState::Diverged { ahead, behind },
        }
    }

    pub fn render(&self) -> String {
        match self {
            RemoteState::UpToDate => "up to date".to_string(),
            RemoteState::Ahead(n) => format!("ahead {}", n),
            RemoteState::Behind(n) => format!("behind {}", n),
            RemoteState::Diverged { ahead, behind } => {
                format!("diverged ahead {} behind {}", ahead, behind)
            }
            RemoteState::Unknown => "unknown".to_string(),
            RemoteState::Unpublished => "unpublished".to_string(),
        }
    }
}

/// One side of a ledger: its data, or its documents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SideStatus {
    /// One entry per change. Each is the tab-separated columns after the side's name.
    pub changes: Vec<String>,
    pub remote: RemoteState,
}

impl SideStatus {
    pub fn is_clean(&self) -> bool {
        self.changes.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerStatus {
    pub data: SideStatus,
    pub docs: SideStatus,
    /// Set when origin could not be fetched: one line, ready for stderr.
    pub fetch_note: Option<String>,
}

impl LedgerStatus {
    pub fn is_clean(&self) -> bool {
        self.data.is_clean() && self.docs.is_clean()
    }

    /// The change lines only: `data` or `docs`, then `clean` or `local changes`, then one
    /// line per change.
    pub fn changes_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        for (name, side) in [("data", &self.data), ("docs", &self.docs)] {
            if side.is_clean() {
                lines.push(format!("{}\tclean", name));
            } else {
                lines.push(format!("{}\tlocal changes", name));
                for change in &side.changes {
                    lines.push(format!("{}\t{}", name, change));
                }
            }
        }
        lines
    }

    /// The whole `tsk ledger status` output.
    pub fn lines(&self) -> Vec<String> {
        let mut lines = self.changes_lines();
        lines.push(format!("data\tremote\t{}", self.data.remote.render()));
        lines.push(format!("docs\tremote\t{}", self.docs.remote.render()));
        lines
    }
}

/// The note for a fetch that failed: one line. The line is the first one that holds
/// `fatal:`, from that word on, which is where git and Dolt state the cause. A message
/// without one gives its last line that has a letter or a digit in it.
pub fn fetch_failed_note(remote: &str, error: &str) -> String {
    let lines: Vec<&str> = error
        .split(['\n', '\r'])
        .map(str::trim)
        .filter(|line| line.chars().any(char::is_alphanumeric))
        .collect();
    let cause = lines
        .iter()
        .find_map(|line| line.find("fatal:").map(|at| &line[at..]))
        .or_else(|| lines.last().copied())
        .unwrap_or("unknown error")
        .trim_start_matches("error: ");
    format!(
        "note: could not fetch {} ({}); reporting from what is local",
        remote, cause
    )
}

/// What a worktree holds that origin does not.
pub struct WorktreeSide {
    /// `(letter, path)` for each changed file, sorted by path. The letter is A, M or D.
    pub files: Vec<(char, String)>,
    pub remote: RemoteState,
}

/// Compare a worktree with the tip just fetched for its branch. `fetched` is that tip, or
/// `Ok(None)` when origin has no such branch, or the fetch error. Compares the working
/// files with the merge base of `HEAD` and the tip, so a file only origin changed is not
/// reported, and a file only the worktree changed is, whether committed or not. Changes
/// nothing.
pub fn worktree_side(
    wt: &Git,
    fetched: &Result<Option<String>, String>,
) -> Result<WorktreeSide, String> {
    let (base, remote) = match fetched {
        Ok(Some(tip)) => {
            let base = wt
                .run_line(["merge-base", "HEAD", tip.as_str()])
                .unwrap_or_else(|_| "HEAD".to_string());
            let range = format!("HEAD...{}", tip);
            let counts = wt.run_line(["rev-list", "--left-right", "--count", range.as_str()])?;
            let (ahead, behind) = parse_counts(&counts)?;
            (base, RemoteState::from_counts(ahead, behind))
        }
        Ok(None) => ("HEAD".to_string(), RemoteState::Unpublished),
        Err(_) => ("HEAD".to_string(), RemoteState::Unknown),
    };
    let diff = wt.run(["diff", "--name-status", "--no-renames", "-z", base.as_str()])?;
    let mut files = parse_name_status(&diff);
    let untracked = wt.run(["ls-files", "--others", "--exclude-standard", "-z"])?;
    for path in split_nul(&untracked) {
        files.push(('A', path.to_string()));
    }
    files.sort_by(|a, b| a.1.cmp(&b.1));
    files.dedup_by(|a, b| a.1 == b.1);
    Ok(WorktreeSide { files, remote })
}

/// Reset a worktree to the fetched tip, or to `HEAD` when origin holds no branch, and
/// remove the files git does not track.
pub fn discard_worktree(wt: &Git, tip: Option<&str>) -> Result<(), String> {
    wt.run(["reset", "--hard", "--quiet", tip.unwrap_or("HEAD")])?;
    wt.run(["clean", "-fdq"])?;
    Ok(())
}

/// The two numbers `git rev-list --left-right --count` prints, separated by a tab.
pub fn parse_counts(text: &str) -> Result<(usize, usize), String> {
    let mut parts = text.split_whitespace().map(str::parse::<usize>);
    match (parts.next(), parts.next(), parts.next()) {
        (Some(Ok(left)), Some(Ok(right)), None) => Ok((left, right)),
        _ => Err(format!(
            "error: git printed '{}' where two commit counts were expected",
            text.trim()
        )),
    }
}

/// `git diff --name-status -z` output: a status, then a path, each ended by NUL. Any
/// status that is not an add or a delete counts as a modification.
pub fn parse_name_status(text: &str) -> Vec<(char, String)> {
    let fields = split_nul(text);
    let mut files = Vec::new();
    for pair in fields.chunks(2) {
        if let [status, path] = pair {
            let letter = match status.chars().next() {
                Some('A') => 'A',
                Some('D') => 'D',
                _ => 'M',
            };
            files.push((letter, path.to_string()));
        }
    }
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_give_the_remote_state() {
        assert_eq!(RemoteState::from_counts(0, 0), RemoteState::UpToDate);
        assert_eq!(RemoteState::from_counts(2, 0), RemoteState::Ahead(2));
        assert_eq!(RemoteState::from_counts(0, 3), RemoteState::Behind(3));
        assert_eq!(
            RemoteState::from_counts(1, 4),
            RemoteState::Diverged {
                ahead: 1,
                behind: 4
            }
        );
    }

    #[test]
    fn the_remote_state_renders_as_one_phrase() {
        assert_eq!(RemoteState::UpToDate.render(), "up to date");
        assert_eq!(RemoteState::Ahead(1).render(), "ahead 1");
        assert_eq!(RemoteState::Behind(2).render(), "behind 2");
        assert_eq!(
            RemoteState::from_counts(1, 2).render(),
            "diverged ahead 1 behind 2"
        );
        assert_eq!(RemoteState::Unknown.render(), "unknown");
        assert_eq!(RemoteState::Unpublished.render(), "unpublished");
    }

    #[test]
    fn rev_list_counts_are_parsed_and_junk_is_refused() {
        assert_eq!(parse_counts("3\t0\n").unwrap(), (3, 0));
        assert_eq!(parse_counts("0 12").unwrap(), (0, 12));
        assert!(parse_counts("").is_err());
        assert!(parse_counts("1\t2\t3").is_err());
        assert!(parse_counts("a\tb").is_err());
    }

    #[test]
    fn name_status_output_is_read_as_letters_and_paths() {
        let text = "A\0new.md\0M\0dir/a b.md\0D\0gone.md\0T\0odd.md\0";
        assert_eq!(
            parse_name_status(text),
            vec![
                ('A', "new.md".to_string()),
                ('M', "dir/a b.md".to_string()),
                ('D', "gone.md".to_string()),
                ('M', "odd.md".to_string()),
            ]
        );
        assert!(parse_name_status("").is_empty());
    }

    fn side(changes: &[&str], remote: RemoteState) -> SideStatus {
        SideStatus {
            changes: changes.iter().map(|c| c.to_string()).collect(),
            remote,
        }
    }

    #[test]
    fn a_clean_ledger_prints_four_lines() {
        let status = LedgerStatus {
            data: side(&[], RemoteState::UpToDate),
            docs: side(&[], RemoteState::UpToDate),
            fetch_note: None,
        };
        assert!(status.is_clean());
        assert_eq!(
            status.lines(),
            vec![
                "data\tclean",
                "docs\tclean",
                "data\tremote\tup to date",
                "docs\tremote\tup to date"
            ]
        );
    }

    #[test]
    fn local_changes_print_one_line_each_and_the_discard_summary_leaves_out_the_remote() {
        let status = LedgerStatus {
            data: side(
                &["missions\tadded 1\tmodified 0\tdeleted 0"],
                RemoteState::Ahead(1),
            ),
            docs: side(&["A\tnotes.md", "D\told.md"], RemoteState::Behind(2)),
            fetch_note: None,
        };
        assert!(!status.is_clean());
        assert_eq!(
            status.lines(),
            vec![
                "data\tlocal changes",
                "data\tmissions\tadded 1\tmodified 0\tdeleted 0",
                "docs\tlocal changes",
                "docs\tA\tnotes.md",
                "docs\tD\told.md",
                "data\tremote\tahead 1",
                "docs\tremote\tbehind 2",
            ]
        );
        assert_eq!(status.changes_lines().len(), 5);
    }

    #[test]
    fn the_fetch_note_is_one_line() {
        let note = fetch_failed_note(
            "origin",
            "error: git fetch failed (exit status 128):\nfatal: unable to access 'x'\n",
        );
        assert_eq!(
            note,
            "note: could not fetch origin (fatal: unable to access 'x'); reporting from what is local"
        );
        assert!(!note.contains('\n'));
        let dolt = fetch_failed_note(
            "origin",
            "error: dolt fetch origin failed (exit status 1):\n- Fetching...fatal: '/o.git' does not appear to be a git repository\nfatal: Could not read\n\n             \r",
        );
        assert_eq!(
            dolt,
            "note: could not fetch origin (fatal: '/o.git' does not appear to be a git repository); reporting from what is local"
        );
        assert_eq!(
            fetch_failed_note("origin", "error: plain failure\n   \n"),
            "note: could not fetch origin (plain failure); reporting from what is local"
        );
    }
}
