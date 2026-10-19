use std::path::{Path, PathBuf};

use super::fetch::{ensure_worktree_of_repo, fetch_tip};
use super::git::{failure_message, split_nul, Git};
use super::location::{LedgerLocation, Repo};
use super::refs::{is_server_refusal, refused_push, LedgerRefs};

pub const MAX_ATTEMPTS: usize = 5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PushResult {
    Pushed { attempts: usize },
    AlreadyOnRemote,
}

pub struct PushOutcome {
    pub location: LedgerLocation,
    pub path: PathBuf,
    pub committed: bool,
    pub commit: String,
    pub result: PushResult,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PushStatus {
    pub flag: char,
    pub summary: String,
}

impl PushStatus {
    pub fn rejected(&self) -> bool {
        self.flag == '!'
    }
}

/// Commit every change in the worktree at `path` and push `HEAD` to `location`'s ref
/// with a compare and swap, rebasing onto a newer remote tip. The documents branch of a
/// ledger uses it; there is no manifest on that branch.
///
/// `known_tip` is the tip the caller fetched or checked earlier in this process:
/// `Some(Some(commit))` for a branch at that commit, `Some(None)` for a branch origin does
/// not hold, and `None` when the caller has no tip, which fetches one first. A known tip
/// saves the fetch before the first push. The push carries the tip as its lease, so a
/// branch that has moved since rejects the push, and the retry fetches.
pub fn push_worktree(
    repo: &Repo,
    location: &LedgerLocation,
    path: &Path,
    message: &str,
    known_tip: Option<Option<String>>,
    on_retry: &mut dyn FnMut(usize, &PushStatus),
) -> Result<PushOutcome, String> {
    let location = location.clone();
    let path = path.to_path_buf();
    let wt = Git::new(&path);
    ensure_worktree_of_repo(&wt, &repo.common_dir)?;

    let mut tip = match known_tip {
        Some(tip) => tip,
        None => fetch_tip(&wt, &location)?,
    };

    let committed = commit_changes(&wt, message)?;

    for attempt in 1..=MAX_ATTEMPTS {
        if attempt > 1 {
            tip = fetch_tip(&wt, &location)?;
        }

        if let Some(remote_tip) = &tip {
            if is_ancestor(&wt, "HEAD", remote_tip)? {
                return Ok(PushOutcome {
                    location,
                    path,
                    committed,
                    commit: remote_tip.clone(),
                    result: PushResult::AlreadyOnRemote,
                });
            }

            if !is_ancestor(&wt, remote_tip, "HEAD")? {
                rebase_onto(&wt, remote_tip, &location)?;
            }
        }

        let status = push_head(&wt, &location, tip.as_deref())?;
        if !status.rejected() {
            let commit = wt.run_line(["rev-parse", "--verify", "HEAD^{commit}"])?;
            return Ok(PushOutcome {
                location,
                path,
                committed,
                commit,
                result: PushResult::Pushed { attempts: attempt },
            });
        }
        if attempt < MAX_ATTEMPTS {
            on_retry(attempt, &status);
        }
    }

    Err(format!(
        "error: {} on {} rejected the push {} times; it moved between each fetch and push.\n       \
         The commit stays in {}. Re-run tsk ledger push to try again",
        location.ref_name(),
        location.label(),
        MAX_ATTEMPTS,
        path.display()
    ))
}

pub(crate) fn commit_changes(wt: &Git, message: &str) -> Result<bool, String> {
    wt.run(["add", "-A"])?;
    let args = ["diff", "--cached", "--quiet"];
    let (args, output) = wt.output(args)?;
    match output.status.code() {
        Some(0) => Ok(false),
        Some(1) => {
            wt.run(["commit", "--quiet", "-m", message])?;
            Ok(true)
        }
        _ => Err(failure_message(&args, &output)),
    }
}

fn is_ancestor(wt: &Git, ancestor: &str, descendant: &str) -> Result<bool, String> {
    let (args, output) = wt.output(["merge-base", "--is-ancestor", ancestor, descendant])?;
    match output.status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => Err(failure_message(&args, &output)),
    }
}

fn rebase_onto(wt: &Git, tip: &str, location: &LedgerLocation) -> Result<(), String> {
    let (_, output) = wt.output(["rebase", "--quiet", tip])?;
    if output.status.success() {
        return Ok(());
    }
    let conflicted = wt
        .run(["diff", "--name-only", "--diff-filter=U", "-z"])
        .map(|names| {
            split_nul(&names)
                .into_iter()
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let _ = wt.output(["rebase", "--abort"])?;
    let files = if conflicted.is_empty() {
        String::new()
    } else {
        format!("\n       Both sides changed: {}", conflicted.join(", "))
    };
    Err(format!(
        "error: rebase onto {}'s {} ({}) conflicted; the rebase is aborted.{}\n       \
         Resolve it in {}: git rebase {}, fix the conflicts,\n       \
         git rebase --continue, then re-run tsk ledger push",
        location.label(),
        location.ref_name(),
        tip,
        files,
        wt.dir().display(),
        tip
    ))
}

fn push_head(
    wt: &Git,
    location: &LedgerLocation,
    expected: Option<&str>,
) -> Result<PushStatus, String> {
    let lease = format!(
        "--force-with-lease={}:{}",
        location.ref_name(),
        expected.unwrap_or("")
    );
    let refspec = format!("HEAD:{}", location.ref_name());
    let (args, output) = wt.output([
        "push",
        "--porcelain",
        lease.as_str(),
        location.remote(),
        refspec.as_str(),
    ])?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let custom = location.refs() == LedgerRefs::Custom;
    let Some(status) = parse_push_status(&stdout, location.ref_name()) else {
        let failure = failure_message(&args, &output);
        if custom && is_server_refusal(&failure) {
            return Err(refused_push(location.ref_name(), &failure));
        }
        return Err(failure);
    };
    if custom && status.rejected() && is_server_refusal(&status.summary) {
        return Err(refused_push(location.ref_name(), &status.summary));
    }
    if !status.rejected() && !output.status.success() {
        return Err(failure_message(&args, &output));
    }
    Ok(status)
}

pub fn parse_push_status(porcelain: &str, ref_name: &str) -> Option<PushStatus> {
    porcelain.lines().find_map(|line| {
        let mut fields = line.splitn(3, '\t');
        let flag_field = fields.next()?;
        let refs = fields.next()?;
        let summary = fields.next().unwrap_or("");
        let mut flag_chars = flag_field.chars();
        let flag = flag_chars.next()?;
        if flag_chars.next().is_some() {
            return None;
        }
        let (_, to) = refs.split_once(':')?;
        if to != ref_name {
            return None;
        }
        Some(PushStatus {
            flag,
            summary: summary.to_string(),
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const REF: &str = "refs/heads/tsk/ledger";

    #[test]
    fn parses_a_fast_forward_line() {
        let out = "To /tmp/origin.git\n \tHEAD:refs/heads/tsk/ledger\t1111111..2222222\nDone\n";
        let status = parse_push_status(out, REF).unwrap();
        assert_eq!(status.flag, ' ');
        assert_eq!(status.summary, "1111111..2222222");
        assert!(!status.rejected());
    }

    #[test]
    fn parses_a_stale_lease_rejection() {
        let out =
            "To /tmp/origin.git\n!\tHEAD:refs/heads/tsk/ledger\t[rejected] (stale info)\nDone\n";
        let status = parse_push_status(out, REF).unwrap();
        assert!(status.rejected());
        assert_eq!(status.summary, "[rejected] (stale info)");
    }

    #[test]
    fn parses_a_remote_rejection() {
        let out = "To /tmp/origin.git\n!\tHEAD:refs/heads/tsk/ledger\t[remote rejected] (cannot lock ref)\nDone\n";
        assert!(parse_push_status(out, REF).unwrap().rejected());
    }

    #[test]
    fn parses_an_up_to_date_line() {
        let out = "To /tmp/origin.git\n=\tHEAD:refs/heads/tsk/ledger\t[up to date]\nDone\n";
        let status = parse_push_status(out, REF).unwrap();
        assert_eq!(status.flag, '=');
        assert!(!status.rejected());
    }

    #[test]
    fn ignores_lines_for_other_refs() {
        let out = "To /tmp/origin.git\n!\tHEAD:refs/tsk/ledger\t[rejected] (stale info)\nDone\n";
        assert_eq!(parse_push_status(out, REF), None);
    }

    #[test]
    fn returns_none_without_a_status_line() {
        assert_eq!(parse_push_status("", REF), None);
        assert_eq!(parse_push_status("To /tmp/origin.git\nDone\n", REF), None);
    }
}
