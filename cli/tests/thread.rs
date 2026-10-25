use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use tempfile::TempDir;

use crate::common;

use common::templates::{self, Seed, Step};

const fn seeded(branch: &'static str) -> Seed {
    Seed {
        branch,
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
    }
}

const MAIN: Seed = seeded("main");
const TRUNK: Seed = seeded("trunk");

const LEDGER_REF: &str = "refs/heads/tsk/ledger";
const BRIEFING: &str = "missions/operational/M-TEST-01-example.md";
/// What resume, detach and set mission print on stderr after a write that does not push.
const NOT_PUBLISHED: &str = "goes out with the next push";
const SESSION_VAR: &str = "CLAUDE_CODE_REMOTE_SESSION_ID";

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
        Fixture::with_default_branch("main")
    }

    /// A published Dolt ledger that holds the briefing of mission M-TEST-01. Clone `work`
    /// runs the commands. Clone `work2` stands for another writer. The ledger for each
    /// default branch is built once per test process (see `common::templates`), then
    /// copied for each test. The copy is a real filesystem copy, so no test writes into the
    /// template and no test sees the change of another.
    fn with_default_branch(branch: &str) -> Fixture {
        let seed = match branch {
            "main" => &MAIN,
            "trunk" => &TRUNK,
            other => panic!("no template for the default branch {}", other),
        };
        Fixture {
            root: templates::instance(seed),
        }
    }

    fn origin(&self) -> PathBuf {
        self.root.path().join("origin.git")
    }

    fn work(&self) -> PathBuf {
        self.root.path().join("work")
    }

    fn other(&self) -> PathBuf {
        self.root.path().join("work2")
    }

    fn home(&self) -> PathBuf {
        self.root.path().join("home")
    }

    fn state(&self) -> PathBuf {
        self.root.path().join("state")
    }

    fn marker(&self) -> PathBuf {
        self.work().join(".git").join("tsk-thread-id")
    }

    /// The actor key the main worktree of the work clone has in the ledger.
    fn worktree_actor(&self) -> String {
        let id = std::fs::read_to_string(self.work().join(".git").join("tsk-clone-id")).unwrap();
        format!("{}/main", id.trim())
    }

    fn binding(&self) -> String {
        stdout(&self.tsk(&["thread", "binding"]))
    }

    /// Run SQL against the Dolt database of the clone at `dir`; `json` rows come back.
    fn rows_in(&self, dir: &Path, sql: &str) -> Vec<serde_json::Value> {
        let path = stdout(
            &self
                .command_in(dir, &["ledger", "path"], None)
                .output()
                .unwrap(),
        );
        let out = Command::new("dolt")
            .args(["sql", "-r", "json", "-q", sql])
            .current_dir(path.trim_end())
            .env("HOME", self.home())
            .env("DOLT_DISABLE_EVENT_FLUSH", "1")
            .output()
            .unwrap();
        assert!(out.status.success(), "{}", stderr(&out));
        let text = stdout(&out);
        if text.trim().is_empty() {
            return Vec::new();
        }
        json(&text)["rows"].as_array().cloned().unwrap_or_default()
    }

    fn exec_in(&self, dir: &Path, sql: &str) {
        let path = stdout(
            &self
                .command_in(dir, &["ledger", "path"], None)
                .output()
                .unwrap(),
        );
        // A Dolt server that an earlier command started holds the store for writing.
        common::stop_servers(self.root.path(), true);
        let out = Command::new("dolt")
            .args(["sql", "-q", sql])
            .current_dir(path.trim_end())
            .env("HOME", self.home())
            .env("DOLT_DISABLE_EVENT_FLUSH", "1")
            .output()
            .unwrap();
        assert!(out.status.success(), "{}", stderr(&out));
    }

    /// Rows of the work clone's database.
    fn rows(&self, sql: &str) -> Vec<serde_json::Value> {
        self.rows_in(&self.work(), sql)
    }

    /// Rows of what origin holds: the other clone fetches first.
    fn origin_rows(&self, sql: &str) -> Vec<serde_json::Value> {
        let other = self.other();
        assert_success(
            &self
                .command_in(&other, &["ledger", "fetch"], None)
                .output()
                .unwrap(),
        );
        self.rows_in(&other, sql)
    }

    /// The stored continuation lines of a thread as origin holds them, oldest first.
    fn origin_entries(&self, id: &str) -> Vec<String> {
        self.origin_rows(&format!(
            "select entry from continuation_entries where thread_id = '{}' order by seq",
            id
        ))
        .iter()
        .map(|row| row["entry"].as_str().unwrap().to_string())
        .collect()
    }

    fn origin_binding_threads(&self) -> std::collections::BTreeMap<String, String> {
        self.origin_rows("select actor, thread_id from bindings")
            .iter()
            .map(|row| {
                (
                    row["actor"].as_str().unwrap().to_string(),
                    row["thread_id"].as_str().unwrap().to_string(),
                )
            })
            .collect()
    }

    /// The bindings the work clone's ledger store holds, as it is, with no fetch.
    fn local_binding_threads(&self) -> std::collections::BTreeMap<String, String> {
        self.rows("select actor, thread_id from bindings")
            .iter()
            .map(|row| {
                (
                    row["actor"].as_str().unwrap().to_string(),
                    row["thread_id"].as_str().unwrap().to_string(),
                )
            })
            .collect()
    }

    fn origin_thread_ids(&self) -> Vec<String> {
        self.origin_rows("select thread_id from threads order by thread_id")
            .iter()
            .map(|row| row["thread_id"].as_str().unwrap().to_string())
            .collect()
    }

    /// The message of the newest Dolt commit origin holds.
    fn origin_subject(&self) -> String {
        self.origin_rows("select message from dolt_log order by date desc, commit_hash limit 1")[0]
            ["message"]
            .as_str()
            .unwrap()
            .to_string()
    }

    /// The other clone writes a thread whose stored lines are `store` and pushes. The
    /// thread links the briefing and names no mission, as a thread made before layout
    /// version 5 does.
    fn seed_thread(&self, id: &str, store: &str) {
        let other = self.other();
        assert_success(
            &self
                .command_in(&other, &["ledger", "fetch"], None)
                .output()
                .unwrap(),
        );
        let mut sql = format!(
            "insert into threads (thread_id, briefing_path, mission_id) values ('{}', '{}', NULL);",
            id, BRIEFING
        );
        for (index, line) in store.lines().enumerate() {
            sql.push_str(&format!(
                "insert into continuation_entries (thread_id, seq, entry) values ('{}', {}, '{}');",
                id,
                index + 1,
                line.replace('\'', "''")
            ));
        }
        self.exec_in(&other, &sql);
        assert_success(
            &self
                .command_in(
                    &other,
                    &["ledger", "push", &format!("Seed thread {}", id)],
                    None,
                )
                .output()
                .unwrap(),
        );
    }

    /// The other clone writes cloud bindings, `(session, thread, registered_at)`, and pushes.
    fn seed_cloud_bindings(&self, rows: &[(&str, &str, &str)]) {
        let other = self.other();
        assert_success(
            &self
                .command_in(&other, &["ledger", "fetch"], None)
                .output()
                .unwrap(),
        );
        let values: Vec<String> = rows
            .iter()
            .map(|(session, thread, at)| {
                format!("('{}', 'cloud', '{}', '{}')", session, thread, at)
            })
            .collect();
        self.exec_in(
            &other,
            &format!(
                "insert into bindings (actor, kind, thread_id, registered_at) values {}",
                values.join(", ")
            ),
        );
        assert_success(
            &self
                .command_in(&other, &["ledger", "push", "Seed bindings"], None)
                .output()
                .unwrap(),
        );
    }

    fn origin_ledger_sha(&self) -> String {
        git_line(&self.origin(), &["rev-parse", LEDGER_REF])
    }

    fn command(&self, args: &[&str], cloud: Option<&str>) -> Command {
        self.command_in(&self.work(), args, cloud)
    }

    fn command_in(&self, dir: &Path, args: &[&str], cloud: Option<&str>) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_tsk"));
        command
            .args(args)
            .current_dir(dir)
            .envs(git_env())
            .env("HOME", self.home())
            .env("XDG_STATE_HOME", self.state())
            .env("TSK_DOLT_SERVER_IDLE_SECS", "3")
            .env("TSK_BACKGROUND_PUBLISH", "off")
            .env("XDG_CONFIG_HOME", self.home().join("config"))
            .env("TSK_HOME", self.home().join(".tsk"))
            .env("TSK_NO_UPDATE_CHECK", "1")
            .env("TSK_LEDGER_REFS", "branches")
            .env("GIT_CEILING_DIRECTORIES", self.root.path())
            .env_remove(SESSION_VAR);
        if let Some(session) = cloud {
            command.env(SESSION_VAR, session);
        }
        command
    }

    fn tsk(&self, args: &[&str]) -> Output {
        self.command(args, None)
            .output()
            .expect("failed to run tsk")
    }

    fn tsk_cloud(&self, session: &str, args: &[&str]) -> Output {
        self.command(args, Some(session))
            .output()
            .expect("failed to run tsk")
    }

    fn tsk_stdin(&self, args: &[&str], input: &str) -> Output {
        let mut child = self
            .command(args, None)
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

    fn start(&self) -> String {
        let output = self.tsk(&["thread", "start", "M-TEST-01"]);
        assert_success(&output);
        let line = stdout(&output);
        line.trim_end()
            .strip_prefix("started:")
            .unwrap_or_else(|| panic!("unexpected start output: {}", line))
            .to_string()
    }

    fn ledger_path(&self) -> PathBuf {
        let output = self.tsk(&["ledger", "path"]);
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

fn json(text: &str) -> serde_json::Value {
    serde_json::from_str(text.trim_end()).unwrap_or_else(|e| panic!("{}: {}", e, text))
}

fn is_thread_id(id: &str) -> bool {
    id.len() == 8
        && id
            .chars()
            .all(|c| c.is_ascii_digit() || c.is_ascii_lowercase())
}

#[test]
fn start_mints_scaffolds_binds_the_worktree_and_pushes() {
    let fx = Fixture::new();
    let id = fx.start();

    assert!(is_thread_id(&id), "{}", id);
    let rows = fx.origin_rows(&format!(
        "select mission_id, briefing_path from threads where thread_id = '{}'",
        id
    ));
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["mission_id"], "M-TEST-01");
    assert!(fx.origin_entries(&id).is_empty());
    // The binding is a row in the ledger, keyed on the worktree actor. No marker file.
    assert!(!fx.marker().exists());
    assert_eq!(fx.binding(), format!("worktree:{}\n", id));
    let actor = fx.worktree_actor();
    let rows = fx.origin_rows("select actor, kind, thread_id from bindings");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["actor"], actor.as_str());
    assert_eq!(rows[0]["kind"], "worktree");
    assert_eq!(rows[0]["thread_id"], id.as_str());
    assert_eq!(
        fx.origin_subject(),
        format!("Start thread {} for mission M-TEST-01", id)
    );
}

#[test]
fn start_with_an_existing_binding_requires_a_resume() {
    let fx = Fixture::new();
    let id = fx.start();
    let before = fx.origin_ledger_sha();

    let output = fx.tsk(&["thread", "start", "M-TEST-01"]);

    assert_exit(&output, 2);
    assert_eq!(stdout(&output), format!("resume-required:{}\n", id));
    assert_eq!(fx.origin_ledger_sha(), before);
}

#[test]
fn start_refuses_a_mission_with_no_record_and_no_briefing() {
    let fx = Fixture::new();
    let before = fx.origin_ledger_sha();

    let output = fx.tsk(&["thread", "start", "M-NONE"]);

    assert_exit(&output, 1);
    assert_eq!(stdout(&output), "");
    assert!(stderr(&output).contains("no such mission: M-NONE"));
    // The old form with a briefing path gives the same refusal.
    let old = fx.tsk(&["thread", "start", "M-NONE", "missions/M-NONE.md"]);
    assert_exit(&old, 1);
    assert!(stderr(&old).contains("no such mission: M-NONE"));
    assert!(!fx.marker().exists());
    assert_eq!(fx.origin_ledger_sha(), before);
}

#[test]
fn start_in_a_cloud_session_binds_through_the_ledger() {
    let fx = Fixture::new();
    let output = fx.tsk_cloud("cse_one", &["thread", "start", "M-TEST-01", BRIEFING]);
    assert!(
        stderr(&output).contains("no longer needed"),
        "{}",
        stderr(&output)
    );
    assert_success(&output);
    let id = stdout(&output)
        .trim_end()
        .strip_prefix("started:")
        .unwrap()
        .to_string();

    let rows = fx.origin_rows("select actor, kind, thread_id, registered_at from bindings");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["actor"], "cse_one");
    assert_eq!(rows[0]["kind"], "cloud");
    assert_eq!(rows[0]["thread_id"], id.as_str());
    assert!(rows[0]["registered_at"].as_str().unwrap().ends_with('Z'));
    assert!(!fx.marker().exists());

    let binding = fx.tsk_cloud("cse_one", &["thread", "binding"]);
    assert_success(&binding);
    assert_eq!(stdout(&binding), format!("cloud:{}\n", id));
}

#[test]
fn pause_appends_an_entry_with_the_nested_git_state_and_pushes() {
    let fx = Fixture::new();
    let id = fx.start();
    let ledger_before = fx.rows("select commit_hash from dolt_log limit 1")[0]["commit_hash"]
        .as_str()
        .unwrap()
        .to_string();
    let main_head = git_line(&fx.work(), &["rev-parse", "HEAD"]);

    let output = fx.tsk(&["thread", "pause", &id, "T-05", "Write the \"thread\" tests"]);

    assert_success(&output);
    assert_eq!(
        stdout(&output),
        format!(
            "paused:{}\ncode: clean\ncode pushed: yes\nledger: pushed\n",
            id
        )
    );
    let stored = fx.origin_entries(&id);
    assert_eq!(stored.len(), 1);
    let line = stored[0].as_str();
    let keys: Vec<_> = json(line).as_object().unwrap().keys().cloned().collect();
    assert_eq!(keys.len(), 7);
    let positions: Vec<_> = [
        "\"mission_id\"",
        "\"mission_link\"",
        "\"task_id\"",
        "\"whats_next\"",
        "\"git\"",
        "\"timestamp\"",
        "\"written_by\"",
    ]
    .iter()
    .map(|key| line.find(key).unwrap())
    .collect();
    assert!(
        positions.windows(2).all(|pair| pair[0] < pair[1]),
        "{}",
        line
    );
    for old in ["commit_on_bootstrap", "commit_on_ledger", "commit_on_main"] {
        assert!(!line.contains(old), "{}", old);
    }
    let entry = json(line);
    assert_eq!(entry["mission_id"], "M-TEST-01");
    assert_eq!(entry["mission_link"], BRIEFING);
    assert_eq!(entry["task_id"], "T-05");
    assert_eq!(entry["whats_next"], "Write the \"thread\" tests");
    assert_eq!(entry["git"]["ledger"]["commit"], ledger_before.as_str());
    assert_eq!(entry["git"]["code"]["ref"], "refs/heads/main");
    assert_eq!(entry["git"]["code"]["commit"], main_head.as_str());
    assert_eq!(entry["git"]["ledger"].as_object().unwrap().len(), 1);
    assert_eq!(entry["git"]["code"].as_object().unwrap().len(), 2);
    assert_eq!(entry["written_by"], fx.worktree_actor().as_str());
    assert_eq!(entry["timestamp"].as_str().unwrap().len(), 20);
    assert_eq!(fx.origin_subject(), format!("Pause thread {}: T-05", id));
}

#[test]
fn pause_prints_the_verification_lines_when_everything_is_clean_and_pushed() {
    let fx = Fixture::new();
    let id = fx.start();

    let output = fx.tsk(&["thread", "pause", &id, "T-05", "Go on"]);

    assert_success(&output);
    assert_eq!(
        stdout(&output),
        format!(
            "paused:{}\ncode: clean\ncode pushed: yes\nledger: pushed\n",
            id
        )
    );
}

#[test]
fn pause_counts_the_dirty_files_of_the_code_worktree() {
    let fx = Fixture::new();
    let id = fx.start();
    write(&fx.work().join("a.md"), "one\n");
    write(&fx.work().join("b.md"), "two\n");

    let output = fx.tsk(&["thread", "pause", &id, "T-05", "Go on"]);

    assert_success(&output);
    assert_eq!(
        stdout(&output),
        format!(
            "paused:{}\ncode: dirty (2 files)\ncode pushed: yes\nledger: pushed\n",
            id
        )
    );
}

#[test]
fn pause_bound_pauses_the_thread_this_worktree_is_bound_to() {
    let fx = Fixture::new();
    let id = fx.start();

    let output = fx.tsk(&["thread", "pause", "--bound", "T-05", "Go on"]);

    assert_success(&output);
    assert!(
        stdout(&output).starts_with(&format!("paused:{}\n", id)),
        "{}",
        stdout(&output)
    );
    let stored = fx.origin_entries(&id);
    assert_eq!(stored.len(), 1);
    assert_eq!(json(&stored[0])["task_id"], "T-05");
    assert_eq!(json(&stored[0])["whats_next"], "Go on");

    // Three arguments with --bound are refused.
    let three = fx.tsk(&["thread", "pause", "--bound", &id, "T-05", "Go on"]);
    assert_exit(&three, 1);
}

#[test]
fn pause_bound_with_no_binding_fails_clearly() {
    let fx = Fixture::new();

    let output = fx.tsk(&["thread", "pause", "--bound", "T-05", "Go on"]);

    assert_exit(&output, 1);
    assert!(
        stderr(&output).contains("bound to no thread"),
        "{}",
        stderr(&output)
    );
    assert_eq!(stdout(&output), "");
}

fn assert_pause_not_recorded(fx: &Fixture, id: &str, before: &str) {
    assert_eq!(fx.origin_ledger_sha(), before);
    let rows = fx.rows(&format!(
        "select count(*) as c from continuation_entries where thread_id = '{}'",
        id
    ));
    assert_eq!(rows[0]["c"], 0);
}

#[test]
fn pause_refuses_a_commit_that_is_on_no_origin_branch() {
    let fx = Fixture::new();
    let id = fx.start();
    write(&fx.work().join("local.md"), "unpushed\n");
    commit_all(&fx.work(), "Local only");
    let before = fx.origin_ledger_sha();

    let output = fx.tsk(&["thread", "pause", &id, BRIEFING, "T-05", "next"]);

    assert_exit(&output, 1);
    assert_eq!(stdout(&output), "");
    assert!(
        stderr(&output).contains("is not reachable from any branch on origin"),
        "{}",
        stderr(&output)
    );
    assert!(stderr(&output).contains("Push main before pausing"));
    assert_pause_not_recorded(&fx, &id, &before);
}

#[test]
fn pause_accepts_a_commit_pushed_only_to_a_feature_branch() {
    let fx = Fixture::new();
    let id = fx.start();
    git(&fx.work(), &["switch", "--quiet", "-c", "feature/x"]);
    write(&fx.work().join("feature.md"), "feature\n");
    commit_all(&fx.work(), "Feature commit");
    git(
        &fx.work(),
        &["push", "--quiet", "origin", "HEAD:refs/heads/feature/x"],
    );
    let head = git_line(&fx.work(), &["rev-parse", "HEAD"]);

    let output = fx.tsk(&["thread", "pause", &id, BRIEFING, "T-05", "next"]);

    assert_success(&output);
    let entry = json(&fx.origin_entries(&id)[0]);
    assert_eq!(entry["git"]["code"]["ref"], "refs/heads/feature/x");
    assert_eq!(entry["git"]["code"]["commit"], head.as_str());
}

#[test]
fn pause_records_the_local_branch_even_when_origin_holds_the_commit_on_another() {
    let fx = Fixture::new();
    let id = fx.start();
    git(&fx.work(), &["switch", "--quiet", "-c", "local-name"]);
    let head = git_line(&fx.work(), &["rev-parse", "HEAD"]);

    let output = fx.tsk(&["thread", "pause", &id, BRIEFING, "T-05", "next"]);

    assert_success(&output);
    let entry = json(&fx.origin_entries(&id)[0]);
    assert_eq!(entry["git"]["code"]["ref"], "refs/heads/local-name");
    assert_eq!(entry["git"]["code"]["commit"], head.as_str());
}

#[test]
fn pause_sees_a_branch_pushed_by_another_clone_without_a_prior_fetch() {
    let fx = Fixture::new();
    let id = fx.start();
    write(&fx.work().join("later.md"), "later\n");
    commit_all(&fx.work(), "Pushed from elsewhere");
    git(
        &fx.work(),
        &["push", "--quiet", "origin", "HEAD:refs/heads/elsewhere"],
    );
    git(
        &fx.work(),
        &["update-ref", "-d", "refs/remotes/origin/elsewhere"],
    );

    let output = fx.tsk(&["thread", "pause", &id, BRIEFING, "T-05", "next"]);

    assert_success(&output);
}

#[test]
fn pause_does_not_count_a_deleted_origin_branch() {
    let fx = Fixture::new();
    let id = fx.start();
    git(&fx.work(), &["switch", "--quiet", "-c", "gone"]);
    write(&fx.work().join("gone.md"), "gone\n");
    commit_all(&fx.work(), "On a branch that is then deleted");
    git(
        &fx.work(),
        &["push", "--quiet", "origin", "HEAD:refs/heads/gone"],
    );
    git(&fx.origin(), &["update-ref", "-d", "refs/heads/gone"]);
    // A fetch that prunes has run since the branch went, so no tracking ref holds the
    // commit. A pause that finds no tracking ref asks origin again.
    git(&fx.work(), &["fetch", "--quiet", "--prune", "origin"]);
    let before = fx.origin_ledger_sha();

    let output = fx.tsk(&["thread", "pause", &id, BRIEFING, "T-05", "next"]);

    assert_exit(&output, 1);
    assert!(stderr(&output).contains("is not reachable from any branch on origin"));
    assert_pause_not_recorded(&fx, &id, &before);
}

/// A directory with a `git` stand-in that logs its arguments, one line per run, to the
/// file named by `TSK_GIT_LOG`, then runs the real `git`.
fn git_log_shim(root: &Path) -> PathBuf {
    let real = std::env::var_os("PATH")
        .and_then(|path| {
            std::env::split_paths(&path)
                .map(|dir| dir.join("git"))
                .find(|candidate| candidate.is_file())
        })
        .expect("git is not on PATH");
    let dir = root.join("git-log-shim");
    std::fs::create_dir_all(&dir).unwrap();
    let shim = dir.join("git");
    std::fs::write(
        &shim,
        format!(
            "#!/bin/sh\necho \"$@\" >> \"$TSK_GIT_LOG\"\nexec '{}' \"$@\"\n",
            real.display()
        ),
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&shim, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    dir
}

impl Fixture {
    /// A pause, and whether it fetched the code repository: ran `git fetch` with the refspec
    /// that mirrors the branches of origin.
    fn pause_fetching_code(&self, id: &str) -> (Output, bool) {
        let log = self.root.path().join("git-log.txt");
        let _ = std::fs::remove_file(&log);
        let mut dirs = vec![git_log_shim(self.root.path())];
        dirs.extend(std::env::split_paths(
            &std::env::var_os("PATH").unwrap_or_default(),
        ));
        let output = self
            .command(&["thread", "pause", id, BRIEFING, "T-05", "next"], None)
            .env("PATH", std::env::join_paths(dirs).unwrap())
            .env("TSK_GIT_LOG", &log)
            .output()
            .expect("failed to run tsk");
        let fetched = std::fs::read_to_string(&log)
            .unwrap_or_default()
            .lines()
            .any(|line| line.contains("+refs/heads/*:refs/remotes/origin/*"));
        (output, fetched)
    }
}

#[test]
fn pause_does_not_fetch_the_code_repository_when_a_tracking_ref_holds_the_commit() {
    let fx = Fixture::new();
    let id = fx.start();
    // The commit was pushed from this clone, which updates its tracking ref.
    write(&fx.work().join("pushed.md"), "pushed\n");
    commit_all(&fx.work(), "Pushed from here");
    git(
        &fx.work(),
        &["push", "--quiet", "origin", "HEAD:refs/heads/main"],
    );

    let (output, fetched) = fx.pause_fetching_code(&id);

    assert_success(&output);
    assert!(!fetched, "the pause fetched the code repository");
    assert_eq!(fx.origin_entries(&id).len(), 1);
}

#[test]
fn pause_fetches_the_code_repository_when_no_tracking_ref_holds_the_commit() {
    let fx = Fixture::new();
    let id = fx.start();
    write(&fx.work().join("later.md"), "later\n");
    commit_all(&fx.work(), "Pushed from elsewhere");
    git(
        &fx.work(),
        &["push", "--quiet", "origin", "HEAD:refs/heads/elsewhere"],
    );
    git(
        &fx.work(),
        &["update-ref", "-d", "refs/remotes/origin/elsewhere"],
    );

    let (output, fetched) = fx.pause_fetching_code(&id);

    assert_success(&output);
    assert!(fetched, "the pause did not ask origin");
    assert_eq!(fx.origin_entries(&id).len(), 1);
}

#[test]
fn pause_that_finds_the_commit_nowhere_fetches_once_and_refuses() {
    let fx = Fixture::new();
    let id = fx.start();
    write(&fx.work().join("local.md"), "unpushed\n");
    commit_all(&fx.work(), "Local only");

    let (output, fetched) = fx.pause_fetching_code(&id);

    assert_exit(&output, 1);
    assert!(fetched, "the pause did not ask origin before it refused");
    assert!(stderr(&output).contains("is not reachable from any branch on origin"));
}

#[test]
fn pause_accepts_a_tracking_ref_until_the_next_fetch() {
    // A branch deleted on origin still has its tracking ref here until a fetch prunes it.
    // The pause goes by the refs it has and does not ask origin.
    let fx = Fixture::new();
    let id = fx.start();
    git(&fx.work(), &["switch", "--quiet", "-c", "gone"]);
    write(&fx.work().join("gone.md"), "gone\n");
    commit_all(&fx.work(), "On a branch that is then deleted");
    git(
        &fx.work(),
        &["push", "--quiet", "origin", "HEAD:refs/heads/gone"],
    );
    git(&fx.origin(), &["update-ref", "-d", "refs/heads/gone"]);

    let (output, fetched) = fx.pause_fetching_code(&id);

    assert_success(&output);
    assert!(!fetched);
}

#[test]
fn pause_works_with_a_non_main_default_branch() {
    let fx = Fixture::with_default_branch("trunk");
    let id = fx.start();

    let output = fx.tsk(&["thread", "pause", &id, BRIEFING, "T-05", "next"]);

    assert_success(&output);
    let entry = json(&fx.origin_entries(&id)[0]);
    assert_eq!(entry["git"]["code"]["ref"], "refs/heads/trunk");
}

#[test]
fn pause_refuses_a_detached_head() {
    let fx = Fixture::new();
    let id = fx.start();
    git(&fx.work(), &["checkout", "--quiet", "--detach"]);
    let before = fx.origin_ledger_sha();

    let output = fx.tsk(&["thread", "pause", &id, BRIEFING, "T-05", "next"]);

    assert_exit(&output, 1);
    assert_eq!(stdout(&output), "");
    assert!(
        stderr(&output).contains("HEAD is detached"),
        "{}",
        stderr(&output)
    );
    assert_pause_not_recorded(&fx, &id, &before);
}

#[test]
fn pause_refuses_an_unknown_thread() {
    let fx = Fixture::new();
    let output = fx.tsk(&["thread", "pause", "0000aaaa", "T-05", "next"]);
    assert_exit(&output, 1);
    assert!(stderr(&output).contains("no such thread: 0000aaaa"));
}

#[test]
fn resume_prints_the_latest_entry_and_binds_the_worktree() {
    let fx = Fixture::new();
    let id = fx.start();
    assert_success(&fx.tsk(&["thread", "pause", &id, BRIEFING, "T-01", "first"]));
    assert_success(&fx.tsk(&["thread", "pause", &id, BRIEFING, "T-02", "second"]));
    assert_success(&fx.tsk(&["thread", "detach"]));
    assert!(!fx.marker().exists());
    let before = fx.origin_ledger_sha();

    let output = fx.tsk(&["thread", "resume", &id]);

    assert_success(&output);
    assert!(stdout(&output).starts_with(&format!("{{\"thread_id\":\"{}\",\"latest\":{{", id)));
    let result = json(&stdout(&output));
    assert_eq!(result["latest"]["task_id"], "T-02");
    assert_eq!(result["latest"]["whats_next"], "second");
    assert_eq!(result["warning"], "");
    assert!(!fx.marker().exists());
    assert_eq!(fx.binding(), format!("worktree:{}\n", id));
    // The binding is a local write (ADR 0019): origin is unchanged, the store holds the
    // row, and the next push publishes it.
    assert_eq!(fx.origin_ledger_sha(), before);
    assert!(stderr(&output).contains(NOT_PUBLISHED));
    // (The status is not asserted here: a detach and a resume within one second leave the
    // row as origin holds it. The detach and cloud cases below check it.)
    assert_eq!(
        fx.local_binding_threads().get(&fx.worktree_actor()),
        Some(&id)
    );
    assert_success(&fx.tsk(&["ledger", "push", "Publish the binding"]));
    assert_eq!(
        fx.origin_binding_threads().get(&fx.worktree_actor()),
        Some(&id)
    );
}

#[test]
fn resume_of_a_thread_with_no_entry_prints_an_empty_latest() {
    let fx = Fixture::new();
    fx.seed_thread("1111aaaa", "");

    let output = fx.tsk(&["thread", "resume", "1111aaaa"]);

    assert_success(&output);
    assert_eq!(
        stdout(&output),
        "{\"thread_id\":\"1111aaaa\",\"latest\":{},\"warning\":\"\",\"mission\":null,\"task\":null}\n"
    );
}

#[test]
fn resume_warns_and_still_binds_when_another_actor_wrote_to_the_thread() {
    let fx = Fixture::new();
    fx.seed_thread(
        "2222bbbb",
        "{\"mission_link\":\"m\",\"task_id\":\"T-01\",\"whats_next\":\"a\",\"git\":{\"ledger\":{\"commit\":\"c\"},\"code\":{\"ref\":\"refs/heads/main\",\"commit\":\"d\"}},\"timestamp\":\"2026-10-01T00:00:00Z\",\"written_by\":\"cse_other\"}\n\
         {\"mission_link\":\"m\",\"task_id\":\"T-01\",\"whats_next\":\"b\",\"git\":{\"ledger\":{\"commit\":\"c\"},\"code\":{\"ref\":\"refs/heads/main\",\"commit\":\"d\"}},\"timestamp\":\"2026-10-02T00:00:00Z\",\"written_by\":\"tsk-ffff0000/feature\"}\n",
    );

    let output = fx.tsk(&["thread", "resume", "2222bbbb"]);

    assert_success(&output);
    let result = json(&stdout(&output));
    assert_eq!(
        result["warning"],
        "thread 2222bbbb is already associated with other actor(s): cse_other,tsk-ffff0000/feature"
    );
    assert_eq!(result["latest"]["whats_next"], "b");
    assert!(!fx.marker().exists());
    assert_eq!(fx.binding(), "worktree:2222bbbb\n");
}

#[test]
fn resume_does_not_warn_when_the_current_actor_wrote_to_the_thread() {
    let fx = Fixture::new();
    let actor = fx.worktree_actor();
    fx.seed_thread(
        "3333cccc",
        &format!(
            "{{\"whats_next\":\"a\",\"written_by\":\"cse_other\"}}\n\
             {{\"whats_next\":\"b\",\"written_by\":\"{}\"}}\n",
            actor
        ),
    );

    let output = fx.tsk(&["thread", "resume", "3333cccc"]);

    assert_success(&output);
    assert_eq!(json(&stdout(&output))["warning"], "");
}

#[test]
fn resume_does_not_warn_against_entries_written_in_the_old_format() {
    let fx = Fixture::new();
    fx.seed_thread(
        "3333dddd",
        "{\"whats_next\":\"a\",\"written_by\":\"urn:tsk:cloudsession:cse_other\"}\n\
         {\"whats_next\":\"b\",\"written_by\":\"urn:tsk:worktree:.git\"}\n",
    );

    let output = fx.tsk(&["thread", "resume", "3333dddd"]);

    assert_success(&output);
    assert_eq!(json(&stdout(&output))["warning"], "");
}

#[test]
fn resume_warns_when_a_new_format_entry_follows_old_format_ones_from_another_clone() {
    let fx = Fixture::new();
    fx.seed_thread(
        "3333eeee",
        "{\"whats_next\":\"a\",\"written_by\":\"urn:tsk:worktree:.git\"}\n\
         {\"whats_next\":\"b\",\"written_by\":\"tsk-ffff0000/main\"}\n",
    );

    let output = fx.tsk(&["thread", "resume", "3333eeee"]);

    assert_success(&output);
    assert_eq!(
        json(&stdout(&output))["warning"],
        "thread 3333eeee is already associated with other actor(s): tsk-ffff0000/main"
    );
}

#[test]
fn a_cloud_session_pause_writes_the_session_id_as_written_by() {
    let fx = Fixture::new();
    let id = fx.start();
    let output = fx.tsk_cloud(
        "cse_writer",
        &["thread", "pause", &id, "T-05", "Cloud pause"],
    );
    assert_success(&output);
    let stored = fx.origin_entries(&id);
    assert_eq!(json(stored.last().unwrap())["written_by"], "cse_writer");
}

#[test]
fn resume_list_and_pause_keep_entries_as_stored() {
    let fx = Fixture::new();
    let stored = "{\"mission_link\":\"missions/operational/M-TEST-01-example.md\",\"task_id\":\"T-03\",\"whats_next\":\"stored entry\",\"git\":{\"ledger\":{\"commit\":\"1111111111111111111111111111111111111111\"},\"code\":{\"ref\":\"refs/heads/main\",\"commit\":\"2222222222222222222222222222222222222222\"}},\"timestamp\":\"2026-09-20T09:00:00Z\",\"written_by\":\"urn:tsk:worktree:.git\"}";
    fx.seed_thread("4onylfsg", &format!("{}\n", stored));

    let output = fx.tsk(&["thread", "resume", "4onylfsg"]);
    assert_success(&output);
    assert_eq!(
        stdout(&output),
        format!(
            "{{\"thread_id\":\"4onylfsg\",\"latest\":{},\"warning\":\"\",\"mission\":null,\"task\":null}}\n",
            stored
        )
    );

    let list = fx.tsk(&["thread", "list"]);
    assert_success(&list);
    let row = json(stdout(&list).lines().next().unwrap());
    assert_eq!(row["id"], "4onylfsg");
    assert_eq!(row["latest_whats_next"], "stored entry");
    assert_eq!(row["latest_timestamp"], "2026-09-20T09:00:00Z");

    assert_success(&fx.tsk(&["thread", "pause", "4onylfsg", BRIEFING, "T-04", "new entry"]));
    let lines = fx.origin_entries("4onylfsg");
    assert_eq!(lines[0], stored);
    assert!(lines[1].contains("\"git\":{\"ledger\":{\"commit\":"));
}

#[test]
fn resume_refuses_an_unknown_thread() {
    let fx = Fixture::new();
    let output = fx.tsk(&["thread", "resume", "0000aaaa"]);
    assert_exit(&output, 1);
    assert_eq!(stdout(&output), "");
    assert!(stderr(&output).contains("no such thread: 0000aaaa"));
    assert!(!fx.marker().exists());
}

#[test]
fn resume_in_a_cloud_session_binds_locally_and_the_next_push_publishes_it() {
    let fx = Fixture::new();
    fx.seed_thread("5555eeee", "");
    let before = fx.origin_ledger_sha();

    let first = fx.tsk_cloud("cse_two", &["thread", "resume", "5555eeee"]);
    assert_success(&first);
    assert!(stderr(&first).contains(NOT_PUBLISHED));
    assert_eq!(fx.origin_ledger_sha(), before);
    assert!(!fx.origin_binding_threads().contains_key("cse_two"));
    assert_eq!(
        fx.local_binding_threads()
            .get("cse_two")
            .map(String::as_str),
        Some("5555eeee")
    );
    assert!(stdout(&fx.tsk(&["ledger", "status", "--local"])).contains("not pushed"));

    assert_success(&fx.tsk_cloud("cse_two", &["ledger", "push", "Publish the binding"]));
    assert_eq!(
        fx.origin_binding_threads()
            .get("cse_two")
            .map(String::as_str),
        Some("5555eeee")
    );
    assert_eq!(fx.origin_subject(), "Publish the binding");
    let after_push = fx.origin_ledger_sha();

    // A second resume of the same thread changes nothing, locally or on origin.
    let second = fx.tsk_cloud("cse_two", &["thread", "resume", "5555eeee"]);
    assert_success(&second);
    assert!(!stderr(&second).contains(NOT_PUBLISHED));
    assert_eq!(fx.origin_ledger_sha(), after_push);
}

#[test]
fn detach_removes_the_worktree_binding_locally_and_the_next_push_publishes_it() {
    let fx = Fixture::new();
    let id = fx.start();
    let before = fx.origin_ledger_sha();

    let output = fx.tsk(&["thread", "detach"]);

    assert_success(&output);
    assert_eq!(stdout(&output), format!("detached:{}\n", id));
    assert!(stderr(&output).contains(NOT_PUBLISHED));
    assert!(!fx.marker().exists());
    assert_exit(&fx.tsk(&["thread", "binding"]), 1);
    // Origin still holds the binding that start published.
    assert_eq!(fx.origin_ledger_sha(), before);
    assert!(fx
        .origin_binding_threads()
        .contains_key(&fx.worktree_actor()));
    assert!(fx.local_binding_threads().is_empty());
    assert!(stdout(&fx.tsk(&["ledger", "status", "--local"])).contains("not pushed"));

    assert_success(&fx.tsk(&["ledger", "push", "Publish the detach"]));
    assert!(fx.origin_binding_threads().is_empty());
    assert_eq!(fx.origin_subject(), "Publish the detach");
    assert_eq!(fx.origin_thread_ids(), vec![id]);
}

const LONG_AGO: &str = "2020-01-01T00:00:00Z";
const FAR_FUTURE: &str = "2999-01-01T00:00:00Z";

/// A fixture whose work clone holds four cloud bindings (two old, one in the future, and
/// the old one of `cse_mine`) and an old worktree binding.
fn fixture_with_old_and_new_bindings() -> Fixture {
    let fx = Fixture::new();
    fx.seed_thread("7777aaaa", "");
    fx.seed_cloud_bindings(&[
        ("cse_old1", "7777aaaa", LONG_AGO),
        ("cse_old2", "7777aaaa", "2020-02-01T00:00:00Z"),
        ("cse_future", "7777aaaa", FAR_FUTURE),
        ("cse_mine", "7777aaaa", LONG_AGO),
    ]);
    assert_success(&fx.tsk(&["thread", "resume", "7777aaaa"]));
    fx.exec_in(
        &fx.work(),
        &format!(
            "update bindings set registered_at = '{}' where kind = 'worktree'",
            LONG_AGO
        ),
    );
    fx
}

fn local_actors(fx: &Fixture) -> Vec<String> {
    fx.rows("select actor from bindings order by actor")
        .iter()
        .map(|row| row["actor"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn prune_bindings_removes_old_cloud_rows_and_keeps_the_rest() {
    let fx = fixture_with_old_and_new_bindings();
    let worktree = fx.worktree_actor();
    let before = fx.origin_ledger_sha();

    let output = fx.tsk_cloud("cse_mine", &["thread", "prune-bindings"]);

    assert_success(&output);
    assert_eq!(
        stdout(&output),
        "removed cse_old1 7777aaaa\nremoved cse_old2 7777aaaa\n2 bindings removed\n"
    );
    let mut expected = vec![
        "cse_future".to_string(),
        "cse_mine".to_string(),
        worktree.clone(),
    ];
    expected.sort();
    assert_eq!(local_actors(&fx), expected);
    // A local write: origin is unchanged until the push.
    assert_eq!(fx.origin_ledger_sha(), before);
    assert!(fx.origin_binding_threads().contains_key("cse_old1"));
    assert_success(&fx.tsk(&["ledger", "push", "Prune bindings"]));
    let published = fx.origin_binding_threads();
    assert!(!published.contains_key("cse_old1"));
    assert!(!published.contains_key("cse_old2"));
    assert!(published.contains_key("cse_future"));
    assert!(published.contains_key("cse_mine"));
    assert!(published.contains_key(&worktree));

    // Nothing is left to remove.
    let again = fx.tsk_cloud("cse_mine", &["thread", "prune-bindings"]);
    assert_success(&again);
    assert_eq!(stdout(&again), "0 bindings removed\n");
}

#[test]
fn prune_bindings_with_dry_run_prints_what_it_would_remove_and_changes_nothing() {
    let fx = fixture_with_old_and_new_bindings();
    let rows_before = local_actors(&fx);

    let output = fx.tsk_cloud(
        "cse_mine",
        &[
            "thread",
            "prune-bindings",
            "--dry-run",
            "--older-than",
            "30",
        ],
    );

    assert_success(&output);
    assert_eq!(
        stdout(&output),
        "would remove cse_old1 7777aaaa\nwould remove cse_old2 7777aaaa\n2 bindings would be removed\n"
    );
    assert_eq!(local_actors(&fx), rows_before);
}

#[test]
fn prune_bindings_never_removes_a_worktree_row_however_old() {
    let fx = fixture_with_old_and_new_bindings();
    let rows_before = local_actors(&fx);

    // From the worktree, every old cloud row goes, `cse_mine` included: it is not this
    // actor's binding. The old worktree row stays.
    let output = fx.tsk(&["thread", "prune-bindings", "--older-than", "0"]);

    assert_success(&output);
    assert_eq!(
        stdout(&output),
        "removed cse_mine 7777aaaa\nremoved cse_old1 7777aaaa\nremoved cse_old2 7777aaaa\n3 bindings removed\n"
    );
    let left = local_actors(&fx);
    assert!(left.contains(&fx.worktree_actor()));
    assert!(left.contains(&"cse_future".to_string()));
    assert_eq!(left.len(), rows_before.len() - 3);
}

#[test]
fn prune_bindings_honours_older_than_and_refuses_a_bad_number() {
    let fx = fixture_with_old_and_new_bindings();

    // No seeded row is 100000 days old, and the future row is never old.
    let output = fx.tsk(&["thread", "prune-bindings", "--older-than", "100000"]);
    assert_success(&output);
    assert_eq!(stdout(&output), "0 bindings removed\n");

    let bad = fx.tsk(&["thread", "prune-bindings", "--older-than", "soon"]);
    assert!(!bad.status.success());
    assert!(stderr(&bad).contains("--older-than"), "{}", stderr(&bad));
}

#[test]
fn detach_without_a_binding_exits_1_with_no_stdout() {
    let fx = Fixture::new();
    let output = fx.tsk(&["thread", "detach"]);
    assert_exit(&output, 1);
    assert_eq!(stdout(&output), "");
    assert!(stderr(&output).contains("nothing to detach"));
}

#[test]
fn detach_in_a_cloud_session_removes_only_its_own_binding() {
    let fx = Fixture::new();
    fx.seed_thread("6666ffff", "");
    fx.seed_cloud_bindings(&[
        ("cse_a", "6666ffff", "2026-10-01T00:00:00Z"),
        ("cse_b", "6666ffff", "2026-10-02T00:00:00Z"),
    ]);
    let before = fx.origin_ledger_sha();

    let output = fx.tsk_cloud("cse_a", &["thread", "detach"]);

    assert_success(&output);
    assert_eq!(stdout(&output), "detached:6666ffff\n");
    assert_eq!(fx.origin_ledger_sha(), before);
    let local = fx.local_binding_threads();
    assert_eq!(local.len(), 1);
    assert_eq!(local.get("cse_b").map(String::as_str), Some("6666ffff"));

    assert_success(&fx.tsk_cloud("cse_a", &["ledger", "push", "Publish the detach"]));
    let remaining = fx.origin_binding_threads();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining.get("cse_b").map(String::as_str), Some("6666ffff"));
    assert_eq!(fx.origin_subject(), "Publish the detach");
}

#[test]
fn stop_deletes_the_bound_thread_and_its_binding() {
    let fx = Fixture::new();
    let id = fx.start();

    let output = fx.tsk(&["thread", "stop"]);

    assert_success(&output);
    assert_eq!(stdout(&output), format!("stopped:{}\n", id));
    assert!(!fx.marker().exists());
    assert_exit(&fx.tsk(&["thread", "binding"]), 1);
    assert!(fx.origin_binding_threads().is_empty());
    assert!(fx.origin_thread_ids().is_empty());
    assert_eq!(fx.origin_subject(), format!("Stop thread {}", id));
}

#[test]
fn stop_by_id_purges_every_cloud_binding_and_keeps_an_unrelated_worktree_binding() {
    let fx = Fixture::new();
    fx.seed_thread("7777aaaa", "");
    fx.seed_thread("8888bbbb", "");
    fx.seed_cloud_bindings(&[
        ("cse_a", "7777aaaa", "2026-10-01T00:00:00Z"),
        ("cse_b", "8888bbbb", "2026-10-02T00:00:00Z"),
        ("cse_c", "7777aaaa", "2026-10-03T00:00:00Z"),
    ]);
    assert_success(&fx.tsk(&["thread", "resume", "8888bbbb"]));

    let output = fx.tsk(&["thread", "stop", "7777aaaa"]);

    assert_success(&output);
    assert_eq!(stdout(&output), "stopped:7777aaaa\n");
    assert_eq!(fx.origin_thread_ids(), vec!["8888bbbb".to_string()]);
    let remaining = fx.origin_binding_threads();
    assert_eq!(remaining.get("cse_b").map(String::as_str), Some("8888bbbb"));
    assert!(!remaining.contains_key("cse_a"));
    assert!(!remaining.contains_key("cse_c"));
    assert_eq!(fx.binding(), "worktree:8888bbbb\n");
}

#[test]
fn stop_without_a_binding_or_an_id_exits_1() {
    let fx = Fixture::new();
    let before = fx.origin_ledger_sha();
    let output = fx.tsk(&["thread", "stop"]);
    assert_exit(&output, 1);
    assert_eq!(stdout(&output), "");
    assert!(stderr(&output).contains("nothing to stop"));
    assert_eq!(fx.origin_ledger_sha(), before);
}

#[test]
fn stop_refuses_an_unknown_thread() {
    let fx = Fixture::new();
    let output = fx.tsk(&["thread", "stop", "0000aaaa"]);
    assert_exit(&output, 1);
    assert!(stderr(&output).contains("no such thread: 0000aaaa"));
}

#[test]
fn list_prints_one_compact_object_per_thread_newest_first() {
    let fx = Fixture::new();
    fx.seed_thread("aaaa0000", "");
    fx.seed_thread(
        "bbbb0000",
        "{\"whats_next\":\"older\",\"timestamp\":\"2026-09-01T00:00:00Z\"}\n",
    );
    fx.seed_thread(
        "cccc0000",
        "{\"whats_next\":\"first\",\"timestamp\":\"2026-08-01T00:00:00Z\"}\n{\"whats_next\":\"newer\",\"timestamp\":\"2026-10-01T00:00:00Z\"}\n",
    );

    let output = fx.tsk(&["thread", "list"]);

    assert_success(&output);
    let lines: Vec<String> = stdout(&output).lines().map(str::to_string).collect();
    assert_eq!(
        lines,
        vec![
            format!("{{\"id\":\"cccc0000\",\"mission_id\":\"M-TEST-01\",\"mission_link\":\"{}\",\"latest_whats_next\":\"newer\",\"latest_timestamp\":\"2026-10-01T00:00:00Z\",\"actors\":[]}}", BRIEFING),
            format!("{{\"id\":\"bbbb0000\",\"mission_id\":\"M-TEST-01\",\"mission_link\":\"{}\",\"latest_whats_next\":\"older\",\"latest_timestamp\":\"2026-09-01T00:00:00Z\",\"actors\":[]}}", BRIEFING),
            format!("{{\"id\":\"aaaa0000\",\"mission_id\":\"M-TEST-01\",\"mission_link\":\"{}\",\"latest_whats_next\":null,\"latest_timestamp\":null,\"actors\":[]}}", BRIEFING),
        ]
    );
}

#[test]
fn list_with_no_threads_prints_nothing() {
    let fx = Fixture::new();
    let output = fx.tsk(&["thread", "list"]);
    assert_success(&output);
    assert_eq!(stdout(&output), "");
}

#[test]
fn binding_prints_the_worktree_binding_or_exits_1() {
    let fx = Fixture::new();
    let miss = fx.tsk(&["thread", "binding"]);
    assert_exit(&miss, 1);
    assert_eq!(stdout(&miss), "");
    assert_eq!(stderr(&miss), "");

    let id = fx.start();
    let hit = fx.tsk(&["thread", "binding"]);
    assert_success(&hit);
    assert_eq!(stdout(&hit), format!("worktree:{}\n", id));
}

#[test]
fn binding_without_fetch_needs_an_existing_ledger_worktree() {
    let fx = Fixture::new();
    // Start from a clone that has not fetched the ledger.
    std::fs::remove_dir_all(fx.state().join("tsk")).unwrap();
    std::fs::remove_file(fx.work().join(".git").join("tsk-clone-id")).unwrap();
    write(&fx.marker(), "1234abcd\n");

    let before_fetch = fx.tsk(&["thread", "binding", "--no-fetch"]);
    assert_exit(&before_fetch, 1);
    assert_eq!(stdout(&before_fetch), "");
    assert!(!fx.state().join("tsk").exists());

    assert_success(&fx.tsk(&["ledger", "fetch"]));
    let after_fetch = fx.tsk(&["thread", "binding", "--no-fetch"]);
    assert_success(&after_fetch);
    assert_eq!(stdout(&after_fetch), "worktree:1234abcd\n");
}

#[test]
fn binding_without_fetch_reads_the_cloud_binding_in_the_existing_store() {
    let fx = Fixture::new();
    fx.seed_cloud_bindings(&[("cse_x", "9999aaaa", "2026-10-01T00:00:00Z")]);
    assert_success(&fx.tsk(&["ledger", "fetch"]));
    let other = fx.other();
    assert_success(
        &fx.command_in(&other, &["ledger", "fetch"], None)
            .output()
            .unwrap(),
    );
    fx.exec_in(&other, "delete from bindings where actor = 'cse_x'");
    assert_success(
        &fx.command_in(&other, &["ledger", "push", "Remove the binding"], None)
            .output()
            .unwrap(),
    );

    let local = fx.tsk_cloud("cse_x", &["thread", "binding", "--no-fetch"]);
    assert_success(&local);
    assert_eq!(stdout(&local), "cloud:9999aaaa\n");

    let fetched = fx.tsk_cloud("cse_x", &["thread", "binding"]);
    assert_exit(&fetched, 1);
}

#[test]
fn guard_blocks_when_unbound_and_passes_when_bound() {
    let fx = Fixture::new();
    let unbound = fx.tsk_stdin(&["thread", "guard"], "{\"session_id\":\"x\"}");
    assert_success(&unbound);
    let decision = json(&stdout(&unbound));
    assert_eq!(decision["decision"], "block");
    assert!(decision["reason"]
        .as_str()
        .unwrap()
        .starts_with("No thread binding was found for this session or worktree."));

    fx.start();
    let bound = fx.tsk_stdin(&["thread", "guard"], "{\"session_id\":\"x\"}");
    assert_success(&bound);
    assert_eq!(stdout(&bound), "");
}

#[test]
fn guard_does_not_ask_the_agent_to_push_unpublished_work() {
    // The Stop hook publishes in the background (ADR 0020); this fixture switches that off.
    let fx = Fixture::new();
    fx.start();
    let hook = "{\"session_id\":\"x\"}";
    assert_eq!(stdout(&fx.tsk_stdin(&["thread", "guard"], hook)), "");

    // A local write that no push has carried out: no block, whatever the hook input says.
    assert_success(&fx.tsk_stdin(&["ledger", "doc", "put", "notes/a.md"], "# a\n"));
    for input in [hook, "{\"stop_hook_active\":true}", "{}"] {
        let out = fx.tsk_stdin(&["thread", "guard"], input);
        assert_success(&out);
        assert_eq!(stdout(&out), "");
        assert_eq!(stderr(&out), "");
    }
    assert!(stdout(&fx.tsk(&["ledger", "status", "--local"])).contains("not pushed"));
}

#[test]
fn guard_asks_nothing_of_origin_for_unpublished_work() {
    let fx = Fixture::new();
    fx.start();
    assert_success(&fx.tsk_stdin(&["ledger", "doc", "put", "notes/a.md"], "# a\n"));
    let moved = fx.root.path().join("origin-moved.git");
    std::fs::rename(fx.origin(), &moved).unwrap();
    let out = fx.tsk_stdin(&["thread", "guard"], "{}");
    std::fs::rename(&moved, fx.origin()).unwrap();
    assert_success(&out);
    assert_eq!(stdout(&out), "");
}

#[test]
fn set_mission_writes_locally_and_the_next_push_publishes_it() {
    let fx = Fixture::new();
    let id = fx.start();
    let before = fx.origin_ledger_sha();

    let cleared = fx.tsk(&["thread", "set", "mission", "--none"]);

    assert_success(&cleared);
    assert_eq!(stdout(&cleared), format!("mission-cleared:{}\n", id));
    assert!(stderr(&cleared).contains(NOT_PUBLISHED));
    assert_eq!(fx.origin_ledger_sha(), before);
    let local = fx.rows(&format!(
        "select mission_id from threads where thread_id = '{}'",
        id
    ));
    assert!(local[0]["mission_id"].is_null());
    assert!(stdout(&fx.tsk(&["ledger", "status", "--local"])).contains("not pushed"));
    let remote = fx.origin_rows(&format!(
        "select mission_id from threads where thread_id = '{}'",
        id
    ));
    assert_eq!(remote[0]["mission_id"], "M-TEST-01");

    assert_success(&fx.tsk(&["ledger", "push", "Publish the cleared mission"]));
    let published = fx.origin_rows(&format!(
        "select mission_id from threads where thread_id = '{}'",
        id
    ));
    assert!(published[0]["mission_id"].is_null());
    assert_eq!(fx.origin_subject(), "Publish the cleared mission");
    let pushed = fx.origin_ledger_sha();

    let set = fx.tsk(&["thread", "set", "mission", "M-TEST-01"]);
    assert_success(&set);
    assert_eq!(stdout(&set), format!("mission-set:{}:M-TEST-01\n", id));
    assert_eq!(fx.origin_ledger_sha(), pushed);
    assert_success(&fx.tsk(&["ledger", "push", "Publish the mission"]));
    let published = fx.origin_rows(&format!(
        "select mission_id from threads where thread_id = '{}'",
        id
    ));
    assert_eq!(published[0]["mission_id"], "M-TEST-01");
    assert_eq!(fx.origin_subject(), "Publish the mission");
}

#[test]
fn start_publishes_the_claim_at_once_for_another_clone() {
    let fx = Fixture::new();
    let before = fx.origin_ledger_sha();

    let id = fx.start();

    // Origin moved, and the other clone sees the thread and its binding after its fetch.
    assert_ne!(fx.origin_ledger_sha(), before);
    let threads = fx.origin_rows(&format!(
        "select mission_id from threads where thread_id = '{}'",
        id
    ));
    assert_eq!(threads[0]["mission_id"], "M-TEST-01");
    let listed = fx
        .command_in(&fx.other(), &["thread", "list"], None)
        .output()
        .unwrap();
    assert_success(&listed);
    let row = json(stdout(&listed).lines().next().unwrap());
    assert_eq!(row["id"], id.as_str());
    assert_eq!(row["actors"][0]["actor"], fx.worktree_actor().as_str());
}

#[test]
fn pause_and_stop_publish_with_the_records_written_before_them() {
    let fx = Fixture::new();
    let id = fx.start();
    // A local write made earlier goes out with the pause.
    assert_success(&fx.tsk_stdin(&["ledger", "doc", "put", "notes/a.md"], "# a\n"));
    assert_success(&fx.tsk(&["thread", "detach"]));
    assert_success(&fx.tsk(&["thread", "resume", &id]));

    let output = fx.tsk(&["thread", "pause", &id, "", "next"]);
    assert_success(&output);
    assert_eq!(fx.origin_entries(&id).len(), 1);
    assert_eq!(
        fx.origin_binding_threads().get(&fx.worktree_actor()),
        Some(&id)
    );
    assert!(stdout(&fx.tsk(&["ledger", "status", "--local"])).contains("nothing to push"));

    assert_success(&fx.tsk(&["thread", "detach"]));
    let stopped = fx.tsk(&["thread", "stop", &id]);
    assert_success(&stopped);
    assert!(fx.origin_thread_ids().is_empty());
    assert!(fx.origin_binding_threads().is_empty());
    assert_eq!(fx.origin_subject(), format!("Stop thread {}", id));
}

#[test]
fn thread_commands_refuse_an_invalid_thread_id() {
    let fx = Fixture::new();
    let output = fx.tsk(&["thread", "stop", "../.."]);
    assert_exit(&output, 1);
    assert!(stderr(&output).contains("invalid thread id"));
}

fn session_start(fx: &Fixture, input: &str, env_file: Option<&Path>) -> Output {
    session_start_with(fx, &[], input, env_file)
}

fn session_start_with(
    fx: &Fixture,
    extra: &[&str],
    input: &str,
    env_file: Option<&Path>,
) -> Output {
    let mut base = vec!["thread", "session-start"];
    base.extend_from_slice(extra);
    let mut command = fx.command(&base, None);
    if let Some(file) = env_file {
        command.env("CLAUDE_ENV_FILE", file);
    } else {
        command.env_remove("CLAUDE_ENV_FILE");
    }
    let mut child = command
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

fn session_context(output: &Output) -> String {
    let value = json(&stdout(output));
    assert_eq!(stdout(output).lines().count(), 1);
    assert_eq!(value.as_object().unwrap().len(), 1);
    assert_eq!(value["hookSpecificOutput"]["hookEventName"], "SessionStart");
    value["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap()
        .to_string()
}

#[test]
fn session_start_fetches_exports_the_path_and_prints_the_unbound_prompt() {
    let fx = Fixture::new();
    let env_file = fx.root.path().join("claude-env");

    let output = session_start(&fx, "{\"source\":\"startup\"}", Some(&env_file));

    assert_success(&output);
    let wt = fx.ledger_path();
    assert!(wt.join(".dolt").is_dir());
    let context = session_context(&output);
    assert!(context.starts_with(&format!(
        "The ledger was fetched and materialised at {} (also exported as $TSK_LEDGER_WT). Rules for the ledger:",
        wt.display()
    )));
    assert!(context.contains("`tsk ledger status`"));
    assert!(!context.contains("index.md"));
    assert!(context.ends_with("run `tsk thread skip` instead."));
    assert!(context.contains("No thread binding was found for this session or worktree."));
    assert!(!context.contains("WARNING"));
    assert_eq!(
        std::fs::read_to_string(&env_file).unwrap(),
        format!("export TSK_LEDGER_WT=\"{}\"\n", wt.display())
    );
}

#[test]
fn session_start_copilot_format_prints_only_additional_context() {
    let fx = Fixture::new();

    let output = session_start_with(
        &fx,
        &["--format", "copilot"],
        "{\"sessionId\":\"cop-1\",\"timestamp\":1760000000000,\"cwd\":\"/x\",\"source\":\"new\"}",
        None,
    );

    assert_success(&output);
    assert_eq!(stdout(&output).lines().count(), 1);
    let value = json(&stdout(&output));
    assert_eq!(value.as_object().unwrap().len(), 1);
    let context = value["additionalContext"].as_str().unwrap();
    assert!(context.starts_with(&format!(
        "The ledger was fetched and materialised at {} (also exported as $TSK_LEDGER_WT).",
        fx.ledger_path().display()
    )));
    assert!(context.contains("No thread binding was found for this session or worktree."));

    let again = session_start_with(
        &fx,
        &["--format", "copilot"],
        "{\"sessionId\":\"cop-1\",\"source\":\"new\"}",
        None,
    );
    assert_success(&again);
    assert_eq!(stdout(&again), "");
}

#[test]
fn session_start_warns_about_an_older_plugin_in_both_formats() {
    let fx = Fixture::new();
    let output = session_start_with(
        &fx,
        &["--plugin-version", "0.0.1"],
        "{\"session_id\":\"p1\",\"source\":\"startup\"}",
        None,
    );
    assert_success(&output);
    let context = session_context(&output);
    assert!(
        context.contains("The tsk plugin loaded in this session is 0.0.1; this tsk expects "),
        "{}",
        context
    );
    assert!(context.contains("/plugin update tsk"));

    let copilot = session_start_with(
        &fx,
        &["--format", "copilot", "--plugin-version", "0.0.1"],
        "{\"sessionId\":\"p2\",\"source\":\"new\"}",
        None,
    );
    assert_success(&copilot);
    let value = json(&stdout(&copilot));
    assert!(value["additionalContext"]
        .as_str()
        .unwrap()
        .contains("The tsk plugin loaded in this session is 0.0.1;"));

    let newer = session_start_with(
        &fx,
        &["--plugin-version", "99.0.0"],
        "{\"session_id\":\"p3\",\"source\":\"startup\"}",
        None,
    );
    assert_success(&newer);
    assert!(!session_context(&newer).contains("plugin loaded"));
}

#[test]
fn session_start_accepts_empty_input_and_needs_no_env_file() {
    let fx = Fixture::new();

    let output = session_start(&fx, "", None);

    assert_success(&output);
    assert!(session_context(&output).contains("was fetched and materialised at"));
}

#[test]
fn session_start_appends_to_an_existing_env_file() {
    let fx = Fixture::new();
    let env_file = fx.root.path().join("claude-env");
    write(&env_file, "export OTHER=\"1\"\n");

    assert_success(&session_start(&fx, "{}", Some(&env_file)));

    let text = std::fs::read_to_string(&env_file).unwrap();
    assert!(text.starts_with("export OTHER=\"1\"\n"));
    assert_eq!(text.lines().count(), 2);
}

#[test]
fn session_start_runs_once_per_event_when_two_hooks_call_it() {
    let fx = Fixture::new();
    let env_file = fx.root.path().join("claude-env");
    let event = "{\"session_id\":\"sess-1\",\"source\":\"clear\"}";

    let first = session_start(&fx, event, Some(&env_file));
    let second = session_start(&fx, event, Some(&env_file));

    assert_success(&first);
    assert!(session_context(&first).contains("was fetched and materialised at"));
    assert_success(&second);
    assert_eq!(stdout(&second), "");
    assert_eq!(
        std::fs::read_to_string(&env_file).unwrap().lines().count(),
        1
    );

    let compact = session_start(
        &fx,
        "{\"session_id\":\"sess-1\",\"source\":\"compact\"}",
        Some(&env_file),
    );
    assert!(session_context(&compact).contains("was fetched and materialised at"));
}

#[test]
fn session_start_names_an_existing_binding_and_the_resume_command() {
    let fx = Fixture::new();
    let id = fx.start();

    let output = session_start(&fx, "{\"source\":\"resume\"}", None);

    assert_success(&output);
    let context = session_context(&output);
    assert!(context.ends_with(&format!(
        "An existing thread binding was found: worktree:{}. Run /tsk:resume-thread {} next.",
        id, id
    )));
    assert!(!context.contains("No thread binding was found"));
}

#[test]
fn session_start_names_a_cloud_binding() {
    let fx = Fixture::new();
    fx.seed_thread("5555eeee", "");
    assert_success(&fx.tsk_cloud("cse_two", &["thread", "resume", "5555eeee"]));
    assert_success(&fx.tsk_cloud("cse_two", &["ledger", "push", "Publish the binding"]));

    let mut command = fx.command(&["thread", "session-start"], Some("cse_two"));
    let output = command
        .stdin(Stdio::null())
        .output()
        .expect("failed to run tsk");

    assert_success(&output);
    let context = session_context(&output);
    assert!(context.contains(
        "An existing thread binding was found: cloud:5555eeee. Run /tsk:resume-thread 5555eeee next."
    ));
    // The cloud ID `cse_two` becomes the `session_two` that `set_session_title` takes.
    assert!(
        context.ends_with(
            "Session title: call set_session_title once, with session_id session_two and title \"M-TEST-01\"."
        ),
        "{}",
        context
    );
}

#[test]
fn session_start_warns_about_a_ledger_commit_that_is_not_on_origin() {
    let fx = Fixture::new();
    assert_success(&session_start(&fx, "{}", None));
    let wt = fx.ledger_path();
    fx.exec_in(
        &fx.work(),
        "insert into bindings (actor, kind, thread_id, registered_at) values ('cse_local', 'cloud', 'zzzzzzzz', '2026-01-01T00:00:00Z')",
    );
    common::stop_servers(fx.root.path(), true);
    for args in [
        vec!["add", "-A"],
        vec!["commit", "-m", "Unpushed ledger commit"],
    ] {
        let out = Command::new("dolt")
            .args(&args)
            .current_dir(&wt)
            .env("HOME", fx.home())
            .env("DOLT_DISABLE_EVENT_FLUSH", "1")
            .output()
            .unwrap();
        assert!(out.status.success(), "{:?}: {}", args, stderr(&out));
    }
    let origin_tip = fx.origin_ledger_sha();

    let output = session_start(&fx, "{}", None);

    assert_success(&output);
    let context = session_context(&output);
    assert!(context.contains(
        "WARNING: the ledger worktree holds a commit that is not on origin's refs/heads/tsk/ledger"
    ));
    assert!(context.contains("Unpushed ledger commit"));
    assert!(context.contains("dolt log origin/main..main"));
    assert!(context.contains("push it with tsk ledger push"));
    assert!(context.contains("No thread binding was found"));
    assert_eq!(fx.origin_ledger_sha(), origin_tip);
}
#[test]
fn session_start_exits_0_with_a_warning_when_the_fetch_fails() {
    let fx = Fixture::new();
    git(
        &fx.work(),
        &[
            "remote",
            "set-url",
            "origin",
            fx.root.path().join("missing.git").to_str().unwrap(),
        ],
    );
    let env_file = fx.root.path().join("claude-env");

    let output = session_start(&fx, "{}", Some(&env_file));

    assert_exit(&output, 0);
    let context = session_context(&output);
    assert!(context
        .starts_with("Warning: the ledger could not be fetched automatically at session start ("));
    assert!(context.ends_with("Run `tsk ledger fetch` manually before reading the missions."));
    assert!(!env_file.exists());
}

#[test]
fn session_start_exits_0_with_a_warning_outside_a_repository() {
    let fx = Fixture::new();
    let outside = fx.root.path().join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    let mut command = fx.command(&["thread", "session-start"], None);
    let output = command
        .current_dir(&outside)
        .stdin(Stdio::null())
        .output()
        .expect("failed to run tsk");

    assert_exit(&output, 0);
    assert!(session_context(&output).contains("Run `tsk ledger fetch` manually"));
}

fn fake_claude(fx: &Fixture, fail: bool) -> (PathBuf, PathBuf) {
    use std::os::unix::fs::PermissionsExt;
    let bin = fx.root.path().join("fake-claude");
    let log = fx.root.path().join("claude.log");
    let failure = if fail {
        "echo \"step failed: $*\" >&2\nexit 4\n"
    } else {
        "exit 0\n"
    };
    std::fs::write(
        &bin,
        format!(
            "#!/bin/sh\necho \"$*\" >> \"{}\"\n{}",
            log.display(),
            failure
        ),
    )
    .unwrap();
    std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
    (bin, log)
}

fn installing_session_start(
    fx: &Fixture,
    claude: &Path,
    remote: Option<&str>,
    input: &str,
) -> Output {
    let mut command = fx.command(
        &["thread", "session-start", "--install-plugin", "claude-cli"],
        None,
    );
    command
        .env("TSK_CLAUDE_BIN", claude)
        .env_remove("CLAUDE_ENV_FILE");
    match remote {
        Some(value) => command.env("CLAUDE_CODE_REMOTE", value),
        None => command.env_remove("CLAUDE_CODE_REMOTE"),
    };
    let mut child = command
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

fn logged_calls(log: &Path) -> Vec<String> {
    std::fs::read_to_string(log)
        .unwrap_or_default()
        .lines()
        .map(str::to_string)
        .collect()
}

#[test]
fn session_start_install_plugin_runs_no_claude_outside_a_cloud_session() {
    let fx = Fixture::new();
    let (claude, log) = fake_claude(&fx, false);

    for remote in [None, Some("false"), Some("")] {
        let output = installing_session_start(&fx, &claude, remote, "{}");
        assert_success(&output);
        session_context(&output);
        assert_eq!(stderr(&output), "");
    }
    assert!(logged_calls(&log).is_empty());
}

#[test]
fn session_start_install_plugin_runs_the_four_steps_in_a_cloud_session() {
    let fx = Fixture::new();
    let (claude, log) = fake_claude(&fx, false);

    let output = installing_session_start(&fx, &claude, Some("true"), "{\"session_id\":\"s-1\"}");

    assert_success(&output);
    let context = session_context(&output);
    assert!(!context.contains("could not be installed"));
    assert_eq!(logged_calls(&log).len(), 4);
    assert_eq!(
        logged_calls(&log)[1],
        "plugin install tsk@jimbarritt-claude-plugins --scope project -y"
    );
    assert_eq!(stderr(&output).lines().count(), 4);
}

#[test]
fn session_start_install_plugin_warns_in_the_context_when_a_step_fails() {
    let fx = Fixture::new();
    let (claude, _log) = fake_claude(&fx, true);

    let output = installing_session_start(&fx, &claude, Some("true"), "{}");

    assert_exit(&output, 0);
    let context = session_context(&output);
    assert!(context.contains("The ledger was fetched"));
    assert!(
        context.contains("Warning: the tsk plugin could not be installed at session start (error: could not add the jimbarritt/claude-plugins marketplace"),
        "{}",
        context
    );
    assert!(context.ends_with("Run `tsk install-plugin claude-cli` manually."));
}

#[test]
fn session_start_install_plugin_installs_nothing_for_a_claimed_event() {
    let fx = Fixture::new();
    let (claude, log) = fake_claude(&fx, false);
    let event = "{\"session_id\":\"s-9\",\"source\":\"startup\"}";

    assert_success(&installing_session_start(&fx, &claude, Some("true"), event));
    assert_eq!(logged_calls(&log).len(), 4);
    let second = installing_session_start(&fx, &claude, Some("true"), event);

    assert_exit(&second, 0);
    assert_eq!(stdout(&second), "");
    assert_eq!(stderr(&second), "");
    assert_eq!(logged_calls(&log).len(), 4);
}

#[test]
fn session_start_install_plugin_works_with_the_copilot_format_and_rejects_copilot_cli() {
    let fx = Fixture::new();
    let (claude, log) = fake_claude(&fx, false);
    let mut command = fx.command(
        &[
            "thread",
            "session-start",
            "--format",
            "copilot",
            "--install-plugin",
            "claude-cli",
        ],
        None,
    );
    let output = command
        .env("TSK_CLAUDE_BIN", &claude)
        .env("CLAUDE_CODE_REMOTE", "true")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_success(&output);
    assert!(json(&stdout(&output))["additionalContext"].is_string());
    assert_eq!(logged_calls(&log).len(), 4);

    let rejected = fx.tsk(&["thread", "session-start", "--install-plugin", "copilot-cli"]);
    assert_eq!(rejected.status.code(), Some(1));
    assert!(stderr(&rejected).contains("accepts only claude-cli"));
}

#[test]
fn version_prints_the_workspace_version() {
    let fx = Fixture::new();
    for flag in ["--version", "-V"] {
        let output = fx.tsk(&[flag]);
        assert_success(&output);
        assert_eq!(
            stdout(&output),
            format!("tsk {}\n", env!("CARGO_PKG_VERSION"))
        );
    }
    assert!(!env!("CARGO_PKG_VERSION").is_empty());
}

// --- Actors and harness instances (tsk actor) ---

/// The `actors` of the first `thread list` row, sorted by actor.
fn listed_actors(fx: &Fixture) -> Vec<serde_json::Value> {
    let output = fx.tsk(&["thread", "list"]);
    assert_success(&output);
    let text = stdout(&output);
    let row = json(text.lines().next().unwrap_or_else(|| panic!("no rows")));
    let mut actors = row["actors"].as_array().cloned().unwrap_or_default();
    actors.sort_by_key(|actor| actor["actor"].as_str().unwrap_or_default().to_string());
    actors
}

impl Fixture {
    fn register(&self, name: &str, kind: &str) -> Output {
        self.tsk(&["actor", "register", name, "--kind", kind])
    }

    fn actor_name(&self) -> String {
        let output = self.tsk(&["actor", "name"]);
        assert_success(&output);
        stdout(&output)
    }
}

#[test]
fn thread_title_is_the_actor_name_and_the_mission_of_the_thread() {
    let fx = Fixture::new();
    let title = |fx: &Fixture| {
        let output = fx.tsk(&["thread", "title"]);
        (output.status.code(), stdout(&output))
    };
    // No actor and no binding: nothing to print.
    assert_eq!(title(&fx), (Some(1), String::new()));

    fx.start();
    assert_eq!(title(&fx), (Some(0), "M-TEST-01\n".to_string()));

    assert_success(&fx.register("alice", "robot"));
    assert_eq!(title(&fx), (Some(0), "M-TEST-01: alice\n".to_string()));

    // A mission switch is a local write, and the title follows it at once.
    assert_success(&fx.tsk(&["thread", "set", "mission", "--none"]));
    assert_eq!(title(&fx), (Some(0), "alice\n".to_string()));
    assert_success(&fx.tsk(&["thread", "set", "mission", "M-TEST-01"]));
    assert_eq!(title(&fx), (Some(0), "M-TEST-01: alice\n".to_string()));
}

#[test]
fn actor_register_makes_the_actor_and_registers_this_worktree_as_it() {
    let fx = Fixture::new();
    let id = fx.start();
    assert_eq!(fx.actor_name(), "no actor registered\n");
    assert_eq!(fx.binding(), format!("worktree:{}\n", id));

    let before = fx.origin_ledger_sha();
    let registered = fx.register("alice", "robot");
    assert_success(&registered);
    assert_eq!(stdout(&registered), "registered alice (robot)\n");
    assert!(stderr(&registered).contains(NOT_PUBLISHED));
    // A local write: origin is unchanged until the next push.
    assert_eq!(fx.origin_ledger_sha(), before);
    assert_eq!(fx.actor_name(), "alice\n");
    assert_eq!(
        fx.rows("select name, kind from actors")[0],
        serde_json::json!({"name": "alice", "kind": "robot"})
    );
    assert_eq!(
        fx.rows("select instance, kind, actor_name from instances")[0],
        serde_json::json!({"instance": fx.worktree_actor(), "kind": "worktree", "actor_name": "alice"})
    );
    // The binding prints the actor on its own line, after the binding.
    assert_eq!(
        fx.binding(),
        format!("worktree:{}\nactor: alice (robot)\n", id)
    );
    assert_eq!(
        stdout(&fx.tsk(&["thread", "binding", "--no-fetch"])),
        format!("worktree:{}\nactor: alice (robot)\n", id)
    );

    assert_success(&fx.tsk(&["ledger", "push", "Publish the registration"]));
    assert_eq!(
        fx.origin_rows("select name from actors")[0]["name"],
        "alice"
    );
    assert_eq!(
        fx.origin_rows("select actor_name from instances")[0]["actor_name"],
        "alice"
    );
}

#[test]
fn actor_register_needs_no_thread_binding() {
    let fx = Fixture::new();
    assert_exit(&fx.tsk(&["thread", "binding"]), 1);

    let registered = fx.register("alice", "human");
    assert_success(&registered);
    assert_eq!(stdout(&registered), "registered alice (human)\n");
    assert_eq!(fx.actor_name(), "alice\n");
    assert!(fx.rows("select actor from bindings").is_empty());

    // A cloud session with no binding registers too.
    let cloud = fx.tsk_cloud("cse_free", &["actor", "register", "bot", "--kind", "robot"]);
    assert_success(&cloud);
    assert_eq!(stdout(&cloud), "registered bot (robot)\n");
    assert_eq!(
        stdout(&fx.tsk_cloud("cse_free", &["actor", "name"])),
        "bot\n"
    );
    assert_eq!(fx.actor_name(), "alice\n");
}

#[test]
fn actor_register_joins_an_existing_name_of_the_same_kind_and_refuses_the_other_kind() {
    let fx = Fixture::new();
    assert_success(&fx.register("alice", "robot"));

    // The same name and kind from another instance joins the actor.
    let joined = fx.tsk_cloud(
        "cse_two",
        &["actor", "register", "alice", "--kind", "robot"],
    );
    assert_success(&joined);
    assert_eq!(
        stdout(&joined),
        "alice (robot) is now this session's actor\n"
    );
    assert_eq!(
        stdout(&fx.tsk_cloud("cse_two", &["actor", "name"])),
        "alice\n"
    );
    assert_eq!(fx.rows("select name from actors").len(), 1);
    assert_eq!(fx.rows("select instance from instances").len(), 2);

    // Registering again from the same instance is a join too, and keeps one row.
    let again = fx.register("alice", "robot");
    assert_success(&again);
    assert_eq!(
        stdout(&again),
        "alice (robot) is now this worktree's actor\n"
    );
    assert_eq!(fx.rows("select instance from instances").len(), 2);

    // The other kind is refused, and nothing changes.
    let refused = fx.tsk_cloud(
        "cse_three",
        &["actor", "register", "alice", "--kind", "human"],
    );
    assert_exit(&refused, 1);
    assert_eq!(stdout(&refused), "");
    assert!(
        stderr(&refused).contains("alice is registered as a robot"),
        "{}",
        stderr(&refused)
    );
    assert_eq!(fx.rows("select instance from instances").len(), 2);
    assert_eq!(
        stdout(&fx.tsk_cloud("cse_three", &["actor", "name"])),
        "no actor registered\n"
    );

    // An instance moves to another actor by registering as it.
    assert_eq!(
        stdout(&fx.register("bob", "robot")),
        "registered bob (robot)\n"
    );
    assert_eq!(fx.actor_name(), "bob\n");
    assert_eq!(fx.rows("select instance from instances").len(), 2);
}

#[test]
fn actor_register_rejects_an_empty_a_long_and_a_path_like_name() {
    let fx = Fixture::new();
    let long = "x".repeat(201);
    for (name, says) in [
        ("", "empty"),
        ("   ", "empty"),
        (long.as_str(), "limit is 200"),
        ("/tmp/x", "start with /"),
    ] {
        let output = fx.register(name, "robot");
        assert_exit(&output, 1);
        assert_eq!(stdout(&output), "");
        assert!(stderr(&output).contains(says), "{}", stderr(&output));
    }
    assert_exit(&fx.tsk(&["actor", "register", "alice", "--kind", "cat"]), 2);
    assert_eq!(fx.actor_name(), "no actor registered\n");
    assert!(fx.rows("select name from actors").is_empty());
    let longest = "y".repeat(200);
    assert_success(&fx.register(&longest, "human"));
    assert_eq!(fx.actor_name(), format!("{}\n", longest));
}

#[test]
fn actor_list_prints_each_actor_with_its_instances() {
    let fx = Fixture::new();
    assert_eq!(fx.ok_list(&["actor", "list"]), "");
    assert_eq!(fx.ok_list(&["actor", "list", "--json"]), "[]\n");
    assert_success(&fx.register("alice", "robot"));
    // Two registrations in one second sort by key: these keys sort in that order too.
    assert_success(&fx.tsk_cloud("cse_2", &["actor", "register", "bob", "--kind", "human"]));
    assert_success(&fx.tsk_cloud("cse_3", &["actor", "register", "bob", "--kind", "human"]));
    let main = fx.worktree_actor();

    assert_eq!(
        fx.ok_list(&["actor", "list", "--porcelain"]),
        format!("alice\trobot\t{}\nbob\thuman\tcse_2,cse_3\n", main)
    );
    let aligned = fx.ok_list(&["actor", "list"]);
    assert_eq!(
        aligned,
        format!("alice  robot  {}\nbob    human  cse_2,cse_3\n", main)
    );
    let listed = json(&fx.ok_list(&["actor", "list", "--json"]));
    assert_eq!(listed.as_array().unwrap().len(), 2);
    assert_eq!(listed[0]["name"], "alice");
    assert_eq!(listed[0]["kind"], "robot");
    assert_eq!(listed[0]["instances"][0]["instance"], main);
    assert_eq!(listed[0]["instances"][0]["kind"], "worktree");
    assert_eq!(listed[1]["name"], "bob");
    assert_eq!(listed[1]["instances"].as_array().unwrap().len(), 2);
    assert_eq!(listed[1]["instances"][1]["instance"], "cse_3");
    assert!(listed[1]["instances"][1]["registered_at"].is_string());
}

impl Fixture {
    fn ok_list(&self, args: &[&str]) -> String {
        let output = self.tsk(args);
        assert_success(&output);
        stdout(&output)
    }
}

#[test]
fn the_actor_stays_while_the_instance_rebinds() {
    let fx = Fixture::new();
    fx.seed_thread("7777aaaa", "");
    let id = fx.start();
    assert_success(&fx.register("alice", "robot"));

    assert_success(&fx.tsk(&["thread", "resume", "7777aaaa"]));
    assert_eq!(fx.binding(), "worktree:7777aaaa\nactor: alice (robot)\n");
    assert_success(&fx.tsk(&["thread", "detach"]));
    assert_exit(&fx.tsk(&["thread", "binding"]), 1);
    assert_eq!(fx.actor_name(), "alice\n");
    assert_success(&fx.tsk(&["thread", "resume", &id]));
    assert_success(&fx.tsk(&["thread", "stop"]));
    assert_eq!(fx.actor_name(), "alice\n");
    let started = fx.start();
    assert_eq!(
        fx.binding(),
        format!("worktree:{}\nactor: alice (robot)\n", started)
    );
}

#[test]
fn list_shows_the_actor_name_of_each_instance() {
    let fx = Fixture::new();
    let id = fx.start();
    assert_success(&fx.tsk_cloud("cse_n", &["thread", "resume", &id]));
    assert_eq!(
        listed_actors(&fx),
        vec![
            serde_json::json!({"actor": "cse_n", "kind": "cloud", "name": null}),
            serde_json::json!({"actor": fx.worktree_actor(), "kind": "worktree", "name": null}),
        ]
    );
    assert_success(&fx.register("alice", "robot"));
    assert_eq!(
        listed_actors(&fx),
        vec![
            serde_json::json!({"actor": "cse_n", "kind": "cloud", "name": null}),
            serde_json::json!({"actor": fx.worktree_actor(), "kind": "worktree", "name": "alice"}),
        ]
    );
}

#[test]
fn session_start_names_the_actor_when_this_instance_is_registered() {
    let fx = Fixture::new();
    let id = fx.start();
    assert_success(&fx.register("alice", "robot"));
    assert_success(&fx.tsk(&["ledger", "push", "Publish the registration"]));

    let output = session_start(&fx, "{\"source\":\"resume\"}", None);

    assert_success(&output);
    let context = session_context(&output);
    assert!(
        context.ends_with(&format!(
            "An existing thread binding was found: worktree:{}. Run /tsk:resume-thread {} next. Actor: alice (robot).",
            id, id
        )),
        "{}",
        context
    );
}
