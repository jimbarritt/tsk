//! `tsk ledger migrate-to-dolt`, end to end against a bare git repo as origin. A file
//! ledger is built with the git backend, migrated, and read back from a fresh clone with
//! the Dolt backend. Cases print a note and return when the `dolt` binary is missing.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use tempfile::TempDir;

const LEDGER_REF: &str = "refs/heads/tsk/ledger";
const DOCS_REF: &str = "refs/heads/tsk/docs";
const ARCHIVE_REF: &str = "refs/heads/archive/tsk-ledger-files";
const VERSION_1_MESSAGE: &str = "error: this ledger stores its documents in Dolt (version 1). Run tsk ledger migrate-docs to move them to the tsk/docs branch.";
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

    /// The files of the docs branch on the origin.
    fn docs_files(&self) -> Vec<String> {
        git(&self.origin(), &["ls-tree", "-r", "--name-only", DOCS_REF])
            .lines()
            .map(str::to_string)
            .collect()
    }

    fn docs_blob(&self, path: &str) -> String {
        let out = Command::new("git")
            .args(["cat-file", "blob", &format!("{}:{}", DOCS_REF, path)])
            .current_dir(self.origin())
            .output()
            .unwrap();
        assert!(out.status.success(), "{}", path);
        String::from_utf8(out.stdout).unwrap()
    }

    /// Build a version 1 Dolt ledger by hand, as an older tsk wrote it: the documents are
    /// rows of a `documents` table and the manifest says version 1 (or `version`). It is
    /// pushed to the origin's ledger ref through a `git+file` remote. `extra_sql` runs in
    /// the same commit.
    fn old_ledger(&self, version: &str, documents: &[(&str, &str)], extra_sql: &str) {
        let dir = self.root.path().join("v1-db");
        std::fs::create_dir_all(&dir).unwrap();
        let run = |args: &[&str]| {
            let out = Command::new("dolt")
                .args(args)
                .current_dir(&dir)
                .env("HOME", self.home())
                .env("DOLT_DISABLE_EVENT_FLUSH", "1")
                .env("DOLT_REMOTE_INFO_BRANCH", "")
                .output()
                .expect("failed to run dolt");
            assert!(
                out.status.success(),
                "dolt {:?} failed: {}",
                args,
                String::from_utf8_lossy(&out.stderr)
            );
        };
        run(&["config", "--global", "--add", "user.name", "old tsk"]);
        run(&["config", "--global", "--add", "user.email", "old@localhost"]);
        run(&["init", "--initial-branch", "main"]);
        let mut sql = String::from(
            "create table manifest (k varchar(64) primary key, v text);\n\
             create table documents (path varchar(1024) primary key, body longtext not null);\n\
             create table threads (thread_id varchar(64) primary key, briefing_path varchar(1024) not null);\n\
             create table continuation_entries (thread_id varchar(64), seq int, entry longtext not null, primary key (thread_id, seq));\n\
             create table cloud_session_bindings (session_id varchar(255) primary key, thread_id varchar(64) not null, registered_at varchar(32) not null);\n\
             create table external_events (position bigint primary key, envelope longtext not null);\n\
             create table watermark (id int primary key, processed_through bigint not null, updated_at varchar(32) not null);\n",
        );
        sql.push_str(&format!(
            "insert into manifest (k, v) values ('version', '{}');\n",
            version
        ));
        for (path, body) in documents {
            sql.push_str(&format!(
                "insert into documents (path, body) values ({}, {});\n",
                sql_text(path),
                sql_text(body)
            ));
        }
        sql.push_str(extra_sql);
        std::fs::write(dir.join("build.sql"), &sql).unwrap();
        run(&["sql", "--file", "build.sql"]);
        std::fs::remove_file(dir.join("build.sql")).unwrap();
        run(&["add", "-A"]);
        run(&["commit", "-m", "Create the ledger"]);
        run(&[
            "remote",
            "add",
            "--ref",
            LEDGER_REF,
            "origin",
            &format!("git+file://{}", self.origin().display()),
        ]);
        run(&["push", "origin", "main"]);
    }

    /// Run SQL against the database of a clone and return the CSV it prints.
    fn dolt_sql(&self, clone: &Path, sql: &str) -> String {
        let path = self.ok(clone, "dolt", &["ledger", "path"]);
        let out = Command::new("dolt")
            .args(["sql", "-r", "csv", "-q", sql])
            .current_dir(path.trim_end())
            .env("HOME", self.home())
            .env("DOLT_DISABLE_EVENT_FLUSH", "1")
            .output()
            .unwrap();
        assert!(out.status.success(), "{}", stderr(&out));
        stdout(&out)
    }

    fn tree_names(&self, name: &str) -> Vec<String> {
        git(&self.origin(), &["ls-tree", "--name-only", name])
            .lines()
            .map(str::to_string)
            .collect()
    }

    /// A file ledger with documents, two threads (one cloud bound, one worktree bound), continuation
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

/// A SQL string literal that carries any text: hex-encoded, so no byte is parsed.
fn sql_text(text: &str) -> String {
    if text.is_empty() {
        return "''".to_string();
    }
    let hex: String = text.bytes().map(|b| format!("{:02x}", b)).collect();
    format!("CONVERT(X'{}' USING utf8mb4)", hex)
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
            .contains("4 documents, 2 threads, 2 continuation entries, 2 bindings, 2 events"),
        "{}",
        stderr(&dry)
    );
    assert_eq!(fx.remote_ref(LEDGER_REF).as_deref(), Some(old.as_str()));
    assert_eq!(fx.remote_ref(ARCHIVE_REF), None);
    assert_eq!(fx.remote_ref(DOCS_REF), None);
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
    // A dry run leaves no documents worktree and no linked worktree behind.
    assert!(!leftovers.iter().any(|n| n == "docs"), "{:?}", leftovers);
    let linked = git(&fx.a(), &["worktree", "list", "--porcelain"]);
    assert!(!linked.contains("docs"), "{}", linked);

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
    assert!(!names.iter().any(|n| n.ends_with(".md")), "{:?}", names);

    // The documents are plain files on the docs branch, byte for byte.
    assert!(fx.remote_ref(DOCS_REF).is_some());
    assert_eq!(
        fx.docs_files(),
        vec![
            "README.md",
            "index.md",
            "missions/operational/M-MIG-01.md",
            "notes/awkward.md",
            "notes/empty.md"
        ]
    );
    assert_eq!(fx.docs_blob("notes/awkward.md"), AWKWARD);
    assert_eq!(fx.docs_blob("notes/empty.md"), "");
    assert_eq!(fx.docs_blob("index.md"), "# Ledger index\n\nOne mission.\n");
    assert!(
        stderr(&run).contains("read back identical"),
        "{}",
        stderr(&run)
    );
    // The migrating clone keeps its documents worktree.
    assert_eq!(
        fx.ok(
            &fx.a(),
            "dolt",
            &["ledger", "doc", "get", "notes/awkward.md"]
        ),
        AWKWARD
    );

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
        "README.md\nindex.md\nmissions/operational/M-MIG-01.md\nnotes/awkward.md\nnotes/empty.md\n"
    );
    assert_eq!(fx.ok(&c, "dolt", &["thread", "list"]), before.list);
    assert_eq!(
        fx.ok(&c, "dolt", &["thread", "resume", &before.thread_id]),
        before.resume
    );
    // The resume bound this worktree, a local change that the events commands refuse over.
    fx.ok(&c, "dolt", &["ledger", "push", "Bind the fresh clone"]);
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
    // The documents are pushed only after the check, so nothing reached the docs branch.
    assert_eq!(fx.remote_ref(DOCS_REF), None);
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

// --- Version 1 ledgers and `tsk ledger migrate-docs` ---

/// Documents with awkward text: quotes, backslashes, newlines, emoji, an empty body.
const OLD_DOCUMENTS: &[(&str, &str)] = &[
    ("index.md", "# Ledger index\n\nOne mission.\n"),
    (BRIEFING, "# Mission: migrate\n"),
    ("notes/awkward.md", AWKWARD),
    ("notes/empty.md", ""),
    (
        "notes/quotes.md",
        "\"double\" 'single' \\ back \\n literal\n\n\n",
    ),
    ("notes/no-newline.md", "ends without one \u{1f600}"),
];

fn thread_sql() -> String {
    format!(
        "insert into threads (thread_id, briefing_path) values ('aaaa1111', {});\n\
         insert into continuation_entries (thread_id, seq, entry) values ('aaaa1111', 1, {});\n",
        sql_text(BRIEFING),
        sql_text("{\"task_id\":\"T-01\",\"whats_next\":\"go\"}")
    )
}

#[test]
fn a_version_one_ledger_is_refused_by_every_command_except_fetch_path_and_migrate_docs() {
    need_dolt!();
    let fx = Fixture::new();
    fx.old_ledger("1", OLD_DOCUMENTS, &thread_sql());
    let a = fx.a();

    let fetch = fx.tsk(&a, "dolt", &["ledger", "fetch"]);
    assert_success(&fetch);
    assert!(
        stdout(&fetch).trim_end().ends_with("/ledger-db"),
        "{}",
        stdout(&fetch)
    );
    assert!(stderr(&fetch).contains("version 1"), "{}", stderr(&fetch));
    assert!(
        stderr(&fetch).contains("tsk ledger migrate-docs"),
        "{}",
        stderr(&fetch)
    );
    assert_success(&fx.tsk(&a, "dolt", &["ledger", "path"]));

    for args in [
        vec!["ledger", "doc", "get", "index.md"],
        vec!["ledger", "doc", "list"],
        vec!["ledger", "doc", "path", "index.md"],
        vec!["ledger", "doc", "rm", "index.md"],
        vec!["ledger", "push", "Try"],
        vec!["thread", "list"],
        vec!["thread", "binding"],
        vec!["thread", "start", "M-MIG-01", BRIEFING],
        vec!["events", "read-new"],
    ] {
        let out = fx.tsk(&a, "dolt", &args);
        assert_fails(&out, VERSION_1_MESSAGE);
        assert!(stdout(&out).is_empty(), "{:?}: {}", args, stdout(&out));
    }
    let mut put = fx
        .command(&a, "dolt", &["ledger", "doc", "put", "x.md"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    put.stdin.take().unwrap().write_all(b"x").unwrap();
    assert_fails(&put.wait_with_output().unwrap(), VERSION_1_MESSAGE);

    // The session start hook says why the ledger is not usable.
    let mut hook = fx
        .command(&a, "dolt", &["thread", "session-start"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    hook.stdin
        .take()
        .unwrap()
        .write_all(b"{\"session_id\":\"v1\",\"source\":\"startup\"}")
        .unwrap();
    let hook = hook.wait_with_output().unwrap();
    assert_success(&hook);
    let context = serde_json::from_str::<serde_json::Value>(&stdout(&hook)).unwrap()
        ["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(
        context.contains("is at layout version 1: its documents are still in Dolt."),
        "{}",
        context
    );
    assert!(
        context.contains("Run `tsk ledger migrate-docs` once to move them to the tsk/docs branch."),
        "{}",
        context
    );
    assert!(
        context.contains("every tsk command except `tsk ledger fetch` stops"),
        "{}",
        context
    );
    assert!(!context.contains("could not be fetched"), "{}", context);
    assert!(!context.contains("No thread binding"), "{}", context);
}

#[test]
fn migrate_docs_dry_run_pushes_nothing_and_leaves_the_table_in_place() {
    need_dolt!();
    let fx = Fixture::new();
    fx.old_ledger("1", OLD_DOCUMENTS, &thread_sql());
    let ledger_before = fx.remote_ref(LEDGER_REF).unwrap();
    let a = fx.a();

    let dry = fx.tsk(&a, "dolt", &["ledger", "migrate-docs", "--dry-run"]);
    assert_success(&dry);
    assert_eq!(stdout(&dry), "dry-run:ok\n");
    assert!(
        stderr(&dry).contains("read 6 documents"),
        "{}",
        stderr(&dry)
    );
    assert!(
        stderr(&dry).contains("read back identical"),
        "{}",
        stderr(&dry)
    );
    assert_eq!(fx.remote_ref(DOCS_REF), None);
    assert_eq!(
        fx.remote_ref(LEDGER_REF).as_deref(),
        Some(ledger_before.as_str())
    );
    // Still version 1 in the database, with the table and its rows.
    assert_eq!(
        fx.dolt_sql(&a, "select count(*) as c from documents"),
        "c\n6\n"
    );
    assert_fails(
        &fx.tsk(&a, "dolt", &["ledger", "doc", "list"]),
        VERSION_1_MESSAGE,
    );

    // A dry run can be repeated, and the real run follows it.
    assert_success(&fx.tsk(&a, "dolt", &["ledger", "migrate-docs", "--dry-run"]));
    let run = fx.tsk(&a, "dolt", &["ledger", "migrate-docs"]);
    assert_success(&run);
    assert_eq!(stdout(&run), "migrated-docs:6\n");
}

#[test]
fn migrate_docs_moves_every_document_byte_for_byte_and_a_fresh_clone_reads_them() {
    need_dolt!();
    let fx = Fixture::new();
    fx.old_ledger("1", OLD_DOCUMENTS, &thread_sql());
    let old_ledger = fx.remote_ref(LEDGER_REF).unwrap();
    let a = fx.a();

    let run = fx.tsk(&a, "dolt", &["ledger", "migrate-docs"]);
    assert_success(&run);
    assert_eq!(stdout(&run), "migrated-docs:6\n");
    assert!(
        stderr(&run).contains("read 6 documents"),
        "{}",
        stderr(&run)
    );

    // Both branches are pushed. The ledger branch holds Dolt's files and no Markdown.
    assert!(fx.remote_ref(DOCS_REF).is_some());
    assert_ne!(fx.remote_ref(LEDGER_REF).unwrap(), old_ledger);
    let names = fx.tree_names(LEDGER_REF);
    assert!(names.contains(&"manifest".to_string()), "{:?}", names);
    assert!(!names.iter().any(|n| n.ends_with(".md")), "{:?}", names);
    let mut expected: Vec<String> = OLD_DOCUMENTS.iter().map(|(p, _)| p.to_string()).collect();
    expected.push("README.md".to_string());
    expected.sort();
    assert_eq!(fx.docs_files(), expected);
    for (path, body) in OLD_DOCUMENTS {
        assert_eq!(&fx.docs_blob(path), body, "{}", path);
    }
    assert_eq!(
        fx.docs_blob("README.md"),
        "Documents of the tsk ledger. Written by tsk ledger doc put.\n"
    );

    // The database: no table, version 8, and one Dolt commit for the change.
    assert_eq!(
        fx.dolt_sql(&a, "select v from manifest where k = 'version'"),
        "v\n8\n"
    );
    // The migrated thread gained the mission column, still null.
    assert_eq!(
        fx.dolt_sql(&a, "select thread_id, mission_id from threads"),
        "thread_id,mission_id\naaaa1111,\n"
    );
    let tables = fx.dolt_sql(&a, "show tables");
    assert!(!tables.lines().any(|t| t == "documents"), "{}", tables);
    assert!(tables.contains("threads"), "{}", tables);
    for table in [
        "missions",
        "mission_constraints",
        "mission_out_of_scope",
        "bindings",
        "tasks",
        "task_blocks",
        "decisions",
        "questions",
    ] {
        assert!(tables.lines().any(|t| t == table), "{}: {}", table, tables);
    }
    let log = fx.dolt_sql(&a, "select message from dolt_log limit 2");
    assert_eq!(
        log,
        "message\nMove the documents to the tsk/docs branch\nCreate the ledger\n"
    );
    // The migrating clone reads its documents from the worktree.
    for (path, body) in OLD_DOCUMENTS {
        assert_eq!(
            &fx.ok(&a, "dolt", &["ledger", "doc", "get", path]),
            body,
            "{}",
            path
        );
    }

    // A rerun is an error, not a second migration.
    let again = fx.tsk(&a, "dolt", &["ledger", "migrate-docs"]);
    assert_fails(&again, "the documents are already on the tsk/docs branch");

    // A fresh clone reads everything, and the thread records came across.
    let c = fx.fresh();
    assert_success(&fx.tsk(&c, "dolt", &["ledger", "fetch"]));
    for (path, body) in OLD_DOCUMENTS {
        assert_eq!(
            &fx.ok(&c, "dolt", &["ledger", "doc", "get", path]),
            body,
            "{}",
            path
        );
    }
    assert_eq!(
        fx.ok(&c, "dolt", &["ledger", "doc", "list", "notes/"]),
        "notes/awkward.md\nnotes/empty.md\nnotes/no-newline.md\nnotes/quotes.md\n"
    );
    let list = fx.ok(&c, "dolt", &["thread", "list"]);
    assert!(list.contains("aaaa1111"), "{}", list);
    assert_fails(
        &fx.tsk(&c, "dolt", &["ledger", "migrate-docs"]),
        "already on the tsk/docs branch",
    );

    // And it writes: a document and a push from the new clone reach the old one.
    fx.put(&c, "dolt", "after.md", "after\n");
    fx.ok(&c, "dolt", &["ledger", "push", "After migration"]);
    fx.ok(&a, "dolt", &["ledger", "fetch"]);
    assert_eq!(
        fx.ok(&a, "dolt", &["ledger", "doc", "get", "after.md"]),
        "after\n"
    );
}

#[test]
fn an_older_tsk_stops_at_the_manifest_check_of_a_version_it_does_not_know() {
    need_dolt!();
    let fx = Fixture::new();
    // A version this tsk does not know stands in for what this tsk meets in a later release.
    fx.old_ledger("9", &[], "");
    let out = fx.tsk(&fx.a(), "dolt", &["ledger", "fetch"]);
    assert_fails(&out, "unsupported ledger version 9");
    assert!(
        stderr(&out).contains("supports version 1, 2, 3, 4, 5, 6, 7, 8"),
        "{}",
        stderr(&out)
    );
    let out = fx.tsk(&fx.a(), "dolt", &["ledger", "migrate-docs"]);
    assert_fails(&out, "unsupported ledger version 9");
    assert_eq!(fx.remote_ref(DOCS_REF), None);
}

#[test]
fn migrate_docs_refuses_a_path_that_cannot_be_a_file_and_names_it() {
    need_dolt!();
    for (bad, needle) in [
        ("../escape.md", "../escape.md"),
        (".git/config", ".git/config"),
        ("/abs.md", "/abs.md"),
    ] {
        let fx = Fixture::new();
        fx.old_ledger("1", &[("index.md", "# i\n"), (bad, "x")], "");
        let out = fx.tsk(&fx.a(), "dolt", &["ledger", "migrate-docs"]);
        assert_fails(&out, needle);
        assert!(
            stderr(&out).contains("nothing was changed"),
            "{}",
            stderr(&out)
        );
        assert_eq!(fx.remote_ref(DOCS_REF), None);
        assert_eq!(
            fx.dolt_sql(&fx.a(), "select count(*) as c from documents"),
            "c\n2\n"
        );
    }

    // A file that is also a directory.
    let fx = Fixture::new();
    fx.old_ledger("1", &[("a", "x"), ("a/b.md", "y")], "");
    let out = fx.tsk(&fx.a(), "dolt", &["ledger", "migrate-docs"]);
    assert_fails(&out, "cannot be a file");
    assert_eq!(fx.remote_ref(DOCS_REF), None);
}

#[test]
fn a_rerun_compares_the_files_the_docs_branch_already_holds_and_goes_on() {
    need_dolt!();
    let fx = Fixture::new();
    fx.old_ledger("1", OLD_DOCUMENTS, &thread_sql());

    // An earlier run pushed the documents and stopped before the Dolt data: the same
    // files are on the docs branch, but the ledger is still version 1.
    let scratch = fx.root.path().join("scratch");
    git(fx.root.path(), &["init", "--quiet", "-b", "x", "scratch"]);
    for (path, body) in OLD_DOCUMENTS {
        let file = scratch.join(path);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, body).unwrap();
    }
    std::fs::write(
        scratch.join("README.md"),
        "Documents of the tsk ledger. Written by tsk ledger doc put.\n",
    )
    .unwrap();
    git(&scratch, &["add", "-A"]);
    git(&scratch, &["commit", "--quiet", "-m", "An earlier run"]);
    git(
        &scratch,
        &[
            "push",
            "--quiet",
            fx.origin().to_str().unwrap(),
            &format!("HEAD:{}", DOCS_REF),
        ],
    );
    let docs_before = fx.remote_ref(DOCS_REF).unwrap();

    let run = fx.tsk(&fx.a(), "dolt", &["ledger", "migrate-docs"]);
    assert_success(&run);
    assert_eq!(stdout(&run), "migrated-docs:6\n");
    assert!(
        stderr(&run).contains("6 already on the branch"),
        "{}",
        stderr(&run)
    );
    // Nothing new to commit on the docs branch: the remote already held every file.
    assert_eq!(
        fx.remote_ref(DOCS_REF).as_deref(),
        Some(docs_before.as_str())
    );
    fx.ok(&fx.fresh(), "dolt", &["ledger", "fetch"]);
    assert_eq!(
        fx.ok(
            &fx.fresh(),
            "dolt",
            &["ledger", "doc", "get", "notes/awkward.md"]
        ),
        AWKWARD
    );
}

#[test]
fn a_docs_branch_file_that_differs_from_its_row_stops_migrate_docs_before_any_push() {
    need_dolt!();
    let fx = Fixture::new();
    fx.old_ledger("1", OLD_DOCUMENTS, "");
    let ledger_before = fx.remote_ref(LEDGER_REF).unwrap();
    let scratch = fx.root.path().join("scratch");
    git(fx.root.path(), &["init", "--quiet", "-b", "x", "scratch"]);
    std::fs::write(scratch.join("index.md"), "# Someone else's index\n").unwrap();
    git(&scratch, &["add", "-A"]);
    git(&scratch, &["commit", "--quiet", "-m", "A different index"]);
    git(
        &scratch,
        &[
            "push",
            "--quiet",
            fx.origin().to_str().unwrap(),
            &format!("HEAD:{}", DOCS_REF),
        ],
    );
    let docs_before = fx.remote_ref(DOCS_REF).unwrap();

    let out = fx.tsk(&fx.a(), "dolt", &["ledger", "migrate-docs"]);
    assert_fails(&out, "index.md");
    assert!(
        stderr(&out).contains("different content"),
        "{}",
        stderr(&out)
    );
    assert_eq!(
        fx.remote_ref(DOCS_REF).as_deref(),
        Some(docs_before.as_str())
    );
    assert_eq!(
        fx.remote_ref(LEDGER_REF).as_deref(),
        Some(ledger_before.as_str())
    );
    assert_eq!(
        fx.dolt_sql(&fx.a(), "select count(*) as c from documents"),
        "c\n6\n"
    );
}

#[test]
fn a_rejected_docs_push_leaves_the_dolt_data_unpushed_and_tsk_ledger_push_resumes() {
    need_dolt!();
    let fx = Fixture::new();
    fx.old_ledger("1", OLD_DOCUMENTS, "");
    let ledger_before = fx.remote_ref(LEDGER_REF).unwrap();
    let hook = fx.origin().join("hooks").join("pre-receive");
    std::fs::write(
        &hook,
        "#!/bin/sh\nwhile read old new ref; do\n  if [ \"$ref\" = refs/heads/tsk/docs ]; then\n    exit 1\n  fi\ndone\n",
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    let a = fx.a();
    let out = fx.tsk(&a, "dolt", &["ledger", "migrate-docs"]);
    assert_fails(&out, "tsk ledger push");
    assert_eq!(fx.remote_ref(DOCS_REF), None);
    assert_eq!(
        fx.remote_ref(LEDGER_REF).as_deref(),
        Some(ledger_before.as_str())
    );

    std::fs::remove_file(&hook).unwrap();
    let resumed = fx.tsk(&a, "dolt", &["ledger", "push", "Resume the migration"]);
    assert_success(&resumed);
    assert!(fx.remote_ref(DOCS_REF).is_some());
    assert_ne!(fx.remote_ref(LEDGER_REF).unwrap(), ledger_before);
    assert_eq!(fx.docs_blob("notes/awkward.md"), AWKWARD);
    let c = fx.fresh();
    fx.ok(&c, "dolt", &["ledger", "fetch"]);
    assert_eq!(
        fx.ok(&c, "dolt", &["ledger", "doc", "get", "notes/awkward.md"]),
        AWKWARD
    );
}

#[test]
fn migrate_docs_on_a_version_two_ledger_says_the_documents_are_already_moved() {
    need_dolt!();
    let fx = Fixture::new();
    fx.ok(&fx.a(), "dolt", &["ledger", "fetch"]);
    fx.put(&fx.a(), "dolt", "x.md", "x\n");
    fx.ok(&fx.a(), "dolt", &["ledger", "push", "Publish"]);
    let out = fx.tsk(&fx.a(), "dolt", &["ledger", "migrate-docs"]);
    assert_fails(&out, "the documents are already on the tsk/docs branch");
    let dry = fx.tsk(&fx.a(), "dolt", &["ledger", "migrate-docs", "--dry-run"]);
    assert_fails(&dry, "the documents are already on the tsk/docs branch");
}
