use std::path::{Path, PathBuf};

use super::fetch::{check_manifest, ensure_worktree_of_repo, fetch_tip};
use super::git::{failure_message, Git};
use super::location::{worktree_path, LedgerLocation, Repo};

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

pub fn push(
    repo: &Repo,
    state_root: &Path,
    message: &str,
    on_retry: &mut dyn FnMut(usize, &PushStatus),
) -> Result<PushOutcome, String> {
    let location = repo.location(state_root)?;
    let path = existing_worktree(repo, state_root)?;
    let wt = Git::new(&path);
    ensure_worktree_of_repo(&wt, &repo.common_dir)?;

    let mut tip = fetch_tip(&wt, &location)?;
    check_manifest(&wt, tip.as_deref().unwrap_or("HEAD"), &location)?;

    let committed = commit_changes(&wt, message)?;

    for attempt in 1..=MAX_ATTEMPTS {
        if attempt > 1 {
            tip = fetch_tip(&wt, &location)?;
            check_manifest(&wt, tip.as_deref().unwrap_or("HEAD"), &location)?;
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

fn existing_worktree(repo: &Repo, state_root: &Path) -> Result<PathBuf, String> {
    let missing = |what: String| {
        format!(
            "error: {}. Run tsk ledger fetch first, which creates the ledger worktree",
            what
        )
    };
    let clone_id = repo.read_clone_id()?.ok_or_else(|| {
        missing(format!(
            "this clone has no clone id yet ({} is absent or empty)",
            repo.clone_id_file().display()
        ))
    })?;
    let path = worktree_path(state_root, &clone_id);
    if !path.is_dir() {
        return Err(missing(format!("{} does not exist", path.display())));
    }
    Ok(path)
}

fn commit_changes(wt: &Git, message: &str) -> Result<bool, String> {
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
    let _ = wt.output(["rebase", "--abort"])?;
    Err(format!(
        "error: rebase onto {}'s {} ({}) conflicted; the rebase is aborted.\n       \
         Resolve it in {}: git rebase {}, fix the conflicts,\n       \
         git rebase --continue, then re-run tsk ledger push",
        location.label(),
        location.ref_name(),
        tip,
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
    let status = parse_push_status(&stdout, location.ref_name())
        .ok_or_else(|| failure_message(&args, &output))?;
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
