//! The local Dolt server (ADR 0018): its lifecycle, its recovery from a dead or hung
//! process, and its behaviour beside the `dolt` command line. Each case drives the `tsk`
//! binary against a bare git repo as origin. The server switch is set by the case, so the
//! cases give the same result whatever `TSK_DOLT_SERVER` the run exports. Cases print a
//! note and return when the `dolt` binary is not installed.

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

use tempfile::TempDir;

use crate::common;

use common::dolt_installed;

use common::templates;

const CLOUD_VAR: &str = "CLAUDE_CODE_REMOTE_SESSION_ID";

fn out(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn err(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "exit {:?}\nstdout: {}\nstderr: {}",
        output.status.code(),
        out(output),
        err(output)
    );
}

fn signal(pid: i32, sig: i32) {
    unsafe {
        libc::kill(pid, sig);
    }
}

fn alive(pid: i32) -> bool {
    if unsafe { libc::kill(pid, 0) } != 0 {
        return false;
    }
    match std::fs::read_to_string(format!("/proc/{}/stat", pid)) {
        Ok(stat) => {
            stat.rsplit_once(')')
                .and_then(|(_, rest)| rest.trim_start().chars().next())
                != Some('Z')
        }
        Err(_) => true,
    }
}

fn wait_until(limit: Duration, what: &str, mut done: impl FnMut() -> bool) {
    let end = Instant::now() + limit;
    while Instant::now() < end {
        if done() {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("timed out waiting for {}", what);
}

/// A directory with a `dolt` stand-in that logs its arguments, one line per run, to the file
/// named by `TSK_DOLT_LOG`, then runs the real `dolt`.
fn dolt_log_shim(root: &Path) -> PathBuf {
    let dir = root.join("dolt-log-shim");
    std::fs::create_dir_all(&dir).unwrap();
    let real = std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
        .map(|dir| dir.join("dolt"))
        .find(|candidate| candidate.is_file())
        .expect("dolt is on PATH");
    let script = format!(
        "#!/bin/sh\necho \"$@\" >> \"$TSK_DOLT_LOG\"\nexec '{}' \"$@\"\n",
        real.display()
    );
    let shim = dir.join("dolt");
    std::fs::write(&shim, script).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&shim, std::fs::Permissions::from_mode(0o755)).unwrap();
    dir
}

/// What a pid file records, by key.
struct Info(HashMap<String, String>);

impl Info {
    fn num(&self, key: &str) -> i32 {
        self.0
            .get(key)
            .unwrap_or_else(|| panic!("no {} in the pid file {:?}", key, self.0))
            .parse()
            .unwrap()
    }

    fn supervisor(&self) -> i32 {
        self.num("supervisor")
    }

    fn dolt(&self) -> i32 {
        self.num("pid")
    }

    fn socket(&self) -> PathBuf {
        PathBuf::from(&self.0["socket"])
    }
}

struct Fx {
    root: TempDir,
    switch: &'static str,
}

impl Drop for Fx {
    fn drop(&mut self) {
        common::stop_servers(self.root.path(), false);
    }
}

impl Fx {
    /// A bare origin with `main`, two clones `work` and `work2`, and the ledger fetched in
    /// `work`. The build runs once per test process with the switch off (see
    /// `common::templates`), and each test gets a copy: the first command of the case
    /// starts its server.
    fn new(switch: &'static str) -> Fx {
        Fx {
            root: templates::instance(&common::templates::FETCHED),
            switch,
        }
    }

    fn home(&self) -> PathBuf {
        self.root.path().join("home")
    }

    fn state(&self) -> PathBuf {
        self.root.path().join("state")
    }

    fn dir(&self, who: char) -> PathBuf {
        self.root
            .path()
            .join(if who == 'a' { "work" } else { "work2" })
    }

    fn command(&self, who: char, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_tsk"));
        command
            .args(args)
            .current_dir(self.dir(who))
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "test@example.com")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "test@example.com")
            .env("HOME", self.home())
            .env("XDG_STATE_HOME", self.state())
            .env("XDG_CONFIG_HOME", self.home().join("config"))
            .env("TSK_HOME", self.home().join(".tsk"))
            .env("TSK_NO_UPDATE_CHECK", "1")
            .env("TSK_LEDGER_REFS", "branches")
            .env("TSK_MACHINE_NAME", "laptop")
            .env("GIT_CEILING_DIRECTORIES", self.root.path())
            .env("TSK_DOLT_SERVER_IDLE_SECS", "60")
            .env_remove(CLOUD_VAR);
        // "unset" leaves the variable out, so the user config or the default decides.
        if self.switch == "unset" {
            command.env_remove("TSK_DOLT_SERVER");
        } else {
            command.env("TSK_DOLT_SERVER", self.switch);
        }
        command
    }

    fn tsk(&self, who: char, args: &[&str]) -> Output {
        self.command(who, args).output().expect("failed to run tsk")
    }

    fn ok(&self, who: char, args: &[&str]) -> String {
        let output = self.tsk(who, args);
        assert_success(&output);
        out(&output)
    }

    /// Run `tsk` with the `dolt` shim first on `PATH`; returns the output and the lines the
    /// shim logged, one per `dolt` process.
    fn counted(&self, who: char, args: &[&str]) -> (Output, Vec<String>) {
        let log = self.root.path().join("dolt-log.txt");
        let _ = std::fs::remove_file(&log);
        let mut path = vec![dolt_log_shim(self.root.path())];
        path.extend(std::env::split_paths(
            &std::env::var_os("PATH").unwrap_or_default(),
        ));
        let output = self
            .command(who, args)
            .env("PATH", std::env::join_paths(path).unwrap())
            .env("TSK_DOLT_LOG", &log)
            .output()
            .unwrap();
        let lines = std::fs::read_to_string(&log)
            .unwrap_or_default()
            .lines()
            .map(str::to_string)
            .collect();
        (output, lines)
    }

    fn clone_id(&self, who: char) -> String {
        std::fs::read_to_string(self.dir(who).join(".git").join("tsk-clone-id"))
            .unwrap()
            .trim()
            .to_string()
    }

    fn state_dir(&self, who: char) -> PathBuf {
        self.state()
            .join("tsk")
            .join("repos")
            .join(self.clone_id(who))
    }

    fn store(&self, who: char) -> PathBuf {
        self.state_dir(who).join("ledger-db")
    }

    fn info(&self, who: char) -> Option<Info> {
        let text = std::fs::read_to_string(self.state_dir(who).join("dolt-server.pid")).ok()?;
        let map = text
            .lines()
            .filter_map(|l| l.split_once('='))
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        Some(Info(map))
    }

    /// The running server of `who`'s store: ready, with live processes.
    fn running(&self, who: char) -> Info {
        let info = self.info(who).expect("a pid file");
        assert_eq!(info.0.get("state").map(String::as_str), Some("ready"));
        assert!(alive(info.supervisor()), "the supervisor is gone");
        assert!(alive(info.dolt()), "the server is gone");
        info
    }

    /// Run the `dolt` command line in `who`'s store.
    fn dolt(&self, who: char, args: &[&str]) -> Output {
        Command::new("dolt")
            .args(args)
            .current_dir(self.store(who))
            .env("HOME", self.home())
            .env("DOLT_DISABLE_EVENT_FLUSH", "1")
            .output()
            .expect("failed to run dolt")
    }
}

/// Connect to the server's socket and log in as `root`, the way any client does, then keep
/// the connection open until the value is dropped.
fn hold_connection(socket: &Path, database: &str) -> std::os::unix::net::UnixStream {
    use std::io::Read;
    let mut stream = std::os::unix::net::UnixStream::connect(socket).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let mut header = [0u8; 4];
    stream.read_exact(&mut header).unwrap();
    let length = header[0] as usize | (header[1] as usize) << 8 | (header[2] as usize) << 16;
    let mut greeting = vec![0u8; length];
    stream.read_exact(&mut greeting).unwrap();
    // CLIENT_LONG_PASSWORD | LONG_FLAG | CONNECT_WITH_DB | PROTOCOL_41 | TRANSACTIONS |
    // SECURE_CONNECTION | PLUGIN_AUTH
    let flags: u32 = 0x1 | 0x4 | 0x8 | 0x200 | 0x2000 | 0x8000 | 0x8_0000;
    let mut body = Vec::new();
    body.extend_from_slice(&flags.to_le_bytes());
    body.extend_from_slice(&0x00ff_ffffu32.to_le_bytes());
    body.push(45);
    body.extend_from_slice(&[0u8; 23]);
    body.extend_from_slice(b"root\0");
    body.push(0);
    body.extend_from_slice(database.as_bytes());
    body.push(0);
    body.extend_from_slice(b"mysql_native_password\0");
    let mut frame = Vec::new();
    frame.extend_from_slice(&(body.len() as u32).to_le_bytes()[..3]);
    frame.push(1);
    frame.extend_from_slice(&body);
    stream.write_all(&frame).unwrap();
    let mut reply = [0u8; 5];
    stream.read_exact(&mut reply).unwrap();
    assert_eq!(reply[4], 0, "the server refused the login");
    stream
}

fn skip() -> bool {
    if dolt_installed() {
        return false;
    }
    eprintln!("note: dolt is not installed; skipping");
    true
}

// --- The switch ---

#[test]
fn with_the_switch_off_no_server_starts_and_the_launches_are_those_of_0_22() {
    if skip() {
        return;
    }
    let fx = Fx::new("off");
    fx.ok('a', &["mission", "create", "M-OFF", "Off"]);
    fx.ok('a', &["ledger", "push", "Publish"]);
    let (listed, launches) = fx.counted('a', &["mission", "list"]);
    assert_success(&listed);
    assert_eq!(launches.len(), 1, "{:?}", launches);
    assert!(launches[0].starts_with("sql"), "{:?}", launches);
    let (added, launches) = fx.counted('a', &["task", "add", "M-OFF", "A task"]);
    assert_success(&added);
    assert_eq!(launches.len(), 2, "{:?}", launches);
    fx.ok('a', &["ledger", "push", "Publish the task"]);
    let (fetched, launches) = fx.counted('a', &["ledger", "fetch"]);
    assert_success(&fetched);
    assert_eq!(launches.len(), 1, "{:?}", launches);
    assert!(
        fx.info('a').is_none(),
        "a pid file exists with the switch off"
    );
    let status = fx.ok('a', &["ledger", "server", "status"]);
    assert!(status.starts_with("server: not running\n"), "{}", status);
    assert!(status.contains("switch: off (environment)"), "{}", status);
}

#[test]
fn with_the_switch_on_a_command_starts_a_server_and_runs_no_dolt_process_for_its_sql() {
    if skip() {
        return;
    }
    let fx = Fx::new("on");
    fx.ok('a', &["mission", "create", "M-ON", "On"]);
    fx.ok('a', &["ledger", "push", "Publish"]);
    let (listed, launches) = fx.counted('a', &["mission", "list"]);
    assert_success(&listed);
    assert_eq!(out(&listed), "M-ON  todo  -  -  On\n");
    assert!(launches.is_empty(), "{:?}", launches);
    let (added, launches) = fx.counted('a', &["task", "add", "M-ON", "A task"]);
    assert_success(&added);
    assert!(launches.is_empty(), "{:?}", launches);
    let info = fx.running('a');
    assert!(info.socket().exists());
    let status = fx.ok('a', &["ledger", "server", "status"]);
    assert!(status.starts_with("server: running\n"), "{}", status);
    assert!(status.contains("switch: on (environment)"), "{}", status);
}

#[test]
fn the_switch_comes_from_the_user_config_when_the_environment_leaves_it_out() {
    if skip() {
        return;
    }
    let fx = Fx::new("off");
    let config = fx.home().join("config").join("tsk");
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(config.join("config.toml"), "[ledger]\nserver = \"on\"\n").unwrap();
    let mut command = fx.command('a', &["ledger", "server", "status"]);
    command.env_remove("TSK_DOLT_SERVER");
    let output = command.output().unwrap();
    assert_success(&output);
    assert!(
        out(&output).contains("switch: on (config)"),
        "{}",
        out(&output)
    );
    // The environment wins over the file.
    let output = fx.tsk('a', &["ledger", "server", "status"]);
    assert!(
        out(&output).contains("switch: off (environment)"),
        "{}",
        out(&output)
    );
}

#[test]
fn with_no_switch_set_the_server_is_used() {
    if skip() {
        return;
    }
    let fx = Fx::new("unset");
    fx.ok('a', &["mission", "create", "M-DEF", "Default"]);
    let (listed, launches) = fx.counted('a', &["mission", "list"]);
    assert_success(&listed);
    assert!(launches.is_empty(), "{:?}", launches);
    assert_eq!(err(&listed), "");
    fx.running('a');
    let status = fx.ok('a', &["ledger", "server", "status"]);
    assert!(status.contains("switch: on (default)"), "{}", status);
    let shown = fx.ok('a', &["config", "ledger-server"]);
    assert_eq!(shown, "ledger server: on\nsource: default\n");
}

#[test]
fn off_in_the_environment_or_the_config_beats_the_default() {
    if skip() {
        return;
    }
    let fx = Fx::new("unset");
    let config = fx.home().join("config").join("tsk");
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(config.join("config.toml"), "[ledger]\nserver = \"off\"\n").unwrap();
    fx.ok('a', &["mission", "create", "M-CFG", "Config off"]);
    assert!(
        fx.info('a').is_none(),
        "a server started with the config off"
    );
    let (listed, launches) = fx.counted('a', &["mission", "list"]);
    assert_success(&listed);
    assert_eq!(launches.len(), 1, "{:?}", launches);
    let status = fx.ok('a', &["ledger", "server", "status"]);
    assert!(status.contains("switch: off (config)"), "{}", status);
    // The environment wins over the file.
    let mut command = fx.command('a', &["config", "ledger-server"]);
    command.env("TSK_DOLT_SERVER", "on");
    assert_eq!(
        out(&command.output().unwrap()),
        "ledger server: on\nsource: environment (TSK_DOLT_SERVER)\n"
    );
    let fx = Fx::new("off");
    assert_eq!(
        fx.ok('a', &["config", "ledger-server"]),
        "ledger server: off\nsource: environment (TSK_DOLT_SERVER)\n"
    );
}

#[test]
fn config_ledger_server_prints_and_writes_the_user_config() {
    if skip() {
        return;
    }
    let fx = Fx::new("unset");
    let file = fx.home().join("config").join("tsk").join("config.toml");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    let before = "# mine\n[nexus]\nurl = \"https://a.test/n\" # keep\n";
    std::fs::write(&file, before).unwrap();
    let written = fx.ok('a', &["config", "ledger-server", "off"]);
    assert!(
        written.starts_with("ledger server: off (written to "),
        "{}",
        written
    );
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(text.starts_with(before), "{}", text);
    assert!(text.contains("[ledger]\nserver = \"off\""), "{}", text);
    assert_eq!(
        fx.ok('a', &["config", "ledger-server"]),
        format!(
            "ledger server: off\nsource: user config ({})\n",
            file.display()
        )
    );
    fx.ok('a', &["config", "ledger-server", "on"]);
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(
        text.contains("server = \"on\"") && text.contains("# mine"),
        "{}",
        text
    );
    assert!(
        fx.tsk('a', &["config", "ledger-server", "maybe"])
            .status
            .code()
            != Some(0)
    );
}

#[test]
fn a_server_that_cannot_start_falls_back_to_dolt_with_one_note() {
    if skip() {
        return;
    }
    let fx = Fx::new("unset");
    // A dolt whose sql-server exits at once. Every other subcommand is the real one.
    let dir = fx.root.path().join("dolt-no-server");
    std::fs::create_dir_all(&dir).unwrap();
    let real = std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
        .map(|d| d.join("dolt"))
        .find(|c| c.is_file())
        .unwrap();
    let shim = dir.join("dolt");
    std::fs::write(
        &shim,
        format!(
            "#!/bin/sh\nif [ \"$1\" = sql-server ]; then echo refused >&2; exit 1; fi\nexec '{}' \"$@\"\n",
            real.display()
        ),
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&shim, std::fs::Permissions::from_mode(0o755)).unwrap();
    let mut path = vec![dir];
    path.extend(std::env::split_paths(&std::env::var_os("PATH").unwrap()));
    let path = std::env::join_paths(path).unwrap();
    let run = |args: &[&str]| fx.command('a', args).env("PATH", &path).output().unwrap();
    let created = run(&["mission", "create", "M-FB", "Fallback"]);
    assert_success(&created);
    let notes = err(&created)
        .lines()
        .filter(|l| l.starts_with("note:"))
        .count();
    assert_eq!(notes, 1, "{}", err(&created));
    assert!(
        err(&created).contains("running dolt directly"),
        "{}",
        err(&created)
    );
    // The command worked without the server: the record is there.
    let listed = run(&["mission", "list", "--porcelain"]);
    assert_success(&listed);
    assert_eq!(out(&listed), "M-FB\ttodo\t-\t-\tFallback\n");
}

// --- Start, status, stop ---

#[test]
fn start_status_and_stop_report_and_clean_up() {
    if skip() {
        return;
    }
    let fx = Fx::new("off");
    fx.ok('a', &["mission", "create", "M-S", "Stop"]);
    fx.ok('a', &["ledger", "push", "Publish"]);
    let started = fx.ok('a', &["ledger", "server", "start"]);
    assert!(started.starts_with("server: running\n"), "{}", started);
    let info = fx.running('a');
    // A second start finds the same server.
    fx.ok('a', &["ledger", "server", "start"]);
    assert_eq!(fx.running('a').dolt(), info.dolt());
    assert_eq!(fx.ok('a', &["ledger", "server", "stop"]), "stopped\n");
    wait_until(Duration::from_secs(10), "the server to go", || {
        !alive(info.dolt()) && !alive(info.supervisor())
    });
    assert!(!info.socket().exists(), "the socket file stayed");
    assert!(fx.info('a').is_none(), "the pid file stayed");
    assert_eq!(fx.ok('a', &["ledger", "server", "stop"]), "not running\n");
    // A stopped server leaves a clean store: nothing staged or changed, and the Dolt
    // command line reads it.
    assert!(!fx.store('a').join(".dolt").join("sql-server.info").exists());
    let status = fx.dolt('a', &["status"]);
    assert_success(&status);
    let text = out(&status);
    assert!(
        text.contains("nothing to commit, working tree clean"),
        "{}",
        text
    );
    assert!(out(&fx.tsk('a', &["mission", "list"])).contains("M-S"));
}

// --- Recovery ---

#[test]
fn a_server_killed_with_sigkill_and_its_socket_left_behind_is_replaced() {
    if skip() {
        return;
    }
    let fx = Fx::new("on");
    fx.ok('a', &["mission", "create", "M-K", "Kill"]);
    let before = fx.running('a');
    // Both processes die at once, as when a container stops: the socket file, the pid file
    // and Dolt's own record stay.
    signal(before.supervisor(), libc::SIGKILL);
    signal(before.dolt(), libc::SIGKILL);
    wait_until(Duration::from_secs(5), "the processes to die", || {
        !alive(before.supervisor()) && !alive(before.dolt())
    });
    assert!(before.socket().exists(), "the socket file did not stay");
    assert!(fx.info('a').is_some());
    let listed = fx.tsk('a', &["mission", "list", "--porcelain"]);
    assert_success(&listed);
    assert_eq!(out(&listed), "M-K\ttodo\t-\t-\tKill\n");
    let after = fx.running('a');
    assert_ne!(after.dolt(), before.dolt());
    // Writes work, and the data from before the kill is there.
    fx.ok('a', &["task", "add", "M-K", "After the kill"]);
    assert!(out(&fx.tsk('a', &["task", "list", "M-K"])).contains("After the kill"));
}

#[test]
fn a_server_that_dies_alone_is_replaced_and_the_supervisor_leaves() {
    if skip() {
        return;
    }
    let fx = Fx::new("on");
    fx.ok('a', &["mission", "create", "M-D", "Die"]);
    let before = fx.running('a');
    signal(before.dolt(), libc::SIGKILL);
    wait_until(Duration::from_secs(10), "the supervisor to notice", || {
        !alive(before.supervisor())
    });
    assert_eq!(
        out(&fx.tsk('a', &["mission", "list", "--porcelain"])),
        "M-D\ttodo\t-\t-\tDie\n"
    );
    assert_ne!(fx.running('a').dolt(), before.dolt());
}

#[test]
fn a_stopped_server_that_hangs_a_connection_is_replaced_after_the_probe_times_out() {
    if skip() {
        return;
    }
    let fx = Fx::new("on");
    fx.ok('a', &["mission", "create", "M-H", "Hang"]);
    let before = fx.running('a');
    signal(before.dolt(), libc::SIGSTOP);
    let started = Instant::now();
    let mut command = fx.command('a', &["mission", "list", "--porcelain"]);
    command.env("TSK_DOLT_SERVER_PROBE_SECS", "2");
    let listed = command.output().unwrap();
    let took = started.elapsed();
    assert_success(&listed);
    assert_eq!(out(&listed), "M-H\ttodo\t-\t-\tHang\n");
    assert!(took < Duration::from_secs(30), "took {:?}", took);
    let after = fx.running('a');
    assert_ne!(after.dolt(), before.dolt());
    wait_until(
        Duration::from_secs(5),
        "the hung server to be killed",
        || !alive(before.dolt()),
    );
    // The new server holds the store: a write lands and reads back.
    fx.ok('a', &["task", "add", "M-H", "After the hang"]);
    assert!(out(&fx.tsk('a', &["task", "list", "M-H"])).contains("After the hang"));
}

#[test]
fn commands_that_start_at_once_start_one_server() {
    if skip() {
        return;
    }
    let fx = Fx::new("on");
    fx.ok('a', &["mission", "create", "M-R", "Race"]);
    fx.ok('a', &["ledger", "server", "stop"]);
    let log = fx.root.path().join("dolt-log.txt");
    let mut path = vec![dolt_log_shim(fx.root.path())];
    path.extend(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    ));
    let path = std::env::join_paths(path).unwrap();
    let children: Vec<_> = (0..4)
        .map(|_| {
            fx.command('a', &["mission", "list", "--porcelain"])
                .env("PATH", &path)
                .env("TSK_DOLT_LOG", &log)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap()
        })
        .collect();
    for child in children {
        let output = child.wait_with_output().unwrap();
        assert_success(&output);
        assert_eq!(out(&output), "M-R\ttodo\t-\t-\tRace\n");
    }
    let launches = std::fs::read_to_string(&log).unwrap_or_default();
    let servers = launches
        .lines()
        .filter(|line| line.starts_with("sql-server"))
        .count();
    assert_eq!(servers, 1, "{}", launches);
    assert_eq!(
        common::servers_running(fx.state_dir('a').to_str().unwrap()),
        1
    );
}

#[test]
fn the_session_start_hook_starts_the_server_and_replaces_a_dead_one() {
    if skip() {
        return;
    }
    let fx = Fx::new("on");
    fx.ok('a', &["mission", "create", "M-SS", "Session"]);
    fx.ok('a', &["ledger", "server", "stop"]);
    let hook = |id: &str| {
        let mut child = fx
            .command('a', &["thread", "session-start", "--format", "claude"])
            .env_remove("CLAUDE_ENV_FILE")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(format!("{{\"session_id\":\"{}\",\"source\":\"startup\"}}", id).as_bytes())
            .unwrap();
        child.wait_with_output().unwrap()
    };
    // No server: the hook starts one, and it is up by the end of the hook or soon after.
    assert_success(&hook("ss-1"));
    wait_until(Duration::from_secs(20), "the server to start", || {
        fx.info('a')
            .is_some_and(|i| i.0.get("state").map(String::as_str) == Some("ready"))
    });
    let first = fx.running('a');
    // A live server is left alone.
    assert_success(&hook("ss-2"));
    assert_eq!(fx.running('a').dolt(), first.dolt());
    // The container hibernates: every process dies, the files stay. The hook replaces it.
    signal(first.supervisor(), libc::SIGKILL);
    signal(first.dolt(), libc::SIGKILL);
    wait_until(Duration::from_secs(5), "the processes to die", || {
        !alive(first.supervisor()) && !alive(first.dolt())
    });
    let resumed = hook("ss-3");
    assert_success(&resumed);
    assert!(
        out(&resumed).contains("hookSpecificOutput"),
        "{}",
        out(&resumed)
    );
    wait_until(Duration::from_secs(20), "the server to be replaced", || {
        fx.info('a').is_some_and(|i| {
            i.0.get("state").map(String::as_str) == Some("ready")
                && i.0.get("pid").and_then(|p| p.parse::<i32>().ok()) != Some(first.dolt())
        })
    });
    let second = fx.running('a');
    assert_ne!(second.dolt(), first.dolt());
    assert_eq!(
        out(&fx.tsk('a', &["mission", "list", "--porcelain"])),
        "M-SS\ttodo\t-\t-\tSession\n"
    );
}

#[test]
fn the_session_start_hook_with_the_default_switch_prints_on_time_and_skips_a_missing_store() {
    if skip() {
        return;
    }
    let hook = |fx: &Fx, who: char, id: &str| {
        let mut child = fx
            .command(who, &["thread", "session-start", "--format", "claude"])
            .env_remove("CLAUDE_ENV_FILE")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(format!("{{\"session_id\":\"{}\",\"source\":\"startup\"}}", id).as_bytes())
            .unwrap();
        let start = Instant::now();
        let output = child.wait_with_output().unwrap();
        (output, start.elapsed())
    };
    // A store exists and no server runs (a container that woke up): the hook replaces it.
    let fx = Fx::new("unset");
    fx.ok('a', &["mission", "create", "M-HD", "Hook default"]);
    fx.ok('a', &["ledger", "server", "stop"]);
    let (output, took) = hook(&fx, 'a', "hd-1");
    assert_success(&output);
    assert!(
        out(&output).contains("hookSpecificOutput"),
        "{}",
        out(&output)
    );
    assert!(took < Duration::from_secs(25), "the hook took {:?}", took);
    wait_until(Duration::from_secs(20), "the server to start", || {
        fx.info('a')
            .is_some_and(|i| i.0.get("state").map(String::as_str) == Some("ready"))
    });
    // The second clone has no ledger store yet. The hook starts no server before it fetches
    // (`store_of` finds no store), and it still prints its context.
    assert!(!fx.dir('b').join(".git").join("tsk-clone-id").exists());
    let (output, _) = hook(&fx, 'b', "hd-2");
    assert_success(&output);
    assert!(
        out(&output).contains("hookSpecificOutput"),
        "{}",
        out(&output)
    );
}

// --- Idle stop and a deleted store ---

#[test]
fn the_server_stops_when_nothing_has_used_it_and_the_next_command_starts_it_again() {
    if skip() {
        return;
    }
    let fx = Fx::new("on");
    fx.ok('a', &["ledger", "server", "stop"]);
    let mut command = fx.command('a', &["mission", "create", "M-I", "Idle"]);
    command.env("TSK_DOLT_SERVER_IDLE_SECS", "2");
    assert_success(&command.output().unwrap());
    let info = fx.running('a');
    wait_until(Duration::from_secs(20), "the idle stop", || {
        !alive(info.dolt()) && !alive(info.supervisor())
    });
    assert!(!info.socket().exists());
    assert!(fx.info('a').is_none());
    assert_eq!(
        out(&fx.tsk('a', &["mission", "list", "--porcelain"])),
        "M-I\ttodo\t-\t-\tIdle\n"
    );
    assert!(fx.info('a').is_some());
}

#[test]
fn a_connection_that_stays_open_keeps_the_server_up_past_the_idle_time() {
    if skip() {
        return;
    }
    let fx = Fx::new("on");
    fx.ok('a', &["ledger", "server", "stop"]);
    let mut command = fx.command('a', &["mission", "create", "M-C", "Held"]);
    command.env("TSK_DOLT_SERVER_IDLE_SECS", "2");
    assert_success(&command.output().unwrap());
    let info = fx.running('a');
    let _held = hold_connection(&info.socket(), "ledger-db");
    std::thread::sleep(Duration::from_secs(6));
    assert!(
        alive(info.dolt()),
        "the server stopped under an open connection"
    );
    drop(_held);
    wait_until(
        Duration::from_secs(20),
        "the idle stop after the connection closed",
        || !alive(info.dolt()),
    );
}

#[test]
fn a_store_that_is_deleted_while_its_server_runs_stops_the_server() {
    if skip() {
        return;
    }
    let fx = Fx::new("on");
    fx.ok('a', &["mission", "create", "M-G", "Gone"]);
    let info = fx.running('a');
    let socket = info.socket();
    std::fs::remove_dir_all(fx.store('a')).unwrap();
    wait_until(Duration::from_secs(20), "the server to stop", || {
        !alive(info.dolt()) && !alive(info.supervisor())
    });
    assert!(!socket.exists());
}

// --- The command line beside a server ---

#[test]
fn the_dolt_command_line_reads_the_store_while_a_server_runs_and_a_write_fails_loudly() {
    if skip() {
        return;
    }
    let fx = Fx::new("on");
    fx.ok('a', &["mission", "create", "M-L", "Line"]);
    let info = fx.running('a');
    // Dolt's record of the server is gone, so the command line does not hand its queries to
    // the server (which prints every number as a string). It opens the store read only.
    assert!(!fx.store('a').join(".dolt").join("sql-server.info").exists());
    let read = fx.dolt(
        'a',
        &[
            "sql",
            "-r",
            "json",
            "-q",
            "select count(*) as c from missions",
        ],
    );
    assert_success(&read);
    assert!(out(&read).contains("\"c\":1"), "{}", out(&read));
    let log = fx.dolt('a', &["log", "--oneline"]);
    assert_success(&log);
    // A change made through the server is there for the next read.
    fx.ok('a', &["task", "add", "M-L", "From the server"]);
    let tasks = fx.dolt(
        'a',
        &["sql", "-r", "json", "-q", "select description from tasks"],
    );
    assert!(out(&tasks).contains("From the server"), "{}", out(&tasks));
    // A write is refused and changes nothing. An older `tsk` meets exactly this.
    let write = fx.dolt(
        'a',
        &[
            "sql",
            "-q",
            "update missions set name = 'Renamed' where code = 'M-L'",
        ],
    );
    assert!(!write.status.success());
    assert!(
        err(&write).to_lowercase().contains("read only"),
        "{}",
        err(&write)
    );
    assert!(out(&fx.tsk('a', &["mission", "list"])).contains("Line"));
    assert_eq!(fx.running('a').dolt(), info.dolt());
    // After the server stops, the same write goes through.
    fx.ok('a', &["ledger", "server", "stop"]);
    assert_success(&fx.dolt(
        'a',
        &[
            "sql",
            "-q",
            "update missions set name = 'Renamed' where code = 'M-L'",
        ],
    ));
    assert!(out(&fx.tsk('a', &["mission", "list"])).contains("Renamed"));
}

#[test]
fn a_command_with_the_switch_off_works_while_a_server_runs_even_a_merge() {
    if skip() {
        return;
    }
    let fx = Fx::new("on");
    fx.ok('a', &["mission", "create", "M-OLD", "Old"]);
    fx.ok('a', &["ledger", "push", "Base"]);
    fx.ok('b', &["ledger", "fetch"]);
    fx.ok('a', &["mission", "create", "M-FROM-A", "From A"]);
    fx.ok('a', &["ledger", "push", "A adds"]);
    let before = fx.running('a');
    // This is what a run with the switch off does: every `dolt` step on the command line.
    // The first write finds the store held by the server, stops it and goes on.
    let off = |who: char, args: &[&str]| {
        let mut command = fx.command(who, args);
        command.env("TSK_DOLT_SERVER", "off");
        let output = command.output().unwrap();
        assert_success(&output);
        out(&output)
    };
    assert!(off('a', &["mission", "list"]).contains("M-OLD"));
    assert!(alive(before.dolt()), "a read stopped the server");
    off(
        'a',
        &["mission", "create", "M-OFF", "Written with the switch off"],
    );
    assert!(!alive(before.dolt()), "a write did not stop the server");
    assert!(out(&fx.tsk('a', &["mission", "list"])).contains("M-OFF"));
    off('a', &["ledger", "push", "A writes with the switch off"]);
    // Clone b has its own store and server. It diverges, then publishes with the switch
    // off while its server runs: the push merges on the command line.
    fx.ok('b', &["mission", "create", "M-FROM-B", "From B"]);
    fx.ok('b', &["ledger", "server", "start"]);
    off('b', &["ledger", "push", "B adds"]);
    off('a', &["ledger", "fetch"]);
    let listed = off('a', &["mission", "list", "--porcelain"]);
    assert!(
        listed.contains("M-FROM-A") && listed.contains("M-FROM-B"),
        "{}",
        listed
    );
}

// --- The ledger flows through the server ---

#[test]
fn two_clones_publish_through_their_servers() {
    if skip() {
        return;
    }
    let fx = Fx::new("on");
    fx.ok('a', &["mission", "create", "M-BASE", "Base"]);
    fx.ok('a', &["ledger", "push", "Base"]);
    fx.ok('b', &["ledger", "fetch"]);
    fx.ok('a', &["mission", "create", "M-FROM-A", "From A"]);
    fx.ok('a', &["ledger", "push", "A adds a mission"]);
    fx.ok('b', &["mission", "create", "M-FROM-B", "From B"]);
    fx.ok('b', &["ledger", "push", "B adds a mission"]);
    fx.ok('a', &["ledger", "fetch"]);
    let expected =
        "M-BASE\ttodo\t-\t-\tBase\nM-FROM-A\ttodo\t-\t-\tFrom A\nM-FROM-B\ttodo\t-\t-\tFrom B\n";
    assert_eq!(fx.ok('a', &["mission", "list", "--porcelain"]), expected);
    fx.ok('b', &["ledger", "fetch"]);
    assert_eq!(fx.ok('b', &["mission", "list", "--porcelain"]), expected);
    // Each clone has a server of its own.
    let (a, b) = (fx.running('a'), fx.running('b'));
    assert_ne!(a.dolt(), b.dolt());
}

#[test]
fn a_push_that_conflicts_stops_with_the_same_message_and_leaves_origin_alone() {
    if skip() {
        return;
    }
    let message = |switch: &'static str| {
        let fx = Fx::new(switch);
        fx.ok('a', &["mission", "create", "M-X", "Conflict"]);
        fx.ok('a', &["ledger", "push", "Base"]);
        fx.ok('b', &["ledger", "fetch"]);
        fx.ok('a', &["mission", "set", "M-X", "status", "in-progress"]);
        fx.ok('a', &["ledger", "push", "A starts it"]);
        fx.ok('b', &["mission", "set", "M-X", "status", "done"]);
        let conflict = fx.tsk('b', &["ledger", "push", "B finishes it"]);
        assert_eq!(conflict.status.code(), Some(1), "{}", err(&conflict));
        let text = err(&conflict).replace(fx.root.path().to_str().unwrap(), "<root>");
        // The local commit stays, and a retry after the discard works.
        let status = fx.ok('b', &["ledger", "status", "--porcelain"]);
        assert!(!status.is_empty());
        (
            text,
            status.replace(fx.root.path().to_str().unwrap(), "<root>"),
        )
    };
    let (off_text, off_status) = message("off");
    let (on_text, on_status) = message("on");
    assert!(on_text.contains("conflicted in missions"), "{}", on_text);
    let strip = |text: &str| {
        // The clone id names the store directory.
        text.split_whitespace()
            .filter(|word| !word.contains("/state/tsk/repos/"))
            .collect::<Vec<_>>()
            .join(" ")
    };
    assert_eq!(strip(&on_text), strip(&off_text));
    assert_eq!(on_status.lines().count(), off_status.lines().count());
}

#[test]
fn a_collision_is_renumbered_at_the_push_with_the_server_on() {
    if skip() {
        return;
    }
    let fx = Fx::new("on");
    fx.ok('a', &["mission", "create", "M-N", "Number"]);
    fx.ok('a', &["ledger", "push", "Base"]);
    fx.ok('b', &["ledger", "fetch"]);
    fx.ok('a', &["task", "add", "M-N", "From A"]);
    fx.ok('a', &["ledger", "push", "A adds a task"]);
    fx.ok('b', &["task", "add", "M-N", "From B"]);
    let pushed = fx.tsk('b', &["ledger", "push", "B adds a task"]);
    assert_success(&pushed);
    assert!(
        out(&pushed).contains("T-01 is now T-02 in M-N"),
        "{}\n{}",
        out(&pushed),
        err(&pushed)
    );
    fx.ok('a', &["ledger", "fetch"]);
    let tasks = fx.ok('a', &["task", "list", "M-N"]);
    assert!(
        tasks.contains("T-01") && tasks.contains("From A"),
        "{}",
        tasks
    );
    assert!(
        tasks.contains("T-02") && tasks.contains("From B"),
        "{}",
        tasks
    );
}
