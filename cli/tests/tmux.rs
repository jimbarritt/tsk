#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::Path;

fn logging_tmux(dir: &Path) -> std::path::PathBuf {
    let script = dir.join("fake-tmux.sh");
    std::fs::write(
        &script,
        format!(
            "#!/bin/sh\necho \"$@\" >> \"{}\"\nexit 0\n",
            dir.join("tmux.log").display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    script
}

fn state_none(dir: &Path, pane: Option<&str>) -> std::process::Output {
    let mut command = crate::common::cmd(env!("CARGO_BIN_EXE_tsk"));
    command
        .args(["tmux", "state", "none"])
        .env("TSK_TMUX_BIN", logging_tmux(dir));
    if let Some(pane) = pane {
        command.env("TMUX_PANE", pane);
    }
    command.output().unwrap()
}

#[test]
fn the_session_end_state_does_nothing_outside_tmux() {
    let dir = tempfile::tempdir().unwrap();
    let output = state_none(dir.path(), None);
    assert!(output.status.success());
    assert!(!dir.path().join("tmux.log").exists());
}

#[test]
fn the_session_end_state_removes_the_four_options_of_its_pane() {
    let dir = tempfile::tempdir().unwrap();
    let output = state_none(dir.path(), Some("%7"));
    assert!(output.status.success());
    let log = std::fs::read_to_string(dir.path().join("tmux.log")).unwrap();
    for name in [
        "@tsk_actor",
        "@tsk_mission",
        "@tsk_mission_name",
        "@tsk_state",
    ] {
        assert!(
            log.contains(&format!("set-option -p -u -t %7 {}", name)),
            "{}",
            log
        );
    }
    assert!(!log.contains("@tsk_harness"), "{}", log);
}

#[test]
fn a_failing_tmux_does_not_fail_the_session_end_state() {
    let dir = tempfile::tempdir().unwrap();
    let output = crate::common::cmd(env!("CARGO_BIN_EXE_tsk"))
        .args(["tmux", "state", "none"])
        .env("TSK_TMUX_BIN", dir.path().join("missing-tmux"))
        .env("TMUX_PANE", "%1")
        .output()
        .unwrap();
    assert!(output.status.success());
}
