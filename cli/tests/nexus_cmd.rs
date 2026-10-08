use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

const ORIGIN_URL: &str = "https://example.test/acme/work-api";

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
        git(root, &["init", "--quiet", "-b", "main", "work-api"]);
        git(&fixture.work(), &["remote", "add", "origin", ORIGIN_URL]);
        git(
            root,
            &["init", "--quiet", "--bare", "-b", "main", "nexus.git"],
        );
        git(root, &["init", "--quiet", "-b", "main", "nexus-seed"]);
        git(
            &fixture.nexus_seed(),
            &["remote", "add", "origin", fixture.nexus().to_str().unwrap()],
        );
        fixture
    }

    fn nexus(&self) -> PathBuf {
        self.root.path().join("nexus.git")
    }

    fn nexus_url(&self) -> String {
        self.nexus().to_str().unwrap().to_string()
    }

    fn nexus_seed(&self) -> PathBuf {
        self.root.path().join("nexus-seed")
    }

    fn work(&self) -> PathBuf {
        self.root.path().join("work-api")
    }

    fn config_file(&self) -> PathBuf {
        self.root.path().join("config/tsk/config.toml")
    }

    fn write_nexus_json(&self, text: &str) {
        std::fs::write(self.nexus_seed().join("nexus.json"), text).unwrap();
        git(&self.nexus_seed(), &["add", "-A"]);
        git(
            &self.nexus_seed(),
            &["commit", "--quiet", "-m", "Set nexus.json"],
        );
        git(
            &self.nexus_seed(),
            &["push", "--quiet", "origin", "HEAD:refs/heads/main"],
        );
    }

    fn nexus_json(&self) -> String {
        git(&self.nexus(), &["show", "refs/heads/main:nexus.json"])
    }

    fn nexus_commits(&self) -> usize {
        git(&self.nexus(), &["rev-list", "--count", "refs/heads/main"])
            .trim()
            .parse()
            .unwrap()
    }

    fn nexus_doc(&self) -> serde_json::Value {
        serde_json::from_str(&self.nexus_json()).unwrap()
    }

    fn repo_id_file(&self) -> PathBuf {
        self.work().join(".git").join("tsk-repo-id")
    }

    fn register(&self, args: &[&str]) -> Output {
        let mut all = vec!["nexus", "register-repo"];
        all.extend_from_slice(args);
        self.tsk(&all)
    }

    fn attach(&self) {
        assert_success(&self.tsk(&["nexus", "add", &self.nexus_url()]));
    }

    fn tsk(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_tsk"))
            .args(args)
            .current_dir(self.work())
            .envs(git_env())
            .env("HOME", self.root.path().join("home"))
            .env("XDG_STATE_HOME", self.root.path().join("state"))
            .env("XDG_CONFIG_HOME", self.root.path().join("config"))
            .env("TSK_MACHINE_NAME", "laptop")
            .env("GIT_CEILING_DIRECTORIES", self.root.path())
            .output()
            .expect("failed to run tsk")
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

fn git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .envs(git_env())
        .output()
        .expect("failed to run git");
    assert!(
        output.status.success(),
        "git {:?} failed in {}: {}",
        args,
        dir.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
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
    assert_eq!(
        output.status.code(),
        Some(1),
        "stdout: {}\nstderr: {}",
        stdout(output),
        stderr(output)
    );
    assert!(
        stderr(output).contains(needle),
        "stderr lacks {:?}: {}",
        needle,
        stderr(output)
    );
}

const TWO_TERRITORIES: &str = r#"{
  "version": 1,
  "territories": [
    {
      "id": "work",
      "name": "Work",
      "repos": [
        { "id": "billing", "url": "https://example.test/acme/billing", "ledger": "nexus" },
        { "id": "scratch", "local": "desktop" }
      ]
    },
    { "id": "home", "name": "Home" }
  ]
}
"#;

#[test]
fn nexus_add_has_the_output_of_config_attach_nexus() {
    let fx = Fixture::new();

    let first = fx.tsk(&["nexus", "add", "https://example.test/o/nexus"]);
    assert_success(&first);
    assert_eq!(
        stdout(&first),
        "attached nexus https://example.test/o/nexus\n"
    );
    let again = fx.tsk(&["nexus", "add", "https://example.test/o/nexus"]);
    assert_success(&again);
    assert_eq!(
        stdout(&again),
        "nexus already attached: https://example.test/o/nexus\n"
    );
    let replaced = fx.tsk(&["nexus", "add", "https://example.test/o/other"]);
    assert_success(&replaced);
    assert_eq!(
        stdout(&replaced),
        "replaced nexus https://example.test/o/nexus with https://example.test/o/other\n"
    );

    let shown = fx.tsk(&["config", "show"]);
    assert!(stdout(&shown).contains("nexus: https://example.test/o/other"));
}

#[test]
fn nexus_list_without_a_nexus_names_the_add_command_and_exits_0() {
    let fx = Fixture::new();

    let output = fx.tsk(&["nexus", "list"]);

    assert_success(&output);
    assert_eq!(
        stdout(&output),
        format!(
            "config: {}\nnexus: none attached\nhint: run tsk nexus add <url> to attach one\n",
            fx.config_file().display()
        )
    );

    let json = fx.tsk(&["nexus", "list", "--json"]);
    assert_success(&json);
    let value: serde_json::Value = serde_json::from_str(&stdout(&json)).unwrap();
    assert_eq!(value["nexus"], serde_json::Value::Null);
    assert_eq!(value["territories"], serde_json::json!([]));
}

#[test]
fn nexus_list_prints_the_territories_and_repos_of_nexus_json() {
    let fx = Fixture::new();
    fx.write_nexus_json(TWO_TERRITORIES);
    fx.attach();

    let output = fx.tsk(&["nexus", "list"]);

    assert_success(&output);
    assert_eq!(
        stdout(&output),
        format!(
            "config: {}\nnexus: {}\n\
             territory: work (Work)\n\
             \x20 billing  url: https://example.test/acme/billing  ledger: nexus\n\
             \x20 scratch  local: desktop  ledger: repo\n\
             territory: home (Home)\n\
             \x20 no repos\n",
            fx.config_file().display(),
            fx.nexus_url()
        )
    );
}

#[test]
fn nexus_list_json_prints_one_object() {
    let fx = Fixture::new();
    fx.write_nexus_json(TWO_TERRITORIES);
    fx.attach();

    let output = fx.tsk(&["nexus", "list", "--json"]);

    assert_success(&output);
    let value: serde_json::Value = serde_json::from_str(&stdout(&output)).unwrap();
    assert_eq!(
        value,
        serde_json::json!({
            "config": fx.config_file().display().to_string(),
            "nexus": fx.nexus_url(),
            "territories": [
                {"id": "work", "name": "Work", "repos": [
                    {"id": "billing", "url": "https://example.test/acme/billing", "local": null, "ledger": "nexus"},
                    {"id": "scratch", "url": null, "local": "desktop", "ledger": "repo"}
                ]},
                {"id": "home", "name": "Home", "repos": []}
            ]
        })
    );
}

#[test]
fn nexus_list_reads_the_latest_nexus_json() {
    let fx = Fixture::new();
    fx.write_nexus_json(r#"{"version":1,"territories":[]}"#);
    fx.attach();
    assert!(stdout(&fx.tsk(&["nexus", "list"])).ends_with("territories: none\n"));

    fx.write_nexus_json(TWO_TERRITORIES);

    assert!(stdout(&fx.tsk(&["nexus", "list"])).contains("billing"));
}

#[test]
fn nexus_list_stops_with_an_error_when_the_nexus_cannot_be_fetched() {
    let fx = Fixture::new();
    assert_success(&fx.tsk(&["nexus", "add", "/nonexistent/nexus.git"]));

    assert_failure_mentioning(&fx.tsk(&["nexus", "list"]), "could not fetch the nexus");
}

const ONE_TERRITORY: &str = r#"{
  "version": 1,
  "owner": "acme",
  "territories": [
    {
      "id": "work",
      "name": "Work",
      "colour": "blue",
      "repos": [
        { "id": "billing", "url": "https://example.test/acme/billing", "ledger": "nexus", "note": "keep" }
      ]
    }
  ]
}
"#;

#[test]
fn register_repo_adds_the_origin_entry_commits_and_pushes() {
    let fx = Fixture::new();
    fx.write_nexus_json(ONE_TERRITORY);
    fx.attach();
    let before = fx.nexus_commits();

    let output = fx.register(&[]);

    assert_success(&output);
    assert_eq!(fx.nexus_commits(), before + 1);
    let doc = fx.nexus_doc();
    assert_eq!(doc["owner"], "acme");
    assert_eq!(doc["version"], 1);
    assert_eq!(doc["territories"][0]["colour"], "blue");
    assert_eq!(doc["territories"][0]["repos"][0]["note"], "keep");
    assert_eq!(
        doc["territories"][0]["repos"][1],
        serde_json::json!({"id": "work-api", "url": ORIGIN_URL, "ledger": "nexus"})
    );
    assert!(fx.nexus_json().ends_with("}\n"));
    assert_eq!(
        git(
            &fx.nexus(),
            &["log", "-1", "--format=%s", "refs/heads/main"]
        )
        .trim(),
        "Register work-api in territory work"
    );
    let text = stdout(&output);
    assert!(
        text.contains("registered in territory work: {\"id\":\"work-api\""),
        "{}",
        text
    );
    assert!(
        text.ends_with("next: run tsk ledger fetch in this repo\n"),
        "{}",
        text
    );

    let fetched = fx.tsk(&["ledger", "fetch"]);
    assert_success(&fetched);
    assert!(
        stderr(&fetched).contains("refs/heads/ledgers/work-api does not exist yet"),
        "{}",
        stderr(&fetched)
    );
}

#[test]
fn register_repo_twice_prints_already_registered_without_a_commit() {
    let fx = Fixture::new();
    fx.write_nexus_json(ONE_TERRITORY);
    fx.attach();
    assert_success(&fx.register(&[]));
    let after_first = fx.nexus_commits();

    let again = fx.register(&[]);

    assert_success(&again);
    assert!(
        stdout(&again).starts_with("already registered in territory work: "),
        "{}",
        stdout(&again)
    );
    assert_eq!(fx.nexus_commits(), after_first);
}

#[test]
fn register_repo_matches_an_existing_ssh_form_of_the_origin() {
    let fx = Fixture::new();
    fx.write_nexus_json(
        r#"{"version":1,"territories":[{"id":"work","repos":[{"id":"work-api","url":"git@example.test:Acme/work-api.git"}]}]}"#,
    );
    fx.attach();
    let before = fx.nexus_commits();

    let output = fx.register(&[]);

    assert_success(&output);
    assert!(stdout(&output).starts_with("already registered"));
    assert_eq!(fx.nexus_commits(), before);
}

#[test]
fn register_repo_refuses_an_id_or_url_already_used_by_another_entry() {
    let fx = Fixture::new();
    fx.write_nexus_json(
        r#"{"version":1,"territories":[{"id":"work","repos":[
          {"id":"work-api","url":"https://example.test/acme/other"},
          {"id":"legacy","url":"https://example.test/acme/billing"}
        ]}]}"#,
    );
    fx.attach();
    let before = fx.nexus_commits();

    assert_failure_mentioning(&fx.register(&[]), "entry with the id \"work-api\"");

    git(
        &fx.work(),
        &[
            "remote",
            "set-url",
            "origin",
            "https://example.test/acme/billing",
        ],
    );
    assert_failure_mentioning(
        &fx.register(&["--id", "billing"]),
        "under the id \"legacy\"",
    );
    assert_eq!(fx.nexus_commits(), before);
}

#[test]
fn register_repo_needs_a_territory_when_the_nexus_has_more_than_one() {
    let fx = Fixture::new();
    fx.write_nexus_json(TWO_TERRITORIES);
    fx.attach();

    assert_failure_mentioning(&fx.register(&[]), "2 territories (work, home)");
    assert_failure_mentioning(
        &fx.register(&["--territory", "play"]),
        "no territory \"play\"",
    );

    let output = fx.register(&["--territory", "home", "--id", "api", "--ledger", "repo"]);

    assert_success(&output);
    let doc = fx.nexus_doc();
    assert_eq!(
        doc["territories"][1]["repos"][0],
        serde_json::json!({"id": "api", "url": ORIGIN_URL, "ledger": "repo"})
    );
    assert_eq!(doc["territories"][0]["repos"].as_array().unwrap().len(), 2);
}

#[test]
fn register_repo_creates_a_territory_when_the_nexus_has_none() {
    let fx = Fixture::new();
    fx.write_nexus_json("{\"version\": 1, \"territories\": []}\n");
    fx.attach();

    assert_failure_mentioning(&fx.register(&[]), "has no territories");

    let output = fx.register(&["--territory", "work", "--territory-name", "Work"]);

    assert_success(&output);
    assert!(stdout(&output).starts_with("created territory work\n"));
    let doc = fx.nexus_doc();
    assert_eq!(doc["territories"][0]["id"], "work");
    assert_eq!(doc["territories"][0]["name"], "Work");
    assert_eq!(doc["territories"][0]["repos"][0]["id"], "work-api");
}

#[test]
fn register_repo_local_names_this_machine_and_writes_the_repo_id() {
    let fx = Fixture::new();
    fx.write_nexus_json(ONE_TERRITORY);
    fx.attach();
    git(&fx.work(), &["remote", "remove", "origin"]);

    assert_failure_mentioning(&fx.register(&[]), "no origin remote URL");

    let output = fx.register(&["--local"]);

    assert_success(&output);
    assert_eq!(
        fx.nexus_doc()["territories"][0]["repos"][1],
        serde_json::json!({"id": "work-api", "local": "laptop", "ledger": "nexus"})
    );
    assert_eq!(
        std::fs::read_to_string(fx.repo_id_file()).unwrap(),
        "work-api\n"
    );
    let before = fx.nexus_commits();
    let again = fx.register(&["--local"]);
    assert_success(&again);
    assert!(stdout(&again).starts_with("already registered"));
    assert_eq!(fx.nexus_commits(), before);

    let fetched = fx.tsk(&["ledger", "fetch"]);
    assert_success(&fetched);
    assert!(
        stderr(&fetched).contains("refs/heads/ledgers/work-api does not exist yet"),
        "{}",
        stderr(&fetched)
    );
}

#[test]
fn register_repo_refuses_a_local_path_origin_and_an_invalid_id() {
    let fx = Fixture::new();
    fx.write_nexus_json(ONE_TERRITORY);
    fx.attach();

    assert_failure_mentioning(&fx.register(&["--id", "Work_API"]), "not a valid repo id");

    git(
        &fx.work(),
        &["remote", "set-url", "origin", "/tmp/origin.git"],
    );
    assert_failure_mentioning(&fx.register(&[]), "run with --local");
}

#[test]
fn register_repo_without_a_nexus_names_the_add_command() {
    let fx = Fixture::new();

    assert_failure_mentioning(&fx.register(&[]), "run tsk nexus add <url> first");
}

#[test]
fn register_repo_fetches_again_and_keeps_a_concurrent_change_after_a_rejected_push() {
    let fx = Fixture::new();
    fx.write_nexus_json(ONE_TERRITORY);
    fx.attach();
    let concurrent = ONE_TERRITORY.replace("\"owner\": \"acme\"", "\"owner\": \"someone else\"");
    std::fs::write(fx.nexus_seed().join("nexus.json"), concurrent).unwrap();
    git(
        &fx.nexus_seed(),
        &["commit", "--quiet", "-am", "Concurrent change"],
    );
    git(
        &fx.nexus_seed(),
        &["push", "--quiet", "origin", "HEAD:refs/heads/concurrent"],
    );
    let marker = fx.root.path().join("rejected-once");
    let hook = fx.nexus().join("hooks").join("pre-receive");
    std::fs::write(
        &hook,
        format!(
            "#!/bin/sh\ncat >/dev/null\nif [ ! -e \"{marker}\" ]; then\n  touch \"{marker}\"\n  env -u GIT_QUARANTINE_PATH -u GIT_OBJECT_DIRECTORY -u GIT_ALTERNATE_OBJECT_DIRECTORIES git update-ref refs/heads/main refs/heads/concurrent\n  exit 1\nfi\n",
            marker = marker.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&hook, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();

    let output = fx.register(&[]);

    assert_success(&output);
    assert!(
        stderr(&output).contains("retrying once"),
        "{}",
        stderr(&output)
    );
    let doc = fx.nexus_doc();
    assert_eq!(doc["owner"], "someone else");
    assert_eq!(doc["territories"][0]["repos"][1]["id"], "work-api");
}
