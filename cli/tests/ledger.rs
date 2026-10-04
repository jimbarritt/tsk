use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

const LEDGER_REF: &str = "refs/heads/tsk/ledger";

struct Fixture {
    root: TempDir,
}

impl Fixture {
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
        git(
            &fixture.seed(),
            &["switch", "--quiet", "--orphan", "ledger-seed"],
        );
        fixture
    }

    fn with_ledger(manifest: Option<&str>) -> Fixture {
        let fixture = Fixture::new();
        if let Some(text) = manifest {
            write(&fixture.seed().join(".tsk-ledger.toml"), text);
        }
        write(&fixture.seed().join("index.md"), "# Ledger index\n");
        commit_all(&fixture.seed(), "Seed the ledger");
        fixture.push_ledger();
        git(
            fixture.root.path(),
            &[
                "clone",
                "--quiet",
                fixture.origin().to_str().unwrap(),
                "work",
            ],
        );
        fixture
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

    fn push_ledger(&self) {
        git(
            &self.seed(),
            &["push", "--quiet", "origin", &format!("HEAD:{}", LEDGER_REF)],
        );
    }

    fn add_ledger_commit(&self, file: &str, content: &str) -> String {
        write(&self.seed().join(file), content);
        commit_all(&self.seed(), &format!("Add {}", file));
        self.push_ledger();
        git_line(&self.seed(), &["rev-parse", "HEAD"])
    }

    fn origin_ledger_sha(&self) -> String {
        git_line(&self.origin(), &["rev-parse", LEDGER_REF])
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
            .env("TSK_HOME", self.home().join(".tsk"))
            .env("GIT_CEILING_DIRECTORIES", self.root.path())
            .output()
            .expect("failed to run tsk")
    }

    fn push(&self, message: &str) -> Output {
        self.tsk(&["ledger", "push", message])
    }

    fn racer(&self) -> PathBuf {
        self.root.path().join("racer")
    }

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
             git -C \"$racer\" fetch --quiet origin {ledger_ref} || exit 1\n\
             git -C \"$racer\" reset --quiet --hard FETCH_HEAD || exit 1\n\
             echo \"race $n\" > \"$racer/race-$n.md\"\n\
             git -C \"$racer\" add -A || exit 1\n\
             git -C \"$racer\" commit --quiet -m \"Race $n\" || exit 1\n\
             git -C \"$racer\" push --quiet origin HEAD:{ledger_ref} || exit 1\n\
             exit 0\n",
            racer = self.racer().display(),
            count = self.root.path().join("race-count").display(),
            races = races,
            ledger_ref = LEDGER_REF,
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

    fn fetch_ok(&self) -> PathBuf {
        let output = self.tsk(&["ledger", "fetch"]);
        assert_success(&output);
        PathBuf::from(stdout(&output).trim_end())
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

#[test]
fn fetch_materialises_the_ledger_at_the_fixed_worktree_path() {
    let fx = Fixture::with_ledger(Some("version = 1\n"));

    let path = fx.fetch_ok();

    let clone_id = std::fs::read_to_string(fx.clone_id_file()).unwrap();
    let clone_id = clone_id.trim();
    let (name, suffix) = clone_id.rsplit_once('-').unwrap();
    assert_eq!(name, "work");
    assert_eq!(suffix.len(), 8);
    assert!(suffix
        .chars()
        .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));

    assert_eq!(
        path,
        fx.state()
            .join("tsk")
            .join("repos")
            .join(clone_id)
            .join("ledger")
    );
    assert!(path.join("index.md").is_file());
    assert!(path.join(".tsk-ledger.toml").is_file());
    assert_eq!(head(&path), fx.origin_ledger_sha());
}

#[test]
fn fetch_creates_a_detached_worktree_of_the_clone() {
    let fx = Fixture::with_ledger(Some("version = 1\n"));
    let path = fx.fetch_ok();

    let symbolic = git_output(&path, &["symbolic-ref", "-q", "HEAD"]);
    assert!(
        !symbolic.status.success(),
        "worktree HEAD should be detached"
    );

    let listing = git(&fx.work(), &["worktree", "list", "--porcelain", "-z"]);
    let canonical = std::fs::canonicalize(&path).unwrap();
    assert!(listing
        .split('\0')
        .filter_map(|f| f.strip_prefix("worktree "))
        .any(|p| std::fs::canonicalize(p).ok() == Some(canonical.clone())));
}

#[test]
fn fetch_refreshes_the_worktree_in_place() {
    let fx = Fixture::with_ledger(Some("version = 1\n"));
    let first = fx.fetch_ok();

    let new_sha = fx.add_ledger_commit("missions.md", "new mission\n");
    let second = fx.fetch_ok();

    assert_eq!(first, second);
    assert_eq!(head(&second), new_sha);
    assert_eq!(
        std::fs::read_to_string(second.join("missions.md")).unwrap(),
        "new mission\n"
    );
}

#[test]
fn fetch_refuses_to_reset_over_uncommitted_changes() {
    let fx = Fixture::with_ledger(Some("version = 1\n"));
    let path = fx.fetch_ok();
    let before = head(&path);

    write(&path.join("index.md"), "edited locally\n");
    fx.add_ledger_commit("other.md", "other\n");

    let output = fx.tsk(&["ledger", "fetch"]);
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("uncommitted changes"),
        "{}",
        stderr(&output)
    );
    assert!(stdout(&output).is_empty());
    assert_eq!(head(&path), before);
    assert_eq!(
        std::fs::read_to_string(path.join("index.md")).unwrap(),
        "edited locally\n"
    );
}

#[test]
fn fetch_refuses_to_reset_over_untracked_files() {
    let fx = Fixture::with_ledger(Some("version = 1\n"));
    let path = fx.fetch_ok();

    write(&path.join("new-thread.md"), "draft\n");

    let output = fx.tsk(&["ledger", "fetch"]);
    assert!(!output.status.success());
    assert!(path.join("new-thread.md").is_file());
}

#[test]
fn fetch_keeps_an_unpushed_commit_and_still_prints_the_path() {
    let fx = Fixture::with_ledger(Some("version = 1\n"));
    let path = fx.fetch_ok();

    write(&path.join("local.md"), "local\n");
    commit_all(&path, "Unpushed ledger change");
    let local = head(&path);
    fx.add_ledger_commit("remote.md", "remote\n");

    let output = fx.tsk(&["ledger", "fetch"]);
    assert_success(&output);
    assert_eq!(PathBuf::from(stdout(&output).trim_end()), path);
    assert!(
        stderr(&output).contains("Unpushed ledger change"),
        "{}",
        stderr(&output)
    );
    assert!(
        stderr(&output).contains("not on origin's refs/heads/tsk/ledger"),
        "{}",
        stderr(&output)
    );
    assert_eq!(head(&path), local);
}

#[test]
fn fetch_uses_the_full_ref_name_when_a_custom_ref_shadows_it() {
    let fx = Fixture::with_ledger(Some("version = 1\n"));
    let branch_sha = fx.origin_ledger_sha();

    git(&fx.seed(), &["switch", "--quiet", "--orphan", "shadow"]);
    write(&fx.seed().join(".tsk-ledger.toml"), "version = 1\n");
    write(&fx.seed().join("shadow.md"), "orphaned custom ref\n");
    commit_all(&fx.seed(), "Shadow");
    git(
        &fx.seed(),
        &["push", "--quiet", "origin", "HEAD:refs/tsk/ledger"],
    );

    let path = fx.fetch_ok();

    assert_eq!(head(&path), branch_sha);
    assert!(!path.join("shadow.md").exists());
}

#[test]
fn fetch_stops_on_an_unsupported_manifest_version_before_touching_the_worktree() {
    let fx = Fixture::with_ledger(Some("version = 2\n"));

    let output = fx.tsk(&["ledger", "fetch"]);

    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("unsupported ledger version 2"),
        "{}",
        stderr(&output)
    );
    assert!(stdout(&output).is_empty());
    assert!(!fx.state().join("tsk").exists());
}

#[test]
fn fetch_stops_when_the_manifest_is_missing() {
    let fx = Fixture::with_ledger(None);

    let output = fx.tsk(&["ledger", "fetch"]);

    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("has no .tsk-ledger.toml"),
        "{}",
        stderr(&output)
    );
    assert!(!fx.state().join("tsk").exists());
}

#[test]
fn fetch_stops_on_a_supported_to_unsupported_upgrade_without_resetting() {
    let fx = Fixture::with_ledger(Some("version = 1\n"));
    let path = fx.fetch_ok();
    let before = head(&path);

    fx.add_ledger_commit(".tsk-ledger.toml", "version = 99\n");
    let output = fx.tsk(&["ledger", "fetch"]);

    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("unsupported ledger version 99"),
        "{}",
        stderr(&output)
    );
    assert_eq!(head(&path), before);
}

#[test]
fn fetch_fails_when_origin_has_no_ledger_branch() {
    let fx = Fixture::new();
    git(
        fx.root.path(),
        &["clone", "--quiet", fx.origin().to_str().unwrap(), "work"],
    );

    let output = fx.tsk(&["ledger", "fetch"]);

    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("refs/heads/tsk/ledger"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn fetch_reuses_an_existing_clone_id() {
    let fx = Fixture::with_ledger(Some("version = 1\n"));
    write(&fx.clone_id_file(), "custom-1234abcd\n");

    let path = fx.fetch_ok();

    assert_eq!(path, fx.state().join("tsk/repos/custom-1234abcd/ledger"));
    assert_eq!(
        std::fs::read_to_string(fx.clone_id_file()).unwrap(),
        "custom-1234abcd\n"
    );
}

#[test]
fn fetch_recreates_a_worktree_whose_directory_was_deleted() {
    let fx = Fixture::with_ledger(Some("version = 1\n"));
    let path = fx.fetch_ok();
    std::fs::remove_dir_all(&path).unwrap();

    let again = fx.fetch_ok();

    assert_eq!(again, path);
    assert_eq!(head(&again), fx.origin_ledger_sha());
}

#[test]
fn fetch_refuses_a_directory_that_is_not_a_worktree_of_this_clone() {
    let fx = Fixture::with_ledger(Some("version = 1\n"));
    write(&fx.clone_id_file(), "work-00000000\n");
    let path = fx.state().join("tsk/repos/work-00000000/ledger");
    std::fs::create_dir_all(&path).unwrap();
    write(&path.join("stray.txt"), "not a worktree\n");

    let output = fx.tsk(&["ledger", "fetch"]);

    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("not a worktree of this clone"),
        "{}",
        stderr(&output)
    );
    assert!(path.join("stray.txt").is_file());
}

#[test]
fn fetch_from_a_linked_worktree_uses_the_same_clone_id() {
    let fx = Fixture::with_ledger(Some("version = 1\n"));
    let from_main = fx.fetch_ok();

    git(
        &fx.work(),
        &["worktree", "add", "--quiet", "--detach", "../linked"],
    );
    let output = fx.tsk_in(&fx.root.path().join("linked"), &["ledger", "fetch"]);
    assert_success(&output);

    assert_eq!(PathBuf::from(stdout(&output).trim_end()), from_main);
}

#[test]
fn path_prints_the_fetched_worktree_path() {
    let fx = Fixture::with_ledger(Some("version = 1\n"));
    let fetched = fx.fetch_ok();

    let output = fx.tsk(&["ledger", "path"]);

    assert_success(&output);
    assert_eq!(stdout(&output), format!("{}\n", fetched.display()));
}

#[test]
fn path_does_not_refresh_the_worktree() {
    let fx = Fixture::with_ledger(Some("version = 1\n"));
    let path = fx.fetch_ok();
    let before = head(&path);
    fx.add_ledger_commit("later.md", "later\n");

    assert_success(&fx.tsk(&["ledger", "path"]));

    assert_eq!(head(&path), before);
    assert!(!path.join("later.md").exists());
}

#[test]
fn path_without_a_clone_id_fails_and_writes_nothing() {
    let fx = Fixture::with_ledger(Some("version = 1\n"));

    let output = fx.tsk(&["ledger", "path"]);

    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("tsk ledger fetch"),
        "{}",
        stderr(&output)
    );
    assert!(stdout(&output).is_empty());
    assert!(!fx.clone_id_file().exists());
    assert!(!fx.state().join("tsk").exists());
}

#[test]
fn path_with_a_clone_id_prints_the_path_before_any_fetch() {
    let fx = Fixture::with_ledger(Some("version = 1\n"));
    write(&fx.clone_id_file(), "work-abcdef12\n");

    let output = fx.tsk(&["ledger", "path"]);

    assert_success(&output);
    assert_eq!(
        stdout(&output),
        format!(
            "{}\n",
            fx.state().join("tsk/repos/work-abcdef12/ledger").display()
        )
    );
    assert!(!fx.state().join("tsk").exists());
}

#[test]
fn path_falls_back_to_home_when_xdg_state_home_is_empty() {
    let fx = Fixture::with_ledger(Some("version = 1\n"));
    write(&fx.clone_id_file(), "work-abcdef12\n");

    let output = Command::new(env!("CARGO_BIN_EXE_tsk"))
        .args(["ledger", "path"])
        .current_dir(fx.work())
        .envs(git_env())
        .env("HOME", fx.home())
        .env("XDG_STATE_HOME", "")
        .output()
        .unwrap();

    assert_success(&output);
    assert_eq!(
        stdout(&output),
        format!(
            "{}\n",
            fx.home()
                .join(".local/state/tsk/repos/work-abcdef12/ledger")
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
    ] {
        let output = fx.tsk_in(&outside, &command);
        assert!(!output.status.success());
        assert!(
            stderr(&output).contains("not inside a git repository"),
            "{}",
            stderr(&output)
        );
    }
}

fn is_ancestor(dir: &Path, ancestor: &str, descendant: &str) -> bool {
    git_output(dir, &["merge-base", "--is-ancestor", ancestor, descendant])
        .status
        .success()
}

fn origin_subject(fx: &Fixture) -> String {
    git_line(&fx.origin(), &["log", "-1", "--format=%s", LEDGER_REF])
}

#[test]
fn push_commits_every_change_and_pushes_it_to_the_ledger_branch() {
    let fx = Fixture::with_ledger(Some("version = 1\n"));
    let path = fx.fetch_ok();
    let before = fx.origin_ledger_sha();

    write(&path.join("index.md"), "# Ledger index\nedited\n");
    std::fs::create_dir_all(path.join("missions")).unwrap();
    write(&path.join("missions/new.md"), "new mission\n");

    let output = fx.push("Record the new mission");

    assert_success(&output);
    let tip = fx.origin_ledger_sha();
    assert_ne!(tip, before);
    assert_eq!(stdout(&output), format!("{}\n", tip));
    assert_eq!(head(&path), tip);
    assert_eq!(origin_subject(&fx), "Record the new mission");
    assert_eq!(
        git_line(&fx.origin(), &["rev-parse", &format!("{}^", LEDGER_REF)]),
        before
    );
    assert_eq!(
        fx.origin_file(LEDGER_REF, "index.md").unwrap(),
        "# Ledger index\nedited\n"
    );
    assert_eq!(
        fx.origin_file(LEDGER_REF, "missions/new.md").unwrap(),
        "new mission\n"
    );
    assert!(git(&path, &["status", "--porcelain=v2", "-z"]).is_empty());
}

#[test]
fn push_with_nothing_to_commit_pushes_nothing_and_exits_zero() {
    let fx = Fixture::with_ledger(Some("version = 1\n"));
    let path = fx.fetch_ok();
    let before = fx.origin_ledger_sha();

    let output = fx.push("Nothing changed");

    assert_success(&output);
    assert_eq!(fx.origin_ledger_sha(), before);
    assert_eq!(head(&path), before);
    assert_eq!(stdout(&output), format!("{}\n", before));
    assert!(
        stderr(&output).contains("nothing to commit"),
        "{}",
        stderr(&output)
    );
    assert!(
        stderr(&output).contains("already holds this worktree's state"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn push_with_nothing_to_commit_sends_a_commit_an_earlier_run_left_unpushed() {
    let fx = Fixture::with_ledger(Some("version = 1\n"));
    let path = fx.fetch_ok();

    write(&path.join("left.md"), "committed, not pushed\n");
    commit_all(&path, "Earlier run");
    let local = head(&path);

    let output = fx.push("Second run");

    assert_success(&output);
    assert!(
        stderr(&output).contains("nothing to commit"),
        "{}",
        stderr(&output)
    );
    assert_eq!(fx.origin_ledger_sha(), local);
    assert_eq!(origin_subject(&fx), "Earlier run");
}

#[test]
fn push_rebases_onto_a_commit_landed_since_the_worktree_was_fetched() {
    let fx = Fixture::with_ledger(Some("version = 1\n"));
    let path = fx.fetch_ok();
    let remote = fx.add_ledger_commit("remote.md", "remote\n");

    write(&path.join("local.md"), "local\n");
    let output = fx.push("Local change");

    assert_success(&output);
    let tip = fx.origin_ledger_sha();
    assert_eq!(head(&path), tip);
    assert!(is_ancestor(&fx.origin(), &remote, &tip));
    assert_eq!(fx.origin_file(LEDGER_REF, "remote.md").unwrap(), "remote\n");
    assert_eq!(fx.origin_file(LEDGER_REF, "local.md").unwrap(), "local\n");
}

#[test]
fn push_retries_when_a_concurrent_writer_lands_between_fetch_and_push() {
    let fx = Fixture::with_ledger(Some("version = 1\n"));
    let path = fx.fetch_ok();
    fx.install_racing_pre_push_hook(1);

    write(&path.join("local.md"), "local\n");
    let output = fx.push("Local change");

    assert_success(&output);
    assert!(
        stderr(&output).contains("push rejected (attempt 1/5)"),
        "{}",
        stderr(&output)
    );
    let tip = fx.origin_ledger_sha();
    assert_eq!(stdout(&output), format!("{}\n", tip));
    assert_eq!(origin_subject(&fx), "Local change");
    assert!(is_ancestor(&fx.origin(), &head(&fx.racer()), &tip));
    assert_eq!(fx.origin_file(LEDGER_REF, "race-1.md").unwrap(), "race 1\n");
    assert_eq!(fx.origin_file(LEDGER_REF, "local.md").unwrap(), "local\n");
}

#[test]
fn push_stops_after_the_retry_bound_without_overwriting_the_concurrent_writer() {
    let fx = Fixture::with_ledger(Some("version = 1\n"));
    let path = fx.fetch_ok();
    fx.install_racing_pre_push_hook(100);

    write(&path.join("local.md"), "local\n");
    let output = fx.push("Local change");

    assert!(!output.status.success());
    assert!(stdout(&output).is_empty());
    assert!(
        stderr(&output).contains("rejected the push 5 times"),
        "{}",
        stderr(&output)
    );
    assert!(
        stderr(&output).contains("push rejected (attempt 4/5)"),
        "{}",
        stderr(&output)
    );
    assert_eq!(fx.origin_ledger_sha(), head(&fx.racer()));
    assert_eq!(origin_subject(&fx), "Race 5");
    assert!(fx.origin_file(LEDGER_REF, "local.md").is_none());
    assert_eq!(
        std::fs::read_to_string(path.join("local.md")).unwrap(),
        "local\n"
    );
    assert_eq!(
        git_line(&path, &["log", "-1", "--format=%s", "HEAD"]),
        "Local change"
    );
}

#[test]
fn push_never_writes_the_colliding_custom_ref() {
    let fx = Fixture::with_ledger(Some("version = 1\n"));

    git(&fx.seed(), &["switch", "--quiet", "--orphan", "shadow"]);
    write(&fx.seed().join(".tsk-ledger.toml"), "version = 1\n");
    write(&fx.seed().join("shadow.md"), "orphaned custom ref\n");
    commit_all(&fx.seed(), "Shadow");
    git(
        &fx.seed(),
        &["push", "--quiet", "origin", "HEAD:refs/tsk/ledger"],
    );
    let shadow = git_line(&fx.origin(), &["rev-parse", "refs/tsk/ledger"]);

    let path = fx.fetch_ok();
    write(&path.join("local.md"), "local\n");
    let output = fx.push("Local change");

    assert_success(&output);
    assert_eq!(
        git_line(&fx.origin(), &["rev-parse", "refs/tsk/ledger"]),
        shadow
    );
    assert_eq!(origin_subject(&fx), "Local change");
    assert!(fx.origin_file(LEDGER_REF, "shadow.md").is_none());
    let refs = git(&fx.origin(), &["for-each-ref", "--format=%(refname)"]);
    let mut refs: Vec<&str> = refs.lines().collect();
    refs.sort();
    assert_eq!(
        refs,
        vec![
            "refs/heads/main",
            "refs/heads/tsk/ledger",
            "refs/tsk/ledger"
        ]
    );
}

#[test]
fn push_aborts_a_conflicting_rebase_and_leaves_the_remote_alone() {
    let fx = Fixture::with_ledger(Some("version = 1\n"));
    let path = fx.fetch_ok();
    let remote = fx.add_ledger_commit("index.md", "remote edit\n");

    write(&path.join("index.md"), "local edit\n");
    let output = fx.push("Local edit");

    assert!(!output.status.success());
    assert!(stdout(&output).is_empty());
    assert!(
        stderr(&output).contains("conflicted"),
        "{}",
        stderr(&output)
    );
    assert_eq!(fx.origin_ledger_sha(), remote);
    assert_eq!(
        git_line(&path, &["log", "-1", "--format=%s", "HEAD"]),
        "Local edit"
    );
    assert!(git(&path, &["status", "--porcelain=v2", "-z"]).is_empty());
    let git_dir = PathBuf::from(git_line(
        &path,
        &["rev-parse", "--path-format=absolute", "--git-dir"],
    ));
    assert!(!git_dir.join("rebase-merge").exists());
    assert!(!git_dir.join("rebase-apply").exists());
}

#[test]
fn push_without_a_worktree_fails_and_names_fetch() {
    let fx = Fixture::with_ledger(Some("version = 1\n"));
    let before = fx.origin_ledger_sha();

    let output = fx.push("No worktree");

    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("tsk ledger fetch"),
        "{}",
        stderr(&output)
    );
    assert!(!fx.clone_id_file().exists());
    assert_eq!(fx.origin_ledger_sha(), before);
}

#[test]
fn push_stops_on_an_unsupported_manifest_version_before_committing() {
    let fx = Fixture::with_ledger(Some("version = 1\n"));
    let path = fx.fetch_ok();
    let before = head(&path);
    let remote = fx.add_ledger_commit(".tsk-ledger.toml", "version = 99\n");

    write(&path.join("local.md"), "local\n");
    let output = fx.push("Local change");

    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("unsupported ledger version 99"),
        "{}",
        stderr(&output)
    );
    assert_eq!(head(&path), before);
    assert_eq!(fx.origin_ledger_sha(), remote);
    assert!(path.join("local.md").is_file());
}
