//! The ledger home: one directory per repo, outside the code repo (ADR 0035).
//!
//! ```text
//! <state>/ledgers/<key>/
//!   .tsk-home        marker: layout, the nexus the home belongs to, the clones moved in
//!   git/             tsk's bare clone: the ledger and documents objects and refs
//!   ledger-db/       the Dolt database
//!   ledger/          worktree of git/ that carries the Dolt file remote
//!   docs/            worktree of git/ that holds the documents
//!   scratch/ push.lock ledger-ref remote-tips dolt-server.* ...
//! ```
//!
//! The key is the nexus entry id for a ledger in a nexus, so every clone and linked code
//! worktree of one repo on a machine shares one home. A second nexus that uses the same
//! entry id gets `<id>~<hash8>`. A ledger that lives in its repo has no stable identity across
//! clones, so its home stays per clone: `_<clone-id>`. The code repo keeps `.git/tsk-clone-id`,
//! `.git/tsk-repo-id` and `.git/tsk-ledger-home`, the name of its home, and nothing else.
//!
//! The per-clone files that git configuration names by path (`harness-names`, the robot
//! signing key and signers file, the hooks) stay in `<state>/repos/<clone-id>/`.

use std::path::{Path, PathBuf};

use super::git::Git;
use super::location::{LedgerLocation, Repo};

pub const LEDGERS_DIR: &str = "ledgers";
pub const GIT_DIR_NAME: &str = "git";
pub const LEDGER_DIR_NAME: &str = "ledger";
pub const HOME_MARKER: &str = ".tsk-home";
/// The file in the code repo's git directory that names its ledger home.
pub const HOME_FILE: &str = "tsk-ledger-home";
/// The directory of the per-clone files, and of the state of tsk 0.69.0 and earlier.
pub const CLONES_DIR: &str = "repos";
/// The layout number the marker of a home records.
pub const HOME_LAYOUT: u32 = 1;

/// How a home is named under `ledgers/`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HomeKey {
    /// A ledger in a nexus: the nexus entry id.
    Nexus(String),
    /// The same entry id in a second nexus: the id and the first 8 hex characters of the
    /// SHA-256 of the nexus identity.
    NexusAlt(String, String),
    /// A ledger in its repo: the clone id.
    Clone(String),
}

impl HomeKey {
    pub fn dir_name(&self) -> String {
        match self {
            HomeKey::Nexus(id) => id.clone(),
            HomeKey::NexusAlt(id, hash) => format!("{}~{}", id, hash),
            HomeKey::Clone(clone) => format!("_{}", clone),
        }
    }

    pub fn from_dir_name(name: &str) -> Option<HomeKey> {
        if name.is_empty() {
            return None;
        }
        if let Some(clone) = name.strip_prefix('_') {
            return (!clone.is_empty()).then(|| HomeKey::Clone(clone.to_string()));
        }
        match name.split_once('~') {
            Some((id, hash)) if !id.is_empty() && !hash.is_empty() => {
                Some(HomeKey::NexusAlt(id.to_string(), hash.to_string()))
            }
            Some(_) => None,
            None => Some(HomeKey::Nexus(name.to_string())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Home {
    root: PathBuf,
    key: HomeKey,
}

/// What a path-only caller finds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Opened {
    /// The home of this clone. It may not exist on disk yet.
    Home(Home),
    /// A store of tsk 0.69.0 or earlier exists in `repos/<clone-id>`, and its key is not
    /// known yet: the move to a home waits for a caller that knows the ledger location.
    Pending(PathBuf),
    /// The clone has no clone id, so it has no ledger state.
    None,
}

impl Home {
    pub fn of_key(state_root: &Path, key: HomeKey) -> Home {
        Home {
            root: state_root.join(LEDGERS_DIR).join(key.dir_name()),
            key,
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn key(&self) -> &HomeKey {
        &self.key
    }

    pub fn db(&self) -> PathBuf {
        self.root.join(super::location::DOLT_DIR_NAME)
    }

    pub fn ledger_dir(&self) -> PathBuf {
        self.root.join(LEDGER_DIR_NAME)
    }

    pub fn docs(&self) -> PathBuf {
        self.root.join(super::location::DOCS_DIR_NAME)
    }

    pub fn git_dir(&self) -> PathBuf {
        self.root.join(GIT_DIR_NAME)
    }

    pub fn scratch(&self) -> PathBuf {
        self.root.join(super::location::SCRATCH_DIR_NAME)
    }

    pub fn marker(&self) -> PathBuf {
        self.root.join(HOME_MARKER)
    }

    /// The handle on the bare clone. Creates nothing.
    pub fn git(&self) -> Git {
        Git::new(self.git_dir())
    }

    /// The bare clone, created when absent, and for a ledger in its repo set up to reach
    /// the code repo's `origin` as the code repo does.
    pub fn ensure_git(&self, repo: &Repo, in_repo: bool) -> Result<Git, String> {
        let dir = self.git_dir();
        let git = Git::new(&dir);
        if !dir.join("HEAD").is_file() {
            std::fs::create_dir_all(&dir)
                .map_err(|e| format!("error: could not create {}: {}", dir.display(), e))?;
            git.run(["init", "--quiet", "--bare"])?;
            git.run(["config", "gc.autoDetach", "false"])?;
        }
        if in_repo {
            configure_origin(repo, &git)?;
        }
        Ok(git)
    }

    /// Repair the links between the bare clone and the worktrees when the state directory
    /// moved: a worktree whose `.git` file names a missing directory.
    pub fn heal(&self) -> Result<(), String> {
        if !self.git_dir().join("HEAD").is_file() {
            return Ok(());
        }
        let mut stale = Vec::new();
        for wt in [self.ledger_dir(), self.docs()] {
            let Ok(text) = std::fs::read_to_string(wt.join(".git")) else {
                continue;
            };
            let target = text.trim().strip_prefix("gitdir:").map(str::trim);
            if target.is_some_and(|target| !Path::new(target).exists()) {
                stale.push(wt);
            }
        }
        if !stale.is_empty() {
            let mut args = vec!["worktree".to_string(), "repair".to_string()];
            args.extend(stale.iter().map(|p| p.to_string_lossy().into_owned()));
            self.git().run(args)?;
        }
        // The store's `origin` names the `ledger` directory by path: it follows a moved state
        // directory too.
        super::migrate_home::rewrite_store_url(self)
    }

    /// True when the home holds a ledger store or a documents worktree.
    pub fn has_state(&self) -> bool {
        self.db().exists() || self.docs().exists()
    }
}

// --- The marker -----------------------------------------------------------------------

/// What `.tsk-home` records.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Marker {
    /// The identity of the nexus the home belongs to, or empty when not known yet.
    pub nexus: String,
    /// The clone ids whose legacy state was moved in.
    pub migrated: Vec<String>,
}

impl Marker {
    pub fn read(home: &Home) -> Option<Marker> {
        let text = std::fs::read_to_string(home.marker()).ok()?;
        let mut marker = Marker::default();
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let value = value.trim();
            match key.trim() {
                "nexus" => marker.nexus = value.to_string(),
                "migrated" => {
                    marker.migrated = value
                        .split(',')
                        .map(str::trim)
                        .filter(|id| !id.is_empty())
                        .map(str::to_string)
                        .collect()
                }
                _ => {}
            }
        }
        Some(marker)
    }

    pub fn write(&self, home: &Home) -> Result<(), String> {
        let text = format!(
            "layout = {}\nnexus = {}\nmigrated = {}\n",
            HOME_LAYOUT,
            self.nexus,
            self.migrated.join(",")
        );
        write_atomic(&home.marker(), &text)
    }
}

pub(crate) fn write_atomic(file: &Path, text: &str) -> Result<(), String> {
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("error: could not create {}: {}", parent.display(), e))?;
    }
    let temp = file.with_extension(format!("tmp{}", std::process::id()));
    std::fs::write(&temp, text)
        .and_then(|_| std::fs::rename(&temp, file))
        .map_err(|e| {
            let _ = std::fs::remove_file(&temp);
            format!("error: could not write {}: {}", file.display(), e)
        })
}

/// The identity of a nexus for the marker: the normalised URL, or for a local path its
/// canonical spelling.
pub fn nexus_identity(url: &str) -> String {
    if let Some(normal) = super::nexus::normalise_url(url) {
        return normal;
    }
    let trimmed = url.trim().trim_end_matches('/');
    let path = trimmed.strip_prefix("file://").unwrap_or(trimmed);
    std::fs::canonicalize(path)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| trimmed.to_string())
}

// --- Finding the home ----------------------------------------------------------------

/// `<state>/repos/<clone-id>`: the per-clone files, and the state of 0.69.0 and earlier.
pub fn clone_state_dir(state_root: &Path, clone_id: &str) -> PathBuf {
    state_root.join(CLONES_DIR).join(clone_id)
}

fn home_file(repo: &Repo) -> PathBuf {
    repo.common_dir.join(HOME_FILE)
}

fn read_home_file(repo: &Repo) -> Option<String> {
    let text = std::fs::read_to_string(home_file(repo)).ok()?;
    let name = text.trim();
    (!name.is_empty() && !name.contains('/')).then(|| name.to_string())
}

pub fn write_home_file(repo: &Repo, home: &Home) -> Result<(), String> {
    if read_home_file(repo).as_deref() == Some(home.key().dir_name().as_str()) {
        return Ok(());
    }
    write_atomic(&home_file(repo), &format!("{}\n", home.key().dir_name()))
}

/// The home of this clone without moving or creating anything: a file read, no process.
///
/// 1. `.git/tsk-ledger-home`, when present.
/// 2. Else, with a clone id: `ledgers/_<clone-id>` when it exists, else `Pending` when a
///    store of 0.69.0 or earlier exists, else the provisional `_<clone-id>` with nothing on
///    disk.
pub fn peek(repo: &Repo, state_root: &Path) -> Result<Opened, String> {
    if let Some(name) = read_home_file(repo) {
        if let Some(key) = HomeKey::from_dir_name(&name) {
            return Ok(Opened::Home(Home::of_key(state_root, key)));
        }
    }
    let Some(clone) = repo.read_clone_id()? else {
        return Ok(Opened::None);
    };
    let own = Home::of_key(state_root, HomeKey::Clone(clone.clone()));
    if own.root().exists() {
        return Ok(Opened::Home(own));
    }
    let legacy = clone_state_dir(state_root, &clone);
    if super::migrate_home::has_legacy_state(&legacy) {
        return Ok(Opened::Pending(legacy));
    }
    Ok(Opened::Home(own))
}

/// [`peek`], and the move of the state of 0.69.0 and earlier when it is there and its key is
/// known (ADR 0035). No network.
pub fn open(repo: &Repo, state_root: &Path) -> Result<Opened, String> {
    let peeked = peek(repo, state_root)?;
    let Some(clone) = repo.read_clone_id()? else {
        return Ok(peeked);
    };
    let legacy_dir = clone_state_dir(state_root, &clone);
    if !super::migrate_home::has_legacy_state(&legacy_dir) {
        return Ok(peeked);
    }
    let named = match &peeked {
        Opened::Home(home) if read_home_file(repo).is_some() => Some(home.clone()),
        _ => None,
    };
    let key = match &named {
        Some(home) => Some(home.key().clone()),
        None => super::migrate_home::legacy_key(&legacy_dir, &clone),
    };
    let Some(key) = key else {
        return Ok(Opened::Pending(legacy_dir));
    };
    let home = Home::of_key(state_root, key);
    super::migrate_home::run(repo, state_root, &home, &clone)?;
    write_home_file(repo, &home)?;
    Ok(Opened::Home(home))
}

/// The home for `location`, created when absent. Moves the state of 0.69.0 and earlier in,
/// makes the bare clone, repairs worktree links, and records the home in the code repo.
pub fn for_location(
    repo: &Repo,
    state_root: &Path,
    location: &LedgerLocation,
) -> Result<Home, String> {
    let clone = repo.ensure_clone_id()?;
    let existing = open(repo, state_root)?;
    let wanted = key_for(state_root, location, &clone);
    let home = Home::of_key(state_root, wanted.clone());
    match &existing {
        Opened::Home(held) if held.key() != &wanted => {
            if held.db().exists() {
                guard_key_flip(held, &home)?;
            }
        }
        Opened::Pending(_) => {
            super::migrate_home::run(repo, state_root, &home, &clone)?;
        }
        _ => {}
    }
    let in_repo = location.repo_id().is_none();
    std::fs::create_dir_all(home.root())
        .map_err(|e| format!("error: could not create {}: {}", home.root().display(), e))?;
    home.ensure_git(repo, in_repo)?;
    home.heal()?;
    let identity = match location.repo_id() {
        Some(_) => nexus_identity(location.remote()),
        None => String::new(),
    };
    let held = Marker::read(&home);
    let mut marker = held.clone().unwrap_or_default();
    if marker.nexus.is_empty() {
        marker.nexus = identity;
    }
    if held.as_ref() != Some(&marker) {
        marker.write(&home)?;
    }
    write_home_file(repo, &home)?;
    Ok(home)
}

/// The key of the home of `location`.
fn key_for(state_root: &Path, location: &LedgerLocation, clone_id: &str) -> HomeKey {
    let Some(id) = location.repo_id() else {
        return HomeKey::Clone(clone_id.to_string());
    };
    let identity = nexus_identity(location.remote());
    let base = Home::of_key(state_root, HomeKey::Nexus(id.to_string()));
    match Marker::read(&base) {
        Some(marker) if !marker.nexus.is_empty() && marker.nexus != identity => {
            HomeKey::NexusAlt(id.to_string(), hash8(&identity))
        }
        _ => HomeKey::Nexus(id.to_string()),
    }
}

/// The ledger of this clone now resolves to another home than the one it used. Unpushed
/// work in the old home stops the command; a clean, pushed home is left and named in a note.
fn guard_key_flip(old: &Home, new: &Home) -> Result<(), String> {
    if let Some(work) = unpushed_work(old)? {
        return Err(format!(
            "error: this clone has unpushed ledger work in {} ({}), and the ledger now resolves to {}.\n       \
             Run tsk nexus move-ledger, or tsk ledger discard there, then run the command again.",
            old.root().display(),
            work,
            new.root().display()
        ));
    }
    eprintln!(
        "note: the ledger of this clone now resolves to {}; the pushed ledger in {} is left as it is",
        new.root().display(),
        old.root().display()
    );
    Ok(())
}

/// What a home holds that no remote has: uncommitted or unpushed Dolt changes, uncommitted
/// or unpushed documents. `None` when there is nothing, or no store.
pub fn unpushed_work(home: &Home) -> Result<Option<String>, String> {
    use super::dolt::Dolt;
    let db = home.db();
    if db.join(".dolt").is_dir() {
        let dolt = Dolt::store(&db);
        // A store with no row and nothing but its creation commits holds nothing to lose.
        let pending = super::store::dolt::pending_commits_of(&dolt);
        let replaceable = match &pending {
            Ok(commits) => super::store::dolt::is_creation_only_commits(commits),
            Err(_) => true,
        } && super::store::dolt::holds_no_rows_in(&dolt)?;
        if !replaceable {
            if dolt.count("select count(*) as c from dolt_status")? > 0 {
                return Ok(Some("uncommitted ledger changes".to_string()));
            }
            match pending {
                Ok(commits) if !commits.is_empty() => {
                    return Ok(Some(format!(
                        "{} ledger commit{} not pushed",
                        commits.len(),
                        if commits.len() == 1 { "" } else { "s" }
                    )));
                }
                Err(_) => return Ok(Some("a ledger that was never pushed".to_string())),
                Ok(_) => {}
            }
        }
    }
    let docs = home.docs();
    if docs.join(".git").exists() {
        let wt = Git::new(&docs);
        if !wt.run(["status", "--porcelain"])?.trim().is_empty() {
            return Ok(Some("uncommitted documents".to_string()));
        }
        let fetched = "refs/tsk-fetch/docs";
        let known = home
            .git()
            .output(["rev-parse", "--verify", "--quiet", fetched])
            .is_ok_and(|(_, out)| out.status.success());
        if known {
            let ahead = wt.run_line(["rev-list", "--count", "HEAD", "--not", fetched])?;
            if ahead.trim() != "0" {
                return Ok(Some("documents commits not pushed".to_string()));
            }
        }
    }
    Ok(None)
}

// --- The bare clone of a ledger in its repo ------------------------------------------

/// The settings of the code repo that a transport needs, copied to the bare clone: a bare
/// clone has no `origin` of its own, and the code repo's git configuration (credentials, ssh
/// command, proxy) no longer applies to it. `url.*` is not copied: the resolved URL already
/// includes the rewrite.
const COPIED_PATTERN: &str = "^(core\\.sshcommand|credential\\..*|http\\..*|protocol\\..*)$";
const COPIED_LIST_KEY: &str = "tsk.copiedkeys";

fn configure_origin(repo: &Repo, bare: &Git) -> Result<(), String> {
    static DONE: std::sync::Mutex<Vec<PathBuf>> = std::sync::Mutex::new(Vec::new());
    if DONE
        .lock()
        .is_ok_and(|done| done.iter().any(|p| p == bare.dir()))
    {
        return Ok(());
    }
    // The settings are read again only when a file that can change them did: the stamp names
    // the git configuration files of the code repo and the user, by size and time.
    let stamp = config_stamp(repo);
    let stamp_file = bare.dir().join(STAMP_FILE);
    let current = std::fs::read_to_string(&stamp_file).unwrap_or_default();
    if current != stamp || !bare.dir().join("config").is_file() {
        copy_origin(repo, bare)?;
        let _ = std::fs::write(&stamp_file, &stamp);
    }
    if let Ok(mut done) = DONE.lock() {
        done.push(bare.dir().to_path_buf());
    }
    Ok(())
}

/// The file in the bare clone that records the configuration the settings were copied from.
const STAMP_FILE: &str = "tsk-config-stamp";

/// A line for each git configuration file that exists and can set the remote or its transport
/// for the code repo: its path, size and modification time.
fn config_stamp(repo: &Repo) -> String {
    let mut files = vec![
        repo.common_dir.join("config"),
        repo.git_dir.join("config.worktree"),
        PathBuf::from("/etc/gitconfig"),
    ];
    if let Some(home) = std::env::var_os("HOME").filter(|v| !v.is_empty()) {
        files.push(PathBuf::from(&home).join(".gitconfig"));
    }
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty()) {
        files.push(PathBuf::from(xdg).join("git").join("config"));
    } else if let Some(home) = std::env::var_os("HOME").filter(|v| !v.is_empty()) {
        files.push(
            PathBuf::from(home)
                .join(".config")
                .join("git")
                .join("config"),
        );
    }
    if let Some(global) = std::env::var_os("GIT_CONFIG_GLOBAL") {
        files.push(PathBuf::from(global));
    }
    let mut text = String::new();
    for file in files {
        if let Ok(meta) = std::fs::metadata(&file) {
            let modified = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            text.push_str(&format!(
                "{}\t{}\t{}\n",
                file.display(),
                meta.len(),
                modified
            ));
        }
    }
    text
}

fn copy_origin(repo: &Repo, bare: &Git) -> Result<(), String> {
    let top = repo
        .git
        .run_line(["rev-parse", "--show-toplevel"])
        .map(PathBuf::from)
        .unwrap_or_else(|_| repo.common_dir.clone());
    // `remote get-url` expands `url.*.insteadOf` and, with `--push`, `pushInsteadOf`, and
    // starts no connection.
    let fetch_url = repo
        .git
        .run_line(["remote", "get-url", super::location::IN_REPO_REMOTE])
        .map(|url| absolute_if_local(&url, &top))
        .map_err(|e| format!("{}\n       The ledger of this repo lives on origin", e))?;
    let push_url = repo
        .git
        .run_line([
            "remote",
            "get-url",
            "--push",
            super::location::IN_REPO_REMOTE,
        ])
        .map(|url| absolute_if_local(&url, &top))
        .unwrap_or_else(|_| fetch_url.clone());
    set_config(bare, "remote.origin.url", Some(&fetch_url))?;
    set_config(
        bare,
        "remote.origin.pushurl",
        (push_url != fetch_url).then_some(push_url.as_str()),
    )?;
    set_config(
        bare,
        "remote.origin.fetch",
        Some("+refs/heads/*:refs/remotes/origin/*"),
    )?;

    // The effective values of the transport settings, read where includeIf blocks are
    // evaluated: in the code repo. A value set for one command (`-c`, the environment) is
    // not a setting of the repo.
    let mut copied: Vec<String> = Vec::new();
    if let Ok(listing) = repo.git.run([
        "config",
        "--show-scope",
        "--get-regexp",
        "-z",
        "--includes",
        COPIED_PATTERN,
    ]) {
        // With `-z` and `--show-scope`: the scope, NUL, the key, a newline, the value, NUL.
        let mut tokens = listing.split('\0');
        while let (Some(scope), Some(entry)) = (tokens.next(), tokens.next()) {
            let Some((key, value)) = entry.split_once('\n') else {
                continue;
            };
            if scope == "command" {
                continue;
            }
            if !copied.iter().any(|k| k == key) {
                bare.output(["config", "--unset-all", key]).ok();
                copied.push(key.to_string());
            }
            bare.run(["config", "--add", key, value])?;
        }
    }
    let previous = bare
        .output(["config", "--get-all", COPIED_LIST_KEY])
        .map(|(_, out)| String::from_utf8_lossy(&out.stdout).into_owned())
        .unwrap_or_default();
    for old in previous.lines().filter(|k| !k.is_empty()) {
        if !copied.iter().any(|k| k == old) {
            bare.output(["config", "--unset-all", old]).ok();
        }
    }
    bare.output(["config", "--unset-all", COPIED_LIST_KEY]).ok();
    for key in &copied {
        bare.run(["config", "--add", COPIED_LIST_KEY, key.as_str()])?;
    }
    Ok(())
}

fn set_config(git: &Git, key: &str, value: Option<&str>) -> Result<(), String> {
    match value {
        Some(value) => {
            let current = git
                .output(["config", "--get", key])
                .ok()
                .filter(|(_, out)| out.status.success())
                .map(|(_, out)| String::from_utf8_lossy(&out.stdout).trim().to_string());
            if current.as_deref() != Some(value) {
                git.run(["config", key, value])?;
            }
        }
        None => {
            git.output(["config", "--unset-all", key]).ok();
        }
    }
    Ok(())
}

/// A relative filesystem path made absolute against the top of the work tree. A URL, and the
/// scp-like `host:path` form, stay as they are.
fn absolute_if_local(url: &str, top: &Path) -> String {
    let url = url.trim();
    if url.contains("://") {
        return match url.strip_prefix("file://") {
            Some(path) => absolute_path(path, top),
            None => url.to_string(),
        };
    }
    if let Some((host, _)) = url.split_once(':') {
        if !host.is_empty() && !host.contains('/') && !host.starts_with('.') {
            return url.to_string();
        }
    }
    absolute_path(url, top)
}

fn absolute_path(path: &str, top: &Path) -> String {
    let joined = if Path::new(path).is_absolute() {
        PathBuf::from(path)
    } else {
        top.join(path)
    };
    std::fs::canonicalize(&joined)
        .unwrap_or(joined)
        .to_string_lossy()
        .into_owned()
}

// --- SHA-256, for the suffix of a second nexus ---------------------------------------

/// The first 8 hex characters of the SHA-256 of `text`.
pub fn hash8(text: &str) -> String {
    sha256_hex(text.as_bytes())[..8].to_string()
}

fn sha256_hex(data: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut message = data.to_vec();
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&((data.len() as u64) * 8).to_be_bytes());
    for chunk in message.chunks(64) {
        let mut w = [0u32; 64];
        for (i, word) in chunk.chunks(4).enumerate() {
            w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let mut v = h;
        for i in 0..64 {
            let s1 = v[4].rotate_right(6) ^ v[4].rotate_right(11) ^ v[4].rotate_right(25);
            let ch = (v[4] & v[5]) ^ (!v[4] & v[6]);
            let t1 = v[7]
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = v[0].rotate_right(2) ^ v[0].rotate_right(13) ^ v[0].rotate_right(22);
            let maj = (v[0] & v[1]) ^ (v[0] & v[2]) ^ (v[1] & v[2]);
            let t2 = s0.wrapping_add(maj);
            v = [
                t1.wrapping_add(t2),
                v[0],
                v[1],
                v[2],
                v[3].wrapping_add(t1),
                v[4],
                v[5],
                v[6],
            ];
        }
        for (slot, add) in h.iter_mut().zip(v) {
            *slot = slot.wrapping_add(add);
        }
    }
    h.iter().map(|word| format!("{:08x}", word)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_matches_the_known_digests() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(hash8("abc"), "ba7816bf");
    }

    #[test]
    fn a_key_names_its_directory_and_is_read_back() {
        for (key, name) in [
            (HomeKey::Nexus("work-api".to_string()), "work-api"),
            (
                HomeKey::NexusAlt("work-api".to_string(), "1a2b3c4d".to_string()),
                "work-api~1a2b3c4d",
            ),
            (HomeKey::Clone("tsk-180ae86e".to_string()), "_tsk-180ae86e"),
        ] {
            assert_eq!(key.dir_name(), name);
            assert_eq!(HomeKey::from_dir_name(name), Some(key));
        }
        assert_eq!(HomeKey::from_dir_name(""), None);
        assert_eq!(HomeKey::from_dir_name("_"), None);
        assert_eq!(HomeKey::from_dir_name("x~"), None);
    }

    #[test]
    fn a_nexus_id_and_a_clone_id_never_name_the_same_directory() {
        // A repo id starts with a lowercase letter or a digit; a clone key starts with `_`.
        assert!(!super::super::nexus::valid_repo_id("_tsk-1234"));
        assert!(!super::super::nexus::valid_repo_id("a~b"));
    }

    #[test]
    fn the_paths_of_a_home_sit_under_its_root() {
        let home = Home::of_key(Path::new("/s/tsk"), HomeKey::Nexus("work-api".to_string()));
        assert_eq!(home.root(), Path::new("/s/tsk/ledgers/work-api"));
        assert_eq!(home.db(), Path::new("/s/tsk/ledgers/work-api/ledger-db"));
        assert_eq!(
            home.ledger_dir(),
            Path::new("/s/tsk/ledgers/work-api/ledger")
        );
        assert_eq!(home.docs(), Path::new("/s/tsk/ledgers/work-api/docs"));
        assert_eq!(home.git_dir(), Path::new("/s/tsk/ledgers/work-api/git"));
        assert_eq!(home.scratch(), Path::new("/s/tsk/ledgers/work-api/scratch"));
    }

    #[test]
    fn the_marker_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let home = Home::of_key(dir.path(), HomeKey::Nexus("x".to_string()));
        assert_eq!(Marker::read(&home), None);
        let marker = Marker {
            nexus: "github.com/acme/nexus".to_string(),
            migrated: vec!["a-1".to_string(), "b-2".to_string()],
        };
        marker.write(&home).unwrap();
        assert_eq!(Marker::read(&home), Some(marker));
    }

    #[test]
    fn a_local_path_nexus_has_its_canonical_path_as_identity() {
        let dir = tempfile::tempdir().unwrap();
        let real = std::fs::canonicalize(dir.path()).unwrap();
        assert_eq!(
            nexus_identity(&format!("{}/", dir.path().display())),
            real.to_string_lossy()
        );
        assert_eq!(
            nexus_identity("git@github.com:Acme/Nexus.git"),
            "github.com/acme/nexus"
        );
    }

    #[test]
    fn relative_and_file_urls_are_made_absolute_and_remote_ones_are_kept() {
        let dir = tempfile::tempdir().unwrap();
        let real = std::fs::canonicalize(dir.path()).unwrap();
        std::fs::create_dir(real.join("origin.git")).unwrap();
        assert_eq!(
            absolute_if_local("origin.git", &real),
            real.join("origin.git").to_string_lossy()
        );
        assert_eq!(
            absolute_if_local(&format!("file://{}/origin.git", real.display()), &real),
            real.join("origin.git").to_string_lossy()
        );
        assert_eq!(
            absolute_if_local("https://example.test/a/b", &real),
            "https://example.test/a/b"
        );
        assert_eq!(
            absolute_if_local("git@example.test:a/b.git", &real),
            "git@example.test:a/b.git"
        );
    }
}
