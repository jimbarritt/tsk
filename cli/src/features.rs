use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub const SOFTWARE_ENGLISH: &str = "software-english";
pub const KNOWN: &[&str] = &[SOFTWARE_ENGLISH];

pub fn read_enabled(file: &Path) -> Result<Vec<String>, String> {
    let table = crate::config::read_table(file)?;
    let Some(features) = table.get("features") else {
        return Ok(Vec::new());
    };
    let bad = || {
        format!(
            "error: features.enabled in {} must be a list of strings",
            file.display()
        )
    };
    let Some(features) = features.as_table() else {
        return Err(format!(
            "error: features in {} must be a table",
            file.display()
        ));
    };
    match features.get("enabled") {
        None => Ok(Vec::new()),
        Some(toml::Value::Array(items)) => items
            .iter()
            .map(|v| v.as_str().map(str::to_string).ok_or_else(bad))
            .collect(),
        Some(_) => Err(bad()),
    }
}

pub fn is_enabled(file: Option<&Path>, name: &str) -> bool {
    file.and_then(|f| read_enabled(f).ok())
        .is_some_and(|list| list.iter().any(|f| f == name))
}

pub fn check_names(names: &[String]) -> Result<Vec<String>, String> {
    let mut out: Vec<String> = Vec::new();
    for name in names {
        let name = name.trim();
        if !KNOWN.contains(&name) {
            return Err(format!(
                "error: unknown feature {}. Known features: {}",
                name,
                KNOWN.join(", ")
            ));
        }
        if !out.iter().any(|n| n == name) {
            out.push(name.to_string());
        }
    }
    Ok(out)
}

pub fn write_enabled(file: &Path, names: &[String]) -> Result<(), String> {
    crate::config::read_table(file)?;
    let text = match std::fs::read_to_string(file) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(format!("error: could not read {}: {}", file.display(), e)),
    };
    let text = crate::config::with_string_list(&text, "features", "enabled", names)?;
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("error: could not create {}: {}", parent.display(), e))?;
    }
    std::fs::write(file, text)
        .map_err(|e| format!("error: could not write {}: {}", file.display(), e))
}

pub fn run_config(names: Vec<String>, none: bool) -> Result<(), String> {
    let file = crate::config::config_file_from_env();
    if none || !names.is_empty() {
        let names = if none {
            Vec::new()
        } else {
            check_names(&names)?
        };
        let file = crate::config::required_config_file()?;
        write_enabled(&file, &names)?;
        println!(
            "features enabled: {} (written to {})",
            list_text(&names),
            file.display()
        );
        return Ok(());
    }
    let enabled = match file.as_deref() {
        Some(file) => read_enabled(file)?,
        None => Vec::new(),
    };
    println!("features enabled: {}", list_text(&enabled));
    match file {
        Some(file) => println!("source: user config ({})", file.display()),
        None => println!("source: none"),
    }
    println!("known features: {}", KNOWN.join(", "));
    Ok(())
}

fn list_text(names: &[String]) -> String {
    if names.is_empty() {
        "none".to_string()
    } else {
        names.join(", ")
    }
}

pub fn find_installer(home: &Path) -> Option<PathBuf> {
    let cache = home.join(".claude").join("plugins").join("cache");
    let mut found: Vec<PathBuf> = Vec::new();
    for market in std::fs::read_dir(cache).ok()?.flatten() {
        let swe = market.path().join("swe");
        for version in std::fs::read_dir(swe).into_iter().flatten().flatten() {
            let script = version
                .path()
                .join("scripts")
                .join("install-commit-hook.sh");
            if script.is_file() {
                found.push(script);
            }
        }
    }
    found.sort_by_key(|p| version_key(p));
    found.pop()
}

fn version_key(script: &Path) -> Vec<u64> {
    let version = script
        .parent()
        .and_then(Path::parent)
        .and_then(Path::file_name)
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    version
        .split(|c: char| !c.is_ascii_digit())
        .filter_map(|p| p.parse().ok())
        .collect()
}

pub fn commit_hook_has_swe(repo: &Path) -> bool {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["rev-parse", "--git-path", "hooks"])
        .stderr(Stdio::null())
        .output();
    let Ok(out) = out else { return false };
    if !out.status.success() {
        return false;
    }
    let dir = PathBuf::from(String::from_utf8_lossy(&out.stdout).trim());
    let dir = if dir.is_absolute() {
        dir
    } else {
        repo.join(dir)
    };
    std::fs::read_to_string(dir.join("pre-commit")).is_ok_and(|t| t.contains("swe"))
}

fn is_git_checkout(repo: &Path) -> bool {
    Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["rev-parse", "--git-dir"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

pub fn install_swe_commit_hooks(home: &Path, top: &Path) -> Vec<PathBuf> {
    let Some(installer) = find_installer(home) else {
        return Vec::new();
    };
    let mut repos = vec![top.to_path_buf()];
    if let Some(parent) = top.parent() {
        let mut siblings: Vec<PathBuf> = std::fs::read_dir(parent)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir() && p != top)
            .collect();
        siblings.sort();
        repos.extend(siblings);
    }
    let mut installed = Vec::new();
    for repo in repos {
        if !is_git_checkout(&repo) || commit_hook_has_swe(&repo) {
            continue;
        }
        let ok = Command::new(&installer)
            .arg(&repo)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success());
        if ok {
            installed.push(repo);
        }
    }
    installed
}

pub fn run_session_start() {
    let file = crate::config::config_file_from_env();
    if !is_enabled(file.as_deref(), SOFTWARE_ENGLISH) {
        return;
    }
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        return;
    };
    let Ok(cwd) = std::env::current_dir() else {
        return;
    };
    let top = Command::new("git")
        .arg("-C")
        .arg(&cwd)
        .args(["rev-parse", "--show-toplevel"])
        .stderr(Stdio::null())
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| PathBuf::from(String::from_utf8_lossy(&o.stdout).trim()))
        .unwrap_or(cwd);
    install_swe_commit_hooks(&home, &top);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("tsk-features-{}-{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn absent_file_has_no_features() {
        let d = dir("absent");
        assert!(read_enabled(&d.join("config.toml")).unwrap().is_empty());
        assert!(!is_enabled(Some(&d.join("config.toml")), SOFTWARE_ENGLISH));
        assert!(!is_enabled(None, SOFTWARE_ENGLISH));
    }

    #[test]
    fn write_then_read_keeps_other_keys_and_comments() {
        let d = dir("write");
        let f = d.join("config.toml");
        std::fs::write(&f, "# mine\n[ledger]\nserver = \"off\"\n").unwrap();
        write_enabled(&f, &[SOFTWARE_ENGLISH.to_string()]).unwrap();
        let text = std::fs::read_to_string(&f).unwrap();
        assert!(text.contains("# mine"));
        assert!(text.contains("server = \"off\""));
        assert!(text.contains("enabled = [\"software-english\"]"));
        assert_eq!(read_enabled(&f).unwrap(), vec![SOFTWARE_ENGLISH]);
        assert!(is_enabled(Some(&f), SOFTWARE_ENGLISH));
        write_enabled(&f, &[]).unwrap();
        assert!(read_enabled(&f).unwrap().is_empty());
    }

    #[test]
    fn unknown_feature_is_refused() {
        let err = check_names(&["nope".to_string()]).unwrap_err();
        assert!(err.contains("unknown feature nope"));
        assert_eq!(
            check_names(&[SOFTWARE_ENGLISH.to_string(), SOFTWARE_ENGLISH.to_string()]).unwrap(),
            vec![SOFTWARE_ENGLISH]
        );
    }

    #[test]
    fn wrong_type_is_an_error() {
        let d = dir("wrong");
        let f = d.join("config.toml");
        std::fs::write(&f, "[features]\nenabled = \"software-english\"\n").unwrap();
        assert!(read_enabled(&f).unwrap_err().contains("list of strings"));
    }

    #[test]
    fn installer_with_the_highest_version_is_found() {
        let home = dir("installer");
        for v in ["0.9.0", "0.10.0"] {
            let s = home.join(format!(".claude/plugins/cache/m/swe/{}/scripts", v));
            std::fs::create_dir_all(&s).unwrap();
            std::fs::write(s.join("install-commit-hook.sh"), "#!/bin/sh\n").unwrap();
        }
        let found = find_installer(&home).unwrap();
        assert!(found.to_string_lossy().contains("0.10.0"));
        assert!(find_installer(&dir("none")).is_none());
    }

    #[test]
    fn hook_is_installed_in_repo_and_sibling_once() {
        let home = dir("hooks-home");
        let s = home.join(".claude/plugins/cache/m/swe/1.0.0/scripts");
        std::fs::create_dir_all(&s).unwrap();
        let script = s.join("install-commit-hook.sh");
        std::fs::write(
            &script,
            "#!/bin/sh\nd=$(git -C \"$1\" rev-parse --git-path hooks)\ncase \"$d\" in /*) ;; *) d=\"$1/$d\" ;; esac\nmkdir -p \"$d\"\necho swe > \"$d/pre-commit\"\n",
        )
        .unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let work = dir("hooks-work");
        for name in ["a", "b"] {
            let r = work.join(name);
            std::fs::create_dir_all(&r).unwrap();
            assert!(Command::new("git")
                .arg("-C")
                .arg(&r)
                .arg("init")
                .arg("-q")
                .env("GIT_CONFIG_COUNT", "0")
                .status()
                .unwrap()
                .success());
        }
        std::fs::create_dir_all(work.join("plain")).unwrap();
        let first = install_swe_commit_hooks(&home, &work.join("a"));
        assert_eq!(first.len(), 2);
        assert!(commit_hook_has_swe(&work.join("a")));
        assert!(commit_hook_has_swe(&work.join("b")));
        assert!(install_swe_commit_hooks(&home, &work.join("a")).is_empty());
    }
}
