//! One set of behaviours, run against both ledger backends. Each case drives the `tsk`
//! binary end to end with `TSK_LEDGER_BACKEND` set, against a bare git repo as origin.
//! The Dolt backend publishes to the same repo through a `git+file` remote. Dolt cases
//! print a note and return when the `dolt` binary is not installed.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use tempfile::TempDir;

const LEDGER_REF: &str = "refs/heads/tsk/ledger";
const DOCS_REF: &str = "refs/heads/tsk/docs";
const BRIEFING: &str = "missions/operational/M-CONF-01-conformance.md";
const CLOUD_VAR: &str = "CLAUDE_CODE_REMOTE_SESSION_ID";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Backend {
    Git,
    Dolt,
}

impl Backend {
    fn name(self) -> &'static str {
        match self {
            Backend::Git => "git",
            Backend::Dolt => "dolt",
        }
    }

    /// Every branch a ledger with documents publishes. The Dolt backend publishes its
    /// documents on a second branch.
    fn published_branches(self) -> Vec<String> {
        let mut refs = vec!["refs/heads/main".to_string(), LEDGER_REF.to_string()];
        if self == Backend::Dolt {
            refs.push(DOCS_REF.to_string());
        }
        refs.sort();
        refs
    }

    /// The documents a new ledger holds before anyone writes one.
    fn seed_documents(self) -> &'static [&'static str] {
        match self {
            Backend::Git => &["index.md"],
            Backend::Dolt => &["README.md", "index.md"],
        }
    }

    /// What `tsk ledger doc list` prints for the seed documents and `extra`, sorted.
    fn listing(self, extra: &[&str]) -> String {
        let mut paths: Vec<&str> = self.seed_documents().to_vec();
        paths.extend_from_slice(extra);
        paths.sort();
        paths.iter().map(|p| format!("{}\n", p)).collect()
    }

    fn available(self) -> bool {
        match self {
            Backend::Git => true,
            Backend::Dolt => dolt_installed(),
        }
    }
}

fn dolt_installed() -> bool {
    Command::new("dolt")
        .arg("version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

struct Fixture {
    root: TempDir,
    backend: Backend,
}

impl Fixture {
    fn new(backend: Backend) -> Fixture {
        let fixture = Fixture {
            root: tempfile::tempdir().unwrap(),
            backend,
        };
        std::fs::create_dir_all(fixture.home()).unwrap();
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

    fn command(&self, dir: &Path, backend: &str, args: &[&str]) -> Command {
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
            .env("HOME", self.home())
            .env("XDG_STATE_HOME", self.state())
            .env("XDG_CONFIG_HOME", self.home().join("config"))
            .env("TSK_HOME", self.home().join(".tsk"))
            .env("TSK_NO_UPDATE_CHECK", "1")
            .env("TSK_LEDGER_BACKEND", backend)
            .env("GIT_CEILING_DIRECTORIES", self.root.path())
            .env_remove(CLOUD_VAR);
        command
    }

    fn tsk(&self, dir: &Path, args: &[&str]) -> Output {
        self.command(dir, self.backend.name(), args)
            .output()
            .expect("failed to run tsk")
    }

    fn tsk_with(&self, dir: &Path, backend: &str, args: &[&str]) -> Output {
        self.command(dir, backend, args)
            .output()
            .expect("failed to run tsk")
    }

    fn tsk_cloud(&self, dir: &Path, session: &str, args: &[&str]) -> Output {
        self.command(dir, self.backend.name(), args)
            .env(CLOUD_VAR, session)
            .output()
            .expect("failed to run tsk")
    }

    fn tsk_stdin(&self, dir: &Path, args: &[&str], input: &str) -> Output {
        let mut child = self
            .command(dir, self.backend.name(), args)
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
                "insert into cloud_session_bindings (session_id, thread_id, registered_at) values ('{}', 't', '2026-01-01T00:00:00Z')",
                session
            ),
        );
    }

    fn binding_count(&self, dir: &Path) -> String {
        self.dolt_sql(dir, "select count(*) as c from cloud_session_bindings")
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

    /// The documents worktree of a clone, which the Dolt backend keeps beside the database.
    fn docs_path(&self, dir: &Path) -> PathBuf {
        self.ledger_path(dir).parent().unwrap().join("docs")
    }

    /// A ledger with the briefing published from clone A.
    fn with_briefing(backend: Backend) -> Fixture {
        let fx = Fixture::new(backend);
        fx.ok(&fx.a(), &["ledger", "fetch"]);
        fx.put(&fx.a(), BRIEFING, "# Mission: conformance\n");
        fx.ok(&fx.a(), &["ledger", "push", "Add the briefing"]);
        fx
    }
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
        let mut command = self.command(dir, self.backend.name(), &["thread", "guard"]);
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
            .command(
                dir,
                self.backend.name(),
                &["thread", "session-start", "--format", format],
            )
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
        json(&self.ok(dir, &["thread", "resume", id]))["latest"].clone()
    }
}

mod cases {
    use super::*;

    pub fn fetch_creates_a_new_ledger_and_the_first_push_creates_the_branch(b: Backend) {
        let fx = Fixture::new(b);
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
        assert_eq!(fx.remote_branches(), b.published_branches());

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
            b.listing(&["missions/M-X.md"])
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

    pub fn documents_round_trip_between_clones(b: Backend) {
        let fx = Fixture::new(b);
        let (a, other) = (fx.a(), fx.b());
        fx.ok(&a, &["ledger", "fetch"]);
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
            b.listing(&["missions/a.md", "missions/b.md", "notes/n.md"])
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

    pub fn threads_start_pause_resume_list_detach_and_stop(b: Backend) {
        let fx = Fixture::with_briefing(b);
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
        assert_eq!(paused, format!("paused:{}\n", id));
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

    pub fn cloud_sessions_bind_through_the_ledger(b: Backend) {
        let fx = Fixture::with_briefing(b);
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
        fx.ok(&a, &["ledger", "fetch"]);
        assert_exit(&fx.tsk_cloud(&a, "cse_two", &["thread", "binding"]), 1);
    }

    pub fn events_append_read_new_and_advance_the_watermark(b: Backend) {
        let fx = Fixture::new(b);
        let (a, other) = (fx.a(), fx.b());
        fx.ok(&a, &["ledger", "fetch"]);
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

    pub fn a_stale_push_is_merged_when_different_rows_changed(b: Backend) {
        let fx = Fixture::new(b);
        let (a, other) = (fx.a(), fx.b());
        fx.ok(&a, &["ledger", "fetch"]);
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
            b.listing(&["base.md", "from-a.md", "from-b.md"])
        );
        assert_eq!(fx.ok(&other, &["ledger", "doc", "get", "from-a.md"]), "a\n");
        assert!(!pushed.is_empty());
    }

    pub fn a_same_row_conflict_stops_and_leaves_the_remote_unchanged(b: Backend) {
        let fx = Fixture::new(b);
        let (a, other) = (fx.a(), fx.b());
        fx.ok(&a, &["ledger", "fetch"]);
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
        if b == Backend::Dolt {
            // The documents are files on their own branch: the conflict names the file.
            assert!(message.contains("shared.md"), "{}", message);
            assert!(message.contains("refs/heads/tsk/docs"), "{}", message);
        }
        assert_eq!(fx.remote_ledger(), before);
    }

    pub fn doc_path_prints_the_file_and_bad_paths_are_refused(b: Backend) {
        let fx = Fixture::new(b);
        let a = fx.a();
        fx.ok(&a, &["ledger", "fetch"]);
        fx.put(&a, "missions/p.md", "x\n");

        let printed = fx.ok(&a, &["ledger", "doc", "path", "missions/p.md"]);
        let file = PathBuf::from(printed.trim_end());
        assert!(file.is_absolute(), "{}", printed);
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "x\n");
        // The file sits in the worktree of the store that holds documents: the ledger
        // worktree for the file ledger, the documents worktree for Dolt.
        let root = match b {
            Backend::Git => fx.ledger_path(&a),
            Backend::Dolt => fx.docs_path(&a),
        };
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

    pub fn documents_that_the_ledger_does_not_reserve_follow_the_backend(b: Backend) {
        let fx = Fixture::new(b);
        let a = fx.a();
        fx.ok(&a, &["ledger", "fetch"]);
        // The file ledger keeps threads/ for itself. Documents on the docs branch have
        // no reserved names.
        let put = fx.tsk_stdin(&a, &["ledger", "doc", "put", "threads/note.md"], "n\n");
        match b {
            Backend::Git => assert_exit(&put, 1),
            Backend::Dolt => {
                assert_success(&put);
                assert_eq!(
                    fx.ok(&a, &["ledger", "doc", "get", "threads/note.md"]),
                    "n\n"
                );
            }
        }
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

    pub fn missions_are_created_and_listed_one_per_line(b: Backend) {
        let fx = Fixture::new(b);
        let a = fx.a();
        fx.ok(&a, &["ledger", "fetch"]);
        assert_eq!(fx.ok(&a, &["mission", "list"]), "");
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
            fx.ok(&a, &["mission", "list"]),
            "M-A1\ttodo\t-\t-\tFirst one\n\
             M-ADMIN\tin-progress\t-\tM-A1\tStanding work\n\
             M-B-TWO\ttodo\t-\t-\tSecond\n"
        );
        assert_eq!(
            fx.ok(&a, &["mission", "list", "M-ADMIN"]),
            "M-ADMIN\tin-progress\t-\tM-A1\tStanding work\n"
        );

        let before = fx.ok(&a, &["mission", "list"]);
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
            &["mission", "list", "M-NONE"],
            "no such mission: M-NONE",
        );
        assert_eq!(fx.ok(&a, &["mission", "list"]), before);
    }

    pub fn every_mission_field_round_trips(b: Backend) {
        let fx = Fixture::new(b);
        let (a, other) = (fx.a(), fx.b());
        fx.ok(&a, &["ledger", "fetch"]);
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

    pub fn status_accepts_three_values_and_an_administrative_mission_keeps_one(b: Backend) {
        let fx = Fixture::new(b);
        let a = fx.a();
        fx.ok(&a, &["ledger", "fetch"]);
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

    pub fn blocks_are_checked_and_blocked_is_derived(b: Backend) {
        let fx = Fixture::new(b);
        let (a, other) = (fx.a(), fx.b());
        fx.ok(&a, &["ledger", "fetch"]);
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
        let listed = |fx: &Fixture, dir: &Path| fx.ok(dir, &["mission", "list"]);
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

    pub fn objective_outcomes_are_numbered_and_numbers_are_not_reused(b: Backend) {
        let fx = Fixture::new(b);
        let a = fx.a();
        fx.ok(&a, &["ledger", "fetch"]);
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

    pub fn documents_are_linked_by_path_and_checked_at_push(b: Backend) {
        let fx = Fixture::new(b);
        let a = fx.a();
        fx.ok(&a, &["ledger", "fetch"]);
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
        if b == Backend::Dolt {
            // The documents had already gone: the removal is on the branch.
            let files = fx.origin_files(DOCS_REF);
            assert!(
                !files.iter().any(|f| f == "missions/M-D/report.md"),
                "{:?}",
                files
            );
        }

        fx.ok(&a, &["mission", "unlink", "M-D", "missions/M-D/report.md"]);
        assert_success(&fx.tsk(&a, &["ledger", "push", "Unlink it"]));
        let other = fx.b();
        fx.ok(&other, &["ledger", "fetch"]);
        assert_eq!(
            mission_json(&fx, &other, "M-D")["documents"],
            serde_json::json!([])
        );
    }

    pub fn mission_writes_stay_local_until_the_push(b: Backend) {
        let fx = Fixture::new(b);
        let (a, other) = (fx.a(), fx.b());
        fx.ok(&a, &["ledger", "fetch"]);
        fx.ok(&a, &["mission", "create", "M-BASE", "Base"]);
        fx.ok(&a, &["ledger", "push", "Base"]);
        fx.ok(&other, &["ledger", "fetch"]);
        let ledger = fx.remote_ledger();

        fx.ok(&a, &["mission", "create", "M-NEW", "New"]);
        fx.ok(&a, &["mission", "set", "M-BASE", "purpose", "changed"]);
        assert_eq!(fx.remote_ledger(), ledger);
        assert_eq!(
            fx.ok(&other, &["mission", "list"]),
            "M-BASE\ttodo\t-\t-\tBase\n"
        );
        fx.ok(&other, &["ledger", "fetch"]);
        assert_eq!(
            fx.ok(&other, &["mission", "list"]),
            "M-BASE\ttodo\t-\t-\tBase\n"
        );
        assert_eq!(mission_json(&fx, &other, "M-BASE")["purpose"], "");

        fx.ok(&a, &["ledger", "push", "Publish"]);
        assert_ne!(fx.remote_ledger(), ledger);
        fx.ok(&other, &["ledger", "fetch"]);
        assert_eq!(
            fx.ok(&other, &["mission", "list"]),
            "M-BASE\ttodo\t-\t-\tBase\nM-NEW\ttodo\t-\t-\tNew\n"
        );
        assert_eq!(mission_json(&fx, &other, "M-BASE")["purpose"], "changed");
    }

    pub fn two_clones_add_missions_that_merge_and_one_field_conflicts(b: Backend) {
        let fx = Fixture::new(b);
        let (a, other) = (fx.a(), fx.b());
        fx.ok(&a, &["ledger", "fetch"]);
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
        assert_eq!(fx.ok(&a, &["mission", "list"]), expected);
        fx.ok(&other, &["ledger", "fetch"]);
        assert_eq!(fx.ok(&other, &["mission", "list"]), expected);

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
        match b {
            Backend::Dolt => assert!(message.contains("missions"), "{}", message),
            Backend::Git => assert!(
                message.contains("records/missions/M-BASE.json"),
                "{}",
                message
            ),
        }
        assert_eq!(fx.remote_ledger(), before);
    }

    pub fn the_json_record_has_every_field(b: Backend) {
        let fx = Fixture::new(b);
        let a = fx.a();
        fx.ok(&a, &["ledger", "fetch"]);
        fx.ok(&a, &["mission", "create", "M-P", "Parent"]);
        fx.ok(&a, &["mission", "create", "M-Q", "Blocker"]);
        fx.ok(&a, &["mission", "create", "M-J", "Json", "--parent", "M-P"]);
        fx.ok(&a, &["mission", "set", "M-J", "purpose", "Why"]);
        fx.ok(
            &a,
            &["mission", "constraint", "add", "M-J", "No new crates"],
        );
        fx.ok(
            &a,
            &["mission", "constraint", "add", "M-J", "Both backends"],
        );
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
            json(&one),
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
                    {"n": 2, "text": "Both backends"}
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

    fn mission_with_tasks(b: Backend, tasks: usize) -> (Fixture, PathBuf) {
        let fx = Fixture::new(b);
        let a = fx.a();
        fx.ok(&a, &["ledger", "fetch"]);
        fx.ok(&a, &["mission", "create", "M-T", "Tasks"]);
        for i in 1..=tasks {
            assert_eq!(
                fx.ok(&a, &["task", "add", "M-T", &format!("Task {}", i)]),
                format!("T-{:02}\n", i)
            );
        }
        (fx, a)
    }

    pub fn tasks_are_minted_per_mission_listed_and_changed(b: Backend) {
        let (fx, a) = mission_with_tasks(b, 0);
        fx.ok(&a, &["mission", "create", "M-U", "Other"]);
        assert_eq!(fx.ok(&a, &["task", "list", "M-T"]), "");
        // The counter is per mission and only grows.
        for i in 1..=12 {
            assert_eq!(
                fx.ok(&a, &["task", "add", "M-T", &format!("Task {}", i)]),
                format!("T-{:02}\n", i)
            );
        }
        assert_eq!(fx.ok(&a, &["task", "add", "M-U", "First of U"]), "T-01\n");
        assert_eq!(fx.ok(&a, &["task", "add", "M-T", "-dash"]), "T-13\n");
        let list = fx.ok(&a, &["task", "list", "M-T"]);
        assert_eq!(list.lines().count(), 13);
        assert_eq!(list.lines().next().unwrap(), "T-01\ttodo\t-\tTask 1");
        assert_eq!(list.lines().last().unwrap(), "T-13\ttodo\t-\t-dash");
        assert_eq!(
            fx.ok(&a, &["task", "list", "M-U"]),
            "T-01\ttodo\t-\tFirst of U\n"
        );

        // Status takes three values.
        for status in ["in-progress", "done", "todo"] {
            fx.ok(&a, &["task", "set", "M-T", "T-02", "status", status]);
            assert_eq!(
                fx.ok(&a, &["task", "list", "M-T"]).lines().nth(1).unwrap(),
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
            fx.ok(&a, &["task", "list", "M-T"]).lines().nth(1).unwrap(),
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
            &["task", "list", "M-NONE"],
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
            fx.ok(&other, &["task", "list", "M-T"]),
            fx.ok(&a, &["task", "list", "M-T"])
        );
        assert_eq!(fx.ok(&other, &["task", "add", "M-T", "From B"]), "T-15\n");
    }

    pub fn task_blocks_are_checked_and_blocked_is_derived(b: Backend) {
        let (fx, a) = mission_with_tasks(b, 3);
        fx.ok(&a, &["mission", "create", "M-U", "Other"]);
        fx.ok(&a, &["task", "add", "M-U", "In U"]);

        assert_eq!(fx.ok(&a, &["task", "block", "M-T", "T-01", "T-02"]), "");
        fx.ok(&a, &["task", "block", "M-T", "T-01", "T-03"]);
        fx.ok(&a, &["task", "block", "M-T", "T-02", "T-03"]);
        assert_eq!(
            fx.ok(&a, &["task", "list", "M-T"]),
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
            fx.ok(&a, &["task", "list", "M-T"]),
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
        assert!(fx.ok(&a, &["task", "list", "M-T"]).contains("T-02,T-03"));

        let before = fx.ok(&a, &["task", "list", "M-T"]);
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
        assert_eq!(fx.ok(&a, &["task", "list", "M-T"]), before);

        assert_eq!(fx.ok(&a, &["task", "unblock", "M-T", "T-01", "T-03"]), "");
        assert!(fx
            .ok(&a, &["task", "list", "M-T"])
            .starts_with("T-01\ttodo\tT-02\tTask 1\n"));
        fx.ok(&a, &["task", "unblock", "M-T", "T-01", "T-02"]);
        assert!(fx
            .ok(&a, &["task", "list", "M-T"])
            .starts_with("T-01\ttodo\t-\tTask 1\n"));
        // With the chain cut, the reverse block is allowed.
        fx.ok(&a, &["task", "unblock", "M-T", "T-02", "T-03"]);
        fx.ok(&a, &["task", "block", "M-T", "T-03", "T-01"]);

        // Blocks survive a push to another clone.
        fx.ok(&a, &["ledger", "push", "Blocks"]);
        let other = fx.b();
        fx.ok(&other, &["ledger", "fetch"]);
        assert!(fx
            .ok(&other, &["task", "list", "M-T"])
            .contains("T-03\tin-progress\tT-01\tTask 3"));
    }

    pub fn decisions_are_recorded_with_who_when_and_an_optional_task(b: Backend) {
        let (fx, a) = mission_with_tasks(b, 2);
        fx.ok(&a, &["mission", "create", "M-U", "Other"]);
        fx.ok(&a, &["task", "add", "M-U", "In U"]);
        assert_eq!(fx.ok(&a, &["decision", "list", "M-T"]), "");
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
        let rows = lines(&fx.ok(&a, &["decision", "list", "M-T"]));
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
        assert_eq!(fx.ok(&a, &["decision", "list", "M-U"]), "");

        let before = fx.ok(&a, &["decision", "list", "M-T"]);
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
        fails(&fx, &a, &["decision", "list", "M-NONE"], "M-NONE");
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
        assert_eq!(fx.ok(&a, &["decision", "list", "M-T"]), before);

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
        assert_eq!(fx.ok(&other, &["decision", "list", "M-T"]), before);
        fx.ok(&other, &["decision", "add", "M-T", "From B", "--by", "Jim"]);
        assert_eq!(
            fx.ok(&other, &["decision", "list", "M-T"]).lines().count(),
            4
        );
    }

    pub fn questions_are_minted_answered_and_listed_while_open(b: Backend) {
        let (fx, a) = mission_with_tasks(b, 0);
        fx.ok(&a, &["mission", "create", "M-U", "Other"]);
        assert_eq!(fx.ok(&a, &["question", "list"]), "");
        assert_eq!(fx.ok(&a, &["question", "add", "M-T", "Why one?"]), "Q-01\n");
        assert_eq!(fx.ok(&a, &["question", "add", "M-T", "Why two?"]), "Q-02\n");
        assert_eq!(
            fx.ok(&a, &["question", "add", "M-U", "Why in U?"]),
            "Q-01\n"
        );
        assert_eq!(fx.ok(&a, &["question", "add", "M-T", "-dash?"]), "Q-03\n");

        let rows = lines(&fx.ok(&a, &["question", "list"]));
        assert_eq!(rows.len(), 4);
        // Across all missions, by mission and then number.
        let keys: Vec<String> = rows.iter().map(|r| format!("{} {}", r[0], r[1])).collect();
        assert_eq!(keys, vec!["M-T Q-01", "M-T Q-02", "M-T Q-03", "M-U Q-01"]);
        for row in &rows {
            assert_eq!(row.len(), 4);
            assert!(is_date(&row[2]), "{:?}", row);
        }
        assert_eq!(rows[0][3], "Why one?");
        assert_eq!(fx.ok(&a, &["question", "list", "M-U"]).lines().count(), 1);

        assert_eq!(
            fx.ok(
                &a,
                &["question", "answer", "M-T", "Q-02", "Because.", "--by", "Jim"]
            ),
            ""
        );
        let open = fx.ok(&a, &["question", "list", "M-T"]);
        assert_eq!(open.lines().count(), 2);
        assert!(!open.contains("Why two?"));
        assert_eq!(fx.ok(&a, &["question", "list"]).lines().count(), 3);
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
        fails(&fx, &a, &["question", "list", "M-NONE"], "M-NONE");
        // The refused answer left Q-01 open, and nothing was minted.
        assert_eq!(fx.ok(&a, &["question", "list", "M-T"]), open);
        assert_eq!(fx.ok(&a, &["question", "add", "M-T", "Next"]), "Q-04\n");

        fx.ok(&a, &["ledger", "push", "Questions"]);
        let other = fx.b();
        fx.ok(&other, &["ledger", "fetch"]);
        assert_eq!(
            fx.ok(&other, &["question", "list"]),
            fx.ok(&a, &["question", "list"])
        );
    }

    pub fn two_clones_add_records_to_different_missions_and_the_ledgers_merge(b: Backend) {
        let fx = Fixture::new(b);
        let (a, other) = (fx.a(), fx.b());
        fx.ok(&a, &["ledger", "fetch"]);
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
                fx.ok(dir, &["task", "list", "M-ONE"]),
                "T-01\ttodo\t-\tFrom A\n"
            );
            assert_eq!(
                fx.ok(dir, &["task", "list", "M-TWO"]),
                "T-01\ttodo\t-\tFrom B\n"
            );
            assert_eq!(fx.ok(dir, &["question", "list"]).lines().count(), 1);
            assert_eq!(
                fx.ok(dir, &["decision", "list", "M-TWO"]).lines().count(),
                1
            );
        }
    }

    pub fn thread_start_takes_a_mission_id_and_checks_it_against_records_and_briefings(b: Backend) {
        let fx = Fixture::with_briefing(b);
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

    pub fn thread_pause_checks_the_task_against_the_mission_record(b: Backend) {
        let fx = Fixture::new(b);
        let a = fx.a();
        fx.ok(&a, &["ledger", "fetch"]);
        fx.ok(&a, &["mission", "create", "M-REC", "Recorded"]);
        fx.ok(&a, &["task", "add", "M-REC", "First task"]);
        fx.put(&a, "missions/operational/M-LEG-legacy.md", "# Legacy\n");
        fx.ok(&a, &["ledger", "push", "Add the missions"]);

        let id = fx.started(&a, &["thread", "start", "M-REC"]);

        // A task of the mission is accepted, and T-1 is T-01.
        assert_eq!(
            fx.ok(&a, &["thread", "pause", &id, "T-1", "first"]),
            format!("paused:{}\n", id)
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
        assert_eq!(stdout(&old), format!("paused:{}\n", id));
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
        match fx.backend {
            Backend::Dolt => {
                fx.dolt_sql(
                    dir,
                    &format!(
                        "insert into threads (thread_id, briefing_path) values ('{}', '{}'); \
                         insert into continuation_entries (thread_id, seq, entry) values ('{}', 1, '{}')",
                        id, BRIEFING, id, entry
                    ),
                );
            }
            Backend::Git => {
                let thread = fx.ledger_path(dir).join("threads").join(id);
                std::fs::create_dir_all(&thread).unwrap();
                write(
                    &thread.join("index.md"),
                    &format!(
                        "# Thread {}\n\nMission briefing: [{}]({})\n",
                        id, BRIEFING, BRIEFING
                    ),
                );
                write(
                    &thread.join("continuation-state.jsonl"),
                    &format!("{}\n", entry),
                );
            }
        }
        fx.ok(dir, &["ledger", "push", "Seed a legacy thread"]);
    }

    pub fn legacy_thread_rows_read_with_a_mission_id_derived_from_the_briefing_path(b: Backend) {
        let fx = Fixture::with_briefing(b);
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
        if b == Backend::Dolt {
            assert_eq!(
                fx.dolt_sql(
                    &a,
                    "select mission_id from threads where thread_id = 'aaaa1111'"
                ),
                "mission_id\n\n"
            );
        } else {
            let index =
                std::fs::read_to_string(fx.ledger_path(&a).join("threads/aaaa1111/index.md"))
                    .unwrap();
            assert!(!index.contains("Mission: "), "{}", index);
        }
        // Another clone reads the same row.
        let other = fx.b();
        fx.ok(&other, &["ledger", "fetch"]);
        assert_eq!(fx.thread_row(&other, "aaaa1111")["mission_id"], "M-CONF-01");
    }

    pub fn the_stop_hook_check_passes_when_the_ledger_has_no_missions(b: Backend) {
        let fx = Fixture::new(b);
        let a = fx.a();
        fx.ok(&a, &["ledger", "fetch"]);
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

    pub fn a_mission_document_alone_makes_the_stop_hook_check_block(b: Backend) {
        let fx = Fixture::with_briefing(b);
        let a = fx.a();
        let reason = fx.guard_reason(&a, None).expect("blocks");
        assert!(reason.contains("index.md"), "{}", reason);
        assert!(reason.contains("`tsk thread skip`"), "{}", reason);
    }

    pub fn skip_silences_the_stop_hook_check_until_a_thread_starts_or_resumes(b: Backend) {
        let fx = Fixture::with_briefing(b);
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

        // So does resume.
        fx.ok(&a, &["thread", "skip"]);
        assert_eq!(fx.guard_reason(&a, None), None);
        fx.ok(&a, &["thread", "resume", &id]);
        assert_eq!(fx.guard_reason(&a, None), None);
        fx.ok(&a, &["thread", "detach"]);
        assert!(fx.guard_reason(&a, None).is_some());

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

    pub fn session_start_names_the_bound_mission_with_its_open_tasks(b: Backend) {
        let fx = Fixture::new(b);
        let a = fx.a();
        fx.ok(&a, &["ledger", "fetch"]);
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

    pub fn thread_commands_carry_local_record_and_document_changes_to_the_remote(b: Backend) {
        let fx = Fixture::new(b);
        let (a, other) = (fx.a(), fx.b());
        fx.ok(&a, &["ledger", "fetch"]);

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
            fx.ok(&a, &["task", "list", "M-REC"]),
            "T-01\ttodo\t-\tFirst\n"
        );

        // Thread list reads with them.
        assert_eq!(fx.ok(&a, &["thread", "list"]), "");

        // Start carries them: one push, and the remote has the records and the document.
        let id = fx.started(&a, &["thread", "start", "M-REC"]);
        fx.ok(&other, &["ledger", "fetch"]);
        assert_eq!(
            fx.ok(&other, &["task", "list", "M-REC"]),
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
        assert_eq!(fx.ok(&other, &["task", "list", "M-REC"]).lines().count(), 2);
        assert_eq!(fx.latest_entry(&other, &id)["task_id"], "T-02");

        // Session start with the changes pending again, and a detach that does not refuse
        // them. A worktree detach pushes nothing: the changes wait for the next push.
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
        assert!(fx.ok(&other, &["mission", "list"]).contains("M-TWO"));
        // `ledger fetch` itself still refuses uncommitted changes.
        fx.ok(&a, &["mission", "create", "M-THREE", "Three"]);
        assert_exit(&fx.tsk(&a, &["ledger", "fetch"]), 1);
    }

    pub fn session_start_keeps_the_index_hint_for_a_bound_mission_with_no_record(b: Backend) {
        let fx = Fixture::with_briefing(b);
        let a = fx.a();
        let id = fx.started(&a, &["thread", "start", "M-CONF-01"]);
        let context = fx.session_context(&a, "s-legacy", "claude");
        assert!(context.contains("index.md"), "{}", context);
        assert!(
            !context.contains("This thread works mission"),
            "{}",
            context
        );
        assert!(context.contains(&format!("worktree:{}", id)), "{}", context);
    }

    pub fn session_start_shows_the_empty_ledger_note_only_without_records_and_documents(
        b: Backend,
    ) {
        let fx = Fixture::new(b);
        let a = fx.a();
        fx.ok(&a, &["ledger", "fetch"]);
        let empty = fx.session_context(&a, "s-empty", "claude");
        assert!(empty.contains("run `tsk guide import`"), "{}", empty);
        // Without records the unbound prompt stays as it was, and gains the skip choice.
        assert!(empty.contains("index.md"), "{}", empty);
        assert!(empty.contains("`tsk thread skip`"), "{}", empty);

        fx.ok(&a, &["mission", "create", "M-REC", "Recorded"]);
        fx.ok(&a, &["ledger", "push", "Add a record"]);
        let with_record = fx.session_context(&a, "s-record", "claude");
        assert!(!with_record.contains("tsk guide import"), "{}", with_record);
    }

    pub fn session_start_says_a_skipped_session_works_without_a_thread(b: Backend) {
        let fx = Fixture::with_briefing(b);
        let a = fx.a();
        fx.ok(&a, &["thread", "skip"]);
        let context = fx.session_context(&a, "s-skip", "claude");
        assert!(context.contains("works without a thread"), "{}", context);
        assert!(!context.contains("No thread binding"), "{}", context);
    }

    /// Every stored continuation entry of a thread, oldest first, as the ledger holds them.
    fn stored_entries(fx: &Fixture, dir: &Path, id: &str) -> Vec<serde_json::Value> {
        match fx.backend {
            Backend::Git => {
                let file = fx
                    .ledger_path(dir)
                    .join("threads")
                    .join(id)
                    .join("continuation-state.jsonl");
                std::fs::read_to_string(file)
                    .unwrap()
                    .lines()
                    .map(json)
                    .collect()
            }
            Backend::Dolt => {
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
        }
    }

    /// True when the ledger stores no mission for the thread: SQL NULL on Dolt, no
    /// `Mission:` line on the file ledger.
    fn stores_no_mission(fx: &Fixture, dir: &Path, id: &str) -> bool {
        match fx.backend {
            Backend::Dolt => {
                let counted = fx.dolt_sql(
                    dir,
                    &format!(
                        "select count(*) as c from threads where thread_id = '{}' and mission_id is null and briefing_path is null",
                        id
                    ),
                );
                counted.lines().nth(1) == Some("1")
            }
            Backend::Git => {
                let index = fx
                    .ledger_path(dir)
                    .join("threads")
                    .join(id)
                    .join("index.md");
                let text = std::fs::read_to_string(index).unwrap();
                !text.contains("Mission")
            }
        }
    }

    const NO_MISSION_ERROR: &str = "has no mission, so it has no tasks. Attach a mission with tsk thread set mission <mission-id>, or pass an empty task ID";

    pub fn a_thread_can_start_with_no_mission_and_pause_with_an_empty_task(b: Backend) {
        let fx = Fixture::with_briefing(b);
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
            format!("paused:{}\n", id)
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

    pub fn thread_set_mission_attaches_changes_and_detaches_a_mission(b: Backend) {
        let fx = Fixture::with_briefing(b);
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
        assert_ne!(fx.remote_ledger(), before);
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

        // Another clone sees the change.
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

    pub fn session_start_says_a_bound_thread_with_no_mission_has_no_tasks(b: Backend) {
        let fx = Fixture::new(b);
        let a = fx.a();
        fx.ok(&a, &["ledger", "fetch"]);
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
    fn mission_change(b: Backend, id: &str) -> String {
        match b {
            Backend::Dolt => "data\tmissions\tadded 1\tmodified 0\tdeleted 0".to_string(),
            Backend::Git => format!("data\tA\trecords/missions/{}.json", id),
        }
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

    pub fn status_and_discard_report_and_drop_local_changes(b: Backend) {
        let fx = Fixture::with_briefing(b);
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
        match b {
            Backend::Dolt => {
                expected.push("data\tmissions\tadded 1\tmodified 0\tdeleted 0".to_string());
                expected.push("data\ttasks\tadded 1\tmodified 0\tdeleted 0".to_string());
            }
            Backend::Git => expected.push(mission_change(b, "M-LOCAL")),
        }
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
        assert!(fx.ok(&a, &["mission", "list"]).contains("M-LOCAL"));

        // The other clone never saw any of it.
        fx.ok(&other, &["ledger", "fetch"]);
        assert_eq!(fx.ok(&other, &["mission", "list"]), "");
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
        assert!(fx.ok(&a, &["mission", "list"]).contains("M-LOCAL"));
        assert_eq!(
            fx.ok(&a, &["ledger", "doc", "get", "local-note.md"]),
            "local\n"
        );

        assert_eq!(fx.ok(&a, &["ledger", "discard", "--yes"]), "discarded\n");
        assert_eq!(status_lines(&fx, &a), clean_status("up to date"));
        assert_eq!(fx.ok(&a, &["mission", "list"]), "");
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
        assert!(fx.ok(&other, &["mission", "list"]).contains("M-AFTER"));
    }

    pub fn status_counts_commits_ahead_and_behind_origin(b: Backend) {
        let fx = Fixture::with_briefing(b);
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
        assert!(lines.contains(&mission_change(b, "M-AHEAD")), "{:?}", lines);
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
        assert!(lines.contains(&mission_change(b, "M-AHEAD")), "{:?}", lines);
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
        let missions = fx.ok(&a, &["mission", "list"]);
        assert!(!missions.contains("M-AHEAD"), "{}", missions);
        assert!(missions.contains("M-OTHER-2"), "{}", missions);
        assert_exit(&fx.get(&a, "ahead.md"), 1);
        assert_eq!(
            fx.ok(&a, &["ledger", "doc", "get", "other-2.md"]),
            "other 2\n"
        );
    }

    pub fn status_works_offline_and_discard_then_changes_nothing(b: Backend) {
        let fx = Fixture::with_briefing(b);
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
        assert!(lines.contains(&mission_change(b, "M-OFF")), "{:?}", lines);
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
        assert!(fx.ok(&a, &["mission", "list"]).contains("M-OFF"));
        assert_eq!(fx.ok(&a, &["ledger", "doc", "get", "off.md"]), "off\n");

        std::fs::rename(&moved, fx.origin()).unwrap();
        assert_eq!(
            status_lines(&fx, &a).last().unwrap(),
            "docs\tremote\tup to date"
        );
    }

    fn human(fx: &Fixture, dir: &Path) -> Vec<String> {
        let out = fx.ok(dir, &["ledger", "status"]);
        assert!(!out.contains('\t'), "{:?}", out);
        for line in out.lines() {
            assert_eq!(line, line.trim_end(), "{:?}", line);
        }
        out.lines().map(str::to_string).collect()
    }

    /// The Local block for one mission record and one document `h.md`, written locally.
    fn local_block(b: Backend, extra_commits: Option<&str>) -> Vec<String> {
        let mut lines = vec!["Local   2 changes not pushed".to_string()];
        match b {
            Backend::Dolt => {
                lines.push("          data  missions  1 added".to_string());
                lines.push("          docs  h.md      added".to_string());
            }
            Backend::Git => {
                lines.push("          data  records/missions/M-H.json  added".to_string());
                lines.push("          docs  h.md                       added".to_string());
            }
        }
        if let Some(plus) = extra_commits {
            lines.push(format!("        plus {} not pushed", plus));
        }
        lines
    }

    fn origin_line(b: Backend, dolt: &str, git: &str) -> String {
        match b {
            Backend::Dolt => format!("Origin  {}", dolt),
            Backend::Git => format!("Origin  {}", git),
        }
    }

    pub fn status_prints_local_and_origin_in_plain_columns(b: Backend) {
        // A ledger that origin does not hold yet.
        let fresh = Fixture::new(b);
        fresh.ok(&fresh.a(), &["ledger", "fetch"]);
        assert_eq!(
            human(&fresh, &fresh.a()),
            vec![
                "Local   nothing to push",
                "Origin  not published yet (tsk ledger push creates it)"
            ]
        );

        let fx = Fixture::with_briefing(b);
        let (a, other) = (fx.a(), fx.b());
        fx.ok(&other, &["ledger", "fetch"]);
        assert_eq!(
            human(&fx, &a),
            vec!["Local   nothing to push", "Origin  up to date"]
        );

        // Changes not pushed; the discard preview prints the Local block alone.
        fx.ok(&a, &["mission", "create", "M-H", "Human"]);
        fx.put(&a, "h.md", "h\n");
        let mut expected = local_block(b, None);
        expected.push("Origin  up to date".to_string());
        assert_eq!(human(&fx, &a), expected);
        let dry = fx.tsk(&a, &["ledger", "discard"]);
        assert_exit(&dry, 1);
        assert_eq!(
            stdout(&dry).lines().collect::<Vec<_>>(),
            local_block(b, None)
        );
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
                origin_line(
                    b,
                    "1 data commit and 1 document commit to fetch (tsk ledger fetch)",
                    "1 ledger commit to fetch (tsk ledger fetch)"
                )
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
        let plus = match b {
            Backend::Dolt => "2 commits",
            Backend::Git => "1 commit",
        };
        let mut expected = local_block(b, Some(plus));
        expected.push(origin_line(
            b,
            "1 data commit and 1 document commit not on origin (tsk ledger push)",
            "1 ledger commit not on origin (tsk ledger push)",
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
                b,
                "diverged: 1 data commit and 1 document commit to push, 1 data commit and 1 document commit to fetch (tsk ledger push merges them)",
                "diverged: 1 ledger commit to push, 1 to fetch (tsk ledger push merges them)"
            )
        );
        fx.ok(&a, &["ledger", "discard", "--yes"]);

        // Offline: origin cannot be reached.
        let moved = fx.root.path().join("origin-away.git");
        std::fs::rename(fx.origin(), &moved).unwrap();
        let out = fx.tsk(&a, &["ledger", "status"]);
        assert_success(&out);
        assert_eq!(
            stdout(&out),
            "Local   nothing to push\nOrigin  unknown (could not reach origin)\n"
        );
        assert_eq!(stderr(&out).lines().count(), 1, "{}", stderr(&out));
        assert!(stderr(&out).starts_with("note: could not fetch "));
        std::fs::rename(&moved, fx.origin()).unwrap();
    }

    pub fn status_and_discard_need_a_ledger_store(b: Backend) {
        let fx = Fixture::new(b);
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

    pub fn a_decision_takes_a_date_with_on_and_refuses_one_that_is_not_real(b: Backend) {
        let (fx, a) = mission_with_tasks(b, 1);
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
        let rows = lines(&fx.ok(&a, &["decision", "list", "M-T"]));
        assert_eq!(rows[0], ["2026-02-27", "Jim", "-", "Old call", "-"]);
        assert_eq!(rows[1], ["2024-02-29", "Jim", "T-01", "Leap day", "-"]);
        assert!(is_date(&rows[2][0]), "{:?}", rows[2]);
        assert_ne!(rows[2][0], "2026-02-27");

        let before = fx.ok(&a, &["decision", "list", "M-T"]);
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
        assert_eq!(fx.ok(&a, &["decision", "list", "M-T"]), before);
        // The date survives a push and a fetch on the other clone.
        fx.ok(&a, &["ledger", "push", "Decisions"]);
        let other = fx.b();
        fx.ok(&other, &["ledger", "fetch"]);
        assert_eq!(fx.ok(&other, &["decision", "list", "M-T"]), before);
        let record = mission_json(&fx, &other, "M-T");
        assert_eq!(record["decisions"][0]["decided_on"], "2026-02-27");
    }

    pub fn an_open_question_is_edited_and_an_answered_one_is_not(b: Backend) {
        let (fx, a) = mission_with_tasks(b, 0);
        fx.ok(&a, &["mission", "create", "M-U", "Other"]);
        fx.ok(&a, &["question", "add", "M-T", "Why one?"]);
        fx.ok(&a, &["question", "add", "M-T", "Why two?"]);
        fx.ok(&a, &["question", "add", "M-U", "Why in U?"]);
        let before = lines(&fx.ok(&a, &["question", "list"]));
        assert_eq!(
            fx.ok(&a, &["question", "edit", "M-T", "Q-01", "Why, then?"]),
            ""
        );
        // Only the text changes: the list keeps its columns, order and dates.
        let after = lines(&fx.ok(&a, &["question", "list"]));
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

    pub fn decisions_name_an_adr_and_are_found_from_the_adr(b: Backend) {
        let (fx, a) = mission_with_tasks(b, 1);
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
        let rows = lines(&fx.ok(&a, &["decision", "list", "M-T"]));
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
            let found = lines(&fx.ok(&a, &["decision", "list", "--adr", &spelling]));
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
        let one = lines(&fx.ok(&a, &["decision", "list", "M-U", "--adr", other_adr]));
        assert_eq!(one.len(), 1);
        assert_eq!(one[0].len(), 5);
        assert_eq!(one[0][3..], ["Other ADR", other_adr]);
        let two = lines(&fx.ok(&a, &["decision", "list", "M-T", "--adr", adr]));
        assert_eq!(two.len(), 2);
        assert_eq!(
            fx.ok(&a, &["decision", "list", "M-T", "--adr", other_adr]),
            ""
        );
        // An ADR that no decision names is not an error, even when the file is gone.
        assert_eq!(
            fx.ok(&a, &["decision", "list", "--adr", "docs/adr/9999-none.md"]),
            ""
        );
        fails(&fx, &a, &["decision", "list"], "needs a mission");
        fails(
            &fx,
            &a,
            &["decision", "list", "M-NONE", "--adr", adr],
            "M-NONE",
        );
        fails(
            &fx,
            &a,
            &["decision", "list", "--adr", "/etc/passwd"],
            "absolute",
        );
        fails(&fx, &a, &["decision", "list", "--adr", "../x.md"], "'..'");

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
            fx.ok(&a, &["decision", "list", "--adr", other_adr])
                .lines()
                .count(),
            2
        );

        // Refused paths write nothing.
        let before = fx.ok(&a, &["decision", "list", "--adr", adr]);
        let count = fx.ok(&a, &["decision", "list", "M-T"]);
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
        assert_eq!(fx.ok(&a, &["decision", "list", "M-T"]), count);
        assert_eq!(fx.ok(&a, &["decision", "list", "--adr", adr]), before);

        // The links travel with the push, and the other clone finds them without the file.
        fx.ok(&a, &["ledger", "push", "Decisions with ADRs"]);
        let other = fx.b();
        fx.ok(&other, &["ledger", "fetch"]);
        assert_eq!(fx.ok(&other, &["decision", "list", "--adr", adr]), before);
    }

    pub fn constraints_and_out_of_scope_are_numbered_lists_whose_numbers_are_not_reused(
        b: Backend,
    ) {
        let fx = Fixture::new(b);
        let (a, other) = (fx.a(), fx.b());
        fx.ok(&a, &["ledger", "fetch"]);
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

    pub fn mission_set_refuses_the_list_fields_and_keeps_the_text_fields(b: Backend) {
        let fx = Fixture::new(b);
        let a = fx.a();
        fx.ok(&a, &["ledger", "fetch"]);
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
}

macro_rules! conformance {
    ($module:ident, $backend:expr; $($case:ident),* $(,)?) => {
        mod $module {
            use super::*;
            $(
                #[test]
                fn $case() {
                    let backend = $backend;
                    if !backend.available() {
                        eprintln!("note: dolt is not installed; skipping {}", stringify!($case));
                        return;
                    }
                    cases::$case(backend);
                }
            )*
        }
    };
}

conformance!(
    git_backend, Backend::Git;
    fetch_creates_a_new_ledger_and_the_first_push_creates_the_branch,
    documents_round_trip_between_clones,
    threads_start_pause_resume_list_detach_and_stop,
    cloud_sessions_bind_through_the_ledger,
    events_append_read_new_and_advance_the_watermark,
    a_stale_push_is_merged_when_different_rows_changed,
    a_same_row_conflict_stops_and_leaves_the_remote_unchanged,
    doc_path_prints_the_file_and_bad_paths_are_refused,
    documents_that_the_ledger_does_not_reserve_follow_the_backend,
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
    session_start_keeps_the_index_hint_for_a_bound_mission_with_no_record,
    session_start_shows_the_empty_ledger_note_only_without_records_and_documents,
    session_start_says_a_skipped_session_works_without_a_thread,
    a_thread_can_start_with_no_mission_and_pause_with_an_empty_task,
    thread_set_mission_attaches_changes_and_detaches_a_mission,
    session_start_says_a_bound_thread_with_no_mission_has_no_tasks,
    thread_commands_carry_local_record_and_document_changes_to_the_remote,
    status_and_discard_report_and_drop_local_changes,
    status_counts_commits_ahead_and_behind_origin,
    status_works_offline_and_discard_then_changes_nothing,
    status_and_discard_need_a_ledger_store,
    status_prints_local_and_origin_in_plain_columns,
    a_decision_takes_a_date_with_on_and_refuses_one_that_is_not_real,
    an_open_question_is_edited_and_an_answered_one_is_not,
    decisions_name_an_adr_and_are_found_from_the_adr,
    constraints_and_out_of_scope_are_numbered_lists_whose_numbers_are_not_reused,
    mission_set_refuses_the_list_fields_and_keeps_the_text_fields,
);

conformance!(
    dolt_backend, Backend::Dolt;
    fetch_creates_a_new_ledger_and_the_first_push_creates_the_branch,
    documents_round_trip_between_clones,
    threads_start_pause_resume_list_detach_and_stop,
    cloud_sessions_bind_through_the_ledger,
    events_append_read_new_and_advance_the_watermark,
    a_stale_push_is_merged_when_different_rows_changed,
    a_same_row_conflict_stops_and_leaves_the_remote_unchanged,
    doc_path_prints_the_file_and_bad_paths_are_refused,
    documents_that_the_ledger_does_not_reserve_follow_the_backend,
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
    session_start_keeps_the_index_hint_for_a_bound_mission_with_no_record,
    session_start_shows_the_empty_ledger_note_only_without_records_and_documents,
    session_start_says_a_skipped_session_works_without_a_thread,
    a_thread_can_start_with_no_mission_and_pause_with_an_empty_task,
    thread_set_mission_attaches_changes_and_detaches_a_mission,
    session_start_says_a_bound_thread_with_no_mission_has_no_tasks,
    thread_commands_carry_local_record_and_document_changes_to_the_remote,
    status_and_discard_report_and_drop_local_changes,
    status_counts_commits_ahead_and_behind_origin,
    status_works_offline_and_discard_then_changes_nothing,
    status_and_discard_need_a_ledger_store,
    status_prints_local_and_origin_in_plain_columns,
    a_decision_takes_a_date_with_on_and_refuses_one_that_is_not_real,
    an_open_question_is_edited_and_an_answered_one_is_not,
    decisions_name_an_adr_and_are_found_from_the_adr,
    constraints_and_out_of_scope_are_numbered_lists_whose_numbers_are_not_reused,
    mission_set_refuses_the_list_fields_and_keeps_the_text_fields,
);

// --- Backend selection and mismatch ---

#[test]
fn an_unknown_backend_is_an_error_naming_it() {
    let fx = Fixture::new(Backend::Git);
    let output = fx.tsk_with(&fx.a(), "sqlite", &["ledger", "path"]);
    assert_exit(&output, 1);
    assert!(stderr(&output).contains("'sqlite'"), "{}", stderr(&output));
}

#[test]
fn the_config_file_selects_the_backend_and_the_environment_overrides_it() {
    if !dolt_installed() {
        eprintln!("note: dolt is not installed; skipping backend selection");
        return;
    }
    let fx = Fixture::new(Backend::Git);
    let config = fx.home().join("config").join("tsk");
    std::fs::create_dir_all(&config).unwrap();
    write(&config.join("config.toml"), "[ledger]\nbackend = \"git\"\n");
    let a = fx.a();

    let mut command = fx.command(&a, "", &["ledger", "fetch"]);
    command.env_remove("TSK_LEDGER_BACKEND");
    let output = command.output().unwrap();
    assert_success(&output);
    let path = stdout(&output).trim_end().to_string();
    assert!(path.ends_with("/ledger"), "{}", path);

    let output = fx.tsk_with(&a, "dolt", &["ledger", "fetch"]);
    assert_success(&output);
    assert!(
        stdout(&output).trim_end().ends_with("/ledger-db"),
        "{}",
        stdout(&output)
    );

    write(
        &config.join("config.toml"),
        "[ledger]\nbackend = \"mongo\"\n",
    );
    let mut command = fx.command(&a, "", &["ledger", "path"]);
    command.env_remove("TSK_LEDGER_BACKEND");
    let output = command.output().unwrap();
    assert_exit(&output, 1);
    assert!(stderr(&output).contains("'mongo'"));
}

#[test]
fn the_git_backend_refuses_a_ref_that_holds_a_dolt_ledger() {
    if !dolt_installed() {
        eprintln!("note: dolt is not installed; skipping backend mismatch");
        return;
    }
    let fx = Fixture::new(Backend::Dolt);
    fx.ok(&fx.a(), &["ledger", "fetch"]);
    fx.put(&fx.a(), "index.md", "# Dolt\n");
    fx.ok(&fx.a(), &["ledger", "push", "Publish"]);

    let output = fx.tsk_with(&fx.b(), "git", &["ledger", "fetch"]);
    assert_exit(&output, 1);
    let message = stderr(&output);
    assert!(message.contains("Dolt ledger"), "{}", message);
    assert!(message.contains("dolt"), "{}", message);
}

#[test]
fn the_dolt_backend_refuses_a_ref_that_holds_a_file_ledger() {
    if !dolt_installed() {
        eprintln!("note: dolt is not installed; skipping backend mismatch");
        return;
    }
    let fx = Fixture::new(Backend::Git);
    fx.ok(&fx.a(), &["ledger", "fetch"]);
    fx.put(&fx.a(), "index.md", "# Files\n");
    fx.ok(&fx.a(), &["ledger", "push", "Publish"]);

    let output = fx.tsk_with(&fx.b(), "dolt", &["ledger", "fetch"]);
    assert_exit(&output, 1);
    let message = stderr(&output);
    assert!(message.contains("file ledger"), "{}", message);
    assert!(
        message.contains("tsk ledger migrate-to-dolt"),
        "{}",
        message
    );
    assert!(message.contains("TSK_LEDGER_BACKEND=git"), "{}", message);
}

#[test]
fn a_missing_dolt_binary_is_reported_with_a_hint() {
    let fx = Fixture::new(Backend::Dolt);
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
        .command(&fx.a(), "dolt", &["ledger", "fetch"])
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

// --- Dolt-only behaviour ---

#[test]
fn dolt_session_start_exports_the_database_path_and_warns_about_pending_commits() {
    if !dolt_installed() {
        eprintln!("note: dolt is not installed; skipping");
        return;
    }
    let fx = Fixture::new(Backend::Dolt);
    let a = fx.a();
    fx.ok(&a, &["ledger", "fetch"]);
    fx.put(&a, "index.md", "# Dolt\n");
    fx.ok(&a, &["ledger", "push", "Publish"]);

    let env_file = fx.root.path().join("claude-env");
    let session_start = |input: &str| {
        let mut child = fx
            .command(&a, "dolt", &["thread", "session-start"])
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
    assert!(
        context.contains("Read the index next with `tsk ledger doc get index.md`."),
        "{}",
        context
    );
    assert!(!context.contains("/index.md next"), "{}", context);
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
    let fx = Fixture::new(Backend::Dolt);
    let a = fx.a();
    fx.ok(&a, &["ledger", "fetch"]);
    let session_start = |session: &str| {
        let mut child = fx
            .command(&a, "dolt", &["thread", "session-start"])
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

// --- Documents on the tsk/docs branch (Dolt backend) ---

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
    let fx = Fixture::new(Backend::Dolt);
    let (a, other) = (fx.a(), fx.b());
    fx.ok(&a, &["ledger", "fetch"]);
    fx.put(&a, "missions/M-X.md", AWKWARD);
    fx.put(&a, "notes/empty.md", "");
    let pushed = fx.ok(&a, &["ledger", "push", "Add documents"]);
    assert!(!pushed.trim().is_empty());

    assert_eq!(fx.remote_branches(), Backend::Dolt.published_branches());

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
    let fx = Fixture::with_briefing(Backend::Dolt);
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
    let fx = Fixture::with_briefing(Backend::Dolt);
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
    let fx = Fixture::new(Backend::Dolt);
    let (a, other) = (fx.a(), fx.b());
    fx.ok(&a, &["ledger", "fetch"]);
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

#[test]
fn unpushed_documents_commits_are_noted_by_fetch_and_a_dirty_docs_worktree_is_refused() {
    if !dolt_installed() {
        eprintln!("note: dolt is not installed; skipping");
        return;
    }
    let fx = Fixture::with_briefing(Backend::Dolt);
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
    let fx = Fixture::with_briefing(Backend::Dolt);
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

// --- Mission and task tables and the version 7 ledger (Dolt backend) ---

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
        let fx = Fixture::new(Backend::Dolt);
        let a = fx.a();
        fx.ok(&a, &["ledger", "fetch"]);
        let mut sql: Vec<String> = MISSION_TABLES
            .iter()
            .map(|t| format!("drop table {}", t))
            .collect();
        sql.extend(OLD_THREADS.iter().map(|s| s.to_string()));
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
        let fx = Fixture::new(Backend::Dolt);
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
        sql.extend(OLD_THREADS.iter().map(|s| s.to_string()));
        sql.push("replace into manifest (k, v) values ('version', '3')".to_string());
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
    assert_eq!(fx.ok(&other, &["task", "list", "M-OLD"]), "");
    assert_eq!(fx.ok(&other, &["decision", "list", "M-OLD"]), "");
    assert_eq!(fx.ok(&other, &["question", "list"]), "");
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
    // records version 7.
    assert_eq!(
        fx.ok(&other, &["task", "add", "M-OLD", "First on version 3"]),
        "T-01\n"
    );
    assert_eq!(fx.manifest_version(&other), "7");
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
        fx.ok(&other, &["mission", "list"]),
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
    assert_eq!(fx.manifest_version(&a), "7");
    assert_eq!(
        fx.ok(&a, &["task", "list", "M-OLD"]),
        "T-01\ttodo\t-\tFirst on version 3\nT-02\ttodo\tT-01\tSecond\n"
    );
    assert_eq!(fx.ok(&a, &["decision", "list", "M-OLD"]).lines().count(), 1);
    assert_eq!(fx.ok(&a, &["question", "list"]).lines().count(), 1);
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
    assert_eq!(fx.manifest_version(&other), "7");
    assert_eq!(fx.ok(&other, &["task", "add", "M-OLD", "x"]), "T-01\n");
}

#[test]
fn a_new_dolt_ledger_has_the_mission_and_task_tables_at_version_seven() {
    if !dolt_installed() {
        eprintln!("note: dolt is not installed; skipping");
        return;
    }
    let fx = Fixture::new(Backend::Dolt);
    let a = fx.a();
    fx.ok(&a, &["ledger", "fetch"]);
    assert_eq!(fx.manifest_version(&a), "7");
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
    assert_eq!(fx.ok(&other, &["mission", "list"]), "");
    assert_eq!(fx.ok(&other, &["mission", "list", "--json"]), "[]\n");
    for args in [
        vec!["mission", "list", "M-X"],
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
    // The first mission write creates the tables, the column and records version 7 in one
    // change.
    fx.ok(&other, &["mission", "create", "M-X", "First on version 2"]);
    assert_eq!(fx.manifest_version(&other), "7");
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
        fx.ok(&other, &["mission", "list"]),
        "M-X\ttodo\t-\t-\tFirst on version 2\n"
    );
    // The other clone is still at version 2 until the push.
    assert_eq!(fx.manifest_version(&a), "2");
    fx.ok(&other, &["ledger", "push", "First mission"]);
    fx.ok(&a, &["ledger", "fetch"]);
    assert_eq!(fx.manifest_version(&a), "7");
    assert_eq!(
        fx.ok(&a, &["mission", "list"]),
        "M-X\ttodo\t-\t-\tFirst on version 2\n"
    );
    fx.ok(&a, &["mission", "create", "M-Y", "Second"]);
    assert_eq!(fx.manifest_version(&a), "7");
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

    // The first thread write creates the tables and the column and records version 7.
    let started = fx.ok(&other, &["thread", "start", "M-CONF-01"]);
    let id = started
        .trim_end()
        .strip_prefix("started:")
        .unwrap()
        .to_string();
    assert_eq!(fx.manifest_version(&other), "7");
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
    assert_eq!(fx.manifest_version(&a), "7");
    assert_eq!(fx.ok(&a, &["mission", "list"]), "");
}

impl Fixture {
    /// A published Dolt ledger at version 4, as the previous release left it: `threads`
    /// has no `mission_id`, its `briefing_path` is not null, and it holds one legacy
    /// thread for the briefing of M-CONF-01.
    fn version_four_ledger() -> Fixture {
        let fx = Fixture::new(Backend::Dolt);
        let a = fx.a();
        fx.ok(&a, &["ledger", "fetch"]);
        fx.put(&a, BRIEFING, "# Mission: conformance\n");
        fx.dolt_sql(
            &a,
            &format!(
                "{}; alter table threads drop column mission_id; \
                 alter table threads modify column briefing_path varchar(1024) not null; \
                 alter table decisions drop column adr_path; \
                 replace into manifest (k, v) values ('version', '4'); \
                 insert into threads (thread_id, briefing_path) values ('aaaa1111', '{}')",
                OLD_LISTS.join("; "),
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

    // The first thread start alters the table and records version 7, in its own push.
    let id = fx.started(&other, &["thread", "start", "M-CONF-01"]);
    assert_eq!(fx.manifest_version(&other), "7");
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
    assert_eq!(fx.manifest_version(&a), "7");
    assert_eq!(fx.thread_row(&a, &id)["mission_id"], "M-CONF-01");
    assert_eq!(fx.thread_row(&a, "aaaa1111")["mission_id"], "M-CONF-01");
}

#[test]
fn a_ledger_version_the_binary_does_not_know_stops_every_command_at_the_manifest_check() {
    // What the previous release does with a Dolt ledger the new release has written:
    // the manifest check is the first thing every command runs. The supported list is
    // faked in a unit test (ledger::manifest); here a version no release knows yet
    // stands in, because this binary supports 1, 2, 3, 4, 5, 6 and 7.
    if !dolt_installed() {
        eprintln!("note: dolt is not installed; skipping");
        return;
    }
    let fx = Fixture::new(Backend::Dolt);
    let a = fx.a();
    fx.ok(&a, &["ledger", "fetch"]);
    fx.dolt_sql(&a, "replace into manifest (k, v) values ('version', '8')");
    for args in [
        vec!["mission", "list"],
        vec!["mission", "create", "M-X", "x"],
        vec!["ledger", "push", "x"],
    ] {
        let out = fx.tsk(&a, &args);
        assert_exit(&out, 1);
        let message = stderr(&out);
        assert!(
            message.contains("unsupported ledger version 8")
                && message.contains("supports version 1, 2, 3, 4, 5, 6, 7")
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
    let fx = Fixture::with_briefing(Backend::Dolt);
    let a = fx.a();
    fx.dolt_sql(&a, "alter table watermark add column note int null");
    let out = fx.ok(&a, &["ledger", "status", "--porcelain"]);
    assert!(
        out.lines().any(|l| l == "data\twatermark\tschema changed"),
        "{}",
        out
    );
    fx.add_binding(&a, "s1");
    fx.dolt_sql(
        &a,
        "alter table cloud_session_bindings add column note2 int null",
    );
    let out = fx.ok(&a, &["ledger", "status", "--porcelain"]);
    assert!(
        out.lines()
            .any(|l| l
                == "data\tcloud_session_bindings\tadded 1\tmodified 0\tdeleted 0\tschema changed"),
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
    assert_eq!(fx.manifest_version(&other), "7");
    assert_eq!(
        fx.ok(&other, &["ledger", "status", "--porcelain"]),
        "data\tlocal changes\n\
         data\tupgrade\tschema upgraded to version 7\n\
         data\tmissions\tadded 1\tmodified 0\tdeleted 0\n\
         docs\tclean\n\
         data\tremote\tup to date\n\
         docs\tremote\tup to date\n"
    );
    assert_eq!(
        fx.ok(&other, &["ledger", "status"]),
        "Local   2 changes not pushed\n\
         \x20         data  upgrade   schema upgraded to version 7 (origin has 2)\n\
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
    assert_eq!(fx.manifest_version(&a), "7");
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
        fx.ok(&a, &["ledger", "status"]),
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
    assert_eq!(fx.manifest_version(&other), "7");
    assert_eq!(
        fx.ok(&other, &["ledger", "status", "--porcelain"]),
        "data\tlocal changes\n\
         data\tupgrade\tschema upgraded to version 7\n\
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
        let fx = Fixture::new(Backend::Dolt);
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
        fx.dolt_sql(
            &a,
            &format!(
                "{}; alter table decisions drop column adr_path; \
                 replace into manifest (k, v) values ('version', '5')",
                OLD_LISTS.join("; ")
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
        fx.ok(&other, &["decision", "list", "M-OLD"]),
        "2026-01-02\tJim\tT-01\tOld call\t-\n"
    );
    assert_eq!(
        fx.ok(&other, &["decision", "list", "--adr", "docs/adr/x.md"]),
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
    assert_eq!(fx.manifest_version(&other), "7");
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
        fx.ok(&other, &["decision", "list", "M-OLD"]),
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
         data\tupgrade\tschema upgraded to version 7\n\
         data\tdecisions\tadded 1\tmodified 0\tdeleted 0\n\
         docs\tclean\n\
         data\tremote\tup to date\n\
         docs\tremote\tup to date\n"
    );

    // The other clone is still at version 5 until the push.
    assert_eq!(fx.manifest_version(&a), "5");
    fx.ok(&other, &["ledger", "push", "First ADR link"]);
    fx.ok(&a, &["ledger", "fetch"]);
    assert_eq!(fx.manifest_version(&a), "7");
    assert_eq!(
        fx.ok(&a, &["decision", "list", "--adr", "docs/adr/0001-first.md"]),
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
    assert_eq!(fx.manifest_version(&other), "7");
    assert!(fx
        .ok(&other, &["decision", "list", "M-OLD"])
        .lines()
        .all(|l| l.ends_with("\t-")));
}

impl Fixture {
    /// A published Dolt ledger at version 6, as the previous release left it: the
    /// constraints and out of scope of a mission are two text columns of `missions`.
    /// M-OLD holds lines with blanks, padding and a Windows line break, M-BLANK holds
    /// only white space, and M-NULL holds nothing.
    fn version_six_ledger() -> Fixture {
        let fx = Fixture::new(Backend::Dolt);
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
                OLD_LISTS.join("; ")
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
    assert_eq!(fx.manifest_version(&other), "7");
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
        "COLUMN_NAME\nid\nname\ncategory\nstatus\nparent\npurpose\ndecision_authority\n\
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
        status.contains("data\tupgrade\tschema upgraded to version 7\n"),
        "{}",
        status
    );
    assert!(!status.contains("manifest"), "{}", status);
    assert!(!status.contains("data\tmissions"), "{}", status);

    // The other clone is still at version 6 until the push.
    assert_eq!(fx.manifest_version(&a), "6");
    fx.ok(&other, &["ledger", "push", "Move the lists"]);
    fx.ok(&a, &["ledger", "fetch"]);
    assert_eq!(fx.manifest_version(&a), "7");
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
    assert_eq!(fx.manifest_version(&other), "7");
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

#[test]
fn the_file_ledger_reads_a_record_that_holds_its_lists_as_strings() {
    let fx = Fixture::new(Backend::Git);
    let (a, other) = (fx.a(), fx.b());
    fx.ok(&a, &["ledger", "fetch"]);
    fx.ok(&a, &["mission", "create", "M-OLD", "Old record"]);
    // The record as the previous release wrote it: both lists are strings, and the
    // counters do not exist.
    let file = fx.ledger_path(&a).join("records/missions/M-OLD.json");
    let mut record: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    record["constraints"] = serde_json::json!("  first\n\nsecond  \r\n");
    record["out_of_scope"] = serde_json::json!("only one");
    let map = record.as_object_mut().unwrap();
    map.remove("next_constraint");
    map.remove("next_out_of_scope");
    write(&file, &serde_json::to_string_pretty(&record).unwrap());

    assert_eq!(
        fx.ok(&a, &["mission", "constraint", "list", "M-OLD"]),
        "1\tfirst\n2\tsecond\n"
    );
    assert_eq!(
        fx.ok(&a, &["mission", "out-of-scope", "list", "M-OLD"]),
        "1\tonly one\n"
    );
    let shown = mission_json_of(&fx, &a, "M-OLD");
    assert_eq!(
        shown["constraints"],
        serde_json::json!([{"n": 1, "text": "first"}, {"n": 2, "text": "second"}])
    );
    // Nothing was written by the reads: the file still holds the strings.
    let unchanged: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    assert_eq!(unchanged["constraints"], "  first\n\nsecond  \r\n");

    // A write converts the record to the list form, numbering on from the last item.
    assert_eq!(
        fx.ok(&a, &["mission", "constraint", "add", "M-OLD", "third"]),
        "3\n"
    );
    let written: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    assert_eq!(
        written["constraints"],
        serde_json::json!([
            {"seq": 1, "text": "first"},
            {"seq": 2, "text": "second"},
            {"seq": 3, "text": "third"}
        ])
    );
    assert_eq!(
        written["out_of_scope"],
        serde_json::json!([{"seq": 1, "text": "only one"}])
    );
    assert_eq!(written["next_constraint"], 4);
    assert_eq!(written["next_out_of_scope"], 2);
    assert_eq!(
        fx.ok(&a, &["mission", "out-of-scope", "add", "M-OLD", "second"]),
        "2\n"
    );
    fx.ok(&a, &["ledger", "push", "Convert the record"]);
    fx.ok(&other, &["ledger", "fetch"]);
    assert_eq!(
        fx.ok(&other, &["mission", "constraint", "list", "M-OLD"]),
        "1\tfirst\n2\tsecond\n3\tthird\n"
    );
}
