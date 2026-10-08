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
