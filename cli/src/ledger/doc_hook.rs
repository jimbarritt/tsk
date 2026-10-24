//! The documents hook point.
//!
//! A repo may hold an executable `.tsk/hooks/pre-docs-commit` at the root of its code
//! worktree, next to `.tsk/config.toml`. A repo the user does not control takes the hook
//! from `TSK_DOCS_HOOK`, the clone's state directory or the user config instead (see
//! [`resolve`]). tsk runs it with the documents worktree as the
//! current directory, `TSK_DOCS_WORKTREE` set to that path, and the changed document
//! paths (relative to the documents worktree) as arguments, at two points:
//!
//! - `tsk ledger doc put`, after the document is written, with that one path;
//! - `tsk ledger push` (and the background publish), before the documents commit, with
//!   every document changed since the last push.
//!
//! A non-zero exit stops the command. The hook is generic: tsk does not know what it
//! checks. `TSK_SKIP_DOC_CHECK=1` skips it at both points.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use super::location::Repo;
use super::refs::repo_config_path;

/// The hook, relative to the root of the code worktree.
pub const HOOK_FILE: &str = ".tsk/hooks/pre-docs-commit";
/// Set to `1` to skip the hook.
pub const SKIP_VAR: &str = "TSK_SKIP_DOC_CHECK";
/// The environment variable that carries the documents worktree to the hook.
pub const WORKTREE_VAR: &str = "TSK_DOCS_WORKTREE";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookRun {
    /// The repo has no hook, or no document changed.
    NotRun,
    /// `TSK_SKIP_DOC_CHECK` is set.
    Skipped,
    /// The hook exited 0.
    Passed,
}

/// The environment variable that names the hook script, the first place looked in.
pub const HOOK_VAR: &str = "TSK_DOCS_HOOK";
/// The file name of the hook in the per-clone, per-user and in-repo places.
pub const HOOK_NAME: &str = "pre-docs-commit";

/// The four places the hook is looked up, in order. The first that exists wins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookSource {
    Env,
    Clone,
    User,
    Repo,
}

impl HookSource {
    pub fn label(self) -> &'static str {
        match self {
            HookSource::Env => "environment (TSK_DOCS_HOOK)",
            HookSource::Clone => "clone (<state>/repos/<clone-id>/hooks/pre-docs-commit)",
            HookSource::User => "user (<config>/tsk/hooks/pre-docs-commit)",
            HookSource::Repo => "repo (.tsk/hooks/pre-docs-commit)",
        }
    }
}

fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
}

fn check_executable(path: &Path) -> Result<(), String> {
    if is_executable(path) {
        Ok(())
    } else {
        Err(format!(
            "error: the documents hook {} is not executable. Run chmod +x on it",
            path.display()
        ))
    }
}

/// The hook that applies, and the place it came from. `clone_dir` is the state directory
/// of the clone (next to `ledger-db`). A candidate that does not exist is skipped. One
/// that exists and is not executable is an error that names its path. `TSK_DOCS_HOOK`
/// set to a path that is not a file is an error too.
pub fn resolve(
    repo: &Repo,
    clone_dir: Option<&Path>,
) -> Result<Option<(PathBuf, HookSource)>, String> {
    if let Some(value) = std::env::var_os(HOOK_VAR).filter(|v| !v.is_empty()) {
        let path = PathBuf::from(value);
        if !path.is_file() {
            return Err(format!(
                "error: {} names {}, which is not a file",
                HOOK_VAR,
                path.display()
            ));
        }
        check_executable(&path)?;
        return Ok(Some((path, HookSource::Env)));
    }
    let mut candidates: Vec<(PathBuf, HookSource)> = Vec::new();
    if let Some(dir) = clone_dir {
        candidates.push((dir.join("hooks").join(HOOK_NAME), HookSource::Clone));
    }
    if let Some(user) = crate::config::config_file_from_env()
        .and_then(|f| f.parent().map(|dir| dir.join("hooks").join(HOOK_NAME)))
    {
        candidates.push((user, HookSource::User));
    }
    if let Some(path) = repo_hook_path(repo) {
        candidates.push((path, HookSource::Repo));
    }
    for (path, source) in candidates {
        if path.is_file() {
            check_executable(&path)?;
            return Ok(Some((path, source)));
        }
    }
    Ok(None)
}

/// The in-repo hook file, which may not exist. `None` outside a working tree.
pub fn repo_hook_path(repo: &Repo) -> Option<PathBuf> {
    let config = repo_config_path(repo)?;
    // `<top>/.tsk/config.toml` -> `<top>/.tsk/hooks/pre-docs-commit`
    let top = config.parent()?.parent()?;
    Some(top.join(HOOK_FILE))
}

/// `tsk config docs-hook`: print the script that applies and where it came from.
pub fn show() -> Result<(), String> {
    let cwd = std::env::current_dir()
        .map_err(|e| format!("error: could not read the current directory: {}", e))?;
    let repo = Repo::discover(&cwd)?;
    let clone_dir = match (super::location::state_root_from_env(), repo.read_clone_id()) {
        (Ok(root), Ok(Some(id))) => super::location::dolt_db_path(&root, &id)
            .parent()
            .map(Path::to_path_buf),
        _ => None,
    };
    match resolve(&repo, clone_dir.as_deref())? {
        Some((path, source)) => {
            println!("docs hook: {}", path.display());
            println!("source: {}", source.label());
        }
        None => println!("docs hook: none"),
    }
    Ok(())
}

fn skip_requested() -> bool {
    std::env::var(SKIP_VAR).is_ok_and(|v| {
        let v = v.trim();
        !v.is_empty() && v != "0"
    })
}

/// Run the hook of `repo` for `paths` in the documents worktree `docs_root`.
///
/// The hook's output passes through unchanged: stdout to stdout and stderr to stderr when
/// it exits 0, and in the error text when it does not. `paths` empty runs nothing.
pub fn run(repo: &Repo, docs_root: &Path, paths: &[String]) -> Result<HookRun, String> {
    // The documents worktree sits in the state directory of the clone.
    let Some((hook, _)) = resolve(repo, docs_root.parent())? else {
        return Ok(HookRun::NotRun);
    };
    run_hook(&hook, docs_root, paths)
}

pub fn run_hook(hook: &Path, docs_root: &Path, paths: &[String]) -> Result<HookRun, String> {
    if !hook.exists() || paths.is_empty() {
        return Ok(HookRun::NotRun);
    }
    if skip_requested() {
        eprintln!(
            "note: {} is set; skipped the documents hook {}",
            SKIP_VAR,
            hook.display()
        );
        return Ok(HookRun::Skipped);
    }
    let output = Command::new(hook)
        .args(paths)
        .current_dir(docs_root)
        .env(WORKTREE_VAR, docs_root)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| {
            format!(
                "error: could not run the documents hook {}: {}.\n       \
                 It must be an executable file with a working interpreter line",
                hook.display(),
                e
            )
        })?;
    if output.status.success() {
        let _ = std::io::stdout().write_all(&output.stdout);
        let _ = std::io::stderr().write_all(&output.stderr);
        return Ok(HookRun::Passed);
    }
    let status = match output.status.code() {
        Some(code) => format!("exit status {}", code),
        None => "terminated by a signal".to_string(),
    };
    let mut text = String::new();
    text.push_str(&String::from_utf8_lossy(&output.stdout));
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str(&format!(
        "error: the documents hook {} failed ({}). Fix the documents it names, or set {}=1 to skip it",
        hook.display(),
        status,
        SKIP_VAR
    ));
    Err(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn script(dir: &Path, body: &str, mode: u32) -> PathBuf {
        let file = dir.join("hook");
        std::fs::write(&file, body).unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(mode)).unwrap();
        file
    }

    #[test]
    fn a_missing_hook_runs_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let run = run_hook(&dir.path().join("none"), dir.path(), &["a.md".into()]).unwrap();
        assert_eq!(run, HookRun::NotRun);
    }

    #[test]
    fn a_failing_hook_returns_its_output_and_a_non_executable_one_names_its_path() {
        let dir = tempfile::tempdir().unwrap();
        let hook = script(
            dir.path(),
            "#!/bin/sh\necho found a problem\nexit 3\n",
            0o755,
        );
        let err = run_hook(&hook, dir.path(), &["a.md".into()]).unwrap_err();
        assert!(err.starts_with("found a problem\n"), "{}", err);
        assert!(err.contains("exit status 3"), "{}", err);
        let hook = script(dir.path(), "#!/bin/sh\nexit 0\n", 0o644);
        let err = run_hook(&hook, dir.path(), &["a.md".into()]).unwrap_err();
        assert!(err.contains(&hook.display().to_string()), "{}", err);
    }
}
