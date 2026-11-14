//! The liveness files of the terminal sessions on this machine.
//!
//! Each session that starts on this machine outside a cloud session has a file in
//! `<state root>/live/<session id>.json`, named by the hook's session ID. It holds the
//! process ID and start time of the `claude` process that runs the session, the time of the
//! last turn, and the time the session ended. The Stop hook updates the last turn and the
//! SessionEnd hook writes the end. Nothing here goes into the ledger: it is a fact about this
//! machine only. The robot choice does not read these files: a robot is busy while it holds
//! a thread, which the ledger says (ADR 0032).

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

pub const LIVE_DIR: &str = "live";

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
