//! `tsk ledger migrate-docs` and the version 1 refusals, end to end against a bare git
//! repo as origin. A version 1 ledger is built by hand with the `dolt` binary. Cases
//! print a note and return when the `dolt` binary is missing.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use tempfile::TempDir;

use crate::common;

use common::dolt_installed;

const LEDGER_REF: &str = "refs/heads/tsk/ledger";
const DOCS_REF: &str = "refs/heads/tsk/docs";
const VERSION_1_MESSAGE: &str = "error: this ledger stores its documents in Dolt (version 1). Run tsk ledger migrate-docs to move them to the tsk/docs branch.";
const BRIEFING: &str = "missions/operational/M-MIG-01.md";
const CLOUD_VAR: &str = "CLAUDE_CODE_REMOTE_SESSION_ID";
const AWKWARD: &str = "no newline at end \"quoted\" 'single' \\ back\\n literal\n\ttab; drop table documents; -- \u{e9} \u{65e5}\u{672c} \u{1f600}\r\nlast";

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
        common::seed_dolt_home(&fixture.home());
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
        for name in ["work", "work3"] {
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

    /// The clone that builds the version 1 ledger and migrates it.
    fn a(&self) -> PathBuf {
        self.root.path().join("work")
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

    fn command(&self, dir: &Path, args: &[&str]) -> Command {
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
            .env("TSK_DOLT_SERVER_IDLE_SECS", "3")
            .env("XDG_CONFIG_HOME", self.home().join("config"))
            .env("TSK_HOME", self.home().join(".tsk"))
            .env("TSK_LEDGER_REFS", "branches")
            .env("GIT_CEILING_DIRECTORIES", self.root.path())
            .env_remove(CLOUD_VAR);
        command
    }

    fn tsk(&self, dir: &Path, args: &[&str]) -> Output {
        self.command(dir, args).output().expect("failed to run tsk")
    }

    fn ok(&self, dir: &Path, args: &[&str]) -> String {
        let output = self.tsk(dir, args);
        assert_success(&output);
        stdout(&output)
    }

    fn put(&self, dir: &Path, path: &str, body: &str) {
        let mut child = self
            .command(dir, &["ledger", "doc", "put", path])
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
        common::write_dolt_identity(&self.home(), "old tsk", "old@localhost");
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
        let path = self.ok(clone, &["ledger", "path"]);
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

    let fetch = fx.tsk(&a, &["ledger", "fetch"]);
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
    assert_success(&fx.tsk(&a, &["ledger", "path"]));

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
        let out = fx.tsk(&a, &args);
        assert_fails(&out, VERSION_1_MESSAGE);
        assert!(stdout(&out).is_empty(), "{:?}: {}", args, stdout(&out));
    }
    let mut put = fx
        .command(&a, &["ledger", "doc", "put", "x.md"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    put.stdin.take().unwrap().write_all(b"x").unwrap();
    assert_fails(&put.wait_with_output().unwrap(), VERSION_1_MESSAGE);

    // The session start hook says why the ledger is not usable.
    let mut hook = fx
        .command(&a, &["thread", "session-start"])
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

    let dry = fx.tsk(&a, &["ledger", "migrate-docs", "--dry-run"]);
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
    assert_fails(&fx.tsk(&a, &["ledger", "doc", "list"]), VERSION_1_MESSAGE);

    // A dry run can be repeated, and the real run follows it.
    assert_success(&fx.tsk(&a, &["ledger", "migrate-docs", "--dry-run"]));
    let run = fx.tsk(&a, &["ledger", "migrate-docs"]);
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

    let run = fx.tsk(&a, &["ledger", "migrate-docs"]);
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

    // The database: no table, version 9, and one Dolt commit for the change.
    assert_eq!(
        fx.dolt_sql(&a, "select v from manifest where k = 'version'"),
        "v\n11\n"
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
            &fx.ok(&a, &["ledger", "doc", "get", path]),
            body,
            "{}",
            path
        );
    }

    // A rerun is an error, not a second migration.
    let again = fx.tsk(&a, &["ledger", "migrate-docs"]);
    assert_fails(&again, "the documents are already on the tsk/docs branch");

    // A fresh clone reads everything, and the thread records came across.
    let c = fx.fresh();
    assert_success(&fx.tsk(&c, &["ledger", "fetch"]));
    for (path, body) in OLD_DOCUMENTS {
        assert_eq!(
            &fx.ok(&c, &["ledger", "doc", "get", path]),
            body,
            "{}",
            path
        );
    }
    assert_eq!(
        fx.ok(&c, &["ledger", "doc", "list", "notes/"]),
        "notes/awkward.md\nnotes/empty.md\nnotes/no-newline.md\nnotes/quotes.md\n"
    );
    let list = fx.ok(&c, &["thread", "list"]);
    assert!(list.contains("aaaa1111"), "{}", list);
    assert_fails(
        &fx.tsk(&c, &["ledger", "migrate-docs"]),
        "already on the tsk/docs branch",
    );

    // And it writes: a document and a push from the new clone reach the old one.
    fx.put(&c, "after.md", "after\n");
    fx.ok(&c, &["ledger", "push", "After migration"]);
    fx.ok(&a, &["ledger", "fetch"]);
    assert_eq!(fx.ok(&a, &["ledger", "doc", "get", "after.md"]), "after\n");
}

#[test]
fn an_older_tsk_stops_at_the_manifest_check_of_a_version_it_does_not_know() {
    need_dolt!();
    let fx = Fixture::new();
    // A version this tsk does not know stands in for what this tsk meets in a later release.
    fx.old_ledger("12", &[], "");
    let out = fx.tsk(&fx.a(), &["ledger", "fetch"]);
    assert_fails(&out, "unsupported ledger version 12");
    assert!(
        stderr(&out).contains("supports version 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11"),
        "{}",
        stderr(&out)
    );
    let out = fx.tsk(&fx.a(), &["ledger", "migrate-docs"]);
    assert_fails(&out, "unsupported ledger version 12");
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
        let out = fx.tsk(&fx.a(), &["ledger", "migrate-docs"]);
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
    let out = fx.tsk(&fx.a(), &["ledger", "migrate-docs"]);
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

    let run = fx.tsk(&fx.a(), &["ledger", "migrate-docs"]);
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
    fx.ok(&fx.fresh(), &["ledger", "fetch"]);
    assert_eq!(
        fx.ok(&fx.fresh(), &["ledger", "doc", "get", "notes/awkward.md"]),
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

    let out = fx.tsk(&fx.a(), &["ledger", "migrate-docs"]);
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
    let out = fx.tsk(&a, &["ledger", "migrate-docs"]);
    assert_fails(&out, "tsk ledger push");
    assert_eq!(fx.remote_ref(DOCS_REF), None);
    assert_eq!(
        fx.remote_ref(LEDGER_REF).as_deref(),
        Some(ledger_before.as_str())
    );

    std::fs::remove_file(&hook).unwrap();
    let resumed = fx.tsk(&a, &["ledger", "push", "Resume the migration"]);
    assert_success(&resumed);
    assert!(fx.remote_ref(DOCS_REF).is_some());
    assert_ne!(fx.remote_ref(LEDGER_REF).unwrap(), ledger_before);
    assert_eq!(fx.docs_blob("notes/awkward.md"), AWKWARD);
    let c = fx.fresh();
    fx.ok(&c, &["ledger", "fetch"]);
    assert_eq!(
        fx.ok(&c, &["ledger", "doc", "get", "notes/awkward.md"]),
        AWKWARD
    );
}

#[test]
fn migrate_docs_on_a_version_two_ledger_says_the_documents_are_already_moved() {
    need_dolt!();
    let fx = Fixture::new();
    fx.ok(&fx.a(), &["ledger", "fetch"]);
    fx.put(&fx.a(), "x.md", "x\n");
    fx.ok(&fx.a(), &["ledger", "push", "Publish"]);
    let out = fx.tsk(&fx.a(), &["ledger", "migrate-docs"]);
    assert_fails(&out, "the documents are already on the tsk/docs branch");
    let dry = fx.tsk(&fx.a(), &["ledger", "migrate-docs", "--dry-run"]);
    assert_fails(&dry, "the documents are already on the tsk/docs branch");
}
