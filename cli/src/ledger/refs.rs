//! Which refs hold a repo's ledger: branches under `refs/heads/*`, or a custom ref outside
//! them. A custom ref is the default for a new ledger. GitHub lists a branch and offers a
//! pull request for it, and does neither for `refs/tsk/*`. The Claude Code cloud proxy
//! refuses to push or update a ref outside `refs/heads/*`, so a repo used from a cloud
//! session keeps branches (ADR 0016).
//!
//! The mode is read in this order, and the first answer wins:
//!
//! 1. `TSK_LEDGER_REFS`, the process override.
//! 2. `ledger_refs` on the repo's entry in the nexus.
//! 3. `[ledger] refs` in `.tsk/config.toml` at the root of the code repo.
//! 4. Discovery: the ref of an existing ledger store, else the answer cached from an earlier
//!    discovery, else one `git ls-remote` for both refs. A ledger that does not exist
//!    anywhere is new: `custom`, except in a Claude Code cloud session, where `branches`.
//!
//! tsk never moves an existing ledger from one mode to the other. A mode that disagrees
//! with the ledger store or with origin stops the command with an error that names both
//! refs.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use super::location::{dolt_db_path, LedgerLocation, Repo};

/// The environment variable that sets the mode for one process. It beats every other source.
pub const ENV_VAR: &str = "TSK_LEDGER_REFS";
/// The directory of the repo config, at the root of the code repo.
pub const REPO_CONFIG_DIR: &str = ".tsk";
pub const REPO_CONFIG_FILE: &str = "config.toml";
/// The file in the clone's state directory that caches what discovery found.
pub const CACHE_FILE: &str = "ledger-refs";

pub const BRANCH_LEDGER_REF: &str = "refs/heads/tsk/ledger";
pub const BRANCH_DOCS_REF: &str = "refs/heads/tsk/docs";
pub const BRANCH_NEXUS_PREFIX: &str = "refs/heads/ledgers/";
pub const CUSTOM_LEDGER_REF: &str = "refs/tsk/ledger";
pub const CUSTOM_DOCS_REF: &str = "refs/tsk/docs";
pub const CUSTOM_NEXUS_PREFIX: &str = "refs/tsk/ledgers/";

/// What a session in the Claude Code cloud is told when a new ledger takes branches by
/// default.
pub const CLOUD_DEFAULT_NOTE: &str = "This is a Claude Code cloud session, which cannot push a custom ref, so the ledger uses branches. To choose, run tsk config ledger-refs custom or branches before the first push.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LedgerRefs {
    Branches,
    Custom,
}

impl LedgerRefs {
    pub fn parse(text: &str) -> Option<LedgerRefs> {
        match text.trim() {
            "branches" => Some(LedgerRefs::Branches),
            "custom" => Some(LedgerRefs::Custom),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            LedgerRefs::Branches => "branches",
            LedgerRefs::Custom => "custom",
        }
    }

    pub fn other(self) -> LedgerRefs {
        match self {
            LedgerRefs::Branches => LedgerRefs::Custom,
            LedgerRefs::Custom => LedgerRefs::Branches,
        }
    }

    /// The data ref of a repo's own ledger.
    pub fn ledger_ref(self) -> &'static str {
        match self {
            LedgerRefs::Branches => BRANCH_LEDGER_REF,
            LedgerRefs::Custom => CUSTOM_LEDGER_REF,
        }
    }

    /// The documents ref of a repo's own ledger.
    pub fn docs_ref(self) -> &'static str {
        match self {
            LedgerRefs::Branches => BRANCH_DOCS_REF,
            LedgerRefs::Custom => CUSTOM_DOCS_REF,
        }
    }

    pub fn nexus_prefix(self) -> &'static str {
        match self {
            LedgerRefs::Branches => BRANCH_NEXUS_PREFIX,
            LedgerRefs::Custom => CUSTOM_NEXUS_PREFIX,
        }
    }

    /// The mode a data ref belongs to, from its prefix.
    pub fn of_ref(ref_name: &str) -> Option<LedgerRefs> {
        if ref_name.starts_with("refs/heads/") {
            Some(LedgerRefs::Branches)
        } else if ref_name.starts_with("refs/tsk/") {
            Some(LedgerRefs::Custom)
        } else {
            None
        }
    }
}

/// Where the mode came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefsSource {
    Env,
    Nexus,
    RepoConfig,
    /// The ref of the ledger store already on this machine.
    LedgerStore,
    /// Found on origin by `git ls-remote`, or cached from an earlier discovery.
    Discovered,
    /// A new ledger outside a cloud session.
    Default,
    /// A new ledger in a Claude Code cloud session.
    CloudDefault,
}

impl RefsSource {
    pub fn label(self) -> &'static str {
        match self {
            RefsSource::Env => "environment",
            RefsSource::Nexus => "nexus",
            RefsSource::RepoConfig => "repo config",
            RefsSource::LedgerStore => "ledger store",
            RefsSource::Discovered => "discovered on origin",
            RefsSource::Default => "default",
            RefsSource::CloudDefault => "default (Claude Code cloud session)",
        }
    }

    /// True when a person or a file chose the mode, as against a mode found or defaulted.
    pub fn is_configured(self) -> bool {
        matches!(
            self,
            RefsSource::Env | RefsSource::Nexus | RefsSource::RepoConfig
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefsChoice {
    pub refs: LedgerRefs,
    pub source: RefsSource,
}

/// The `ledger_refs` setting on a nexus entry: the entry id and the raw value.
pub type EntrySetting = (String, String);

// --- The repo config file -------------------------------------------------------------

/// `.tsk/config.toml` at the top of the code worktree, or `None` outside one (a bare
/// repository has none).
pub fn repo_config_path(repo: &Repo) -> Option<PathBuf> {
    let top = work_tree_top(repo.git.dir())?;
    Some(top.join(REPO_CONFIG_DIR).join(REPO_CONFIG_FILE))
}

/// The top of the working tree that holds `dir`: the nearest directory at or above it with
/// a `.git` entry (a directory, or the file of a linked worktree). A file walk, so it
/// starts no process.
fn work_tree_top(dir: &Path) -> Option<PathBuf> {
    let dir = std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    dir.ancestors()
        .find(|candidate| candidate.join(".git").exists())
        .map(Path::to_path_buf)
}

fn read_table(file: &Path) -> Result<toml::Table, String> {
    match std::fs::read_to_string(file) {
        Ok(text) => text.parse::<toml::Table>().map_err(|e| {
            format!(
                "error: {} is not valid TOML: {}",
                file.display(),
                e.message()
            )
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(toml::Table::new()),
        Err(e) => Err(format!("error: could not read {}: {}", file.display(), e)),
    }
}

fn invalid_value(what: &str, value: &str) -> String {
    format!(
        "error: {} is \"{}\"; the values are \"branches\" and \"custom\"",
        what, value
    )
}

/// The `[ledger] refs` value of a repo config file. `None` when the file, the table or the
/// key is absent.
pub fn read_repo_refs(file: &Path) -> Result<Option<LedgerRefs>, String> {
    let table = read_table(file)?;
    let Some(ledger) = table.get("ledger") else {
        return Ok(None);
    };
    let Some(ledger) = ledger.as_table() else {
        return Err(format!(
            "error: ledger in {} must be a table",
            file.display()
        ));
    };
    match ledger.get("refs") {
        None => Ok(None),
        Some(toml::Value::String(value)) => LedgerRefs::parse(value).map(Some).ok_or_else(|| {
            invalid_value(&format!("ledger.refs in {}", file.display()), value.trim())
        }),
        Some(_) => Err(format!(
            "error: ledger.refs in {} must be a string",
            file.display()
        )),
    }
}

/// Write `[ledger] refs` and keep every other key. Creates the directory.
pub fn write_repo_refs(file: &Path, refs: LedgerRefs) -> Result<(), String> {
    let mut table = read_table(file)?;
    let mut ledger = match table.remove("ledger") {
        Some(toml::Value::Table(existing)) => existing,
        Some(_) => {
            return Err(format!(
                "error: ledger in {} must be a table",
                file.display()
            ))
        }
        None => toml::Table::new(),
    };
    ledger.insert(
        "refs".to_string(),
        toml::Value::String(refs.as_str().to_string()),
    );
    table.insert("ledger".to_string(), toml::Value::Table(ledger));
    let text = toml::to_string(&table)
        .map_err(|e| format!("error: could not serialise the repo config: {}", e))?;
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("error: could not create {}: {}", parent.display(), e))?;
    }
    std::fs::write(file, text)
        .map_err(|e| format!("error: could not write {}: {}", file.display(), e))
}

// --- Reading the configured mode ------------------------------------------------------

/// The mode a person or a file chose, in order: the environment, the nexus entry, the repo
/// config. `None` when none of them says.
pub fn configured(repo: &Repo, entry: Option<&EntrySetting>) -> Result<Option<RefsChoice>, String> {
    if let Some(value) = std::env::var(ENV_VAR).ok().filter(|v| !v.trim().is_empty()) {
        let refs = LedgerRefs::parse(&value)
            .ok_or_else(|| invalid_value(&format!("{} in the environment", ENV_VAR), &value))?;
        return Ok(Some(RefsChoice {
            refs,
            source: RefsSource::Env,
        }));
    }
    if let Some((id, value)) = entry {
        let refs = LedgerRefs::parse(value).ok_or_else(|| {
            invalid_value(&format!("ledger_refs on the nexus entry \"{}\"", id), value)
        })?;
        return Ok(Some(RefsChoice {
            refs,
            source: RefsSource::Nexus,
        }));
    }
    if let Some(file) = repo_config_path(repo) {
        if let Some(refs) = read_repo_refs(&file)? {
            return Ok(Some(RefsChoice {
                refs,
                source: RefsSource::RepoConfig,
            }));
        }
    }
    Ok(None)
}

// --- Discovery ------------------------------------------------------------------------

/// The mode of the ledger store on this machine, from the ref its `origin` remote records.
/// A file read, no process.
pub fn store_ref(repo: &Repo, state_root: &Path) -> Option<String> {
    let id = repo.read_clone_id().ok().flatten()?;
    let state = dolt_db_path(state_root, &id)
        .join(".dolt")
        .join("repo_state.json");
    let state: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(state).ok()?).ok()?;
    state
        .get("remotes")?
        .get(super::dolt::REMOTE)?
        .get("params")?
        .get("git_ref")?
        .as_str()
        .map(str::to_string)
}

fn clone_dir(repo: &Repo, state_root: &Path) -> Option<PathBuf> {
    let id = repo.read_clone_id().ok().flatten()?;
    dolt_db_path(state_root, &id)
        .parent()
        .map(Path::to_path_buf)
}

fn cache_key(base: &LedgerLocation) -> String {
    format!("{}\t{}", base.remote(), base.repo_id().unwrap_or("-"))
}

fn read_cache(repo: &Repo, state_root: &Path, base: &LedgerLocation) -> Option<LedgerRefs> {
    let file = clone_dir(repo, state_root)?.join(CACHE_FILE);
    let key = cache_key(base);
    std::fs::read_to_string(file)
        .ok()?
        .lines()
        .find_map(|line| {
            let (found, mode) = line.rsplit_once('\t')?;
            (found == key).then(|| LedgerRefs::parse(mode))?
        })
}

fn write_cache(repo: &Repo, state_root: &Path, base: &LedgerLocation, refs: LedgerRefs) {
    let Ok(id) = repo.ensure_clone_id() else {
        return;
    };
    let Some(dir) = dolt_db_path(state_root, &id)
        .parent()
        .map(Path::to_path_buf)
    else {
        return;
    };
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(
        dir.join(CACHE_FILE),
        format!("{}\t{}\n", cache_key(base), refs.as_str()),
    );
}

/// Forget what discovery cached. A failure to remove is ignored: a stale answer is
/// corrected by the check that follows every fetch.
pub fn forget_cache(repo: &Repo, state_root: &Path) {
    if let Some(dir) = clone_dir(repo, state_root) {
        let _ = std::fs::remove_file(dir.join(CACHE_FILE));
    }
}

/// The refs `base` would use in `refs` mode: the ledger ref and the documents ref.
fn refs_of(base: &LedgerLocation, refs: LedgerRefs) -> (String, String) {
    let located = base.with_refs(refs, RefsSource::Discovered);
    (located.ref_name().to_string(), located.docs_ref_name())
}

/// What `git ls-remote` listed for the exact ref names.
pub fn listed(listing: &str, names: &[&str]) -> Vec<String> {
    listing
        .lines()
        .filter_map(|line| line.split_whitespace().nth(1))
        .filter(|name| names.contains(name))
        .map(str::to_string)
        .collect()
}

/// One `git ls-remote` for the ledger ref in both modes. `None` when origin cannot be
/// asked or holds neither ref. Branches win when both exist.
fn ask_origin(repo: &Repo, base: &LedgerLocation) -> Option<LedgerRefs> {
    let (branch, _) = refs_of(base, LedgerRefs::Branches);
    let (custom, _) = refs_of(base, LedgerRefs::Custom);
    let (_, output) = repo
        .git
        .output(["ls-remote", base.remote(), branch.as_str(), custom.as_str()])
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let found = listed(
        &String::from_utf8_lossy(&output.stdout),
        &[branch.as_str(), custom.as_str()],
    );
    if found.contains(&branch) {
        Some(LedgerRefs::Branches)
    } else if found.contains(&custom) {
        Some(LedgerRefs::Custom)
    } else {
        None
    }
}

static CLOUD_DEFAULT_USED: AtomicBool = AtomicBool::new(false);

/// True when this process chose branches for a new ledger because it runs in a Claude Code
/// cloud session.
pub fn cloud_default_used() -> bool {
    CLOUD_DEFAULT_USED.load(Ordering::SeqCst)
}

/// The sentence for a command that created a ledger, or `None` when the mode was not a
/// cloud default.
pub fn cloud_default_note() -> Option<&'static str> {
    cloud_default_used().then_some(CLOUD_DEFAULT_NOTE)
}

fn in_cloud_session() -> bool {
    crate::thread::session_start::is_remote()
}

/// Discovery, for a ledger that no person or file configured.
pub fn discover(repo: &Repo, state_root: &Path, base: &LedgerLocation) -> RefsChoice {
    if let Some(refs) = store_ref(repo, state_root)
        .as_deref()
        .and_then(LedgerRefs::of_ref)
    {
        return RefsChoice {
            refs,
            source: RefsSource::LedgerStore,
        };
    }
    if let Some(refs) = read_cache(repo, state_root, base) {
        return RefsChoice {
            refs,
            source: RefsSource::Discovered,
        };
    }
    if let Some(refs) = ask_origin(repo, base) {
        write_cache(repo, state_root, base, refs);
        return RefsChoice {
            refs,
            source: RefsSource::Discovered,
        };
    }
    if in_cloud_session() {
        CLOUD_DEFAULT_USED.store(true, Ordering::SeqCst);
        return RefsChoice {
            refs: LedgerRefs::Branches,
            source: RefsSource::CloudDefault,
        };
    }
    RefsChoice {
        refs: LedgerRefs::Custom,
        source: RefsSource::Default,
    }
}

// --- Checks ---------------------------------------------------------------------------

/// Where to change the setting, by where it came from.
fn how_to_set(source: RefsSource, refs: LedgerRefs) -> String {
    match source {
        RefsSource::Env => format!("set {} to {}, or unset it", ENV_VAR, refs.as_str()),
        RefsSource::Nexus => format!(
            "set \"ledger_refs\": \"{}\" on the repo's entry in nexus.json",
            refs.as_str()
        ),
        _ => format!("run: tsk config ledger-refs {}", refs.as_str()),
    }
}

/// The error for a mode that disagrees with a ledger that already exists.
fn mismatch(
    location: &LedgerLocation,
    held_where: &str,
    held: LedgerRefs,
    held_ref: &str,
) -> String {
    let wanted = location.refs();
    format!(
        "error: this repo is set to {} ledger refs ({}), which use {}, but {} holds the ledger on {}.\n       \
         tsk does not move a ledger between branches and a custom ref, and it does not start a second ledger.\n       \
         To follow {}, {}",
        wanted.as_str(),
        location.source().label(),
        location.ref_name(),
        held_where,
        held_ref,
        held_where,
        how_to_set(location.source(), held)
    )
}

/// A mode that disagrees with the ledger store on this machine stops the command.
pub fn check_store(
    repo: &Repo,
    state_root: &Path,
    location: &LedgerLocation,
) -> Result<(), String> {
    if !location.source().is_configured() {
        return Ok(());
    }
    let Some(held_ref) = store_ref(repo, state_root) else {
        return Ok(());
    };
    let Some(held) = LedgerRefs::of_ref(&held_ref) else {
        return Ok(());
    };
    if held == location.refs() {
        return Ok(());
    }
    Err(mismatch(
        location,
        "the ledger store on this machine",
        held,
        &held_ref,
    ))
}

/// Called where the ledger ref is absent from origin and a new ledger would be made. When
/// origin holds the ledger on the other refs, it stops the command instead of making a
/// second ledger.
pub fn check_origin_has_no_other(repo: &Repo, location: &LedgerLocation) -> Result<(), String> {
    let other = location.with_refs(location.refs().other(), location.source());
    let tip = crate::ledger::store::dolt::remote_tip(&repo.git, &other)?;
    if tip.is_none() {
        return Ok(());
    }
    forget_cache_for(repo);
    Err(mismatch(
        location,
        location.label(),
        other.refs(),
        other.ref_name(),
    ))
}

fn forget_cache_for(repo: &Repo) {
    if let Ok(root) = super::location::state_root_from_env() {
        forget_cache(repo, &root);
    }
}

// --- Refusal by the host --------------------------------------------------------------

/// The error for a push the host refused for a ref outside `refs/heads/*`. `detail` is the
/// output of the failed git or Dolt command: one line of it that shows the refusal is
/// kept.
pub fn refused_push(ref_name: &str, detail: &str) -> String {
    let cause = detail
        .lines()
        .map(str::trim)
        .find(|line| is_server_refusal(line))
        .map(|line| match line.find("[remote rejected]") {
            Some(at) => line[at..].to_string(),
            None => line.chars().take(200).collect(),
        })
        .map(|line| format!("\n       The server said: {}", line))
        .unwrap_or_default();
    format!(
        "error: origin refused the push to {}. The host may accept only branches, as the Claude Code cloud proxy does.{}\n       \
         The local commits stay. To use branches for this ledger, run: tsk config ledger-refs branches",
        ref_name, cause
    )
}

/// True when git's or Dolt's output shows that the server declined the update of a ref, or
/// cut the push off as the Claude Code cloud proxy does, as against a push that lost a
/// race. Used only for a custom ref.
pub fn is_server_refusal(text: &str) -> bool {
    let text = text.to_lowercase();
    [
        "hook declined",
        "remote rejected",
        "not permitted",
        "forbidden",
        "403",
        "rpc failed",
        "unexpected disconnect",
        "connection reset",
    ]
    .iter()
    .any(|word| text.contains(word))
        && !text.contains("non-fast-forward")
        && !text.contains("stale info")
        && !text.contains("fetch first")
}

// --- Resolved location memo -----------------------------------------------------------

thread_local! {
    static MEMO: RefCell<Option<(PathBuf, PathBuf, LedgerLocation)>> = const { RefCell::new(None) };
}

pub fn memo_get(repo: &Repo, state_root: &Path) -> Option<LedgerLocation> {
    MEMO.with(|memo| {
        memo.borrow()
            .as_ref()
            .filter(|(common, root, _)| *common == repo.common_dir && root == state_root)
            .map(|(_, _, location)| location.clone())
    })
}

pub fn memo_set(repo: &Repo, state_root: &Path, location: &LedgerLocation) {
    MEMO.with(|memo| {
        *memo.borrow_mut() = Some((
            repo.common_dir.clone(),
            state_root.to_path_buf(),
            location.clone(),
        ));
    });
}

pub fn memo_forget() {
    MEMO.with(|memo| *memo.borrow_mut() = None);
}

// --- Moving an unpublished store ------------------------------------------------------

/// Point the ledger store's `origin` remote at the ref of `location`. Called by
/// `tsk config ledger-refs` for a store that was never published, so a person who chose
/// the wrong mode before the first push can correct it. Does nothing when no store exists
/// or it already uses the ref.
pub fn repoint_store(
    repo: &Repo,
    state_root: &Path,
    location: &LedgerLocation,
) -> Result<bool, String> {
    let Some(id) = repo.read_clone_id()? else {
        return Ok(false);
    };
    let path = dolt_db_path(state_root, &id);
    if !path.join(".dolt").is_dir() {
        return Ok(false);
    }
    if store_ref(repo, state_root).as_deref() == Some(location.ref_name()) {
        return Ok(false);
    }
    let url = crate::ledger::store::dolt::remote_url(repo, location)?;
    let db = super::dolt::Dolt::new(&path);
    db.run(["remote", "remove", super::dolt::REMOTE])?;
    db.run([
        "remote",
        "add",
        "--ref",
        location.ref_name(),
        super::dolt::REMOTE,
        url.as_str(),
    ])?;
    Ok(true)
}

/// The error for a switch of an existing, published ledger.
pub fn cannot_switch(held_ref: &str, held_where: &str, wanted: LedgerRefs) -> String {
    format!(
        "error: {} holds this ledger on {}, so it cannot use {} ledger refs.\n       \
         tsk does not move a ledger between branches and a custom ref, and it does not start a second ledger.",
        held_where,
        held_ref,
        wanted.as_str()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modes_parse_and_print() {
        assert_eq!(LedgerRefs::parse("branches"), Some(LedgerRefs::Branches));
        assert_eq!(LedgerRefs::parse(" custom\n"), Some(LedgerRefs::Custom));
        assert_eq!(LedgerRefs::parse("Custom"), None);
        assert_eq!(LedgerRefs::parse(""), None);
        assert_eq!(LedgerRefs::Branches.as_str(), "branches");
        assert_eq!(LedgerRefs::Custom.other(), LedgerRefs::Branches);
    }

    #[test]
    fn every_mode_names_its_refs_in_full() {
        for (mode, ledger, docs, nexus) in [
            (
                LedgerRefs::Branches,
                "refs/heads/tsk/ledger",
                "refs/heads/tsk/docs",
                "refs/heads/ledgers/",
            ),
            (
                LedgerRefs::Custom,
                "refs/tsk/ledger",
                "refs/tsk/docs",
                "refs/tsk/ledgers/",
            ),
        ] {
            assert_eq!(mode.ledger_ref(), ledger);
            assert_eq!(mode.docs_ref(), docs);
            assert_eq!(mode.nexus_prefix(), nexus);
            assert_eq!(LedgerRefs::of_ref(ledger), Some(mode));
        }
        assert_eq!(LedgerRefs::of_ref("tsk/ledger"), None);
    }

    #[test]
    fn the_repo_config_is_written_read_and_keeps_other_keys() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join(".tsk").join("config.toml");
        assert_eq!(read_repo_refs(&file).unwrap(), None);
        write_repo_refs(&file, LedgerRefs::Custom).unwrap();
        assert_eq!(read_repo_refs(&file).unwrap(), Some(LedgerRefs::Custom));
        std::fs::write(
            &file,
            "colour = \"blue\"\n[ledger]\nother = 1\nrefs = \"custom\"\n",
        )
        .unwrap();
        write_repo_refs(&file, LedgerRefs::Branches).unwrap();
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(text.contains("colour = \"blue\""), "{}", text);
        assert!(text.contains("other = 1"), "{}", text);
        assert_eq!(read_repo_refs(&file).unwrap(), Some(LedgerRefs::Branches));
    }

    #[test]
    fn the_top_of_a_working_tree_is_the_nearest_directory_with_a_git_entry() {
        let dir = tempfile::tempdir().unwrap();
        let top = std::fs::canonicalize(dir.path()).unwrap();
        std::fs::create_dir_all(top.join("a/b")).unwrap();
        assert_eq!(work_tree_top(&top.join("a/b")), find_above(&top));
        std::fs::create_dir(top.join(".git")).unwrap();
        assert_eq!(work_tree_top(&top.join("a/b")), Some(top.clone()));
        // A linked worktree has a `.git` file in its top, and wins over the tree above it.
        std::fs::write(top.join("a/.git"), "gitdir: elsewhere\n").unwrap();
        assert_eq!(work_tree_top(&top.join("a/b")), Some(top.join("a")));
    }

    /// What the walk finds above the temporary directory, which the test cannot control.
    fn find_above(top: &Path) -> Option<PathBuf> {
        top.ancestors()
            .skip(1)
            .find(|candidate| candidate.join(".git").exists())
            .map(Path::to_path_buf)
    }

    #[test]
    fn the_repo_config_rejects_a_bad_value_and_a_bad_type() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("config.toml");
        std::fs::write(&file, "[ledger]\nrefs = \"tags\"\n").unwrap();
        let error = read_repo_refs(&file).unwrap_err();
        assert!(error.contains("\"tags\""), "{}", error);
        assert!(error.contains("\"branches\" and \"custom\""), "{}", error);
        std::fs::write(&file, "[ledger]\nrefs = 3\n").unwrap();
        assert!(read_repo_refs(&file).is_err());
        std::fs::write(&file, "ledger = 3\n").unwrap();
        assert!(read_repo_refs(&file).is_err());
        std::fs::write(&file, "[ledger]\nbackend = \"dolt\"\n").unwrap();
        assert_eq!(read_repo_refs(&file).unwrap(), None);
    }

    #[test]
    fn a_listing_counts_only_the_exact_ref_names() {
        let listing = format!(
            "{a}\trefs/tsk/ledger\n{a}\trefs/heads/x/refs/tsk/ledger\n",
            a = "a".repeat(40)
        );
        assert_eq!(
            listed(&listing, &["refs/heads/tsk/ledger", "refs/tsk/ledger"]),
            vec!["refs/tsk/ledger".to_string()]
        );
    }

    #[test]
    fn a_hook_decline_is_a_refusal_and_a_lost_race_is_not() {
        assert!(is_server_refusal(
            "! [remote rejected] HEAD -> refs/tsk/docs (pre-receive hook declined)"
        ));
        assert!(is_server_refusal(
            "Write access is not permitted through this proxy"
        ));
        assert!(!is_server_refusal(
            "! [rejected] HEAD -> x (non-fast-forward)"
        ));
        assert!(!is_server_refusal("! [rejected] HEAD -> x (stale info)"));
        assert!(!is_server_refusal("failed to push some refs; fetch first"));
    }

    #[test]
    fn the_refusal_keeps_one_line_of_what_the_server_said() {
        let detail = "To file:///o.git\n!\trefs/dolt/x:refs/tsk/ledger\t[remote rejected] (pre-receive hook declined)\nDone\nhint: dolt does not support interactive credential prompts";
        let text = refused_push("refs/tsk/ledger", detail);
        assert_eq!(
            text,
            "error: origin refused the push to refs/tsk/ledger. The host may accept only branches, as the Claude Code cloud proxy does.\n       The server said: [remote rejected] (pre-receive hook declined)\n       The local commits stay. To use branches for this ledger, run: tsk config ledger-refs branches"
        );
    }

    #[test]
    fn the_refusal_names_the_ref_and_the_fix() {
        let text = refused_push("refs/tsk/docs", "");
        assert_eq!(
            text,
            "error: origin refused the push to refs/tsk/docs. The host may accept only branches, as the Claude Code cloud proxy does.\n       The local commits stay. To use branches for this ledger, run: tsk config ledger-refs branches"
        );
    }
}
