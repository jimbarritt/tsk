//! Concurrent writers against one ledger (M-TSK-LOCAL T-07). The ledger is in a local bare
//! nexus. Two clones of one repo each hold a ledger store, and many `tsk` processes write
//! and push at the same time, in one clone and in both. Every write that exits 0 must be in
//! both clones afterwards, with its own task number, and no write may fail on a Dolt
//! serialization failure. Cases print a note and return when `dolt` is not installed.

use std::path::{Path, PathBuf};
use std::process::Output;
use std::thread;

use tempfile::TempDir;

use crate::common;

const ORIGIN_URL: &str = "https://example.test/acme/work-api";
const MISSION: &str = "M-TEST-ONE";

struct Fixture {
    root: TempDir,
}

impl Drop for Fixture {
    /// Stop the Dolt servers that the commands started (ADR 0018) before the files go.
    fn drop(&mut self) {
        common::stop_servers(self.root.path(), false);
    }
}

fn git_env() -> Vec<(&'static str, &'static str)> {
    vec![
        ("GIT_CONFIG_GLOBAL", "/dev/null"),
        ("GIT_CONFIG_NOSYSTEM", "1"),
        ("GIT_AUTHOR_NAME", "Test"),
        ("GIT_AUTHOR_EMAIL", "test@example.com"),
        ("GIT_COMMITTER_NAME", "Test"),
        ("GIT_COMMITTER_EMAIL", "test@example.com"),
        ("TSK_LEDGER_REFS", "branches"),
    ]
}

fn git(dir: &Path, args: &[&str]) -> String {
    let output = common::cmd("git")
        .args(args)
        .current_dir(dir)
        .envs(git_env())
        .output()
        .expect("failed to run git");
    assert!(
        output.status.success(),
        "git {:?} failed in {}: {}",
        args,
        dir.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
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

impl Fixture {
    /// A local bare nexus that holds the ledger of `work-api`, and two clones of the repo,
    /// `work` and `work2`, each with the ledger fetched and one mission in it.
    fn new() -> Fixture {
        let fixture = Fixture {
            root: tempfile::tempdir().unwrap(),
        };
        let root = fixture.root.path();
        for dir in ["home", "state", "config"] {
            std::fs::create_dir_all(root.join(dir)).unwrap();
        }
        common::seed_dolt_home(&root.join("home"));

        git(
            root,
            &["init", "--quiet", "--bare", "-b", "main", "code.git"],
        );
        git(root, &["init", "--quiet", "-b", "main", "code-seed"]);
        let seed = root.join("code-seed");
        std::fs::write(seed.join("README.md"), "main\n").unwrap();
        git(&seed, &["add", "-A"]);
        git(&seed, &["commit", "--quiet", "-m", "Main commit"]);
        let code = root.join("code.git");
        git(
            &seed,
            &[
                "push",
                "--quiet",
                code.to_str().unwrap(),
                "HEAD:refs/heads/main",
            ],
        );
        for name in ["work", "work2"] {
            git(root, &["clone", "--quiet", code.to_str().unwrap(), name]);
            let clone = root.join(name);
            git(&clone, &["config", "remote.origin.url", ORIGIN_URL]);
            git(
                &clone,
                &[
                    "config",
                    &format!("url.{}.insteadOf", code.display()),
                    ORIGIN_URL,
                ],
            );
        }

        git(
            root,
            &["init", "--quiet", "--bare", "-b", "main", "nexus.git"],
        );
        git(root, &["init", "--quiet", "-b", "main", "nexus-seed"]);
        let nexus_seed = root.join("nexus-seed");
        std::fs::write(
            nexus_seed.join("nexus.json"),
            format!(
                "{{\"version\":1,\"territories\":[{{\"id\":\"t\",\"name\":\"T\",\"repos\":[{{\"id\":\"work-api\",\"url\":\"{}\",\"ledger\":\"nexus\"}}]}}]}}\n",
                ORIGIN_URL
            ),
        )
        .unwrap();
        git(&nexus_seed, &["add", "-A"]);
        git(&nexus_seed, &["commit", "--quiet", "-m", "Set nexus.json"]);
        let nexus = fixture.nexus();
        git(
            &nexus_seed,
            &[
                "push",
                "--quiet",
                nexus.to_str().unwrap(),
                "HEAD:refs/heads/main",
            ],
        );

        let nexus_url = format!("file://{}", nexus.display());
        assert_success(&fixture.tsk(&fixture.a(), &["config", "attach-nexus", &nexus_url]));
        assert_success(&fixture.tsk(&fixture.a(), &["ledger", "fetch"]));
        assert_success(&fixture.tsk(&fixture.a(), &["mission", "create", MISSION, "One"]));
        assert_success(&fixture.tsk(&fixture.a(), &["ledger", "push", "Add the mission"]));
        assert_success(&fixture.tsk(&fixture.b(), &["ledger", "fetch"]));
        fixture
    }

    fn nexus(&self) -> PathBuf {
        self.root.path().join("nexus.git")
    }

    fn a(&self) -> PathBuf {
        self.root.path().join("work")
    }

    fn b(&self) -> PathBuf {
        self.root.path().join("work2")
    }

    fn tsk(&self, dir: &Path, args: &[&str]) -> Output {
        let root = self.root.path();
        common::cmd(env!("CARGO_BIN_EXE_tsk"))
            .args(args)
            .current_dir(dir)
            .envs(git_env())
            .env("HOME", root.join("home"))
            .env("XDG_STATE_HOME", root.join("state"))
            .env("XDG_CONFIG_HOME", root.join("config"))
            .env("TSK_HOME", root.join("home/.tsk"))
            .env("TSK_DOLT_SERVER_IDLE_SECS", "3")
            .env("TSK_BACKGROUND_PUBLISH", "off")
            .env("TSK_NO_UPDATE_CHECK", "1")
            .env("TSK_AUTO_UPDATE", "never")
            .env("TSK_MACHINE_NAME", "laptop")
            .env("GIT_CEILING_DIRECTORIES", root)
            .env_remove("CLAUDE_CODE_REMOTE_SESSION_ID")
            .output()
            .expect("failed to run tsk")
    }

    /// Run every job at once, each in its own thread, and return the outputs in order.
    fn all_at_once(&self, jobs: &[(PathBuf, Vec<String>)]) -> Vec<Output> {
        thread::scope(|scope| {
            let handles: Vec<_> = jobs
                .iter()
                .map(|(dir, args)| {
                    scope.spawn(move || {
                        let mut outputs = Vec::new();
                        for step in args.split(|a| a == ";") {
                            let step: Vec<&str> = step.iter().map(String::as_str).collect();
                            outputs.push(self.tsk(dir, &step));
                        }
                        outputs
                    })
                })
                .collect();
            handles
                .into_iter()
                .flat_map(|h| h.join().unwrap())
                .collect()
        })
    }

    /// The task list of the mission in a clone: `(id, description)` for each task. The
    /// porcelain columns are id, status, blocked-by, description and external id.
    fn tasks(&self, dir: &Path) -> Vec<(String, String)> {
        let output = self.tsk(dir, &["task", "list", MISSION, "--porcelain"]);
        assert_success(&output);
        stdout(&output)
            .lines()
            .filter(|line| !line.is_empty())
            .map(|line| {
                let fields: Vec<&str> = line.split('\t').collect();
                (fields[0].to_string(), fields[3].to_string())
            })
            .collect()
    }
}

fn job(dir: &Path, steps: &[&[&str]]) -> (PathBuf, Vec<String>) {
    let mut args = Vec::new();
    for (i, step) in steps.iter().enumerate() {
        if i > 0 {
            args.push(";".to_string());
        }
        args.extend(step.iter().map(|s| s.to_string()));
    }
    (dir.to_path_buf(), args)
}

/// Check that each output exited 0 and printed no serialization failure.
fn assert_all_succeeded(outputs: &[Output]) {
    for output in outputs {
        assert_success(output);
        assert!(
            !stderr(output).contains("serialization failure"),
            "a write met a serialization failure: {}",
            stderr(output)
        );
    }
}

/// Check that `tasks` holds each description exactly once, with distinct task IDs.
fn assert_holds_each_once(tasks: &[(String, String)], descriptions: &[String]) {
    for description in descriptions {
        let count = tasks.iter().filter(|(_, d)| d == description).count();
        assert_eq!(
            count, 1,
            "{:?} appears {} times in {:?}",
            description, count, tasks
        );
    }
    let mut ids: Vec<&String> = tasks.iter().map(|(id, _)| id).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), tasks.len(), "duplicate task IDs in {:?}", tasks);
}

#[test]
fn concurrent_task_adds_in_one_clone_all_land_with_distinct_numbers() {
    if !common::dolt_installed() {
        eprintln!("note: dolt is not installed; skipping");
        return;
    }
    let fx = Fixture::new();
    let descriptions: Vec<String> = (0..8).map(|n| format!("one clone {}", n)).collect();
    let jobs: Vec<_> = descriptions
        .iter()
        .map(|d| job(&fx.a(), &[&["task", "add", MISSION, d]]))
        .collect();
    assert_all_succeeded(&fx.all_at_once(&jobs));

    let tasks = fx.tasks(&fx.a());
    assert_eq!(tasks.len(), descriptions.len(), "{:?}", tasks);
    assert_holds_each_once(&tasks, &descriptions);
}

#[test]
fn concurrent_writers_in_two_clones_against_a_local_bare_nexus_lose_nothing() {
    if !common::dolt_installed() {
        eprintln!("note: dolt is not installed; skipping");
        return;
    }
    let fx = Fixture::new();
    let mut descriptions = Vec::new();
    let mut jobs = Vec::new();
    for (name, dir) in [("a", fx.a()), ("b", fx.b())] {
        for n in 0..5 {
            let description = format!("clone {} writer {}", name, n);
            jobs.push(job(
                &dir,
                &[
                    &["task", "add", MISSION, &description],
                    &["ledger", "push", &format!("Add {}", description)],
                ],
            ));
            descriptions.push(description);
        }
    }
    assert_all_succeeded(&fx.all_at_once(&jobs));

    // Every push exited 0, so the nexus holds every write. A fetch brings each clone to it.
    for dir in [fx.a(), fx.b()] {
        assert_success(&fx.tsk(&dir, &["ledger", "fetch"]));
    }
    let a = fx.tasks(&fx.a());
    let b = fx.tasks(&fx.b());
    assert_eq!(a.len(), descriptions.len(), "{:?}", a);
    assert_holds_each_once(&a, &descriptions);
    assert_eq!(a, b);
}

#[test]
fn a_concurrent_fetch_does_not_reset_over_a_write_in_the_same_clone() {
    if !common::dolt_installed() {
        eprintln!("note: dolt is not installed; skipping");
        return;
    }
    let fx = Fixture::new();
    // Clone B publishes, so clone A is behind and each fetch in A resets its store to the
    // nexus when it finds no local change.
    assert_success(&fx.tsk(&fx.b(), &["task", "add", MISSION, "from b"]));
    assert_success(&fx.tsk(&fx.b(), &["ledger", "push", "Add from b"]));

    let descriptions: Vec<String> = (0..6).map(|n| format!("beside a fetch {}", n)).collect();
    let mut jobs: Vec<_> = descriptions
        .iter()
        .map(|d| job(&fx.a(), &[&["task", "add", MISSION, d]]))
        .collect();
    for _ in 0..6 {
        jobs.push(job(&fx.a(), &[&["ledger", "fetch"]]));
    }
    let outputs = fx.all_at_once(&jobs);
    // A fetch that finds the local changes leaves the store as it is and says so; only the
    // task adds must succeed.
    assert_all_succeeded(&outputs[..descriptions.len()]);

    assert_success(&fx.tsk(&fx.a(), &["ledger", "push", "Publish the adds"]));
    let tasks = fx.tasks(&fx.a());
    let mut expected = descriptions.clone();
    expected.push("from b".to_string());
    assert_eq!(tasks.len(), expected.len(), "{:?}", tasks);
    assert_holds_each_once(&tasks, &expected);
}
