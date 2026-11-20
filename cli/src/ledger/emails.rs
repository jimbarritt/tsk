use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use super::erase;
use super::git::Git;
use super::location::LedgerLocation;
use super::private_email::NOREPLY;

pub const COMMAND: &str = "rewrite-emails";

pub const LEDGER_NEW: &str = "refs/tsk-rewrite/ledger";
pub const LEDGER_OLD: &str = "refs/tsk-rewrite/ledger-old";
pub const DOCS_NEW: &str = "refs/tsk-rewrite/docs";
pub const DOCS_OLD: &str = "refs/tsk-rewrite/docs-old";

const RECORD_END: char = '\u{1}';

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmailCount {
    pub email: String,
    pub authored: usize,
    pub committed: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefEmails {
    pub label: &'static str,
    pub ref_name: String,
    pub tip: String,
    pub counts: Vec<EmailCount>,
}

pub fn scan(git: &Git, tip: &str) -> Result<Vec<EmailCount>, String> {
    let listing = git.run(["log", "--format=%ae%x00%ce", tip])?;
    let mut counts: BTreeMap<String, EmailCount> = BTreeMap::new();
    for line in listing.lines() {
        let Some((author, committer)) = line.split_once('\0') else {
            continue;
        };
        for (email, is_author) in [(author, true), (committer, false)] {
            let entry = counts
                .entry(email.to_ascii_lowercase())
                .or_insert_with(|| EmailCount {
                    email: email.to_string(),
                    authored: 0,
                    committed: 0,
                });
            if is_author {
                entry.authored += 1;
            } else {
                entry.committed += 1;
            }
        }
    }
    Ok(counts.into_values().collect())
}

pub fn porcelain_lines(refs: &[RefEmails]) -> Vec<String> {
    let mut lines = Vec::new();
    for found in refs {
        for count in &found.counts {
            lines.push(format!(
                "{}\t{}\t{}\t{}",
                found.ref_name, count.email, count.authored, count.committed
            ));
        }
    }
    lines
}

pub fn listing_lines(refs: &[RefEmails]) -> Vec<String> {
    let mut lines = Vec::new();
    for found in refs {
        lines.push(format!(
            "{} {} (tip {})",
            found.label,
            found.ref_name,
            erase::short(&found.tip)
        ));
        for count in &found.counts {
            lines.push(format!(
                "  {}  authored {}, committed {}",
                count.email, count.authored, count.committed
            ));
        }
    }
    lines
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    to: String,
    from: Vec<String>,
}

impl Selection {
    pub fn new(to: &str, from: &[String]) -> Result<Selection, String> {
        let to = to.trim();
        let valid = match to.split_once('@') {
            Some((local, domain)) => {
                !local.is_empty()
                    && !domain.is_empty()
                    && !to.contains(char::is_whitespace)
                    && !to.contains(['<', '>'])
                    && to.to_ascii_lowercase().ends_with(NOREPLY)
            }
            None => false,
        };
        if !valid {
            return Err(format!(
                "error: --to {} is not a noreply address. Give one such as 12345+name{}",
                to, NOREPLY
            ));
        }
        let mut named = Vec::new();
        for email in from {
            let email = email.trim();
            if email.is_empty() || !email.contains('@') || email.contains(char::is_whitespace) {
                return Err(format!("error: --from {} is not an email address", email));
            }
            named.push(email.to_ascii_lowercase());
        }
        Ok(Selection {
            to: to.to_string(),
            from: named,
        })
    }

    pub fn to(&self) -> &str {
        &self.to
    }

    pub fn selects(&self, email: &str) -> bool {
        let lower = email.to_ascii_lowercase();
        if lower == self.to.to_ascii_lowercase() {
            return false;
        }
        if self.from.is_empty() {
            return !lower.ends_with(NOREPLY);
        }
        self.from.contains(&lower)
    }
}

#[derive(Debug, Clone)]
pub struct Signer {
    pub options: Vec<String>,
    pub env: Vec<(String, String)>,
    pub name: String,
}

impl Signer {
    pub fn from_identity(identity: &crate::attribution::RobotIdentity) -> Signer {
        let env = identity
            .ssh_auth_sock
            .iter()
            .map(|socket| ("SSH_AUTH_SOCK".to_string(), socket.clone()))
            .collect();
        Signer {
            options: identity.git_options(),
            env,
            name: identity.name.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rewritten {
    pub new_tip: String,
    pub rewritten: usize,
    pub total: usize,
    pub first_changed: String,
}

struct Parsed {
    id: String,
    parents: Vec<String>,
    tree: String,
    author_name: String,
    author_email: String,
    author_date: String,
    committer_name: String,
    committer_email: String,
    committer_date: String,
    message: String,
}

fn history(git: &Git, tip: &str) -> Result<Vec<Parsed>, String> {
    let listing = git.run([
        "log",
        "--topo-order",
        "--reverse",
        "--date=raw",
        "--format=%H%x00%P%x00%T%x00%an%x00%ae%x00%ad%x00%cn%x00%ce%x00%cd%x00%B%x01",
        tip,
    ])?;
    let mut commits = Vec::new();
    for record in listing.split(RECORD_END) {
        let record = record.trim_start_matches('\n');
        if record.is_empty() {
            continue;
        }
        let fields: Vec<&str> = record.splitn(10, '\0').collect();
        let [id, parents, tree, an, ae, ad, cn, ce, cd, message] = fields.as_slice() else {
            return Err(format!(
                "error: could not read a commit of the history that ends at {}",
                erase::short(tip)
            ));
        };
        commits.push(Parsed {
            id: id.to_string(),
            parents: parents.split_whitespace().map(str::to_string).collect(),
            tree: tree.to_string(),
            author_name: an.to_string(),
            author_email: ae.to_string(),
            author_date: ad.to_string(),
            committer_name: cn.to_string(),
            committer_email: ce.to_string(),
            committer_date: cd.to_string(),
            message: message.to_string(),
        });
    }
    Ok(commits)
}

fn copy_commit(
    git: &Git,
    signer: Option<&Signer>,
    commit: &Parsed,
    parents: &[String],
    selection: &Selection,
) -> Result<String, String> {
    let replace = |email: &str| {
        if selection.selects(email) {
            selection.to().to_string()
        } else {
            email.to_string()
        }
    };
    let mut env = vec![
        ("GIT_AUTHOR_NAME".to_string(), commit.author_name.clone()),
        (
            "GIT_AUTHOR_EMAIL".to_string(),
            replace(&commit.author_email),
        ),
        ("GIT_AUTHOR_DATE".to_string(), commit.author_date.clone()),
        (
            "GIT_COMMITTER_NAME".to_string(),
            commit.committer_name.clone(),
        ),
        (
            "GIT_COMMITTER_EMAIL".to_string(),
            replace(&commit.committer_email),
        ),
        (
            "GIT_COMMITTER_DATE".to_string(),
            commit.committer_date.clone(),
        ),
    ];
    let mut args = Vec::new();
    if let Some(signer) = signer {
        env.extend(signer.env.iter().cloned());
        args.extend(signer.options.iter().cloned());
    }
    args.extend(["commit-tree".to_string(), commit.tree.clone()]);
    if signer.is_some() {
        args.push("-S".to_string());
    }
    for parent in parents {
        args.push("-p".to_string());
        args.push(parent.clone());
    }
    args.extend(["-F".to_string(), "-".to_string()]);
    let git = git.with_env(env);
    crate::signing::with_retries(crate::signing::retry_delay(), || {
        git.run_stdin(args.clone(), &commit.message)
    })
    .map(|id| id.trim().to_string())
}

pub fn rewrite(
    git: &Git,
    tip: &str,
    selection: &Selection,
    signer: Option<&Signer>,
    dry_run: bool,
) -> Result<Option<Rewritten>, String> {
    let commits = history(git, tip)?;
    let total = commits.len();
    let mut map: HashMap<String, String> = HashMap::new();
    let mut rewritten = 0;
    let mut first_changed: Option<String> = None;
    for commit in &commits {
        let parents: Vec<String> = commit
            .parents
            .iter()
            .map(|parent| map.get(parent).cloned().unwrap_or_else(|| parent.clone()))
            .collect();
        let changed = parents != commit.parents
            || selection.selects(&commit.author_email)
            || selection.selects(&commit.committer_email);
        if !changed {
            map.insert(commit.id.clone(), commit.id.clone());
            continue;
        }
        let new_id = if dry_run {
            format!("dry-{}", commit.id)
        } else {
            copy_commit(git, signer, commit, &parents, selection)?
        };
        if first_changed.is_none() {
            first_changed = Some(commit.id.clone());
        }
        rewritten += 1;
        map.insert(commit.id.clone(), new_id);
    }
    let Some(first_changed) = first_changed else {
        return Ok(None);
    };
    let new_tip = map.get(tip).cloned().ok_or_else(|| {
        format!(
            "error: the rewrite did not reach the tip {}",
            erase::short(tip)
        )
    })?;
    Ok(Some(Rewritten {
        new_tip,
        rewritten,
        total,
        first_changed,
    }))
}

pub fn add_marker(
    git: &Git,
    base: &str,
    old_commit: &str,
    old_tip: &str,
    selection: &Selection,
    signer: Option<&Signer>,
) -> Result<String, String> {
    let text = erase::marker_text(git, base, old_commit, old_tip, COMMAND);
    let index = erase::TempIndex::new(git)?;
    let tree = index.with_file(base, erase::MARKER, &text)?;
    let name = signer
        .map_or("tsk", |signer| signer.name.as_str())
        .to_string();
    let mut env = vec![
        ("GIT_AUTHOR_NAME".to_string(), name.clone()),
        ("GIT_AUTHOR_EMAIL".to_string(), selection.to().to_string()),
        ("GIT_COMMITTER_NAME".to_string(), name),
        (
            "GIT_COMMITTER_EMAIL".to_string(),
            selection.to().to_string(),
        ),
    ];
    let mut args = Vec::new();
    if let Some(signer) = signer {
        env.extend(signer.env.iter().cloned());
        args.extend(signer.options.iter().cloned());
    }
    args.extend(["commit-tree".to_string(), tree]);
    if signer.is_some() {
        args.push("-S".to_string());
    }
    args.extend(
        [
            "-p",
            base,
            "-m",
            "Rewrite the commit emails of the documents history with tsk ledger rewrite-emails",
        ]
        .map(String::from),
    );
    crate::signing::run_signed(&git.with_env(env), args)
        .map(|id| id.trim_end_matches(['\n', '\r']).to_string())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingRef {
    pub label: &'static str,
    pub ref_name: String,
    pub old_tip: String,
    pub new_tip: String,
}

pub fn store_pending(git: &Git, docs: bool, old_tip: &str, new_tip: &str) -> Result<(), String> {
    let (new_ref, old_ref) = if docs {
        (DOCS_NEW, DOCS_OLD)
    } else {
        (LEDGER_NEW, LEDGER_OLD)
    };
    git.run(["update-ref", new_ref, new_tip])?;
    git.run(["update-ref", old_ref, old_tip])?;
    Ok(())
}

fn read_ref(git: &Git, name: &str) -> Option<String> {
    git.run_line([
        "rev-parse",
        "--verify",
        "--quiet",
        &format!("{}^{{commit}}", name),
    ])
    .ok()
}

pub fn read_pending(git: &Git, location: &LedgerLocation) -> Vec<PendingRef> {
    let mut found = Vec::new();
    if let (Some(new_tip), Some(old_tip)) = (read_ref(git, LEDGER_NEW), read_ref(git, LEDGER_OLD)) {
        found.push(PendingRef {
            label: "ledger data",
            ref_name: location.ref_name().to_string(),
            old_tip,
            new_tip,
        });
    }
    if let (Some(new_tip), Some(old_tip)) = (read_ref(git, DOCS_NEW), read_ref(git, DOCS_OLD)) {
        found.push(PendingRef {
            label: "documents",
            ref_name: location.docs().ref_name().to_string(),
            old_tip,
            new_tip,
        });
    }
    found
}

pub fn clear_pending(git: &Git) {
    for name in [LEDGER_NEW, LEDGER_OLD, DOCS_NEW, DOCS_OLD] {
        let _ = git.run(["update-ref", "-d", name]);
    }
}

pub fn push_args(remote: &str, pending: &[PendingRef]) -> Vec<String> {
    let mut args = vec![
        "push".to_string(),
        "--porcelain".to_string(),
        "--atomic".to_string(),
    ];
    for entry in pending {
        args.push(format!(
            "--force-with-lease={}:{}",
            entry.ref_name, entry.old_tip
        ));
    }
    args.push(remote.to_string());
    for entry in pending {
        args.push(format!("{}:{}", entry.new_tip, entry.ref_name));
    }
    args
}

pub fn push_command_text(label: &str, pending: &[PendingRef]) -> String {
    let leases: Vec<String> = pending
        .iter()
        .map(|entry| format!("--force-with-lease={}:{}", entry.ref_name, entry.old_tip))
        .collect();
    let specs: Vec<String> = pending
        .iter()
        .map(|entry| format!("{}:{}", entry.new_tip, entry.ref_name))
        .collect();
    format!(
        "git push --atomic {} {} {}",
        leases.join(" "),
        label,
        specs.join(" ")
    )
}

pub fn pending_lines(pending: &[PendingRef], label: &str) -> Vec<String> {
    let mut lines = Vec::new();
    for entry in pending {
        lines.push(format!(
            "  {} {} on {}: {} -> {}",
            entry.label,
            entry.ref_name,
            label,
            erase::short(&entry.old_tip),
            erase::short(&entry.new_tip)
        ));
    }
    lines
}

pub fn other_homes_holding(ledgers_dir: &Path, own_home: &Path, tips: &[String]) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(ledgers_dir) else {
        return Vec::new();
    };
    let mut found: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path != own_home)
        .filter(|path| {
            let git_dir = path.join(super::home::GIT_DIR_NAME);
            git_dir.join("HEAD").is_file()
                && tips.iter().any(|tip| erase::still_present(&git_dir, tip))
        })
        .collect();
    found.sort();
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn run(dir: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .args(args)
            .current_dir(dir)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {:?}: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap().trim().to_string()
    }

    fn commit(dir: &Path, file: &str, author: &str, committer: &str) -> String {
        std::fs::write(dir.join(file), file).unwrap();
        run(dir, &["add", file]);
        let output = Command::new("git")
            .args(["commit", "--quiet", "-m", &format!("Add {}", file)])
            .current_dir(dir)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_AUTHOR_NAME", "Author")
            .env("GIT_AUTHOR_EMAIL", author)
            .env("GIT_COMMITTER_NAME", "Committer")
            .env("GIT_COMMITTER_EMAIL", committer)
            .output()
            .unwrap();
        assert!(output.status.success());
        run(dir, &["rev-parse", "HEAD"])
    }

    fn repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        run(dir.path(), &["init", "--quiet", "-b", "main"]);
        dir
    }

    fn noreply() -> String {
        format!("1+robot{}", NOREPLY)
    }

    #[test]
    fn a_noreply_address_is_required_and_a_plain_one_is_refused() {
        assert!(Selection::new(&noreply(), &[]).is_ok());
        assert!(Selection::new("jim@example.com", &[]).is_err());
        assert!(Selection::new("noreply", &[]).is_err());
        assert!(Selection::new("a b@noreply.example.com", &[]).is_err());
        assert!(Selection::new(&noreply(), &["not an email".to_string()]).is_err());
    }

    #[test]
    fn without_from_every_address_outside_the_noreply_form_is_selected() {
        let selection = Selection::new(&noreply(), &[]).unwrap();
        assert!(selection.selects("jim@example.com"));
        assert!(!selection.selects("2+other@users.noreply.github.com"));
        assert!(!selection.selects(&noreply().to_uppercase()));
    }

    #[test]
    fn with_from_only_the_named_addresses_are_selected_whatever_their_case() {
        let selection = Selection::new(&noreply(), &["Jim@Example.com".to_string()]).unwrap();
        assert!(selection.selects("jim@example.com"));
        assert!(!selection.selects("other@example.com"));
        assert!(!selection.selects(&noreply()));
    }

    #[test]
    fn a_scan_counts_authors_and_committers_apart() {
        let dir = repo();
        commit(dir.path(), "a", "a@example.com", "b@example.com");
        commit(dir.path(), "b", "a@example.com", "a@example.com");
        let counts = scan(&Git::new(dir.path()), "HEAD").unwrap();
        assert_eq!(
            counts,
            vec![
                EmailCount {
                    email: "a@example.com".to_string(),
                    authored: 2,
                    committed: 1
                },
                EmailCount {
                    email: "b@example.com".to_string(),
                    authored: 0,
                    committed: 1
                },
            ]
        );
    }

    #[test]
    fn a_rewrite_changes_emails_and_parents_and_keeps_trees_names_dates_and_messages() {
        let dir = repo();
        let git = Git::new(dir.path());
        let first = commit(dir.path(), "a", &noreply(), &noreply());
        commit(dir.path(), "b", "a@example.com", &noreply());
        commit(dir.path(), "c", &noreply(), &noreply());
        let tip = run(dir.path(), &["rev-parse", "HEAD"]);
        let selection = Selection::new(&noreply(), &[]).unwrap();

        let done = rewrite(&git, &tip, &selection, None, false)
            .unwrap()
            .unwrap();

        assert_eq!(done.total, 3);
        assert_eq!(done.rewritten, 2);
        assert_ne!(done.new_tip, tip);
        assert_eq!(
            run(
                dir.path(),
                &["rev-parse", &format!("{}^{{tree}}", done.new_tip)]
            ),
            run(dir.path(), &["rev-parse", &format!("{}^{{tree}}", tip)])
        );
        let emails = run(dir.path(), &["log", "--format=%ae %ce", &done.new_tip]);
        assert!(!emails.contains("example.com"), "{}", emails);
        let names = run(dir.path(), &["log", "--format=%an %cn", &done.new_tip]);
        assert!(names.lines().all(|l| l == "Author Committer"), "{}", names);
        assert_eq!(
            run(dir.path(), &["log", "--format=%s", &done.new_tip])
                .lines()
                .collect::<Vec<_>>(),
            vec!["Add c", "Add b", "Add a"]
        );
        let oldest = run(dir.path(), &["rev-list", "--max-parents=0", &done.new_tip]);
        assert_eq!(oldest, first);
        assert_ne!(done.first_changed, first);
        assert_eq!(
            run(dir.path(), &["log", "-1", "--format=%at", &done.new_tip]),
            run(dir.path(), &["log", "-1", "--format=%at", &tip])
        );
    }

    #[test]
    fn a_history_with_nothing_to_replace_is_not_rewritten() {
        let dir = repo();
        commit(dir.path(), "a", &noreply(), &noreply());
        let selection = Selection::new(&noreply(), &[]).unwrap();
        let tip = run(dir.path(), &["rev-parse", "HEAD"]);
        assert_eq!(
            rewrite(&Git::new(dir.path()), &tip, &selection, None, false).unwrap(),
            None
        );
    }

    #[test]
    fn a_dry_run_counts_the_rewrite_and_creates_no_commit() {
        let dir = repo();
        commit(dir.path(), "a", "a@example.com", "a@example.com");
        commit(dir.path(), "b", &noreply(), &noreply());
        let tip = run(dir.path(), &["rev-parse", "HEAD"]);
        let selection = Selection::new(&noreply(), &[]).unwrap();
        let before = run(dir.path(), &["count-objects", "-v"]);

        let done = rewrite(&Git::new(dir.path()), &tip, &selection, None, true)
            .unwrap()
            .unwrap();

        assert_eq!(done.rewritten, 2);
        assert_eq!(run(dir.path(), &["count-objects", "-v"]), before);
    }

    #[test]
    fn a_merge_keeps_both_parents_after_the_rewrite() {
        let dir = repo();
        let git = Git::new(dir.path());
        commit(dir.path(), "base", "a@example.com", "a@example.com");
        run(dir.path(), &["checkout", "--quiet", "-b", "side"]);
        commit(dir.path(), "side", "a@example.com", "a@example.com");
        run(dir.path(), &["checkout", "--quiet", "main"]);
        commit(dir.path(), "main", "a@example.com", "a@example.com");
        run(
            dir.path(),
            &[
                "-c",
                "user.name=m",
                "-c",
                "user.email=m@example.com",
                "merge",
                "--quiet",
                "--no-ff",
                "-m",
                "Merge",
                "side",
            ],
        );
        let tip = run(dir.path(), &["rev-parse", "HEAD"]);
        let selection = Selection::new(&noreply(), &[]).unwrap();

        let done = rewrite(&git, &tip, &selection, None, false)
            .unwrap()
            .unwrap();

        assert_eq!(done.total, 4);
        assert_eq!(done.rewritten, 4);
        let parents = run(dir.path(), &["log", "-1", "--format=%P", &done.new_tip]);
        assert_eq!(parents.split_whitespace().count(), 2);
        let all = run(dir.path(), &["log", "--format=%ae %ce", &done.new_tip]);
        assert!(!all.contains("example.com"), "{}", all);
    }

    #[test]
    fn a_rewrite_with_a_signer_signs_every_rewritten_commit() {
        let dir = repo();
        let keys = tempfile::tempdir().unwrap();
        let key = keys.path().join("key");
        let made = Command::new("ssh-keygen")
            .args(["-q", "-t", "ed25519", "-N", "", "-f"])
            .arg(&key)
            .output();
        if !made.is_ok_and(|output| output.status.success()) {
            return;
        }
        commit(dir.path(), "a", "a@example.com", "a@example.com");
        commit(dir.path(), "b", "a@example.com", "a@example.com");
        let tip = run(dir.path(), &["rev-parse", "HEAD"]);
        let signer = Signer {
            options: [
                ("gpg.format", "ssh"),
                ("gpg.ssh.program", "ssh-keygen"),
                ("user.signingkey", key.to_str().unwrap()),
                ("commit.gpgsign", "true"),
            ]
            .iter()
            .flat_map(|(k, v)| ["-c".to_string(), format!("{}={}", k, v)])
            .collect(),
            env: Vec::new(),
            name: "Robot".to_string(),
        };
        let selection = Selection::new(&noreply(), &[]).unwrap();

        let done = rewrite(
            &Git::new(dir.path()),
            &tip,
            &selection,
            Some(&signer),
            false,
        )
        .unwrap()
        .unwrap();

        for id in run(dir.path(), &["rev-list", &done.new_tip]).lines() {
            let raw = run(dir.path(), &["cat-file", "commit", id]);
            assert!(raw.contains("gpgsig"), "{} is not signed:\n{}", id, raw);
        }
    }

    #[test]
    fn the_push_arguments_lease_each_ref_on_its_old_tip() {
        let pending = vec![
            PendingRef {
                label: "ledger data",
                ref_name: "refs/heads/tsk/ledger".to_string(),
                old_tip: "aaa".to_string(),
                new_tip: "bbb".to_string(),
            },
            PendingRef {
                label: "documents",
                ref_name: "refs/heads/tsk/docs".to_string(),
                old_tip: "ccc".to_string(),
                new_tip: "ddd".to_string(),
            },
        ];
        assert_eq!(
            push_args("origin", &pending),
            vec![
                "push",
                "--porcelain",
                "--atomic",
                "--force-with-lease=refs/heads/tsk/ledger:aaa",
                "--force-with-lease=refs/heads/tsk/docs:ccc",
                "origin",
                "bbb:refs/heads/tsk/ledger",
                "ddd:refs/heads/tsk/docs",
            ]
        );
    }

    #[test]
    fn other_homes_that_hold_a_commit_are_found_and_the_own_home_is_not() {
        let state = tempfile::tempdir().unwrap();
        let own = state.path().join("own");
        let other = state.path().join("other");
        let empty = state.path().join("empty");
        for home in [&own, &other, &empty] {
            std::fs::create_dir_all(home.join("git")).unwrap();
            run(&home.join("git"), &["init", "--quiet", "--bare"]);
        }
        let work = repo();
        let id = commit(work.path(), "a", "a@example.com", "a@example.com");
        for home in [&own, &other] {
            run(
                &home.join("git"),
                &["fetch", "--quiet", work.path().to_str().unwrap(), "HEAD"],
            );
        }
        assert_eq!(other_homes_holding(state.path(), &own, &[id]), vec![other]);
    }
}
