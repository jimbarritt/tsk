/// End-to-end tests for tsk.
///
/// These tests start tskd as a subprocess, exercise the full stack via the
/// client helper and the tsk binary, verify both JSON responses and
/// filesystem side-effects.
///
/// Prerequisites: run `cargo build --workspace` before `cargo test --workspace`
/// so that the tskd and tsk binaries exist in target/debug/.
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn workspace_root() -> PathBuf {
    // CARGO_MANIFEST_DIR is the cli/ directory; workspace root is its parent
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest.parent().unwrap().to_path_buf()
}

fn tskd_binary() -> PathBuf {
    workspace_root().join("target").join("debug").join("tskd")
}

fn tsk_binary() -> PathBuf {
    workspace_root().join("target").join("debug").join("tsk")
}

/// Create a unique temporary project directory for a test.
fn temp_project() -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();
    let pid = std::process::id();
    let path = PathBuf::from(format!("/tmp/tsk-test-{}-{}", pid, nanos));
    std::fs::create_dir_all(&path).unwrap();
    path
}

fn kill_daemon(project_root: &Path, mut daemon: Child) {
    let _ = daemon.kill();
    let _ = daemon.wait();
    // Wait for socket to disappear before returning so the caller can safely rebind
    let sock = tsk_core::socket_path(project_root);
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline && sock.exists() {
        std::thread::sleep(Duration::from_millis(50));
    }
    // Remove it unconditionally in case it persisted
    let _ = std::fs::remove_file(&sock);
}

fn cleanup(project_root: &Path, daemon: Child) {
    kill_daemon(project_root, daemon);
    let _ = std::fs::remove_dir_all(project_root);
}

/// Start tskd pointing at the given project root and wait for its socket to appear.
fn start_daemon(project_root: &Path) -> Child {
    let binary = tskd_binary();
    assert!(
        binary.exists(),
        "tskd binary not found at {:?}. Run `cargo build --workspace` first.",
        binary
    );

    let child = Command::new(&binary)
        .arg(project_root)
        .spawn()
        .expect("Failed to spawn tskd");

    // Wait up to 5 seconds for the socket to appear
    let sock = tsk_core::socket_path(project_root);
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if sock.exists() {
            return child;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("tskd socket never appeared at {:?}", sock);
}

// ---------------------------------------------------------------------------
// Tests: daemon via client helper (send_request)
// ---------------------------------------------------------------------------

#[test]
fn daemon_creates_tsk_directories_on_startup() {
    let project = temp_project();
    let daemon = start_daemon(&project);

    assert!(
        project.join("tsk").join("event-log").exists(),
        "tsk/event-log should exist"
    );
    assert!(
        project.join("tsk").join("threads").exists(),
        "tsk/threads should exist"
    );

    cleanup(&project, daemon);
}

#[test]
fn thread_start_returns_thread_with_correct_slug_and_short_hash() {
    let project = temp_project();
    let daemon = start_daemon(&project);
    let sock = tsk_core::socket_path(&project);

    let result = tsk_core::send_request(
        &sock,
        "thread.start",
        serde_json::json!({
            "slug": "fix-login",
            "priority": "PRIO",
            "description": "Fix the login bug"
        }),
    )
    .expect("thread.start should succeed");

    assert_eq!(result["slug"], "fix-login");
    assert_eq!(result["priority"], "PRIO");

    let short_hash = result["short_hash"].as_str().unwrap();
    assert_eq!(short_hash.len(), 7, "short_hash should be 7 chars");

    let full_hash = result["hash"].as_str().unwrap();
    assert_eq!(full_hash.len(), 64, "hash should be 64 chars");
    assert!(full_hash.starts_with(short_hash));

    cleanup(&project, daemon);
}

#[test]
fn thread_start_creates_index_json_with_thread() {
    let project = temp_project();
    let daemon = start_daemon(&project);
    let sock = tsk_core::socket_path(&project);

    tsk_core::send_request(
        &sock,
        "thread.start",
        serde_json::json!({
            "slug": "fix-login",
            "priority": "PRIO",
            "description": "Fix the login bug"
        }),
    )
    .unwrap();

    let index_path = tsk_core::index_path(&project);
    assert!(index_path.exists(), "index.json should exist");

    let content = std::fs::read_to_string(&index_path).unwrap();
    let threads: Vec<serde_json::Value> = serde_json::from_str(&content).unwrap();
    assert_eq!(threads.len(), 1);
    assert_eq!(threads[0]["slug"], "fix-login");

    cleanup(&project, daemon);
}

#[test]
fn thread_start_creates_thread_directory() {
    let project = temp_project();
    let daemon = start_daemon(&project);
    let sock = tsk_core::socket_path(&project);

    let result = tsk_core::send_request(
        &sock,
        "thread.start",
        serde_json::json!({
            "slug": "fix-login",
            "priority": "PRIO",
            "description": "Fix the login bug"
        }),
    )
    .unwrap();

    let short_hash = result["short_hash"].as_str().unwrap();
    let thread_dir = tsk_core::thread_dir(&project, short_hash, "fix-login");
    assert!(
        thread_dir.exists(),
        "thread directory {:?} should exist",
        thread_dir
    );

    cleanup(&project, daemon);
}

#[test]
fn thread_start_appends_event_to_ndjson_log() {
    let project = temp_project();
    let daemon = start_daemon(&project);
    let sock = tsk_core::socket_path(&project);

    tsk_core::send_request(
        &sock,
        "thread.start",
        serde_json::json!({
            "slug": "fix-login",
            "priority": "PRIO",
            "description": "Fix the login bug"
        }),
    )
    .unwrap();

    let log_path = tsk_core::event_log_path(&project);
    assert!(log_path.exists(), "events.ndjson should exist");

    let content = std::fs::read_to_string(&log_path).unwrap();
    assert!(!content.is_empty(), "event log should not be empty");

    let event: serde_json::Value = serde_json::from_str(content.lines().next().unwrap()).unwrap();
    assert_eq!(event["event"], "ThreadStarted");
    assert_eq!(event["slug"], "fix-login");

    cleanup(&project, daemon);
}

#[test]
fn thread_list_returns_all_threads() {
    let project = temp_project();
    let daemon = start_daemon(&project);
    let sock = tsk_core::socket_path(&project);

    tsk_core::send_request(
        &sock,
        "thread.start",
        serde_json::json!({"slug": "fix-login", "priority": "PRIO", "description": "Fix login"}),
    )
    .unwrap();

    tsk_core::send_request(
        &sock,
        "thread.start",
        serde_json::json!({"slug": "update-deps", "priority": "BG", "description": "Update deps"}),
    )
    .unwrap();

    let result = tsk_core::send_request(&sock, "thread.list", serde_json::json!({})).unwrap();
    let threads = result["threads"].as_array().unwrap();
    assert_eq!(threads.len(), 2);

    let slugs: Vec<&str> = threads
        .iter()
        .map(|t| t["slug"].as_str().unwrap())
        .collect();
    assert!(slugs.contains(&"fix-login"));
    assert!(slugs.contains(&"update-deps"));

    cleanup(&project, daemon);
}

#[test]
fn thread_start_rejects_duplicate_slug() {
    let project = temp_project();
    let daemon = start_daemon(&project);
    let sock = tsk_core::socket_path(&project);

    tsk_core::send_request(
        &sock,
        "thread.start",
        serde_json::json!({"slug": "fix-login", "priority": "PRIO", "description": "First"}),
    )
    .unwrap();

    let result = tsk_core::send_request(
        &sock,
        "thread.start",
        serde_json::json!({"slug": "fix-login", "priority": "PRIO", "description": "Duplicate"}),
    );
    assert!(result.is_err(), "duplicate slug should be an error");

    cleanup(&project, daemon);
}

#[test]
fn daemon_loads_state_from_index_json_on_restart() {
    let project = temp_project();

    // Start daemon, create a thread, stop daemon (keep project dir)
    {
        let daemon = start_daemon(&project);
        let sock = tsk_core::socket_path(&project);
        tsk_core::send_request(
            &sock,
            "thread.start",
            serde_json::json!({"slug": "fix-login", "priority": "PRIO", "description": "Fix it"}),
        )
        .unwrap();
        kill_daemon(&project, daemon);
    }

    // Restart daemon, list threads — should still have the thread
    let daemon = start_daemon(&project);
    let sock = tsk_core::socket_path(&project);

    let result = tsk_core::send_request(&sock, "thread.list", serde_json::json!({})).unwrap();
    let threads = result["threads"].as_array().unwrap();
    assert_eq!(threads.len(), 1, "thread should persist across restarts");
    assert_eq!(threads[0]["slug"], "fix-login");

    cleanup(&project, daemon);
}

// ---------------------------------------------------------------------------
// Tests: CLI binary (tsk thread start / tsk thread list)
// ---------------------------------------------------------------------------

fn run_tsk(project_root: &Path, args: &[&str]) -> std::process::Output {
    let binary = tsk_binary();
    assert!(
        binary.exists(),
        "tsk binary not found at {:?}. Run `cargo build --workspace` first.",
        binary
    );
    Command::new(&binary)
        .args(args)
        .env("TSK_PROJECT_ROOT", project_root)
        .output()
        .expect("Failed to run tsk")
}

#[test]
fn cli_thread_start_outputs_json_with_thread() {
    let project = temp_project();
    let daemon = start_daemon(&project);

    let output = run_tsk(&project, &["thread", "start", "fix-login", "PRIO", "Fix the login bug"]);
    assert!(output.status.success(), "tsk should exit 0");

    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout should be valid JSON");
    assert_eq!(v["slug"], "fix-login");
    assert_eq!(v["priority"], "PRIO");

    cleanup(&project, daemon);
}

#[test]
fn cli_thread_list_outputs_json_with_threads() {
    let project = temp_project();
    let daemon = start_daemon(&project);

    run_tsk(&project, &["thread", "start", "fix-login", "PRIO", "Fix login"]);
    run_tsk(&project, &["thread", "start", "update-deps", "BG", "Update deps"]);

    let output = run_tsk(&project, &["thread", "list"]);
    assert!(output.status.success(), "tsk thread list should exit 0");

    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout should be valid JSON");
    let threads = v["threads"].as_array().unwrap();
    assert_eq!(threads.len(), 2);

    let slugs: Vec<&str> = threads
        .iter()
        .map(|t| t["slug"].as_str().unwrap())
        .collect();
    assert!(slugs.contains(&"fix-login"));
    assert!(slugs.contains(&"update-deps"));

    cleanup(&project, daemon);
}

#[test]
fn cli_errors_when_daemon_not_running() {
    let project = temp_project();
    // Don't start the daemon

    let output = run_tsk(&project, &["thread", "list"]);
    assert!(!output.status.success(), "should fail when daemon not running");

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("tskd is not running"),
        "should print helpful error, got: {}",
        stderr
    );

    let _ = std::fs::remove_dir_all(&project);
}
