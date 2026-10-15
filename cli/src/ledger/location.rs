use std::collections::hash_map::RandomState;
use std::ffi::OsString;
use std::hash::{BuildHasher, Hasher};
use std::path::{Path, PathBuf};

use super::git::Git;

pub const IN_REPO_REMOTE: &str = "origin";
pub const IN_REPO_REF: &str = "refs/heads/tsk/ledger";
pub const IN_REPO_DOCS_REF: &str = "refs/heads/tsk/docs";
pub const WORKTREE_DIR_NAME: &str = "ledger";
pub const DOCS_DIR_NAME: &str = "docs";
pub const DOLT_DIR_NAME: &str = "ledger-db";
pub const CLONE_ID_FILE: &str = "tsk-clone-id";
const CLONE_ID_SUFFIX_LEN: usize = 8;

pub const NEXUS_REF_PREFIX: &str = "refs/heads/ledgers/";
pub const REPO_ID_FILE: &str = "tsk-repo-id";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerLocation {
    remote: String,
    ref_name: String,
    repo_id: Option<String>,
}

impl LedgerLocation {
    pub fn in_repo() -> LedgerLocation {
        LedgerLocation {
            remote: IN_REPO_REMOTE.to_string(),
            ref_name: IN_REPO_REF.to_string(),
            repo_id: None,
        }
    }

    pub fn nexus(nexus_url: &str, repo_id: &str) -> LedgerLocation {
        LedgerLocation {
            remote: nexus_url.to_string(),
            ref_name: format!("{}{}", NEXUS_REF_PREFIX, repo_id),
            repo_id: Some(repo_id.to_string()),
        }
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

    /// The ref that holds the ledger's documents, on the same remote. A nexus ref ends in
    /// `-docs` rather than `/docs`, because git cannot hold both `ledgers/<id>` and
    /// `ledgers/<id>/docs` as refs: one would be a file and the other a directory.
    pub fn docs_ref_name(&self) -> String {
        match &self.repo_id {
            Some(id) => format!("{}{}-docs", NEXUS_REF_PREFIX, id),
            None => IN_REPO_DOCS_REF.to_string(),
        }
    }

    /// The location of the documents branch: this location's remote and label, with the
    /// docs ref in place of the ledger ref.
    pub fn docs(&self) -> LedgerLocation {
        LedgerLocation {
            remote: self.remote.clone(),
            ref_name: self.docs_ref_name(),
            repo_id: self.repo_id.clone(),
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

#[derive(Clone)]
pub struct Repo {
    pub git: Git,
    pub common_dir: PathBuf,
}

impl Repo {
    pub fn discover(dir: &Path) -> Result<Repo, String> {
        let git = Git::new(dir);
        let common_dir = git
            .run_line(["rev-parse", "--path-format=absolute", "--git-common-dir"])
            .map_err(|e| {
                format!(
                    "error: not inside a git repository ({})",
                    e.trim_start_matches("error: ")
                )
            })?;
        Ok(Repo {
            git,
            common_dir: PathBuf::from(common_dir),
        })
    }

    pub fn location(&self, state_root: &Path) -> Result<LedgerLocation, String> {
        super::nexus::resolve(
            self,
            state_root,
            crate::config::config_file_from_env().as_deref(),
        )
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

pub fn worktree_path(state_root: &Path, clone_id: &str) -> PathBuf {
    state_root
        .join("repos")
        .join(clone_id)
        .join(WORKTREE_DIR_NAME)
}

/// Where the Dolt backend keeps the worktree of its documents branch.
pub fn docs_worktree_path(state_root: &Path, clone_id: &str) -> PathBuf {
    state_root.join("repos").join(clone_id).join(DOCS_DIR_NAME)
}

/// Where the Dolt backend keeps its database, beside the files backend's worktree.
pub fn dolt_db_path(state_root: &Path, clone_id: &str) -> PathBuf {
    state_root.join("repos").join(clone_id).join(DOLT_DIR_NAME)
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(location.ref_name(), "refs/heads/ledgers/work-api");
        assert_eq!(location.label(), "the nexus");
        assert_eq!(location.repo_id(), Some("work-api"));
    }

    #[test]
    fn the_docs_ref_is_tsk_docs_in_the_repo_and_id_docs_on_the_nexus() {
        let in_repo = LedgerLocation::in_repo();
        assert_eq!(in_repo.docs_ref_name(), "refs/heads/tsk/docs");
        assert_eq!(in_repo.docs().ref_name(), "refs/heads/tsk/docs");
        assert_eq!(in_repo.docs().remote(), "origin");

        let nexus = LedgerLocation::nexus("https://example.test/o/nexus", "work-api");
        assert_eq!(nexus.docs_ref_name(), "refs/heads/ledgers/work-api-docs");
        let docs = nexus.docs();
        assert_eq!(docs.ref_name(), "refs/heads/ledgers/work-api-docs");
        assert_eq!(docs.remote(), "https://example.test/o/nexus");
        assert_eq!(docs.label(), "the nexus");
        // The two refs can coexist: neither is a path prefix of the other.
        assert!(!docs
            .ref_name()
            .starts_with(&format!("{}/", nexus.ref_name())));
    }

    #[test]
    fn docs_worktree_path_is_docs_under_clone_id() {
        assert_eq!(
            docs_worktree_path(Path::new("/s/tsk"), "tsk-180ae86e"),
            PathBuf::from("/s/tsk/repos/tsk-180ae86e/docs")
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
    fn worktree_path_is_ledger_under_clone_id() {
        let path = worktree_path(Path::new("/s/tsk"), "tsk-180ae86e");
        assert_eq!(path, PathBuf::from("/s/tsk/repos/tsk-180ae86e/ledger"));
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
