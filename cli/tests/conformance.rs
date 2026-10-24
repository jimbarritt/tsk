//! One set of behaviours of the ledger. Each case drives the `tsk` binary end to end
//! against a bare git repo as origin. The Dolt database publishes to the same repo through
//! a `git+file` remote. Cases print a note and return when the `dolt` binary is not
//! installed.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use tempfile::TempDir;

use crate::common;

use common::dolt_installed;

use common::templates::{self, Seed, Step};

const LEDGER_REF: &str = "refs/heads/tsk/ledger";
const DOCS_REF: &str = "refs/heads/tsk/docs";
const BRIEFING: &str = "missions/operational/M-CONF-01-conformance.md";
const CLOUD_VAR: &str = "CLAUDE_CODE_REMOTE_SESSION_ID";

/// Every branch a ledger with documents publishes: the code, the data and the documents.
fn published_branches() -> Vec<String> {
    let mut refs = vec![
        "refs/heads/main".to_string(),
        LEDGER_REF.to_string(),
        DOCS_REF.to_string(),
    ];
    refs.sort();
    refs
}

/// What `tsk ledger doc list` prints for the seed documents and `extra`, sorted.
fn listing(extra: &[&str]) -> String {
    let mut paths: Vec<&str> = vec!["README.md", "index.md"];
    paths.extend_from_slice(extra);
    paths.sort();
    paths.iter().map(|p| format!("{}\n", p)).collect()
}

/// True when the whole run has the Dolt server switch on (`TSK_DOLT_SERVER=on`).
fn server_on() -> bool {
    std::env::var("TSK_DOLT_SERVER").is_ok_and(|v| v.trim().eq_ignore_ascii_case("on"))
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
    fn new() -> Fixture {
        let fixture = Fixture {
            root: tempfile::tempdir().unwrap(),
        };
        std::fs::create_dir_all(fixture.home()).unwrap();
        common::seed_dolt_home(&fixture.home());
        std::fs::create_dir_all(fixture.state()).unwrap();
        let root = fixture.root.path();
        git(
            root,
            &["init", "--quiet", "--bare", "-b", "main", "origin.git"],
        );
        git(root, &["init", "--quiet", "-b", "main", "seed"]);
        let seed = root.join("seed");
        git(
            &seed,
            &[
                "remote",
                "add",
                "origin",
                fixture.origin().to_str().unwrap(),
            ],
        );
        write(&seed.join("README.md"), "main\n");
        git(&seed, &["add", "-A"]);
        git(&seed, &["commit", "--quiet", "-m", "Main commit"]);
        git(
            &seed,
            &["push", "--quiet", "origin", "HEAD:refs/heads/main"],
        );
        for name in ["work", "work2"] {
            git(
                root,
                &["clone", "--quiet", fixture.origin().to_str().unwrap(), name],
            );
        }
        fixture
    }

    /// A fixture that costs many `tsk` and `dolt` runs to build is built once per test
    /// process, then copied for each test that asks for it (see `common::templates`).
    /// `build` returns the fixture and a note a test may need from it (a thread ID, say).
    /// The copy is a real filesystem copy. Every file that holds the path of the built
    /// fixture, such as a git remote URL or a Dolt remote, is rewritten to the path of the
    /// copy.
    fn cached(key: &str, build: impl FnOnce() -> (Fixture, String)) -> (Fixture, String) {
        let (root, note) = templates::cached(key, build);
        (Fixture { root }, note)
    }

    fn origin(&self) -> PathBuf {
        self.root.path().join("origin.git")
    }

    fn a(&self) -> PathBuf {
        self.root.path().join("work")
    }

    fn b(&self) -> PathBuf {
        self.root.path().join("work2")
    }

    fn home(&self) -> PathBuf {
        self.root.path().join("home")
    }

    fn state(&self) -> PathBuf {
        self.root.path().join("state")
    }

    fn command(&self, dir: &Path, args: &[&str]) -> Command {
        self.command_with(Path::new(env!("CARGO_BIN_EXE_tsk")), dir, args)
    }

    /// [`Self::command`] for another binary, such as an earlier release.
    fn command_with(&self, program: &Path, dir: &Path, args: &[&str]) -> Command {
        let mut command = Command::new(program);
        command
            .args(args)
            .current_dir(dir)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "test@example.com")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "test@example.com")
            .env("HOME", self.home())
            .env("XDG_STATE_HOME", self.state())
            .env("TSK_DOLT_SERVER_IDLE_SECS", "3")
            .env("TSK_BACKGROUND_PUBLISH", "off")
            .env("XDG_CONFIG_HOME", self.home().join("config"))
            .env("TSK_HOME", self.home().join(".tsk"))
            .env("TSK_NO_UPDATE_CHECK", "1")
            .env("TSK_LEDGER_REFS", "branches")
            .env("TSK_MACHINE_NAME", "laptop")
            .env("GIT_CEILING_DIRECTORIES", self.root.path())
            .env_remove(CLOUD_VAR);
        command
    }

    fn tsk(&self, dir: &Path, args: &[&str]) -> Output {
        self.command(dir, args).output().expect("failed to run tsk")
    }

    fn tsk_cloud(&self, dir: &Path, session: &str, args: &[&str]) -> Output {
        self.command(dir, args)
            .env(CLOUD_VAR, session)
            .output()
            .expect("failed to run tsk")
    }

    fn tsk_stdin(&self, dir: &Path, args: &[&str], input: &str) -> Output {
        let mut child = self
            .command(dir, args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
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

    fn ok(&self, dir: &Path, args: &[&str]) -> String {
        let output = self.tsk(dir, args);
        assert_success(&output);
        stdout(&output)
    }

    fn put(&self, dir: &Path, path: &str, body: &str) {
        assert_success(&self.tsk_stdin(dir, &["ledger", "doc", "put", path], body));
    }

    fn get(&self, dir: &Path, path: &str) -> Output {
        self.tsk(dir, &["ledger", "doc", "get", path])
    }

    fn remote_ledger(&self) -> Option<String> {
        let out = git(&self.origin(), &["ls-remote", ".", LEDGER_REF]);
        out.split_whitespace().next().map(str::to_string)
    }

    /// Every branch on the origin, sorted.
    fn remote_branches(&self) -> Vec<String> {
        let out = git(
            &self.origin(),
            &["for-each-ref", "--format=%(refname)", "refs/heads/"],
        );
        let mut refs: Vec<String> = out.lines().map(str::to_string).collect();
        refs.sort();
        refs
    }

    fn ledger_path(&self, dir: &Path) -> PathBuf {
        PathBuf::from(self.ok(dir, &["ledger", "path"]).trim_end())
    }

    /// Run SQL against the clone's Dolt database. The change stays in the working set
    /// until a `tsk ledger push` commits it.
    fn dolt_sql(&self, dir: &Path, sql: &str) -> String {
        // A Dolt server that an earlier command started holds the store for writing, and
        // `dolt` would open it read only. Stop it; the next command starts it again.
        common::stop_servers(self.root.path(), true);
        let out = Command::new("dolt")
            .args(["sql", "-r", "csv", "-q", sql])
            .current_dir(self.ledger_path(dir))
            .env("HOME", self.home())
            .env("DOLT_DISABLE_EVENT_FLUSH", "1")
            .output()
            .unwrap();
        assert!(out.status.success(), "{}", stderr(&out));
        stdout(&out)
    }

    /// A data row no document touches, to give the Dolt side a change of its own.
    fn add_binding(&self, dir: &Path, session: &str) {
        self.dolt_sql(
            dir,
            &format!(
                "insert into bindings (actor, kind, thread_id, registered_at) values ('{}', 'cloud', 't', '2026-01-01T00:00:00Z')",
                session
            ),
        );
    }

    fn binding_count(&self, dir: &Path) -> String {
        self.dolt_sql(dir, "select count(*) as c from bindings")
            .lines()
            .nth(1)
            .unwrap_or("")
            .to_string()
    }

    fn remote_docs(&self) -> Option<String> {
        let out = git(&self.origin(), &["ls-remote", ".", DOCS_REF]);
        out.split_whitespace().next().map(str::to_string)
    }

    /// The files of a branch of the origin, as `ls-tree -r` lists them.
    fn origin_files(&self, reference: &str) -> Vec<String> {
        git(&self.origin(), &["ls-tree", "-r", "--name-only", reference])
            .lines()
            .map(str::to_string)
            .collect()
    }

    /// The documents worktree of a clone, which the Dolt store keeps beside the database.
    fn docs_path(&self, dir: &Path) -> PathBuf {
        self.ledger_path(dir).parent().unwrap().join("docs")
    }

    /// A ledger with the briefing published from clone A.
    fn with_briefing() -> Fixture {
        Fixture {
            root: templates::instance(&BRIEFING_SEED),
        }
    }

    fn build_with_briefing() -> Fixture {
        let fx = Fixture::new();
        fx.ok(&fx.a(), &["ledger", "fetch"]);
        fx.put(&fx.a(), BRIEFING, "# Mission: conformance\n");
        fx.ok(&fx.a(), &["ledger", "push", "Add the briefing"]);
        fx
    }

    /// A new ledger fetched into clone A, which publishes nothing.
    fn fetched() -> Fixture {
        Fixture {
            root: templates::instance(&templates::FETCHED),
        }
    }
}

impl templates::Root for Fixture {
    fn root(&self) -> &Path {
        self.root.path()
    }
}

/// A ledger with the briefing published from clone A (`work`).
const BRIEFING_SEED: Seed = Seed {
    branch: "main",
    clones: &["work", "work2"],
    refs: Some("branches"),
    steps: &[
        Step::Tsk("work", &["ledger", "fetch"]),
        Step::Stdin(
            "work",
            &["ledger", "doc", "put", BRIEFING],
            "# Mission: conformance\n",
        ),
        Step::Tsk("work", &["ledger", "push", "Add the briefing"]),
    ],
};

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
    String::from_utf8_lossy(&output.stdout)
        .trim_end()
        .to_string()
}

fn write(path: &Path, content: &str) {
    std::fs::write(path, content).unwrap();
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// `tsk ledger status` output without its `Server` line, which depends on the switch.
/// The text with each mission `uid` of a JSON record blanked: a `uid` is random for a new
/// mission, so two ledgers made the same way never agree on it.
fn without_uids(text: String) -> String {
    let mut out = String::new();
    let mut rest = text.as_str();
    while let Some(at) = rest.find("\"uid\":\"") {
        let end = at + "\"uid\":\"".len();
        out.push_str(&rest[..end]);
        out.push('-');
        rest = &rest[end..];
        let close = rest.find('"').unwrap_or(rest.len());
        rest = &rest[close..];
    }
    out.push_str(rest);
    out
}

/// A mission record without its `uid`, which must be a 36 character UUID.
fn without_uid(mut record: serde_json::Value) -> serde_json::Value {
    let uid = record
        .as_object_mut()
        .and_then(|object| object.remove("uid"))
        .expect("the record has a uid");
    assert_eq!(uid.as_str().map(str::len), Some(36), "{}", uid);
    record
}

fn no_server_line(text: String) -> String {
    text.split_inclusive('\n')
        .filter(|line| !line.starts_with("Server "))
        .collect()
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

fn assert_exit(output: &Output, code: i32) {
    assert_eq!(
        output.status.code(),
        Some(code),
        "stdout: {}\nstderr: {}",
        stdout(output),
        stderr(output)
    );
}

fn json(text: &str) -> serde_json::Value {
    serde_json::from_str(text).unwrap_or_else(|e| panic!("not JSON ({}): {}", e, text))
}

impl Fixture {
    /// The Stop hook check, as the hook runs it: a session ID on stdin.
    fn guard(&self, dir: &Path, cloud: Option<&str>) -> Output {
        let mut command = self.command(dir, &["thread", "guard"]);
        if let Some(session) = cloud {
            command.env(CLOUD_VAR, session);
        }
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(b"{\"session_id\":\"x\"}")
            .unwrap();
        child.wait_with_output().unwrap()
    }

    /// The reason the Stop hook check blocks with, or `None` when it passes in silence.
    fn guard_reason(&self, dir: &Path, cloud: Option<&str>) -> Option<String> {
        let output = self.guard(dir, cloud);
        assert_success(&output);
        let text = stdout(&output);
        if text.is_empty() {
            return None;
        }
        let decision = json(&text);
        assert_eq!(decision["decision"], "block");
        Some(decision["reason"].as_str().unwrap().to_string())
    }

    /// The context `tsk thread session-start` prints. Each call needs its own session ID.
    fn session_context(&self, dir: &Path, session: &str, format: &str) -> String {
        let mut child = self
            .command(dir, &["thread", "session-start", "--format", format])
            .env_remove("CLAUDE_ENV_FILE")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(
                format!("{{\"session_id\":\"{}\",\"source\":\"startup\"}}", session).as_bytes(),
            )
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert_success(&output);
        let value = json(&stdout(&output));
        let context = match format {
            "copilot" => &value["additionalContext"],
            _ => &value["hookSpecificOutput"]["additionalContext"],
        };
        context.as_str().unwrap().to_string()
    }

    /// The `thread list` row of a thread.
    fn thread_row(&self, dir: &Path, id: &str) -> serde_json::Value {
        let list = self.ok(dir, &["thread", "list"]);
        list.lines()
            .map(json)
            .find(|row| row["id"] == id)
            .unwrap_or_else(|| panic!("no row for {}: {}", id, list))
    }

    fn started(&self, dir: &Path, args: &[&str]) -> String {
        let output = self.tsk(dir, args);
        assert_success(&output);
        stdout(&output)
            .trim_end()
            .strip_prefix("started:")
            .unwrap_or_else(|| panic!("{}", stdout(&output)))
            .to_string()
    }

    /// The latest continuation state entry of a thread, read by resuming it.
    fn latest_entry(&self, dir: &Path, id: &str) -> serde_json::Value {
        let latest = json(&self.ok(dir, &["thread", "resume", id]))["latest"].clone();
        // A resume binds in the local store only (ADR 0019). The old resume pushed.
        self.ok(dir, &["ledger", "push", "Bind for the test"]);
        latest
    }
}

impl Fixture {
    /// The actor key of the main worktree of a clone: the clone ID and `main`.
    fn main_actor(&self, dir: &Path) -> String {
        let id = std::fs::read_to_string(dir.join(".git").join("tsk-clone-id")).unwrap();
        format!("{}/main", id.trim())
    }

    fn marker(&self, dir: &Path) -> PathBuf {
        dir.join(".git").join("tsk-thread-id")
    }

    /// The actors `thread list` names for a thread, as `kind:actor`, sorted.
    fn actors_of(&self, dir: &Path, id: &str) -> Vec<String> {
        let row = self.thread_row(dir, id);
        let mut actors: Vec<String> = row["actors"]
            .as_array()
            .unwrap_or_else(|| panic!("no actors in {}", row))
            .iter()
            .map(|a| {
                format!(
                    "{}:{}",
                    a["kind"].as_str().unwrap(),
                    a["actor"].as_str().unwrap()
                )
            })
            .collect();
        actors.sort();
        actors
    }
}

mod cases {
    use super::*;

    pub fn fetch_creates_a_new_ledger_and_the_first_push_creates_the_branch() {
        let fx = Fixture::new();
        let a = fx.a();
        assert_eq!(fx.remote_ledger(), None);

        let fetch = fx.tsk(&a, &["ledger", "fetch"]);
        assert_success(&fetch);
        assert!(
            stderr(&fetch).contains("does not exist yet"),
            "{}",
            stderr(&fetch)
        );
        let path = PathBuf::from(stdout(&fetch).trim_end());
        assert!(path.is_dir(), "{}", path.display());
        assert_eq!(path, fx.ledger_path(&a));
        assert_eq!(fx.remote_ledger(), None);
        assert_eq!(
            fx.ok(&a, &["ledger", "doc", "get", "index.md"]),
            "# Ledger index\n"
        );

        let again = fx.tsk(&a, &["ledger", "fetch"]);
        assert_success(&again);
        assert!(stderr(&again).contains("does not exist yet"));

        fx.put(&a, "missions/M-X.md", "# Mission X\n");
        let push = fx.tsk(&a, &["ledger", "push", "Add M-X"]);
        assert_success(&push);
        assert!(!stdout(&push).trim().is_empty());
        assert!(fx.remote_ledger().is_some());
        assert_eq!(fx.remote_branches(), published_branches());

        let second = fx.b();
        let fetch = fx.tsk(&second, &["ledger", "fetch"]);
        assert_success(&fetch);
        assert!(
            !stderr(&fetch).contains("does not exist"),
            "{}",
            stderr(&fetch)
        );
        assert_eq!(
            fx.ok(&second, &["ledger", "doc", "get", "missions/M-X.md"]),
            "# Mission X\n"
        );
        assert_eq!(
            fx.ok(&second, &["ledger", "doc", "list"]),
            listing(&["missions/M-X.md"])
        );

        let nothing = fx.tsk(&a, &["ledger", "push", "Nothing"]);
        assert_success(&nothing);
        assert!(
            stderr(&nothing).contains("nothing to commit"),
            "{}",
            stderr(&nothing)
        );
        assert!(stderr(&nothing).contains("nothing to push"));
    }

    pub fn documents_round_trip_between_clones() {
        let fx = Fixture::fetched();
        let (a, other) = (fx.a(), fx.b());
        let awkward = "no newline at end \"quoted\" 'single' \\ back\n\ttab; drop table documents; -- \u{e9} \u{65e5}\u{672c} \u{1f600}\r\nlast";
        fx.put(&a, "missions/b.md", awkward);
        fx.put(&a, "missions/a.md", "a");
        fx.put(&a, "notes/n.md", "");
        assert_eq!(
            fx.ok(&a, &["ledger", "doc", "list", "missions/"]),
            "missions/a.md\nmissions/b.md\n"
        );
        assert_eq!(
            fx.ok(&a, &["ledger", "doc", "get", "missions/b.md"]),
            awkward
        );

        fx.ok(&a, &["ledger", "push", "Add documents"]);

        fx.put(&a, "scratch.md", "uncommitted\n");
        let dirty = fx.tsk(&a, &["ledger", "fetch"]);
        assert_exit(&dirty, 1);
        assert!(
            stderr(&dirty).contains("uncommitted changes"),
            "{}",
            stderr(&dirty)
        );
        assert_success(&fx.tsk(&a, &["ledger", "doc", "rm", "scratch.md"]));
        fx.ok(&a, &["ledger", "fetch"]);

        fx.ok(&other, &["ledger", "fetch"]);
        assert_eq!(
            fx.ok(&other, &["ledger", "doc", "get", "missions/b.md"]),
            awkward
        );
        assert_eq!(fx.ok(&other, &["ledger", "doc", "get", "notes/n.md"]), "");
        assert_eq!(
            fx.ok(&other, &["ledger", "doc", "list"]),
            listing(&["missions/a.md", "missions/b.md", "notes/n.md"])
        );

        assert_success(&fx.tsk(&other, &["ledger", "doc", "rm", "missions/a.md"]));
        assert_exit(
            &fx.tsk(&other, &["ledger", "doc", "rm", "missions/a.md"]),
            1,
        );
        assert_exit(&fx.get(&other, "missions/a.md"), 1);
        fx.ok(&other, &["ledger", "push", "Remove a"]);
        fx.ok(&a, &["ledger", "fetch"]);
        assert_exit(&fx.get(&a, "missions/a.md"), 1);
        assert_eq!(
            fx.ok(&a, &["ledger", "doc", "list", "missions/"]),
            "missions/b.md\n"
        );

        for reserved in ["threads/x", "../x.md", "/abs.md"] {
            assert_exit(&fx.get(&a, reserved), 1);
        }
    }

    pub fn threads_start_pause_resume_list_detach_and_stop() {
        let fx = Fixture::with_briefing();
        let (a, other) = (fx.a(), fx.b());

        let start = fx.ok(&a, &["thread", "start", "M-CONF-01"]);
        let id = start
            .trim_end()
            .strip_prefix("started:")
            .unwrap_or_else(|| panic!("{}", start))
            .to_string();
        assert_eq!(
            fx.ok(&a, &["thread", "binding"]),
            format!("worktree:{}\n", id)
        );

        let paused = fx.ok(&a, &["thread", "pause", &id, "T-01", "first \"step\""]);
        assert_eq!(
            paused,
            format!(
                "paused:{}\ncode: clean\ncode pushed: yes\nledger: pushed\n",
                id
            )
        );
        let list = fx.ok(&a, &["thread", "list"]);
        let row = json(list.lines().next().unwrap());
        assert_eq!(row["id"], id.as_str());
        assert_eq!(row["mission_id"], "M-CONF-01");
        assert_eq!(row["mission_link"], BRIEFING);
        assert_eq!(row["latest_whats_next"], "first \"step\"");

        assert_eq!(
            fx.ok(&a, &["thread", "detach"]),
            format!("detached:{}\n", id)
        );
        assert_exit(&fx.tsk(&a, &["thread", "binding", "--no-fetch"]), 1);
        assert_exit(&fx.tsk(&a, &["thread", "start", "M-NONE"]), 1);
        // A detach is local. A read with local changes does not fetch, so publish it.
        fx.ok(&a, &["ledger", "push", "Publish the detach"]);

        let resumed = json(&fx.ok(&other, &["thread", "resume", &id]));
        assert_eq!(resumed["thread_id"], id.as_str());
        assert_eq!(resumed["latest"]["task_id"], "T-01");
        assert_eq!(resumed["latest"]["mission_id"], "M-CONF-01");
        assert_eq!(resumed["latest"]["mission_link"], BRIEFING);
        let commit = resumed["latest"]["git"]["ledger"]["commit"]
            .as_str()
            .unwrap();
        assert!(!commit.is_empty());
        assert_eq!(
            fx.ok(&other, &["thread", "binding"]),
            format!("worktree:{}\n", id)
        );

        assert_exit(&fx.tsk(&a, &["thread", "pause", "zzzzzzzz", "T", "x"]), 1);

        assert_eq!(
            fx.ok(&other, &["thread", "stop"]),
            format!("stopped:{}\n", id)
        );
        assert_eq!(fx.ok(&a, &["thread", "list"]), "");
    }

    pub fn cloud_sessions_bind_through_the_ledger() {
        let fx = Fixture::with_briefing();
        let (a, other) = (fx.a(), fx.b());
        let start = fx.tsk_cloud(&a, "cse_one", &["thread", "start", "M-CONF-01"]);
        assert_success(&start);
        let id = stdout(&start)
            .trim_end()
            .strip_prefix("started:")
            .unwrap()
            .to_string();
        let binding = fx.tsk_cloud(&a, "cse_one", &["thread", "binding"]);
        assert_eq!(stdout(&binding), format!("cloud:{}\n", id));

        let second = fx.tsk_cloud(&other, "cse_two", &["thread", "resume", &id]);
        assert_success(&second);
        let seen = fx.tsk_cloud(&other, "cse_two", &["thread", "binding"]);
        assert_eq!(stdout(&seen), format!("cloud:{}\n", id));

        let detach = fx.tsk_cloud(&a, "cse_one", &["thread", "detach"]);
        assert_eq!(stdout(&detach), format!("detached:{}\n", id));
        assert_exit(&fx.tsk_cloud(&a, "cse_one", &["thread", "binding"]), 1);

        let stop = fx.tsk_cloud(&other, "cse_two", &["thread", "stop"]);
        assert_eq!(stdout(&stop), format!("stopped:{}\n", id));
        // The detach was local. The push merges it with the stop and publishes it.
        fx.ok(&a, &["ledger", "push", "Publish the detach"]);
        fx.ok(&a, &["ledger", "fetch"]);
        assert_exit(&fx.tsk_cloud(&a, "cse_two", &["thread", "binding"]), 1);
    }

    pub fn worktree_bindings_are_rows_keyed_on_the_clone_and_the_worktree() {
        let fx = Fixture::with_briefing();
        let a = fx.a();
        let id = fx.started(&a, &["thread", "start", "M-CONF-01"]);
        let main = fx.main_actor(&a);
        assert!(
            main.starts_with("work-") && main.ends_with("/main"),
            "{}",
            main
        );

        // The binding is a row. No marker file is written.
        assert!(!fx.marker(&a).exists());
        assert_eq!(
            fx.ok(&a, &["thread", "binding"]),
            format!("worktree:{}\n", id)
        );
        assert_eq!(fx.actors_of(&a, &id), vec![format!("worktree:{}", main)]);
        let row = fx.thread_row(&a, &id);
        assert_eq!(
            row["actors"],
            serde_json::json!([{"actor": main, "kind": "worktree", "name": null}])
        );

        // A linked worktree of the same clone is another actor, named for the worktree.
        let linked = fx.root.path().join("linked");
        git(
            &a,
            &[
                "worktree",
                "add",
                "--quiet",
                "-b",
                "side",
                linked.to_str().unwrap(),
            ],
        );
        assert_exit(&fx.tsk(&linked, &["thread", "binding"]), 1);
        let clone = main.trim_end_matches("/main").to_string();
        let side = format!("{}/linked", clone);
        fx.ok(&linked, &["thread", "resume", &id]);
        assert_eq!(
            fx.ok(&linked, &["thread", "binding"]),
            format!("worktree:{}\n", id)
        );
        assert!(!linked.join(".git").is_dir());
        assert_eq!(
            fx.actors_of(&a, &id),
            vec![format!("worktree:{}", side), format!("worktree:{}", main)]
        );

        // Detaching one worktree leaves the other bound.
        assert_eq!(
            fx.ok(&linked, &["thread", "detach"]),
            format!("detached:{}\n", id)
        );
        assert_exit(&fx.tsk(&linked, &["thread", "binding"]), 1);
        assert_eq!(
            fx.ok(&a, &["thread", "binding"]),
            format!("worktree:{}\n", id)
        );
        assert_eq!(fx.actors_of(&a, &id), vec![format!("worktree:{}", main)]);

        // A second clone has a clone ID of its own, so its main worktree is another actor.
        // Both clones push their local binding changes.
        fx.ok(&a, &["ledger", "push", "Bind the first clone"]);
        let other = fx.b();
        fx.ok(&other, &["thread", "resume", &id]);
        fx.ok(&other, &["ledger", "push", "Bind the second clone"]);
        let other_main = fx.main_actor(&other);
        assert_ne!(other_main, main);
        let mut expected = vec![
            format!("worktree:{}", main),
            format!("worktree:{}", other_main),
        ];
        expected.sort();
        assert_eq!(fx.actors_of(&a, &id), expected);
    }

    pub fn a_marker_file_becomes_a_binding_row_and_is_removed() {
        let fx = Fixture::with_briefing();
        let (a, other) = (fx.a(), fx.b());
        let id = fx.started(&a, &["thread", "start", "M-CONF-01"]);
        fx.ok(&a, &["thread", "detach"]);
        assert_exit(&fx.tsk(&a, &["thread", "binding"]), 1);
        fx.ok(&a, &["ledger", "push", "Publish the detach"]);

        // A clone with no ledger store reads the marker as it is, and fails on nothing.
        write(&fx.marker(&other), &format!("{}\n", id));
        assert_exit(&fx.tsk(&other, &["thread", "binding", "--no-fetch"]), 1);
        assert!(fx.marker(&other).exists());

        // A marker the older tsk wrote. Reading it without a fetch changes nothing.
        write(&fx.marker(&a), &format!("{}\n", id));
        assert_eq!(
            fx.ok(&a, &["thread", "binding", "--no-fetch"]),
            format!("worktree:{}\n", id)
        );
        assert!(fx.marker(&a).exists());
        assert_eq!(fx.guard_reason(&a, None), None);
        assert!(fx.marker(&a).exists());

        // The first command that resolves the binding writes the row and removes the file.
        assert_eq!(
            fx.ok(&a, &["thread", "binding"]),
            format!("worktree:{}\n", id)
        );
        assert!(!fx.marker(&a).exists());
        assert_eq!(
            fx.actors_of(&a, &id),
            vec![format!("worktree:{}", fx.main_actor(&a))]
        );
        // The file is never written again, and the row alone binds.
        assert_eq!(
            fx.ok(&a, &["thread", "binding", "--no-fetch"]),
            format!("worktree:{}\n", id)
        );
        // The row the marker became is a local write: the check asks for a push, once.
        fx.ok(&a, &["ledger", "push", "Publish the row"]);
        assert_eq!(fx.guard_reason(&a, None), None);
        fx.ok(&a, &["thread", "detach"]);
        fx.ok(&a, &["thread", "resume", &id]);
        assert!(!fx.marker(&a).exists());

        // The clone that had no store fetches, migrates, and the row is its own.
        assert_eq!(
            fx.ok(&other, &["thread", "binding"]),
            format!("worktree:{}\n", id)
        );
        assert!(!fx.marker(&other).exists());
        assert_eq!(fx.actors_of(&other, &id).len(), 2);

        // A marker beside a row is stale: the row is the binding and the file goes.
        write(&fx.marker(&a), "ffffffff\n");
        assert_eq!(
            fx.ok(&a, &["thread", "binding"]),
            format!("worktree:{}\n", id)
        );
        assert!(!fx.marker(&a).exists());
    }

    pub fn two_actors_bind_one_thread_and_stop_clears_every_kind() {
        let fx = Fixture::with_briefing();
        let (a, other) = (fx.a(), fx.b());
        let start = fx.tsk_cloud(&a, "cse_one", &["thread", "start", "M-CONF-01"]);
        assert_success(&start);
        let id = stdout(&start)
            .trim_end()
            .strip_prefix("started:")
            .unwrap()
            .to_string();
        assert_success(&fx.tsk_cloud(&other, "cse_two", &["thread", "resume", &id]));
        fx.ok(&other, &["ledger", "push", "Bind the cloud session"]);
        fx.ok(&a, &["thread", "resume", &id]);
        fx.ok(&a, &["ledger", "push", "Bind the worktree"]);

        let main = fx.main_actor(&a);
        let expected = vec![
            "cloud:cse_one".to_string(),
            "cloud:cse_two".to_string(),
            format!("worktree:{}", main),
        ];
        assert_eq!(fx.actors_of(&a, &id), expected);
        assert_eq!(fx.actors_of(&other, &id), expected);
        for row in fx.ok(&a, &["thread", "list"]).lines().map(json) {
            assert_eq!(row["actors"].as_array().unwrap().len(), 3, "{}", row);
        }

        // Each actor reads its own binding.
        assert_eq!(
            stdout(&fx.tsk_cloud(&a, "cse_one", &["thread", "binding"])),
            format!("cloud:{}\n", id)
        );
        assert_eq!(
            stdout(&fx.tsk_cloud(&other, "cse_two", &["thread", "binding"])),
            format!("cloud:{}\n", id)
        );
        assert_eq!(
            fx.ok(&a, &["thread", "binding"]),
            format!("worktree:{}\n", id)
        );

        // Stopping the thread clears the bindings of both kinds.
        let stop = fx.tsk_cloud(&other, "cse_two", &["thread", "stop"]);
        assert_eq!(stdout(&stop), format!("stopped:{}\n", id));
        assert_exit(&fx.tsk_cloud(&a, "cse_one", &["thread", "binding"]), 1);
        assert_exit(&fx.tsk(&a, &["thread", "binding"]), 1);
        assert_exit(&fx.tsk_cloud(&other, "cse_two", &["thread", "binding"]), 1);
        assert_eq!(fx.ok(&a, &["thread", "list"]), "");
    }

    pub fn rebinding_replaces_the_old_binding_of_an_actor() {
        let fx = Fixture::with_briefing();
        let (a, other) = (fx.a(), fx.b());
        let first = fx.started(&a, &["thread", "start", "M-CONF-01"]);
        let second = {
            let output = fx.tsk_cloud(&other, "cse_two", &["thread", "start", "M-CONF-01"]);
            assert_success(&output);
            stdout(&output)
                .trim_end()
                .strip_prefix("started:")
                .unwrap()
                .to_string()
        };
        let main = format!("worktree:{}", fx.main_actor(&a));
        assert_eq!(fx.actors_of(&a, &first), vec![main.clone()]);
        assert_eq!(fx.actors_of(&a, &second), vec!["cloud:cse_two".to_string()]);

        // The worktree moves to the second thread: its old row is replaced, not added to.
        fx.ok(&a, &["thread", "resume", &second]);
        assert_eq!(
            fx.ok(&a, &["thread", "binding"]),
            format!("worktree:{}\n", second)
        );
        assert_eq!(fx.actors_of(&a, &first), Vec::<String>::new());
        let mut both = vec!["cloud:cse_two".to_string(), main.clone()];
        both.sort();
        assert_eq!(fx.actors_of(&a, &second), both);

        // So does a cloud session.
        assert_success(&fx.tsk_cloud(&a, "cse_two", &["thread", "resume", &first]));
        assert_eq!(
            stdout(&fx.tsk_cloud(&a, "cse_two", &["thread", "binding"])),
            format!("cloud:{}\n", first)
        );
        assert_eq!(fx.actors_of(&a, &first), vec!["cloud:cse_two".to_string()]);
        assert_eq!(fx.actors_of(&a, &second), vec![main]);
    }

    pub fn two_clones_bind_their_worktrees_at_once_and_both_pushes_succeed() {
        let fx = Fixture::with_briefing();
        let (a, other) = (fx.a(), fx.b());
        let id = fx.started(&a, &["thread", "start", "M-CONF-01"]);
        fx.ok(&other, &["ledger", "fetch"]);

        // Each clone binds its own worktree. The change is local, so neither sees the other.
        fx.ok(&other, &["thread", "resume", &id]);
        fx.ok(&a, &["thread", "detach"]);
        fx.ok(&a, &["thread", "resume", &id]);
        fx.ok(&a, &["ledger", "push", "Bind the first clone"]);
        fx.ok(&other, &["ledger", "push", "Bind the second clone"]);

        let mut expected = vec![
            format!("worktree:{}", fx.main_actor(&a)),
            format!("worktree:{}", fx.main_actor(&other)),
        ];
        expected.sort();
        assert_eq!(fx.actors_of(&a, &id), expected);
        assert_eq!(fx.actors_of(&other, &id), expected);
    }

    pub fn a_worktree_resume_and_detach_write_locally_and_the_next_push_leaves_the_store_clean() {
        let fx = Fixture::with_briefing();
        let (a, other) = (fx.a(), fx.b());
        let id = fx.started(&a, &["thread", "start", "M-CONF-01"]);
        let after_start = fx.remote_ledger();
        let detach = fx.tsk(&a, &["thread", "detach"]);
        assert_success(&detach);
        assert!(stderr(&detach).contains("goes out with the next push"));
        let status = fx.ok(&a, &["ledger", "status", "--local", "--porcelain"]);
        assert!(status.contains("data\tlocal changes"), "{}", status);
        fx.ok(&a, &["thread", "resume", &id]);
        // Neither command pushed. (A resume in the second of the start can leave the row
        // as origin holds it, so the store may be clean again here.)
        assert_eq!(fx.remote_ledger(), after_start);

        // The next push publishes them and leaves nothing to push.
        fx.ok(&a, &["ledger", "push", "Publish the binding"]);
        fx.ok(&a, &["ledger", "fetch"]);
        let status = fx.ok(&a, &["ledger", "status", "--porcelain"]);
        assert!(status.contains("data\tclean"), "{}", status);
        assert!(!status.contains("local changes"), "{}", status);
        let after_push = fx.remote_ledger();
        fx.ok(&a, &["thread", "detach"]);
        assert_eq!(fx.remote_ledger(), after_push);
        fx.ok(&a, &["ledger", "push", "Publish the detach"]);
        fx.ok(&a, &["ledger", "fetch"]);
        let status = fx.ok(&a, &["ledger", "status", "--porcelain"]);
        assert!(status.contains("data\tclean"), "{}", status);

        // Pending records and the bind go out together with the next push.
        fx.ok(&a, &["mission", "create", "M-PEND", "Pending"]);
        fx.ok(&a, &["thread", "resume", &id]);
        fx.ok(&other, &["ledger", "fetch"]);
        assert!(!fx
            .ok(&other, &["mission", "list", "--porcelain"])
            .contains("M-PEND"));
        fx.ok(&a, &["ledger", "push", "Publish the record and the bind"]);
        fx.ok(&other, &["ledger", "fetch"]);
        assert!(fx
            .ok(&other, &["mission", "list", "--porcelain"])
            .contains("M-PEND"));
        assert_eq!(
            fx.actors_of(&other, &id),
            vec![format!("worktree:{}", fx.main_actor(&a))]
        );
    }

    pub fn a_worktree_resume_with_origin_away_keeps_the_binding_locally() {
        let fx = Fixture::with_briefing();
        let a = fx.a();
        let id = fx.started(&a, &["thread", "start", "M-CONF-01"]);
        fx.ok(&a, &["thread", "detach"]);

        let moved = fx.root.path().join("origin-away.git");
        std::fs::rename(fx.origin(), &moved).unwrap();
        let resume = fx.tsk(&a, &["thread", "resume", &id]);
        assert_success(&resume);
        assert_eq!(json(&stdout(&resume))["thread_id"], id.as_str());
        assert!(
            stderr(&resume).contains("note: saved in the local ledger store"),
            "{}",
            stderr(&resume)
        );
        assert_eq!(
            fx.ok(&a, &["thread", "binding", "--no-fetch"]),
            format!("worktree:{}\n", id)
        );
        // The check answers from the local store with origin away. It is bound, so it never
        // asks for a thread. It asks for a push when the store holds the bind, which a
        // resume within the second of the start leaves identical to what origin holds.
        if let Some(reason) = fx.guard_reason(&a, None) {
            assert!(reason.contains("tsk ledger push"), "{}", reason);
            assert!(!reason.contains("No thread binding"), "{}", reason);
        }

        let detach = fx.tsk(&a, &["thread", "detach"]);
        assert_success(&detach);
        assert!(stderr(&detach).contains("note: saved in the local ledger store"));
        assert_exit(&fx.tsk(&a, &["thread", "binding", "--no-fetch"]), 1);

        // Origin returns and the next push carries the change.
        std::fs::rename(&moved, fx.origin()).unwrap();
        fx.ok(&a, &["ledger", "push", "Back online"]);
    }

    pub fn events_append_read_new_and_advance_the_watermark() {
        let fx = Fixture::fetched();
        let (a, other) = (fx.a(), fx.b());
        assert_eq!(
            fx.ok(&a, &["events", "read-new"]),
            "{\"new_count\":0,\"total_count\":0,\"events\":[]}\n"
        );

        let payload = fx.root.path().join("payload.json");
        write(&payload, "{\"n\": 1,  \"s\": \"a b\"}\n");
        let appended = fx.tsk(
            &a,
            &[
                "events",
                "append",
                "github",
                "dependabot_alert",
                "created",
                "o/r",
                payload.to_str().unwrap(),
            ],
        );
        assert_success(&appended);
        let batch = fx.tsk_stdin(
            &a,
            &["events", "append-batch", "--source", "github", "--action", "polled"],
            "{\"event_type\":\"t\",\"repo\":\"o/r\",\"payload\":{\"k\":2}}\n{\"event_type\":\"u\",\"repo\":\"o/r\",\"payload\":3}\n",
        );
        assert_success(&batch);
        assert_eq!(stdout(&batch), "queued:2\n");

        let read = json(&fx.ok(&other, &["events", "read-new"]));
        assert_eq!(read["new_count"], 3);
        assert_eq!(read["total_count"], 3);
        assert_eq!(read["events"][0]["payload"]["n"], 1);
        assert_eq!(read["events"][1]["action"], "polled");
        assert_eq!(read["events"][2]["event_type"], "u");

        assert_eq!(
            fx.ok(&other, &["events", "advance-watermark", "2"]),
            "watermark:2\n"
        );
        let read = json(&fx.ok(&a, &["events", "read-new"]));
        assert_eq!(read["new_count"], 1);
        assert_eq!(read["total_count"], 3);
        assert_eq!(read["events"][0]["event_type"], "u");
        assert_exit(&fx.tsk(&a, &["events", "advance-watermark", "1"]), 1);
        let same = fx.tsk(&a, &["events", "advance-watermark", "2"]);
        assert_success(&same);
        assert_eq!(stdout(&same), "");
    }

    pub fn a_stale_push_is_merged_when_different_rows_changed() {
        let fx = Fixture::fetched();
        let (a, other) = (fx.a(), fx.b());
        fx.put(&a, "base.md", "base\n");
        fx.ok(&a, &["ledger", "push", "Base"]);
        fx.ok(&other, &["ledger", "fetch"]);

        fx.put(&a, "from-a.md", "a\n");
        fx.ok(&a, &["ledger", "push", "From A"]);

        fx.put(&other, "from-b.md", "b\n");
        let stale = fx.tsk(&other, &["ledger", "push", "From B"]);
        assert_success(&stale);
        let pushed = stdout(&stale).trim().to_string();

        fx.ok(&a, &["ledger", "fetch"]);
        assert_eq!(
            fx.ok(&a, &["ledger", "doc", "list"]),
            listing(&["base.md", "from-a.md", "from-b.md"])
        );
        assert_eq!(fx.ok(&other, &["ledger", "doc", "get", "from-a.md"]), "a\n");
        assert!(!pushed.is_empty());
    }

    pub fn a_same_row_conflict_stops_and_leaves_the_remote_unchanged() {
        let fx = Fixture::fetched();
        let (a, other) = (fx.a(), fx.b());
        fx.put(&a, "shared.md", "base\n");
        fx.ok(&a, &["ledger", "push", "Base"]);
        fx.ok(&other, &["ledger", "fetch"]);

        fx.put(&a, "shared.md", "from a\n");
        fx.ok(&a, &["ledger", "push", "A edits shared"]);
        let before = fx.remote_ledger();

        fx.put(&other, "shared.md", "from b\n");
        let conflict = fx.tsk(&other, &["ledger", "push", "B edits shared"]);
        assert_exit(&conflict, 1);
        let message = stderr(&conflict);
        assert!(message.contains("conflicted"), "{}", message);
        // The documents are files on their own branch: the conflict names the file.
        assert!(message.contains("shared.md"), "{}", message);
        assert!(message.contains("refs/heads/tsk/docs"), "{}", message);
        assert_eq!(fx.remote_ledger(), before);
    }

    pub fn doc_path_prints_the_file_and_bad_paths_are_refused() {
        let fx = Fixture::fetched();
        let a = fx.a();
        fx.put(&a, "missions/p.md", "x\n");

        let printed = fx.ok(&a, &["ledger", "doc", "path", "missions/p.md"]);
        let file = PathBuf::from(printed.trim_end());
        assert!(file.is_absolute(), "{}", printed);
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "x\n");
        // The file sits in the documents worktree.
        let root = fx.docs_path(&a);
        assert_eq!(
            std::fs::canonicalize(&file).unwrap(),
            std::fs::canonicalize(root.join("missions/p.md")).unwrap()
        );

        let missing = fx.tsk(&a, &["ledger", "doc", "path", "missions/none.md"]);
        assert_exit(&missing, 1);
        assert_eq!(
            stderr(&missing).trim_end(),
            "error: no such document: missions/none.md"
        );
        assert_success(&fx.tsk(&a, &["ledger", "doc", "rm", "missions/p.md"]));
        assert_exit(&fx.tsk(&a, &["ledger", "doc", "path", "missions/p.md"]), 1);

        for bad in [
            "../x.md",
            "/abs.md",
            ".git/config",
            "a/.git/x",
            "a/../../x.md",
        ] {
            assert_exit(&fx.tsk_stdin(&a, &["ledger", "doc", "put", bad], "x"), 1);
            assert_exit(&fx.get(&a, bad), 1);
            assert_exit(&fx.tsk(&a, &["ledger", "doc", "rm", bad]), 1);
            assert_exit(&fx.tsk(&a, &["ledger", "doc", "path", bad]), 1);
        }
    }

    pub fn documents_have_no_reserved_names() {
        let fx = Fixture::fetched();
        let a = fx.a();
        // Documents on the docs branch have no reserved names.
        let put = fx.tsk_stdin(&a, &["ledger", "doc", "put", "threads/note.md"], "n\n");
        assert_success(&put);
        assert_eq!(
            fx.ok(&a, &["ledger", "doc", "get", "threads/note.md"]),
            "n\n"
        );
    }

    // --- Missions ---

    fn mission_json(fx: &Fixture, dir: &Path, id: &str) -> serde_json::Value {
        json(&fx.ok(dir, &["mission", "list", id, "--json"]))
    }

    fn fails(fx: &Fixture, dir: &Path, args: &[&str], needle: &str) {
        let out = fx.tsk(dir, args);
        assert_exit(&out, 1);
        assert!(stderr(&out).starts_with("error: "), "{}", stderr(&out));
        assert!(stderr(&out).contains(needle), "{}", stderr(&out));
        assert_eq!(stdout(&out), "");
    }

    pub fn missions_are_created_and_listed_one_per_line() {
        let fx = Fixture::fetched();
        let a = fx.a();
        assert_eq!(fx.ok(&a, &["mission", "list", "--porcelain"]), "");
        assert_eq!(fx.ok(&a, &["mission", "create", "M-B-TWO", "Second"]), "");
        fx.ok(&a, &["mission", "create", "M-A1", "First one"]);
        fx.ok(
            &a,
            &[
                "mission",
                "create",
                "M-ADMIN",
                "Standing work",
                "--category",
                "administrative",
                "--parent",
                "M-A1",
            ],
        );
        assert_eq!(
            fx.ok(&a, &["mission", "list", "--porcelain"]),
            "M-A1\ttodo\t-\t-\tFirst one\n\
             M-ADMIN\tin-progress\t-\tM-A1\tStanding work\n\
             M-B-TWO\ttodo\t-\t-\tSecond\n"
        );
        assert_eq!(
            fx.ok(&a, &["mission", "list", "M-ADMIN", "--porcelain"]),
            "M-ADMIN\tin-progress\t-\tM-A1\tStanding work\n"
        );

        let before = fx.ok(&a, &["mission", "list", "--porcelain"]);
        fails(
            &fx,
            &a,
            &["mission", "create", "M-A1", "Again"],
            "already exists",
        );
        for bad in ["m-x", "X", "M-", "M-A--B", "M-a"] {
            fails(
                &fx,
                &a,
                &["mission", "create", bad, "Bad"],
                "not a mission ID",
            );
        }
        fails(
            &fx,
            &a,
            &["mission", "create", "M-C", "x", "--parent", "M-NONE"],
            "M-NONE",
        );
        fails(
            &fx,
            &a,
            &["mission", "create", "M-C", "x", "--category", "odd"],
            "operational and administrative",
        );
        fails(
            &fx,
            &a,
            &["mission", "create", "M-C", ""],
            "cannot be empty",
        );
        fails(
            &fx,
            &a,
            &["mission", "create", "M-C", "two\nlines"],
            "one line",
        );
        fails(
            &fx,
            &a,
            &["mission", "list", "M-NONE", "--porcelain"],
            "no such mission: M-NONE",
        );
        assert_eq!(fx.ok(&a, &["mission", "list", "--porcelain"]), before);
    }

    pub fn every_mission_field_round_trips() {
        let fx = Fixture::fetched();
        let (a, other) = (fx.a(), fx.b());
        fx.ok(&a, &["mission", "create", "M-F", "Fields"]);
        fx.ok(&a, &["ledger", "push", "Base"]);
        fx.ok(&other, &["ledger", "fetch"]);

        let name = "N\u{e9}m \u{65e5}\u{672c} \u{1f600} 'q' \"d\"; drop table missions; --";
        fx.ok(&a, &["mission", "set", "M-F", "name", name]);
        let multi = "Line one \"quoted\" 'single' \\ back\n\ttabbed; drop table missions; --\n\u{e9} \u{65e5}\u{672c} \u{1f600}\r\nlast, no newline";
        for (field, key) in [
            ("purpose", "purpose"),
            ("decision-authority", "decision_authority"),
        ] {
            let set = fx.tsk_stdin(&a, &["mission", "set", "M-F", field, "-"], multi);
            assert_success(&set);
            assert_eq!(mission_json(&fx, &a, "M-F")[key], multi, "{}", field);
        }
        fx.ok(
            &a,
            &["mission", "set", "M-F", "purpose", "from the argument"],
        );
        fx.ok(&a, &["mission", "constraint", "add", "M-F", "-dash start"]);
        assert_success(&fx.tsk_stdin(
            &a,
            &["mission", "set", "M-F", "status", "-"],
            "in-progress\n",
        ));

        let record = mission_json(&fx, &a, "M-F");
        assert_eq!(record["name"], name);
        assert_eq!(record["purpose"], "from the argument");
        assert_eq!(
            record["constraints"],
            serde_json::json!([{"n": 1, "text": "-dash start"}])
        );
        assert_eq!(record["out_of_scope"], serde_json::json!([]));
        assert_eq!(record["decision_authority"], multi);
        assert_eq!(record["status"], "in-progress");

        fails(
            &fx,
            &a,
            &["mission", "set", "M-F", "colour", "red"],
            "name, status, purpose, decision-authority",
        );
        fails(
            &fx,
            &a,
            &["mission", "set", "M-NONE", "purpose", "x"],
            "no such mission",
        );
        fails(
            &fx,
            &a,
            &["mission", "set", "M-F", "name", ""],
            "cannot be empty",
        );

        fx.ok(&a, &["ledger", "push", "Fields"]);
        fx.ok(&other, &["ledger", "fetch"]);
        assert_eq!(mission_json(&fx, &other, "M-F"), record);
    }

    pub fn status_accepts_three_values_and_an_administrative_mission_keeps_one() {
        let fx = Fixture::fetched();
        let a = fx.a();
        fx.ok(&a, &["mission", "create", "M-OP", "Operational"]);
        fx.ok(
            &a,
            &[
                "mission",
                "create",
                "M-AD",
                "Admin",
                "--category",
                "administrative",
            ],
        );
        for status in ["in-progress", "done", "todo"] {
            fx.ok(&a, &["mission", "set", "M-OP", "status", status]);
            assert_eq!(mission_json(&fx, &a, "M-OP")["status"], status);
        }
        for bad in ["blocked", "deferred", "DONE", "", "in progress"] {
            fails(
                &fx,
                &a,
                &["mission", "set", "M-OP", "status", bad],
                "todo, in-progress and done",
            );
        }
        assert_eq!(mission_json(&fx, &a, "M-OP")["status"], "todo");

        assert_eq!(mission_json(&fx, &a, "M-AD")["status"], "in-progress");
        fails(
            &fx,
            &a,
            &["mission", "set", "M-AD", "status", "todo"],
            "has no end",
        );
        fails(
            &fx,
            &a,
            &["mission", "set", "M-AD", "status", "done"],
            "has no end",
        );
        fx.ok(&a, &["mission", "set", "M-AD", "status", "in-progress"]);
        assert_eq!(mission_json(&fx, &a, "M-AD")["status"], "in-progress");
    }

    pub fn blocks_are_checked_and_blocked_is_derived() {
        let fx = Fixture::fetched();
        let (a, other) = (fx.a(), fx.b());
        for (id, name) in [("M-A", "Alpha"), ("M-B", "Beta"), ("M-C", "Gamma")] {
            fx.ok(&a, &["mission", "create", id, name]);
        }
        fails(
            &fx,
            &a,
            &["mission", "block", "M-A", "M-NONE"],
            "no such mission: M-NONE",
        );
        fails(
            &fx,
            &a,
            &["mission", "block", "M-NONE", "M-A"],
            "no such mission: M-NONE",
        );
        fails(
            &fx,
            &a,
            &["mission", "block", "M-A", "M-A"],
            "cannot block itself",
        );
        fails(
            &fx,
            &a,
            &["mission", "unblock", "M-A", "M-B"],
            "is not blocked by M-B",
        );

        fx.ok(&a, &["mission", "block", "M-A", "M-B"]);
        fx.ok(&a, &["mission", "block", "M-B", "M-C"]);
        fails(
            &fx,
            &a,
            &["mission", "block", "M-A", "M-B"],
            "already blocked",
        );
        fails(
            &fx,
            &a,
            &["mission", "block", "M-C", "M-A"],
            "cycle (M-C -> M-A -> M-B -> M-C)",
        );
        fails(&fx, &a, &["mission", "block", "M-C", "M-B"], "cycle");

        // Blocked is never stored: it follows the status of the blockers.
        let listed = |fx: &Fixture, dir: &Path| fx.ok(dir, &["mission", "list", "--porcelain"]);
        assert_eq!(
            listed(&fx, &a),
            "M-A\ttodo\tM-B\t-\tAlpha\nM-B\ttodo\tM-C\t-\tBeta\nM-C\ttodo\t-\t-\tGamma\n"
        );
        fx.ok(&a, &["mission", "block", "M-A", "M-C"]);
        fx.ok(&a, &["mission", "set", "M-C", "status", "done"]);
        assert_eq!(
            listed(&fx, &a),
            "M-A\ttodo\tM-B\t-\tAlpha\nM-B\ttodo\t-\t-\tBeta\nM-C\tdone\t-\t-\tGamma\n"
        );
        let record = mission_json(&fx, &a, "M-A");
        assert_eq!(record["blocked"], true);
        assert_eq!(
            record["blocked_by"][0],
            serde_json::json!({"id": "M-B", "blocking": true})
        );
        assert_eq!(
            record["blocked_by"][1],
            serde_json::json!({"id": "M-C", "blocking": false})
        );
        fx.ok(&a, &["mission", "set", "M-B", "status", "done"]);
        assert_eq!(
            listed(&fx, &a),
            "M-A\ttodo\t-\t-\tAlpha\nM-B\tdone\t-\t-\tBeta\nM-C\tdone\t-\t-\tGamma\n"
        );
        assert_eq!(mission_json(&fx, &a, "M-A")["blocked"], false);
        fx.ok(&a, &["mission", "set", "M-B", "status", "in-progress"]);
        assert!(listed(&fx, &a).starts_with("M-A\ttodo\tM-B\t"));

        fx.ok(&a, &["mission", "unblock", "M-A", "M-B"]);
        fx.ok(&a, &["mission", "unblock", "M-A", "M-C"]);
        assert!(listed(&fx, &a).starts_with("M-A\ttodo\t-\t"));
        // The pushed records read back the same on another clone.
        fx.ok(&a, &["mission", "block", "M-A", "M-B"]);
        fx.ok(&a, &["ledger", "push", "Blocks"]);
        fx.ok(&other, &["ledger", "fetch"]);
        assert_eq!(listed(&fx, &other), listed(&fx, &a));
    }

    pub fn objective_outcomes_are_numbered_and_numbers_are_not_reused() {
        let fx = Fixture::fetched();
        let a = fx.a();
        fx.ok(&a, &["mission", "create", "M-O", "Objectives"]);
        assert_eq!(
            fx.ok(&a, &["mission", "objective", "add", "M-O", "First"]),
            "1\n"
        );
        assert_eq!(
            fx.ok(&a, &["mission", "objective", "add", "M-O", "Second \u{e9}"]),
            "2\n"
        );
        assert_eq!(
            fx.ok(&a, &["mission", "objective", "add", "M-O", "-Third"]),
            "3\n"
        );
        assert_eq!(
            fx.ok(&a, &["mission", "objective", "list", "M-O"]),
            "1\topen\tFirst\n2\topen\tSecond \u{e9}\n3\topen\t-Third\n"
        );
        fx.ok(&a, &["mission", "objective", "done", "M-O", "2"]);
        assert_eq!(
            mission_json(&fx, &a, "M-O")["objective"][1],
            serde_json::json!({"n": 2, "outcome": "Second \u{e9}", "met": true})
        );
        fx.ok(&a, &["mission", "objective", "undone", "M-O", "2"]);
        assert_eq!(mission_json(&fx, &a, "M-O")["objective"][1]["met"], false);

        fx.ok(&a, &["mission", "objective", "remove", "M-O", "3"]);
        fx.ok(&a, &["mission", "objective", "remove", "M-O", "1"]);
        assert_eq!(
            fx.ok(&a, &["mission", "objective", "add", "M-O", "Fourth"]),
            "4\n"
        );
        assert_eq!(
            fx.ok(&a, &["mission", "objective", "list", "M-O"]),
            "2\topen\tSecond \u{e9}\n4\topen\tFourth\n"
        );
        fails(
            &fx,
            &a,
            &["mission", "objective", "done", "M-O", "3"],
            "no objective outcome 3",
        );
        fails(
            &fx,
            &a,
            &["mission", "objective", "remove", "M-O", "1"],
            "no objective outcome 1",
        );
        fails(
            &fx,
            &a,
            &["mission", "objective", "add", "M-O", ""],
            "cannot be empty",
        );
        fails(
            &fx,
            &a,
            &["mission", "objective", "add", "M-NONE", "x"],
            "no such mission",
        );
        fails(
            &fx,
            &a,
            &["mission", "objective", "list", "M-NONE"],
            "no such mission",
        );

        // The counter survives a push and a fresh clone.
        fx.ok(&a, &["mission", "objective", "remove", "M-O", "4"]);
        fx.ok(&a, &["ledger", "push", "Objectives"]);
        let other = fx.b();
        fx.ok(&other, &["ledger", "fetch"]);
        assert_eq!(
            fx.ok(&other, &["mission", "objective", "add", "M-O", "Fifth"]),
            "5\n"
        );
    }

    pub fn documents_are_linked_by_path_and_checked_at_push() {
        let fx = Fixture::fetched();
        let a = fx.a();
        fx.ok(&a, &["mission", "create", "M-D", "Documents"]);
        fx.put(&a, "missions/M-D/report.md", "# Report\n");

        fails(
            &fx,
            &a,
            &[
                "mission",
                "link",
                "M-D",
                "missions/none.md",
                "--role",
                "report",
            ],
            "error: no such document: missions/none.md",
        );
        fails(
            &fx,
            &a,
            &[
                "mission",
                "link",
                "M-D",
                "missions/M-D/report.md",
                "--role",
                "summary",
            ],
            "intelligence, report and design",
        );
        fails(
            &fx,
            &a,
            &["mission", "link", "M-D", "../escape.md", "--role", "report"],
            "no such document",
        );
        fails(
            &fx,
            &a,
            &[
                "mission",
                "link",
                "M-NONE",
                "missions/M-D/report.md",
                "--role",
                "report",
            ],
            "no such mission",
        );
        assert_eq!(
            mission_json(&fx, &a, "M-D")["documents"],
            serde_json::json!([])
        );

        fx.ok(
            &a,
            &[
                "mission",
                "link",
                "M-D",
                "missions/M-D/report.md",
                "--role",
                "report",
            ],
        );
        fx.ok(
            &a,
            &["mission", "link", "M-D", "./index.md", "--role", "design"],
        );
        assert_eq!(
            mission_json(&fx, &a, "M-D")["documents"],
            serde_json::json!([
                {"path": "index.md", "role": "design"},
                {"path": "missions/M-D/report.md", "role": "report"}
            ])
        );
        // Linking again changes the role.
        fx.ok(
            &a,
            &[
                "mission",
                "link",
                "M-D",
                "missions/M-D/report.md",
                "--role",
                "intelligence",
            ],
        );
        assert_eq!(
            mission_json(&fx, &a, "M-D")["documents"][1]["role"],
            "intelligence"
        );
        fx.ok(&a, &["mission", "unlink", "M-D", "index.md"]);
        fails(
            &fx,
            &a,
            &["mission", "unlink", "M-D", "index.md"],
            "does not link index.md",
        );
        fx.ok(&a, &["ledger", "push", "Link a report"]);

        // The document is removed after the link: the push stops and names both.
        assert_success(&fx.tsk(&a, &["ledger", "doc", "rm", "missions/M-D/report.md"]));
        let ledger_before = fx.remote_ledger();
        let stopped = fx.tsk(&a, &["ledger", "push", "Report gone"]);
        assert_exit(&stopped, 1);
        let message = stderr(&stopped);
        assert!(message.contains("M-D"), "{}", message);
        assert!(message.contains("missions/M-D/report.md"), "{}", message);
        assert_eq!(fx.remote_ledger(), ledger_before);
        // The documents had already gone: the removal is on the branch.
        let files = fx.origin_files(DOCS_REF);
        assert!(
            !files.iter().any(|f| f == "missions/M-D/report.md"),
            "{:?}",
            files
        );

        fx.ok(&a, &["mission", "unlink", "M-D", "missions/M-D/report.md"]);
        assert_success(&fx.tsk(&a, &["ledger", "push", "Unlink it"]));
        let other = fx.b();
        fx.ok(&other, &["ledger", "fetch"]);
        assert_eq!(
            mission_json(&fx, &other, "M-D")["documents"],
            serde_json::json!([])
        );
    }

    pub fn mission_writes_stay_local_until_the_push() {
        let fx = Fixture::fetched();
        let (a, other) = (fx.a(), fx.b());
        fx.ok(&a, &["mission", "create", "M-BASE", "Base"]);
        fx.ok(&a, &["ledger", "push", "Base"]);
        fx.ok(&other, &["ledger", "fetch"]);
        let ledger = fx.remote_ledger();

        fx.ok(&a, &["mission", "create", "M-NEW", "New"]);
        fx.ok(&a, &["mission", "set", "M-BASE", "purpose", "changed"]);
        assert_eq!(fx.remote_ledger(), ledger);
        assert_eq!(
            fx.ok(&other, &["mission", "list", "--porcelain"]),
            "M-BASE\ttodo\t-\t-\tBase\n"
        );
        fx.ok(&other, &["ledger", "fetch"]);
        assert_eq!(
            fx.ok(&other, &["mission", "list", "--porcelain"]),
            "M-BASE\ttodo\t-\t-\tBase\n"
        );
        assert_eq!(mission_json(&fx, &other, "M-BASE")["purpose"], "");

        fx.ok(&a, &["ledger", "push", "Publish"]);
        assert_ne!(fx.remote_ledger(), ledger);
        fx.ok(&other, &["ledger", "fetch"]);
        assert_eq!(
            fx.ok(&other, &["mission", "list", "--porcelain"]),
            "M-BASE\ttodo\t-\t-\tBase\nM-NEW\ttodo\t-\t-\tNew\n"
        );
        assert_eq!(mission_json(&fx, &other, "M-BASE")["purpose"], "changed");
    }

    pub fn two_clones_add_missions_that_merge_and_one_field_conflicts() {
        let fx = Fixture::fetched();
        let (a, other) = (fx.a(), fx.b());
        fx.ok(&a, &["mission", "create", "M-BASE", "Base"]);
        fx.ok(&a, &["ledger", "push", "Base"]);
        fx.ok(&other, &["ledger", "fetch"]);

        fx.ok(&a, &["mission", "create", "M-FROM-A", "From A"]);
        fx.ok(&a, &["ledger", "push", "A adds a mission"]);
        fx.ok(&other, &["mission", "create", "M-FROM-B", "From B"]);
        let stale = fx.tsk(&other, &["ledger", "push", "B adds a mission"]);
        assert_success(&stale);
        fx.ok(&a, &["ledger", "fetch"]);
        let expected = "M-BASE\ttodo\t-\t-\tBase\nM-FROM-A\ttodo\t-\t-\tFrom A\nM-FROM-B\ttodo\t-\t-\tFrom B\n";
        assert_eq!(fx.ok(&a, &["mission", "list", "--porcelain"]), expected);
        fx.ok(&other, &["ledger", "fetch"]);
        assert_eq!(fx.ok(&other, &["mission", "list", "--porcelain"]), expected);

        // Different fields of one mission merge.
        fx.ok(&a, &["mission", "set", "M-BASE", "purpose", "from a"]);
        fx.ok(&a, &["ledger", "push", "A sets the purpose"]);
        fx.ok(
            &other,
            &["mission", "constraint", "add", "M-BASE", "from b"],
        );
        assert_success(&fx.tsk(&other, &["ledger", "push", "B adds a constraint"]));
        fx.ok(&a, &["ledger", "fetch"]);
        let merged = mission_json(&fx, &a, "M-BASE");
        assert_eq!(merged["purpose"], "from a");
        assert_eq!(
            merged["constraints"],
            serde_json::json!([{"n": 1, "text": "from b"}])
        );

        // The same field of the same mission: stop, name it, leave the remote alone.
        fx.ok(&other, &["ledger", "fetch"]);
        fx.ok(&a, &["mission", "set", "M-BASE", "status", "in-progress"]);
        fx.ok(&a, &["ledger", "push", "A starts it"]);
        let before = fx.remote_ledger();
        fx.ok(&other, &["mission", "set", "M-BASE", "status", "done"]);
        let conflict = fx.tsk(&other, &["ledger", "push", "B finishes it"]);
        assert_exit(&conflict, 1);
        let message = stderr(&conflict);
        assert!(message.contains("conflicted"), "{}", message);
        assert!(message.contains("missions"), "{}", message);
        assert_eq!(fx.remote_ledger(), before);
    }

    pub fn the_json_record_has_every_field() {
        let fx = Fixture::fetched();
        let a = fx.a();
        fx.ok(&a, &["mission", "create", "M-P", "Parent"]);
        fx.ok(&a, &["mission", "create", "M-Q", "Blocker"]);
        fx.ok(&a, &["mission", "create", "M-J", "Json", "--parent", "M-P"]);
        fx.ok(&a, &["mission", "set", "M-J", "purpose", "Why"]);
        fx.ok(
            &a,
            &["mission", "constraint", "add", "M-J", "No new crates"],
        );
        fx.ok(&a, &["mission", "constraint", "add", "M-J", "Both lists"]);
        fx.ok(
            &a,
            &["mission", "out-of-scope", "add", "M-J", "Projections"],
        );
        fx.ok(&a, &["mission", "objective", "add", "M-J", "One"]);
        fx.ok(&a, &["mission", "objective", "add", "M-J", "Two"]);
        fx.ok(&a, &["mission", "objective", "done", "M-J", "1"]);
        fx.ok(&a, &["mission", "block", "M-J", "M-Q"]);
        fx.put(&a, "m/design.md", "d\n");
        fx.ok(
            &a,
            &["mission", "link", "M-J", "m/design.md", "--role", "design"],
        );

        let one = fx.ok(&a, &["mission", "list", "M-J", "--json"]);
        assert_eq!(one.lines().count(), 1);
        assert_eq!(
            without_uid(json(&one)),
            serde_json::json!({
                "id": "M-J",
                "name": "Json",
                "category": "operational",
                "status": "todo",
                "parent": "M-P",
                "blocked": true,
                "purpose": "Why",
                "decision_authority": "",
                "constraints": [
                    {"n": 1, "text": "No new crates"},
                    {"n": 2, "text": "Both lists"}
                ],
                "out_of_scope": [{"n": 1, "text": "Projections"}],
                "objective": [
                    {"n": 1, "outcome": "One", "met": true},
                    {"n": 2, "outcome": "Two", "met": false}
                ],
                "blocked_by": [{"id": "M-Q", "blocking": true}],
                "documents": [{"path": "m/design.md", "role": "design"}],
                "tasks": [],
                "decisions": [],
                "questions": []
            })
        );
        let all = json(&fx.ok(&a, &["mission", "list", "--json"]));
        let ids: Vec<&str> = all
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m["id"].as_str().unwrap())
            .collect();
        assert_eq!(ids, vec!["M-J", "M-P", "M-Q"]);
        assert_eq!(all[0], json(&one));
        assert_eq!(fx.ok(&a, &["mission", "create", "M-E", "Empty"]), "");
        assert_eq!(
            mission_json(&fx, &a, "M-E")["objective"],
            serde_json::json!([])
        );
    }

    // --- Tasks, decisions and questions ---

    /// `YYYY-MM-DD`: the tests do not depend on today's date, only on its shape.
    fn is_date(text: &str) -> bool {
        let b = text.as_bytes();
        b.len() == 10
            && b.iter().enumerate().all(|(i, c)| match i {
                4 | 7 => *c == b'-',
                _ => c.is_ascii_digit(),
            })
    }

    fn lines(text: &str) -> Vec<Vec<String>> {
        text.lines()
            .map(|l| l.split('\t').map(str::to_string).collect())
            .collect()
    }

    fn mission_with_tasks(tasks: usize) -> (Fixture, PathBuf) {
        let fx = Fixture::fetched();
        let a = fx.a();
        fx.ok(&a, &["mission", "create", "M-T", "Tasks"]);
        for i in 1..=tasks {
            assert_eq!(
                fx.ok(&a, &["task", "add", "M-T", &format!("Task {}", i)]),
                format!("T-{:02}\n", i)
            );
        }
        (fx, a)
    }

    pub fn tasks_are_minted_per_mission_listed_and_changed() {
        let (fx, a) = mission_with_tasks(0);
        fx.ok(&a, &["mission", "create", "M-U", "Other"]);
        assert_eq!(fx.ok(&a, &["task", "list", "M-T", "--porcelain"]), "");
        // The counter is per mission and only grows.
        for i in 1..=12 {
            assert_eq!(
                fx.ok(&a, &["task", "add", "M-T", &format!("Task {}", i)]),
                format!("T-{:02}\n", i)
            );
        }
        assert_eq!(fx.ok(&a, &["task", "add", "M-U", "First of U"]), "T-01\n");
        assert_eq!(fx.ok(&a, &["task", "add", "M-T", "-dash"]), "T-13\n");
        let list = fx.ok(&a, &["task", "list", "M-T", "--porcelain"]);
        assert_eq!(list.lines().count(), 13);
        assert_eq!(list.lines().next().unwrap(), "T-01\ttodo\t-\tTask 1");
        assert_eq!(list.lines().last().unwrap(), "T-13\ttodo\t-\t-dash");
        assert_eq!(
            fx.ok(&a, &["task", "list", "M-U", "--porcelain"]),
            "T-01\ttodo\t-\tFirst of U\n"
        );

        // Status takes three values.
        for status in ["in-progress", "done", "todo"] {
            fx.ok(&a, &["task", "set", "M-T", "T-02", "status", status]);
            assert_eq!(
                fx.ok(&a, &["task", "list", "M-T", "--porcelain"])
                    .lines()
                    .nth(1)
                    .unwrap(),
                format!("T-02\t{}\t-\tTask 2", status)
            );
        }
        fails(
            &fx,
            &a,
            &["task", "set", "M-T", "T-02", "status", "blocked"],
            "todo, in-progress and done",
        );
        fx.ok(
            &a,
            &["task", "set", "M-T", "T-02", "description", "Renamed"],
        );
        assert_eq!(
            fx.ok(&a, &["task", "list", "M-T", "--porcelain"])
                .lines()
                .nth(1)
                .unwrap(),
            "T-02\ttodo\t-\tRenamed"
        );
        fails(
            &fx,
            &a,
            &["task", "set", "M-T", "T-02", "colour", "red"],
            "not a task field",
        );
        fails(
            &fx,
            &a,
            &["task", "set", "M-T", "T-02", "description", "two\nlines"],
            "one line",
        );

        // Unknown IDs name the ID.
        fails(
            &fx,
            &a,
            &["task", "add", "M-NONE", "x"],
            "no such mission: M-NONE",
        );
        fails(
            &fx,
            &a,
            &["task", "list", "M-NONE", "--porcelain"],
            "no such mission: M-NONE",
        );
        fails(
            &fx,
            &a,
            &["task", "set", "M-T", "T-99", "status", "done"],
            "M-T has no task T-99",
        );
        fails(
            &fx,
            &a,
            &["task", "set", "M-NONE", "T-01", "status", "done"],
            "M-NONE",
        );
        fails(
            &fx,
            &a,
            &["task", "set", "M-T", "task-1", "status", "done"],
            "not a task ID",
        );
        fails(&fx, &a, &["task", "add", "M-T", "bad\nline"], "one line");
        fails(&fx, &a, &["task", "add", "M-T", "bad\ttab"], "one line");
        fails(&fx, &a, &["task", "add", "M-T", ""], "cannot be empty");
        // A refused add mints nothing.
        assert_eq!(fx.ok(&a, &["task", "add", "M-T", "Fourteenth"]), "T-14\n");

        // The records survive a push and a fresh clone.
        fx.ok(&a, &["ledger", "push", "Tasks"]);
        let other = fx.b();
        fx.ok(&other, &["ledger", "fetch"]);
        assert_eq!(
            fx.ok(&other, &["task", "list", "M-T", "--porcelain"]),
            fx.ok(&a, &["task", "list", "M-T", "--porcelain"])
        );
        assert_eq!(fx.ok(&other, &["task", "add", "M-T", "From B"]), "T-15\n");
    }

    pub fn task_blocks_are_checked_and_blocked_is_derived() {
        let (fx, a) = mission_with_tasks(3);
        fx.ok(&a, &["mission", "create", "M-U", "Other"]);
        fx.ok(&a, &["task", "add", "M-U", "In U"]);

        assert_eq!(fx.ok(&a, &["task", "block", "M-T", "T-01", "T-02"]), "");
        fx.ok(&a, &["task", "block", "M-T", "T-01", "T-03"]);
        fx.ok(&a, &["task", "block", "M-T", "T-02", "T-03"]);
        assert_eq!(
            fx.ok(&a, &["task", "list", "M-T", "--porcelain"]),
            "T-01\ttodo\tT-02,T-03\tTask 1\n\
             T-02\ttodo\tT-03\tTask 2\n\
             T-03\ttodo\t-\tTask 3\n"
        );
        let json = mission_json(&fx, &a, "M-T");
        assert_eq!(json["tasks"][0]["blocked"], true);
        assert_eq!(json["tasks"][0]["id"], "T-01");
        assert_eq!(
            json["tasks"][0]["blocked_by"],
            serde_json::json!([
                {"id": "T-02", "blocking": true},
                {"id": "T-03", "blocking": true}
            ])
        );
        assert_eq!(json["tasks"][2]["blocked"], false);

        // Done blockers stop blocking; the link stays.
        fx.ok(&a, &["task", "set", "M-T", "T-03", "status", "done"]);
        assert_eq!(
            fx.ok(&a, &["task", "list", "M-T", "--porcelain"]),
            "T-01\ttodo\tT-02\tTask 1\n\
             T-02\ttodo\t-\tTask 2\n\
             T-03\tdone\t-\tTask 3\n"
        );
        let json = mission_json(&fx, &a, "M-T");
        assert_eq!(json["tasks"][1]["blocked"], false);
        assert_eq!(
            json["tasks"][0]["blocked_by"],
            serde_json::json!([
                {"id": "T-02", "blocking": true},
                {"id": "T-03", "blocking": false}
            ])
        );
        fx.ok(&a, &["task", "set", "M-T", "T-03", "status", "in-progress"]);
        assert!(fx
            .ok(&a, &["task", "list", "M-T", "--porcelain"])
            .contains("T-02,T-03"));

        let before = fx.ok(&a, &["task", "list", "M-T", "--porcelain"]);
        fails(&fx, &a, &["task", "block", "M-T", "T-01", "T-01"], "itself");
        fails(
            &fx,
            &a,
            &["task", "block", "M-T", "T-01", "T-02"],
            "already blocked",
        );
        fails(
            &fx,
            &a,
            &["task", "block", "M-T", "T-03", "T-01"],
            "cycle (T-03 -> T-01 -> T-02 -> T-03)",
        );
        fails(&fx, &a, &["task", "block", "M-T", "T-01", "T-09"], "T-09");
        fails(&fx, &a, &["task", "block", "M-T", "T-09", "T-01"], "T-09");
        fails(
            &fx,
            &a,
            &["task", "block", "M-NONE", "T-01", "T-02"],
            "M-NONE",
        );
        // A task of another mission cannot be a blocker.
        fails(
            &fx,
            &a,
            &["task", "block", "M-U", "T-01", "T-02"],
            "M-U has no task T-02",
        );
        fails(
            &fx,
            &a,
            &["task", "unblock", "M-T", "T-03", "T-01"],
            "not blocked",
        );
        assert_eq!(fx.ok(&a, &["task", "list", "M-T", "--porcelain"]), before);

        assert_eq!(fx.ok(&a, &["task", "unblock", "M-T", "T-01", "T-03"]), "");
        assert!(fx
            .ok(&a, &["task", "list", "M-T", "--porcelain"])
            .starts_with("T-01\ttodo\tT-02\tTask 1\n"));
        fx.ok(&a, &["task", "unblock", "M-T", "T-01", "T-02"]);
        assert!(fx
            .ok(&a, &["task", "list", "M-T", "--porcelain"])
            .starts_with("T-01\ttodo\t-\tTask 1\n"));
        // With the chain cut, the reverse block is allowed.
        fx.ok(&a, &["task", "unblock", "M-T", "T-02", "T-03"]);
        fx.ok(&a, &["task", "block", "M-T", "T-03", "T-01"]);

        // Blocks survive a push to another clone.
        fx.ok(&a, &["ledger", "push", "Blocks"]);
        let other = fx.b();
        fx.ok(&other, &["ledger", "fetch"]);
        assert!(fx
            .ok(&other, &["task", "list", "M-T", "--porcelain"])
            .contains("T-03\tin-progress\tT-01\tTask 3"));
    }

    pub fn decisions_are_recorded_with_who_when_and_an_optional_task() {
        let (fx, a) = mission_with_tasks(2);
        fx.ok(&a, &["mission", "create", "M-U", "Other"]);
        fx.ok(&a, &["task", "add", "M-U", "In U"]);
        assert_eq!(fx.ok(&a, &["decision", "list", "M-T", "--porcelain"]), "");
        assert_eq!(
            fx.ok(&a, &["decision", "add", "M-T", "Use Dolt", "--by", "Jim"]),
            ""
        );
        fx.ok(
            &a,
            &[
                "decision",
                "add",
                "M-T",
                "Split the task",
                "--by",
                "an agent",
                "--task",
                "T-02",
            ],
        );
        fx.ok(
            &a,
            &["decision", "add", "M-T", "-leading dash", "--by", "Jim"],
        );
        let rows = lines(&fx.ok(&a, &["decision", "list", "M-T", "--porcelain"]));
        assert_eq!(rows.len(), 3);
        // Oldest first: the order they were added.
        let texts: Vec<&str> = rows.iter().map(|r| r[3].as_str()).collect();
        assert_eq!(texts, vec!["Use Dolt", "Split the task", "-leading dash"]);
        for row in &rows {
            // Date, who, task, text and the ADR path, which is `-` when none is named.
            assert_eq!(row.len(), 5);
            assert!(is_date(&row[0]), "{:?}", row);
            assert_eq!(row[4], "-");
        }
        assert_eq!(rows[0][1..3], ["Jim", "-"]);
        assert_eq!(rows[1][1..3], ["an agent", "T-02"]);
        assert_eq!(fx.ok(&a, &["decision", "list", "M-U", "--porcelain"]), "");

        let before = fx.ok(&a, &["decision", "list", "M-T", "--porcelain"]);
        fails(
            &fx,
            &a,
            &[
                "decision", "add", "M-T", "x", "--by", "Jim", "--task", "T-09",
            ],
            "M-T has no task T-09",
        );
        // A task of another mission does not count.
        fails(
            &fx,
            &a,
            &[
                "decision", "add", "M-U", "x", "--by", "Jim", "--task", "T-02",
            ],
            "M-U has no task T-02",
        );
        fails(
            &fx,
            &a,
            &[
                "decision", "add", "M-T", "x", "--by", "Jim", "--task", "nope",
            ],
            "not a task ID",
        );
        fails(
            &fx,
            &a,
            &["decision", "add", "M-NONE", "x", "--by", "Jim"],
            "M-NONE",
        );
        fails(
            &fx,
            &a,
            &["decision", "list", "M-NONE", "--porcelain"],
            "M-NONE",
        );
        fails(
            &fx,
            &a,
            &["decision", "add", "M-T", "two\nlines", "--by", "Jim"],
            "one line",
        );
        fails(
            &fx,
            &a,
            &["decision", "add", "M-T", "x", "--by", "Ji\nm"],
            "one line",
        );
        fails(
            &fx,
            &a,
            &["decision", "add", "M-T", "x", "--by", ""],
            "cannot be empty",
        );
        assert_eq!(
            fx.ok(&a, &["decision", "list", "M-T", "--porcelain"]),
            before
        );

        let json = mission_json(&fx, &a, "M-T");
        assert_eq!(json["decisions"][1]["task"], "T-02");
        assert_eq!(json["decisions"][1]["decided_by"], "an agent");
        assert_eq!(json["decisions"][0]["task"], serde_json::Value::Null);
        assert!(is_date(
            json["decisions"][0]["decided_on"].as_str().unwrap()
        ));

        fx.ok(&a, &["ledger", "push", "Decisions"]);
        let other = fx.b();
        fx.ok(&other, &["ledger", "fetch"]);
        assert_eq!(
            fx.ok(&other, &["decision", "list", "M-T", "--porcelain"]),
            before
        );
        fx.ok(&other, &["decision", "add", "M-T", "From B", "--by", "Jim"]);
        assert_eq!(
            fx.ok(&other, &["decision", "list", "M-T", "--porcelain"])
                .lines()
                .count(),
            4
        );
    }

    pub fn questions_are_minted_answered_and_listed_while_open() {
        let (fx, a) = mission_with_tasks(0);
        fx.ok(&a, &["mission", "create", "M-U", "Other"]);
        assert_eq!(fx.ok(&a, &["question", "list", "--porcelain"]), "");
        assert_eq!(fx.ok(&a, &["question", "add", "M-T", "Why one?"]), "Q-01\n");
        assert_eq!(fx.ok(&a, &["question", "add", "M-T", "Why two?"]), "Q-02\n");
        assert_eq!(
            fx.ok(&a, &["question", "add", "M-U", "Why in U?"]),
            "Q-01\n"
        );
        assert_eq!(fx.ok(&a, &["question", "add", "M-T", "-dash?"]), "Q-03\n");

        let rows = lines(&fx.ok(&a, &["question", "list", "--porcelain"]));
        assert_eq!(rows.len(), 4);
        // Across all missions, by mission and then number.
        let keys: Vec<String> = rows.iter().map(|r| format!("{} {}", r[0], r[1])).collect();
        assert_eq!(keys, vec!["M-T Q-01", "M-T Q-02", "M-T Q-03", "M-U Q-01"]);
        for row in &rows {
            assert_eq!(row.len(), 4);
            assert!(is_date(&row[2]), "{:?}", row);
        }
        assert_eq!(rows[0][3], "Why one?");
        assert_eq!(
            fx.ok(&a, &["question", "list", "M-U", "--porcelain"])
                .lines()
                .count(),
            1
        );

        assert_eq!(
            fx.ok(
                &a,
                &["question", "answer", "M-T", "Q-02", "Because.", "--by", "Jim"]
            ),
            ""
        );
        let open = fx.ok(&a, &["question", "list", "M-T", "--porcelain"]);
        assert_eq!(open.lines().count(), 2);
        assert!(!open.contains("Why two?"));
        assert_eq!(
            fx.ok(&a, &["question", "list", "--porcelain"])
                .lines()
                .count(),
            3
        );
        let json = mission_json(&fx, &a, "M-T");
        let q = &json["questions"][1];
        assert_eq!(q["id"], "Q-02");
        assert_eq!(q["status"], "answered");
        assert_eq!(q["answer"], "Because.");
        assert_eq!(q["answered_by"], "Jim");
        assert!(is_date(q["answered_on"].as_str().unwrap()));
        assert_eq!(json["questions"][0]["status"], "open");
        assert_eq!(json["questions"][0]["answer"], serde_json::Value::Null);

        fails(
            &fx,
            &a,
            &["question", "answer", "M-T", "Q-02", "Again", "--by", "Jim"],
            "already answered",
        );
        fails(
            &fx,
            &a,
            &["question", "answer", "M-T", "Q-09", "x", "--by", "Jim"],
            "M-T has no question Q-09",
        );
        fails(
            &fx,
            &a,
            &["question", "answer", "M-U", "Q-02", "x", "--by", "Jim"],
            "M-U has no question Q-02",
        );
        fails(
            &fx,
            &a,
            &["question", "answer", "M-NONE", "Q-01", "x", "--by", "Jim"],
            "M-NONE",
        );
        fails(
            &fx,
            &a,
            &["question", "answer", "M-T", "T-01", "x", "--by", "Jim"],
            "not a question ID",
        );
        fails(
            &fx,
            &a,
            &[
                "question",
                "answer",
                "M-T",
                "Q-01",
                "two\nlines",
                "--by",
                "Jim",
            ],
            "one line",
        );
        fails(
            &fx,
            &a,
            &["question", "add", "M-T", "two\nlines"],
            "one line",
        );
        fails(&fx, &a, &["question", "add", "M-NONE", "x"], "M-NONE");
        fails(
            &fx,
            &a,
            &["question", "list", "M-NONE", "--porcelain"],
            "M-NONE",
        );
        // The refused answer left Q-01 open, and nothing was minted.
        assert_eq!(fx.ok(&a, &["question", "list", "M-T", "--porcelain"]), open);
        assert_eq!(fx.ok(&a, &["question", "add", "M-T", "Next"]), "Q-04\n");

        fx.ok(&a, &["ledger", "push", "Questions"]);
        let other = fx.b();
        fx.ok(&other, &["ledger", "fetch"]);
        assert_eq!(
            fx.ok(&other, &["question", "list", "--porcelain"]),
            fx.ok(&a, &["question", "list", "--porcelain"])
        );
    }

    pub fn two_clones_add_records_to_different_missions_and_the_ledgers_merge() {
        let fx = Fixture::fetched();
        let (a, other) = (fx.a(), fx.b());
        fx.ok(&a, &["mission", "create", "M-ONE", "One"]);
        fx.ok(&a, &["mission", "create", "M-TWO", "Two"]);
        fx.ok(&a, &["ledger", "push", "Base"]);
        fx.ok(&other, &["ledger", "fetch"]);
        fx.ok(&a, &["task", "add", "M-ONE", "From A"]);
        fx.ok(&a, &["question", "add", "M-ONE", "From A?"]);
        fx.ok(&other, &["task", "add", "M-TWO", "From B"]);
        fx.ok(
            &other,
            &["decision", "add", "M-TWO", "From B", "--by", "Jim"],
        );
        fx.ok(&a, &["ledger", "push", "A"]);
        fx.ok(&other, &["ledger", "push", "B"]);
        fx.ok(&a, &["ledger", "fetch"]);
        for dir in [&a, &other] {
            fx.ok(dir, &["ledger", "fetch"]);
            assert_eq!(
                fx.ok(dir, &["task", "list", "M-ONE", "--porcelain"]),
                "T-01\ttodo\t-\tFrom A\n"
            );
            assert_eq!(
                fx.ok(dir, &["task", "list", "M-TWO", "--porcelain"]),
                "T-01\ttodo\t-\tFrom B\n"
            );
            assert_eq!(
                fx.ok(dir, &["question", "list", "--porcelain"])
                    .lines()
                    .count(),
                1
            );
            assert_eq!(
                fx.ok(dir, &["decision", "list", "M-TWO", "--porcelain"])
                    .lines()
                    .count(),
                1
            );
        }
    }

    pub fn thread_start_takes_a_mission_id_and_checks_it_against_records_and_briefings() {
        let fx = Fixture::with_briefing();
        let a = fx.a();

        // A mission known only by its briefing document stores the briefing path.
        let first = fx.started(&a, &["thread", "start", "M-CONF-01"]);
        let row = fx.thread_row(&a, &first);
        assert_eq!(row["mission_id"], "M-CONF-01");
        assert_eq!(row["mission_link"], BRIEFING);
        fx.ok(&a, &["thread", "detach"]);

        // An unknown mission is refused with an error that names it, and writes nothing.
        let before = fx.remote_ledger();
        let refused = fx.tsk(&a, &["thread", "start", "M-NONE"]);
        assert_exit(&refused, 1);
        assert!(
            stderr(&refused).contains("no such mission: M-NONE"),
            "{}",
            stderr(&refused)
        );
        assert_eq!(stdout(&refused), "");
        assert_exit(&fx.tsk(&a, &["thread", "binding", "--no-fetch"]), 1);
        assert_eq!(fx.remote_ledger(), before);

        // A sub-mission's briefing does not make its parent exist.
        fx.put(&a, "missions/operational/M-CONF-01-02-sub.md", "# Sub\n");
        fx.ok(&a, &["ledger", "push", "Add a sub-mission briefing"]);
        let refused = fx.tsk(&a, &["thread", "start", "M-CONF"]);
        assert_exit(&refused, 1);
        assert!(stderr(&refused).contains("M-CONF"), "{}", stderr(&refused));
        let sub = fx.started(&a, &["thread", "start", "M-CONF-01-02"]);
        assert_eq!(fx.thread_row(&a, &sub)["mission_id"], "M-CONF-01-02");
        fx.ok(&a, &["thread", "detach"]);

        // A mission with a record stores the ID alone.
        fx.ok(&a, &["mission", "create", "M-REC", "Recorded"]);
        fx.ok(&a, &["ledger", "push", "Add M-REC"]);
        let recorded = fx.started(&a, &["thread", "start", "M-REC"]);
        let row = fx.thread_row(&a, &recorded);
        assert_eq!(row["mission_id"], "M-REC");
        assert_eq!(row["mission_link"], "");
        fx.ok(&a, &["thread", "detach"]);

        // The old two-argument form still works. The second argument is ignored.
        let old = fx.tsk(&a, &["thread", "start", "M-REC", "missions/ignored.md"]);
        assert_success(&old);
        assert!(stdout(&old).starts_with("started:"), "{}", stdout(&old));
        assert!(
            stderr(&old).contains("no longer needed"),
            "{}",
            stderr(&old)
        );
        let old_id = stdout(&old)
            .trim_end()
            .trim_start_matches("started:")
            .to_string();
        assert_eq!(fx.thread_row(&a, &old_id)["mission_link"], "");
        fx.ok(&a, &["thread", "detach"]);

        // A record wins over a briefing document for the same mission.
        fx.ok(&a, &["mission", "create", "M-CONF-01", "Both"]);
        fx.ok(&a, &["ledger", "push", "Add the record"]);
        let both = fx.started(&a, &["thread", "start", "M-CONF-01"]);
        let row = fx.thread_row(&a, &both);
        assert_eq!(row["mission_id"], "M-CONF-01");
        assert_eq!(row["mission_link"], "");
    }

    pub fn thread_pause_checks_the_task_against_the_mission_record() {
        let fx = Fixture::fetched();
        let a = fx.a();
        fx.ok(&a, &["mission", "create", "M-REC", "Recorded"]);
        fx.ok(&a, &["task", "add", "M-REC", "First task"]);
        fx.put(&a, "missions/operational/M-LEG-legacy.md", "# Legacy\n");
        fx.ok(&a, &["ledger", "push", "Add the missions"]);

        let id = fx.started(&a, &["thread", "start", "M-REC"]);

        // A task of the mission is accepted, and T-1 is T-01.
        assert_eq!(
            fx.ok(&a, &["thread", "pause", &id, "T-1", "first"]),
            format!(
                "paused:{}\ncode: clean\ncode pushed: yes\nledger: pushed\n",
                id
            )
        );
        let latest = fx.latest_entry(&a, &id);
        assert_eq!(latest["mission_id"], "M-REC");
        assert_eq!(latest["task_id"], "T-01");
        assert_eq!(latest["whats_next"], "first");
        assert!(latest.get("mission_link").is_none(), "{}", latest);
        for key in ["git", "timestamp", "written_by"] {
            assert!(latest.get(key).is_some(), "{}: {}", key, latest);
        }

        // A task the mission does not have is refused, by name, and nothing is recorded.
        let before = fx.remote_ledger();
        let refused = fx.tsk(&a, &["thread", "pause", &id, "T-09", "second"]);
        assert_exit(&refused, 1);
        assert!(stderr(&refused).contains("T-09"), "{}", stderr(&refused));
        assert_exit(&fx.tsk(&a, &["thread", "pause", &id, "later", "second"]), 1);
        assert_eq!(fx.remote_ledger(), before);
        assert_eq!(fx.latest_entry(&a, &id)["whats_next"], "first");

        // An empty task is accepted.
        fx.ok(&a, &["thread", "pause", &id, "", "no task"]);
        let latest = fx.latest_entry(&a, &id);
        assert_eq!(latest["task_id"], "");
        assert_eq!(latest["whats_next"], "no task");

        // The old four-argument form is accepted. The mission link is ignored.
        let old = fx.tsk(
            &a,
            &[
                "thread",
                "pause",
                &id,
                "missions/ignored.md",
                "T-01",
                "old form",
            ],
        );
        assert_success(&old);
        assert_eq!(
            stdout(&old),
            format!(
                "paused:{}\ncode: clean\ncode pushed: yes\nledger: pushed\n",
                id
            )
        );
        assert!(
            stderr(&old).contains("no longer needed"),
            "{}",
            stderr(&old)
        );
        let latest = fx.latest_entry(&a, &id);
        assert_eq!(latest["task_id"], "T-01");
        assert_eq!(latest["whats_next"], "old form");
        assert!(latest.get("mission_link").is_none(), "{}", latest);
        // The old form still checks the task.
        assert_exit(
            &fx.tsk(
                &a,
                &["thread", "pause", &id, "missions/ignored.md", "T-09", "x"],
            ),
            1,
        );

        // A mission with no record has no tasks to check against, and its entries carry
        // the briefing path.
        fx.ok(&a, &["thread", "detach"]);
        let legacy = fx.started(&a, &["thread", "start", "M-LEG"]);
        fx.ok(&a, &["thread", "pause", &legacy, "T-99", "unchecked"]);
        let latest = fx.latest_entry(&a, &legacy);
        assert_eq!(latest["mission_id"], "M-LEG");
        assert_eq!(
            latest["mission_link"],
            "missions/operational/M-LEG-legacy.md"
        );
        assert_eq!(latest["task_id"], "T-99");
    }

    /// Put a thread row and one entry in the ledger store the way an older tsk wrote
    /// them, and push: no mission ID, only a briefing path.
    fn seed_legacy_thread(fx: &Fixture, dir: &Path, id: &str) {
        let entry = format!(
            "{{\"mission_link\":\"{}\",\"task_id\":\"T-01\",\"whats_next\":\"old\",\"written_by\":\"urn:tsk:worktree:old\"}}",
            BRIEFING
        );
        fx.dolt_sql(
            dir,
            &format!(
                "insert into threads (thread_id, briefing_path) values ('{}', '{}'); \
                 insert into continuation_entries (thread_id, seq, entry) values ('{}', 1, '{}')",
                id, BRIEFING, id, entry
            ),
        );
        fx.ok(dir, &["ledger", "push", "Seed a legacy thread"]);
    }

    pub fn legacy_thread_rows_read_with_a_mission_id_derived_from_the_briefing_path() {
        let fx = Fixture::with_briefing();
        let a = fx.a();
        seed_legacy_thread(&fx, &a, "aaaa1111");

        let row = fx.thread_row(&a, "aaaa1111");
        assert_eq!(row["mission_id"], "M-CONF-01");
        assert_eq!(row["mission_link"], BRIEFING);
        assert_eq!(row["latest_whats_next"], "old");

        // A pause on the legacy thread records the derived mission and the briefing path.
        fx.ok(&a, &["thread", "pause", "aaaa1111", "T-77", "new"]);
        let latest = fx.latest_entry(&a, "aaaa1111");
        assert_eq!(latest["mission_id"], "M-CONF-01");
        assert_eq!(latest["mission_link"], BRIEFING);
        assert_eq!(latest["task_id"], "T-77");

        // The derived ID is not written back.
        assert_eq!(
            fx.dolt_sql(
                &a,
                "select mission_id from threads where thread_id = 'aaaa1111'"
            ),
            "mission_id\n\n"
        );
        // Another clone reads the same row.
        let other = fx.b();
        fx.ok(&other, &["ledger", "fetch"]);
        assert_eq!(fx.thread_row(&other, "aaaa1111")["mission_id"], "M-CONF-01");
    }

    pub fn the_stop_hook_check_passes_when_the_ledger_has_no_missions() {
        let fx = Fixture::fetched();
        let a = fx.a();
        assert_eq!(fx.guard_reason(&a, None), None);
        assert_eq!(stdout(&fx.guard(&a, None)), "");

        // A record alone is a mission: the reason offers the missions not done.
        fx.ok(&a, &["mission", "create", "M-REC", "Recorded"]);
        fx.ok(&a, &["mission", "create", "M-DONE", "Finished"]);
        fx.ok(&a, &["mission", "set", "M-DONE", "status", "done"]);
        fx.ok(&a, &["ledger", "push", "Add missions"]);
        let reason = fx.guard_reason(&a, None).expect("blocks");
        assert!(
            reason.starts_with("No thread binding was found for this session or worktree."),
            "{}",
            reason
        );
        assert!(reason.contains("M-REC\ttodo\t-\t-\tRecorded"), "{}", reason);
        assert!(!reason.contains("M-DONE"), "{}", reason);
        assert!(!reason.contains("index.md"), "{}", reason);
        assert!(reason.contains("`tsk thread skip`"), "{}", reason);

        // A bound session passes.
        fx.started(&a, &["thread", "start", "M-REC"]);
        assert_eq!(fx.guard_reason(&a, None), None);
    }

    pub fn a_mission_document_alone_makes_the_stop_hook_check_block() {
        let fx = Fixture::with_briefing();
        let a = fx.a();
        let reason = fx.guard_reason(&a, None).expect("blocks");
        assert!(reason.contains("`tsk mission list`"), "{}", reason);
        assert!(!reason.contains("index.md"), "{}", reason);
        assert!(reason.contains("`tsk thread skip`"), "{}", reason);
    }

    pub fn skip_silences_the_stop_hook_check_until_a_thread_starts_or_resumes() {
        let fx = Fixture::with_briefing();
        let a = fx.a();
        assert!(fx.guard_reason(&a, None).is_some());

        assert_eq!(fx.ok(&a, &["thread", "skip"]), "skipped\n");
        assert_eq!(fx.guard_reason(&a, None), None);
        // The skip is local: the ledger has nothing to commit.
        let push = fx.tsk(&a, &["ledger", "push", "Nothing"]);
        assert_success(&push);
        assert!(
            stderr(&push).contains("nothing to commit"),
            "{}",
            stderr(&push)
        );
        // And another clone knows nothing of it.
        assert!(fx.guard_reason(&fx.b(), None).is_some());

        // Start clears the skip.
        let id = fx.started(&a, &["thread", "start", "M-CONF-01"]);
        assert_eq!(fx.guard_reason(&a, None), None);
        fx.ok(&a, &["thread", "detach"]);
        assert!(fx.guard_reason(&a, None).is_some());
        fx.ok(&a, &["ledger", "push", "Publish the detach"]);

        // So does resume.
        fx.ok(&a, &["thread", "skip"]);
        assert_eq!(fx.guard_reason(&a, None), None);
        fx.ok(&a, &["thread", "resume", &id]);
        fx.ok(&a, &["ledger", "push", "Publish the binding"]);
        assert_eq!(fx.guard_reason(&a, None), None);
        fx.ok(&a, &["thread", "detach"]);
        assert!(fx.guard_reason(&a, None).is_some());
        fx.ok(&a, &["ledger", "push", "Publish the detach"]);

        // A cloud session skips for itself only.
        let skipped = fx.tsk_cloud(&a, "cse_skip", &["thread", "skip"]);
        assert_eq!(stdout(&skipped), "skipped\n");
        assert_eq!(fx.guard_reason(&a, Some("cse_skip")), None);
        assert!(fx.guard_reason(&a, Some("cse_other")).is_some());
        assert!(fx.guard_reason(&a, None).is_some());
        fx.tsk_cloud(&a, "cse_skip", &["thread", "start", "M-CONF-01"]);
        fx.tsk_cloud(&a, "cse_skip", &["thread", "detach"]);
        assert!(fx.guard_reason(&a, Some("cse_skip")).is_some());
    }

    pub fn session_start_names_the_bound_mission_with_its_open_tasks() {
        let fx = Fixture::fetched();
        let a = fx.a();
        fx.ok(&a, &["mission", "create", "M-REC", "Recorded mission"]);
        fx.ok(&a, &["mission", "create", "M-DONE", "Finished mission"]);
        fx.ok(&a, &["mission", "set", "M-DONE", "status", "done"]);
        fx.ok(&a, &["task", "add", "M-REC", "Open one"]);
        fx.ok(&a, &["task", "add", "M-REC", "Closed one"]);
        fx.ok(&a, &["task", "add", "M-REC", "Waits"]);
        fx.ok(&a, &["task", "block", "M-REC", "T-03", "T-01"]);
        fx.ok(&a, &["task", "set", "M-REC", "T-02", "status", "done"]);
        fx.ok(&a, &["ledger", "push", "Add the missions"]);

        // Unbound: the missions not done, and no index.
        let unbound = fx.session_context(&a, "s-unbound", "claude");
        assert!(
            unbound.contains("M-REC\ttodo\t-\t-\tRecorded mission"),
            "{}",
            unbound
        );
        assert!(!unbound.contains("M-DONE"), "{}", unbound);
        assert!(!unbound.contains("index.md"), "{}", unbound);
        assert!(
            unbound.contains("No thread binding was found"),
            "{}",
            unbound
        );
        assert!(unbound.contains("`tsk thread skip`"), "{}", unbound);
        assert!(!unbound.contains("tsk guide import"), "{}", unbound);

        // Bound: the mission and its open tasks only.
        let id = fx.started(&a, &["thread", "start", "M-REC"]);
        for (session, format) in [("s-bound", "claude"), ("s-bound-copilot", "copilot")] {
            let bound = fx.session_context(&a, session, format);
            assert!(
                bound.contains("This thread works mission M-REC (todo): Recorded mission."),
                "{}",
                bound
            );
            assert!(bound.contains("T-01\ttodo\t-\tOpen one"), "{}", bound);
            // The last task line ends with a line break, not a space.
            assert!(
                bound.contains(
                    "T-01\ttodo\t-\tOpen one\nT-03\ttodo\tT-01\tWaits\nAn existing thread binding"
                ),
                "{}",
                bound
            );
            assert!(!bound.contains("Closed one"), "{}", bound);
            assert!(!bound.contains("index.md"), "{}", bound);
            assert!(!bound.contains("M-DONE"), "{}", bound);
            assert!(
                bound.contains(&format!(
                    "An existing thread binding was found: worktree:{}. Run /tsk:resume-thread {} next.",
                    id, id
                )),
                "{}",
                bound
            );
            assert!(!bound.contains("No thread binding"), "{}", bound);
        }

        // A mission with every task done has no open tasks.
        fx.ok(&a, &["task", "set", "M-REC", "T-01", "status", "done"]);
        fx.ok(&a, &["task", "set", "M-REC", "T-03", "status", "done"]);
        fx.ok(&a, &["ledger", "push", "Close the tasks"]);
        let closed = fx.session_context(&a, "s-closed", "claude");
        assert!(closed.contains("It has no open tasks."), "{}", closed);
    }

    pub fn thread_commands_carry_local_record_and_document_changes_to_the_remote() {
        let fx = Fixture::fetched();
        let (a, other) = (fx.a(), fx.b());

        // Records and a document written since the last push.
        fx.ok(&a, &["mission", "create", "M-REC", "Recorded"]);
        fx.ok(&a, &["task", "add", "M-REC", "First"]);
        fx.put(&a, "notes/n.md", "note\n");

        // Session start leaves them in place, says so, and does not report a failure.
        let context = fx.session_context(&a, "s-local-1", "claude");
        assert!(
            context.contains("local changes that are not pushed"),
            "{}",
            context
        );
        assert!(!context.contains("could not be fetched"), "{}", context);
        assert!(context.contains("M-REC\ttodo"), "{}", context);
        assert_eq!(
            fx.ok(&a, &["task", "list", "M-REC", "--porcelain"]),
            "T-01\ttodo\t-\tFirst\n"
        );

        // Thread list reads with them.
        assert_eq!(fx.ok(&a, &["thread", "list"]), "");

        // Start carries them: one push, and the remote has the records and the document.
        let id = fx.started(&a, &["thread", "start", "M-REC"]);
        fx.ok(&other, &["ledger", "fetch"]);
        assert_eq!(
            fx.ok(&other, &["task", "list", "M-REC", "--porcelain"]),
            "T-01\ttodo\t-\tFirst\n"
        );
        assert_eq!(
            fx.ok(&other, &["ledger", "doc", "get", "notes/n.md"]),
            "note\n"
        );
        assert_eq!(fx.thread_row(&other, &id)["mission_id"], "M-REC");
        let context = fx.session_context(&a, "s-local-2", "claude");
        assert!(
            !context.contains("local changes that are not pushed"),
            "{}",
            context
        );

        // Pause carries a new record.
        fx.ok(&a, &["task", "add", "M-REC", "Second"]);
        fx.ok(&a, &["thread", "pause", &id, "T-02", "carry on"]);
        fx.ok(&other, &["ledger", "fetch"]);
        assert_eq!(
            fx.ok(&other, &["task", "list", "M-REC", "--porcelain"])
                .lines()
                .count(),
            2
        );
        assert_eq!(fx.latest_entry(&other, &id)["task_id"], "T-02");

        // Session start with the changes pending again, and a detach that does not refuse
        // them. A worktree detach pushes like a cloud one, with the pending records.
        fx.ok(&a, &["mission", "create", "M-TWO", "Two"]);
        let context = fx.session_context(&a, "s-local-3", "claude");
        assert!(
            context.contains("local changes that are not pushed"),
            "{}",
            context
        );
        assert!(
            context.contains("This thread works mission M-REC"),
            "{}",
            context
        );
        fx.ok(&a, &["thread", "detach"]);
        fx.ok(&a, &["ledger", "push", "Push the rest"]);
        fx.ok(&other, &["ledger", "fetch"]);
        assert!(fx
            .ok(&other, &["mission", "list", "--porcelain"])
            .contains("M-TWO"));
        // `ledger fetch` itself still refuses uncommitted changes.
        fx.ok(&a, &["mission", "create", "M-THREE", "Three"]);
        assert_exit(&fx.tsk(&a, &["ledger", "fetch"]), 1);
    }

    pub fn session_start_names_no_index_for_a_bound_mission_with_no_record() {
        let fx = Fixture::with_briefing();
        let a = fx.a();
        let id = fx.started(&a, &["thread", "start", "M-CONF-01"]);
        let context = fx.session_context(&a, "s-legacy", "claude");
        assert!(!context.contains("index.md"), "{}", context);
        assert!(
            !context.contains("This thread works mission"),
            "{}",
            context
        );
        assert!(context.contains(&format!("worktree:{}", id)), "{}", context);
    }

    pub fn session_start_shows_the_empty_ledger_note_only_without_records_and_documents() {
        let fx = Fixture::fetched();
        let a = fx.a();
        let empty = fx.session_context(&a, "s-empty", "claude");
        assert!(empty.contains("run `tsk guide import`"), "{}", empty);
        // The unbound prompt names `tsk mission list`, never the retired index.
        assert!(!empty.contains("index.md"), "{}", empty);
        assert!(empty.contains("`tsk mission list`"), "{}", empty);
        assert!(empty.contains("`tsk thread skip`"), "{}", empty);

        fx.ok(&a, &["mission", "create", "M-REC", "Recorded"]);
        fx.ok(&a, &["ledger", "push", "Add a record"]);
        let with_record = fx.session_context(&a, "s-record", "claude");
        assert!(!with_record.contains("tsk guide import"), "{}", with_record);
    }

    pub fn session_start_says_a_skipped_session_works_without_a_thread() {
        let fx = Fixture::with_briefing();
        let a = fx.a();
        fx.ok(&a, &["thread", "skip"]);
        let context = fx.session_context(&a, "s-skip", "claude");
        assert!(context.contains("works without a thread"), "{}", context);
        assert!(!context.contains("No thread binding"), "{}", context);
    }

    /// Every stored continuation entry of a thread, oldest first, as the ledger holds them.
    fn stored_entries(fx: &Fixture, dir: &Path, id: &str) -> Vec<serde_json::Value> {
        let out = Command::new("dolt")
            .args([
                "sql",
                "-r",
                "json",
                "-q",
                &format!(
                    "select entry from continuation_entries where thread_id = '{}' order by seq",
                    id
                ),
            ])
            .current_dir(fx.ledger_path(dir))
            .env("HOME", fx.home())
            .env("DOLT_DISABLE_EVENT_FLUSH", "1")
            .output()
            .unwrap();
        assert!(out.status.success(), "{}", stderr(&out));
        json(&stdout(&out))["rows"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| json(row["entry"].as_str().unwrap()))
            .collect()
    }

    /// True when the ledger stores no mission for the thread: SQL NULL for the mission
    /// and the briefing path.
    fn stores_no_mission(fx: &Fixture, dir: &Path, id: &str) -> bool {
        let counted = fx.dolt_sql(
            dir,
            &format!(
                "select count(*) as c from threads where thread_id = '{}' and mission_id is null and briefing_path is null",
                id
            ),
        );
        counted.lines().nth(1) == Some("1")
    }

    const NO_MISSION_ERROR: &str = "has no mission, so it has no tasks. Attach a mission with tsk thread set mission <mission-id>, or pass an empty task ID";

    pub fn a_thread_can_start_with_no_mission_and_pause_with_an_empty_task() {
        let fx = Fixture::with_briefing();
        let a = fx.a();
        fx.ok(&a, &["mission", "create", "M-REC", "Recorded"]);
        fx.ok(&a, &["task", "add", "M-REC", "First task"]);
        fx.ok(&a, &["ledger", "push", "Add M-REC"]);

        // Start with no argument mints a thread with no mission, and clears the skip.
        fx.ok(&a, &["thread", "skip"]);
        assert_eq!(fx.guard_reason(&a, None), None);
        let before = fx.remote_ledger();
        let id = fx.started(&a, &["thread", "start"]);
        assert_ne!(fx.remote_ledger(), before);
        assert!(stores_no_mission(&fx, &a, &id));
        assert_eq!(fx.guard_reason(&a, None), None);
        assert_eq!(
            fx.ok(&a, &["thread", "binding", "--no-fetch"]),
            format!("worktree:{}\n", id)
        );
        let row = fx.thread_row(&a, &id);
        assert!(row["mission_id"].is_null(), "{}", row);
        assert_eq!(row["mission_link"], "");
        assert!(row["latest_whats_next"].is_null(), "{}", row);
        // Resume of a thread with no entries prints no entry and does not fail.
        let resumed = json(&fx.ok(&a, &["thread", "resume", &id]));
        assert_eq!(resumed["thread_id"], id.as_str());
        assert_eq!(resumed["latest"], serde_json::json!({}));

        // A second start with a binding in place asks for a resume.
        let again = fx.tsk(&a, &["thread", "start"]);
        assert_exit(&again, 2);
        assert_eq!(stdout(&again), format!("resume-required:{}\n", id));

        // The skip is cleared by the start: once detached, the Stop hook check asks again.
        fx.ok(&a, &["thread", "detach"]);
        assert!(fx.guard_reason(&a, None).is_some());
        fx.ok(&a, &["thread", "resume", &id]);

        // A task ID is refused, with the exact message, and nothing is written.
        let before = fx.remote_ledger();
        let refused = fx.tsk(&a, &["thread", "pause", &id, "T-01", "x"]);
        assert_exit(&refused, 1);
        assert_eq!(
            stderr(&refused).trim_end(),
            format!("error: thread {} {}", id, NO_MISSION_ERROR)
        );
        assert_eq!(stdout(&refused), "");
        assert_eq!(fx.remote_ledger(), before);
        assert_exit(&fx.tsk(&a, &["thread", "pause", &id, "later", "x"]), 1);

        // An empty task ID is accepted, and the entry records a null mission.
        assert_eq!(
            fx.ok(&a, &["thread", "pause", &id, "", "Review the draft"]),
            format!(
                "paused:{}\ncode: clean\ncode pushed: yes\nledger: pushed\n",
                id
            )
        );
        let latest = fx.latest_entry(&a, &id);
        assert!(latest["mission_id"].is_null(), "{}", latest);
        assert!(latest.get("mission_link").is_none(), "{}", latest);
        assert_eq!(latest["task_id"], "");
        assert_eq!(latest["whats_next"], "Review the draft");
        let stored = stored_entries(&fx, &a, &id);
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].get("mission_id"), Some(&serde_json::Value::Null));
        let row = fx.thread_row(&a, &id);
        assert!(row["mission_id"].is_null(), "{}", row);
        assert_eq!(row["latest_whats_next"], "Review the draft");

        // A cloud session starts a thread with no mission too.
        fx.ok(&a, &["thread", "detach"]);
        let cloud = fx.tsk_cloud(&a, "cse_none", &["thread", "start"]);
        assert_success(&cloud);
        let cloud_id = stdout(&cloud)
            .trim_end()
            .strip_prefix("started:")
            .unwrap()
            .to_string();
        assert!(fx.thread_row(&a, &cloud_id)["mission_id"].is_null());
        assert_eq!(
            stdout(&fx.tsk_cloud(&a, "cse_none", &["thread", "binding"])),
            format!("cloud:{}\n", cloud_id)
        );
    }

    pub fn thread_set_mission_attaches_changes_and_detaches_a_mission() {
        let fx = Fixture::with_briefing();
        let a = fx.a();
        fx.ok(&a, &["mission", "create", "M-ONE", "First mission"]);
        fx.ok(&a, &["mission", "create", "M-TWO", "Second mission"]);
        fx.ok(&a, &["task", "add", "M-ONE", "One task"]);
        fx.ok(&a, &["ledger", "push", "Add the missions"]);

        let id = fx.started(&a, &["thread", "start"]);
        fx.ok(&a, &["thread", "pause", &id, "", "before the mission"]);

        // An unknown mission is refused, by name, and nothing is written.
        let before = fx.remote_ledger();
        let refused = fx.tsk(&a, &["thread", "set", "mission", "M-NONE"]);
        assert_exit(&refused, 1);
        assert!(
            stderr(&refused).contains("no such mission: M-NONE"),
            "{}",
            stderr(&refused)
        );
        assert_eq!(stdout(&refused), "");
        assert_eq!(fx.remote_ledger(), before);
        assert!(stores_no_mission(&fx, &a, &id));

        // The bound thread is the default.
        assert_eq!(
            fx.ok(&a, &["thread", "set", "mission", "M-ONE"]),
            format!("mission-set:{}:M-ONE\n", id)
        );
        // A set is a local write (ADR 0019): origin is unchanged until the next push.
        assert_eq!(fx.remote_ledger(), before);
        let row = fx.thread_row(&a, &id);
        assert_eq!(row["mission_id"], "M-ONE");
        assert_eq!(row["mission_link"], "");

        // A pause now checks the task against the mission, and records the mission. The
        // entry written before keeps its null mission.
        fx.ok(&a, &["thread", "pause", &id, "T-1", "with the mission"]);
        let latest = fx.latest_entry(&a, &id);
        assert_eq!(latest["mission_id"], "M-ONE");
        assert_eq!(latest["task_id"], "T-01");
        assert_exit(&fx.tsk(&a, &["thread", "pause", &id, "T-09", "x"]), 1);
        let stored = stored_entries(&fx, &a, &id);
        assert_eq!(stored.len(), 2);
        assert!(stored[0]["mission_id"].is_null(), "{}", stored[0]);
        assert_eq!(stored[0]["whats_next"], "before the mission");
        assert_eq!(stored[1]["mission_id"], "M-ONE");

        // The mission changes with an explicit thread ID, and the earlier entry keeps M-ONE.
        fx.ok(&a, &["thread", "detach"]);
        assert_eq!(
            fx.ok(&a, &["thread", "set", "mission", "M-TWO", &id]),
            format!("mission-set:{}:M-TWO\n", id)
        );
        assert_eq!(fx.thread_row(&a, &id)["mission_id"], "M-TWO");
        fx.ok(&a, &["thread", "resume", &id]);
        fx.ok(&a, &["thread", "pause", &id, "", "on the second mission"]);
        let stored = stored_entries(&fx, &a, &id);
        assert_eq!(stored.len(), 3);
        assert_eq!(stored[1]["mission_id"], "M-ONE");
        assert_eq!(stored[2]["mission_id"], "M-TWO");
        // M-TWO has no tasks, so a task is refused by the mission record.
        assert_exit(&fx.tsk(&a, &["thread", "pause", &id, "T-01", "x"]), 1);

        // A mission known only by its briefing stores the briefing path.
        fx.ok(&a, &["thread", "set", "mission", "M-CONF-01"]);
        let row = fx.thread_row(&a, &id);
        assert_eq!(row["mission_id"], "M-CONF-01");
        assert_eq!(row["mission_link"], BRIEFING);

        // Another clone sees the change after the push.
        fx.ok(&a, &["ledger", "push", "Publish the mission"]);
        let b_clone = fx.b();
        assert_eq!(fx.thread_row(&b_clone, &id)["mission_id"], "M-CONF-01");

        // --none detaches the mission, and the briefing path goes with it.
        assert_eq!(
            fx.ok(&a, &["thread", "set", "mission", "--none"]),
            format!("mission-cleared:{}\n", id)
        );
        assert!(stores_no_mission(&fx, &a, &id));
        let row = fx.thread_row(&a, &id);
        assert!(row["mission_id"].is_null(), "{}", row);
        assert_eq!(row["mission_link"], "");
        let refused = fx.tsk(&a, &["thread", "pause", &id, "T-01", "x"]);
        assert_exit(&refused, 1);
        assert!(stderr(&refused).contains(NO_MISSION_ERROR));
        // The entries are untouched.
        let stored = stored_entries(&fx, &a, &id);
        assert_eq!(stored.len(), 3);
        assert_eq!(stored[1]["mission_id"], "M-ONE");

        // --none with an explicit thread ID, after a mission is set again.
        fx.ok(&a, &["thread", "set", "mission", "M-ONE"]);
        fx.ok(&a, &["thread", "detach"]);
        assert_eq!(
            fx.ok(&a, &["thread", "set", "mission", "--none", &id]),
            format!("mission-cleared:{}\n", id)
        );
        assert!(stores_no_mission(&fx, &a, &id));

        // Refusals: no binding and no thread ID, an unknown thread, a bad ID, and --none
        // with a mission ID.
        let unbound = fx.tsk(&a, &["thread", "set", "mission", "M-ONE"]);
        assert_exit(&unbound, 1);
        assert!(
            stderr(&unbound).contains("no thread binding found"),
            "{}",
            stderr(&unbound)
        );
        let unknown = fx.tsk(&a, &["thread", "set", "mission", "M-ONE", "zzzzzzzz"]);
        assert_exit(&unknown, 1);
        assert!(
            stderr(&unknown).contains("no such thread: zzzzzzzz"),
            "{}",
            stderr(&unknown)
        );
        assert_exit(
            &fx.tsk(&a, &["thread", "set", "mission", "M-ONE", "bad"]),
            1,
        );
        let both = fx.tsk(&a, &["thread", "set", "mission", "--none", "M-ONE", &id]);
        assert_exit(&both, 1);
        assert!(stderr(&both).contains("--none"), "{}", stderr(&both));
        assert!(stores_no_mission(&fx, &a, &id));
    }

    pub fn session_start_carries_the_latest_entry_so_the_agent_can_answer_at_once() {
        let fx = Fixture::fetched();
        let (a, b) = (fx.a(), fx.b());
        fx.ok(&a, &["mission", "create", "M-REC", "Recorded mission"]);
        fx.ok(&a, &["task", "add", "M-REC", "Open one"]);
        fx.ok(&a, &["task", "add", "M-REC", "Second one"]);
        fx.ok(&a, &["ledger", "push", "Add the mission"]);
        let id = fx.started(&a, &["thread", "start", "M-REC"]);

        // No entry yet: the existing text, with nothing about a latest entry.
        let none = fx.session_context(&a, "s-le-1", "claude");
        assert!(none.contains("This thread works mission M-REC"), "{}", none);
        assert!(!none.contains("latest pause entry"), "{}", none);

        // An entry by this actor: the task and its description, what is next, who and when.
        fx.ok(&a, &["thread", "pause", &id, "T-02", "Write the tests"]);
        let own = fx.session_context(&a, "s-le-2", "claude");
        assert!(
            own.contains("Its latest pause entry is on task T-02 (Second one), written by "),
            "{}",
            own
        );
        assert!(own.contains("What's next: Write the tests."), "{}", own);
        assert!(own.contains("M-REC (todo): Recorded mission."), "{}", own);
        assert!(
            own.contains("run no command until the person confirms"),
            "{}",
            own
        );
        assert!(!own.contains("WARNING"), "{}", own);

        // An entry by another actor adds a warning that names it.
        fx.ok(&b, &["thread", "resume", &id]);
        fx.ok(&b, &["thread", "pause", &id, "T-01", "Review the diff"]);
        let other = fx.session_context(&a, "s-le-3", "claude");
        assert!(
            other.contains("Its latest pause entry is on task T-01 (Open one), written by "),
            "{}",
            other
        );
        assert!(other.contains("What's next: Review the diff."), "{}", other);
        assert!(
            other.contains(&format!(
                "WARNING: thread {} is already associated with other actor(s): ",
                id
            )),
            "{}",
            other
        );

        // A thread with no mission keeps the shorter text.
        fx.ok(&a, &["thread", "set", "mission", "--none"]);
        let none = fx.session_context(&a, "s-le-4", "claude");
        assert!(none.contains("This thread has no mission"), "{}", none);
        assert!(!none.contains("latest pause entry"), "{}", none);
    }

    pub fn thread_resume_carries_the_mission_name_and_the_task_description() {
        let fx = Fixture::fetched();
        let a = fx.a();
        fx.ok(&a, &["mission", "create", "M-REC", "Recorded mission"]);
        fx.ok(&a, &["task", "add", "M-REC", "Open one"]);
        fx.ok(&a, &["ledger", "push", "Add the mission"]);
        let id = fx.started(&a, &["thread", "start", "M-REC"]);
        fx.ok(&a, &["thread", "pause", &id, "T-01", "Go on"]);

        let result = json(&fx.ok(&a, &["thread", "resume", &id]));
        assert_eq!(result["thread_id"], id.as_str());
        assert_eq!(result["latest"]["task_id"], "T-01");
        assert_eq!(result["warning"], "");
        assert_eq!(result["mission"]["id"], "M-REC");
        assert_eq!(result["mission"]["name"], "Recorded mission");
        assert_eq!(result["task"]["id"], "T-01");
        assert_eq!(result["task"]["description"], "Open one");
    }

    pub fn session_start_says_a_bound_thread_with_no_mission_has_no_tasks() {
        let fx = Fixture::fetched();
        let a = fx.a();
        fx.ok(&a, &["mission", "create", "M-REC", "Recorded mission"]);
        fx.ok(&a, &["task", "add", "M-REC", "Open one"]);
        fx.ok(&a, &["ledger", "push", "Add the mission"]);
        let id = fx.started(&a, &["thread", "start"]);

        let binding = format!(
            "An existing thread binding was found: worktree:{}. Run /tsk:resume-thread {} next.",
            id, id
        );
        let attach = "To attach a mission, run `tsk thread set mission <mission-id>`.";

        // No pause entry yet.
        for (session, format) in [("s-nm-1", "claude"), ("s-nm-2", "copilot")] {
            let context = fx.session_context(&a, session, format);
            let expected = format!(
                "{} This thread has no mission, so it has no tasks. Its latest what's next: none yet. {}",
                binding, attach
            );
            assert!(context.contains(&expected), "{}", context);
            assert!(
                !context.contains("This thread works mission"),
                "{}",
                context
            );
        }

        // With a pause entry, its what's next.
        fx.ok(&a, &["thread", "pause", &id, "", "Review the draft"]);
        for (session, format) in [("s-nm-3", "claude"), ("s-nm-4", "copilot")] {
            let context = fx.session_context(&a, session, format);
            let expected = format!(
                "{} This thread has no mission, so it has no tasks. Its latest what's next: Review the draft. {}",
                binding, attach
            );
            assert!(context.contains(&expected), "{}", context);
        }

        // With a mission, the context is the mission's, and says nothing of "no mission".
        fx.ok(&a, &["thread", "set", "mission", "M-REC"]);
        let context = fx.session_context(&a, "s-nm-5", "claude");
        assert!(
            context.contains("This thread works mission M-REC (todo): Recorded mission."),
            "{}",
            context
        );
        assert!(!context.contains("has no mission"), "{}", context);
    }

    // --- Status, discard and decision dates ---

    /// The change line `status` prints for a mission record written locally.
    fn mission_change() -> String {
        "data\tmissions\tadded 1\tmodified 0\tdeleted 0".to_string()
    }

    fn status_lines(fx: &Fixture, dir: &Path) -> Vec<String> {
        fx.ok(dir, &["ledger", "status", "--porcelain"])
            .lines()
            .map(str::to_string)
            .collect()
    }

    fn clean_status(remote: &str) -> Vec<String> {
        vec![
            "data\tclean".to_string(),
            "docs\tclean".to_string(),
            format!("data\tremote\t{}", remote),
            format!("docs\tremote\t{}", remote),
        ]
    }

    pub fn status_and_discard_report_and_drop_local_changes() {
        let fx = Fixture::with_briefing();
        let (a, other) = (fx.a(), fx.b());
        fx.ok(&other, &["ledger", "fetch"]);
        assert_eq!(status_lines(&fx, &a), clean_status("up to date"));
        let ledger_before = fx.remote_ledger();
        let docs_before = fx.remote_docs();

        fx.ok(&a, &["mission", "create", "M-LOCAL", "Local only"]);
        fx.ok(&a, &["task", "add", "M-LOCAL", "A task"]);
        fx.put(&a, "local-note.md", "local\n");
        fx.put(&a, "index.md", "# Changed\n");
        fx.ok(&a, &["ledger", "doc", "rm", BRIEFING]);

        let mut expected = vec!["data\tlocal changes".to_string()];
        expected.push(mission_change());
        expected.push("data\ttasks\tadded 1\tmodified 0\tdeleted 0".to_string());
        expected.push("docs\tlocal changes".to_string());
        // Sorted by path.
        expected.push("docs\tM\tindex.md".to_string());
        expected.push("docs\tA\tlocal-note.md".to_string());
        expected.push(format!("docs\tD\t{}", BRIEFING));
        expected.push("data\tremote\tup to date".to_string());
        expected.push("docs\tremote\tup to date".to_string());
        assert_eq!(status_lines(&fx, &a), expected);
        // Status wrote nothing: the changes are all still there.
        assert_eq!(status_lines(&fx, &a), expected);
        assert!(fx
            .ok(&a, &["mission", "list", "--porcelain"])
            .contains("M-LOCAL"));

        // The other clone never saw any of it.
        fx.ok(&other, &["ledger", "fetch"]);
        assert_eq!(fx.ok(&other, &["mission", "list", "--porcelain"]), "");
        assert_exit(&fx.get(&other, "local-note.md"), 1);

        // Without --yes: the same summary, no remote lines, exit 1, nothing changes.
        let dry = fx.tsk(&a, &["ledger", "discard", "--porcelain"]);
        assert_exit(&dry, 1);
        assert_eq!(
            stdout(&dry).lines().collect::<Vec<_>>(),
            expected[..expected.len() - 2].to_vec()
        );
        assert_eq!(
            stderr(&dry),
            "error: this would discard the local changes above. Rerun with --yes to discard them.\n"
        );
        assert_eq!(status_lines(&fx, &a), expected);
        assert!(fx
            .ok(&a, &["mission", "list", "--porcelain"])
            .contains("M-LOCAL"));
        assert_eq!(
            fx.ok(&a, &["ledger", "doc", "get", "local-note.md"]),
            "local\n"
        );

        assert_eq!(fx.ok(&a, &["ledger", "discard", "--yes"]), "discarded\n");
        assert_eq!(status_lines(&fx, &a), clean_status("up to date"));
        assert_eq!(fx.ok(&a, &["mission", "list", "--porcelain"]), "");
        assert_exit(&fx.get(&a, "local-note.md"), 1);
        assert_eq!(
            fx.ok(&a, &["ledger", "doc", "get", BRIEFING]),
            "# Mission: conformance\n"
        );
        assert_ne!(
            fx.ok(&a, &["ledger", "doc", "get", "index.md"]),
            "# Changed\n"
        );
        // Origin was not touched, and a second discard is harmless.
        assert_eq!(fx.remote_ledger(), ledger_before);
        assert_eq!(fx.remote_docs(), docs_before);
        assert_eq!(fx.ok(&a, &["ledger", "discard", "--yes"]), "discarded\n");
        assert_eq!(status_lines(&fx, &other), clean_status("up to date"));
        // A clean store has nothing to confirm.
        assert_eq!(
            fx.ok(&a, &["ledger", "discard", "--porcelain"]),
            "data\tclean\ndocs\tclean\n"
        );
        // The ledger still works after a discard.
        fx.ok(&a, &["mission", "create", "M-AFTER", "After"]);
        fx.ok(&a, &["ledger", "push", "After the discard"]);
        fx.ok(&other, &["ledger", "fetch"]);
        assert!(fx
            .ok(&other, &["mission", "list", "--porcelain"])
            .contains("M-AFTER"));
    }

    pub fn status_counts_commits_ahead_and_behind_origin() {
        let fx = Fixture::with_briefing();
        let (a, other) = (fx.a(), fx.b());
        fx.ok(&other, &["ledger", "fetch"]);

        // Behind: the other clone pushes a record and a document. What it pushed is not
        // reported as a local change here.
        fx.ok(
            &other,
            &["mission", "create", "M-OTHER", "From the other clone"],
        );
        fx.put(&other, "other.md", "other\n");
        fx.ok(&other, &["ledger", "push", "Other change"]);
        assert_eq!(status_lines(&fx, &a), clean_status("behind 1"));
        fx.ok(&a, &["ledger", "fetch"]);
        assert_eq!(status_lines(&fx, &a), clean_status("up to date"));

        // Ahead: a push that origin refuses leaves a commit that is not pushed.
        let hook = install_hook(
            &fx.origin(),
            "pre-receive",
            "#!/bin/sh\nwhile read old new ref; do\n  case \"$ref\" in\n    refs/heads/tsk/*) echo 'closed' >&2; exit 1;;\n  esac\ndone\n",
        );
        fx.ok(
            &a,
            &["mission", "create", "M-AHEAD", "Committed, not pushed"],
        );
        fx.put(&a, "ahead.md", "ahead\n");
        assert_exit(&fx.tsk(&a, &["ledger", "push", "Refused"]), 1);
        let lines = status_lines(&fx, &a);
        assert_eq!(lines[0], "data\tlocal changes");
        assert!(lines.contains(&mission_change()), "{:?}", lines);
        assert!(
            lines.contains(&"docs\tA\tahead.md".to_string()),
            "{:?}",
            lines
        );
        assert_eq!(
            lines[lines.len() - 2..],
            ["data\tremote\tahead 1", "docs\tremote\tahead 1"]
        );
        // The committed work is what the dry run lists too.
        let dry = fx.tsk(&a, &["ledger", "discard", "--porcelain"]);
        assert_exit(&dry, 1);
        assert!(stdout(&dry).contains("ahead.md"), "{}", stdout(&dry));

        // Diverged: the other clone pushes while this one holds its commit.
        std::fs::remove_file(&hook).unwrap();
        fx.ok(&other, &["mission", "create", "M-OTHER-2", "Second"]);
        fx.put(&other, "other-2.md", "other 2\n");
        fx.ok(&other, &["ledger", "push", "Second other change"]);
        let lines = status_lines(&fx, &a);
        assert_eq!(
            lines[lines.len() - 2..],
            [
                "data\tremote\tdiverged ahead 1 behind 1",
                "docs\tremote\tdiverged ahead 1 behind 1"
            ]
        );
        assert!(lines.contains(&mission_change()), "{:?}", lines);
        assert!(
            !lines.iter().any(|l| l.contains("M-OTHER-2")),
            "{:?}",
            lines
        );
        assert!(
            !lines.iter().any(|l| l.contains("other-2.md")),
            "{:?}",
            lines
        );

        // Discard drops the commit and lands on origin's state.
        assert_eq!(fx.ok(&a, &["ledger", "discard", "--yes"]), "discarded\n");
        assert_eq!(status_lines(&fx, &a), clean_status("up to date"));
        let missions = fx.ok(&a, &["mission", "list", "--porcelain"]);
        assert!(!missions.contains("M-AHEAD"), "{}", missions);
        assert!(missions.contains("M-OTHER-2"), "{}", missions);
        assert_exit(&fx.get(&a, "ahead.md"), 1);
        assert_eq!(
            fx.ok(&a, &["ledger", "doc", "get", "other-2.md"]),
            "other 2\n"
        );
    }

    pub fn status_works_offline_and_discard_then_changes_nothing() {
        let fx = Fixture::with_briefing();
        let a = fx.a();
        fx.ok(&a, &["mission", "create", "M-OFF", "Offline"]);
        fx.put(&a, "off.md", "off\n");
        let moved = fx.root.path().join("origin-away.git");
        std::fs::rename(fx.origin(), &moved).unwrap();

        let status = fx.tsk(&a, &["ledger", "status", "--porcelain"]);
        assert_success(&status);
        let note = stderr(&status);
        assert_eq!(note.lines().count(), 1, "{}", note);
        assert!(
            note.starts_with("note: could not fetch ")
                && note.trim_end().ends_with("reporting from what is local"),
            "{}",
            note
        );
        let lines: Vec<String> = stdout(&status).lines().map(str::to_string).collect();
        assert!(lines.contains(&mission_change()), "{:?}", lines);
        assert!(
            lines.contains(&"docs\tA\toff.md".to_string()),
            "{:?}",
            lines
        );
        assert_eq!(
            lines[lines.len() - 2..],
            ["data\tremote\tunknown", "docs\tremote\tunknown"]
        );

        // Discard cannot reset to a state it could not fetch, so it changes nothing.
        let refused = fx.tsk(&a, &["ledger", "discard", "--yes"]);
        assert_exit(&refused, 1);
        assert!(
            stderr(&refused).contains("Nothing was discarded"),
            "{}",
            stderr(&refused)
        );
        assert!(fx
            .ok(&a, &["mission", "list", "--porcelain"])
            .contains("M-OFF"));
        assert_eq!(fx.ok(&a, &["ledger", "doc", "get", "off.md"]), "off\n");

        std::fs::rename(&moved, fx.origin()).unwrap();
        assert_eq!(
            status_lines(&fx, &a).last().unwrap(),
            "docs\tremote\tup to date"
        );
    }

    fn human(fx: &Fixture, dir: &Path) -> Vec<String> {
        let out = no_server_line(fx.ok(dir, &["ledger", "status"]));
        assert!(!out.contains('\t'), "{:?}", out);
        for line in out.lines() {
            assert_eq!(line, line.trim_end(), "{:?}", line);
        }
        out.lines().map(str::to_string).collect()
    }

    /// The Local block for one mission record and one document `h.md`, written locally.
    fn local_block(extra_commits: Option<&str>) -> Vec<String> {
        let mut lines = vec!["Local   2 changes not pushed".to_string()];
        lines.push("          data  missions  1 added".to_string());
        lines.push("          docs  h.md      added".to_string());
        if let Some(plus) = extra_commits {
            lines.push(format!("        plus {} not pushed", plus));
        }
        lines
    }

    fn origin_line(text: &str) -> String {
        format!("Origin  {}", text)
    }

    pub fn status_prints_local_and_origin_in_plain_columns() {
        // A ledger that origin does not hold yet.
        let fresh = Fixture::new();
        fresh.ok(&fresh.a(), &["ledger", "fetch"]);
        assert_eq!(
            human(&fresh, &fresh.a()),
            vec![
                "Local   nothing to push",
                "Origin  not published yet (tsk ledger push creates it)"
            ]
        );

        let fx = Fixture::with_briefing();
        let (a, other) = (fx.a(), fx.b());
        fx.ok(&other, &["ledger", "fetch"]);
        assert_eq!(
            human(&fx, &a),
            vec!["Local   nothing to push", "Origin  up to date"]
        );

        // Changes not pushed; the discard preview prints the Local block alone.
        fx.ok(&a, &["mission", "create", "M-H", "Human"]);
        fx.put(&a, "h.md", "h\n");
        let mut expected = local_block(None);
        expected.push("Origin  up to date".to_string());
        assert_eq!(human(&fx, &a), expected);
        let dry = fx.tsk(&a, &["ledger", "discard"]);
        assert_exit(&dry, 1);
        assert_eq!(stdout(&dry).lines().collect::<Vec<_>>(), local_block(None));
        assert!(!stdout(&dry).contains('\t'));
        assert_eq!(
            stderr(&dry),
            "error: this would discard the local changes above. Rerun with --yes to discard them.\n"
        );
        // The porcelain form is the tab-separated one.
        assert!(fx
            .ok(&a, &["ledger", "status", "--porcelain"])
            .starts_with("data\tlocal changes\n"));
        assert_eq!(fx.ok(&a, &["ledger", "discard", "--yes"]), "discarded\n");
        assert_eq!(
            fx.ok(&a, &["ledger", "discard"]),
            "Local   nothing to push\n"
        );

        // Behind: the other clone pushes a record and a document.
        fx.ok(&other, &["mission", "create", "M-O", "Other"]);
        fx.put(&other, "o.md", "o\n");
        fx.ok(&other, &["ledger", "push", "Other change"]);
        assert_eq!(
            human(&fx, &a),
            vec![
                "Local   nothing to push".to_string(),
                origin_line("1 data commit and 1 document commit to fetch (tsk ledger fetch)")
            ]
        );
        fx.ok(&a, &["ledger", "fetch"]);

        // Ahead: a refused push leaves commits that are not on origin.
        let hook = install_hook(
            &fx.origin(),
            "pre-receive",
            "#!/bin/sh\nwhile read old new ref; do\n  case \"$ref\" in\n    refs/heads/tsk/*) echo 'closed' >&2; exit 1;;\n  esac\ndone\n",
        );
        fx.ok(&a, &["mission", "create", "M-H", "Human"]);
        fx.put(&a, "h.md", "h\n");
        assert_exit(&fx.tsk(&a, &["ledger", "push", "Refused"]), 1);
        let mut expected = local_block(Some("2 commits"));
        expected.push(origin_line(
            "1 data commit and 1 document commit not on origin (tsk ledger push)",
        ));
        assert_eq!(human(&fx, &a), expected);

        // Diverged: the other clone pushes while this one holds its commits.
        std::fs::remove_file(&hook).unwrap();
        fx.ok(&other, &["mission", "create", "M-O2", "Other 2"]);
        fx.put(&other, "o2.md", "o2\n");
        fx.ok(&other, &["ledger", "push", "Second other change"]);
        let lines = human(&fx, &a);
        assert_eq!(
            lines.last().unwrap(),
            &origin_line(
                "diverged: 1 data commit and 1 document commit to push, 1 data commit and 1 document commit to fetch (tsk ledger push merges them)"
            )
        );
        fx.ok(&a, &["ledger", "discard", "--yes"]);

        // Offline: origin cannot be reached.
        let moved = fx.root.path().join("origin-away.git");
        std::fs::rename(fx.origin(), &moved).unwrap();
        let out = fx.tsk(&a, &["ledger", "status"]);
        assert_success(&out);
        assert_eq!(
            no_server_line(stdout(&out)),
            "Local   nothing to push\nOrigin  unknown (could not reach origin)\n"
        );
        assert_eq!(stderr(&out).lines().count(), 1, "{}", stderr(&out));
        assert!(stderr(&out).starts_with("note: could not fetch "));
        std::fs::rename(&moved, fx.origin()).unwrap();
    }

    pub fn status_and_discard_need_a_ledger_store() {
        let fx = Fixture::new();
        let a = fx.a();
        for args in [
            &["ledger", "status"][..],
            &["ledger", "discard", "--yes"][..],
        ] {
            let out = fx.tsk(&a, args);
            assert_exit(&out, 1);
            assert!(stderr(&out).starts_with("error: "), "{}", stderr(&out));
            assert!(
                stderr(&out).contains("tsk ledger fetch"),
                "{}",
                stderr(&out)
            );
            assert_eq!(stdout(&out), "");
        }
    }

    pub fn a_decision_takes_a_date_with_on_and_refuses_one_that_is_not_real() {
        let (fx, a) = mission_with_tasks(1);
        fx.ok(
            &a,
            &[
                "decision",
                "add",
                "M-T",
                "Old call",
                "--by",
                "Jim",
                "--on",
                "2026-02-27",
            ],
        );
        fx.ok(
            &a,
            &[
                "decision",
                "add",
                "M-T",
                "Leap day",
                "--by",
                "Jim",
                "--task",
                "T-01",
                "--on",
                "2024-02-29",
            ],
        );
        fx.ok(&a, &["decision", "add", "M-T", "Today", "--by", "Jim"]);
        let rows = lines(&fx.ok(&a, &["decision", "list", "M-T", "--porcelain"]));
        assert_eq!(rows[0], ["2026-02-27", "Jim", "-", "Old call", "-"]);
        assert_eq!(rows[1], ["2024-02-29", "Jim", "T-01", "Leap day", "-"]);
        assert!(is_date(&rows[2][0]), "{:?}", rows[2]);
        assert_ne!(rows[2][0], "2026-02-27");

        let before = fx.ok(&a, &["decision", "list", "M-T", "--porcelain"]);
        for bad in [
            "2026-02-30",
            "2026-02-29",
            "2026-13-01",
            "2026-00-10",
            "27-02-2026",
            "2026-2-7",
            "yesterday",
            "",
        ] {
            fails(
                &fx,
                &a,
                &["decision", "add", "M-T", "x", "--by", "Jim", "--on", bad],
                "YYYY-MM-DD",
            );
        }
        assert_eq!(
            fx.ok(&a, &["decision", "list", "M-T", "--porcelain"]),
            before
        );
        // The date survives a push and a fetch on the other clone.
        fx.ok(&a, &["ledger", "push", "Decisions"]);
        let other = fx.b();
        fx.ok(&other, &["ledger", "fetch"]);
        assert_eq!(
            fx.ok(&other, &["decision", "list", "M-T", "--porcelain"]),
            before
        );
        let record = mission_json(&fx, &other, "M-T");
        assert_eq!(record["decisions"][0]["decided_on"], "2026-02-27");
    }

    pub fn a_question_takes_dates_with_on_and_refuses_one_that_is_not_real() {
        let (fx, a) = mission_with_tasks(0);
        assert_eq!(
            fx.ok(
                &a,
                &["question", "add", "M-T", "Old one?", "--on", "2026-02-27"]
            ),
            "Q-01\n"
        );
        assert_eq!(
            fx.ok(
                &a,
                &["question", "add", "M-T", "Leap day?", "--on", "2024-02-29"]
            ),
            "Q-02\n"
        );
        assert_eq!(fx.ok(&a, &["question", "add", "M-T", "Today?"]), "Q-03\n");
        let rows = lines(&fx.ok(&a, &["question", "list", "--porcelain"]));
        assert_eq!(rows[0], ["M-T", "Q-01", "2026-02-27", "Old one?"]);
        assert_eq!(rows[1], ["M-T", "Q-02", "2024-02-29", "Leap day?"]);
        assert!(is_date(&rows[2][2]), "{:?}", rows[2]);
        assert_ne!(rows[2][2], "2026-02-27");

        fx.ok(
            &a,
            &[
                "question",
                "answer",
                "M-T",
                "Q-01",
                "Done.",
                "--by",
                "Jim",
                "--on",
                "2026-03-01",
            ],
        );
        fx.ok(
            &a,
            &["question", "answer", "M-T", "Q-02", "Done.", "--by", "Jim"],
        );
        let record = mission_json(&fx, &a, "M-T");
        assert_eq!(record["questions"][0]["asked_on"], "2026-02-27");
        assert_eq!(record["questions"][0]["answered_on"], "2026-03-01");
        assert_eq!(record["questions"][1]["asked_on"], "2024-02-29");
        assert!(is_date(
            record["questions"][1]["answered_on"].as_str().unwrap()
        ));

        let before = mission_json(&fx, &a, "M-T");
        for bad in [
            "2026-02-30",
            "2026-02-29",
            "2026-13-01",
            "27-02-2026",
            "yesterday",
            "",
        ] {
            fails(
                &fx,
                &a,
                &["question", "add", "M-T", "x?", "--on", bad],
                "YYYY-MM-DD",
            );
            fails(
                &fx,
                &a,
                &[
                    "question", "answer", "M-T", "Q-03", "x", "--by", "Jim", "--on", bad,
                ],
                "YYYY-MM-DD",
            );
        }
        assert_eq!(mission_json(&fx, &a, "M-T"), before);

        // The dates survive a push and a fetch on the other clone.
        fx.ok(&a, &["ledger", "push", "Question dates"]);
        let other = fx.b();
        fx.ok(&other, &["ledger", "fetch"]);
        let record = mission_json(&fx, &other, "M-T");
        assert_eq!(record["questions"][0]["asked_on"], "2026-02-27");
        assert_eq!(record["questions"][0]["answered_on"], "2026-03-01");
    }

    pub fn an_open_question_is_edited_and_an_answered_one_is_not() {
        let (fx, a) = mission_with_tasks(0);
        fx.ok(&a, &["mission", "create", "M-U", "Other"]);
        fx.ok(&a, &["question", "add", "M-T", "Why one?"]);
        fx.ok(&a, &["question", "add", "M-T", "Why two?"]);
        fx.ok(&a, &["question", "add", "M-U", "Why in U?"]);
        let before = lines(&fx.ok(&a, &["question", "list", "--porcelain"]));
        assert_eq!(
            fx.ok(&a, &["question", "edit", "M-T", "Q-01", "Why, then?"]),
            ""
        );
        // Only the text changes: the list keeps its columns, order and dates.
        let after = lines(&fx.ok(&a, &["question", "list", "--porcelain"]));
        assert_eq!(after.len(), 3);
        assert_eq!(after[0][..3], before[0][..3]);
        assert_eq!(after[0][3], "Why, then?");
        assert_eq!(after[1], before[1]);
        assert_eq!(after[2], before[2]);
        // A leading dash is text, as in question add.
        fx.ok(&a, &["question", "edit", "M-T", "Q-02", "-dash?"]);
        assert_eq!(
            mission_json(&fx, &a, "M-T")["questions"][1]["text"],
            "-dash?"
        );
        assert_eq!(
            mission_json(&fx, &a, "M-T")["questions"][1]["asked_on"],
            after[1][2]
        );

        // An answered question is final.
        fx.ok(
            &a,
            &[
                "question", "answer", "M-T", "Q-02", "Because.", "--by", "Jim",
            ],
        );
        fails(
            &fx,
            &a,
            &["question", "edit", "M-T", "Q-02", "Changed"],
            "Q-02 of M-T is answered; it cannot be edited",
        );
        let json = mission_json(&fx, &a, "M-T");
        assert_eq!(json["questions"][1]["text"], "-dash?");
        assert_eq!(json["questions"][1]["answer"], "Because.");

        fails(
            &fx,
            &a,
            &["question", "edit", "M-T", "Q-09", "x"],
            "M-T has no question Q-09",
        );
        fails(
            &fx,
            &a,
            &["question", "edit", "M-U", "Q-02", "x"],
            "M-U has no question Q-02",
        );
        fails(
            &fx,
            &a,
            &["question", "edit", "M-NONE", "Q-01", "x"],
            "M-NONE",
        );
        fails(
            &fx,
            &a,
            &["question", "edit", "M-T", "T-01", "x"],
            "not a question ID",
        );
        fails(
            &fx,
            &a,
            &["question", "edit", "M-T", "Q-01", "two\nlines"],
            "one line",
        );
        fails(
            &fx,
            &a,
            &["question", "edit", "M-T", "Q-01", "  "],
            "cannot be empty",
        );
        assert_eq!(
            mission_json(&fx, &a, "M-T")["questions"][0]["text"],
            "Why, then?"
        );

        // The edit travels with the push.
        fx.ok(&a, &["ledger", "push", "Edit a question"]);
        let other = fx.b();
        fx.ok(&other, &["ledger", "fetch"]);
        assert_eq!(
            mission_json(&fx, &other, "M-T")["questions"][0]["text"],
            "Why, then?"
        );
    }

    pub fn decisions_name_an_adr_and_are_found_from_the_adr() {
        let (fx, a) = mission_with_tasks(1);
        fx.ok(&a, &["mission", "create", "M-U", "Other"]);
        let adr = "docs/adr/0014-dolt-is-the-ledger-store-behind-a-storage-seam.md";
        let other_adr = "docs/adr/0015-other.md";
        std::fs::create_dir_all(a.join("docs/adr")).unwrap();
        write(&a.join(adr), "# ADR 14\n");
        write(&a.join(other_adr), "# ADR 15\n");

        fx.ok(
            &a,
            &[
                "decision", "add", "M-T", "Use Dolt", "--by", "Jim", "--adr", adr,
            ],
        );
        fx.ok(&a, &["decision", "add", "M-T", "No ADR", "--by", "Jim"]);
        // The stored path is one spelling, and a task and a date go with an ADR.
        fx.ok(
            &a,
            &[
                "decision",
                "add",
                "M-T",
                "Spelled long",
                "--by",
                "Jim",
                "--task",
                "T-01",
                "--on",
                "2026-03-14",
                "--adr",
                &format!("./{}", adr),
            ],
        );
        fx.ok(
            &a,
            &[
                "decision", "add", "M-U", "Same ADR", "--by", "Ann", "--adr", adr,
            ],
        );
        fx.ok(
            &a,
            &[
                "decision",
                "add",
                "M-U",
                "Other ADR",
                "--by",
                "Ann",
                "--adr",
                other_adr,
            ],
        );

        // By mission: the four columns and the ADR path last.
        let rows = lines(&fx.ok(&a, &["decision", "list", "M-T", "--porcelain"]));
        assert_eq!(rows.len(), 3);
        for row in &rows {
            assert_eq!(row.len(), 5, "{:?}", row);
        }
        assert_eq!(rows[0][3..], ["Use Dolt", adr]);
        assert_eq!(rows[1][3..], ["No ADR", "-"]);
        assert_eq!(rows[2][2..], ["T-01", "Spelled long", adr]);
        assert_eq!(rows[2][0], "2026-03-14");

        // By ADR across missions: the mission leads, so there are six columns.
        for spelling in [adr.to_string(), format!("./{}", adr)] {
            let found = lines(&fx.ok(&a, &["decision", "list", "--adr", &spelling, "--porcelain"]));
            assert_eq!(found.len(), 3, "{:?}", found);
            for row in &found {
                assert_eq!(row.len(), 6, "{:?}", row);
                assert_eq!(row[5], adr);
            }
            let keys: Vec<(&str, &str)> = found
                .iter()
                .map(|r| (r[0].as_str(), r[4].as_str()))
                .collect();
            assert_eq!(
                keys,
                vec![
                    ("M-T", "Use Dolt"),
                    ("M-T", "Spelled long"),
                    ("M-U", "Same ADR")
                ]
            );
            assert_eq!(found[2][1..4], [rows[0][0].as_str(), "Ann", "-"][..]);
        }
        // By mission and ADR: one mission, the five-column form.
        let one = lines(&fx.ok(
            &a,
            &["decision", "list", "M-U", "--adr", other_adr, "--porcelain"],
        ));
        assert_eq!(one.len(), 1);
        assert_eq!(one[0].len(), 5);
        assert_eq!(one[0][3..], ["Other ADR", other_adr]);
        let two = lines(&fx.ok(
            &a,
            &["decision", "list", "M-T", "--adr", adr, "--porcelain"],
        ));
        assert_eq!(two.len(), 2);
        assert_eq!(
            fx.ok(
                &a,
                &["decision", "list", "M-T", "--adr", other_adr, "--porcelain"]
            ),
            ""
        );
        // An ADR that no decision names is not an error, even when the file is gone.
        assert_eq!(
            fx.ok(
                &a,
                &[
                    "decision",
                    "list",
                    "--adr",
                    "docs/adr/9999-none.md",
                    "--porcelain"
                ]
            ),
            ""
        );
        // With no mission and no --adr, every decision of every mission is listed, led by
        // the mission, in the six columns of the ADR form.
        let every = lines(&fx.ok(&a, &["decision", "list", "--porcelain"]));
        assert!(every.len() >= 3, "{:?}", every);
        for row in &every {
            assert_eq!(row.len(), 6, "{:?}", row);
        }
        let aligned = fx.ok(&a, &["decision", "list"]);
        assert_eq!(aligned.lines().count(), every.len());
        assert!(!aligned.contains('\t'), "{:?}", aligned);
        fails(
            &fx,
            &a,
            &["decision", "list", "M-NONE", "--adr", adr, "--porcelain"],
            "M-NONE",
        );
        fails(
            &fx,
            &a,
            &["decision", "list", "--adr", "/etc/passwd", "--porcelain"],
            "absolute",
        );
        fails(
            &fx,
            &a,
            &["decision", "list", "--adr", "../x.md", "--porcelain"],
            "'..'",
        );

        // The record carries the path, and null when there is none.
        let json = mission_json(&fx, &a, "M-T");
        assert_eq!(json["decisions"][0]["adr"], adr);
        assert_eq!(json["decisions"][1]["adr"], serde_json::Value::Null);
        assert_eq!(json["decisions"][2]["adr"], adr);

        // A path is checked against the repository the command runs in, from any
        // directory of its working tree.
        let nested = a.join("docs");
        let out = fx.tsk(
            &nested,
            &[
                "decision",
                "add",
                "M-U",
                "From a subdirectory",
                "--by",
                "Ann",
                "--adr",
                other_adr,
            ],
        );
        assert_success(&out);
        assert_eq!(
            fx.ok(&a, &["decision", "list", "--adr", other_adr, "--porcelain"])
                .lines()
                .count(),
            2
        );

        // Refused paths write nothing.
        let before = fx.ok(&a, &["decision", "list", "--adr", adr, "--porcelain"]);
        let count = fx.ok(&a, &["decision", "list", "M-T", "--porcelain"]);
        fails(
            &fx,
            &a,
            &[
                "decision",
                "add",
                "M-T",
                "x",
                "--by",
                "Jim",
                "--adr",
                "/etc/passwd",
            ],
            "is absolute",
        );
        for bad in [
            "../outside.md",
            "docs/../../outside.md",
            "docs/adr/../../../x.md",
        ] {
            fails(
                &fx,
                &a,
                &["decision", "add", "M-T", "x", "--by", "Jim", "--adr", bad],
                "contains '..'",
            );
        }
        for missing in ["docs/adr/0099-missing.md", "docs/adr", "docs"] {
            fails(
                &fx,
                &a,
                &[
                    "decision", "add", "M-T", "x", "--by", "Jim", "--adr", missing,
                ],
                "no such file in the repository",
            );
        }
        fails(
            &fx,
            &a,
            &["decision", "add", "M-T", "x", "--by", "Jim", "--adr", ""],
            "cannot be empty",
        );
        fails(
            &fx,
            &a,
            &[
                "decision",
                "add",
                "M-T",
                "x",
                "--by",
                "Jim",
                "--adr",
                "docs/a\tb.md",
            ],
            "tab or a line break",
        );
        assert_eq!(
            fx.ok(&a, &["decision", "list", "M-T", "--porcelain"]),
            count
        );
        assert_eq!(
            fx.ok(&a, &["decision", "list", "--adr", adr, "--porcelain"]),
            before
        );

        // The links travel with the push, and the other clone finds them without the file.
        fx.ok(&a, &["ledger", "push", "Decisions with ADRs"]);
        let other = fx.b();
        fx.ok(&other, &["ledger", "fetch"]);
        assert_eq!(
            fx.ok(&other, &["decision", "list", "--adr", adr, "--porcelain"]),
            before
        );
    }

    pub fn constraints_and_out_of_scope_are_numbered_lists_whose_numbers_are_not_reused() {
        let fx = Fixture::fetched();
        let (a, other) = (fx.a(), fx.b());
        fx.ok(&a, &["mission", "create", "M-L", "Lists"]);
        fx.ok(&a, &["mission", "create", "M-U", "Untouched"]);
        for (command, key, noun) in [
            ("constraint", "constraints", "constraint"),
            ("out-of-scope", "out_of_scope", "out-of-scope item"),
        ] {
            let list = |dir: &Path, id: &str| fx.ok(dir, &["mission", command, "list", id]);
            assert_eq!(list(&a, "M-L"), "", "{}", command);
            assert_eq!(mission_json(&fx, &a, "M-L")[key], serde_json::json!([]));
            for (text, number) in [("one", "1"), ("two", "2"), ("three", "3")] {
                assert_eq!(
                    fx.ok(&a, &["mission", command, "add", "M-L", text]),
                    format!("{}\n", number),
                    "{}",
                    command
                );
            }
            assert_eq!(list(&a, "M-L"), "1\tone\n2\ttwo\n3\tthree\n");
            assert_eq!(
                mission_json(&fx, &a, "M-L")[key],
                serde_json::json!([
                    {"n": 1, "text": "one"},
                    {"n": 2, "text": "two"},
                    {"n": 3, "text": "three"}
                ])
            );

            // A removed number stays gone, the highest one included.
            assert_eq!(fx.ok(&a, &["mission", command, "remove", "M-L", "2"]), "");
            assert_eq!(list(&a, "M-L"), "1\tone\n3\tthree\n");
            assert_eq!(
                fx.ok(&a, &["mission", command, "add", "M-L", "four"]),
                "4\n"
            );
            fx.ok(&a, &["mission", command, "remove", "M-L", "4"]);
            assert_eq!(
                fx.ok(&a, &["mission", command, "add", "M-L", "-dash five"]),
                "5\n"
            );
            assert_eq!(list(&a, "M-L"), "1\tone\n3\tthree\n5\t-dash five\n");

            fails(
                &fx,
                &a,
                &["mission", command, "remove", "M-L", "2"],
                &format!("M-L has no {} 2", noun),
            );
            fails(
                &fx,
                &a,
                &["mission", command, "remove", "M-L", "9"],
                &format!("M-L has no {} 9", noun),
            );
            fails(
                &fx,
                &a,
                &["mission", command, "remove", "M-U", "1"],
                "M-U has no",
            );
            for args in [
                vec!["mission", command, "add", "M-NONE", "x"],
                vec!["mission", command, "remove", "M-NONE", "1"],
                vec!["mission", command, "list", "M-NONE"],
            ] {
                fails(&fx, &a, &args, "no such mission: M-NONE");
            }
            fails(
                &fx,
                &a,
                &["mission", command, "add", "M-L", ""],
                "cannot be empty",
            );
            fails(
                &fx,
                &a,
                &["mission", command, "add", "M-L", "  "],
                "cannot be empty",
            );
            fails(
                &fx,
                &a,
                &["mission", command, "add", "M-L", "two\nlines"],
                "is one line",
            );
            assert_eq!(list(&a, "M-L"), "1\tone\n3\tthree\n5\t-dash five\n");
            // Another mission has lists of its own.
            assert_eq!(list(&a, "M-U"), "");
            assert_eq!(
                fx.ok(&a, &["mission", command, "add", "M-U", "mine"]),
                "1\n"
            );
        }

        // The two lists count apart, and the lists travel with the push.
        let record = mission_json(&fx, &a, "M-L");
        assert_eq!(record["constraints"], record["out_of_scope"]);
        fx.ok(&a, &["ledger", "push", "Lists"]);
        fx.ok(&other, &["ledger", "fetch"]);
        assert_eq!(mission_json(&fx, &other, "M-L"), record);
        assert_eq!(
            fx.ok(&other, &["mission", "constraint", "add", "M-L", "from b"]),
            "6\n"
        );
        assert_eq!(
            fx.ok(&other, &["mission", "out-of-scope", "add", "M-L", "from b"]),
            "6\n"
        );
    }

    pub fn lists_print_aligned_columns_and_porcelain_keeps_the_tabs() {
        let fx = Fixture::fetched();
        let a = fx.a();
        fx.ok(&a, &["mission", "create", "M-A1", "First one"]);
        fx.ok(
            &a,
            &[
                "mission",
                "create",
                "M-LONGER-ID",
                "Second",
                "--parent",
                "M-A1",
            ],
        );
        fx.ok(&a, &["task", "add", "M-A1", "Write it"]);
        fx.ok(&a, &["task", "add", "M-A1", "Test it"]);
        fx.ok(
            &a,
            &["task", "set", "M-A1", "T-02", "status", "in-progress"],
        );
        fx.ok(&a, &["task", "block", "M-A1", "T-02", "T-01"]);
        fx.ok(
            &a,
            &[
                "decision",
                "add",
                "M-A1",
                "Use Dolt",
                "--by",
                "Jim",
                "--on",
                "2026-02-27",
            ],
        );
        fx.ok(
            &a,
            &[
                "decision",
                "add",
                "M-A1",
                "Split",
                "--by",
                "an agent",
                "--task",
                "T-02",
                "--on",
                "2026-03-01",
            ],
        );
        fx.ok(&a, &["question", "add", "M-A1", "Why?"]);
        fx.ok(&a, &["question", "add", "M-LONGER-ID", "How?"]);

        let aligned = |args: &[&str]| -> Vec<String> {
            let out = fx.ok(&a, args);
            assert!(!out.contains('\t'), "{:?}", out);
            for line in out.lines() {
                assert_eq!(line, line.trim_end(), "{:?}", line);
            }
            out.lines().map(str::to_string).collect()
        };
        // The same command with --porcelain prints the tab-separated form.
        let squeeze = |line: &str| line.split_whitespace().collect::<Vec<_>>().join("\t");
        let same_cells = |args: &[&str]| {
            let mut porcelain = args.to_vec();
            porcelain.push("--porcelain");
            let tabs = fx.ok(&a, &porcelain);
            assert!(tabs.contains('\t') || tabs.is_empty(), "{:?}", tabs);
            let cells: Vec<String> = aligned(args).iter().map(|l| squeeze(l)).collect();
            let expected: Vec<String> = tabs.lines().map(squeeze).collect();
            assert_eq!(cells, expected);
        };

        assert_eq!(
            aligned(&["mission", "list"]),
            vec![
                "M-A1         todo  -  -     First one",
                "M-LONGER-ID  todo  -  M-A1  Second",
            ]
        );
        assert_eq!(
            aligned(&["task", "list", "M-A1"]),
            vec![
                "T-01  todo         -     Write it",
                "T-02  in-progress  T-01  Test it",
            ]
        );
        assert_eq!(
            aligned(&["question", "list"])
                .iter()
                .map(|l| l.split("  ").next().unwrap().to_string())
                .collect::<Vec<_>>(),
            vec!["M-A1", "M-LONGER-ID"]
        );
        let decisions = aligned(&["decision", "list", "M-A1"]);
        assert_eq!(decisions.len(), 2);
        assert!(
            decisions[0].starts_with("2026-02-27  Jim "),
            "{:?}",
            decisions
        );
        same_cells(&["mission", "list"]);
        same_cells(&["task", "list", "M-A1"]);
        same_cells(&["question", "list"]);
        same_cells(&["decision", "list", "M-A1"]);
        assert_eq!(
            fx.ok(&a, &["task", "list", "M-A1", "--porcelain"]),
            "T-01\ttodo\t-\tWrite it\nT-02\tin-progress\tT-01\tTest it\n"
        );
        // The JSON form is not aligned.
        assert!(fx.ok(&a, &["mission", "list", "--json"]).starts_with("[{"));
    }

    pub fn mission_set_refuses_the_list_fields_and_keeps_the_text_fields() {
        let fx = Fixture::fetched();
        let a = fx.a();
        fx.ok(&a, &["mission", "create", "M-S", "Set"]);
        fx.ok(&a, &["mission", "constraint", "add", "M-S", "keep"]);
        let before = mission_json(&fx, &a, "M-S");
        for (field, message) in [
            (
                "constraints",
                "constraints are a list now; use tsk mission constraint add|remove|list",
            ),
            (
                "out-of-scope",
                "out-of-scope is a list now; use tsk mission out-of-scope add|remove|list",
            ),
        ] {
            let out = fx.tsk(&a, &["mission", "set", "M-S", field, "text"]);
            assert_exit(&out, 1);
            assert_eq!(stderr(&out), format!("error: {}\n", message));
            assert_eq!(stdout(&out), "");
            // The value on stdin is refused the same way.
            let out = fx.tsk_stdin(&a, &["mission", "set", "M-S", field, "-"], "a\nb\n");
            assert_exit(&out, 1);
            assert_eq!(stderr(&out), format!("error: {}\n", message));
        }
        assert_eq!(mission_json(&fx, &a, "M-S"), before);
        for field in ["name", "status", "purpose", "decision-authority"] {
            let value = if field == "status" {
                "in-progress"
            } else {
                "v"
            };
            fx.ok(&a, &["mission", "set", "M-S", field, value]);
        }
        let after = mission_json(&fx, &a, "M-S");
        assert_eq!(after["constraints"], before["constraints"]);
        assert_eq!(after["purpose"], "v");
    }

    // --- Numbers and collisions ---

    /// Two missions, M-X with one record of every kind and M-Y with none, pushed from clone
    /// A and fetched into clone B. It is built once per test process, and each test gets a
    /// copy.
    fn numbered_fixture() -> Fixture {
        Fixture::cached("numbered", || (build_numbered_fixture(), String::new())).0
    }

    fn build_numbered_fixture() -> Fixture {
        let fx = Fixture::fetched();
        let (a, other) = (fx.a(), fx.b());
        fx.ok(&a, &["mission", "create", "M-X", "X"]);
        fx.ok(&a, &["mission", "create", "M-Y", "Y"]);
        fx.ok(&a, &["task", "add", "M-X", "base task"]);
        fx.ok(&a, &["question", "add", "M-X", "base question?"]);
        fx.ok(&a, &["mission", "objective", "add", "M-X", "base outcome"]);
        fx.ok(
            &a,
            &["mission", "constraint", "add", "M-X", "base constraint"],
        );
        fx.ok(&a, &["mission", "out-of-scope", "add", "M-X", "base scope"]);
        fx.ok(
            &a,
            &["decision", "add", "M-X", "base decision", "--by", "Jim"],
        );
        fx.ok(&a, &["ledger", "push", "Base"]);
        fx.ok(&other, &["ledger", "fetch"]);
        fx
    }

    /// The lines a push printed before the commit it prints last.
    fn renumber_lines(out: &Output) -> Vec<String> {
        let text = stdout(out);
        let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
        assert!(lines.pop().is_some_and(|l| l.len() >= 20), "{}", text);
        lines
    }

    /// `(number, text)` of each item of a kind of a mission, oldest first. A decision has
    /// no number to read, so its number is its place in the list.
    fn items(fx: &Fixture, dir: &Path, mission: &str, kind: &str) -> Vec<(i64, String)> {
        let record = mission_json(fx, dir, mission);
        let list = |key: &str, number: &str, field: &str| -> Vec<(i64, String)> {
            record[key]
                .as_array()
                .unwrap()
                .iter()
                .enumerate()
                .map(|(place, item)| {
                    let digits: String = item[number]
                        .to_string()
                        .chars()
                        .filter(|c| c.is_ascii_digit())
                        .collect();
                    (
                        digits.parse().unwrap_or(place as i64 + 1),
                        item[field].as_str().unwrap().to_string(),
                    )
                })
                .collect()
        };
        match kind {
            "task" => list("tasks", "id", "description"),
            "question" => list("questions", "id", "text"),
            "objective" => list("objective", "n", "outcome"),
            "constraint" => list("constraints", "n", "text"),
            "out-of-scope" => list("out_of_scope", "n", "text"),
            "decision" => list("decisions", "none", "text"),
            other => panic!("{}", other),
        }
    }

    fn add_item(fx: &Fixture, dir: &Path, mission: &str, kind: &str, text: &str) -> String {
        let args: Vec<&str> = match kind {
            "task" => vec!["task", "add", mission, text],
            "question" => vec!["question", "add", mission, text],
            "objective" => vec!["mission", "objective", "add", mission, text],
            "constraint" => vec!["mission", "constraint", "add", mission, text],
            "out-of-scope" => vec!["mission", "out-of-scope", "add", mission, text],
            "decision" => vec!["decision", "add", mission, text, "--by", "Jim"],
            other => panic!("{}", other),
        };
        fx.ok(dir, &args).trim().to_string()
    }

    pub fn a_second_push_renumbers_its_task_and_prints_the_change() {
        let fx = numbered_fixture();
        let (a, other) = (fx.a(), fx.b());
        assert_eq!(fx.ok(&a, &["task", "add", "M-X", "from a"]), "T-02\n");
        assert_eq!(fx.ok(&other, &["task", "add", "M-X", "from b"]), "T-02\n");

        // The first push has nothing to renumber: it prints the commit and nothing else.
        let first = fx.tsk(&a, &["ledger", "push", "A adds a task"]);
        assert_success(&first);
        assert_eq!(stdout(&first).lines().count(), 1);

        let second = fx.tsk(&other, &["ledger", "push", "B adds a task"]);
        assert_success(&second);
        assert_eq!(renumber_lines(&second), vec!["T-02 is now T-03 in M-X"]);
        assert_eq!(stdout(&second).lines().count(), 2);
        assert!(!stderr(&second).contains("conflict"), "{}", stderr(&second));
        let expected = "T-01\ttodo\t-\tbase task\nT-02\ttodo\t-\tfrom a\nT-03\ttodo\t-\tfrom b\n";
        assert_eq!(
            fx.ok(&other, &["task", "list", "M-X", "--porcelain"]),
            expected
        );
        // The remote holds both tasks, and the counter is past both.
        fx.ok(&a, &["ledger", "fetch"]);
        assert_eq!(fx.ok(&a, &["task", "list", "M-X", "--porcelain"]), expected);
        assert_eq!(fx.ok(&other, &["task", "add", "M-X", "third"]), "T-04\n");
        assert_eq!(fx.ok(&a, &["task", "add", "M-X", "fourth"]), "T-04\n");
        assert_success(&fx.tsk(&other, &["ledger", "push", "B adds a third"]));
        let again = fx.tsk(&a, &["ledger", "push", "A adds a fourth"]);
        assert_success(&again);
        assert_eq!(renumber_lines(&again), vec!["T-04 is now T-05 in M-X"]);
        assert_eq!(
            fx.ok(&a, &["task", "list", "M-X", "--porcelain"]),
            format!("{}T-04\ttodo\t-\tthird\nT-05\ttodo\t-\tfourth\n", expected)
        );
    }

    pub fn every_kind_of_record_is_renumbered_at_the_second_push() {
        for (kind, old, new, third) in [
            ("task", "T-02", "T-03", "T-04"),
            ("question", "Q-02", "Q-03", "Q-04"),
            ("objective", "objective 2", "objective 3", "4"),
            ("constraint", "constraint 2", "constraint 3", "4"),
            ("out-of-scope", "out-of-scope 2", "out-of-scope 3", "4"),
            ("decision", "decision 2", "decision 3", ""),
        ] {
            let fx = numbered_fixture();
            let (a, other) = (fx.a(), fx.b());
            add_item(&fx, &a, "M-X", kind, "from a");
            add_item(&fx, &other, "M-X", kind, "from b");
            assert_success(&fx.tsk(&a, &["ledger", "push", "A"]));
            let second = fx.tsk(&other, &["ledger", "push", "B"]);
            assert_success(&second);
            assert_eq!(
                renumber_lines(&second),
                vec![format!("{} is now {} in M-X", old, new)],
                "{}",
                kind
            );
            let found = items(&fx, &other, "M-X", kind);
            let numbers: Vec<i64> = found.iter().map(|i| i.0).collect();
            let texts: Vec<&str> = found.iter().map(|i| i.1.as_str()).collect();
            assert_eq!(numbers, vec![1, 2, 3], "{}", kind);
            assert_eq!(texts[1..], ["from a", "from b"], "{}", kind);
            // The counter is past both rows, here and on the remote.
            let added = add_item(&fx, &other, "M-X", kind, "third");
            if kind != "decision" {
                assert_eq!(added, third, "{}", kind);
            }
            assert_success(&fx.tsk(&other, &["ledger", "push", "B again"]));
            fx.ok(&a, &["ledger", "fetch"]);
            let on_a = items(&fx, &a, "M-X", kind);
            assert_eq!(on_a.len(), 4, "{}", kind);
            assert_eq!(on_a[2].1, "from b", "{}", kind);
            assert_eq!(on_a[3].1, "third", "{}", kind);
        }
    }

    pub fn a_renumbered_task_keeps_its_blocks_and_its_decisions() {
        let fx = numbered_fixture();
        let (a, other) = (fx.a(), fx.b());
        fx.ok(&a, &["task", "add", "M-X", "a1"]);
        assert_success(&fx.tsk(&a, &["ledger", "push", "A"]));
        fx.ok(&other, &["task", "add", "M-X", "b1"]);
        fx.ok(&other, &["task", "add", "M-X", "b2"]);
        fx.ok(&other, &["task", "block", "M-X", "T-03", "T-02"]);
        fx.ok(
            &other,
            &[
                "decision", "add", "M-X", "about b1", "--task", "T-02", "--by", "Jim",
            ],
        );
        fx.ok(
            &other,
            &[
                "decision", "add", "M-X", "about b2", "--task", "T-03", "--by", "Jim",
            ],
        );
        let out = fx.tsk(&other, &["ledger", "push", "B"]);
        assert_success(&out);
        // Only T-02 collided. T-03 is free on the remote, so it keeps its number.
        assert_eq!(renumber_lines(&out), vec!["T-02 is now T-04 in M-X"]);
        assert_eq!(
            fx.ok(&other, &["task", "list", "M-X", "--porcelain"]),
            "T-01\ttodo\t-\tbase task\nT-02\ttodo\t-\ta1\nT-03\ttodo\tT-04\tb2\nT-04\ttodo\t-\tb1\n"
        );
        let decisions = fx.ok(&other, &["decision", "list", "M-X", "--porcelain"]);
        let named: Vec<(&str, &str)> = decisions
            .lines()
            .map(|row| {
                let cells: Vec<&str> = row.split('\t').collect();
                (cells[3], cells[2])
            })
            .collect();
        assert_eq!(
            named,
            vec![
                ("base decision", "-"),
                ("about b1", "T-04"),
                ("about b2", "T-03")
            ]
        );
        fx.ok(&a, &["ledger", "fetch"]);
        assert_eq!(
            fx.ok(&a, &["task", "list", "M-X", "--porcelain"]),
            fx.ok(&other, &["task", "list", "M-X", "--porcelain"])
        );
    }

    pub fn several_collisions_in_one_push_are_all_renumbered_in_order() {
        let fx = numbered_fixture();
        let (a, other) = (fx.a(), fx.b());
        for text in ["a2", "a3"] {
            fx.ok(&a, &["task", "add", "M-X", text]);
        }
        fx.ok(&a, &["question", "add", "M-X", "from a?"]);
        fx.ok(&a, &["mission", "constraint", "add", "M-X", "from a"]);
        for text in ["b2", "b3"] {
            fx.ok(&other, &["task", "add", "M-X", text]);
        }
        fx.ok(&other, &["question", "add", "M-X", "from b?"]);
        fx.ok(&other, &["mission", "constraint", "add", "M-X", "from b"]);
        fx.ok(
            &other,
            &["mission", "constraint", "add", "M-X", "from b too"],
        );
        assert_success(&fx.tsk(&a, &["ledger", "push", "A"]));
        let out = fx.tsk(&other, &["ledger", "push", "B"]);
        assert_success(&out);
        // Only constraint 2 collided: A added no 3.
        assert_eq!(
            renumber_lines(&out),
            vec![
                "T-02 is now T-04 in M-X",
                "T-03 is now T-05 in M-X",
                "Q-02 is now Q-03 in M-X",
                "constraint 2 is now constraint 4 in M-X",
            ]
        );
        let tasks = fx.ok(&other, &["task", "list", "M-X", "--porcelain"]);
        let texts: Vec<&str> = tasks
            .lines()
            .map(|l| l.rsplit('\t').next().unwrap())
            .collect();
        assert_eq!(texts, vec!["base task", "a2", "a3", "b2", "b3"]);
        let constraints = items(&fx, &other, "M-X", "constraint");
        assert_eq!(
            constraints,
            vec![
                (1, "base constraint".to_string()),
                (2, "from a".to_string()),
                (3, "from b too".to_string()),
                (4, "from b".to_string())
            ]
        );
    }

    pub fn a_collision_and_changes_elsewhere_merge_together() {
        let fx = numbered_fixture();
        let (a, other) = (fx.a(), fx.b());
        fx.ok(&a, &["task", "add", "M-X", "from a"]);
        fx.ok(&a, &["mission", "set", "M-X", "purpose", "set by a"]);
        fx.ok(&other, &["task", "add", "M-X", "from b"]);
        fx.ok(&other, &["task", "add", "M-Y", "in y"]);
        fx.ok(&other, &["mission", "create", "M-NEW", "New"]);
        assert_success(&fx.tsk(&a, &["ledger", "push", "A"]));
        let out = fx.tsk(&other, &["ledger", "push", "B"]);
        assert_success(&out);
        // M-Y was untouched on the other side, so its task keeps its number.
        assert_eq!(renumber_lines(&out), vec!["T-02 is now T-03 in M-X"]);
        fx.ok(&a, &["ledger", "fetch"]);
        for dir in [&a, &other] {
            assert_eq!(
                fx.ok(dir, &["task", "list", "M-Y", "--porcelain"]),
                "T-01\ttodo\t-\tin y\n"
            );
            assert_eq!(
                fx.ok(dir, &["mission", "list", "--porcelain"]),
                "M-NEW\ttodo\t-\t-\tNew\nM-X\ttodo\t-\t-\tX\nM-Y\ttodo\t-\t-\tY\n"
            );
            assert_eq!(mission_json(&fx, dir, "M-X")["purpose"], "set by a");
            assert_eq!(items(&fx, dir, "M-X", "task").len(), 3);
        }
    }

    pub fn a_counter_that_both_clones_moved_is_settled_without_a_collision() {
        let fx = numbered_fixture();
        let (a, other) = (fx.a(), fx.b());
        // A adds two outcomes and removes them: no row, but the counter moves to 4.
        fx.ok(&a, &["mission", "objective", "add", "M-X", "gone"]);
        fx.ok(&a, &["mission", "objective", "add", "M-X", "gone too"]);
        fx.ok(&a, &["mission", "objective", "remove", "M-X", "2"]);
        fx.ok(&a, &["mission", "objective", "remove", "M-X", "3"]);
        fx.ok(&other, &["mission", "objective", "add", "M-X", "from b"]);
        assert_success(&fx.tsk(&a, &["ledger", "push", "A"]));
        let out = fx.tsk(&other, &["ledger", "push", "B"]);
        assert_success(&out);
        // Outcome 2 is free on the remote, since A removed it: no collision, no line.
        assert_eq!(renumber_lines(&out), Vec::<String>::new());
        // The numbers A used and dropped are not handed out again.
        assert_eq!(
            fx.ok(&other, &["mission", "objective", "add", "M-X", "next"]),
            "4\n"
        );
    }

    pub fn a_conflict_that_remains_stops_the_push_and_undoes_the_renumbering() {
        let fx = numbered_fixture();
        let (a, other) = (fx.a(), fx.b());
        fx.ok(&a, &["task", "add", "M-X", "from a"]);
        fx.ok(&a, &["mission", "set", "M-X", "status", "in-progress"]);
        assert_success(&fx.tsk(&a, &["ledger", "push", "A"]));
        let before = fx.remote_ledger();
        fx.ok(&other, &["task", "add", "M-X", "from b"]);
        fx.ok(&other, &["mission", "set", "M-X", "status", "done"]);
        let out = fx.tsk(&other, &["ledger", "push", "B"]);
        assert_exit(&out, 1);
        assert!(stderr(&out).contains("conflicted"), "{}", stderr(&out));
        assert!(stderr(&out).contains("missions"), "{}", stderr(&out));
        assert_eq!(stdout(&out), "");
        assert_eq!(fx.remote_ledger(), before);
        // B's local commit stays as it was, with its own number.
        assert_eq!(
            fx.ok(&other, &["task", "list", "M-X", "--porcelain"]),
            "T-01\ttodo\t-\tbase task\nT-02\ttodo\t-\tfrom b\n"
        );
    }
}

macro_rules! conformance {
    ($($case:ident),* $(,)?) => {
        $(
            #[test]
            fn $case() {
                if !dolt_installed() {
                    eprintln!("note: dolt is not installed; skipping {}", stringify!($case));
                    return;
                }
                cases::$case();
            }
        )*
    };
}

conformance!(
    fetch_creates_a_new_ledger_and_the_first_push_creates_the_branch,
    documents_round_trip_between_clones,
    threads_start_pause_resume_list_detach_and_stop,
    cloud_sessions_bind_through_the_ledger,
    worktree_bindings_are_rows_keyed_on_the_clone_and_the_worktree,
    a_marker_file_becomes_a_binding_row_and_is_removed,
    two_actors_bind_one_thread_and_stop_clears_every_kind,
    rebinding_replaces_the_old_binding_of_an_actor,
    two_clones_bind_their_worktrees_at_once_and_both_pushes_succeed,
    a_worktree_resume_and_detach_write_locally_and_the_next_push_leaves_the_store_clean,
    a_worktree_resume_with_origin_away_keeps_the_binding_locally,
    events_append_read_new_and_advance_the_watermark,
    a_stale_push_is_merged_when_different_rows_changed,
    a_same_row_conflict_stops_and_leaves_the_remote_unchanged,
    doc_path_prints_the_file_and_bad_paths_are_refused,
    documents_have_no_reserved_names,
    missions_are_created_and_listed_one_per_line,
    every_mission_field_round_trips,
    status_accepts_three_values_and_an_administrative_mission_keeps_one,
    blocks_are_checked_and_blocked_is_derived,
    objective_outcomes_are_numbered_and_numbers_are_not_reused,
    documents_are_linked_by_path_and_checked_at_push,
    mission_writes_stay_local_until_the_push,
    two_clones_add_missions_that_merge_and_one_field_conflicts,
    the_json_record_has_every_field,
    tasks_are_minted_per_mission_listed_and_changed,
    task_blocks_are_checked_and_blocked_is_derived,
    decisions_are_recorded_with_who_when_and_an_optional_task,
    questions_are_minted_answered_and_listed_while_open,
    two_clones_add_records_to_different_missions_and_the_ledgers_merge,
    thread_start_takes_a_mission_id_and_checks_it_against_records_and_briefings,
    thread_pause_checks_the_task_against_the_mission_record,
    legacy_thread_rows_read_with_a_mission_id_derived_from_the_briefing_path,
    the_stop_hook_check_passes_when_the_ledger_has_no_missions,
    a_mission_document_alone_makes_the_stop_hook_check_block,
    skip_silences_the_stop_hook_check_until_a_thread_starts_or_resumes,
    session_start_names_the_bound_mission_with_its_open_tasks,
    session_start_names_no_index_for_a_bound_mission_with_no_record,
    session_start_shows_the_empty_ledger_note_only_without_records_and_documents,
    session_start_says_a_skipped_session_works_without_a_thread,
    a_thread_can_start_with_no_mission_and_pause_with_an_empty_task,
    thread_set_mission_attaches_changes_and_detaches_a_mission,
    session_start_says_a_bound_thread_with_no_mission_has_no_tasks,
    session_start_carries_the_latest_entry_so_the_agent_can_answer_at_once,
    thread_resume_carries_the_mission_name_and_the_task_description,
    thread_commands_carry_local_record_and_document_changes_to_the_remote,
    status_and_discard_report_and_drop_local_changes,
    status_counts_commits_ahead_and_behind_origin,
    status_works_offline_and_discard_then_changes_nothing,
    status_and_discard_need_a_ledger_store,
    status_prints_local_and_origin_in_plain_columns,
    a_decision_takes_a_date_with_on_and_refuses_one_that_is_not_real,
    a_question_takes_dates_with_on_and_refuses_one_that_is_not_real,
    an_open_question_is_edited_and_an_answered_one_is_not,
    decisions_name_an_adr_and_are_found_from_the_adr,
    constraints_and_out_of_scope_are_numbered_lists_whose_numbers_are_not_reused,
    mission_set_refuses_the_list_fields_and_keeps_the_text_fields,
    lists_print_aligned_columns_and_porcelain_keeps_the_tabs,
    a_second_push_renumbers_its_task_and_prints_the_change,
    every_kind_of_record_is_renumbered_at_the_second_push,
    a_renumbered_task_keeps_its_blocks_and_its_decisions,
    several_collisions_in_one_push_are_all_renumbered_in_order,
    a_collision_and_changes_elsewhere_merge_together,
    a_counter_that_both_clones_moved_is_settled_without_a_collision,
    a_conflict_that_remains_stops_the_push_and_undoes_the_renumbering,
);

// --- A ref that holds a file ledger ---

#[test]
fn a_ref_that_holds_a_file_ledger_is_refused_and_names_the_way_out() {
    if !dolt_installed() {
        eprintln!("note: dolt is not installed; skipping");
        return;
    }
    let fx = Fixture::new();
    // A ledger ref as the removed files backend wrote it.
    let seed = fx.root.path().join("seed");
    git(&seed, &["switch", "--quiet", "--orphan", "files-ledger"]);
    write(&seed.join(".tsk-ledger.toml"), "version = 1\n");
    write(&seed.join("index.md"), "# Files\n");
    git(&seed, &["add", "-A"]);
    git(&seed, &["commit", "--quiet", "-m", "A file ledger"]);
    git(
        &seed,
        &["push", "--quiet", "origin", &format!("HEAD:{}", LEDGER_REF)],
    );

    let output = fx.tsk(&fx.b(), &["ledger", "fetch"]);
    assert_exit(&output, 1);
    let message = stderr(&output);
    assert!(message.contains("holds a file ledger"), "{}", message);
    assert!(message.contains("removed in 0.16.0"), "{}", message);
    assert!(message.contains("tsk 0.15.1"), "{}", message);
    assert!(
        message.contains("tsk ledger migrate-to-dolt"),
        "{}",
        message
    );
}

#[test]
fn a_missing_dolt_binary_is_reported_with_a_hint() {
    let fx = Fixture::new();
    let bin = fx.root.path().join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let git_path = Command::new("sh")
        .args(["-c", "command -v git"])
        .output()
        .unwrap();
    let git_path = String::from_utf8_lossy(&git_path.stdout).trim().to_string();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&git_path, bin.join("git")).unwrap();
    let output = fx
        .command(&fx.a(), &["ledger", "fetch"])
        .env("PATH", &bin)
        .output()
        .unwrap();
    assert_exit(&output, 1);
    let message = stderr(&output);
    assert!(message.contains("dolt is not installed"), "{}", message);
    assert!(
        message.contains("github.com/dolthub/dolt/releases"),
        "{}",
        message
    );
}

// --- The Dolt store ---

#[test]
fn dolt_session_start_exports_the_database_path_and_warns_about_pending_commits() {
    if !dolt_installed() {
        eprintln!("note: dolt is not installed; skipping");
        return;
    }
    let fx = Fixture::fetched();
    let a = fx.a();
    fx.put(&a, "index.md", "# Dolt\n");
    fx.ok(&a, &["ledger", "push", "Publish"]);

    let env_file = fx.root.path().join("claude-env");
    let session_start = |input: &str| {
        let mut child = fx
            .command(&a, &["thread", "session-start"])
            .env("CLAUDE_ENV_FILE", &env_file)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
        child.wait_with_output().unwrap()
    };
    let output = session_start("{\"session_id\":\"s1\",\"source\":\"startup\"}");
    assert_success(&output);
    let path = fx.ledger_path(&a);
    assert!(
        stdout(&output).contains(path.to_str().unwrap()),
        "{}",
        stdout(&output)
    );
    assert!(std::fs::read_to_string(&env_file)
        .unwrap()
        .contains(&format!("export TSK_LEDGER_WT=\"{}\"", path.display())));

    // A commit that never reached the remote.
    let db = |args: &[&str]| {
        common::stop_servers(fx.root.path(), true);
        let out = Command::new("dolt")
            .args(args)
            .current_dir(&path)
            .env("HOME", fx.home())
            .env("DOLT_DISABLE_EVENT_FLUSH", "1")
            .output()
            .unwrap();
        assert!(out.status.success(), "{}", stderr(&out));
    };
    // A data change, committed in the database and never pushed.
    fx.add_binding(&a, "pending");
    db(&["add", "-A"]);
    db(&["commit", "-m", "Pending local work"]);

    let fetch = fx.tsk(&a, &["ledger", "fetch"]);
    assert_success(&fetch);
    assert!(
        stderr(&fetch).contains("holds a commit that is not on"),
        "{}",
        stderr(&fetch)
    );
    assert!(stderr(&fetch).contains("Pending local work"));
    assert_eq!(fx.binding_count(&a), "1");

    let output = session_start("{\"session_id\":\"s2\",\"source\":\"startup\"}");
    assert_success(&output);
    let context = json(stdout(&output).trim())["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(context.contains("WARNING"), "{}", context);
    assert!(!context.contains("index.md"), "{}", context);
    assert!(
        context.contains("dolt log origin/main..main"),
        "{}",
        context
    );

    fx.ok(&a, &["ledger", "push", "Publish pending"]);
    let fetch = fx.tsk(&a, &["ledger", "fetch"]);
    assert!(!stderr(&fetch).contains("holds a commit"));
}

#[test]
fn session_start_names_the_import_guide_only_while_the_ledger_has_no_missions() {
    if !dolt_installed() {
        eprintln!("note: dolt is not installed; skipping");
        return;
    }
    let fx = Fixture::fetched();
    let a = fx.a();
    let session_start = |session: &str| {
        let mut child = fx
            .command(&a, &["thread", "session-start"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(
                format!("{{\"session_id\":\"{}\",\"source\":\"startup\"}}", session).as_bytes(),
            )
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert_success(&output);
        stdout(&output)
    };
    assert!(session_start("empty").contains("run `tsk guide import`"));

    fx.put(&a, "missions/operational/M-X-x.md", "# Mission: X\n");
    fx.ok(&a, &["ledger", "push", "Add M-X"]);
    let with_mission = session_start("with-mission");
    assert!(
        !with_mission.contains("tsk guide import"),
        "{}",
        with_mission
    );

    let guide = fx.ok(&a, &["guide", "import"]);
    assert!(guide.starts_with("# Importing an existing plan into tsk\n"));
}

// --- Documents on the tsk/docs branch ---

const AWKWARD: &str = "no newline at end \"quoted\" 'single' \\ back\n\ttab; drop table documents; -- \u{e9} \u{65e5}\u{672c} \u{1f600}\r\nlast";

fn install_hook(origin: &Path, name: &str, script: &str) -> PathBuf {
    let hook = origin.join("hooks").join(name);
    std::fs::write(&hook, script).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    hook
}

#[test]
fn dolt_documents_are_plain_files_on_the_docs_branch_and_dolt_files_on_the_ledger_branch() {
    if !dolt_installed() {
        eprintln!("note: dolt is not installed; skipping");
        return;
    }
    let fx = Fixture::fetched();
    let (a, other) = (fx.a(), fx.b());
    fx.put(&a, "missions/M-X.md", AWKWARD);
    fx.put(&a, "notes/empty.md", "");
    let pushed = fx.ok(&a, &["ledger", "push", "Add documents"]);
    assert!(!pushed.trim().is_empty());

    assert_eq!(fx.remote_branches(), published_branches());

    // The docs branch: real files, one blob each, byte for byte, and no manifest.
    let files = fx.origin_files(DOCS_REF);
    assert_eq!(
        files,
        vec!["README.md", "index.md", "missions/M-X.md", "notes/empty.md"]
    );
    assert!(!files
        .iter()
        .any(|f| f.contains("tsk-ledger") || f == "manifest"));
    let blob = |path: &str| {
        let out = Command::new("git")
            .args(["cat-file", "blob", &format!("{}:{}", DOCS_REF, path)])
            .current_dir(fx.origin())
            .output()
            .unwrap();
        assert!(out.status.success(), "{}", path);
        String::from_utf8(out.stdout).unwrap()
    };
    assert_eq!(blob("missions/M-X.md"), AWKWARD);
    assert_eq!(blob("notes/empty.md"), "");
    assert_eq!(
        blob("README.md"),
        "Documents of the tsk ledger. Written by tsk ledger doc put.\n"
    );
    let mode = git(&fx.origin(), &["ls-tree", DOCS_REF, "README.md"]);
    assert!(mode.starts_with("100644 blob "), "{}", mode);

    // The ledger branch: Dolt's files only. No Markdown, no file ledger manifest.
    let ledger_files = fx.origin_files(LEDGER_REF);
    assert!(
        ledger_files.iter().any(|f| f == "manifest"),
        "{:?}",
        ledger_files
    );
    assert!(
        !ledger_files.iter().any(|f| f.ends_with(".md")),
        "{:?}",
        ledger_files
    );
    assert!(!ledger_files.iter().any(|f| f == ".tsk-ledger.toml"));

    // A fresh clone gets the document through fetch and doc get.
    let fetch = fx.tsk(&other, &["ledger", "fetch"]);
    assert_success(&fetch);
    assert!(stdout(&fetch).trim_end().ends_with("/ledger-db"));
    assert_eq!(
        fx.ok(&other, &["ledger", "doc", "get", "missions/M-X.md"]),
        AWKWARD
    );
    assert_eq!(
        fx.ok(&other, &["ledger", "doc", "get", "notes/empty.md"]),
        ""
    );
    assert_eq!(
        fx.ok(&other, &["ledger", "doc", "list", "notes/"]),
        "notes/empty.md\n"
    );
    // The documents worktree is a linked worktree of the clone, beside the database.
    let docs = fx.docs_path(&other);
    assert!(docs.join("missions/M-X.md").is_file());
    let listing = git(&other, &["worktree", "list", "--porcelain", "-z"]);
    let canonical = std::fs::canonicalize(&docs).unwrap();
    assert!(listing
        .split('\0')
        .filter_map(|f| f.strip_prefix("worktree "))
        .any(|p| std::fs::canonicalize(p).ok() == Some(canonical.clone())));
}

#[test]
fn dolt_push_prints_the_dolt_commit_or_the_docs_commit_when_only_documents_changed() {
    if !dolt_installed() {
        eprintln!("note: dolt is not installed; skipping");
        return;
    }
    let fx = Fixture::with_briefing();
    let a = fx.a();
    let docs_before = fx.remote_docs();
    let ledger_before = fx.remote_ledger();

    fx.put(&a, "only-docs.md", "d\n");
    let push = fx.tsk(&a, &["ledger", "push", "Only documents"]);
    assert_success(&push);
    let docs_after = fx.remote_docs();
    assert_ne!(docs_after, docs_before);
    assert_eq!(fx.remote_ledger(), ledger_before);
    assert_eq!(stdout(&push).trim_end(), docs_after.unwrap());
    assert!(
        !stderr(&push).contains("nothing to push"),
        "{}",
        stderr(&push)
    );

    // A data change alone leaves the docs branch where it is.
    fx.add_binding(&a, "only-data");
    let docs_now = fx.remote_docs();
    let push = fx.tsk(&a, &["ledger", "push", "Only data"]);
    assert_success(&push);
    assert_eq!(fx.remote_docs(), docs_now);
    assert_ne!(fx.remote_ledger(), ledger_before);
    assert!(!stdout(&push).trim().is_empty());
}

#[test]
fn a_rejected_docs_push_stops_before_the_dolt_push_and_the_next_push_delivers_both() {
    if !dolt_installed() {
        eprintln!("note: dolt is not installed; skipping");
        return;
    }
    let fx = Fixture::with_briefing();
    let (a, other) = (fx.a(), fx.b());
    let ledger_before = fx.remote_ledger();
    let docs_before = fx.remote_docs();

    let hook = install_hook(
        &fx.origin(),
        "pre-receive",
        "#!/bin/sh\nwhile read old new ref; do\n  if [ \"$ref\" = refs/heads/tsk/docs ]; then\n    echo 'docs are closed' >&2\n    exit 1\n  fi\ndone\n",
    );

    fx.put(&a, "late.md", "late\n");
    fx.add_binding(&a, "late");
    let failed = fx.tsk(&a, &["ledger", "push", "Both change"]);
    assert_exit(&failed, 1);
    assert!(
        stderr(&failed).contains("refs/heads/tsk/docs"),
        "{}",
        stderr(&failed)
    );
    // The Dolt data did not go: a record must never name a document the remote lacks.
    assert_eq!(fx.remote_ledger(), ledger_before);
    assert_eq!(fx.remote_docs(), docs_before);
    // Both sides keep their local state. The reader on the other clone sees neither.
    fx.ok(&other, &["ledger", "fetch"]);
    assert_exit(&fx.get(&other, "late.md"), 1);
    assert_eq!(fx.binding_count(&other), "0");

    std::fs::remove_file(&hook).unwrap();
    let delivered = fx.tsk(&a, &["ledger", "push", "Retry"]);
    assert_success(&delivered);
    assert_ne!(fx.remote_docs(), docs_before);
    assert_ne!(fx.remote_ledger(), ledger_before);
    fx.ok(&other, &["ledger", "fetch"]);
    assert_eq!(
        fx.ok(&other, &["ledger", "doc", "get", "late.md"]),
        "late\n"
    );
    assert_eq!(fx.binding_count(&other), "1");
}

#[test]
fn a_stale_docs_push_is_rebased_for_different_files_and_refused_for_the_same_file() {
    if !dolt_installed() {
        eprintln!("note: dolt is not installed; skipping");
        return;
    }
    let fx = Fixture::fetched();
    let (a, other) = (fx.a(), fx.b());
    fx.put(&a, "base.md", "base\n");
    fx.ok(&a, &["ledger", "push", "Base"]);
    fx.ok(&other, &["ledger", "fetch"]);

    // Different files: the second push rebases and goes.
    fx.put(&a, "from-a.md", "a\n");
    fx.ok(&a, &["ledger", "push", "From A"]);
    fx.put(&other, "from-b.md", "b\n");
    fx.ok(&other, &["ledger", "push", "From B"]);
    let files = fx.origin_files(DOCS_REF);
    for expected in ["base.md", "from-a.md", "from-b.md"] {
        assert!(files.iter().any(|f| f == expected), "{:?}", files);
    }
    // Rebased, not merged: the docs history stays linear.
    assert_eq!(
        git(&fx.origin(), &["rev-list", "--merges", "--count", DOCS_REF]),
        "0"
    );

    // The same file: stop, name it, and leave the remote alone.
    fx.ok(&a, &["ledger", "fetch"]);
    fx.put(&a, "same.md", "from a\n");
    fx.ok(&a, &["ledger", "push", "A writes same"]);
    let docs_before = fx.remote_docs();
    fx.put(&other, "same.md", "from b\n");
    let conflict = fx.tsk(&other, &["ledger", "push", "B writes same"]);
    assert_exit(&conflict, 1);
    assert!(
        stderr(&conflict).contains("same.md"),
        "{}",
        stderr(&conflict)
    );
    assert!(
        stderr(&conflict).contains("conflicted"),
        "{}",
        stderr(&conflict)
    );
    assert_eq!(fx.remote_docs(), docs_before);
}

const REPLACED_NOTE: &str =
    "note: replaced an empty local ledger (creation commits only, no rows) with origin's ledger";

#[test]
fn fetch_replaces_an_empty_local_ledger_with_the_ledger_origin_gets_later() {
    if !dolt_installed() {
        eprintln!("note: dolt is not installed; skipping");
        return;
    }
    let fx = Fixture::new();
    let (a, b) = (fx.a(), fx.b());
    // A runs before origin has a ledger, so it makes an empty one of its own.
    fx.ok(&a, &["ledger", "fetch"]);
    // B publishes the first ledger: a different root.
    fx.ok(&b, &["ledger", "fetch"]);
    fx.ok(&b, &["mission", "create", "M-REMOTE", "Made in B"]);
    fx.put(&b, "note.md", "from b\n");
    fx.ok(&b, &["ledger", "push", "B creates the ledger"]);

    let fetch = fx.tsk(&a, &["ledger", "fetch"]);
    assert_success(&fetch);
    assert!(stderr(&fetch).contains(REPLACED_NOTE), "{}", stderr(&fetch));
    assert!(
        !stderr(&fetch).contains("holds a commit"),
        "{}",
        stderr(&fetch)
    );
    assert!(fx.ok(&a, &["mission", "list"]).contains("M-REMOTE"));
    assert_eq!(fx.ok(&a, &["ledger", "doc", "get", "note.md"]), "from b\n");

    // The replacement is once: the next fetch is quiet, and a push works.
    let again = fx.tsk(&a, &["ledger", "fetch"]);
    assert_success(&again);
    assert!(!stderr(&again).contains("replaced"), "{}", stderr(&again));
    fx.ok(&a, &["mission", "create", "M-LATER", "Made in A"]);
    fx.ok(&a, &["ledger", "push", "A adds a mission"]);
}

#[test]
fn fetch_keeps_a_local_ledger_that_holds_a_commit_of_its_own() {
    if !dolt_installed() {
        eprintln!("note: dolt is not installed; skipping");
        return;
    }
    let fx = Fixture::new();
    let (a, b) = (fx.a(), fx.b());
    fx.ok(&a, &["ledger", "fetch"]);
    fx.ok(&a, &["mission", "create", "M-LOCAL", "Made in A"]);
    // A record, committed in the database and never pushed.
    let path = fx.ledger_path(&a);
    let db = |args: &[&str]| {
        common::stop_servers(fx.root.path(), true);
        let out = Command::new("dolt")
            .args(args)
            .current_dir(&path)
            .env("HOME", fx.home())
            .env("DOLT_DISABLE_EVENT_FLUSH", "1")
            .output()
            .unwrap();
        assert!(out.status.success(), "{}", stderr(&out));
    };
    db(&["add", "-A"]);
    db(&["commit", "-m", "Pending local work"]);

    fx.ok(&b, &["ledger", "fetch"]);
    fx.ok(&b, &["mission", "create", "M-REMOTE", "Made in B"]);
    fx.ok(&b, &["ledger", "push", "B creates the ledger"]);

    let fetch = fx.tsk(&a, &["ledger", "fetch"]);
    assert_success(&fetch);
    assert!(
        stderr(&fetch).contains("holds a commit that is not on"),
        "{}",
        stderr(&fetch)
    );
    // The ledger is kept. Its documents branch is still only the creation commit, so that
    // side has nothing to lose and origin's replaces it.
    assert!(
        !stderr(&fetch).contains(REPLACED_NOTE),
        "{}",
        stderr(&fetch)
    );
    let missions = fx.ok(&a, &["mission", "list"]);
    assert!(missions.contains("M-LOCAL"), "{}", missions);
    assert!(!missions.contains("M-REMOTE"), "{}", missions);
}

#[test]
fn unpushed_documents_commits_are_noted_by_fetch_and_a_dirty_docs_worktree_is_refused() {
    if !dolt_installed() {
        eprintln!("note: dolt is not installed; skipping");
        return;
    }
    let fx = Fixture::with_briefing();
    let a = fx.a();
    let docs = fx.docs_path(&a);
    fx.put(&a, "local.md", "local\n");
    git(&docs, &["add", "-A"]);
    git(&docs, &["commit", "--quiet", "-m", "Local document commit"]);

    let fetch = fx.tsk(&a, &["ledger", "fetch"]);
    assert_success(&fetch);
    assert!(
        stderr(&fetch).contains("holds a commit that is not on origin's refs/heads/tsk/docs"),
        "{}",
        stderr(&fetch)
    );
    assert!(stderr(&fetch).contains("Local document commit"));
    assert_eq!(fx.ok(&a, &["ledger", "doc", "get", "local.md"]), "local\n");

    fx.put(&a, "dirty.md", "dirty\n");
    let dirty = fx.tsk(&a, &["ledger", "fetch"]);
    assert_exit(&dirty, 1);
    assert!(
        stderr(&dirty).contains("uncommitted changes"),
        "{}",
        stderr(&dirty)
    );
    assert_success(&fx.tsk(&a, &["ledger", "doc", "rm", "dirty.md"]));

    fx.ok(&a, &["ledger", "push", "Publish local"]);
    let fetch = fx.tsk(&a, &["ledger", "fetch"]);
    assert!(
        !stderr(&fetch).contains("holds a commit"),
        "{}",
        stderr(&fetch)
    );
}

#[test]
fn thread_commands_work_on_a_version_two_ledger() {
    if !dolt_installed() {
        eprintln!("note: dolt is not installed; skipping");
        return;
    }
    let fx = Fixture::with_briefing();
    let a = fx.a();
    let events = json(&fx.ok(&a, &["events", "read-new"]));
    assert_eq!(events["total_count"], 0);

    let start = fx.ok(&a, &["thread", "start", "M-CONF-01"]);
    assert!(start.starts_with("started:"), "{}", start);
    assert!(fx.ok(&a, &["thread", "detach"]).starts_with("detached:"));
    let missing = fx.tsk(&a, &["thread", "start", "M-NONE"]);
    assert_exit(&missing, 1);
    assert!(
        stderr(&missing).contains("no such mission: M-NONE"),
        "{}",
        stderr(&missing)
    );
}

// --- Mission and task tables and the version 8 ledger ---

const TASK_TABLES: [&str; 4] = ["questions", "decisions", "task_blocks", "tasks"];

const MISSION_TABLES: [&str; 10] = [
    "mission_out_of_scope",
    "mission_constraints",
    "questions",
    "decisions",
    "task_blocks",
    "tasks",
    "mission_documents",
    "mission_blocks",
    "objective_outcomes",
    "missions",
];

/// What a ledger below version 7 has for the constraints and out of scope of a mission: two
/// text columns on `missions`, and no list tables or counters.
const OLD_LISTS: [&str; 6] = [
    "drop table mission_constraints",
    "drop table mission_out_of_scope",
    "alter table missions drop column next_constraint",
    "alter table missions drop column next_out_of_scope",
    "alter table missions add column constraints_text longtext null",
    "alter table missions add column out_of_scope longtext null",
];

/// What a ledger below version 8 has for bindings: the `cloud_session_bindings` table, and
/// no `bindings`. It has no `actors` and no `instances` either (version 9).
const OLD_BINDINGS: [&str; 4] = [
    "drop table actors",
    "drop table instances",
    "drop table bindings",
    "create table cloud_session_bindings (session_id varchar(255) primary key, thread_id varchar(64) not null, registered_at varchar(32) not null)",
];

/// The statements that take a current ledger back to what versions 3 to 7 have for the
/// lists and the bindings, joined for one `dolt sql` call.
fn old_schema() -> String {
    OLD_LISTS
        .iter()
        .chain(OLD_BINDINGS.iter())
        .copied()
        .collect::<Vec<_>>()
        .join("; ")
}

/// The statements that take the missions of a current ledger back to what layouts 3 to 9
/// have: no `code` column, and the code as the ID in every table that names a mission. A
/// mission made by this binary has a random ID, so each table is rewritten to the code.
/// A fixture for an older layout runs them first, then removes what that layout lacks.
fn layout_nine_sql() -> String {
    let mut sql = vec!["set foreign_key_checks = 0".to_string()];
    for table in [
        "objective_outcomes",
        "mission_blocks",
        "mission_documents",
        "mission_constraints",
        "mission_out_of_scope",
        "tasks",
        "task_blocks",
        "decisions",
        "questions",
    ] {
        sql.push(format!(
            "update {t} set mission_id = (select code from missions where id = {t}.mission_id)",
            t = table
        ));
    }
    sql.push(
        "update mission_blocks set blocked_by_id = (select code from missions \
         where id = mission_blocks.blocked_by_id)"
            .to_string(),
    );
    sql.push(
        "update threads set mission_id = coalesce((select code from missions \
         where id = threads.mission_id), mission_id)"
            .to_string(),
    );
    sql.push(
        "update missions set parent = (select c from (select id as i, code as c from missions) x \
         where x.i = missions.parent)"
            .to_string(),
    );
    sql.push("update missions set id = code".to_string());
    sql.push("alter table missions drop column code".to_string());
    sql.join("; ")
}

/// What a ledger below version 5 has for `threads`: no `mission_id`, and a briefing path
/// that is never null.
const OLD_THREADS: [&str; 2] = [
    "alter table threads drop column mission_id",
    "alter table threads modify column briefing_path varchar(1024) not null",
];

impl Fixture {
    fn manifest_version(&self, dir: &Path) -> String {
        self.dolt_sql(dir, "select v from manifest where k = 'version'")
            .lines()
            .nth(1)
            .unwrap_or("")
            .to_string()
    }

    fn tables(&self, dir: &Path) -> String {
        self.dolt_sql(dir, "show tables")
    }

    /// A published Dolt ledger at version 2, as the previous release left it: clone A
    /// drops the mission tables, records the version and pushes.
    fn version_two_ledger() -> Fixture {
        Fixture::cached("version-two", || {
            (Fixture::build_version_two_ledger(), String::new())
        })
        .0
    }

    fn build_version_two_ledger() -> Fixture {
        let fx = Fixture::new();
        let a = fx.a();
        fx.ok(&a, &["ledger", "fetch"]);
        let mut sql: Vec<String> = MISSION_TABLES
            .iter()
            .map(|t| format!("drop table {}", t))
            .collect();
        sql.extend(OLD_THREADS.iter().map(|s| s.to_string()));
        sql.extend(OLD_BINDINGS.iter().map(|s| s.to_string()));
        sql.push("replace into manifest (k, v) values ('version', '2')".to_string());
        fx.dolt_sql(&a, &sql.join("; "));
        fx.put(&a, "base.md", "base\n");
        fx.ok(&a, &["ledger", "push", "A version 2 ledger"]);
        fx
    }
}

impl Fixture {
    /// A published Dolt ledger at version 3, as the previous release left it: one mission
    /// with an objective outcome and a blocker, and no task, decision or question tables.
    fn version_three_ledger() -> Fixture {
        Fixture::cached("version-three", || {
            (Fixture::build_version_three_ledger(), String::new())
        })
        .0
    }

    fn build_version_three_ledger() -> Fixture {
        let fx = Fixture::new();
        let a = fx.a();
        fx.ok(&a, &["ledger", "fetch"]);
        fx.ok(&a, &["mission", "create", "M-OLD", "Made on three"]);
        fx.ok(&a, &["mission", "create", "M-DEP", "Dependency"]);
        fx.ok(&a, &["mission", "block", "M-OLD", "M-DEP"]);
        fx.ok(&a, &["mission", "objective", "add", "M-OLD", "An outcome"]);
        fx.ok(&a, &["mission", "set", "M-OLD", "purpose", "Keep me"]);
        let mut sql: Vec<String> = TASK_TABLES
            .iter()
            .map(|t| format!("drop table {}", t))
            .collect();
        sql.push("alter table missions drop column next_task".to_string());
        sql.push("alter table missions drop column next_question".to_string());
        sql.extend(OLD_LISTS.iter().map(|s| s.to_string()));
        sql.extend(OLD_BINDINGS.iter().map(|s| s.to_string()));
        sql.extend(OLD_THREADS.iter().map(|s| s.to_string()));
        sql.push("replace into manifest (k, v) values ('version', '3')".to_string());
        fx.dolt_sql(&a, &layout_nine_sql());
        fx.dolt_sql(&a, &sql.join("; "));
        fx.ok(&a, &["ledger", "push", "A version 3 ledger"]);
        fx
    }
}

#[test]
fn a_version_three_ledger_reads_without_task_tables_and_the_first_record_write_upgrades_it() {
    if !dolt_installed() {
        eprintln!("note: dolt is not installed; skipping");
        return;
    }
    let fx = Fixture::version_three_ledger();
    let (a, other) = (fx.a(), fx.b());
    fx.ok(&other, &["ledger", "fetch"]);
    assert_eq!(fx.manifest_version(&other), "3");
    assert!(!fx.tables(&other).contains("tasks"));

    // Reads see the missions and no records, and write nothing.
    assert_eq!(fx.ok(&other, &["task", "list", "M-OLD", "--porcelain"]), "");
    assert_eq!(
        fx.ok(&other, &["decision", "list", "M-OLD", "--porcelain"]),
        ""
    );
    assert_eq!(fx.ok(&other, &["question", "list", "--porcelain"]), "");
    let before = fx.ok(&other, &["mission", "list", "M-OLD", "--json"]);
    let parsed = json(&before);
    assert_eq!(parsed["tasks"], serde_json::json!([]));
    assert_eq!(parsed["decisions"], serde_json::json!([]));
    assert_eq!(parsed["questions"], serde_json::json!([]));
    // A refused write upgrades nothing.
    let out = fx.tsk(&other, &["task", "add", "M-NONE", "x"]);
    assert_exit(&out, 1);
    assert!(stderr(&out).contains("no such mission: M-NONE"));
    assert_eq!(fx.manifest_version(&other), "3");
    assert_eq!(
        fx.dolt_sql(&other, "select count(*) as c from dolt_status"),
        "c\n0\n"
    );

    // The first write creates the tables, adds the counters, adds the thread column and
    // records version 9.
    assert_eq!(
        fx.ok(&other, &["task", "add", "M-OLD", "First on version 3"]),
        "T-01\n"
    );
    assert_eq!(fx.manifest_version(&other), "10");
    let tables = fx.tables(&other);
    for table in TASK_TABLES {
        assert!(tables.lines().any(|t| t == table), "{}: {}", table, tables);
    }
    assert_eq!(
        fx.dolt_sql(
            &other,
            "select next_task, next_question from missions order by id"
        ),
        "next_task,next_question\n1,1\n2,1\n"
    );
    // The mission rows are intact.
    assert_eq!(
        fx.ok(&other, &["mission", "list", "--porcelain"]),
        "M-DEP\ttodo\t-\t-\tDependency\nM-OLD\ttodo\tM-DEP\t-\tMade on three\n"
    );
    let after = json(&fx.ok(&other, &["mission", "list", "M-OLD", "--json"]));
    assert_eq!(after["purpose"], "Keep me");
    assert_eq!(
        after["objective"],
        serde_json::json!([{"n": 1, "outcome": "An outcome", "met": false}])
    );
    assert_eq!(after["blocked_by"][0]["id"], "M-DEP");
    assert_eq!(after["tasks"][0]["description"], "First on version 3");
    // Every record kind works afterwards.
    fx.ok(&other, &["task", "add", "M-OLD", "Second"]);
    fx.ok(&other, &["task", "block", "M-OLD", "T-02", "T-01"]);
    fx.ok(
        &other,
        &[
            "decision", "add", "M-OLD", "Go", "--by", "Jim", "--task", "T-01",
        ],
    );
    assert_eq!(
        fx.ok(&other, &["question", "add", "M-OLD", "Why?"]),
        "Q-01\n"
    );

    // The other clone is still at version 3 until the push.
    assert_eq!(fx.manifest_version(&a), "3");
    fx.ok(&other, &["ledger", "push", "First task"]);
    fx.ok(&a, &["ledger", "fetch"]);
    assert_eq!(fx.manifest_version(&a), "10");
    assert_eq!(
        fx.ok(&a, &["task", "list", "M-OLD", "--porcelain"]),
        "T-01\ttodo\t-\tFirst on version 3\nT-02\ttodo\tT-01\tSecond\n"
    );
    assert_eq!(
        fx.ok(&a, &["decision", "list", "M-OLD", "--porcelain"])
            .lines()
            .count(),
        1
    );
    assert_eq!(
        fx.ok(&a, &["question", "list", "--porcelain"])
            .lines()
            .count(),
        1
    );
}

#[test]
fn a_version_three_ledger_upgrades_at_a_mission_write_too() {
    if !dolt_installed() {
        eprintln!("note: dolt is not installed; skipping");
        return;
    }
    let fx = Fixture::version_three_ledger();
    let other = fx.b();
    fx.ok(&other, &["ledger", "fetch"]);
    assert_eq!(fx.manifest_version(&other), "3");
    fx.ok(&other, &["mission", "create", "M-NEW", "New"]);
    assert_eq!(fx.manifest_version(&other), "10");
    assert_eq!(fx.ok(&other, &["task", "add", "M-OLD", "x"]), "T-01\n");
}

#[test]
fn a_new_dolt_ledger_has_the_mission_and_task_tables_at_version_eight() {
    if !dolt_installed() {
        eprintln!("note: dolt is not installed; skipping");
        return;
    }
    let fx = Fixture::fetched();
    let a = fx.a();
    assert_eq!(fx.manifest_version(&a), "10");
    let tables = fx.tables(&a);
    for table in MISSION_TABLES {
        assert!(tables.contains(table), "{}", tables);
    }
    assert_eq!(
        fx.dolt_sql(
            &a,
            "select column_name, is_nullable from information_schema.columns where table_name = 'threads' order by ordinal_position"
        ),
        "COLUMN_NAME,IS_NULLABLE\nthread_id,NO\nbriefing_path,YES\nmission_id,YES\n"
    );
}

#[test]
fn a_version_two_ledger_reads_as_no_missions_and_the_first_mission_write_upgrades_it() {
    if !dolt_installed() {
        eprintln!("note: dolt is not installed; skipping");
        return;
    }
    let fx = Fixture::version_two_ledger();
    let (a, other) = (fx.a(), fx.b());
    fx.ok(&other, &["ledger", "fetch"]);
    assert_eq!(fx.manifest_version(&other), "2");

    // Reads behave as an empty mission set and write nothing.
    assert_eq!(fx.ok(&other, &["mission", "list", "--porcelain"]), "");
    assert_eq!(fx.ok(&other, &["mission", "list", "--json"]), "[]\n");
    for args in [
        vec!["mission", "list", "M-X", "--porcelain"],
        vec!["mission", "list", "M-X", "--json"],
        vec!["mission", "objective", "list", "M-X"],
        vec!["mission", "set", "M-X", "status", "done"],
        vec!["mission", "block", "M-X", "M-Y"],
        vec!["mission", "unblock", "M-X", "M-Y"],
        vec!["mission", "objective", "add", "M-X", "One"],
        vec!["mission", "link", "M-X", "base.md", "--role", "report"],
        vec!["mission", "unlink", "M-X", "base.md"],
        vec!["mission", "create", "M-Y", "Bad", "--parent", "M-NONE"],
        vec!["mission", "create", "bad id", "Bad"],
    ] {
        let out = fx.tsk(&other, &args);
        assert_exit(&out, 1);
        assert!(
            stderr(&out).starts_with("error: "),
            "{:?}: {}",
            args,
            stderr(&out)
        );
    }
    assert_eq!(fx.manifest_version(&other), "2");
    assert!(
        !fx.tables(&other).contains("missions"),
        "{}",
        fx.tables(&other)
    );
    assert_eq!(
        fx.dolt_sql(&other, "select count(*) as c from dolt_status"),
        "c\n0\n"
    );
    // Everything else a version 2 ledger did still works, and it pushes.
    assert_eq!(
        fx.ok(&other, &["ledger", "doc", "get", "base.md"]),
        "base\n"
    );
    // The first mission write creates the tables, the column and records version 9 in one
    // change.
    fx.ok(&other, &["mission", "create", "M-X", "First on version 2"]);
    assert_eq!(fx.manifest_version(&other), "10");
    let tables = fx.tables(&other);
    for table in MISSION_TABLES {
        assert!(tables.contains(table), "{}", tables);
    }
    let status = fx.dolt_sql(
        &other,
        "select table_name from dolt_status order by table_name",
    );
    assert!(status.contains("manifest"), "{}", status);
    assert!(status.contains("missions"), "{}", status);
    assert!(status.contains("threads"), "{}", status);
    assert_eq!(
        fx.ok(&other, &["mission", "list", "--porcelain"]),
        "M-X\ttodo\t-\t-\tFirst on version 2\n"
    );
    // The other clone is still at version 2 until the push.
    assert_eq!(fx.manifest_version(&a), "2");
    fx.ok(&other, &["ledger", "push", "First mission"]);
    fx.ok(&a, &["ledger", "fetch"]);
    assert_eq!(fx.manifest_version(&a), "10");
    assert_eq!(
        fx.ok(&a, &["mission", "list", "--porcelain"]),
        "M-X\ttodo\t-\t-\tFirst on version 2\n"
    );
    fx.ok(&a, &["mission", "create", "M-Y", "Second"]);
    assert_eq!(fx.manifest_version(&a), "10");
}

#[test]
fn a_version_two_ledger_upgrades_at_the_first_thread_write_and_not_before() {
    if !dolt_installed() {
        eprintln!("note: dolt is not installed; skipping");
        return;
    }
    let fx = Fixture::version_two_ledger();
    let (a, other) = (fx.a(), fx.b());
    fx.ok(&other, &["ledger", "fetch"]);
    fx.put(&other, "missions/operational/M-CONF-01-old.md", "# Old\n");
    fx.ok(&other, &["ledger", "push", "A briefing"]);
    assert_eq!(fx.manifest_version(&other), "2");

    // Reads and refused writes upgrade nothing.
    assert_eq!(fx.ok(&other, &["thread", "list"]), "");
    assert_exit(&fx.tsk(&other, &["thread", "start", "M-NONE"]), 1);
    assert_exit(
        &fx.tsk(&other, &["thread", "pause", "aaaa0000", "", "x"]),
        1,
    );
    assert_eq!(fx.ok(&other, &["thread", "skip"]), "skipped\n");
    assert_eq!(fx.manifest_version(&other), "2");
    assert_eq!(
        fx.dolt_sql(&other, "select count(*) as c from dolt_status"),
        "c\n0\n"
    );

    // The first thread write creates the tables and the column and records version 9.
    let started = fx.ok(&other, &["thread", "start", "M-CONF-01"]);
    let id = started
        .trim_end()
        .strip_prefix("started:")
        .unwrap()
        .to_string();
    assert_eq!(fx.manifest_version(&other), "10");
    let tables = fx.tables(&other);
    for table in MISSION_TABLES {
        assert!(tables.contains(table), "{}", tables);
    }
    assert_eq!(
        fx.dolt_sql(
            &other,
            &format!(
                "select mission_id, briefing_path from threads where thread_id = '{}'",
                id
            )
        ),
        "mission_id,briefing_path\nM-CONF-01,missions/operational/M-CONF-01-old.md\n"
    );
    // The start pushed itself.
    assert_eq!(fx.manifest_version(&a), "2");
    fx.ok(&a, &["ledger", "fetch"]);
    assert_eq!(fx.manifest_version(&a), "10");
    assert_eq!(fx.ok(&a, &["mission", "list", "--porcelain"]), "");
}

impl Fixture {
    /// A published Dolt ledger at version 4, as the previous release left it: `threads`
    /// has no `mission_id`, its `briefing_path` is not null, and it holds one legacy
    /// thread for the briefing of M-CONF-01.
    fn version_four_ledger() -> Fixture {
        Fixture::cached("version-four", || {
            (Fixture::build_version_four_ledger(), String::new())
        })
        .0
    }

    fn build_version_four_ledger() -> Fixture {
        let fx = Fixture::new();
        let a = fx.a();
        fx.ok(&a, &["ledger", "fetch"]);
        fx.put(&a, BRIEFING, "# Mission: conformance\n");
        fx.dolt_sql(&a, &layout_nine_sql());
        fx.dolt_sql(
            &a,
            &format!(
                "{}; alter table threads drop column mission_id; \
                 alter table threads modify column briefing_path varchar(1024) not null; \
                 alter table decisions drop column adr_path; \
                 replace into manifest (k, v) values ('version', '4'); \
                 insert into threads (thread_id, briefing_path) values ('aaaa1111', '{}')",
                old_schema(),
                BRIEFING
            ),
        );
        fx.ok(&a, &["ledger", "push", "A version 4 ledger"]);
        fx
    }
}

#[test]
fn a_version_four_ledger_reads_threads_and_upgrades_at_the_first_thread_start() {
    if !dolt_installed() {
        eprintln!("note: dolt is not installed; skipping");
        return;
    }
    let fx = Fixture::version_four_ledger();
    let (a, other) = (fx.a(), fx.b());
    fx.ok(&other, &["ledger", "fetch"]);
    assert_eq!(fx.manifest_version(&other), "4");

    // Reads see the legacy thread with a derived mission ID, and write nothing.
    let row = fx.thread_row(&other, "aaaa1111");
    assert_eq!(row["mission_id"], "M-CONF-01");
    assert_eq!(row["mission_link"], BRIEFING);
    assert!(fx.guard_reason(&other, None).is_some());
    let context = fx.session_context(&other, "s-four", "claude");
    assert!(context.contains("No thread binding"), "{}", context);
    assert_eq!(fx.ok(&other, &["thread", "skip"]), "skipped\n");
    // Refused writes upgrade nothing either.
    let refused = fx.tsk(&other, &["thread", "start", "M-NONE"]);
    assert_exit(&refused, 1);
    assert!(stderr(&refused).contains("M-NONE"), "{}", stderr(&refused));
    assert_exit(
        &fx.tsk(&other, &["thread", "pause", "bbbb2222", "", "x"]),
        1,
    );
    assert_eq!(fx.manifest_version(&other), "4");
    assert_eq!(
        fx.dolt_sql(&other, "select count(*) as c from dolt_status"),
        "c\n0\n"
    );

    // A pause appends an entry, which needs no new column: the version stays 4.
    fx.ok(&other, &["thread", "pause", "aaaa1111", "", "carry on"]);
    assert_eq!(fx.manifest_version(&other), "4");
    assert_eq!(
        fx.latest_entry(&other, "aaaa1111")["mission_id"],
        "M-CONF-01"
    );
    fx.ok(&other, &["thread", "detach"]);

    // The first thread start alters the table and records version 9, in its own push.
    let id = fx.started(&other, &["thread", "start", "M-CONF-01"]);
    assert_eq!(fx.manifest_version(&other), "10");
    assert_eq!(
        fx.dolt_sql(
            &other,
            "select thread_id, mission_id, briefing_path from threads order by thread_id = 'aaaa1111' desc"
        ),
        format!(
            "thread_id,mission_id,briefing_path\naaaa1111,,{}\n{},M-CONF-01,{}\n",
            BRIEFING, id, BRIEFING
        )
    );
    assert_eq!(fx.thread_row(&other, "aaaa1111")["mission_id"], "M-CONF-01");

    // The other clone takes the upgrade with its next fetch.
    assert_eq!(fx.manifest_version(&a), "4");
    fx.ok(&a, &["ledger", "fetch"]);
    assert_eq!(fx.manifest_version(&a), "10");
    assert_eq!(fx.thread_row(&a, &id)["mission_id"], "M-CONF-01");
    assert_eq!(fx.thread_row(&a, "aaaa1111")["mission_id"], "M-CONF-01");
}

#[test]
fn a_ledger_version_the_binary_does_not_know_stops_every_command_at_the_manifest_check() {
    // What the previous release does with a Dolt ledger the new release has written:
    // the manifest check is the first thing every command runs. The supported list is
    // faked in a unit test (ledger::manifest); here a version no release knows yet
    // stands in, because this binary supports 1, 2, 3, 4, 5, 6, 7, 8, 9 and 10.
    if !dolt_installed() {
        eprintln!("note: dolt is not installed; skipping");
        return;
    }
    let fx = Fixture::fetched();
    let a = fx.a();
    fx.dolt_sql(&a, "replace into manifest (k, v) values ('version', '11')");
    for args in [
        vec!["mission", "list", "--porcelain"],
        vec!["mission", "create", "M-X", "x"],
        vec!["ledger", "push", "x"],
    ] {
        let out = fx.tsk(&a, &args);
        assert_exit(&out, 1);
        let message = stderr(&out);
        assert!(
            message.contains("unsupported ledger version 11")
                && message.contains("supports version 1, 2, 3, 4, 5, 6, 7, 8, 9, 10")
                && message.contains("Update tsk"),
            "{:?}: {}",
            args,
            message
        );
    }
}

#[test]
fn dolt_status_marks_a_table_whose_schema_changed_and_rows_did_not() {
    if !dolt_installed() {
        eprintln!("note: dolt is not installed; skipping");
        return;
    }
    let fx = Fixture::with_briefing();
    let a = fx.a();
    fx.dolt_sql(&a, "alter table watermark add column note int null");
    let out = fx.ok(&a, &["ledger", "status", "--porcelain"]);
    assert!(
        out.lines().any(|l| l == "data\twatermark\tschema changed"),
        "{}",
        out
    );
    fx.add_binding(&a, "s1");
    fx.dolt_sql(&a, "alter table bindings add column note2 int null");
    let out = fx.ok(&a, &["ledger", "status", "--porcelain"]);
    assert!(
        out.lines()
            .any(|l| l == "data\tbindings\tadded 1\tmodified 0\tdeleted 0\tschema changed"),
        "{}",
        out
    );
    assert_eq!(fx.ok(&a, &["ledger", "discard", "--yes"]), "discarded\n");
    assert_eq!(
        fx.ok(&a, &["ledger", "status", "--porcelain"])
            .lines()
            .next(),
        Some("data\tclean")
    );
}

#[test]
fn dolt_status_labels_a_schema_upgrade_and_hides_the_lines_it_caused() {
    if !dolt_installed() {
        eprintln!("note: dolt is not installed; skipping");
        return;
    }
    let fx = Fixture::version_two_ledger();
    let (a, other) = (fx.a(), fx.b());
    fx.ok(&other, &["ledger", "fetch"]);
    assert_eq!(fx.manifest_version(&other), "2");
    // Before any write there is nothing to label.
    assert_eq!(
        fx.ok(&other, &["ledger", "status", "--porcelain"]),
        "data\tclean\ndocs\tclean\ndata\tremote\tup to date\ndocs\tremote\tup to date\n"
    );

    // The first write upgrades the layout and adds a mission. The status shows the
    // upgrade as one change and the mission as another, and no manifest or schema lines.
    fx.ok(
        &other,
        &["mission", "create", "M-NEW", "First on version 2"],
    );
    assert_eq!(fx.manifest_version(&other), "10");
    assert_eq!(
        fx.ok(&other, &["ledger", "status", "--porcelain"]),
        "data\tlocal changes\n\
         data\tupgrade\tschema upgraded to version 10\n\
         data\tmissions\tadded 1\tmodified 0\tdeleted 0\n\
         docs\tclean\n\
         data\tremote\tup to date\n\
         docs\tremote\tup to date\n"
    );
    assert_eq!(
        no_server_line(fx.ok(&other, &["ledger", "status"])),
        "Local   2 changes not pushed\n\
         \x20         data  upgrade   schema upgraded to version 10 (origin has 2)\n\
         \x20         data  missions  1 added\n\
         Origin  up to date\n"
    );
    fx.ok(&other, &["ledger", "push", "Upgrade and add a mission"]);
    assert_eq!(
        fx.ok(&other, &["ledger", "status", "--porcelain"])
            .lines()
            .next(),
        Some("data\tclean")
    );

    // Clone A takes the upgrade with its fetch. Its next write is an ordinary change:
    // origin has the same version, so there is no label.
    fx.ok(&a, &["ledger", "fetch"]);
    assert_eq!(fx.manifest_version(&a), "10");
    fx.ok(&a, &["mission", "create", "M-LATER", "After the upgrade"]);
    assert_eq!(
        fx.ok(&a, &["ledger", "status", "--porcelain"]),
        "data\tlocal changes\n\
         data\tmissions\tadded 1\tmodified 0\tdeleted 0\n\
         docs\tclean\n\
         data\tremote\tup to date\n\
         docs\tremote\tup to date\n"
    );
    assert_eq!(
        no_server_line(fx.ok(&a, &["ledger", "status"])),
        "Local   1 change not pushed\n\
         \x20         data  missions  1 added\n\
         Origin  up to date\n"
    );
}

#[test]
fn dolt_status_keeps_the_rows_a_person_changed_in_an_upgrade_write() {
    if !dolt_installed() {
        eprintln!("note: dolt is not installed; skipping");
        return;
    }
    // Version 3 holds two missions. The write edits one of them. Dolt counts both rows
    // as modified because the table gained columns; only the edited one is a change.
    let fx = Fixture::version_three_ledger();
    let other = fx.b();
    fx.ok(&other, &["ledger", "fetch"]);
    fx.ok(&other, &["mission", "set", "M-OLD", "purpose", "Changed"]);
    assert_eq!(fx.manifest_version(&other), "10");
    assert_eq!(
        fx.ok(&other, &["ledger", "status", "--porcelain"]),
        "data\tlocal changes\n\
         data\tupgrade\tschema upgraded to version 10\n\
         data\tmissions\tadded 0\tmodified 1\tdeleted 0\n\
         docs\tclean\n\
         data\tremote\tup to date\n\
         docs\tremote\tup to date\n"
    );
}

impl Fixture {
    /// A published Dolt ledger at version 5, as the previous release left it: a mission
    /// with a task and a decision, and a `decisions` table without `adr_path`.
    fn version_five_ledger() -> Fixture {
        Fixture::cached("version-five", || {
            (Fixture::build_version_five_ledger(), String::new())
        })
        .0
    }

    fn build_version_five_ledger() -> Fixture {
        let fx = Fixture::new();
        let a = fx.a();
        fx.ok(&a, &["ledger", "fetch"]);
        fx.ok(&a, &["mission", "create", "M-OLD", "Made on five"]);
        fx.ok(&a, &["task", "add", "M-OLD", "A task"]);
        fx.ok(
            &a,
            &[
                "decision",
                "add",
                "M-OLD",
                "Old call",
                "--by",
                "Jim",
                "--task",
                "T-01",
                "--on",
                "2026-01-02",
            ],
        );
        fx.dolt_sql(&a, &layout_nine_sql());
        fx.dolt_sql(
            &a,
            &format!(
                "{}; alter table decisions drop column adr_path; \
                 replace into manifest (k, v) values ('version', '5')",
                old_schema()
            ),
        );
        fx.ok(&a, &["ledger", "push", "A version 5 ledger"]);
        fx
    }
}

#[test]
fn a_version_five_ledger_reads_decisions_and_the_first_adr_write_adds_the_column() {
    if !dolt_installed() {
        eprintln!("note: dolt is not installed; skipping");
        return;
    }
    let fx = Fixture::version_five_ledger();
    let (a, other) = (fx.a(), fx.b());
    fx.ok(&other, &["ledger", "fetch"]);
    assert_eq!(fx.manifest_version(&other), "5");
    assert!(!fx
        .dolt_sql(&other, "show create table decisions")
        .contains("adr_path"));

    // Reads see the decision with no ADR, and write nothing.
    assert_eq!(
        fx.ok(&other, &["decision", "list", "M-OLD", "--porcelain"]),
        "2026-01-02\tJim\tT-01\tOld call\t-\n"
    );
    assert_eq!(
        fx.ok(
            &other,
            &["decision", "list", "--adr", "docs/adr/x.md", "--porcelain"]
        ),
        ""
    );
    let record = json(&fx.ok(&other, &["mission", "list", "M-OLD", "--json"]));
    assert_eq!(record["decisions"][0]["adr"], serde_json::Value::Null);
    // A write that is refused upgrades nothing, an ADR that is not a file included.
    let refused = fx.tsk(
        &other,
        &[
            "decision",
            "add",
            "M-OLD",
            "x",
            "--by",
            "Jim",
            "--adr",
            "docs/adr/none.md",
        ],
    );
    assert_exit(&refused, 1);
    assert_exit(
        &fx.tsk(&other, &["decision", "add", "M-NONE", "x", "--by", "Jim"]),
        1,
    );
    assert_eq!(fx.manifest_version(&other), "5");
    assert_eq!(
        fx.dolt_sql(&other, "select count(*) as c from dolt_status"),
        "c\n0\n"
    );
    // The first write that names an ADR adds the column.
    std::fs::create_dir_all(other.join("docs/adr")).unwrap();
    write(&other.join("docs/adr/0001-first.md"), "# ADR 1\n");
    fx.ok(
        &other,
        &[
            "decision",
            "add",
            "M-OLD",
            "New call",
            "--by",
            "Jim",
            "--on",
            "2026-01-03",
            "--adr",
            "docs/adr/0001-first.md",
        ],
    );
    assert_eq!(fx.manifest_version(&other), "10");
    assert_eq!(
        fx.dolt_sql(
            &other,
            "select column_name, is_nullable, column_type from information_schema.columns \
             where table_name = 'decisions' and column_name = 'adr_path'"
        ),
        "COLUMN_NAME,IS_NULLABLE,COLUMN_TYPE\nadr_path,YES,varchar(1024)\n"
    );
    // The old decision keeps its row, with a null path.
    assert_eq!(
        fx.ok(&other, &["decision", "list", "M-OLD", "--porcelain"]),
        "2026-01-02\tJim\tT-01\tOld call\t-\n2026-01-03\tJim\t-\tNew call\tdocs/adr/0001-first.md\n"
    );
    assert_eq!(
        fx.dolt_sql(&other, "select seq, adr_path from decisions order by seq"),
        "seq,adr_path\n1,\n2,docs/adr/0001-first.md\n"
    );
    // The status labels the upgrade, and shows the one decision that was added.
    assert_eq!(
        fx.ok(&other, &["ledger", "status", "--porcelain"]),
        "data\tlocal changes\n\
         data\tupgrade\tschema upgraded to version 10\n\
         data\tdecisions\tadded 1\tmodified 0\tdeleted 0\n\
         docs\tclean\n\
         data\tremote\tup to date\n\
         docs\tremote\tup to date\n"
    );

    // The other clone is still at version 5 until the push.
    assert_eq!(fx.manifest_version(&a), "5");
    fx.ok(&other, &["ledger", "push", "First ADR link"]);
    fx.ok(&a, &["ledger", "fetch"]);
    assert_eq!(fx.manifest_version(&a), "10");
    assert_eq!(
        fx.ok(
            &a,
            &[
                "decision",
                "list",
                "--adr",
                "docs/adr/0001-first.md",
                "--porcelain"
            ]
        ),
        "M-OLD\t2026-01-03\tJim\t-\tNew call\tdocs/adr/0001-first.md\n"
    );
}

#[test]
fn a_decision_without_an_adr_upgrades_a_version_five_ledger_too() {
    if !dolt_installed() {
        eprintln!("note: dolt is not installed; skipping");
        return;
    }
    let fx = Fixture::version_five_ledger();
    let other = fx.b();
    fx.ok(&other, &["ledger", "fetch"]);
    fx.ok(
        &other,
        &["decision", "add", "M-OLD", "Plain", "--by", "Jim"],
    );
    assert_eq!(fx.manifest_version(&other), "10");
    assert!(fx
        .ok(&other, &["decision", "list", "M-OLD", "--porcelain"])
        .lines()
        .all(|l| l.ends_with("\t-")));
}

impl Fixture {
    /// A published Dolt ledger at version 6, as the previous release left it: the
    /// constraints and out of scope of a mission are two text columns of `missions`.
    /// M-OLD holds lines with blanks, padding and a Windows line break, M-BLANK holds
    /// only white space, and M-NULL holds nothing.
    fn version_six_ledger() -> Fixture {
        Fixture::cached("version-six", || {
            (Fixture::build_version_six_ledger(), String::new())
        })
        .0
    }

    fn build_version_six_ledger() -> Fixture {
        let fx = Fixture::new();
        let a = fx.a();
        fx.ok(&a, &["ledger", "fetch"]);
        for (id, name) in [
            ("M-OLD", "Made on six"),
            ("M-BLANK", "Blank text"),
            ("M-NULL", "No text"),
        ] {
            fx.ok(&a, &["mission", "create", id, name]);
        }
        fx.ok(&a, &["mission", "set", "M-OLD", "purpose", "Keep me"]);
        fx.ok(&a, &["mission", "objective", "add", "M-OLD", "An outcome"]);
        fx.dolt_sql(&a, &layout_nine_sql());
        fx.dolt_sql(
            &a,
            &format!(
                "{}; \
                 update missions set \
                   constraints_text = concat('  first', char(10), char(10), ' second  ', char(10), \
                     char(9), char(10), 'third', char(13), char(10)), \
                   out_of_scope = 'only one' where id = 'M-OLD'; \
                 update missions set constraints_text = concat(' ', char(10), ' '), \
                   out_of_scope = '' where id = 'M-BLANK'; \
                 replace into manifest (k, v) values ('version', '6')",
                old_schema()
            ),
        );
        fx.ok(&a, &["ledger", "push", "A version 6 ledger"]);
        fx
    }
}

#[test]
fn a_version_six_ledger_reads_its_text_as_lists_and_the_first_write_moves_it() {
    if !dolt_installed() {
        eprintln!("note: dolt is not installed; skipping");
        return;
    }
    let fx = Fixture::version_six_ledger();
    let (a, other) = (fx.a(), fx.b());
    fx.ok(&other, &["ledger", "fetch"]);
    assert_eq!(fx.manifest_version(&other), "6");

    // Reads split the text by the line rule and write nothing.
    assert_eq!(
        fx.ok(&other, &["mission", "constraint", "list", "M-OLD"]),
        "1\tfirst\n2\tsecond\n3\tthird\n"
    );
    assert_eq!(
        fx.ok(&other, &["mission", "out-of-scope", "list", "M-OLD"]),
        "1\tonly one\n"
    );
    for id in ["M-BLANK", "M-NULL"] {
        assert_eq!(fx.ok(&other, &["mission", "constraint", "list", id]), "");
        assert_eq!(fx.ok(&other, &["mission", "out-of-scope", "list", id]), "");
    }
    let record = mission_json_of(&fx, &other, "M-OLD");
    assert_eq!(
        record["constraints"],
        serde_json::json!([
            {"n": 1, "text": "first"},
            {"n": 2, "text": "second"},
            {"n": 3, "text": "third"}
        ])
    );
    assert_eq!(
        record["out_of_scope"],
        serde_json::json!([{"n": 1, "text": "only one"}])
    );
    // A refused write upgrades nothing.
    assert_exit(
        &fx.tsk(&other, &["mission", "constraint", "add", "M-NONE", "x"]),
        1,
    );
    assert_exit(
        &fx.tsk(&other, &["mission", "constraint", "remove", "M-OLD", "9"]),
        1,
    );
    assert_eq!(fx.manifest_version(&other), "6");
    assert_eq!(
        fx.dolt_sql(&other, "select count(*) as c from dolt_status"),
        "c\n0\n"
    );
    assert!(fx
        .dolt_sql(&other, "show create table missions")
        .contains("constraints_text"));

    // The first write moves the text into the tables, drops the columns and records the
    // version, in one local change with the item it added.
    assert_eq!(
        fx.ok(&other, &["mission", "constraint", "add", "M-OLD", "fourth"]),
        "4\n"
    );
    assert_eq!(fx.manifest_version(&other), "10");
    let tables = fx.tables(&other);
    assert!(
        tables.lines().any(|t| t == "mission_constraints"),
        "{}",
        tables
    );
    assert!(
        tables.lines().any(|t| t == "mission_out_of_scope"),
        "{}",
        tables
    );
    assert_eq!(
        fx.dolt_sql(
            &other,
            "select column_name from information_schema.columns where table_name = 'missions' \
             order by ordinal_position"
        ),
        "COLUMN_NAME\nid\ncode\nname\ncategory\nstatus\nparent\npurpose\ndecision_authority\n\
         next_objective\nnext_task\nnext_question\nnext_constraint\nnext_out_of_scope\n"
    );
    assert_eq!(
        fx.dolt_sql(
            &other,
            "select mission_id, seq, text from mission_constraints order by mission_id, seq"
        ),
        "mission_id,seq,text\nM-OLD,1,first\nM-OLD,2,second\nM-OLD,3,third\nM-OLD,4,fourth\n"
    );
    assert_eq!(
        fx.dolt_sql(
            &other,
            "select mission_id, seq, text from mission_out_of_scope"
        ),
        "mission_id,seq,text\nM-OLD,1,only one\n"
    );
    // The counters follow the last item; a mission with no text keeps 1.
    assert_eq!(
        fx.dolt_sql(
            &other,
            "select id, next_constraint, next_out_of_scope from missions order by id"
        ),
        "id,next_constraint,next_out_of_scope\nM-BLANK,1,1\nM-NULL,1,1\nM-OLD,5,2\n"
    );
    // The rest of the mission is intact.
    let after = mission_json_of(&fx, &other, "M-OLD");
    assert_eq!(after["purpose"], "Keep me");
    assert_eq!(
        after["objective"],
        serde_json::json!([{"n": 1, "outcome": "An outcome", "met": false}])
    );
    assert_eq!(
        after["constraints"][3],
        serde_json::json!({"n": 4, "text": "fourth"})
    );
    // Numbers keep growing, and the other lists work.
    assert_eq!(
        fx.ok(
            &other,
            &["mission", "out-of-scope", "add", "M-OLD", "second out"]
        ),
        "2\n"
    );
    assert_eq!(
        fx.ok(
            &other,
            &["mission", "constraint", "add", "M-NULL", "first for null"]
        ),
        "1\n"
    );
    // The status shows the upgrade and the rows that exist because of it and the
    // writes, not the columns that went.
    let status = fx.ok(&other, &["ledger", "status", "--porcelain"]);
    assert!(
        status.contains("data\tupgrade\tschema upgraded to version 10\n"),
        "{}",
        status
    );
    assert!(!status.contains("manifest"), "{}", status);
    assert!(!status.contains("data\tmissions"), "{}", status);

    // The other clone is still at version 6 until the push.
    assert_eq!(fx.manifest_version(&a), "6");
    fx.ok(&other, &["ledger", "push", "Move the lists"]);
    fx.ok(&a, &["ledger", "fetch"]);
    assert_eq!(fx.manifest_version(&a), "10");
    assert_eq!(
        fx.ok(&a, &["mission", "constraint", "list", "M-OLD"]),
        "1\tfirst\n2\tsecond\n3\tthird\n4\tfourth\n"
    );
    assert_eq!(
        fx.ok(&a, &["mission", "out-of-scope", "list", "M-OLD"]),
        "1\tonly one\n2\tsecond out\n"
    );
}

fn mission_json_of(fx: &Fixture, dir: &Path, id: &str) -> serde_json::Value {
    json(&fx.ok(dir, &["mission", "list", id, "--json"]))
}

#[test]
fn a_thread_write_moves_the_lists_of_a_version_six_ledger_too() {
    if !dolt_installed() {
        eprintln!("note: dolt is not installed; skipping");
        return;
    }
    let fx = Fixture::version_six_ledger();
    let other = fx.b();
    fx.ok(&other, &["ledger", "fetch"]);
    assert_eq!(fx.manifest_version(&other), "6");
    fx.started(&other, &["thread", "start", "M-OLD"]);
    assert_eq!(fx.manifest_version(&other), "10");
    assert_eq!(
        fx.ok(&other, &["mission", "constraint", "list", "M-OLD"]),
        "1\tfirst\n2\tsecond\n3\tthird\n"
    );
    assert_eq!(
        fx.dolt_sql(
            &other,
            "select next_constraint, next_out_of_scope from missions where id = 'M-OLD'"
        ),
        "next_constraint,next_out_of_scope\n4,2\n"
    );
}

// --- Bindings of every actor in one table, and the version 8 ledger ---

impl Fixture {
    /// A published Dolt ledger at version 7, as the previous release left it: the bindings
    /// are rows of `cloud_session_bindings`, and the one thread is bound by `cse_old`.
    /// Returns the thread ID.
    fn version_seven_ledger() -> (Fixture, String) {
        Fixture::cached("version-seven", Fixture::build_version_seven_ledger)
    }

    fn build_version_seven_ledger() -> (Fixture, String) {
        let fx = Fixture::build_with_briefing();
        let a = fx.a();
        fx.ok(&a, &["ledger", "fetch"]);
        let start = fx.tsk_cloud(&a, "cse_old", &["thread", "start", "M-CONF-01"]);
        assert_success(&start);
        let id = stdout(&start)
            .trim_end()
            .strip_prefix("started:")
            .unwrap()
            .to_string();
        let sql = format!(
            "{}; \
             insert into cloud_session_bindings (session_id, thread_id, registered_at) \
               values ('cse_old', '{}', '2026-01-01T00:00:00Z'), ('cse_gone', 'zzzzzzzz', '2026-01-02T00:00:00Z'); \
             replace into manifest (k, v) values ('version', '7')",
            OLD_BINDINGS.join("; "),
            id
        );
        fx.dolt_sql(&a, &layout_nine_sql());
        fx.dolt_sql(&a, &sql);
        fx.ok(&a, &["ledger", "push", "A version 7 ledger"]);
        (fx, id)
    }
}

#[test]
fn a_version_seven_ledger_reads_its_bindings_from_the_old_table_and_the_first_write_upgrades_it() {
    if !dolt_installed() {
        eprintln!("note: dolt is not installed; skipping");
        return;
    }
    let (fx, id) = Fixture::version_seven_ledger();
    let other = fx.b();
    fx.ok(&other, &["ledger", "fetch"]);
    assert_eq!(fx.manifest_version(&other), "7");

    // Reads use the old table and upgrade nothing, offline or not.
    let clean = || fx.dolt_sql(&other, "select count(*) as c from dolt_status");
    assert_eq!(
        stdout(&fx.tsk_cloud(&other, "cse_old", &["thread", "binding"])),
        format!("cloud:{}\n", id)
    );
    assert_eq!(
        stdout(&fx.tsk_cloud(&other, "cse_old", &["thread", "binding", "--no-fetch"])),
        format!("cloud:{}\n", id)
    );
    assert_exit(&fx.tsk_cloud(&other, "cse_new", &["thread", "binding"]), 1);
    assert_eq!(fx.guard_reason(&other, Some("cse_old")), None);
    assert!(fx.guard_reason(&other, Some("cse_new")).is_some());
    let row = fx.thread_row(&other, &id);
    assert_eq!(
        row["actors"],
        serde_json::json!([{"actor": "cse_old", "kind": "cloud", "name": null}])
    );
    assert_eq!(fx.manifest_version(&other), "7");
    assert_eq!(clean(), "c\n0\n");
    assert!(fx
        .tables(&other)
        .lines()
        .any(|t| t == "cloud_session_bindings"));
    assert!(!fx.tables(&other).lines().any(|t| t == "bindings"));

    // A write upgrades: this bind creates the table, copies the rows as cloud rows, drops
    // the old table and records version 9, and the push after it carries it.
    assert_success(&fx.tsk_cloud(&other, "cse_new", &["thread", "resume", &id]));
    assert_eq!(fx.manifest_version(&other), "10");
    // The bind is local. The next push carries the upgrade.
    fx.ok(&other, &["ledger", "push", "Upgrade to version 9"]);
    let tables = fx.tables(&other);
    assert!(tables.lines().any(|t| t == "bindings"), "{}", tables);
    assert!(
        !tables.lines().any(|t| t == "cloud_session_bindings"),
        "{}",
        tables
    );
    assert_eq!(
        fx.dolt_sql(
            &other,
            "select actor, kind, thread_id from bindings order by actor"
        ),
        format!(
            "actor,kind,thread_id\ncse_gone,cloud,zzzzzzzz\ncse_new,cloud,{id}\ncse_old,cloud,{id}\n",
            id = id
        )
    );
    assert_eq!(
        fx.dolt_sql(
            &other,
            "select column_name, column_type from information_schema.columns \
             where table_name = 'bindings' order by ordinal_position"
        ),
        BINDINGS_COLUMNS
    );
    // The original clone picks the upgrade up from origin.
    let a = fx.a();
    fx.ok(&a, &["ledger", "fetch"]);
    assert_eq!(fx.manifest_version(&a), "10");
    assert_eq!(
        stdout(&fx.tsk_cloud(&a, "cse_old", &["thread", "binding"])),
        format!("cloud:{}\n", id)
    );
}

/// The columns of `bindings`, as `information_schema.columns` lists them. Version 9 leaves
/// the table as version 8 made it.
const BINDINGS_COLUMNS: &str =
    "COLUMN_NAME,COLUMN_TYPE\nactor,varchar(255)\nkind,\"enum('cloud','worktree')\"\n\
     thread_id,varchar(64)\nregistered_at,varchar(32)\n";

// --- Actors and harness instances, and the version 9 ledger ---

impl Fixture {
    /// A published Dolt ledger at version 8, as the previous release left it: no `actors`
    /// and no `instances` table, and the one thread is bound by `cse_old`. Returns the
    /// thread ID.
    fn version_eight_ledger() -> (Fixture, String) {
        Fixture::cached("version-eight", Fixture::build_version_eight_ledger)
    }

    fn build_version_eight_ledger() -> (Fixture, String) {
        let fx = Fixture::build_with_briefing();
        let a = fx.a();
        fx.ok(&a, &["ledger", "fetch"]);
        let start = fx.tsk_cloud(&a, "cse_old", &["thread", "start", "M-CONF-01"]);
        assert_success(&start);
        let id = stdout(&start)
            .trim_end()
            .strip_prefix("started:")
            .unwrap()
            .to_string();
        fx.dolt_sql(&a, &layout_nine_sql());
        fx.dolt_sql(
            &a,
            "drop table actors; drop table instances; \
             insert into bindings (actor, kind, thread_id, registered_at) \
               values ('cse_gone', 'cloud', 'zzzzzzzz', '2026-01-02T00:00:00Z'); \
             replace into manifest (k, v) values ('version', '8')",
        );
        fx.ok(&a, &["ledger", "push", "A version 8 ledger"]);
        (fx, id)
    }
}

#[test]
fn a_version_eight_ledger_reads_without_actor_tables_and_the_first_registration_adds_them() {
    if !dolt_installed() {
        eprintln!("note: dolt is not installed; skipping");
        return;
    }
    let (fx, id) = Fixture::version_eight_ledger();
    let other = fx.b();
    fx.ok(&other, &["ledger", "fetch"]);
    assert_eq!(fx.manifest_version(&other), "8");
    let tables = fx.tables(&other);
    assert!(!tables.lines().any(|t| t == "actors"), "{}", tables);
    assert!(!tables.lines().any(|t| t == "instances"), "{}", tables);

    // Reads see no actor, and upgrade nothing.
    let clean = || fx.dolt_sql(&other, "select count(*) as c from dolt_status");
    assert_eq!(
        stdout(&fx.tsk_cloud(&other, "cse_old", &["thread", "binding"])),
        format!("cloud:{}\n", id)
    );
    assert_eq!(
        fx.thread_row(&other, &id)["actors"],
        serde_json::json!([{"actor": "cse_old", "kind": "cloud", "name": null}])
    );
    assert_eq!(
        stdout(&fx.tsk_cloud(&other, "cse_old", &["actor", "name"])),
        "no actor registered\n"
    );
    assert_eq!(fx.ok(&other, &["actor", "list"]), "");
    assert_eq!(fx.ok(&other, &["actor", "list", "--json"]), "[]\n");
    assert_eq!(fx.guard_reason(&other, Some("cse_old")), None);
    assert_eq!(fx.manifest_version(&other), "8");
    assert_eq!(clean(), "c\n0\n");

    // The first registration upgrades: both tables are made, every binding row is kept,
    // and version 9 is recorded in the same local change.
    let registered = fx.tsk_cloud(
        &other,
        "cse_old",
        &["actor", "register", "alice", "--kind", "robot"],
    );
    assert_success(&registered);
    assert_eq!(stdout(&registered), "registered alice (robot)\n");
    assert_eq!(fx.manifest_version(&other), "10");
    let tables = fx.tables(&other);
    assert!(tables.lines().any(|t| t == "actors"), "{}", tables);
    assert!(tables.lines().any(|t| t == "instances"), "{}", tables);
    assert_eq!(
        fx.dolt_sql(
            &other,
            "select column_name, column_type from information_schema.columns \
             where table_name = 'bindings' order by ordinal_position"
        ),
        BINDINGS_COLUMNS
    );
    assert_eq!(
        fx.dolt_sql(
            &other,
            "select actor, kind, thread_id from bindings order by actor"
        ),
        format!(
            "actor,kind,thread_id\ncse_gone,cloud,zzzzzzzz\ncse_old,cloud,{}\n",
            id
        )
    );
    assert_eq!(
        fx.dolt_sql(&other, "select name, kind from actors"),
        "name,kind\nalice,robot\n"
    );
    assert_eq!(
        fx.dolt_sql(&other, "select instance, kind, actor_name from instances"),
        "instance,kind,actor_name\ncse_old,cloud,alice\n"
    );
    let status = fx.ok(&other, &["ledger", "status", "--local", "--porcelain"]);
    assert!(
        status.contains("data\tupgrade\tschema upgraded to version 10\n"),
        "{}",
        status
    );
    // The registration is local until the next push, which carries the upgrade too.
    fx.ok(&other, &["ledger", "push", "Upgrade to version 9"]);
    let a = fx.a();
    fx.ok(&a, &["ledger", "fetch"]);
    assert_eq!(fx.manifest_version(&a), "10");
    assert_eq!(
        stdout(&fx.tsk_cloud(&a, "cse_old", &["thread", "binding"])),
        format!("cloud:{}\nactor: alice (robot)\n", id)
    );
    assert_eq!(
        stdout(&fx.tsk_cloud(&a, "cse_gone", &["actor", "name"])),
        "no actor registered\n"
    );
    assert_eq!(
        fx.ok(&a, &["actor", "list", "--porcelain"]),
        "alice\trobot\tcse_old\n"
    );
}

#[test]
fn a_marker_file_on_a_version_seven_ledger_is_migrated_by_a_write_that_upgrades_it() {
    if !dolt_installed() {
        eprintln!("note: dolt is not installed; skipping");
        return;
    }
    let (fx, id) = Fixture::version_seven_ledger();
    let other = fx.b();
    fx.ok(&other, &["ledger", "fetch"]);
    write(&fx.marker(&other), &format!("{}\n", id));

    // Without a fetch the marker is read and left, and the ledger is not written.
    assert_eq!(
        fx.ok(&other, &["thread", "binding", "--no-fetch"]),
        format!("worktree:{}\n", id)
    );
    assert!(fx.marker(&other).exists());
    assert_eq!(fx.manifest_version(&other), "7");

    // The row is written, the ledger is upgraded in the same local change, the file goes.
    assert_eq!(
        fx.ok(&other, &["thread", "binding"]),
        format!("worktree:{}\n", id)
    );
    assert!(!fx.marker(&other).exists());
    assert_eq!(fx.manifest_version(&other), "10");
    let actor = fx.main_actor(&other);
    assert_eq!(
        fx.dolt_sql(
            &other,
            "select actor, kind, thread_id from bindings order by actor"
        ),
        format!(
            "actor,kind,thread_id\ncse_gone,cloud,zzzzzzzz\ncse_old,cloud,{id}\n{actor},worktree,{id}\n",
            id = id,
            actor = actor
        )
    );
    // The change is local: origin still holds version 7.
    let status = fx.ok(&other, &["ledger", "status", "--porcelain"]);
    assert!(
        status.contains("data\tupgrade\tschema upgraded to version 10\n"),
        "{}",
        status
    );
    // The rows the upgrade copied are not shown as changes: only the worktree row is.
    assert!(
        status.contains("data\tbindings\tadded 1\tmodified 0\tdeleted 0\n"),
        "{}",
        status
    );
    assert!(!status.contains("cloud_session_bindings"), "{}", status);
    fx.ok(&other, &["ledger", "push", "Carry the binding"]);
    let a = fx.a();
    fx.ok(&a, &["ledger", "fetch"]);
    assert_eq!(fx.manifest_version(&a), "10");
    assert_eq!(fx.actors_of(&a, &id).len(), 2);
}

#[test]
fn the_stop_hook_check_reads_the_local_ledger_store_and_works_offline() {
    if !dolt_installed() {
        eprintln!("note: dolt is not installed; skipping");
        return;
    }
    let fx = Fixture::with_briefing();
    let a = fx.a();
    // Unbound: the check blocks. Bound by the worktree row: it passes.
    assert!(fx.guard_reason(&a, None).is_some());
    let id = fx.started(&a, &["thread", "start", "M-CONF-01"]);
    assert!(!fx.marker(&a).exists());
    let start = fx.tsk_cloud(&a, "cse_off", &["thread", "resume", &id]);
    assert_success(&start);
    // The bind is local. The check would ask for a push, so publish it first.
    fx.ok(&a, &["ledger", "push", "Publish the binding"]);

    // The origin is gone: the check never fetches, so it answers from the local store.
    let moved = fx.root.path().join("origin-away.git");
    std::fs::rename(fx.origin(), &moved).unwrap();
    assert_eq!(fx.guard_reason(&a, None), None);
    assert_eq!(fx.guard_reason(&a, Some("cse_off")), None);
    assert!(fx.guard_reason(&a, Some("cse_other")).is_some());
    assert_eq!(
        fx.ok(&a, &["thread", "binding", "--no-fetch"]),
        format!("worktree:{}\n", id)
    );
    // A detach is a write that needs the origin, but the local store answers the check
    // once the row is gone: nothing else in the store binds this worktree.
    std::fs::rename(&moved, fx.origin()).unwrap();
    fx.ok(&a, &["thread", "detach"]);
    std::fs::rename(fx.origin(), &moved).unwrap();
    assert!(fx.guard_reason(&a, None).is_some());
}

// --- A ledger held in a nexus repo ---

const NEXUS_REF: &str = "refs/heads/ledgers/work-api";
const NEXUS_DOCS_REF: &str = "refs/heads/ledgers/work-api-docs";

impl Fixture {
    /// A bare nexus repo whose `nexus.json` holds `work-api` as a nexus-held ledger local
    /// to this machine, attached in the user config. Both clones carry the cached repo ID,
    /// so the entry is found without matching the origin URL.
    fn attach_nexus(&self) -> PathBuf {
        let root = self.root.path();
        git(
            root,
            &["init", "--quiet", "--bare", "-b", "main", "nexus.git"],
        );
        git(root, &["init", "--quiet", "-b", "main", "nexus-seed"]);
        let seed = root.join("nexus-seed");
        let nexus = root.join("nexus.git");
        git(&seed, &["remote", "add", "origin", nexus.to_str().unwrap()]);
        write(
            &seed.join("nexus.json"),
            "{\"version\":1,\"territories\":[{\"id\":\"t\",\"name\":\"T\",\"repos\":[\
             {\"id\":\"work-api\",\"local\":\"laptop\",\"ledger\":\"nexus\"}]}]}\n",
        );
        git(&seed, &["add", "-A"]);
        git(&seed, &["commit", "--quiet", "-m", "Set nexus.json"]);
        git(
            &seed,
            &["push", "--quiet", "origin", "HEAD:refs/heads/main"],
        );
        self.ok(
            &self.a(),
            &["config", "attach-nexus", nexus.to_str().unwrap()],
        );
        for clone in [self.a(), self.b()] {
            write(&clone.join(".git").join("tsk-repo-id"), "work-api\n");
        }
        nexus
    }
}

fn ref_exists(repo: &Path, reference: &str) -> bool {
    Command::new("git")
        .args(["rev-parse", "--verify", "--quiet", reference])
        .current_dir(repo)
        .output()
        .unwrap()
        .status
        .success()
}

#[test]
fn a_ledger_held_in_a_nexus_repo_round_trips_between_clones() {
    if !dolt_installed() {
        eprintln!("note: dolt is not installed; skipping");
        return;
    }
    let fx = Fixture::new();
    let nexus = fx.attach_nexus();
    let (a, other) = (fx.a(), fx.b());

    // A new ledger names the nexus ref, not the managed repo's.
    let fetch = fx.tsk(&a, &["ledger", "fetch"]);
    assert_success(&fetch);
    assert!(
        stderr(&fetch).contains("the nexus's refs/heads/ledgers/work-api does not exist yet"),
        "{}",
        stderr(&fetch)
    );
    assert!(stdout(&fetch).trim_end().ends_with("/ledger-db"));

    // Records, a document, a thread and events written in clone A.
    fx.put(&a, BRIEFING, "# Mission: conformance\n");
    fx.ok(&a, &["mission", "create", "M-CONF-01", "Conformance"]);
    fx.ok(&a, &["task", "add", "M-CONF-01", "Do the thing"]);
    let id = fx.started(&a, &["thread", "start", "M-CONF-01"]);
    fx.ok(&a, &["thread", "pause", &id, "T-01", "Carry on"]);
    let payload = fx.root.path().join("payload.json");
    write(&payload, "{\"id\": 1}");
    fx.ok(
        &a,
        &[
            "events",
            "append",
            "github",
            "dependabot_alert",
            "created",
            "acme/work-api",
            payload.to_str().unwrap(),
        ],
    );
    let push = fx.ok(&a, &["ledger", "push", "First commit"]);
    assert!(!push.trim().is_empty());

    // The ledger and its documents are on the nexus, and nothing is on the managed repo.
    assert!(ref_exists(&nexus, NEXUS_REF));
    assert!(ref_exists(&nexus, NEXUS_DOCS_REF));
    assert!(!ref_exists(&nexus, "refs/heads/ledgers/work-api/docs"));
    assert_eq!(fx.remote_ledger(), None);
    assert_eq!(fx.remote_docs(), None);
    assert_eq!(fx.remote_branches(), vec!["refs/heads/main".to_string()]);
    let docs = git(&nexus, &["ls-tree", "-r", "--name-only", NEXUS_DOCS_REF]);
    assert!(docs.lines().any(|path| path == BRIEFING), "{}", docs);
    assert!(
        !docs.lines().any(|path| path == ".tsk-ledger.toml"),
        "{}",
        docs
    );
    let tree = git(&nexus, &["ls-tree", "--name-only", NEXUS_REF]);
    assert!(tree.lines().any(|path| path == "manifest"), "{}", tree);
    assert_eq!(
        std::fs::read_to_string(a.join(".git").join("tsk-repo-id"))
            .unwrap()
            .trim(),
        "work-api"
    );

    // Clone B reads all of it from the nexus.
    let fetch = fx.tsk(&other, &["ledger", "fetch"]);
    assert_success(&fetch);
    assert!(
        !stderr(&fetch).contains("does not exist"),
        "{}",
        stderr(&fetch)
    );
    assert_eq!(
        fx.ok(&other, &["ledger", "doc", "get", BRIEFING]),
        "# Mission: conformance\n"
    );
    assert!(fx
        .ok(&other, &["mission", "list", "--porcelain"])
        .starts_with("M-CONF-01\t"));
    assert_eq!(
        fx.ok(&other, &["task", "list", "M-CONF-01", "--porcelain"])
            .lines()
            .count(),
        1
    );
    let resumed = json(&fx.ok(&other, &["thread", "resume", &id]));
    assert_eq!(resumed["latest"]["task_id"], "T-01");
    assert_eq!(resumed["latest"]["whats_next"], "Carry on");
    // The bind is local, and a read of the event queue refuses local changes.
    fx.ok(&other, &["ledger", "push", "Bind B"]);
    assert_eq!(fx.ok(&other, &["events", "read-new"]).lines().count(), 1);

    // A write in B reaches A through the nexus too, and a stale push merges.
    fx.ok(&other, &["task", "add", "M-CONF-01", "Second"]);
    fx.ok(&a, &["mission", "create", "M-CONF-02", "Other"]);
    fx.ok(&other, &["ledger", "push", "From B"]);
    fx.ok(&a, &["ledger", "push", "From A"]);
    fx.ok(&other, &["ledger", "fetch"]);
    assert_eq!(
        fx.ok(&other, &["task", "list", "M-CONF-01", "--porcelain"])
            .lines()
            .count(),
        2
    );
    assert!(fx
        .ok(&other, &["mission", "list", "--porcelain"])
        .contains("M-CONF-02"));
    assert_eq!(fx.remote_ledger(), None);

    // A manifest that names another repo stops the push, and the nexus is unchanged.
    let before = git(&nexus, &["rev-parse", NEXUS_REF]);
    fx.dolt_sql(
        &a,
        "replace into manifest (k, v) values ('repo_id', 'someone-else')",
    );
    let output = fx.tsk(&a, &["ledger", "push", "Wrong repo id"]);
    assert_exit(&output, 1);
    assert!(
        stderr(&output).contains("repo_id \"someone-else\""),
        "{}",
        stderr(&output)
    );
    assert_eq!(git(&nexus, &["rev-parse", NEXUS_REF]), before);
}

// --- A clone that pushes between another command's refresh and its push ---

/// The path of an installed program, found on `PATH` before it is changed.
fn installed_program(name: &str) -> PathBuf {
    std::env::var_os("PATH")
        .and_then(|path| {
            std::env::split_paths(&path)
                .map(|dir| dir.join(name))
                .find(|candidate| candidate.is_file())
        })
        .unwrap_or_else(|| panic!("{} is not on PATH", name))
}

/// A directory with a `git` stand-in. Before the first `git push` of a command that has
/// `TSK_RACE` set, it runs that shell command once (`TSK_RACE_DONE` marks that it ran), then
/// runs the real `git`. The first push of a command is the push of the documents, after its
/// refresh and before the push of the data, so the shell command acts between the refresh
/// and the pushes.
fn race_shim(root: &Path) -> PathBuf {
    let dir = root.join("race-shim");
    std::fs::create_dir_all(&dir).unwrap();
    let script = format!(
        "#!/bin/sh\n\
         if [ \"$1\" = push ] && [ -n \"$TSK_RACE\" ] && [ ! -e \"$TSK_RACE_DONE\" ]; then\n\
         \x20 : > \"$TSK_RACE_DONE\"\n\
         \x20 sh -c \"$TSK_RACE\" >&2 || exit 97\n\
         fi\n\
         exec '{}' \"$@\"\n",
        installed_program("git").display()
    );
    let shim = dir.join("git");
    std::fs::write(&shim, script).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&shim, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    dir
}

fn path_with_first(dir: &Path) -> std::ffi::OsString {
    let mut dirs = vec![dir.to_path_buf()];
    dirs.extend(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    ));
    std::env::join_paths(dirs).unwrap()
}

#[test]
fn a_clone_that_pushes_between_the_refresh_and_the_push_is_merged_not_overwritten() {
    if !dolt_installed() {
        eprintln!("note: dolt is not installed; skipping");
        return;
    }
    // A stop asks origin first, so the rejections are said aloud.
    a_race_between_the_refresh_and_the_push(&["thread", "stop"], true, true);
}

#[test]
fn a_start_that_went_on_the_record_of_the_last_fetch_is_merged_and_says_nothing() {
    if !dolt_installed() {
        eprintln!("note: dolt is not installed; skipping");
        return;
    }
    // A start does not ask origin first, so its first rejection only shows the record was old.
    a_race_between_the_refresh_and_the_push(&["thread", "start"], false, false);
}

/// A thread command of A runs while B writes a document and a mission and pushes both, after
/// A's command has refreshed and before it pushes. `bound` starts a thread on A first, for a
/// command that needs one. `says_so` is whether the command asked origin before it wrote, so
/// that the rejections of its first push are said aloud.
fn a_race_between_the_refresh_and_the_push(args: &[&str], bound: bool, says_so: bool) {
    let fx = Fixture::fetched();
    let (a, b) = (fx.a(), fx.b());
    fx.put(&a, "base.md", "base\n");
    fx.ok(&a, &["ledger", "push", "Base"]);
    if bound {
        fx.ok(&a, &["thread", "start"]);
    }
    fx.ok(&b, &["ledger", "fetch"]);
    // A holds a document of its own that its thread command pushes with its data.
    fx.put(&a, "from-a.md", "a\n");

    // B writes a document and a mission and pushes both, after A's command has refreshed
    // and before it pushes.
    let tsk = env!("CARGO_BIN_EXE_tsk");
    let race = format!(
        "cd '{b}' && printf 'from b\\n' | '{tsk}' ledger doc put from-b.md && \
         '{tsk}' mission create M-RACE Race && '{tsk}' ledger push 'B races'",
        b = b.display(),
        tsk = tsk
    );
    let done = fx.root.path().join("race-done");
    let output = fx
        .command(&a, args)
        .env("PATH", path_with_first(&race_shim(fx.root.path())))
        .env("TSK_RACE", race)
        .env("TSK_RACE_DONE", &done)
        .output()
        .unwrap();
    assert_success(&output);
    assert!(done.exists(), "B did not push in between");
    // Neither push went over the other. Both sides were rejected once, then merged. A command
    // that went on the record of the last fetch found its record old, and does not say so.
    let notes = stderr(&output);
    if says_so {
        assert!(
            notes.contains("[rejected] (stale info)"),
            "documents: {}",
            notes
        );
        assert!(
            notes.contains("[rejected] (fetch first)")
                || notes.contains("[rejected] (non-fast-forward)"),
            "data: {}",
            notes
        );
    } else {
        assert!(!notes.contains("[rejected]"), "{}", notes);
    }

    let files = fx.origin_files(DOCS_REF);
    for expected in ["from-a.md", "from-b.md"] {
        assert!(files.iter().any(|f| f == expected), "{:?}", files);
    }
    // A's local store holds B's mission, and B sees A's thread.
    assert!(fx
        .ok(&a, &["mission", "list", "--porcelain"])
        .contains("M-RACE"));
    fx.ok(&b, &["ledger", "fetch"]);
    // A stop deleted the thread. A start made one.
    let threads = if args[1] == "stop" { 0 } else { 1 };
    assert_eq!(fx.ok(&b, &["thread", "list"]).lines().count(), threads);
    assert!(fx
        .ok(&b, &["mission", "list", "--porcelain"])
        .contains("M-RACE"));
    // Nothing is left unpushed on A.
    let status = fx.ok(&a, &["ledger", "status"]);
    assert!(status.contains("nothing to push"), "{}", status);
    assert!(status.contains("up to date"), "{}", status);
}

// --- The up-to-date check: one ls-remote, and a fetch only when origin has something new ---

/// A directory with a `git` stand-in that logs its arguments, one line per run, to the file
/// named by `TSK_GIT_LOG`, then runs the real `git`. The processes that `dolt` starts find
/// it on `PATH` too.
fn git_log_shim(root: &Path) -> PathBuf {
    let dir = root.join("git-log-shim");
    std::fs::create_dir_all(&dir).unwrap();
    let script = format!(
        "#!/bin/sh\necho \"$@\" >> \"$TSK_GIT_LOG\"\nexec '{}' \"$@\"\n",
        installed_program("git").display()
    );
    let shim = dir.join("git");
    std::fs::write(&shim, script).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&shim, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    dir
}

/// The network operations a command started: the `git` runs whose verb is `ls-remote`,
/// `fetch` or `push`, counting those that `dolt` started.
#[derive(Debug, PartialEq, Eq)]
struct NetOps {
    /// The runs of `tsk` asking for both ledger refs at once.
    asks: usize,
    /// Every `ls-remote`, which includes those `dolt` starts inside a fetch or a push.
    ls_remote: usize,
    fetch: usize,
    push: usize,
}

fn net_ops(log: &str) -> NetOps {
    let count = |verb: &str| {
        log.lines()
            .filter(|line| line.split_whitespace().any(|word| word == verb))
            .count()
    };
    let asks = log
        .lines()
        .filter(|line| {
            line.split_whitespace().any(|word| word == "ls-remote")
                && line.contains(LEDGER_REF)
                && line.contains(DOCS_REF)
        })
        .count();
    NetOps {
        asks,
        ls_remote: count("ls-remote"),
        fetch: count("fetch"),
        push: count("push"),
    }
}

impl Fixture {
    /// Run `tsk` with the logging `git` stand-in and return its output and the network
    /// operations it started.
    fn counted(&self, dir: &Path, args: &[&str]) -> (Output, NetOps, String) {
        let log = self.root.path().join("git-log.txt");
        let _ = std::fs::remove_file(&log);
        let output = self
            .command(dir, args)
            .env("PATH", path_with_first(&git_log_shim(self.root.path())))
            .env("TSK_GIT_LOG", &log)
            .output()
            .unwrap();
        let text = std::fs::read_to_string(&log).unwrap_or_default();
        (output, net_ops(&text), text)
    }

    fn counted_ok(&self, dir: &Path, args: &[&str]) -> (String, NetOps) {
        let (output, ops, log) = self.counted(dir, args);
        assert_success(&output);
        if std::env::var_os("TSK_TEST_SHOW_GIT_LOG").is_some() {
            eprintln!("--- {:?}\n{}", args, log);
        }
        (stdout(&output), ops)
    }
}

const NO_NET: NetOps = NetOps {
    asks: 0,
    ls_remote: 0,
    fetch: 0,
    push: 0,
};

const NOTHING_NEW: NetOps = NetOps {
    asks: 1,
    ls_remote: 1,
    fetch: 0,
    push: 0,
};

#[test]
fn a_refresh_with_nothing_new_asks_origin_once_and_fetches_nothing() {
    if !dolt_installed() {
        eprintln!("note: dolt is not installed; skipping");
        return;
    }
    let fx = Fixture::with_briefing();
    let (a, b) = (fx.a(), fx.b());
    fx.ok(&b, &["ledger", "fetch"]);

    // The first fetch of a clone brings the data. The second finds nothing new.
    let (output, ops, log) = fx.counted(&b, &["ledger", "fetch"]);
    assert_success(&output);
    assert_eq!(ops, NOTHING_NEW);
    // One question names both refs, so one connection answers for the data and the documents.
    assert!(
        log.contains(&format!("ls-remote origin {} {}", LEDGER_REF, DOCS_REF)),
        "{}",
        log
    );
    let (_, ops) = fx.counted_ok(&b, &["ledger", "status"]);
    assert_eq!(ops, NOTHING_NEW);
    let (_, ops) = fx.counted_ok(&b, &["thread", "list"]);
    assert_eq!(ops, NOTHING_NEW);
    // A push does not ask origin first: the record of the last fetch stands in for it.
    let (_, ops) = fx.counted_ok(&b, &["ledger", "push", "Nothing"]);
    assert_eq!(ops, NO_NET);
    let (output, ops, _) = fx.counted(&b, &["thread", "session-start", "--format", "claude"]);
    assert_success(&output);
    assert_eq!(ops, NOTHING_NEW);

    // The same holds on the clone that pushed: its own push is recorded as fetched.
    fx.put(&a, "late.md", "late\n");
    fx.add_binding(&a, "late");
    let (_, ops) = fx.counted_ok(&a, &["ledger", "push", "A pushes"]);
    assert_eq!(ops.asks, 0);
    assert_eq!(ops.ls_remote, 0);
    assert!(ops.push >= 2, "{:?}", ops);
    let (_, ops) = fx.counted_ok(&a, &["ledger", "fetch"]);
    assert_eq!(ops, NOTHING_NEW);
    let (_, ops) = fx.counted_ok(&a, &["ledger", "status"]);
    assert_eq!(ops, NOTHING_NEW);

    // A command that does not read origin starts no network operation.
    let (_, ops) = fx.counted_ok(&a, &["mission", "list"]);
    assert_eq!(
        ops,
        NetOps {
            asks: 0,
            ls_remote: 0,
            fetch: 0,
            push: 0
        }
    );
}

#[test]
fn a_change_on_origin_is_fetched_once_and_then_the_check_is_quiet_again() {
    if !dolt_installed() {
        eprintln!("note: dolt is not installed; skipping");
        return;
    }
    let fx = Fixture::with_briefing();
    let (a, b) = (fx.a(), fx.b());
    fx.ok(&b, &["ledger", "fetch"]);

    // A changes the data and the documents.
    fx.put(&a, "late.md", "late\n");
    fx.add_binding(&a, "late");
    fx.ok(&a, &["ledger", "push", "A pushes both"]);

    let (_, ops) = fx.counted_ok(&b, &["ledger", "status"]);
    assert_eq!(ops.asks, 1);
    assert!(ops.fetch >= 2, "both refs are fetched: {:?}", ops);
    // The status fetched the remote refs and nothing else: B still has to merge.
    let (_, ops) = fx.counted_ok(&b, &["ledger", "fetch"]);
    assert_eq!(ops, NOTHING_NEW);
    assert_eq!(fx.binding_count(&b), "1");
    assert_eq!(fx.ok(&b, &["ledger", "doc", "get", "late.md"]), "late\n");

    // Only the documents change: the data is not fetched.
    fx.put(&a, "later.md", "later\n");
    fx.ok(&a, &["ledger", "push", "A pushes a document"]);
    let (_, ops) = fx.counted_ok(&b, &["ledger", "fetch"]);
    assert_eq!(ops.asks, 1);
    assert_eq!(ops.fetch, 1, "only the documents moved: {:?}", ops);
    assert_eq!(fx.ok(&b, &["ledger", "doc", "get", "later.md"]), "later\n");
    let (_, ops) = fx.counted_ok(&b, &["ledger", "fetch"]);
    assert_eq!(ops, NOTHING_NEW);
}

#[test]
fn status_local_asks_origin_nothing_and_says_it_did_not_check() {
    if !dolt_installed() {
        eprintln!("note: dolt is not installed; skipping");
        return;
    }
    let fx = Fixture::with_briefing();
    let (a, b) = (fx.a(), fx.b());
    fx.ok(&b, &["ledger", "fetch"]);

    // Clean, then with a data change and a document change that are not pushed.
    let (text, ops) = fx.counted_ok(&b, &["ledger", "status", "--local"]);
    assert_eq!(
        ops,
        NetOps {
            asks: 0,
            ls_remote: 0,
            fetch: 0,
            push: 0
        }
    );
    assert_eq!(
        no_server_line(text),
        "Local   nothing to push\nOrigin  not checked (--local)\n"
    );

    fx.add_binding(&b, "late");
    fx.put(&b, "note.md", "n\n");
    let (text, ops) = fx.counted_ok(&b, &["ledger", "status", "--local"]);
    let text = no_server_line(text);
    assert_eq!(ops.asks + ops.ls_remote + ops.fetch + ops.push, 0);
    assert!(
        text.starts_with("Local   2 changes not pushed\n"),
        "{}",
        text
    );
    assert!(
        text.ends_with("Origin  not checked (--local)\n"),
        "{}",
        text
    );
    let (porcelain, _) = fx.counted_ok(&b, &["ledger", "status", "--local", "--porcelain"]);
    assert!(
        porcelain.ends_with("data\tremote\tnot checked\ndocs\tremote\tnot checked\n"),
        "{}",
        porcelain
    );

    // Commits not pushed are counted from what was last fetched, and origin moving is
    // not noticed: that is what the flag means.
    fx.ok(&b, &["ledger", "push", "B pushes"]);
    fx.put(&a, "from-a.md", "a\n");
    fx.ok(&a, &["ledger", "push", "A pushes"]);
    let text = no_server_line(fx.ok(&b, &["ledger", "status", "--local"]));
    assert_eq!(
        text,
        "Local   nothing to push\nOrigin  not checked (--local)\n"
    );
    // The plain status does ask, and sees A's push.
    let text = fx.ok(&b, &["ledger", "status"]);
    assert!(text.contains("to fetch"), "{}", text);
}

// --- The dolt processes of a thread write ---

/// A directory with a `dolt` stand-in that logs its arguments, one line per run, to the file
/// named by `TSK_DOLT_LOG`, then runs the real `dolt`.
fn dolt_log_shim(root: &Path) -> PathBuf {
    let dir = root.join("dolt-log-shim");
    std::fs::create_dir_all(&dir).unwrap();
    let script = format!(
        "#!/bin/sh\necho \"$@\" >> \"$TSK_DOLT_LOG\"\nexec '{}' \"$@\"\n",
        installed_program("dolt").display()
    );
    let shim = dir.join("dolt");
    std::fs::write(&shim, script).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&shim, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    dir
}

impl Fixture {
    /// Run `tsk` and count the `dolt` processes it started.
    fn dolt_launches(&self, dir: &Path, args: &[&str]) -> (Output, usize) {
        let log = self.root.path().join("dolt-log.txt");
        let _ = std::fs::remove_file(&log);
        let output = self
            .command(dir, args)
            .env("PATH", path_with_first(&dolt_log_shim(self.root.path())))
            .env("TSK_DOLT_LOG", &log)
            .output()
            .unwrap();
        let launches = std::fs::read_to_string(&log)
            .unwrap_or_default()
            .lines()
            .count();
        (output, launches)
    }
}

#[test]
fn a_thread_write_launches_a_handful_of_dolt_processes() {
    if !dolt_installed() {
        eprintln!("note: dolt is not installed; skipping");
        return;
    }
    let fx = Fixture::fetched();
    let a = fx.a();
    fx.ok(&a, &["mission", "create", "M-LAUNCH", "Launch"]);
    fx.ok(&a, &["task", "add", "M-LAUNCH", "A task"]);
    fx.ok(&a, &["ledger", "push", "Mission"]);
    // The first command of a machine sets the Dolt identity, which costs a process. Run
    // one before counting.
    fx.ok(&a, &["ledger", "fetch"]);

    // Each count is the refresh in one process, the writes, the commit and its reads in
    // one process, the checks before the push in one, and the push. Before the reads were
    // batched these took 18 or more.
    let (output, launches) = fx.dolt_launches(&a, &["thread", "start", "M-LAUNCH"]);
    assert_success(&output);
    assert!(
        launches <= 6,
        "thread start launched {} dolt processes",
        launches
    );
    let id = stdout(&output)
        .trim_end()
        .strip_prefix("started:")
        .unwrap()
        .to_string();

    let (output, launches) = fx.dolt_launches(&a, &["thread", "pause", &id, "T-01", "Next"]);
    assert_success(&output);
    assert!(
        launches <= 5,
        "thread pause launched {} dolt processes",
        launches
    );

    // A resume of the thread this clone is bound to writes nothing.
    let (output, launches) = fx.dolt_launches(&a, &["thread", "resume", &id]);
    assert_success(&output);
    assert!(
        launches <= 2,
        "thread resume launched {} dolt processes",
        launches
    );
}

#[test]
fn thread_list_reads_every_thread_in_a_few_dolt_processes() {
    if !dolt_installed() {
        eprintln!("note: dolt is not installed; skipping");
        return;
    }
    let fx = Fixture::fetched();
    let a = fx.a();
    fx.ok(&a, &["mission", "create", "M-LIST", "List"]);
    fx.ok(&a, &["task", "add", "M-LIST", "A task"]);
    // Four threads, each bound to a cloud session of its own, two of them paused.
    let mut ids = Vec::new();
    for session in ["cse_one", "cse_two", "cse_three", "cse_four"] {
        let started = fx.tsk_cloud(&a, session, &["thread", "start", "M-LIST"]);
        assert_success(&started);
        ids.push(
            stdout(&started)
                .trim_end()
                .strip_prefix("started:")
                .unwrap()
                .to_string(),
        );
    }
    for (index, session) in ["cse_one", "cse_two"].iter().enumerate() {
        let paused = fx.tsk_cloud(
            &a,
            session,
            &["thread", "pause", &ids[index], "T-01", "Next steps"],
        );
        assert_success(&paused);
    }

    let (output, launches) = fx.dolt_launches(&a, &["thread", "list"]);
    assert_success(&output);
    // The refresh with its reads, and the continuation lines of every thread. It was two
    // processes for each thread, and more for the refresh.
    assert!(
        launches <= 3,
        "thread list launched {} dolt processes",
        launches
    );
    let rows: Vec<serde_json::Value> = stdout(&output).lines().map(json).collect();
    assert_eq!(rows.len(), 4);
    let paused: Vec<&serde_json::Value> = rows
        .iter()
        .filter(|row| row["latest_whats_next"] == "Next steps")
        .collect();
    assert_eq!(paused.len(), 2);
    // Newest pause first, then the threads that were never paused.
    assert_eq!(rows[0]["latest_whats_next"], "Next steps");
    assert_eq!(rows[1]["latest_whats_next"], "Next steps");
    assert!(rows[2]["latest_whats_next"].is_null());
}

// --- The query-by-query paths: TSK_NO_BATCH=1 gives the same output as the batched reads ---

/// One command of a scripted flow, run in clone `who` (`a` or `b`). `{tid}` in an argument
/// stands for the thread the latest `thread start` made.
#[derive(Clone)]
struct Run {
    who: char,
    args: Vec<String>,
    stdin: Option<String>,
}

fn run(who: char, args: &[&str]) -> Run {
    Run {
        who,
        args: args.iter().map(|a| a.to_string()).collect(),
        stdin: None,
    }
}

fn run_with_stdin(who: char, args: &[&str], stdin: &str) -> Run {
    Run {
        stdin: Some(stdin.to_string()),
        ..run(who, args)
    }
}

/// What a command printed, how it ended and how many `dolt` processes it started.
#[derive(Clone, Debug, PartialEq)]
struct Observed {
    command: String,
    stdout: String,
    stderr: String,
    code: i32,
    launches: usize,
}

/// Run the script in a fresh fixture, with the batched reads or without them, and return
/// what every command did with the ids, hashes, times and paths masked.
fn observe(no_batch: bool, script: &[Run]) -> Vec<Observed> {
    let extra: &[(&str, &str)] = if no_batch {
        &[("TSK_NO_BATCH", "1")]
    } else {
        &[]
    };
    observe_with(extra, script)
}

/// [`observe`] with `extra` set in the environment of every command.
fn observe_with(extra: &[(&str, &str)], script: &[Run]) -> Vec<Observed> {
    observe_program(None, extra, script)
}

/// [`observe_with`] for another binary than the one under test, such as an earlier release.
fn observe_program(
    program: Option<&Path>,
    extra: &[(&str, &str)],
    script: &[Run],
) -> Vec<Observed> {
    observe_in(&Fixture::fetched(), program, extra, script)
}

/// [`observe_program`] in a fixture the caller made.
fn observe_in(
    fx: &Fixture,
    program: Option<&Path>,
    extra: &[(&str, &str)],
    script: &[Run],
) -> Vec<Observed> {
    let log = fx.root.path().join("dolt-log.txt");
    let shim = dolt_log_shim(fx.root.path());
    let mut thread_ids: Vec<String> = Vec::new();
    let mut seen = Vec::new();
    for step in script {
        let dir = if step.who == 'a' { fx.a() } else { fx.b() };
        let args: Vec<String> = step
            .args
            .iter()
            .map(|arg| arg.replace("{tid}", thread_ids.last().map_or("", String::as_str)))
            .collect();
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let _ = std::fs::remove_file(&log);
        let mut command = match program {
            Some(program) => fx.command_with(program, &dir, &refs),
            None => fx.command(&dir, &refs),
        };
        command
            .env("PATH", path_with_first(&shim))
            .env("TSK_DOLT_LOG", &log)
            .env_remove("CLAUDE_ENV_FILE");
        for (name, value) in extra {
            command.env(name, value);
        }
        let output = match &step.stdin {
            Some(input) => {
                let mut child = command
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
                    .unwrap();
                child
                    .stdin
                    .take()
                    .unwrap()
                    .write_all(input.as_bytes())
                    .unwrap();
                child.wait_with_output().unwrap()
            }
            None => command.output().unwrap(),
        };
        if let Some(id) = stdout(&output).trim_end().strip_prefix("started:") {
            thread_ids.push(id.to_string());
        }
        let launches = std::fs::read_to_string(&log)
            .unwrap_or_default()
            .lines()
            .count();
        seen.push((format!("{} {}", step.who, args.join(" ")), output, launches));
    }
    let mut replacements: Vec<(String, String)> = Vec::new();
    for (who, dir) in [("a", fx.a()), ("b", fx.b())] {
        if let Ok(id) = std::fs::read_to_string(dir.join(".git").join("tsk-clone-id")) {
            replacements.push((id.trim().to_string(), format!("<clone-{}>", who)));
        }
    }
    for (index, id) in thread_ids.iter().enumerate() {
        replacements.push((id.clone(), format!("<thread-{}>", index + 1)));
    }
    let root = fx.root.path().to_string_lossy().into_owned();
    let mask = |text: &str| mask_run(text, &root, &replacements);
    seen.into_iter()
        .map(|(command, output, launches)| Observed {
            command: mask(&command),
            stdout: mask(&stdout(&output)),
            stderr: mask(&stderr(&output)),
            code: output.status.code().unwrap_or(-1),
            launches,
        })
        .collect()
}

/// Replace what differs between two runs of the same script: the fixture path, the clone
/// and thread ids, Dolt and git commit ids and UTC timestamps.
fn mask_run(text: &str, root: &str, replacements: &[(String, String)]) -> String {
    let mut out = text.replace(root, "<root>");
    for (from, to) in replacements {
        out = out.replace(from.as_str(), to);
    }
    let chars: Vec<char> = out.chars().collect();
    let mut masked = String::new();
    let mut index = 0;
    while index < chars.len() {
        if let Some(len) = timestamp_len(&chars[index..]) {
            masked.push_str("<time>");
            index += len;
            continue;
        }
        if chars[index].is_ascii_alphanumeric() {
            let end = (index..chars.len())
                .find(|&at| !chars[at].is_ascii_alphanumeric())
                .unwrap_or(chars.len());
            let word: String = chars[index..end].iter().collect();
            let dolt_hash = word.len() == 32
                && word
                    .chars()
                    .all(|c| c.is_ascii_digit() || ('a'..='v').contains(&c));
            let git_hash = word.len() == 40 && word.chars().all(|c| c.is_ascii_hexdigit());
            masked.push_str(if dolt_hash {
                "<dolt-hash>"
            } else if git_hash {
                "<git-hash>"
            } else {
                &word
            });
            index = end;
            continue;
        }
        masked.push(chars[index]);
        index += 1;
    }
    masked
}

/// The length of a `2026-10-19T12:00:00Z` timestamp at the start of `chars`.
fn timestamp_len(chars: &[char]) -> Option<usize> {
    const SHAPE: &str = "dddd-dd-ddTdd:dd:ddZ";
    if chars.len() < SHAPE.len() {
        return None;
    }
    let fits = SHAPE.chars().zip(chars).all(|(shape, c)| match shape {
        'd' => c.is_ascii_digit(),
        other => *c == other,
    });
    fits.then_some(SHAPE.len())
}

/// The `TSK_DOLT_SERVER` value that the run does not use: the third run of a script sets it.
fn other_server_setting() -> &'static str {
    if server_on() {
        "off"
    } else {
        "on"
    }
}

/// Run the script three times, each in a fresh fixture: with the batched reads, with
/// `TSK_NO_BATCH=1`, and with the Dolt server switch the other way round. Require the same
/// output, errors and exit codes from every command in the batched and the one by one
/// runs, and in the runs with the server off and on. The one by one run must start more
/// `dolt` processes than the batched run (when the server is off), and the run with the
/// server must start fewer than half of those of the run without it.
fn assert_same_three_ways(script: &[Run]) {
    let batched = observe(false, script);
    let one_by_one = observe(true, script);
    let flipped = observe_with(&[("TSK_DOLT_SERVER", other_server_setting())], script);
    assert_same_observed("batched against one by one", &batched, &one_by_one);
    assert_same_server_runs(&batched, &flipped);
}

/// [`assert_same_three_ways`] for a script that follows [`seed_records`], given as the
/// `tail` after the seed. The seed runs once per test process, batched, into a cached
/// template, and its observed commands are kept. The batched and the one by one runs each
/// start from their own copy of the seeded fixture and run only `tail`, and the seed
/// commands themselves are compared in the batched and the one by one runs by
/// `records_read_the_same_without_batching_and_with_the_server`. The run with the server
/// the other way round runs the whole script in a fresh fixture, and its commands are
/// compared with the seed and the batched tail together.
fn assert_same_three_ways_after_seed(tail: &[Run]) {
    let run_tail = |no_batch: bool| {
        let extra: &[(&str, &str)] = if no_batch {
            &[("TSK_NO_BATCH", "1")]
        } else {
            &[]
        };
        observe_in(&Fixture::seeded_records(), None, extra, tail)
    };
    let batched_tail = run_tail(false);
    let one_by_one_tail = run_tail(true);
    let mut script = seed_records();
    script.extend(tail.iter().cloned());
    let flipped = observe_with(&[("TSK_DOLT_SERVER", other_server_setting())], &script);
    let mut batched = Fixture::seeded_records_observed();
    batched.extend(batched_tail.iter().cloned());
    assert_same_observed(
        "batched against one by one",
        &batched_tail,
        &one_by_one_tail,
    );
    assert_same_server_runs(&batched, &flipped);
}

static SEED_OBSERVED: std::sync::OnceLock<Vec<Observed>> = std::sync::OnceLock::new();

impl Fixture {
    /// A fetched ledger with [`seed_records`] written to clone A, built once per test
    /// process. A test gets its own copy and never writes to the template.
    fn seeded_records() -> Fixture {
        Fixture::cached("seeded-records", || {
            // Built here, not through `Fixture::fetched`: `cached` holds the template
            // lock while it builds, and a nested `cached` call would wait on itself.
            let fx = Fixture::new();
            fx.ok(&fx.a(), &["ledger", "fetch"]);
            let seeded = observe_in(&fx, None, &[], &seed_records());
            for step in &seeded {
                assert_eq!(
                    step.code, 0,
                    "the seed command {} failed: {}",
                    step.command, step.stderr
                );
            }
            SEED_OBSERVED.set(seeded).unwrap();
            (fx, String::new())
        })
        .0
    }

    /// What the seed commands of [`Fixture::seeded_records`] did when the template was
    /// built, with the same masks as a run in a fresh fixture.
    fn seeded_records_observed() -> Vec<Observed> {
        Fixture::seeded_records();
        SEED_OBSERVED.get().unwrap().clone()
    }
}

fn assert_same_observed(what: &str, batched: &[Observed], one_by_one: &[Observed]) {
    if std::env::var_os("TSK_TEST_SHOW_FLOW").is_some() {
        for (with, without) in batched.iter().zip(one_by_one) {
            eprintln!(
                "[{} -> {}] {} (exit {})\n  out: {}\n  err: {}",
                with.launches,
                without.launches,
                with.command,
                with.code,
                with.stdout
                    .replace('\n', "\\n")
                    .chars()
                    .take(110)
                    .collect::<String>(),
                with.stderr
                    .replace('\n', "\\n")
                    .chars()
                    .take(110)
                    .collect::<String>()
            );
        }
    }
    assert_eq!(batched.len(), one_by_one.len(), "{}", what);
    for (with, without) in batched.iter().zip(one_by_one) {
        assert_eq!(with.command, without.command, "{}", what);
        assert_eq!(
            no_server_line(without_uids(with.stdout.clone())),
            no_server_line(without_uids(without.stdout.clone())),
            "{}: stdout of {}",
            what,
            with.command
        );
        assert_eq!(
            with.stderr, without.stderr,
            "{}: stderr of {}",
            what, with.command
        );
        assert_eq!(
            with.code, without.code,
            "{}: exit code of {}",
            what, with.command
        );
    }
    let total = |runs: &[Observed]| runs.iter().map(|o| o.launches).sum::<usize>();
    // With the Dolt server on, neither run starts a `dolt` process for its SQL, so the
    // counts cannot tell the paths apart. The comparison of the output above stands.
    if server_on() {
        return;
    }
    assert!(
        total(one_by_one) > total(batched),
        "{}: TSK_NO_BATCH=1 started {} dolt processes against {} batched",
        what,
        total(one_by_one),
        total(batched)
    );
}

/// Require the same output, errors and exit codes from every command in a run with the
/// Dolt server off and a run with it on. With the server on, a command starts no `dolt`
/// process for its SQL, so the run with the server starts fewer in total. `one` and
/// `other` are the two runs of the script, in either order.
fn assert_same_server_runs(one: &[Observed], other: &[Observed]) {
    let (off, on) = if server_on() {
        (other, one)
    } else {
        (one, other)
    };
    let what = "with the server against without";
    assert_eq!(off.len(), on.len(), "{}", what);
    for (without, with) in off.iter().zip(on) {
        assert_eq!(without.command, with.command, "{}", what);
        assert_eq!(
            no_server_line(without_uids(without.stdout.clone())),
            no_server_line(without_uids(with.stdout.clone())),
            "{}: stdout of {}",
            what,
            without.command
        );
        assert_eq!(
            without.stderr, with.stderr,
            "{}: stderr of {}",
            what, without.command
        );
        assert_eq!(
            without.code, with.code,
            "{}: exit code of {}",
            what, without.command
        );
    }
    let total = |runs: &[Observed]| runs.iter().map(|o| o.launches).sum::<usize>();
    assert!(
        total(on) * 2 < total(off),
        "{}: the server run started {} dolt processes against {} without it",
        what,
        total(on),
        total(off)
    );
}

fn seed_records() -> Vec<Run> {
    vec![
        run('a', &["mission", "create", "M-NB", "No batch"]),
        run(
            'a',
            &["mission", "objective", "add", "M-NB", "Read the same"],
        ),
        run('a', &["mission", "constraint", "add", "M-NB", "One output"]),
        run('a', &["task", "add", "M-NB", "First task"]),
        run('a', &["task", "add", "M-NB", "Second task"]),
        run('a', &["task", "block", "M-NB", "T-02", "T-01"]),
        run(
            'a',
            &[
                "decision",
                "add",
                "M-NB",
                "Keep both paths",
                "--by",
                "Jim",
                "--on",
                "2026-10-19",
            ],
        ),
        run('a', &["question", "add", "M-NB", "Why two paths?"]),
        run_with_stdin('a', &["ledger", "doc", "put", "m/design.md"], "# Design\n"),
        run(
            'a',
            &["mission", "link", "M-NB", "m/design.md", "--role", "design"],
        ),
    ]
}

fn records_script() -> Vec<Run> {
    let mut script = seed_records();
    script.extend([
        run('a', &["mission", "list"]),
        run('a', &["mission", "list", "--porcelain"]),
        run('a', &["mission", "list", "M-NB", "--json"]),
        run('a', &["task", "list", "M-NB"]),
        run('a', &["decision", "list", "M-NB"]),
        run('a', &["question", "list"]),
        run('a', &["task", "set", "M-NB", "T-01", "status", "done"]),
        run('a', &["task", "list", "M-NB", "--porcelain"]),
        run('a', &["mission", "list", "M-MISSING", "--json"]),
    ]);
    script
}

#[test]
fn records_read_the_same_without_batching_and_with_the_server() {
    if !dolt_installed() {
        eprintln!("note: dolt is not installed; skipping");
        return;
    }
    let script = records_script();
    assert_same_three_ways(&script);
}

fn fetch_status_script() -> Vec<Run> {
    let mut script = seed_records();
    script.extend(fetch_status_tail());
    script
}

fn fetch_status_tail() -> Vec<Run> {
    vec![
        run('a', &["ledger", "status"]),
        run('a', &["ledger", "status", "--porcelain"]),
        run('a', &["ledger", "push", "Seed the records"]),
        run('b', &["ledger", "fetch"]),
        run('b', &["ledger", "status"]),
        run('b', &["ledger", "status", "--local"]),
        run('b', &["mission", "list"]),
        run('a', &["task", "add", "M-NB", "Third task"]),
        run('a', &["ledger", "status"]),
        run('a', &["ledger", "status", "--local", "--porcelain"]),
        run('a', &["ledger", "push", "Third task"]),
        run('b', &["ledger", "status"]),
        run('b', &["ledger", "status", "--porcelain"]),
        run('b', &["ledger", "fetch"]),
        run('b', &["ledger", "status"]),
        run('b', &["task", "list", "M-NB"]),
        run('b', &["ledger", "discard"]),
    ]
}

#[test]
fn fetch_and_status_read_the_same_without_batching_and_with_the_server() {
    if !dolt_installed() {
        eprintln!("note: dolt is not installed; skipping");
        return;
    }
    let tail = fetch_status_tail();
    assert_same_three_ways_after_seed(&tail);
}

fn push_merge_script() -> Vec<Run> {
    let mut script = seed_records();
    script.extend(push_merge_tail());
    script
}

fn push_merge_tail() -> Vec<Run> {
    vec![
        run('a', &["ledger", "push", "Seed the records"]),
        run('b', &["ledger", "fetch"]),
        // Both change different rows and documents, and the second push merges.
        run('a', &["task", "add", "M-NB", "From A"]),
        run_with_stdin('a', &["ledger", "doc", "put", "from-a.md"], "a\n"),
        run('a', &["ledger", "push", "From A"]),
        run('b', &["task", "add", "M-NB", "From B"]),
        run_with_stdin('b', &["ledger", "doc", "put", "from-b.md"], "b\n"),
        run('b', &["ledger", "push", "From B"]),
        run('b', &["ledger", "push", "Nothing more"]),
        run('a', &["ledger", "fetch"]),
        run('a', &["task", "list", "M-NB"]),
        run('a', &["ledger", "doc", "list"]),
        // A linked document that is gone stops the push, with the same error.
        run('a', &["ledger", "doc", "rm", "m/design.md"]),
        run('a', &["task", "add", "M-NB", "Stopped"]),
        run('a', &["ledger", "push", "Stopped by a link"]),
        run('a', &["ledger", "status"]),
    ]
}

#[test]
fn push_and_merge_read_the_same_without_batching_and_with_the_server() {
    if !dolt_installed() {
        eprintln!("note: dolt is not installed; skipping");
        return;
    }
    let tail = push_merge_tail();
    assert_same_three_ways_after_seed(&tail);
}

fn thread_script() -> Vec<Run> {
    let mut script = seed_records();
    script.extend(thread_tail());
    script
}

fn thread_tail() -> Vec<Run> {
    vec![
        run('a', &["ledger", "push", "Seed the records"]),
        run('a', &["thread", "start", "M-NB"]),
        run('a', &["thread", "start", "M-NB"]),
        run('a', &["thread", "pause", "{tid}", "T-01", "First pause"]),
        run(
            'a',
            &[
                "thread",
                "pause",
                "{tid}",
                "T-09",
                "A task that is not there",
            ],
        ),
        run('a', &["thread", "pause", "{tid}", "T-02", "Second pause"]),
        run('a', &["thread", "list"]),
        run('a', &["thread", "resume", "{tid}"]),
        run('b', &["ledger", "fetch"]),
        run('b', &["thread", "resume", "{tid}"]),
        run('b', &["thread", "list"]),
        run('a', &["thread", "set", "mission", "--none"]),
        run('a', &["thread", "list"]),
        run('a', &["thread", "binding"]),
        run('a', &["thread", "stop"]),
        run('a', &["thread", "list"]),
        run('a', &["thread", "resume", "{tid}"]),
        run_with_stdin(
            'a',
            &["thread", "session-start", "--format", "claude"],
            "{\"session_id\":\"nb-1\",\"source\":\"startup\"}",
        ),
        run_with_stdin('a', &["thread", "guard"], "{\"session_id\":\"x\"}"),
    ]
}

#[test]
fn thread_commands_read_the_same_without_batching_and_with_the_server() {
    if !dolt_installed() {
        eprintln!("note: dolt is not installed; skipping");
        return;
    }
    let tail = thread_tail();
    assert_same_three_ways_after_seed(&tail);
}

/// With the switch off, `tsk` is the release before the server: the same output, errors and
/// exit codes from every command, and the same number of `dolt` processes. The case runs
/// when `TSK_BASELINE_BIN` names that release's binary (a 0.22.0 build), and skips
/// otherwise, because the test run does not have one.
#[test]
fn with_the_switch_off_every_command_is_that_of_the_baseline_release() {
    if !dolt_installed() {
        eprintln!("note: dolt is not installed; skipping");
        return;
    }
    let Some(baseline) = std::env::var_os("TSK_BASELINE_BIN") else {
        eprintln!("note: TSK_BASELINE_BIN is not set; skipping");
        return;
    };
    let baseline = PathBuf::from(baseline);
    let mut commands = 0;
    for script in [
        records_script(),
        fetch_status_script(),
        push_merge_script(),
        thread_script(),
    ] {
        let old = observe_program(Some(&baseline), &[("TSK_DOLT_SERVER", "off")], &script);
        let new = observe_program(None, &[("TSK_DOLT_SERVER", "off")], &script);
        assert_eq!(old.len(), new.len());
        for (before, after) in old.iter().zip(&new) {
            assert_eq!(before.command, after.command);
            assert_eq!(before.stdout, after.stdout, "stdout of {}", before.command);
            assert_eq!(before.stderr, after.stderr, "stderr of {}", before.command);
            assert_eq!(before.code, after.code, "exit code of {}", before.command);
            assert_eq!(
                before.launches, after.launches,
                "dolt processes of {}",
                before.command
            );
            commands += 1;
        }
    }
    eprintln!("note: {} commands matched the baseline", commands);
}

// --- A pause, a start and a push do not ask origin first; a fresh clone fetches once ---

impl Fixture {
    /// A fetched ledger with mission M-PAUSE and its task T-01, and a thread started on it
    /// from clone A. Returns the fixture and the thread ID.
    fn paused_thread() -> (Fixture, String) {
        let fx = Fixture::fetched();
        let a = fx.a();
        fx.ok(&a, &["mission", "create", "M-PAUSE", "Pause"]);
        fx.ok(&a, &["task", "add", "M-PAUSE", "First"]);
        fx.ok(&a, &["ledger", "push", "Mission"]);
        let started = fx.ok(&a, &["thread", "start", "M-PAUSE"]);
        let id = started
            .trim_end()
            .strip_prefix("started:")
            .unwrap()
            .to_string();
        (fx, id)
    }

    /// Clone B adds a task to M-PAUSE and publishes it, so that clone A's record of the
    /// last fetch is out of date.
    fn b_moves_origin(&self, task: &str) {
        let b = self.b();
        self.ok(&b, &["ledger", "fetch"]);
        self.ok(&b, &["task", "add", "M-PAUSE", task]);
        self.ok(&b, &["ledger", "push", "B adds a task"]);
    }
}

#[test]
fn a_pause_does_not_ask_origin_and_pushes_once() {
    if !dolt_installed() {
        eprintln!("note: dolt is not installed; skipping");
        return;
    }
    let (fx, id) = Fixture::paused_thread();
    let (a, b) = (fx.a(), fx.b());

    let (output, ops, log) = fx.counted(&a, &["thread", "pause", &id, "T-01", "Next step"]);
    assert_success(&output);
    assert_eq!(
        stdout(&output),
        format!(
            "paused:{}\ncode: clean\ncode pushed: yes\nledger: pushed\n",
            id
        )
    );
    // The record of the last fetch stands in for the question to origin, so the one
    // network operation is the push of the data. The documents did not change.
    assert_eq!(
        ops,
        NetOps {
            asks: 0,
            ls_remote: 0,
            fetch: 0,
            push: 1
        },
        "{}",
        log
    );

    // The entry reached origin.
    fx.ok(&b, &["ledger", "fetch"]);
    let list = fx.ok(&b, &["thread", "list"]);
    assert!(
        list.contains("\"latest_whats_next\":\"Next step\""),
        "{}",
        list
    );
    // Nothing is left to push on A.
    let status = fx.ok(&a, &["ledger", "status"]);
    assert!(status.contains("nothing to push"), "{}", status);
}

#[test]
fn a_pause_after_origin_moved_is_rejected_quietly_once_and_keeps_both_changes() {
    if !dolt_installed() {
        eprintln!("note: dolt is not installed; skipping");
        return;
    }
    let (fx, id) = Fixture::paused_thread();
    let (a, b) = (fx.a(), fx.b());
    fx.b_moves_origin("From B");

    let output = fx.tsk(&a, &["thread", "pause", &id, "T-01", "After the move"]);
    assert_success(&output);
    assert_eq!(
        stdout(&output),
        format!(
            "paused:{}\ncode: clean\ncode pushed: yes\nledger: pushed\n",
            id
        )
    );
    // The first rejection only shows that the record was out of date, which an ask to
    // origin first would have shown without a note. It is not said.
    assert_eq!(stderr(&output), "");

    fx.ok(&b, &["ledger", "fetch"]);
    let tasks = fx.ok(&b, &["task", "list", "M-PAUSE", "--porcelain"]);
    assert!(tasks.contains("From B"), "{}", tasks);
    let list = fx.ok(&b, &["thread", "list"]);
    assert!(list.contains("After the move"), "{}", list);
}

#[test]
fn a_pause_checks_what_it_does_not_know_against_a_fresh_fetch() {
    if !dolt_installed() {
        eprintln!("note: dolt is not installed; skipping");
        return;
    }
    let (fx, _) = Fixture::paused_thread();
    let (a, b) = (fx.a(), fx.b());

    // B starts a thread and adds a task after A's last fetch. A's record knows neither.
    fx.ok(&b, &["ledger", "fetch"]);
    let started = fx.tsk_cloud(&b, "cse_pause_b", &["thread", "start", "M-PAUSE"]);
    assert_success(&started);
    let other = stdout(&started)
        .trim_end()
        .strip_prefix("started:")
        .unwrap()
        .to_string();
    fx.b_moves_origin("Second");

    let output = fx.tsk(&a, &["thread", "pause", &other, "T-02", "From A"]);
    assert_success(&output);
    assert_eq!(
        stdout(&output),
        format!(
            "paused:{}\ncode: clean\ncode pushed: yes\nledger: pushed\n",
            other
        )
    );
    fx.ok(&a, &["ledger", "fetch"]);
    let list = fx.ok(&a, &["thread", "list"]);
    assert!(list.contains("From A"), "{}", list);

    // A thread nobody has made, and a task the mission lacks, are named as before.
    let unknown = fx.tsk(&a, &["thread", "pause", "zzzzzzzz", "", "Next"]);
    assert_exit(&unknown, 1);
    assert!(
        stderr(&unknown).starts_with("error: no such thread: zzzzzzzz (expected "),
        "{}",
        stderr(&unknown)
    );
    let missing = fx.tsk(&a, &["thread", "pause", &other, "T-09", "Next"]);
    assert_exit(&missing, 1);
    assert!(stderr(&missing).contains("T-09"), "{}", stderr(&missing));
}

#[test]
fn a_thread_start_checks_the_mission_against_a_fresh_fetch_when_the_record_lacks_it() {
    if !dolt_installed() {
        eprintln!("note: dolt is not installed; skipping");
        return;
    }
    let (fx, _) = Fixture::paused_thread();
    let (a, b) = (fx.a(), fx.b());
    fx.ok(&b, &["ledger", "fetch"]);
    fx.ok(&b, &["mission", "create", "M-LATE", "Late"]);
    fx.ok(&b, &["ledger", "push", "B adds a mission"]);

    // A's record does not hold M-LATE yet; the start finds it on a fresh fetch.
    let started = fx.tsk_cloud(&a, "cse_late", &["thread", "start", "M-LATE"]);
    assert_success(&started);
    assert!(
        stdout(&started).starts_with("started:"),
        "{}",
        stdout(&started)
    );
    let unknown = fx.tsk_cloud(&a, "cse_unknown", &["thread", "start", "M-NOPE"]);
    assert_exit(&unknown, 1);
    assert!(
        stderr(&unknown).starts_with("error: no such mission: M-NOPE"),
        "{}",
        stderr(&unknown)
    );
}

#[test]
fn a_push_and_a_pause_launch_few_dolt_processes() {
    if !dolt_installed() {
        eprintln!("note: dolt is not installed; skipping");
        return;
    }
    let (fx, id) = Fixture::paused_thread();
    let a = fx.a();
    fx.ok(&a, &["ledger", "fetch"]);

    // The refresh, the write with the commit and the reads of the push, and the push.
    let (output, launches) = fx.dolt_launches(&a, &["thread", "pause", &id, "T-01", "Next"]);
    assert_success(&output);
    assert!(
        launches <= 3,
        "a pause launched {} dolt processes",
        launches
    );

    fx.ok(&a, &["task", "add", "M-PAUSE", "Another"]);
    let (output, launches) = fx.dolt_launches(&a, &["ledger", "push", "A change"]);
    assert_success(&output);
    assert!(
        launches <= 3,
        "a push with a change launched {} dolt processes",
        launches
    );
    let (output, launches) = fx.dolt_launches(&a, &["ledger", "push", "Nothing"]);
    assert_success(&output);
    assert!(
        launches <= 2,
        "a push with nothing to send launched {} dolt processes",
        launches
    );
}

#[test]
fn a_fresh_clone_fetches_both_refs_in_one_network_operation() {
    if !dolt_installed() {
        eprintln!("note: dolt is not installed; skipping");
        return;
    }
    let fx = Fixture::with_briefing();
    let b = fx.b();

    // B has no ledger store and no record: one fetch brings both refs, with no question to
    // origin and no second fetch.
    let (output, ops, log) = fx.counted(&b, &["ledger", "fetch"]);
    assert_success(&output);
    assert_eq!(
        ops,
        NetOps {
            asks: 0,
            ls_remote: 0,
            fetch: 1,
            push: 0
        },
        "{}",
        log
    );
    assert!(log.contains(&format!("fetch --quiet --no-tags origin +{}:", LEDGER_REF)));
    assert_eq!(
        fx.ok(&b, &["ledger", "doc", "get", BRIEFING]),
        "# Mission: conformance\n"
    );
    // The next fetch finds nothing new and asks once, as before.
    let (_, ops) = fx.counted_ok(&b, &["ledger", "fetch"]);
    assert_eq!(ops, NOTHING_NEW);
}

#[test]
fn a_fresh_clone_of_a_new_ledger_asks_when_origin_lacks_the_refs() {
    if !dolt_installed() {
        eprintln!("note: dolt is not installed; skipping");
        return;
    }
    let fx = Fixture::new();
    let a = fx.a();
    // Origin holds neither ref: the combined fetch fails and the question says so.
    let (output, ops, _) = fx.counted(&a, &["ledger", "fetch"]);
    assert_success(&output);
    assert_eq!(ops.fetch, 1);
    // The question for both refs, as before the combined fetch existed.
    assert_eq!(ops.asks, 1);
    assert!(
        stderr(&output).contains("does not exist yet"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn a_refresh_that_finds_origin_moved_reads_the_database_in_few_processes() {
    if !dolt_installed() {
        eprintln!("note: dolt is not installed; skipping");
        return;
    }
    let (fx, _) = Fixture::paused_thread();
    let (a, b) = (fx.a(), fx.b());
    fx.ok(&a, &["ledger", "fetch"]);
    fx.b_moves_origin("Moved");

    // The reads that ride with the question, the fetch, the reads after it, and the reset
    // with the reads that follow it. It was five.
    let (output, launches) = fx.dolt_launches(&a, &["ledger", "fetch"]);
    assert_success(&output);
    assert!(
        launches <= 4,
        "a refresh launched {} dolt processes",
        launches
    );
    let tasks = fx.ok(&a, &["task", "list", "M-PAUSE", "--porcelain"]);
    assert!(tasks.contains("Moved"), "{}", tasks);

    // A clone with a change of its own refuses to reset over it, as before, when origin moved.
    fx.ok(&b, &["task", "add", "M-PAUSE", "Local to B"]);
    fx.ok(&a, &["task", "add", "M-PAUSE", "From A"]);
    fx.ok(&a, &["ledger", "push", "A adds a task"]);
    let refused = fx.tsk(&b, &["ledger", "fetch"]);
    assert_exit(&refused, 1);
    assert!(
        stderr(&refused).contains("holds uncommitted changes"),
        "{}",
        stderr(&refused)
    );
    fx.ok(&b, &["ledger", "push", "B pushes its task"]);
    let tasks = fx.ok(&b, &["task", "list", "M-PAUSE", "--porcelain"]);
    assert!(
        tasks.contains("From A") && tasks.contains("Local to B"),
        "{}",
        tasks
    );
}

// --- Layout 10: a mission has a code that a rename changes and an internal ID that it does not ---

impl Fixture {
    /// A published Dolt ledger at version 9, as the previous release left it: the missions
    /// table has no `code`. It holds M-NINE, with a parent, a blocker, a constraint, an
    /// out-of-scope item, an objective, a document, two tasks with a block, a decision that
    /// names a task, a question and a thread. Returns the thread ID.
    fn version_nine_ledger() -> (Fixture, String) {
        Fixture::cached("version-nine", Fixture::build_version_nine_ledger)
    }

    fn build_version_nine_ledger() -> (Fixture, String) {
        let fx = Fixture::new();
        let a = fx.a();
        fx.ok(&a, &["ledger", "fetch"]);
        fx.ok(&a, &["mission", "create", "M-TOP", "Parent"]);
        fx.ok(&a, &["mission", "create", "M-DEP", "Dependency"]);
        fx.ok(
            &a,
            &[
                "mission",
                "create",
                "M-NINE",
                "Made on nine",
                "--parent",
                "M-TOP",
            ],
        );
        fx.ok(&a, &["mission", "block", "M-NINE", "M-DEP"]);
        fx.ok(&a, &["mission", "set", "M-NINE", "purpose", "Keep me"]);
        fx.ok(
            &a,
            &["mission", "constraint", "add", "M-NINE", "A constraint"],
        );
        fx.ok(
            &a,
            &["mission", "out-of-scope", "add", "M-NINE", "Not this"],
        );
        fx.ok(&a, &["mission", "objective", "add", "M-NINE", "An outcome"]);
        fx.put(&a, "m/nine.md", "nine\n");
        fx.ok(
            &a,
            &["mission", "link", "M-NINE", "m/nine.md", "--role", "design"],
        );
        fx.ok(&a, &["task", "add", "M-NINE", "First"]);
        fx.ok(&a, &["task", "add", "M-NINE", "Second"]);
        fx.ok(&a, &["task", "block", "M-NINE", "T-02", "T-01"]);
        fx.ok(
            &a,
            &[
                "decision", "add", "M-NINE", "--by", "Jim", "--task", "T-01", "Decided",
            ],
        );
        fx.ok(&a, &["question", "add", "M-NINE", "Why?"]);
        let id = fx.started(&a, &["thread", "start", "M-NINE"]);
        fx.dolt_sql(&a, &layout_nine_sql());
        fx.dolt_sql(&a, "replace into manifest (k, v) values ('version', '9')");
        fx.ok(&a, &["ledger", "push", "A version 9 ledger"]);
        (fx, id)
    }

    /// The record of a mission as JSON, without its `uid`.
    fn record(&self, dir: &Path, mission: &str) -> serde_json::Value {
        let mut record = json(&self.ok(dir, &["mission", "list", mission, "--json"]));
        if let Some(object) = record.as_object_mut() {
            object.remove("uid");
        }
        record
    }

    /// The row of a mission as `id,code`, read from the table by its code.
    fn id_and_code(&self, dir: &Path, code: &str) -> String {
        self.dolt_sql(
            dir,
            &format!("select id, code from missions where code = '{}'", code),
        )
        .lines()
        .nth(1)
        .unwrap_or("")
        .to_string()
    }
}

#[test]
fn a_version_nine_ledger_upgrades_by_adding_the_code_and_keeps_every_record() {
    if !dolt_installed() {
        eprintln!("note: dolt is not installed; skipping");
        return;
    }
    let (fx, id) = Fixture::version_nine_ledger();
    let b = fx.b();
    fx.ok(&b, &["ledger", "fetch"]);
    assert_eq!(fx.manifest_version(&b), "9");
    assert!(!fx
        .dolt_sql(&b, "show create table missions")
        .contains("code"));

    // A read shows every record of the older layout and upgrades nothing. A mission of
    // that layout has its first ID as its internal ID.
    let before: Vec<serde_json::Value> = ["M-TOP", "M-DEP", "M-NINE"]
        .iter()
        .map(|m| fx.record(&b, m))
        .collect();
    assert_eq!(before[2]["parent"], "M-TOP");
    assert_eq!(before[2]["blocked_by"][0]["id"], "M-DEP");
    assert_eq!(before[2]["tasks"].as_array().unwrap().len(), 2);
    assert_eq!(before[2]["decisions"][0]["task"], "T-01");
    let shown = json(&fx.ok(&b, &["mission", "list", "M-NINE", "--json"]));
    assert_eq!(shown["uid"], "M-NINE");
    assert_eq!(fx.manifest_version(&b), "9");
    assert_eq!(fx.thread_row(&b, &id)["mission_id"], "M-NINE");

    // The first write upgrades, in a change that adds one column and one index.
    fx.ok(&b, &["mission", "set", "M-NINE", "purpose", "Changed"]);
    assert_eq!(fx.manifest_version(&b), "10");
    for (at, mission) in ["M-TOP", "M-DEP", "M-NINE"].iter().enumerate() {
        assert_eq!(
            fx.id_and_code(&b, mission),
            format!("{},{}", mission, mission)
        );
        let mut record = fx.record(&b, mission);
        if *mission == "M-NINE" {
            assert_eq!(record["purpose"], "Changed");
            record["purpose"] = serde_json::json!("Keep me");
        }
        assert_eq!(record, before[at], "{}", mission);
    }
    assert_eq!(fx.thread_row(&b, &id)["mission_id"], "M-NINE");
    // A new mission gets a random internal ID, and its code is the ID a person typed.
    assert_eq!(fx.ok(&b, &["task", "add", "M-NINE", "Third"]), "T-03\n");
    fx.ok(&b, &["mission", "create", "M-NEW", "New"]);
    let row = fx.id_and_code(&b, "M-NEW");
    let (internal, code) = row.split_once(',').unwrap();
    assert_eq!(code, "M-NEW");
    assert_eq!(internal.len(), 36, "{}", row);
    assert_eq!(
        json(&fx.ok(&b, &["mission", "list", "M-NEW", "--json"]))["uid"],
        internal
    );
}

#[test]
fn two_clones_that_upgrade_a_version_nine_ledger_on_their_own_merge() {
    if !dolt_installed() {
        eprintln!("note: dolt is not installed; skipping");
        return;
    }
    let (fx, _id) = Fixture::version_nine_ledger();
    let (a, b) = (fx.a(), fx.b());
    fx.ok(&a, &["ledger", "fetch"]);
    fx.ok(&b, &["ledger", "fetch"]);
    // Each clone upgrades with a write of its own, before either pushes.
    fx.ok(&a, &["task", "add", "M-NINE", "From A"]);
    fx.ok(&b, &["question", "add", "M-NINE", "From B?"]);
    assert_eq!(fx.manifest_version(&a), "10");
    assert_eq!(fx.manifest_version(&b), "10");
    fx.ok(&a, &["ledger", "push", "A upgrades and adds a task"]);
    fx.ok(&b, &["ledger", "push", "B upgrades and asks"]);

    fx.ok(&a, &["ledger", "fetch"]);
    for dir in [&a, &b] {
        assert!(fx
            .ok(dir, &["task", "list", "M-NINE", "--porcelain"])
            .contains("From A"));
        assert!(fx
            .ok(dir, &["question", "list", "M-NINE", "--porcelain"])
            .contains("From B?"));
        let record = fx.record(dir, "M-NINE");
        assert_eq!(record["parent"], "M-TOP");
        assert_eq!(record["decisions"][0]["task"], "T-01");
        assert_eq!(fx.id_and_code(dir, "M-NINE"), "M-NINE,M-NINE");
    }
}

// --- tsk mv ---

impl Fixture {
    /// A ledger with M-OLD, which has a parent, a blocker, a child, two tasks (T-02 is blocked
    /// by T-01), a decision on T-01, a decision on no task, a question, a thread and two
    /// linked documents, one named for it. M-OTHER waits for M-OLD and has a task that names
    /// it. It is pushed from clone A and fetched into clone B.
    fn mv_ledger() -> (Fixture, String) {
        let fx = Fixture::new();
        let (a, b) = (fx.a(), fx.b());
        fx.ok(&a, &["ledger", "fetch"]);
        for (id, name) in [
            ("M-TOP", "Parent"),
            ("M-DEP", "Dependency"),
            ("M-OTHER", "Other"),
        ] {
            fx.ok(&a, &["mission", "create", id, name]);
        }
        fx.ok(
            &a,
            &[
                "mission", "create", "M-OLD", "Old name", "--parent", "M-TOP",
            ],
        );
        fx.ok(
            &a,
            &["mission", "create", "M-KID", "Kid", "--parent", "M-OLD"],
        );
        fx.ok(&a, &["mission", "block", "M-OLD", "M-DEP"]);
        fx.ok(&a, &["mission", "block", "M-OTHER", "M-OLD"]);
        fx.ok(&a, &["task", "add", "M-OLD", "First"]);
        fx.ok(&a, &["task", "add", "M-OLD", "Second"]);
        fx.ok(&a, &["task", "block", "M-OLD", "T-02", "T-01"]);
        fx.ok(
            &a,
            &[
                "decision",
                "add",
                "M-OLD",
                "--by",
                "Jim",
                "--task",
                "T-01",
                "--on",
                "2026-01-02",
                "On one",
            ],
        );
        fx.ok(
            &a,
            &[
                "decision",
                "add",
                "M-OLD",
                "--by",
                "Jim",
                "--on",
                "2026-01-02",
                "On none",
            ],
        );
        fx.ok(&a, &["question", "add", "M-OLD", "Why?"]);
        fx.ok(&a, &["task", "add", "M-OTHER", "Wait for M-OLD, then go"]);
        fx.put(&a, "missions/M-OLD-notes.md", "notes\n");
        fx.put(&a, "other/keep.md", "keep\n");
        fx.ok(
            &a,
            &[
                "mission",
                "link",
                "M-OLD",
                "missions/M-OLD-notes.md",
                "--role",
                "intelligence",
            ],
        );
        fx.ok(
            &a,
            &[
                "mission",
                "link",
                "M-OLD",
                "other/keep.md",
                "--role",
                "design",
            ],
        );
        let id = fx.started(&a, &["thread", "start", "M-OLD"]);
        fx.ok(&a, &["ledger", "push", "The mv ledger"]);
        fx.ok(&b, &["ledger", "fetch"]);
        (fx, id)
    }

    /// The internal ID of a mission, as `mission list --json` prints it.
    fn internal_id(&self, dir: &Path, mission: &str) -> String {
        json(&self.ok(dir, &["mission", "list", mission, "--json"]))["uid"]
            .as_str()
            .unwrap_or_default()
            .to_string()
    }
}

#[test]
fn mission_mv_changes_the_code_and_every_record_follows() {
    if !dolt_installed() {
        eprintln!("note: dolt is not installed; skipping");
        return;
    }
    let (fx, thread) = Fixture::mv_ledger();
    let (a, b) = (fx.a(), fx.b());
    let internal = fx.internal_id(&a, "M-OLD");
    assert_eq!(internal.len(), 36, "{}", internal);
    let lines = "mission M-OLD -> M-NEW\n\
                 doc missions/M-OLD-notes.md -> missions/M-NEW-notes.md\n\
                 mentions M-OTHER T-01\n";

    // A dry run prints the lines and changes nothing.
    let dry = fx.tsk(&a, &["mission", "mv", "M-OLD", "M-NEW", "--dry-run"]);
    assert_success(&dry);
    assert_eq!(stdout(&dry), lines);
    assert!(stderr(&dry).contains("dry run"), "{}", stderr(&dry));
    assert!(fx.ok(&a, &["mission", "list"]).contains("M-OLD"));

    // Nothing in the way: a taken ID, a bad ID and an unknown mission are refused.
    for (args, needle) in [
        (["mission", "mv", "M-OLD", "M-OTHER"], "already exists"),
        (["mission", "mv", "M-OLD", "m-new"], "is not a mission ID"),
        (["mission", "mv", "M-NONE", "M-NEW"], "no such mission"),
        (["mission", "mv", "M-OLD", "M-OLD"], "already has the ID"),
    ] {
        let refused = fx.tsk(&a, &args);
        assert_exit(&refused, 1);
        assert!(stderr(&refused).contains(needle), "{}", stderr(&refused));
    }
    // A document that a rename would overwrite is refused and nothing changes.
    fx.put(&a, "missions/M-NEW-notes.md", "in the way\n");
    let refused = fx.tsk(&a, &["mission", "mv", "M-OLD", "M-NEW"]);
    assert_exit(&refused, 1);
    assert!(
        stderr(&refused).contains("which exists"),
        "{}",
        stderr(&refused)
    );
    assert!(fx.ok(&a, &["mission", "list"]).contains("M-OLD"));
    fx.ok(&a, &["ledger", "doc", "rm", "missions/M-NEW-notes.md"]);

    assert_eq!(fx.ok(&a, &["mission", "mv", "M-OLD", "M-NEW"]), lines);
    assert!(stderr(&fx.tsk(&a, &["mission", "list", "M-OLD"])).contains("no such mission"));
    // The internal ID is the one it had, and every record is under the new code.
    assert_eq!(fx.internal_id(&a, "M-NEW"), internal);
    let record = fx.record(&a, "M-NEW");
    assert_eq!(record["name"], "Old name");
    assert_eq!(record["parent"], "M-TOP");
    assert_eq!(record["blocked_by"][0]["id"], "M-DEP");
    assert_eq!(record["tasks"].as_array().unwrap().len(), 2);
    assert_eq!(record["tasks"][1]["blocked_by"][0]["id"], "T-01");
    assert_eq!(record["decisions"][0]["task"], "T-01");
    assert_eq!(record["questions"].as_array().unwrap().len(), 1);
    assert_eq!(
        record["documents"],
        serde_json::json!([
            {"path": "missions/M-NEW-notes.md", "role": "intelligence"},
            {"path": "other/keep.md", "role": "design"}
        ])
    );
    // The others that name it follow, and a text that names it is left as written.
    assert_eq!(fx.record(&a, "M-OTHER")["blocked_by"][0]["id"], "M-NEW");
    assert_eq!(fx.record(&a, "M-KID")["parent"], "M-NEW");
    assert_eq!(
        fx.record(&a, "M-OTHER")["tasks"][0]["description"],
        "Wait for M-OLD, then go"
    );
    assert_eq!(fx.thread_row(&a, &thread)["mission_id"], "M-NEW");
    let docs = fx.ok(&a, &["ledger", "doc", "list"]);
    assert!(docs.contains("missions/M-NEW-notes.md"), "{}", docs);
    assert!(!docs.contains("missions/M-OLD-notes.md"), "{}", docs);
    assert_eq!(
        fx.ok(&a, &["ledger", "doc", "get", "missions/M-NEW-notes.md"]),
        "notes\n"
    );

    // Published, the rename reaches the other clone, whose record is the same.
    fx.ok(&a, &["ledger", "push", "Rename M-OLD"]);
    fx.ok(&b, &["ledger", "fetch"]);
    assert_eq!(fx.record(&b, "M-NEW"), record);
    assert_eq!(fx.internal_id(&b, "M-NEW"), internal);
    assert_eq!(fx.thread_row(&b, &thread)["mission_id"], "M-NEW");
    // A new mission can take the old ID: its internal ID is its own.
    fx.ok(&b, &["mission", "create", "M-OLD", "Reuses the ID"]);
    assert_ne!(fx.internal_id(&b, "M-OLD"), internal);
    assert_eq!(fx.record(&b, "M-NEW")["tasks"].as_array().unwrap().len(), 2);
}

#[test]
fn a_rename_on_one_clone_merges_with_a_task_added_to_the_old_code_on_another() {
    if !dolt_installed() {
        eprintln!("note: dolt is not installed; skipping");
        return;
    }
    let (fx, _thread) = Fixture::mv_ledger();
    let (a, b) = (fx.a(), fx.b());
    fx.ok(&a, &["mission", "mv", "M-OLD", "M-NEW"]);
    fx.ok(
        &b,
        &[
            "task",
            "add",
            "M-OLD",
            "Added while the other clone renamed",
        ],
    );
    fx.ok(&b, &["question", "add", "M-OLD", "Asked meanwhile?"]);
    fx.ok(&a, &["ledger", "push", "Rename"]);
    fx.ok(&b, &["ledger", "push", "Add a task and a question"]);
    fx.ok(&a, &["ledger", "fetch"]);
    for dir in [&a, &b] {
        let tasks = fx.ok(dir, &["task", "list", "M-NEW", "--porcelain"]);
        assert!(
            tasks.contains("Added while the other clone renamed"),
            "{}",
            tasks
        );
        assert!(fx
            .ok(dir, &["question", "list", "M-NEW", "--porcelain"])
            .contains("Asked meanwhile?"));
        assert!(stderr(&fx.tsk(dir, &["mission", "list", "M-OLD"])).contains("no such mission"));
    }
}

#[test]
fn task_mv_moves_the_task_and_its_decisions_and_refuses_blocks() {
    if !dolt_installed() {
        eprintln!("note: dolt is not installed; skipping");
        return;
    }
    let (fx, _thread) = Fixture::mv_ledger();
    let (a, b) = (fx.a(), fx.b());
    let refused = fx.tsk(&a, &["task", "mv", "M-OLD", "T-01", "M-OTHER"]);
    assert_exit(&refused, 1);
    assert!(
        stderr(&refused).contains("T-02 is blocked by T-01"),
        "{}",
        stderr(&refused)
    );
    assert_eq!(
        fx.ok(&a, &["task", "list", "M-OLD", "--porcelain"])
            .lines()
            .count(),
        2
    );

    let moved = fx.ok(&a, &["task", "mv", "M-OLD", "T-01", "M-OTHER", "--force"]);
    assert_eq!(
        moved,
        "dropped block in M-OLD: T-02 is blocked by T-01\n\
         task M-OLD T-01 -> M-OTHER T-02\n\
         decision M-OLD 1 -> M-OTHER 1\n"
    );
    let old = fx.ok(&a, &["task", "list", "M-OLD", "--porcelain"]);
    assert_eq!(old, "T-02\ttodo\t-\tSecond\n");
    let other = fx.ok(&a, &["task", "list", "M-OTHER", "--porcelain"]);
    assert_eq!(
        other,
        "T-01\ttodo\t-\tWait for M-OLD, then go\nT-02\ttodo\t-\tFirst\n"
    );
    assert_eq!(
        fx.ok(&a, &["decision", "list", "M-OLD", "--porcelain"])
            .lines()
            .count(),
        1
    );
    assert_eq!(
        fx.ok(&a, &["decision", "list", "M-OTHER", "--porcelain"]),
        "2026-01-02\tJim\tT-02\tOn one\t-\n"
    );
    // The first mission does not give the number again.
    assert_eq!(fx.ok(&a, &["task", "add", "M-OLD", "Third"]), "T-03\n");

    fx.ok(&a, &["ledger", "push", "Move a task"]);
    fx.ok(&b, &["ledger", "fetch"]);
    assert_eq!(
        fx.ok(&b, &["task", "list", "M-OTHER", "--porcelain"]),
        other
    );
    // A move to the same or an unknown mission is refused.
    for to in ["M-OLD", "M-NONE"] {
        let refused = fx.tsk(&a, &["task", "mv", "M-OLD", "T-02", to]);
        assert_exit(&refused, 1);
    }
}

#[test]
fn decision_mv_and_question_mv_move_by_place_and_by_number() {
    if !dolt_installed() {
        eprintln!("note: dolt is not installed; skipping");
        return;
    }
    let (fx, _thread) = Fixture::mv_ledger();
    let (a, b) = (fx.a(), fx.b());
    assert_eq!(
        fx.ok(&a, &["decision", "mv", "M-OLD", "1", "M-OTHER"]),
        "decision M-OLD 1 -> M-OTHER 1\ntask link cleared (was T-01 of M-OLD)\n"
    );
    assert_eq!(
        fx.ok(&a, &["decision", "list", "M-OLD", "--porcelain"]),
        "2026-01-02\tJim\t-\tOn none\t-\n"
    );
    // The next one is the first of the list now.
    fx.ok(&a, &["task", "add", "M-OTHER", "Target task"]);
    assert_eq!(
        fx.ok(
            &a,
            &["decision", "mv", "M-OLD", "1", "M-OTHER", "--task", "T-02"]
        ),
        "decision M-OLD 1 -> M-OTHER 2\ntask link T-02 of M-OTHER\n"
    );
    let refused = fx.tsk(&a, &["decision", "mv", "M-OLD", "1", "M-OTHER"]);
    assert_exit(&refused, 1);
    assert!(
        stderr(&refused).contains("has 0 decisions"),
        "{}",
        stderr(&refused)
    );

    fx.ok(
        &a,
        &[
            "question", "answer", "M-OLD", "Q-01", "Because", "--by", "Jim",
        ],
    );
    assert_eq!(
        fx.ok(&a, &["question", "mv", "M-OLD", "Q-01", "M-OTHER"]),
        "question M-OLD Q-01 -> M-OTHER Q-01\n"
    );
    let record = fx.record(&a, "M-OTHER");
    assert_eq!(record["questions"][0]["answer"], "Because");
    assert_eq!(record["questions"][0]["status"], "answered");
    assert_eq!(fx.record(&a, "M-OLD")["questions"], serde_json::json!([]));

    fx.ok(&a, &["ledger", "push", "Move records"]);
    fx.ok(&b, &["ledger", "fetch"]);
    assert_eq!(fx.record(&b, "M-OTHER"), record);
}

#[test]
fn doc_mv_renames_the_file_and_every_link_to_it() {
    if !dolt_installed() {
        eprintln!("note: dolt is not installed; skipping");
        return;
    }
    let (fx, _thread) = Fixture::mv_ledger();
    let (a, b) = (fx.a(), fx.b());
    fx.ok(
        &a,
        &[
            "mission",
            "link",
            "M-OTHER",
            "other/keep.md",
            "--role",
            "report",
        ],
    );
    assert_eq!(
        fx.ok(
            &a,
            &["ledger", "doc", "mv", "other/keep.md", "other/kept.md"]
        ),
        "doc other/keep.md -> other/kept.md\nlink M-OLD design\nlink M-OTHER report\n"
    );
    assert_eq!(
        fx.ok(&a, &["ledger", "doc", "get", "other/kept.md"]),
        "keep\n"
    );
    assert!(
        stderr(&fx.tsk(&a, &["ledger", "doc", "get", "other/keep.md"]))
            .contains("no such document")
    );
    assert_eq!(
        fx.record(&a, "M-OTHER")["documents"],
        serde_json::json!([{"path": "other/kept.md", "role": "report"}])
    );
    // A missing document and an existing target are refused.
    for (from, to, needle) in [
        ("other/none.md", "other/x.md", "no such document"),
        ("other/kept.md", "missions/M-OLD-notes.md", "exists"),
        ("other/kept.md", "other/kept.md", "already at"),
    ] {
        let refused = fx.tsk(&a, &["ledger", "doc", "mv", from, to]);
        assert_exit(&refused, 1);
        assert!(stderr(&refused).contains(needle), "{}", stderr(&refused));
    }
    // The push checks that every link names a document, and passes.
    fx.ok(&a, &["ledger", "push", "Move a document"]);
    fx.ok(&b, &["ledger", "fetch"]);
    assert_eq!(
        fx.ok(&b, &["ledger", "doc", "get", "other/kept.md"]),
        "keep\n"
    );
    assert_eq!(
        fx.record(&b, "M-OLD")["documents"][1]["path"],
        "other/kept.md"
    );
}
