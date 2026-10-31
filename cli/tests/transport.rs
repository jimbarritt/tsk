//! The ledger transport, end to end, with a bare git repo as origin and the `dolt` binary
//! (ADR 0017). A new ledger holds a Dolt file remote on its ref and moves with plain git.
//! The older layout, `git-remote`, stays as a fallback and converts with
//! `tsk ledger migrate-transport`. Cases print a note and return when `dolt` is missing.

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use tempfile::TempDir;

use crate::common;

use common::dolt_installed;

use common::templates::{self, Seed, Step};

const PUBLISHED: Seed = Seed {
    branch: "main",
    clones: &["a", "b", "c"],
    refs: Some("branches"),
    steps: &[
        Step::Tsk("a", &["ledger", "fetch"]),
        Step::Tsk("a", &["mission", "create", "M-ONE", "One"]),
        Step::Tsk("a", &["ledger", "push", "Create the ledger"]),
    ],
};

const LEDGER_REF: &str = "refs/heads/tsk/ledger";
const DOCS_REF: &str = "refs/heads/tsk/docs";

macro_rules! need_dolt {
    () => {
        if !dolt_installed() {
            eprintln!("skipped: the dolt binary is not installed");
            return;
        }
    };
}

struct Fixture {
    root: TempDir,
}

impl Drop for Fixture {
    /// Stop the Dolt servers that the commands started (ADR 0018) before the files go.
    fn drop(&mut self) {
        common::stop_servers(self.root.path(), false);
    }
}

impl Fixture {
    /// A bare origin holding `main`, and the clones `a`, `b` and `c`. No ledger exists.
    fn new() -> Fixture {
        let fx = Fixture {
            root: tempfile::tempdir().unwrap(),
        };
        std::fs::create_dir_all(fx.home()).unwrap();
        common::seed_dolt_home(&fx.home());
        std::fs::create_dir_all(fx.state()).unwrap();
        let root = fx.root.path();
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
        git(&seed, &["add", "-A"]);
        git(&seed, &["commit", "--quiet", "-m", "Main commit"]);
        git(
            &seed,
            &["push", "--quiet", "origin", "HEAD:refs/heads/main"],
        );
        for name in ["a", "b", "c"] {
            git(
                root,
                &["clone", "--quiet", fx.origin().to_str().unwrap(), name],
            );
        }
        fx
    }

    fn origin(&self) -> PathBuf {
        self.root.path().join("origin.git")
    }

    fn clone_dir(&self, name: &str) -> PathBuf {
        self.root.path().join(name)
    }

    fn home(&self) -> PathBuf {
        self.root.path().join("home")
    }

    fn state(&self) -> PathBuf {
        self.root.path().join("state")
    }

    fn command(&self, dir: &Path, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_tsk"));
        command
            .args(args)
            .current_dir(dir)
            .envs(git_env())
            .env("HOME", self.home())
            .env("XDG_STATE_HOME", self.state())
            .env("TSK_DOLT_SERVER_IDLE_SECS", "3")
            .env("XDG_CONFIG_HOME", self.home().join("config"))
            .env("TSK_HOME", self.home().join(".tsk"))
            .env("TSK_AUTO_UPDATE", "never")
            .env("GIT_CEILING_DIRECTORIES", self.root.path())
            .env_remove("TSK_LEDGER_TRANSPORT");
        common::with_git_shim(&mut command, self.root.path());
        command
    }

    fn tsk(&self, name: &str, args: &[&str]) -> Output {
        self.command(&self.clone_dir(name), args).output().unwrap()
    }

    fn ok(&self, name: &str, args: &[&str]) -> String {
        let output = self.tsk(name, args);
        assert_success(&output);
        stdout(&output)
    }

    /// Run `tsk` with `TSK_LEDGER_TRANSPORT=git-remote`.
    fn tsk_git_remote(&self, name: &str, args: &[&str]) -> Output {
        self.command(&self.clone_dir(name), args)
            .env("TSK_LEDGER_TRANSPORT", "git-remote")
            .output()
            .unwrap()
    }

    fn ledger_db(&self, name: &str) -> PathBuf {
        PathBuf::from(self.ok(name, &["ledger", "path"]).trim_end())
    }

    /// The URL of the store's `origin` remote, from Dolt's own state file.
    fn store_url(&self, name: &str) -> String {
        let state = self.ledger_db(name).join(".dolt").join("repo_state.json");
        let state: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(state).unwrap()).unwrap();
        state["remotes"]["origin"]["url"]
            .as_str()
            .unwrap()
            .to_string()
    }

    fn origin_tip(&self) -> String {
        git_line(&self.origin(), &["rev-parse", LEDGER_REF])
    }

    fn origin_names(&self) -> Vec<String> {
        git(&self.origin(), &["ls-tree", "--name-only", LEDGER_REF])
            .lines()
            .map(str::to_string)
            .collect()
    }

    fn origin_file_count(&self) -> usize {
        git(
            &self.origin(),
            &["ls-tree", "-r", "--name-only", LEDGER_REF],
        )
        .lines()
        .count()
    }

    fn origin_blob(&self, reference: &str, file: &str) -> Option<String> {
        let output = git_output(
            &self.origin(),
            &["cat-file", "blob", &format!("{}:{}", reference, file)],
        );
        output
            .status
            .success()
            .then(|| String::from_utf8(output.stdout).unwrap())
    }

    fn is_ancestor(&self, older: &str, newer: &str) -> bool {
        git_output(
            &self.origin(),
            &["merge-base", "--is-ancestor", older, newer],
        )
        .status
        .success()
    }

    fn merge_commits_on_ledger(&self) -> usize {
        git_line(
            &self.origin(),
            &["rev-list", "--merges", "--count", LEDGER_REF],
        )
        .parse()
        .unwrap()
    }

    /// A ledger the clone `a` has published, with mission M-ONE. The build runs once per
    /// test process (see `common::templates`), and each test gets a copy.
    fn published() -> Fixture {
        Fixture {
            root: templates::instance(&PUBLISHED),
        }
    }

    /// A `git` shim that, once per `races`, runs `TSK_RACE` before a push of the ledger
    /// ref goes out. The shell command acts as another writer. (A pre-push hook of the code
    /// repo would not run: tsk starts its own git with hooks off.)
    fn install_race_hook(&self, races: usize) {
        let prelude = format!(
            "if [ \"$1\" = push ]; then\n\
             case \"$*\" in *{ledger}*)\n\
             count_file='{count}'\n\
             n=$(cat \"$count_file\" 2>/dev/null || echo 0)\n\
             if [ \"$n\" -lt {races} ]; then\n\
             echo $((n + 1)) > \"$count_file\"\n\
             unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_PREFIX GIT_COMMON_DIR\n\
             sh -c \"$TSK_RACE\" >&2 || exit 97\n\
             fi;;\n\
             esac\n\
             fi",
            ledger = LEDGER_REF,
            count = self.root.path().join("race-count").display(),
            races = races,
        );
        common::install_git_shim(self.root.path(), &prelude);
    }

    /// The shell command that has clone `b` run `tsk` with `args`, then push.
    fn race_command(&self, steps: &[&str]) -> String {
        let tsk = env!("CARGO_BIN_EXE_tsk");
        let mut parts = vec![format!("cd '{}'", self.clone_dir("b").display())];
        for step in steps {
            parts.push(format!("'{}' {}", tsk, step));
        }
        parts.join(" && ")
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

fn git_output(dir: &Path, args: &[&str]) -> Output {
    Command::new("git")
        .args(args)
        .current_dir(dir)
        .envs(git_env())
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
    String::from_utf8(output.stdout).unwrap()
}

fn git_line(dir: &Path, args: &[&str]) -> String {
    git(dir, args).trim_end().to_string()
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

fn assert_exit(output: &Output, code: i32) {
    assert_eq!(
        output.status.code(),
        Some(code),
        "stdout: {}\nstderr: {}",
        stdout(output),
        stderr(output)
    );
}

fn installed_git() -> PathBuf {
    std::env::var_os("PATH")
        .and_then(|path| {
            std::env::split_paths(&path)
                .map(|dir| dir.join("git"))
                .find(|candidate| candidate.is_file())
        })
        .expect("git is not on PATH")
}

#[test]
fn a_new_ledger_publishes_a_file_remote_and_two_clones_read_each_other() {
    need_dolt!();
    let fx = Fixture::published();

    // The ref holds a Dolt file remote: the marker, the manifest and table files, and no
    // lock files.
    let names = fx.origin_names();
    for expected in [".tsk-transport", ".gitignore", "manifest"] {
        assert!(names.iter().any(|n| n == expected), "{:?}", names);
    }
    assert!(!names.iter().any(|n| n == "LOCK"), "{:?}", names);
    assert_eq!(
        fx.origin_blob(LEDGER_REF, ".tsk-transport").as_deref(),
        Some("file-remote\n")
    );
    assert_eq!(
        fx.origin_blob(LEDGER_REF, ".gitignore").as_deref(),
        Some("LOCK\noldgen/LOCK\n")
    );
    let first_tip = fx.origin_tip();
    let first_files = fx.origin_file_count();

    // The store of `a` talks to a directory, which is a linked worktree.
    assert!(
        fx.store_url("a").starts_with("file://"),
        "{}",
        fx.store_url("a")
    );
    let remote_dir = fx.ledger_db("a").parent().unwrap().join("ledger-remote");
    assert!(remote_dir.join(".git").exists());

    // `b` reads the mission, adds a task and publishes. `a` reads it back.
    fx.ok("b", &["ledger", "fetch"]);
    assert!(fx.store_url("b").starts_with("file://"));
    assert!(fx
        .ok("b", &["mission", "list", "--porcelain"])
        .contains("M-ONE"));
    fx.ok("b", &["task", "add", "M-ONE", "from b"]);
    fx.ok("b", &["ledger", "push", "B adds a task"]);
    let second_tip = fx.origin_tip();
    assert_ne!(first_tip, second_tip);
    assert!(fx.origin_file_count() > first_files);
    fx.ok("a", &["ledger", "fetch"]);
    assert_eq!(
        fx.ok("a", &["task", "list", "M-ONE", "--porcelain"]),
        "T-01\ttodo\t-\tfrom b\n"
    );
    fx.ok("a", &["task", "add", "M-ONE", "from a"]);
    fx.ok("a", &["ledger", "push", "A adds a task"]);

    // Every publish is a normal commit on top of the last: nothing was forced or merged.
    let third_tip = fx.origin_tip();
    assert!(fx.is_ancestor(&first_tip, &second_tip));
    assert!(fx.is_ancestor(&second_tip, &third_tip));
    assert_eq!(fx.merge_commits_on_ledger(), 0);
    let status = fx.ok("a", &["ledger", "status"]);
    assert!(status.contains("nothing to push"), "{}", status);
    assert!(status.contains("up to date"), "{}", status);
}

#[test]
fn dolt_never_starts_git_for_the_ledger_data() {
    need_dolt!();
    let fx = Fixture::new();
    let shim_dir = fx.root.path().join("shim");
    std::fs::create_dir_all(&shim_dir).unwrap();
    let log = fx.root.path().join("git-log.txt");
    let shim = shim_dir.join("git");
    write(
        &shim,
        &format!(
            "#!/bin/sh\necho \"$PWD $@\" >> '{}'\nexec '{}' \"$@\"\n",
            log.display(),
            installed_git().display()
        ),
    );
    let mut permissions = std::fs::metadata(&shim).unwrap().permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o755);
    std::fs::set_permissions(&shim, permissions).unwrap();
    let mut dirs = vec![shim_dir];
    dirs.extend(std::env::split_paths(&std::env::var_os("PATH").unwrap()));
    let path = std::env::join_paths(dirs).unwrap();
    let run = |name: &str, args: &[&str]| {
        let output = fx
            .command(&fx.clone_dir(name), args)
            .env("PATH", &path)
            .output()
            .unwrap();
        assert_success(&output);
    };

    run("a", &["ledger", "fetch"]);
    run("a", &["mission", "create", "M-ONE", "One"]);
    run("a", &["ledger", "push", "First"]);
    run("b", &["ledger", "fetch"]);
    run("b", &["task", "add", "M-ONE", "from b"]);
    run("b", &["ledger", "push", "Second"]);
    run("a", &["ledger", "fetch"]);

    let text = std::fs::read_to_string(&log).unwrap();
    assert!(!text.is_empty());
    assert!(!text.contains("git-remote-cache"), "{}", text);
    assert!(!text.contains("/ledger-db"), "{}", text);
}

#[test]
fn a_stale_writer_is_rejected_by_a_plain_push_then_merges_and_pushes_again() {
    need_dolt!();
    let fx = Fixture::published();
    fx.ok("b", &["ledger", "fetch"]);
    let base_tip = fx.origin_tip();
    fx.install_race_hook(1);
    fx.ok("a", &["mission", "create", "M-THREE", "Three"]);

    // `b` publishes M-TWO after `a` has fetched and before its push goes out.
    let race = fx.race_command(&["mission create M-TWO Two", "ledger push 'B races'"]);
    let output = fx
        .command(&fx.clone_dir("a"), &["ledger", "push", "A publishes"])
        .env("TSK_RACE", race)
        .output()
        .unwrap();
    assert_success(&output);
    // The push went on the record of the last fetch, so the first rejection only shows the
    // record was old. It is not said aloud, and the retry merges.
    let notes = stderr(&output);
    assert!(!notes.contains("push rejected"), "{}", notes);

    // Both writers' rows are on origin and in `a`, and the history is linear.
    let list = fx.ok("a", &["mission", "list", "--porcelain"]);
    for id in ["M-ONE", "M-TWO", "M-THREE"] {
        assert!(list.contains(id), "{}", list);
    }
    fx.ok("c", &["ledger", "fetch"]);
    let from_origin = fx.ok("c", &["mission", "list", "--porcelain"]);
    for id in ["M-ONE", "M-TWO", "M-THREE"] {
        assert!(from_origin.contains(id), "{}", from_origin);
    }
    assert!(fx.is_ancestor(&base_tip, &fx.origin_tip()));
    assert_eq!(fx.merge_commits_on_ledger(), 0);
}

#[test]
fn a_task_added_to_one_mission_by_two_writers_is_renumbered_and_printed() {
    need_dolt!();
    let fx = Fixture::published();
    fx.ok("b", &["ledger", "fetch"]);
    fx.install_race_hook(1);
    fx.ok("a", &["task", "add", "M-ONE", "from a"]);

    let race = fx.race_command(&["task add M-ONE 'from b'", "ledger push 'B races'"]);
    let output = fx
        .command(&fx.clone_dir("a"), &["ledger", "push", "A publishes"])
        .env("TSK_RACE", race)
        .output()
        .unwrap();
    assert_success(&output);
    let lines: Vec<String> = stdout(&output).lines().map(str::to_string).collect();
    assert_eq!(lines[0], "T-01 is now T-02 in M-ONE", "{:?}", lines);
    assert_eq!(
        fx.ok("a", &["task", "list", "M-ONE", "--porcelain"]),
        "T-01\ttodo\t-\tfrom b\nT-02\ttodo\t-\tfrom a\n"
    );
    fx.ok("b", &["ledger", "fetch"]);
    assert_eq!(
        fx.ok("b", &["task", "list", "M-ONE", "--porcelain"]),
        "T-01\ttodo\t-\tfrom b\nT-02\ttodo\t-\tfrom a\n"
    );
}

#[test]
fn a_same_cell_conflict_exits_1_and_leaves_origin_alone() {
    need_dolt!();
    let fx = Fixture::published();
    fx.ok("b", &["ledger", "fetch"]);
    fx.ok("a", &["mission", "set", "M-ONE", "name", "Name from a"]);
    fx.ok("a", &["ledger", "push", "A renames"]);
    let before = fx.origin_tip();

    fx.ok("b", &["mission", "set", "M-ONE", "name", "Name from b"]);
    let output = fx.tsk("b", &["ledger", "push", "B renames"]);
    assert_exit(&output, 1);
    assert!(
        stderr(&output).contains("conflicted"),
        "{}",
        stderr(&output)
    );
    assert!(stderr(&output).contains("missions"), "{}", stderr(&output));
    assert_eq!(fx.origin_tip(), before);
    // B's commit stays, and so does its name.
    assert!(fx
        .ok("b", &["mission", "list", "--porcelain"])
        .contains("Name from b"));
}

#[test]
fn documents_and_records_publish_together() {
    need_dolt!();
    let fx = Fixture::published();
    let mut child = fx
        .command(
            &fx.clone_dir("a"),
            &["ledger", "doc", "put", "notes/plan.1c0de051.md"],
        )
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();
    use std::io::Write;
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"the plan\n")
        .unwrap();
    assert!(child.wait().unwrap().success());
    fx.ok("a", &["mission", "create", "M-TWO", "Two"]);
    let ledger_before = fx.origin_tip();
    fx.ok("a", &["ledger", "push", "Plan and mission"]);

    assert_ne!(fx.origin_tip(), ledger_before);
    assert_eq!(
        fx.origin_blob(DOCS_REF, "notes/plan.1c0de051.md")
            .as_deref(),
        Some("the plan\n")
    );
    fx.ok("b", &["ledger", "fetch"]);
    assert_eq!(
        fx.ok("b", &["ledger", "doc", "get", "notes/plan.md"]),
        "the plan\n"
    );
    assert!(fx
        .ok("b", &["mission", "list", "--porcelain"])
        .contains("M-TWO"));
}

#[test]
fn a_store_whose_remote_directory_is_gone_fetches_it_again() {
    need_dolt!();
    let fx = Fixture::published();
    let remote_dir = fx.ledger_db("a").parent().unwrap().join("ledger-remote");
    fx.ok("a", &["task", "add", "M-ONE", "after the loss"]);
    // Remove the directory and its registration, as a cleaned state directory would.
    std::fs::remove_dir_all(&remote_dir).unwrap();
    fx.ok("a", &["ledger", "push", "Publish without the directory"]);
    assert!(remote_dir.join(".git").exists());
    fx.ok("b", &["ledger", "fetch"]);
    assert_eq!(
        fx.ok("b", &["task", "list", "M-ONE", "--porcelain"]),
        "T-01\ttodo\t-\tafter the loss\n"
    );
}

#[test]
fn a_ledger_made_with_the_git_remote_transport_works_and_a_clone_follows_the_ref() {
    need_dolt!();
    let fx = Fixture::new();
    assert_success(&fx.tsk_git_remote("a", &["ledger", "fetch"]));
    fx.ok("a", &["mission", "create", "M-ONE", "One"]);
    assert_success(&fx.tsk_git_remote("a", &["ledger", "push", "Create the ledger"]));

    // The ref holds the blobstore, not a file remote.
    assert!(fx.origin_blob(LEDGER_REF, ".tsk-transport").is_none());
    assert!(fx.origin_names().iter().any(|n| n == "manifest"));
    assert!(
        fx.store_url("a").starts_with("git+"),
        "{}",
        fx.store_url("a")
    );

    // A clone without the variable takes the layout the ref holds, and both push.
    fx.ok("b", &["ledger", "fetch"]);
    assert!(
        fx.store_url("b").starts_with("git+"),
        "{}",
        fx.store_url("b")
    );
    fx.ok("b", &["task", "add", "M-ONE", "from b"]);
    fx.ok("b", &["ledger", "push", "B adds a task"]);
    fx.ok("a", &["ledger", "fetch"]);
    assert_eq!(
        fx.ok("a", &["task", "list", "M-ONE", "--porcelain"]),
        "T-01\ttodo\t-\tfrom b\n"
    );
    fx.ok("a", &["task", "add", "M-ONE", "from a"]);
    fx.ok("a", &["ledger", "push", "A adds a task"]);
    assert!(fx.origin_blob(LEDGER_REF, ".tsk-transport").is_none());
}

#[test]
fn an_unknown_transport_value_is_refused() {
    need_dolt!();
    let fx = Fixture::new();
    let output = fx
        .command(&fx.clone_dir("a"), &["ledger", "fetch"])
        .env("TSK_LEDGER_TRANSPORT", "carrier-pigeon")
        .output()
        .unwrap();
    assert_exit(&output, 1);
    assert!(
        stderr(&output).contains("TSK_LEDGER_TRANSPORT"),
        "{}",
        stderr(&output)
    );
}

/// A git-remote ledger published by `a`, with `b` a clone of it.
fn old_layout() -> Fixture {
    let fx = Fixture::new();
    assert_success(&fx.tsk_git_remote("a", &["ledger", "fetch"]));
    fx.ok("a", &["mission", "create", "M-ONE", "One"]);
    fx.ok("a", &["task", "add", "M-ONE", "first"]);
    assert_success(&fx.tsk_git_remote("a", &["ledger", "push", "Create the ledger"]));
    fx.ok("b", &["ledger", "fetch"]);
    assert!(fx.store_url("b").starts_with("git+"));
    fx
}

#[test]
fn migrate_transport_converts_an_old_ledger_with_a_normal_commit() {
    need_dolt!();
    let fx = old_layout();
    let old_tip = fx.origin_tip();

    // Unpushed commits stop it and name the push.
    fx.ok("a", &["task", "add", "M-ONE", "unpushed"]);
    let refused = fx.tsk("a", &["ledger", "migrate-transport"]);
    assert_exit(&refused, 1);
    assert!(
        stderr(&refused).contains("tsk ledger push"),
        "{}",
        stderr(&refused)
    );
    assert_eq!(fx.origin_tip(), old_tip);
    assert_success(&fx.tsk_git_remote("a", &["ledger", "push", "Unpushed"]));
    let old_tip = fx.origin_tip();

    let output = fx.tsk("a", &["ledger", "migrate-transport"]);
    assert_success(&output);
    let lines: Vec<String> = stdout(&output).lines().map(str::to_string).collect();
    assert_eq!(lines[0], "migrated-transport:file-remote");
    assert_eq!(lines[1], fx.origin_tip());

    // One normal commit on the old tip: the old content is replaced, not forced.
    assert!(fx.is_ancestor(&old_tip, &fx.origin_tip()));
    assert_eq!(
        git_line(
            &fx.origin(),
            &[
                "rev-list",
                "--count",
                &format!("{}..{}", old_tip, LEDGER_REF)
            ]
        ),
        "1"
    );
    assert_eq!(
        fx.origin_blob(LEDGER_REF, ".tsk-transport").as_deref(),
        Some("file-remote\n")
    );
    assert!(fx.store_url("a").starts_with("file://"));
    assert!(fx
        .ok("a", &["mission", "list", "--porcelain"])
        .contains("M-ONE"));
    let status = fx.ok("a", &["ledger", "status"]);
    assert!(status.contains("up to date"), "{}", status);

    // A second run has nothing to do.
    let again = fx.ok("a", &["ledger", "migrate-transport"]);
    assert_eq!(again, "migrated-transport:already\n");
    assert_eq!(fx.merge_commits_on_ledger(), 0);
}

#[test]
fn a_clean_clone_follows_a_migration_on_its_next_fetch_and_a_fresh_clone_takes_the_file_remote() {
    need_dolt!();
    let fx = old_layout();
    assert_success(&fx.tsk("a", &["ledger", "migrate-transport"]));

    // `b` still uses the old layout. Its next fetch finds the marker and moves the store.
    assert!(fx.store_url("b").starts_with("git+"));
    let output = fx.tsk("b", &["ledger", "fetch"]);
    assert_success(&output);
    assert!(
        stderr(&output).contains("file remote"),
        "{}",
        stderr(&output)
    );
    assert!(fx.store_url("b").starts_with("file://"));
    assert_eq!(
        fx.ok("b", &["task", "list", "M-ONE", "--porcelain"]),
        "T-01\ttodo\t-\tfirst\n"
    );

    // It publishes over the new transport, and `a` reads it.
    fx.ok("b", &["task", "add", "M-ONE", "from b"]);
    fx.ok("b", &["ledger", "push", "B after the migration"]);
    fx.ok("a", &["ledger", "fetch"]);
    let both = "T-01\ttodo\t-\tfirst\nT-02\ttodo\t-\tfrom b\n";
    assert_eq!(fx.ok("a", &["task", "list", "M-ONE", "--porcelain"]), both);

    // A fresh clone takes the file remote.
    fx.ok("c", &["ledger", "fetch"]);
    assert!(fx.store_url("c").starts_with("file://"));
    assert_eq!(fx.ok("c", &["task", "list", "M-ONE", "--porcelain"]), both);
    assert_eq!(fx.merge_commits_on_ledger(), 0);
}

#[test]
fn a_clone_with_unpushed_changes_keeps_them_across_a_migration() {
    need_dolt!();
    let fx = old_layout();
    assert_success(&fx.tsk("a", &["ledger", "migrate-transport"]));

    // `b` holds a change origin lacks. A fetch refuses to reset over it and names the push.
    fx.ok("b", &["task", "add", "M-ONE", "local to b"]);
    let refused = fx.tsk("b", &["ledger", "fetch"]);
    assert_exit(&refused, 1);
    assert!(
        stderr(&refused).contains("tsk ledger push"),
        "{}",
        stderr(&refused)
    );
    assert!(fx.store_url("b").starts_with("git+"));

    // The push moves the store to the file remote, merges, and publishes the change.
    let output = fx.tsk("b", &["ledger", "push", "B after the migration"]);
    assert_success(&output);
    assert!(
        stderr(&output).contains("file remote"),
        "{}",
        stderr(&output)
    );
    assert!(fx.store_url("b").starts_with("file://"));
    fx.ok("a", &["ledger", "fetch"]);
    assert_eq!(
        fx.ok("a", &["task", "list", "M-ONE", "--porcelain"]),
        "T-01\ttodo\t-\tfirst\nT-02\ttodo\t-\tlocal to b\n"
    );
    assert_eq!(fx.merge_commits_on_ledger(), 0);
}
