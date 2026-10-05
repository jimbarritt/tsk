use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use tempfile::TempDir;

const LEDGER_REF: &str = "refs/heads/tsk/ledger";
const BRIEFING: &str = "missions/operational/M-TEST-01-example.md";
const SESSION_VAR: &str = "CLAUDE_CODE_REMOTE_SESSION_ID";

struct Fixture {
    root: TempDir,
}

impl Fixture {
    fn new() -> Fixture {
        Fixture::with_default_branch("main")
    }

    fn with_default_branch(branch: &str) -> Fixture {
        let fixture = Fixture {
            root: tempfile::tempdir().unwrap(),
        };
        let root = fixture.root.path();
        std::fs::create_dir_all(fixture.home()).unwrap();
        std::fs::create_dir_all(fixture.state()).unwrap();
        git(
            root,
            &["init", "--quiet", "--bare", "-b", branch, "origin.git"],
        );
        git(root, &["init", "--quiet", "-b", branch, "seed"]);
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
            &[
                "push",
                "--quiet",
                "origin",
                &format!("HEAD:refs/heads/{}", branch),
            ],
        );
        git(
            &fixture.seed(),
            &["switch", "--quiet", "--orphan", "ledger-seed"],
        );
        write(&fixture.seed().join(".tsk-ledger.toml"), "version = 1\n");
        write(&fixture.seed().join("index.md"), "# Ledger index\n");
        write(&fixture.seed().join(BRIEFING), "# Mission: example\n");
        commit_all(&fixture.seed(), "Seed the ledger");
        fixture.push_ledger();
        git(
            root,
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

    fn marker(&self) -> PathBuf {
        self.work().join(".git").join("tsk-thread-id")
    }

    fn push_ledger(&self) {
        git(
            &self.seed(),
            &["push", "--quiet", "origin", &format!("HEAD:{}", LEDGER_REF)],
        );
    }

    fn seed_thread(&self, id: &str, store: &str) {
        let dir = self.seed().join("threads").join(id);
        write(
            &dir.join("index.md"),
            &format!(
                "# Thread {}\n\nMission briefing: [{}]({})\n",
                id, BRIEFING, BRIEFING
            ),
        );
        write(&dir.join("continuation-state.jsonl"), store);
        commit_all(&self.seed(), &format!("Seed thread {}", id));
        self.push_ledger();
    }

    fn seed_lookup(&self, text: &str) {
        write(
            &self
                .seed()
                .join("threads")
                .join("lookup-by-cloud-session.json"),
            text,
        );
        commit_all(&self.seed(), "Seed the cloud session lookup");
        self.push_ledger();
    }

    fn origin_ledger_sha(&self) -> String {
        git_line(&self.origin(), &["rev-parse", LEDGER_REF])
    }

    fn origin_file(&self, file: &str) -> Option<String> {
        let output = git_output(
            &self.origin(),
            &["cat-file", "blob", &format!("{}:{}", LEDGER_REF, file)],
        );
        if output.status.success() {
            Some(String::from_utf8(output.stdout).unwrap())
        } else {
            None
        }
    }

    fn command(&self, args: &[&str], cloud: Option<&str>) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_tsk"));
        command
            .args(args)
            .current_dir(self.work())
            .envs(git_env())
            .env("HOME", self.home())
            .env("XDG_STATE_HOME", self.state())
            .env("TSK_HOME", self.home().join(".tsk"))
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
        let output = self.tsk(&["thread", "start", "M-TEST-01", BRIEFING]);
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
    assert_eq!(
        fx.origin_file(&format!("threads/{}/index.md", id)).unwrap(),
        format!(
            "# Thread {}\n\nMission briefing: [{}]({})\n",
            id, BRIEFING, BRIEFING
        )
    );
    assert_eq!(
        fx.origin_file(&format!("threads/{}/continuation-state.jsonl", id))
            .unwrap(),
        ""
    );
    assert_eq!(
        std::fs::read_to_string(fx.marker()).unwrap(),
        format!("{}\n", id)
    );
    assert!(fx
        .origin_file("threads/lookup-by-cloud-session.json")
        .is_none());
    let subject = git_line(&fx.origin(), &["log", "-1", "--format=%s", LEDGER_REF]);
    assert_eq!(
        subject,
        format!("Start thread {} for mission M-TEST-01", id)
    );
}

#[test]
fn start_with_an_existing_binding_requires_a_resume() {
    let fx = Fixture::new();
    let id = fx.start();
    let before = fx.origin_ledger_sha();

    let output = fx.tsk(&["thread", "start", "M-TEST-01", BRIEFING]);

    assert_exit(&output, 2);
    assert_eq!(stdout(&output), format!("resume-required:{}\n", id));
    assert_eq!(fx.origin_ledger_sha(), before);
}

#[test]
fn start_refuses_a_briefing_that_is_not_on_the_ledger() {
    let fx = Fixture::new();
    let before = fx.origin_ledger_sha();

    let output = fx.tsk(&["thread", "start", "M-NONE", "missions/M-NONE.md"]);

    assert_exit(&output, 1);
    assert_eq!(stdout(&output), "");
    assert!(stderr(&output).contains("mission briefing not found"));
    assert!(!fx.marker().exists());
    assert_eq!(fx.origin_ledger_sha(), before);
}

#[test]
fn start_in_a_cloud_session_binds_through_the_lookup() {
    let fx = Fixture::new();
    let output = fx.tsk_cloud("cse_one", &["thread", "start", "M-TEST-01", BRIEFING]);
    assert_success(&output);
    let id = stdout(&output)
        .trim_end()
        .strip_prefix("started:")
        .unwrap()
        .to_string();

    let lookup = json(
        &fx.origin_file("threads/lookup-by-cloud-session.json")
            .unwrap(),
    );
    assert_eq!(lookup["cse_one"]["thread_id"], id.as_str());
    assert!(lookup["cse_one"]["registered_at"]
        .as_str()
        .unwrap()
        .ends_with('Z'));
    assert!(!fx.marker().exists());

    let binding = fx.tsk_cloud("cse_one", &["thread", "binding"]);
    assert_success(&binding);
    assert_eq!(stdout(&binding), format!("cloud:{}\n", id));
}

#[test]
fn pause_appends_an_entry_with_the_nested_git_state_and_pushes() {
    let fx = Fixture::new();
    let id = fx.start();
    let ledger_before = fx.origin_ledger_sha();
    let main_head = git_line(&fx.work(), &["rev-parse", "HEAD"]);

    let output = fx.tsk(&[
        "thread",
        "pause",
        &id,
        BRIEFING,
        "T-05",
        "Write the \"thread\" tests",
    ]);

    assert_success(&output);
    assert_eq!(stdout(&output), format!("paused:{}\n", id));
    let store = fx
        .origin_file(&format!("threads/{}/continuation-state.jsonl", id))
        .unwrap();
    assert_eq!(store.lines().count(), 1);
    assert!(store.ends_with('\n'));
    let line = store.lines().next().unwrap();
    let keys: Vec<_> = json(line).as_object().unwrap().keys().cloned().collect();
    assert_eq!(keys.len(), 6);
    let positions: Vec<_> = [
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
    assert_eq!(entry["mission_link"], BRIEFING);
    assert_eq!(entry["task_id"], "T-05");
    assert_eq!(entry["whats_next"], "Write the \"thread\" tests");
    assert_eq!(entry["git"]["ledger"]["commit"], ledger_before.as_str());
    assert_eq!(entry["git"]["code"]["ref"], "refs/heads/main");
    assert_eq!(entry["git"]["code"]["commit"], main_head.as_str());
    assert_eq!(entry["git"]["ledger"].as_object().unwrap().len(), 1);
    assert_eq!(entry["git"]["code"].as_object().unwrap().len(), 2);
    assert_eq!(entry["written_by"], "urn:tsk:worktree:.git");
    assert_eq!(entry["timestamp"].as_str().unwrap().len(), 20);
    let subject = git_line(&fx.origin(), &["log", "-1", "--format=%s", LEDGER_REF]);
    assert_eq!(subject, format!("Pause thread {}: T-05", id));
}

fn assert_pause_not_recorded(fx: &Fixture, id: &str, before: &str) {
    assert_eq!(fx.origin_ledger_sha(), before);
    let wt = fx.ledger_path();
    assert_eq!(
        std::fs::read_to_string(wt.join(format!("threads/{}/continuation-state.jsonl", id)))
            .unwrap(),
        ""
    );
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
    let store = fx
        .origin_file(&format!("threads/{}/continuation-state.jsonl", id))
        .unwrap();
    let entry = json(store.lines().next().unwrap());
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
    let store = fx
        .origin_file(&format!("threads/{}/continuation-state.jsonl", id))
        .unwrap();
    let entry = json(store.lines().next().unwrap());
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
    let before = fx.origin_ledger_sha();

    let output = fx.tsk(&["thread", "pause", &id, BRIEFING, "T-05", "next"]);

    assert_exit(&output, 1);
    assert!(stderr(&output).contains("is not reachable from any branch on origin"));
    assert_pause_not_recorded(&fx, &id, &before);
}

#[test]
fn pause_works_with_a_non_main_default_branch() {
    let fx = Fixture::with_default_branch("trunk");
    let id = fx.start();

    let output = fx.tsk(&["thread", "pause", &id, BRIEFING, "T-05", "next"]);

    assert_success(&output);
    let store = fx
        .origin_file(&format!("threads/{}/continuation-state.jsonl", id))
        .unwrap();
    let entry = json(store.lines().next().unwrap());
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
    let output = fx.tsk(&["thread", "pause", "0000aaaa", BRIEFING, "T-05", "next"]);
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
    assert_eq!(
        std::fs::read_to_string(fx.marker()).unwrap(),
        format!("{}\n", id)
    );
    assert_eq!(fx.origin_ledger_sha(), before);
}

#[test]
fn resume_of_a_thread_with_no_entry_prints_an_empty_latest() {
    let fx = Fixture::new();
    fx.seed_thread("1111aaaa", "");

    let output = fx.tsk(&["thread", "resume", "1111aaaa"]);

    assert_success(&output);
    assert_eq!(
        stdout(&output),
        "{\"thread_id\":\"1111aaaa\",\"latest\":{},\"warning\":\"\"}\n"
    );
}

#[test]
fn resume_warns_and_still_binds_when_another_actor_wrote_to_the_thread() {
    let fx = Fixture::new();
    fx.seed_thread(
        "2222bbbb",
        "{\"mission_link\":\"m\",\"task_id\":\"T-01\",\"whats_next\":\"a\",\"git\":{\"ledger\":{\"commit\":\"c\"},\"code\":{\"ref\":\"refs/heads/main\",\"commit\":\"d\"}},\"timestamp\":\"2026-10-01T00:00:00Z\",\"written_by\":\"urn:tsk:cloudsession:cse_other\"}\n\
         {\"mission_link\":\"m\",\"task_id\":\"T-01\",\"whats_next\":\"b\",\"git\":{\"ledger\":{\"commit\":\"c\"},\"code\":{\"ref\":\"refs/heads/main\",\"commit\":\"d\"}},\"timestamp\":\"2026-10-02T00:00:00Z\",\"written_by\":\"urn:tsk:worktree:feature\"}\n",
    );

    let output = fx.tsk(&["thread", "resume", "2222bbbb"]);

    assert_success(&output);
    let result = json(&stdout(&output));
    assert_eq!(
        result["warning"],
        "thread 2222bbbb is already associated with other actor(s): urn:tsk:cloudsession:cse_other,urn:tsk:worktree:feature"
    );
    assert_eq!(result["latest"]["whats_next"], "b");
    assert_eq!(std::fs::read_to_string(fx.marker()).unwrap(), "2222bbbb\n");
}

#[test]
fn resume_does_not_warn_when_the_current_actor_wrote_to_the_thread() {
    let fx = Fixture::new();
    fx.seed_thread(
        "3333cccc",
        "{\"whats_next\":\"a\",\"written_by\":\"urn:tsk:cloudsession:cse_other\"}\n\
         {\"whats_next\":\"b\",\"written_by\":\"urn:tsk:worktree:.git\"}\n",
    );

    let output = fx.tsk(&["thread", "resume", "3333cccc"]);

    assert_success(&output);
    assert_eq!(json(&stdout(&output))["warning"], "");
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
            "{{\"thread_id\":\"4onylfsg\",\"latest\":{},\"warning\":\"\"}}\n",
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
    let store = fx
        .origin_file("threads/4onylfsg/continuation-state.jsonl")
        .unwrap();
    let lines: Vec<_> = store.lines().collect();
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
fn resume_in_a_cloud_session_binds_once_and_pushes() {
    let fx = Fixture::new();
    fx.seed_thread("5555eeee", "");

    let first = fx.tsk_cloud("cse_two", &["thread", "resume", "5555eeee"]);
    assert_success(&first);
    let lookup = json(
        &fx.origin_file("threads/lookup-by-cloud-session.json")
            .unwrap(),
    );
    assert_eq!(lookup["cse_two"]["thread_id"], "5555eeee");
    let subject = git_line(&fx.origin(), &["log", "-1", "--format=%s", LEDGER_REF]);
    assert_eq!(
        subject,
        "Bind urn:tsk:cloudsession:cse_two to thread 5555eeee"
    );
    let after_first = fx.origin_ledger_sha();

    let second = fx.tsk_cloud("cse_two", &["thread", "resume", "5555eeee"]);
    assert_success(&second);
    assert_eq!(fx.origin_ledger_sha(), after_first);
}

#[test]
fn detach_removes_the_worktree_marker_only() {
    let fx = Fixture::new();
    let id = fx.start();
    let before = fx.origin_ledger_sha();

    let output = fx.tsk(&["thread", "detach"]);

    assert_success(&output);
    assert_eq!(stdout(&output), format!("detached:{}\n", id));
    assert!(!fx.marker().exists());
    assert_eq!(fx.origin_ledger_sha(), before);
    assert!(fx
        .origin_file(&format!("threads/{}/index.md", id))
        .is_some());
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
fn detach_in_a_cloud_session_removes_only_its_lookup_entry() {
    let fx = Fixture::new();
    fx.seed_thread("6666ffff", "");
    fx.seed_lookup(
        "{\n  \"cse_a\": {\n    \"thread_id\": \"6666ffff\",\n    \"registered_at\": \"2026-10-01T00:00:00Z\"\n  },\n  \"cse_b\": {\n    \"thread_id\": \"6666ffff\",\n    \"registered_at\": \"2026-10-02T00:00:00Z\"\n  }\n}\n",
    );

    let output = fx.tsk_cloud("cse_a", &["thread", "detach"]);

    assert_success(&output);
    assert_eq!(stdout(&output), "detached:6666ffff\n");
    assert_eq!(
        fx.origin_file("threads/lookup-by-cloud-session.json").unwrap(),
        "{\n  \"cse_b\": {\n    \"thread_id\": \"6666ffff\",\n    \"registered_at\": \"2026-10-02T00:00:00Z\"\n  }\n}\n"
    );
    let subject = git_line(&fx.origin(), &["log", "-1", "--format=%s", LEDGER_REF]);
    assert_eq!(
        subject,
        "Detach urn:tsk:cloudsession:cse_a from thread 6666ffff"
    );
}

#[test]
fn stop_deletes_the_bound_thread_and_its_marker() {
    let fx = Fixture::new();
    let id = fx.start();

    let output = fx.tsk(&["thread", "stop"]);

    assert_success(&output);
    assert_eq!(stdout(&output), format!("stopped:{}\n", id));
    assert!(!fx.marker().exists());
    assert!(fx
        .origin_file(&format!("threads/{}/index.md", id))
        .is_none());
    let subject = git_line(&fx.origin(), &["log", "-1", "--format=%s", LEDGER_REF]);
    assert_eq!(subject, format!("Stop thread {}", id));
}

#[test]
fn stop_by_id_purges_every_cloud_binding_and_keeps_an_unrelated_marker() {
    let fx = Fixture::new();
    fx.seed_thread("7777aaaa", "");
    fx.seed_thread("8888bbbb", "");
    fx.seed_lookup(
        "{\n  \"cse_a\": {\n    \"thread_id\": \"7777aaaa\",\n    \"registered_at\": \"2026-10-01T00:00:00Z\"\n  },\n  \"cse_b\": {\n    \"thread_id\": \"8888bbbb\",\n    \"registered_at\": \"2026-10-02T00:00:00Z\"\n  },\n  \"cse_c\": {\n    \"thread_id\": \"7777aaaa\",\n    \"registered_at\": \"2026-10-03T00:00:00Z\"\n  }\n}\n",
    );
    assert_success(&fx.tsk(&["thread", "resume", "8888bbbb"]));

    let output = fx.tsk(&["thread", "stop", "7777aaaa"]);

    assert_success(&output);
    assert_eq!(stdout(&output), "stopped:7777aaaa\n");
    assert!(fx.origin_file("threads/7777aaaa/index.md").is_none());
    assert!(fx.origin_file("threads/8888bbbb/index.md").is_some());
    assert_eq!(
        fx.origin_file("threads/lookup-by-cloud-session.json").unwrap(),
        "{\n  \"cse_b\": {\n    \"thread_id\": \"8888bbbb\",\n    \"registered_at\": \"2026-10-02T00:00:00Z\"\n  }\n}\n"
    );
    assert_eq!(std::fs::read_to_string(fx.marker()).unwrap(), "8888bbbb\n");
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
    fx.seed_lookup("{}\n");

    let output = fx.tsk(&["thread", "list"]);

    assert_success(&output);
    let lines: Vec<String> = stdout(&output).lines().map(str::to_string).collect();
    assert_eq!(
        lines,
        vec![
            format!("{{\"id\":\"cccc0000\",\"mission_link\":\"{}\",\"latest_whats_next\":\"newer\",\"latest_timestamp\":\"2026-10-01T00:00:00Z\"}}", BRIEFING),
            format!("{{\"id\":\"bbbb0000\",\"mission_link\":\"{}\",\"latest_whats_next\":\"older\",\"latest_timestamp\":\"2026-09-01T00:00:00Z\"}}", BRIEFING),
            format!("{{\"id\":\"aaaa0000\",\"mission_link\":\"{}\",\"latest_whats_next\":null,\"latest_timestamp\":null}}", BRIEFING),
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
fn binding_without_fetch_reads_the_cloud_lookup_in_the_existing_worktree() {
    let fx = Fixture::new();
    fx.seed_lookup(
        "{\"cse_x\":{\"thread_id\":\"9999aaaa\",\"registered_at\":\"2026-10-01T00:00:00Z\"}}\n",
    );
    assert_success(&fx.tsk(&["ledger", "fetch"]));
    fx.seed_lookup("{}\n");

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
fn thread_commands_refuse_an_invalid_thread_id() {
    let fx = Fixture::new();
    let output = fx.tsk(&["thread", "stop", "../.."]);
    assert_exit(&output, 1);
    assert!(stderr(&output).contains("invalid thread id"));
}

fn session_start(fx: &Fixture, input: &str, env_file: Option<&Path>) -> Output {
    let mut command = fx.command(&["thread", "session-start"], None);
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
    assert!(wt.join("index.md").is_file());
    let context = session_context(&output);
    assert!(context.starts_with(&format!(
        "The ledger was fetched and materialised at {} (also exported as $TSK_LEDGER_WT). Read {}/index.md next.",
        wt.display(),
        wt.display()
    )));
    assert!(context.ends_with("if an existing thread is named instead."));
    assert!(context.contains("No thread binding was found for this session or worktree."));
    assert!(!context.contains("WARNING"));
    assert_eq!(
        std::fs::read_to_string(&env_file).unwrap(),
        format!("export TSK_LEDGER_WT=\"{}\"\n", wt.display())
    );
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

    let mut command = fx.command(&["thread", "session-start"], Some("cse_two"));
    let output = command
        .stdin(Stdio::null())
        .output()
        .expect("failed to run tsk");

    assert_success(&output);
    assert!(session_context(&output).ends_with(
        "An existing thread binding was found: cloud:5555eeee. Run /tsk:resume-thread 5555eeee next."
    ));
}

#[test]
fn session_start_warns_about_a_ledger_commit_that_is_not_on_origin() {
    let fx = Fixture::new();
    assert_success(&session_start(&fx, "{}", None));
    let wt = fx.ledger_path();
    write(&wt.join("local.md"), "local\n");
    commit_all(&wt, "Unpushed ledger commit");
    let local_head = git_line(&wt, &["rev-parse", "HEAD"]);
    let origin_tip = fx.origin_ledger_sha();

    let output = session_start(&fx, "{}", None);

    assert_success(&output);
    let context = session_context(&output);
    assert!(context.contains(
        "WARNING: the ledger worktree holds a commit that is not on origin's refs/heads/tsk/ledger"
    ));
    assert!(context.contains("Unpushed ledger commit"));
    assert!(context.contains(&format!("log -p {}..HEAD", origin_tip)));
    assert!(context.contains("push it with tsk ledger push"));
    assert!(context.contains("No thread binding was found"));
    assert_eq!(git_line(&wt, &["rev-parse", "HEAD"]), local_head);
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
    assert!(context.ends_with("Run `tsk ledger fetch` manually before reading index.md."));
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
