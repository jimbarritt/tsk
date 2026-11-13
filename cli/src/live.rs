//! Which terminal sessions on this machine are still open, so the robot choice offers only
//! robots that no open session uses. A cloud session asks the platform instead
//! (`tsk thread next --open`).
//!
//! Each session that starts on this machine outside a cloud session has a file in
//! `<state root>/live/<session id>.json`, named by the hook's session ID. It holds the
//! process ID and start time of the `claude` process that runs the session, the time of the
//! last turn, and the time the session ended. The Stop hook updates the last turn and the
//! SessionEnd hook writes the end. Nothing here goes into the ledger: it is a fact about this
//! machine only.
//!
//! A session is open when it has no end, and either its `claude` process still runs with
//! the same start time, or, with no process recorded, its last turn is within the idle
//! limit. A robot is taken while one of its sessions is open, or while a tmux pane shows
//! the robot in `@tsk_actor` and runs `claude`.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

pub const LIVE_DIR: &str = "live";

/// The idle limit, in days, for a session with no recorded process. `TSK_ROBOT_IDLE_DAYS`
/// overrides it.
pub const IDLE_DAYS: u64 = 7;
pub const IDLE_DAYS_VAR: &str = "TSK_ROBOT_IDLE_DAYS";

/// The command name of the Claude Code process.
const HARNESS_COMMAND: &str = "claude";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiveFile {
    pub session_id: String,
    pub pid: Option<u32>,
    /// The process start time as `ps -o lstart=` prints it.
    pub process_started: Option<String>,
    /// Unix seconds of the last session start or turn.
    pub last_active: u64,
    /// Unix seconds of the session end.
    pub ended: Option<u64>,
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn safe(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

pub fn live_dir(state_root: &Path) -> PathBuf {
    state_root.join(LIVE_DIR)
}

fn file_of(state_root: &Path, session_id: &str) -> Option<PathBuf> {
    safe(session_id).then(|| live_dir(state_root).join(format!("{}.json", session_id)))
}

fn read(path: &Path) -> Option<LiveFile> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

fn write(path: &Path, file: &LiveFile) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(text) = serde_json::to_string(file) {
        let tmp = path.with_extension("json.tmp");
        if std::fs::write(&tmp, text).is_ok() {
            let _ = std::fs::rename(&tmp, path);
        }
    }
}

/// One process as `ps` describes it: parent ID, start time and command name.
fn ps(pid: u32) -> Option<(u32, String, String)> {
    let out = Command::new("ps")
        .args([
            "-o",
            "ppid=",
            "-o",
            "lstart=",
            "-o",
            "comm=",
            "-p",
            &pid.to_string(),
        ])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    parse_ps(&String::from_utf8_lossy(&out.stdout))
}

/// Parse `ps -o ppid= -o lstart= -o comm=`: the parent ID, the five fields of the start
/// time (`Fri Nov 13 20:01:02 2026`), and the command, which can hold spaces.
pub fn parse_ps(line: &str) -> Option<(u32, String, String)> {
    let mut fields = line.split_whitespace();
    let ppid = fields.next()?.parse().ok()?;
    let started: Vec<&str> = fields.by_ref().take(5).collect();
    if started.len() != 5 {
        return None;
    }
    let command: Vec<&str> = fields.collect();
    Some((ppid, started.join(" "), command.join(" ")))
}

/// True when the base name of a command names Claude Code.
pub fn is_harness(command: &str) -> bool {
    Path::new(command.trim())
        .file_name()
        .is_some_and(|name| name.to_string_lossy() == HARNESS_COMMAND)
}

/// The `claude` process above this one, with its start time: the hooks run as its children.
fn harness_process() -> Option<(u32, String)> {
    let mut pid = std::process::id();
    for _ in 0..12 {
        let (ppid, started, command) = ps(pid)?;
        if is_harness(&command) {
            return Some((pid, started));
        }
        if ppid <= 1 {
            return None;
        }
        pid = ppid;
    }
    None
}

/// Record a session start: the `claude` process, when found, and the time.
pub fn started(state_root: &Path, session_id: &str) {
    let Some(path) = file_of(state_root, session_id) else {
        return;
    };
    let process = harness_process();
    write(
        &path,
        &LiveFile {
            session_id: session_id.to_string(),
            pid: process.as_ref().map(|(pid, _)| *pid),
            process_started: process.map(|(_, started)| started),
            last_active: now_secs(),
            ended: None,
        },
    );
}

/// Record a turn: the last active time of a session with a file.
pub fn active(state_root: &Path, session_id: &str) {
    let Some(path) = file_of(state_root, session_id) else {
        return;
    };
    if let Some(mut file) = read(&path) {
        file.last_active = now_secs();
        write(&path, &file);
    }
}

/// Record the end of a session.
pub fn ended(state_root: &Path, session_id: &str) {
    let Some(path) = file_of(state_root, session_id) else {
        return;
    };
    let mut file = read(&path).unwrap_or(LiveFile {
        session_id: session_id.to_string(),
        ..LiveFile::default()
    });
    file.ended = Some(now_secs());
    write(&path, &file);
}

fn idle_limit_secs() -> u64 {
    std::env::var(IDLE_DAYS_VAR)
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(IDLE_DAYS)
        * 24
        * 60
        * 60
}

/// Whether a session is open, from its file, whether its process runs with the same start
/// time (`None` when no process is recorded), the time now and the idle limit.
pub fn is_open(file: &LiveFile, process_runs: Option<bool>, now: u64, idle_limit: u64) -> bool {
    if file.ended.is_some() {
        return false;
    }
    match process_runs {
        Some(runs) => runs,
        None => now.saturating_sub(file.last_active) < idle_limit,
    }
}

fn process_runs(file: &LiveFile) -> Option<bool> {
    let (pid, started) = (file.pid?, file.process_started.as_deref()?);
    Some(
        ps(pid).is_some_and(|(_, now_started, command)| {
            now_started == started && is_harness(&command)
        }),
    )
}

/// The IDs of the sessions on this machine that are open.
pub fn open_session_ids(state_root: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(live_dir(state_root)) else {
        return Vec::new();
    };
    let now = now_secs();
    let limit = idle_limit_secs();
    entries
        .flatten()
        .filter_map(|entry| read(&entry.path()))
        .filter(|file| is_open(file, process_runs(file), now, limit))
        .map(|file| file.session_id)
        .collect()
}

/// The robots that a tmux pane shows in `@tsk_actor` while it runs `claude`. Empty outside
/// tmux or with no tmux server.
pub fn tmux_robots() -> Vec<String> {
    let bin = std::env::var(crate::tmux::TMUX_BIN_VAR).unwrap_or_else(|_| "tmux".to_string());
    let Ok(out) = Command::new(bin)
        .args([
            "list-panes",
            "-a",
            "-F",
            "#{pane_current_command}\t#{@tsk_actor}",
        ])
        .output()
    else {
        return Vec::new();
    };
    if !out.status.success() {
        return Vec::new();
    }
    parse_panes(&String::from_utf8_lossy(&out.stdout))
}

/// The actor names of the panes that run `claude`.
pub fn parse_panes(text: &str) -> Vec<String> {
    let mut names: Vec<String> = text
        .lines()
        .filter_map(|line| line.split_once('\t'))
        .filter(|(command, actor)| is_harness(command) && !actor.trim().is_empty())
        .map(|(_, actor)| actor.trim().to_string())
        .collect();
    names.sort();
    names.dedup();
    names
}

/// The input of a hook: the session ID.
#[derive(Deserialize, Default)]
struct HookSession {
    session_id: Option<String>,
}

/// The session ID from a hook's JSON input.
pub fn hook_session_id(input: &str) -> Option<String> {
    serde_json::from_str::<HookSession>(input)
        .ok()
        .and_then(|h| h.session_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ps_output_parses_with_a_command_that_has_spaces() {
        assert_eq!(
            parse_ps("  4321 Fri Nov 13 20:01:02 2026 /usr/local/bin/claude\n"),
            Some((
                4321,
                "Fri Nov 13 20:01:02 2026".to_string(),
                "/usr/local/bin/claude".to_string()
            ))
        );
        assert_eq!(
            parse_ps("1 Fri Nov 13 20:01:02 2026 tmux: server").map(|p| p.2),
            Some("tmux: server".to_string())
        );
        assert_eq!(parse_ps("x"), None);
    }

    #[test]
    fn the_harness_is_the_claude_command_only() {
        assert!(is_harness("claude"));
        assert!(is_harness("/opt/homebrew/bin/claude"));
        assert!(!is_harness("zsh"));
        assert!(!is_harness("claude-helper"));
    }

    #[test]
    fn a_session_is_open_until_it_ends_its_process_stops_or_it_idles() {
        let file = LiveFile {
            session_id: "s".into(),
            last_active: 1_000,
            ..LiveFile::default()
        };
        let day = 24 * 60 * 60;
        assert!(is_open(&file, Some(true), 1_000_000, day));
        assert!(!is_open(&file, Some(false), 1_000, day));
        assert!(is_open(&file, None, 1_000 + day - 1, day));
        assert!(!is_open(&file, None, 1_000 + day, day));
        let ended = LiveFile {
            ended: Some(2_000),
            ..file
        };
        assert!(!is_open(&ended, Some(true), 2_000, day));
    }

    #[test]
    fn panes_running_claude_name_their_robots() {
        let text = "claude\tRobot Bee\nzsh\tRobot Gone\nclaude\t\nclaude\tRobot Bee\n";
        assert_eq!(parse_panes(text), vec!["Robot Bee".to_string()]);
    }

    #[test]
    fn start_turn_and_end_write_the_file() {
        let root = tempfile::tempdir().unwrap();
        started(root.path(), "abc");
        let path = live_dir(root.path()).join("abc.json");
        let first = read(&path).unwrap();
        assert_eq!(first.session_id, "abc");
        assert_eq!(first.ended, None);
        active(root.path(), "abc");
        ended(root.path(), "abc");
        assert!(read(&path).unwrap().ended.is_some());
        started(root.path(), "../x");
        assert!(!live_dir(root.path()).join("../x.json").exists());
        assert_eq!(
            hook_session_id(r#"{"session_id":"abc"}"#).as_deref(),
            Some("abc")
        );
    }
}
