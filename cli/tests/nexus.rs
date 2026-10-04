use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

const IN_REPO_REF: &str = "refs/heads/tsk/ledger";
const NEXUS_REF: &str = "refs/heads/ledgers/work-api";
const ORIGIN_URL: &str = "https://example.test/acme/work-api";
const BRIEFING: &str = "missions/operational/M-TEST-01-example.md";
const WORK_API_ENTRY: &str =
    r#"{"id":"work-api","url":"https://example.test/acme/work-api","ledger":"nexus"}"#;

struct Fixture {
    root: TempDir,
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

    fn seed_nexus_ledger(&self, manifest: &str) {
        let dir = self.root.path().join("ledger-seed");
        git(
            self.root.path(),
            &["init", "--quiet", "-b", "main", "ledger-seed"],
        );
        write(&dir.join(".tsk-ledger.toml"), manifest);
        write(&dir.join("index.md"), "# Ledger index\n");
        write_nested(&dir.join(BRIEFING), "# Mission: example\n");
        commit_all(&dir, "Seed the nexus ledger");
        git(
            &dir,
            &[
                "push",
                "--quiet",
                self.nexus().to_str().unwrap(),
                &format!("HEAD:{}", NEXUS_REF),
            ],
        );
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

fn write_nested(path: &Path, content: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    write(path, content);
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
    assert_eq!(
        std::fs::read_to_string(path.join(".tsk-ledger.toml")).unwrap(),
        "version = 1\nrepo_id = \"work-api\"\n"
    );
    assert!(path.join("index.md").is_file());
    assert!(!fx.ref_exists(&fx.nexus(), NEXUS_REF));

    write_nested(&path.join("missions").join("M-1.md"), "mission\n");
    let pushed = fx.tsk(&["ledger", "push", "First commit"]);
    assert_success(&pushed);

    assert!(fx.ref_exists(&fx.nexus(), NEXUS_REF));
    assert!(!fx.ref_exists(&fx.managed(), IN_REPO_REF));
    assert_eq!(
        stdout(&pushed).trim_end(),
        git_line(&fx.nexus(), &["rev-parse", NEXUS_REF])
    );
    assert_eq!(
        fx.file_on(&fx.nexus(), NEXUS_REF, "missions/M-1.md")
            .as_deref(),
        Some("mission\n")
    );
    assert_eq!(
        fx.file_on(&fx.nexus(), NEXUS_REF, ".tsk-ledger.toml")
            .as_deref(),
        Some("version = 1\nrepo_id = \"work-api\"\n")
    );
    assert_eq!(
        std::fs::read_to_string(fx.repo_id_file()).unwrap().trim(),
        "work-api"
    );
}

#[test]
fn the_ledger_worktree_stays_a_linked_worktree_of_the_managed_clone() {
    let fx = Fixture::new();
    fx.set_nexus_json(WORK_API_ENTRY);
    fx.attach();

    let path = fx.fetch_ok();

    let listing = git(&fx.work(), &["worktree", "list", "--porcelain", "-z"]);
    let canonical = std::fs::canonicalize(&path).unwrap();
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
    write(&path.join("one.md"), "one\n");
    fx.push_ok("One");
    write(&path.join("two.md"), "two\n");
    fx.push_ok("Two");

    assert_eq!(
        git_line(&fx.nexus(), &["rev-list", "--count", NEXUS_REF]),
        "3"
    );
    let again = fx.fetch_ok();
    assert_eq!(again, path);
    assert_eq!(
        git_line(&path, &["rev-parse", "HEAD"]),
        git_line(&fx.nexus(), &["rev-parse", NEXUS_REF])
    );
}

#[test]
fn an_existing_nexus_ledger_is_fetched() {
    let fx = Fixture::new();
    fx.set_nexus_json(WORK_API_ENTRY);
    fx.attach();
    fx.seed_nexus_ledger("version = 1\nrepo_id = \"work-api\"\n");

    let path = fx.fetch_ok();

    assert_eq!(
        git_line(&path, &["rev-parse", "HEAD"]),
        git_line(&fx.nexus(), &["rev-parse", NEXUS_REF])
    );
    assert!(path.join(BRIEFING).is_file());
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

        let path = fx.fetch_ok();

        assert_eq!(
            std::fs::read_to_string(path.join(".tsk-ledger.toml")).unwrap(),
            "version = 1\nrepo_id = \"work-api\"\n",
            "{}",
            url
        );
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

    let path = fx.fetch_ok();

    assert!(std::fs::read_to_string(path.join(".tsk-ledger.toml"))
        .unwrap()
        .contains("repo_id = \"work-api\""));
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
    let path = fx.fetch_ok();

    assert_eq!(
        git_line(&path, &["rev-parse", "HEAD"]),
        git_line(&fx.nexus(), &["rev-parse", NEXUS_REF])
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
    let path = fx.fetch_ok();
    assert!(std::fs::read_to_string(path.join(".tsk-ledger.toml"))
        .unwrap()
        .contains("repo_id = \"work-api\""));
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
    let path = PathBuf::from(stdout(&output).trim_end());
    assert_eq!(
        std::fs::read_to_string(path.join(".tsk-ledger.toml")).unwrap(),
        "version = 1\n"
    );
    assert!(stderr(&output).contains("refs/heads/tsk/ledger does not exist yet"));

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
    fx.seed_nexus_ledger("version = 1\nrepo_id = \"someone-else\"\n");

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
    assert_failure_mentioning(&output, "git fetch --quiet");
    assert!(stderr(&output).contains("refs/heads/ledgers/work-api"));
}

#[test]
fn threads_pause_and_resume_against_a_nexus_ledger() {
    let fx = Fixture::new();
    fx.set_nexus_json(WORK_API_ENTRY);
    fx.attach();
    let path = fx.fetch_ok();
    write_nested(&path.join(BRIEFING), "# Mission: example\n");
    fx.push_ok("Add the briefing");

    let started = fx.tsk(&["thread", "start", "M-TEST-01", BRIEFING]);
    assert_success(&started);
    let id = stdout(&started)
        .trim_end()
        .strip_prefix("started:")
        .unwrap()
        .to_string();

    let head = git_line(&fx.work(), &["rev-parse", "HEAD"]);
    let paused = fx.tsk(&["thread", "pause", &id, BRIEFING, "T-01", "Carry on"]);
    assert_success(&paused);
    let store = fx
        .file_on(
            &fx.nexus(),
            NEXUS_REF,
            &format!("threads/{}/continuation-state.jsonl", id),
        )
        .unwrap();
    let entry: serde_json::Value = serde_json::from_str(store.lines().next().unwrap()).unwrap();
    assert_eq!(entry["task_id"], "T-01");
    assert_eq!(entry["git"]["code"]["commit"], head.as_str());
    assert_eq!(entry["git"]["ledger"]["commit"].as_str().unwrap().len(), 40);
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

    let queue = fx
        .file_on(&fx.nexus(), NEXUS_REF, "external-events/queue.ndjson")
        .unwrap();
    assert_eq!(queue.lines().count(), 1);
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

    let output = fx.tsk(&["thread", "start", "M-TEST-01", BRIEFING]);

    assert_failure_mentioning(&output, BRIEFING);
}
