//! Updates tsk and re-runs the command when the ledger is at a layout this tsk does not
//! support. See `docs/domain/ledger-layout.md` (Manifest version).
//!
//! Only the unsupported-layout error (`manifest::unsupported_layout`) starts an update.
//! The hook commands never update. `TSK_AUTO_UPDATE=never` and `TSK_NO_UPDATE_CHECK`
//! switch it off. A tsk that is newer than the ledger reads the old layout and upgrades
//! it on its first write, so nothing here runs for that direction.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::ledger::manifest::{self, DOLT_SUPPORTED_VERSIONS};
use crate::self_update::{self, Version};

/// `never` switches the automatic update off. Any other value, or none, leaves it on.
pub const AUTO_UPDATE_VAR: &str = "TSK_AUTO_UPDATE";
/// Set to `1` on the re-run, so a tsk that still fails does not update again.
pub const UPDATED_VAR: &str = "TSK_AUTO_UPDATED";
/// A script that the update step runs instead of the real update. Honoured only when
/// `TSK_TEST_MODE=1` is also set. It gets the path of the installed tsk as `$1`.
pub const TEST_UPDATE_CMD_VAR: &str = "TSK_SELF_UPDATE_CMD";
pub const TEST_MODE_VAR: &str = "TSK_TEST_MODE";

/// True when the environment allows an automatic update.
pub fn enabled(auto_update: Option<&str>, no_update_check: Option<&str>) -> bool {
    if no_update_check.is_some_and(|v| !v.is_empty() && v != "0") {
        return false;
    }
    !auto_update.is_some_and(|v| v.trim().eq_ignore_ascii_case("never"))
}

/// True when the loop guard is set: an update already ran for this command.
pub fn already_updated(updated: Option<&str>) -> bool {
    updated.is_some_and(|v| !v.is_empty() && v != "0")
}

/// The highest layout this tsk supports.
fn supported_max() -> i64 {
    DOLT_SUPPORTED_VERSIONS.iter().copied().max().unwrap_or(0)
}

pub fn warning(layout: i64, running: &str, supported: i64) -> String {
    format!(
        "warning: this ledger is at layout {} and this tsk ({}) supports up to {}. Updating tsk now.",
        layout, running, supported
    )
}

pub fn still_unsupported_line(layout: i64) -> String {
    format!(
        "tsk was updated but still does not support layout {}; update it by hand: tsk self-update",
        layout
    )
}

/// What `handle` decides for a failed command.
#[derive(Debug, PartialEq, Eq)]
pub enum Decision {
    /// Print the error as before.
    Keep,
    /// Print the error, then the extra line (the guard stopped a second update).
    KeepWithLine(String),
    /// Warn, update and re-run. Holds the layout.
    Update(i64),
}

/// Whether a failed command starts an update. `hook` is true for the hook commands.
pub fn decide(
    error: &str,
    hook: bool,
    auto_update: Option<&str>,
    no_update_check: Option<&str>,
    updated: Option<&str>,
) -> Decision {
    let Some(layout) = manifest::unsupported_layout(error) else {
        return Decision::Keep;
    };
    if layout <= supported_max() || hook || !enabled(auto_update, no_update_check) {
        return Decision::Keep;
    }
    if already_updated(updated) {
        return Decision::KeepWithLine(still_unsupported_line(layout));
    }
    Decision::Update(layout)
}

/// Handles the error of a failed command. Returns when the caller prints the error and
/// exits 1: `Some(line)` is an extra line to print after the error. On a successful
/// update it does not return: it replaces the process with the re-run command.
pub fn handle(error: &str, hook: bool) -> Option<String> {
    let var = |name: &str| std::env::var(name).ok();
    let layout = match decide(
        error,
        hook,
        var(AUTO_UPDATE_VAR).as_deref(),
        var(self_update::NO_CHECK_VAR).as_deref(),
        var(UPDATED_VAR).as_deref(),
    ) {
        Decision::Keep => return None,
        Decision::KeepWithLine(line) => return Some(line),
        Decision::Update(layout) => layout,
    };
    eprintln!(
        "{}",
        warning(layout, env!("CARGO_PKG_VERSION"), supported_max())
    );
    match update_and_rerun() {
        Ok(never) => match never {},
        Err(e) => {
            eprintln!("{}", e);
            None
        }
    }
}

/// Where the installed tsk can be found again after an update. Homebrew removes the
/// versioned Cellar path of the old binary, so the path the command was invoked by (a
/// symlink that the update repoints) comes first. The canonical path of the running
/// binary comes second, and is the one a Cargo or release binary update replaces.
fn installed_candidates() -> Vec<PathBuf> {
    let mut found = Vec::new();
    if let Some(argv0) = std::env::args_os().next() {
        if let Some(path) = resolve_invoked(&argv0) {
            found.push(path);
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        let text = exe.to_string_lossy().into_owned();
        let exe = PathBuf::from(text.strip_suffix(" (deleted)").unwrap_or(&text));
        found.push(std::fs::canonicalize(&exe).unwrap_or(exe));
    }
    found
}

/// `argv[0]` as an absolute path: itself when it holds a slash, otherwise the first
/// match on `PATH`.
fn resolve_invoked(argv0: &OsString) -> Option<PathBuf> {
    let path = Path::new(argv0);
    if path.components().count() > 1 {
        return std::path::absolute(path).ok();
    }
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|dir| dir.join(path))
        .find(|candidate| candidate.is_file())
}

fn version_of(binary: &Path) -> Option<Version> {
    let out = Command::new(binary).arg("--version").output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    Version::parse(text.split_whitespace().last()?)
}

/// Runs the update step: the test script when the test hook is honoured, otherwise the
/// code of `tsk self-update`. Output goes to stderr.
fn run_update(candidates: &[PathBuf]) -> Result<(), String> {
    let script = std::env::var_os(TEST_UPDATE_CMD_VAR).filter(|v| !v.is_empty());
    if let (Some(script), Some("1")) = (script, std::env::var(TEST_MODE_VAR).ok().as_deref()) {
        let target = candidates.last().cloned().unwrap_or_default();
        let status = Command::new(&script)
            .arg(&target)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::from(std::io::stderr()))
            .status()
            .map_err(|e| format!("error: could not run the update script: {}", e))?;
        return if status.success() {
            Ok(())
        } else {
            Err(format!("error: the update script failed ({})", status))
        };
    }
    match self_update::update_to_latest()? {
        Some(done) => {
            eprintln!(
                "updated tsk {} -> {} ({})",
                done.from,
                done.to,
                done.method.label()
            );
            Ok(())
        }
        None => Err("error: tsk is already the latest release".to_string()),
    }
}

/// Updates tsk, checks that the installed version rose, and replaces this process with
/// the original command line run by the updated binary. Returns only an error.
fn update_and_rerun() -> Result<std::convert::Infallible, String> {
    let before = self_update::running_version()?;
    let candidates = installed_candidates();
    run_update(&candidates)?;
    let (binary, after) = candidates
        .iter()
        .find_map(|c| version_of(c).map(|v| (c.clone(), v)))
        .ok_or_else(|| "error: could not find the updated tsk to run again".to_string())?;
    if after <= before {
        return Err(format!(
            "error: the update left tsk at {}, not newer than {}",
            after, before
        ));
    }
    rerun(&binary)
}

#[cfg(unix)]
fn rerun(binary: &Path) -> Result<std::convert::Infallible, String> {
    use std::os::unix::process::CommandExt;
    let error = Command::new(binary)
        .args(std::env::args_os().skip(1))
        .env(UPDATED_VAR, "1")
        .exec();
    Err(format!(
        "error: could not run {}: {}",
        binary.display(),
        error
    ))
}

#[cfg(not(unix))]
fn rerun(binary: &Path) -> Result<std::convert::Infallible, String> {
    let status = Command::new(binary)
        .args(std::env::args_os().skip(1))
        .env(UPDATED_VAR, "1")
        .status()
        .map_err(|e| format!("error: could not run {}: {}", binary.display(), e))?;
    std::process::exit(status.code().unwrap_or(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ERR: &str = "error: unsupported ledger version 9 in the manifest; this tsk supports version 1, 2, 3, 4, 5, 6, 7, 8. Update tsk";

    #[test]
    fn the_switches_turn_the_update_off() {
        assert!(enabled(None, None));
        assert!(enabled(Some("always"), Some("0")));
        assert!(enabled(Some("other"), Some("")));
        assert!(!enabled(Some("never"), None));
        assert!(!enabled(Some(" NEVER "), None));
        assert!(!enabled(None, Some("1")));
    }

    #[test]
    fn an_unsupported_layout_above_the_supported_ones_starts_an_update() {
        assert_eq!(decide(ERR, false, None, None, None), Decision::Update(9));
    }

    #[test]
    fn the_loop_guard_stops_a_second_update_and_adds_the_line() {
        assert_eq!(
            decide(ERR, false, None, None, Some("1")),
            Decision::KeepWithLine(still_unsupported_line(9))
        );
        assert!(still_unsupported_line(9)
            .starts_with("tsk was updated but still does not support layout 9;"));
    }

    #[test]
    fn hooks_opt_outs_and_other_errors_never_update() {
        assert_eq!(decide(ERR, true, None, None, None), Decision::Keep);
        assert_eq!(
            decide(ERR, false, Some("never"), None, None),
            Decision::Keep
        );
        assert_eq!(decide(ERR, false, None, Some("1"), None), Decision::Keep);
        for other in [
            "error: could not reach origin",
            "error: merge conflict in table missions",
        ] {
            assert_eq!(decide(other, false, None, None, None), Decision::Keep);
        }
    }

    #[test]
    fn a_layout_this_tsk_supports_is_not_a_newer_ledger() {
        let old = ERR.replace("version 9", "version 0");
        assert_eq!(decide(&old, false, None, None, None), Decision::Keep);
    }

    #[test]
    fn the_warning_names_the_layouts_and_the_running_version() {
        assert_eq!(
            warning(9, "0.17.0", 8),
            "warning: this ledger is at layout 9 and this tsk (0.17.0) supports up to 8. Updating tsk now."
        );
    }
}
