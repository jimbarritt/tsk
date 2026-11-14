//! `tsk harness`: new, list, path, cd, sleep, wake and delete. tmux is a fake script that
//! records its arguments and answers `has-session` from a list of session names. The tests
//! pass `--no-attach`, and most pass `--no-claude`.

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use tempfile::TempDir;

use crate::common;

use common::templates::{self, Seed, Step};

const BRIEFING: &str = "missions/operational/M-TEST-01-example.1c0de060.md";
const SESSION_VAR: &str = "CLAUDE_CODE_REMOTE_SESSION_ID";

/// The ledger that the thread tests use: one mission, M-TEST-01. The template is shared.
const SEED: Seed = Seed {
    branch: "main",
    clones: &["work", "work2"],
    refs: Some("branches"),
    steps: &[
        Step::Tsk("work", &["ledger", "fetch"]),
        Step::Stdin(
            "work",
            &["ledger", "doc", "put", BRIEFING],
            "# Mission: example\n",
        ),
        Step::Tsk("work", &["ledger", "push", "Seed the ledger"]),
    ],
};

const FAKE_TMUX: &str = r#"#!/bin/sh
echo "$@" >> "$FAKE_TMUX_LOG"
case "$1" in
  has-session)
    name="${3#=}"
    if grep -qx "$name" "$FAKE_TMUX_SESSIONS" 2>/dev/null; then exit 0; else exit 1; fi
    ;;
  new-session)
    echo "$4" >> "$FAKE_TMUX_SESSIONS"
    ;;
  kill-session)
    name="${3#=}"
    grep -vx "$name" "$FAKE_TMUX_SESSIONS" > "$FAKE_TMUX_SESSIONS.tmp"
    mv "$FAKE_TMUX_SESSIONS.tmp" "$FAKE_TMUX_SESSIONS"
    ;;
esac
exit 0
"#;

struct Fixture {
    root: TempDir,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        common::stop_servers(self.root.path(), false);
    }
}

impl Fixture {
    fn new() -> Fixture {
        use std::os::unix::fs::PermissionsExt;
        let fx = Fixture {
            root: templates::instance(&SEED),
        };
        let script = fx.root.path().join("fake-tmux.sh");
        std::fs::write(&script, FAKE_TMUX).unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::write(fx.sessions_file(), "").unwrap();
        std::fs::write(fx.tmux_log(), "").unwrap();
        fx
    }

    fn work(&self) -> PathBuf {
        self.root.path().join("work")
    }

    fn home(&self) -> PathBuf {
        self.root.path().join("home")
    }

    /// The worktrees root the tests use, under the home directory.
    fn trees(&self) -> PathBuf {
        self.home().join("trees")
    }

    fn sessions_file(&self) -> PathBuf {
        self.root.path().join("tmux-sessions")
    }

    fn tmux_log(&self) -> PathBuf {
        self.root.path().join("tmux.log")
    }

    fn tmux_lines(&self) -> Vec<String> {
        std::fs::read_to_string(self.tmux_log())
            .unwrap()
            .lines()
            .map(str::to_string)
            .collect()
    }

    fn sessions(&self) -> Vec<String> {
        std::fs::read_to_string(self.sessions_file())
            .unwrap()
            .lines()
            .map(str::to_string)
            .collect()
    }

    fn clone_id(&self) -> String {
        std::fs::read_to_string(self.work().join(".git").join("tsk-clone-id"))
            .unwrap()
            .trim()
            .to_string()
    }

    fn state_file(&self, name: &str) -> PathBuf {
        self.root
            .path()
            .join("state")
            .join("tsk")
            .join("harnesses")
            .join(self.clone_id())
            .join(format!("{}.json", name))
    }

    fn names_file(&self) -> PathBuf {
        self.root
            .path()
            .join("state")
            .join("tsk")
            .join("repos")
            .join(self.clone_id())
            .join("harness-names")
    }

    fn state(&self, name: &str) -> serde_json::Value {
        json(&std::fs::read_to_string(self.state_file(name)).unwrap())
    }

    /// A tsk command in `dir`. `root` is the value of GIT_WORKTREES_ROOT, when there is one.
    fn command_in(&self, dir: &Path, args: &[&str], root: Option<&Path>) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_tsk"));
        command
            .args(args)
            .current_dir(dir)
            .envs(git_env())
            .env("HOME", self.home())
            .env("XDG_STATE_HOME", self.root.path().join("state"))
            .env("XDG_CONFIG_HOME", self.home().join("config"))
            .env("TSK_HOME", self.home().join(".tsk"))
            .env("TSK_DOLT_SERVER_IDLE_SECS", "3")
            .env("TSK_BACKGROUND_PUBLISH", "off")
            .env("TSK_NO_UPDATE_CHECK", "1")
            .env("TSK_NO_PLUGIN_UPDATE", "1")
            .env("TSK_LEDGER_REFS", "branches")
            .env("GIT_CEILING_DIRECTORIES", self.root.path())
            .env("TSK_TMUX_BIN", self.root.path().join("fake-tmux.sh"))
            .env("FAKE_TMUX_LOG", self.tmux_log())
            .env("FAKE_TMUX_SESSIONS", self.sessions_file())
            .env_remove("TMUX")
            .env_remove("CLAUDE_CODE_REMOTE")
            .env_remove(SESSION_VAR)
            .env_remove("GIT_WORKTREES_ROOT")
            .stdin(Stdio::null());
        if let Some(root) = root {
            command.env("GIT_WORKTREES_ROOT", root);
        }
        command
    }

    fn tsk(&self, args: &[&str]) -> Output {
        let trees = self.trees();
        self.command_in(&self.work(), args, Some(&trees))
            .output()
            .expect("failed to run tsk")
    }

    fn tsk_no_root(&self, args: &[&str]) -> Output {
        self.command_in(&self.work(), args, None)
            .output()
            .expect("failed to run tsk")
    }

    fn tsk_at(&self, dir: &Path, args: &[&str]) -> Output {
        let trees = self.trees();
        self.command_in(dir, args, Some(&trees))
            .output()
            .expect("failed to run tsk")
    }

    /// `tsk harness new --robot Marvin --mission <mission> --name <name> --no-claude
    /// --no-attach`, which must succeed.
    fn new_harness(&self, name: &str, mission: &str) -> PathBuf {
        let output = self.tsk(&[
            "harness",
            "new",
            "--robot",
            "Marvin",
            "--mission",
            mission,
            "--name",
            name,
            "--no-claude",
            "--no-attach",
        ]);
        assert_success(&output);
        self.trees().join("work").join(name)
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

fn git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
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
    String::from_utf8(output.stdout)
        .unwrap()
        .trim_end()
        .to_string()
}

fn json(text: &str) -> serde_json::Value {
    serde_json::from_str(text.trim_end()).unwrap_or_else(|e| panic!("{}: {}", e, text))
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

fn assert_failure(output: &Output) -> String {
    assert_eq!(
        output.status.code(),
        Some(1),
        "expected exit 1\nstdout: {}\nstderr: {}",
        stdout(output),
        stderr(output)
    );
    stderr(output)
}

#[test]
fn the_worktrees_root_is_the_environment_then_the_config_and_new_fails_with_neither() {
    let fx = Fixture::new();
    let new = [
        "harness",
        "new",
        "--robot",
        "Marvin",
        "--mission",
        "M-TEST-01",
        "--name",
        "fox",
        "--no-claude",
        "--no-attach",
    ];

    // Neither: the command fails and names both.
    let err = assert_failure(&fx.tsk_no_root(&new));
    assert!(err.contains("GIT_WORKTREES_ROOT"), "{}", err);
    assert!(err.contains("tsk config worktrees-root"), "{}", err);
    let shown = stdout(&fx.tsk_no_root(&["config", "worktrees-root"]));
    assert!(shown.contains("not set"), "{}", shown);

    // The config alone.
    let from_config = fx.root.path().join("config-trees");
    let written = fx.tsk_no_root(&["config", "worktrees-root", from_config.to_str().unwrap()]);
    assert_success(&written);
    let shown = stdout(&fx.tsk_no_root(&["config", "worktrees-root"]));
    assert!(
        shown.contains(&format!("worktrees root: {}", from_config.display())),
        "{}",
        shown
    );
    assert!(shown.contains("source: user config"), "{}", shown);
    assert_success(&fx.tsk_no_root(&new));
    assert!(from_config.join("work").join("fox").is_dir());

    // The environment wins over the config.
    let from_env = fx.root.path().join("env-trees");
    let shown = stdout(
        &fx.command_in(&fx.work(), &["config", "worktrees-root"], Some(&from_env))
            .output()
            .unwrap(),
    );
    assert!(
        shown.contains("source: environment (GIT_WORKTREES_ROOT)"),
        "{}",
        shown
    );
    let mut second = new.to_vec();
    second[7] = "owl";
    let output = fx
        .command_in(&fx.work(), &second, Some(&from_env))
        .output()
        .unwrap();
    assert_success(&output);
    assert!(from_env.join("work").join("owl").is_dir());

    // An empty environment value counts as unset, so the config applies again.
    let mut third = new.to_vec();
    third[7] = "elk";
    let output = fx
        .command_in(&fx.work(), &third, Some(Path::new("")))
        .output()
        .unwrap();
    assert_success(&output);
    assert!(from_config.join("work").join("elk").is_dir());
}

#[test]
fn new_makes_the_worktree_the_branch_the_state_and_the_tmux_session() {
    let fx = Fixture::new();
    // An ignored env file is copied. A file that is not ignored is not.
    std::fs::write(fx.work().join(".gitignore"), ".env\n").unwrap();
    std::fs::write(fx.work().join(".env"), "KEY=1\n").unwrap();
    std::fs::write(fx.work().join(".env.local"), "KEY=2\n").unwrap();

    let output = fx.tsk(&[
        "harness",
        "new",
        "--robot",
        "Marvin",
        "--mission",
        "M-TEST-01",
        "--name",
        "otter",
        "--no-claude",
        "--no-attach",
    ]);
    assert_success(&output);
    let out = stdout(&output);
    let path = fx.trees().join("work").join("otter");
    assert!(out.contains("harness: otter"), "{}", out);
    assert!(
        out.contains(&format!("path: {}", path.display())),
        "{}",
        out
    );
    assert!(out.contains("tmux session: M-TEST-01"), "{}", out);

    // The worktree is on a new branch from origin's default branch.
    assert!(path.join("README.md").is_file());
    assert_eq!(
        git(&path, &["rev-parse", "--abbrev-ref", "HEAD"]),
        "harness/otter"
    );
    assert_eq!(
        git(&path, &["rev-parse", "HEAD"]),
        git(&fx.work(), &["rev-parse", "origin/main"])
    );
    assert_eq!(
        std::fs::read_to_string(path.join(".env")).unwrap(),
        "KEY=1\n"
    );
    assert!(!path.join(".env.local").exists());
    // The worktree has the terminal instance key <clone-id>/<name>.
    let git_dir = git(&path, &["rev-parse", "--git-dir"]);
    assert!(git_dir.ends_with("/worktrees/otter"), "{}", git_dir);

    // The state file and the used names.
    let state = fx.state("otter");
    assert_eq!(state["name"], "otter");
    assert_eq!(state["worktree"], path.to_str().unwrap());
    assert_eq!(state["branch"], "harness/otter");
    assert_eq!(state["main_worktree"], fx.work().to_str().unwrap());
    assert_eq!(state["tmux_session"], "M-TEST-01");
    assert_eq!(state["robot"], "Marvin");
    assert_eq!(state["mission"], "M-TEST-01");
    assert!(state["thread_id"].is_null());
    assert!(state["created_at"].as_str().unwrap().ends_with('Z'));
    assert_eq!(std::fs::read_to_string(fx.names_file()).unwrap(), "otter\n");

    // tmux: a detached session in the worktree, the harness option, and no claude.
    let log = fx.tmux_lines();
    assert!(
        log.contains(&format!(
            "new-session -d -s M-TEST-01 -c {}",
            path.display()
        )),
        "{:?}",
        log
    );
    assert!(
        log.contains(&"set-option -t =M-TEST-01: @tsk_harness otter".to_string()),
        "{:?}",
        log
    );
    assert!(!log.iter().any(|l| l.starts_with("send-keys")), "{:?}", log);
}

#[test]
fn new_starts_claude_with_a_first_prompt_and_names_the_session_after_the_harness_without_a_mission_code(
) {
    let fx = Fixture::new();
    let output = fx.tsk(&[
        "harness",
        "new",
        "--robot",
        "Marvin",
        "--mission",
        "M-TEST-01",
        "--name",
        "fox",
        "--no-attach",
    ]);
    assert_success(&output);
    let log = fx.tmux_lines();
    let keys: Vec<&String> = log.iter().filter(|l| l.starts_with("send-keys")).collect();
    assert_eq!(keys.len(), 2, "{:?}", log);
    assert!(keys[0].contains("-l claude 'Run tsk actor register \"Marvin\" --kind robot. Then run /tsk:start-thread M-TEST-01.'"), "{}", keys[0]);
    assert!(keys[1].ends_with("Enter"), "{}", keys[1]);

    // A new mission gives no code, so the session has the harness name.
    let output = fx.tsk(&[
        "harness",
        "new",
        "--new-robot",
        "--new-mission",
        "--name",
        "owl",
        "--no-attach",
    ]);
    assert_success(&output);
    assert!(
        stdout(&output).contains("tmux session: owl"),
        "{}",
        stdout(&output)
    );
    let log = fx.tmux_lines();
    assert!(
        log.iter().any(|l| l
            .contains("-l claude 'Run /tsk:construct-robot. Then ask me what the new mission is")),
        "{:?}",
        log
    );
    assert!(fx.state("owl")["robot"].is_null());
    assert!(fx.state("owl")["mission"].is_null());

    // A mission code that a tmux session has already gives way to the harness name.
    let output = fx.tsk(&[
        "harness",
        "new",
        "--robot",
        "Marvin",
        "--mission",
        "M-TEST-01",
        "--name",
        "elk",
        "--no-claude",
        "--no-attach",
    ]);
    assert_success(&output);
    assert!(
        stdout(&output).contains("tmux session: elk"),
        "{}",
        stdout(&output)
    );
}

#[test]
fn new_picks_unused_words_and_refuses_a_used_or_invalid_name() {
    let fx = Fixture::new();
    let mut seen = Vec::new();
    for _ in 0..3 {
        let output = fx.tsk(&[
            "harness",
            "new",
            "--robot",
            "Marvin",
            "--mission",
            "M-X",
            "--no-claude",
            "--no-attach",
        ]);
        assert_success(&output);
        let name = stdout(&output)
            .lines()
            .find_map(|l| l.strip_prefix("harness: "))
            .unwrap()
            .to_string();
        assert!(!seen.contains(&name), "{} was picked twice", name);
        assert!(
            (3..=9).contains(&name.len()) && name.chars().all(|c| c.is_ascii_lowercase()),
            "{}",
            name
        );
        assert!(fx.trees().join("work").join(&name).is_dir());
        seen.push(name);
    }
    let used = fx.names_file();
    assert_eq!(std::fs::read_to_string(used).unwrap().lines().count(), 3);

    let refuse = |name: &str| {
        let args = [
            "harness",
            "new",
            "--robot",
            "Marvin",
            "--mission",
            "M-X",
            "--name",
            name,
            "--no-claude",
            "--no-attach",
        ];
        assert_failure(&fx.tsk(&args))
    };
    let err = refuse(&seen[0]);
    assert!(err.contains("is used"), "{}", err);
    let err = refuse("Not A Name");
    assert!(err.contains("not a harness name"), "{}", err);
}

#[test]
fn new_without_a_terminal_names_the_flag_that_is_missing() {
    let fx = Fixture::new();
    let err = assert_failure(&fx.tsk(&["harness", "new", "--no-attach"]));
    assert!(err.contains("--thread"), "{}", err);
    let err = assert_failure(&fx.tsk(&["harness", "new", "--robot", "Marvin", "--no-attach"]));
    assert!(err.contains("--mission <id> or --new-mission"), "{}", err);
    let err = assert_failure(&fx.tsk(&["harness", "new", "--mission", "M-X", "--no-attach"]));
    assert!(err.contains("--robot <name> or --new-robot"), "{}", err);
    // Nothing was made.
    assert!(!fx.trees().join("work").exists());
}

#[test]
fn list_shows_awake_and_asleep_from_tmux_in_a_table_a_porcelain_and_json() {
    let fx = Fixture::new();
    fx.new_harness("fox", "M-TEST-01");
    fx.new_harness("owl", "M-OTHER");
    assert_eq!(fx.sessions(), vec!["M-TEST-01", "M-OTHER"]);
    // The owl session goes away.
    std::fs::write(fx.sessions_file(), "M-TEST-01\n").unwrap();

    let output = fx.tsk(&["harness", "list"]);
    assert_success(&output);
    let table = stdout(&output);
    println!("{}", table);
    let lines: Vec<&str> = table.lines().collect();
    assert_eq!(lines.len(), 3, "{}", table);
    assert_eq!(
        lines[0].split_whitespace().collect::<Vec<_>>(),
        ["NAME", "ROBOT", "MISSION", "THREAD", "STATE", "DIRECTORY"]
    );
    assert_eq!(
        lines[1].split_whitespace().collect::<Vec<_>>(),
        [
            "fox",
            "Marvin",
            "M-TEST-01",
            "-",
            "awake",
            "~/trees/work/fox"
        ]
    );
    assert_eq!(
        lines[2].split_whitespace().collect::<Vec<_>>(),
        [
            "owl",
            "Marvin",
            "M-OTHER",
            "-",
            "asleep",
            "~/trees/work/owl"
        ]
    );

    let porcelain = stdout(&fx.tsk(&["harness", "list", "--porcelain"]));
    let rows: Vec<Vec<&str>> = porcelain.lines().map(|l| l.split('\t').collect()).collect();
    assert_eq!(
        rows[0],
        [
            "fox",
            "Marvin",
            "M-TEST-01",
            "-",
            "awake",
            fx.trees().join("work").join("fox").to_str().unwrap()
        ]
    );
    assert_eq!(rows[1][4], "asleep");

    let listed = json(&stdout(&fx.tsk(&["harness", "list", "--json"])));
    assert_eq!(listed[0]["name"], "fox");
    assert_eq!(listed[0]["state"], "awake");
    assert_eq!(listed[1]["state"], "asleep");
    assert_eq!(listed[1]["mission"], "M-OTHER");

    // Another clone's harnesses are not listed, and an empty list says so.
    let other = fx.tsk_at(
        &fx.root.path().join("work2"),
        &["harness", "list", "--porcelain"],
    );
    assert_success(&other);
    assert_eq!(stdout(&other), "");
}

#[test]
fn path_takes_a_unique_prefix_and_cd_explains_the_shell_function() {
    let fx = Fixture::new();
    let fox = fx.new_harness("fox", "M-A");
    fx.new_harness("fox2", "M-B");
    let owl = fx.new_harness("owl", "M-C");

    let path = |name: &str| fx.tsk(&["harness", "path", name]);
    assert_eq!(stdout(&path("ow")).trim(), owl.to_str().unwrap());
    // An exact name wins over a longer name that it starts.
    assert_eq!(stdout(&path("fox")).trim(), fox.to_str().unwrap());
    // It works from inside a worktree too.
    let inside = fx.tsk_at(&owl, &["harness", "path", "fox"]);
    assert_eq!(stdout(&inside).trim(), fox.to_str().unwrap());

    let err = assert_failure(&path("fo"));
    assert!(err.contains("fox") && err.contains("fox2"), "{}", err);
    assert!(!err.contains("owl"), "{}", err);
    let err = assert_failure(&path("zz"));
    assert!(err.contains("fox") && err.contains("owl"), "{}", err);

    let cd = fx.tsk(&["harness", "cd", "owl"]);
    let err = assert_failure(&cd);
    assert!(
        err.contains("tsk harness cd needs the shell function: run tsk install zshrc, or use: cd \"$(tsk harness path <name>)\""),
        "{}",
        err
    );
    assert_eq!(stdout(&cd), "");
}

fn start_thread(fx: &Fixture, worktree: &Path) -> String {
    let output = fx.tsk_at(worktree, &["thread", "start", "M-TEST-01"]);
    assert_success(&output);
    stdout(&output)
        .trim_end()
        .strip_prefix("started:")
        .unwrap_or_else(|| panic!("unexpected start output: {}", stdout(&output)))
        .to_string()
}

fn thread_row(fx: &Fixture, id: &str) -> serde_json::Value {
    let output = fx.tsk(&["thread", "list"]);
    assert_success(&output);
    stdout(&output)
        .lines()
        .map(json)
        .find(|row| row["id"] == id)
        .unwrap_or_else(|| panic!("no thread {}", id))
}

#[test]
fn sleep_pauses_a_held_thread_and_stops_the_session_and_wake_resumes_it() {
    if !common::dolt_installed() {
        eprintln!("skipped: dolt is not installed");
        return;
    }
    let fx = Fixture::new();
    let fox = fx.new_harness("fox", "M-TEST-01");
    let thread = start_thread(&fx, &fox);
    assert_eq!(thread_row(&fx, &thread)["state"], "held");
    assert_eq!(fx.sessions(), vec!["M-TEST-01"]);

    let output = fx.tsk(&["harness", "sleep", "fo"]);
    assert_success(&output);
    assert!(
        stdout(&output).contains(&format!("paused thread {}", thread)),
        "{}",
        stdout(&output)
    );
    let row = thread_row(&fx, &thread);
    assert_eq!(row["state"], "paused");
    let note = row["latest_whats_next"].as_str().unwrap();
    assert!(
        note.starts_with("Harness fox put to sleep at ")
            && note.contains("the previous pause entry holds the hand-over"),
        "{}",
        note
    );
    assert!(fx
        .tmux_lines()
        .contains(&"kill-session -t =M-TEST-01".to_string()));
    assert!(fx.sessions().is_empty());
    // The worktree and the state stay, and the state knows the thread now.
    assert!(fox.is_dir());
    assert_eq!(fx.state("fox")["thread_id"], thread.as_str());
    let listed = stdout(&fx.tsk(&["harness", "list", "--porcelain"]));
    assert!(
        listed.contains(&format!("\t{}\tasleep\t", thread)),
        "{}",
        listed
    );

    // A harness that holds no thread sleeps with no pause entry.
    let again = fx.tsk(&["harness", "sleep", "fox"]);
    assert_success(&again);
    assert!(
        !stdout(&again).contains("paused thread"),
        "{}",
        stdout(&again)
    );

    // Wake: the tmux session starts again in the worktree and claude resumes the thread.
    let output = fx.tsk(&["harness", "wake", "fox", "--no-attach"]);
    assert_success(&output);
    assert_eq!(fx.sessions(), vec!["M-TEST-01"]);
    let log = fx.tmux_lines();
    let wake_keys = log
        .iter()
        .rfind(|l| l.starts_with("send-keys") && l.contains("-l claude"))
        .unwrap();
    assert!(
        wake_keys.contains(&format!(
            "claude 'Run tsk actor register \"Marvin\" --kind robot. Then run /tsk:resume-thread {}.'",
            thread
        )),
        "{}",
        wake_keys
    );
    // A second wake finds it awake.
    let output = fx.tsk(&["harness", "wake", "fox", "--no-attach"]);
    assert!(
        stdout(&output).contains("awake already"),
        "{}",
        stdout(&output)
    );

    // A new harness can pick up the paused thread by its ID.
    fx.tsk(&["harness", "sleep", "fox"]);
    let _ = std::fs::write(fx.sessions_file(), "");
    let output = fx.tsk(&[
        "harness",
        "new",
        "--thread",
        &thread,
        "--name",
        "owl",
        "--no-attach",
    ]);
    assert_success(&output);
    assert_eq!(fx.state("owl")["thread_id"], thread.as_str());
    assert_eq!(fx.state("owl")["mission"], "M-TEST-01");
    assert!(
        fx.tmux_lines()
            .iter()
            .any(|l| l.contains(&format!("claude 'Run /tsk:resume-thread {}.'", thread))),
        "{:?}",
        fx.tmux_lines()
    );
    let err = assert_failure(&fx.tsk(&["harness", "new", "--thread", "nope0000", "--no-attach"]));
    assert!(err.contains("no such thread"), "{}", err);
}

#[test]
fn delete_refuses_changes_and_unpushed_commits_and_force_removes_and_retires_the_name() {
    let fx = Fixture::new();
    let fox = fx.new_harness("fox", "M-TEST-01");

    // A dirty worktree.
    std::fs::write(fox.join("notes.txt"), "work in progress\n").unwrap();
    let err = assert_failure(&fx.tsk(&["harness", "delete", "fox"]));
    assert!(err.contains("uncommitted changes"), "{}", err);
    assert!(fox.is_dir());
    assert!(fx.state_file("fox").is_file());

    // A commit on no remote branch.
    git(&fox, &["add", "-A"]);
    git(&fox, &["commit", "--quiet", "-m", "Work"]);
    let err = assert_failure(&fx.tsk(&["harness", "delete", "fox"]));
    assert!(err.contains("on no remote branch"), "{}", err);
    assert!(fox.is_dir());

    // --force removes the worktree, keeps the branch that holds unpushed commits, says so.
    let output = fx.tsk(&["harness", "delete", "fox", "--force"]);
    assert_success(&output);
    let out = stdout(&output);
    assert!(out.contains("branch harness/fox kept"), "{}", out);
    assert!(!fox.exists());
    assert!(!fx.state_file("fox").exists());
    assert!(fx
        .tmux_lines()
        .contains(&"kill-session -t =M-TEST-01".to_string()));
    assert!(fx.sessions().is_empty());
    git(
        &fx.work(),
        &["rev-parse", "--verify", "refs/heads/harness/fox"],
    );
    assert!(!stdout(&fx.tsk(&["harness", "list", "--porcelain"])).contains("fox"));

    // The name stays retired.
    assert_eq!(std::fs::read_to_string(fx.names_file()).unwrap(), "fox\n");
    let err = assert_failure(&fx.tsk(&[
        "harness",
        "new",
        "--robot",
        "Marvin",
        "--mission",
        "M-X",
        "--name",
        "fox",
        "--no-claude",
        "--no-attach",
    ]));
    assert!(err.contains("is used"), "{}", err);

    // A clean harness with nothing unpushed is deleted without --force, branch and all.
    let owl = fx.new_harness("owl", "M-TEST-01");
    let output = fx.tsk(&["harness", "delete", "owl"]);
    assert_success(&output);
    assert!(
        stdout(&output).contains("branch harness/owl deleted"),
        "{}",
        stdout(&output)
    );
    assert!(!owl.exists());
    let branches = git(&fx.work(), &["branch", "--list", "harness/owl"]);
    assert_eq!(branches, "");
    assert_eq!(
        std::fs::read_to_string(fx.names_file()).unwrap(),
        "fox\nowl\n"
    );
}

#[test]
fn delete_pauses_the_thread_that_the_harness_holds() {
    if !common::dolt_installed() {
        eprintln!("skipped: dolt is not installed");
        return;
    }
    let fx = Fixture::new();
    let fox = fx.new_harness("fox", "M-TEST-01");
    let thread = start_thread(&fx, &fox);
    assert_eq!(thread_row(&fx, &thread)["state"], "held");
    let output = fx.tsk(&["harness", "delete", "fox"]);
    assert_success(&output);
    let row = thread_row(&fx, &thread);
    assert_eq!(row["state"], "paused");
    assert!(
        row["latest_whats_next"]
            .as_str()
            .unwrap()
            .starts_with("Harness fox deleted at "),
        "{}",
        row["latest_whats_next"]
    );
    assert!(!fox.exists());
}

#[test]
fn the_harness_commands_are_in_the_help() {
    let fx = Fixture::new();
    let top = stdout(&fx.tsk(&["--help"]));
    assert!(top.contains("harness"), "{}", top);
    for sub in ["new", "list", "path", "cd", "sleep", "wake", "delete"] {
        let output = fx.tsk(&["harness", sub, "--help"]);
        assert_success(&output);
        assert!(!stdout(&output).contains('\u{2014}'), "{}", sub);
    }
}
