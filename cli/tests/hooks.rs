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
        std::fs::copy(
            workspace()
                .join("plugin")
                .join("hooks")
                .join("install-tsk.sh"),
            repo.join("plugin").join("hooks").join("install-tsk.sh"),
        )
        .unwrap();
        // The stand-in tsk reports 0.0.0, so the hook tests start with a current tsk.
        std::fs::write(repo.join("plugin").join("tsk-version"), "0.0.0\n").unwrap();
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

// The tsk installer. A scratch PATH holds only the tools the script needs, so no tsk on the
// real machine is found. Release archives are built locally and served by a file:// base.

const TARGETS: [&str; 4] = [
    "aarch64-apple-darwin",
    "x86_64-apple-darwin",
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
];

fn which(program: &str) -> Option<PathBuf> {
    std::env::var("PATH")
        .unwrap_or_default()
        .split(':')
        .map(|dir| Path::new(dir).join(program))
        .find(|path| path.is_file())
}

/// A stand-in tsk that reports `version`. `self_update` is `none` (the command does
/// nothing), `fail` (exits 1), or `upgrade` (replaces the stand-in with one at 0.26.0).
/// Every `self-update` run is logged to `$SU_LOG`.
fn stub_tsk(path: &Path, version: &str, self_update: &str) {
    let behaviour = match self_update {
        "fail" => "exit 1".to_string(),
        "upgrade" => "printf '#!/bin/sh\\ncase \"$1\" in --version) echo \"tsk 0.26.0\";; esac\\n' > \"$0\"; chmod +x \"$0\"".to_string(),
        _ => "exit 0".to_string(),
    };
    write_script(
        path,
        &format!(
            "#!/bin/sh\ncase \"$1\" in\n  --version) echo \"tsk {}\" ;;\n  self-update) echo run >> \"$SU_LOG\"; {} ;;\nesac\n",
            version, behaviour
        ),
    );
}

struct Installer {
    machine: Machine,
}

impl Installer {
    fn new() -> Option<Installer> {
        let machine = Machine::new()?;
        for tool in ["curl", "tar", "git"] {
            if !have(tool) {
                eprintln!("note: {} is missing; skipping", tool);
                return None;
            }
        }
        let sys = machine.root.path().join("sys");
        std::fs::create_dir_all(&sys).unwrap();
        for tool in [
            "awk", "sed", "sort", "head", "tail", "tr", "uname", "mktemp", "curl", "tar",
            "install", "mv", "rm", "dirname", "readlink", "cat", "git", "mkdir", "env", "bash",
            "chmod", "grep", "date", "sh", "gzip", "gunzip", "basename", "id",
        ] {
            if let Some(real) = which(tool) {
                std::os::unix::fs::symlink(real, sys.join(tool)).unwrap();
            }
        }
        std::fs::write(
            machine.repo().join("plugin").join("tsk-version"),
            "0.26.0\n",
        )
        .unwrap();
        let installer = Installer { machine };
        installer.release("0.26.0");
        Some(installer)
    }

    fn path(&self, dirs: &[&Path]) -> String {
        let mut parts: Vec<String> = dirs.iter().map(|d| d.display().to_string()).collect();
        parts.push(self.machine.root.path().join("sys").display().to_string());
        parts.join(":")
    }

    fn releases(&self) -> PathBuf {
        self.machine.root.path().join("releases")
    }

    /// Builds the archives of release `version` for the four targets, holding a tsk that
    /// reports `version`.
    fn release(&self, version: &str) {
        let work = self.machine.root.path().join(format!("build-{}", version));
        std::fs::create_dir_all(&work).unwrap();
        stub_tsk(&work.join("tsk"), version, "none");
        let dir = self.releases().join(format!("v{}", version));
        std::fs::create_dir_all(&dir).unwrap();
        for target in TARGETS {
            let status = Command::new("tar")
                .arg("-czf")
                .arg(dir.join(format!("tsk-{}-{}.tar.gz", version, target)))
                .arg("-C")
                .arg(&work)
                .arg("tsk")
                .status()
                .unwrap();
            assert!(status.success());
        }
    }

    /// Runs the installer with the directories first on the PATH. Returns its output.
    fn run(&self, dirs: &[&Path], envs: &[(&str, String)]) -> String {
        let script = self
            .machine
            .repo()
            .join("plugin")
            .join("hooks")
            .join("install-tsk.sh");
        let mut command = self.machine.command(&script);
        command
            .env("PATH", self.path(dirs))
            .env(
                "TSK_RELEASE_DOWNLOAD_URL",
                format!("file://{}", self.releases().display()),
            )
            .env(
                "TSK_RELEASE_REPO_URL",
                self.machine.root.path().join("no-repo"),
            )
            .env("SU_LOG", self.machine.log("selfupdate"))
            .env_remove("CARGO_HOME")
            .env_remove("TSK_INSTALL_DIR");
        for (name, value) in envs {
            match name.strip_prefix('-') {
                Some(unset) => command.env_remove(unset),
                None => command.env(name, value),
            };
        }
        let output = command.output().unwrap();
        assert!(output.status.success(), "the installer must exit 0");
        String::from_utf8_lossy(&output.stdout).into_owned()
    }

    /// Sets the version in plugin/tsk-version.
    fn require(&self, version: &str) {
        std::fs::write(
            self.machine.repo().join("plugin").join("tsk-version"),
            format!("{}\n", version),
        )
        .unwrap();
    }

    /// Replaces the stand-in `uname` on the scratch PATH.
    fn uname(&self, os: &str, arch: &str) {
        let uname = self.machine.root.path().join("sys").join("uname");
        let _ = std::fs::remove_file(&uname);
        write_script(
            &uname,
            &format!(
                "#!/bin/sh\ncase \"$1\" in -s) echo {} ;; -m) echo {} ;; esac\n",
                os, arch
            ),
        );
    }

    /// The names left in `dir` that start with a dot: staged files of a failed install.
    fn leftovers(&self, dir: &Path) -> Vec<String> {
        std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with('.'))
            .collect()
    }

    fn version_of(&self, tsk: &Path) -> String {
        let output = Command::new(tsk).arg("--version").output().unwrap();
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    fn dir(&self, name: &str) -> PathBuf {
        let dir = self.machine.root.path().join(name);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }
}

fn bad_base() -> (&'static str, String) {
    (
        "TSK_RELEASE_DOWNLOAD_URL",
        "file:///nonexistent".to_string(),
    )
}

#[test]
fn the_tsk_installer_installs_the_release_when_there_is_no_tsk() {
    let Some(i) = Installer::new() else {
        return;
    };
    let target = i.dir("install");
    let env_file = i.machine.root.path().join("claude-env");
    let out = i.run(
        &[],
        &[
            ("TSK_INSTALL_DIR", target.display().to_string()),
            ("CLAUDE_ENV_FILE", env_file.display().to_string()),
        ],
    );
    assert_eq!(out, "");
    assert_eq!(i.version_of(&target.join("tsk")), "tsk 0.26.0");
    // The directory was not on the PATH, so later commands of the session get it.
    let env = std::fs::read_to_string(&env_file).unwrap();
    assert!(env.contains(&target.display().to_string()), "{}", env);
}

#[test]
fn the_tsk_installer_takes_the_highest_release_tag_when_it_can_look_it_up() {
    let Some(i) = Installer::new() else {
        return;
    };
    i.release("0.30.1");
    let repo = i.dir("tags");
    for args in [
        vec!["init", "-q"],
        vec![
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@e",
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "x",
        ],
        vec!["tag", "v0.26.0"],
        vec!["tag", "v0.30.1"],
        vec!["tag", "v0.9.0"],
        vec!["tag", "v1.0.0-rc1"],
    ] {
        assert!(Command::new("git")
            .args(&args)
            .current_dir(&repo)
            .status()
            .unwrap()
            .success());
    }
    let target = i.dir("install");
    let out = i.run(
        &[],
        &[
            ("TSK_INSTALL_DIR", target.display().to_string()),
            ("TSK_RELEASE_REPO_URL", repo.display().to_string()),
        ],
    );
    assert_eq!(out, "");
    assert_eq!(i.version_of(&target.join("tsk")), "tsk 0.30.1");
}

#[test]
fn the_tsk_installer_replaces_a_stale_tsk_through_self_update() {
    let Some(i) = Installer::new() else {
        return;
    };
    let first = i.dir("first");
    let behind = i.dir("behind");
    stub_tsk(&first.join("tsk"), "0.12.1", "upgrade");
    stub_tsk(&behind.join("tsk"), "0.12.1", "none");
    let out = i.run(&[&first, &behind], &[]);
    assert_eq!(out, "");
    assert_eq!(i.machine.lines("selfupdate"), vec!["run".to_string()]);
    assert_eq!(i.version_of(&first.join("tsk")), "tsk 0.26.0");
    // The copy later on the path is not the one the session runs.
    assert_eq!(i.version_of(&behind.join("tsk")), "tsk 0.12.1");
}

#[test]
fn the_tsk_installer_downloads_over_a_stale_tsk_that_cannot_self_update() {
    let Some(i) = Installer::new() else {
        return;
    };
    for mode in ["fail", "none"] {
        let first = i.dir(&format!("first-{}", mode));
        stub_tsk(&first.join("tsk"), "0.12.1", mode);
        let out = i.run(&[&first], &[]);
        assert_eq!(out, "", "{}", mode);
        assert_eq!(i.version_of(&first.join("tsk")), "tsk 0.26.0", "{}", mode);
    }
}

#[test]
fn the_tsk_installer_does_not_run_self_update_on_a_cargo_install() {
    let Some(i) = Installer::new() else {
        return;
    };
    // self-update would compile with cargo there, so the archive replaces the file.
    let cargo_bin = i.machine.home().join(".cargo").join("bin");
    std::fs::create_dir_all(&cargo_bin).unwrap();
    stub_tsk(&cargo_bin.join("tsk"), "0.12.1", "upgrade");
    let out = i.run(&[&cargo_bin], &[]);
    assert_eq!(out, "");
    assert_eq!(i.machine.lines("selfupdate"), Vec::<String>::new());
    assert_eq!(i.version_of(&cargo_bin.join("tsk")), "tsk 0.26.0");
}

#[test]
fn the_tsk_installer_never_replaces_a_newer_or_equal_development_build() {
    let Some(i) = Installer::new() else {
        return;
    };
    let dev = i.dir("dev");
    let behind = i.dir("behind");
    stub_tsk(&dev.join("tsk"), "0.27.0", "upgrade");
    stub_tsk(&behind.join("tsk"), "0.12.1", "upgrade");
    // A download base that does not exist: any download attempt is a warning.
    let out = i.run(&[&dev, &behind], &[bad_base()]);
    assert_eq!(out, "");
    assert_eq!(i.machine.lines("selfupdate"), Vec::<String>::new());
    assert_eq!(i.version_of(&dev.join("tsk")), "tsk 0.27.0");
    assert_eq!(i.version_of(&behind.join("tsk")), "tsk 0.12.1");

    // A build at exactly the required version is current too.
    stub_tsk(&dev.join("tsk"), "0.26.0", "upgrade");
    let out = i.run(&[&dev], &[bad_base()]);
    assert_eq!(out, "");
    assert_eq!(i.machine.lines("selfupdate"), Vec::<String>::new());
}

#[test]
fn the_tsk_installer_warns_and_exits_0_when_the_download_fails() {
    let Some(i) = Installer::new() else {
        return;
    };
    let target = i.dir("install");
    let out = i.run(
        &[],
        &[
            ("TSK_INSTALL_DIR", target.display().to_string()),
            bad_base(),
        ],
    );
    assert!(
        out.starts_with("WARNING: downloading tsk 0.26.0 failed"),
        "{}",
        out
    );
    assert!(!target.join("tsk").exists());

    // A stale tsk stays in place when the download fails.
    let first = i.dir("first");
    stub_tsk(&first.join("tsk"), "0.12.1", "fail");
    let out = i.run(&[&first], &[bad_base()]);
    assert!(out.starts_with("WARNING: downloading tsk"), "{}", out);
    assert_eq!(i.version_of(&first.join("tsk")), "tsk 0.12.1");
}

#[test]
fn the_tsk_installer_picks_the_archive_target_of_the_platform() {
    let Some(i) = Installer::new() else {
        return;
    };
    let sys = i.machine.root.path().join("sys");
    for (os, arch, target) in [
        ("Darwin", "arm64", "aarch64-apple-darwin"),
        ("Darwin", "x86_64", "x86_64-apple-darwin"),
        ("Linux", "x86_64", "x86_64-unknown-linux-gnu"),
        ("Linux", "aarch64", "aarch64-unknown-linux-gnu"),
    ] {
        // Only the archive of this target exists, and its tsk reports the target.
        let work = i.dir(&format!("only-{}", target));
        write_script(
            &work.join("tsk"),
            &format!("#!/bin/sh\necho \"tsk 0.26.0 {}\"\n", target),
        );
        let base = i.dir(&format!("base-{}", target));
        let dir = base.join("v0.26.0");
        std::fs::create_dir_all(&dir).unwrap();
        assert!(Command::new("tar")
            .arg("-czf")
            .arg(dir.join(format!("tsk-0.26.0-{}.tar.gz", target)))
            .arg("-C")
            .arg(&work)
            .arg("tsk")
            .status()
            .unwrap()
            .success());
        let _ = std::fs::remove_file(sys.join("uname"));
        write_script(
            &sys.join("uname"),
            &format!(
                "#!/bin/sh\ncase \"$1\" in -s) echo {} ;; -m) echo {} ;; esac\n",
                os, arch
            ),
        );
        let install = i.dir(&format!("install-{}", target));
        let out = i.run(
            &[],
            &[
                ("TSK_INSTALL_DIR", install.display().to_string()),
                (
                    "TSK_RELEASE_DOWNLOAD_URL",
                    format!("file://{}", base.display()),
                ),
            ],
        );
        assert_eq!(out, "", "{}", target);
        assert!(
            i.version_of(&install.join("tsk")).ends_with(target),
            "{}",
            target
        );
    }
    // A platform with no release binary is a warning.
    let _ = std::fs::remove_file(sys.join("uname"));
    write_script(
        &sys.join("uname"),
        "#!/bin/sh\ncase \"$1\" in -s) echo Linux ;; -m) echo riscv64 ;; esac\n",
    );
    let out = i.run(
        &[],
        &[("TSK_INSTALL_DIR", i.dir("none").display().to_string())],
    );
    assert!(
        out.starts_with("WARNING: tsk has no release binary for Linux on riscv64"),
        "{}",
        out
    );
}

#[test]
fn the_tsk_installer_follows_the_rules_of_self_update() {
    // The script cannot call the Rust code, so this test reads both files and fails when
    // the target mapping, the archive URL, the defaults or the tag lookup differ.
    let rust = std::fs::read_to_string(workspace().join("cli/src/self_update.rs")).unwrap();
    let script = std::fs::read_to_string(workspace().join("plugin/hooks/install-tsk.sh")).unwrap();
    let quoted = |line: &str| -> Vec<String> {
        line.split('"')
            .skip(1)
            .step_by(2)
            .map(str::to_string)
            .collect()
    };
    let body = |text: &str, start: &str, end: &str| -> Vec<String> {
        let from = text.find(start).unwrap_or_else(|| panic!("no {}", start));
        let rest = &text[from..];
        rest[..rest.find(end).unwrap_or_else(|| panic!("no {}", end))]
            .lines()
            .map(|l| l.trim().to_string())
            .collect()
    };

    // The targets. Rust: ("macos", "aarch64") => Ok("aarch64-apple-darwin"),
    let mut rust_map = std::collections::BTreeMap::new();
    for line in body(&rust, "pub fn target_for", "_ =>") {
        if line.starts_with("(\"") {
            let q = quoted(&line);
            assert_eq!(q.len(), 3, "{}", line);
            rust_map.insert((q[0].clone(), q[1].clone()), q[2].clone());
        }
    }
    assert_eq!(rust_map.len(), 4, "{:?}", rust_map);
    // Script: Darwin/arm64 | Darwin/aarch64) echo "aarch64-apple-darwin" ;;
    let mut covered = std::collections::BTreeSet::new();
    for line in body(&script, "target_for() {", "*) return 1") {
        let Some((patterns, rest)) = line.split_once(')') else {
            continue;
        };
        if !rest.contains("echo") {
            continue;
        }
        let target = quoted(rest).remove(0);
        for pattern in patterns.split('|') {
            let (os, arch) = pattern.trim().split_once('/').unwrap();
            let os = match os {
                "Darwin" => "macos",
                "Linux" => "linux",
                other => panic!("unknown os {}", other),
            };
            let arch = match arch {
                "arm64" | "aarch64" => "aarch64",
                "x86_64" | "amd64" => "x86_64",
                other => panic!("unknown arch {}", other),
            };
            assert_eq!(
                rust_map.get(&(os.to_string(), arch.to_string())),
                Some(&target),
                "{}",
                line
            );
            covered.insert((os.to_string(), arch.to_string()));
        }
    }
    assert_eq!(
        covered,
        rust_map.keys().cloned().collect(),
        "the script must map every platform that self_update.rs maps"
    );

    // The archive URL. Rust: format!("{}/v{}/tsk-{}-{}.tar.gz", base.trim_end_matches('/'), ...)
    let lines = body(&rust, "pub fn archive_url", "\n}");
    let format_at = lines.iter().position(|l| l.contains("\"{}/v")).unwrap();
    let mut template = quoted(&lines[format_at]).remove(0);
    let args: Vec<&str> = lines[format_at + 1..]
        .iter()
        .map(|l| l.trim_end_matches(',').trim())
        .filter(|l| !l.is_empty() && *l != ")")
        .collect();
    assert_eq!(
        args,
        ["base.trim_end_matches('/')", "version", "version", "target"],
        "the arguments of archive_url changed; update this test and install-tsk.sh"
    );
    for value in ["BASE", "VER", "VER", "TARGET"] {
        template = template.replacen("{}", value, 1);
    }
    let url_line = script
        .lines()
        .find(|l| l.trim_start().starts_with("url=\""))
        .unwrap();
    let script_url = quoted(url_line)
        .remove(0)
        .replace("${DOWNLOAD_BASE%/}", "BASE")
        .replace("$version", "VER")
        .replace("$target", "TARGET");
    assert_eq!(script_url, template);

    // The defaults and the variables that override them.
    for (constant, variable) in [
        ("DEFAULT_REPO_URL", "TSK_RELEASE_REPO_URL"),
        ("DEFAULT_DOWNLOAD_URL", "TSK_RELEASE_DOWNLOAD_URL"),
    ] {
        let line = rust
            .lines()
            .find(|l| l.starts_with(&format!("pub const {}:", constant)))
            .unwrap();
        let default = quoted(line).remove(0);
        assert!(
            script.contains(&format!("${{{}:-{}}}", variable, default)),
            "{} default {} differs",
            variable,
            default
        );
        assert!(
            rust.contains(&format!("\"{}\"", variable)),
            "{} is not a variable of self_update.rs",
            variable
        );
    }

    // The tag lookup: the same command and the same tag shape.
    assert!(rust.contains("[\"ls-remote\", \"--tags\", \"--refs\""));
    assert!(script.contains("ls-remote --tags --refs"));
    assert!(rust.contains("strip_prefix(\"refs/tags/v\")"));
    assert!(script.contains("refs/tags/v\\("));
}

#[test]
fn the_tsk_installer_prefers_the_homebrew_directory_on_apple_silicon() {
    let Some(i) = Installer::new() else {
        return;
    };
    let brew = i.dir("opt-homebrew-bin");
    let brew_env = || ("TSK_HOMEBREW_BIN_DIR", brew.display().to_string());

    // Apple silicon, directory present and writable: it is the target.
    i.uname("Darwin", "arm64");
    let out = i.run(&[], &[brew_env()]);
    assert_eq!(out, "");
    assert_eq!(i.version_of(&brew.join("tsk")), "tsk 0.26.0");
    std::fs::remove_file(brew.join("tsk")).unwrap();

    // TSK_INSTALL_DIR is explicit, so it wins.
    let explicit = i.dir("explicit");
    i.run(
        &[],
        &[
            brew_env(),
            ("TSK_INSTALL_DIR", explicit.display().to_string()),
        ],
    );
    assert!(explicit.join("tsk").exists());
    assert!(!brew.join("tsk").exists());
}

#[test]
fn the_tsk_installer_replaces_a_plain_file_in_the_homebrew_directory_but_not_a_cellar_file() {
    let Some(i) = Installer::new() else {
        return;
    };
    // A file this script put in /opt/homebrew/bin is replaced like any other.
    let brew = i.dir("homebrew/bin");
    stub_tsk(&brew.join("tsk"), "0.12.1", "none");
    let out = i.run(&[&brew], &[]);
    assert_eq!(out, "");
    assert_eq!(i.version_of(&brew.join("tsk")), "tsk 0.26.0");

    // A link into a Cellar is brew's, so the script warns and leaves it.
    let cellar = i.dir("homebrew/Cellar/tsk/0.12.1/bin");
    stub_tsk(&cellar.join("tsk"), "0.12.1", "none");
    let linked = i.dir("homebrew/linked");
    std::os::unix::fs::symlink(cellar.join("tsk"), linked.join("tsk")).unwrap();
    let out = i.run(&[&linked], &[]);
    assert!(out.contains("brew upgrade"), "{}", out);
    assert_eq!(i.version_of(&cellar.join("tsk")), "tsk 0.12.1");
}

#[test]
fn the_tsk_installer_compares_versions_as_numbers_and_reads_any_version_output() {
    let Some(i) = Installer::new() else {
        return;
    };
    let dir = i.dir("cmp");
    let write = |output: &str| {
        write_script(
            &dir.join("tsk"),
            &format!(
                "#!/bin/sh\ncase \"$1\" in --version) echo '{}' ;; self-update) exit 0 ;; esac\n",
                output
            ),
        );
    };
    // (required, installed output, replaced)
    for (required, output, replaced) in [
        ("0.15.0", "tsk 0.9.0", true),
        ("0.9.0", "tsk 0.15.0", false),
        ("0.26.10", "tsk 0.26.1", true),
        ("0.26.2", "tsk 0.26.10", false),
        ("0.26.0", "tsk 0.26.0", false),
        ("1.0.0", "tsk 0.99.99", true),
        (
            "0.26.0",
            "tsk version 0.30.2 (git abc123, built 2026-10-24)",
            false,
        ),
        ("0.26.0", "0.30.2", false),
        ("0.26.0", "tsk 0.30.2-dev+7e1a095", false),
        ("0.31.0", "tsk 0.30.2-dev+7e1a095", true),
    ] {
        i.require(required);
        i.release(required);
        write(output);
        i.run(&[&dir], &[]);
        let now = i.version_of(&dir.join("tsk"));
        assert_eq!(
            now != output,
            replaced,
            "required {} installed {:?} now {:?}",
            required,
            output,
            now
        );
    }
    // No version in the output at all: the file is treated as stale and replaced.
    i.require("0.26.0");
    write("usage: tsk <command>");
    i.run(&[&dir], &[]);
    assert_eq!(i.version_of(&dir.join("tsk")), "tsk 0.26.0");
}

#[test]
fn the_tsk_installer_picks_the_highest_tag_as_numbers() {
    let Some(i) = Installer::new() else {
        return;
    };
    i.release("0.26.10");
    i.release("0.9.0");
    let repo = i.dir("tags-numeric");
    for args in [
        vec!["init", "-q"],
        vec![
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@e",
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "x",
        ],
        vec!["tag", "v0.26.2"],
        vec!["tag", "v0.26.10"],
        vec!["tag", "v0.9.0"],
    ] {
        assert!(Command::new("git")
            .args(&args)
            .current_dir(&repo)
            .status()
            .unwrap()
            .success());
    }
    let target = i.dir("install");
    i.run(
        &[],
        &[
            ("TSK_INSTALL_DIR", target.display().to_string()),
            ("TSK_RELEASE_REPO_URL", repo.display().to_string()),
        ],
    );
    assert_eq!(i.version_of(&target.join("tsk")), "tsk 0.26.10");
}

#[test]
fn a_failed_install_leaves_the_old_tsk_whole_and_no_staged_file() {
    let Some(i) = Installer::new() else {
        return;
    };
    let dir = i.dir("atomic");
    stub_tsk(&dir.join("tsk"), "0.12.1", "none");
    let before = std::fs::read(dir.join("tsk")).unwrap();

    // A truncated archive: curl succeeds, tar fails.
    let base = i.dir("truncated");
    let archive_dir = base.join("v0.26.0");
    std::fs::create_dir_all(&archive_dir).unwrap();
    let whole = std::fs::read(
        i.releases()
            .join("v0.26.0")
            .join("tsk-0.26.0-x86_64-unknown-linux-gnu.tar.gz"),
    )
    .unwrap();
    for target in TARGETS {
        std::fs::write(
            archive_dir.join(format!("tsk-0.26.0-{}.tar.gz", target)),
            &whole[..whole.len() / 2],
        )
        .unwrap();
    }
    let out = i.run(
        &[&dir],
        &[(
            "TSK_RELEASE_DOWNLOAD_URL",
            format!("file://{}", base.display()),
        )],
    );
    assert!(
        out.starts_with("WARNING: unpacking tsk 0.26.0 failed"),
        "{}",
        out
    );
    assert_eq!(std::fs::read(dir.join("tsk")).unwrap(), before);
    assert_eq!(i.leftovers(&dir), Vec::<String>::new());

    // A curl that writes part of the file and then fails.
    let sys = i.machine.root.path().join("sys");
    let _ = std::fs::remove_file(sys.join("curl"));
    write_script(
        &sys.join("curl"),
        "#!/bin/sh\nwhile [ $# -gt 0 ]; do [ \"$1\" = -o ] && out=\"$2\"; shift; done\n\
         printf 'partial' > \"$out\"; echo 'curl: (18) transfer closed' >&2; exit 18\n",
    );
    let out = i.run(&[&dir], &[]);
    assert!(
        out.starts_with("WARNING: downloading tsk 0.26.0 failed"),
        "{}",
        out
    );
    assert!(out.contains("transfer closed"), "{}", out);
    assert_eq!(std::fs::read(dir.join("tsk")).unwrap(), before);
    assert_eq!(i.leftovers(&dir), Vec::<String>::new());
}

#[test]
fn the_tsk_installer_replaces_the_target_of_a_symbolic_link_and_keeps_the_link() {
    let Some(i) = Installer::new() else {
        return;
    };
    let real = i.dir("real");
    let links = i.dir("links");
    stub_tsk(&real.join("tsk-real"), "0.12.1", "none");
    std::os::unix::fs::symlink("../real/tsk-real", links.join("tsk")).unwrap();
    let out = i.run(&[&links], &[]);
    assert_eq!(out, "");
    assert!(std::fs::symlink_metadata(links.join("tsk"))
        .unwrap()
        .file_type()
        .is_symlink());
    assert_eq!(i.version_of(&real.join("tsk-real")), "tsk 0.26.0");
    assert_eq!(i.version_of(&links.join("tsk")), "tsk 0.26.0");
}

#[test]
fn the_tsk_installer_handles_paths_with_spaces_and_an_unset_home() {
    let Some(i) = Installer::new() else {
        return;
    };
    let spaced = i.dir("with space/bin dir");
    let out = i.run(
        &[],
        &[
            ("TSK_INSTALL_DIR", spaced.display().to_string()),
            ("-HOME", String::new()),
            ("-CLAUDE_ENV_FILE", String::new()),
        ],
    );
    assert_eq!(out, "");
    assert_eq!(i.version_of(&spaced.join("tsk")), "tsk 0.26.0");

    // A stale tsk in a directory with spaces is replaced in place.
    let old = i.dir("old dir/bin");
    stub_tsk(&old.join("tsk"), "0.12.1", "none");
    let out = i.run(&[&old], &[("-HOME", String::new())]);
    assert_eq!(out, "");
    assert_eq!(i.version_of(&old.join("tsk")), "tsk 0.26.0");
}

#[test]
fn two_tsk_installers_at_once_leave_one_whole_tsk() {
    let Some(i) = Installer::new() else {
        return;
    };
    let dir = i.dir("race");
    stub_tsk(&dir.join("tsk"), "0.12.1", "none");
    let script = i
        .machine
        .repo()
        .join("plugin")
        .join("hooks")
        .join("install-tsk.sh");
    let spawn = || {
        let mut command = i.machine.command(&script);
        command
            .env("PATH", i.path(&[&dir]))
            .env(
                "TSK_RELEASE_DOWNLOAD_URL",
                format!("file://{}", i.releases().display()),
            )
            .env(
                "TSK_RELEASE_REPO_URL",
                i.machine.root.path().join("no-repo"),
            )
            .env("SU_LOG", i.machine.log("selfupdate"))
            .stdout(Stdio::piped());
        command.spawn().unwrap()
    };
    let (a, b) = (spawn(), spawn());
    for child in [a, b] {
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success());
        assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    }
    assert_eq!(i.version_of(&dir.join("tsk")), "tsk 0.26.0");
    assert_eq!(i.leftovers(&dir), Vec::<String>::new());
}

#[test]
fn the_tsk_installer_warns_when_it_cannot_write_the_directory() {
    let Some(i) = Installer::new() else {
        return;
    };
    // A file where the directory should be: mkdir and the staged file both fail, whoever
    // runs the test.
    let blocker = i.machine.root.path().join("blocker");
    std::fs::write(&blocker, "x").unwrap();
    let out = i.run(
        &[],
        &[("TSK_INSTALL_DIR", blocker.join("bin").display().to_string())],
    );
    assert!(out.starts_with("WARNING: installing tsk into"), "{}", out);
}

/// The code block under "## 3. Add the setup script" in the cloud environment guide.
fn guide_setup_script() -> String {
    let guide = std::fs::read_to_string(
        workspace()
            .join("docs")
            .join("user-guide")
            .join("new-cloud-environment.md"),
    )
    .unwrap();
    let section = guide
        .split("## 3. Add the setup script")
        .nth(1)
        .expect("the guide has a step 3");
    let start = section.find("```bash\n").expect("step 3 has a bash block") + "```bash\n".len();
    let end = start + section[start..].find("\n```").expect("the block ends");
    format!("{}\n", &section[start..end])
}

#[test]
fn the_cloud_environment_guide_fetches_the_setup_script_of_ops_cloud_from_main() {
    let script = guide_setup_script();
    assert!(
        script.contains(
            "curl -sSfL https://raw.githubusercontent.com/jimbarritt/tsk/main/ops/cloud/setup-environment.sh | bash"
        ),
        "docs/user-guide/new-cloud-environment.md step 3 does not fetch ops/cloud/setup-environment.sh from main:\n{}",
        script
    );
    assert!(
        script.contains("set -euo pipefail"),
        "a failed fetch must fail the build"
    );
    assert!(workspace()
        .join("ops")
        .join("cloud")
        .join("setup-environment.sh")
        .is_file());
}

#[test]
fn the_setup_script_needs_the_dolt_version_that_the_dolt_installer_pins() {
    let script = std::fs::read_to_string(
        workspace()
            .join("ops")
            .join("cloud")
            .join("setup-environment.sh"),
    )
    .unwrap();
    let installer = std::fs::read_to_string(
        workspace()
            .join("plugin")
            .join("hooks")
            .join("install-dolt.sh"),
    )
    .unwrap();
    let pinned = |text: &str| {
        text.lines()
            .find_map(|line| line.strip_prefix("DOLT_VERSION="))
            .map(|v| v.trim_matches('"').to_string())
            .expect("DOLT_VERSION is set")
    };
    assert_eq!(pinned(&script), pinned(&installer));
}

#[test]
fn the_plugin_registers_the_pre_tool_use_guard_for_claude() {
    let claude: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(workspace().join("plugin/hooks/hooks.json")).unwrap(),
    )
    .unwrap();
    let entry = &claude["hooks"]["PreToolUse"][0];
    let matcher = entry["matcher"].as_str().unwrap();
    for tool in ["Bash", "Edit", "Write", "NotebookEdit"] {
        assert!(matcher.split('|').any(|t| t == tool), "{}", matcher);
    }
    let command = entry["hooks"][0]["command"].as_str().unwrap();
    assert!(command.ends_with("hooks/pre-tool-use.sh\""), "{}", command);
    let script = std::fs::read_to_string(workspace().join("plugin/hooks/pre-tool-use.sh")).unwrap();
    assert!(script.contains("tsk ledger guard"), "{}", script);
}

/// Runs `tsk ledger guard` with a hook input and returns its stdout and exit status.
fn run_guard(input: &str, envs: &[(&str, &str)]) -> (bool, String) {
    let state = tempfile::tempdir().unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_tsk"));
    command
        .args(["ledger", "guard"])
        .env("XDG_STATE_HOME", state.path())
        .env("HOME", state.path())
        .env_remove("TSK_LEDGER_GUARD")
        .env_remove("TSK_LEDGER_WT")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in envs {
        command.env(key, value);
    }
    let mut child = command.spawn().unwrap();
    {
        use std::io::Write;
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
    }
    let output = child.wait_with_output().unwrap();
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
    )
}

#[test]
fn the_guard_denies_git_against_a_ledger_ref_and_allows_tsk() {
    let denied = "{\"tool_name\":\"Bash\",\"tool_input\":{\"command\":\"git fetch origin refs/heads/tsk/ledger\"},\"cwd\":\"/work\"}";
    let (ok, out) = run_guard(denied, &[]);
    assert!(ok);
    let value: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(value["hookSpecificOutput"]["hookEventName"], "PreToolUse");
    assert_eq!(value["hookSpecificOutput"]["permissionDecision"], "deny");
    assert!(value["hookSpecificOutput"]["permissionDecisionReason"]
        .as_str()
        .unwrap()
        .contains("ADR 0028"));

    let allowed = "{\"tool_name\":\"Bash\",\"tool_input\":{\"command\":\"tsk ledger push \\\"Claim M-X T-01\\\"\"},\"cwd\":\"/work\"}";
    assert_eq!(run_guard(allowed, &[]), (true, String::new()));
}

#[test]
fn the_guard_denies_a_write_in_the_ledger_store_and_the_switch_allows_it() {
    let state = tempfile::tempdir().unwrap();
    let path = state.path().join("tsk/repos/x/docs/a.md");
    let input = serde_json::json!({
        "tool_name": "Write",
        "tool_input": {"file_path": path},
        "cwd": "/work",
    })
    .to_string();
    let envs = [("XDG_STATE_HOME", state.path().to_str().unwrap())];
    let (ok, out) = run_guard(&input, &envs);
    assert!(ok);
    assert!(out.contains("\"deny\""), "{}", out);
    let mut off = envs.to_vec();
    off.push(("TSK_LEDGER_GUARD", "off"));
    assert_eq!(run_guard(&input, &off), (true, String::new()));
}

#[test]
fn the_guard_allows_a_malformed_input() {
    assert_eq!(run_guard("not json", &[]), (true, String::new()));
}

/// The PreToolUse script with a stand-in `tsk`.
fn pre_tool_use_hook(tsk_script: Option<&str>) -> (Option<i32>, String) {
    let root = tempfile::tempdir().unwrap();
    let bin = root.path().join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    if let Some(text) = tsk_script {
        write_script(&bin.join("tsk"), text);
    }
    let script = workspace().join("plugin/hooks/pre-tool-use.sh");
    let output = Command::new("/bin/bash")
        .arg(script)
        .env("PATH", bin.display().to_string())
        .stdin(Stdio::null())
        .output()
        .unwrap();
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
    )
}

#[test]
fn the_pre_tool_use_script_prints_the_decision_and_exits_0() {
    let (code, out) = pre_tool_use_hook(Some("#!/bin/sh\necho '{\"deny\":true}'\nexit 0\n"));
    assert_eq!(code, Some(0));
    assert_eq!(out, "{\"deny\":true}\n");
}

#[test]
fn the_pre_tool_use_script_fails_open() {
    // No tsk, and a tsk that does not know the command: a non-zero exit would block the call.
    assert_eq!(pre_tool_use_hook(None), (Some(0), String::new()));
    let (code, out) = pre_tool_use_hook(Some(
        "#!/bin/sh\necho 'unrecognized subcommand' >&2\nexit 2\n",
    ));
    assert_eq!((code, out), (Some(0), String::new()));
}
