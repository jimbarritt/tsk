//! Template ledgers. A test that needs a published ledger asks for a copy of one, and
//! each distinct setup is built once per test process.
//!
//! A template is built in a temporary directory, which holds a bare `origin.git`, a `seed`
//! clone that pushed `main`, the clones, and the `home` and `state` directories the `tsk`
//! commands use. The directory is then copied into the template store under the target
//! directory. Each test gets a real filesystem copy in a directory of its own, and every
//! text file that holds the path of the built directory (a git remote URL, a Dolt remote,
//! a worktree link) is rewritten to the path of the copy. So no test writes into a
//! template, and no test sees the change of another.
//!
//! A Dolt server that a build started would leave its files and a store that is still
//! changing in the copy. A build of [`instance`] runs with the server switch off, and
//! [`cached`] stops the servers of any other build, and waits for them to clean up, before
//! it copies.

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use tempfile::TempDir;

/// What a template build needs to know about a directory it keeps alive until the copy is
/// made. A fixture type that stops servers when it is dropped implements this too.
pub trait Root {
    fn root(&self) -> &Path;
}

impl Root for TempDir {
    fn root(&self) -> &Path {
        self.path()
    }
}

struct Template {
    dir: PathBuf,
    built_at: String,
    note: String,
}

type Registry = Mutex<HashMap<String, Arc<OnceLock<Template>>>>;

fn registry() -> &'static Registry {
    static REGISTRY: OnceLock<Registry> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

/// A copy of the template `key` in a new temporary directory, with the note its build
/// returned (a thread ID, say). `build` runs once per process, in the first test that asks.
/// Tests that ask for other keys at the same time build them at the same time. A test that
/// asks for the same key waits for the build.
pub fn cached<R: Root>(key: &str, build: impl FnOnce() -> (R, String)) -> (TempDir, String) {
    let cell = registry()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .entry(key.to_string())
        .or_default()
        .clone();
    let template = cell.get_or_init(|| {
        let (built, note) = build();
        static COUNT: AtomicUsize = AtomicUsize::new(0);
        let dir = template_store().join(COUNT.fetch_add(1, Ordering::Relaxed).to_string());
        // A running Dolt server would leave its files and a store that is still changing in
        // the copy. Stop them first, and let them clean up.
        super::stop_servers(built.root(), true);
        run_cp(&built.root().join("."), &dir);
        Template {
            dir,
            built_at: built.root().to_string_lossy().into_owned(),
            note,
        }
    });
    let root = tempfile::tempdir().unwrap();
    run_cp(&template.dir.join("."), root.path());
    rewrite_paths(
        root.path(),
        &template.built_at,
        &root.path().to_string_lossy(),
    );
    (root, template.note.clone())
}

/// The directory that holds this process's templates, under the target directory. The next
/// test run removes it, with any other store whose process has ended.
fn template_store() -> PathBuf {
    static STORE: OnceLock<PathBuf> = OnceLock::new();
    STORE
        .get_or_init(|| {
            let parent = Path::new(env!("CARGO_TARGET_TMPDIR"));
            let prefix = "templates-";
            if let Ok(entries) = std::fs::read_dir(parent) {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().into_owned();
                    if let Some(pid) = name.strip_prefix(prefix) {
                        if !Path::new("/proc").join(pid).exists() {
                            let _ = std::fs::remove_dir_all(entry.path());
                        }
                    }
                }
            }
            let dir = parent.join(format!("{}{}", prefix, std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            dir
        })
        .clone()
}

/// `cp -a from to`, which keeps modes, times and links. `from` ends in `/.` to copy the
/// content of a directory.
pub fn run_cp(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    let output = Command::new("cp")
        .arg("-a")
        .arg(from)
        .arg(to)
        .output()
        .expect("failed to run cp");
    assert!(
        output.status.success(),
        "cp failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Replace `old` by `new` in every file under `dir` that holds it. A file that holds it
/// and is not UTF-8 text fails the test: a byte replacement could corrupt it.
pub fn rewrite_paths(dir: &Path, old: &str, new: &str) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let kind = std::fs::symlink_metadata(&path).unwrap().file_type();
        if kind.is_dir() {
            rewrite_paths(&path, old, new);
        } else if kind.is_file() {
            let bytes = std::fs::read(&path).unwrap();
            if !bytes.windows(old.len()).any(|w| w == old.as_bytes()) {
                continue;
            }
            let text = String::from_utf8(bytes)
                .unwrap_or_else(|_| panic!("{} holds a path and is not text", path.display()));
            std::fs::write(&path, text.replace(old, new)).unwrap();
        }
    }
}

/// One `tsk` command of a [`Seed`], run in a clone.
#[derive(Debug)]
pub enum Step {
    /// The clone, and the arguments.
    Tsk(&'static str, &'static [&'static str]),
    /// The clone, the arguments, and what the command reads on standard input.
    Stdin(&'static str, &'static [&'static str], &'static str),
}

/// A published ledger, described by what it takes to build it. Two tests that ask for equal
/// seeds share one template, wherever they are.
#[derive(Debug)]
pub struct Seed {
    /// The default branch of `origin.git`.
    pub branch: &'static str,
    /// The clones of `origin.git`, named as the directories under the root.
    pub clones: &'static [&'static str],
    /// The value of `TSK_LEDGER_REFS` for the build, or `None` to leave it out, so that a new
    /// ledger takes its default refs.
    pub refs: Option<&'static str>,
    /// The commands, in order. A command that fails fails the test that asked first.
    pub steps: &'static [Step],
}

/// A new ledger that clone `work` has fetched and nobody has pushed. Clone `work2` has
/// fetched nothing. Several test files start here.
pub const FETCHED: Seed = Seed {
    branch: "main",
    clones: &["work", "work2"],
    refs: Some("branches"),
    steps: &[Step::Tsk("work", &["ledger", "fetch"])],
};

/// A copy of the template for `seed`: the directories of the build, with the servers
/// stopped and the paths rewritten. The caller wraps it in its own fixture type.
pub fn instance(seed: &Seed) -> TempDir {
    cached(&format!("{:?}", seed), || (build(seed), String::new())).0
}

fn build(seed: &Seed) -> TempDir {
    let root = tempfile::tempdir().unwrap();
    let path = root.path();
    std::fs::create_dir_all(path.join("home")).unwrap();
    std::fs::create_dir_all(path.join("state")).unwrap();
    git(
        path,
        &["init", "--quiet", "--bare", "-b", seed.branch, "origin.git"],
    );
    git(path, &["init", "--quiet", "-b", seed.branch, "seed"]);
    let origin = path.join("origin.git");
    let work = path.join("seed");
    git(
        &work,
        &["remote", "add", "origin", origin.to_str().unwrap()],
    );
    std::fs::write(work.join("README.md"), "main\n").unwrap();
    git(&work, &["add", "-A"]);
    git(&work, &["commit", "--quiet", "-m", "Main commit"]);
    git(
        &work,
        &[
            "push",
            "--quiet",
            "origin",
            &format!("HEAD:refs/heads/{}", seed.branch),
        ],
    );
    for name in seed.clones {
        git(path, &["clone", "--quiet", origin.to_str().unwrap(), name]);
    }
    for step in seed.steps {
        let (clone, args, input) = match step {
            Step::Tsk(clone, args) => (clone, *args, ""),
            Step::Stdin(clone, args, input) => (clone, *args, *input),
        };
        let output = tsk(path, seed, &path.join(clone), args, input);
        assert!(
            output.status.success(),
            "tsk {:?} failed in {}: {}",
            args,
            clone,
            String::from_utf8_lossy(&output.stderr)
        );
    }
    root
}

fn tsk(root: &Path, seed: &Seed, dir: &Path, args: &[&str], input: &str) -> Output {
    let home = root.join("home");
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
        .env("HOME", &home)
        .env("XDG_STATE_HOME", root.join("state"))
        .env("XDG_CONFIG_HOME", home.join("config"))
        .env("TSK_HOME", home.join(".tsk"))
        .env("GIT_CEILING_DIRECTORIES", root)
        .env("TSK_DOLT_SERVER", "off")
        .env("TSK_NO_UPDATE_CHECK", "1")
        .env("TSK_AUTO_UPDATE", "never")
        .env("TSK_BACKGROUND_PUBLISH", "off")
        .env_remove("TSK_LEDGER_TRANSPORT")
        .env_remove("CLAUDE_CODE_REMOTE")
        .env_remove("CLAUDE_CODE_REMOTE_SESSION_ID")
        .env_remove("CLAUDE_ENV_FILE");
    match seed.refs {
        Some(refs) => command.env("TSK_LEDGER_REFS", refs),
        None => command.env_remove("TSK_LEDGER_REFS"),
    };
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to run tsk");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

fn git(dir: &Path, args: &[&str]) {
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
}
