use std::ffi::OsString;
use std::os::unix::fs::DirBuilderExt;
use std::path::{Component, Path, PathBuf};

use crate::config;
use crate::ledger::git::Git;

pub fn data_dir(xdg_data_home: Option<OsString>, home: Option<OsString>) -> Option<PathBuf> {
    let base = match xdg_data_home.filter(|v| !v.is_empty()) {
        Some(xdg) => PathBuf::from(xdg),
        None => PathBuf::from(home.filter(|v| !v.is_empty())?)
            .join(".local")
            .join("share"),
    };
    Some(base.join("tsk"))
}

pub fn default_path(xdg_data_home: Option<OsString>, home: Option<OsString>) -> Option<PathBuf> {
    data_dir(xdg_data_home, home).map(|dir| dir.join("nexus.git"))
}

pub fn resolve_path(
    arg: Option<&Path>,
    home: Option<&OsString>,
    default: Option<PathBuf>,
    cwd: &Path,
) -> Result<PathBuf, String> {
    let Some(arg) = arg else {
        return default.ok_or_else(|| {
            "error: neither XDG_DATA_HOME nor HOME is set; cannot locate the default nexus folder"
                .to_string()
        });
    };
    let text = arg.to_string_lossy();
    let expanded = if text == "~" || text.starts_with("~/") {
        let home = home
            .filter(|v| !v.is_empty())
            .ok_or_else(|| "error: HOME is not set; cannot expand ~".to_string())?;
        PathBuf::from(home).join(text.trim_start_matches('~').trim_start_matches('/'))
    } else {
        arg.to_path_buf()
    };
    Ok(if expanded.is_absolute() {
        expanded
    } else {
        cwd.join(expanded)
    })
}

pub fn normalise(path: &Path) -> PathBuf {
    let components: Vec<Component> = path.components().collect();
    for held in (1..=components.len()).rev() {
        let prefix: PathBuf = components[..held].iter().collect();
        let Ok(mut resolved) = prefix.canonicalize() else {
            continue;
        };
        for part in &components[held..] {
            match part {
                Component::ParentDir => {
                    resolved.pop();
                }
                Component::CurDir => {}
                other => resolved.push(other),
            }
        }
        return resolved;
    }
    path.to_path_buf()
}

fn same_nexus(attached: &str, url: &str) -> bool {
    match (
        attached.strip_prefix("file://"),
        url.strip_prefix("file://"),
    ) {
        (Some(a), Some(b)) => normalise(Path::new(a)) == normalise(Path::new(b)),
        _ => attached == url,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Found {
    Missing,
    EmptyDir,
    EmptyBare,
    UsedBare,
    Other(String),
}

pub fn inspect(path: &Path) -> Found {
    let Ok(meta) = std::fs::metadata(path) else {
        return Found::Missing;
    };
    if !meta.is_dir() {
        return Found::Other("a file".to_string());
    }
    match std::fs::read_dir(path) {
        Ok(mut entries) => {
            if entries.next().is_none() {
                return Found::EmptyDir;
            }
        }
        Err(e) => return Found::Other(format!("a folder that cannot be read ({})", e)),
    }
    let git = Git::new(path);
    let bare = git
        .run(["rev-parse", "--is-bare-repository"])
        .map(|text| text.trim() == "true")
        .unwrap_or(false);
    let own = git
        .run(["rev-parse", "--git-dir"])
        .map(|text| text.trim() == ".")
        .unwrap_or(false);
    if !(bare && own) {
        return Found::Other("a folder that is not empty and not a bare git repo".to_string());
    }
    match git.run(["for-each-ref"]) {
        Ok(refs) if refs.trim().is_empty() => Found::EmptyBare,
        Ok(_) => Found::UsedBare,
        Err(e) => Found::Other(format!("a bare git repo that cannot be read ({})", e)),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    pub path: PathBuf,
    pub url: String,
    pub found: Found,
    pub attached: Option<String>,
}

pub fn url_of(path: &Path) -> String {
    format!("file://{}", path.display())
}

pub fn plan(
    path: PathBuf,
    found: Found,
    attached: Option<String>,
    replace: bool,
) -> Result<Plan, String> {
    let url = url_of(&path);
    let is_attached = attached
        .as_deref()
        .is_some_and(|current| same_nexus(current, &url));
    let held = match &found {
        Found::Other(what) => Some(what.clone()),
        Found::UsedBare if !is_attached => Some("a bare git repo that holds refs".to_string()),
        _ => None,
    };
    if let Some(what) = held {
        return Err(format!(
            "error: {} exists and is {}; give a path that does not exist or is an empty folder or an empty bare git repo",
            path.display(),
            what
        ));
    }
    if let Some(current) = &attached {
        if !is_attached && !replace {
            return Err(format!(
                "error: a nexus is already attached: {}; rerun with --replace to attach the new one in its place",
                current
            ));
        }
    }
    Ok(Plan {
        path,
        url,
        found,
        attached,
    })
}

impl Plan {
    pub fn lines(&self) -> Vec<String> {
        let mut lines = vec!["About to:".to_string()];
        match self.found {
            Found::Missing => lines.push(format!(
                "  create {} as a bare git repo on the branch main (the folder above it is created at mode 700 when it is missing)",
                self.path.display()
            )),
            Found::EmptyDir => lines.push(format!(
                "  make {} a bare git repo on the branch main",
                self.path.display()
            )),
            Found::UsedBare => lines.push(format!(
                "  keep {}, a bare git repo that holds the attached nexus",
                self.path.display()
            )),
            _ => lines.push(format!(
                "  keep {}, an empty bare git repo",
                self.path.display()
            )),
        }
        match &self.attached {
            Some(current) if same_nexus(current, &self.url) => {
                lines.push(format!("  keep {} as the attached nexus", self.url))
            }
            Some(current) => lines.push(format!(
                "  attach {} as the nexus, replacing {} (its ledgers stay there and are not moved)",
                self.url, current
            )),
            None => lines.push(format!("  attach {} as the nexus", self.url)),
        }
        lines
    }
}

fn create(plan: &Plan) -> Result<(), String> {
    if plan.found == Found::EmptyBare {
        return Git::new(&plan.path)
            .run(["symbolic-ref", "HEAD", "refs/heads/main"])
            .map(|_| ())
            .map_err(|e| {
                format!(
                    "error: could not set HEAD of {}: {}",
                    plan.path.display(),
                    e
                )
            });
    }
    if plan.found == Found::UsedBare {
        return Ok(());
    }
    let parent = plan
        .path
        .parent()
        .ok_or_else(|| format!("error: {} has no parent folder", plan.path.display()))?;
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(parent)
        .map_err(|e| format!("error: could not create {}: {}", parent.display(), e))?;
    let path = plan.path.to_string_lossy().into_owned();
    Git::new(parent)
        .run(["init", "--quiet", "--bare", "--initial-branch=main", &path])
        .map(|_| ())
        .map_err(|e| {
            format!(
                "{}\nnote: git init --initial-branch needs git 2.28 or later",
                e
            )
        })
}

pub fn init_local(path_arg: Option<PathBuf>, yes: bool, replace: bool) -> Result<(), String> {
    let file = config::required_config_file()?;
    let home = std::env::var_os("HOME");
    let default = default_path(std::env::var_os("XDG_DATA_HOME"), home.clone());
    let cwd = std::env::current_dir()
        .map_err(|e| format!("error: could not read the current directory: {}", e))?;
    let path = normalise(&resolve_path(
        path_arg.as_deref(),
        home.as_ref(),
        default,
        &cwd,
    )?);
    let attached = config::read_nexus_url(&file)?;
    let plan = plan(path.clone(), inspect(&path), attached, replace)?;
    let ready = matches!(plan.found, Found::EmptyBare | Found::UsedBare);
    if ready
        && plan
            .attached
            .as_deref()
            .is_some_and(|c| same_nexus(c, &plan.url))
    {
        create(&plan)?;
        println!(
            "already set up: {} is the attached nexus",
            plan.path.display()
        );
        return Ok(());
    }
    for line in plan.lines() {
        println!("{}", line);
    }
    if !yes {
        let go = crate::harness::ask_yes_no("Go on? [y/n]")
            .map_err(|_| "error: no answer was given on stdin; rerun with --yes".to_string())?;
        if !go {
            println!("nothing changed");
            return Ok(());
        }
    }
    create(&plan)?;
    if let Err(e) = config::attach_and_report(&file, &plan.url) {
        return Err(format!(
            "{}\nnote: the folder {} is made; attach it with: tsk nexus add {}",
            e,
            plan.path.display(),
            plan.url
        ));
    }
    println!(
        "next: run tsk ledger init --nexus --prefix <PREFIX> in each repo, or /tsk:init-ledger. This folder holds the only copy of every ledger in it: see docs/user-guide/local-nexus.md for backup"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn os(text: &str) -> Option<OsString> {
        Some(OsString::from(text))
    }

    #[test]
    fn data_dir_prefers_xdg_then_home_and_ignores_an_empty_value() {
        assert_eq!(data_dir(os("/x"), os("/h")), Some(PathBuf::from("/x/tsk")));
        assert_eq!(
            data_dir(None, os("/h")),
            Some(PathBuf::from("/h/.local/share/tsk"))
        );
        assert_eq!(
            data_dir(os(""), os("/h")),
            Some(PathBuf::from("/h/.local/share/tsk"))
        );
        assert_eq!(data_dir(None, None), None);
        assert_eq!(
            default_path(os("/x"), None),
            Some(PathBuf::from("/x/tsk/nexus.git"))
        );
    }

    #[test]
    fn a_path_is_made_absolute_and_a_tilde_is_expanded() {
        let home = OsString::from("/h");
        let cwd = Path::new("/work");
        assert_eq!(
            resolve_path(Some(Path::new("~/n.git")), Some(&home), None, cwd).unwrap(),
            PathBuf::from("/h/n.git")
        );
        assert_eq!(
            resolve_path(Some(Path::new("n.git")), Some(&home), None, cwd).unwrap(),
            PathBuf::from("/work/n.git")
        );
        assert_eq!(
            resolve_path(Some(Path::new("/a/n.git")), None, None, cwd).unwrap(),
            PathBuf::from("/a/n.git")
        );
        assert!(resolve_path(None, None, None, cwd)
            .unwrap_err()
            .contains("XDG_DATA_HOME"));
        assert_eq!(url_of(Path::new("/a/n.git")), "file:///a/n.git");
    }

    #[test]
    fn the_plan_refuses_a_path_that_is_not_empty_and_accepts_the_empty_ones() {
        let path = PathBuf::from("/a/n.git");
        let refused = plan(
            path.clone(),
            Found::Other("a file".to_string()),
            None,
            false,
        )
        .unwrap_err();
        assert!(refused.contains("/a/n.git") && refused.contains("a file"));
        for found in [Found::Missing, Found::EmptyDir, Found::EmptyBare] {
            assert!(plan(path.clone(), found, None, false).is_ok());
        }
    }

    #[test]
    fn a_path_is_normalised_through_dots_and_links_of_its_existing_part() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().canonicalize().unwrap();
        std::fs::create_dir_all(real.join("a")).unwrap();
        std::os::unix::fs::symlink(real.join("a"), real.join("link")).unwrap();
        assert_eq!(
            normalise(&real.join("link/../a/n.git")),
            real.join("a/n.git")
        );
        assert_eq!(
            normalise(&real.join("link/new/../n.git")),
            real.join("a/n.git")
        );
        assert!(same_nexus(
            &format!("file://{}/link/n.git", real.display()),
            &format!("file://{}/a/n.git", real.display())
        ));
        assert!(!same_nexus("https://example.test/n", "file:///a/n.git"));
    }

    #[test]
    fn a_bare_repo_that_holds_refs_is_accepted_only_as_the_attached_nexus() {
        let path = PathBuf::from("/a/n.git");
        assert!(plan(path.clone(), Found::UsedBare, None, false)
            .unwrap_err()
            .contains("holds refs"));
        assert!(plan(
            path,
            Found::UsedBare,
            Some("file:///a/n.git".to_string()),
            false
        )
        .is_ok());
    }

    #[test]
    fn another_attached_nexus_needs_replace_and_the_same_one_is_unchanged() {
        let path = PathBuf::from("/a/n.git");
        let other = Some("https://example.test/n".to_string());
        let refused = plan(path.clone(), Found::Missing, other.clone(), false).unwrap_err();
        assert!(refused.contains("--replace") && refused.contains("https://example.test/n"));
        let replaced = plan(path.clone(), Found::Missing, other, true).unwrap();
        assert!(replaced.lines().join("\n").contains("replacing"));
        let same = plan(
            path,
            Found::EmptyBare,
            Some("file:///a/n.git".to_string()),
            false,
        )
        .unwrap();
        assert!(same.lines().join("\n").contains("keep file:///a/n.git"));
    }
}
