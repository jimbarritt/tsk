//! Helpers that the test files share. A file that uses them declares `mod common;`.
//!
//! The local Dolt server (ADR 0018) is a process that outlives the `tsk` command that
//! started it. A fixture that ran commands with the server switch on stops the servers of
//! its state directory when it is dropped, and before its files are copied.

#![allow(dead_code)]

pub mod templates;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// The processes a `dolt-server.pid` file names: the supervisor, then the server.
fn pids_of(pid_file: &Path) -> Vec<i32> {
    let text = std::fs::read_to_string(pid_file).unwrap_or_default();
    let mut pids = Vec::new();
    for key in ["supervisor", "pid"] {
        for line in text.lines() {
            if let Some(value) = line.strip_prefix(&format!("{}=", key)) {
                if let Ok(pid) = value.trim().parse::<i32>() {
                    pids.push(pid);
                }
            }
        }
    }
    pids
}

/// Every `dolt-server.pid` under `<root>/**/tsk/repos/*/`, found by looking two levels
/// below each directory named `repos`.
pub fn pid_files(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![(root.to_path_buf(), 0usize)];
    while let Some((dir, depth)) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.file_name().is_some_and(|n| n == "dolt-server.pid") {
                found.push(path);
            } else if depth < 6 && entry.file_type().is_ok_and(|t| t.is_dir()) {
                let name = entry.file_name();
                // The git directories and the stores hold nothing of interest.
                if name != ".git" && name != "ledger-db" && name != "ledger-remote" {
                    stack.push((path, depth + 1));
                }
            }
        }
    }
    found
}

fn alive(pid: i32) -> bool {
    if unsafe { libc::kill(pid, 0) } != 0 {
        return false;
    }
    // A zombie is gone for this purpose.
    match std::fs::read_to_string(format!("/proc/{}/stat", pid)) {
        Ok(stat) => {
            stat.rsplit_once(')')
                .and_then(|(_, rest)| rest.trim_start().chars().next())
                != Some('Z')
        }
        Err(_) => true,
    }
}

/// Stop every Dolt server whose state is under `root`. `graceful` sends SIGTERM and waits
/// for the supervisor to clean up, which a copy of the directory needs. Otherwise the
/// processes are killed outright, which is enough before a directory is deleted.
pub fn stop_servers(root: &Path, graceful: bool) {
    for file in pid_files(root) {
        let pids = pids_of(&file);
        if graceful {
            if let Some(&supervisor) = pids.first() {
                unsafe {
                    libc::kill(supervisor, libc::SIGTERM);
                }
                let end = Instant::now() + Duration::from_secs(15);
                while alive(supervisor) && Instant::now() < end {
                    std::thread::sleep(Duration::from_millis(20));
                }
            }
        }
        for pid in pids {
            if alive(pid) {
                unsafe {
                    libc::kill(pid, libc::SIGKILL);
                }
            }
        }
    }
}

/// The number of `dolt sql-server` processes whose command line holds `needle`.
pub fn servers_running(needle: &str) -> usize {
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return 0;
    };
    entries
        .flatten()
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|n| n.chars().all(|c| c.is_ascii_digit()))
        })
        .filter(|entry| {
            std::fs::read(entry.path().join("cmdline")).is_ok_and(|bytes| {
                let line = String::from_utf8_lossy(&bytes).replace('\0', " ");
                line.contains("sql-server") && line.contains(needle)
            })
        })
        .count()
}
