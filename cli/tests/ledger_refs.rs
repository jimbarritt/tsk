//! Which refs hold a ledger: branches under `refs/heads/*` or a custom ref (`refs/tsk/*`).
//! Each case drives the `tsk` binary against local bare repos as origin and as nexus.
//! Cases print a note and return when the `dolt` binary is not installed. The harness does
//! not set `TSK_LEDGER_REFS`, so the order of the sources is the one a user meets.

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use tempfile::TempDir;

const CLOUD_NOTE: &str = "This is a Claude Code cloud session, which cannot push a custom ref, so the ledger uses branches. To choose, run tsk config ledger-refs custom or branches before the first push.";

fn dolt_installed() -> bool {
    Command::new("dolt")
        .arg("version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

macro_rules! need_dolt {
    () => {
        if !dolt_installed() {
            eprintln!("skipped: dolt is not installed");
            return;
        }
    };
}

struct Fx {
    root: TempDir,
}

impl Fx {
    fn new() -> Fx {
        let fx = Fx {
            root: tempfile::tempdir().unwrap(),
        };
        let root = fx.root.path();
        for dir in ["home", "state", "config", "shim"] {
            std::fs::create_dir_all(root.join(dir)).unwrap();
        }
        bare_with_main(root, "origin");
        for name in ["work", "work2"] {
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

    fn nexus(&self) -> PathBuf {
        self.root.path().join("nexus.git")
    }

    fn work(&self) -> PathBuf {
        self.root.path().join("work")
    }

    fn work2(&self) -> PathBuf {
        self.root.path().join("work2")
    }

    fn command(&self, dir: &Path, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_tsk"));
        command
            .args(args)
            .current_dir(dir)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "test@example.com")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "test@example.com")
            .env("HOME", self.root.path().join("home"))
            .env("XDG_STATE_HOME", self.root.path().join("state"))
            .env("TSK_DOLT_SERVER_IDLE_SECS", "3")
            .env("XDG_CONFIG_HOME", self.root.path().join("config"))
            .env("TSK_HOME", self.root.path().join("home/.tsk"))
            .env("TSK_NO_UPDATE_CHECK", "1")
            .env("TSK_MACHINE_NAME", "laptop")
            .env("GIT_CEILING_DIRECTORIES", self.root.path())
            .env_remove("TSK_LEDGER_REFS")
            .env_remove("CLAUDE_CODE_REMOTE")
            .env_remove("CLAUDE_CODE_REMOTE_SESSION_ID");
        command
    }

    fn tsk(&self, dir: &Path, args: &[&str]) -> Output {
        self.command(dir, args).output().expect("failed to run tsk")
    }

    fn tsk_env(&self, dir: &Path, args: &[&str], env: &[(&str, &str)]) -> Output {
        let mut command = self.command(dir, args);
        for (key, value) in env {
            command.env(key, value);
        }
        command.output().expect("failed to run tsk")
    }

    fn ok(&self, dir: &Path, args: &[&str]) -> String {
        let output = self.tsk(dir, args);
        assert_success(&output);
        stdout(&output)
    }

    fn put(&self, dir: &Path, path: &str, body: &str) {
        let mut child = self
            .command(dir, &["ledger", "doc", "put", path])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        std::io::Write::write_all(&mut child.stdin.take().unwrap(), body.as_bytes()).unwrap();
        assert_success(&child.wait_with_output().unwrap());
    }

    /// Every ref of a bare repo, sorted.
    fn refs_of(&self, bare: &Path) -> Vec<String> {
        let out = git(bare, &["for-each-ref", "--format=%(refname)"]);
        let mut refs: Vec<String> = out.lines().map(str::to_string).collect();
        refs.sort();
        refs
    }

    fn origin_refs(&self) -> Vec<String> {
        self.refs_of(&self.origin())
    }

    /// A nexus holding one entry for the repo `work-api`, kept on this machine only so the
    /// local clone paths need no URL. `extra` is more JSON for the entry.
    fn with_nexus(&self, extra: &str) {
        let root = self.root.path();
        bare_with_main(root, "nexus");
        self.set_nexus_entry(extra);
        assert_success(&self.tsk(
            &self.work(),
            &["config", "attach-nexus", self.nexus().to_str().unwrap()],
        ));
        for dir in [self.work(), self.work2()] {
            std::fs::write(dir.join(".git/tsk-repo-id"), "work-api\n").unwrap();
        }
    }

    fn set_nexus_entry(&self, extra: &str) {
        let seed = self.root.path().join("nexus-seed");
        let text = format!(
            "{{\"version\":1,\"territories\":[{{\"id\":\"t\",\"name\":\"T\",\"repos\":[{{\"id\":\"work-api\",\"local\":\"laptop\",\"ledger\":\"nexus\"{}}}]}}]}}\n",
            extra
        );
        write(&seed.join("nexus.json"), &text);
        git(&seed, &["add", "-A"]);
        git(&seed, &["commit", "--quiet", "-m", "Set nexus.json"]);
        git(
            &seed,
            &["push", "--quiet", "origin", "HEAD:refs/heads/main"],
        );
    }

    /// The directory of a clone's ledger store, which exists once the store was made.
    fn store_dir(&self, work: &Path) -> PathBuf {
        let id = std::fs::read_to_string(work.join(".git/tsk-clone-id"))
            .unwrap()
            .trim()
            .to_string();
        self.root
            .path()
            .join("state/tsk/repos")
            .join(id)
            .join("ledger-db")
    }

    /// A pre-receive hook on a bare repo that refuses each ref matching one of `patterns`.
    fn refuse(&self, bare: &Path, patterns: &[&str]) {
        let cases = patterns.join("|");
        let script = format!(
            "#!/bin/sh\nwhile read old new ref; do\n  case \"$ref\" in\n    {}) echo \"refusing $ref\" >&2; exit 1;;\n  esac\ndone\n",
            cases
        );
        let hook = bare.join("hooks/pre-receive");
        write(&hook, &script);
        let mut mode = std::fs::metadata(&hook).unwrap().permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut mode, 0o755);
        std::fs::set_permissions(&hook, mode).unwrap();
    }
}

fn bare_with_main(root: &Path, name: &str) {
    let bare = root.join(format!("{}.git", name));
    let seed = root.join(format!("{}-seed", name));
    git(
        root,
        &[
            "init",
            "--quiet",
            "--bare",
            "-b",
            "main",
            bare.to_str().unwrap(),
        ],
    );
    git(
        root,
        &["init", "--quiet", "-b", "main", seed.to_str().unwrap()],
    );
    git(&seed, &["remote", "add", "origin", bare.to_str().unwrap()]);
    write(&seed.join("README.md"), "main\n");
    git(&seed, &["add", "-A"]);
    git(&seed, &["commit", "--quiet", "-m", "Main commit"]);
    git(
        &seed,
        &["push", "--quiet", "origin", "HEAD:refs/heads/main"],
    );
}

fn git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@example.com")
        .output()
        .expect("failed to run git");
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn write(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, text).unwrap();
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
        "exit {:?}\nstdout: {}\nstderr: {}",
        output.status.code(),
        stdout(output),
        stderr(output)
    );
}

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

// --- Round trips ----------------------------------------------------------------------

/// Fetch a new ledger, write a record and a document, push, then read both from a second
/// clone and push a change back.
fn round_trip(fx: &Fx, published: &[&str], bare: &Path) {
    let (a, b) = (fx.work(), fx.work2());
    assert_success(&fx.tsk(&a, &["ledger", "fetch"]));
    assert_success(&fx.tsk(
        &a,
        &[
            "mission",
            "create",
            "M-ONE",
            "First",
            "--category",
            "operational",
        ],
    ));
    fx.put(&a, "notes/one.md", "from a\n");
    assert_success(&fx.tsk(&a, &["ledger", "push", "A writes"]));
    assert_eq!(fx.refs_of(bare), strings(published));

    let fetched = fx.tsk(&b, &["ledger", "fetch"]);
    assert_success(&fetched);
    assert!(
        !stderr(&fetched).contains("does not exist yet"),
        "{}",
        stderr(&fetched)
    );
    assert!(fx.ok(&b, &["mission", "list"]).contains("M-ONE"));
    assert_eq!(
        fx.ok(&b, &["ledger", "doc", "get", "notes/one.md"]),
        "from a\n"
    );

    assert_success(&fx.tsk(
        &b,
        &[
            "mission",
            "create",
            "M-TWO",
            "Second",
            "--category",
            "operational",
        ],
    ));
    fx.put(&b, "notes/two.md", "from b\n");
    assert_success(&fx.tsk(&b, &["ledger", "push", "B writes"]));
    assert_success(&fx.tsk(&a, &["ledger", "fetch"]));
    let list = fx.ok(&a, &["mission", "list"]);
    assert!(list.contains("M-ONE") && list.contains("M-TWO"), "{}", list);
    assert_eq!(
        fx.ok(&a, &["ledger", "doc", "get", "notes/two.md"]),
        "from b\n"
    );
    assert!(fx.ok(&a, &["ledger", "status"]).contains("up to date"));
}

#[test]
fn a_new_ledger_takes_custom_refs_and_round_trips_between_clones() {
    need_dolt!();
    let fx = Fx::new();
    let shown = fx.ok(&fx.work(), &["config", "ledger-refs"]);
    assert_eq!(
        shown,
        "ledger refs: custom\nsource: default\nledger ref: refs/tsk/ledger\ndocs ref: refs/tsk/docs\n"
    );
    let fetched = fx.tsk(&fx.work(), &["ledger", "fetch"]);
    assert_success(&fetched);
    let note = stderr(&fetched);
    assert!(
        note.contains("origin's refs/tsk/ledger does not exist yet"),
        "{}",
        note
    );
    assert!(
        note.contains("The first tsk ledger push creates the ref"),
        "{}",
        note
    );
    assert!(!note.contains("cloud session"), "{}", note);
    round_trip(
        &fx,
        &["refs/heads/main", "refs/tsk/docs", "refs/tsk/ledger"],
        &fx.origin(),
    );
    // No branch other than main shows on the host.
    assert!(fx
        .origin_refs()
        .iter()
        .all(|r| !r.starts_with("refs/heads/") || r == "refs/heads/main"));
    // The second clone found the ledger on origin, and the first holds a store.
    let shown = fx.ok(&fx.work2(), &["config", "ledger-refs"]);
    assert!(shown.contains("source: ledger store"), "{}", shown);
}

#[test]
fn branches_chosen_in_the_repo_config_round_trip() {
    need_dolt!();
    let fx = Fx::new();
    let set = fx.ok(&fx.work(), &["config", "ledger-refs", "branches"]);
    assert!(set.contains("ledger refs: branches"), "{}", set);
    assert!(set.contains(".tsk/config.toml"), "{}", set);
    // The second clone reads the same file from the repo tree.
    let file = fx.work().join(".tsk/config.toml");
    std::fs::create_dir_all(fx.work2().join(".tsk")).unwrap();
    std::fs::copy(&file, fx.work2().join(".tsk/config.toml")).unwrap();
    round_trip(
        &fx,
        &[
            "refs/heads/main",
            "refs/heads/tsk/docs",
            "refs/heads/tsk/ledger",
        ],
        &fx.origin(),
    );
}

#[test]
fn a_clone_with_no_setting_finds_a_branch_ledger_on_origin() {
    need_dolt!();
    let fx = Fx::new();
    assert_success(&fx.tsk(&fx.work(), &["config", "ledger-refs", "branches"]));
    assert_success(&fx.tsk(&fx.work(), &["ledger", "fetch"]));
    assert_success(&fx.tsk(&fx.work(), &["ledger", "push", "publish"]));
    let shown = fx.ok(&fx.work2(), &["config", "ledger-refs"]);
    assert_eq!(
        shown,
        "ledger refs: branches\nsource: discovered on origin\nledger ref: refs/heads/tsk/ledger\ndocs ref: refs/heads/tsk/docs\n"
    );
    assert_success(&fx.tsk(&fx.work2(), &["ledger", "fetch"]));
}

fn nexus_round_trip(extra: &str, published: &[&str]) {
    let fx = Fx::new();
    fx.with_nexus(extra);
    round_trip(&fx, published, &fx.nexus());
    // The code repo's origin holds no ledger ref.
    assert_eq!(fx.origin_refs(), strings(&["refs/heads/main"]));
}

#[test]
fn a_nexus_ledger_round_trips_on_custom_refs_by_default() {
    need_dolt!();
    nexus_round_trip(
        "",
        &[
            "refs/heads/main",
            "refs/tsk/ledgers/work-api",
            "refs/tsk/ledgers/work-api-docs",
        ],
    );
}

#[test]
fn a_nexus_entry_sets_custom_refs() {
    need_dolt!();
    nexus_round_trip(
        ",\"ledger_refs\":\"custom\"",
        &[
            "refs/heads/main",
            "refs/tsk/ledgers/work-api",
            "refs/tsk/ledgers/work-api-docs",
        ],
    );
}

#[test]
fn a_nexus_entry_sets_branches() {
    need_dolt!();
    nexus_round_trip(
        ",\"ledger_refs\":\"branches\"",
        &[
            "refs/heads/ledgers/work-api",
            "refs/heads/ledgers/work-api-docs",
            "refs/heads/main",
        ],
    );
}

// --- The order of the sources ---------------------------------------------------------

#[test]
fn the_nexus_entry_beats_the_repo_config_and_the_environment_beats_both() {
    need_dolt!();
    let fx = Fx::new();
    fx.with_nexus(",\"ledger_refs\":\"branches\"");
    // The repo config says custom, and the nexus says branches.
    let set = fx.tsk(&fx.work(), &["config", "ledger-refs", "custom"]);
    assert_success(&set);
    assert!(
        stderr(&set).contains(
            "the nexus entry \"work-api\" sets ledger_refs to \"branches\", which takes precedence"
        ),
        "{}",
        stderr(&set)
    );
    let shown = fx.ok(&fx.work(), &["config", "ledger-refs"]);
    assert!(
        shown.starts_with("ledger refs: branches\nsource: nexus\n"),
        "{}",
        shown
    );
    assert!(
        shown.contains("ledger ref: refs/heads/ledgers/work-api"),
        "{}",
        shown
    );
    // The environment beats the nexus.
    let output = fx.tsk_env(
        &fx.work(),
        &["config", "ledger-refs"],
        &[("TSK_LEDGER_REFS", "custom")],
    );
    assert_success(&output);
    assert!(
        stdout(&output).starts_with("ledger refs: custom\nsource: environment\n"),
        "{}",
        stdout(&output)
    );
    assert!(stdout(&output).contains("ledger ref: refs/tsk/ledgers/work-api\n"));
    // A value that is neither mode is an error that names it.
    let bad = fx.tsk_env(
        &fx.work(),
        &["config", "ledger-refs"],
        &[("TSK_LEDGER_REFS", "tags")],
    );
    assert_eq!(bad.status.code(), Some(1));
    assert!(
        stderr(&bad).contains("\"branches\" and \"custom\""),
        "{}",
        stderr(&bad)
    );
}

#[test]
fn the_repo_config_is_read_when_no_nexus_is_attached() {
    need_dolt!();
    let fx = Fx::new();
    // Origin holds a custom ledger. A clone whose repo config says custom follows it.
    assert_success(&fx.tsk(&fx.work(), &["ledger", "fetch"]));
    assert_success(&fx.tsk(&fx.work(), &["ledger", "push", "publish"]));
    assert_success(&fx.tsk(&fx.work2(), &["config", "ledger-refs", "custom"]));
    let shown = fx.ok(&fx.work2(), &["config", "ledger-refs"]);
    assert!(
        shown.starts_with("ledger refs: custom\nsource: repo config ("),
        "{}",
        shown
    );
    assert_success(&fx.tsk(&fx.work2(), &["ledger", "fetch"]));
}

#[test]
fn discovery_asks_origin_once_and_caches_the_answer() {
    need_dolt!();
    let fx = Fx::new();
    assert_success(&fx.tsk(&fx.work(), &["ledger", "fetch"]));
    assert_success(&fx.tsk(&fx.work(), &["ledger", "push", "publish"]));
    // work2 has no store yet: the first command asks, the cache answers the next.
    let first = fx.ok(&fx.work2(), &["config", "ledger-refs"]);
    assert!(first.contains("source: discovered on origin"), "{}", first);
    let id = std::fs::read_to_string(fx.work2().join(".git/tsk-clone-id")).unwrap();
    let cache = fx
        .root
        .path()
        .join("state/tsk/repos")
        .join(id.trim())
        .join("ledger-refs");
    assert_eq!(
        std::fs::read_to_string(&cache).unwrap(),
        "origin\t-\tcustom\n"
    );
    // Origin goes away; the cached answer still serves.
    std::fs::rename(fx.origin(), fx.root.path().join("origin-moved.git")).unwrap();
    let second = fx.ok(&fx.work2(), &["config", "ledger-refs"]);
    assert!(
        second.contains("source: discovered on origin"),
        "{}",
        second
    );
    std::fs::rename(fx.root.path().join("origin-moved.git"), fx.origin()).unwrap();
    // Setting the mode forgets the cache.
    assert_success(&fx.tsk(&fx.work2(), &["config", "ledger-refs", "custom"]));
    assert!(!cache.exists());
}

// --- A Claude Code cloud session ------------------------------------------------------

#[test]
fn a_new_ledger_in_a_cloud_session_takes_branches_and_says_so() {
    need_dolt!();
    let fx = Fx::new();
    let cloud = [("CLAUDE_CODE_REMOTE", "true")];
    let shown = fx.tsk_env(&fx.work(), &["config", "ledger-refs"], &cloud);
    assert_success(&shown);
    assert_eq!(
        stdout(&shown),
        "ledger refs: branches\nsource: default (Claude Code cloud session)\nledger ref: refs/heads/tsk/ledger\ndocs ref: refs/heads/tsk/docs\n"
    );
    let fetched = fx.tsk_env(&fx.work(), &["ledger", "fetch"], &cloud);
    assert_success(&fetched);
    assert!(
        stderr(&fetched).contains(CLOUD_NOTE),
        "{}",
        stderr(&fetched)
    );
    assert!(stderr(&fetched).contains("The first tsk ledger push creates the branch"));
    assert_success(&fx.tsk_env(&fx.work(), &["ledger", "push", "publish"], &cloud));
    assert_eq!(
        fx.origin_refs(),
        strings(&[
            "refs/heads/main",
            "refs/heads/tsk/docs",
            "refs/heads/tsk/ledger"
        ])
    );
    // Once the ledger exists the sentence is gone.
    let again = fx.tsk_env(&fx.work(), &["ledger", "fetch"], &cloud);
    assert_success(&again);
    assert!(!stderr(&again).contains(CLOUD_NOTE));
}

#[test]
fn the_session_start_context_of_a_new_cloud_ledger_carries_the_sentence() {
    need_dolt!();
    let fx = Fx::new();
    let output = fx.tsk_env(
        &fx.work(),
        &["thread", "session-start"],
        &[("CLAUDE_CODE_REMOTE", "true")],
    );
    assert_success(&output);
    assert!(stdout(&output).contains(CLOUD_NOTE), "{}", stdout(&output));
    // Outside a cloud session the context has no such sentence.
    let local = fx.tsk(&fx.work2(), &["thread", "session-start"]);
    assert_success(&local);
    assert!(!stdout(&local).contains("cloud session, which cannot"));
}

#[test]
fn an_explicit_choice_in_a_cloud_session_is_kept() {
    need_dolt!();
    let fx = Fx::new();
    assert_success(&fx.tsk(&fx.work(), &["config", "ledger-refs", "custom"]));
    let fetched = fx.tsk_env(
        &fx.work(),
        &["ledger", "fetch"],
        &[("CLAUDE_CODE_REMOTE", "true")],
    );
    assert_success(&fetched);
    assert!(!stderr(&fetched).contains(CLOUD_NOTE));
    assert!(stderr(&fetched).contains("origin's refs/tsk/ledger does not exist yet"));
}

// --- Mismatch -------------------------------------------------------------------------

#[test]
fn custom_refs_set_against_a_branch_ledger_stop_before_a_second_ledger() {
    need_dolt!();
    let fx = Fx::new();
    assert_success(&fx.tsk(&fx.work(), &["config", "ledger-refs", "branches"]));
    assert_success(&fx.tsk(&fx.work(), &["ledger", "fetch"]));
    assert_success(&fx.tsk(&fx.work(), &["ledger", "push", "publish"]));
    let before = fx.origin_refs();

    assert_success(&fx.tsk(&fx.work2(), &["config", "ledger-refs", "custom"]));
    let output = fx.tsk(&fx.work2(), &["ledger", "fetch"]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        stderr(&output),
        "error: this repo is set to custom ledger refs (repo config), which use refs/tsk/ledger, but origin holds the ledger on refs/heads/tsk/ledger.\n       tsk does not move a ledger between branches and a custom ref, and it does not start a second ledger.\n       To follow origin, run: tsk config ledger-refs branches\n"
    );
    assert!(!fx.store_dir(&fx.work2()).exists());
    assert_eq!(fx.origin_refs(), before);

    // The fix the error names works.
    assert_success(&fx.tsk(&fx.work2(), &["config", "ledger-refs", "branches"]));
    assert_success(&fx.tsk(&fx.work2(), &["ledger", "fetch"]));
}

#[test]
fn branches_set_against_a_custom_ledger_stop_before_a_second_ledger() {
    need_dolt!();
    let fx = Fx::new();
    assert_success(&fx.tsk(&fx.work(), &["ledger", "fetch"]));
    assert_success(&fx.tsk(&fx.work(), &["ledger", "push", "publish"]));
    let before = fx.origin_refs();

    assert_success(&fx.tsk(&fx.work2(), &["config", "ledger-refs", "branches"]));
    let output = fx.tsk(&fx.work2(), &["ledger", "fetch"]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        stderr(&output),
        "error: this repo is set to branches ledger refs (repo config), which use refs/heads/tsk/ledger, but origin holds the ledger on refs/tsk/ledger.\n       tsk does not move a ledger between branches and a custom ref, and it does not start a second ledger.\n       To follow origin, run: tsk config ledger-refs custom\n"
    );
    assert_eq!(fx.origin_refs(), before);
}

#[test]
fn the_nexus_mismatch_names_the_entry_as_the_place_to_change() {
    need_dolt!();
    let fx = Fx::new();
    fx.with_nexus("");
    assert_success(&fx.tsk(&fx.work(), &["ledger", "fetch"]));
    assert_success(&fx.tsk(&fx.work(), &["ledger", "push", "publish"]));
    fx.set_nexus_entry(",\"ledger_refs\":\"branches\"");
    let output = fx.tsk(&fx.work2(), &["ledger", "fetch"]);
    assert_eq!(output.status.code(), Some(1));
    let text = stderr(&output);
    assert!(text.contains("(nexus)"), "{}", text);
    assert!(text.contains("refs/tsk/ledgers/work-api"), "{}", text);
    assert!(
        text.contains("set \"ledger_refs\": \"custom\" on the repo's entry in nexus.json"),
        "{}",
        text
    );
}

#[test]
fn a_published_ledger_store_cannot_switch_and_says_which_refs_it_is_on() {
    need_dolt!();
    let fx = Fx::new();
    assert_success(&fx.tsk(&fx.work(), &["ledger", "fetch"]));
    assert_success(&fx.tsk(&fx.work(), &["ledger", "push", "publish"]));
    let output = fx.tsk(&fx.work(), &["config", "ledger-refs", "branches"]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        stderr(&output),
        "error: origin holds this ledger on refs/tsk/ledger, so it cannot use branches ledger refs.\n       tsk does not move a ledger between branches and a custom ref, and it does not start a second ledger.\n"
    );
    assert!(!fx.work().join(".tsk/config.toml").exists());

    // A mode that comes from the environment disagrees with the store the same way.
    let status = fx.tsk_env(
        &fx.work(),
        &["ledger", "status"],
        &[("TSK_LEDGER_REFS", "branches")],
    );
    assert_eq!(status.status.code(), Some(1));
    assert!(
        stderr(&status)
            .contains("the ledger store on this machine holds the ledger on refs/tsk/ledger"),
        "{}",
        stderr(&status)
    );
    assert!(stderr(&status).contains("set TSK_LEDGER_REFS to custom, or unset it"));
}

// --- A host that refuses a custom ref -------------------------------------------------

#[test]
fn a_refused_custom_push_names_the_ref_and_the_fix_and_keeps_the_local_commits() {
    need_dolt!();
    let fx = Fx::new();
    fx.refuse(&fx.origin(), &["refs/tsk/*"]);
    let work = fx.work();
    assert_success(&fx.tsk(&work, &["ledger", "fetch"]));
    assert_success(&fx.tsk(
        &work,
        &[
            "mission",
            "create",
            "M-KEEP",
            "Keep",
            "--category",
            "operational",
        ],
    ));
    let output = fx.tsk(&work, &["ledger", "push", "try custom"]);
    assert_eq!(output.status.code(), Some(1));
    let text = stderr(&output);
    assert!(
        text.starts_with("error: origin refused the push to refs/tsk/docs. The host may accept only branches, as the Claude Code cloud proxy does."),
        "{}",
        text
    );
    assert!(
        text.contains("The server said: [remote rejected] (pre-receive hook declined)"),
        "{}",
        text
    );
    assert!(text.ends_with("The local commits stay. To use branches for this ledger, run: tsk config ledger-refs branches\n"), "{}", text);
    assert!(!text.contains("rejected the push 5 times"), "{}", text);
    // No ref reached origin, and the record is still in the store.
    assert_eq!(fx.origin_refs(), strings(&["refs/heads/main"]));
    assert!(fx.ok(&work, &["mission", "list"]).contains("M-KEEP"));

    // The fix works with nothing lost.
    assert_success(&fx.tsk(&work, &["config", "ledger-refs", "branches"]));
    assert_success(&fx.tsk(&work, &["ledger", "push", "try branches"]));
    assert_eq!(
        fx.origin_refs(),
        strings(&[
            "refs/heads/main",
            "refs/heads/tsk/docs",
            "refs/heads/tsk/ledger"
        ])
    );
    assert_success(&fx.tsk(&fx.work2(), &["ledger", "fetch"]));
    assert!(fx.ok(&fx.work2(), &["mission", "list"]).contains("M-KEEP"));
}

#[test]
fn a_refused_custom_data_push_names_the_data_ref() {
    need_dolt!();
    let fx = Fx::new();
    fx.refuse(&fx.origin(), &["refs/tsk/ledger"]);
    let work = fx.work();
    assert_success(&fx.tsk(&work, &["ledger", "fetch"]));
    assert_success(&fx.tsk(
        &work,
        &[
            "mission",
            "create",
            "M-KEEP",
            "Keep",
            "--category",
            "operational",
        ],
    ));
    let output = fx.tsk(&work, &["ledger", "push", "try custom"]);
    assert_eq!(output.status.code(), Some(1));
    let text = stderr(&output);
    assert!(
        text.starts_with("error: origin refused the push to refs/tsk/ledger. The host may accept only branches, as the Claude Code cloud proxy does."),
        "{}",
        text
    );
    assert!(text.contains("tsk config ledger-refs branches"), "{}", text);
    assert!(fx.ok(&work, &["mission", "list"]).contains("M-KEEP"));
}

// --- The config command ---------------------------------------------------------------

#[test]
fn ledger_refs_writes_the_repo_config_and_keeps_other_keys() {
    let fx = Fx::new();
    let file = fx.work().join(".tsk/config.toml");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, "colour = \"blue\"\n").unwrap();
    let output = fx.tsk(&fx.work(), &["config", "ledger-refs", "branches"]);
    assert_success(&output);
    let printed = stdout(&output);
    assert!(
        printed.contains(&format!(
            "wrote {}",
            fx.work()
                .canonicalize()
                .unwrap()
                .join(".tsk/config.toml")
                .display()
        )) || printed.contains("wrote ") && printed.contains(".tsk/config.toml"),
        "{}",
        printed
    );
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(text.contains("colour = \"blue\""), "{}", text);
    assert!(
        text.contains("[ledger]") && text.contains("refs = \"branches\""),
        "{}",
        text
    );
    let shown = fx.ok(&fx.work(), &["config", "ledger-refs"]);
    assert!(
        shown.starts_with("ledger refs: branches\nsource: repo config ("),
        "{}",
        shown
    );
    // Another word is refused before anything is written.
    let bad = fx.tsk(&fx.work(), &["config", "ledger-refs", "tags"]);
    assert_eq!(bad.status.code(), Some(2));
    assert!(std::fs::read_to_string(&file).unwrap().contains("branches"));
}

#[test]
fn ledger_refs_outside_a_repo_is_a_plain_error() {
    let fx = Fx::new();
    let outside = fx.root.path().join("home");
    let output = fx.tsk(&outside, &["config", "ledger-refs"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr(&output).contains("not inside a git repository"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn a_second_writer_merges_onto_a_custom_ref_it_did_not_fetch() {
    need_dolt!();
    let fx = Fx::new();
    let (a, b) = (fx.work(), fx.work2());
    assert_success(&fx.tsk(&a, &["ledger", "fetch"]));
    assert_success(&fx.tsk(&a, &["ledger", "push", "publish"]));
    assert_success(&fx.tsk(&b, &["ledger", "fetch"]));
    // A publishes a record and a document while B holds its own, unpushed.
    assert_success(&fx.tsk(
        &a,
        &[
            "mission",
            "create",
            "M-AAA",
            "A's",
            "--category",
            "operational",
        ],
    ));
    fx.put(&a, "notes/a.md", "from a\n");
    assert_success(&fx.tsk(&a, &["ledger", "push", "A writes"]));
    assert_success(&fx.tsk(
        &b,
        &[
            "mission",
            "create",
            "M-BBB",
            "B's",
            "--category",
            "operational",
        ],
    ));
    fx.put(&b, "notes/b.md", "from b\n");
    assert_success(&fx.tsk(&b, &["ledger", "push", "B writes"]));
    assert_success(&fx.tsk(&a, &["ledger", "fetch"]));
    let list = fx.ok(&a, &["mission", "list"]);
    assert!(list.contains("M-AAA") && list.contains("M-BBB"), "{}", list);
    assert_eq!(
        fx.ok(&a, &["ledger", "doc", "get", "notes/b.md"]),
        "from b\n"
    );
    assert_eq!(
        fx.ok(&b, &["ledger", "doc", "get", "notes/a.md"]),
        "from a\n"
    );
    assert_eq!(
        fx.origin_refs(),
        strings(&["refs/heads/main", "refs/tsk/docs", "refs/tsk/ledger"])
    );
}

// --- Refs are spelled in full ---------------------------------------------------------

/// A `git` on the path that logs its arguments, then runs the real one. Dolt runs `git`
/// for its remote, so the log holds those calls too.
fn install_logging_git(fx: &Fx) -> (PathBuf, PathBuf) {
    let real = String::from_utf8(
        Command::new("sh")
            .args(["-c", "command -v git"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    let log = fx.root.path().join("git-calls.log");
    let shim = fx.root.path().join("shim/git");
    write(
        &shim,
        &format!(
            "#!/bin/sh\nfor a in \"$@\"; do printf '%s\\n' \"$a\" >> '{}'; done\nexec {} \"$@\"\n",
            log.display(),
            real.trim()
        ),
    );
    let mut mode = std::fs::metadata(&shim).unwrap().permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut mode, 0o755);
    std::fs::set_permissions(&shim, mode).unwrap();
    (fx.root.path().join("shim"), log)
}

/// Every word in `log` that names a ledger or documents ref must be the full ref, which
/// starts with `refs/`, or a path.
fn short_refs(log: &str) -> Vec<String> {
    let mut bad = Vec::new();
    for line in log.lines() {
        for word in line.split([':', '=', '+', ' ']) {
            let names_ledger = word.contains("tsk/ledger")
                || word.contains("tsk/docs")
                || word.contains("ledgers/");
            if names_ledger
                && !word.starts_with("refs/")
                && !word.starts_with('/')
                && !word.contains("://")
            {
                bad.push(line.to_string());
            }
        }
    }
    bad
}

fn full_spelling(extra_nexus: Option<&str>, expect: &str) {
    let fx = Fx::new();
    if let Some(extra) = extra_nexus {
        fx.with_nexus(extra);
    }
    let (shim, log) = install_logging_git(&fx);
    let path = format!(
        "{}:{}",
        shim.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let env = [("PATH", path.as_str())];
    let (a, b) = (fx.work(), fx.work2());
    for (dir, args) in [
        (&a, vec!["ledger", "fetch"]),
        (
            &a,
            vec![
                "mission",
                "create",
                "M-ONE",
                "First",
                "--category",
                "operational",
            ],
        ),
        (&a, vec!["ledger", "push", "A"]),
        (&b, vec!["ledger", "fetch"]),
        (&b, vec!["ledger", "status"]),
        (&a, vec!["ledger", "status"]),
    ] {
        assert_success(&fx.tsk_env(dir, &args, &env));
    }
    let calls = std::fs::read_to_string(&log).unwrap();
    assert!(
        calls.contains(expect),
        "the log has no {}:\n{}",
        expect,
        calls
    );
    assert_eq!(short_refs(&calls), Vec::<String>::new(), "{}", calls);
}

#[test]
fn git_is_given_full_ref_names_for_a_repos_own_custom_ledger() {
    need_dolt!();
    full_spelling(None, "refs/tsk/ledger");
}

#[test]
fn git_is_given_full_ref_names_for_a_nexus_custom_ledger() {
    need_dolt!();
    full_spelling(Some(""), "refs/tsk/ledgers/work-api-docs");
}

#[test]
fn the_short_ref_check_catches_a_short_name() {
    assert_eq!(
        short_refs("fetch\norigin\nrefs/tsk/ledger\n"),
        Vec::<String>::new()
    );
    assert_eq!(
        short_refs("fetch\norigin\ntsk/ledger\n"),
        vec!["tsk/ledger".to_string()]
    );
    assert_eq!(
        short_refs("HEAD:tsk/docs\n"),
        vec!["HEAD:tsk/docs".to_string()]
    );
    assert_eq!(
        short_refs("--force-with-lease=refs/tsk/docs:abc\n"),
        Vec::<String>::new()
    );
}
