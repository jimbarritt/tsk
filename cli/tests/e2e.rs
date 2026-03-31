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

// ---------------------------------------------------------------------------
// Tests: thread.update
// ---------------------------------------------------------------------------

#[test]
fn thread_update_changes_description() {
    let project = temp_project();
    let daemon = start_daemon(&project);
    let sock = tsk_core::socket_path(&project);

    tsk_core::send_request(&sock, "thread.create",
        serde_json::json!({"slug": "fix-login", "priority": "PRIO", "description": "Original"}),
    ).unwrap();

    let result = tsk_core::send_request(&sock, "thread.update",
        serde_json::json!({"id": "fix-login", "description": "Updated description"}),
    ).unwrap();

    assert_eq!(result["description"], "Updated description");
    assert_eq!(result["slug"], "fix-login", "slug unchanged");

    cleanup(&project, daemon);
}

#[test]
fn thread_update_changes_priority() {
    let project = temp_project();
    let daemon = start_daemon(&project);
    let sock = tsk_core::socket_path(&project);

    tsk_core::send_request(&sock, "thread.create",
        serde_json::json!({"slug": "fix-login", "priority": "PRIO", "description": "Fix it"}),
    ).unwrap();

    let result = tsk_core::send_request(&sock, "thread.update",
        serde_json::json!({"id": "fix-login", "priority": "BG"}),
    ).unwrap();

    assert_eq!(result["priority"], "BG");

    cleanup(&project, daemon);
}

#[test]
fn thread_update_changes_slug_and_renames_directory() {
    let project = temp_project();
    let daemon = start_daemon(&project);
    let sock = tsk_core::socket_path(&project);

    let created = tsk_core::send_request(&sock, "thread.create",
        serde_json::json!({"slug": "old-slug", "priority": "BG", "description": "Test"}),
    ).unwrap();
    let id = created["id"].as_u64().unwrap() as u32;

    let result = tsk_core::send_request(&sock, "thread.update",
        serde_json::json!({"id": "old-slug", "slug": "new-slug"}),
    ).unwrap();

    assert_eq!(result["slug"], "new-slug");
    assert!(!tsk_core::thread_dir(&project, id, "old-slug").exists(), "old dir should be gone");
    assert!(tsk_core::thread_dir(&project, id, "new-slug").exists(), "new dir should exist");

    cleanup(&project, daemon);
}

#[test]
fn thread_update_with_no_fields_is_a_noop() {
    let project = temp_project();
    let daemon = start_daemon(&project);
    let sock = tsk_core::socket_path(&project);

    tsk_core::send_request(&sock, "thread.create",
        serde_json::json!({"slug": "fix-login", "priority": "PRIO", "description": "Fix it"}),
    ).unwrap();

    let result = tsk_core::send_request(&sock, "thread.update",
        serde_json::json!({"id": "fix-login"}),
    ).unwrap();

    assert_eq!(result["slug"], "fix-login");
    assert_eq!(result["priority"], "PRIO");
    assert_eq!(result["description"], "Fix it");

    cleanup(&project, daemon);
}

#[test]
fn thread_update_errors_on_slug_collision() {
    let project = temp_project();
    let daemon = start_daemon(&project);
    let sock = tsk_core::socket_path(&project);

    tsk_core::send_request(&sock, "thread.create",
        serde_json::json!({"slug": "first", "priority": "PRIO", "description": "First"}),
    ).unwrap();
    tsk_core::send_request(&sock, "thread.create",
        serde_json::json!({"slug": "second", "priority": "BG", "description": "Second"}),
    ).unwrap();

    let result = tsk_core::send_request(&sock, "thread.update",
        serde_json::json!({"id": "first", "slug": "second"}),
    );
    assert!(result.is_err(), "should error on slug collision");

    cleanup(&project, daemon);
}

#[test]
fn thread_update_errors_on_unknown_id() {
    let project = temp_project();
    let daemon = start_daemon(&project);
    let sock = tsk_core::socket_path(&project);

    let result = tsk_core::send_request(&sock, "thread.update",
        serde_json::json!({"id": "nonexistent", "description": "nope"}),
    );
    assert!(result.is_err(), "should error for unknown thread");

    cleanup(&project, daemon);
}

#[test]
fn cli_thread_update_changes_description() {
    let project = temp_project();
    let daemon = start_daemon(&project);

    run_tsk(&project, &["thread", "create", "fix-login", "PRIO", "Original"]);

    let output = run_tsk(&project, &["thread", "update", "fix-login", "--description", "Updated"]);
    assert!(output.status.success());

    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(v["description"], "Updated");

    cleanup(&project, daemon);
}

#[test]
fn cli_thread_update_rejects_invalid_priority() {
    let project = temp_project();
    let daemon = start_daemon(&project);

    run_tsk(&project, &["thread", "create", "fix-login", "PRIO", "Fix it"]);

    let output = run_tsk(&project, &["thread", "update", "fix-login", "--priority", "INVALID"]);
    assert!(!output.status.success());

    cleanup(&project, daemon);
}

// ---------------------------------------------------------------------------
// Tests: thread.wait / thread.resume
// ---------------------------------------------------------------------------

#[test]
fn thread_wait_sets_state_to_waiting() {
    let project = temp_project();
    let daemon = start_daemon(&project);
    let sock = tsk_core::socket_path(&project);

    tsk_core::send_request(&sock, "thread.create",
        serde_json::json!({"slug": "fix-login", "priority": "PRIO", "description": "Fix it"}),
    ).unwrap();

    let result = tsk_core::send_request(&sock, "thread.wait",
        serde_json::json!({"id": "fix-login", "reason": "waiting for PR review"}),
    ).unwrap();

    assert_eq!(result["state"]["waiting"]["reason"], "waiting for PR review");

    cleanup(&project, daemon);
}

#[test]
fn thread_wait_without_reason_is_valid() {
    let project = temp_project();
    let daemon = start_daemon(&project);
    let sock = tsk_core::socket_path(&project);

    tsk_core::send_request(&sock, "thread.create",
        serde_json::json!({"slug": "fix-login", "priority": "PRIO", "description": "Fix it"}),
    ).unwrap();

    let result = tsk_core::send_request(&sock, "thread.wait",
        serde_json::json!({"id": "fix-login"}),
    ).unwrap();

    assert!(result["state"]["waiting"].is_object());

    cleanup(&project, daemon);
}

#[test]
fn thread_wait_errors_if_already_waiting() {
    let project = temp_project();
    let daemon = start_daemon(&project);
    let sock = tsk_core::socket_path(&project);

    tsk_core::send_request(&sock, "thread.create",
        serde_json::json!({"slug": "fix-login", "priority": "PRIO", "description": "Fix it"}),
    ).unwrap();
    tsk_core::send_request(&sock, "thread.wait",
        serde_json::json!({"id": "fix-login"}),
    ).unwrap();

    let result = tsk_core::send_request(&sock, "thread.wait",
        serde_json::json!({"id": "fix-login"}),
    );
    assert!(result.is_err(), "should error if already waiting");

    cleanup(&project, daemon);
}

#[test]
fn thread_resume_restores_previous_state() {
    let project = temp_project();
    let daemon = start_daemon(&project);
    let sock = tsk_core::socket_path(&project);

    tsk_core::send_request(&sock, "thread.create",
        serde_json::json!({"slug": "fix-login", "priority": "PRIO", "description": "Fix it"}),
    ).unwrap();
    // Activate it so previous_state will be Active
    tsk_core::send_request(&sock, "thread.switch_to",
        serde_json::json!({"id": "fix-login"}),
    ).unwrap();
    tsk_core::send_request(&sock, "thread.wait",
        serde_json::json!({"id": "fix-login"}),
    ).unwrap();

    let result = tsk_core::send_request(&sock, "thread.resume",
        serde_json::json!({"id": "fix-login", "note": "PR was approved"}),
    ).unwrap();

    assert_eq!(result["state"], "paused", "resume always returns to paused (active is a deliberate act via switch-to)");

    cleanup(&project, daemon);
}

#[test]
fn thread_resume_errors_if_not_waiting() {
    let project = temp_project();
    let daemon = start_daemon(&project);
    let sock = tsk_core::socket_path(&project);

    tsk_core::send_request(&sock, "thread.create",
        serde_json::json!({"slug": "fix-login", "priority": "PRIO", "description": "Fix it"}),
    ).unwrap();

    let result = tsk_core::send_request(&sock, "thread.resume",
        serde_json::json!({"id": "fix-login"}),
    );
    assert!(result.is_err(), "should error if not waiting");

    cleanup(&project, daemon);
}

#[test]
fn thread_wait_persists_across_daemon_restart() {
    let project = temp_project();

    {
        let daemon = start_daemon(&project);
        let sock = tsk_core::socket_path(&project);
        tsk_core::send_request(&sock, "thread.create",
            serde_json::json!({"slug": "fix-login", "priority": "PRIO", "description": "Fix it"}),
        ).unwrap();
        tsk_core::send_request(&sock, "thread.wait",
            serde_json::json!({"id": "fix-login", "reason": "waiting for review"}),
        ).unwrap();
        kill_daemon(&project, daemon);
    }

    let daemon = start_daemon(&project);
    let sock = tsk_core::socket_path(&project);
    let result = tsk_core::send_request(&sock, "thread.list", serde_json::json!({})).unwrap();
    let threads = result["threads"].as_array().unwrap();
    assert_eq!(threads[0]["state"]["waiting"]["reason"], "waiting for review");

    cleanup(&project, daemon);
}

#[test]
fn cli_thread_wait_sets_state_to_waiting() {
    let project = temp_project();
    let daemon = start_daemon(&project);

    run_tsk(&project, &["thread", "create", "fix-login", "PRIO", "Fix it"]);

    let output = run_tsk(&project, &["thread", "wait", "fix-login", "waiting for deploy"]);
    assert!(output.status.success());

    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(v["state"]["waiting"]["reason"], "waiting for deploy");

    cleanup(&project, daemon);
}

#[test]
fn cli_thread_resume_restores_state() {
    let project = temp_project();
    let daemon = start_daemon(&project);

    run_tsk(&project, &["thread", "create", "fix-login", "PRIO", "Fix it"]);
    run_tsk(&project, &["thread", "wait", "fix-login"]);

    let output = run_tsk(&project, &["thread", "resume", "fix-login", "unblocked"]);
    assert!(output.status.success());

    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(v["state"], "paused");

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

// ---------------------------------------------------------------------------
// Tests: task CRUD on active thread
// ---------------------------------------------------------------------------

#[test]
fn task_create_on_active_thread() {
    let project = temp_project();
    let daemon = start_daemon(&project);
    let sock = tsk_core::socket_path(&project);

    // Create and activate a thread
    tsk_core::send_request(&sock, "thread.create",
        serde_json::json!({"slug": "fix-login", "priority": "PRIO", "description": "Fix it"}),
    ).unwrap();
    tsk_core::send_request(&sock, "thread.switch_to",
        serde_json::json!({"id": "fix-login"}),
    ).unwrap();

    let result = tsk_core::send_request(&sock, "task.create",
        serde_json::json!({"description": "Write unit tests"}),
    ).unwrap();

    assert_eq!(result["description"], "Write unit tests");
    assert_eq!(result["state"], "not-started");
    assert_eq!(result["id"], "TSK-0001-0001");
    assert_eq!(result["seq"], 1);

    cleanup(&project, daemon);
}

#[test]
fn task_create_assigns_sequential_ids() {
    let project = temp_project();
    let daemon = start_daemon(&project);
    let sock = tsk_core::socket_path(&project);

    tsk_core::send_request(&sock, "thread.create",
        serde_json::json!({"slug": "my-thread", "priority": "PRIO", "description": "Thread"}),
    ).unwrap();
    tsk_core::send_request(&sock, "thread.switch_to",
        serde_json::json!({"id": "my-thread"}),
    ).unwrap();

    tsk_core::send_request(&sock, "task.create",
        serde_json::json!({"description": "First task"}),
    ).unwrap();
    let result = tsk_core::send_request(&sock, "task.create",
        serde_json::json!({"description": "Second task"}),
    ).unwrap();

    assert_eq!(result["id"], "TSK-0001-0002");
    assert_eq!(result["seq"], 2);

    cleanup(&project, daemon);
}

#[test]
fn task_create_with_due_by() {
    let project = temp_project();
    let daemon = start_daemon(&project);
    let sock = tsk_core::socket_path(&project);

    tsk_core::send_request(&sock, "thread.create",
        serde_json::json!({"slug": "fix-login", "priority": "PRIO", "description": "Fix it"}),
    ).unwrap();
    tsk_core::send_request(&sock, "thread.switch_to",
        serde_json::json!({"id": "fix-login"}),
    ).unwrap();

    let result = tsk_core::send_request(&sock, "task.create",
        serde_json::json!({"description": "Ship feature", "due_by": "2026-03-31"}),
    ).unwrap();

    assert_eq!(result["due_by"], "2026-03-31");

    cleanup(&project, daemon);
}

#[test]
fn task_list_returns_tasks_for_active_thread() {
    let project = temp_project();
    let daemon = start_daemon(&project);
    let sock = tsk_core::socket_path(&project);

    tsk_core::send_request(&sock, "thread.create",
        serde_json::json!({"slug": "fix-login", "priority": "PRIO", "description": "Fix it"}),
    ).unwrap();
    tsk_core::send_request(&sock, "thread.switch_to",
        serde_json::json!({"id": "fix-login"}),
    ).unwrap();

    tsk_core::send_request(&sock, "task.create",
        serde_json::json!({"description": "First"}),
    ).unwrap();
    tsk_core::send_request(&sock, "task.create",
        serde_json::json!({"description": "Second"}),
    ).unwrap();

    let result = tsk_core::send_request(&sock, "task.list", serde_json::json!({})).unwrap();
    let tasks = result["tasks"].as_array().unwrap();
    assert_eq!(tasks.len(), 2);

    let descs: Vec<&str> = tasks.iter().map(|t| t["description"].as_str().unwrap()).collect();
    assert!(descs.contains(&"First"));
    assert!(descs.contains(&"Second"));

    cleanup(&project, daemon);
}

#[test]
fn task_list_is_sorted_by_seq() {
    let project = temp_project();
    let daemon = start_daemon(&project);
    let sock = tsk_core::socket_path(&project);

    tsk_core::send_request(&sock, "thread.create",
        serde_json::json!({"slug": "work", "priority": "PRIO", "description": "Work"}),
    ).unwrap();
    tsk_core::send_request(&sock, "thread.switch_to",
        serde_json::json!({"id": "work"}),
    ).unwrap();

    tsk_core::send_request(&sock, "task.create", serde_json::json!({"description": "A"})).unwrap();
    let t2 = tsk_core::send_request(&sock, "task.create", serde_json::json!({"description": "B"})).unwrap();
    tsk_core::send_request(&sock, "task.create", serde_json::json!({"description": "C"})).unwrap();

    // Move task B to seq=1 (before A)
    let t2_id = t2["id"].as_str().unwrap();
    tsk_core::send_request(&sock, "task.update",
        serde_json::json!({"id": t2_id, "seq": 0}),
    ).unwrap();

    let result = tsk_core::send_request(&sock, "task.list", serde_json::json!({})).unwrap();
    let tasks = result["tasks"].as_array().unwrap();
    assert_eq!(tasks[0]["description"], "B", "B should come first after seq update");

    cleanup(&project, daemon);
}

// ---------------------------------------------------------------------------
// Tests: state transitions
// ---------------------------------------------------------------------------

#[test]
fn task_start_transitions_to_in_progress() {
    let project = temp_project();
    let daemon = start_daemon(&project);
    let sock = tsk_core::socket_path(&project);

    tsk_core::send_request(&sock, "thread.create",
        serde_json::json!({"slug": "fix-login", "priority": "PRIO", "description": "Fix it"}),
    ).unwrap();
    tsk_core::send_request(&sock, "thread.switch_to",
        serde_json::json!({"id": "fix-login"}),
    ).unwrap();
    let task = tsk_core::send_request(&sock, "task.create",
        serde_json::json!({"description": "Implement auth"}),
    ).unwrap();
    let task_id = task["id"].as_str().unwrap();

    let result = tsk_core::send_request(&sock, "task.start",
        serde_json::json!({"id": task_id}),
    ).unwrap();

    assert_eq!(result["state"], "in-progress");

    cleanup(&project, daemon);
}

#[test]
fn task_block_transitions_to_blocked() {
    let project = temp_project();
    let daemon = start_daemon(&project);
    let sock = tsk_core::socket_path(&project);

    tsk_core::send_request(&sock, "thread.create",
        serde_json::json!({"slug": "fix-login", "priority": "PRIO", "description": "Fix it"}),
    ).unwrap();
    tsk_core::send_request(&sock, "thread.switch_to",
        serde_json::json!({"id": "fix-login"}),
    ).unwrap();
    let task = tsk_core::send_request(&sock, "task.create",
        serde_json::json!({"description": "Implement auth"}),
    ).unwrap();
    let task_id = task["id"].as_str().unwrap();
    tsk_core::send_request(&sock, "task.start",
        serde_json::json!({"id": task_id}),
    ).unwrap();

    let result = tsk_core::send_request(&sock, "task.block",
        serde_json::json!({"id": task_id, "reason": "waiting for API spec"}),
    ).unwrap();

    assert_eq!(result["state"], "blocked");
    assert_eq!(result["blocked_reason"], "waiting for API spec");

    cleanup(&project, daemon);
}

#[test]
fn task_start_unblocks_a_blocked_task() {
    let project = temp_project();
    let daemon = start_daemon(&project);
    let sock = tsk_core::socket_path(&project);

    tsk_core::send_request(&sock, "thread.create",
        serde_json::json!({"slug": "fix-login", "priority": "PRIO", "description": "Fix it"}),
    ).unwrap();
    tsk_core::send_request(&sock, "thread.switch_to",
        serde_json::json!({"id": "fix-login"}),
    ).unwrap();
    let task = tsk_core::send_request(&sock, "task.create",
        serde_json::json!({"description": "Implement auth"}),
    ).unwrap();
    let task_id = task["id"].as_str().unwrap();
    tsk_core::send_request(&sock, "task.start", serde_json::json!({"id": task_id})).unwrap();
    tsk_core::send_request(&sock, "task.block",
        serde_json::json!({"id": task_id, "reason": "API not ready"}),
    ).unwrap();

    let result = tsk_core::send_request(&sock, "task.start",
        serde_json::json!({"id": task_id}),
    ).unwrap();

    assert_eq!(result["state"], "in-progress");
    assert!(result.get("blocked_reason").map_or(true, |v| v.is_null()),
        "blocked_reason should be cleared");

    cleanup(&project, daemon);
}

#[test]
fn task_complete_marks_as_done() {
    let project = temp_project();
    let daemon = start_daemon(&project);
    let sock = tsk_core::socket_path(&project);

    tsk_core::send_request(&sock, "thread.create",
        serde_json::json!({"slug": "fix-login", "priority": "PRIO", "description": "Fix it"}),
    ).unwrap();
    tsk_core::send_request(&sock, "thread.switch_to",
        serde_json::json!({"id": "fix-login"}),
    ).unwrap();
    let task = tsk_core::send_request(&sock, "task.create",
        serde_json::json!({"description": "Write tests"}),
    ).unwrap();
    let task_id = task["id"].as_str().unwrap();
    tsk_core::send_request(&sock, "task.start", serde_json::json!({"id": task_id})).unwrap();

    let result = tsk_core::send_request(&sock, "task.complete",
        serde_json::json!({"id": task_id}),
    ).unwrap();

    assert_eq!(result["state"], "done");

    cleanup(&project, daemon);
}

#[test]
fn task_cancel_from_any_state() {
    let project = temp_project();
    let daemon = start_daemon(&project);
    let sock = tsk_core::socket_path(&project);

    tsk_core::send_request(&sock, "thread.create",
        serde_json::json!({"slug": "fix-login", "priority": "PRIO", "description": "Fix it"}),
    ).unwrap();
    tsk_core::send_request(&sock, "thread.switch_to",
        serde_json::json!({"id": "fix-login"}),
    ).unwrap();

    // Cancel from not-started
    let task = tsk_core::send_request(&sock, "task.create",
        serde_json::json!({"description": "Not needed"}),
    ).unwrap();
    let result = tsk_core::send_request(&sock, "task.cancel",
        serde_json::json!({"id": task["id"].as_str().unwrap()}),
    ).unwrap();
    assert_eq!(result["state"], "cancelled");

    // Cancel from in-progress
    let task2 = tsk_core::send_request(&sock, "task.create",
        serde_json::json!({"description": "Also not needed"}),
    ).unwrap();
    let id2 = task2["id"].as_str().unwrap();
    tsk_core::send_request(&sock, "task.start", serde_json::json!({"id": id2})).unwrap();
    let result2 = tsk_core::send_request(&sock, "task.cancel",
        serde_json::json!({"id": id2}),
    ).unwrap();
    assert_eq!(result2["state"], "cancelled");

    cleanup(&project, daemon);
}

#[test]
fn task_block_errors_if_not_in_progress() {
    let project = temp_project();
    let daemon = start_daemon(&project);
    let sock = tsk_core::socket_path(&project);

    tsk_core::send_request(&sock, "thread.create",
        serde_json::json!({"slug": "fix-login", "priority": "PRIO", "description": "Fix it"}),
    ).unwrap();
    tsk_core::send_request(&sock, "thread.switch_to",
        serde_json::json!({"id": "fix-login"}),
    ).unwrap();
    let task = tsk_core::send_request(&sock, "task.create",
        serde_json::json!({"description": "Not started yet"}),
    ).unwrap();

    let result = tsk_core::send_request(&sock, "task.block",
        serde_json::json!({"id": task["id"].as_str().unwrap(), "reason": "nope"}),
    );
    assert!(result.is_err(), "should error when blocking a not-started task");

    cleanup(&project, daemon);
}

#[test]
fn task_complete_errors_if_cancelled() {
    let project = temp_project();
    let daemon = start_daemon(&project);
    let sock = tsk_core::socket_path(&project);

    tsk_core::send_request(&sock, "thread.create",
        serde_json::json!({"slug": "fix-login", "priority": "PRIO", "description": "Fix it"}),
    ).unwrap();
    tsk_core::send_request(&sock, "thread.switch_to",
        serde_json::json!({"id": "fix-login"}),
    ).unwrap();
    let task = tsk_core::send_request(&sock, "task.create",
        serde_json::json!({"description": "Cancelled"}),
    ).unwrap();
    let task_id = task["id"].as_str().unwrap();
    tsk_core::send_request(&sock, "task.cancel", serde_json::json!({"id": task_id})).unwrap();

    let result = tsk_core::send_request(&sock, "task.complete",
        serde_json::json!({"id": task_id}),
    );
    assert!(result.is_err(), "should error when completing a cancelled task");

    cleanup(&project, daemon);
}

// ---------------------------------------------------------------------------
// Tests: --thread flag (Diversion pattern)
// ---------------------------------------------------------------------------

#[test]
fn task_create_on_non_active_thread_via_thread_flag() {
    let project = temp_project();
    let daemon = start_daemon(&project);
    let sock = tsk_core::socket_path(&project);

    // Create two threads; activate thread-a
    tsk_core::send_request(&sock, "thread.create",
        serde_json::json!({"slug": "thread-a", "priority": "PRIO", "description": "Thread A"}),
    ).unwrap();
    tsk_core::send_request(&sock, "thread.create",
        serde_json::json!({"slug": "thread-b", "priority": "BG", "description": "Thread B"}),
    ).unwrap();
    tsk_core::send_request(&sock, "thread.switch_to",
        serde_json::json!({"id": "thread-a"}),
    ).unwrap();

    // Create a task on thread-b without switching to it
    let result = tsk_core::send_request(&sock, "task.create",
        serde_json::json!({"description": "Diversion task", "thread": "thread-b"}),
    ).unwrap();

    // Task id should reference thread-b's id (2)
    assert_eq!(result["id"], "TSK-0002-0001");

    // Active thread should still be thread-a
    let list = tsk_core::send_request(&sock, "thread.list", serde_json::json!({})).unwrap();
    let threads = list["threads"].as_array().unwrap();
    let active = threads.iter().find(|t| t["slug"] == "thread-a").unwrap();
    assert_eq!(active["state"], "active", "thread-a should still be active");

    cleanup(&project, daemon);
}

#[test]
fn task_list_on_non_active_thread_via_thread_flag() {
    let project = temp_project();
    let daemon = start_daemon(&project);
    let sock = tsk_core::socket_path(&project);

    tsk_core::send_request(&sock, "thread.create",
        serde_json::json!({"slug": "thread-a", "priority": "PRIO", "description": "Thread A"}),
    ).unwrap();
    tsk_core::send_request(&sock, "thread.create",
        serde_json::json!({"slug": "thread-b", "priority": "BG", "description": "Thread B"}),
    ).unwrap();
    tsk_core::send_request(&sock, "thread.switch_to",
        serde_json::json!({"id": "thread-a"}),
    ).unwrap();

    // Add a task to thread-b (diversion)
    tsk_core::send_request(&sock, "task.create",
        serde_json::json!({"description": "Task on B", "thread": "thread-b"}),
    ).unwrap();

    // list tasks on thread-b
    let result = tsk_core::send_request(&sock, "task.list",
        serde_json::json!({"thread": "thread-b"}),
    ).unwrap();
    let tasks = result["tasks"].as_array().unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0]["description"], "Task on B");

    // list tasks on active thread (thread-a) — should be empty
    let result_a = tsk_core::send_request(&sock, "task.list", serde_json::json!({})).unwrap();
    assert_eq!(result_a["tasks"].as_array().unwrap().len(), 0);

    cleanup(&project, daemon);
}

// ---------------------------------------------------------------------------
// Tests: task.update
// ---------------------------------------------------------------------------

#[test]
fn task_update_changes_description() {
    let project = temp_project();
    let daemon = start_daemon(&project);
    let sock = tsk_core::socket_path(&project);

    tsk_core::send_request(&sock, "thread.create",
        serde_json::json!({"slug": "fix-login", "priority": "PRIO", "description": "Fix it"}),
    ).unwrap();
    tsk_core::send_request(&sock, "thread.switch_to",
        serde_json::json!({"id": "fix-login"}),
    ).unwrap();
    let task = tsk_core::send_request(&sock, "task.create",
        serde_json::json!({"description": "Original description"}),
    ).unwrap();
    let task_id = task["id"].as_str().unwrap();

    let result = tsk_core::send_request(&sock, "task.update",
        serde_json::json!({"id": task_id, "description": "Updated description"}),
    ).unwrap();

    assert_eq!(result["description"], "Updated description");

    cleanup(&project, daemon);
}

#[test]
fn task_update_changes_due_by() {
    let project = temp_project();
    let daemon = start_daemon(&project);
    let sock = tsk_core::socket_path(&project);

    tsk_core::send_request(&sock, "thread.create",
        serde_json::json!({"slug": "fix-login", "priority": "PRIO", "description": "Fix it"}),
    ).unwrap();
    tsk_core::send_request(&sock, "thread.switch_to",
        serde_json::json!({"id": "fix-login"}),
    ).unwrap();
    let task = tsk_core::send_request(&sock, "task.create",
        serde_json::json!({"description": "Ship it"}),
    ).unwrap();
    let task_id = task["id"].as_str().unwrap();

    let result = tsk_core::send_request(&sock, "task.update",
        serde_json::json!({"id": task_id, "due_by": "2026-04-01"}),
    ).unwrap();

    assert_eq!(result["due_by"], "2026-04-01");

    cleanup(&project, daemon);
}

#[test]
fn task_update_errors_on_unknown_task_id() {
    let project = temp_project();
    let daemon = start_daemon(&project);
    let sock = tsk_core::socket_path(&project);

    tsk_core::send_request(&sock, "thread.create",
        serde_json::json!({"slug": "fix-login", "priority": "PRIO", "description": "Fix it"}),
    ).unwrap();
    tsk_core::send_request(&sock, "thread.switch_to",
        serde_json::json!({"id": "fix-login"}),
    ).unwrap();

    let result = tsk_core::send_request(&sock, "task.update",
        serde_json::json!({"id": "TSK-0001-9999", "description": "nope"}),
    );
    assert!(result.is_err(), "should error for unknown task id");

    cleanup(&project, daemon);
}

#[test]
fn tasks_persist_in_tasks_json_file() {
    let project = temp_project();
    let daemon = start_daemon(&project);
    let sock = tsk_core::socket_path(&project);

    let thread = tsk_core::send_request(&sock, "thread.create",
        serde_json::json!({"slug": "fix-login", "priority": "PRIO", "description": "Fix it"}),
    ).unwrap();
    let thread_id = thread["id"].as_u64().unwrap() as u32;
    tsk_core::send_request(&sock, "thread.switch_to",
        serde_json::json!({"id": "fix-login"}),
    ).unwrap();
    tsk_core::send_request(&sock, "task.create",
        serde_json::json!({"description": "Persisted task"}),
    ).unwrap();

    let tasks_path = tsk_core::tasks_path(&project, thread_id, "fix-login");
    assert!(tasks_path.exists(), "tasks.json should exist at {:?}", tasks_path);

    let content = std::fs::read_to_string(&tasks_path).unwrap();
    let tasks: Vec<serde_json::Value> = serde_json::from_str(&content).unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0]["description"], "Persisted task");

    cleanup(&project, daemon);
}

// ---------------------------------------------------------------------------
// Tests: CLI binary task commands
// ---------------------------------------------------------------------------

#[test]
fn cli_task_create_outputs_json() {
    let project = temp_project();
    let daemon = start_daemon(&project);

    run_tsk(&project, &["thread", "create", "fix-login", "PRIO", "Fix login"]);
    run_tsk(&project, &["thread", "switch-to", "fix-login"]);

    let output = run_tsk(&project, &["task", "create", "Write unit tests"]);
    assert!(output.status.success(), "tsk task create should exit 0");

    let v: serde_json::Value = serde_json::from_slice(&output.stdout).expect("should be valid JSON");
    assert_eq!(v["description"], "Write unit tests");
    assert_eq!(v["state"], "not-started");
    assert_eq!(v["id"], "TSK-0001-0001");

    cleanup(&project, daemon);
}

#[test]
fn cli_task_list_outputs_json() {
    let project = temp_project();
    let daemon = start_daemon(&project);

    run_tsk(&project, &["thread", "create", "fix-login", "PRIO", "Fix login"]);
    run_tsk(&project, &["thread", "switch-to", "fix-login"]);
    run_tsk(&project, &["task", "create", "First task"]);
    run_tsk(&project, &["task", "create", "Second task"]);

    let output = run_tsk(&project, &["task", "list"]);
    assert!(output.status.success());

    let v: serde_json::Value = serde_json::from_slice(&output.stdout).expect("should be valid JSON");
    assert_eq!(v["tasks"].as_array().unwrap().len(), 2);

    cleanup(&project, daemon);
}

#[test]
fn cli_task_start_transitions_state() {
    let project = temp_project();
    let daemon = start_daemon(&project);

    run_tsk(&project, &["thread", "create", "fix-login", "PRIO", "Fix login"]);
    run_tsk(&project, &["thread", "switch-to", "fix-login"]);
    run_tsk(&project, &["task", "create", "Implement feature"]);

    let output = run_tsk(&project, &["task", "start", "TSK-0001-0001"]);
    assert!(output.status.success());

    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(v["state"], "in-progress");

    cleanup(&project, daemon);
}

#[test]
fn cli_task_block_transitions_state() {
    let project = temp_project();
    let daemon = start_daemon(&project);

    run_tsk(&project, &["thread", "create", "fix-login", "PRIO", "Fix login"]);
    run_tsk(&project, &["thread", "switch-to", "fix-login"]);
    run_tsk(&project, &["task", "create", "Implement feature"]);
    run_tsk(&project, &["task", "start", "TSK-0001-0001"]);

    let output = run_tsk(&project, &["task", "block", "TSK-0001-0001", "Waiting for API"]);
    assert!(output.status.success());

    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(v["state"], "blocked");
    assert_eq!(v["blocked_reason"], "Waiting for API");

    cleanup(&project, daemon);
}

#[test]
fn cli_task_complete_transitions_state() {
    let project = temp_project();
    let daemon = start_daemon(&project);

    run_tsk(&project, &["thread", "create", "fix-login", "PRIO", "Fix login"]);
    run_tsk(&project, &["thread", "switch-to", "fix-login"]);
    run_tsk(&project, &["task", "create", "Implement feature"]);
    run_tsk(&project, &["task", "start", "TSK-0001-0001"]);

    let output = run_tsk(&project, &["task", "complete", "TSK-0001-0001"]);
    assert!(output.status.success());

    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(v["state"], "done");

    cleanup(&project, daemon);
}

#[test]
fn cli_task_cancel_transitions_state() {
    let project = temp_project();
    let daemon = start_daemon(&project);

    run_tsk(&project, &["thread", "create", "fix-login", "PRIO", "Fix login"]);
    run_tsk(&project, &["thread", "switch-to", "fix-login"]);
    run_tsk(&project, &["task", "create", "Not needed"]);

    let output = run_tsk(&project, &["task", "cancel", "TSK-0001-0001"]);
    assert!(output.status.success());

    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(v["state"], "cancelled");

    cleanup(&project, daemon);
}

#[test]
fn cli_task_update_changes_description() {
    let project = temp_project();
    let daemon = start_daemon(&project);

    run_tsk(&project, &["thread", "create", "fix-login", "PRIO", "Fix login"]);
    run_tsk(&project, &["thread", "switch-to", "fix-login"]);
    run_tsk(&project, &["task", "create", "Original"]);

    let output = run_tsk(&project, &[
        "task", "update", "TSK-0001-0001", "--description", "Updated",
    ]);
    assert!(output.status.success());

    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(v["description"], "Updated");

    cleanup(&project, daemon);
}

#[test]
fn cli_task_create_with_thread_flag_is_a_diversion() {
    let project = temp_project();
    let daemon = start_daemon(&project);

    run_tsk(&project, &["thread", "create", "thread-a", "PRIO", "Thread A"]);
    run_tsk(&project, &["thread", "create", "thread-b", "BG", "Thread B"]);
    run_tsk(&project, &["thread", "switch-to", "thread-a"]);

    // Add a task to thread-b while thread-a is active
    let output = run_tsk(&project, &[
        "task", "create", "Diversion task", "--thread", "thread-b",
    ]);
    assert!(output.status.success());

    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(v["id"], "TSK-0002-0001", "task should belong to thread-b (id=2)");

    // Verify active thread unchanged
    let list_out = run_tsk(&project, &["thread", "list"]);
    let list: serde_json::Value = serde_json::from_slice(&list_out.stdout).unwrap();
    let active = list["threads"].as_array().unwrap()
        .iter().find(|t| t["slug"] == "thread-a").unwrap();
    assert_eq!(active["state"], "active");

    cleanup(&project, daemon);
}
