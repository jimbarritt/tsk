use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use clap::Args;

pub const REPO_URL_VAR: &str = "TSK_RELEASE_REPO_URL";
pub const DOWNLOAD_URL_VAR: &str = "TSK_RELEASE_DOWNLOAD_URL";
pub const NO_CHECK_VAR: &str = "TSK_NO_UPDATE_CHECK";
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

pub fn check_disabled() -> bool {
    std::env::var(NO_CHECK_VAR).is_ok_and(|v| !v.is_empty() && v != "0")
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
    fn label(self) -> &'static str {
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

/// Installs `version` by the method the running binary was installed with. Returns
/// the method used.
fn install(version: Version) -> Result<Method, String> {
    let exe = current_exe()?;
    let cargo_home = std::env::var_os("CARGO_HOME").map(PathBuf::from);
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let method = detect_method(&exe, cargo_home.as_deref(), home.as_deref());
    eprintln!("installed as: {}", method.label());
    match method {
        Method::Homebrew => run_streaming("brew", &["upgrade", BREW_FORMULA])?,
        Method::Cargo => run_streaming(
            "cargo",
            &[
                "install",
                CRATE_NAME,
                "--locked",
                "--version",
                &version.to_string(),
            ],
        )?,
        Method::ReleaseBinary => install_release_binary(&exe, version)?,
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
    let latest = latest_release(NOTICE_TIMEOUT).ok().flatten()?;
    (latest > current).then(|| available_sentence(latest, current))
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
    let latest = match latest_release(CLOUD_LOOKUP_TIMEOUT) {
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

    #[test]
    fn the_notice_names_both_versions_and_the_command() {
        assert_eq!(
            available_sentence(Version(0, 7, 0), Version(0, 6, 0)),
            "tsk 0.7.0 is available (running 0.6.0). Run `tsk self-update` to upgrade."
        );
    }
}
