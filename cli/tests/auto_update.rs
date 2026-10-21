//! A command refused because the ledger layout is newer than this tsk supports updates
//! tsk and runs again (M-TIDY T-10). The update is faked: `TSK_SELF_UPDATE_CMD` names a
//! script that the update step runs instead of the real update, and the binary honours
//! it only with `TSK_TEST_MODE=1`.

use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use tempfile::TempDir;

mod common;

const RUNNING: &str = env!("CARGO_PKG_VERSION");

fn dolt_installed() -> bool {
    Command::new("dolt")
        .arg("version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn run(dir: &Path, program: &str, args: &[&str]) {
    let output = Command::new(program)
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@example.com")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{} {:?}: {}",
        program,
        args,
        String::from_utf8_lossy(&output.stderr)
    );
}

struct Fixture {
    root: TempDir,
}

impl Fixture {
    /// A work clone with a fetched ledger whose manifest says layout 9, and a copy of
    /// the built binary in `bin/` that stands for the installed tsk.
    fn new() -> Fixture {
        let fx = Fixture {
            root: tempfile::tempdir().unwrap(),
        };
        let root = fx.root.path();
        std::fs::create_dir_all(fx.home()).unwrap();
        run(
            root,
            "git",
            &["init", "-q", "--bare", "-b", "main", "origin.git"],
        );
        run(root, "git", &["init", "-q", "-b", "main", "seed"]);
        let seed = root.join("seed");
        run(
            &seed,
            "git",
            &["remote", "add", "origin", fx.origin().to_str().unwrap()],
        );
        std::fs::write(seed.join("README.md"), "main\n").unwrap();
        run(&seed, "git", &["add", "-A"]);
        run(&seed, "git", &["commit", "-q", "-m", "seed"]);
        run(
            &seed,
            "git",
            &["push", "-q", "origin", "HEAD:refs/heads/main"],
        );
        run(
            root,
            "git",
            &["clone", "-q", fx.origin().to_str().unwrap(), "work"],
        );
        std::fs::create_dir_all(root.join("bin")).unwrap();
        std::fs::copy(env!("CARGO_BIN_EXE_tsk"), fx.installed()).unwrap();
        let fetch = fx.tsk(&["ledger", "fetch"], "", &[]);
        assert!(fetch.status.success(), "{}", err(&fetch));
        let path = fx.tsk(&["ledger", "path"], "", &[]);
        // A Dolt server that the fetch started holds the store; stop it before a write.
        common::stop_servers(root, true);
        let sql = Command::new("dolt")
            .args([
                "sql",
                "-q",
                "replace into manifest (k, v) values ('version', '9')",
            ])
            .current_dir(String::from_utf8_lossy(&path.stdout).trim_end())
            .env("HOME", fx.home())
            .env("DOLT_DISABLE_EVENT_FLUSH", "1")
            .output()
            .unwrap();
        assert!(sql.status.success(), "{}", err(&sql));
        fx
    }

    fn home(&self) -> PathBuf {
        self.root.path().join("home")
    }

    fn origin(&self) -> PathBuf {
        self.root.path().join("origin.git")
    }

    fn work(&self) -> PathBuf {
        self.root.path().join("work")
    }

    fn installed(&self) -> PathBuf {
        self.root.path().join("bin").join("tsk")
    }

    fn marker(&self) -> PathBuf {
        self.root.path().join("update-ran")
    }

    /// A script that stands for the update. It records that it ran and replaces the
    /// installed tsk (`$1`) with a script that reports `version`, echoes its arguments,
    /// the loop guard variable and its stdin, and exits 7.
    fn update_script(&self, version: &str) -> PathBuf {
        let script = self.root.path().join("update.sh");
        let body = format!(
            "#!/bin/sh\ntouch \"{marker}\"\ncat > \"$1.new\" <<'EOF'\n#!/bin/sh\n\
             if [ \"$1\" = \"--version\" ]; then echo \"tsk {version}\"; exit 0; fi\n\
             echo \"rerun: $* guard=$TSK_AUTO_UPDATED stdin=$(cat)\"\nexit 7\nEOF\n\
             chmod 755 \"$1.new\"\nmv \"$1.new\" \"$1\"\n",
            marker = self.marker().display(),
            version = version
        );
        std::fs::write(&script, body).unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        script
    }

    /// A script that runs and changes nothing, or fails.
    fn plain_script(&self, exit: i32) -> PathBuf {
        let script = self.root.path().join("plain.sh");
        let body = format!(
            "#!/bin/sh\ntouch \"{}\"\nexit {}\n",
            self.marker().display(),
            exit
        );
        std::fs::write(&script, body).unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        script
    }

    fn tsk(&self, args: &[&str], stdin: &str, env: &[(&str, &Path)]) -> Output {
        self.tsk_with(args, stdin, env, &[])
    }

    fn tsk_with(
        &self,
        args: &[&str],
        stdin: &str,
        paths: &[(&str, &Path)],
        vars: &[(&str, &str)],
    ) -> Output {
        let mut command = Command::new(self.installed());
        command
            .args(args)
            .current_dir(self.work())
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "test@example.com")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "test@example.com")
            .env("HOME", self.home())
            .env("XDG_STATE_HOME", self.home().join("state"))
            .env("XDG_CONFIG_HOME", self.home().join("config"))
            .env("TSK_HOME", self.home().join(".tsk"))
            .env("TSK_LEDGER_REFS", "branches")
            .env("TSK_UPDATE_CHECK", "never")
            .env("GIT_CEILING_DIRECTORIES", self.root.path())
            .env_remove("TSK_NO_UPDATE_CHECK")
            .env_remove("TSK_AUTO_UPDATE")
            .env_remove("TSK_AUTO_UPDATED")
            .env_remove("TSK_SELF_UPDATE_CMD")
            .env_remove("TSK_TEST_MODE")
            .env_remove("CLAUDE_CODE_REMOTE")
            .env_remove("CLAUDE_CODE_REMOTE_SESSION_ID")
            .env_remove("CLAUDE_ENV_FILE")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (k, v) in paths {
            command.env(k, v);
        }
        for (k, v) in vars {
            command.env(k, v);
        }
        let mut child = command.spawn().unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(stdin.as_bytes())
            .unwrap();
        child.wait_with_output().unwrap()
    }

    /// Runs with the update script wired in.
    fn with_update(
        &self,
        args: &[&str],
        stdin: &str,
        script: &Path,
        vars: &[(&str, &str)],
    ) -> Output {
        let mut all = vec![("TSK_TEST_MODE", "1")];
        all.extend_from_slice(vars);
        self.tsk_with(args, stdin, &[("TSK_SELF_UPDATE_CMD", script)], &all)
    }
}

fn out(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn err(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

fn warning() -> String {
    format!(
        "warning: this ledger is at layout 9 and this tsk ({}) supports up to 8. Updating tsk now.",
        RUNNING
    )
}

macro_rules! need_dolt {
    () => {
        if !dolt_installed() {
            eprintln!("note: dolt is not installed; skipping");
            return;
        }
    };
}

#[test]
fn a_newer_ledger_warns_updates_and_runs_the_command_again() {
    need_dolt!();
    let fx = Fixture::new();
    let script = fx.update_script("99.0.0");
    let o = fx.with_update(&["mission", "list", "--porcelain"], "piped\n", &script, &[]);
    assert!(err(&o).contains(&warning()), "{}", err(&o));
    assert!(fx.marker().exists());
    assert_eq!(
        out(&o),
        "rerun: mission list --porcelain guard=1 stdin=piped\n"
    );
    assert_eq!(o.status.code(), Some(7), "{}", err(&o));
    assert!(
        !err(&o).contains("unsupported ledger version"),
        "{}",
        err(&o)
    );
}

#[test]
fn a_tsk_that_still_does_not_support_the_layout_stops_with_the_guard_line() {
    need_dolt!();
    let fx = Fixture::new();
    let script = fx.update_script("99.0.0");
    let o = fx.with_update(
        &["mission", "list", "--porcelain"],
        "",
        &script,
        &[("TSK_AUTO_UPDATED", "1")],
    );
    assert_eq!(o.status.code(), Some(1));
    let e = err(&o);
    assert!(e.contains("error: unsupported ledger version 9"), "{}", e);
    assert!(
        e.trim_end().ends_with(
            "tsk was updated but still does not support layout 9; update it by hand: tsk self-update"
        ),
        "{}",
        e
    );
    assert!(!e.contains("Updating tsk now"), "{}", e);
    assert!(!fx.marker().exists());
}

#[test]
fn the_hook_commands_never_update() {
    need_dolt!();
    let fx = Fixture::new();
    let script = fx.update_script("99.0.0");
    for args in [&["thread", "session-start"][..], &["thread", "guard"][..]] {
        let o = fx.with_update(args, "{}", &script, &[]);
        assert!(
            !err(&o).contains("Updating tsk now"),
            "{:?}: {}",
            args,
            err(&o)
        );
        assert!(!out(&o).contains("rerun:"), "{:?}: {}", args, out(&o));
        assert!(!fx.marker().exists(), "{:?}", args);
    }
}

#[test]
fn the_opt_outs_keep_the_error_and_run_no_update() {
    need_dolt!();
    let fx = Fixture::new();
    let script = fx.update_script("99.0.0");
    for var in [("TSK_AUTO_UPDATE", "never"), ("TSK_NO_UPDATE_CHECK", "1")] {
        let o = fx.with_update(&["mission", "list", "--porcelain"], "", &script, &[var]);
        assert_eq!(o.status.code(), Some(1), "{:?}", var);
        assert!(
            err(&o).contains("error: unsupported ledger version 9"),
            "{:?}: {}",
            var,
            err(&o)
        );
        assert!(!err(&o).contains("Updating tsk now"), "{}", err(&o));
        assert!(!err(&o).contains("update it by hand"), "{}", err(&o));
        assert!(!fx.marker().exists(), "{:?}", var);
    }
}

#[test]
fn the_update_script_is_ignored_without_test_mode() {
    need_dolt!();
    let fx = Fixture::new();
    let script = fx.update_script("99.0.0");
    // The real update looks up releases at an origin with no release tag, so it fails.
    let o = fx.tsk_with(
        &["mission", "list", "--porcelain"],
        "",
        &[("TSK_SELF_UPDATE_CMD", &script)],
        &[("TSK_RELEASE_REPO_URL", fx.origin().to_str().unwrap())],
    );
    assert_eq!(o.status.code(), Some(1));
    assert!(!fx.marker().exists());
    assert!(err(&o).contains("has no release tag"), "{}", err(&o));
    assert!(
        err(&o).contains("unsupported ledger version 9"),
        "{}",
        err(&o)
    );
}

#[test]
fn a_failed_update_prints_its_error_then_the_original_error() {
    need_dolt!();
    let fx = Fixture::new();
    let script = fx.plain_script(3);
    let o = fx.with_update(&["mission", "list", "--porcelain"], "", &script, &[]);
    assert_eq!(o.status.code(), Some(1));
    let e = err(&o);
    let at = |needle: &str| {
        e.find(needle)
            .unwrap_or_else(|| panic!("{}: {}", needle, e))
    };
    assert!(at(&warning()) < at("the update script failed"));
    assert!(at("the update script failed") < at("error: unsupported ledger version 9"));
    assert!(fx.marker().exists());
}

#[test]
fn an_update_that_does_not_raise_the_version_is_an_error() {
    need_dolt!();
    let fx = Fixture::new();
    let script = fx.plain_script(0);
    let o = fx.with_update(&["mission", "list", "--porcelain"], "", &script, &[]);
    assert_eq!(o.status.code(), Some(1));
    let e = err(&o);
    assert!(e.contains("not newer than"), "{}", e);
    assert!(e.contains("error: unsupported ledger version 9"), "{}", e);
}

#[test]
fn an_error_that_is_not_the_unsupported_layout_never_updates() {
    need_dolt!();
    let fx = Fixture::new();
    let script = fx.update_script("99.0.0");
    // The removed files backend is an error of another kind, with the same exit code.
    let o = fx.with_update(
        &["mission", "list", "--porcelain"],
        "",
        &script,
        &[("TSK_LEDGER_BACKEND", "git")],
    );
    assert_eq!(o.status.code(), Some(1), "{}", err(&o));
    assert!(!err(&o).contains("Updating tsk now"), "{}", err(&o));
    assert!(!fx.marker().exists());
}
