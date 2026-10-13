use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

struct Fixture {
    root: TempDir,
}

impl Fixture {
    fn new() -> Fixture {
        let fixture = Fixture {
            root: tempfile::tempdir().unwrap(),
        };
        for dir in ["home", "bin", "empty", "work"] {
            std::fs::create_dir_all(fixture.root.path().join(dir)).unwrap();
        }
        let init = Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(fixture.root.path().join("work"))
            .env("GIT_CEILING_DIRECTORIES", fixture.root.path())
            .output()
            .expect("failed to run git");
        assert!(init.status.success());
        fixture
    }

    fn log(&self) -> PathBuf {
        self.root.path().join("claude.log")
    }

    fn fake_claude(&self, fail_on: Option<&str>) -> PathBuf {
        self.fake_binary("claude", fail_on, 4)
    }

    fn fake_binary(&self, name: &str, fail_on: Option<&str>, message_exit: i32) -> PathBuf {
        let path = self.root.path().join("bin").join(name);
        let fail = match fail_on {
            Some(word) => format!(
                "case \"$*\" in *\"{}\"*) echo \"step failed: $*\" >&2; exit {};; esac\n",
                word, message_exit
            ),
            None => String::new(),
        };
        let script = format!(
            "#!/bin/sh\necho \"$*\" >> \"{}\"\n{}exit 0\n",
            self.log().display(),
            fail
        );
        std::fs::write(&path, script).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    fn logged(&self) -> Vec<String> {
        std::fs::read_to_string(self.log())
            .unwrap_or_default()
            .lines()
            .map(str::to_string)
            .collect()
    }

    fn tsk(&self, path: &Path, claude_bin: Option<&Path>, args: &[&str]) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_tsk"));
        command
            .args(args)
            .current_dir(self.root.path().join("work"))
            .env("HOME", self.root.path().join("home"))
            .env("XDG_STATE_HOME", self.root.path().join("home/state"))
            .env("XDG_CONFIG_HOME", self.root.path().join("home/config"))
            .env("PATH", self.path_with_git(path))
            .env("GIT_CEILING_DIRECTORIES", self.root.path())
            .env_remove("TSK_CLAUDE_BIN")
            .env_remove("TSK_COPILOT_BIN");
        if let Some(bin) = claude_bin {
            command.env(
                if bin.ends_with("copilot") {
                    "TSK_COPILOT_BIN"
                } else {
                    "TSK_CLAUDE_BIN"
                },
                bin,
            );
        }
        command.output().expect("failed to run tsk")
    }

    /// `path` plus a directory holding only a link to git, so the lookup of
    /// `claude` still sees just `path`.
    fn path_with_git(&self, path: &Path) -> std::ffi::OsString {
        let dir = self.root.path().join("gitbin");
        let link = dir.join("git");
        if !link.exists() {
            std::fs::create_dir_all(&dir).unwrap();
            let git = String::from_utf8(
                Command::new("sh")
                    .args(["-c", "command -v git"])
                    .output()
                    .unwrap()
                    .stdout,
            )
            .unwrap();
            std::os::unix::fs::symlink(git.trim(), &link).unwrap();
        }
        std::env::join_paths([path, dir.as_path()]).unwrap()
    }

    fn settings(&self) -> PathBuf {
        self.root.path().join("work/.claude/settings.json")
    }

    fn bin_dir(&self) -> PathBuf {
        self.root.path().join("bin")
    }

    fn empty_dir(&self) -> PathBuf {
        self.root.path().join("empty")
    }
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

const DEFAULT_STEPS: [&str; 4] = [
    "plugin marketplace add jimbarritt/claude-plugins",
    "plugin install tsk@jimbarritt-claude-plugins --scope project -y",
    "plugin marketplace update jimbarritt-claude-plugins",
    "plugin update tsk@jimbarritt-claude-plugins --scope project",
];

#[test]
fn install_plugin_runs_the_four_steps_through_the_override_binary() {
    let fx = Fixture::new();
    let claude = fx.fake_claude(None);

    let output = fx.tsk(
        &fx.empty_dir(),
        Some(&claude),
        &["install-plugin", "claude-cli"],
    );

    assert_success(&output);
    assert_eq!(fx.logged(), DEFAULT_STEPS);
    let text = stdout(&output);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 6);
    assert_eq!(
        lines[1],
        "install tsk@jimbarritt-claude-plugins at project scope: claude plugin install tsk@jimbarritt-claude-plugins --scope project -y"
    );
}

#[test]
fn install_plugin_finds_claude_on_path_and_exits_0_when_run_twice() {
    let fx = Fixture::new();
    fx.fake_claude(None);

    assert_success(&fx.tsk(&fx.bin_dir(), None, &["install-plugin", "claude-cli"]));
    assert_success(&fx.tsk(&fx.bin_dir(), None, &["install-plugin", "claude-cli"]));

    let logged = fx.logged();
    assert_eq!(logged.len(), 8);
    assert_eq!(logged[..4], DEFAULT_STEPS);
    assert_eq!(logged[4..], DEFAULT_STEPS);
}

#[test]
fn install_plugin_passes_scope_plugins_and_marketplace() {
    let fx = Fixture::new();
    let claude = fx.fake_claude(None);

    let output = fx.tsk(
        &fx.empty_dir(),
        Some(&claude),
        &[
            "install-plugin",
            "claude-cli",
            "--scope",
            "user",
            "--plugin",
            "swe",
            "--plugin",
            "tsk",
            "--marketplace",
            "acme/plugins",
            "--marketplace-name",
            "acme",
        ],
    );

    assert_success(&output);
    assert_eq!(
        fx.logged(),
        [
            "plugin marketplace add acme/plugins",
            "plugin install swe@acme --scope user -y",
            "plugin install tsk@acme --scope user -y",
            "plugin marketplace update acme",
            "plugin update swe@acme --scope user",
            "plugin update tsk@acme --scope user",
        ]
    );
}

#[test]
fn install_plugin_stops_with_an_error_when_claude_is_not_found() {
    let fx = Fixture::new();

    let output = fx.tsk(&fx.empty_dir(), None, &["install-plugin", "claude-cli"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr(&output).contains("claude is not on PATH"),
        "{}",
        stderr(&output)
    );
    assert!(stderr(&output).contains("TSK_CLAUDE_BIN"));
}

#[test]
fn install_plugin_names_the_failed_step_and_shows_its_stderr() {
    let fx = Fixture::new();
    let claude = fx.fake_claude(Some("marketplace update"));

    let output = fx.tsk(
        &fx.empty_dir(),
        Some(&claude),
        &["install-plugin", "claude-cli"],
    );

    assert_eq!(output.status.code(), Some(1));
    let err = stderr(&output);
    assert!(
        err.contains("could not refresh the jimbarritt-claude-plugins marketplace"),
        "{}",
        err
    );
    assert!(err.contains("exit status 4"), "{}", err);
    assert!(
        err.contains("step failed: plugin marketplace update jimbarritt-claude-plugins"),
        "{}",
        err
    );
    assert_eq!(fx.logged().len(), 3);
    assert_eq!(stdout(&output).lines().count(), 2);
}

const COPILOT_STEPS: [&str; 4] = [
    "plugin marketplace add jimbarritt/tsk",
    "plugin install tsk@tsk",
    "plugin marketplace update tsk",
    "plugin update tsk",
];

#[test]
fn copilot_cli_runs_the_four_steps_through_the_override_binary() {
    let fx = Fixture::new();
    let copilot = fx.fake_binary("copilot", None, 4);

    let output = fx.tsk(
        &fx.empty_dir(),
        Some(&copilot),
        &["install-plugin", "copilot-cli"],
    );

    assert_success(&output);
    assert_eq!(fx.logged(), COPILOT_STEPS);
    let text = stdout(&output);
    assert_eq!(text.lines().count(), 4);
    assert!(!fx.settings().exists());
    assert_eq!(
        text.lines().nth(1).unwrap(),
        "install tsk@tsk: copilot plugin install tsk@tsk"
    );
}

#[test]
fn copilot_cli_passes_plugins_and_marketplace_overrides() {
    let fx = Fixture::new();
    let copilot = fx.fake_binary("copilot", None, 4);

    let output = fx.tsk(
        &fx.empty_dir(),
        Some(&copilot),
        &[
            "install-plugin",
            "copilot-cli",
            "--plugin",
            "swe",
            "--marketplace",
            "acme/plugins",
            "--marketplace-name",
            "acme",
        ],
    );

    assert_success(&output);
    assert_eq!(
        fx.logged(),
        [
            "plugin marketplace add acme/plugins",
            "plugin install swe@acme",
            "plugin marketplace update acme",
            "plugin update swe",
        ]
    );
}

#[test]
fn copilot_cli_rejects_an_explicit_scope_before_running_anything() {
    let fx = Fixture::new();
    let copilot = fx.fake_binary("copilot", None, 4);

    let output = fx.tsk(
        &fx.empty_dir(),
        Some(&copilot),
        &["install-plugin", "copilot-cli", "--scope", "user"],
    );

    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr(&output).contains("installs plugins per user"),
        "{}",
        stderr(&output)
    );
    assert!(fx.logged().is_empty());
}

#[test]
fn copilot_cli_stops_with_an_error_naming_copilot_when_it_is_not_found() {
    let fx = Fixture::new();

    let output = fx.tsk(&fx.empty_dir(), None, &["install-plugin", "copilot-cli"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr(&output).contains("copilot is not on PATH"),
        "{}",
        stderr(&output)
    );
    assert!(stderr(&output).contains("TSK_COPILOT_BIN"));
}

#[test]
fn copilot_cli_names_the_failed_step_and_shows_its_stderr() {
    let fx = Fixture::new();
    let copilot = fx.fake_binary("copilot", Some("marketplace update"), 4);

    let output = fx.tsk(
        &fx.empty_dir(),
        Some(&copilot),
        &["install-plugin", "copilot-cli"],
    );

    assert_eq!(output.status.code(), Some(1));
    let err = stderr(&output);
    assert!(
        err.contains("could not refresh the tsk marketplace"),
        "{}",
        err
    );
    assert!(err.contains("exit status 4"), "{}", err);
    assert!(
        err.contains("step failed: plugin marketplace update tsk"),
        "{}",
        err
    );
    assert_eq!(fx.logged().len(), 3);
}

#[test]
fn copilot_cli_treats_already_registered_and_installed_as_success() {
    let fx = Fixture::new();
    let path = fx.bin_dir().join("copilot");
    std::fs::write(
        &path,
        format!(
            "#!/bin/sh\necho \"$*\" >> \"{}\"\ncase \"$*\" in *marketplace\\ add*|*plugin\\ install*) echo 'Already installed' >&2; exit 1;; esac\nexit 0\n",
            fx.log().display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();

    let output = fx.tsk(
        &fx.empty_dir(),
        Some(&path),
        &["install-plugin", "copilot-cli"],
    );

    assert_success(&output);
    assert_eq!(fx.logged(), COPILOT_STEPS);
    let text = stdout(&output);
    let lines: Vec<&str> = text.lines().collect();
    assert!(lines[0].ends_with("(already done, continuing)"), "{}", text);
    assert!(lines[1].ends_with("(already done, continuing)"), "{}", text);
    assert!(!lines[2].contains("already"));
}

const HOOK_TEXT: &str = r#"{
  "hooks": {
    "SessionStart": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "tsk thread session-start --install-plugin claude-cli"
          }
        ]
      }
    ]
  }
}
"#;

#[test]
fn claude_cli_writes_the_hook_into_an_empty_repo() {
    let fx = Fixture::new();
    let claude = fx.fake_claude(None);

    let output = fx.tsk(
        &fx.empty_dir(),
        Some(&claude),
        &["install-plugin", "claude-cli"],
    );

    assert_success(&output);
    assert_eq!(std::fs::read_to_string(fx.settings()).unwrap(), HOOK_TEXT);
    let text = stdout(&output);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 6);
    assert_eq!(
        lines[4],
        "wrote the SessionStart hook to .claude/settings.json"
    );
    assert!(lines[5].starts_with("commit .claude/settings.json"));
}

#[test]
fn a_second_run_leaves_the_hook_alone() {
    let fx = Fixture::new();
    let claude = fx.fake_claude(None);
    let args = ["install-plugin", "claude-cli"];
    assert_success(&fx.tsk(&fx.empty_dir(), Some(&claude), &args));

    let output = fx.tsk(&fx.empty_dir(), Some(&claude), &args);

    assert_success(&output);
    assert_eq!(std::fs::read_to_string(fx.settings()).unwrap(), HOOK_TEXT);
    let text = stdout(&output);
    assert_eq!(
        text.lines().last().unwrap(),
        "hook already present: .claude/settings.json"
    );
    assert!(!text.contains("commit .claude"));
}

#[test]
fn no_hook_writes_no_settings_file() {
    let fx = Fixture::new();
    let claude = fx.fake_claude(None);

    let output = fx.tsk(
        &fx.empty_dir(),
        Some(&claude),
        &["install-plugin", "claude-cli", "--no-hook"],
    );

    assert_success(&output);
    assert!(!fx.settings().exists());
    assert_eq!(stdout(&output).lines().count(), 4);
}

#[test]
fn user_scope_writes_no_hook_and_says_so() {
    let fx = Fixture::new();
    let claude = fx.fake_claude(None);

    let output = fx.tsk(
        &fx.empty_dir(),
        Some(&claude),
        &["install-plugin", "claude-cli", "--scope", "user"],
    );

    assert_success(&output);
    assert!(!fx.settings().exists());
    assert!(stdout(&output).contains("skipped the SessionStart hook"));
}

#[test]
fn a_malformed_settings_file_is_an_error_and_stays_as_it_was() {
    let fx = Fixture::new();
    let claude = fx.fake_claude(None);
    std::fs::create_dir_all(fx.settings().parent().unwrap()).unwrap();
    std::fs::write(fx.settings(), "{oops").unwrap();

    let output = fx.tsk(
        &fx.empty_dir(),
        Some(&claude),
        &["install-plugin", "claude-cli"],
    );

    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("malformed"), "{}", stderr(&output));
    assert_eq!(std::fs::read_to_string(fx.settings()).unwrap(), "{oops");
}

#[test]
fn an_existing_settings_file_keeps_its_other_keys() {
    let fx = Fixture::new();
    let claude = fx.fake_claude(None);
    std::fs::create_dir_all(fx.settings().parent().unwrap()).unwrap();
    std::fs::write(fx.settings(), "{\"model\":\"x\",\"env\":{}}").unwrap();

    assert_success(&fx.tsk(
        &fx.empty_dir(),
        Some(&claude),
        &["install-plugin", "claude-cli"],
    ));

    let text = std::fs::read_to_string(fx.settings()).unwrap();
    let at = |key: &str| text.find(&format!("\n  \"{}\"", key)).unwrap();
    assert!(at("model") < at("env") && at("env") < at("hooks"));
}
