//! ADR 0035: the ledger home of a repo, outside the code repo, and the automatic move of the
//! state and the nexus refs of tsk 0.69.0 and earlier.
//!
//! Each case builds a ledger with the current `tsk`, turns it into the legacy layout
//! (`common::legacy`), and runs the current `tsk` against it. A built fixture is a template,
//! copied for each test (see `common::templates`).

use std::path::{Path, PathBuf};
use std::process::Output;

use tempfile::TempDir;

use crate::common;
use common::legacy::{self, Knobs, Layout};
use common::templates;

const BRIEFING: &str = "missions/M-A.1c0de050.md";
const ENTRY: &str = r#"{"id":"work-api","local":"laptop","ledger":"nexus","prefix":"WRK"}"#;

struct Fx {
    root: TempDir,
    refs: &'static str,
}

impl templates::Root for Fx {
    fn root(&self) -> &Path {
        self.root.path()
    }
}

impl Drop for Fx {
    fn drop(&mut self) {
        common::stop_servers(self.root.path(), false);
    }
}

fn prefix(refs: &str) -> &'static str {
    if refs == "branches" {
        "refs/heads/"
    } else {
        "refs/tsk/"
    }
}

impl Fx {
    /// The directories, a bare origin with a clone `work` (and `work2`), and, for a ledger in a
    /// nexus, a bare nexus with an entry for this repo, attached.
    fn new(refs: &'static str, nexus: bool) -> Fx {
        let fx = Fx {
            root: tempfile::tempdir().unwrap(),
            refs,
        };
        let root = fx.root.path();
        for dir in ["home", "state", "config"] {
            std::fs::create_dir_all(root.join(dir)).unwrap();
        }
        common::seed_dolt_home(&root.join("home"));
        git(
            root,
            &["init", "--quiet", "--bare", "-b", "main", "origin.git"],
        );
        git(root, &["init", "--quiet", "-b", "main", "seed"]);
        let seed = root.join("seed");
        git(
            &seed,
            &["remote", "add", "origin", fx.origin().to_str().unwrap()],
        );
        write(&seed.join("README.md"), "main\n");
        commit_all(&seed, "Main commit");
        git(
            &seed,
            &["push", "--quiet", "origin", "HEAD:refs/heads/main"],
        );
        for name in ["work", "work2"] {
            git(
                root,
                &["clone", "--quiet", fx.origin().to_str().unwrap(), name],
            );
        }
        if nexus {
            git(
                root,
                &["init", "--quiet", "--bare", "-b", "main", "nexus.git"],
            );
            git(root, &["init", "--quiet", "-b", "main", "nexus-seed"]);
            git(
                &fx.nexus_seed(),
                &["remote", "add", "origin", fx.nexus().to_str().unwrap()],
            );
            fx.set_nexus_json(ENTRY);
            assert_success(&fx.tsk(
                &fx.work(),
                &["config", "attach-nexus", fx.nexus().to_str().unwrap()],
            ));
            for clone in [fx.work(), fx.work2()] {
                write(&clone.join(".git").join("tsk-repo-id"), "work-api\n");
            }
        }
        fx
    }

    fn origin(&self) -> PathBuf {
        self.root.path().join("origin.git")
    }
    fn nexus(&self) -> PathBuf {
        self.root.path().join("nexus.git")
    }
    fn nexus_seed(&self) -> PathBuf {
        self.root.path().join("nexus-seed")
    }
    fn work(&self) -> PathBuf {
        self.root.path().join("work")
    }
    fn work2(&self) -> PathBuf {
        self.root.path().join("work2")
    }
    fn state(&self) -> PathBuf {
        self.root.path().join("state")
    }
    fn user_home(&self) -> PathBuf {
        self.root.path().join("home")
    }
    fn tsk_dir(&self) -> PathBuf {
        self.state().join("tsk")
    }
    fn layout(&self, work: &Path) -> Layout {
        Layout::new(&self.state(), work)
    }
    fn home_of(&self, work: &Path) -> PathBuf {
        common::ledger_home(&self.state(), work)
    }

    fn set_nexus_json(&self, entry: &str) {
        let seed = self.nexus_seed();
        if git_output(&seed, &["fetch", "--quiet", "origin", "main"])
            .status
            .success()
        {
            git(&seed, &["reset", "--quiet", "--hard", "FETCH_HEAD"]);
        }
        write(
            &seed.join("nexus.json"),
            &format!(
                "{{\"version\":1,\"territories\":[{{\"id\":\"t\",\"name\":\"T\",\"repos\":[{}]}}]}}\n",
                entry
            ),
        );
        commit_all(&seed, "Set nexus.json");
        git(
            &seed,
            &["push", "--quiet", "origin", "HEAD:refs/heads/main"],
        );
    }

    fn command(&self, dir: &Path, args: &[&str]) -> std::process::Command {
        let mut command = common::cmd(env!("CARGO_BIN_EXE_tsk"));
        command
            .args(args)
            .current_dir(dir)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "test@example.com")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "test@example.com")
            .env("HOME", self.user_home())
            .env("XDG_STATE_HOME", self.state())
            .env("XDG_CONFIG_HOME", self.root.path().join("config"))
            .env("TSK_HOME", self.user_home().join(".tsk"))
            .env("TSK_MACHINE_NAME", "laptop")
            .env("TSK_LEDGER_REFS", self.refs)
            .env("TSK_DOLT_SERVER", "off")
            .env("TSK_NO_UPDATE_CHECK", "1")
            .env("TSK_AUTO_UPDATE", "never")
            .env("TSK_BACKGROUND_PUBLISH", "off")
            .env("GIT_CEILING_DIRECTORIES", self.root.path())
            .env_remove("CLAUDE_CODE_REMOTE")
            .env_remove("CLAUDE_CODE_REMOTE_SESSION_ID")
            .env_remove("TSK_LEDGER_TRANSPORT");
        common::with_git_shim(&mut command, self.root.path());
        command
    }

    fn tsk(&self, dir: &Path, args: &[&str]) -> Output {
        self.command(dir, args).output().expect("failed to run tsk")
    }

    fn tsk_env(&self, dir: &Path, args: &[&str], envs: &[(&str, &str)]) -> Output {
        let mut command = self.command(dir, args);
        for (key, value) in envs {
            command.env(key, value);
        }
        command.output().expect("failed to run tsk")
    }

    fn ok(&self, dir: &Path, args: &[&str]) -> String {
        let output = self.tsk(dir, args);
        assert_success(&output);
        stdout(&output)
    }

    /// The ledger of `work`, built by the current tsk: a document, a mission, a push.
    fn build_ledger(&self) {
        let work = self.work();
        self.ok(&work, &["ledger", "fetch"]);
        let doc = self.root.path().join("briefing.md");
        write(&doc, "# Mission: alpha\n");
        self.ok(
            &work,
            &["ledger", "doc", "put", BRIEFING, doc.to_str().unwrap()],
        );
        self.ok(&work, &["mission", "create", "M-A", "Alpha"]);
        self.ok(&work, &["ledger", "push", "Seed the ledger"]);
    }

    fn make_legacy(&self, work: &Path, knobs: Knobs) {
        let nexus = knobs.nexus.then(|| self.nexus());
        legacy::make_legacy(
            &self.layout(work),
            &self.user_home(),
            nexus.as_deref(),
            prefix(self.refs),
            knobs,
        );
    }

    /// A template: the fixture, a built ledger in the current layout (`knobs` None) or in the
    /// layout of 0.69.0 and earlier.
    fn template(
        key: &str,
        refs: &'static str,
        nexus: bool,
        knobs: Option<Knobs>,
        second: bool,
    ) -> Fx {
        let key = format!(
            "migrate-home-{}-{}-{}-{:?}-{}",
            key, refs, nexus, knobs, second
        );
        let (root, _) = templates::cached(&key, || {
            let fx = Fx::new(refs, nexus);
            fx.build_ledger();
            if let Some(knobs) = knobs {
                if second {
                    let (a, b) = (fx.work(), fx.work2());
                    fx.ok(&b, &["ledger", "fetch"]);
                    fx.ok(&a, &["task", "add", "M-A", "Task from the first clone"]);
                    fx.make_legacy(
                        &a,
                        Knobs {
                            nexus: false,
                            ..knobs
                        },
                    );
                    fx.ok(&b, &["ledger", "fetch"]);
                    fx.ok(&b, &["task", "add", "M-A", "Task from the second clone"]);
                    fx.make_legacy(&b, knobs);
                } else {
                    fx.make_legacy(&fx.work(), knobs);
                }
                if nexus {
                    // An entry written by 0.69.0 has no marker.
                    fx.set_nexus_json(ENTRY);
                }
            }
            (fx, String::new())
        });
        Fx { root, refs }
    }

    fn legacy(refs: &'static str, nexus: bool, knobs: Knobs) -> Fx {
        Fx::template("legacy", refs, nexus, Some(Knobs { nexus, ..knobs }), false)
    }

    fn current(refs: &'static str, nexus: bool) -> Fx {
        Fx::template("current", refs, nexus, None, false)
    }

    fn ref_exists(&self, repo: &Path, name: &str) -> bool {
        git_output(repo, &["rev-parse", "--verify", "--quiet", name])
            .status
            .success()
    }

    fn refs_of(&self, repo: &Path) -> Vec<String> {
        git(repo, &["for-each-ref", "--format=%(refname)"])
            .lines()
            .map(str::to_string)
            .collect()
    }

    fn entry_marker(&self) -> Option<String> {
        let text = git(&self.nexus(), &["show", "refs/heads/main:nexus.json"]);
        let json: serde_json::Value = serde_json::from_str(&text).unwrap();
        json["territories"][0]["repos"][0]["ledger_refs"]
            .as_str()
            .map(str::to_string)
    }

    fn store_sql(&self, work: &Path, sql: &str) -> String {
        let store = self.home_of(work).join("ledger-db");
        legacy::dolt(
            &self.state(),
            &self.user_home(),
            &store,
            &["sql", "-r", "csv", "-q", sql],
        )
    }
}

fn git_output(dir: &Path, args: &[&str]) -> Output {
    common::cmd("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@example.com")
        .output()
        .expect("failed to run git")
}

fn git(dir: &Path, args: &[&str]) -> String {
    let output = git_output(dir, args);
    assert!(
        output.status.success(),
        "git {:?} failed in {}: {}",
        args,
        dir.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .unwrap()
        .trim_end()
        .to_string()
}

fn commit_all(dir: &Path, message: &str) {
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "--quiet", "-m", message]);
}

fn write(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, content).unwrap();
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "tsk failed: status {:?}\nstdout: {}\nstderr: {}",
        output.status,
        stdout(output),
        stderr(output)
    );
}

fn assert_failure_mentioning(output: &Output, needle: &str) {
    assert!(
        !output.status.success() && stderr(output).contains(needle),
        "expected a failure mentioning {:?}\nstatus: {:?}\nstdout: {}\nstderr: {}",
        needle,
        output.status,
        stdout(output),
        stderr(output)
    );
}

const FULL: Knobs = Knobs {
    unpushed_dolt: true,
    dirty_docs: true,
    nexus: true,
};

const BOTH_MODES: [&str; 2] = ["custom", "branches"];

// --- A. The local move ----------------------------------------------------------------

#[test]
fn upgrade_moves_a_legacy_store_and_unpushed_dolt_commits_survive() {
    if !common::dolt_installed() {
        return;
    }
    for refs in BOTH_MODES {
        let fx = Fx::legacy(refs, true, FULL);
        let work = fx.work();
        let layout = fx.layout(&work);
        assert!(layout.clone_dir().join("ledger-db").is_dir());

        let output = fx.tsk(&work, &["ledger", "status"]);
        assert_success(&output);
        assert!(
            stderr(&output).contains("moved the ledger state of this clone"),
            "{}",
            stderr(&output)
        );
        let home = fx.state().join("tsk/ledgers/work-api");
        assert_eq!(fx.home_of(&work), home);
        assert!(home.join("ledger-db/.dolt").is_dir());
        assert!(!layout.clone_dir().join("ledger-db").exists());
        assert!(stdout(&output).contains(&format!("store {}", home.join("ledger-db").display())));
        // What the status of the home shows.
        assert!(
            stdout(&output).contains(&format!("refs {} (", refs)),
            "{}",
            stdout(&output)
        );

        // The commit that was not pushed and the row that was not committed are both there.
        let log = legacy::dolt(
            &fx.state(),
            &fx.user_home(),
            &home.join("ledger-db"),
            &["log", "--oneline"],
        );
        assert!(log.contains("A local commit that is not pushed"), "{}", log);
        let rows = fx.store_sql(&work, "select k, v from manifest where k like '%-note'");
        assert!(
            rows.contains("unpushed-note,one") && rows.contains("uncommitted-note,two"),
            "{}",
            rows
        );

        // The store still publishes: its remote is the directory in the home.
        let pushed = fx.tsk(&work, &["ledger", "push", "After the move"]);
        assert_success(&pushed);

        let status = fx.ok(&work, &["ledger", "status"]);
        assert!(status.contains("Local   nothing to push"), "{}", status);
    }
}

#[test]
fn upgrade_keeps_uncommitted_and_unpushed_documents() {
    if !common::dolt_installed() {
        return;
    }
    let fx = Fx::legacy("custom", true, FULL);
    let work = fx.work();
    assert_success(&fx.tsk(&work, &["ledger", "status"]));
    let docs = fx.home_of(&work).join("docs");
    assert_eq!(
        std::fs::read_to_string(docs.join("uncommitted.md")).unwrap(),
        "# not committed\n"
    );
    assert_eq!(
        std::fs::read_to_string(docs.join("unpushed-commit.md")).unwrap(),
        "# committed, not pushed\n"
    );
    assert!(git(&docs, &["log", "--format=%s", "-3"]).contains("An unpushed documents commit"));
    assert!(git(&docs, &["status", "--porcelain"]).contains("uncommitted.md"));
    // The push sends both.
    assert_success(&fx.tsk(&work, &["ledger", "push", "Documents"]));
    let docs_ref = "refs/tsk/ledgers/work-api/docs";
    let listing = git(&fx.nexus(), &["ls-tree", "-r", "--name-only", docs_ref]);
    // The push gave each document the id of its path (layout 14).
    assert!(
        listing.contains("uncommitted.") && listing.contains("unpushed-commit."),
        "{}",
        listing
    );
}

#[test]
fn upgrade_leaves_no_ledger_worktree_ref_or_state_in_the_code_repo() {
    if !common::dolt_installed() {
        return;
    }
    let fx = Fx::legacy("custom", true, FULL);
    let work = fx.work();
    assert!(git(&work, &["worktree", "list"]).lines().count() > 1);
    assert!(!git(&work, &["for-each-ref", "refs/tsk-fetch"]).is_empty());
    assert_success(&fx.tsk(&work, &["ledger", "status"]));
    assert_eq!(git(&work, &["worktree", "list"]).lines().count(), 1);
    assert_eq!(git(&work, &["for-each-ref", "refs/tsk-fetch"]), "");
    let mut names: Vec<String> = std::fs::read_dir(work.join(".git"))
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("tsk-") || n == "worktrees")
        .collect();
    names.sort();
    names.retain(|n| n != "worktrees");
    assert!(
        names.iter().all(|n| matches!(
            n.as_str(),
            "tsk-clone-id" | "tsk-repo-id" | "tsk-ledger-home"
        )),
        "{:?}",
        names
    );
    assert!(
        !work.join(".git/worktrees").exists()
            || std::fs::read_dir(work.join(".git/worktrees"))
                .unwrap()
                .count()
                == 0
    );
}

#[test]
fn upgrade_leaves_per_clone_files_in_place() {
    if !common::dolt_installed() {
        return;
    }
    let fx = Fx::legacy("custom", true, FULL);
    let work = fx.work();
    let clone_dir = fx.layout(&work).clone_dir();
    write(&clone_dir.join("harness-names"), "ada\n");
    write(&clone_dir.join("robot-signing-key"), "key\n");
    write(&clone_dir.join("hooks/pre-docs-commit"), "#!/bin/sh\n");
    assert_success(&fx.tsk(&work, &["ledger", "status"]));
    for file in [
        "harness-names",
        "robot-signing-key",
        "hooks/pre-docs-commit",
    ] {
        assert!(clone_dir.join(file).exists(), "{} was moved", file);
    }
    assert!(!fx.home_of(&work).join("harness-names").exists());
}

#[test]
fn upgrade_is_idempotent_when_run_twice() {
    if !common::dolt_installed() {
        return;
    }
    let fx = Fx::legacy("custom", true, FULL);
    let work = fx.work();
    let first = fx.tsk(&work, &["ledger", "status"]);
    assert_success(&first);
    assert!(stderr(&first).contains("moved the ledger state"));
    let second = fx.tsk(&work, &["ledger", "status"]);
    assert_success(&second);
    assert!(!stderr(&second).contains("moved"), "{}", stderr(&second));
    assert_eq!(stdout(&first), stdout(&second));
}

#[test]
fn upgrade_resumes_after_an_interruption_at_each_step() {
    if !common::dolt_installed() {
        return;
    }
    for step in tsk_steps() {
        let fx = Fx::legacy("custom", true, FULL);
        let work = fx.work();
        let stopped = fx.tsk_env(
            &work,
            &["ledger", "status"],
            &[("TSK_TEST_MIGRATE_STOP_AFTER", step)],
        );
        assert_eq!(
            stopped.status.code(),
            Some(97),
            "{}: {}",
            step,
            stderr(&stopped)
        );
        let again = fx.tsk(&work, &["ledger", "status"]);
        assert_success(&again);
        let status = stdout(&again);
        assert!(
            status.contains("2 changes not pushed") || status.contains("changes not pushed"),
            "{}: {}",
            step,
            status
        );
        let home = fx.home_of(&work);
        assert!(home.join("ledger-db/.dolt").is_dir(), "{}", step);
        assert_eq!(
            std::fs::read_to_string(home.join("docs/uncommitted.md")).unwrap(),
            "# not committed\n",
            "{}",
            step
        );
        let rows = fx.store_sql(&work, "select k from manifest where k like '%-note'");
        assert!(
            rows.contains("unpushed-note") && rows.contains("uncommitted-note"),
            "{}: {}",
            step,
            rows
        );
        let layout = fx.layout(&work);
        assert!(!layout.clone_dir().join("docs").exists(), "{}", step);
        assert!(!layout.clone_dir().join("ledger-db").exists(), "{}", step);
        assert_eq!(
            git(&work, &["worktree", "list"]).lines().count(),
            1,
            "{}",
            step
        );
        assert_success(&fx.tsk(&work, &["ledger", "push", "After"]));
    }
}

fn tsk_steps() -> Vec<&'static str> {
    vec![
        "servers",
        "git",
        "dolt",
        "docs",
        "ledger",
        "url",
        "scratch",
        "verify",
        "unregister",
        "finish",
    ]
}

#[test]
fn two_processes_migrating_at_once_leave_one_consistent_home() {
    if !common::dolt_installed() {
        return;
    }
    let fx = Fx::legacy("custom", true, FULL);
    let work = fx.work();
    let first = fx.command(&work, &["ledger", "status"]).spawn_output();
    let second = fx.command(&work, &["ledger", "status"]).spawn_output();
    let (a, b) = (first(), second());
    assert_success(&a);
    assert_success(&b);
    let moved = [&a, &b]
        .iter()
        .filter(|o| stderr(o).contains("moved the ledger state"))
        .count();
    assert_eq!(moved, 1, "{}\n{}", stderr(&a), stderr(&b));
    assert_eq!(stdout(&a), stdout(&b));
    let homes: Vec<_> = std::fs::read_dir(fx.tsk_dir().join("ledgers"))
        .unwrap()
        .flatten()
        .collect();
    assert_eq!(homes.len(), 1);
}

trait SpawnOutput {
    fn spawn_output(self) -> Box<dyn FnOnce() -> Output>;
}

impl SpawnOutput for std::process::Command {
    fn spawn_output(mut self) -> Box<dyn FnOnce() -> Output> {
        let handle = std::thread::spawn(move || self.output().unwrap());
        Box::new(move || handle.join().unwrap())
    }
}

#[test]
fn the_verify_gate_stops_the_move_on_a_short_copy_and_deletes_nothing() {
    if !common::dolt_installed() {
        return;
    }
    let fx = Fx::legacy("custom", true, FULL);
    let work = fx.work();
    let layout = fx.layout(&work);
    let failed = fx.tsk_env(
        &work,
        &["ledger", "status"],
        &[("TSK_TEST_MIGRATE_CORRUPT_COPY", "docs")],
    );
    assert_failure_mentioning(&failed, "stopped at verify");
    // Nothing of the old state is gone.
    assert!(layout.clone_dir().join("docs/uncommitted.md").is_file());
    assert!(layout.clone_dir().join("ledger-remote").is_dir());
    assert_eq!(git(&work, &["worktree", "list"]).lines().count(), 3);
    // The next run makes the copy again.
    assert_success(&fx.tsk(&work, &["ledger", "status"]));
    assert_eq!(
        std::fs::read_to_string(fx.home_of(&work).join("docs/uncommitted.md")).unwrap(),
        "# not committed\n"
    );
}

#[test]
fn legacy_directories_are_renamed_to_leftovers_never_deleted() {
    if !common::dolt_installed() {
        return;
    }
    let fx = Fx::legacy("custom", true, FULL);
    let work = fx.work();
    let output = fx.tsk(&work, &["ledger", "status"]);
    assert_success(&output);
    let leftovers = fx.layout(&work).clone_dir().join("legacy-leftovers");
    assert!(leftovers.join("docs/uncommitted.md").is_file());
    assert!(leftovers.join("ledger-remote").is_dir());
    assert!(
        stderr(&output).contains(&leftovers.display().to_string()),
        "{}",
        stderr(&output)
    );
}

#[test]
fn only_our_worktree_registrations_are_removed_from_the_code_repo() {
    if !common::dolt_installed() {
        return;
    }
    let fx = Fx::legacy("custom", true, FULL);
    let work = fx.work();
    let mine = fx.root.path().join("my-worktree");
    git(
        &work,
        &[
            "worktree",
            "add",
            "--detach",
            "--quiet",
            mine.to_str().unwrap(),
        ],
    );
    std::fs::remove_dir_all(&mine).unwrap();
    assert_success(&fx.tsk(&work, &["ledger", "status"]));
    let listing = git(&work, &["worktree", "list", "--porcelain"]);
    assert!(listing.contains("my-worktree"), "{}", listing);
    assert!(!listing.contains("ledger-remote"), "{}", listing);
}

#[test]
fn second_clone_of_one_repo_is_merged_into_the_shared_home() {
    if !common::dolt_installed() {
        return;
    }
    let fx = Fx::template("legacy-second", "custom", true, Some(FULL), true);
    let (a, b) = (fx.work(), fx.work2());
    assert_success(&fx.tsk(&a, &["ledger", "status"]));
    let second = fx.tsk(&b, &["ledger", "status"]);
    assert_success(&second);
    assert!(
        stderr(&second).contains("merged the ledger store of another clone"),
        "{}",
        stderr(&second)
    );
    // Both clones now work in one home.
    assert_eq!(fx.home_of(&a), fx.home_of(&b));
    let tasks = fx.ok(&a, &["task", "list", "M-A"]);
    assert!(tasks.contains("Task from the first clone"), "{}", tasks);
    assert!(tasks.contains("Task from the second clone"), "{}", tasks);
    // The second copy is kept, not deleted.
    let preserved = fx.home_of(&a).join("preserved");
    assert!(preserved.is_dir() && std::fs::read_dir(&preserved).unwrap().count() == 1);
    // The documents of the second clone are in the shared worktree.
    let docs = fx.home_of(&a).join("docs");
    assert!(docs.join("uncommitted.md").is_file());
}

#[test]
fn second_clone_fold_renumbers_a_task_added_to_the_same_mission_in_both_clones() {
    if !common::dolt_installed() {
        return;
    }
    let fx = Fx::template("legacy-second", "custom", true, Some(FULL), true);
    let (a, b) = (fx.work(), fx.work2());
    assert_success(&fx.tsk(&a, &["ledger", "status"]));
    assert_success(&fx.tsk(&b, &["ledger", "status"]));
    let tasks = fx.ok(&a, &["task", "list", "M-A", "--porcelain"]);
    let ids: Vec<&str> = tasks.lines().filter_map(|l| l.split('\t').next()).collect();
    assert!(ids.contains(&"T-01") && ids.contains(&"T-02"), "{}", tasks);
}

/// Clone A stops after `stop`; clone B has a document of its own. Whatever the order of the
/// runs, B's document is in the shared documents worktree at the end.
fn second_clone_after_a_stopped_first(stop: &str, b_waits: bool) {
    let fx = Fx::template("legacy-second", "custom", true, Some(FULL), true);
    let (a, b) = (fx.work(), fx.work2());
    // Clone B has a document of its own, and not the uncommitted file both fixtures share:
    // that one would stop the merge as an untracked file of the shared worktree.
    let b_docs = fx.layout(&b).clone_dir().join("docs");
    std::fs::remove_file(b_docs.join("uncommitted.md")).unwrap();
    write(&b_docs.join("b_only.md"), "# only in clone B\n");
    let stopped = fx.tsk_env(
        &a,
        &["ledger", "status"],
        &[("TSK_TEST_MIGRATE_STOP_AFTER", stop)],
    );
    assert_eq!(stopped.status.code(), Some(97), "{}", stderr(&stopped));
    if b_waits {
        // A has copied the documents and not yet passed the gate: B must not fold into them.
        let early = fx.tsk(&b, &["ledger", "status"]);
        assert_failure_mentioning(&early, "has not finished");
    }
    let resumed = fx.tsk(&a, &["ledger", "status"]);
    assert_success(&resumed);
    if !b_waits {
        let bo = fx.tsk(&b, &["ledger", "status"]);
        assert_success(&bo);
        // A second run of A finds nothing to do and keeps B's document.
        assert_success(&fx.tsk(&a, &["ledger", "status"]));
    } else {
        let second = fx.tsk(&b, &["ledger", "status"]);
        assert_success(&second);
    }
    let docs = fx.home_of(&a).join("docs");
    assert_eq!(
        std::fs::read_to_string(docs.join("b_only.md")).unwrap(),
        "# only in clone B\n"
    );
    assert!(docs.join("uncommitted.md").is_file());
}

#[test]
fn a_second_clone_waits_for_a_first_move_that_stopped_before_the_gate() {
    if !common::dolt_installed() {
        return;
    }
    second_clone_after_a_stopped_first("docs", true);
}

#[test]
fn a_second_clone_folds_into_a_first_move_that_passed_the_gate_and_the_resume_keeps_it() {
    if !common::dolt_installed() {
        return;
    }
    second_clone_after_a_stopped_first("verify", false);
}

#[test]
fn linked_code_worktrees_and_two_clones_share_one_home() {
    if !common::dolt_installed() {
        return;
    }
    let fx = Fx::current("custom", true);
    let linked = fx.root.path().join("linked");
    git(
        &fx.work(),
        &[
            "worktree",
            "add",
            "--detach",
            "--quiet",
            linked.to_str().unwrap(),
        ],
    );
    let path_a = fx.ok(&fx.work(), &["ledger", "fetch"]);
    let path_b = fx.ok(&fx.work2(), &["ledger", "fetch"]);
    let path_c = fx.ok(&linked, &["ledger", "fetch"]);
    assert_eq!(path_a, path_b);
    assert_eq!(path_a, path_c);
    assert!(
        path_a.trim_end().ends_with("ledgers/work-api/ledger-db"),
        "{}",
        path_a
    );
    // The lock is one, so a command in one clone sees what another wrote.
    fx.ok(&fx.work2(), &["mission", "create", "M-B", "Beta"]);
    assert!(fx.ok(&linked, &["mission", "list"]).contains("M-B"));
}

#[test]
fn in_repo_ledger_gets_an_underscore_clone_home() {
    if !common::dolt_installed() {
        return;
    }
    let fx = Fx::legacy(
        "branches",
        false,
        Knobs {
            unpushed_dolt: true,
            dirty_docs: true,
            nexus: false,
        },
    );
    let work = fx.work();
    let clone_id = std::fs::read_to_string(work.join(".git/tsk-clone-id")).unwrap();
    assert_success(&fx.tsk(&work, &["ledger", "status"]));
    let home = fx
        .state()
        .join("tsk/ledgers")
        .join(format!("_{}", clone_id.trim()));
    assert!(home.join("ledger-db/.dolt").is_dir());
    assert_eq!(fx.home_of(&work), home);
    assert_eq!(git(&work, &["worktree", "list"]).lines().count(), 1);
    // The bare clone reaches origin as the code repo does.
    assert_eq!(
        git(&home.join("git"), &["config", "remote.origin.url"]),
        std::fs::canonicalize(fx.origin())
            .unwrap()
            .to_string_lossy()
    );
    // The ledger refs of a repo's own ledger did not move.
    assert!(fx.ref_exists(&fx.origin(), "refs/heads/tsk/ledger"));
}

#[test]
fn in_repo_bare_clone_takes_the_origin_url_including_a_relative_one_and_follows_a_change() {
    if !common::dolt_installed() {
        return;
    }
    let fx = Fx::current("branches", false);
    let work = fx.work();
    let home = fx.home_of(&work);
    let bare = home.join("git");
    // A relative origin URL is made absolute.
    git(&work, &["config", "remote.origin.url", "../origin.git"]);
    force_config_stamp_refresh(&bare);
    assert_success(&fx.tsk(&work, &["ledger", "status"]));
    assert_eq!(
        git(&bare, &["config", "remote.origin.url"]),
        std::fs::canonicalize(fx.origin())
            .unwrap()
            .to_string_lossy()
    );
    // A change of the origin follows.
    let other = fx.root.path().join("other.git");
    std::fs::rename(fx.origin(), &other).unwrap();
    git(
        &work,
        &["config", "remote.origin.url", other.to_str().unwrap()],
    );
    force_config_stamp_refresh(&bare);
    assert_success(&fx.tsk(&work, &["ledger", "status"]));
    assert_eq!(
        std::fs::canonicalize(git(&bare, &["config", "remote.origin.url"])).unwrap(),
        other.canonicalize().unwrap()
    );
}

/// The bare clone copies the settings again when a git configuration file changed. A test
/// that changes one within the same clock tick of the stamp drops the stamp instead.
fn force_config_stamp_refresh(bare: &Path) {
    let _ = std::fs::remove_file(bare.join("tsk-config-stamp"));
}

#[test]
fn in_repo_bare_clone_follows_a_repo_local_url_insteadof_and_credential_config() {
    if !common::dolt_installed() {
        return;
    }
    let fx = Fx::current("branches", false);
    let work = fx.work();
    let bare = fx.home_of(&work).join("git");
    let origin = fx.origin();
    git(
        &work,
        &[
            "config",
            "remote.origin.url",
            "https://example.test/acme/work-api",
        ],
    );
    git(
        &work,
        &[
            "config",
            &format!("url.{}.insteadOf", origin.display()),
            "https://example.test/acme/work-api",
        ],
    );
    git(
        &work,
        &["config", "credential.helper", "store --file=/nonexistent"],
    );
    git(
        &work,
        &["config", "http.proxy", "http://proxy.invalid:3128"],
    );
    force_config_stamp_refresh(&bare);
    assert_success(&fx.tsk(&work, &["ledger", "status"]));
    // The resolved URL, not the one the repo names; the settings are copied; `url.*` is not.
    assert_eq!(
        std::fs::canonicalize(git(&bare, &["config", "remote.origin.url"]).trim_end()).unwrap(),
        origin.canonicalize().unwrap()
    );
    assert_eq!(
        git(&bare, &["config", "credential.helper"]),
        "store --file=/nonexistent"
    );
    assert_eq!(
        git(&bare, &["config", "http.proxy"]),
        "http://proxy.invalid:3128"
    );
    let urls = git_output(&bare, &["config", "--local", "--get-regexp", "^url\\."]);
    assert!(
        !urls.status.success(),
        "{}",
        String::from_utf8_lossy(&urls.stdout)
    );
    // A setting the repo drops is dropped from the bare clone.
    git(&work, &["config", "--unset", "http.proxy"]);
    force_config_stamp_refresh(&bare);
    assert_success(&fx.tsk(&work, &["ledger", "status"]));
    assert!(!git_output(&bare, &["config", "--get", "http.proxy"])
        .status
        .success());
}

#[test]
fn a_moved_state_directory_is_healed() {
    if !common::dolt_installed() {
        return;
    }
    let fx = Fx::current("custom", true);
    let moved = fx.root.path().join("state-moved");
    common::stop_servers(fx.root.path(), true);
    std::fs::rename(fx.state(), &moved).unwrap();
    let output = fx.tsk_env(
        &fx.work(),
        &["ledger", "status"],
        &[("XDG_STATE_HOME", moved.to_str().unwrap())],
    );
    assert_success(&output);
    let docs = moved.join("tsk/ledgers/work-api/docs");
    assert_eq!(git(&docs, &["rev-parse", "--is-inside-work-tree"]), "true");
    assert!(
        stdout(&output).contains("Origin  up to date"),
        "{}",
        stdout(&output)
    );
}

#[test]
fn a_symlinked_state_root_keeps_the_store_url_consistent() {
    if !common::dolt_installed() {
        return;
    }
    let fx = Fx::legacy("custom", true, FULL);
    let link = fx.root.path().join("state-link");
    std::os::unix::fs::symlink(fx.state(), &link).unwrap();
    let work = fx.work();
    let first = fx.tsk_env(
        &work,
        &["ledger", "status"],
        &[("XDG_STATE_HOME", link.to_str().unwrap())],
    );
    assert_success(&first);
    // The code repo keeps no registration of a ledger worktree.
    let list = git(&work, &["worktree", "list"]);
    assert_eq!(list.lines().count(), 1, "{}", list);
    // The real path and the link both reach the store, with no "points at" error.
    let second = fx.tsk(&work, &["ledger", "status"]);
    assert_success(&second);
    assert!(
        !stderr(&second).contains("points at"),
        "{}",
        stderr(&second)
    );
    assert_success(&fx.tsk_env(
        &work,
        &["ledger", "push", "x"],
        &[("XDG_STATE_HOME", link.to_str().unwrap())],
    ));
}

#[test]
fn a_legacy_store_with_no_recorded_ref_is_not_moved_by_open_and_is_finished_by_fetch() {
    if !common::dolt_installed() {
        return;
    }
    let fx = Fx::legacy(
        "custom",
        true,
        Knobs {
            unpushed_dolt: false,
            dirty_docs: false,
            nexus: true,
        },
    );
    let work = fx.work();
    let clone_dir = fx.layout(&work).clone_dir();
    std::fs::remove_file(clone_dir.join("ledger-ref")).unwrap();
    let path = fx.tsk(&work, &["ledger", "path"]);
    // The key is unknown, so nothing moved yet.
    assert!(clone_dir.join("ledger-db").is_dir());
    assert!(!path.status.success() || stdout(&path).contains("ledger-db"));
    let fetch = fx.tsk(&work, &["ledger", "fetch"]);
    assert_success(&fetch);
    assert!(!clone_dir.join("ledger-db").exists());
    assert!(fx
        .state()
        .join("tsk/ledgers/work-api/ledger-db/.dolt")
        .is_dir());
}

#[test]
fn a_dangling_docs_gitdir_moves_the_directory_to_leftovers_and_finishes_the_rest() {
    if !common::dolt_installed() {
        return;
    }
    let fx = Fx::legacy(
        "custom",
        true,
        Knobs {
            unpushed_dolt: false,
            dirty_docs: false,
            nexus: true,
        },
    );
    let work = fx.work();
    // The code repo forgets the worktree: its administrative directory is gone.
    let admin = work.join(".git/worktrees");
    for entry in std::fs::read_dir(&admin).unwrap().flatten() {
        let gitdir = std::fs::read_to_string(entry.path().join("gitdir")).unwrap();
        if gitdir.contains("/docs/") {
            std::fs::remove_dir_all(entry.path()).unwrap();
        }
    }
    let output = fx.tsk(&work, &["ledger", "fetch"]);
    assert_success(&output);
    assert!(
        stderr(&output).contains("not a readable git worktree"),
        "{}",
        stderr(&output)
    );
    let leftovers = fx.layout(&work).clone_dir().join("legacy-leftovers");
    assert!(leftovers.join("docs").is_dir());
    // The rest finished: the store moved, and the documents were fetched afresh.
    assert!(fx.home_of(&work).join("ledger-db/.dolt").is_dir());
    assert!(fx.home_of(&work).join("docs").is_dir());
    assert!(fx
        .store_sql(&work, "select code from missions")
        .contains("M-A"));
}

#[test]
fn a_key_flip_with_unpushed_work_stops_and_with_clean_state_only_notes() {
    if !common::dolt_installed() {
        return;
    }
    // A repo with a ledger in the repo, then a nexus entry that says the ledger is in the nexus.
    let fx = Fx::template("flip", "branches", false, None, false);
    let work = fx.work();
    let clone_id = std::fs::read_to_string(work.join(".git/tsk-clone-id")).unwrap();
    git(
        fx.root.path(),
        &["init", "--quiet", "--bare", "-b", "main", "nexus.git"],
    );
    git(
        fx.root.path(),
        &["init", "--quiet", "-b", "main", "nexus-seed"],
    );
    git(
        &fx.nexus_seed(),
        &["remote", "add", "origin", fx.nexus().to_str().unwrap()],
    );
    fx.set_nexus_json(ENTRY);
    assert_success(&fx.tsk(
        &work,
        &["config", "attach-nexus", fx.nexus().to_str().unwrap()],
    ));
    write(&work.join(".git/tsk-repo-id"), "work-api\n");
    let old_home = fx
        .state()
        .join("tsk/ledgers")
        .join(format!("_{}", clone_id.trim()));
    // Unpushed work in the old home.
    write(&old_home.join("docs/draft.md"), "# draft\n");
    let stopped = fx.tsk(&work, &["ledger", "fetch"]);
    assert_failure_mentioning(&stopped, "unpushed ledger work");
    assert!(
        stderr(&stopped).contains("tsk nexus move-ledger"),
        "{}",
        stderr(&stopped)
    );
    // Without it, the command goes on and says where the old store is.
    std::fs::remove_file(old_home.join("docs/draft.md")).unwrap();
    let noted = fx.tsk(&work, &["ledger", "fetch"]);
    assert_success(&noted);
    assert!(
        stderr(&noted).contains("is left as it is"),
        "{}",
        stderr(&noted)
    );
}

#[test]
fn the_same_entry_id_in_two_nexus_urls_gets_two_homes() {
    if !common::dolt_installed() {
        return;
    }
    let fx = Fx::current("custom", true);
    let first = fx.home_of(&fx.work());
    // A second nexus with the same entry id.
    let second = fx.root.path().join("nexus2.git");
    git(
        fx.root.path(),
        &[
            "clone",
            "--quiet",
            "--bare",
            fx.nexus().to_str().unwrap(),
            second.to_str().unwrap(),
        ],
    );
    assert_success(&fx.tsk(
        &fx.work(),
        &["config", "attach-nexus", second.to_str().unwrap()],
    ));
    let path = fx.ok(&fx.work(), &["ledger", "fetch"]);
    let alt = fx.home_of(&fx.work());
    assert_ne!(first, alt);
    let name = alt.file_name().unwrap().to_string_lossy().into_owned();
    assert!(name.starts_with("work-api~"), "{}", name);
    assert!(
        path.trim_end().starts_with(&alt.display().to_string()),
        "{}",
        path
    );
    assert!(first.join("ledger-db").is_dir());
}

// --- B. The move of the nexus refs ---------------------------------------------------------

#[test]
fn nexus_refs_move_to_ledger_and_docs() {
    if !common::dolt_installed() {
        return;
    }
    for refs in BOTH_MODES {
        let fx = Fx::legacy(refs, true, FULL);
        let p = prefix(refs);
        let (old_l, old_d) = (
            format!("{}ledgers/work-api", p),
            format!("{}ledgers/work-api-docs", p),
        );
        let before_l = git(&fx.nexus(), &["rev-parse", &old_l]);
        assert!(fx.entry_marker().is_none());
        assert_success(&fx.tsk(&fx.work(), &["ledger", "status"]));
        let names = fx.refs_of(&fx.nexus());
        assert!(
            names.contains(&format!("{}ledgers/work-api/ledger", p)),
            "{:?}",
            names
        );
        assert!(
            names.contains(&format!("{}ledgers/work-api/docs", p)),
            "{:?}",
            names
        );
        assert!(
            !names.contains(&old_l) && !names.contains(&old_d),
            "{:?}",
            names
        );
        assert!(names.iter().all(|n| !n.contains("backup")), "{:?}", names);
        // The commit moved, with its history.
        assert_eq!(
            git(
                &fx.nexus(),
                &["rev-parse", &format!("{}ledgers/work-api/ledger", p)]
            ),
            before_l
        );
        // The entry is marked at the new layout, in the mode of the refs.
        let marker = fx.entry_marker().expect("the entry is marked");
        assert!(
            marker.starts_with(&format!(
                "{}-v2\nerror: unsupported ledger version 16 in ",
                refs
            )),
            "{}",
            marker
        );
        // The code repo's origin is untouched.
        assert!(!fx.ref_exists(&fx.origin(), "refs/heads/tsk/ledger"));
    }
}

#[test]
fn backups_are_pushed_first_then_one_combined_push_then_backups_deleted() {
    if !common::dolt_installed() {
        return;
    }
    let fx = Fx::legacy("custom", true, FULL);
    let log = fx.root.path().join("push.log");
    common::install_git_shim(
        fx.root.path(),
        &format!(
            "case \"$*\" in *push*ledgers*) echo \"$*\" >> '{}';; esac",
            log.display()
        ),
    );
    assert_success(&fx.tsk(&fx.work(), &["ledger", "status"]));
    let pushes: Vec<String> = std::fs::read_to_string(&log)
        .unwrap()
        .lines()
        .map(str::to_string)
        .collect();
    let backup_first = pushes
        .iter()
        .position(|l| {
            l.contains(":refs/tsk/ledgers-backup/work-api/ledger")
                && !l.contains(" :refs/tsk/ledgers/")
        })
        .expect("a backup push");
    let combined = pushes
        .iter()
        .position(|l| {
            l.contains("--force-with-lease=refs/tsk/ledgers/work-api:")
                && l.contains(":refs/tsk/ledgers/work-api/ledger")
        })
        .expect("a combined push");
    let deleted = pushes
        .iter()
        .rposition(|l| {
            l.contains(":refs/tsk/ledgers-backup/work-api/ledger") && l.contains("force-with-lease")
        })
        .expect("a backup delete");
    assert!(
        backup_first < combined && combined < deleted,
        "{:#?}",
        pushes
    );
}

#[test]
fn a_stale_lease_changes_nothing() {
    if !common::dolt_installed() {
        return;
    }
    let fx = Fx::legacy("custom", true, FULL);
    let other = git(&fx.nexus(), &["rev-parse", "refs/heads/main"]);
    // Another writer moves the old ref between the move's read and its push.
    common::install_git_shim(
        fx.root.path(),
        &format!(
            "case \"$*\" in *--force-with-lease=refs/tsk/ledgers/work-api:*) env -u GIT_DIR \"$REAL_GIT\" -C '{}' update-ref refs/tsk/ledgers/work-api {};; esac",
            fx.nexus().display(),
            other
        ),
    );
    let output = fx.tsk(&fx.work(), &["ledger", "status"]);
    assert_success(&output);
    assert!(
        stderr(&output).contains("changed while the move ran"),
        "{}",
        stderr(&output)
    );
    let names = fx.refs_of(&fx.nexus());
    assert!(
        !names.contains(&"refs/tsk/ledgers/work-api/ledger".to_string()),
        "{:?}",
        names
    );
    assert_eq!(
        git(&fx.nexus(), &["rev-parse", "refs/tsk/ledgers/work-api"]),
        other
    );
    assert!(fx.entry_marker().is_none());
}

#[test]
fn a_second_machine_after_the_move_just_proceeds() {
    if !common::dolt_installed() {
        return;
    }
    let fx = Fx::legacy(
        "custom",
        true,
        Knobs {
            unpushed_dolt: false,
            dirty_docs: false,
            nexus: true,
        },
    );
    assert_success(&fx.tsk(&fx.work(), &["ledger", "status"]));
    let before = fx.refs_of(&fx.nexus());
    let output = fx.tsk(&fx.work2(), &["ledger", "fetch"]);
    assert_success(&output);
    assert!(!stderr(&output).contains("move"), "{}", stderr(&output));
    assert_eq!(fx.refs_of(&fx.nexus()), before);
}

#[test]
fn two_machines_moving_at_once_converge() {
    if !common::dolt_installed() {
        return;
    }
    let fx = Fx::template("legacy-second", "custom", true, Some(FULL), true);
    let (a, b) = (fx.work(), fx.work2());
    let first = fx.command(&a, &["ledger", "status"]).spawn_output();
    let second = fx.command(&b, &["ledger", "status"]).spawn_output();
    let (x, y) = (first(), second());
    assert_success(&x);
    assert_success(&y);
    let names = fx.refs_of(&fx.nexus());
    assert!(
        names.contains(&"refs/tsk/ledgers/work-api/ledger".to_string()),
        "{:?}",
        names
    );
    assert!(
        names
            .iter()
            .all(|n| !n.contains("backup") && n != "refs/tsk/ledgers/work-api"),
        "{:?}",
        names
    );
}

fn hook(fx: &Fx, body: &str) {
    use std::os::unix::fs::PermissionsExt;
    let file = fx.nexus().join("hooks/pre-receive");
    write(&file, &format!("#!/bin/sh\n{}\n", body));
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn a_server_that_rejects_the_combined_push_uses_the_delete_then_create_fallback() {
    if !common::dolt_installed() {
        return;
    }
    let fx = Fx::legacy("custom", true, FULL);
    // A push that changes the old ref and the new one together is refused.
    hook(
        &fx,
        "n=0; while read old new ref; do case \"$ref\" in refs/tsk/ledgers/work-api|refs/tsk/ledgers/work-api/ledger) n=$((n+1));; esac; done; [ \"$n\" -lt 2 ] || { echo 'refusing the combined push' >&2; exit 1; }",
    );
    assert_success(&fx.tsk(&fx.work(), &["ledger", "status"]));
    let names = fx.refs_of(&fx.nexus());
    assert!(
        names.contains(&"refs/tsk/ledgers/work-api/ledger".to_string()),
        "{:?}",
        names
    );
    assert!(
        !names.contains(&"refs/tsk/ledgers/work-api".to_string()),
        "{:?}",
        names
    );
    assert!(names.iter().all(|n| !n.contains("backup")), "{:?}", names);
}

#[test]
fn a_crash_between_delete_and_create_resumes_from_the_backup_and_never_starts_a_fresh_ledger() {
    if !common::dolt_installed() {
        return;
    }
    let fx = Fx::legacy("custom", true, FULL);
    // The state after a move that stopped between its delete and its create.
    let tip = git(&fx.nexus(), &["rev-parse", "refs/tsk/ledgers/work-api"]);
    git(
        &fx.nexus(),
        &[
            "update-ref",
            "refs/tsk/ledgers-backup/work-api/ledger",
            &tip,
        ],
    );
    git(
        &fx.nexus(),
        &["update-ref", "-d", "refs/tsk/ledgers/work-api"],
    );
    assert_success(&fx.tsk(&fx.work(), &["ledger", "status"]));
    assert_eq!(
        git(
            &fx.nexus(),
            &["rev-parse", "refs/tsk/ledgers/work-api/ledger"]
        ),
        tip
    );
    let names = fx.refs_of(&fx.nexus());
    assert!(names.iter().all(|n| !n.contains("backup")), "{:?}", names);
}

#[test]
fn an_old_name_recreated_between_delete_and_create_stops_with_both_tips_and_moves_nothing() {
    if !common::dolt_installed() {
        return;
    }
    let fx = Fx::legacy("custom", true, FULL);
    let real = git(&fx.nexus(), &["rev-parse", "refs/tsk/ledgers/work-api"]);
    let empty = git(&fx.nexus(), &["rev-parse", "refs/heads/main"]);
    git(
        &fx.nexus(),
        &[
            "update-ref",
            "refs/tsk/ledgers-backup/work-api/ledger",
            &real,
        ],
    );
    git(
        &fx.nexus(),
        &["update-ref", "refs/tsk/ledgers/work-api", &empty],
    );
    let output = fx.tsk(&fx.work(), &["ledger", "status"]);
    assert_failure_mentioning(&output, "differ");
    assert!(
        stderr(&output).contains(&real) && stderr(&output).contains(&empty),
        "{}",
        stderr(&output)
    );
    let names = fx.refs_of(&fx.nexus());
    assert!(
        !names.contains(&"refs/tsk/ledgers/work-api/ledger".to_string()),
        "{:?}",
        names
    );
    assert_eq!(
        git(
            &fx.nexus(),
            &["rev-parse", "refs/tsk/ledgers-backup/work-api/ledger"]
        ),
        real
    );
}

#[test]
fn a_refused_delete_keeps_the_ledger_usable_on_the_legacy_names_and_prints_one_note() {
    if !common::dolt_installed() {
        return;
    }
    let fx = Fx::legacy("custom", true, FULL);
    hook(
        &fx,
        "zero=0000000000000000000000000000000000000000; while read old new ref; do [ \"$new\" = \"$zero\" ] && case \"$ref\" in refs/tsk/ledgers/work-api|refs/tsk/ledgers/work-api-docs) echo 'deletes are not allowed' >&2; exit 1;; esac; done; exit 0",
    );
    let output = fx.tsk(&fx.work(), &["ledger", "status"]);
    assert_success(&output);
    let notes = stderr(&output);
    assert_eq!(
        notes.matches("stays on the old ref names").count(),
        1,
        "{}",
        notes
    );
    assert!(
        stdout(&output).contains("the nexus refs are not at the new names yet"),
        "{}",
        stdout(&output)
    );
    // Nothing moved or was lost; the ledger works on the old names.
    let names = fx.refs_of(&fx.nexus());
    assert!(
        names.contains(&"refs/tsk/ledgers/work-api".to_string()),
        "{:?}",
        names
    );
    assert!(
        !names.contains(&"refs/tsk/ledgers/work-api/ledger".to_string()),
        "{:?}",
        names
    );
    assert!(
        stdout(&output).contains("data refs/tsk/ledgers/work-api,"),
        "{}",
        stdout(&output)
    );
    assert_success(&fx.tsk(&fx.work(), &["ledger", "push", "On the old names"]));
    assert_ne!(
        git(&fx.nexus(), &["rev-parse", "refs/tsk/ledgers/work-api"]),
        git(&fx.nexus(), &["rev-parse", "refs/heads/main"])
    );
    // A second command within the hour does not try again.
    let again = fx.tsk(&fx.work(), &["ledger", "status"]);
    assert_success(&again);
    assert!(
        stderr(&again).contains("tried again at the next tsk ledger push"),
        "{}",
        stderr(&again)
    );
}

#[test]
fn a_backup_that_the_old_ref_has_moved_past_is_advanced_and_pushes_keep_working() {
    if !common::dolt_installed() {
        return;
    }
    let fx = Fx::legacy("custom", true, FULL);
    // A server that refuses every delete, so the backup survives next to the old ref.
    hook(
        &fx,
        "zero=0000000000000000000000000000000000000000; while read old new ref; do [ \"$new\" = \"$zero\" ] && echo 'deletes are not allowed' >&2 && exit 1; done; exit 0",
    );
    let work = fx.work();
    assert_success(&fx.tsk(&work, &["ledger", "status"]));
    let backup = "refs/tsk/ledgers-backup/work-api/ledger";
    assert!(
        fx.ref_exists(&fx.nexus(), backup),
        "{:?}",
        fx.refs_of(&fx.nexus())
    );
    for n in 0..3 {
        fx.ok(&work, &["mission", "create", &format!("M-S{}", n), "Stale"]);
        assert_success(&fx.tsk(&work, &["ledger", "push", "On the old names"]));
        // The backup follows the old ref; nothing forks and no command fails.
        assert_success(&fx.tsk(&work, &["ledger", "status"]));
        let (held, tip) = (
            git(&fx.nexus(), &["rev-parse", backup]),
            git(&fx.nexus(), &["rev-parse", "refs/tsk/ledgers/work-api"]),
        );
        let behind = git_output(&fx.nexus(), &["merge-base", "--is-ancestor", &held, &tip]);
        assert!(behind.status.success(), "round {}: {} vs {}", n, held, tip);
    }
}

#[test]
fn an_unreachable_remote_uses_the_recorded_layout_and_does_not_write() {
    if !common::dolt_installed() {
        return;
    }
    let fx = Fx::legacy("custom", true, FULL);
    let moved = fx.root.path().join("nexus-away.git");
    std::fs::rename(fx.nexus(), &moved).unwrap();
    let output = fx.tsk(&fx.work(), &["ledger", "status", "--local"]);
    assert_success(&output);
    assert!(
        stdout(&output).contains("data refs/tsk/ledgers/work-api,"),
        "{}",
        stdout(&output)
    );
    std::fs::rename(&moved, fx.nexus()).unwrap();
    assert!(fx
        .refs_of(&fx.nexus())
        .contains(&"refs/tsk/ledgers/work-api".to_string()));
}

#[test]
fn a_ledger_found_in_another_mode_returns_the_mismatch_error_before_any_write() {
    if !common::dolt_installed() {
        return;
    }
    let fx = Fx::legacy("custom", true, FULL);
    let before = fx.refs_of(&fx.nexus());
    let output = fx.tsk_env(
        &fx.work(),
        &["ledger", "fetch"],
        &[("TSK_LEDGER_REFS", "branches")],
    );
    assert_failure_mentioning(
        &output,
        "tsk does not move a ledger between branches and a custom ref",
    );
    assert_eq!(fx.refs_of(&fx.nexus()), before);
}

#[test]
fn no_ledger_yet_writes_no_marker_and_read_only_commands_never_write_nexus_json() {
    if !common::dolt_installed() {
        return;
    }
    let fx = Fx::new("custom", true);
    let before = git(&fx.nexus(), &["rev-parse", "refs/heads/main"]);
    fx.tsk(&fx.work(), &["ledger", "status"]);
    fx.tsk(&fx.work(), &["ledger", "fetch"]);
    assert_eq!(git(&fx.nexus(), &["rev-parse", "refs/heads/main"]), before);
    assert!(fx.entry_marker().is_none());
    // The first push creates the ledger at the new names, and marks the entry.
    fx.ok(&fx.work(), &["mission", "create", "M-A", "Alpha"]);
    fx.ok(&fx.work(), &["ledger", "push", "First"]);
    assert!(fx
        .refs_of(&fx.nexus())
        .contains(&"refs/tsk/ledgers/work-api/ledger".to_string()));
    assert!(fx.entry_marker().is_some());
}

#[test]
fn the_marker_is_set_after_the_move_and_retried_at_the_next_publish_after_a_failed_nexus_push() {
    if !common::dolt_installed() {
        return;
    }
    let fx = Fx::legacy("custom", true, FULL);
    // The nexus refuses a push to its default branch, but takes the ledger refs.
    hook(
        &fx,
        "while read old new ref; do case \"$ref\" in refs/heads/main) echo 'main is protected' >&2; exit 1;; esac; done; exit 0",
    );
    let output = fx.tsk(&fx.work(), &["ledger", "status"]);
    assert_success(&output);
    assert!(
        stderr(&output).contains("could not mark the nexus entry"),
        "{}",
        stderr(&output)
    );
    assert!(fx.entry_marker().is_none());
    assert!(fx
        .refs_of(&fx.nexus())
        .contains(&"refs/tsk/ledgers/work-api/ledger".to_string()));
    // The next publish tries again, and the nexus takes it now.
    std::fs::remove_file(fx.nexus().join("hooks/pre-receive")).unwrap();
    assert_success(&fx.tsk(&fx.work(), &["ledger", "push", "Publish"]));
    assert!(fx.entry_marker().is_some());
}

#[test]
fn a_stray_old_docs_ref_is_deleted_when_contained_and_merged_into_the_new_docs_when_not() {
    if !common::dolt_installed() {
        return;
    }
    let fx = Fx::legacy(
        "custom",
        true,
        Knobs {
            unpushed_dolt: false,
            dirty_docs: false,
            nexus: true,
        },
    );
    assert_success(&fx.tsk(&fx.work(), &["ledger", "status"]));
    let new_docs = "refs/tsk/ledgers/work-api/docs";
    let old_docs = "refs/tsk/ledgers/work-api-docs";
    // An older tsk recreated the old name with a commit the new one holds.
    let tip = git(&fx.nexus(), &["rev-parse", new_docs]);
    git(&fx.nexus(), &["update-ref", old_docs, &tip]);
    assert_success(&fx.tsk(&fx.work(), &["ledger", "fetch"]));
    assert!(!fx.ref_exists(&fx.nexus(), old_docs));
    // Then with a commit of its own: it is merged in.
    let scratch = fx.root.path().join("stray");
    git(
        fx.root.path(),
        &[
            "clone",
            "--quiet",
            "--no-checkout",
            fx.nexus().to_str().unwrap(),
            scratch.to_str().unwrap(),
        ],
    );
    git(
        &scratch,
        &[
            "fetch",
            "--quiet",
            "origin",
            &format!("{}:refs/stray", new_docs),
        ],
    );
    git(&scratch, &["checkout", "--quiet", "--detach", "refs/stray"]);
    write(&scratch.join("from-the-old-tsk.md"), "# old\n");
    commit_all(&scratch, "A document from an older tsk");
    git(
        &scratch,
        &["push", "--quiet", "origin", &format!("HEAD:{}", old_docs)],
    );
    assert_success(&fx.tsk(&fx.work(), &["ledger", "fetch"]));
    assert!(!fx.ref_exists(&fx.nexus(), old_docs));
    let files = git(&fx.nexus(), &["ls-tree", "-r", "--name-only", new_docs]);
    assert!(files.contains("from-the-old-tsk.md"), "{}", files);
}

// --- C. Older binaries ---------------------------------------------------------------------

#[test]
fn an_old_style_create_of_the_old_ref_name_is_refused_by_the_server_and_ls_remote_finds_nothing() {
    for p in ["refs/tsk/", "refs/heads/"] {
        let root = tempfile::tempdir().unwrap();
        git(
            root.path(),
            &["init", "--quiet", "--bare", "-b", "main", "server.git"],
        );
        git(root.path(), &["init", "--quiet", "-b", "main", "client"]);
        let client = root.path().join("client");
        write(&client.join("f"), "x\n");
        commit_all(&client, "first");
        let server = root.path().join("server.git");
        // The new names exist, as after the move.
        git(
            &client,
            &[
                "push",
                "--quiet",
                server.to_str().unwrap(),
                &format!("HEAD:{}ledgers/x/ledger", p),
                &format!("HEAD:{}ledgers/x/docs", p),
            ],
        );
        // What an older tsk does when it finds no ledger: create the old name.
        let refused = git_output(
            &client,
            &[
                "push",
                "--quiet",
                server.to_str().unwrap(),
                &format!("HEAD:{}ledgers/x", p),
            ],
        );
        assert!(!refused.status.success(), "the server took {}ledgers/x", p);
        assert!(
            String::from_utf8_lossy(&refused.stderr).contains("cannot lock ref")
                || String::from_utf8_lossy(&refused.stderr).contains("failed to update ref"),
            "{}",
            String::from_utf8_lossy(&refused.stderr)
        );
        // And it does not see the ledger under the name it asks for.
        let asked = git_output(
            &client,
            &[
                "ls-remote",
                "--exit-code",
                server.to_str().unwrap(),
                &format!("{}ledgers/x", p),
            ],
        );
        assert_eq!(asked.status.code(), Some(2));
    }
}

#[test]
fn the_new_binary_never_creates_a_ledger_while_a_legacy_or_backup_ref_exists() {
    if !common::dolt_installed() {
        return;
    }
    let fx = Fx::legacy("custom", true, FULL);
    let real = git(&fx.nexus(), &["rev-parse", "refs/tsk/ledgers/work-api"]);
    // A marked entry whose data ref is gone, with only a backup left. The documents are still
    // under their old name.
    git(
        &fx.nexus(),
        &[
            "update-ref",
            "refs/tsk/ledgers-backup/work-api/ledger",
            &real,
        ],
    );
    git(
        &fx.nexus(),
        &["update-ref", "-d", "refs/tsk/ledgers/work-api"],
    );
    let marker = "custom-v2\\nerror: unsupported ledger version 16 in the nexus entry (ledger ref layout 2); this tsk supports up to ledger version 15. Update tsk";
    fx.set_nexus_json(&format!(
        r#"{{"id":"work-api","local":"laptop","ledger":"nexus","prefix":"WRK","ledger_refs":"{}"}}"#,
        marker
    ));
    // The nexus takes no new ref, so the move cannot finish. The fetch does not make a new
    // ledger in its place.
    hook(
        &fx,
        "zero=0000000000000000000000000000000000000000; while read old new ref; do [ \"$old\" = \"$zero\" ] && case \"$ref\" in refs/tsk/ledgers/work-api/ledger) echo 'no new refs' >&2; exit 1;; esac; done; exit 0",
    );
    let output = fx.tsk(&fx.work2(), &["ledger", "fetch"]);
    assert_failure_mentioning(&output, "has not finished");
    assert!(!fx.state().join("tsk/ledgers/work-api/ledger-db").exists());
    let names = fx.refs_of(&fx.nexus());
    assert!(
        !names.contains(&"refs/tsk/ledgers/work-api/ledger".to_string()),
        "{:?}",
        names
    );
    // With the nexus open again, the fetch finishes the move from the backup: the ledger it
    // makes is the one that was there, not a new one.
    std::fs::remove_file(fx.nexus().join("hooks/pre-receive")).unwrap();
    std::fs::remove_file(fx.state().join("tsk/ledgers/work-api/remote-move.last")).ok();
    assert_success(&fx.tsk(&fx.work2(), &["ledger", "fetch"]));
    assert_eq!(
        git(
            &fx.nexus(),
            &["rev-parse", "refs/tsk/ledgers/work-api/ledger"]
        ),
        real
    );
    assert!(fx.ok(&fx.work2(), &["mission", "list"]).contains("M-A"));
}

/// A real older binary, named by `TSK_OLD_BIN`, stops at the marker and updates itself. CI
/// builds the last release and sets `TSK_REQUIRE_OLD_BIN=1`, which turns a missing binary into
/// a failure.
#[test]
fn an_old_binary_stops_at_the_marker_and_updates() {
    let Some(old) = std::env::var_os("TSK_OLD_BIN").map(PathBuf::from) else {
        assert!(
            std::env::var_os("TSK_REQUIRE_OLD_BIN").is_none(),
            "TSK_REQUIRE_OLD_BIN is set and TSK_OLD_BIN is not"
        );
        eprintln!("note: TSK_OLD_BIN is not set; skipping");
        return;
    };
    if !common::dolt_installed() {
        return;
    }
    let fx = Fx::current("custom", true);
    assert!(fx.entry_marker().is_some());
    let store_before = std::fs::read_dir(fx.tsk_dir().join("repos"))
        .map(|d| d.count())
        .unwrap_or(0);
    let update_log = fx.root.path().join("update.log");
    let stub = fx.root.path().join("update.sh");
    write(
        &stub,
        &format!(
            "#!/bin/sh\necho \"updated $1\" >> '{}'\nexit 1\n",
            update_log.display()
        ),
    );
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    // Run in a clone that has no store, as another machine would.
    let mut command = common::cmd(&old);
    command
        .args(["ledger", "fetch"])
        .current_dir(fx.work2())
        .env("HOME", fx.user_home())
        .env("XDG_STATE_HOME", fx.state())
        .env("XDG_CONFIG_HOME", fx.root.path().join("config"))
        .env("TSK_HOME", fx.user_home().join(".tsk"))
        .env("TSK_MACHINE_NAME", "laptop")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("TSK_TEST_MODE", "1")
        .env("TSK_SELF_UPDATE_CMD", &stub)
        .env_remove("TSK_AUTO_UPDATE")
        .env_remove("TSK_NO_UPDATE_CHECK")
        .env_remove("TSK_LEDGER_REFS");
    let output = command.output().unwrap();
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("unsupported ledger version 16"),
        "{}",
        stderr(&output)
    );
    assert!(update_log.exists(), "the old binary did not run its update");
    // It created no store, and pushed nothing.
    let store_after = std::fs::read_dir(fx.tsk_dir().join("repos"))
        .map(|d| d.count())
        .unwrap_or(0);
    assert_eq!(store_before, store_after);
    assert!(!fx.ref_exists(&fx.nexus(), "refs/tsk/ledgers/work-api"));
}
