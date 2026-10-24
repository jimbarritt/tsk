//! Git hooks of the code repo never run for tsk's own worktrees, and the documents hook
//! point (`.tsk/hooks/pre-docs-commit`) runs on `doc put` and before a push commits.
//!
//! The tests reuse the fixture of the background publish tests: a seeded ledger on a local
//! bare origin, and one clone, `work`.

use super::*;

fn write_hook(path: &Path, body: &str, mode: u32) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, body).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
}

fn docs_hook(fx: &Fixture) -> PathBuf {
    fx.work().join(".tsk/hooks/pre-docs-commit")
}

fn skip(command: &mut Command) -> &mut Command {
    command.env("TSK_SKIP_DOC_CHECK", "1")
}

fn put(fx: &Fixture, path: &str, skipping: bool) -> Output {
    let mut command = fx.command(&["ledger", "doc", "put", path]);
    if skipping {
        skip(&mut command);
    }
    run_stdin(&mut command, "# note\n")
}

fn push(fx: &Fixture, skipping: bool) -> Output {
    let mut command = fx.command(&["ledger", "push", "Publish notes"]);
    if skipping {
        skip(&mut command);
    }
    command.output().unwrap()
}

#[test]
fn git_hooks_of_the_code_repo_do_not_run_for_the_ledger_worktrees() {
    let fx = Fixture::new();
    let marker = fx.root.path().join("git-hook-ran");
    for hook in [
        "pre-commit",
        "commit-msg",
        "pre-push",
        "post-commit",
        "pre-rebase",
    ] {
        write_hook(
            &fx.work().join(".git/hooks").join(hook),
            &format!("#!/bin/sh\necho {} >> {}\nexit 1\n", hook, marker.display()),
            0o755,
        );
    }
    let before = fx.origin_sha();
    assert_success(&put(&fx, "notes/hooks.md", false));
    assert_success(&push(&fx, false));
    assert_ne!(fx.origin_sha(), before, "the documents were not pushed");
    assert!(
        !marker.exists(),
        "{}",
        std::fs::read_to_string(&marker).unwrap()
    );
}

#[test]
fn a_failing_docs_hook_makes_put_exit_1_and_the_document_stays() {
    let fx = Fixture::new();
    write_hook(
        &docs_hook(&fx),
        "#!/bin/sh\necho \"lint: $1 has a problem\"\necho \"on stderr\" >&2\nexit 1\n",
        0o755,
    );
    let out = put(&fx, "notes/bad.md", false);
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert!(stderr(&out).contains("lint: notes/bad.md has a problem"));
    assert!(stderr(&out).contains("on stderr"));
    let got = fx.tsk(&["ledger", "doc", "get", "notes/bad.md"]);
    assert_success(&got);
    assert_eq!(stdout(&got), "# note\n");
}

#[test]
fn a_failing_docs_hook_blocks_the_push_and_nothing_is_published() {
    let fx = Fixture::new();
    assert_success(&put(&fx, "notes/early.md", false));
    write_hook(
        &docs_hook(&fx),
        "#!/bin/sh\necho \"lint: stop\"\nexit 1\n",
        0o755,
    );
    let before = fx.origin_sha();
    let out = push(&fx, false);
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert!(stderr(&out).contains("lint: stop"), "{}", stderr(&out));
    assert_eq!(fx.origin_sha(), before);
    // The document and the local state stay.
    assert!(fx.unpushed());
    assert_success(&fx.tsk(&["ledger", "doc", "get", "notes/early.md"]));
    // Once the hook passes, the same push goes through.
    write_hook(&docs_hook(&fx), "#!/bin/sh\nexit 0\n", 0o755);
    assert_success(&push(&fx, false));
    assert_ne!(fx.origin_sha(), before);
}

#[test]
fn a_passing_docs_hook_gets_the_paths_the_directory_and_the_environment() {
    let fx = Fixture::new();
    let log = fx.root.path().join("hook.log");
    write_hook(
        &docs_hook(&fx),
        &format!(
            "#!/bin/sh\necho \"args: $*\" >> {log}\necho \"env: $TSK_DOCS_WORKTREE\" >> {log}\necho \"cwd: $(pwd -P)\" >> {log}\necho hook output\nexit 0\n",
            log = log.display()
        ),
        0o755,
    );
    let first = put(&fx, "notes/one.md", false);
    assert_success(&first);
    assert!(stdout(&first).contains("hook output"));
    let second = put(&fx, "notes/two.md", false);
    assert_success(&second);
    let docs =
        String::from_utf8(fx.tsk(&["ledger", "doc", "path", "notes/one.md"]).stdout).unwrap();
    let docs = Path::new(docs.trim())
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let text = std::fs::read_to_string(&log).unwrap();
    assert!(text.contains("args: notes/one.md\n"), "{}", text);
    assert!(text.contains("args: notes/two.md\n"), "{}", text);
    assert!(
        text.contains(&format!("env: {}\n", docs.display())),
        "{} / {}",
        text,
        docs.display()
    );
    assert!(
        text.contains(&format!(
            "cwd: {}\n",
            docs.canonicalize().unwrap().display()
        )),
        "{}",
        text
    );

    std::fs::remove_file(&log).unwrap();
    assert_success(&push(&fx, false));
    let text = std::fs::read_to_string(&log).unwrap();
    assert_eq!(text.matches("args:").count(), 1, "{}", text);
    assert!(
        text.contains("args: notes/one.md notes/two.md\n"),
        "{}",
        text
    );
}

#[test]
fn the_skip_variable_skips_the_hook_and_says_so() {
    let fx = Fixture::new();
    write_hook(&docs_hook(&fx), "#!/bin/sh\necho stop\nexit 1\n", 0o755);
    let before = fx.origin_sha();
    let put = put(&fx, "notes/skipped.md", true);
    assert_success(&put);
    assert!(
        stderr(&put).contains("TSK_SKIP_DOC_CHECK"),
        "{}",
        stderr(&put)
    );
    assert!(!stderr(&put).contains("stop"));
    let pushed = push(&fx, true);
    assert_success(&pushed);
    assert!(
        stderr(&pushed).contains("TSK_SKIP_DOC_CHECK"),
        "{}",
        stderr(&pushed)
    );
    assert_ne!(fx.origin_sha(), before);
}

#[test]
fn a_hook_that_is_not_executable_is_an_error_that_names_its_path() {
    let fx = Fixture::new();
    write_hook(&docs_hook(&fx), "#!/bin/sh\nexit 0\n", 0o644);
    let out = put(&fx, "notes/x.md", false);
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).contains("pre-docs-commit"), "{}", stderr(&out));
    assert!(stderr(&out).contains(".tsk/hooks/pre-docs-commit"));
}

#[test]
fn a_background_publish_that_a_hook_stops_shows_the_hook_output_in_the_warning() {
    let fx = Fixture::new();
    write_hook(
        &docs_hook(&fx),
        "#!/bin/sh\necho \"lint: notes/bg.md fails rule 7\"\nexit 1\n",
        0o755,
    );
    assert_success(&put(&fx, "notes/bg.md", true));
    assert_success(&fx.tsk(&["ledger", "push", "--background", "bgmsg-hook"]));
    fx.wait_idle();
    let result = fx.result().unwrap();
    assert_eq!(result["state"], "failed", "{}", result);
    assert!(
        result["error"]
            .as_str()
            .unwrap()
            .contains("lint: notes/bg.md fails rule 7"),
        "{}",
        result
    );
    let next = fx.tsk(&["ledger", "status", "--local"]);
    assert_success(&next);
    assert!(stderr(&next).starts_with(WARNING), "{}", stderr(&next));
    assert!(
        stderr(&next).contains("lint: notes/bg.md fails rule 7"),
        "{}",
        stderr(&next)
    );
}

#[test]
fn a_background_publish_that_git_stops_shows_its_stderr_in_the_warning() {
    let fx = Fixture::new();
    assert_success(&put(&fx, "notes/p.md", false));
    git(
        &fx.work(),
        &["remote", "set-url", "origin", "/nonexistent/origin.git"],
    );
    assert_success(&fx.tsk(&["ledger", "push", "--background", "bgmsg-nogit"]));
    fx.wait_idle();
    let next = fx.tsk(&["ledger", "status", "--local"]);
    assert!(stderr(&next).starts_with(WARNING), "{}", stderr(&next));
    assert!(stderr(&next).contains("nonexistent"), "{}", stderr(&next));
}

fn marker_hook(path: &Path, name: &str, log: &Path) {
    write_hook(
        path,
        &format!("#!/bin/sh\necho {} >> {}\nexit 0\n", name, log.display()),
        0o755,
    );
}

fn hook_source(fx: &Fixture, env: Option<&Path>) -> String {
    let mut command = fx.command(&["config", "docs-hook"]);
    if let Some(env) = env {
        command.env("TSK_DOCS_HOOK", env);
    }
    let out = command.output().unwrap();
    assert_success(&out);
    stdout(&out)
}

#[test]
fn the_hook_is_looked_up_in_the_environment_then_the_clone_then_the_user_then_the_repo() {
    let fx = Fixture::new();
    let log = fx.root.path().join("which.log");
    let repo = docs_hook(&fx);
    let clone = fx.state_dir().join("hooks/pre-docs-commit");
    let user = fx.home().join("config/tsk/hooks/pre-docs-commit");
    let env = fx.root.path().join("env-hook");

    assert_eq!(hook_source(&fx, None), "docs hook: none\n");
    assert_success(&put(&fx, "notes/a.md", false));

    marker_hook(&repo, "repo", &log);
    let shown = hook_source(&fx, None);
    assert!(
        shown.contains(&format!("docs hook: {}\n", repo.display())),
        "{}",
        shown
    );
    assert!(shown.contains("source: repo"), "{}", shown);
    assert_success(&put(&fx, "notes/a.md", false));

    marker_hook(&user, "user", &log);
    assert!(hook_source(&fx, None).contains("source: user"));
    assert_success(&put(&fx, "notes/a.md", false));

    marker_hook(&clone, "clone", &log);
    let shown = hook_source(&fx, None);
    assert!(
        shown.contains(&format!("docs hook: {}\n", clone.display())),
        "{}",
        shown
    );
    assert!(shown.contains("source: clone"), "{}", shown);
    assert_success(&put(&fx, "notes/a.md", false));

    marker_hook(&env, "env", &log);
    assert!(hook_source(&fx, Some(&env)).contains("source: environment"));
    let mut command = fx.command(&["ledger", "doc", "put", "notes/a.md"]);
    command.env("TSK_DOCS_HOOK", &env);
    assert_success(&run_stdin(&mut command, "# note\n"));

    let ran = std::fs::read_to_string(&log).unwrap();
    assert_eq!(
        ran.lines().collect::<Vec<_>>(),
        ["repo", "user", "clone", "env"]
    );
}

#[test]
fn a_hook_that_is_named_but_missing_or_not_executable_is_an_error_that_names_its_path() {
    let fx = Fixture::new();
    let missing = fx.root.path().join("no-such-hook");
    let mut command = fx.command(&["config", "docs-hook"]);
    command.env("TSK_DOCS_HOOK", &missing);
    let out = command.output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(
        stderr(&out).contains(&missing.display().to_string()),
        "{}",
        stderr(&out)
    );

    let clone = fx.state_dir().join("hooks/pre-docs-commit");
    write_hook(&clone, "#!/bin/sh\nexit 0\n", 0o644);
    let out = fx.tsk(&["config", "docs-hook"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(
        stderr(&out).contains(&clone.display().to_string()),
        "{}",
        stderr(&out)
    );
}
