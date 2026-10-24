//! What a ledger store holds that origin does not, for `tsk ledger status` and
//! `tsk ledger discard`. The git helpers serve the documents of a Dolt ledger, which are
//! files in a worktree.

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

    /// Local commits origin lacks.
    pub fn ahead(&self) -> usize {
        match self {
            RemoteState::Ahead(n) => *n,
            RemoteState::Diverged { ahead, .. } => *ahead,
            _ => 0,
        }
    }

    /// Origin commits the local state lacks.
    pub fn behind(&self) -> usize {
        match self {
            RemoteState::Behind(n) => *n,
            RemoteState::Diverged { behind, .. } => *behind,
            _ => 0,
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

/// One change not pushed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    /// A table name, or a file path.
    pub name: String,
    pub kind: ChangeKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeKind {
    /// A Dolt table: rows added, modified and deleted, and whether the schema changed.
    Table {
        added: i64,
        modified: i64,
        deleted: i64,
        schema: bool,
    },
    /// A file: `A`, `M` or `D`.
    File(char),
    /// The local manifest version differs from origin's: the schema was upgraded by a
    /// write. The manifest line and the schema-only table lines it caused are not shown.
    Upgrade { version: i64, origin: i64 },
}

impl Change {
    /// The label for a schema upgrade, to version `version` while origin has `origin`.
    pub fn upgrade(version: i64, origin: i64) -> Change {
        Change {
            name: "upgrade".to_string(),
            kind: ChangeKind::Upgrade { version, origin },
        }
    }

    pub fn file(letter: char, path: &str) -> Change {
        Change {
            name: path.to_string(),
            kind: ChangeKind::File(letter),
        }
    }

    /// The tab-separated columns of the `--porcelain` line after the side's name.
    pub fn porcelain(&self) -> String {
        match &self.kind {
            ChangeKind::File(letter) => format!("{}\t{}", letter, self.name),
            ChangeKind::Upgrade { version, .. } => {
                format!("{}\tschema upgraded to version {}", self.name, version)
            }
            ChangeKind::Table {
                added,
                modified,
                deleted,
                schema,
            } => {
                if *added + *modified + *deleted == 0 && *schema {
                    return format!("{}\tschema changed", self.name);
                }
                let mut text = format!(
                    "{}\tadded {}\tmodified {}\tdeleted {}",
                    self.name, added, modified, deleted
                );
                if *schema {
                    text.push_str("\tschema changed");
                }
                text
            }
        }
    }

    /// What changed, in words: `1 added, 2 modified`, `schema changed`, `added`.
    pub fn describe(&self) -> String {
        match &self.kind {
            ChangeKind::File('A') => "added".to_string(),
            ChangeKind::File('D') => "deleted".to_string(),
            ChangeKind::File(_) => "modified".to_string(),
            ChangeKind::Upgrade { version, origin } => {
                format!(
                    "schema upgraded to version {} (origin has {})",
                    version, origin
                )
            }
            ChangeKind::Table {
                added,
                modified,
                deleted,
                schema,
            } => {
                let mut parts: Vec<String> = [
                    (added, "added"),
                    (modified, "modified"),
                    (deleted, "deleted"),
                ]
                .iter()
                .filter(|(n, _)| **n > 0)
                .map(|(n, word)| format!("{} {}", n, word))
                .collect();
                if *schema {
                    parts.push("schema changed".to_string());
                }
                if parts.is_empty() {
                    "changed".to_string()
                } else {
                    parts.join(", ")
                }
            }
        }
    }
}

/// Label a schema upgrade among the changes of a Dolt ledger. `version` is the local
/// manifest version and `origin` the version origin holds, and they differ. The manifest
/// line goes, and so does every table line whose only change is its schema. A table whose
/// schema changed and whose rows changed keeps its line with the rows the person changed:
/// `real_modified` counts the rows of a table whose own columns differ, which tells a row
/// the upgrade touched from a row that was edited. The label comes first and counts as
/// one change.
pub fn label_upgrade(
    changes: Vec<Change>,
    version: i64,
    origin: i64,
    real_modified: &mut dyn FnMut(&str) -> Result<i64, String>,
) -> Result<Vec<Change>, String> {
    let mut labelled = vec![Change::upgrade(version, origin)];
    for change in changes {
        if change.name == "manifest" {
            continue;
        }
        let kind = match change.kind {
            ChangeKind::Table {
                added,
                modified,
                deleted,
                schema: true,
            } => {
                let modified = if modified > 0 {
                    real_modified(&change.name)?
                } else {
                    0
                };
                if added + modified + deleted == 0 {
                    continue;
                }
                ChangeKind::Table {
                    added,
                    modified,
                    deleted,
                    schema: false,
                }
            }
            other => other,
        };
        labelled.push(Change {
            name: change.name,
            kind,
        });
    }
    Ok(labelled)
}

/// Hide the rows a schema upgrade copied from table `from` into table `to` before it
/// dropped `from`. `copied` is the number of rows `from` held at the base: that many
/// deleted rows leave `from`'s line and that many added rows leave `to`'s line. A line
/// with nothing left to say goes. Rows a person changed on top of the copy stay.
pub fn hide_copied_rows(changes: Vec<Change>, from: &str, to: &str, copied: i64) -> Vec<Change> {
    changes
        .into_iter()
        .filter_map(|mut change| {
            if let ChangeKind::Table {
                added,
                modified,
                deleted,
                schema,
            } = &mut change.kind
            {
                if change.name == from {
                    *deleted = (*deleted - copied).max(0);
                } else if change.name == to {
                    *added = (*added - copied).max(0);
                } else {
                    return Some(change);
                }
                if *added + *modified + *deleted == 0 && !*schema {
                    return None;
                }
            }
            Some(change)
        })
        .collect()
}

/// One side of a ledger: its data, or its documents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SideStatus {
    pub changes: Vec<Change>,
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
    /// False for `tsk ledger status --local`, which asks origin nothing. The commit counts
    /// then come from what was last fetched, and the Origin line says it was not checked.
    pub origin_checked: bool,
}

/// What the `--porcelain` remote lines say when origin was not asked.
const NOT_CHECKED: &str = "not checked";

const LABEL_WIDTH: usize = 8;
const DETAIL_INDENT: usize = 10;

fn plural(n: usize, noun: &str) -> String {
    if n == 1 {
        format!("{} {}", n, noun)
    } else {
        format!("{} {}s", n, noun)
    }
}

fn labelled(label: &str, text: &str) -> String {
    format!("{:<width$}{}", label, text, width = LABEL_WIDTH)
}

/// `1 data commit and 2 document commits`: the non-zero parts, joined.
fn commit_parts(parts: &[(&str, usize)]) -> String {
    parts
        .iter()
        .filter(|(_, n)| *n > 0)
        .map(|(noun, n)| plural(*n, &format!("{} commit", noun)))
        .collect::<Vec<_>>()
        .join(" and ")
}

impl LedgerStatus {
    pub fn is_clean(&self) -> bool {
        self.data.is_clean() && self.docs.is_clean()
    }

    /// True when the ledger store holds anything origin lacks: changes not committed, or
    /// commits not pushed. Asks nothing of origin.
    pub fn has_unpushed(&self) -> bool {
        let (data, docs) = self.ahead();
        !self.is_clean() || data + docs > 0
    }

    /// The number of changes not committed plus the commits not pushed, both sides.
    pub fn unpushed_count(&self) -> usize {
        let (data, docs) = self.ahead();
        self.data.changes.len() + self.docs.changes.len() + data + docs
    }

    /// The `--porcelain` change lines: `data` or `docs`, then `clean` or `local changes`,
    /// then one line per change.
    pub fn changes_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        for (name, side) in [("data", &self.data), ("docs", &self.docs)] {
            if side.is_clean() {
                lines.push(format!("{}\tclean", name));
            } else {
                lines.push(format!("{}\tlocal changes", name));
                for change in &side.changes {
                    lines.push(format!("{}\t{}", name, change.porcelain()));
                }
            }
        }
        lines
    }

    /// The whole `tsk ledger status --porcelain` output.
    pub fn lines(&self) -> Vec<String> {
        let mut lines = self.changes_lines();
        if !self.origin_checked {
            lines.push(format!("data\tremote\t{}", NOT_CHECKED));
            lines.push(format!("docs\tremote\t{}", NOT_CHECKED));
            return lines;
        }
        lines.push(format!("data\tremote\t{}", self.data.remote.render()));
        lines.push(format!("docs\tremote\t{}", self.docs.remote.render()));
        lines
    }

    fn ahead(&self) -> (usize, usize) {
        (self.data.remote.ahead(), self.docs.remote.ahead())
    }

    fn behind(&self) -> (usize, usize) {
        (self.data.remote.behind(), self.docs.remote.behind())
    }

    /// The `Local` block of the human output: the headline, then the detail lines.
    pub fn human_local(&self) -> Vec<String> {
        let changes: Vec<(&str, &Change)> = self
            .data
            .changes
            .iter()
            .map(|c| ("data", c))
            .chain(self.docs.changes.iter().map(|c| ("docs", c)))
            .collect();
        let (a_data, a_docs) = self.ahead();
        let commits = a_data + a_docs;
        let mut lines = Vec::new();
        if changes.is_empty() {
            if commits == 0 {
                lines.push(labelled("Local", "nothing to push"));
            } else {
                lines.push(labelled(
                    "Local",
                    &format!("{} not pushed", plural(commits, "commit")),
                ));
            }
            return lines;
        }
        lines.push(labelled(
            "Local",
            &format!("{} not pushed", plural(changes.len(), "change")),
        ));
        let name_width = changes.iter().map(|(_, c)| c.name.chars().count()).max();
        let name_width = name_width.unwrap_or(0);
        for (store, change) in &changes {
            lines.push(format!(
                "{}{}  {:<nw$}  {}",
                " ".repeat(DETAIL_INDENT),
                store,
                change.name,
                change.describe(),
                nw = name_width
            ));
        }
        if commits > 0 {
            lines.push(format!(
                "{}plus {} not pushed",
                " ".repeat(LABEL_WIDTH),
                plural(commits, "commit")
            ));
        }
        lines
    }

    /// The `Origin` line of the human output.
    pub fn human_origin(&self) -> String {
        if !self.origin_checked {
            return labelled("Origin", "not checked (--local)");
        }
        let remotes = [&self.data.remote, &self.docs.remote];
        let text = if remotes.iter().any(|r| **r == RemoteState::Unknown) {
            "unknown (could not reach origin)".to_string()
        } else if remotes.iter().any(|r| **r == RemoteState::Unpublished) {
            "not published yet (tsk ledger push creates it)".to_string()
        } else {
            let (d_noun, doc_noun) = ("data", "document");
            let (a_data, a_docs) = self.ahead();
            let (b_data, b_docs) = self.behind();
            let push = [(d_noun, a_data), (doc_noun, a_docs)];
            let fetch = [(d_noun, b_data), (doc_noun, b_docs)];
            let (push_n, fetch_n) = (a_data + a_docs, b_data + b_docs);
            match (push_n, fetch_n) {
                (0, 0) => "up to date".to_string(),
                (0, _) => format!("{} to fetch (tsk ledger fetch)", commit_parts(&fetch)),
                (_, 0) => format!("{} not on origin (tsk ledger push)", commit_parts(&push)),
                _ => {
                    let push_text = commit_parts(&push);
                    let single_noun = |parts: &[(&str, usize); 2]| {
                        parts.iter().filter(|(_, n)| *n > 0).count() == 1
                    };
                    let same_noun = single_noun(&push)
                        && single_noun(&fetch)
                        && push.iter().position(|(_, n)| *n > 0)
                            == fetch.iter().position(|(_, n)| *n > 0);
                    let fetch_text = if same_noun {
                        fetch_n.to_string()
                    } else {
                        commit_parts(&fetch)
                    };
                    format!(
                        "diverged: {} to push, {} to fetch (tsk ledger push merges them)",
                        push_text, fetch_text
                    )
                }
            }
        };
        labelled("Origin", &text)
    }

    /// The whole human `tsk ledger status` output.
    pub fn human_lines(&self) -> Vec<String> {
        let mut lines = self.human_local();
        lines.push(self.human_origin());
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

    fn table(name: &str, added: i64, modified: i64, deleted: i64, schema: bool) -> Change {
        Change {
            name: name.to_string(),
            kind: ChangeKind::Table {
                added,
                modified,
                deleted,
                schema,
            },
        }
    }

    fn side(changes: Vec<Change>, remote: RemoteState) -> SideStatus {
        SideStatus { changes, remote }
    }

    fn status(data: SideStatus, docs: SideStatus) -> LedgerStatus {
        LedgerStatus {
            origin_checked: true,
            data,
            docs,
            fetch_note: None,
        }
    }

    fn clean(remote: RemoteState) -> LedgerStatus {
        status(side(vec![], remote.clone()), side(vec![], remote))
    }

    #[test]
    fn a_clean_ledger_prints_four_porcelain_lines() {
        let status = clean(RemoteState::UpToDate);
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
    fn porcelain_prints_one_line_per_change() {
        let status = status(
            side(
                vec![
                    table("missions", 1, 0, 0, false),
                    table("watermark", 0, 0, 0, true),
                    table("tasks", 2, 1, 0, true),
                ],
                RemoteState::Ahead(1),
            ),
            side(
                vec![Change::file('A', "notes.md"), Change::file('D', "old.md")],
                RemoteState::Behind(2),
            ),
        );
        assert!(!status.is_clean());
        assert_eq!(
            status.lines(),
            vec![
                "data\tlocal changes",
                "data\tmissions\tadded 1\tmodified 0\tdeleted 0",
                "data\twatermark\tschema changed",
                "data\ttasks\tadded 2\tmodified 1\tdeleted 0\tschema changed",
                "docs\tlocal changes",
                "docs\tA\tnotes.md",
                "docs\tD\told.md",
                "data\tremote\tahead 1",
                "docs\tremote\tbehind 2",
            ]
        );
        assert_eq!(status.changes_lines().len(), 7);
    }

    #[test]
    fn an_upgrade_label_replaces_the_manifest_and_schema_only_lines() {
        let changes = vec![
            table("manifest", 0, 1, 0, false),
            table("missions", 1, 1, 0, true),
            table("tasks", 0, 0, 0, true),
            table("threads", 0, 2, 0, true),
            table("decisions", 0, 1, 0, false),
        ];
        // The upgrade touched the modified rows of `missions` and `threads`: none of
        // them was edited.
        let mut real = |_: &str| -> Result<i64, String> { Ok(0) };
        let labelled = label_upgrade(changes, 6, 5, &mut real).unwrap();
        assert_eq!(
            labelled
                .iter()
                .map(|c| (c.name.as_str(), c.describe()))
                .collect::<Vec<_>>(),
            vec![
                (
                    "upgrade",
                    "schema upgraded to version 6 (origin has 5)".to_string()
                ),
                ("missions", "1 added".to_string()),
                ("decisions", "1 modified".to_string()),
            ]
        );
        assert_eq!(
            labelled[0].porcelain(),
            "upgrade\tschema upgraded to version 6"
        );
        let s = status(
            side(labelled, RemoteState::UpToDate),
            side(vec![], RemoteState::UpToDate),
        );
        assert_eq!(s.human_local()[0], "Local   3 changes not pushed");
        assert_eq!(
            s.human_local()[1],
            "          data  upgrade    schema upgraded to version 6 (origin has 5)"
        );
    }

    #[test]
    fn rows_copied_by_an_upgrade_leave_both_table_lines() {
        let table = |name: &str, added, deleted| Change {
            name: name.to_string(),
            kind: ChangeKind::Table {
                added,
                modified: 0,
                deleted,
                schema: false,
            },
        };
        let hidden = hide_copied_rows(
            vec![
                table("bindings", 3, 0),
                table("cloud_session_bindings", 0, 2),
                table("threads", 1, 0),
            ],
            "cloud_session_bindings",
            "bindings",
            2,
        );
        assert_eq!(
            hidden
                .iter()
                .map(|c| (c.name.as_str(), c.describe()))
                .collect::<Vec<_>>(),
            vec![
                ("bindings", "1 added".to_string()),
                ("threads", "1 added".to_string())
            ]
        );
        let all = hide_copied_rows(
            vec![
                table("bindings", 2, 0),
                table("cloud_session_bindings", 0, 2),
            ],
            "cloud_session_bindings",
            "bindings",
            2,
        );
        assert!(all.is_empty());
    }

    #[test]
    fn the_human_form_for_a_clean_ledger() {
        assert_eq!(
            clean(RemoteState::UpToDate).human_lines(),
            vec!["Local   nothing to push", "Origin  up to date"]
        );
    }

    #[test]
    fn the_human_form_lists_changes_aligned_under_the_headline() {
        let s = status(
            side(
                vec![
                    table("missions", 1, 0, 0, false),
                    table("tasks", 1, 0, 0, false),
                ],
                RemoteState::UpToDate,
            ),
            side(vec![Change::file('M', "index.md")], RemoteState::UpToDate),
        );
        assert_eq!(
            s.human_local(),
            vec![
                "Local   3 changes not pushed",
                "          data  missions  1 added",
                "          data  tasks     1 added",
                "          docs  index.md  modified",
            ]
        );
        let one = status(
            side(vec![table("m", 0, 2, 3, true)], RemoteState::UpToDate),
            side(vec![], RemoteState::UpToDate),
        );
        assert_eq!(
            one.human_local(),
            vec![
                "Local   1 change not pushed",
                "          data  m  2 modified, 3 deleted, schema changed",
            ]
        );
        let schema_only = status(
            side(vec![table("m", 0, 0, 0, true)], RemoteState::UpToDate),
            side(vec![], RemoteState::UpToDate),
        );
        assert!(schema_only.human_local()[1].ends_with("m  schema changed"));
    }

    #[test]
    fn unpushed_commits_are_counted_after_the_changes_or_alone() {
        let with_changes = status(
            side(vec![table("m", 1, 0, 0, false)], RemoteState::Ahead(1)),
            side(vec![], RemoteState::Ahead(1)),
        );
        assert_eq!(
            with_changes.human_local(),
            vec![
                "Local   1 change not pushed",
                "          data  m  1 added",
                "        plus 2 commits not pushed",
            ]
        );
        assert_eq!(
            status(
                side(vec![], RemoteState::Ahead(1)),
                side(vec![], RemoteState::UpToDate)
            )
            .human_local(),
            vec!["Local   1 commit not pushed"]
        );
    }

    fn origin(data: RemoteState, docs: RemoteState) -> String {
        status(side(vec![], data), side(vec![], docs)).human_origin()
    }

    #[test]
    fn the_origin_line_names_each_state() {
        use RemoteState::*;
        assert_eq!(
            origin(Behind(2), Behind(1)),
            "Origin  2 data commits and 1 document commit to fetch (tsk ledger fetch)"
        );
        assert_eq!(
            origin(Behind(1), UpToDate),
            "Origin  1 data commit to fetch (tsk ledger fetch)"
        );
        assert_eq!(
            origin(UpToDate, Behind(3)),
            "Origin  3 document commits to fetch (tsk ledger fetch)"
        );
        assert_eq!(
            origin(Ahead(1), UpToDate),
            "Origin  1 data commit not on origin (tsk ledger push)"
        );
        assert_eq!(
            origin(Ahead(1), Ahead(2)),
            "Origin  1 data commit and 2 document commits not on origin (tsk ledger push)"
        );
        assert_eq!(
            origin(RemoteState::from_counts(1, 2), UpToDate),
            "Origin  diverged: 1 data commit to push, 2 to fetch (tsk ledger push merges them)"
        );
        assert_eq!(
            origin(
                RemoteState::from_counts(1, 1),
                RemoteState::from_counts(1, 1)
            ),
            "Origin  diverged: 1 data commit and 1 document commit to push, 1 data commit and 1 document commit to fetch (tsk ledger push merges them)"
        );
        assert_eq!(
            origin(Ahead(1), Behind(1)),
            "Origin  diverged: 1 data commit to push, 1 document commit to fetch (tsk ledger push merges them)"
        );
        assert_eq!(
            origin(Unpublished, Unpublished),
            "Origin  not published yet (tsk ledger push creates it)"
        );
        assert_eq!(
            origin(Unknown, Unknown),
            "Origin  unknown (could not reach origin)"
        );
    }

    #[test]
    fn the_human_form_holds_no_tab_and_no_trailing_space() {
        let s = status(
            side(vec![table("m", 1, 0, 0, false)], RemoteState::Ahead(1)),
            side(vec![Change::file('A', "a.md")], RemoteState::Unknown),
        );
        for line in s.human_lines() {
            assert!(!line.contains('\t'), "{:?}", line);
            assert_eq!(line, line.trim_end(), "{:?}", line);
        }
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
