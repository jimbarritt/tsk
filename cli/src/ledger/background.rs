//! The background publish (ADR 0020).
//!
//! `tsk ledger push --background` starts a detached one-shot child that runs the normal
//! push path, and returns at once. The child is not a daemon: it has no socket and no
//! loop, and it keeps no state beyond two files in the state directory of the clone.
//!
//! - `push.lock`: a lock taken with `flock` on a file descriptor. The parent takes it and
//!   the child inherits the descriptor, so the lock lives as long as the child does, and
//!   the kernel frees it when the child ends, however it ends. Every push of the clone
//!   (`tsk ledger push`, `thread start`, `pause` and `stop`) takes the same lock, so two
//!   pushes never write the store together.
//! - `background-push.result`: JSON with the state (`running`, `ok` or `failed`), the pid,
//!   the time, the commit the push left on origin, the error text, and whether a warning
//!   about the failure was shown.
//!
//! A `running` result whose lock nobody holds belongs to a child that was killed. It reads
//! as a failure named `interrupted`.

use std::fs::{File, OpenOptions};
use std::os::unix::io::{AsRawFd, FromRawFd};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use super::store::LedgerStore;
use crate::thread::clock::{format_utc, now_secs};

/// The default message of a background publish.
pub const DEFAULT_MESSAGE: &str = "Background publish";
/// `off` switches the background publish of the Stop hook off.
pub const SWITCH_VAR: &str = "TSK_BACKGROUND_PUBLISH";
/// How long a push waits for another push of the same clone, in seconds.
pub const LOCK_WAIT_VAR: &str = "TSK_PUSH_LOCK_SECS";
const DEFAULT_LOCK_WAIT_SECS: u64 = 60;

/// True in the child, which holds the push lock through the descriptor it inherited.
static HOLDS_LOCK: AtomicBool = AtomicBool::new(false);
/// True in commands that must not print the warning (the pushes themselves).
static QUIET: AtomicBool = AtomicBool::new(false);

/// Do not print the warning about an earlier failed background publish in this process.
pub fn be_quiet() {
    QUIET.store(true, Ordering::SeqCst);
}

pub fn enabled() -> bool {
    std::env::var(SWITCH_VAR).map_or(true, |v| v.trim() != "off")
}

pub struct Paths {
    pub dir: PathBuf,
    pub lock: PathBuf,
    pub result: PathBuf,
    pub log: PathBuf,
}

impl Paths {
    /// The files of the clone whose ledger store is the Dolt database at `db`.
    pub fn of_store(db: &Path) -> Paths {
        let dir = db
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        Paths {
            lock: dir.join("push.lock"),
            result: dir.join("background-push.result"),
            log: dir.join("background-push.log"),
            dir,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Outcome {
    /// `running`, `ok` or `failed`.
    pub state: String,
    #[serde(default)]
    pub pid: u32,
    #[serde(default)]
    pub time: u64,
    #[serde(default)]
    pub commit: String,
    #[serde(default)]
    pub error: String,
    #[serde(default)]
    pub shown: bool,
}

fn read_outcome(paths: &Paths) -> Option<Outcome> {
    let text = std::fs::read_to_string(&paths.result).ok()?;
    serde_json::from_str(&text).ok()
}

fn write_outcome(paths: &Paths, outcome: &Outcome) {
    let Ok(text) = serde_json::to_string(outcome) else {
        return;
    };
    let tmp = paths
        .dir
        .join(format!(".background-push.{}", std::process::id()));
    if std::fs::write(&tmp, text).is_ok() && std::fs::rename(&tmp, &paths.result).is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
}

fn open_lock(paths: &Paths) -> Result<File, String> {
    std::fs::create_dir_all(&paths.dir)
        .map_err(|e| format!("error: could not create {}: {}", paths.dir.display(), e))?;
    OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(&paths.lock)
        .map_err(|e| format!("error: could not open {}: {}", paths.lock.display(), e))
}

fn try_flock(file: &File) -> bool {
    unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) == 0 }
}

/// Whether some process holds the push lock.
fn lock_is_held(paths: &Paths) -> bool {
    match open_lock(paths) {
        Ok(file) => !try_flock(&file),
        Err(_) => false,
    }
}

/// The result as a reader should see it: a `running` result with no lock holder is the
/// result of a child that was killed.
fn effective(paths: &Paths) -> Option<Outcome> {
    let mut outcome = read_outcome(paths)?;
    if outcome.state == "running" && !lock_is_held(paths) {
        outcome = Outcome {
            state: "failed".to_string(),
            pid: outcome.pid,
            time: now_secs(),
            commit: String::new(),
            error: "interrupted: the background publish ended before it finished".to_string(),
            shown: false,
        };
        write_outcome(paths, &outcome);
    }
    Some(outcome)
}

/// Print one warning line on stderr when the last background publish failed and no
/// warning was shown for it yet. Reads one small file and prints nothing otherwise.
pub fn warn_once(db: &Path) {
    if QUIET.load(Ordering::SeqCst) {
        return;
    }
    let paths = Paths::of_store(db);
    if !paths.result.exists() {
        return;
    }
    let Some(mut outcome) = effective(&paths) else {
        return;
    };
    if outcome.state != "failed" || outcome.shown {
        return;
    }
    eprintln!(
        "{}",
        warning_text(&format_utc(outcome.time), &outcome.error)
    );
    outcome.shown = true;
    write_outcome(&paths, &outcome);
}

/// A foreground push succeeded: an earlier failure is no longer true.
pub fn clear_failure(db: &Path) {
    let paths = Paths::of_store(db);
    if let Some(outcome) = read_outcome(&paths) {
        if outcome.state == "failed" {
            let _ = std::fs::remove_file(&paths.result);
        }
    }
}

/// How many lines of the error the warning shows.
const WARNING_LINES: usize = 20;

/// The warning for a failed publish. An error of one line reads on one line. A longer one
/// (the stderr of a failed command, the output of the documents hook) shows its first
/// lines, so the cause is visible.
fn warning_text(time: &str, error: &str) -> String {
    let error = error.trim();
    let error = error.strip_prefix("error: ").unwrap_or(error);
    let lines: Vec<&str> = error.lines().collect();
    if lines.len() <= 1 {
        return format!(
            "warning: the last background publish failed at {}: {}; run tsk ledger push",
            time,
            first_line(error)
        );
    }
    let mut text = format!("warning: the last background publish failed at {}:\n", time);
    for line in lines.iter().take(WARNING_LINES) {
        text.push_str("  ");
        text.push_str(line.trim_end());
        text.push('\n');
    }
    if lines.len() > WARNING_LINES {
        text.push_str(&format!(
            "  ({} more lines are in background-push.log)\n",
            lines.len() - WARNING_LINES
        ));
    }
    text.push_str("Run tsk ledger push");
    text
}

fn first_line(text: &str) -> String {
    let line = text
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("unknown error");
    line.strip_prefix("error: ").unwrap_or(line).to_string()
}

/// The line `tsk ledger status` prints, or `None` when no background publish ran.
pub fn status_line(db: &Path) -> Option<String> {
    let paths = Paths::of_store(db);
    let outcome = effective(&paths)?;
    let text = match outcome.state.as_str() {
        "running" => "running".to_string(),
        "ok" => format!("ok at {}", format_utc(outcome.time)),
        _ => format!(
            "failed at {}: {}",
            format_utc(outcome.time),
            first_line(&outcome.error)
        ),
    };
    Some(format!("Background publish: {}", text))
}

/// Held while a push runs.
pub struct PushLock {
    _file: Option<File>,
}

/// Take the push lock of the clone, waiting for another push. The child of a background
/// publish already holds it.
pub fn lock_for_push(db: &Path) -> Result<PushLock, String> {
    if HOLDS_LOCK.load(Ordering::SeqCst) {
        return Ok(PushLock { _file: None });
    }
    let paths = Paths::of_store(db);
    let file = open_lock(&paths)?;
    let wait = std::env::var(LOCK_WAIT_VAR)
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .unwrap_or(DEFAULT_LOCK_WAIT_SECS);
    let end = Instant::now() + Duration::from_secs(wait);
    loop {
        if try_flock(&file) {
            return Ok(PushLock { _file: Some(file) });
        }
        if Instant::now() >= end {
            return Err(format!(
                "error: another push of this ledger (a background publish, say) holds {}.\n       Try again when it ends",
                paths.lock.display()
            ));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Started {
    /// The store holds nothing that origin lacks.
    NothingToPush,
    /// A push holds the lock; it will carry the changes.
    Busy,
    /// A child was started.
    Child(u32),
}

/// Start a background publish when the store holds unpushed changes. Asks origin nothing
/// and waits for nothing.
pub fn start(store: &dyn LedgerStore, message: &str) -> Result<Started, String> {
    if !store.exists_locally()? {
        return Ok(Started::NothingToPush);
    }
    let db = store.path()?;
    let paths = Paths::of_store(&db);
    let lock = open_lock(&paths)?;
    if !try_flock(&lock) {
        return Ok(Started::Busy);
    }
    if !store.status_local()?.has_unpushed() {
        return Ok(Started::NothingToPush);
    }
    // The lock is held while the result says `running`, so that no reader takes it for
    // the result of a killed child.
    write_outcome(
        &paths,
        &Outcome {
            state: "running".to_string(),
            time: now_secs(),
            ..Outcome::default()
        },
    );
    match spawn(&paths, &lock, message) {
        Ok(pid) => Ok(Started::Child(pid)),
        Err(e) => {
            write_outcome(
                &paths,
                &Outcome {
                    state: "failed".to_string(),
                    time: now_secs(),
                    error: e.clone(),
                    ..Outcome::default()
                },
            );
            Err(e)
        }
    }
}

fn spawn(paths: &Paths, lock: &File, message: &str) -> Result<u32, String> {
    let exe = std::env::current_exe()
        .map_err(|e| format!("error: could not find the tsk binary: {}", e))?;
    let log = File::create(&paths.log)
        .map_err(|e| format!("error: could not open {}: {}", paths.log.display(), e))?;
    let log_err = log
        .try_clone()
        .map_err(|e| format!("error: could not open {}: {}", paths.log.display(), e))?;
    let fd = lock.as_raw_fd();
    let mut command = Command::new(exe);
    command
        .args(["ledger", "background-push-run", "--lock-fd"])
        .arg(fd.to_string())
        .arg(message)
        .stdin(Stdio::null())
        .stdout(log)
        .stderr(log_err);
    // Its own session, so no signal for the caller's process group reaches it, and the
    // lock descriptor stays open across the exec. The child shares the open file
    // description, so the lock outlives the parent's copy of the descriptor.
    unsafe {
        command.pre_exec(move || {
            libc::setsid();
            libc::fcntl(fd, libc::F_SETFD, 0);
            Ok(())
        });
    }
    let child = command
        .spawn()
        .map_err(|e| format!("error: could not start the background publish: {}", e))?;
    Ok(child.id())
}

/// The body of the child: `tsk ledger background-push-run`. Runs the normal push and
/// records the result. The lock is held through descriptor `lock_fd`.
pub fn run_child(cwd: &Path, lock_fd: i32, message: &str) -> Result<(), String> {
    // From here the lock belongs to this process alone: a process it starts (the Dolt
    // server, git) must not inherit it.
    unsafe {
        libc::fcntl(lock_fd, libc::F_SETFD, libc::FD_CLOEXEC);
    }
    // Keep the descriptor open for the whole run.
    let _lock = unsafe { File::from_raw_fd(lock_fd) };
    HOLDS_LOCK.store(true, Ordering::SeqCst);
    be_quiet();

    let paths = db_path(cwd).map(|db| Paths::of_store(&db));
    let record = |outcome: Outcome| {
        if let Some(paths) = &paths {
            write_outcome(paths, &outcome);
        }
    };
    let result = (|| {
        let store = super::store::open(cwd)?;
        record(Outcome {
            state: "running".to_string(),
            pid: std::process::id(),
            time: now_secs(),
            ..Outcome::default()
        });
        store.trust_last_fetch();
        super::push_reporting(store.as_ref(), message)
    })();
    match result {
        Ok(report) => {
            record(Outcome {
                state: "ok".to_string(),
                pid: std::process::id(),
                time: now_secs(),
                commit: report.head,
                ..Outcome::default()
            });
            Ok(())
        }
        Err(e) => {
            record(Outcome {
                state: "failed".to_string(),
                pid: std::process::id(),
                time: now_secs(),
                error: e.clone(),
                ..Outcome::default()
            });
            Err(e)
        }
    }
}

/// The ledger store of the clone that holds `cwd`, when the clone has a clone id.
fn db_path(cwd: &Path) -> Option<PathBuf> {
    let repo = super::location::Repo::discover(cwd).ok()?;
    let id = repo.read_clone_id().ok().flatten()?;
    let root = super::location::state_root_from_env().ok()?;
    Some(super::location::dolt_db_path(&root, &id))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch() -> (tempfile::TempDir, Paths) {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::of_store(&dir.path().join("ledger-db"));
        (dir, paths)
    }

    #[test]
    fn the_files_sit_beside_the_store() {
        let (dir, paths) = scratch();
        assert_eq!(paths.lock, dir.path().join("push.lock"));
        assert_eq!(paths.result, dir.path().join("background-push.result"));
    }

    #[test]
    fn the_first_line_of_an_error_loses_its_prefix() {
        assert_eq!(first_line("error: boom\n  more"), "boom");
        assert_eq!(first_line(""), "unknown error");
    }

    #[test]
    fn a_long_error_shows_its_first_lines_in_the_warning() {
        let one = warning_text("T", "error: boom");
        assert_eq!(
            one,
            "warning: the last background publish failed at T: boom; run tsk ledger push"
        );
        let long: String = (1..=30).map(|n| format!("line {}\n", n)).collect();
        let text = warning_text("T", &format!("error: commit failed:\n{}", long));
        assert!(text.contains("  line 18"), "{}", text);
        assert!(!text.contains("line 25"), "{}", text);
        assert!(text.contains("more lines"), "{}", text);
    }

    #[test]
    fn a_running_result_without_a_lock_holder_reads_as_interrupted() {
        let (_dir, paths) = scratch();
        write_outcome(
            &paths,
            &Outcome {
                state: "running".to_string(),
                pid: 99999,
                time: 1,
                ..Outcome::default()
            },
        );
        let outcome = effective(&paths).unwrap();
        assert_eq!(outcome.state, "failed");
        assert!(outcome.error.starts_with("interrupted"));
        assert!(!outcome.shown);
    }

    #[test]
    fn a_running_result_with_a_lock_holder_stays_running() {
        let (dir, paths) = scratch();
        let held = open_lock(&paths).unwrap();
        assert!(try_flock(&held));
        write_outcome(
            &paths,
            &Outcome {
                state: "running".to_string(),
                ..Outcome::default()
            },
        );
        assert_eq!(effective(&paths).unwrap().state, "running");
        assert_eq!(
            status_line(&dir.path().join("ledger-db")).unwrap(),
            "Background publish: running"
        );
    }

    #[test]
    fn a_failure_is_cleared_by_a_later_success_and_an_ok_stays() {
        let (dir, paths) = scratch();
        let db = dir.path().join("ledger-db");
        write_outcome(
            &paths,
            &Outcome {
                state: "ok".to_string(),
                ..Outcome::default()
            },
        );
        clear_failure(&db);
        assert!(paths.result.exists());
        write_outcome(
            &paths,
            &Outcome {
                state: "failed".to_string(),
                ..Outcome::default()
            },
        );
        clear_failure(&db);
        assert!(!paths.result.exists());
    }
}
