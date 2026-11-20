use std::path::{Path, PathBuf};
use std::process::Output;

use tempfile::TempDir;

use crate::common;

use common::templates;

const IN_REPO_REF: &str = "refs/heads/tsk/ledger";
const NEXUS_REF: &str = "refs/heads/ledgers/work-api/ledger";
const NEXUS_DOCS_REF: &str = "refs/heads/ledgers/work-api/docs";
const ORIGIN_URL: &str = "https://example.test/acme/work-api";
const BRIEFING: &str = "missions/operational/M-TEST-01-example.1c0de060.md";
const WORK_API_ENTRY: &str =
    r#"{"id":"work-api","url":"https://example.test/acme/work-api","ledger":"nexus"}"#;

struct Fixture {
    root: TempDir,
}

impl templates::Root for Fixture {
    fn root(&self) -> &Path {
        self.root.path()
    }
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
        common::seed_dolt_home(&root.join("home"));
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

    /// A fixture whose nexus holds the entry `work-api` with `ledger: nexus`, attached, with
    /// the ledger fetched. The build runs once per test process (see `common::templates`),
    /// and each test gets a copy. A test whose subject is the first fetch builds its own.
    fn attached_and_fetched() -> Fixture {
        Fixture::template("attached-and-fetched", |fx| {
            fx.set_nexus_json(WORK_API_ENTRY);
            fx.attach();
            fx.fetch_ok();
        })
    }

    /// [`Fixture::attached_and_fetched`] with the briefing pushed to the nexus ledger.
    fn attached_and_seeded() -> Fixture {
        Fixture::template("attached-and-seeded", |fx| {
            fx.set_nexus_json(WORK_API_ENTRY);
            fx.attach();
            fx.fetch_ok();
            let doc = fx.root.path().join("briefing.md");
            write(&doc, "# Mission: example\n");
            assert_success(&fx.tsk(&["ledger", "doc", "put", BRIEFING, doc.to_str().unwrap()]));
            fx.push_ok("Seed the nexus ledger");
        })
    }

    fn template(key: &str, build: impl FnOnce(&Fixture)) -> Fixture {
        let (root, _) = templates::cached(key, || {
            let fx = Fixture::new();
            build(&fx);
            (fx, String::new())
        });
        Fixture { root }
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
        // The nexus may have moved: tsk marks an entry at a publish.
        sync_to_origin(&self.nexus_seed());
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
        self.tsk_as_env(machine, args, &[])
    }

    fn tsk_env(&self, args: &[&str], env: &[(&str, &str)]) -> Output {
        self.tsk_as_env("laptop", args, env)
    }

    fn tsk_as_env(&self, machine: &str, args: &[&str], env: &[(&str, &str)]) -> Output {
        crate::common::cmd(env!("CARGO_BIN_EXE_tsk"))
            .args(args)
            .env_remove("GIT_AUTHOR_NAME")
            .env_remove("TSK_SESSION")
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
            .envs(env.iter().copied())
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
        let out = crate::common::cmd("dolt")
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

    /// Forget this clone's ledger: the home is removed, so the next fetch builds a new
    /// database from what the remote holds.
    fn forget_local_ledger(&self) {
        let home = common::ledger_home(&self.root.path().join("state"), &self.work());
        common::stop_servers(&home, true);
        let _ = std::fs::remove_dir_all(&home);
        let _ = std::fs::remove_file(self.work().join(".git").join("tsk-ledger-home"));
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
            let out = crate::common::cmd("dolt")
                .args(&args)
                .current_dir(&path)
                .env("HOME", self.root.path().join("home"))
                .env("DOLT_DISABLE_EVENT_FLUSH", "1")
                .output()
                .unwrap();
            assert!(out.status.success(), "{:?}: {}", args, stderr(&out));
        }
        // The ledger is a Dolt file remote carried by plain git: publish the directory.
        let remote_dir = path.parent().unwrap().join("ledger");
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

/// Bring a seed clone to what its origin holds, when it holds a `main`.
fn sync_to_origin(seed: &Path) {
    if git_output(seed, &["fetch", "--quiet", "origin", "main"])
        .status
        .success()
    {
        git(seed, &["reset", "--quiet", "--hard", "FETCH_HEAD"]);
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
    crate::common::cmd("git")
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
        stderr(&first)
            .contains("the nexus's refs/heads/ledgers/work-api/ledger does not exist yet"),
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
        "missions/M-1.1c0de050.md",
        doc.to_str().unwrap(),
    ]));
    let pushed = fx.tsk(&["ledger", "push", "First commit"]);
    assert_success(&pushed);

    assert!(fx.ref_exists(&fx.nexus(), NEXUS_REF));
    assert!(fx.ref_exists(&fx.nexus(), NEXUS_DOCS_REF));
    assert!(!fx.ref_exists(&fx.managed(), IN_REPO_REF));
    assert_eq!(
        fx.file_on(&fx.nexus(), NEXUS_DOCS_REF, "missions/M-1.1c0de050.md")
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
fn the_documents_worktree_belongs_to_the_ledger_clone_and_the_code_repo_lists_none() {
    let fx = Fixture::new();
    fx.set_nexus_json(WORK_API_ENTRY);
    fx.attach();

    let path = fx.fetch_ok();
    let docs = path.parent().unwrap().join("docs");
    let canonical = std::fs::canonicalize(&docs).unwrap();
    let lists = |dir: &Path| {
        git(dir, &["worktree", "list", "--porcelain", "-z"])
            .split('\0')
            .filter_map(|f| f.strip_prefix("worktree ").map(str::to_string))
            .any(|p| std::fs::canonicalize(p).ok() == Some(canonical.clone()))
    };
    assert!(!lists(&fx.work()));
    assert!(lists(&path.parent().unwrap().join("git")));
    // One home per repo, keyed by the nexus entry id.
    assert_eq!(
        path,
        fx.root.path().join("state/tsk/ledgers/work-api/ledger-db")
    );
    // The code repo holds no ledger refs and no ledger objects.
    assert_eq!(git(&fx.work(), &["for-each-ref", "refs/tsk-fetch"]), "");
    assert_eq!(
        std::fs::read_to_string(fx.work().join(".git/tsk-ledger-home"))
            .unwrap()
            .trim(),
        "work-api"
    );
}

#[test]
fn a_later_push_and_fetch_round_trip_through_the_nexus_branch() {
    let fx = Fixture::attached_and_fetched();
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
    let fx = Fixture::attached_and_seeded();

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
    let fx = Fixture::attached_and_seeded();
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

    assert_failure_mentioning(&fx.tsk(&["ledger", "fetch"]), "has no entry for this repo");
    assert!(!fx.ref_exists(&fx.managed(), IN_REPO_REF));
    assert!(!fx.ref_exists(&fx.nexus(), NEXUS_REF));
    assert!(!fx.repo_id_file().exists());
}

const OTHER_ENTRY: &str = r#"{"id":"something-else","url":"https://example.test/acme/other","ledger":"nexus","prefix":"OTH"}"#;

#[test]
fn a_repo_the_nexus_does_not_list_stops_and_creates_no_ledger() {
    let fx = Fixture::new();
    fx.set_nexus_json(OTHER_ENTRY);
    fx.attach();

    let fetched = fx.tsk(&["ledger", "fetch"]);
    assert_failure_mentioning(&fetched, "has no entry for this repo");
    assert!(stderr(&fetched).contains("tsk ledger init --nexus or --repo"));
    assert!(!fx.ref_exists(&fx.managed(), IN_REPO_REF));
    assert!(!fx.ref_exists(&fx.nexus(), "refs/heads/ledgers/something-else"));
}

#[test]
fn a_repo_the_nexus_does_not_list_keeps_the_ledger_origin_holds() {
    let fx = Fixture::new();
    fx.fetch_ok();
    fx.push_ok("Create the ledger");
    fx.set_nexus_json(OTHER_ENTRY);
    fx.attach();
    let state = fx.root.path().join("state");
    common::stop_servers(fx.root.path(), true);
    std::fs::remove_dir_all(&state).unwrap();
    std::fs::create_dir_all(&state).unwrap();

    fx.fetch_ok();
    assert!(fx.ref_exists(&fx.managed(), IN_REPO_REF));
}

#[test]
fn status_with_no_ledger_reports_the_nexus_entry_and_the_next_action() {
    let fx = Fixture::new();
    fx.set_nexus_json(WORK_API_ENTRY);
    fx.attach();

    let status = fx.tsk(&["ledger", "status"]);
    assert_success(&status);
    let text = stdout(&status);
    assert!(text.contains("Local   no ledger in this clone"), "{}", text);
    assert!(
        text.contains(": entry work-api, ledger in the nexus"),
        "{}",
        text
    );
    assert!(text.contains("Next    run tsk ledger fetch"), "{}", text);
    assert!(!fx.ref_exists(&fx.nexus(), NEXUS_REF));

    fx.set_nexus_json(OTHER_ENTRY);
    let porcelain = fx.tsk(&["ledger", "status", "--porcelain"]);
    assert_success(&porcelain);
    assert_eq!(
        stdout(&porcelain),
        format!(
            "local\tnone\nnexus\t{}\nnexus-state\tno-entry\norigin\tno-ledger\nnext\tinit\n",
            fx.nexus().display()
        )
    );
    let local = fx.tsk(&["ledger", "status", "--local"]);
    assert_eq!(stdout(&local), "Local   no ledger in this clone\n");
}

#[test]
fn status_with_an_unreachable_nexus_names_add_repo_only_in_a_cloud_session() {
    let fx = Fixture::new();
    let missing = fx.root.path().join("missing.git");
    assert_success(&fx.tsk(&["config", "attach-nexus", missing.to_str().unwrap()]));

    let run = |cloud: bool| {
        let mut command = crate::common::cmd(env!("CARGO_BIN_EXE_tsk"));
        command
            .args(["ledger", "status"])
            .current_dir(fx.work())
            .envs(git_env())
            .env("HOME", fx.root.path().join("home"))
            .env("XDG_STATE_HOME", fx.root.path().join("state"))
            .env("XDG_CONFIG_HOME", fx.root.path().join("config"))
            .env("GIT_CEILING_DIRECTORIES", fx.root.path());
        if cloud {
            command.env("CLAUDE_CODE_REMOTE", "true");
        } else {
            command.env_remove("CLAUDE_CODE_REMOTE");
        }
        command.output().unwrap()
    };
    let terminal = stdout(&run(false));
    assert!(terminal.contains("cannot be fetched"), "{}", terminal);
    assert!(
        terminal.contains("check that git on this machine can fetch"),
        "{}",
        terminal
    );
    assert!(!terminal.contains("add_repo"), "{}", terminal);

    let cloud = run(true);
    assert!(
        stdout(&cloud).contains("attach the nexus repository"),
        "{}",
        stdout(&cloud)
    );
}

#[test]
fn init_nexus_adds_the_repo_and_creates_its_ledger_in_the_nexus() {
    let fx = Fixture::new();
    fx.set_nexus_json(OTHER_ENTRY);
    fx.attach();

    let missing = fx.tsk(&["ledger", "init", "--nexus"]);
    assert_failure_mentioning(&missing, "--prefix is required");
    let taken = fx.tsk(&["ledger", "init", "--nexus", "--prefix", "OTH"]);
    assert_failure_mentioning(&taken, "already used by the repo \"something-else\"");

    let created = fx.tsk(&["ledger", "init", "--nexus", "--prefix", "WAPI"]);
    assert_success(&created);
    assert!(stdout(&created).trim_end().ends_with("ledger-db"));
    assert!(fx.ref_exists(&fx.nexus(), NEXUS_REF));
    assert!(!fx.ref_exists(&fx.managed(), IN_REPO_REF));
    let index = fx
        .file_on(&fx.nexus(), "refs/heads/main", "nexus.json")
        .unwrap();
    assert!(index.contains("\"work-api\""), "{}", index);
    assert!(index.contains("\"WAPI\""), "{}", index);
    assert_eq!(stdout(&fx.tsk(&["mission", "prefix"])), "WAPI\n");

    let again = fx.tsk(&["ledger", "init", "--nexus", "--prefix", "WAPI"]);
    assert_failure_mentioning(&again, "holds a ledger already");
}

impl Fixture {
    fn set_empty_nexus(&self) {
        sync_to_origin(&self.nexus_seed());
        write(
            &self.nexus_seed().join("nexus.json"),
            "{\"version\":1,\"territories\":[]}\n",
        );
        commit_all(&self.nexus_seed(), "Empty nexus.json");
        git(
            &self.nexus_seed(),
            &["push", "--quiet", "origin", "HEAD:refs/heads/main"],
        );
    }
}

#[test]
fn init_nexus_creates_a_territory_with_its_name() {
    let fx = Fixture::new();
    fx.set_empty_nexus();
    fx.attach();

    let missing = fx.tsk(&["ledger", "init", "--nexus", "--prefix", "WAPI"]);
    assert_failure_mentioning(&missing, "--territory");
    let created = fx.tsk(&[
        "ledger",
        "init",
        "--nexus",
        "--prefix",
        "WAPI",
        "--territory",
        "work",
        "--territory-name",
        "Work",
    ]);
    assert_success(&created);
    let index = nexus_json(&fx);
    assert_eq!(index["territories"][0]["id"], "work");
    assert_eq!(index["territories"][0]["name"], "Work");
}

#[test]
fn init_nexus_names_a_new_territory_after_its_id_without_a_name() {
    let fx = Fixture::new();
    fx.set_empty_nexus();
    fx.attach();

    let created = fx.tsk(&[
        "ledger",
        "init",
        "--nexus",
        "--prefix",
        "WAPI",
        "--territory",
        "work",
    ]);
    assert_success(&created);
    assert_eq!(nexus_json(&fx)["territories"][0]["name"], "work");
}

#[test]
fn init_territory_name_needs_a_territory() {
    let fx = Fixture::new();
    fx.set_nexus_json(OTHER_ENTRY);
    fx.attach();

    let output = fx.tsk(&[
        "ledger",
        "init",
        "--nexus",
        "--prefix",
        "WAPI",
        "--territory-name",
        "Work",
    ]);
    assert_failure_mentioning(&output, "--territory");
}

#[test]
fn init_nexus_writes_the_workflow_to_the_new_entry() {
    let fx = Fixture::new();
    fx.set_nexus_json(OTHER_ENTRY);
    fx.attach();

    let created = fx.tsk(&[
        "ledger",
        "init",
        "--nexus",
        "--prefix",
        "WAPI",
        "--workflow",
        "pr",
    ]);
    assert_success(&created);
    let index = fx
        .file_on(&fx.nexus(), "refs/heads/main", "nexus.json")
        .unwrap();
    assert!(
        index.contains("\"workflow\": \"pull-request\""),
        "{}",
        index
    );
    let status = stdout(&fx.tsk(&["ledger", "status"]));
    assert!(status.contains("workflow: pull-request"), "{}", status);
}

#[test]
fn init_without_a_workflow_and_with_no_terminal_leaves_it_unset() {
    let fx = Fixture::new();
    fx.set_nexus_json(OTHER_ENTRY);
    fx.attach();

    assert_success(&fx.tsk(&["ledger", "init", "--nexus", "--prefix", "WAPI"]));
    let index = fx
        .file_on(&fx.nexus(), "refs/heads/main", "nexus.json")
        .unwrap();
    assert!(!index.contains("workflow"), "{}", index);
    let status = stdout(&fx.tsk(&["ledger", "status"]));
    assert!(status.contains("workflow: not set"), "{}", status);
}

#[test]
fn init_rejects_a_workflow_that_is_not_main_or_pr() {
    let fx = Fixture::new();
    fx.set_nexus_json(OTHER_ENTRY);
    fx.attach();

    let output = fx.tsk(&[
        "ledger",
        "init",
        "--nexus",
        "--prefix",
        "WAPI",
        "--workflow",
        "trunk",
    ]);
    assert_failure_mentioning(&output, "not a push workflow");
    assert!(!fx.ref_exists(&fx.nexus(), NEXUS_REF));
}

#[test]
fn init_repo_adds_the_repo_with_ledger_repo_and_creates_its_ledger_on_origin() {
    let fx = Fixture::new();
    fx.set_nexus_json(OTHER_ENTRY);
    fx.attach();

    let created = fx.tsk(&["ledger", "init", "--repo", "--prefix", "WAPI"]);
    assert_success(&created);
    assert!(fx.ref_exists(&fx.managed(), IN_REPO_REF));
    assert!(!fx.ref_exists(&fx.nexus(), NEXUS_REF));
    let index = fx
        .file_on(&fx.nexus(), "refs/heads/main", "nexus.json")
        .unwrap();
    assert!(index.contains("\"ledger\": \"repo\""), "{}", index);
}

#[test]
fn init_with_no_nexus_creates_the_ledger_on_origin() {
    let fx = Fixture::new();

    assert_failure_mentioning(
        &fx.tsk(&["ledger", "init", "--nexus"]),
        "no nexus is attached",
    );
    let created = fx.tsk(&["ledger", "init", "--repo"]);
    assert_success(&created);
    assert!(fx.ref_exists(&fx.managed(), IN_REPO_REF));
}

#[test]
fn an_entry_local_to_another_machine_is_skipped() {
    let fx = Fixture::new();
    fx.set_nexus_json(r#"{"id":"work-api","local":"desktop","ledger":"nexus"}"#);
    fx.attach();
    write(&fx.repo_id_file(), "work-api\n");

    let output = fx.tsk_as("laptop", &["ledger", "fetch"]);
    assert_failure_mentioning(&output, "has no entry for this repo");

    let on_desktop = fx.tsk_as("desktop", &["ledger", "fetch"]);
    assert_success(&on_desktop);
    assert!(stderr(&on_desktop).contains("refs/heads/ledgers/work-api/ledger does not exist yet"));
}

#[test]
fn a_local_path_origin_is_not_matched_without_a_cached_id() {
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

    assert_failure_mentioning(&fx.tsk(&["ledger", "fetch"]), "has no entry for this repo");
    assert!(!fx.ref_exists(&fx.managed(), IN_REPO_REF));
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
    let fx = Fixture::attached_and_fetched();

    let moved = fx.root.path().join("nexus-moved.git");
    std::fs::rename(fx.nexus(), &moved).unwrap();
    let output = fx.tsk(&["ledger", "fetch"]);

    assert_failure_mentioning(&output, "reading the copy held in");
    assert_failure_mentioning(&output, "git ls-remote");
    assert!(stderr(&output).contains("refs/heads/ledgers/work-api"));
}

#[test]
fn threads_pause_and_resume_against_a_nexus_ledger() {
    let fx = Fixture::attached_and_seeded();

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
    let fx = Fixture::attached_and_fetched();
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
    if !common::dolt_installed() {
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
        "missions/M-1.1c0de050.md",
        doc.to_str().unwrap(),
    ]));
    let pushed = fx.tsk(&["ledger", "push", "First commit"]);
    assert_success(&pushed);

    // Both refs live under `ledgers/<id>/`, and the names of tsk 0.69.0 and earlier are absent.
    assert!(fx.ref_exists(&fx.nexus(), NEXUS_REF));
    assert!(fx.ref_exists(&fx.nexus(), NEXUS_DOCS_REF));
    assert!(!fx.ref_exists(&fx.nexus(), "refs/heads/ledgers/work-api"));
    assert!(!fx.ref_exists(&fx.nexus(), "refs/heads/ledgers/work-api-docs"));
    assert!(!fx.ref_exists(&fx.managed(), "refs/heads/tsk/docs"));
    assert_eq!(
        fx.file_on(&fx.nexus(), NEXUS_DOCS_REF, "missions/M-1.1c0de050.md")
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

const IN_REPO_DOCS_REF: &str = "refs/heads/tsk/docs";
const REPO_ENTRY: &str = r#"{"id":"work-api","url":"https://example.test/acme/work-api"}"#;

/// A repo ledger on origin with one document pushed, and a nexus that lists the repo with
/// its ledger in the repo.
fn repo_ledger_seeded() -> Fixture {
    Fixture::template("repo-ledger-seeded", |fx| {
        fx.set_nexus_json(REPO_ENTRY);
        fx.attach();
        fx.fetch_ok();
        assert_success(&fx.tsk(&["mission", "prefix", "WORK"]));
        let doc = fx.root.path().join("briefing.md");
        write(&doc, "# Mission: example\n");
        assert_success(&fx.tsk(&["ledger", "doc", "put", BRIEFING, doc.to_str().unwrap()]));
        fx.push_ok("Seed the repo ledger");
    })
}

fn nexus_json(fx: &Fixture) -> serde_json::Value {
    serde_json::from_str(
        &fx.file_on(&fx.nexus(), "refs/heads/main", "nexus.json")
            .unwrap(),
    )
    .unwrap()
}

#[test]
fn move_ledger_copies_both_refs_and_points_the_entry_at_the_nexus() {
    let fx = repo_ledger_seeded();
    let data = git_line(&fx.managed(), &["rev-parse", IN_REPO_REF]);
    let docs = git_line(&fx.managed(), &["rev-parse", IN_REPO_DOCS_REF]);

    let moved = fx.tsk(&["nexus", "move-ledger", "--ledger-refs", "branches"]);
    assert_success(&moved);
    let out = stdout(&moved);
    assert!(
        out.contains("git push origin --delete refs/heads/tsk/ledger refs/heads/tsk/docs"),
        "{}",
        out
    );

    assert_eq!(git_line(&fx.nexus(), &["rev-parse", NEXUS_REF]), data);
    assert_eq!(git_line(&fx.nexus(), &["rev-parse", NEXUS_DOCS_REF]), docs);
    let entry = &nexus_json(&fx)["territories"][0]["repos"][0];
    assert_eq!(entry["ledger"], "nexus");
    // The entry is written at the new ref layout: the mode, then the line that makes an older
    // tsk stop and update itself.
    let marker = entry["ledger_refs"].as_str().unwrap();
    assert!(
        marker.starts_with("branches-v2\nerror: unsupported ledger version 16 in "),
        "{}",
        marker
    );
    assert_eq!(entry["prefix"], "WORK");

    // The home of the ledger in the repo became the home of the ledger in the nexus, and the
    // code repo holds no ledger worktree or ref.
    let state = fx.root.path().join("state");
    let home = state.join("tsk/ledgers/work-api");
    assert!(home.join("ledger-db/.dolt").is_dir());
    assert_eq!(
        std::fs::read_to_string(fx.work().join(".git/tsk-ledger-home"))
            .unwrap()
            .trim(),
        "work-api"
    );
    let old_homes: Vec<_> = std::fs::read_dir(state.join("tsk/ledgers"))
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with('_'))
        .collect();
    assert!(old_homes.is_empty(), "{:?}", old_homes);
    assert_eq!(git(&fx.work(), &["worktree", "list"]).lines().count(), 1);
    assert_eq!(git(&fx.work(), &["for-each-ref", "refs/tsk-fetch"]), "");

    // The same clone reads the moved ledger and publishes to the nexus.
    assert_eq!(
        stdout(&fx.tsk(&["ledger", "doc", "get", BRIEFING])),
        "# Mission: example\n"
    );
    let two = fx.root.path().join("two.md");
    write(&two, "two\n");
    assert_success(&fx.tsk(&[
        "ledger",
        "doc",
        "put",
        "notes/two.md",
        two.to_str().unwrap(),
    ]));
    fx.push_ok("After the move");
    assert_ne!(git_line(&fx.nexus(), &["rev-parse", NEXUS_DOCS_REF]), docs);
    assert_eq!(git_line(&fx.managed(), &["rev-parse", IN_REPO_REF]), data);
    assert_eq!(
        git_line(&fx.managed(), &["rev-parse", IN_REPO_DOCS_REF]),
        docs
    );

    // A clone that never saw the ledger fetches it from the nexus.
    fx.forget_local_ledger();
    fx.fetch_ok();
    assert_eq!(
        stdout(&fx.tsk(&["ledger", "doc", "get", "notes/two.md"])),
        "two\n"
    );

    let again = fx.tsk(&["nexus", "move-ledger"]);
    assert!(!again.status.success());
    assert!(
        stderr(&again).contains("already held in the nexus"),
        "{}",
        stderr(&again)
    );
}

#[test]
fn move_ledger_refuses_while_the_store_holds_unpushed_changes() {
    let fx = repo_ledger_seeded();
    let doc = fx.root.path().join("local.md");
    write(&doc, "local\n");
    assert_success(&fx.tsk(&[
        "ledger",
        "doc",
        "put",
        "notes/local.md",
        doc.to_str().unwrap(),
    ]));

    let moved = fx.tsk(&["nexus", "move-ledger"]);
    assert!(!moved.status.success());
    assert!(
        stderr(&moved).contains("run tsk ledger push first"),
        "{}",
        stderr(&moved)
    );
    assert!(!fx.ref_exists(&fx.nexus(), NEXUS_REF));
    assert_eq!(
        nexus_json(&fx)["territories"][0]["repos"][0]["ledger"],
        serde_json::Value::Null
    );
}

#[test]
fn move_ledger_does_not_overwrite_a_different_ledger_in_the_nexus() {
    let fx = repo_ledger_seeded();
    git(
        &fx.managed(),
        &[
            "push",
            "--quiet",
            fx.nexus().to_str().unwrap(),
            &format!("refs/heads/main:{}", NEXUS_REF),
        ],
    );

    let moved = fx.tsk(&["nexus", "move-ledger", "--ledger-refs", "branches"]);
    assert!(!moved.status.success());
    assert!(
        stderr(&moved).contains("tsk does not overwrite a ledger"),
        "{}",
        stderr(&moved)
    );
    assert_eq!(
        nexus_json(&fx)["territories"][0]["repos"][0]["ledger"],
        serde_json::Value::Null
    );
}

#[test]
fn init_takes_the_prefix_of_an_entry_that_holds_one() {
    let fx = Fixture::new();
    fx.set_nexus_json(
        r#"{"id":"work-api","url":"https://example.test/acme/work-api","ledger":"repo","prefix":"WAPI"}"#,
    );
    fx.attach();

    assert_success(&fx.tsk(&["ledger", "init", "--repo"]));
    assert_eq!(stdout(&fx.tsk(&["mission", "prefix"])), "WAPI\n");
}

#[test]
fn init_with_a_robot_identity_and_no_robot_set_up_leaves_the_nexus_untouched() {
    let fx = Fixture::new();
    fx.set_nexus_json(OTHER_ENTRY);
    fx.attach();
    let before = git_line(&fx.nexus(), &["rev-parse", "refs/heads/main"]);

    let output = crate::common::cmd(env!("CARGO_BIN_EXE_tsk"))
        .args(["ledger", "init", "--nexus", "--prefix", "WAPI"])
        .current_dir(fx.work())
        .envs(git_env())
        .env("HOME", fx.root.path().join("home"))
        .env("XDG_STATE_HOME", fx.root.path().join("state"))
        .env("XDG_CONFIG_HOME", fx.root.path().join("config"))
        .env("TSK_MACHINE_NAME", "laptop")
        .env("TSK_HOME", fx.root.path().join("home/.tsk"))
        .env("TSK_ATTRIBUTION_IDENTITY", "robot")
        .env("GIT_CEILING_DIRECTORIES", fx.root.path())
        .output()
        .unwrap();
    assert_failure_mentioning(&output, "nothing was changed");
    assert_eq!(
        git_line(&fx.nexus(), &["rev-parse", "refs/heads/main"]),
        before
    );
}

fn robot_session(fx: &Fixture) -> Vec<(String, String)> {
    let key = fx.root.path().join("robot-key");
    let generated = crate::common::cmd("ssh-keygen")
        .args(["-q", "-t", "ed25519", "-N", "", "-f"])
        .arg(&key)
        .output()
        .expect("failed to run ssh-keygen");
    assert!(generated.status.success());
    let private = std::fs::read_to_string(&key).unwrap();
    let env = vec![
        ("TSK_ATTRIBUTION_IDENTITY".to_string(), "robot".to_string()),
        (
            "TSK_ATTRIBUTION_EMAIL".to_string(),
            "robots@example.test".to_string(),
        ),
        ("TSK_ROBOT_SIGNING_KEY".to_string(), private),
    ];
    let pairs: Vec<(&str, &str)> = env.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    assert_success(&fx.tsk_env(&["thread", "session-start"], &pairs));
    env
}

#[test]
fn init_with_a_robot_identity_and_no_robot_name_stops_and_names_the_way_out() {
    let fx = Fixture::new();
    fx.set_nexus_json(OTHER_ENTRY);
    fx.attach();
    let env = robot_session(&fx);
    let pairs: Vec<(&str, &str)> = env.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let before = git_line(&fx.nexus(), &["rev-parse", "refs/heads/main"]);

    let output = fx.tsk_env(&["ledger", "init", "--nexus", "--prefix", "WAPI"], &pairs);
    assert_failure_mentioning(&output, "tsk actor register \"<name>\" --kind robot");
    assert_failure_mentioning(&output, "construct-robot");
    assert_failure_mentioning(&output, "nothing was changed");
    assert_eq!(
        git_line(&fx.nexus(), &["rev-parse", "refs/heads/main"]),
        before
    );
}

#[test]
fn init_with_a_robot_identity_registers_the_robot_of_the_session_and_sets_the_prefix() {
    let fx = Fixture::new();
    fx.set_nexus_json(OTHER_ENTRY);
    fx.attach();
    let env = robot_session(&fx);
    let pairs: Vec<(&str, &str)> = env.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    write(&fx.work().join(".git/tsk-robot-name"), "Marten\n");

    let output = fx.tsk_env(&["ledger", "init", "--nexus", "--prefix", "WAPI"], &pairs);
    assert_success(&output);
    let actors = stdout(&fx.tsk_env(&["actor", "list", "--porcelain"], &pairs));
    assert!(
        actors.contains("Marten") && actors.contains("robot"),
        "{}",
        actors
    );
    assert_eq!(stdout(&fx.tsk(&["mission", "prefix"])), "WAPI\n");
}

#[test]
fn not_used_is_recorded_in_the_nexus_entry_when_the_nexus_lists_the_repo() {
    let fx = Fixture::new();
    fx.set_nexus_json(REPO_ENTRY);
    fx.attach();

    let written = fx.tsk(&["ledger", "not-used"]);
    assert_success(&written);
    assert!(stdout(&written).contains("nexus entry work-api"));
    assert!(!fx.work().join(".git/tsk-not-used").exists());
    assert_eq!(
        nexus_json(&fx)["territories"][0]["repos"][0]["tsk"],
        "not-used"
    );

    let started = stdout(&fx.tsk(&["thread", "session-start"]));
    assert!(started.contains("marked as not under tsk"), "{}", started);
    let status = stdout(&fx.tsk(&["ledger", "status"]));
    assert!(status.contains("marked as not under tsk"), "{}", status);
    let guard = fx.tsk(&["thread", "guard"]);
    assert_success(&guard);
    assert!(!stdout(&guard).contains("block"), "{}", stdout(&guard));

    let forgotten = fx.tsk(&["ledger", "not-used", "--forget"]);
    assert_success(&forgotten);
    assert!(stdout(&forgotten).contains("removed"));
    assert!(nexus_json(&fx)["territories"][0]["repos"][0]
        .get("tsk")
        .is_none());
    let again = stdout(&fx.tsk(&["ledger", "status"]));
    assert!(!again.contains("marked as not under tsk"), "{}", again);
}

#[test]
fn not_used_is_recorded_in_the_clone_when_the_nexus_does_not_list_the_repo() {
    let fx = Fixture::new();
    fx.set_nexus_json(OTHER_ENTRY);
    fx.attach();

    assert_success(&fx.tsk(&["ledger", "not-used"]));
    assert!(fx.work().join(".git/tsk-not-used").is_file());
    let index = fx
        .file_on(&fx.nexus(), "refs/heads/main", "nexus.json")
        .unwrap();
    assert!(!index.contains("not-used"), "{}", index);
    let started = stdout(&fx.tsk(&["thread", "session-start"]));
    assert!(started.contains("marked as not under tsk"), "{}", started);

    assert_success(&fx.tsk(&["ledger", "not-used", "--forget"]));
    assert!(!fx.work().join(".git/tsk-not-used").exists());
}

#[test]
fn a_repo_config_set_before_init_picks_the_refs_but_never_the_place_of_the_ledger() {
    let fx = Fixture::new();
    fx.set_nexus_json(OTHER_ENTRY);
    fx.attach();
    let unset = [("TSK_LEDGER_REFS", "")];
    assert_success(&fx.tsk_env(&["config", "ledger-refs", "custom"], &unset));
    assert!(fx.work().join(".tsk/config.toml").is_file());

    let created = fx.tsk_env(&["ledger", "init", "--nexus", "--prefix", "WAPI"], &unset);
    assert_success(&created);
    assert!(fx.ref_exists(&fx.nexus(), "refs/tsk/ledgers/work-api/ledger"));
    assert!(!fx.ref_exists(&fx.managed(), "refs/tsk/ledger"));
    assert!(!fx.ref_exists(&fx.managed(), IN_REPO_REF));
    let entry = &nexus_json(&fx)["territories"][0]["repos"][1];
    assert_eq!(entry["id"], "work-api");
    assert_eq!(entry["ledger"], "nexus");
}

#[test]
fn a_repo_config_set_before_init_does_not_move_an_in_repo_ledger_into_the_nexus() {
    let fx = Fixture::new();
    fx.set_nexus_json(OTHER_ENTRY);
    fx.attach();
    let unset = [("TSK_LEDGER_REFS", "")];
    assert_success(&fx.tsk_env(&["config", "ledger-refs", "branches"], &unset));

    let created = fx.tsk_env(&["ledger", "init", "--repo", "--prefix", "WAPI"], &unset);
    assert_success(&created);
    assert!(fx.ref_exists(&fx.managed(), IN_REPO_REF));
    assert!(!fx.ref_exists(&fx.nexus(), NEXUS_REF));
    let entry = &nexus_json(&fx)["territories"][0]["repos"][1];
    assert_eq!(entry["ledger"], "repo");
}

#[test]
fn init_saves_the_workflow_to_an_existing_entry_only_after_the_ledger_is_created() {
    let fx = Fixture::new();
    fx.set_nexus_json(
        r#"{"id":"work-api","url":"https://example.test/acme/work-api","ledger":"repo","ledger_refs":"bogus"}"#,
    );
    fx.attach();
    let unset = [("TSK_LEDGER_REFS", "")];

    let failed = fx.tsk_env(&["ledger", "init", "--repo", "--workflow", "pr"], &unset);
    assert!(!failed.status.success(), "{}", stdout(&failed));
    let index = fx
        .file_on(&fx.nexus(), "refs/heads/main", "nexus.json")
        .unwrap();
    assert!(!index.contains("workflow"), "{}", index);

    fx.set_nexus_json(REPO_ENTRY);
    let created = fx.tsk_env(&["ledger", "init", "--repo", "--workflow", "pr"], &unset);
    assert_success(&created);
    assert_eq!(
        nexus_json(&fx)["territories"][0]["repos"][0]["workflow"],
        "pull-request"
    );
}

#[test]
fn status_porcelain_prints_the_workflow_of_the_entry() {
    let fx = Fixture::new();
    fx.set_nexus_json(OTHER_ENTRY);
    fx.attach();
    let before = stdout(&fx.tsk(&["ledger", "status", "--porcelain"]));
    assert!(before.contains("nexus-state\tno-entry"), "{}", before);

    assert_success(&fx.tsk(&[
        "ledger",
        "init",
        "--nexus",
        "--prefix",
        "WAPI",
        "--workflow",
        "pr",
    ]));
    let after = stdout(&fx.tsk(&["ledger", "status", "--porcelain"]));
    assert!(after.contains("workflow\tpull-request\n"), "{}", after);
    let human = stdout(&fx.tsk(&["ledger", "status"]));
    assert!(human.contains("workflow: pull-request"), "{}", human);
}
