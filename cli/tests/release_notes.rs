use std::process::Output;

fn tsk(args: &[&str]) -> Output {
    crate::common::cmd(env!("CARGO_BIN_EXE_tsk"))
        .args(args)
        .output()
        .unwrap()
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn no_argument_prints_the_notes_of_the_installed_version() {
    let output = tsk(&["release-notes"]);
    assert!(output.status.success());
    let notes = std::fs::read_to_string(format!(
        "{}/../docs/releases/v{}.md",
        env!("CARGO_MANIFEST_DIR"),
        env!("CARGO_PKG_VERSION")
    ))
    .unwrap();
    assert_eq!(stdout(&output), notes);
}

#[test]
fn a_version_prints_its_notes() {
    let output = tsk(&["release-notes", "0.79.0"]);
    assert!(output.status.success());
    assert!(!stdout(&output).is_empty());
}

#[test]
fn since_prints_each_later_release_under_a_heading_newest_first() {
    let output = tsk(&["release-notes", "--since", "0.78.0"]);
    assert!(output.status.success());
    let text = stdout(&output);
    let newer = text.find("# 0.80.0\n").unwrap();
    let older = text.find("# 0.79.0\n").unwrap();
    assert!(newer < older);
    assert!(!text.contains("# 0.78.0\n"));
}

#[test]
fn an_unknown_version_exits_1_and_prints_nothing_on_stdout() {
    let output = tsk(&["release-notes", "99.0.0"]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stdout(&output), "");
    assert!(String::from_utf8_lossy(&output.stderr).contains("no release notes for version 99.0.0"));
}

#[test]
fn the_top_level_help_names_the_command() {
    let output = tsk(&["--help"]);
    assert!(stdout(&output).contains("release-notes"));
}
