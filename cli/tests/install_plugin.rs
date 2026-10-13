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
            .env("PATH", path)
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
    assert_eq!(lines.len(), 4);
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
