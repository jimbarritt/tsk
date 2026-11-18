//! A ledger in the state layout of tsk 0.69.0 and earlier (ADR 0035), made from a ledger that
//! the current `tsk` built.
//!
//! The current binary builds a ledger in its home (`<state>/ledgers/<key>`). `make_legacy`
//! turns that into what 0.69.0 left on a machine: `<state>/repos/<clone-id>/` with `ledger-db`,
//! `docs` and `ledger-remote`, the last two as linked worktrees of the code repo, the ledger
//! objects in the code repo under `refs/tsk-fetch/*`, the Dolt URL naming `ledger-remote`, and
//! on a nexus the old ref names. The next `tsk` command has to move it all back.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Output;

/// What a legacy fixture holds that origin lacks, and where it is.
#[derive(Debug, Clone, Copy, Default)]
pub struct Knobs {
    /// One Dolt commit that is not pushed, and one uncommitted row.
    pub unpushed_dolt: bool,
    /// An uncommitted document and an unpushed documents commit.
    pub dirty_docs: bool,
    /// The nexus refs are renamed to the old names. Off for a ledger in its repo.
    pub nexus: bool,
}

/// The names of a clone's things.
pub struct Layout {
    pub state: PathBuf,
    pub work: PathBuf,
    pub clone_id: String,
}

impl Layout {
    pub fn new(state: &Path, work: &Path) -> Layout {
        let clone_id = std::fs::read_to_string(work.join(".git").join("tsk-clone-id"))
            .unwrap()
            .trim()
            .to_string();
        Layout {
            state: state.to_path_buf(),
            work: work.to_path_buf(),
            clone_id,
        }
    }

    pub fn clone_dir(&self) -> PathBuf {
        self.state.join("tsk").join("repos").join(&self.clone_id)
    }

    pub fn home(&self) -> PathBuf {
        super::ledger_home(&self.state, &self.work)
    }
}

fn run(dir: &Path, program: &str, args: &[&str], envs: &[(&str, &str)]) -> Output {
    let mut command = super::cmd(program);
    command
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@example.com");
    for (key, value) in envs {
        command.env(key, value);
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{} {:?} failed in {}: {}",
        program,
        args,
        dir.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

pub fn git(dir: &Path, args: &[&str]) -> String {
    String::from_utf8(run(dir, "git", args, &[]).stdout)
        .unwrap()
        .trim_end()
        .to_string()
}

/// Run `dolt` in a store. A server that a command started is stopped first.
pub fn dolt(state: &Path, home: &Path, store: &Path, args: &[&str]) -> String {
    super::stop_servers(state, true);
    let envs = [
        ("HOME", home.to_str().unwrap()),
        ("DOLT_DISABLE_EVENT_FLUSH", "1"),
    ];
    String::from_utf8(run(store, "dolt", args, &envs).stdout).unwrap()
}

/// Turn the ledger of the clone `work`, built by the current `tsk` in a home, into the layout of
/// tsk 0.69.0 and earlier. `user_home` is the `HOME` of the fixture (it holds the Dolt
/// identity); `nexus_bare` is the bare nexus, when the ledger lives there.
pub fn make_legacy(
    layout: &Layout,
    user_home: &Path,
    nexus_bare: Option<&Path>,
    refs_prefix: &str,
    knobs: Knobs,
) {
    super::stop_servers(&layout.state, true);
    let home = layout.home();
    let clone_dir = layout.clone_dir();
    let work = &layout.work;
    std::fs::create_dir_all(&clone_dir).unwrap();

    // The Dolt store, with the unpushed work.
    std::fs::rename(home.join("ledger-db"), clone_dir.join("ledger-db")).unwrap();
    let store = clone_dir.join("ledger-db");
    if knobs.unpushed_dolt {
        dolt(
            &layout.state,
            user_home,
            &store,
            &[
                "sql",
                "-q",
                "replace into manifest (k, v) values ('unpushed-note', 'one')",
            ],
        );
        dolt(&layout.state, user_home, &store, &["add", "-A"]);
        dolt(
            &layout.state,
            user_home,
            &store,
            &["commit", "-m", "A local commit that is not pushed"],
        );
        dolt(
            &layout.state,
            user_home,
            &store,
            &[
                "sql",
                "-q",
                "replace into manifest (k, v) values ('uncommitted-note', 'two')",
            ],
        );
    }

    // The ledger ref: the name a 0.69.0 store recorded.
    let current = std::fs::read_to_string(home.join("ledger-ref")).unwrap();
    let current = current.trim();
    // A nexus ledger had `ledgers/<id>`; a ledger in its repo has `tsk/ledger`, as before.
    let recorded = if current.contains("/ledgers/") {
        current
            .strip_suffix("/ledger")
            .unwrap_or(current)
            .to_string()
    } else {
        current.to_string()
    };
    std::fs::write(clone_dir.join("ledger-ref"), format!("{}\n", recorded)).unwrap();
    if home.join("scratch").exists() {
        std::fs::rename(home.join("scratch"), clone_dir.join("scratch")).unwrap();
    }

    // The two worktrees become worktrees of the code repo, with their objects fetched into it.
    let bare = home.join("git");
    let docs_sha = git(&home.join("docs"), &["rev-parse", "HEAD"]);
    let ledger_sha = git(&home.join("ledger"), &["rev-parse", "HEAD"]);
    git(&bare, &["update-ref", "refs/legacy/ledger", &ledger_sha]);
    git(&bare, &["update-ref", "refs/legacy/docs", &docs_sha]);
    git(
        work,
        &[
            "fetch",
            "--quiet",
            bare.to_str().unwrap(),
            "+refs/legacy/ledger:refs/tsk-fetch/ledger",
            "+refs/legacy/docs:refs/tsk-fetch/docs",
        ],
    );
    let docs_new = clone_dir.join("docs");
    let remote_new = clone_dir.join("ledger-remote");
    git(
        work,
        &[
            "worktree",
            "add",
            "--detach",
            "--quiet",
            docs_new.to_str().unwrap(),
            &docs_sha,
        ],
    );
    git(
        work,
        &[
            "worktree",
            "add",
            "--detach",
            "--quiet",
            remote_new.to_str().unwrap(),
            &ledger_sha,
        ],
    );
    // What the new worktrees held beyond their commits (a push that was not published).
    copy_over(&home.join("ledger"), &remote_new);
    copy_over(&home.join("docs"), &docs_new);
    if knobs.dirty_docs {
        std::fs::write(
            docs_new.join("unpushed-commit.md"),
            "# committed, not pushed\n",
        )
        .unwrap();
        git(&docs_new, &["add", "-A"]);
        git(
            &docs_new,
            &["commit", "--quiet", "-m", "An unpushed documents commit"],
        );
        std::fs::write(docs_new.join("uncommitted.md"), "# not committed\n").unwrap();
    }

    // The store's origin names the old directory.
    let state_file = store.join(".dolt").join("repo_state.json");
    let text = std::fs::read_to_string(&state_file).unwrap();
    let old_url = format!(
        "file://{}",
        std::fs::canonicalize(home.join("ledger"))
            .unwrap()
            .display()
    );
    let new_url = format!(
        "file://{}",
        std::fs::canonicalize(&remote_new).unwrap().display()
    );
    assert!(text.contains(&old_url), "{}\n{}", old_url, text);
    std::fs::write(&state_file, text.replace(&old_url, &new_url)).unwrap();

    // The home goes, and with it the bare clone and the code repo's name for it.
    std::fs::remove_dir_all(&home).unwrap();
    let _ = std::fs::remove_file(work.join(".git").join("tsk-ledger-home"));

    if let Some(nexus) = nexus_bare {
        // The old names: the refs are renamed with plumbing, which has no D/F check to go through.
        let pairs = [("ledger", ""), ("docs", "-docs")];
        let mut moves = Vec::new();
        for (new, old) in pairs {
            let new_ref = format!("{}ledgers/work-api/{}", refs_prefix, new);
            let old_ref = format!("{}ledgers/work-api{}", refs_prefix, old);
            moves.push((git(nexus, &["rev-parse", &new_ref]), new_ref, old_ref));
        }
        // Both new refs go before either old one comes: git cannot hold `ledgers/x` and
        // `ledgers/x/docs` together.
        for (_, new_ref, _) in &moves {
            git(nexus, &["update-ref", "-d", new_ref]);
        }
        for (sha, _, old_ref) in &moves {
            git(nexus, &["update-ref", old_ref, sha]);
        }
    }
}

/// Copy every file of `from` except `.git` over `to`.
fn copy_over(from: &Path, to: &Path) {
    let output = super::cmd("sh")
        .arg("-c")
        .arg("tar cf - --exclude=./.git . | tar xf - -C \"$1\"")
        .arg("sh")
        .arg(to)
        .current_dir(from)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "copy failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
