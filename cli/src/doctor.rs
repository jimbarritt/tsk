//! `tsk doctor`: check the environment and print what is wrong, with the fix for each.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The version of the tsk that runs.
const VERSION: &str = env!("CARGO_PKG_VERSION");
const MIN_DOLT: (u64, u64, u64) = (2, 5, 0);

pub struct Check {
    pub name: &'static str,
    pub ok: bool,
    pub detail: String,
    pub fix: Option<String>,
}

impl Check {
    fn pass(name: &'static str, detail: impl Into<String>) -> Check {
        Check {
            name,
            ok: true,
            detail: detail.into(),
            fix: None,
        }
    }

    fn fail(name: &'static str, detail: impl Into<String>, fix: impl Into<String>) -> Check {
        Check {
            name,
            ok: false,
            detail: detail.into(),
            fix: Some(fix.into()),
        }
    }
}

/// A `tsk` file found on PATH, in PATH order, with the version it prints.
pub struct Found {
    pub path: PathBuf,
    pub version: Option<String>,
}

/// Every distinct `tsk` on PATH, first the one a shell runs.
pub fn tsk_on_path(path_var: Option<std::ffi::OsString>) -> Vec<Found> {
    let mut seen: Vec<PathBuf> = Vec::new();
    let mut found = Vec::new();
    let Some(path_var) = path_var else {
        return found;
    };
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join("tsk");
        if !candidate.is_file() {
            continue;
        }
        let real = std::fs::canonicalize(&candidate).unwrap_or_else(|_| candidate.clone());
        if seen.contains(&real) {
            continue;
        }
        seen.push(real);
        let version = version_of(&candidate);
        found.push(Found {
            path: candidate,
            version,
        });
    }
    found
}

fn version_of(bin: &Path) -> Option<String> {
    let output = Command::new(bin)
        .arg("--version")
        .env("TSK_AUTO_UPDATE", "never")
        .env("TSK_NO_UPDATE_CHECK", "1")
        .output()
        .ok()?;
    parse_tsk_version(&String::from_utf8_lossy(&output.stdout))
}

/// `tsk 0.32.0` gives `0.32.0`.
pub fn parse_tsk_version(text: &str) -> Option<String> {
    let word = text.split_whitespace().nth(1)?;
    word.chars()
        .next()
        .filter(|c| c.is_ascii_digit())
        .map(|_| word.to_string())
}

/// `dolt version 2.5.1` gives `(2, 5, 1)`.
pub fn parse_dolt_version(text: &str) -> Option<(u64, u64, u64)> {
    let word = text.split_whitespace().nth(2)?;
    let mut parts = word.split(|c: char| !c.is_ascii_digit());
    Some((
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
    ))
}

/// The check of the `tsk` files on PATH.
pub fn path_check(found: &[Found]) -> Check {
    let describe = |f: &Found| {
        format!(
            "{} ({})",
            f.path.display(),
            f.version.as_deref().unwrap_or("version unknown")
        )
    };
    match found {
        [] => Check::fail(
            "tsk on PATH",
            "no tsk on PATH",
            "install tsk: brew install jimbarritt/tap/tsk, or download a release archive",
        ),
        [only] => Check::pass("tsk on PATH", describe(only)),
        [first, others @ ..] => Check::fail(
            "tsk on PATH",
            format!(
                "{} tsk files on PATH. The one that runs is {}. Others: {}",
                found.len(),
                describe(first),
                others.iter().map(describe).collect::<Vec<_>>().join(", ")
            ),
            "remove the stale copies (for example cargo uninstall tsk, or delete the file), so the one you want is the only one on PATH",
        ),
    }
}

/// A sentence for `tsk self-update` when more than one `tsk` is on PATH, else `None`.
pub fn shadow_note(found: &[Found]) -> Option<String> {
    if found.len() < 2 {
        return None;
    }
    let others: Vec<String> = found[1..]
        .iter()
        .map(|f| {
            format!(
                "{} ({})",
                f.path.display(),
                f.version.as_deref().unwrap_or("version unknown")
            )
        })
        .collect();
    Some(format!(
        "note: more than one tsk is on PATH. {} runs. Others: {}. Run tsk doctor for the fix.",
        found[0].path.display(),
        others.join(", ")
    ))
}

fn tool_check(name: &'static str, program: &str, args: &[&str], fix: &str) -> Check {
    match Command::new(program).args(args).output() {
        Ok(output) if output.status.success() => Check::pass(
            name,
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .next()
                .unwrap_or(program)
                .trim()
                .to_string(),
        ),
        _ => Check::fail(name, format!("{} is not installed", program), fix),
    }
}

fn dolt_check() -> Check {
    let fix = "install dolt 2.5.0 or later: plugin/hooks/install-dolt.sh, or https://docs.dolthub.com/introduction/installation";
    match Command::new("dolt").arg("version").output() {
        Ok(output) if output.status.success() => {
            let text = String::from_utf8_lossy(&output.stdout).to_string();
            match parse_dolt_version(&text) {
                Some(v) if v >= MIN_DOLT => Check::pass("dolt", text.lines().next().unwrap_or("")),
                Some(v) => Check::fail(
                    "dolt",
                    format!("dolt {}.{}.{} is older than 2.5.0", v.0, v.1, v.2),
                    fix,
                ),
                None => Check::fail("dolt", format!("could not read: {}", text.trim()), fix),
            }
        }
        _ => Check::fail("dolt", "dolt is not installed", fix),
    }
}

fn git_checks() -> Vec<Check> {
    let mut git = Command::new("git");
    git.args(["rev-parse", "--show-toplevel"]);
    crate::gitenv::apply(&mut git);
    let mut checks = Vec::new();
    match git.output() {
        Err(_) => {
            checks.push(Check::fail("git", "git is not installed", "install git"));
            return checks;
        }
        Ok(output) if !output.status.success() => {
            checks.push(Check::pass("git", "installed"));
            checks.push(Check::fail(
                "git repo",
                "this directory is not in a git repository",
                "run tsk doctor inside the repo that uses tsk",
            ));
            return checks;
        }
        Ok(output) => {
            checks.push(Check::pass("git", "installed"));
            checks.push(Check::pass(
                "git repo",
                String::from_utf8_lossy(&output.stdout).trim().to_string(),
            ));
        }
    }
    let mut remote = Command::new("git");
    remote.args(["remote", "get-url", "origin"]);
    crate::gitenv::apply(&mut remote);
    match remote.output() {
        Ok(o) if o.status.success() => checks.push(Check::pass(
            "origin remote",
            String::from_utf8_lossy(&o.stdout).trim().to_string(),
        )),
        _ => checks.push(Check::fail(
            "origin remote",
            "the repo has no origin remote",
            "git remote add origin <url>: the ledger is published to origin",
        )),
    }
    match crate::ledger::refs_cmd::summary() {
        Ok(text) => checks.push(Check::pass("ledger refs", text)),
        Err(e) => checks.push(Check::fail(
            "ledger refs",
            e.trim_start_matches("error: ").to_string(),
            "run tsk config ledger-refs for the detail",
        )),
    }
    checks
}

/// The plugin versions in the Claude Code plugin file, for plugins named `tsk@...`.
pub fn plugin_versions(file_text: &str) -> Vec<String> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(file_text) else {
        return Vec::new();
    };
    let Some(plugins) = value.get("plugins").and_then(|p| p.as_object()) else {
        return Vec::new();
    };
    plugins
        .iter()
        .filter(|(name, _)| name.starts_with("tsk@"))
        .flat_map(|(_, entries)| entries.as_array().cloned().unwrap_or_default())
        .filter_map(|e| e.get("version").and_then(|v| v.as_str()).map(String::from))
        .collect()
}

fn plugin_check() -> Check {
    let expected = crate::thread::session_start::EXPECTED_PLUGIN_VERSION;
    let fix = "run tsk install-plugin claude-cli inside the repo";
    let Some(home) = std::env::var_os("HOME") else {
        return Check::fail("plugin", "HOME is not set", fix);
    };
    let file = PathBuf::from(home).join(".claude/plugins/installed_plugins.json");
    let Ok(text) = std::fs::read_to_string(&file) else {
        return Check::fail(
            "plugin",
            format!(
                "{} is not readable: no Claude Code plugin found",
                file.display()
            ),
            fix,
        );
    };
    let versions = plugin_versions(&text);
    if versions.is_empty() {
        return Check::fail("plugin", "the tsk plugin is not installed", fix);
    }
    match crate::thread::session_start::plugin_version_note(
        versions.iter().max().map(String::as_str),
        expected,
    ) {
        Some(_) => Check::fail(
            "plugin",
            format!(
                "the tsk plugin is {}, this tsk expects {}",
                versions.join(", "),
                expected
            ),
            "run tsk install-plugin claude-cli to update it",
        ),
        None => Check::pass(
            "plugin",
            format!(
                "tsk plugin {} (tsk expects {})",
                versions.join(", "),
                expected
            ),
        ),
    }
}

fn xcode_check() -> Option<Check> {
    if !cfg!(target_os = "macos") {
        return None;
    }
    let output = Command::new("xcodebuild")
        .args(["-license", "check"])
        .output()
        .ok()?;
    Some(if output.status.success() {
        Check::pass("Xcode licence", "accepted")
    } else {
        Check::fail(
            "Xcode licence",
            "the Xcode licence is not accepted, and a brew step can fail on it",
            "run sudo xcodebuild -license accept",
        )
    })
}

fn server_check() -> Option<Check> {
    let cwd = std::env::current_dir().ok()?;
    let store = crate::ledger::store::open(&cwd).ok()?;
    let path = store.path().ok()?;
    let status = crate::ledger::store::server::status(&path);
    let (on, _) = crate::ledger::store::server::switch();
    Some(match (status.state, on) {
        ("running", _) => Check::pass("Dolt server", "running"),
        (state, true) => Check::pass(
            "Dolt server",
            format!(
                "{} (it starts on the next tsk command or session start)",
                state
            ),
        ),
        (_, false) => Check::pass("Dolt server", "switched off"),
    })
}

pub fn checks() -> Vec<Check> {
    let mut checks = vec![path_check(&tsk_on_path(std::env::var_os("PATH")))];
    checks.push(dolt_check());
    checks.push(tool_check(
        "jq",
        "jq",
        &["--version"],
        "install jq (brew install jq, or apt-get install jq)",
    ));
    checks.extend(git_checks());
    checks.push(plugin_check());
    checks.extend(xcode_check());
    checks.extend(server_check());
    checks
}

pub fn run() -> Result<i32, String> {
    println!("tsk {}", VERSION);
    let checks = checks();
    let mut failed = 0;
    for check in &checks {
        if check.ok {
            println!("ok    {}: {}", check.name, check.detail);
        } else {
            failed += 1;
            println!("FAIL  {}: {}", check.name, check.detail);
            if let Some(fix) = &check.fix {
                println!("      fix: {}", fix);
            }
        }
    }
    if failed == 0 {
        println!("all {} checks pass", checks.len());
        Ok(0)
    } else {
        println!("{} of {} checks fail", failed, checks.len());
        Ok(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(path: &str, version: Option<&str>) -> Found {
        Found {
            path: PathBuf::from(path),
            version: version.map(String::from),
        }
    }

    #[test]
    fn versions_are_read_from_the_tool_output() {
        assert_eq!(parse_tsk_version("tsk 0.32.0\n").as_deref(), Some("0.32.0"));
        assert_eq!(parse_tsk_version("nothing"), None);
        assert_eq!(parse_dolt_version("dolt version 2.5.1\n"), Some((2, 5, 1)));
        assert_eq!(parse_dolt_version("dolt version 2.10.0"), Some((2, 10, 0)));
        assert!(parse_dolt_version("dolt version 2.4.9").unwrap() < MIN_DOLT);
        assert_eq!(parse_dolt_version("garbage"), None);
    }

    #[test]
    fn one_tsk_on_path_passes_and_several_name_the_one_that_runs() {
        assert!(!path_check(&[]).ok);
        let one = path_check(&[found("/a/tsk", Some("0.32.0"))]);
        assert!(one.ok, "{}", one.detail);
        let two = path_check(&[
            found("/cargo/tsk", Some("0.2.1")),
            found("/brew/tsk", Some("0.32.0")),
        ]);
        assert!(!two.ok);
        assert!(two
            .detail
            .contains("The one that runs is /cargo/tsk (0.2.1)"));
        assert!(two.detail.contains("/brew/tsk (0.32.0)"));
        assert!(two.fix.unwrap().contains("remove the stale copies"));
    }

    #[test]
    fn the_shadow_note_needs_two_files() {
        assert_eq!(shadow_note(&[found("/a/tsk", None)]), None);
        let note = shadow_note(&[found("/a/tsk", Some("0.2.1")), found("/b/tsk", None)]).unwrap();
        assert!(note.contains("/a/tsk runs"), "{}", note);
        assert!(note.contains("/b/tsk (version unknown)"), "{}", note);
    }

    #[test]
    fn plugin_versions_come_from_the_tsk_entries_only() {
        let text = r#"{"version":2,"plugins":{
            "tsk@jimbarritt-claude-plugins":[{"scope":"project","version":"0.3.28"}],
            "swe@jimbarritt-claude-plugins":[{"scope":"project","version":"0.10.0"}]}}"#;
        assert_eq!(plugin_versions(text), vec!["0.3.28".to_string()]);
        assert!(plugin_versions("not json").is_empty());
    }
}
