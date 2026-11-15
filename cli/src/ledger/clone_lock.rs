//! The clone lock: one `flock` per ledger store, on the file `push.lock` beside it, held by
//! every tsk process that writes the store (M-TSK-LOCAL T-07).
//!
//! A command reads the records, changes them in memory and saves them in one transaction.
//! The transaction starts at the save, so a save made from a stale read overwrites a row
//! that another process wrote after the read: two `task add` commands read the same
//! `next_task`, and the second save replaces the first task. Two saves that overlap fail
//! with a Dolt serialization failure instead. A refresh that resets the store to origin
//! discards a write made after its dirty check. A push that commits, renumbers and merges
//! while another process writes can merge a half-written state.
//!
//! The lock stops all of these. A command that can write takes it at its first access to
//! the store and holds it until the store is dropped, so its reads and its writes come
//! from one state that no other process changes. A command that only reads takes no lock
//! ([`set_reader`]). A write from such a command, a refresh and a push still take it for
//! their own duration.
//!
//! The file is the one the push lock of 0.21.0 and later uses, so a writer of this release
//! also waits for a push of an older release, and a push waits for a writer.
//!
//! The lock is re-entrant in a process: a second take of the same store's lock counts up,
//! and the `flock` ends when the last holder drops. The descriptor is close-on-exec, so no
//! process that tsk starts (the Dolt server, git) inherits it. The kernel frees the lock
//! when the process ends, however it ends.

use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// How long a process waits for the lock, in seconds.
pub const LOCK_WAIT_VAR: &str = "TSK_PUSH_LOCK_SECS";
const DEFAULT_LOCK_WAIT_SECS: u64 = 60;
/// The name of the lock file, beside the ledger store.
pub const LOCK_FILE: &str = "push.lock";

/// True in a process whose command only reads.
static READER: AtomicBool = AtomicBool::new(false);
/// True in the child of a background publish, which holds the lock through the descriptor
/// it inherited from its parent.
static INHERITED: AtomicBool = AtomicBool::new(false);

struct Held {
    _file: File,
    count: usize,
}

/// The locks this process holds, by lock file.
static HELD: Mutex<Option<HashMap<PathBuf, Held>>> = Mutex::new(None);

/// Mark this process as one that only reads: it takes the lock at a write, a refresh or a
/// push, not at its first access to the store.
pub fn set_reader() {
    READER.store(true, Ordering::SeqCst);
}

/// True unless the command of this process only reads.
pub fn is_writer() -> bool {
    !READER.load(Ordering::SeqCst)
}

/// Record that this process holds the lock through an inherited descriptor.
pub fn set_inherited() {
    INHERITED.store(true, Ordering::SeqCst);
}

/// The lock file of the ledger store at `db`.
pub fn lock_path(db: &Path) -> PathBuf {
    db.parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(LOCK_FILE)
}

/// One hold on the clone lock. The `flock` ends when the last hold of the process drops.
#[derive(Debug)]
pub struct CloneLock {
    path: Option<PathBuf>,
}

impl Drop for CloneLock {
    fn drop(&mut self) {
        let Some(path) = self.path.take() else {
            return;
        };
        let mut held = HELD.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(map) = held.as_mut() {
            if let Some(entry) = map.get_mut(&path) {
                entry.count -= 1;
                if entry.count == 0 {
                    // Dropping the file closes the descriptor, which ends the flock.
                    map.remove(&path);
                }
            }
        }
    }
}

fn open(path: &Path) -> Result<File, String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("error: could not create {}: {}", dir.display(), e))?;
    }
    OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(path)
        .map_err(|e| format!("error: could not open {}: {}", path.display(), e))
}

fn try_flock(file: &File) -> bool {
    unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) == 0 }
}

fn wait_secs() -> u64 {
    std::env::var(LOCK_WAIT_VAR)
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .unwrap_or(DEFAULT_LOCK_WAIT_SECS)
}

/// True when this process holds the lock of the ledger store at `db`.
#[cfg(test)]
pub fn holds(db: &Path) -> bool {
    if INHERITED.load(Ordering::SeqCst) {
        return true;
    }
    let held = HELD.lock().unwrap_or_else(|e| e.into_inner());
    held.as_ref()
        .is_some_and(|map| map.contains_key(&lock_path(db)))
}

/// Take the clone lock of the ledger store at `db`, waiting up to [`LOCK_WAIT_VAR`]
/// seconds for another process to drop it.
pub fn take(db: &Path) -> Result<CloneLock, String> {
    if INHERITED.load(Ordering::SeqCst) {
        return Ok(CloneLock { path: None });
    }
    let path = lock_path(db);
    {
        let mut held = HELD.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(entry) = held.get_or_insert_with(HashMap::new).get_mut(&path) {
            entry.count += 1;
            return Ok(CloneLock { path: Some(path) });
        }
    }
    let file = open(&path)?;
    let end = Instant::now() + Duration::from_secs(wait_secs());
    let mut pause = Duration::from_millis(2);
    while !try_flock(&file) {
        if Instant::now() >= end {
            return Err(format!(
                "error: another tsk process (a push or a write, say) holds {}.\n       Try again when it ends",
                path.display()
            ));
        }
        std::thread::sleep(pause);
        pause = (pause * 2).min(Duration::from_millis(20));
    }
    let mut held = HELD.lock().unwrap_or_else(|e| e.into_inner());
    held.get_or_insert_with(HashMap::new).insert(
        path.clone(),
        Held {
            _file: file,
            count: 1,
        },
    );
    Ok(CloneLock { path: Some(path) })
}

/// The descriptor of the lock this process holds on the store at `db`, for a child that
/// carries the lock on (the background publish).
pub fn held_fd(db: &Path) -> Option<i32> {
    let held = HELD.lock().unwrap_or_else(|e| e.into_inner());
    held.as_ref()
        .and_then(|map| map.get(&lock_path(db)))
        .map(|entry| entry._file.as_raw_fd())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store_in(dir: &Path) -> PathBuf {
        let db = dir.join("ledger-db");
        std::fs::create_dir_all(&db).unwrap();
        db
    }

    fn free_to_another_descriptor(db: &Path) -> bool {
        let file = open(&lock_path(db)).unwrap();
        try_flock(&file)
    }

    #[test]
    fn a_hold_excludes_another_descriptor_until_it_drops() {
        let dir = tempfile::tempdir().unwrap();
        let db = store_in(dir.path());
        let lock = take(&db).unwrap();
        assert!(holds(&db));
        assert!(!free_to_another_descriptor(&db));
        drop(lock);
        assert!(!holds(&db));
        assert!(free_to_another_descriptor(&db));
    }

    #[test]
    fn the_lock_is_reentrant_and_ends_with_the_last_hold() {
        let dir = tempfile::tempdir().unwrap();
        let db = store_in(dir.path());
        let outer = take(&db).unwrap();
        let inner = take(&db).unwrap();
        drop(outer);
        assert!(holds(&db));
        assert!(!free_to_another_descriptor(&db));
        drop(inner);
        assert!(free_to_another_descriptor(&db));
    }

    #[test]
    fn the_lock_file_is_the_push_lock_beside_the_store() {
        let dir = tempfile::tempdir().unwrap();
        let db = store_in(dir.path());
        assert_eq!(lock_path(&db), dir.path().join("push.lock"));
    }
}
