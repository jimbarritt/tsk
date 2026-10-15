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
const BRIEFING: &str = "missions/M-CONF-01.md";
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

        let start = fx.ok(&a, &["thread", "start", "M-CONF-01", BRIEFING]);
        let id = start
            .trim_end()
            .strip_prefix("started:")
            .unwrap_or_else(|| panic!("{}", start))
            .to_string();
        assert_eq!(
            fx.ok(&a, &["thread", "binding"]),
            format!("worktree:{}\n", id)
        );

        let paused = fx.ok(
            &a,
            &["thread", "pause", &id, BRIEFING, "T-01", "first \"step\""],
        );
        assert_eq!(paused, format!("paused:{}\n", id));
        let list = fx.ok(&a, &["thread", "list"]);
        let row = json(list.lines().next().unwrap());
        assert_eq!(row["id"], id.as_str());
        assert_eq!(row["mission_link"], BRIEFING);
        assert_eq!(row["latest_whats_next"], "first \"step\"");

        assert_eq!(
            fx.ok(&a, &["thread", "detach"]),
            format!("detached:{}\n", id)
        );
        assert_exit(&fx.tsk(&a, &["thread", "binding", "--no-fetch"]), 1);
        assert_exit(
            &fx.tsk(&a, &["thread", "start", "M-CONF-01", "missions/missing.md"]),
            1,
        );

        let resumed = json(&fx.ok(&other, &["thread", "resume", &id]));
        assert_eq!(resumed["thread_id"], id.as_str());
        assert_eq!(resumed["latest"]["task_id"], "T-01");
        assert_eq!(resumed["latest"]["mission_link"], BRIEFING);
        let commit = resumed["latest"]["git"]["ledger"]["commit"]
            .as_str()
            .unwrap();
        assert!(!commit.is_empty());
        assert_eq!(
            fx.ok(&other, &["thread", "binding"]),
            format!("worktree:{}\n", id)
        );

        assert_exit(
            &fx.tsk(&a, &["thread", "pause", "zzzzzzzz", BRIEFING, "T", "x"]),
            1,
        );

        assert_eq!(
            fx.ok(&other, &["thread", "stop"]),
            format!("stopped:{}\n", id)
        );
        assert_eq!(fx.ok(&a, &["thread", "list"]), "");
    }

    pub fn cloud_sessions_bind_through_the_ledger(b: Backend) {
        let fx = Fixture::with_briefing(b);
        let (a, other) = (fx.a(), fx.b());
        let start = fx.tsk_cloud(&a, "cse_one", &["thread", "start", "M-CONF-01", BRIEFING]);
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

    let start = fx.ok(&a, &["thread", "start", "M-CONF-01", BRIEFING]);
    assert!(start.starts_with("started:"), "{}", start);
    assert!(fx.ok(&a, &["thread", "detach"]).starts_with("detached:"));
    let missing = fx.tsk(&a, &["thread", "start", "M-CONF-01", "missions/none.md"]);
    assert_exit(&missing, 1);
    assert!(
        stderr(&missing).contains("mission briefing not found"),
        "{}",
        stderr(&missing)
    );
    // The briefing path in the message points at the documents worktree.
    assert!(
        stderr(&missing).contains("/docs/missions/none.md"),
        "{}",
        stderr(&missing)
    );
}
