use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

mod common;

const IN_REPO_REF: &str = "refs/heads/tsk/ledger";
const NEXUS_REF: &str = "refs/heads/ledgers/work-api";
const NEXUS_DOCS_REF: &str = "refs/heads/ledgers/work-api-docs";
const ORIGIN_URL: &str = "https://example.test/acme/work-api";
const BRIEFING: &str = "missions/operational/M-TEST-01-example.md";
const WORK_API_ENTRY: &str =
    r#"{"id":"work-api","url":"https://example.test/acme/work-api","ledger":"nexus"}"#;

struct Fixture {
    root: TempDir,
}

impl Drop for Fixture {
    /// Stop the Dolt servers that the commands started (ADR 0018) before the files go.
    fn drop(&mut self) {
        common::stop_servers(self.root.path(), false);
    }
}

impl Fixture {
    fn new() -> Fixture {
        let fixture = Fixture {
            root: tempfile::tempdir().unwrap(),
        };
        let root = fixture.root.path();
        for dir in ["home", "state", "config"] {
            std::fs::create_dir_all(root.join(dir)).unwrap();
        }
        git(
            root,
            &["init", "--quiet", "--bare", "-b", "main", "managed.git"],
        );
        git(root, &["init", "--quiet", "-b", "main", "managed-seed"]);
        let managed_seed = root.join("managed-seed");
        git(
            &managed_seed,
            &[
                "remote",
                "add",
                "origin",
                fixture.managed().to_str().unwrap(),
            ],
        );
        write(&managed_seed.join("README.md"), "main\n");
        commit_all(&managed_seed, "Main commit");
        git(
            &managed_seed,
            &["push", "--quiet", "origin", "HEAD:refs/heads/main"],
        );
        git(
            root,
            &[
                "clone",
                "--quiet",
                fixture.managed().to_str().unwrap(),
                "work",
            ],
        );
        git(
            &fixture.work(),
            &["config", "remote.origin.url", ORIGIN_URL],
        );
        git(
            &fixture.work(),
            &[
                "config",
                &format!("url.{}.insteadOf", fixture.managed().display()),
                "unused",
            ],
        );
        git(
            &fixture.work(),
            &[
                "config",
                "--unset-all",
                &format!("url.{}.insteadOf", fixture.managed().display()),
            ],
        );
        git(
            &fixture.work(),
            &[
                "config",
                &format!("url.{}.insteadOf", fixture.managed().display()),
                ORIGIN_URL,
            ],
        );

        git(
            root,
            &["init", "--quiet", "--bare", "-b", "main", "nexus.git"],
        );
        git(root, &["init", "--quiet", "-b", "main", "nexus-seed"]);
        git(
            &fixture.nexus_seed(),
            &["remote", "add", "origin", fixture.nexus().to_str().unwrap()],
        );
        fixture.set_nexus_json("");
        fixture
    }

    fn managed(&self) -> PathBuf {
        self.root.path().join("managed.git")
    }

    fn nexus(&self) -> PathBuf {
        self.root.path().join("nexus.git")
    }

    fn nexus_seed(&self) -> PathBuf {
        self.root.path().join("nexus-seed")
    }

    fn work(&self) -> PathBuf {
        self.root.path().join("work")
    }

    fn config_file(&self) -> PathBuf {
        self.root.path().join("config/tsk/config.toml")
    }

    fn repo_id_file(&self) -> PathBuf {
        self.work().join(".git").join("tsk-repo-id")
    }

    fn set_nexus_json(&self, entries: &str) {
        let text = format!(
            "{{\"version\":1,\"territories\":[{{\"id\":\"t\",\"name\":\"T\",\"repos\":[{}]}}]}}\n",
            entries
        );
        write(&self.nexus_seed().join("nexus.json"), &text);
        commit_all(&self.nexus_seed(), "Set nexus.json");
        git(
            &self.nexus_seed(),
            &["push", "--quiet", "origin", "HEAD:refs/heads/main"],
        );
    }

    fn attach(&self) {
        let output = self.tsk(&["config", "attach-nexus", self.nexus().to_str().unwrap()]);
        assert_success(&output);
    }

    fn tsk(&self, args: &[&str]) -> Output {
        self.tsk_as("laptop", args)
    }

    fn tsk_as(&self, machine: &str, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_tsk"))
            .args(args)
            .current_dir(self.work())
            .envs(git_env())
            .env("HOME", self.root.path().join("home"))
            .env("XDG_STATE_HOME", self.root.path().join("state"))
            .env("TSK_DOLT_SERVER_IDLE_SECS", "3")
            .env("XDG_CONFIG_HOME", self.root.path().join("config"))
            .env("TSK_MACHINE_NAME", machine)
            .env("TSK_HOME", self.root.path().join("home/.tsk"))
            .env("GIT_CEILING_DIRECTORIES", self.root.path())
            .env_remove("CLAUDE_CODE_REMOTE_SESSION_ID")
            .output()
            .expect("failed to run tsk")
    }

    fn fetch_ok(&self) -> PathBuf {
        let output = self.tsk(&["ledger", "fetch"]);
        assert_success(&output);
        PathBuf::from(stdout(&output).trim_end())
    }

    fn push_ok(&self, message: &str) {
        assert_success(&self.tsk(&["ledger", "push", message]));
    }

    fn ref_exists(&self, repo: &Path, git_ref: &str) -> bool {
        git_output(repo, &["rev-parse", "--verify", "--quiet", git_ref])
            .status
            .success()
    }

    fn file_on(&self, repo: &Path, git_ref: &str, file: &str) -> Option<String> {
        let output = git_output(
            repo,
            &["cat-file", "blob", &format!("{}:{}", git_ref, file)],
        );
        output
            .status
            .success()
            .then(|| String::from_utf8(output.stdout).unwrap())
    }

    /// Run SQL against the Dolt database of the work clone and return the CSV it prints.
    fn sql(&self, sql: &str) -> String {
        let path = stdout(&self.tsk(&["ledger", "path"]));
        // A Dolt server that an earlier command started holds the store for writing.
        common::stop_servers(self.root.path(), true);
        let out = Command::new("dolt")
            .args(["sql", "-r", "csv", "-q", sql])
            .current_dir(path.trim_end())
            .env("HOME", self.root.path().join("home"))
            .env("DOLT_DISABLE_EVENT_FLUSH", "1")
            .output()
            .unwrap();
        assert!(out.status.success(), "{}", stderr(&out));
        stdout(&out)
    }

    /// The `repo_id` row of the manifest table of the work clone's ledger.
    fn manifest_repo_id(&self) -> String {
        self.sql("select v from manifest where k = 'repo_id'")
            .lines()
            .nth(1)
            .unwrap_or("")
            .to_string()
    }

    /// Forget this clone's ledger: the next fetch mints a new clone ID, so it builds a
    /// new database from what the remote holds.
    fn forget_local_ledger(&self) {
        std::fs::remove_file(self.work().join(".git").join("tsk-clone-id")).unwrap();
    }

    /// The head of the work clone's documents worktree.
    fn docs_head(&self) -> String {
        let ledger = PathBuf::from(stdout(&self.tsk(&["ledger", "path"])).trim_end());
        git_line(
            &ledger.parent().unwrap().join("docs"),
            &["rev-parse", "HEAD"],
        )
    }

    /// Push a Dolt commit by hand whose manifest names another repo, so that a fetch
    /// from a clone meets a ledger that belongs elsewhere.
    fn publish_ledger_of_another_repo(&self) {
        self.fetch_ok();
        let path = PathBuf::from(stdout(&self.tsk(&["ledger", "path"])).trim_end());
        self.sql("replace into manifest (k, v) values ('repo_id', 'someone-else')");
        for args in [
            vec!["add", "-A"],
            vec!["commit", "-m", "Another repo"],
            vec!["push", "origin", "main"],
        ] {
            let out = Command::new("dolt")
                .args(&args)
                .current_dir(&path)
                .env("HOME", self.root.path().join("home"))
                .env("DOLT_DISABLE_EVENT_FLUSH", "1")
                .output()
                .unwrap();
            assert!(out.status.success(), "{:?}: {}", args, stderr(&out));
        }
        // The ledger is a Dolt file remote carried by plain git: publish the directory.
        let remote_dir = path.parent().unwrap().join("ledger-remote");
        git(&remote_dir, &["add", "-A"]);
        git(&remote_dir, &["commit", "--quiet", "-m", "Another repo"]);
        git(
            &remote_dir,
            &[
                "push",
                "--quiet",
                self.nexus().to_str().unwrap(),
                &format!("HEAD:{}", NEXUS_REF),
            ],
        );
        self.forget_local_ledger();
    }
}

fn git_env() -> Vec<(&'static str, &'static str)> {
    vec![
        ("GIT_CONFIG_GLOBAL", "/dev/null"),
        ("GIT_CONFIG_NOSYSTEM", "1"),
        ("GIT_AUTHOR_NAME", "Test"),
        ("GIT_AUTHOR_EMAIL", "test@example.com"),
        ("GIT_COMMITTER_NAME", "Test"),
        ("GIT_COMMITTER_EMAIL", "test@example.com"),
        ("TSK_LEDGER_REFS", "branches"),
    ]
}

fn git_output(dir: &Path, args: &[&str]) -> Output {
    Command::new("git")
        .args(args)
        .current_dir(dir)
        .envs(git_env())
        .output()
        .expect("failed to run git")
}

fn git(dir: &Path, args: &[&str]) -> String {
    let output = git_output(dir, args);
    assert!(
        output.status.success(),
        "git {:?} failed in {}: {}",
        args,
        dir.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn git_line(dir: &Path, args: &[&str]) -> String {
    git(dir, args).trim_end().to_string()
}

fn commit_all(dir: &Path, message: &str) {
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "--quiet", "-m", message]);
}

fn write(path: &Path, content: &str) {
    std::fs::write(path, content).unwrap();
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

fn assert_failure_mentioning(output: &Output, needle: &str) {
    assert!(
        !output.status.success(),
        "tsk succeeded: {}",
        stdout(output)
    );
    assert!(
        stderr(output).contains(needle),
        "stderr lacks {:?}: {}",
        needle,
        stderr(output)
    );
}

#[test]
fn attach_nexus_writes_the_user_config_and_reports_each_change() {
    let fx = Fixture::new();

    let first = fx.tsk(&["config", "attach-nexus", "https://example.test/o/nexus"]);
    assert_success(&first);
    assert_eq!(
        stdout(&first),
        "attached nexus https://example.test/o/nexus\n"
    );
    let text = std::fs::read_to_string(fx.config_file()).unwrap();
    let table: toml::Table = text.parse().unwrap();
    assert_eq!(
        table["nexus"]["url"].as_str(),
        Some("https://example.test/o/nexus")
    );

    let again = fx.tsk(&["config", "attach-nexus", "https://example.test/o/nexus"]);
    assert_success(&again);
    assert_eq!(
        stdout(&again),
        "nexus already attached: https://example.test/o/nexus\n"
    );

    let replaced = fx.tsk(&["config", "attach-nexus", "https://example.test/o/other"]);
    assert_success(&replaced);
    assert_eq!(
        stdout(&replaced),
        "replaced nexus https://example.test/o/nexus with https://example.test/o/other\n"
    );
}

#[test]
fn config_show_prints_the_file_and_the_attached_nexus() {
    let fx = Fixture::new();
    let none = fx.tsk(&["config", "show"]);
    assert_success(&none);
    assert!(
        stdout(&none).contains("nexus: none attached"),
        "{}",
        stdout(&none)
    );

    fx.tsk(&["config", "attach-nexus", "https://example.test/o/nexus"]);
    let shown = fx.tsk(&["config", "show"]);
    assert_success(&shown);
    assert!(stdout(&shown).contains(fx.config_file().to_str().unwrap()));
    assert!(stdout(&shown).contains("nexus: https://example.test/o/nexus"));
}

#[test]
fn no_attached_nexus_keeps_the_ledger_in_the_repo() {
    let fx = Fixture::new();
    fx.set_nexus_json(WORK_API_ENTRY);

    fx.fetch_ok();
    fx.push_ok("Create the ledger");

    assert!(fx.ref_exists(&fx.managed(), IN_REPO_REF));
    assert!(!fx.ref_exists(&fx.nexus(), NEXUS_REF));
    assert!(!fx.repo_id_file().exists());
}

#[test]
fn an_entry_with_ledger_nexus_holds_the_ledger_in_the_nexus() {
    let fx = Fixture::new();
    fx.set_nexus_json(WORK_API_ENTRY);
    fx.attach();

    let first = fx.tsk(&["ledger", "fetch"]);
    assert_success(&first);
    assert!(
        stderr(&first).contains("the nexus's refs/heads/ledgers/work-api does not exist yet"),
        "{}",
        stderr(&first)
    );
    let path = PathBuf::from(stdout(&first).trim_end());
    assert!(path.ends_with("ledger-db"), "{}", path.display());
    assert_eq!(fx.manifest_repo_id(), "work-api");
    assert!(!fx.ref_exists(&fx.nexus(), NEXUS_REF));

    let doc = fx.root.path().join("doc.md");
    write(&doc, "mission\n");
    assert_success(&fx.tsk(&[
        "ledger",
        "doc",
        "put",
        "missions/M-1.md",
        doc.to_str().unwrap(),
    ]));
    let pushed = fx.tsk(&["ledger", "push", "First commit"]);
    assert_success(&pushed);

    assert!(fx.ref_exists(&fx.nexus(), NEXUS_REF));
    assert!(fx.ref_exists(&fx.nexus(), NEXUS_DOCS_REF));
    assert!(!fx.ref_exists(&fx.managed(), IN_REPO_REF));
    assert_eq!(
        fx.file_on(&fx.nexus(), NEXUS_DOCS_REF, "missions/M-1.md")
            .as_deref(),
        Some("mission\n")
    );
    assert!(fx.file_on(&fx.nexus(), NEXUS_REF, "manifest").is_some());
    assert_eq!(
        std::fs::read_to_string(fx.repo_id_file()).unwrap().trim(),
        "work-api"
    );
}

#[test]
fn the_documents_worktree_stays_a_linked_worktree_of_the_managed_clone() {
    let fx = Fixture::new();
    fx.set_nexus_json(WORK_API_ENTRY);
    fx.attach();

    let path = fx.fetch_ok();
    let docs = path.parent().unwrap().join("docs");

    let listing = git(&fx.work(), &["worktree", "list", "--porcelain", "-z"]);
    let canonical = std::fs::canonicalize(&docs).unwrap();
    assert!(listing
        .split('\0')
        .filter_map(|f| f.strip_prefix("worktree "))
        .any(|p| std::fs::canonicalize(p).ok() == Some(canonical.clone())));
    assert!(path.starts_with(fx.root.path().join("state/tsk/repos")));
}

#[test]
fn a_later_push_and_fetch_round_trip_through_the_nexus_branch() {
    let fx = Fixture::new();
    fx.set_nexus_json(WORK_API_ENTRY);
    fx.attach();
    let path = fx.fetch_ok();
    for (name, text) in [("one.md", "one\n"), ("two.md", "two\n")] {
        let doc = fx.root.path().join(name);
        write(&doc, text);
        assert_success(&fx.tsk(&["ledger", "doc", "put", name, doc.to_str().unwrap()]));
        fx.push_ok(name);
    }

    // The documents branch holds the seed commit and one commit for each push.
    assert_eq!(
        git_line(&fx.nexus(), &["rev-list", "--count", NEXUS_DOCS_REF]),
        "3"
    );
    let again = fx.fetch_ok();
    assert_eq!(again, path);
    assert_eq!(
        fx.docs_head(),
        git_line(&fx.nexus(), &["rev-parse", NEXUS_DOCS_REF])
    );
}

#[test]
fn an_existing_nexus_ledger_is_fetched() {
    let fx = Fixture::new();
    fx.set_nexus_json(WORK_API_ENTRY);
    fx.attach();
    fx.fetch_ok();
    let doc = fx.root.path().join("briefing.md");
    write(&doc, "# Mission: example\n");
    assert_success(&fx.tsk(&["ledger", "doc", "put", BRIEFING, doc.to_str().unwrap()]));
    fx.push_ok("Seed the nexus ledger");

    // A new clone ID stands for a clone that has never seen the ledger.
    fx.forget_local_ledger();
    let fetch = fx.tsk(&["ledger", "fetch"]);
    assert_success(&fetch);
    assert!(
        !stderr(&fetch).contains("does not exist"),
        "{}",
        stderr(&fetch)
    );

    assert_eq!(
        stdout(&fx.tsk(&["ledger", "doc", "get", BRIEFING])),
        "# Mission: example\n"
    );
    assert_eq!(
        fx.docs_head(),
        git_line(&fx.nexus(), &["rev-parse", NEXUS_DOCS_REF])
    );
}

#[test]
fn ledger_repo_and_a_missing_ledger_field_keep_the_ledger_in_the_repo() {
    for entry in [
        r#"{"id":"work-api","url":"https://example.test/acme/work-api","ledger":"repo"}"#,
        r#"{"id":"work-api","url":"https://example.test/acme/work-api"}"#,
        r#"{"id":"something-else","url":"https://example.test/acme/other","ledger":"nexus"}"#,
    ] {
        let fx = Fixture::new();
        fx.set_nexus_json(entry);
        fx.attach();

        fx.fetch_ok();
        fx.push_ok("Create the ledger");

        assert!(fx.ref_exists(&fx.managed(), IN_REPO_REF), "{}", entry);
        assert!(!fx.ref_exists(&fx.nexus(), "refs/heads/ledgers/something-else"));
        assert!(!fx.ref_exists(&fx.nexus(), NEXUS_REF), "{}", entry);
    }
}

#[test]
fn an_unknown_ledger_value_stops_with_an_error() {
    let fx = Fixture::new();
    fx.set_nexus_json(
        r#"{"id":"work-api","url":"https://example.test/acme/work-api","ledger":"cloud"}"#,
    );
    fx.attach();

    assert_failure_mentioning(&fx.tsk(&["ledger", "fetch"]), "ledger \"cloud\"");
}

#[test]
fn an_entry_is_found_through_https_and_ssh_forms_of_its_url() {
    for url in [
        "https://example.test/acme/work-api.git",
        "git@example.test:acme/work-api.git",
        "ssh://git@example.test:22/Acme/Work-API",
    ] {
        let fx = Fixture::new();
        fx.set_nexus_json(&format!(
            r#"{{"id":"work-api","url":"{}","ledger":"nexus"}}"#,
            url
        ));
        fx.attach();

        fx.fetch_ok();

        assert_eq!(fx.manifest_repo_id(), "work-api", "{}", url);
    }
}

#[test]
fn an_ssh_origin_matches_an_https_entry() {
    let fx = Fixture::new();
    git(
        &fx.work(),
        &[
            "config",
            "remote.origin.url",
            "git@example.test:acme/work-api.git",
        ],
    );
    fx.set_nexus_json(WORK_API_ENTRY);
    fx.attach();

    fx.fetch_ok();

    assert_eq!(fx.manifest_repo_id(), "work-api");
}

#[test]
fn the_cached_repo_id_is_used_before_the_origin_url() {
    let fx = Fixture::new();
    fx.set_nexus_json(WORK_API_ENTRY);
    fx.attach();
    fx.fetch_ok();
    fx.push_ok("Create the ledger");
    assert_eq!(
        std::fs::read_to_string(fx.repo_id_file()).unwrap().trim(),
        "work-api"
    );

    fx.set_nexus_json(
        r#"{"id":"work-api","url":"https://example.test/acme/renamed","ledger":"nexus"}"#,
    );
    fx.fetch_ok();

    assert_eq!(
        fx.docs_head(),
        git_line(&fx.nexus(), &["rev-parse", NEXUS_DOCS_REF])
    );
    assert!(!fx.ref_exists(&fx.managed(), IN_REPO_REF));
}

#[test]
fn an_entry_with_no_url_is_found_through_the_cached_id_on_its_machine() {
    let entry = r#"{"id":"work-api","local":"laptop","ledger":"nexus"}"#;
    let fx = Fixture::new();
    fx.set_nexus_json(entry);
    fx.attach();

    write(&fx.repo_id_file(), "work-api\n");
    fx.fetch_ok();
    assert_eq!(fx.manifest_repo_id(), "work-api");
    fx.push_ok("Create the ledger");
    assert!(fx.ref_exists(&fx.nexus(), NEXUS_REF));
}

#[test]
fn an_entry_with_no_url_and_no_cached_id_is_not_found() {
    let fx = Fixture::new();
    fx.set_nexus_json(r#"{"id":"work-api","local":"laptop","ledger":"nexus"}"#);
    fx.attach();

    fx.fetch_ok();
    fx.push_ok("Create the ledger");

    assert!(fx.ref_exists(&fx.managed(), IN_REPO_REF));
    assert!(!fx.ref_exists(&fx.nexus(), NEXUS_REF));
    assert!(!fx.repo_id_file().exists());
}

#[test]
fn an_entry_local_to_another_machine_is_skipped() {
    let fx = Fixture::new();
    fx.set_nexus_json(r#"{"id":"work-api","local":"desktop","ledger":"nexus"}"#);
    fx.attach();
    write(&fx.repo_id_file(), "work-api\n");

    let output = fx.tsk_as("laptop", &["ledger", "fetch"]);
    assert_success(&output);
    assert_eq!(fx.manifest_repo_id(), "");
    assert!(stderr(&output).contains("refs/heads/tsk/ledger does not exist yet"));

    fx.forget_local_ledger();
    let on_desktop = fx.tsk_as("desktop", &["ledger", "fetch"]);
    assert_success(&on_desktop);
    assert!(stderr(&on_desktop).contains("refs/heads/ledgers/work-api does not exist yet"));
}

#[test]
fn a_local_path_origin_keeps_the_ledger_in_the_repo_without_a_cached_id() {
    let fx = Fixture::new();
    git(
        &fx.work(),
        &[
            "config",
            "remote.origin.url",
            fx.managed().to_str().unwrap(),
        ],
    );
    fx.set_nexus_json(WORK_API_ENTRY);
    fx.attach();

    fx.fetch_ok();
    fx.push_ok("Create the ledger");

    assert!(fx.ref_exists(&fx.managed(), IN_REPO_REF));
    assert!(!fx.ref_exists(&fx.nexus(), NEXUS_REF));
}

#[test]
fn a_local_path_origin_uses_the_nexus_when_the_cached_id_names_an_entry() {
    let fx = Fixture::new();
    git(
        &fx.work(),
        &[
            "config",
            "remote.origin.url",
            fx.managed().to_str().unwrap(),
        ],
    );
    fx.set_nexus_json(r#"{"id":"work-api","local":"laptop","ledger":"nexus"}"#);
    fx.attach();
    write(&fx.repo_id_file(), "work-api\n");

    fx.fetch_ok();
    fx.push_ok("Create the ledger");

    assert!(fx.ref_exists(&fx.nexus(), NEXUS_REF));
    assert!(!fx.ref_exists(&fx.managed(), IN_REPO_REF));
}

#[test]
fn a_manifest_repo_id_that_differs_from_the_entry_id_stops_fetch_and_push() {
    let fx = Fixture::new();
    fx.set_nexus_json(WORK_API_ENTRY);
    fx.attach();
    fx.publish_ledger_of_another_repo();

    let output = fx.tsk(&["ledger", "fetch"]);

    assert_failure_mentioning(&output, "repo_id \"someone-else\"");
    assert!(stderr(&output).contains("\"work-api\""));
}

#[test]
fn an_invalid_repo_id_in_a_nexus_entry_stops_with_an_error() {
    let fx = Fixture::new();
    fx.set_nexus_json(
        r#"{"id":"Work_API","url":"https://example.test/acme/work-api","ledger":"nexus"}"#,
    );
    fx.attach();

    assert_failure_mentioning(&fx.tsk(&["ledger", "fetch"]), "not a valid repo id");
}

#[test]
fn an_attached_nexus_that_cannot_be_read_stops_with_an_error() {
    let fx = Fixture::new();
    let output = fx.tsk(&["config", "attach-nexus", "/nonexistent/nexus.git"]);
    assert_success(&output);

    assert_failure_mentioning(&fx.tsk(&["ledger", "fetch"]), "could not fetch the nexus");
}

#[test]
fn the_held_copy_of_the_nexus_resolves_the_location_when_a_refresh_fails() {
    let fx = Fixture::new();
    fx.set_nexus_json(WORK_API_ENTRY);
    fx.attach();
    fx.fetch_ok();

    let moved = fx.root.path().join("nexus-moved.git");
    std::fs::rename(fx.nexus(), &moved).unwrap();
    let output = fx.tsk(&["ledger", "fetch"]);

    assert_failure_mentioning(&output, "reading the copy held in");
    assert_failure_mentioning(&output, "git ls-remote");
    assert!(stderr(&output).contains("refs/heads/ledgers/work-api"));
}

#[test]
fn threads_pause_and_resume_against_a_nexus_ledger() {
    let fx = Fixture::new();
    fx.set_nexus_json(WORK_API_ENTRY);
    fx.attach();
    fx.fetch_ok();
    let doc = fx.root.path().join("briefing.md");
    write(&doc, "# Mission: example\n");
    assert_success(&fx.tsk(&["ledger", "doc", "put", BRIEFING, doc.to_str().unwrap()]));
    fx.push_ok("Add the briefing");

    let started = fx.tsk(&["thread", "start", "M-TEST-01", BRIEFING]);
    assert_success(&started);
    let id = stdout(&started)
        .trim_end()
        .strip_prefix("started:")
        .unwrap()
        .to_string();

    let head = git_line(&fx.work(), &["rev-parse", "HEAD"]);
    let paused = fx.tsk(&["thread", "pause", &id, "T-01", "Carry on"]);
    assert_success(&paused);
    let stored = fx.sql(&format!(
        "select entry from continuation_entries where thread_id = '{}'",
        id
    ));
    let entry_text = stored
        .lines()
        .nth(1)
        .unwrap()
        .trim_matches('"')
        .replace("\"\"", "\"");
    let entry: serde_json::Value = serde_json::from_str(&entry_text).unwrap();
    assert_eq!(entry["task_id"], "T-01");
    assert_eq!(entry["git"]["code"]["commit"], head.as_str());
    assert!(!entry["git"]["ledger"]["commit"]
        .as_str()
        .unwrap()
        .is_empty());
    assert!(!fx.ref_exists(&fx.managed(), IN_REPO_REF));

    let detached = fx.tsk(&["thread", "detach"]);
    assert_success(&detached);
    let resumed = fx.tsk(&["thread", "resume", &id]);
    assert_success(&resumed);
    assert!(
        stdout(&resumed).contains("Carry on"),
        "{}",
        stdout(&resumed)
    );
}

#[test]
fn events_append_and_read_new_against_a_nexus_ledger() {
    let fx = Fixture::new();
    fx.set_nexus_json(WORK_API_ENTRY);
    fx.attach();
    fx.fetch_ok();
    let payload = fx.root.path().join("payload.json");
    write(&payload, "{\"id\": 1}");

    let appended = fx.tsk(&[
        "events",
        "append",
        "github",
        "dependabot_alert",
        "created",
        "acme/work-api",
        payload.to_str().unwrap(),
    ]);
    assert_success(&appended);

    assert_eq!(
        fx.sql("select count(*) as c from external_events")
            .lines()
            .nth(1),
        Some("1")
    );
    assert!(fx.ref_exists(&fx.nexus(), NEXUS_REF));
    let unread = fx.tsk(&["events", "read-new"]);
    assert_success(&unread);
    let unread: serde_json::Value = serde_json::from_str(&stdout(&unread)).unwrap();
    assert_eq!(unread["new_count"], 1);
    assert!(!fx.ref_exists(&fx.managed(), IN_REPO_REF));
}

#[test]
fn a_nexus_ledger_is_created_when_the_first_command_is_a_thread_start() {
    let fx = Fixture::new();
    fx.set_nexus_json(WORK_API_ENTRY);
    fx.attach();

    let output = fx.tsk(&["thread", "start", "M-TEST-01"]);

    assert_failure_mentioning(&output, "M-TEST-01");
}

#[test]
fn a_dolt_nexus_ledger_keeps_its_documents_on_the_id_docs_branch() {
    let dolt = Command::new("dolt")
        .arg("version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !dolt {
        eprintln!("note: dolt is not installed; skipping");
        return;
    }
    let fx = Fixture::new();
    fx.set_nexus_json(WORK_API_ENTRY);
    fx.attach();

    let fetch = fx.tsk(&["ledger", "fetch"]);
    assert_success(&fetch);
    let doc = fx.root.path().join("doc.md");
    write(&doc, "in the nexus\n");
    assert_success(&fx.tsk(&[
        "ledger",
        "doc",
        "put",
        "missions/M-1.md",
        doc.to_str().unwrap(),
    ]));
    let pushed = fx.tsk(&["ledger", "push", "First commit"]);
    assert_success(&pushed);

    // The ref is `ledgers/<id>-docs`, never `ledgers/<id>/docs`, which cannot exist
    // beside the ledger ref `ledgers/<id>`.
    assert!(fx.ref_exists(&fx.nexus(), NEXUS_REF));
    assert!(fx.ref_exists(&fx.nexus(), NEXUS_DOCS_REF));
    assert!(!fx.ref_exists(&fx.nexus(), "refs/heads/ledgers/work-api/docs"));
    assert!(!fx.ref_exists(&fx.managed(), "refs/heads/tsk/docs"));
    assert_eq!(
        fx.file_on(&fx.nexus(), NEXUS_DOCS_REF, "missions/M-1.md")
            .as_deref(),
        Some("in the nexus\n")
    );
    assert_eq!(
        fx.file_on(&fx.nexus(), NEXUS_DOCS_REF, ".tsk-ledger.toml"),
        None
    );
    assert!(fx.file_on(&fx.nexus(), NEXUS_REF, "manifest").is_some());
    let get = fx.tsk(&["ledger", "doc", "get", "missions/M-1.md"]);
    assert_eq!(stdout(&get), "in the nexus\n");
}
