//! The git protocol version that tsk sets for its git processes, and for the git
//! processes that `dolt` starts. A `git` first on `PATH` logs the config environment of
//! every launch, then runs the real one. Cases return when `dolt` is not installed.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use tempfile::TempDir;

use crate::common;

use common::dolt_installed;

fn git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@example.com")
        .output()
        .expect("failed to run git");
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout)
        .trim_end()
        .to_string()
}

struct Fixture {
    root: TempDir,
}

/// What the logging `git` wrote for one launch.
#[derive(Debug)]
struct Launch {
    count: String,
    entries: Vec<(String, String)>,
    args: String,
}

impl Launch {
    fn is_network(&self) -> bool {
        let verb = self
            .args
            .split_whitespace()
            .find(|word| ["fetch", "push", "ls-remote", "clone", "pull"].contains(word));
        verb.is_some()
    }

    fn sets(&self, key: &str, value: &str) -> bool {
        self.entries.iter().any(|(k, v)| k == key && v == value)
    }
}

impl Fixture {
    fn new() -> Fixture {
        let fx = Fixture {
            root: tempfile::tempdir().unwrap(),
        };
        let root = fx.root.path();
        std::fs::create_dir_all(root.join("home")).unwrap();
        common::seed_dolt_home(&root.join("home"));
        std::fs::create_dir_all(root.join("state")).unwrap();
        git(
            root,
            &["init", "--quiet", "--bare", "-b", "main", "origin.git"],
        );
        git(root, &["init", "--quiet", "-b", "main", "seed"]);
        let seed = root.join("seed");
        git(
            &seed,
            &["remote", "add", "origin", fx.origin().to_str().unwrap()],
        );
        std::fs::write(seed.join("README.md"), "main\n").unwrap();
        git(&seed, &["add", "-A"]);
        git(&seed, &["commit", "--quiet", "-m", "Main commit"]);
        git(
            &seed,
            &["push", "--quiet", "origin", "HEAD:refs/heads/main"],
        );
        for name in ["work", "work2"] {
            git(
                root,
                &["clone", "--quiet", fx.origin().to_str().unwrap(), name],
            );
        }
        fx.install_logging_git();
        fx
    }

    fn origin(&self) -> PathBuf {
        self.root.path().join("origin.git")
    }

    fn log(&self) -> PathBuf {
        self.root.path().join("launches.log")
    }

    /// A `git` that logs the config environment and the arguments of each launch.
    fn install_logging_git(&self) {
        let real = String::from_utf8(
            Command::new("sh")
                .args(["-c", "command -v git"])
                .output()
                .unwrap()
                .stdout,
        )
        .unwrap();
        let bin = self.root.path().join("wrapbin");
        std::fs::create_dir_all(&bin).unwrap();
        let script = bin.join("git");
        std::fs::write(
            &script,
            format!(
                "#!/bin/sh\nprintf 'count=%s;k0=%s;v0=%s;k1=%s;v1=%s;args=%s\\n' \"$GIT_CONFIG_COUNT\" \"$GIT_CONFIG_KEY_0\" \"$GIT_CONFIG_VALUE_0\" \"$GIT_CONFIG_KEY_1\" \"$GIT_CONFIG_VALUE_1\" \"$*\" >> '{}'\nexec '{}' \"$@\"\n",
                self.log().display(),
                real.trim()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    fn command(&self, dir: &Path, args: &[&str]) -> Command {
        let path = format!(
            "{}:{}",
            self.root.path().join("wrapbin").display(),
            std::env::var("PATH").unwrap()
        );
        let mut command = Command::new(env!("CARGO_BIN_EXE_tsk"));
        command
            .args(args)
            .current_dir(dir)
            .env("PATH", path)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "test@example.com")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "test@example.com")
            .env("HOME", self.root.path().join("home"))
            .env("XDG_STATE_HOME", self.root.path().join("state"))
            .env("TSK_DOLT_SERVER_IDLE_SECS", "3")
            .env(
                "XDG_CONFIG_HOME",
                self.root.path().join("home").join("config"),
            )
            .env("TSK_HOME", self.root.path().join("home").join(".tsk"))
            .env("TSK_NO_UPDATE_CHECK", "1")
            .env("TSK_LEDGER_REFS", "branches")
            .env("TSK_MACHINE_NAME", "laptop")
            .env("GIT_CEILING_DIRECTORIES", self.root.path())
            .env_remove("GIT_CONFIG_COUNT")
            .env_remove("TSK_GIT_PROTOCOL")
            .env_remove("CLAUDE_CODE_REMOTE_SESSION_ID");
        command
    }

    fn ok(&self, command: &mut Command) -> String {
        let output: Output = command.output().expect("failed to run tsk");
        assert!(
            output.status.success(),
            "tsk failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).into_owned()
    }

    /// Fetch and push from the first clone, then fetch and read from the second.
    /// Returns what the second clone reads.
    fn round_trip(&self, tweak: &dyn Fn(&mut Command)) -> String {
        let (a, b) = (
            self.root.path().join("work"),
            self.root.path().join("work2"),
        );
        let mut fetch = self.command(&a, &["ledger", "fetch"]);
        tweak(&mut fetch);
        self.ok(&mut fetch);
        let mut put = self.command(&a, &["ledger", "doc", "put", "notes/n.md"]);
        tweak(&mut put);
        let mut child = put
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        std::io::Write::write_all(&mut child.stdin.take().unwrap(), b"# Note\n").unwrap();
        assert!(child.wait_with_output().unwrap().status.success());
        let mut push = self.command(&a, &["ledger", "push", "Add a note"]);
        tweak(&mut push);
        self.ok(&mut push);
        let mut fetch_b = self.command(&b, &["ledger", "fetch"]);
        tweak(&mut fetch_b);
        self.ok(&mut fetch_b);
        let mut get = self.command(&b, &["ledger", "doc", "get", "notes/n.md"]);
        tweak(&mut get);
        self.ok(&mut get)
    }

    fn launches(&self) -> Vec<Launch> {
        let text = std::fs::read_to_string(self.log()).unwrap_or_default();
        text.lines()
            .map(|line| {
                let field = |name: &str| {
                    line.split(';')
                        .find_map(|part| part.strip_prefix(name)?.strip_prefix('='))
                        .unwrap_or("")
                        .to_string()
                };
                let args = line
                    .split_once(";args=")
                    .map(|(_, rest)| rest.to_string())
                    .unwrap_or_default();
                Launch {
                    count: field("count"),
                    entries: vec![(field("k0"), field("v0")), (field("k1"), field("v1"))],
                    args,
                }
            })
            .collect()
    }
}

#[test]
fn every_git_launch_of_a_fetch_and_a_push_carries_protocol_version_0() {
    if !dolt_installed() {
        eprintln!("dolt is not installed; skipping");
        return;
    }
    let fx = Fixture::new();
    // The fetch, the push and the read all work with the setting.
    assert_eq!(fx.round_trip(&|_| {}), "# Note\n");
    let launches = fx.launches();
    let network: Vec<&Launch> = launches.iter().filter(|l| l.is_network()).collect();
    assert!(
        network.len() >= 3,
        "expected network launches, got {:?}",
        launches
    );
    for launch in &launches {
        assert_eq!(launch.count, "1", "{:?}", launch);
        assert!(launch.sets("protocol.version", "0"), "{:?}", launch);
    }
}

#[test]
fn the_entry_goes_after_the_users_git_config_entries() {
    if !dolt_installed() {
        eprintln!("dolt is not installed; skipping");
        return;
    }
    let fx = Fixture::new();
    let tweak = |command: &mut Command| {
        command
            .env("GIT_CONFIG_COUNT", "1")
            .env("GIT_CONFIG_KEY_0", "core.abbrev")
            .env("GIT_CONFIG_VALUE_0", "12");
    };
    assert_eq!(fx.round_trip(&tweak), "# Note\n");
    let launches = fx.launches();
    assert!(!launches.is_empty());
    for launch in &launches {
        assert_eq!(launch.count, "2", "{:?}", launch);
        assert_eq!(
            launch.entries[0],
            ("core.abbrev".to_string(), "12".to_string()),
            "{:?}",
            launch
        );
        assert_eq!(
            launch.entries[1],
            ("protocol.version".to_string(), "0".to_string()),
            "{:?}",
            launch
        );
    }
}

#[test]
fn tsk_git_protocol_default_leaves_the_setting_out() {
    if !dolt_installed() {
        eprintln!("dolt is not installed; skipping");
        return;
    }
    let fx = Fixture::new();
    let tweak = |command: &mut Command| {
        command.env("TSK_GIT_PROTOCOL", "default");
    };
    assert_eq!(fx.round_trip(&tweak), "# Note\n");
    let launches = fx.launches();
    assert!(!launches.is_empty());
    for launch in &launches {
        assert_eq!(launch.count, "", "{:?}", launch);
        assert!(!launch.sets("protocol.version", "0"), "{:?}", launch);
    }
}
