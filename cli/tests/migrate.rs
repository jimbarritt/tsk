//! `tsk ledger migrate-to-dolt`, end to end against a bare git repo as origin. A file
//! ledger is built with the git backend, migrated, and read back from a fresh clone with
//! the Dolt backend. Cases print a note and return when the `dolt` binary is missing.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use tempfile::TempDir;

const LEDGER_REF: &str = "refs/heads/tsk/ledger";
const ARCHIVE_REF: &str = "refs/heads/archive/tsk-ledger-files";
const BRIEFING: &str = "missions/operational/M-MIG-01.md";
const CLOUD_VAR: &str = "CLAUDE_CODE_REMOTE_SESSION_ID";
const AWKWARD: &str = "no newline at end \"quoted\" 'single' \\ back\\n literal\n\ttab; drop table documents; -- \u{e9} \u{65e5}\u{672c} \u{1f600}\r\nlast";

fn dolt_installed() -> bool {
    Command::new("dolt")
        .arg("version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

macro_rules! need_dolt {
    () => {
        if !dolt_installed() {
            eprintln!("skipped: the dolt binary is not installed");
            return;
        }
    };
}

struct Fixture {
    root: TempDir,
}

impl Fixture {
    fn new() -> Fixture {
        let fixture = Fixture {
            root: tempfile::tempdir().unwrap(),
        };
        std::fs::create_dir_all(fixture.home()).unwrap();
        std::fs::create_dir_all(fixture.state()).unwrap();
        let root = fixture.root.path();
        git(
            root,
            &["init", "--quiet", "--bare", "-b", "main", "origin.git"],
        );
        git(root, &["init", "--quiet", "-b", "main", "seed"]);
        let seed = root.join("seed");
        git(
            &seed,
            &[
                "remote",
                "add",
                "origin",
                fixture.origin().to_str().unwrap(),
            ],
        );
        std::fs::write(seed.join("README.md"), "main\n").unwrap();
        git(&seed, &["add", "-A"]);
        git(&seed, &["commit", "--quiet", "-m", "Main commit"]);
        git(
            &seed,
            &["push", "--quiet", "origin", "HEAD:refs/heads/main"],
        );
        for name in ["work", "work2", "work3"] {
            git(
                root,
                &["clone", "--quiet", fixture.origin().to_str().unwrap(), name],
            );
        }
        fixture
    }

    fn origin(&self) -> PathBuf {
        self.root.path().join("origin.git")
    }

    /// The clone that builds the file ledger and migrates it.
    fn a(&self) -> PathBuf {
        self.root.path().join("work")
    }

    fn b(&self) -> PathBuf {
        self.root.path().join("work2")
    }

    /// A clone that has never seen the ledger.
    fn fresh(&self) -> PathBuf {
        self.root.path().join("work3")
    }

    fn home(&self) -> PathBuf {
        self.root.path().join("home")
    }

    fn state(&self) -> PathBuf {
        self.root.path().join("state")
    }

    fn command(&self, dir: &Path, backend: &str, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_tsk"));
        command
            .args(args)
            .current_dir(dir)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "test@example.com")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "test@example.com")
            .env("HOME", self.home())
            .env("XDG_STATE_HOME", self.state())
            .env("XDG_CONFIG_HOME", self.home().join("config"))
            .env("TSK_HOME", self.home().join(".tsk"))
            .env("TSK_LEDGER_BACKEND", backend)
            .env("GIT_CEILING_DIRECTORIES", self.root.path())
            .env_remove(CLOUD_VAR);
        command
    }

    fn tsk(&self, dir: &Path, backend: &str, args: &[&str]) -> Output {
        self.command(dir, backend, args)
            .output()
            .expect("failed to run tsk")
    }

    fn ok(&self, dir: &Path, backend: &str, args: &[&str]) -> String {
        let output = self.tsk(dir, backend, args);
        assert_success(&output);
        stdout(&output)
    }

    fn put(&self, dir: &Path, backend: &str, path: &str, body: &str) {
        let mut child = self
            .command(dir, backend, &["ledger", "doc", "put", path])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("failed to run tsk");
        child
            .stdin
            .take()
            .unwrap()
            .write_all(body.as_bytes())
            .unwrap();
        assert_success(&child.wait_with_output().unwrap());
    }

    fn remote_ref(&self, name: &str) -> Option<String> {
        let out = git(&self.origin(), &["ls-remote", ".", name]);
        out.split_whitespace().next().map(str::to_string)
    }

    fn tree_names(&self, name: &str) -> Vec<String> {
        git(&self.origin(), &["ls-tree", "--name-only", name])
            .lines()
            .map(str::to_string)
            .collect()
    }

    /// A file ledger with documents, two threads (one cloud bound), continuation
    /// entries, events and a watermark, all published by clone A with the git backend.
    fn file_ledger(&self) -> Before {
        let (a, b) = (self.a(), self.b());
        self.ok(&a, "git", &["ledger", "fetch"]);
        self.put(&a, "git", "index.md", "# Ledger index\n\nOne mission.\n");
        self.put(&a, "git", BRIEFING, "# Mission: migrate\n");
        self.put(&a, "git", "notes/awkward.md", AWKWARD);
        self.put(&a, "git", "notes/empty.md", "");
        self.ok(&a, "git", &["ledger", "push", "Add documents"]);

        let start = self.ok(&a, "git", &["thread", "start", "M-MIG-01", BRIEFING]);
        let id = start
            .trim_end()
            .strip_prefix("started:")
            .unwrap()
            .to_string();
        self.ok(
            &a,
            "git",
            &[
                "thread",
                "pause",
                &id,
                BRIEFING,
                "T-01",
                "first \"step\" \u{1f600}",
            ],
        );
        self.ok(
            &a,
            "git",
            &["thread", "pause", &id, BRIEFING, "T-02", "second\\step"],
        );

        let cloud = self
            .command(&a, "git", &["thread", "start", "M-MIG-01", BRIEFING])
            .env(CLOUD_VAR, "cse_one")
            .output()
            .unwrap();
        assert_success(&cloud);

        let batch = self.command(
            &a,
            "git",
            &[
                "events",
                "append-batch",
                "--source",
                "github",
                "--action",
                "polled",
            ],
        );
        let mut child = {
            let mut c = batch;
            c.stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap()
        };
        child.stdin.take().unwrap().write_all(
            b"{\"event_type\":\"t\",\"repo\":\"o/r\",\"payload\":{\"k\":\"q\\\"uote\"}}\n{\"event_type\":\"u\",\"repo\":\"o/r\",\"payload\":3}\n",
        ).unwrap();
        assert_success(&child.wait_with_output().unwrap());
        self.ok(&a, "git", &["events", "advance-watermark", "1"]);

        Before {
            list: self.ok(&a, "git", &["thread", "list"]),
            resume: self.ok(&b, "git", &["thread", "resume", &id]),
            events: self.ok(&a, "git", &["events", "read-new"]),
            thread_id: id,
        }
    }
}

/// What the git backend printed before the migration.
struct Before {
    thread_id: String,
    list: String,
    resume: String,
    events: String,
}

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

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "exit {:?}\nstdout: {}\nstderr: {}",
        output.status.code(),
        stdout(output),
        stderr(output)
    );
}

fn assert_fails(output: &Output, needle: &str) {
    assert_eq!(
        output.status.code(),
        Some(1),
        "stdout: {}\nstderr: {}",
        stdout(output),
        stderr(output)
    );
    assert!(stderr(output).contains(needle), "{}", stderr(output));
}

#[test]
fn a_file_ledger_migrates_and_reads_back_identically_from_a_fresh_clone() {
    need_dolt!();
    let fx = Fixture::new();
    let before = fx.file_ledger();
    let old = fx.remote_ref(LEDGER_REF).expect("file ledger published");
    assert!(fx
        .tree_names(LEDGER_REF)
        .contains(&".tsk-ledger.toml".to_string()));

    let dry = fx.tsk(&fx.a(), "dolt", &["ledger", "migrate-to-dolt", "--dry-run"]);
    assert_success(&dry);
    assert_eq!(stdout(&dry), "dry-run:ok\n");
    assert!(
        stderr(&dry)
            .contains("4 documents, 2 threads, 2 continuation entries, 1 cloud bindings, 2 events"),
        "{}",
        stderr(&dry)
    );
    assert_eq!(fx.remote_ref(LEDGER_REF).as_deref(), Some(old.as_str()));
    assert_eq!(fx.remote_ref(ARCHIVE_REF), None);
    let repos = fx.state().join("tsk").join("repos");
    let leftovers: Vec<String> = std::fs::read_dir(
        std::fs::read_dir(&repos)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path(),
    )
    .unwrap()
    .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
    .collect();
    assert!(
        !leftovers.iter().any(|n| n.contains("migrate")),
        "{:?}",
        leftovers
    );

    let run = fx.tsk(&fx.a(), "dolt", &["ledger", "migrate-to-dolt"]);
    assert_success(&run);
    let out = stdout(&run);
    let parts: Vec<&str> = out.trim_end().split(':').collect();
    assert_eq!(parts.len(), 3, "{}", out);
    assert_eq!(parts[0], "migrated");
    assert_eq!(parts[1], old);
    assert!(!parts[2].is_empty());
    assert!(stderr(&run).contains(ARCHIVE_REF), "{}", stderr(&run));

    assert_eq!(fx.remote_ref(ARCHIVE_REF).as_deref(), Some(old.as_str()));
    let now = fx.remote_ref(LEDGER_REF).unwrap();
    assert_ne!(now, old);
    let names = fx.tree_names(LEDGER_REF);
    assert!(names.contains(&"manifest".to_string()), "{:?}", names);
    assert!(!names.contains(&".tsk-ledger.toml".to_string()));

    let c = fx.fresh();
    let fetch = fx.tsk(&c, "dolt", &["ledger", "fetch"]);
    assert_success(&fetch);
    for (path, body) in [
        ("index.md", "# Ledger index\n\nOne mission.\n"),
        (BRIEFING, "# Mission: migrate\n"),
        ("notes/awkward.md", AWKWARD),
        ("notes/empty.md", ""),
    ] {
        assert_eq!(
            fx.ok(&c, "dolt", &["ledger", "doc", "get", path]),
            body,
            "{}",
            path
        );
    }
    assert_eq!(
        fx.ok(&c, "dolt", &["ledger", "doc", "list"]),
        "index.md\nmissions/operational/M-MIG-01.md\nnotes/awkward.md\nnotes/empty.md\n"
    );
    assert_eq!(fx.ok(&c, "dolt", &["thread", "list"]), before.list);
    assert_eq!(
        fx.ok(&c, "dolt", &["thread", "resume", &before.thread_id]),
        before.resume
    );
    assert_eq!(fx.ok(&c, "dolt", &["events", "read-new"]), before.events);

    fx.ok(
        &c,
        "dolt",
        &[
            "thread",
            "pause",
            &before.thread_id,
            BRIEFING,
            "T-03",
            "after migration",
        ],
    );
    fx.ok(&c, "dolt", &["ledger", "push", "Nothing more"]);
    let list = fx.ok(&c, "dolt", &["thread", "list"]);
    assert!(list.contains("after migration"), "{}", list);
    assert_ne!(fx.remote_ref(LEDGER_REF).unwrap(), now);

    // The migrating clone picks the new entry up from its own database.
    fx.ok(&fx.a(), "dolt", &["ledger", "fetch"]);
    assert!(fx
        .ok(&fx.a(), "dolt", &["thread", "list"])
        .contains("after migration"));

    let again = fx.tsk(&fx.a(), "dolt", &["ledger", "migrate-to-dolt"]);
    assert_fails(&again, "already a Dolt ledger");
}

#[test]
fn migrating_without_a_ledger_fails() {
    need_dolt!();
    let fx = Fixture::new();
    let out = fx.tsk(&fx.a(), "dolt", &["ledger", "migrate-to-dolt"]);
    assert_fails(&out, "no ledger to migrate");
    assert_eq!(fx.remote_ref(ARCHIVE_REF), None);
}

#[test]
fn an_archive_branch_at_another_commit_stops_the_migration() {
    need_dolt!();
    let fx = Fixture::new();
    fx.file_ledger();
    let old = fx.remote_ref(LEDGER_REF).unwrap();
    git(
        &fx.origin(),
        &[
            "update-ref",
            ARCHIVE_REF,
            &git(&fx.origin(), &["rev-parse", "refs/heads/main"]),
        ],
    );
    let out = fx.tsk(&fx.a(), "dolt", &["ledger", "migrate-to-dolt"]);
    assert_fails(&out, "not an ancestor");
    assert_eq!(fx.remote_ref(LEDGER_REF).as_deref(), Some(old.as_str()));
}

#[test]
fn a_non_utf8_document_is_named_and_nothing_is_pushed() {
    need_dolt!();
    let fx = Fixture::new();
    fx.file_ledger();
    let old = fx.remote_ref(LEDGER_REF).unwrap();
    // Add a binary blob to the ledger through plumbing on a scratch clone of the ledger.
    let scratch = fx.root.path().join("scratch");
    git(fx.root.path(), &["init", "--quiet", "-b", "x", "scratch"]);
    git(
        &scratch,
        &[
            "fetch",
            "--quiet",
            fx.origin().to_str().unwrap(),
            LEDGER_REF,
        ],
    );
    git(&scratch, &["checkout", "--quiet", "FETCH_HEAD"]);
    std::fs::write(scratch.join("blob.bin"), [0xff, 0xfe, 0x00, 0x80]).unwrap();
    git(&scratch, &["add", "-A"]);
    git(&scratch, &["commit", "--quiet", "-m", "Add a binary file"]);
    git(
        &scratch,
        &[
            "push",
            "--quiet",
            fx.origin().to_str().unwrap(),
            &format!("HEAD:{}", LEDGER_REF),
        ],
    );
    let pushed = fx.remote_ref(LEDGER_REF).unwrap();
    assert_ne!(pushed, old);

    let out = fx.tsk(&fx.a(), "dolt", &["ledger", "migrate-to-dolt"]);
    assert_fails(&out, "blob.bin");
    assert_eq!(fx.remote_ref(ARCHIVE_REF), None);
    assert_eq!(fx.remote_ref(LEDGER_REF).as_deref(), Some(pushed.as_str()));
}

#[test]
fn another_writer_pushing_during_the_migration_aborts_it_and_a_rerun_succeeds() {
    need_dolt!();
    let fx = Fixture::new();
    fx.file_ledger();
    let old = fx.remote_ref(LEDGER_REF).unwrap();

    // When the archive branch is created, move the ledger ref on, as a second writer
    // pushing in the gap would.
    let hook = fx.origin().join("hooks").join("post-receive");
    std::fs::write(
        &hook,
        format!(
            "#!/bin/sh\nwhile read old new ref; do\n  if [ \"$ref\" = {archive} ]; then\n    tip=$(git rev-parse {ledger})\n    tree=$(git rev-parse {ledger}^{{tree}})\n    next=$(git commit-tree \"$tree\" -p \"$tip\" -m 'Another writer')\n    git update-ref {ledger} \"$next\"\n  fi\ndone\n",
            archive = ARCHIVE_REF,
            ledger = LEDGER_REF
        ),
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    let out = fx.tsk(&fx.a(), "dolt", &["ledger", "migrate-to-dolt"]);
    assert_fails(&out, "another writer pushed");
    let moved = fx.remote_ref(LEDGER_REF).unwrap();
    assert_ne!(moved, old);
    assert!(fx
        .tree_names(LEDGER_REF)
        .contains(&".tsk-ledger.toml".to_string()));
    assert_eq!(fx.remote_ref(ARCHIVE_REF).as_deref(), Some(old.as_str()));

    std::fs::remove_file(&hook).unwrap();
    let rerun = fx.tsk(&fx.a(), "dolt", &["ledger", "migrate-to-dolt"]);
    assert_success(&rerun);
    assert!(stdout(&rerun).starts_with(&format!("migrated:{}:", moved)));
    assert_eq!(fx.remote_ref(ARCHIVE_REF).as_deref(), Some(moved.as_str()));
    assert!(fx.tree_names(LEDGER_REF).contains(&"manifest".to_string()));
}
