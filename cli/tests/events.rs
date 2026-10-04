use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use tempfile::TempDir;

const LEDGER_REF: &str = "refs/heads/tsk/ledger";
const QUEUE: &str = "external-events/queue.ndjson";
const WATERMARK: &str = "external-events/watermark.json";

struct Fixture {
    root: TempDir,
}

impl Fixture {
    fn new() -> Fixture {
        let fixture = Fixture {
            root: tempfile::tempdir().unwrap(),
        };
        let root = fixture.root.path();
        std::fs::create_dir_all(fixture.home()).unwrap();
        std::fs::create_dir_all(fixture.state()).unwrap();
        git(
            root,
            &["init", "--quiet", "--bare", "-b", "main", "origin.git"],
        );
        git(root, &["init", "--quiet", "-b", "main", "seed"]);
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
        write(&fixture.seed().join(".tsk-ledger.toml"), "version = 1\n");
        write(&fixture.seed().join("index.md"), "# Ledger index\n");
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

    fn payload(&self, name: &str, content: &str) -> PathBuf {
        let path = self.root.path().join("payloads").join(name);
        write(&path, content);
        path
    }

    fn push_ledger(&self) {
        git(
            &self.seed(),
            &["push", "--quiet", "origin", &format!("HEAD:{}", LEDGER_REF)],
        );
    }

    fn seed_file(&self, file: &str, content: &str) {
        write(&self.seed().join(file), content);
        commit_all(&self.seed(), &format!("Seed {}", file));
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

    fn origin_subject(&self) -> String {
        git_line(&self.origin(), &["log", "-1", "--format=%s", LEDGER_REF])
    }

    fn tsk(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_tsk"))
            .args(args)
            .current_dir(self.work())
            .envs(git_env())
            .env("HOME", self.home())
            .env("XDG_STATE_HOME", self.state())
            .env("TSK_HOME", self.home().join(".tsk"))
            .env("GIT_CEILING_DIRECTORIES", self.root.path())
            .env_remove("CLAUDE_CODE_REMOTE_SESSION_ID")
            .output()
            .expect("failed to run tsk")
    }

    fn origin_ledger_commits(&self) -> u64 {
        git_line(&self.origin(), &["rev-list", "--count", LEDGER_REF])
            .parse()
            .unwrap()
    }

    fn append_batch(&self, args: &[&str], input: &str) -> Output {
        let mut child = Command::new(env!("CARGO_BIN_EXE_tsk"))
            .args(["events", "append-batch"])
            .args(args)
            .current_dir(self.work())
            .envs(git_env())
            .env("HOME", self.home())
            .env("XDG_STATE_HOME", self.state())
            .env("TSK_HOME", self.home().join(".tsk"))
            .env("GIT_CEILING_DIRECTORIES", self.root.path())
            .env_remove("CLAUDE_CODE_REMOTE_SESSION_ID")
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
        child.wait_with_output().expect("failed to run tsk")
    }

    fn append(&self, payload: &Path) -> Output {
        self.tsk(&[
            "events",
            "append",
            "github",
            "dependabot_alert",
            "created",
            "o/r",
            payload.to_str().unwrap(),
        ])
    }

    fn read_new(&self) -> serde_json::Value {
        let output = self.tsk(&["events", "read-new"]);
        assert_success(&output);
        json(&stdout(&output))
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

fn is_timestamp(text: &str) -> bool {
    text.len() == 20 && text.ends_with('Z') && &text[10..11] == "T"
}

const THREE_EVENTS: &str = "{\"n\":1}\n{\"n\":2}\n{\"n\":3}\n";

#[test]
fn append_queues_one_compact_envelope_and_pushes() {
    let fx = Fixture::new();
    let payload = fx.payload(
        "alert.json",
        "{\n  \"z\": 1,\n  \"a\": {\"b\": [1, 2]}\n}\n",
    );

    let output = fx.append(&payload);
    assert_success(&output);
    assert_eq!(stdout(&output), "queued\n");

    let queue = fx.origin_file(QUEUE).unwrap();
    assert_eq!(queue.matches('\n').count(), 1);
    let line = queue.trim_end();
    let envelope = json(line);
    let received_at = envelope["received_at"].as_str().unwrap();
    assert!(is_timestamp(received_at), "{}", received_at);
    assert_eq!(
        line,
        format!(
            "{{\"source\":\"github\",\"event_type\":\"dependabot_alert\",\"action\":\"created\",\"repo\":\"o/r\",\"received_at\":\"{}\",\"payload\":{{\"z\":1,\"a\":{{\"b\":[1,2]}}}}}}",
            received_at
        )
    );
    assert_eq!(
        fx.origin_subject(),
        "External event queue: github dependabot_alert (created) on o/r"
    );
}

#[test]
fn append_adds_to_the_end_of_an_existing_queue() {
    let fx = Fixture::new();
    fx.seed_file(QUEUE, THREE_EVENTS);
    let payload = fx.payload("alert.json", "{\"n\": 4}");

    assert_success(&fx.append(&payload));

    let queue = fx.origin_file(QUEUE).unwrap();
    assert!(queue.starts_with(THREE_EVENTS), "{}", queue);
    let lines: Vec<&str> = queue.lines().collect();
    assert_eq!(lines.len(), 4);
    assert_eq!(json(lines[3])["payload"], json("{\"n\":4}"));
}

#[test]
fn append_queues_null_for_an_empty_payload_file() {
    let fx = Fixture::new();
    let payload = fx.payload("empty.json", "");

    assert_success(&fx.append(&payload));

    let envelope = json(&fx.origin_file(QUEUE).unwrap());
    assert!(envelope["payload"].is_null());
}

#[test]
fn append_refuses_a_missing_payload_file_and_pushes_nothing() {
    let fx = Fixture::new();
    let before = fx.origin_ledger_sha();
    let missing = fx.root.path().join("payloads").join("absent.json");

    let output = fx.append(&missing);

    assert_exit(&output, 1);
    assert_eq!(stdout(&output), "");
    assert!(
        stderr(&output).contains(&format!("no such payload file: {}", missing.display())),
        "{}",
        stderr(&output)
    );
    assert_eq!(fx.origin_ledger_sha(), before);
}

#[test]
fn append_refuses_a_payload_that_is_not_json_and_pushes_nothing() {
    let fx = Fixture::new();
    let before = fx.origin_ledger_sha();
    let payload = fx.payload("bad.json", "{not json");

    let output = fx.append(&payload);

    assert_exit(&output, 1);
    assert!(
        stderr(&output).contains("is not JSON"),
        "{}",
        stderr(&output)
    );
    assert_eq!(fx.origin_ledger_sha(), before);
}

#[test]
fn read_new_on_an_empty_queue_prints_zero_counts() {
    let fx = Fixture::new();
    let output = fx.tsk(&["events", "read-new"]);
    assert_success(&output);
    assert_eq!(
        stdout(&output),
        "{\"new_count\":0,\"total_count\":0,\"events\":[]}\n"
    );
}

#[test]
fn read_new_without_a_watermark_prints_every_event() {
    let fx = Fixture::new();
    fx.seed_file(QUEUE, "{\"n\": 1, \"a\": 2}\n{\"n\":2}\n");

    let output = fx.tsk(&["events", "read-new"]);

    assert_success(&output);
    assert_eq!(
        stdout(&output),
        "{\"new_count\":2,\"total_count\":2,\"events\":[{\"n\":1,\"a\":2},{\"n\":2}]}\n"
    );
}

#[test]
fn read_new_with_a_watermark_prints_only_later_events() {
    let fx = Fixture::new();
    fx.seed_file(QUEUE, THREE_EVENTS);
    fx.seed_file(
        WATERMARK,
        "{\n  \"processed_through\": 2,\n  \"updated_at\": \"2026-09-20T09:24:54Z\"\n}\n",
    );

    let value = fx.read_new();

    assert_eq!(
        value,
        json("{\"new_count\":1,\"total_count\":3,\"events\":[{\"n\":3}]}")
    );
}

#[test]
fn read_new_with_the_watermark_at_the_end_prints_nothing_new() {
    let fx = Fixture::new();
    fx.seed_file(QUEUE, THREE_EVENTS);
    fx.seed_file(WATERMARK, "{\"processed_through\": 3}\n");

    let output = fx.tsk(&["events", "read-new"]);

    assert_success(&output);
    assert_eq!(
        stdout(&output),
        "{\"new_count\":0,\"total_count\":3,\"events\":[]}\n"
    );
}

#[test]
fn read_new_writes_nothing_and_repeats_its_answer() {
    let fx = Fixture::new();
    fx.seed_file(QUEUE, THREE_EVENTS);
    let before = fx.origin_ledger_sha();

    let first = fx.read_new();
    let second = fx.read_new();

    assert_eq!(first, second);
    assert_eq!(first["new_count"], 3);
    assert_eq!(fx.origin_ledger_sha(), before);
    assert_eq!(fx.origin_file(WATERMARK), None);
}

#[test]
fn read_new_sees_an_event_another_clone_appended() {
    let fx = Fixture::new();
    assert_eq!(fx.read_new()["total_count"], 0);

    fx.seed_file(QUEUE, "{\"n\":1}\n");

    assert_eq!(
        fx.read_new(),
        json("{\"new_count\":1,\"total_count\":1,\"events\":[{\"n\":1}]}")
    );
}

#[test]
fn advance_writes_the_watermark_and_pushes() {
    let fx = Fixture::new();
    fx.seed_file(QUEUE, THREE_EVENTS);

    let output = fx.tsk(&["events", "advance-watermark", "2"]);

    assert_success(&output);
    assert_eq!(stdout(&output), "watermark:2\n");
    let text = fx.origin_file(WATERMARK).unwrap();
    let value = json(&text);
    assert_eq!(value["processed_through"], 2);
    let updated_at = value["updated_at"].as_str().unwrap();
    assert!(is_timestamp(updated_at), "{}", updated_at);
    assert_eq!(
        text,
        format!(
            "{{\n  \"processed_through\": 2,\n  \"updated_at\": \"{}\"\n}}\n",
            updated_at
        )
    );
    assert_eq!(
        fx.origin_subject(),
        "External event queue: advance watermark 0 -> 2"
    );
    assert_eq!(
        fx.read_new(),
        json("{\"new_count\":1,\"total_count\":3,\"events\":[{\"n\":3}]}")
    );
}

#[test]
fn advance_to_the_current_count_is_a_no_op_and_pushes_nothing() {
    let fx = Fixture::new();
    fx.seed_file(QUEUE, THREE_EVENTS);
    assert_success(&fx.tsk(&["events", "advance-watermark", "3"]));
    let after_first = fx.origin_ledger_sha();

    let output = fx.tsk(&["events", "advance-watermark", "3"]);

    assert_success(&output);
    assert_eq!(stdout(&output), "");
    assert!(
        stderr(&output).contains("note: watermark already at 3; nothing to advance."),
        "{}",
        stderr(&output)
    );
    assert_eq!(fx.origin_ledger_sha(), after_first);
}

#[test]
fn advance_to_zero_without_a_watermark_is_a_no_op() {
    let fx = Fixture::new();
    let before = fx.origin_ledger_sha();

    let output = fx.tsk(&["events", "advance-watermark", "0"]);

    assert_success(&output);
    assert_eq!(stdout(&output), "");
    assert_eq!(fx.origin_ledger_sha(), before);
    assert_eq!(fx.origin_file(WATERMARK), None);
}

#[test]
fn advance_refuses_to_move_the_watermark_backwards() {
    let fx = Fixture::new();
    fx.seed_file(WATERMARK, "{\"processed_through\": 5}\n");
    let before = fx.origin_ledger_sha();

    let output = fx.tsk(&["events", "advance-watermark", "4"]);

    assert_exit(&output, 1);
    assert_eq!(stdout(&output), "");
    assert!(
        stderr(&output).contains("refusing to move the watermark backwards (5 -> 4)"),
        "{}",
        stderr(&output)
    );
    assert_eq!(fx.origin_ledger_sha(), before);
}

#[test]
fn advance_refuses_a_count_that_is_not_a_non_negative_integer() {
    let fx = Fixture::new();
    for bad in ["-1", "two", "1.5"] {
        let output = fx.tsk(&["events", "advance-watermark", bad]);
        assert_exit(&output, 1);
        assert!(
            stderr(&output).contains(&format!(
                "count must be a non-negative integer, got '{}'",
                bad
            )),
            "{}",
            stderr(&output)
        );
    }
}

#[test]
fn append_read_and_advance_run_as_one_cycle() {
    let fx = Fixture::new();
    let first = fx.payload("one.json", "{\"id\": 1}");
    let second = fx.payload("two.json", "{\"id\": 2}");
    assert_success(&fx.append(&first));
    assert_success(&fx.append(&second));

    let unread = fx.read_new();
    assert_eq!(unread["new_count"], 2);
    assert_eq!(unread["events"][0]["payload"]["id"], 1);
    assert_eq!(unread["events"][1]["payload"]["id"], 2);

    let total = unread["total_count"].to_string();
    assert_success(&fx.tsk(&["events", "advance-watermark", &total]));
    assert_eq!(
        fx.read_new(),
        json("{\"new_count\":0,\"total_count\":2,\"events\":[]}")
    );

    let third = fx.payload("three.json", "{\"id\": 3}");
    assert_success(&fx.append(&third));
    let unread = fx.read_new();
    assert_eq!(unread["new_count"], 1);
    assert_eq!(unread["total_count"], 3);
    assert_eq!(unread["events"][0]["payload"]["id"], 3);
}

const BATCH: &str = "{\"event_type\":\"dependabot_alert\",\"repo\":\"o/r\",\"payload\":{\"z\": 1, \"a\": [1, 2]}}\n\
{\"event_type\":\"code_scanning_alert\",\"repo\":\"o/r\",\"payload\":null}\n\
{\"source\":\"manual\",\"action\":\"fixed\",\"event_type\":\"secret_scanning_alert\",\"repo\":\"o/s\",\"payload\":[3]}\n";

#[test]
fn append_batch_queues_every_line_with_one_commit_and_one_push() {
    let fx = Fixture::new();
    fx.seed_file(QUEUE, THREE_EVENTS);
    let before = fx.origin_ledger_commits();

    let output = fx.append_batch(&["--source", "github", "--action", "polled"], BATCH);

    assert_success(&output);
    assert_eq!(stdout(&output), "queued:3\n");
    assert_eq!(fx.origin_ledger_commits(), before + 1);
    assert_eq!(
        fx.origin_subject(),
        "External event queue: append batch of 3 events"
    );

    let queue = fx.origin_file(QUEUE).unwrap();
    assert!(queue.starts_with(THREE_EVENTS), "{}", queue);
    let lines: Vec<&str> = queue.lines().skip(3).collect();
    assert_eq!(lines.len(), 3);
    let received_at = json(lines[0])["received_at"].as_str().unwrap().to_string();
    assert!(is_timestamp(&received_at), "{}", received_at);
    assert_eq!(
        lines,
        vec![
            format!("{{\"source\":\"github\",\"event_type\":\"dependabot_alert\",\"action\":\"polled\",\"repo\":\"o/r\",\"received_at\":\"{}\",\"payload\":{{\"z\":1,\"a\":[1,2]}}}}", received_at),
            format!("{{\"source\":\"github\",\"event_type\":\"code_scanning_alert\",\"action\":\"polled\",\"repo\":\"o/r\",\"received_at\":\"{}\",\"payload\":null}}", received_at),
            format!("{{\"source\":\"manual\",\"event_type\":\"secret_scanning_alert\",\"action\":\"fixed\",\"repo\":\"o/s\",\"received_at\":\"{}\",\"payload\":[3]}}", received_at),
        ]
    );
}

#[test]
fn append_batch_takes_source_and_action_from_each_line_without_flags() {
    let fx = Fixture::new();
    let input = "{\"source\":\"github\",\"action\":\"created\",\"event_type\":\"t\",\"repo\":\"o/r\",\"payload\":1}\n";

    let output = fx.append_batch(&[], input);

    assert_success(&output);
    assert_eq!(stdout(&output), "queued:1\n");
    let envelope = json(&fx.origin_file(QUEUE).unwrap());
    assert_eq!(envelope["source"], "github");
    assert_eq!(envelope["action"], "created");
    assert_eq!(
        fx.origin_subject(),
        "External event queue: append batch of 1 event"
    );
}

#[test]
fn append_batch_with_empty_stdin_queues_and_pushes_nothing() {
    let fx = Fixture::new();
    let before = fx.origin_ledger_sha();

    let output = fx.append_batch(&["--source", "github", "--action", "polled"], "");

    assert_success(&output);
    assert_eq!(stdout(&output), "queued:0\n");
    assert_eq!(fx.origin_ledger_sha(), before);
    assert_eq!(fx.origin_file(QUEUE), None);
}

#[test]
fn append_batch_rejects_the_whole_batch_for_one_bad_line() {
    let fx = Fixture::new();
    let before = fx.origin_ledger_sha();
    let input = format!("{}{}", BATCH, "{\"event_type\":\"t\",\"payload\":1}\n");

    let output = fx.append_batch(&["--source", "github", "--action", "polled"], &input);

    assert_exit(&output, 1);
    assert_eq!(stdout(&output), "");
    assert!(
        stderr(&output).contains("tsk events append-batch: stdin line 4: missing field repo"),
        "{}",
        stderr(&output)
    );
    assert_eq!(fx.origin_ledger_sha(), before);
    assert_eq!(fx.origin_file(QUEUE), None);
}

#[test]
fn append_batch_rejects_a_line_without_source_when_no_flag_is_given() {
    let fx = Fixture::new();
    let before = fx.origin_ledger_sha();

    let output = fx.append_batch(&["--action", "polled"], BATCH);

    assert_exit(&output, 1);
    assert!(
        stderr(&output)
            .contains("stdin line 1: missing field source, and no --source given for the batch"),
        "{}",
        stderr(&output)
    );
    assert_eq!(fx.origin_ledger_sha(), before);
}

#[test]
fn append_batch_events_are_read_back_by_read_new() {
    let fx = Fixture::new();
    assert_success(&fx.append_batch(&["--source", "github", "--action", "polled"], BATCH));

    let unread = fx.read_new();

    assert_eq!(unread["new_count"], 3);
    assert_eq!(unread["events"][2]["payload"], json("[3]"));
}
