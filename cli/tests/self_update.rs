use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

struct Fixture {
    root: tempfile::TempDir,
}

fn target() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => "aarch64-apple-darwin",
        ("macos", "x86_64") => "x86_64-apple-darwin",
        ("linux", "x86_64") => "x86_64-unknown-linux-gnu",
        ("linux", "aarch64") => "aarch64-unknown-linux-gnu",
        other => panic!("unsupported test platform {:?}", other),
    }
}

fn run(dir: &Path, program: &str, args: &[&str]) {
    let output = Command::new(program)
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@example.com")
        .output_retry();
    assert!(
        output.status.success(),
        "{} {:?}: {}",
        program,
        args,
        String::from_utf8_lossy(&output.stderr)
    );
}

impl Fixture {
    /// A bare repo tagged with `tags`, and a release directory for each of `releases`
    /// whose `tsk` is a shell script printing `tsk <version>`.
    fn new(tags: &[&str], releases: &[&str]) -> Fixture {
        let root = tempfile::tempdir().unwrap();
        let fx = Fixture { root };
        let seed = fx.root.path().join("seed");
        std::fs::create_dir_all(&seed).unwrap();
        run(&seed, "git", &["init", "-q", "-b", "main"]);
        std::fs::write(seed.join("f"), "x").unwrap();
        run(&seed, "git", &["add", "f"]);
        run(&seed, "git", &["commit", "-q", "-m", "seed"]);
        for tag in tags {
            run(&seed, "git", &["tag", tag]);
        }
        run(&seed, "git", &["tag", "vnext"]);
        run(
            fx.root.path(),
            "git",
            &["clone", "-q", "--bare", "seed", "origin.git"],
        );
        for version in releases {
            let staging = fx.root.path().join("staging");
            std::fs::create_dir_all(&staging).unwrap();
            let script = staging.join("tsk");
            std::fs::write(&script, format!("#!/bin/sh\necho \"tsk {}\"\n", version)).unwrap();
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
            let dir = fx
                .root
                .path()
                .join("releases")
                .join(format!("v{}", version));
            std::fs::create_dir_all(&dir).unwrap();
            let archive = dir.join(format!("tsk-{}-{}.tar.gz", version, target()));
            run(
                &staging,
                "tar",
                &["-czf", &archive.to_string_lossy(), "tsk"],
            );
            std::fs::remove_dir_all(&staging).unwrap();
        }
        fx
    }

    fn origin(&self) -> PathBuf {
        self.root.path().join("origin.git")
    }

    fn releases(&self) -> String {
        format!("file://{}", self.root.path().join("releases").display())
    }

    /// A copy of the built binary in its own directory, so it counts as a release binary.
    fn installed(&self) -> PathBuf {
        let dir = self.root.path().join("bin");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("tsk");
        std::fs::copy(env!("CARGO_BIN_EXE_tsk"), &path).unwrap();
        path
    }

    fn command(&self, bin: &Path, args: &[&str]) -> Command {
        let home = self.root.path().join("home");
        std::fs::create_dir_all(&home).unwrap();
        let mut command = Command::new(bin);
        command
            .args(args)
            .current_dir(self.root.path())
            .env("HOME", &home)
            .env("XDG_STATE_HOME", home.join("state"))
            .env("XDG_CONFIG_HOME", home.join("config"))
            .env("TSK_HOME", home.join(".tsk"))
            .env("GIT_CEILING_DIRECTORIES", self.root.path())
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("CARGO_HOME", home.join("cargo"))
            .env("TSK_RELEASE_REPO_URL", self.origin())
            .env("TSK_RELEASE_DOWNLOAD_URL", self.releases())
            .env_remove("TSK_NO_UPDATE_CHECK")
            .env_remove("CLAUDE_CODE_REMOTE")
            .env_remove("CLAUDE_CODE_REMOTE_SESSION_ID")
            .env_remove("CLAUDE_ENV_FILE");
        command
    }

    fn tsk(&self, bin: &Path, args: &[&str]) -> Output {
        self.command(bin, args).stdin(Stdio::null()).output_retry()
    }
}

/// Runs a command to completion. Linux refuses to execute a file that is open for
/// writing, and a fork on another test thread can hold the write handle of a file this
/// test has just copied or written. The refusal (ETXTBSY) is brief, so retry on it.
trait OutputRetry {
    fn output_retry(&mut self) -> Output;
}

impl OutputRetry for Command {
    fn output_retry(&mut self) -> Output {
        for _ in 0..200 {
            match self.output() {
                Err(e) if e.kind() == std::io::ErrorKind::ExecutableFileBusy => {
                    std::thread::sleep(std::time::Duration::from_millis(25));
                }
                result => return result.unwrap(),
            }
        }
        self.output().unwrap()
    }
}

fn out(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn err(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn version_of(bin: &Path) -> String {
    let output = Command::new(bin).output_retry();
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

#[test]
fn self_update_replaces_a_release_binary_with_the_latest_release() {
    let fx = Fixture::new(&["v0.1.0", "v99.0.0"], &["99.0.0"]);
    let bin = fx.installed();
    let output = fx.tsk(&bin, &["self-update"]);
    assert!(output.status.success(), "{}", err(&output));
    assert!(
        out(&output).contains(&format!(
            "updated tsk {} -> 99.0.0",
            env!("CARGO_PKG_VERSION")
        )),
        "{}",
        out(&output)
    );
    assert!(out(&output).contains("release binary"), "{}", out(&output));
    assert_eq!(version_of(&bin), "tsk 99.0.0");
    let mode = std::fs::metadata(&bin).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o755);
    let leftovers: Vec<_> = std::fs::read_dir(bin.parent().unwrap())
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(leftovers, vec!["tsk".to_string()]);
}

/// A copy of the built binary in the cargo bin directory of the fixture, and a `cargo` on
/// the path that appends its arguments to a log and exits with `code`.
fn cargo_installed(fx: &Fixture, code: i32) -> (PathBuf, PathBuf, PathBuf) {
    let bin_dir = fx.root.path().join("home").join("cargo").join("bin");
    std::fs::create_dir_all(&bin_dir).unwrap();
    let bin = bin_dir.join("tsk");
    std::fs::copy(env!("CARGO_BIN_EXE_tsk"), &bin).unwrap();
    let fake = fx.root.path().join("fake");
    std::fs::create_dir_all(&fake).unwrap();
    let log = fx.root.path().join("cargo.log");
    let script = fake.join("cargo");
    std::fs::write(
        &script,
        format!(
            "#!/bin/sh\necho \"$@\" >> '{}'\nexit {}\n",
            log.display(),
            code
        ),
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    (bin, fake, log)
}

fn with_path(mut command: Command, first: &Path) -> Command {
    let path = format!(
        "{}:{}",
        first.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    command.env("PATH", path);
    command
}

#[test]
fn self_update_takes_the_release_archive_for_a_binary_under_the_cargo_bin_directory() {
    let fx = Fixture::new(&["v0.1.0", "v99.0.0"], &["99.0.0"]);
    let (bin, fake, log) = cargo_installed(&fx, 1);
    let output = with_path(fx.command(&bin, &["self-update"]), &fake)
        .stdin(Stdio::null())
        .output_retry();
    assert!(output.status.success(), "{}", err(&output));
    assert!(
        out(&output).contains("-> 99.0.0 (release binary)"),
        "{}",
        out(&output)
    );
    assert_eq!(version_of(&bin), "tsk 99.0.0");
    assert!(
        !log.exists(),
        "cargo ran: {}",
        std::fs::read_to_string(&log).unwrap()
    );
}

#[test]
fn self_update_falls_back_to_cargo_install_when_the_archive_cannot_be_installed() {
    let fx = Fixture::new(&["v99.0.0"], &[]);
    let (bin, fake, log) = cargo_installed(&fx, 0);
    let output = with_path(fx.command(&bin, &["self-update"]), &fake)
        .stdin(Stdio::null())
        .output_retry();
    assert!(output.status.success(), "{}", err(&output));
    assert!(
        out(&output).contains("-> 99.0.0 (Cargo)"),
        "{}",
        out(&output)
    );
    assert!(
        err(&output).contains("falling back to cargo install"),
        "{}",
        err(&output)
    );
    assert_eq!(
        std::fs::read_to_string(&log).unwrap().trim(),
        "install tsk-bin --locked --version 99.0.0"
    );
}

#[test]
fn self_update_reports_a_failed_download_and_leaves_the_binary() {
    let fx = Fixture::new(&["v99.0.0"], &[]);
    let bin = fx.installed();
    let before = std::fs::read(&bin).unwrap();
    let output = fx.tsk(&bin, &["self-update"]);
    assert!(!output.status.success());
    assert!(err(&output).contains("curl"), "{}", err(&output));
    assert_eq!(std::fs::read(&bin).unwrap(), before);
}

#[test]
fn check_reports_a_newer_release_and_installs_nothing() {
    let fx = Fixture::new(&["v0.1.0", "v99.0.0"], &["99.0.0"]);
    let bin = fx.installed();
    let output = fx.tsk(&bin, &["self-update", "--check"]);
    assert!(output.status.success(), "{}", err(&output));
    assert_eq!(
        out(&output).trim(),
        format!(
            "tsk 99.0.0 is available (running {}); run tsk self-update",
            env!("CARGO_PKG_VERSION")
        )
    );
    assert_ne!(version_of(&bin), "tsk 99.0.0");
}

#[test]
fn the_latest_release_case_prints_one_line_and_exits_0() {
    let fx = Fixture::new(&["v0.1.0"], &[]);
    let bin = fx.installed();
    for args in [&["self-update"][..], &["self-update", "--check"][..]] {
        let output = fx.tsk(&bin, args);
        assert!(output.status.success(), "{}", err(&output));
        assert_eq!(
            out(&output).trim(),
            format!("tsk {} is the latest release", env!("CARGO_PKG_VERSION"))
        );
    }
}

#[test]
fn a_repo_with_no_release_tag_is_an_error() {
    let fx = Fixture::new(&[], &[]);
    let output = fx.tsk(&fx.installed(), &["self-update", "--check"]);
    assert!(!output.status.success());
    assert!(err(&output).contains("no release tag"), "{}", err(&output));
}

#[test]
fn session_start_names_a_newer_release() {
    let fx = Fixture::new(&["v0.1.0", "v99.0.0"], &[]);
    let bin = fx.installed();
    let output = fx.tsk(&bin, &["thread", "session-start"]);
    assert!(output.status.success(), "{}", err(&output));
    let notice = format!(
        "tsk 99.0.0 is available (running {}). Run `tsk self-update` to upgrade.",
        env!("CARGO_PKG_VERSION")
    );
    let json: serde_json::Value = serde_json::from_str(out(&output).trim()).unwrap();
    let context = json["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap();
    assert!(context.ends_with(&notice), "{}", context);

    // The second start in the day answers from the cache and says so.
    let copilot = fx.tsk(&bin, &["thread", "session-start", "--format", "copilot"]);
    let json: serde_json::Value = serde_json::from_str(out(&copilot).trim()).unwrap();
    let context = json["additionalContext"].as_str().unwrap();
    assert!(context.contains(&notice), "{}", context);
    assert!(
        context.ends_with("Set TSK_UPDATE_CHECK=always to check now."),
        "{}",
        context
    );
    assert!(
        context.contains("The release check is cached"),
        "{}",
        context
    );
}

/// A `git` in front of the real one on `PATH` that appends each launch to a log.
fn counting_git(fx: &Fixture) -> (PathBuf, PathBuf) {
    let dir = fx.root.path().join("wrapbin");
    std::fs::create_dir_all(&dir).unwrap();
    let log = fx.root.path().join("git-launches.log");
    let real = String::from_utf8(
        Command::new("sh")
            .args(["-c", "command -v git"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    let script = dir.join("git");
    std::fs::write(
        &script,
        format!(
            "#!/bin/sh\necho \"$*\" >> '{}'\nexec '{}' \"$@\"\n",
            log.display(),
            real.trim()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    (dir, log)
}

fn launches(log: &Path, needle: &str) -> usize {
    std::fs::read_to_string(log)
        .unwrap_or_default()
        .lines()
        .filter(|line| line.contains(needle))
        .count()
}

fn start_counting(fx: &Fixture, bin: &Path, dir: &Path, mode: Option<&str>) -> Output {
    let path = format!("{}:{}", dir.display(), std::env::var("PATH").unwrap());
    let mut command = fx.command(bin, &["thread", "session-start"]);
    command.env("PATH", path).stdin(Stdio::null());
    if let Some(mode) = mode {
        command.env("TSK_UPDATE_CHECK", mode);
    }
    command.output_retry()
}

#[test]
fn session_start_looks_up_releases_once_a_day_and_the_modes_override() {
    let fx = Fixture::new(&["v0.1.0", "v99.0.0"], &[]);
    let bin = fx.installed();
    let (wrap, log) = counting_git(&fx);

    start_counting(&fx, &bin, &wrap, None);
    assert_eq!(launches(&log, "ls-remote --tags"), 1);
    let second = start_counting(&fx, &bin, &wrap, None);
    assert_eq!(launches(&log, "ls-remote --tags"), 1, "{}", err(&second));
    assert!(out(&second).contains("is available"), "{}", out(&second));
    assert!(
        out(&second).contains("The release check is cached"),
        "{}",
        out(&second)
    );

    start_counting(&fx, &bin, &wrap, Some("always"));
    assert_eq!(launches(&log, "ls-remote --tags"), 2);
    start_counting(&fx, &bin, &wrap, Some("never"));
    assert_eq!(launches(&log, "ls-remote --tags"), 2);

    // A check older than a day goes to the network again.
    let cache = fx.root.path().join("home/state/tsk/update-check");
    let text = std::fs::read_to_string(&cache).unwrap();
    let old = text
        .lines()
        .map(|line| match line.strip_prefix("checked_at=") {
            Some(_) => "checked_at=1".to_string(),
            None => line.to_string(),
        })
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(&cache, old).unwrap();
    start_counting(&fx, &bin, &wrap, None);
    assert_eq!(launches(&log, "ls-remote --tags"), 3);

    // The explicit command always asks the network.
    let mut update = fx.command(&bin, &["self-update", "--check"]);
    let path = format!("{}:{}", wrap.display(), std::env::var("PATH").unwrap());
    update.env("PATH", path).stdin(Stdio::null());
    update.output_retry();
    assert_eq!(launches(&log, "ls-remote --tags"), 4);
}

#[test]
fn session_start_is_silent_when_current_switched_off_or_unreachable() {
    let fx = Fixture::new(&["v0.1.0", "v99.0.0"], &[]);
    let bin = fx.installed();
    let off = fx
        .command(&bin, &["thread", "session-start"])
        .env("TSK_NO_UPDATE_CHECK", "1")
        .stdin(Stdio::null())
        .output_retry();
    assert!(!out(&off).contains("self-update"), "{}", out(&off));

    let missing = fx
        .command(&bin, &["thread", "session-start"])
        .env("TSK_RELEASE_REPO_URL", fx.root.path().join("nowhere.git"))
        .stdin(Stdio::null())
        .output_retry();
    assert!(missing.status.success());
    assert!(!out(&missing).contains("self-update"), "{}", out(&missing));

    let current = Fixture::new(&["v0.1.0"], &[]);
    let output = current.tsk(&current.installed(), &["thread", "session-start"]);
    assert!(!out(&output).contains("self-update"), "{}", out(&output));
}

#[test]
fn a_cloud_session_upgrades_before_it_installs_the_plugin() {
    let fx = Fixture::new(&["v99.0.0"], &["99.0.0"]);
    let bin = fx.installed();
    let claude = fx.root.path().join("claude");
    std::fs::write(&claude, "#!/bin/sh\nexit 0\n").unwrap();
    std::fs::set_permissions(&claude, std::fs::Permissions::from_mode(0o755)).unwrap();
    let output = fx
        .command(
            &bin,
            &["thread", "session-start", "--install-plugin", "claude-cli"],
        )
        .env("CLAUDE_CODE_REMOTE", "true")
        .env("TSK_CLAUDE_BIN", &claude)
        .stdin(Stdio::null())
        .output_retry();
    assert!(output.status.success(), "{}", err(&output));
    assert_eq!(version_of(&bin), "tsk 99.0.0");
    assert!(err(&output).contains("updated tsk"), "{}", err(&output));
    let json: serde_json::Value = serde_json::from_str(out(&output).trim()).unwrap();
    let context = json["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap();
    assert!(!context.contains("could not be upgraded"), "{}", context);
    assert!(!context.contains("could not be installed"), "{}", context);
    assert!(!context.contains("is available"), "{}", context);
}

#[test]
fn a_failed_cloud_upgrade_adds_a_warning_and_the_session_still_starts() {
    let fx = Fixture::new(&["v99.0.0"], &[]);
    let bin = fx.installed();
    let claude = fx.root.path().join("claude");
    std::fs::write(&claude, "#!/bin/sh\nexit 0\n").unwrap();
    std::fs::set_permissions(&claude, std::fs::Permissions::from_mode(0o755)).unwrap();
    let output = fx
        .command(
            &bin,
            &["thread", "session-start", "--install-plugin", "claude-cli"],
        )
        .env("CLAUDE_CODE_REMOTE", "true")
        .env("TSK_CLAUDE_BIN", &claude)
        .stdin(Stdio::null())
        .output_retry();
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_str(out(&output).trim()).unwrap();
    let context = json["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap();
    assert!(
        context.contains("Warning: tsk could not be upgraded at session start"),
        "{}",
        context
    );
}
