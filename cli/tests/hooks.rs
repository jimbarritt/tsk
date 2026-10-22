//! The SessionStart hook scripts: what they start, and what they print. The scripts run with
//! stand-ins for `claude`, `dolt` and `tsk` that log their arguments, one line per run.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

fn have(program: &str) -> bool {
    Command::new(program)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
}

fn write_script(path: &Path, text: &str) {
    std::fs::write(path, text).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
}

/// A scratch machine: a home, a state directory, stand-in programs and a repository that
/// holds the scripts.
struct Machine {
    root: tempfile::TempDir,
}

impl Machine {
    fn new() -> Option<Machine> {
        if !(have("git") && have("jq") && cfg!(unix)) {
            eprintln!("note: git or jq is missing; skipping");
            return None;
        }
        let machine = Machine {
            root: tempfile::tempdir().unwrap(),
        };
        let bin = machine.bin();
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::create_dir_all(machine.home()).unwrap();
        // `claude` fails for the plugin step named in CLAUDE_FAIL (for example `install`).
        write_script(
            &bin.join("claude"),
            "#!/bin/sh\necho \"$@\" >> \"$CLAUDE_LOG\"\n\
             if [ -n \"${CLAUDE_FAIL:-}\" ] && [ \"$2\" = \"$CLAUDE_FAIL\" ]; then echo \"it broke\" >&2; exit 1; fi\n\
             exit 0\n",
        );
        write_script(
            &bin.join("dolt"),
            "#!/bin/sh\necho \"$@\" >> \"$DOLT_LOG\"\n\
             case \"$1\" in version) echo \"dolt version 2.5.0\" ;; esac\nexit 0\n",
        );
        write_script(
            &bin.join("tsk"),
            "#!/bin/sh\n\
             case \"$1\" in\n\
               --version) echo \"tsk 0.0.0\" ;;\n\
               thread) cat > /dev/null; echo '{\"hookSpecificOutput\":{\"hookEventName\":\"SessionStart\",\"additionalContext\":\"tsk context\"}}' ;;\n\
             esac\nexit 0\n",
        );
        // The repository the repo script runs in: the scripts, and a workspace version.
        let repo = machine.repo();
        std::fs::create_dir_all(repo.join("plugin").join("hooks")).unwrap();
        std::fs::copy(
            workspace()
                .join("plugin")
                .join("hooks")
                .join("install-dolt.sh"),
            repo.join("plugin").join("hooks").join("install-dolt.sh"),
        )
        .unwrap();
        std::fs::write(repo.join("Cargo.toml"), "version = \"0.0.0\"\n").unwrap();
        let init = Command::new("git")
            .args(["init", "-q"])
            .current_dir(&repo)
            .status()
            .unwrap();
        assert!(init.success());
        Some(machine)
    }

    fn bin(&self) -> PathBuf {
        self.root.path().join("bin")
    }

    fn home(&self) -> PathBuf {
        self.root.path().join("home")
    }

    fn repo(&self) -> PathBuf {
        self.root.path().join("repo")
    }

    fn log(&self, name: &str) -> PathBuf {
        self.root.path().join(format!("{}.log", name))
    }

    fn lines(&self, name: &str) -> Vec<String> {
        std::fs::read_to_string(self.log(name))
            .unwrap_or_default()
            .lines()
            .map(str::to_string)
            .collect()
    }

    fn clear(&self) {
        for name in ["claude", "dolt"] {
            let _ = std::fs::remove_file(self.log(name));
        }
    }

    fn command(&self, script: &Path) -> Command {
        let mut command = Command::new("bash");
        command
            .arg(script)
            .current_dir(self.repo())
            .env("HOME", self.home())
            .env("XDG_STATE_HOME", self.home().join("state"))
            .env("TSK_DOLT_SERVER_IDLE_SECS", "3")
            .env_remove("DOLT_ROOT_PATH")
            .env_remove("TSK_PLUGIN_ENSURE_MINUTES")
            .env_remove("CLAUDE_ENV_FILE")
            .env("CLAUDE_LOG", self.log("claude"))
            .env("DOLT_LOG", self.log("dolt"))
            .env(
                "PATH",
                format!(
                    "{}:{}",
                    self.bin().display(),
                    std::env::var("PATH").unwrap_or_default()
                ),
            );
        command
    }

    fn install_dolt(&self) -> String {
        let script = self
            .repo()
            .join("plugin")
            .join("hooks")
            .join("install-dolt.sh");
        let output = self.command(&script).output().unwrap();
        assert!(output.status.success());
        String::from_utf8_lossy(&output.stdout).into_owned()
    }

    /// The repo script with the hook input on its standard input. Returns its output.
    fn repo_hook(&self, envs: &[(&str, &str)]) -> String {
        let script = workspace()
            .join("ops")
            .join("local")
            .join("claude-session-start.sh");
        let mut command = self.command(&script);
        for (name, value) in envs {
            command.env(name, value);
        }
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        {
            use std::io::Write;
            child
                .stdin
                .take()
                .unwrap()
                .write_all(b"{\"session_id\":\"s1\",\"source\":\"resume\"}")
                .unwrap();
        }
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success());
        String::from_utf8_lossy(&output.stdout).into_owned()
    }

    fn write_identity(&self) {
        let dir = self.home().join(".dolt");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("config_global.json"),
            "{\"user.email\":\"t@example.test\",\"user.name\":\"t\"}",
        )
        .unwrap();
    }

    fn stamp(&self, name: &str) -> PathBuf {
        self.home().join("state").join("tsk").join(name)
    }
}

#[test]
fn the_dolt_installer_checks_the_version_once_and_trusts_its_stamp() {
    let Some(machine) = Machine::new() else {
        return;
    };
    machine.write_identity();
    assert_eq!(machine.install_dolt(), "");
    assert_eq!(machine.lines("dolt"), vec!["version".to_string()]);
    assert!(machine.stamp("dolt-version-ok").is_file());

    // The second run starts no dolt process: the stamp is newer than the binary.
    machine.clear();
    assert_eq!(machine.install_dolt(), "");
    assert_eq!(machine.lines("dolt"), Vec::<String>::new());

    // A binary newer than the stamp is checked again.
    let past = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
    std::fs::File::options()
        .write(true)
        .open(machine.stamp("dolt-version-ok"))
        .unwrap()
        .set_modified(past)
        .unwrap();
    machine.clear();
    assert_eq!(machine.install_dolt(), "");
    assert_eq!(machine.lines("dolt"), vec!["version".to_string()]);
}

#[test]
fn the_dolt_installer_reads_the_identity_from_the_file_and_asks_dolt_when_it_is_absent() {
    let Some(machine) = Machine::new() else {
        return;
    };
    // No identity file: dolt is asked for each of the two values, and sets them.
    assert_eq!(machine.install_dolt(), "");
    let lines = machine.lines("dolt");
    assert_eq!(lines[0], "version");
    assert!(lines.contains(&"config --global --get user.name".to_string()));
    assert!(lines.contains(&"config --global --get user.email".to_string()));
    assert!(lines.contains(&"config --global --add user.name tsk".to_string()));

    // With the file, neither the name nor the email costs a process.
    machine.write_identity();
    machine.clear();
    assert_eq!(machine.install_dolt(), "");
    assert!(
        !machine
            .lines("dolt")
            .iter()
            .any(|line| line.starts_with("config")),
        "{:?}",
        machine.lines("dolt")
    );
}

#[test]
fn the_repo_script_adds_and_refreshes_the_marketplace_once_for_both_plugins() {
    let Some(machine) = Machine::new() else {
        return;
    };
    machine.write_identity();
    let out = machine.repo_hook(&[]);
    let context: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(
        context["hookSpecificOutput"]["additionalContext"],
        "tsk context"
    );
    assert_eq!(
        machine.lines("claude"),
        vec![
            "plugin marketplace add jimbarritt/claude-plugins",
            "plugin install swe@jimbarritt-claude-plugins --scope project -y",
            "plugin install tsk@jimbarritt-claude-plugins --scope project -y",
            "plugin marketplace update jimbarritt-claude-plugins",
            "plugin update swe@jimbarritt-claude-plugins --scope project",
            "plugin update tsk@jimbarritt-claude-plugins --scope project",
        ]
    );
    assert!(machine.stamp("plugin-ensure-ok").is_file());
}

#[test]
fn the_repo_script_skips_the_plugin_steps_within_the_hour_and_on_demand_runs_them() {
    let Some(machine) = Machine::new() else {
        return;
    };
    machine.write_identity();
    machine.repo_hook(&[]);
    machine.clear();

    // Within the hour nothing is run, and the output is the same.
    let out = machine.repo_hook(&[]);
    assert_eq!(machine.lines("claude"), Vec::<String>::new());
    let context: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(
        context["hookSpecificOutput"]["additionalContext"],
        "tsk context"
    );

    // Zero minutes means every time.
    machine.repo_hook(&[("TSK_PLUGIN_ENSURE_MINUTES", "0")]);
    assert_eq!(machine.lines("claude").len(), 6);

    // A stamp older than the limit runs them again.
    machine.clear();
    let past = std::time::SystemTime::now() - std::time::Duration::from_secs(2 * 3600);
    std::fs::File::options()
        .write(true)
        .open(machine.stamp("plugin-ensure-ok"))
        .unwrap()
        .set_modified(past)
        .unwrap();
    machine.repo_hook(&[]);
    assert_eq!(machine.lines("claude").len(), 6);
}

#[test]
fn a_failed_plugin_step_is_a_warning_in_the_context_and_leaves_no_stamp() {
    let Some(machine) = Machine::new() else {
        return;
    };
    machine.write_identity();
    let out = machine.repo_hook(&[("CLAUDE_FAIL", "install")]);
    let context: serde_json::Value = serde_json::from_str(&out).unwrap();
    let text = context["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap();
    assert!(text.starts_with("tsk context "), "{}", text);
    assert!(
        text.contains("WARNING: installing the swe plugin failed: it broke"),
        "{}",
        text
    );
    assert!(
        text.contains("WARNING: installing the tsk plugin failed: it broke"),
        "{}",
        text
    );
    // Neither plugin installed, so neither is updated; the marketplace is still refreshed.
    let lines = machine.lines("claude");
    assert!(lines
        .iter()
        .any(|l| l.starts_with("plugin marketplace update")));
    assert!(!lines.iter().any(|l| l.starts_with("plugin update")));
    assert!(!machine.stamp("plugin-ensure-ok").exists());

    // A failed marketplace refresh warns once for each plugin.
    let out = machine.repo_hook(&[("CLAUDE_FAIL", "marketplace")]);
    let context: serde_json::Value = serde_json::from_str(&out).unwrap();
    let text = context["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap();
    assert!(
        text.contains("WARNING: adding the jimbarritt-claude-plugins marketplace failed: it broke"),
        "{}",
        text
    );
    assert_eq!(
        text.matches("adding the jimbarritt-claude-plugins marketplace failed")
            .count(),
        2
    );
}

/// The SessionEnd script with a stand-in `tsk`. `STATUS` is what `tsk ledger status
/// --local` prints, `STATUS_FAIL` makes it exit 1, and `PUSH_SLEEP` makes
/// `tsk ledger push` run that many seconds. Every call is logged, one line each.
struct EndRun {
    exit_ok: bool,
    calls: Vec<String>,
    seconds: u64,
}

fn session_end(status: &str, status_fails: bool, push_sleep: u32, push_fails: bool) -> EndRun {
    let root = tempfile::tempdir().unwrap();
    let bin = root.path().join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    write_script(
        &bin.join("tsk"),
        "#!/bin/sh\necho \"$@\" >> \"$TSK_LOG\"\n\
         case \"$2\" in\n\
           status) [ -n \"$STATUS_FAIL\" ] && exit 1; printf '%b' \"$STATUS\"; exit 0 ;;\n\
           push) sleep \"$PUSH_SLEEP\"; [ -n \"$PUSH_FAIL\" ] && exit 1; exit 0 ;;\n\
         esac\nexit 0\n",
    );
    let log = root.path().join("tsk.log");
    let script = workspace()
        .join("plugin")
        .join("hooks")
        .join("session-end.sh");
    let started = std::time::Instant::now();
    let mut child = Command::new("bash")
        .arg(script)
        .env(
            "PATH",
            format!(
                "{}:{}",
                bin.display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        )
        .env("TSK_LOG", &log)
        .env("STATUS", status)
        .env("STATUS_FAIL", if status_fails { "1" } else { "" })
        .env("PUSH_SLEEP", push_sleep.to_string())
        .env("PUSH_FAIL", if push_fails { "1" } else { "" })
        .env("TSK_SESSION_END_TIMEOUT_SECS", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        use std::io::Write;
        child
            .stdin
            .take()
            .unwrap()
            .write_all(b"{\"session_id\":\"s1\",\"reason\":\"other\"}")
            .unwrap();
    }
    let output = child.wait_with_output().unwrap();
    assert!(
        output.stdout.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    EndRun {
        exit_ok: output.status.success(),
        calls: std::fs::read_to_string(&log)
            .unwrap_or_default()
            .lines()
            .map(str::to_string)
            .collect(),
        seconds: started.elapsed().as_secs(),
    }
}

const DIRTY: &str =
    "Local   2 changes not pushed\\n  data  missions  1 added\\nOrigin  not checked (--local)\\n";
const CLEAN: &str = "Local   nothing to push\\nOrigin  not checked (--local)\\n";
const AHEAD: &str = "Local   1 commit not pushed\\nOrigin  not checked (--local)\\n";

#[test]
fn the_session_end_script_pushes_when_the_store_holds_unpushed_work() {
    if !(have("bash") && cfg!(unix)) {
        return;
    }
    for status in [DIRTY, AHEAD] {
        let run = session_end(status, false, 0, false);
        assert!(run.exit_ok);
        assert_eq!(
            run.calls,
            vec![
                "ledger status --local".to_string(),
                "ledger push End of session".to_string()
            ]
        );
    }
}

#[test]
fn the_session_end_script_does_not_push_a_clean_store() {
    if !(have("bash") && cfg!(unix)) {
        return;
    }
    let run = session_end(CLEAN, false, 0, false);
    assert!(run.exit_ok);
    assert_eq!(run.calls, vec!["ledger status --local".to_string()]);
}

#[test]
fn the_session_end_script_exits_0_whatever_tsk_does() {
    if !(have("bash") && cfg!(unix)) {
        return;
    }
    // No ledger store: the status fails, nothing is pushed.
    let run = session_end("", true, 0, false);
    assert!(run.exit_ok);
    assert_eq!(run.calls, vec!["ledger status --local".to_string()]);
    // A push that fails.
    let run = session_end(DIRTY, false, 0, true);
    assert!(run.exit_ok);
    assert_eq!(run.calls.len(), 2);
    // A push that hangs is cut off by the timeout.
    if have("timeout") {
        let run = session_end(DIRTY, false, 30, false);
        assert!(run.exit_ok);
        assert_eq!(run.calls.len(), 2);
        assert!(run.seconds < 10, "took {} s", run.seconds);
    }
}

#[test]
fn the_plugin_registers_the_session_end_hook_for_both_harnesses() {
    let claude: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(workspace().join("plugin/hooks/hooks.json")).unwrap(),
    )
    .unwrap();
    let hook = &claude["hooks"]["SessionEnd"][0]["hooks"][0];
    let command = hook["command"].as_str().unwrap();
    assert!(command.ends_with("hooks/session-end.sh\""), "{}", command);
    assert!(hook["timeout"].is_number());
    let copilot: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(workspace().join("plugin/hooks.json")).unwrap(),
    )
    .unwrap();
    let bash = copilot["hooks"]["sessionEnd"][0]["bash"].as_str().unwrap();
    assert!(bash.contains("hooks/session-end.sh"), "{}", bash);
}

/// The Stop script with a stand-in `tsk` that logs its arguments and its input.
fn stop_hook(with_tsk: bool, input: &str) -> (bool, String, String) {
    let root = tempfile::tempdir().unwrap();
    let bin = root.path().join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let log = root.path().join("tsk.log");
    if with_tsk {
        write_script(
            &bin.join("tsk"),
            "#!/bin/sh\necho \"$@\" >> \"$TSK_LOG\"\n/bin/cat >> \"$TSK_LOG\"\necho '{\"decision\":\"block\"}'\nexit 0\n",
        );
    }
    let script = workspace().join("plugin").join("hooks").join("stop.sh");
    let mut child = Command::new("/bin/bash")
        .arg(script)
        .env("PATH", bin.display().to_string())
        .env("TSK_LOG", &log)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        use std::io::Write;
        child.stdin.take().unwrap().write_all(input.as_bytes()).ok(); // the script may exit without reading its input
    }
    let output = child.wait_with_output().unwrap();
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        std::fs::read_to_string(&log).unwrap_or_default(),
    )
}

#[test]
fn the_stop_script_runs_the_guard_with_the_hook_input_and_adds_nothing() {
    let (ok, out, log) = stop_hook(true, "{\"stop_hook_active\":true}");
    assert!(ok);
    assert_eq!(out, "{\"decision\":\"block\"}\n");
    assert_eq!(log, "thread guard\n{\"stop_hook_active\":true}");
}

#[test]
fn the_stop_script_exits_0_without_tsk() {
    let (ok, out, log) = stop_hook(false, "{}");
    assert!(ok);
    assert_eq!(out, "");
    assert_eq!(log, "");
}
