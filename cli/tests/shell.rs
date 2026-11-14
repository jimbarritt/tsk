//! `tsk shell-init` and `tsk install zshrc`.

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Output};

use tempfile::TempDir;

fn tsk(home: &Path, zdotdir: Option<&Path>, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_tsk"));
    command
        .args(args)
        .current_dir(home)
        .env("HOME", home)
        .env("XDG_STATE_HOME", home.join("state"))
        .env("XDG_CONFIG_HOME", home.join("config"))
        .env("GIT_CEILING_DIRECTORIES", home)
        .env_remove("CLAUDE_CODE_REMOTE")
        .env_remove("ZDOTDIR");
    if let Some(dir) = zdotdir {
        command.env("ZDOTDIR", dir);
    }
    command.output().expect("failed to run tsk")
}

fn out(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn err(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

const LINE: &str = "eval \"$(tsk shell-init zsh)\"";
const MARKER: &str = "# tsk install zshrc: tsk harness cd";

#[test]
fn install_zshrc_adds_the_line_once_under_zdotdir() {
    let root = TempDir::new().unwrap();
    let home = root.path().join("home");
    let zdot = root.path().join("zdot");
    std::fs::create_dir_all(&home).unwrap();

    let first = tsk(&home, Some(&zdot), &["install", "zshrc"]);
    assert!(first.status.success(), "{}", err(&first));
    let rc = zdot.join(".zshrc");
    let text = std::fs::read_to_string(&rc).unwrap();
    assert_eq!(text, format!("{}\n{}\n", MARKER, LINE));
    assert!(!home.join(".zshrc").exists());

    let second = tsk(&home, Some(&zdot), &["install", "zshrc"]);
    assert!(second.status.success());
    assert!(out(&second).contains("already"), "{}", out(&second));
    assert_eq!(std::fs::read_to_string(&rc).unwrap(), text);
    assert_eq!(text.matches(LINE).count(), 1);
}

#[test]
fn install_zshrc_uses_home_and_keeps_existing_text() {
    let root = TempDir::new().unwrap();
    let home = root.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::write(home.join(".zshrc"), "export A=1").unwrap();

    let run = tsk(&home, None, &["install", "zshrc"]);
    assert!(run.status.success(), "{}", err(&run));
    let text = std::fs::read_to_string(home.join(".zshrc")).unwrap();
    assert!(text.starts_with("export A=1\n"));
    assert_eq!(text.matches(LINE).count(), 1);
}

#[test]
fn install_zshrc_print_changes_no_file() {
    let root = TempDir::new().unwrap();
    let home = root.path().join("home");
    std::fs::create_dir_all(&home).unwrap();

    let run = tsk(&home, None, &["install", "zshrc", "--print"]);
    assert!(run.status.success());
    let printed = out(&run);
    assert!(printed.contains(MARKER));
    assert!(printed.contains(LINE));
    assert!(!home.join(".zshrc").exists());
}

#[test]
fn shell_init_rejects_other_shells() {
    let root = TempDir::new().unwrap();
    let run = tsk(root.path(), None, &["shell-init", "fish"]);
    assert_eq!(run.status.code(), Some(1));
    assert!(err(&run).contains("zsh and bash"), "{}", err(&run));
}

fn shell_path(name: &str) -> Option<String> {
    let found = Command::new("sh")
        .args(["-c", &format!("command -v {}", name)])
        .output()
        .ok()?;
    let path = String::from_utf8_lossy(&found.stdout).trim().to_string();
    (found.status.success() && !path.is_empty()).then_some(path)
}

#[test]
fn the_function_changes_directory_and_passes_other_calls() {
    let Some(shell) = shell_path("zsh").or_else(|| shell_path("bash")) else {
        eprintln!("skipped: neither zsh nor bash is installed");
        return;
    };
    let root = TempDir::new().unwrap();
    let home = root.path().join("home");
    let bin = root.path().join("bin");
    let target = root.path().join("harness-x");
    for dir in [&home, &bin, &target] {
        std::fs::create_dir_all(dir).unwrap();
    }
    let fake = bin.join("tsk");
    std::fs::write(
        &fake,
        format!(
            "#!/bin/sh\nif [ \"$1 $2\" = \"harness path\" ]; then echo \"{}\"; exit 0; fi\n\
             echo \"fake: $*\"\nexit 7\n",
            target.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();

    let init = tsk(&home, None, &["shell-init", "zsh"]);
    assert!(init.status.success());
    let script = format!(
        "{}\ntsk harness cd x || exit 20\npwd -P\ncd /\ntsk harness cd; echo \"usage status $?\"\n\
         tsk other thing; echo \"pass status $?\"\n",
        out(&init)
    );
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap());
    let run = Command::new(&shell)
        .args(["-c", &script])
        .current_dir("/")
        .env("PATH", path)
        .env_remove("CLAUDE_CODE_REMOTE")
        .output()
        .unwrap();
    let stdout = out(&run);
    let expected = std::fs::canonicalize(&target).unwrap();
    assert!(
        stdout.contains(&expected.display().to_string()),
        "{}\n{}",
        stdout,
        err(&run)
    );
    assert!(stdout.contains("usage status 1"), "{}", stdout);
    assert!(err(&run).contains("usage: tsk harness cd <name>"));
    assert!(stdout.contains("fake: other thing"), "{}", stdout);
    assert!(stdout.contains("pass status 7"), "{}", stdout);
}
