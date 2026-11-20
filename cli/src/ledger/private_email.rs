use std::process::Output;

use super::git::Git;

pub(super) const NOREPLY: &str = "@users.noreply.github.com";
const LISTED: usize = 5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitEmail {
    pub sha: String,
    pub author: String,
    pub committer: String,
    pub subject: String,
}

pub fn is_private_email_refusal(text: &str) -> bool {
    let text = text.to_lowercase();
    ["gh007", "private email address", "email privacy"]
        .iter()
        .any(|word| text.contains(word))
}

pub fn offending_commits(wt: &Git, remote_tip: Option<&str>) -> Result<Vec<CommitEmail>, String> {
    let format = "--format=%H%x1f%ae%x1f%ce%x1f%s";
    let range = remote_tip.map(|tip| format!("{}..HEAD", tip));
    let listed = match range {
        Some(range) => wt
            .run(["log", "--reverse", format, range.as_str()])
            .or_else(|_| wt.run(["log", "--reverse", format, "HEAD"])),
        None => wt.run(["log", "--reverse", format, "HEAD"]),
    }?;
    Ok(listed
        .lines()
        .filter_map(|line| {
            let mut fields = line.split('\u{1f}');
            Some(CommitEmail {
                sha: fields.next()?.to_string(),
                author: fields.next()?.to_string(),
                committer: fields.next()?.to_string(),
                subject: fields.next().unwrap_or("").to_string(),
            })
        })
        .filter(|c| !c.author.ends_with(NOREPLY) || !c.committer.ends_with(NOREPLY))
        .collect())
}

pub fn private_email_error(ref_name: &str, commits: &[CommitEmail]) -> String {
    let mut text = format!(
        "error: GitHub refused the push to {}: it would publish a private email address (GH007).\n",
        ref_name
    );
    if commits.is_empty() {
        text.push_str(
            "       No local commit in the range has an author or committer email outside the noreply form, so the address is on a commit outside the range.\n",
        );
    } else {
        text.push_str("       These local commits have an author or committer email that is not a noreply address:\n");
        for commit in commits.iter().take(LISTED) {
            let short: String = commit.sha.chars().take(8).collect();
            text.push_str(&format!(
                "       {}  author {}  committer {}  {}\n",
                short, commit.author, commit.committer, commit.subject
            ));
        }
        if commits.len() > LISTED {
            text.push_str(&format!("       ... and {} more\n", commits.len() - LISTED));
        }
    }
    text.push_str(
        "       The local commits stay. Make the address public or turn off the protection at https://github.com/settings/emails, or rewrite the author of the commits to the noreply address, then run tsk ledger push again.",
    );
    text
}

pub fn refusal_error(
    wt: &Git,
    ref_name: &str,
    remote_tip: Option<&str>,
    stdout: &str,
    output: &Output,
) -> Option<String> {
    let combined = format!("{}\n{}", stdout, String::from_utf8_lossy(&output.stderr));
    if !is_private_email_refusal(&combined) {
        return None;
    }
    let commits = offending_commits(wt, remote_tip).unwrap_or_default();
    Some(private_email_error(ref_name, &commits))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn commit(n: usize) -> CommitEmail {
        CommitEmail {
            sha: format!("{}{}", n, "0".repeat(39)),
            author: "a@x.test".into(),
            committer: "a@x.test".into(),
            subject: format!("change {}", n),
        }
    }

    #[test]
    fn the_refusal_is_recognised_in_both_forms() {
        assert!(is_private_email_refusal(
            "remote: error: GH007: Your push would publish a private email address."
        ));
        assert!(is_private_email_refusal(
            "[remote rejected] (push declined due to email privacy restrictions)"
        ));
        assert!(!is_private_email_refusal("[rejected] (stale info)"));
        assert!(!is_private_email_refusal("non-fast-forward"));
        assert!(!is_private_email_refusal("pre-receive hook declined"));
    }

    #[test]
    fn the_error_lists_the_commits_up_to_the_cap() {
        let one = private_email_error("refs/heads/tsk/ledger", &[commit(1)]);
        assert!(one.contains("10000000  author a@x.test"), "{}", one);
        assert!(one.contains("GH007") && !one.contains("more"), "{}", one);
        let many: Vec<CommitEmail> = (1..=8).map(commit).collect();
        let text = private_email_error("refs/heads/tsk/ledger", &many);
        assert!(text.contains("... and 3 more"), "{}", text);
        assert!(!text.contains("change 6"), "{}", text);
        let none = private_email_error("refs/heads/tsk/ledger", &[]);
        assert!(none.contains("outside the range"), "{}", none);
    }

    fn repo(dir: &std::path::Path) -> Git {
        let git = Git::new(dir);
        git.run(["init", "--quiet", "--initial-branch=main"])
            .unwrap();
        git.run(["config", "commit.gpgsign", "false"]).unwrap();
        git
    }

    fn commit_as(git: &Git, file: &str, author: &str, committer: &str) -> String {
        std::fs::write(git.dir().join(file), file).unwrap();
        git.run(["add", file]).unwrap();
        let git = Git::new(git.dir()).with_env(
            [
                ("GIT_AUTHOR_NAME", "n"),
                ("GIT_AUTHOR_EMAIL", author),
                ("GIT_COMMITTER_NAME", "n"),
                ("GIT_COMMITTER_EMAIL", committer),
            ]
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
        );
        git.run(["commit", "--quiet", "-m", file]).unwrap();
        git.run_line(["rev-parse", "HEAD"]).unwrap()
    }

    #[test]
    fn the_commits_with_a_non_noreply_email_are_listed() {
        let dir = tempfile::tempdir().unwrap();
        let git = repo(dir.path());
        let noreply = "1+b@users.noreply.github.com";
        let first = commit_as(&git, "one", "a@x.test", noreply);
        let second = commit_as(&git, "two", noreply, noreply);
        let third = commit_as(&git, "three", noreply, "c@x.test");
        let all = offending_commits(&git, None).unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].sha, first);
        assert_eq!(all[0].author, "a@x.test");
        assert_eq!(all[1].sha, third);
        assert_eq!(all[1].committer, "c@x.test");
        assert_eq!(all[1].subject, "three");
        let since = offending_commits(&git, Some(&second)).unwrap();
        assert_eq!(since.len(), 1);
        assert_eq!(since[0].sha, third);
    }
}
