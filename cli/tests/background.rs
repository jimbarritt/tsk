//! The background publish (ADR 0020): `tsk ledger push --background`, the failure warning,
//! the lock, and the Stop hook (`tsk thread guard`) that starts it.
//!
//! A fixture that started a background child waits for it, or kills it, when it is dropped,
//! so no process outlives a test. `TSK_DOLT_SERVER` is passed through from the
//! environment, so the suite runs with the server on and off.

use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

use tempfile::TempDir;

mod common;

const BRIEFING: &str = "missions/operational/M-TEST-01-example.md";

const SESSION_VAR: &str = "CLAUDE_CODE_REMOTE_SESSION_ID";

struct Fixture {
    root: TempDir,
    server: Option<&'static str>,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        // Wait for a child that still runs, kill it after 20 s, then stop the servers.
        for lock in self.locks() {
            let end = Instant::now() + Duration::from_secs(20);
            while lock_is_held(&lock) && Instant::now() < end {
                std::thread::sleep(Duration::from_millis(50));
            }
            if lock_is_held(&lock) {
                let pid = self.result().map_or(0, |r| r["pid"].as_i64().unwrap_or(0));
                if pid > 0 {
                    unsafe {
                        libc::kill(pid as i32, libc::SIGKILL);
                    }
                }
            }
        }
        common::stop_servers(self.root.path(), false);
    }
}

fn lock_is_held(lock: &Path) -> bool {
    use std::os::unix::io::AsRawFd;
    let Ok(file) = std::fs::OpenOptions::new().write(true).open(lock) else {
        return false;
    };
    unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) != 0 }
}

impl Fixture {
    fn new() -> Fixture {
        Fixture::build(None)
    }

    /// A fixture whose commands run with `TSK_DOLT_SERVER` set to `server`.
    fn with_server(server: &'static str) -> Fixture {
        Fixture::build(Some(server))
    }

    fn build(server: Option<&'static str>) -> Fixture {
        let fixture = Fixture {
            root: tempfile::tempdir().unwrap(),
            server,
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
        std::fs::write(fixture.seed().join("README.md"), "main\n").unwrap();
        git(&fixture.seed(), &["add", "-A"]);
        git(&fixture.seed(), &["commit", "--quiet", "-m", "Main commit"]);
        git(
            &fixture.seed(),
            &["push", "--quiet", "origin", "HEAD:refs/heads/main"],
        );
        git(
            root,
            &[
                "clone",
                "--quiet",
                fixture.origin().to_str().unwrap(),
                "work",
            ],
        );
        assert_success(&fixture.tsk(&["ledger", "fetch"]));
        assert_success(
            &fixture.tsk_stdin(&["ledger", "doc", "put", BRIEFING], "# Mission: example\n"),
        );
        assert_success(&fixture.tsk(&["ledger", "push", "Seed the ledger"]));
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

    fn bin(&self) -> PathBuf {
        self.root.path().join("bin")
    }

    /// The state directories of the clones: `<state>/tsk/repos/<clone-id>`.
    fn state_dirs(&self) -> Vec<PathBuf> {
        let repos = self.state().join("tsk").join("repos");
        std::fs::read_dir(repos)
            .map(|entries| entries.flatten().map(|e| e.path()).collect())
            .unwrap_or_default()
    }

    fn state_dir(&self) -> PathBuf {
        self.state_dirs().remove(0)
    }

    fn locks(&self) -> Vec<PathBuf> {
        self.state_dirs()
            .into_iter()
            .map(|d| d.join("push.lock"))
            .filter(|p| p.exists())
            .collect()
    }

    fn result_file(&self) -> PathBuf {
        self.state_dir().join("background-push.result")
    }

    fn result(&self) -> Option<serde_json::Value> {
        let text = std::fs::read_to_string(self.result_file()).ok()?;
        serde_json::from_str(&text).ok()
    }

    fn write_result(&self, value: serde_json::Value) {
        std::fs::write(self.result_file(), value.to_string()).unwrap();
    }

    /// Wait until no push holds the lock and the result is not `running`.
    fn wait_idle(&self) {
        let lock = self.state_dir().join("push.lock");
        let end = Instant::now() + Duration::from_secs(90);
        loop {
            let running = self
                .result()
                .is_some_and(|r| r["state"] == "running" && lock_is_held(&lock));
            if !running && !lock_is_held(&lock) {
                return;
            }
            assert!(Instant::now() < end, "the background publish did not end");
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// Processes of `background-push-run` for the message `message`.
    fn children(&self, message: &str) -> Vec<i32> {
        let mut pids = Vec::new();
        for entry in std::fs::read_dir("/proc").unwrap().flatten() {
            let Some(pid) = entry
                .file_name()
                .to_str()
                .and_then(|n| n.parse::<i32>().ok())
            else {
                continue;
            };
            if let Ok(bytes) = std::fs::read(entry.path().join("cmdline")) {
                let line = String::from_utf8_lossy(&bytes).replace('\0', " ");
                if line.contains("background-push-run") && line.contains(message) {
                    pids.push(pid);
                }
            }
        }
        pids
    }

    fn origin_sha(&self) -> String {
        git(&self.origin(), &["rev-parse", "refs/heads/tsk/docs"])
            .trim_end()
            .to_string()
    }

    fn unpushed(&self) -> bool {
        stdout(&self.tsk(&["ledger", "status", "--local"])).contains("not pushed")
    }

    fn put_note(&self, name: &str) {
        assert_success(&self.tsk_stdin(&["ledger", "doc", "put", name], "# note\n"));
    }

    /// A `git` that sleeps `seconds` before a push, first on PATH for `command`.
    fn slow_git(&self, command: &mut Command, seconds: u32) {
        std::fs::create_dir_all(self.bin()).unwrap();
        let real = String::from_utf8(
            Command::new("sh")
                .args(["-c", "command -v git"])
                .output()
                .unwrap()
                .stdout,
        )
        .unwrap();
        let script = self.bin().join("git");
        std::fs::write(
            &script,
            format!(
                "#!/bin/sh\nfor a in \"$@\"; do if [ \"$a\" = push ]; then sleep {}; fi; done\nexec {} \"$@\"\n",
                seconds,
                real.trim()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        command.env(
            "PATH",
            format!(
                "{}:{}",
                self.bin().display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        );
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_tsk"));
        command
            .args(args)
            .current_dir(self.work())
            .envs(git_env())
            .env("HOME", self.home())
            .env("XDG_STATE_HOME", self.state())
            .env("TSK_DOLT_SERVER_IDLE_SECS", "3")
            .env("XDG_CONFIG_HOME", self.home().join("config"))
            .env("TSK_HOME", self.home().join(".tsk"))
            .env("TSK_NO_UPDATE_CHECK", "1")
            .env("TSK_LEDGER_REFS", "branches")
            .env("GIT_CEILING_DIRECTORIES", self.root.path())
            .env_remove("TSK_BACKGROUND_PUBLISH")
            .env_remove(SESSION_VAR);
        if let Some(server) = self.server {
            command.env("TSK_DOLT_SERVER", server);
        }
        command
    }

    fn tsk(&self, args: &[&str]) -> Output {
        self.command(args).output().expect("failed to run tsk")
    }

    fn tsk_stdin(&self, args: &[&str], input: &str) -> Output {
        run_stdin(&mut self.command(args), input)
    }

    fn guard(&self, input: &str) -> Output {
        self.tsk_stdin(&["thread", "guard"], input)
    }

    fn start(&self) {
        assert_success(&self.tsk(&["thread", "start", "M-TEST-01"]));
    }
}

fn run_stdin(command: &mut Command, input: &str) -> Output {
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
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
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
        "tsk failed: {:?}\nstdout: {}\nstderr: {}",
        output.status,
        stdout(output),
        stderr(output)
    );
}

const WARNING: &str = "warning: the last background publish failed at ";

#[test]
fn a_background_push_publishes_and_returns_at_once() {
    let fx = Fixture::new();
    let before = fx.origin_sha();
    fx.put_note("notes/a.md");
    assert!(fx.unpushed());

    let started = Instant::now();
    let out = fx.tsk(&["ledger", "push", "--background", "Publish the note"]);
    let took = started.elapsed();
    assert_success(&out);
    assert_eq!(stdout(&out), "");
    assert_eq!(stderr(&out), "");
    assert!(took < Duration::from_secs(2), "took {:?}", took);

    fx.wait_idle();
    let result = fx.result().expect("a result file");
    assert_eq!(result["state"], "ok", "{}", result);
    assert_ne!(fx.origin_sha(), before);
    assert_eq!(result["commit"], fx.origin_sha().as_str());
    assert!(!fx.unpushed());
    let status = stdout(&fx.tsk(&["ledger", "status", "--local"]));
    assert!(status.contains("Background publish: ok at "), "{}", status);
}

#[test]
fn the_default_message_is_background_publish() {
    let fx = Fixture::new();
    fx.put_note("notes/a.md");
    assert_success(&fx.tsk(&["ledger", "push", "--background"]));
    fx.wait_idle();
    assert_eq!(fx.result().unwrap()["state"], "ok");
    let docs = git(
        &fx.origin(),
        &["log", "-1", "--format=%s", "refs/heads/tsk/docs"],
    );
    assert_eq!(docs.trim_end(), "Background publish");
}

#[test]
fn with_nothing_to_push_no_process_starts_and_nothing_is_written() {
    let fx = Fixture::new();
    let out = fx.tsk(&["ledger", "push", "--background", "bgmsg-nothing"]);
    assert_success(&out);
    assert_eq!(stdout(&out), "");
    assert_eq!(stderr(&out), "");
    assert!(fx.children("bgmsg-nothing").is_empty());
    assert!(fx.result().is_none());
    assert!(!fx.state_dir().join("background-push.log").exists());
}

#[test]
fn two_simultaneous_background_pushes_start_one_child() {
    let fx = Fixture::new();
    fx.put_note("notes/a.md");
    let mut first = fx.command(&["ledger", "push", "--background", "bgmsg-two"]);
    let mut second = fx.command(&["ledger", "push", "--background", "bgmsg-two"]);
    fx.slow_git(&mut first, 2);
    fx.slow_git(&mut second, 2);
    let a = std::thread::spawn(move || first.output().unwrap());
    let b = std::thread::spawn(move || second.output().unwrap());
    assert_success(&a.join().unwrap());
    assert_success(&b.join().unwrap());
    let running = fx.children("bgmsg-two");
    assert_eq!(running.len(), 1, "{:?}", running);
    fx.wait_idle();
    assert_eq!(fx.result().unwrap()["state"], "ok");
    assert!(!fx.unpushed());
}

#[test]
fn a_push_in_progress_makes_a_background_push_skip() {
    let fx = Fixture::new();
    fx.put_note("notes/a.md");
    let lock = fx.state_dir().join("push.lock");
    let file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(&lock)
        .unwrap();
    {
        use std::os::unix::io::AsRawFd;
        assert_eq!(unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) }, 0);
    }

    let out = fx.tsk(&["ledger", "push", "--background", "bgmsg-skip"]);
    assert_success(&out);
    assert_eq!(stderr(&out), "");
    assert!(fx.children("bgmsg-skip").is_empty());
    assert!(fx.result().is_none());
    assert!(fx.unpushed());

    // A foreground push waits for the lock, then gives up with a named error.
    let blocked = fx
        .command(&["ledger", "push", "Wait"])
        .env("TSK_PUSH_LOCK_SECS", "1")
        .output()
        .unwrap();
    assert!(!blocked.status.success());
    assert!(stderr(&blocked).contains("holds"), "{}", stderr(&blocked));

    drop(file);
    assert_success(&fx.tsk(&["ledger", "push", "Now"]));
    assert!(!fx.unpushed());
}

#[test]
fn a_failure_writes_the_result_and_warns_once_on_the_next_command() {
    let fx = Fixture::new();
    fx.put_note("notes/a.md");
    git(
        &fx.work(),
        &["remote", "set-url", "origin", "/nonexistent/origin.git"],
    );

    assert_success(&fx.tsk(&["ledger", "push", "--background", "bgmsg-fail"]));
    fx.wait_idle();
    let result = fx.result().unwrap();
    assert_eq!(result["state"], "failed", "{}", result);
    assert!(!result["error"].as_str().unwrap().is_empty());
    assert_eq!(result["shown"], false);

    let first = fx.tsk(&["ledger", "status", "--local"]);
    assert_success(&first);
    assert!(stderr(&first).starts_with(WARNING), "{}", stderr(&first));
    assert!(stderr(&first).contains("; run tsk ledger push"));
    assert_eq!(stderr(&first).lines().count(), 1);
    assert!(stdout(&first).contains("Background publish: failed at "));

    let second = fx.tsk(&["ledger", "status", "--local"]);
    assert!(!stderr(&second).contains(WARNING), "{}", stderr(&second));
    // The stdout of the two runs is the same.
    assert_eq!(stdout(&first), stdout(&second));

    // A later push that succeeds clears the failure.
    git(
        &fx.work(),
        &["remote", "set-url", "origin", fx.origin().to_str().unwrap()],
    );
    assert_success(&fx.tsk(&["ledger", "push", "Publish by hand"]));
    assert!(fx.result().is_none());
    let after = fx.tsk(&["ledger", "status", "--local"]);
    assert!(!stdout(&after).contains("Background publish"));
}

#[test]
fn a_later_background_success_replaces_the_failure() {
    let fx = Fixture::new();
    fx.put_note("notes/a.md");
    git(
        &fx.work(),
        &["remote", "set-url", "origin", "/nonexistent/origin.git"],
    );
    assert_success(&fx.tsk(&["ledger", "push", "--background", "bgmsg-f1"]));
    fx.wait_idle();
    assert_eq!(fx.result().unwrap()["state"], "failed");
    git(
        &fx.work(),
        &["remote", "set-url", "origin", fx.origin().to_str().unwrap()],
    );
    assert_success(&fx.tsk(&["ledger", "push", "--background", "bgmsg-f2"]));
    fx.wait_idle();
    assert_eq!(fx.result().unwrap()["state"], "ok");
    let out = fx.tsk(&["ledger", "status", "--local"]);
    assert!(!stderr(&out).contains(WARNING));
}

#[test]
fn killing_the_child_frees_the_lock_and_reads_as_interrupted() {
    let fx = Fixture::new();
    fx.put_note("notes/a.md");
    let mut command = fx.command(&["ledger", "push", "--background", "bgmsg-kill"]);
    fx.slow_git(&mut command, 20);
    assert_success(&command.output().unwrap());

    let lock = fx.state_dir().join("push.lock");
    let end = Instant::now() + Duration::from_secs(30);
    while fx.children("bgmsg-kill").is_empty() {
        assert!(Instant::now() < end, "the child did not start");
        std::thread::sleep(Duration::from_millis(20));
    }
    // Wait until the child is in its push, so the kill lands mid-run.
    while fx.result().is_none_or(|r| r["pid"] == 0) {
        assert!(Instant::now() < end, "the child did not record itself");
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(lock_is_held(&lock));
    for pid in fx.children("bgmsg-kill") {
        unsafe {
            libc::kill(pid, libc::SIGKILL);
        }
    }
    let end = Instant::now() + Duration::from_secs(10);
    while lock_is_held(&lock) {
        assert!(Instant::now() < end, "the lock was not released");
        std::thread::sleep(Duration::from_millis(20));
    }

    let out = fx.tsk(&["ledger", "status", "--local"]);
    assert_success(&out);
    assert!(stderr(&out).starts_with(WARNING), "{}", stderr(&out));
    assert!(stderr(&out).contains("interrupted"), "{}", stderr(&out));
    assert_eq!(fx.result().unwrap()["state"], "failed");
    // The work is still unpushed, and a new background push can start.
    assert!(fx.unpushed());
    assert_success(&fx.tsk(&["ledger", "push", "--background", "bgmsg-kill2"]));
    fx.wait_idle();
    assert!(!fx.unpushed());
}

#[test]
fn the_guard_prints_nothing_and_returns_fast_with_unpushed_work() {
    let fx = Fixture::new();
    fx.start();
    fx.put_note("notes/a.md");
    let before = fx.origin_sha();

    let started = Instant::now();
    let out = fx.guard("{\"session_id\":\"x\"}");
    let took = started.elapsed();
    assert_success(&out);
    assert_eq!(stdout(&out), "");
    assert_eq!(stderr(&out), "");
    assert!(took < Duration::from_secs(3), "took {:?}", took);

    fx.wait_idle();
    assert_eq!(fx.result().unwrap()["state"], "ok");
    assert_ne!(fx.origin_sha(), before);
    assert!(!fx.unpushed());
    let subject = git(
        &fx.origin(),
        &["log", "-1", "--format=%s", "refs/heads/tsk/docs"],
    );
    assert_eq!(subject.trim_end(), "Background publish");
}

#[test]
fn the_guard_with_a_clean_store_starts_nothing() {
    let fx = Fixture::new();
    fx.start();
    let out = fx.guard("{}");
    assert_success(&out);
    assert_eq!(stdout(&out), "");
    assert!(fx.result().is_none());
}

#[test]
fn the_guard_still_blocks_for_a_missing_binding_and_starts_no_publish() {
    let fx = Fixture::new();
    fx.put_note("notes/a.md");
    let out = fx.guard("{\"session_id\":\"x\"}");
    assert_success(&out);
    let decision: serde_json::Value = serde_json::from_str(stdout(&out).trim_end()).unwrap();
    assert_eq!(decision["decision"], "block");
    let reason = decision["reason"].as_str().unwrap();
    assert!(reason.starts_with("No thread binding was found"));
    assert!(!reason.contains("tsk ledger push"));
    assert!(fx.result().is_none());
    assert!(fx.unpushed());
}

#[test]
fn the_guard_never_blocks_for_unpushed_work_whatever_stop_hook_active_says() {
    let fx = Fixture::new();
    fx.start();
    for input in [
        "{\"stop_hook_active\":true}",
        "{\"stop_hook_active\":false}",
        "",
    ] {
        fx.put_note("notes/a.md");
        let out = fx.guard(input);
        assert_success(&out);
        assert_eq!(stdout(&out), "", "input {:?}", input);
        fx.wait_idle();
        assert_success(&fx.tsk(&["ledger", "push", "Settle"]));
    }
}

#[test]
fn the_guard_does_nothing_when_the_switch_is_off() {
    let fx = Fixture::new();
    fx.start();
    fx.put_note("notes/a.md");
    let out = run_stdin(
        fx.command(&["thread", "guard"])
            .env("TSK_BACKGROUND_PUBLISH", "off"),
        "{}",
    );
    assert_success(&out);
    assert_eq!(stdout(&out), "");
    assert!(fx.result().is_none());
    assert!(fx.unpushed());
}

#[test]
fn the_guard_and_the_session_start_print_the_warning_once() {
    let fx = Fixture::new();
    fx.start();
    let failed = serde_json::json!({
        "state": "failed", "pid": 1, "time": 1_700_000_000u64,
        "commit": "", "error": "error: origin said no\n       more detail", "shown": false
    });

    fx.write_result(failed.clone());
    let out = fx.guard("{}");
    assert_success(&out);
    assert_eq!(stdout(&out), "");
    assert_eq!(
        stderr(&out),
        "warning: the last background publish failed at 2023-11-14T22:13:20Z: origin said no; run tsk ledger push\n"
    );
    let again = fx.guard("{}");
    assert_eq!(stderr(&again), "");

    fx.write_result(failed);
    let start = fx.tsk_stdin(
        &["thread", "session-start"],
        "{\"session_id\":\"s-warn\",\"source\":\"startup\"}",
    );
    assert_success(&start);
    assert!(stderr(&start).contains(WARNING), "{}", stderr(&start));
    assert_eq!(stderr(&start).matches(WARNING).count(), 1);
    // Standard output holds the hook JSON and nothing of the warning.
    assert!(!stdout(&start).contains("warning:"));
}

#[test]
fn status_names_the_server_and_its_switch() {
    for (switch, expect) in [("off", "Server  not running (switch off)")] {
        let fx = Fixture::with_server(switch);
        for args in [
            &["ledger", "status"][..],
            &["ledger", "status", "--local"][..],
        ] {
            let out = fx.tsk(args);
            assert_success(&out);
            assert!(
                stdout(&out).lines().any(|l| l == expect),
                "{}\n{}",
                expect,
                stdout(&out)
            );
        }
        let porcelain = stdout(&fx.tsk(&["ledger", "status", "--porcelain"]));
        assert!(!porcelain.contains("Server"));
    }
}

#[test]
fn status_names_a_running_server_with_its_pid_and_idle_stop() {
    let fx = Fixture::with_server("on");
    let out = fx
        .command(&["ledger", "status", "--local"])
        .output()
        .unwrap();
    assert_success(&out);
    let line = stdout(&out)
        .lines()
        .find(|l| l.starts_with("Server"))
        .map(str::to_string)
        .expect("a Server line");
    assert!(
        line.starts_with("Server  running (pid ") && line.ends_with(", idle stop 3 s)"),
        "{}",
        line
    );
}
