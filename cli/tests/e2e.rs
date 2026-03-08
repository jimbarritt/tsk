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
        .env("TSK_PROJECT_ROOT", project_root)
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
fn thread_create_returns_thread_with_correct_slug_and_short_hash() {
    let project = temp_project();
    let daemon = start_daemon(&project);
    let sock = tsk_core::socket_path(&project);

    let result = tsk_core::send_request(
        &sock,
        "thread.create",
        serde_json::json!({
            "slug": "fix-login",
            "priority": "PRIO",
            "description": "Fix the login bug"
        }),
    )
    .expect("thread.create should succeed");

    assert_eq!(result["slug"], "fix-login");
    assert_eq!(result["priority"], "PRIO");
    assert_eq!(result["state"], "paused", "newly created thread should be paused");
    assert_eq!(result["id"], 1, "first thread should have id 1");

    cleanup(&project, daemon);
}

#[test]
fn thread_create_creates_index_json_with_thread() {
    let project = temp_project();
    let daemon = start_daemon(&project);
    let sock = tsk_core::socket_path(&project);

    tsk_core::send_request(
        &sock,
        "thread.create",
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
fn thread_create_creates_thread_directory() {
    let project = temp_project();
    let daemon = start_daemon(&project);
    let sock = tsk_core::socket_path(&project);

    let result = tsk_core::send_request(
        &sock,
        "thread.create",
        serde_json::json!({
            "slug": "fix-login",
            "priority": "PRIO",
            "description": "Fix the login bug"
        }),
    )
    .unwrap();

    let id = result["id"].as_u64().unwrap() as u32;
    let thread_dir = tsk_core::thread_dir(&project, id, "fix-login");
    assert!(
        thread_dir.exists(),
        "thread directory {:?} should exist",
        thread_dir
    );

    cleanup(&project, daemon);
}

#[test]
fn thread_create_appends_event_to_ndjson_log() {
    let project = temp_project();
    let daemon = start_daemon(&project);
    let sock = tsk_core::socket_path(&project);

    tsk_core::send_request(
        &sock,
        "thread.create",
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
    assert_eq!(event["event"], "ThreadCreated");
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
        "thread.create",
        serde_json::json!({"slug": "fix-login", "priority": "PRIO", "description": "Fix login"}),
    )
    .unwrap();

    tsk_core::send_request(
        &sock,
        "thread.create",
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
fn thread_create_rejects_duplicate_slug() {
    let project = temp_project();
    let daemon = start_daemon(&project);
    let sock = tsk_core::socket_path(&project);

    tsk_core::send_request(
        &sock,
        "thread.create",
        serde_json::json!({"slug": "fix-login", "priority": "PRIO", "description": "First"}),
    )
    .unwrap();

    let result = tsk_core::send_request(
        &sock,
        "thread.create",
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
            "thread.create",
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
// Tests: CLI binary (tsk thread create / tsk thread list)
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
fn cli_thread_create_outputs_json_with_thread() {
    let project = temp_project();
    let daemon = start_daemon(&project);

    let output = run_tsk(&project, &["thread", "create", "fix-login", "PRIO", "Fix the login bug"]);
    assert!(output.status.success(), "tsk should exit 0");

    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout should be valid JSON");
    assert_eq!(v["slug"], "fix-login");
    assert_eq!(v["priority"], "PRIO");
    assert_eq!(v["state"], "paused");

    cleanup(&project, daemon);
}

#[test]
fn cli_thread_list_outputs_json_with_threads() {
    let project = temp_project();
    let daemon = start_daemon(&project);

    run_tsk(&project, &["thread", "create", "fix-login", "PRIO", "Fix login"]);
    run_tsk(&project, &["thread", "create", "update-deps", "BG", "Update deps"]);

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

// ---------------------------------------------------------------------------
// Tests: thread create, switch-to, dir in response
// ---------------------------------------------------------------------------

#[test]
fn thread_create_includes_dir_in_response() {
    let project = temp_project();
    let daemon = start_daemon(&project);
    let sock = tsk_core::socket_path(&project);

    let result = tsk_core::send_request(
        &sock,
        "thread.create",
        serde_json::json!({"slug": "fix-login", "priority": "PRIO", "description": "Fix it"}),
    )
    .unwrap();

    let dir = result["dir"].as_str().expect("response should include dir");
    assert!(
        std::path::Path::new(dir).is_absolute(),
        "dir should be an absolute path, got: {}",
        dir
    );
    assert!(dir.contains("fix-login"), "dir should contain the slug");
    assert!(
        std::path::Path::new(dir).exists(),
        "dir should actually exist on disk"
    );

    cleanup(&project, daemon);
}

#[test]
fn thread_create_does_not_change_active_thread() {
    let project = temp_project();
    let daemon = start_daemon(&project);
    let sock = tsk_core::socket_path(&project);

    // Create first thread and activate it
    tsk_core::send_request(
        &sock,
        "thread.create",
        serde_json::json!({"slug": "first", "priority": "PRIO", "description": "First"}),
    )
    .unwrap();
    tsk_core::send_request(
        &sock,
        "thread.switch_to",
        serde_json::json!({"id": "first"}),
    )
    .unwrap();

    // Create second thread — first should remain active
    tsk_core::send_request(
        &sock,
        "thread.create",
        serde_json::json!({"slug": "second", "priority": "BG", "description": "Second"}),
    )
    .unwrap();

    let result = tsk_core::send_request(&sock, "thread.list", serde_json::json!({})).unwrap();
    let threads = result["threads"].as_array().unwrap();

    let first = threads.iter().find(|t| t["slug"] == "first").unwrap();
    let second = threads.iter().find(|t| t["slug"] == "second").unwrap();

    assert_eq!(first["state"], "active", "first thread should remain active");
    assert_eq!(second["state"], "paused", "newly created thread should be paused");

    cleanup(&project, daemon);
}

#[test]
fn thread_switch_to_activates_target_and_pauses_others() {
    let project = temp_project();
    let daemon = start_daemon(&project);
    let sock = tsk_core::socket_path(&project);

    tsk_core::send_request(
        &sock,
        "thread.create",
        serde_json::json!({"slug": "first", "priority": "PRIO", "description": "First"}),
    )
    .unwrap();
    tsk_core::send_request(
        &sock,
        "thread.create",
        serde_json::json!({"slug": "second", "priority": "BG", "description": "Second"}),
    )
    .unwrap();

    // Switch to first
    let result = tsk_core::send_request(
        &sock,
        "thread.switch_to",
        serde_json::json!({"id": "first"}),
    )
    .unwrap();

    assert_eq!(result["slug"], "first");
    assert_eq!(result["state"], "active");

    // Verify full list state
    let list = tsk_core::send_request(&sock, "thread.list", serde_json::json!({})).unwrap();
    let threads = list["threads"].as_array().unwrap();
    let first = threads.iter().find(|t| t["slug"] == "first").unwrap();
    let second = threads.iter().find(|t| t["slug"] == "second").unwrap();

    assert_eq!(first["state"], "active");
    assert_eq!(second["state"], "paused");

    cleanup(&project, daemon);
}

#[test]
fn thread_switch_to_returns_dir() {
    let project = temp_project();
    let daemon = start_daemon(&project);
    let sock = tsk_core::socket_path(&project);

    tsk_core::send_request(
        &sock,
        "thread.create",
        serde_json::json!({"slug": "first", "priority": "PRIO", "description": "First"}),
    )
    .unwrap();
    tsk_core::send_request(
        &sock,
        "thread.create",
        serde_json::json!({"slug": "second", "priority": "BG", "description": "Second"}),
    )
    .unwrap();

    let result =
        tsk_core::send_request(&sock, "thread.switch_to", serde_json::json!({"id": "first"}))
            .unwrap();

    let dir = result["dir"].as_str().expect("switch_to should return dir");
    assert!(std::path::Path::new(dir).is_absolute());
    assert!(dir.contains("first"));

    cleanup(&project, daemon);
}

#[test]
fn thread_switch_to_errors_for_unknown_id() {
    let project = temp_project();
    let daemon = start_daemon(&project);
    let sock = tsk_core::socket_path(&project);

    let result = tsk_core::send_request(
        &sock,
        "thread.switch_to",
        serde_json::json!({"id": "nonexistent"}),
    );
    assert!(result.is_err(), "should error for unknown thread id");

    cleanup(&project, daemon);
}

#[test]
fn cli_switch_to_outputs_json() {
    let project = temp_project();
    let daemon = start_daemon(&project);

    run_tsk(&project, &["thread", "create", "first", "PRIO", "First"]);
    run_tsk(&project, &["thread", "create", "second", "BG", "Second"]);

    let output = run_tsk(&project, &["thread", "switch-to", "first"]);
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("should be valid JSON");
    assert_eq!(v["slug"], "first");
    assert_eq!(v["state"], "active");
    assert!(v["dir"].as_str().is_some(), "should include dir");

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
