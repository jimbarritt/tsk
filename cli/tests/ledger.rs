//! The ledger commands on a Dolt ledger, end to end, with a bare git repo as origin. A
//! publisher clone writes the first ledger. Clone `work` is the one under test. The
//! documents of a ledger are files on the `tsk/docs` branch, checked out as a linked
//! worktree of the clone, and these tests cover the fetch and the compare-and-swap push
//! of that worktree as well as the commands that find it.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

mod common;

use common::templates::{self, Seed, Step};

/// A ledger the publisher has fetched and pushed. `work` has not fetched it.
const PUBLISHED: Seed = Seed {
    branch: "main",
    clones: &["work", "publisher"],
    refs: Some("branches"),
    steps: &[
        Step::Tsk("publisher", &["ledger", "fetch"]),
        Step::Tsk("publisher", &["ledger", "push", "Create the ledger"]),
    ],
};

const DOCS_REF: &str = "refs/heads/tsk/docs";

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
    /// A bare origin holding `main`, and the clone `work`. No ledger is published.
    fn new() -> Fixture {
        let fixture = Fixture {
            root: tempfile::tempdir().unwrap(),
        };
        std::fs::create_dir_all(fixture.home()).unwrap();
        std::fs::create_dir_all(fixture.state()).unwrap();
        git(
            fixture.root.path(),
            &["init", "--quiet", "--bare", "-b", "main", "origin.git"],
        );
        git(
            fixture.root.path(),
            &["init", "--quiet", "-b", "main", "seed"],
        );
        git(
            &fixture.seed(),
            &[
                "remote",
                "add",
                "origin",
                fixture.origin().to_str().unwrap(),
            ],
        );
        write(&fixture.seed().join("README.md"), "main\n");
        commit_all(&fixture.seed(), "Main commit");
        git(
            &fixture.seed(),
            &["push", "--quiet", "origin", "HEAD:refs/heads/main"],
        );
        for name in ["work", "publisher"] {
            git(
                fixture.root.path(),
                &["clone", "--quiet", fixture.origin().to_str().unwrap(), name],
            );
        }
        fixture
    }

    /// [`Fixture::new`] and a ledger the publisher has pushed: both branches exist on
    /// origin. The clone `work` has not fetched it. The build runs once per test process
    /// (see `common::templates`), and each test gets a copy.
    fn with_ledger() -> Fixture {
        Fixture {
            root: templates::instance(&PUBLISHED),
        }
    }

    fn origin(&self) -> PathBuf {
        self.root.path().join("origin.git")
    }

    fn seed(&self) -> PathBuf {
        self.root.path().join("seed")
    }

    fn work(&self) -> PathBuf {
        self.root.path().join("work")
    }

    fn home(&self) -> PathBuf {
        self.root.path().join("home")
    }

    fn state(&self) -> PathBuf {
        self.root.path().join("state")
    }

    fn origin_ref_sha(&self, reference: &str) -> String {
        git_line(&self.origin(), &["rev-parse", reference])
    }

    fn origin_docs_sha(&self) -> String {
        self.origin_ref_sha(DOCS_REF)
    }

    /// Land a commit on origin's documents branch from outside the clone under test.
    fn add_docs_commit(&self, file: &str, content: &str) -> String {
        let seed = self.seed();
        git(&seed, &["fetch", "--quiet", "origin", DOCS_REF]);
        git(
            &seed,
            &["checkout", "--quiet", "-B", "docs-seed", "FETCH_HEAD"],
        );
        write(&seed.join(file), content);
        commit_all(&seed, &format!("Add {}", file));
        git(
            &seed,
            &["push", "--quiet", "origin", &format!("HEAD:{}", DOCS_REF)],
        );
        git_line(&seed, &["rev-parse", "HEAD"])
    }

    fn clone_id_file(&self) -> PathBuf {
        self.work().join(".git").join("tsk-clone-id")
    }

    fn tsk(&self, args: &[&str]) -> Output {
        self.tsk_in(&self.work(), args)
    }

    fn tsk_in(&self, dir: &Path, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_tsk"))
            .args(args)
            .current_dir(dir)
            .envs(git_env())
            .env("HOME", self.home())
            .env("XDG_STATE_HOME", self.state())
            .env("TSK_DOLT_SERVER_IDLE_SECS", "3")
            .env("XDG_CONFIG_HOME", self.home().join("config"))
            .env("TSK_HOME", self.home().join(".tsk"))
            .env("GIT_CEILING_DIRECTORIES", self.root.path())
            .output()
            .expect("failed to run tsk")
    }

    fn tsk_stdin(&self, args: &[&str], input: &str) -> Output {
        use std::io::Write;
        let mut child = Command::new(env!("CARGO_BIN_EXE_tsk"))
            .args(args)
            .current_dir(self.work())
            .envs(git_env())
            .env("HOME", self.home())
            .env("XDG_STATE_HOME", self.state())
            .env("TSK_DOLT_SERVER_IDLE_SECS", "3")
            .env("XDG_CONFIG_HOME", self.home().join("config"))
            .env("TSK_HOME", self.home().join(".tsk"))
            .env("GIT_CEILING_DIRECTORIES", self.root.path())
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("failed to run tsk");
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
        child.wait_with_output().unwrap()
    }

    fn push(&self, message: &str) -> Output {
        self.tsk(&["ledger", "push", message])
    }

    fn racer(&self) -> PathBuf {
        self.root.path().join("racer")
    }

    /// A pre-push hook in the clone that, for the first `races` pushes, lands a commit on
    /// origin's documents branch from the racer clone before the push goes through.
    fn install_racing_pre_push_hook(&self, races: usize) {
        git(
            self.root.path(),
            &["clone", "--quiet", self.origin().to_str().unwrap(), "racer"],
        );
        let script = format!(
            "#!/bin/sh\n\
             cat >/dev/null\n\
             unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_PREFIX GIT_COMMON_DIR\n\
             racer='{racer}'\n\
             count_file='{count}'\n\
             n=$(cat \"$count_file\" 2>/dev/null || echo 0)\n\
             [ \"$n\" -ge {races} ] && exit 0\n\
             n=$((n + 1))\n\
             echo \"$n\" > \"$count_file\"\n\
             git -C \"$racer\" fetch --quiet origin {docs_ref} || exit 1\n\
             git -C \"$racer\" reset --quiet --hard FETCH_HEAD || exit 1\n\
             echo \"race $n\" > \"$racer/race-$n.md\"\n\
             git -C \"$racer\" add -A || exit 1\n\
             git -C \"$racer\" commit --quiet -m \"Race $n\" || exit 1\n\
             git -C \"$racer\" push --quiet origin HEAD:{docs_ref} || exit 1\n\
             exit 0\n",
            racer = self.racer().display(),
            count = self.root.path().join("race-count").display(),
            races = races,
            docs_ref = DOCS_REF,
        );
        let hook = self.work().join(".git").join("hooks").join("pre-push");
        std::fs::create_dir_all(hook.parent().unwrap()).unwrap();
        write(&hook, &script);
        let mut permissions = std::fs::metadata(&hook).unwrap().permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o755);
        std::fs::set_permissions(&hook, permissions).unwrap();
    }

    fn origin_file(&self, git_ref: &str, file: &str) -> Option<String> {
        let output = git_output(
            &self.origin(),
            &["cat-file", "blob", &format!("{}:{}", git_ref, file)],
        );
        if output.status.success() {
            Some(String::from_utf8(output.stdout).unwrap())
        } else {
            None
        }
    }

    /// Fetch the ledger in the clone under test; returns the path of the Dolt database.
    fn fetch_ok(&self) -> PathBuf {
        let output = self.tsk(&["ledger", "fetch"]);
        assert_success(&output);
        PathBuf::from(stdout(&output).trim_end())
    }

    /// Fetch the ledger; returns the documents worktree, which sits beside the database.
    fn fetch_docs(&self) -> PathBuf {
        self.fetch_ok().parent().unwrap().join("docs")
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

fn head(dir: &Path) -> String {
    git_line(dir, &["rev-parse", "HEAD"])
}

fn is_ancestor(dir: &Path, ancestor: &str, descendant: &str) -> bool {
    git_output(dir, &["merge-base", "--is-ancestor", ancestor, descendant])
        .status
        .success()
}

fn origin_docs_subject(fx: &Fixture) -> String {
    git_line(&fx.origin(), &["log", "-1", "--format=%s", DOCS_REF])
}

// --- fetch ---

#[test]
fn fetch_materialises_the_ledger_at_the_fixed_path() {
    let fx = Fixture::with_ledger();

    let path = fx.fetch_ok();

    let clone_id = std::fs::read_to_string(fx.clone_id_file()).unwrap();
    let clone_id = clone_id.trim();
    let (name, suffix) = clone_id.rsplit_once('-').unwrap();
    assert_eq!(name, "work");
    assert_eq!(suffix.len(), 8);
    assert!(suffix
        .chars()
        .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));

    let base = fx.state().join("tsk").join("repos").join(clone_id);
    assert_eq!(path, base.join("ledger-db"));
    assert!(path.join(".dolt").is_dir());
    let docs = base.join("docs");
    assert!(docs.join("index.md").is_file());
    assert_eq!(head(&docs), fx.origin_docs_sha());
}

#[test]
fn fetch_creates_a_detached_documents_worktree_of_the_clone() {
    let fx = Fixture::with_ledger();
    let docs = fx.fetch_docs();

    let symbolic = git_output(&docs, &["symbolic-ref", "-q", "HEAD"]);
    assert!(
        !symbolic.status.success(),
        "worktree HEAD should be detached"
    );

    let listing = git(&fx.work(), &["worktree", "list", "--porcelain", "-z"]);
    let canonical = std::fs::canonicalize(&docs).unwrap();
    assert!(listing
        .split('\0')
        .filter_map(|f| f.strip_prefix("worktree "))
        .any(|p| std::fs::canonicalize(p).ok() == Some(canonical.clone())));
}

#[test]
fn fetch_refreshes_the_documents_worktree_in_place() {
    let fx = Fixture::with_ledger();
    let first = fx.fetch_docs();

    let new_sha = fx.add_docs_commit("missions.md", "new mission\n");
    let second = fx.fetch_docs();

    assert_eq!(first, second);
    assert_eq!(head(&second), new_sha);
    assert_eq!(
        std::fs::read_to_string(second.join("missions.md")).unwrap(),
        "new mission\n"
    );
}

#[test]
fn fetch_refuses_to_reset_over_uncommitted_changes() {
    let fx = Fixture::with_ledger();
    let docs = fx.fetch_docs();
    let before = head(&docs);

    write(&docs.join("index.md"), "edited locally\n");
    fx.add_docs_commit("other.md", "other\n");

    let output = fx.tsk(&["ledger", "fetch"]);
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("uncommitted changes"),
        "{}",
        stderr(&output)
    );
    assert!(stdout(&output).is_empty());
    assert_eq!(head(&docs), before);
    assert_eq!(
        std::fs::read_to_string(docs.join("index.md")).unwrap(),
        "edited locally\n"
    );
}

#[test]
fn fetch_refuses_to_reset_over_untracked_files() {
    let fx = Fixture::with_ledger();
    let docs = fx.fetch_docs();

    write(&docs.join("new-thread.md"), "draft\n");

    let output = fx.tsk(&["ledger", "fetch"]);
    assert!(!output.status.success());
    assert!(docs.join("new-thread.md").is_file());
}

#[test]
fn fetch_keeps_an_unpushed_documents_commit_and_still_prints_the_path() {
    let fx = Fixture::with_ledger();
    let docs = fx.fetch_docs();

    write(&docs.join("local.md"), "local\n");
    commit_all(&docs, "Unpushed documents change");
    let local = head(&docs);
    fx.add_docs_commit("remote.md", "remote\n");

    let output = fx.tsk(&["ledger", "fetch"]);
    assert_success(&output);
    assert_eq!(
        PathBuf::from(stdout(&output).trim_end()),
        docs.parent().unwrap().join("ledger-db")
    );
    assert!(
        stderr(&output).contains("Unpushed documents change"),
        "{}",
        stderr(&output)
    );
    assert!(
        stderr(&output).contains("not on origin's refs/heads/tsk/docs"),
        "{}",
        stderr(&output)
    );
    assert_eq!(head(&docs), local);
}

/// An orphan commit pushed to a custom ref of the same name as a branch, as the original
/// design's refs were. Git resolves an unqualified name against it first.
fn shadow(fx: &Fixture, custom_ref: &str) -> String {
    let branch = format!("shadow-{}", custom_ref.replace('/', "-"));
    git(&fx.seed(), &["switch", "--quiet", "--orphan", &branch]);
    write(&fx.seed().join("shadow.md"), "orphaned custom ref\n");
    commit_all(&fx.seed(), "Shadow");
    git(
        &fx.seed(),
        &["push", "--quiet", "origin", &format!("HEAD:{}", custom_ref)],
    );
    git_line(&fx.origin(), &["rev-parse", custom_ref])
}

#[test]
fn fetch_uses_the_full_ref_names_when_custom_refs_shadow_them() {
    let fx = Fixture::with_ledger();
    let branch_sha = fx.origin_docs_sha();
    shadow(&fx, "refs/tsk/docs");
    shadow(&fx, "refs/tsk/ledger");

    let docs = fx.fetch_docs();

    assert_eq!(head(&docs), branch_sha);
    assert!(!docs.join("shadow.md").exists());
}

#[test]
fn fetch_reuses_an_existing_clone_id() {
    let fx = Fixture::with_ledger();
    write(&fx.clone_id_file(), "custom-1234abcd\n");

    let path = fx.fetch_ok();

    assert_eq!(path, fx.state().join("tsk/repos/custom-1234abcd/ledger-db"));
    assert_eq!(
        std::fs::read_to_string(fx.clone_id_file()).unwrap(),
        "custom-1234abcd\n"
    );
}

#[test]
fn fetch_recreates_a_documents_worktree_whose_directory_was_deleted() {
    let fx = Fixture::with_ledger();
    let docs = fx.fetch_docs();
    std::fs::remove_dir_all(&docs).unwrap();

    let again = fx.fetch_docs();

    assert_eq!(again, docs);
    assert_eq!(head(&again), fx.origin_docs_sha());
}

#[test]
fn fetch_refuses_a_documents_directory_that_is_not_a_worktree_of_this_clone() {
    let fx = Fixture::with_ledger();
    write(&fx.clone_id_file(), "work-00000000\n");
    let docs = fx.state().join("tsk/repos/work-00000000/docs");
    std::fs::create_dir_all(&docs).unwrap();
    write(&docs.join("stray.txt"), "not a worktree\n");

    let output = fx.tsk(&["ledger", "fetch"]);

    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("not a worktree of this clone"),
        "{}",
        stderr(&output)
    );
    assert!(docs.join("stray.txt").is_file());
}

#[test]
fn fetch_from_a_linked_worktree_uses_the_same_clone_id() {
    let fx = Fixture::with_ledger();
    let from_main = fx.fetch_ok();

    git(
        &fx.work(),
        &["worktree", "add", "--quiet", "--detach", "../linked"],
    );
    let output = fx.tsk_in(&fx.root.path().join("linked"), &["ledger", "fetch"]);
    assert_success(&output);

    assert_eq!(PathBuf::from(stdout(&output).trim_end()), from_main);
}

// --- path ---

#[test]
fn path_prints_the_fetched_store_path() {
    let fx = Fixture::with_ledger();
    let fetched = fx.fetch_ok();

    let output = fx.tsk(&["ledger", "path"]);

    assert_success(&output);
    assert_eq!(stdout(&output), format!("{}\n", fetched.display()));
}

#[test]
fn path_does_not_refresh_the_store() {
    let fx = Fixture::with_ledger();
    let docs = fx.fetch_docs();
    let before = head(&docs);
    fx.add_docs_commit("later.md", "later\n");

    assert_success(&fx.tsk(&["ledger", "path"]));

    assert_eq!(head(&docs), before);
    assert!(!docs.join("later.md").exists());
}

#[test]
fn path_without_a_clone_id_fails_and_writes_nothing() {
    let fx = Fixture::with_ledger();

    let output = fx.tsk(&["ledger", "path"]);

    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("tsk ledger fetch"),
        "{}",
        stderr(&output)
    );
    assert!(stdout(&output).is_empty());
    assert!(!fx.clone_id_file().exists());
    assert!(!fx
        .state()
        .join("tsk")
        .join("repos")
        .join("work-abcdef12")
        .exists());
}

#[test]
fn path_with_a_clone_id_prints_the_path_before_any_fetch() {
    let fx = Fixture::with_ledger();
    write(&fx.clone_id_file(), "work-abcdef12\n");

    let output = fx.tsk(&["ledger", "path"]);

    assert_success(&output);
    assert_eq!(
        stdout(&output),
        format!(
            "{}\n",
            fx.state()
                .join("tsk/repos/work-abcdef12/ledger-db")
                .display()
        )
    );
    assert!(!fx.state().join("tsk/repos/work-abcdef12").exists());
}

#[test]
fn path_falls_back_to_home_when_xdg_state_home_is_empty() {
    let fx = Fixture::with_ledger();
    write(&fx.clone_id_file(), "work-abcdef12\n");

    let output = Command::new(env!("CARGO_BIN_EXE_tsk"))
        .args(["ledger", "path"])
        .current_dir(fx.work())
        .envs(git_env())
        .env("HOME", fx.home())
        .env("XDG_STATE_HOME", "")
        .env("TSK_DOLT_SERVER_IDLE_SECS", "3")
        .env("XDG_CONFIG_HOME", fx.home().join("config"))
        .output()
        .unwrap();

    assert_success(&output);
    assert_eq!(
        stdout(&output),
        format!(
            "{}\n",
            fx.home()
                .join(".local/state/tsk/repos/work-abcdef12/ledger-db")
                .display()
        )
    );
}

#[test]
fn ledger_commands_fail_outside_a_git_repository() {
    let fx = Fixture::new();
    let outside = fx.root.path().join("outside");
    std::fs::create_dir_all(&outside).unwrap();

    for command in [
        vec!["ledger", "fetch"],
        vec!["ledger", "path"],
        vec!["ledger", "push", "message"],
        vec!["ledger", "status"],
        vec!["ledger", "discard", "--yes"],
        vec!["ledger", "doc", "put", "notes.md"],
        vec!["ledger", "doc", "get", "notes.md"],
        vec!["ledger", "migrate-docs"],
        vec!["mission", "list"],
        vec!["mission", "create", "M-X", "Name"],
        vec!["task", "list", "M-X"],
        vec!["decision", "list", "M-X"],
        vec!["decision", "add", "M-X", "Text", "--by", "jim"],
        vec!["question", "list"],
        vec!["thread", "list"],
        vec!["thread", "start", "M-X"],
        vec!["thread", "skip"],
        vec!["events", "read-new"],
    ] {
        let output = fx.tsk_in(&outside, &command);
        assert!(!output.status.success());
        assert_eq!(
            stderr(&output),
            "error: not inside a git repository. Run tsk from inside the repository whose ledger you want.\n"
        );
        assert_eq!(output.status.code(), Some(1), "{:?}", command);
        assert_eq!(stdout(&output), "", "{:?}", command);
    }
}

#[test]
fn commands_that_need_no_repository_still_run_outside_one() {
    let fx = Fixture::new();
    let outside = fx.root.path().join("outside");
    std::fs::create_dir_all(&outside).unwrap();

    for command in [
        vec!["--version"],
        vec!["guide", "--help"],
        vec!["nexus", "list"],
    ] {
        let output = fx.tsk_in(&outside, &command);
        assert_success(&output);
        assert!(!stderr(&output).contains("not inside a git repository"));
    }
}

#[test]
fn tsk_with_no_arguments_prints_the_help_and_exits_2() {
    let fx = Fixture::new();
    let outside = fx.root.path().join("outside");
    std::fs::create_dir_all(&outside).unwrap();

    let output = fx.tsk_in(&outside, &[]);

    // clap's convention for a missing subcommand: the help goes to stderr, status 2.
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(stdout(&output), "");
    let help = stderr(&output);
    assert!(help.contains("Usage: tsk [COMMAND]"), "{}", help);
    assert!(help.contains("ledger"), "{}", help);
    assert!(!help.contains("tskd"), "{}", help);
}

#[test]
fn the_removed_files_backend_is_refused_and_any_other_value_is_ignored() {
    let fx = Fixture::with_ledger();
    let refusal = "error: the files ledger backend was removed in 0.16.0; use tsk 0.15.1 to read an old file ledger\n";

    let mut command = Command::new(env!("CARGO_BIN_EXE_tsk"));
    let output = command
        .args(["ledger", "fetch"])
        .current_dir(fx.work())
        .envs(git_env())
        .env("HOME", fx.home())
        .env("XDG_STATE_HOME", fx.state())
        .env("TSK_DOLT_SERVER_IDLE_SECS", "3")
        .env("XDG_CONFIG_HOME", fx.home().join("config"))
        .env("TSK_LEDGER_BACKEND", "git")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stderr(&output), refusal);
    assert_eq!(stdout(&output), "");
    assert!(!fx.clone_id_file().exists());

    // The same value in the user config.
    let config = fx.home().join("config").join("tsk");
    std::fs::create_dir_all(&config).unwrap();
    write(&config.join("config.toml"), "[ledger]\nbackend = \"git\"\n");
    let output = fx.tsk(&["ledger", "path"]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stderr(&output), refusal);

    // The environment wins over the file, and any other value is ignored.
    for value in ["dolt", "sqlite"] {
        let output = Command::new(env!("CARGO_BIN_EXE_tsk"))
            .args(["ledger", "fetch"])
            .current_dir(fx.work())
            .envs(git_env())
            .env("HOME", fx.home())
            .env("XDG_STATE_HOME", fx.state())
            .env("TSK_DOLT_SERVER_IDLE_SECS", "3")
            .env("XDG_CONFIG_HOME", fx.home().join("config"))
            .env("TSK_LEDGER_BACKEND", value)
            .output()
            .unwrap();
        assert_success(&output);
    }
    write(
        &config.join("config.toml"),
        "[ledger]\nbackend = \"mongo\"\n",
    );
    assert_success(&fx.tsk(&["ledger", "path"]));
}

// --- push ---

#[test]
fn push_commits_every_document_change_and_pushes_it_to_the_documents_branch() {
    let fx = Fixture::with_ledger();
    let docs = fx.fetch_docs();
    let before = fx.origin_docs_sha();

    write(&docs.join("index.md"), "# Ledger index\nedited\n");
    write(&docs.join("missions/new.md"), "new mission\n");

    let output = fx.push("Record the new mission");

    assert_success(&output);
    let tip = fx.origin_docs_sha();
    assert_ne!(tip, before);
    assert_eq!(head(&docs), tip);
    assert_eq!(origin_docs_subject(&fx), "Record the new mission");
    assert_eq!(fx.origin_ref_sha(&format!("{}^", DOCS_REF)), before);
    assert_eq!(
        fx.origin_file(DOCS_REF, "index.md").unwrap(),
        "# Ledger index\nedited\n"
    );
    assert_eq!(
        fx.origin_file(DOCS_REF, "missions/new.md").unwrap(),
        "new mission\n"
    );
    assert!(git(&docs, &["status", "--porcelain=v2", "-z"]).is_empty());
}

#[test]
fn push_sends_a_documents_commit_an_earlier_run_left_unpushed() {
    let fx = Fixture::with_ledger();
    let docs = fx.fetch_docs();

    write(&docs.join("left.md"), "committed, not pushed\n");
    commit_all(&docs, "Earlier run");
    let local = head(&docs);

    let output = fx.push("Second run");

    assert_success(&output);
    assert_eq!(fx.origin_docs_sha(), local);
    assert_eq!(origin_docs_subject(&fx), "Earlier run");
}

#[test]
fn push_rebases_onto_a_documents_commit_landed_since_the_fetch() {
    let fx = Fixture::with_ledger();
    let docs = fx.fetch_docs();
    let remote = fx.add_docs_commit("remote.md", "remote\n");

    write(&docs.join("local.md"), "local\n");
    let output = fx.push("Local change");

    assert_success(&output);
    let tip = fx.origin_docs_sha();
    assert_eq!(head(&docs), tip);
    assert!(is_ancestor(&fx.origin(), &remote, &tip));
    assert_eq!(fx.origin_file(DOCS_REF, "remote.md").unwrap(), "remote\n");
    assert_eq!(fx.origin_file(DOCS_REF, "local.md").unwrap(), "local\n");
}

#[test]
fn push_retries_when_a_concurrent_writer_lands_between_fetch_and_push() {
    let fx = Fixture::with_ledger();
    let docs = fx.fetch_docs();
    fx.install_racing_pre_push_hook(1);

    write(&docs.join("local.md"), "local\n");
    let output = fx.push("Local change");

    assert_success(&output);
    // A push goes on the record of the last fetch, so the first rejection only shows the
    // record was old. It is not said aloud, and the retry merges.
    assert!(
        !stderr(&output).contains("push rejected"),
        "{}",
        stderr(&output)
    );
    let tip = fx.origin_docs_sha();
    assert_eq!(origin_docs_subject(&fx), "Local change");
    assert!(is_ancestor(&fx.origin(), &head(&fx.racer()), &tip));
    assert_eq!(fx.origin_file(DOCS_REF, "race-1.md").unwrap(), "race 1\n");
    assert_eq!(fx.origin_file(DOCS_REF, "local.md").unwrap(), "local\n");
}

#[test]
fn push_stops_after_the_retry_bound_without_overwriting_the_concurrent_writer() {
    let fx = Fixture::with_ledger();
    let docs = fx.fetch_docs();
    fx.install_racing_pre_push_hook(100);

    write(&docs.join("local.md"), "local\n");
    let output = fx.push("Local change");

    assert!(!output.status.success());
    assert!(stdout(&output).is_empty());
    assert!(
        stderr(&output).contains("rejected the push 10 times"),
        "{}",
        stderr(&output)
    );
    assert!(
        stderr(&output).contains("push rejected (attempt 9/10)"),
        "{}",
        stderr(&output)
    );
    assert_eq!(fx.origin_docs_sha(), head(&fx.racer()));
    assert_eq!(origin_docs_subject(&fx), "Race 10");
    assert!(fx.origin_file(DOCS_REF, "local.md").is_none());
    assert_eq!(
        std::fs::read_to_string(docs.join("local.md")).unwrap(),
        "local\n"
    );
    assert_eq!(
        git_line(&docs, &["log", "-1", "--format=%s", "HEAD"]),
        "Local change"
    );
}

#[test]
fn push_never_writes_the_colliding_custom_refs() {
    let fx = Fixture::with_ledger();
    let docs_shadow = shadow(&fx, "refs/tsk/docs");
    let ledger_shadow = shadow(&fx, "refs/tsk/ledger");

    let docs = fx.fetch_docs();
    write(&docs.join("local.md"), "local\n");
    let output = fx.push("Local change");

    assert_success(&output);
    assert_eq!(fx.origin_ref_sha("refs/tsk/docs"), docs_shadow);
    assert_eq!(fx.origin_ref_sha("refs/tsk/ledger"), ledger_shadow);
    assert_eq!(origin_docs_subject(&fx), "Local change");
    assert!(fx.origin_file(DOCS_REF, "shadow.md").is_none());
    let refs = git(&fx.origin(), &["for-each-ref", "--format=%(refname)"]);
    let mut refs: Vec<&str> = refs.lines().collect();
    refs.sort();
    assert_eq!(
        refs,
        vec![
            "refs/heads/main",
            "refs/heads/tsk/docs",
            "refs/heads/tsk/ledger",
            "refs/tsk/docs",
            "refs/tsk/ledger"
        ]
    );
}

#[test]
fn push_aborts_a_conflicting_rebase_and_leaves_the_remote_alone() {
    let fx = Fixture::with_ledger();
    let docs = fx.fetch_docs();
    let remote = fx.add_docs_commit("index.md", "remote edit\n");

    write(&docs.join("index.md"), "local edit\n");
    let output = fx.push("Local edit");

    assert!(!output.status.success());
    assert!(stdout(&output).is_empty());
    assert!(
        stderr(&output).contains("conflicted"),
        "{}",
        stderr(&output)
    );
    assert_eq!(fx.origin_docs_sha(), remote);
    assert_eq!(
        git_line(&docs, &["log", "-1", "--format=%s", "HEAD"]),
        "Local edit"
    );
    assert!(git(&docs, &["status", "--porcelain=v2", "-z"]).is_empty());
    let git_dir = PathBuf::from(git_line(
        &docs,
        &["rev-parse", "--path-format=absolute", "--git-dir"],
    ));
    assert!(!git_dir.join("rebase-merge").exists());
    assert!(!git_dir.join("rebase-apply").exists());
}

#[test]
fn push_without_a_store_fails_and_names_fetch() {
    let fx = Fixture::with_ledger();
    let before = fx.origin_docs_sha();

    let output = fx.push("No store");

    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("tsk ledger fetch"),
        "{}",
        stderr(&output)
    );
    assert!(!fx.clone_id_file().exists());
    assert_eq!(fx.origin_docs_sha(), before);
}

// --- doc ---

#[test]
fn doc_get_prints_the_body_exactly_with_no_added_newline() {
    let fx = Fixture::with_ledger();
    let docs = fx.fetch_docs();
    write(&docs.join("missions/m.md"), "no trailing newline");
    let output = fx.tsk(&["ledger", "doc", "get", "missions/m.md"]);
    assert_success(&output);
    assert_eq!(stdout(&output), "no trailing newline");
    let output = fx.tsk(&["ledger", "doc", "get", "index.md"]);
    assert_eq!(stdout(&output), "# Ledger index\n");
}

#[test]
fn doc_get_of_an_absent_document_fails_with_the_path() {
    let fx = Fixture::with_ledger();
    fx.fetch_ok();
    let output = fx.tsk(&["ledger", "doc", "get", "missions/none.md"]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stdout(&output), "");
    assert_eq!(
        stderr(&output),
        "error: no such document: missions/none.md\n"
    );
}

#[test]
fn doc_put_from_stdin_or_a_file_writes_the_worktree_and_does_not_push() {
    let fx = Fixture::with_ledger();
    let docs = fx.fetch_docs();
    let before = fx.origin_docs_sha();

    let output = fx.tsk_stdin(&["ledger", "doc", "put", "missions/new.md"], "from stdin\n");
    assert_success(&output);
    assert_eq!(stdout(&output), "");
    assert_eq!(
        std::fs::read_to_string(docs.join("missions/new.md")).unwrap(),
        "from stdin\n"
    );

    let source = fx.root.path().join("body.md");
    write(&source, "from file");
    let output = fx.tsk(&["ledger", "doc", "put", "index.md", source.to_str().unwrap()]);
    assert_success(&output);
    assert_eq!(
        std::fs::read_to_string(docs.join("index.md")).unwrap(),
        "from file"
    );
    assert_eq!(fx.origin_docs_sha(), before);

    let output = fx.push("Record the documents");
    assert_success(&output);
    assert_eq!(
        fx.origin_file(DOCS_REF, "missions/new.md").unwrap(),
        "from stdin\n"
    );
}

#[test]
fn doc_list_prints_sorted_paths_and_filters_by_prefix() {
    let fx = Fixture::with_ledger();
    let docs = fx.fetch_docs();
    write(&docs.join("future-missions-tbd.md"), "x");
    write(&docs.join("missions/b.md"), "b");
    write(&docs.join("missions/operational/a.md"), "a");

    let output = fx.tsk(&["ledger", "doc", "list"]);
    assert_success(&output);
    assert_eq!(
        stdout(&output),
        "README.md\nfuture-missions-tbd.md\nindex.md\nmissions/b.md\nmissions/operational/a.md\n"
    );
    let output = fx.tsk(&["ledger", "doc", "list", "missions/"]);
    assert_eq!(
        stdout(&output),
        "missions/b.md\nmissions/operational/a.md\n"
    );
    let output = fx.tsk(&["ledger", "doc", "list", "nothing"]);
    assert_success(&output);
    assert_eq!(stdout(&output), "");
}

#[test]
fn doc_rm_removes_the_document_and_fails_when_it_is_absent() {
    let fx = Fixture::with_ledger();
    let docs = fx.fetch_docs();
    write(&docs.join("missions/m.md"), "m");

    let output = fx.tsk(&["ledger", "doc", "rm", "missions/m.md"]);
    assert_success(&output);
    assert_eq!(stdout(&output), "");
    assert!(!docs.join("missions/m.md").exists());

    let output = fx.tsk(&["ledger", "doc", "rm", "missions/m.md"]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stderr(&output), "error: no such document: missions/m.md\n");
}

#[test]
fn doc_commands_reject_paths_that_cannot_be_documents() {
    let fx = Fixture::with_ledger();
    fx.fetch_ok();
    for bad in [
        "/etc/passwd",
        "../outside.md",
        "missions/../../outside.md",
        ".git",
        "a/.git/config",
    ] {
        for args in [
            vec!["ledger", "doc", "get", bad],
            vec!["ledger", "doc", "rm", bad],
        ] {
            let output = fx.tsk(&args);
            assert_eq!(output.status.code(), Some(1), "{:?}", args);
            assert!(stderr(&output).starts_with("error: "), "{:?}", args);
        }
        let output = fx.tsk_stdin(&["ledger", "doc", "put", bad], "x");
        assert_eq!(output.status.code(), Some(1), "put {}", bad);
    }
    assert!(!fx.root.path().join("outside.md").exists());
}

#[test]
fn doc_commands_do_not_fetch_and_need_a_fetched_store() {
    let fx = Fixture::with_ledger();
    let output = fx.tsk(&["ledger", "doc", "list"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("Run tsk ledger fetch first"));
    assert!(!fx.clone_id_file().exists());
}
