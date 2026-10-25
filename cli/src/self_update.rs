use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use clap::Args;

pub const REPO_URL_VAR: &str = "TSK_RELEASE_REPO_URL";
pub const DOWNLOAD_URL_VAR: &str = "TSK_RELEASE_DOWNLOAD_URL";
pub const NO_CHECK_VAR: &str = "TSK_NO_UPDATE_CHECK";
/// `always` checks the network at every session start. `never` does not check.
pub const UPDATE_CHECK_VAR: &str = "TSK_UPDATE_CHECK";
/// The file in the tsk state directory that holds the last check.
const CACHE_FILE: &str = "update-check";
/// The session start check goes to the network at most this often.
const CHECK_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);
pub const DEFAULT_REPO_URL: &str = "https://github.com/jimbarritt/tsk";
pub const DEFAULT_DOWNLOAD_URL: &str = "https://github.com/jimbarritt/tsk/releases/download";
pub const BREW_FORMULA: &str = "jimbarritt/tap/tsk";
pub const CRATE_NAME: &str = "tsk-bin";

/// How long `tsk self-update` waits for the tag lookup.
const LOOKUP_TIMEOUT: Duration = Duration::from_secs(60);
/// How long the session start notice waits for the tag lookup.
const NOTICE_TIMEOUT: Duration = Duration::from_secs(5);
/// How long the cloud upgrade waits for the tag lookup.
const CLOUD_LOOKUP_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Args)]
pub struct SelfUpdateArgs {
    #[arg(
        long,
        help = "Only report whether a newer release exists; never install"
    )]
    pub check: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version(pub u64, pub u64, pub u64);

impl Version {
    pub fn parse(text: &str) -> Option<Version> {
        let mut parts = text.split('.');
        let mut next = || -> Option<u64> {
            let part = parts.next()?;
            if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            part.parse().ok()
        };
        let version = Version(next()?, next()?, next()?);
        if parts.next().is_some() {
            return None;
        }
        Some(version)
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.0, self.1, self.2)
    }
}

/// The highest `vX.Y.Z` tag in `git ls-remote --tags --refs` output. Tags that do
/// not parse as three numeric parts are ignored.
pub fn highest_version(ls_remote: &str) -> Option<Version> {
    ls_remote
        .lines()
        .filter_map(|line| line.split_whitespace().nth(1))
        .filter_map(|reference| reference.strip_prefix("refs/tags/v"))
        .filter_map(Version::parse)
        .max()
}

pub fn running_version() -> Result<Version, String> {
    let text = env!("CARGO_PKG_VERSION");
    Version::parse(text).ok_or_else(|| format!("error: the running version {} is not X.Y.Z", text))
}

fn repo_url() -> String {
    std::env::var(REPO_URL_VAR)
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| DEFAULT_REPO_URL.to_string())
}

fn download_base() -> String {
    std::env::var(DOWNLOAD_URL_VAR)
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| DEFAULT_DOWNLOAD_URL.to_string())
}

/// How often the session start lookup goes to the network.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckMode {
    /// At most once in `CHECK_INTERVAL`; in between, the last result is used.
    Daily,
    /// At every session start (`TSK_UPDATE_CHECK=always`).
    Always,
    /// Never (`TSK_UPDATE_CHECK=never`, or `TSK_NO_UPDATE_CHECK` set to any value but 0).
    Never,
}

/// The mode from the values of `TSK_UPDATE_CHECK` and `TSK_NO_UPDATE_CHECK`. Any other
/// value of `TSK_UPDATE_CHECK` means `Daily`.
pub fn check_mode(update_check: Option<&str>, no_check: Option<&str>) -> CheckMode {
    if no_check.is_some_and(|v| !v.is_empty() && v != "0") {
        return CheckMode::Never;
    }
    match update_check
        .map(|v| v.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("never") => CheckMode::Never,
        Some("always") => CheckMode::Always,
        _ => CheckMode::Daily,
    }
}

fn mode_from_env() -> CheckMode {
    check_mode(
        std::env::var(UPDATE_CHECK_VAR).ok().as_deref(),
        std::env::var(NO_CHECK_VAR).ok().as_deref(),
    )
}

pub fn check_disabled() -> bool {
    mode_from_env() == CheckMode::Never
}

/// The result of the last network lookup, kept in the tsk state directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedCheck {
    /// Seconds since the Unix epoch.
    pub checked_at: u64,
    /// The repository the lookup asked, so a changed `TSK_RELEASE_REPO_URL` misses.
    pub repo_url: String,
    pub latest: Option<Version>,
}

impl CachedCheck {
    pub fn render(&self) -> String {
        format!(
            "checked_at={}\nrepo_url={}\nlatest={}\n",
            self.checked_at,
            self.repo_url,
            self.latest
                .map_or_else(|| "none".to_string(), |v| v.to_string())
        )
    }

    pub fn parse(text: &str) -> Option<CachedCheck> {
        let field = |name: &str| {
            text.lines()
                .find_map(|line| line.strip_prefix(name)?.strip_prefix('='))
        };
        let latest = match field("latest")? {
            "none" => None,
            version => Some(Version::parse(version)?),
        };
        Some(CachedCheck {
            checked_at: field("checked_at")?.parse().ok()?,
            repo_url: field("repo_url")?.to_string(),
            latest,
        })
    }

    /// True when the result answers a lookup of `repo_url` made at `now`, and is less
    /// than `CHECK_INTERVAL` old. A result dated in the future (the clock moved back)
    /// is not fresh.
    pub fn is_fresh(&self, repo_url: &str, now: u64) -> bool {
        self.repo_url == repo_url
            && self.checked_at <= now
            && now - self.checked_at < CHECK_INTERVAL.as_secs()
    }
}

/// A release lookup, from the network or from the cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lookup {
    pub latest: Option<Version>,
    /// Seconds since the Unix epoch of the network lookup, set when the cache answered.
    pub cached_from: Option<u64>,
}

fn cache_file() -> Option<PathBuf> {
    crate::ledger::location::state_root_from_env()
        .ok()
        .map(|root| root.join(CACHE_FILE))
}

fn read_cache(file: &Path) -> Option<CachedCheck> {
    CachedCheck::parse(&std::fs::read_to_string(file).ok()?)
}

fn write_cache(file: &Path, check: &CachedCheck) {
    // A cache that cannot be written only means the next start looks again.
    let _ = (|| -> std::io::Result<()> {
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let staged = file.with_extension(format!("tmp{}", std::process::id()));
        std::fs::write(&staged, check.render())?;
        std::fs::rename(&staged, file).inspect_err(|_| {
            let _ = std::fs::remove_file(&staged);
        })
    })();
}

/// The latest release under `mode`. `Daily` answers from the cache file when it is
/// fresh, and otherwise calls `fetch` and stores its result. `Always` calls `fetch`
/// and stores the result. `Never` returns `Ok` with no release and does not call
/// `fetch`. A failed `fetch` is not stored, so the next call looks again. `now` is
/// seconds since the Unix epoch.
pub fn lookup_with(
    mode: CheckMode,
    cache: Option<&Path>,
    repo_url: &str,
    now: u64,
    fetch: &dyn Fn() -> Result<Option<Version>, String>,
) -> Result<Lookup, String> {
    if mode == CheckMode::Never {
        return Ok(Lookup {
            latest: None,
            cached_from: None,
        });
    }
    if mode == CheckMode::Daily {
        if let Some(cached) = cache.and_then(read_cache) {
            if cached.is_fresh(repo_url, now) {
                return Ok(Lookup {
                    latest: cached.latest,
                    cached_from: Some(cached.checked_at),
                });
            }
        }
    }
    let latest = fetch()?;
    if let Some(file) = cache {
        write_cache(
            file,
            &CachedCheck {
                checked_at: now,
                repo_url: repo_url.to_string(),
                latest,
            },
        );
    }
    Ok(Lookup {
        latest,
        cached_from: None,
    })
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// The release lookup of the session start paths: cached under `TSK_UPDATE_CHECK`.
fn session_lookup(timeout: Duration) -> Result<Lookup, String> {
    let url = repo_url();
    lookup_with(
        mode_from_env(),
        cache_file().as_deref(),
        &url,
        unix_now(),
        &|| Ok(highest_version(&ls_remote(&url, timeout)?)),
    )
}

/// The sentence added to the notice when the cache answered.
pub fn cached_sentence(checked_at: u64, now: u64) -> String {
    let hours = now.saturating_sub(checked_at) / 3600;
    let age = match hours {
        0 => "less than an hour ago".to_string(),
        1 => "1 hour ago".to_string(),
        n => format!("{} hours ago", n),
    };
    format!(
        "The release check is cached: it last ran {}, and runs once a day. Set TSK_UPDATE_CHECK=always to check now.",
        age
    )
}

fn drain<R: Read + Send + 'static>(stream: Option<R>) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        if let Some(mut stream) = stream {
            let _ = stream.read_to_end(&mut bytes);
        }
        bytes
    })
}

/// Runs `git ls-remote` against `url` and kills it after `timeout`.
fn ls_remote(url: &str, timeout: Duration) -> Result<String, String> {
    let mut command = Command::new("git");
    command
        .args(["ls-remote", "--tags", "--refs", url, "refs/tags/v*"])
        .env("GIT_TERMINAL_PROMPT", "0");
    crate::gitenv::apply(&mut command);
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("error: could not run git to find the latest release: {}", e))?;
    let out = drain(child.stdout.take());
    let err = drain(child.stderr.take());
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if start.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!(
                    "error: looking up the latest release at {} timed out after {} seconds",
                    url,
                    timeout.as_secs()
                ));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(e) => return Err(format!("error: could not wait for git: {}", e)),
        }
    };
    let stdout = out.join().unwrap_or_default();
    let stderr = err.join().unwrap_or_default();
    if !status.success() {
        let stderr = String::from_utf8_lossy(&stderr);
        return Err(format!(
            "error: could not list the releases at {}: {}",
            url,
            stderr.trim()
        ));
    }
    Ok(String::from_utf8_lossy(&stdout).into_owned())
}

/// The highest release version, or `None` when the repo has no `vX.Y.Z` tag.
pub fn latest_release(timeout: Duration) -> Result<Option<Version>, String> {
    Ok(highest_version(&ls_remote(&repo_url(), timeout)?))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Homebrew,
    Cargo,
    ReleaseBinary,
}

impl Method {
    pub fn label(self) -> &'static str {
        match self {
            Method::Homebrew => "Homebrew",
            Method::Cargo => "Cargo",
            Method::ReleaseBinary => "release binary",
        }
    }
}

/// How the binary at `exe` (already canonicalised) was installed. A path with
/// `/Cellar/`, `/homebrew/` or `/linuxbrew/` is Homebrew. A path under
/// `$CARGO_HOME/bin` or `~/.cargo/bin` is Cargo. Anything else is a release binary.
pub fn detect_method(exe: &Path, cargo_home: Option<&Path>, home: Option<&Path>) -> Method {
    let text = exe.to_string_lossy();
    if ["/Cellar/", "/homebrew/", "/linuxbrew/"]
        .iter()
        .any(|marker| text.contains(marker))
    {
        return Method::Homebrew;
    }
    let cargo_bins = cargo_home
        .map(|dir| dir.join("bin"))
        .into_iter()
        .chain(home.map(|dir| dir.join(".cargo").join("bin")));
    for bin in cargo_bins {
        let bin = std::fs::canonicalize(&bin).unwrap_or(bin);
        if exe.starts_with(&bin) {
            return Method::Cargo;
        }
    }
    Method::ReleaseBinary
}

/// The release target for a platform, as `std::env::consts` names it.
pub fn target_for(os: &str, arch: &str) -> Result<&'static str, String> {
    match (os, arch) {
        ("macos", "aarch64") => Ok("aarch64-apple-darwin"),
        ("macos", "x86_64") => Ok("x86_64-apple-darwin"),
        ("linux", "x86_64") => Ok("x86_64-unknown-linux-gnu"),
        ("linux", "aarch64") => Ok("aarch64-unknown-linux-gnu"),
        _ => Err(format!(
            "error: no tsk release binary for {} on {}; install with cargo install {} --locked",
            os, arch, CRATE_NAME
        )),
    }
}

pub fn archive_url(base: &str, version: Version, target: &str) -> String {
    format!(
        "{}/v{}/tsk-{}-{}.tar.gz",
        base.trim_end_matches('/'),
        version,
        version,
        target
    )
}

/// Runs a command with its stdout sent to stderr, so `self-update` keeps stdout for
/// its own lines and a hook keeps stdout for its JSON.
fn run_streaming(program: &str, args: &[&str]) -> Result<(), String> {
    let command = format!("{} {}", program, args.join(" "));
    let status = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::from(std::io::stderr()))
        .stderr(Stdio::inherit())
        .status()
        .map_err(|e| format!("error: could not run {}: {}", command, e))?;
    if status.success() {
        return Ok(());
    }
    Err(match status.code() {
        Some(code) => format!("error: {} failed (exit status {})", command, code),
        None => format!("error: {} was terminated by a signal", command),
    })
}

fn permission_hint(path: &Path, e: &std::io::Error) -> String {
    if e.kind() == std::io::ErrorKind::PermissionDenied {
        format!(
            "error: no permission to replace {} ({}); rerun tsk self-update with sufficient permissions, for example with sudo",
            path.display(),
            e
        )
    } else {
        format!("error: could not replace {}: {}", path.display(), e)
    }
}

/// Copies `new` next to `exe` and renames it over `exe`. The rename is atomic on one
/// filesystem, and a running binary keeps its old inode.
fn replace_binary(new: &Path, exe: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let dir = exe
        .parent()
        .ok_or_else(|| format!("error: {} has no parent directory", exe.display()))?;
    let staged = dir.join(format!(".tsk-new-{}", std::process::id()));
    let result = std::fs::copy(new, &staged)
        .and_then(|_| std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o755)))
        .and_then(|_| std::fs::rename(&staged, exe));
    if let Err(e) = result {
        let _ = std::fs::remove_file(&staged);
        return Err(permission_hint(exe, &e));
    }
    Ok(())
}

fn install_release_binary(exe: &Path, version: Version) -> Result<(), String> {
    let target = target_for(std::env::consts::OS, std::env::consts::ARCH)?;
    let url = archive_url(&download_base(), version, target);
    let tmp = std::env::temp_dir().join(format!("tsk-self-update-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp)
        .map_err(|e| format!("error: could not create {}: {}", tmp.display(), e))?;
    let result = (|| {
        let archive = tmp.join("archive");
        run_streaming("curl", &["-sSfL", "-o", &archive.to_string_lossy(), &url])?;
        run_streaming(
            "tar",
            &[
                "-xzf",
                &archive.to_string_lossy(),
                "-C",
                &tmp.to_string_lossy(),
            ],
        )?;
        let new = tmp.join("tsk");
        if !new.is_file() {
            return Err(format!("error: {} holds no tsk binary at its root", url));
        }
        replace_binary(&new, exe)
    })();
    let _ = std::fs::remove_dir_all(&tmp);
    result
}

fn current_exe() -> Result<PathBuf, String> {
    let exe = std::env::current_exe()
        .map_err(|e| format!("error: could not find the running binary: {}", e))?;
    Ok(std::fs::canonicalize(&exe).unwrap_or(exe))
}

/// Installs `version`. A Homebrew binary upgrades with brew. Any other binary takes the
/// release archive for its platform, and a binary under the cargo bin directory falls back
/// to `cargo install` when the archive fails. Returns the method used.
fn install(version: Version) -> Result<Method, String> {
    let exe = current_exe()?;
    let cargo_home = std::env::var_os("CARGO_HOME").map(PathBuf::from);
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let method = detect_method(&exe, cargo_home.as_deref(), home.as_deref());
    eprintln!("installed as: {}", method.label());
    match method {
        Method::Homebrew => run_streaming("brew", &["upgrade", BREW_FORMULA])?,
        Method::ReleaseBinary => install_release_binary(&exe, version)?,
        // The release archive installs with no compile. `cargo install` builds the crate,
        // which takes about 25 s, so it is the fallback for an archive that cannot be
        // installed (no archive for the platform, or no release asset yet).
        Method::Cargo => {
            if let Err(e) = install_release_binary(&exe, version) {
                eprintln!("{}", e);
                eprintln!("falling back to cargo install");
                run_streaming(
                    "cargo",
                    &[
                        "install",
                        CRATE_NAME,
                        "--locked",
                        "--version",
                        &version.to_string(),
                    ],
                )?;
                return Ok(Method::Cargo);
            }
            return Ok(Method::ReleaseBinary);
        }
    }
    Ok(method)
}

pub fn run(args: SelfUpdateArgs) -> Result<(), String> {
    let current = running_version()?;
    let latest = latest_release(LOOKUP_TIMEOUT)?
        .ok_or_else(|| format!("error: {} has no release tag vX.Y.Z", repo_url()))?;
    if latest <= current {
        println!("tsk {} is the latest release", current);
        return Ok(());
    }
    if args.check {
        println!(
            "tsk {} is available (running {}); run tsk self-update",
            latest, current
        );
        return Ok(());
    }
    let method = install(latest)?;
    println!("updated tsk {} -> {} ({})", current, latest, method.label());
    Ok(())
}

/// What an update did: the versions before and after, and the install method.
pub struct Updated {
    pub from: Version,
    pub to: Version,
    pub method: Method,
}

/// Installs the latest release when it is newer than the running version, and prints
/// nothing to stdout. `None` means the running version is the latest. This is the update
/// that `tsk self-update` makes, for a caller that needs the result.
pub fn update_to_latest() -> Result<Option<Updated>, String> {
    let from = running_version()?;
    let to = latest_release(LOOKUP_TIMEOUT)?
        .ok_or_else(|| format!("error: {} has no release tag vX.Y.Z", repo_url()))?;
    if to <= from {
        return Ok(None);
    }
    let method = install(to)?;
    Ok(Some(Updated { from, to, method }))
}

pub fn available_sentence(latest: Version, current: Version) -> String {
    format!(
        "tsk {} is available (running {}). Run `tsk self-update` to upgrade.",
        latest, current
    )
}

/// The session start notice, or `None` when tsk is current or the lookup fails,
/// times out or is switched off.
pub fn session_notice() -> Option<String> {
    if check_disabled() {
        return None;
    }
    let current = running_version().ok()?;
    let lookup = session_lookup(NOTICE_TIMEOUT).ok()?;
    notice_for(lookup, current, unix_now())
}

/// The notice for a lookup, or `None` when `current` is the latest release. A cached
/// lookup adds a sentence that says so.
pub fn notice_for(lookup: Lookup, current: Version, now: u64) -> Option<String> {
    let latest = lookup.latest.filter(|latest| *latest > current)?;
    let mut notice = available_sentence(latest, current);
    if let Some(checked_at) = lookup.cached_from {
        notice.push(' ');
        notice.push_str(&cached_sentence(checked_at, now));
    }
    Some(notice)
}

#[derive(Debug, PartialEq, Eq)]
pub enum CloudUpgrade {
    /// Nothing to do: tsk is current, the check is off, or the lookup failed.
    Current,
    Updated,
    Failed(String),
}

/// Upgrades tsk in a cloud session when a newer release exists. Output goes to stderr.
pub fn upgrade_for_cloud() -> CloudUpgrade {
    if check_disabled() {
        return CloudUpgrade::Current;
    }
    let Ok(current) = running_version() else {
        return CloudUpgrade::Current;
    };
    let latest = match session_lookup(CLOUD_LOOKUP_TIMEOUT).map(|lookup| lookup.latest) {
        Ok(Some(latest)) if latest > current => latest,
        Ok(_) => return CloudUpgrade::Current,
        Err(e) => {
            eprintln!("{}", e);
            return CloudUpgrade::Current;
        }
    };
    match install(latest) {
        Ok(method) => {
            eprintln!("updated tsk {} -> {} ({})", current, latest, method.label());
            CloudUpgrade::Updated
        }
        Err(e) => {
            eprintln!("{}", e);
            CloudUpgrade::Failed(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LS: &str = "aaa\trefs/tags/v0.9.0\nbbb\trefs/tags/v0.10.0\nccc\trefs/tags/v0.2.1\nddd\trefs/tags/v1.0\neee\trefs/tags/v1.0.0-rc1\nfff\trefs/tags/vnext\nggg\trefs/tags/archive/tsk-bootstrap\nhhh\trefs/tags/v0.1.0.1\n";

    #[test]
    fn version_parse_needs_three_numeric_parts() {
        assert_eq!(Version::parse("0.10.2"), Some(Version(0, 10, 2)));
        for bad in ["1.0", "1.0.0.1", "1.0.0-rc1", "", "a.b.c", "1..2", "+1.0.0"] {
            assert_eq!(Version::parse(bad), None, "{}", bad);
        }
    }

    #[test]
    fn highest_version_compares_numerically_and_ignores_other_tags() {
        assert_eq!(highest_version(LS), Some(Version(0, 10, 0)));
        assert!(Version(0, 10, 0) > Version(0, 9, 0));
        assert_eq!(highest_version(""), None);
        assert_eq!(highest_version("aaa\trefs/tags/vnext\n"), None);
    }

    #[test]
    fn homebrew_paths_are_detected_first() {
        for path in [
            "/opt/homebrew/Cellar/tsk/0.6.0/bin/tsk",
            "/usr/local/Cellar/tsk/0.6.0/bin/tsk",
            "/home/linuxbrew/.linuxbrew/Cellar/tsk/0.6.0/bin/tsk",
            "/opt/homebrew/bin/tsk",
        ] {
            assert_eq!(
                detect_method(Path::new(path), None, None),
                Method::Homebrew,
                "{}",
                path
            );
        }
    }

    #[test]
    fn cargo_paths_come_from_cargo_home_or_the_home_directory() {
        let exe = Path::new("/home/u/.cargo/bin/tsk");
        assert_eq!(
            detect_method(exe, None, Some(Path::new("/home/u"))),
            Method::Cargo
        );
        assert_eq!(
            detect_method(
                Path::new("/data/cargo/bin/tsk"),
                Some(Path::new("/data/cargo")),
                Some(Path::new("/home/u"))
            ),
            Method::Cargo
        );
        assert_eq!(
            detect_method(exe, Some(Path::new("/data/cargo")), None),
            Method::ReleaseBinary
        );
    }

    #[test]
    fn any_other_path_is_a_release_binary() {
        assert_eq!(
            detect_method(
                Path::new("/usr/local/bin/tsk"),
                Some(Path::new("/data/cargo")),
                Some(Path::new("/home/u"))
            ),
            Method::ReleaseBinary
        );
    }

    #[test]
    fn targets_cover_the_four_release_platforms() {
        assert_eq!(
            target_for("macos", "aarch64").unwrap(),
            "aarch64-apple-darwin"
        );
        assert_eq!(
            target_for("macos", "x86_64").unwrap(),
            "x86_64-apple-darwin"
        );
        assert_eq!(
            target_for("linux", "x86_64").unwrap(),
            "x86_64-unknown-linux-gnu"
        );
        assert_eq!(
            target_for("linux", "aarch64").unwrap(),
            "aarch64-unknown-linux-gnu"
        );
        let err = target_for("windows", "x86_64").unwrap_err();
        assert!(err.contains("windows on x86_64"), "{}", err);
        assert!(target_for("linux", "riscv64").is_err());
    }

    #[test]
    fn archive_url_follows_the_release_layout() {
        assert_eq!(
            archive_url(DEFAULT_DOWNLOAD_URL, Version(0, 7, 0), "x86_64-apple-darwin"),
            "https://github.com/jimbarritt/tsk/releases/download/v0.7.0/tsk-0.7.0-x86_64-apple-darwin.tar.gz"
        );
        assert_eq!(
            archive_url("file:///r/", Version(1, 0, 0), "t"),
            "file:///r/v1.0.0/tsk-1.0.0-t.tar.gz"
        );
    }

    const DAY: u64 = 24 * 60 * 60;
    const URL: &str = "https://example.test/tsk";

    fn counting_fetch(
        calls: &std::cell::Cell<u32>,
        result: Option<Version>,
    ) -> impl Fn() -> Result<Option<Version>, String> + '_ {
        move || {
            calls.set(calls.get() + 1);
            Ok(result)
        }
    }

    #[test]
    fn the_mode_follows_the_two_variables() {
        assert_eq!(check_mode(None, None), CheckMode::Daily);
        assert_eq!(check_mode(Some("daily"), None), CheckMode::Daily);
        assert_eq!(check_mode(Some("nonsense"), None), CheckMode::Daily);
        assert_eq!(check_mode(Some("always"), None), CheckMode::Always);
        assert_eq!(check_mode(Some(" Always "), None), CheckMode::Always);
        assert_eq!(check_mode(Some("never"), None), CheckMode::Never);
        assert_eq!(check_mode(None, Some("1")), CheckMode::Never);
        assert_eq!(check_mode(Some("always"), Some("1")), CheckMode::Never);
        assert_eq!(check_mode(None, Some("0")), CheckMode::Daily);
        assert_eq!(check_mode(None, Some("")), CheckMode::Daily);
    }

    #[test]
    fn a_cached_check_round_trips_and_a_damaged_file_is_ignored() {
        let found = CachedCheck {
            checked_at: 1_800_000_000,
            repo_url: URL.to_string(),
            latest: Some(Version(0, 17, 0)),
        };
        assert_eq!(CachedCheck::parse(&found.render()), Some(found));
        let none = CachedCheck {
            checked_at: 5,
            repo_url: URL.to_string(),
            latest: None,
        };
        assert_eq!(CachedCheck::parse(&none.render()), Some(none));
        for bad in [
            "",
            "latest=1.0.0\n",
            "checked_at=x\nrepo_url=u\nlatest=none\n",
        ] {
            assert_eq!(CachedCheck::parse(bad), None, "{}", bad);
        }
    }

    #[test]
    fn a_check_is_fresh_for_24_hours_for_the_same_repository() {
        let check = CachedCheck {
            checked_at: 1_000,
            repo_url: URL.to_string(),
            latest: None,
        };
        assert!(check.is_fresh(URL, 1_000));
        assert!(check.is_fresh(URL, 1_000 + DAY - 1));
        assert!(!check.is_fresh(URL, 1_000 + DAY));
        assert!(!check.is_fresh("https://other.test/tsk", 1_100));
        // The clock moved back.
        assert!(!check.is_fresh(URL, 999));
    }

    #[test]
    fn the_daily_mode_looks_once_a_day() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("state").join("update-check");
        let calls = std::cell::Cell::new(0);
        let fetch = counting_fetch(&calls, Some(Version(9, 0, 0)));
        let t0 = 1_800_000_000;

        let first = lookup_with(CheckMode::Daily, Some(&cache), URL, t0, &fetch).unwrap();
        assert_eq!(first.latest, Some(Version(9, 0, 0)));
        assert_eq!(first.cached_from, None);
        assert_eq!(calls.get(), 1);

        let again =
            lookup_with(CheckMode::Daily, Some(&cache), URL, t0 + 3 * 3600, &fetch).unwrap();
        assert_eq!(again.latest, Some(Version(9, 0, 0)));
        assert_eq!(again.cached_from, Some(t0));
        assert_eq!(calls.get(), 1);

        let next_day = lookup_with(CheckMode::Daily, Some(&cache), URL, t0 + DAY, &fetch).unwrap();
        assert_eq!(next_day.cached_from, None);
        assert_eq!(calls.get(), 2);
        let after =
            lookup_with(CheckMode::Daily, Some(&cache), URL, t0 + DAY + 60, &fetch).unwrap();
        assert_eq!(after.cached_from, Some(t0 + DAY));
        assert_eq!(calls.get(), 2);
    }

    #[test]
    fn a_cached_answer_of_no_release_is_also_used() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("update-check");
        let calls = std::cell::Cell::new(0);
        let fetch = counting_fetch(&calls, None);
        lookup_with(CheckMode::Daily, Some(&cache), URL, 100, &fetch).unwrap();
        let second = lookup_with(CheckMode::Daily, Some(&cache), URL, 200, &fetch).unwrap();
        assert_eq!(second.latest, None);
        assert_eq!(second.cached_from, Some(100));
        assert_eq!(calls.get(), 1);
    }

    #[test]
    fn the_always_mode_looks_every_time_and_stores_the_result() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("update-check");
        let calls = std::cell::Cell::new(0);
        let fetch = counting_fetch(&calls, Some(Version(1, 2, 3)));
        lookup_with(CheckMode::Always, Some(&cache), URL, 100, &fetch).unwrap();
        let second = lookup_with(CheckMode::Always, Some(&cache), URL, 101, &fetch).unwrap();
        assert_eq!(second.cached_from, None);
        assert_eq!(calls.get(), 2);
        assert_eq!(read_cache(&cache).unwrap().checked_at, 101);
    }

    #[test]
    fn the_never_mode_does_not_look_and_does_not_write() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("update-check");
        let calls = std::cell::Cell::new(0);
        let fetch = counting_fetch(&calls, Some(Version(1, 2, 3)));
        let lookup = lookup_with(CheckMode::Never, Some(&cache), URL, 100, &fetch).unwrap();
        assert_eq!(lookup.latest, None);
        assert_eq!(calls.get(), 0);
        assert!(!cache.exists());
    }

    #[test]
    fn a_failed_lookup_is_not_stored() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("update-check");
        let failing = || -> Result<Option<Version>, String> { Err("offline".to_string()) };
        assert!(lookup_with(CheckMode::Daily, Some(&cache), URL, 100, &failing).is_err());
        assert!(!cache.exists());
    }

    #[test]
    fn a_changed_repository_or_a_damaged_cache_looks_again() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("update-check");
        let calls = std::cell::Cell::new(0);
        let fetch = counting_fetch(&calls, None);
        lookup_with(CheckMode::Daily, Some(&cache), URL, 100, &fetch).unwrap();
        lookup_with(
            CheckMode::Daily,
            Some(&cache),
            "https://other.test/tsk",
            101,
            &fetch,
        )
        .unwrap();
        assert_eq!(calls.get(), 2);
        std::fs::write(&cache, "garbage").unwrap();
        lookup_with(CheckMode::Daily, Some(&cache), URL, 102, &fetch).unwrap();
        assert_eq!(calls.get(), 3);
    }

    #[test]
    fn a_missing_state_directory_means_no_cache_and_a_lookup_every_time() {
        let calls = std::cell::Cell::new(0);
        let fetch = counting_fetch(&calls, None);
        lookup_with(CheckMode::Daily, None, URL, 100, &fetch).unwrap();
        lookup_with(CheckMode::Daily, None, URL, 101, &fetch).unwrap();
        assert_eq!(calls.get(), 2);
    }

    #[test]
    fn a_cached_notice_says_so_and_a_fresh_one_does_not() {
        let current = Version(0, 16, 0);
        let fresh = Lookup {
            latest: Some(Version(0, 17, 0)),
            cached_from: None,
        };
        assert_eq!(
            notice_for(fresh, current, 20_000).unwrap(),
            "tsk 0.17.0 is available (running 0.16.0). Run `tsk self-update` to upgrade."
        );
        let cached = Lookup {
            latest: Some(Version(0, 17, 0)),
            cached_from: Some(20_000 - 3 * 3600 - 5),
        };
        assert_eq!(
            notice_for(cached, current, 20_000).unwrap(),
            "tsk 0.17.0 is available (running 0.16.0). Run `tsk self-update` to upgrade. The release check is cached: it last ran 3 hours ago, and runs once a day. Set TSK_UPDATE_CHECK=always to check now."
        );
        let current_only = Lookup {
            latest: Some(current),
            cached_from: Some(1),
        };
        assert_eq!(notice_for(current_only, current, 20_000), None);
        assert!(cached_sentence(100, 150).contains("less than an hour ago"));
        assert!(cached_sentence(0, 3600).contains("1 hour ago"));
    }

    #[test]
    fn the_notice_names_both_versions_and_the_command() {
        assert_eq!(
            available_sentence(Version(0, 7, 0), Version(0, 6, 0)),
            "tsk 0.7.0 is available (running 0.6.0). Run `tsk self-update` to upgrade."
        );
    }
}
