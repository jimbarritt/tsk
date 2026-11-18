use std::path::Path;
use std::process::Command;

const JANE: &str = "Jane Doe <1+jane@users.noreply.github.com>";

fn git(dir: &Path, args: &[&str]) {
    let status = crate::common::cmd("git")
        .current_dir(dir)
        .args(args)
        .status()
        .unwrap();
    assert!(status.success());
}

fn commit_msg(dir: &Path, home: &Path, message: &str, extra: &[(&str, &str)]) -> String {
    let file = dir.join("MSG");
    std::fs::write(&file, message).unwrap();
    let mut command: Command = crate::common::cmd(env!("CARGO_BIN_EXE_tsk"));
    command
        .current_dir(dir)
        .args(["actor", "commit-msg"])
        .arg(&file)
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", home.join("config"))
        .env("XDG_STATE_HOME", home.join("state"))
        .env("TSK_HOME", home.join(".tsk"))
        .env("TSK_AUTO_UPDATE", "never")
        .env("CLAUDE_CODE_REMOTE_SESSION_ID", "s1")
        .env("TSK_SESSION", "urn:tsk:session:claude-cli:s1")
        .env_remove("TSK_ATTRIBUTION_CO_AUTHOR")
        .env_remove("TSK_ATTRIBUTION_IDENTITY");
    for (key, value) in extra {
        command.env(key, value);
    }
    assert!(command.status().unwrap().success());
    std::fs::read_to_string(&file).unwrap()
}

fn repo() -> Option<(tempfile::TempDir, tempfile::TempDir)> {
    if crate::common::cmd("git").arg("--version").output().is_err() {
        eprintln!("note: git is missing; skipping");
        return None;
    }
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "-q"]);
    Some((dir, tempfile::tempdir().unwrap()))
}

#[test]
fn the_commit_msg_hook_writes_the_session_trailer_from_the_session_variable() {
    let Some((dir, home)) = repo() else {
        return;
    };
    let out = commit_msg(
        dir.path(),
        home.path(),
        "Add a thing\n\nClaude-Session: https://claude.ai/code/session_1\n",
        &[("TSK_ATTRIBUTION_IDENTITY", "robot")],
    );
    assert!(
        out.starts_with("Add a thing\n\nAgent-Session: urn:tsk:session:claude-cli:s1\n"),
        "{out}"
    );
    assert!(!out.contains("Claude-Session"), "{out}");
}

#[test]
fn the_commit_msg_hook_changes_nothing_without_the_session_variable() {
    let Some((dir, home)) = repo() else {
        return;
    };
    let message = "Add a thing\n\nClaude-Session: https://claude.ai/code/session_1\n";
    let file = dir.path().join("MSG");
    std::fs::write(&file, message).unwrap();
    let status = crate::common::cmd(env!("CARGO_BIN_EXE_tsk"))
        .current_dir(dir.path())
        .args(["actor", "commit-msg"])
        .arg(&file)
        .env("HOME", home.path())
        .env("XDG_CONFIG_HOME", home.path().join("config"))
        .env("XDG_STATE_HOME", home.path().join("state"))
        .env("TSK_HOME", home.path().join(".tsk"))
        .env("TSK_AUTO_UPDATE", "never")
        .env("CLAUDE_CODE_REMOTE_SESSION_ID", "s1")
        .env("TSK_ATTRIBUTION_IDENTITY", "robot")
        .env("TSK_ATTRIBUTION_CO_AUTHOR", JANE)
        .env_remove("TSK_SESSION")
        .status()
        .unwrap();
    assert!(status.success());
    assert_eq!(std::fs::read_to_string(&file).unwrap(), message);
}

#[test]
fn the_commit_msg_hook_adds_no_co_author_in_an_autonomous_session() {
    let Some((dir, home)) = repo() else {
        return;
    };
    let robot = [
        ("TSK_ATTRIBUTION_IDENTITY", "robot"),
        ("TSK_ATTRIBUTION_CO_AUTHOR", JANE),
    ];
    let out = commit_msg(dir.path(), home.path(), "Add a thing\n", &robot);
    assert!(out.contains("Agent-Session:"), "{out}");
    assert!(!out.contains("Co-authored-by"), "{out}");
}

#[test]
fn the_commit_msg_hook_adds_no_co_author_when_none_is_set() {
    let Some((dir, home)) = repo() else {
        return;
    };
    let out = commit_msg(
        dir.path(),
        home.path(),
        "Add a thing\n",
        &[("TSK_ATTRIBUTION_IDENTITY", "robot")],
    );
    assert!(out.contains("Agent-Session:"), "{out}");
    assert!(!out.contains("Co-authored-by"), "{out}");
}

#[test]
fn the_commit_msg_hook_leaves_a_default_identity_message_alone() {
    let Some((dir, home)) = repo() else {
        return;
    };
    let out = commit_msg(
        dir.path(),
        home.path(),
        "Add a thing\n",
        &[("TSK_ATTRIBUTION_CO_AUTHOR", JANE)],
    );
    assert_eq!(out, "Add a thing\n");
}
