use std::collections::hash_map::RandomState;
use std::ffi::OsString;
use std::hash::{BuildHasher, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use super::git::Git;
use super::refs::{LedgerRefs, RefsSource};

pub const IN_REPO_REMOTE: &str = "origin";
pub const DOCS_DIR_NAME: &str = "docs";
pub const SCRATCH_DIR_NAME: &str = "scratch";
pub const DOLT_DIR_NAME: &str = "ledger-db";
pub const CLONE_ID_FILE: &str = "tsk-clone-id";
const CLONE_ID_SUFFIX_LEN: usize = 8;

pub const REPO_ID_FILE: &str = "tsk-repo-id";
pub const NOT_USED_FILE: &str = "tsk-not-used";

/// Which names the data ref and the documents ref of a nexus ledger have (ADR 0035).
///
/// - `V1`: `ledgers/<id>` and `ledgers/<id>-docs`, the names of tsk 0.69.0 and earlier.
/// - `V2`: `ledgers/<id>/ledger` and `ledgers/<id>/docs`.
///
/// A ledger that lives in its repo has one layout: its refs do not change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefLayout {
    V1,
    V2,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerLocation {
    remote: String,
    ref_name: String,
    repo_id: Option<String>,
    refs: LedgerRefs,
    source: RefsSource,
    layout: RefLayout,
    /// The layout of the documents ref. It differs from `layout` only while the move to the
    /// new names is unfinished for one of the two refs.
    docs_layout: RefLayout,
    /// The nexus entry carries the marker of the new ref layout (ADR 0035).
    marked: bool,
}

/// The data ref of a nexus ledger, in `refs` mode and `layout`.
fn nexus_data_ref(refs: LedgerRefs, layout: RefLayout, id: &str) -> String {
    match layout {
        RefLayout::V1 => format!("{}{}", refs.nexus_prefix(), id),
        RefLayout::V2 => format!("{}{}/ledger", refs.nexus_prefix(), id),
    }
}

/// The documents ref of a nexus ledger, in `refs` mode and `layout`.
fn nexus_docs_ref(refs: LedgerRefs, layout: RefLayout, id: &str) -> String {
    match layout {
        RefLayout::V1 => format!("{}{}-docs", refs.nexus_prefix(), id),
        RefLayout::V2 => format!("{}{}/docs", refs.nexus_prefix(), id),
    }
}

/// The prefix of the refs that hold a copy of a ledger while the move to the new names runs.
pub fn backup_prefix(refs: LedgerRefs) -> &'static str {
    match refs {
        LedgerRefs::Branches => "refs/heads/ledgers-backup/",
        LedgerRefs::Custom => "refs/tsk/ledgers-backup/",
    }
}

impl LedgerLocation {
    /// The location of a repo's own ledger on branches. Resolution builds the location in
    /// the mode in force with [`LedgerLocation::with_refs`].
    pub fn in_repo() -> LedgerLocation {
        LedgerLocation {
            remote: IN_REPO_REMOTE.to_string(),
            ref_name: LedgerRefs::Branches.ledger_ref().to_string(),
            repo_id: None,
            refs: LedgerRefs::Branches,
            source: RefsSource::Default,
            layout: RefLayout::V2,
            docs_layout: RefLayout::V2,
            marked: false,
        }
    }

    /// The location of a nexus-held ledger on branches, at the current ref layout.
    pub fn nexus(nexus_url: &str, repo_id: &str) -> LedgerLocation {
        LedgerLocation {
            remote: nexus_url.to_string(),
            ref_name: nexus_data_ref(LedgerRefs::Branches, RefLayout::V2, repo_id),
            repo_id: Some(repo_id.to_string()),
            refs: LedgerRefs::Branches,
            source: RefsSource::Default,
            layout: RefLayout::V2,
            docs_layout: RefLayout::V2,
            marked: false,
        }
    }

    /// The same ledger in `refs` mode: the data ref and the documents ref change, the remote
    /// and the repo id do not.
    pub fn with_refs(&self, refs: LedgerRefs, source: RefsSource) -> LedgerLocation {
        LedgerLocation {
            remote: self.remote.clone(),
            ref_name: match &self.repo_id {
                Some(id) => nexus_data_ref(refs, self.layout, id),
                None => refs.ledger_ref().to_string(),
            },
            repo_id: self.repo_id.clone(),
            refs,
            source,
            layout: self.layout,
            docs_layout: self.docs_layout,
            marked: self.marked,
        }
    }

    /// The same ledger, noting whether its nexus entry carries the marker of the new ref
    /// layout.
    pub fn with_marked(&self, marked: bool) -> LedgerLocation {
        LedgerLocation {
            marked,
            ..self.clone()
        }
    }

    /// True when the nexus entry of this ledger carries the marker of the new ref layout.
    pub fn marked(&self) -> bool {
        self.marked
    }

    /// The same ledger under the ref names of `layout`. An in-repo ledger has one layout, so
    /// it is returned as it is.
    pub fn with_layout(&self, layout: RefLayout) -> LedgerLocation {
        match &self.repo_id {
            Some(id) => LedgerLocation {
                remote: self.remote.clone(),
                ref_name: nexus_data_ref(self.refs, layout, id),
                repo_id: self.repo_id.clone(),
                refs: self.refs,
                source: self.source,
                layout,
                docs_layout: layout,
                marked: self.marked,
            },
            None => self.clone(),
        }
    }

    /// The same ledger with the documents ref under the names of `layout`, whatever the data
    /// ref uses.
    pub fn with_docs_layout(&self, layout: RefLayout) -> LedgerLocation {
        LedgerLocation {
            docs_layout: layout,
            ..self.clone()
        }
    }

    pub fn docs_layout(&self) -> RefLayout {
        self.docs_layout
    }

    /// A probe location whose data ref is `ref_name`, on the same remote: for asking whether
    /// the remote holds that ref.
    pub fn named(of: &LedgerLocation, ref_name: &str) -> LedgerLocation {
        LedgerLocation {
            ref_name: ref_name.to_string(),
            ..of.clone()
        }
    }

    pub fn layout(&self) -> RefLayout {
        self.layout
    }

    /// The data and documents refs a nexus ledger had before the move to the new names, or
    /// `None` for a ledger in its repo.
    pub fn legacy_ref_names(&self) -> Option<(String, String)> {
        let id = self.repo_id.as_deref()?;
        Some((
            nexus_data_ref(self.refs, RefLayout::V1, id),
            nexus_docs_ref(self.refs, RefLayout::V1, id),
        ))
    }

    /// The refs that hold a copy of the data and the documents while the move runs, or
    /// `None` for a ledger in its repo.
    pub fn backup_ref_names(&self) -> Option<(String, String)> {
        let id = self.repo_id.as_deref()?;
        let prefix = backup_prefix(self.refs);
        Some((
            format!("{}{}/ledger", prefix, id),
            format!("{}{}/docs", prefix, id),
        ))
    }

    pub fn refs(&self) -> LedgerRefs {
        self.refs
    }

    pub fn source(&self) -> RefsSource {
        self.source
    }

    pub fn remote(&self) -> &str {
        &self.remote
    }

    pub fn ref_name(&self) -> &str {
        &self.ref_name
    }

    pub fn repo_id(&self) -> Option<&str> {
        self.repo_id.as_deref()
    }

    /// The ref that holds the ledger's documents, on the same remote. A nexus ledger at the
    /// current layout keeps both refs under `ledgers/<id>/`: `ledger` and `docs`. The names
    /// before ADR 0035 were `ledgers/<id>` and `ledgers/<id>-docs`, because git cannot hold
    /// `ledgers/<id>` and `ledgers/<id>/docs` together.
    pub fn docs_ref_name(&self) -> String {
        match &self.repo_id {
            Some(id) => nexus_docs_ref(self.refs, self.docs_layout, id),
            None => self.refs.docs_ref().to_string(),
        }
    }

    /// The location of the documents branch: this location's remote and label, with the
    /// docs ref in place of the ledger ref.
    pub fn docs(&self) -> LedgerLocation {
        LedgerLocation {
            remote: self.remote.clone(),
            ref_name: self.docs_ref_name(),
            repo_id: self.repo_id.clone(),
            refs: self.refs,
            source: self.source,
            layout: self.docs_layout,
            docs_layout: self.docs_layout,
            marked: self.marked,
        }
    }

    pub fn label(&self) -> &str {
        if self.repo_id.is_some() {
            "the nexus"
        } else {
            &self.remote
        }
    }
}

/// What every command that needs the repository says, once, when it runs outside one.
pub const NOT_IN_REPO: &str =
    "error: not inside a git repository. Run tsk from inside the repository whose ledger you want.";

#[derive(Clone)]
pub struct Repo {
    pub git: Git,
    pub common_dir: PathBuf,
    /// The git directory of this checkout: `common_dir` for the main one, a directory under
    /// it for a linked code worktree.
    pub git_dir: PathBuf,
}

/// The repositories found in this process, by the directory asked from. A command asks
/// for the same directory more than once, and each ask is a start of `git`.
static DISCOVERED: Mutex<Vec<(PathBuf, Repo)>> = Mutex::new(Vec::new());

impl Repo {
    pub fn discover(dir: &Path) -> Result<Repo, String> {
        let known = DISCOVERED.lock().ok().and_then(|found| {
            found
                .iter()
                .find(|(asked, _)| asked == dir)
                .map(|(_, r)| r.clone())
        });
        if let Some(repo) = known {
            return Ok(repo);
        }
        let repo = Repo::ask_git(dir)?;
        if let Ok(mut found) = DISCOVERED.lock() {
            found.push((dir.to_path_buf(), repo.clone()));
        }
        Ok(repo)
    }

    fn ask_git(dir: &Path) -> Result<Repo, String> {
        let git = Git::new(dir);
        let (args, output) = git.output([
            "rev-parse",
            "--path-format=absolute",
            "--git-common-dir",
            "--git-dir",
        ])?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("not a git repository") {
                return Err(NOT_IN_REPO.to_string());
            }
            return Err(super::git::failure_message(&args, &output));
        }
        let text = String::from_utf8_lossy(&output.stdout);
        let mut lines = text.lines();
        let (Some(common_dir), Some(git_dir)) = (lines.next(), lines.next()) else {
            return Err(super::git::failure_message(&args, &output));
        };
        Ok(Repo {
            git,
            common_dir: PathBuf::from(common_dir.trim_end_matches('\r')),
            git_dir: PathBuf::from(git_dir.trim_end_matches('\r')),
        })
    }

    /// The ledger's location in the mode in force. Resolved once per process: the nexus
    /// index, the repo config and discovery are read the first time.
    pub fn location(&self, state_root: &Path) -> Result<LedgerLocation, String> {
        if let Some(known) = super::refs::memo_get(self, state_root) {
            return Ok(known);
        }
        let location = super::nexus::resolve(
            self,
            state_root,
            crate::config::config_file_from_env().as_deref(),
        )?;
        super::refs::memo_set(self, state_root, &location);
        Ok(location)
    }

    pub fn not_used_file(&self) -> PathBuf {
        self.common_dir.join(NOT_USED_FILE)
    }

    pub fn is_not_used(&self) -> bool {
        self.not_used_file().is_file()
    }

    pub fn write_not_used(&self) -> Result<(), String> {
        std::fs::write(self.not_used_file(), "not used\n").map_err(|e| {
            format!(
                "error: could not write {}: {}",
                self.not_used_file().display(),
                e
            )
        })
    }

    pub fn clear_not_used(&self) -> Result<bool, String> {
        match std::fs::remove_file(self.not_used_file()) {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(format!(
                "error: could not remove {}: {}",
                self.not_used_file().display(),
                e
            )),
        }
    }

    pub fn repo_id_file(&self) -> PathBuf {
        self.common_dir.join(REPO_ID_FILE)
    }

    pub fn read_repo_id(&self) -> Result<Option<String>, String> {
        read_clone_id(&self.repo_id_file())
    }

    pub fn write_repo_id(&self, id: &str) -> Result<(), String> {
        std::fs::write(self.repo_id_file(), format!("{}\n", id)).map_err(|e| {
            format!(
                "error: could not write the repo id to {}: {}",
                self.repo_id_file().display(),
                e
            )
        })
    }

    pub fn raw_origin_url(&self) -> Result<Option<String>, String> {
        let (args, output) = self.git.output(["config", "--get", "remote.origin.url"])?;
        match output.status.code() {
            Some(0) => {
                let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
                Ok(if url.is_empty() { None } else { Some(url) })
            }
            Some(1) => Ok(None),
            _ => Err(super::git::failure_message(&args, &output)),
        }
    }

    pub fn clone_id_file(&self) -> PathBuf {
        self.common_dir.join(CLONE_ID_FILE)
    }

    pub fn read_clone_id(&self) -> Result<Option<String>, String> {
        read_clone_id(&self.clone_id_file())
    }

    pub fn ensure_clone_id(&self) -> Result<String, String> {
        if let Some(id) = self.read_clone_id()? {
            return Ok(id);
        }
        let id = format!(
            "{}-{}",
            clone_id_name(&self.common_dir),
            mint_token(CLONE_ID_SUFFIX_LEN, &self.common_dir.to_string_lossy())
        );
        std::fs::write(self.clone_id_file(), format!("{}\n", id)).map_err(|e| {
            format!(
                "error: could not write the clone id to {}: {}",
                self.clone_id_file().display(),
                e
            )
        })?;
        Ok(id)
    }
}

/// The one git repository directly below `dir` that is not the nexus, or `None` when there
/// is none or more than one. A Claude Code cloud session with more than one repository
/// starts in their parent directory, so `tsk` run there (the hooks, or an agent) finds the
/// repository whose ledger it means. A hidden directory, and a repository whose origin
/// names `nexus_url`, do not count.
pub fn repo_below(dir: &Path, nexus_url: Option<&str>) -> Option<PathBuf> {
    let nexus = nexus_url.and_then(super::nexus::normalise_url);
    let mut found = Vec::new();
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        let hidden = entry.file_name().to_string_lossy().starts_with('.');
        if hidden || !path.is_dir() || !path.join(".git").exists() {
            continue;
        }
        let origin = Git::new(&path)
            .run_line(["config", "--get", "remote.origin.url"])
            .ok()
            .and_then(|url| super::nexus::normalise_url(&url));
        if nexus.is_some() && origin == nexus {
            continue;
        }
        found.push(path);
    }
    if found.len() == 1 {
        found.pop()
    } else {
        None
    }
}

/// In a cloud session whose working directory is in no git repository, change it to the
/// one repository below it ([`repo_below`]). Does nothing outside a cloud session, inside a
/// repository, or when there is no single repository below.
pub fn enter_repo_below() {
    if !crate::thread::session_start::is_remote() {
        return;
    }
    let Ok(cwd) = std::env::current_dir() else {
        return;
    };
    if Git::new(&cwd).run_line(["rev-parse", "--git-dir"]).is_ok() {
        return;
    }
    let nexus = crate::config::config_file_from_env()
        .and_then(|file| crate::config::read_nexus_url(&file).ok().flatten());
    if let Some(repo) = repo_below(&cwd, nexus.as_deref()) {
        let _ = std::env::set_current_dir(repo);
    }
}

pub fn read_clone_id(file: &Path) -> Result<Option<String>, String> {
    match std::fs::read_to_string(file) {
        Ok(text) => {
            let id: String = text.chars().filter(|c| !c.is_whitespace()).collect();
            Ok(if id.is_empty() { None } else { Some(id) })
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("error: could not read {}: {}", file.display(), e)),
    }
}

pub fn clone_id_name(common_dir: &Path) -> String {
    let name: String = common_dir
        .parent()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        .collect();
    if name.is_empty() {
        "repo".to_string()
    } else {
        name
    }
}

pub fn mint_token(length: usize, extra_seed: &str) -> String {
    let mut token = String::new();
    let mut round: u64 = 0;
    while token.len() < length {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let mut hasher = RandomState::new().build_hasher();
        hasher.write_u128(nanos);
        hasher.write_u32(std::process::id());
        hasher.write_u64(round);
        hasher.write(extra_seed.as_bytes());
        token.push_str(&format!("{:016x}", hasher.finish()));
        round += 1;
    }
    token.truncate(length);
    token
}

pub fn state_root(
    xdg_state_home: Option<OsString>,
    home: Option<OsString>,
) -> Result<PathBuf, String> {
    let base = match xdg_state_home.filter(|v| !v.is_empty()) {
        Some(xdg) => PathBuf::from(xdg),
        None => match home.filter(|v| !v.is_empty()) {
            Some(home) => PathBuf::from(home).join(".local").join("state"),
            None => return Err(
                "error: neither XDG_STATE_HOME nor HOME is set; cannot locate the ledger worktree"
                    .to_string(),
            ),
        },
    };
    Ok(base.join("tsk"))
}

pub fn state_root_from_env() -> Result<PathBuf, String> {
    state_root(std::env::var_os("XDG_STATE_HOME"), std::env::var_os("HOME"))
}

/// Where a ledger home keeps its scratch directory: beside `ledger-db` and `docs`, outside
/// the Dolt database and outside every git tree, so nothing commits, pushes, lists or drops it.
pub fn scratch_path(db_path: &Path) -> PathBuf {
    db_path.with_file_name(SCRATCH_DIR_NAME)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn init_repo(dir: &Path, origin: &str) {
        let run = |args: &[&str]| {
            let status = std::process::Command::new("git")
                .args(args)
                .status()
                .unwrap();
            assert!(status.success());
        };
        run(&["init", "--quiet", dir.to_str().unwrap()]);
        run(&[
            "-C",
            dir.to_str().unwrap(),
            "config",
            "remote.origin.url",
            origin,
        ]);
    }

    #[test]
    fn repo_below_finds_the_one_repository_that_is_not_the_nexus() {
        let root = tempfile::tempdir().unwrap();
        let nexus = "https://github.com/acme/tsk-nexus";
        init_repo(&root.path().join("tsk"), "https://github.com/acme/tsk");
        init_repo(&root.path().join("tsk-nexus"), nexus);
        init_repo(
            &root.path().join(".hidden"),
            "https://github.com/acme/hidden",
        );
        std::fs::create_dir(root.path().join("plain")).unwrap();

        assert_eq!(
            repo_below(root.path(), Some("git@github.com:acme/tsk-nexus.git")),
            Some(root.path().join("tsk"))
        );
        assert_eq!(repo_below(root.path(), None), None);

        init_repo(&root.path().join("other"), "https://github.com/acme/other");
        assert_eq!(repo_below(root.path(), Some(nexus)), None);
    }

    #[test]
    fn in_repo_location_uses_full_ref_name_on_origin() {
        let location = LedgerLocation::in_repo();
        assert_eq!(location.remote(), "origin");
        assert_eq!(location.ref_name(), "refs/heads/tsk/ledger");
        assert_eq!(location.label(), "origin");
        assert_eq!(location.repo_id(), None);
        assert!(location.ref_name().starts_with("refs/heads/"));
    }

    #[test]
    fn nexus_location_uses_the_repo_id_branch_on_the_nexus_url() {
        let location = LedgerLocation::nexus("https://example.test/o/nexus", "work-api");
        assert_eq!(location.remote(), "https://example.test/o/nexus");
        assert_eq!(location.ref_name(), "refs/heads/ledgers/work-api/ledger");
        assert_eq!(location.label(), "the nexus");
        assert_eq!(location.repo_id(), Some("work-api"));
    }

    #[test]
    fn the_docs_ref_is_tsk_docs_in_the_repo_and_id_slash_docs_on_the_nexus() {
        let in_repo = LedgerLocation::in_repo();
        assert_eq!(in_repo.docs_ref_name(), "refs/heads/tsk/docs");
        assert_eq!(in_repo.docs().ref_name(), "refs/heads/tsk/docs");
        assert_eq!(in_repo.docs().remote(), "origin");

        let nexus = LedgerLocation::nexus("https://example.test/o/nexus", "work-api");
        assert_eq!(nexus.docs_ref_name(), "refs/heads/ledgers/work-api/docs");
        let docs = nexus.docs();
        assert_eq!(docs.ref_name(), "refs/heads/ledgers/work-api/docs");
        assert_eq!(docs.remote(), "https://example.test/o/nexus");
        assert_eq!(docs.label(), "the nexus");
        // The two refs can coexist: neither is a path prefix of the other.
        assert!(!docs
            .ref_name()
            .starts_with(&format!("{}/", nexus.ref_name())));
    }

    #[test]
    fn the_old_layout_names_the_refs_of_tsk_0_69_and_earlier() {
        let nexus = LedgerLocation::nexus("n", "work-api").with_layout(RefLayout::V1);
        assert_eq!(nexus.ref_name(), "refs/heads/ledgers/work-api");
        assert_eq!(nexus.docs_ref_name(), "refs/heads/ledgers/work-api-docs");
        assert_eq!(nexus.layout(), RefLayout::V1);
        let current = nexus.with_layout(RefLayout::V2);
        assert_eq!(
            nexus.legacy_ref_names(),
            Some((
                "refs/heads/ledgers/work-api".to_string(),
                "refs/heads/ledgers/work-api-docs".to_string()
            ))
        );
        assert_eq!(current.ref_name(), "refs/heads/ledgers/work-api/ledger");
        assert_eq!(
            current.backup_ref_names(),
            Some((
                "refs/heads/ledgers-backup/work-api/ledger".to_string(),
                "refs/heads/ledgers-backup/work-api/docs".to_string()
            ))
        );
        let in_repo = LedgerLocation::in_repo();
        assert_eq!(in_repo.legacy_ref_names(), None);
        assert_eq!(in_repo.backup_ref_names(), None);
        assert_eq!(
            in_repo.with_layout(RefLayout::V1).ref_name(),
            "refs/heads/tsk/ledger"
        );
    }

    #[test]
    fn a_custom_ref_location_names_every_ref_in_full() {
        use crate::ledger::refs::{LedgerRefs, RefsSource};
        let in_repo = LedgerLocation::in_repo().with_refs(LedgerRefs::Custom, RefsSource::Default);
        assert_eq!(in_repo.ref_name(), "refs/tsk/ledger");
        assert_eq!(in_repo.docs_ref_name(), "refs/tsk/docs");
        assert_eq!(in_repo.docs().ref_name(), "refs/tsk/docs");
        assert_eq!(in_repo.remote(), "origin");
        assert_eq!(in_repo.refs(), LedgerRefs::Custom);
        assert_eq!(in_repo.source(), RefsSource::Default);

        let nexus = LedgerLocation::nexus("https://example.test/o/nexus", "work-api")
            .with_refs(LedgerRefs::Custom, RefsSource::Nexus);
        assert_eq!(nexus.ref_name(), "refs/tsk/ledgers/work-api/ledger");
        assert_eq!(nexus.docs_ref_name(), "refs/tsk/ledgers/work-api/docs");
        assert_eq!(
            nexus.backup_ref_names().unwrap().0,
            "refs/tsk/ledgers-backup/work-api/ledger"
        );
        assert_eq!(nexus.docs().label(), "the nexus");
        assert_eq!(nexus.repo_id(), Some("work-api"));
        // The two refs can coexist: neither is a path prefix of the other.
        assert!(!nexus
            .docs_ref_name()
            .starts_with(&format!("{}/", nexus.ref_name())));
    }

    #[test]
    fn with_refs_round_trips_to_the_branch_location() {
        use crate::ledger::refs::{LedgerRefs, RefsSource};
        let branches = LedgerLocation::nexus("n", "x");
        let custom = branches.with_refs(LedgerRefs::Custom, RefsSource::Default);
        assert_ne!(branches, custom);
        assert_eq!(
            custom.with_refs(LedgerRefs::Branches, RefsSource::Default),
            branches
        );
    }

    #[test]
    fn scratch_path_is_scratch_beside_the_database() {
        assert_eq!(
            scratch_path(Path::new("/s/tsk/ledgers/work-api/ledger-db")),
            PathBuf::from("/s/tsk/ledgers/work-api/scratch")
        );
    }

    #[test]
    fn state_root_prefers_xdg_state_home() {
        let root = state_root(Some("/x/state".into()), Some("/home/u".into())).unwrap();
        assert_eq!(root, PathBuf::from("/x/state/tsk"));
    }

    #[test]
    fn state_root_treats_empty_xdg_as_unset() {
        let root = state_root(Some("".into()), Some("/home/u".into())).unwrap();
        assert_eq!(root, PathBuf::from("/home/u/.local/state/tsk"));
    }

    #[test]
    fn state_root_falls_back_to_home() {
        let root = state_root(None, Some("/home/u".into())).unwrap();
        assert_eq!(root, PathBuf::from("/home/u/.local/state/tsk"));
    }

    #[test]
    fn state_root_fails_without_xdg_or_home() {
        assert!(state_root(None, None).is_err());
    }

    #[test]
    fn clone_id_name_is_the_clone_directory_name() {
        assert_eq!(clone_id_name(Path::new("/code/tsk/.git")), "tsk");
    }

    #[test]
    fn clone_id_name_strips_unsafe_characters() {
        assert_eq!(
            clone_id_name(Path::new("/code/my repo!@v1.2_x/.git")),
            "myrepov1.2_x"
        );
    }

    #[test]
    fn clone_id_name_falls_back_to_repo() {
        assert_eq!(clone_id_name(Path::new("/code/***/.git")), "repo");
        assert_eq!(clone_id_name(Path::new("/")), "repo");
    }

    #[test]
    fn mint_token_is_lowercase_hex_of_requested_length() {
        for length in [1, 8, 16, 20] {
            let token = mint_token(length, "seed");
            assert_eq!(token.len(), length);
            assert!(token
                .chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
        }
    }

    #[test]
    fn mint_token_differs_between_calls() {
        assert_ne!(mint_token(8, "seed"), mint_token(8, "seed"));
    }

    #[test]
    fn read_clone_id_strips_whitespace_and_treats_empty_as_absent() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join(CLONE_ID_FILE);
        assert_eq!(read_clone_id(&file).unwrap(), None);
        std::fs::write(&file, "  \n").unwrap();
        assert_eq!(read_clone_id(&file).unwrap(), None);
        std::fs::write(&file, " tsk-1234abcd \n").unwrap();
        assert_eq!(
            read_clone_id(&file).unwrap(),
            Some("tsk-1234abcd".to_string())
        );
    }
}
