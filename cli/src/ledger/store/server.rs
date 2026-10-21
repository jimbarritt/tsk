//! A local `dolt sql-server` per ledger store, behind a switch (ADR 0018).
//!
//! With the switch on, the SQL of a Dolt ledger store goes to a long-lived server on a
//! Unix socket in the store directory in place of one `dolt sql` process for each batch.
//! The server starts at the first command that needs it, stops when nothing has used it
//! for a while, and is replaced when it is dead or hung. The `dolt` command line works on
//! the store the whole time: a `dolt` that finds the store held by the server talks to it
//! through the loopback port that the server records in `.dolt/sql-server.info`, so an
//! older `tsk` that has no switch reads and writes the same store.
//!
//! Three kinds of process take part:
//!
//! - the **client**: any `tsk` command. It connects to the socket and runs SQL, and when
//!   there is no live server it starts one under a file lock.
//! - the **supervisor**: `tsk ledger server run`, detached with `setsid`. It starts
//!   `dolt sql-server`, waits until it answers, records it in the pid file, and stops it
//!   after a period with no use (Dolt has no option for that), or when the store
//!   directory is gone, or when the server dies.
//! - the **server**: `dolt sql-server`, a child of the supervisor.
//!
//! Every file lives beside the store, in its state directory, except the socket, which is
//! inside the store directory (or in `/tmp` when that path is too long for a Unix socket).

use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::MetadataExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant, SystemTime};

use serde_json::Value;

use super::mysql::{Conn, Error as SqlError, ResultSet};

/// `on` or `off`. The environment wins over `[ledger] server` in the user config.
pub const SWITCH_VAR: &str = "TSK_DOLT_SERVER";
/// Seconds with no connection after which the server stops. The default is 120.
pub const IDLE_VAR: &str = "TSK_DOLT_SERVER_IDLE_SECS";
/// Seconds a liveness check waits for `select 1` before it calls the server hung.
pub const PROBE_VAR: &str = "TSK_DOLT_SERVER_PROBE_SECS";
/// Seconds one statement may take before the connection is given up.
pub const QUERY_VAR: &str = "TSK_DOLT_SERVER_QUERY_SECS";

/// Seconds a fetch or a push through the server may take.
pub const NETWORK_VAR: &str = "TSK_DOLT_SERVER_NETWORK_SECS";

pub const DEFAULT_IDLE_SECS: u64 = 120;
const DEFAULT_NETWORK_SECS: u64 = 600;
const DEFAULT_PROBE_SECS: u64 = 5;
const DEFAULT_QUERY_SECS: u64 = 120;
/// How long a start may take before it is called failed.
const START_SECS: u64 = 30;
/// How long a command waits for another command's start or for the lock.
const LOCK_SECS: u64 = 60;

const SOCKET_NAME: &str = ".tsk-dolt.sock";
/// A Unix socket path must fit in `sockaddr_un.sun_path` (108 bytes on Linux, 104 on macOS).
const SOCKET_PATH_MAX: usize = 100;

// ---------------------------------------------------------------------------
// The switch
// ---------------------------------------------------------------------------

/// Where the switch was read from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Env,
    Config,
    Default,
}

impl Source {
    pub fn as_str(self) -> &'static str {
        match self {
            Source::Env => "environment",
            Source::Config => "config",
            Source::Default => "default",
        }
    }
}

fn parse_switch(text: &str) -> Option<bool> {
    match text.trim().to_ascii_lowercase().as_str() {
        "on" | "1" | "true" | "yes" => Some(true),
        "off" | "0" | "false" | "no" => Some(false),
        _ => None,
    }
}

/// The switch from a value of the environment and a value of the config file. A value that
/// is neither `on` nor `off` counts as unset.
pub fn resolve_switch(env: Option<&str>, config: Option<&str>) -> (bool, Source) {
    if let Some(on) = env.and_then(parse_switch) {
        return (on, Source::Env);
    }
    if let Some(on) = config.and_then(parse_switch) {
        return (on, Source::Config);
    }
    (false, Source::Default)
}

/// Whether the SQL of a store goes through a server, and where the answer came from. Read
/// once per process.
pub fn switch() -> (bool, Source) {
    static SWITCH: OnceLock<(bool, Source)> = OnceLock::new();
    *SWITCH.get_or_init(|| {
        let env = std::env::var(SWITCH_VAR).ok();
        let config = crate::config::config_file_from_env()
            .and_then(|file| crate::config::read_ledger_server(&file));
        resolve_switch(env.as_deref(), config.as_deref())
    })
}

pub fn enabled() -> bool {
    switch().0
}

fn env_secs(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .unwrap_or(default)
}

pub fn idle_secs() -> u64 {
    env_secs(IDLE_VAR, DEFAULT_IDLE_SECS)
}

fn probe_timeout() -> Duration {
    Duration::from_secs(env_secs(PROBE_VAR, DEFAULT_PROBE_SECS).max(1))
}

/// The time limit of a procedure that crosses the network: `dolt_fetch` and `dolt_push`.
pub fn network_timeout() -> Duration {
    Duration::from_secs(env_secs(NETWORK_VAR, DEFAULT_NETWORK_SECS).max(1))
}

pub fn query_timeout() -> Duration {
    Duration::from_secs(env_secs(QUERY_VAR, DEFAULT_QUERY_SECS).max(1))
}

// ---------------------------------------------------------------------------
// Files
// ---------------------------------------------------------------------------

/// Every path the server of one store uses.
pub struct Paths {
    pub store: PathBuf,
    pub state: PathBuf,
    pub lock: PathBuf,
    pub pid: PathBuf,
    pub log: PathBuf,
    pub config: PathBuf,
    pub seen: PathBuf,
    pub cfg_dir: PathBuf,
}

impl Paths {
    pub fn new(store: &Path) -> Paths {
        let state = store
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        Paths {
            store: store.to_path_buf(),
            lock: state.join("dolt-server.lock"),
            pid: state.join("dolt-server.pid"),
            log: state.join("dolt-server.log"),
            config: state.join("dolt-server.yaml"),
            seen: state.join("dolt-server.seen"),
            cfg_dir: state.join("dolt-server-cfg"),
            state,
        }
    }

    /// The socket path: in the store directory, or in `/tmp` when that is too long.
    pub fn socket(&self) -> PathBuf {
        let inside = self.store.join(SOCKET_NAME);
        if inside.as_os_str().len() <= SOCKET_PATH_MAX {
            return inside;
        }
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.store.hash(&mut hasher);
        let uid = unsafe { libc::getuid() };
        PathBuf::from("/tmp").join(format!("tsk-dolt-{}-{:016x}.sock", uid, hasher.finish()))
    }

    /// Dolt's own record of the running server, which its command line reads.
    fn dolt_info(&self) -> PathBuf {
        self.store.join(".dolt").join("sql-server.info")
    }

    fn database(&self) -> String {
        self.store
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    /// Identifies the store directory by its `.dolt` directory, so a store that was
    /// deleted and made again at the same path is not taken for the one a server serves.
    fn store_id(&self) -> String {
        std::fs::metadata(self.store.join(".dolt"))
            .map(|m| format!("{}:{}", m.dev(), m.ino()))
            .unwrap_or_default()
    }
}

/// What the pid file records.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Info {
    pub supervisor: u32,
    pub dolt: Option<u32>,
    pub ready: bool,
    pub socket: PathBuf,
    pub port: Option<u16>,
    pub store_id: String,
    pub started: u64,
    pub idle: u64,
}

impl Info {
    fn render(&self) -> String {
        let mut text = String::new();
        if let Some(pid) = self.dolt {
            text.push_str(&format!("pid={}\n", pid));
        }
        text.push_str(&format!("supervisor={}\n", self.supervisor));
        text.push_str(&format!(
            "state={}\n",
            if self.ready { "ready" } else { "starting" }
        ));
        text.push_str(&format!("socket={}\n", self.socket.display()));
        if let Some(port) = self.port {
            text.push_str(&format!("port={}\n", port));
        }
        text.push_str(&format!("store_id={}\n", self.store_id));
        text.push_str(&format!("started={}\n", self.started));
        text.push_str(&format!("idle={}\n", self.idle));
        text
    }

    fn parse(text: &str) -> Option<Info> {
        let mut info = Info::default();
        let mut found = false;
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            match key {
                "pid" => info.dolt = value.parse().ok(),
                "supervisor" => {
                    info.supervisor = value.parse().ok()?;
                    found = true;
                }
                "state" => info.ready = value == "ready",
                "socket" => info.socket = PathBuf::from(value),
                "port" => info.port = value.parse().ok(),
                "store_id" => info.store_id = value.to_string(),
                "started" => info.started = value.parse().unwrap_or(0),
                "idle" => info.idle = value.parse().unwrap_or(0),
                _ => {}
            }
        }
        found.then_some(info)
    }
}

fn read_info(paths: &Paths) -> Option<Info> {
    Info::parse(&std::fs::read_to_string(&paths.pid).ok()?)
}

fn write_info(paths: &Paths, info: &Info) -> Result<(), String> {
    let tmp = paths
        .state
        .join(format!("dolt-server.pid.{}", std::process::id()));
    std::fs::write(&tmp, info.render())
        .and_then(|()| std::fs::rename(&tmp, &paths.pid))
        .map_err(|e| format!("error: could not write {}: {}", paths.pid.display(), e))
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Mark the server as used now. Best effort: a store whose state directory cannot be
/// written to still works, and its server stops one idle period after the last mark.
fn touch_seen(paths: &Paths) {
    if let Ok(file) = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(&paths.seen)
    {
        let _ = file.set_modified(SystemTime::now());
    }
}

// ---------------------------------------------------------------------------
// Processes
// ---------------------------------------------------------------------------

fn signal(pid: u32, sig: i32) {
    unsafe {
        libc::kill(pid as i32, sig);
    }
}

fn exists(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    let found = unsafe { libc::kill(pid as i32, 0) } == 0
        || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM);
    found && !is_zombie(pid)
}

fn is_zombie(pid: u32) -> bool {
    match std::fs::read_to_string(format!("/proc/{}/stat", pid)) {
        Ok(stat) => {
            stat.rsplit_once(')')
                .and_then(|(_, rest)| rest.trim_start().chars().next())
                == Some('Z')
        }
        Err(_) => false,
    }
}

fn command_line(pid: u32) -> Option<String> {
    if let Ok(bytes) = std::fs::read(format!("/proc/{}/cmdline", pid)) {
        return Some(
            String::from_utf8_lossy(&bytes)
                .replace('\0', " ")
                .trim()
                .to_string(),
        );
    }
    let out = Command::new("ps")
        .args(["-o", "command=", "-p", &pid.to_string()])
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// A live process whose command line holds `needle`. A pid file that outlived its server
/// can name a process that has nothing to do with it; this is the check before a signal.
fn alive_and_ours(pid: u32, needle: &str) -> bool {
    exists(pid) && command_line(pid).is_some_and(|line| line.contains(needle))
}

fn wait_gone(pid: u32, limit: Duration) -> bool {
    let end = Instant::now() + limit;
    while Instant::now() < end {
        if !exists(pid) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    !exists(pid)
}

/// SIGTERM, then SIGKILL when the process outlasts `grace`.
fn terminate(pid: u32, grace: Duration) {
    signal(pid, libc::SIGTERM);
    if !wait_gone(pid, grace) {
        signal(pid, libc::SIGKILL);
        wait_gone(pid, Duration::from_secs(5));
    }
}

struct Lock {
    _file: File,
}

impl Lock {
    /// Take the exclusive lock, waiting up to `wait` (`None`: do not wait).
    fn acquire(paths: &Paths, wait: Option<Duration>) -> Result<Option<Lock>, String> {
        use std::os::unix::io::AsRawFd;
        std::fs::create_dir_all(&paths.state)
            .map_err(|e| format!("error: could not create {}: {}", paths.state.display(), e))?;
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(&paths.lock)
            .map_err(|e| format!("error: could not open {}: {}", paths.lock.display(), e))?;
        let end = Instant::now() + wait.unwrap_or_default();
        loop {
            if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
                return Ok(Some(Lock { _file: file }));
            }
            if Instant::now() >= end {
                return Ok(None);
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

// ---------------------------------------------------------------------------
// The client side: find a live server, or start one
// ---------------------------------------------------------------------------

fn probe(paths: &Paths, info: &Info, timeout: Duration) -> Result<Conn, SqlError> {
    let mut conn = Conn::connect(&info.socket, &paths.database(), Some(timeout))?;
    conn.query("select 1")?;
    conn.set_timeout(Some(query_timeout()))?;
    Ok(conn)
}

/// What a start found.
pub enum Ensured {
    /// A live server, and a connection to it.
    Ready(Conn),
    /// A server is starting (or another command holds the lock); nothing was waited for.
    Starting,
}

/// A connection to the server of `store`, which is started when there is none alive. A
/// server that is dead, stale or hung is replaced. Fails when no server can be started
/// (`dolt` is not installed, or something else holds the store).
pub fn connect(store: &Path) -> Result<Conn, String> {
    match ensure(store, true)? {
        Ensured::Ready(conn) => Ok(conn),
        Ensured::Starting => Err("error: the Dolt server did not start".to_string()),
    }
}

/// Start the server when none is alive, and return without waiting for it. This is the
/// hook path: a live server costs one connection and one `select 1`.
pub fn ensure_background(store: &Path) -> Result<(), String> {
    ensure(store, false).map(|_| ())
}

fn ensure(store: &Path, wait: bool) -> Result<Ensured, String> {
    let paths = Paths::new(store);
    // The fast path: a live server needs no lock.
    if let Some(info) = read_info(&paths) {
        if info.ready && info.store_id == paths.store_id() {
            if let Ok(conn) = probe(&paths, &info, probe_timeout()) {
                touch_seen(&paths);
                return Ok(Ensured::Ready(conn));
            }
        }
    }
    let lock_wait = wait.then(|| Duration::from_secs(LOCK_SECS));
    let Some(_lock) = Lock::acquire(&paths, lock_wait)? else {
        return Ok(Ensured::Starting);
    };
    let needle_store = paths.store.to_string_lossy().into_owned();
    if let Some(info) = read_info(&paths) {
        let supervised = alive_and_ours(info.supervisor, &needle_store);
        let same_store = info.store_id == paths.store_id();
        if supervised && same_store {
            if info.ready {
                // Another command may have started it while this one waited for the lock.
                if let Ok(conn) = probe(&paths, &info, probe_timeout()) {
                    touch_seen(&paths);
                    return Ok(Ensured::Ready(conn));
                }
            } else if wait {
                if let Some(conn) = wait_ready(&paths, Duration::from_secs(START_SECS))? {
                    touch_seen(&paths);
                    return Ok(Ensured::Ready(conn));
                }
            } else {
                return Ok(Ensured::Starting);
            }
        }
        // Dead, stale, hung or for another store: take it down and clear its files.
        kill_server(&paths, &info);
    } else {
        clear_files(&paths);
    }
    start_supervisor(&paths)?;
    if !wait {
        return Ok(Ensured::Starting);
    }
    match wait_ready(&paths, Duration::from_secs(START_SECS))? {
        Some(conn) => {
            touch_seen(&paths);
            Ok(Ensured::Ready(conn))
        }
        None => Err(format!(
            "error: the Dolt server did not start within {} s; see {}",
            START_SECS,
            paths.log.display()
        )),
    }
}

/// Wait until the pid file says ready and the server answers. `None` when the time ran
/// out. An error when the supervisor died on the way.
fn wait_ready(paths: &Paths, limit: Duration) -> Result<Option<Conn>, String> {
    let end = Instant::now() + limit;
    loop {
        match read_info(paths) {
            Some(info) => {
                if info.ready {
                    if let Ok(conn) = probe(paths, &info, probe_timeout()) {
                        return Ok(Some(conn));
                    }
                }
                if !exists(info.supervisor) {
                    return Err(format!(
                        "error: the Dolt server stopped while starting; see {}",
                        paths.log.display()
                    ));
                }
            }
            None => {
                return Err(format!(
                    "error: the Dolt server left no pid file; see {}",
                    paths.log.display()
                ))
            }
        }
        if Instant::now() >= end {
            return Ok(None);
        }
        std::thread::sleep(Duration::from_millis(15));
    }
}

/// Stop what the pid file names, as far as it is ours, and remove every file of the server.
fn kill_server(paths: &Paths, info: &Info) {
    let store = paths.store.to_string_lossy().into_owned();
    let config = paths.config.to_string_lossy().into_owned();
    if alive_and_ours(info.supervisor, &store) {
        // The supervisor is stopped first, so it does not start anything while the
        // server goes. A hung server ignores SIGTERM: the grace is short.
        signal(info.supervisor, libc::SIGKILL);
        wait_gone(info.supervisor, Duration::from_secs(5));
    }
    if let Some(pid) = info.dolt {
        if alive_and_ours(pid, &config) {
            signal(pid, libc::SIGKILL);
            wait_gone(pid, Duration::from_secs(5));
        }
    }
    clear_files(paths);
}

fn clear_files(paths: &Paths) {
    let _ = std::fs::remove_file(&paths.pid);
    let _ = std::fs::remove_file(paths.socket());
    let _ = std::fs::remove_file(paths.store.join(SOCKET_NAME));
    // A record left by a server that died: the command line would take it for a live one
    // when its pid is reused.
    let _ = std::fs::remove_file(paths.dolt_info());
}

fn start_supervisor(paths: &Paths) -> Result<(), String> {
    let exe = std::env::current_exe()
        .map_err(|e| format!("error: could not find the tsk binary: {}", e))?;
    let idle = idle_secs();
    let mut command = Command::new(exe);
    command
        .args(["ledger", "server", "run", "--store"])
        .arg(&paths.store)
        .args(["--idle", &idle.to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    // Its own session: no signal aimed at the caller's terminal or process group reaches it.
    unsafe {
        command.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
    let child = command
        .spawn()
        .map_err(|e| format!("error: could not start the Dolt server supervisor: {}", e))?;
    let info = Info {
        supervisor: child.id(),
        dolt: None,
        ready: false,
        socket: paths.socket(),
        port: None,
        store_id: paths.store_id(),
        started: now_secs(),
        idle,
    };
    write_info(paths, &info)
}

/// Stop the server of `store`. Returns whether one was running.
pub fn stop(store: &Path) -> Result<bool, String> {
    let paths = Paths::new(store);
    let _lock = Lock::acquire(&paths, Some(Duration::from_secs(LOCK_SECS)))?;
    let Some(info) = read_info(&paths) else {
        clear_files(&paths);
        return Ok(false);
    };
    let store_text = paths.store.to_string_lossy().into_owned();
    let config = paths.config.to_string_lossy().into_owned();
    let was_running = alive_and_ours(info.supervisor, &store_text)
        || info.dolt.is_some_and(|pid| alive_and_ours(pid, &config));
    if alive_and_ours(info.supervisor, &store_text) {
        // The supervisor stops the server it started, then removes the files.
        terminate(info.supervisor, Duration::from_secs(10));
    }
    if let Some(pid) = info.dolt {
        if alive_and_ours(pid, &config) {
            terminate(pid, Duration::from_secs(10));
        }
    }
    clear_files(&paths);
    Ok(was_running)
}

/// Stop the server of `store` when a pid file says one may be running; true when one was.
/// For a `dolt` write that the server's hold on the store refused. Failures are ignored:
/// the command then reports its own.
pub fn stop_if_running(store: &Path) -> bool {
    if Paths::new(store).pid.exists() {
        disconnect(store);
        return stop(store).unwrap_or(false);
    }
    false
}

/// What `tsk ledger server status` prints.
pub struct Status {
    pub state: &'static str,
    pub info: Option<Info>,
    pub paths: Paths,
}

pub fn status(store: &Path) -> Status {
    let paths = Paths::new(store);
    let info = read_info(&paths);
    let store_text = paths.store.to_string_lossy().into_owned();
    let state = match &info {
        None => "not running",
        Some(info) if !alive_and_ours(info.supervisor, &store_text) => "not running",
        Some(info) if !info.ready => "starting",
        Some(info) => match probe(&paths, info, probe_timeout()) {
            Ok(_) => "running",
            Err(_) => "not answering",
        },
    };
    Status { state, info, paths }
}

// ---------------------------------------------------------------------------
// The supervisor
// ---------------------------------------------------------------------------

static TERMINATE: AtomicBool = AtomicBool::new(false);

extern "C" fn on_signal(_: libc::c_int) {
    TERMINATE.store(true, Ordering::SeqCst);
}

fn free_port() -> Result<u16, String> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")
        .map_err(|e| format!("error: could not find a free port: {}", e))?;
    listener
        .local_addr()
        .map(|a| a.port())
        .map_err(|e| format!("error: could not find a free port: {}", e))
}

fn yaml_string(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

fn write_config(paths: &Paths, port: u16, socket: &Path) -> Result<(), String> {
    // Dolt takes a port from 1024 to 65535 and has no switch for a socket alone. It binds
    // loopback only, and its command line finds the server through this port.
    let text = format!(
        "log_level: warning\n\
         listener:\n  host: 127.0.0.1\n  port: {}\n  socket: {}\n\
         data_dir: {}\n\
         cfg_dir: {}\n\
         behavior:\n  event_scheduler: \"OFF\"\n  auto_gc_behavior:\n    enable: false\n",
        port,
        yaml_string(&socket.to_string_lossy()),
        yaml_string(&paths.store.to_string_lossy()),
        yaml_string(&paths.cfg_dir.to_string_lossy()),
    );
    std::fs::write(&paths.config, text)
        .map_err(|e| format!("error: could not write {}: {}", paths.config.display(), e))
}

fn spawn_dolt(paths: &Paths) -> Result<std::process::Child, String> {
    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&paths.log)
        .map_err(|e| format!("error: could not open {}: {}", paths.log.display(), e))?;
    let log_err = log
        .try_clone()
        .map_err(|e| format!("error: could not open {}: {}", paths.log.display(), e))?;
    let mut command = Command::new("dolt");
    command
        .args(["sql-server", "--config"])
        .arg(&paths.config)
        .current_dir(&paths.store)
        .env("DOLT_DISABLE_EVENT_FLUSH", "1")
        .env("DOLT_REMOTE_INFO_BRANCH", "")
        .stdin(Stdio::null())
        .stdout(log)
        .stderr(log_err);
    crate::gitenv::apply(&mut command);
    command.spawn().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            super::super::dolt::NOT_INSTALLED.to_string()
        } else {
            format!("error: could not run dolt sql-server: {}", e)
        }
    })
}

/// Run the supervisor for `store`: start `dolt sql-server`, record it, and stop it after
/// `idle_secs` with no use, when the store directory goes, or when asked to. Does not
/// return until the server is gone.
pub fn supervise(store: &Path, idle_secs: u64) -> Result<(), String> {
    unsafe {
        let handler = on_signal as extern "C" fn(libc::c_int) as libc::sighandler_t;
        libc::signal(libc::SIGTERM, handler);
        libc::signal(libc::SIGINT, handler);
        libc::signal(libc::SIGHUP, libc::SIG_IGN);
    }
    let paths = Paths::new(store);
    let socket = paths.socket();
    let me = std::process::id();
    let started = now_secs();
    let store_id = paths.store_id();
    let _ = std::fs::remove_file(&socket);
    let _ = std::fs::remove_file(paths.dolt_info());
    let _ = std::fs::remove_file(&paths.log);
    touch_seen(&paths);

    let result = run_server(&paths, &socket, me, started, &store_id, idle_secs);
    // Whatever happened, leave nothing that looks alive.
    if read_info(&paths).is_some_and(|info| info.supervisor == me) {
        let _ = std::fs::remove_file(&paths.pid);
    }
    let _ = std::fs::remove_file(&socket);
    result
}

fn run_server(
    paths: &Paths,
    socket: &Path,
    me: u32,
    started: u64,
    store_id: &str,
    idle_secs: u64,
) -> Result<(), String> {
    let mut child = None;
    let mut port = 0;
    for _ in 0..5 {
        port = free_port()?;
        write_config(paths, port, socket)?;
        let mut spawned = spawn_dolt(paths)?;
        let pid = spawned.id();
        let info = Info {
            supervisor: me,
            dolt: Some(pid),
            ready: false,
            socket: socket.to_path_buf(),
            port: Some(port),
            store_id: store_id.to_string(),
            started,
            idle: idle_secs,
        };
        write_info(paths, &info)?;
        // Ready when the socket answers; failed when the process exits first.
        let end = Instant::now() + Duration::from_secs(START_SECS);
        let mut ready = false;
        let mut exited = false;
        while Instant::now() < end && !TERMINATE.load(Ordering::SeqCst) {
            if let Ok(Some(_)) = spawned.try_wait() {
                exited = true;
                break;
            }
            if Conn::connect(socket, &paths.database(), Some(Duration::from_secs(2)))
                .and_then(|mut c| c.query("select 1").map(|_| ()))
                .is_ok()
            {
                ready = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(15));
        }
        if ready {
            write_info(
                paths,
                &Info {
                    ready: true,
                    ..info
                },
            )?;
            // Dolt's record of the server is how its command line finds the server and
            // talks to it. That answer has every number as a string, which a program that
            // reads `dolt sql -r json` (an older `tsk`, for one) takes for text, and writes
            // from it would be wrong. Without the record the command line opens the store
            // read only: reads give the same typed answers as ever, and a write fails with
            // "database is read only" in place of going wrong.
            let _ = std::fs::remove_file(paths.dolt_info());
            child = Some(spawned);
            break;
        }
        if exited {
            // Most likely the port was taken between choosing it and binding it.
            let log = std::fs::read_to_string(&paths.log).unwrap_or_default();
            if log.to_lowercase().contains("already in use") {
                continue;
            }
            return Err(format!(
                "error: dolt sql-server exited while starting; see {}",
                paths.log.display()
            ));
        }
        // Ran out of time, or asked to stop.
        let _ = spawned.kill();
        let _ = spawned.wait();
        return Err(format!(
            "error: dolt sql-server did not answer; see {}",
            paths.log.display()
        ));
    }
    let Some(mut child) = child else {
        return Err(format!(
            "error: dolt sql-server could not bind a port (last tried {}); see {}",
            port,
            paths.log.display()
        ));
    };

    let idle = Duration::from_secs(idle_secs);
    let mut busy_until = SystemTime::now();
    let mut last_check = Instant::now();
    let why = loop {
        if TERMINATE.load(Ordering::SeqCst) {
            break "asked to stop";
        }
        if let Ok(Some(_)) = child.try_wait() {
            // Died: kill -9, a crash, or someone stopped it.
            return Ok(());
        }
        if !paths.store.join(".dolt").exists() {
            break "the store is gone";
        }
        // A command marks the server as used when it connects.
        if let Ok(modified) = std::fs::metadata(&paths.seen).and_then(|m| m.modified()) {
            if modified > busy_until {
                busy_until = modified;
            }
        }
        let quiet = SystemTime::now()
            .duration_since(busy_until)
            .unwrap_or_default();
        if quiet >= idle && last_check.elapsed() >= Duration::from_millis(500) {
            last_check = Instant::now();
            // A connection that is still open is use, whoever holds it.
            match others_connected(paths, socket) {
                Some(0) => break "idle",
                Some(_) => busy_until = SystemTime::now(),
                None => {}
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    if let Ok(mut log) = OpenOptions::new().append(true).open(&paths.log) {
        let _ = writeln!(log, "tsk: stopping the server: {}", why);
    }
    let pid = child.id();
    signal(pid, libc::SIGTERM);
    let end = Instant::now() + Duration::from_secs(10);
    while Instant::now() < end {
        if let Ok(Some(_)) = child.try_wait() {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    if child.try_wait().ok().flatten().is_none() {
        let _ = child.kill();
        let _ = child.wait();
    }
    // Dolt removes its own record on a clean stop; this covers the other case.
    if std::fs::read_to_string(paths.dolt_info())
        .ok()
        .and_then(|text| text.split(':').next().map(str::to_string))
        .is_some_and(|first| first.trim() == pid.to_string())
    {
        let _ = std::fs::remove_file(paths.dolt_info());
    }
    Ok(())
}

/// The number of connections besides the one that asks, or `None` when it cannot be asked.
fn others_connected(paths: &Paths, socket: &Path) -> Option<i64> {
    let mut conn = Conn::connect(socket, &paths.database(), Some(Duration::from_secs(3))).ok()?;
    let sets = conn
        .query("select count(*) as c from information_schema.processlist")
        .ok()?;
    let count = sets.first()?.rows.first()?.get("c")?.as_i64()?;
    Some(count - 1)
}

// ---------------------------------------------------------------------------
// Running SQL through the server
// ---------------------------------------------------------------------------

thread_local! {
    static CONNECTIONS: std::cell::RefCell<HashMap<PathBuf, Conn>> =
        std::cell::RefCell::new(HashMap::new());
}

/// The commit identity of the session, as `dolt sql` takes it from its configuration.
fn identity_statement() -> Option<String> {
    let root = ["DOLT_ROOT_PATH", "HOME"]
        .iter()
        .find_map(|name| std::env::var_os(name).filter(|v| !v.is_empty()));
    let config: Option<Value> = root.and_then(|root| {
        let file = Path::new(&root).join(".dolt").join("config_global.json");
        serde_json::from_str(&std::fs::read_to_string(file).ok()?).ok()
    });
    let from_config = |key: &str| {
        config
            .as_ref()
            .and_then(|c| c.get(key))
            .and_then(Value::as_str)
            .map(str::to_string)
    };
    let pick = |env: &str, key: &str, fallback: &str| {
        std::env::var(env)
            .ok()
            .filter(|v| !v.is_empty())
            .or_else(|| from_config(key))
            .unwrap_or_else(|| fallback.to_string())
    };
    let name = pick("DOLT_AUTHOR_NAME", "user.name", "tsk");
    let email = pick("DOLT_AUTHOR_EMAIL", "user.email", "tsk@localhost");
    let committer_name = pick("DOLT_COMMITTER_NAME", "user.name", "tsk");
    let committer_email = pick("DOLT_COMMITTER_EMAIL", "user.email", "tsk@localhost");
    let quote = |text: &str| format!("'{}'", text.replace('\\', "\\\\").replace('\'', "''"));
    let mut sets = vec![
        format!("@@session.dolt_author_name = {}", quote(&name)),
        format!("@@session.dolt_author_email = {}", quote(&email)),
        format!("@@session.dolt_committer_name = {}", quote(&committer_name)),
        format!(
            "@@session.dolt_committer_email = {}",
            quote(&committer_email)
        ),
    ];
    for (env, var) in [
        ("DOLT_AUTHOR_DATE", "dolt_author_date"),
        ("DOLT_COMMITTER_DATE", "dolt_committer_date"),
    ] {
        if let Ok(value) = std::env::var(env) {
            if !value.is_empty() {
                sets.push(format!("@@session.{} = {}", var, quote(&value)));
            }
        }
    }
    Some(format!("set {}", sets.join(", ")))
}

fn open(store: &Path) -> Result<Conn, String> {
    let mut conn = connect(store)?;
    if let Some(statement) = identity_statement() {
        conn.query(&statement)
            .map_err(|e| format!("error: could not set the commit identity: {}", e))?;
    }
    Ok(conn)
}

/// Set when a server could not be reached or started: the rest of the process uses the
/// `dolt` command line, and does not try again.
static UNAVAILABLE: AtomicBool = AtomicBool::new(false);

/// Why the server could not be used.
pub enum Failure {
    /// No server could be reached or started. The caller falls back to the `dolt` command.
    Unavailable,
    /// The server ran the statements and one failed, or the connection broke mid-way.
    Failed(SqlError),
}

/// Run `f` on a connection to the server of `store`. A connection that broke before `f`
/// could have changed anything is replaced once: a read-only `f` is run again, and so is
/// one whose first statement never left. `read_only` says that running `f` twice is safe.
pub fn with_connection<T>(
    store: &Path,
    read_only: bool,
    mut f: impl FnMut(&mut Conn) -> Result<T, SqlError>,
) -> Result<T, Failure> {
    if UNAVAILABLE.load(Ordering::SeqCst) {
        return Err(Failure::Unavailable);
    }
    let open_or_give_up = || {
        open(store).map_err(|_| {
            UNAVAILABLE.store(true, Ordering::SeqCst);
            Failure::Unavailable
        })
    };
    let cached = CONNECTIONS.with(|map| map.borrow_mut().remove(store));
    let mut attempts = 0;
    let mut conn = match cached {
        Some(conn) => conn,
        None => open_or_give_up()?,
    };
    loop {
        match f(&mut conn) {
            Ok(value) => {
                CONNECTIONS.with(|map| map.borrow_mut().insert(store.to_path_buf(), conn));
                return Ok(value);
            }
            Err(e) if e.is_connection() && read_only && attempts < 2 => {
                attempts += 1;
                drop(conn);
                conn = open_or_give_up()?;
            }
            Err(e) => {
                if !e.is_connection() {
                    // A statement failed. The connection is fine unless a transaction is
                    // open; roll it back before the connection is reused.
                    let _ = conn.query("rollback");
                    CONNECTIONS.with(|map| map.borrow_mut().insert(store.to_path_buf(), conn));
                }
                return Err(Failure::Failed(e));
            }
        }
    }
}

/// Close this process's connection to the server of `store`, when it holds one.
#[allow(dead_code)]
pub fn disconnect(store: &Path) {
    CONNECTIONS.with(|map| map.borrow_mut().remove(store));
}

/// The text of a failure, in the form the `dolt sql` path gives.
pub fn failure_text(failure: &SqlError) -> String {
    match failure {
        SqlError::Server { message, .. } => {
            format!("error: dolt sql failed:\n{}", message.trim_end())
        }
        other => format!("error: lost the connection to the Dolt server: {}", other),
    }
}

/// True for SQL that only reads.
pub fn is_read_only(sql: &str) -> bool {
    let word: String = sql
        .trim_start()
        .chars()
        .take_while(|c| c.is_ascii_alphabetic())
        .collect::<String>()
        .to_ascii_lowercase();
    matches!(
        word.as_str(),
        "select" | "show" | "with" | "describe" | "explain"
    )
}

/// The rows of every result set, concatenated: the form of `Dolt::rows`.
pub fn flatten(sets: Vec<ResultSet>) -> Vec<Value> {
    sets.into_iter().flat_map(|set| set.rows).collect()
}

// ---------------------------------------------------------------------------
// `tsk ledger server`
// ---------------------------------------------------------------------------

#[derive(clap::Subcommand)]
pub enum ServerCommands {
    #[command(
        about = "Print whether the local Dolt server of this clone's ledger store is running, with its pids, socket and port, and the state of the switch"
    )]
    Status,
    #[command(
        about = "Start the local Dolt server for this clone's ledger store, or replace a dead or hung one, and wait until it answers; works with the switch off"
    )]
    Start,
    #[command(
        about = "Stop the local Dolt server of this clone's ledger store; prints stopped or not running"
    )]
    Stop,
    #[command(
        hide = true,
        about = "The supervisor that `start` and every command run detached"
    )]
    Run {
        #[arg(long)]
        store: PathBuf,
        #[arg(long)]
        idle: u64,
    },
}

/// The ledger store of the clone that holds `cwd`.
pub fn store_of(cwd: &Path) -> Result<PathBuf, String> {
    let repo = crate::ledger::location::Repo::discover(cwd)?;
    let id = repo.read_clone_id()?.ok_or_else(|| {
        "error: this clone has no ledger store yet. Run tsk ledger fetch first".to_string()
    })?;
    let root = crate::ledger::location::state_root_from_env()?;
    let store = crate::ledger::location::dolt_db_path(&root, &id);
    if !store.join(".dolt").is_dir() {
        return Err(format!(
            "error: {} is not a Dolt database. Run tsk ledger fetch first",
            store.display()
        ));
    }
    Ok(store)
}

pub fn run_command(action: ServerCommands, cwd: &Path) -> Result<(), String> {
    match action {
        ServerCommands::Run { store, idle } => supervise(&store, idle),
        ServerCommands::Status => {
            print_status(&store_of(cwd)?);
            Ok(())
        }
        ServerCommands::Start => {
            let store = store_of(cwd)?;
            // Hold the connection until the status has been read: a short idle period
            // must not stop the server between the start and the report.
            let _conn = connect(&store)?;
            print_status(&store);
            Ok(())
        }
        ServerCommands::Stop => {
            let store = store_of(cwd)?;
            disconnect(&store);
            println!(
                "{}",
                if stop(&store)? {
                    "stopped"
                } else {
                    "not running"
                }
            );
            Ok(())
        }
    }
}

fn print_status(store: &Path) {
    let status = status(store);
    println!("server: {}", status.state);
    println!("store: {}", status.paths.store.display());
    if let Some(info) = &status.info {
        if status.state != "not running" {
            if let Some(pid) = info.dolt {
                println!("dolt pid: {}", pid);
            }
            println!("supervisor pid: {}", info.supervisor);
            println!("socket: {}", info.socket.display());
            if let Some(port) = info.port {
                println!("port: {}", port);
            }
            println!("idle stop: {} s", info.idle);
        }
    }
    println!("log: {}", status.paths.log.display());
    let (on, source) = switch();
    println!(
        "switch: {} ({})",
        if on { "on" } else { "off" },
        source.as_str()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_environment_beats_the_config_and_a_bad_value_counts_as_unset() {
        assert_eq!(resolve_switch(None, None), (false, Source::Default));
        assert_eq!(resolve_switch(Some("on"), Some("off")), (true, Source::Env));
        assert_eq!(
            resolve_switch(Some("off"), Some("on")),
            (false, Source::Env)
        );
        assert_eq!(resolve_switch(None, Some("on")), (true, Source::Config));
        assert_eq!(
            resolve_switch(Some("maybe"), Some("on")),
            (true, Source::Config)
        );
        assert_eq!(resolve_switch(Some(""), None), (false, Source::Default));
    }

    #[test]
    fn the_pid_file_round_trips() {
        let info = Info {
            supervisor: 12,
            dolt: Some(34),
            ready: true,
            socket: PathBuf::from("/s/ledger-db/.tsk-dolt.sock"),
            port: Some(40000),
            store_id: "1:2".to_string(),
            started: 99,
            idle: 120,
        };
        assert_eq!(Info::parse(&info.render()), Some(info));
        assert_eq!(Info::parse("junk"), None);
    }

    #[test]
    fn a_long_store_path_puts_the_socket_in_tmp() {
        let short = Paths::new(Path::new("/s/tsk/repos/x/ledger-db"));
        assert_eq!(
            short.socket(),
            PathBuf::from("/s/tsk/repos/x/ledger-db/.tsk-dolt.sock")
        );
        let long = Paths::new(&PathBuf::from(format!("/{}/ledger-db", "d".repeat(120))));
        assert!(long.socket().starts_with("/tmp"));
        assert!(long.socket().as_os_str().len() <= SOCKET_PATH_MAX);
    }

    #[test]
    fn only_selects_count_as_reads() {
        assert!(is_read_only("  select 1"));
        assert!(is_read_only("SHOW TABLES"));
        assert!(!is_read_only("call dolt_commit('-m', 'x')"));
        assert!(!is_read_only("insert into t values (1)"));
    }
}
