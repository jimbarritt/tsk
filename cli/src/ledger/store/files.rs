//! The git-file backend: the ledger is a branch of the managed repo's remote, checked
//! out as a detached linked worktree outside the repository. Documents, threads,
//! continuation logs, bindings and events are ordinary files in that worktree, and
//! publishing is a git commit and push. The fetch and push steps live in
//! `ledger::fetch` and `ledger::push`.

use std::io::Write;
use std::path::{Path, PathBuf};

use crate::events::queue::{
    parse_watermark, render_watermark, EVENTS_DIR, QUEUE_FILE, WATERMARK_FILE,
};
use crate::ledger::git::Git;
use crate::ledger::location::{state_root_from_env, worktree_path, Repo};
use crate::ledger::manifest::{self, ManifestInfo, MANIFEST_FILE};
use crate::ledger::status::{self, Change, LedgerStatus, SideStatus};
use crate::ledger::{fetch, push};
use crate::mission::model::{self, Mission};
use crate::thread::entry::STORE_FILE;
use crate::thread::lookup::{
    lookup_path, worktree_binding_file_name, worktree_bindings_dir, CloudBinding, Lookup,
    WorktreeBinding,
};
use crate::thread::ops::{index_md, index_mission, mission_link};

use super::{BindingKind, BindingRow, LedgerStore, PushReport, RefreshOutcome, ThreadInfo};

const THREADS_DIR: &str = "threads";
/// The top-level directory of record files. One JSON file per mission sits in
/// `records/missions/`, so git merges two missions without a conflict.
pub const RECORDS_DIR: &str = "records";
const MISSIONS_DIR: &str = "missions";

/// True for the parts of the file ledger that are data: records, threads, the event
/// queue and the manifest. Every other file is a document.
pub fn is_data_path(path: &str) -> bool {
    let first = path.split('/').next().unwrap_or("");
    first == RECORDS_DIR || first == THREADS_DIR || first == EVENTS_DIR || path == MANIFEST_FILE
}

pub struct FilesStore {
    repo: Repo,
    state_root: PathBuf,
}

impl FilesStore {
    pub fn open(dir: &Path) -> Result<FilesStore, String> {
        let repo = Repo::discover(dir)?;
        let state_root = state_root_from_env()?;
        Ok(FilesStore { repo, state_root })
    }

    fn hint(&self) -> Option<PathBuf> {
        self.repo
            .read_clone_id()
            .ok()
            .flatten()
            .map(|id| worktree_path(&self.state_root, &id))
    }

    fn tree(&self) -> Result<FileTree, String> {
        let missing = |what: String| {
            format!(
                "error: {}. Run tsk ledger fetch first, which creates the ledger worktree",
                what
            )
        };
        let clone_id = self.repo.read_clone_id()?.ok_or_else(|| {
            missing(format!(
                "this clone has no clone id yet ({} is absent or empty)",
                self.repo.clone_id_file().display()
            ))
        })?;
        let root = worktree_path(&self.state_root, &clone_id);
        if !root.is_dir() {
            return Err(missing(format!("{} does not exist", root.display())));
        }
        Ok(FileTree { root, docs: false })
    }

    fn describe(&self, rel: &str) -> String {
        match self.hint() {
            Some(root) => root.join(rel).display().to_string(),
            None => rel.to_string(),
        }
    }
}

impl FilesStore {
    fn refresh_with(&self, keep_local: bool) -> Result<RefreshOutcome, String> {
        let outcome = fetch::fetch(&self.repo, &self.state_root, keep_local)?;
        let pending_note =
            (!outcome.pending.is_empty()).then(|| crate::ledger::pending_note(&outcome));
        let unpublished_note = outcome
            .unpublished
            .then(|| crate::ledger::unpublished_note(&outcome));
        Ok(RefreshOutcome {
            remote_label: format!(
                "{}'s {}",
                outcome.location.label(),
                outcome.location.ref_name()
            ),
            path: outcome.path,
            remote_head: outcome.commit,
            pending: outcome.pending,
            unpublished: outcome.unpublished,
            pending_note,
            unpublished_note,
        })
    }
}

impl LedgerStore for FilesStore {
    fn refresh(&self) -> Result<RefreshOutcome, String> {
        self.refresh_with(false)
    }

    fn refresh_keeping_local(&self) -> Result<RefreshOutcome, String> {
        self.refresh_with(true)
    }

    fn has_local_changes(&self) -> Result<bool, String> {
        let wt = Git::new(&self.tree()?.root);
        Ok(!wt.run(["status", "--porcelain=v2", "-z"])?.is_empty())
    }

    fn status(&self) -> Result<LedgerStatus, String> {
        let wt = Git::new(&self.tree()?.root);
        let location = self.repo.location(&self.state_root)?;
        let fetched = fetch::fetch_tip(&self.repo.git, &location);
        let side = status::worktree_side(&wt, &fetched)?;
        let (mut data, mut docs) = (Vec::new(), Vec::new());
        for (letter, path) in side.files {
            let line = Change::file(letter, &path);
            if is_data_path(&path) {
                data.push(line);
            } else {
                docs.push(line);
            }
        }
        // One branch holds both, so both stand the same against origin.
        Ok(LedgerStatus {
            data: SideStatus {
                changes: data,
                remote: side.remote.clone(),
            },
            docs: SideStatus {
                changes: docs,
                remote: side.remote,
            },
            shared_branch: true,
            fetch_note: fetched.err().map(|e| {
                status::fetch_failed_note(
                    &format!("{}'s {}", location.label(), location.ref_name()),
                    &e,
                )
            }),
        })
    }

    fn discard(&self) -> Result<(), String> {
        let wt = Git::new(&self.tree()?.root);
        let location = self.repo.location(&self.state_root)?;
        let tip = fetch::fetch_tip(&self.repo.git, &location).map_err(|e| {
            format!(
                "{}\n       Nothing was discarded: the ledger cannot be reset to a state that was not fetched",
                e
            )
        })?;
        status::discard_worktree(&wt, tip.as_deref())
    }

    fn path(&self) -> Result<PathBuf, String> {
        let clone_id = self.repo.read_clone_id()?.ok_or_else(|| {
            format!(
                "error: this clone has no clone id yet ({} is absent or empty).\n       Run tsk ledger fetch, which mints it.",
                self.repo.clone_id_file().display()
            )
        })?;
        Ok(worktree_path(&self.state_root, &clone_id))
    }

    fn exists_locally(&self) -> Result<bool, String> {
        Ok(self.hint().is_some_and(|path| path.is_dir()))
    }

    fn commit_and_push(
        &self,
        message: &str,
        notes: &mut dyn FnMut(&str),
    ) -> Result<PushReport, String> {
        // Documents and records share one commit here, so a link to a document that is
        // gone stops the push before anything is committed.
        let tree = self.tree()?;
        model::check_links(
            &tree.missions_load()?,
            &|path| tree.doc_path(path).is_file(),
            "Nothing was committed or pushed",
        )?;
        let outcome = {
            let mut on_retry = |attempt: usize, status: &push::PushStatus| {
                notes(&crate::ledger::retry_note(attempt, status))
            };
            push::push(&self.repo, &self.state_root, message, &mut on_retry)?
        };
        for note in crate::ledger::push_notes(&outcome) {
            notes(&note);
        }
        Ok(PushReport {
            committed: outcome.committed,
            head: outcome.commit,
            already_on_remote: outcome.result == push::PushResult::AlreadyOnRemote,
        })
    }

    fn head_id(&self) -> Result<String, String> {
        self.tree()?.head_id()
    }

    fn manifest(&self) -> Result<ManifestInfo, String> {
        self.tree()?.manifest()
    }

    fn check_manifest(&self) -> Result<i64, String> {
        let text = self.tree()?.manifest_text()?;
        let location = self.repo.location(&self.state_root)?;
        manifest::check(&text, location.repo_id())
    }

    fn doc_get(&self, path: &str) -> Result<Option<String>, String> {
        super::check_document_path(path)?;
        self.tree()?.doc_get(path)
    }

    fn doc_put(&self, path: &str, body: &str) -> Result<(), String> {
        super::check_document_path(path)?;
        self.tree()?.doc_put(path, body)
    }

    fn doc_list(&self, prefix: &str) -> Result<Vec<String>, String> {
        self.tree()?.doc_list(prefix)
    }

    fn doc_remove(&self, path: &str) -> Result<bool, String> {
        super::check_document_path(path)?;
        self.tree()?.doc_remove(path)
    }

    fn doc_exists(&self, path: &str) -> Result<bool, String> {
        if super::check_document_path(path).is_err() {
            return Ok(false);
        }
        Ok(self.tree()?.doc_path(path).is_file())
    }

    fn doc_path(&self, path: &str) -> Result<Option<PathBuf>, String> {
        super::check_document_path(path)?;
        let file = self.tree()?.doc_path(path);
        Ok(file.is_file().then_some(file))
    }

    fn thread_exists(&self, id: &str) -> Result<bool, String> {
        Ok(self.tree()?.thread_dir(id).is_dir())
    }

    fn thread_create(
        &self,
        id: &str,
        mission_id: Option<&str>,
        briefing_path: Option<&str>,
    ) -> Result<(), String> {
        self.tree()?.thread_create(id, mission_id, briefing_path)
    }

    fn thread_set_mission(
        &self,
        id: &str,
        mission_id: Option<&str>,
        briefing_path: Option<&str>,
    ) -> Result<(), String> {
        self.tree()?
            .thread_set_mission(id, mission_id, briefing_path)
    }

    fn thread_delete(&self, id: &str) -> Result<(), String> {
        self.tree()?.thread_delete(id)
    }

    fn thread_ids(&self) -> Result<Vec<String>, String> {
        self.tree()?.thread_ids()
    }

    fn thread_info(&self, id: &str) -> Result<ThreadInfo, String> {
        Ok(self.tree()?.thread_info(id))
    }

    fn continuation_append(&self, id: &str, line: &str) -> Result<(), String> {
        self.tree()?.continuation_append(id, line)
    }

    fn continuation_list(&self, id: &str) -> Result<Vec<String>, String> {
        self.tree()?.continuation_list(id)
    }

    fn bindings_list(&self) -> Result<Vec<BindingRow>, String> {
        self.tree()?.bindings_list()
    }

    fn binding_set(&self, row: &BindingRow) -> Result<(), String> {
        self.tree()?.binding_set(row)
    }

    fn binding_remove(&self, actor: &str) -> Result<(), String> {
        self.tree()?.binding_remove(actor)
    }

    fn binding_remove_thread(&self, thread_id: &str) -> Result<(), String> {
        self.tree()?.binding_remove_thread(thread_id)
    }

    fn events_append(&self, lines: &str) -> Result<(), String> {
        self.tree()?.events_append(lines)
    }

    fn events_list(&self) -> Result<Option<Vec<String>>, String> {
        self.tree()?.events_list()
    }

    fn events_count(&self) -> Result<u64, String> {
        Ok(self
            .tree()?
            .events_list()?
            .map(|lines| lines.len() as u64)
            .unwrap_or(0))
    }

    fn watermark_get(&self) -> Result<u64, String> {
        self.tree()?.watermark_get()
    }

    fn watermark_set(&self, processed_through: u64, updated_at: &str) -> Result<(), String> {
        self.tree()?.watermark_set(processed_through, updated_at)
    }

    fn missions_load(&self) -> Result<Vec<Mission>, String> {
        self.tree()?.missions_load()
    }

    fn mission_save(&self, mission: &Mission) -> Result<(), String> {
        self.tree()?.mission_save(mission)
    }

    fn describe_document(&self, path: &str) -> String {
        self.describe(path)
    }

    fn describe_thread(&self, id: &str) -> String {
        self.describe(&format!("{}/{}/{}", THREADS_DIR, id, STORE_FILE))
    }

    fn describe_threads(&self) -> String {
        self.describe(THREADS_DIR)
    }

    fn pending_inspect_hint(&self, path: &Path, remote_head: &str) -> String {
        format!("git -C '{}' log -p {}..HEAD", path.display(), remote_head)
    }
}

/// File operations on a materialised ledger worktree.
pub struct FileTree {
    pub root: PathBuf,
    /// True for the documents branch of a Dolt ledger: every top-level name is a
    /// document, and a file name that is not valid UTF-8 is an error. False for the file
    /// ledger, whose `threads/`, `external-events/` and manifest are not documents.
    pub docs: bool,
}

fn read_optional(path: &Path) -> Result<Option<String>, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("error: could not read {}: {}", path.display(), e)),
    }
}

fn remove_if_present(path: &Path) -> Result<(), String> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("error: could not remove {}: {}", path.display(), e)),
    }
}

fn ensure_parent(path: &Path) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("error: could not create {}: {}", dir.display(), e))?;
    }
    Ok(())
}

impl FileTree {
    pub(crate) fn doc_path(&self, path: &str) -> PathBuf {
        self.root.join(path)
    }

    fn missions_dir(&self) -> PathBuf {
        self.root.join(RECORDS_DIR).join(MISSIONS_DIR)
    }

    fn missions_load(&self) -> Result<Vec<Mission>, String> {
        let dir = self.missions_dir();
        let entries = match std::fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(format!("error: could not read {}: {}", dir.display(), e)),
        };
        let mut missions = Vec::new();
        for entry in entries {
            let entry =
                entry.map_err(|e| format!("error: could not read {}: {}", dir.display(), e))?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let text = std::fs::read_to_string(&path)
                .map_err(|e| format!("error: could not read {}: {}", path.display(), e))?;
            let mut mission: Mission = serde_json::from_str(&text)
                .map_err(|e| format!("error: {} is not a mission record: {}", path.display(), e))?;
            mission.follow_list_counters();
            missions.push(mission);
        }
        missions.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(missions)
    }

    /// One pretty-printed file per mission, keys in the order of the struct.
    fn mission_save(&self, mission: &Mission) -> Result<(), String> {
        let file = self.missions_dir().join(format!("{}.json", mission.id));
        ensure_parent(&file)?;
        let mut text = serde_json::to_string_pretty(mission)
            .map_err(|e| format!("error: could not write the mission record: {}", e))?;
        text.push('\n');
        std::fs::write(&file, text)
            .map_err(|e| format!("error: could not write {}: {}", file.display(), e))
    }

    fn thread_dir(&self, id: &str) -> PathBuf {
        self.root.join(THREADS_DIR).join(id)
    }

    /// Change the cloud session lookup. A file that does not exist is not created for a
    /// change that leaves it empty.
    fn edit_lookup(&self, change: impl FnOnce(&mut Lookup)) -> Result<(), String> {
        let path = lookup_path(&self.root);
        let existing = Lookup::read(&path)?;
        let before = existing.clone().unwrap_or_default();
        let mut lookup = before.clone();
        change(&mut lookup);
        if existing.is_none() && lookup == before {
            return Ok(());
        }
        lookup.write(&path)
    }

    fn worktree_binding_path(&self, actor: &str) -> PathBuf {
        worktree_bindings_dir(&self.root).join(worktree_binding_file_name(actor))
    }

    /// Every worktree binding file, sorted by file name.
    fn worktree_bindings(&self) -> Result<Vec<WorktreeBinding>, String> {
        let dir = worktree_bindings_dir(&self.root);
        let mut files: Vec<PathBuf> = match std::fs::read_dir(&dir) {
            Ok(entries) => entries
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|ext| ext == "json"))
                .collect(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(format!("error: could not read {}: {}", dir.display(), e)),
        };
        files.sort();
        files
            .iter()
            .map(|path| {
                let text = std::fs::read_to_string(path)
                    .map_err(|e| format!("error: could not read {}: {}", path.display(), e))?;
                WorktreeBinding::parse(&text, path)
            })
            .collect()
    }

    fn queue_path(&self) -> PathBuf {
        self.root.join(EVENTS_DIR).join(QUEUE_FILE)
    }

    fn watermark_path(&self) -> PathBuf {
        self.root.join(EVENTS_DIR).join(WATERMARK_FILE)
    }

    fn head_id(&self) -> Result<String, String> {
        Git::new(&self.root).run_line(["rev-parse", "--verify", "HEAD^{commit}"])
    }

    #[allow(dead_code)]
    fn manifest_text(&self) -> Result<String, String> {
        read_optional(&self.root.join(MANIFEST_FILE))?.ok_or_else(|| {
            format!(
                "error: the ledger has no {}; it is not a tsk ledger",
                MANIFEST_FILE
            )
        })
    }

    #[allow(dead_code)]
    fn manifest(&self) -> Result<ManifestInfo, String> {
        manifest::read(&self.manifest_text()?)
    }

    pub(crate) fn doc_get(&self, path: &str) -> Result<Option<String>, String> {
        let file = self.doc_path(path);
        if !file.is_file() {
            return Ok(None);
        }
        read_optional(&file)
    }

    pub(crate) fn doc_put(&self, path: &str, body: &str) -> Result<(), String> {
        let file = self.doc_path(path);
        ensure_parent(&file)?;
        std::fs::write(&file, body)
            .map_err(|e| format!("error: could not write {}: {}", file.display(), e))
    }

    pub(crate) fn doc_remove(&self, path: &str) -> Result<bool, String> {
        let file = self.doc_path(path);
        if !file.is_file() {
            return Ok(false);
        }
        std::fs::remove_file(&file)
            .map_err(|e| format!("error: could not remove {}: {}", file.display(), e))?;
        Ok(true)
    }

    pub(crate) fn doc_list(&self, prefix: &str) -> Result<Vec<String>, String> {
        let mut found = Vec::new();
        self.walk(&self.root, "", &mut found)?;
        found.retain(|path| path.starts_with(prefix));
        found.sort();
        Ok(found)
    }

    fn walk(&self, dir: &Path, rel: &str, found: &mut Vec<String>) -> Result<(), String> {
        let entries = std::fs::read_dir(dir)
            .map_err(|e| format!("error: could not read {}: {}", dir.display(), e))?;
        for entry in entries {
            let entry =
                entry.map_err(|e| format!("error: could not read {}: {}", dir.display(), e))?;
            let name = match entry.file_name().to_str() {
                Some(name) => name.to_string(),
                None if self.docs => {
                    return Err(format!(
                        "error: {} holds a file whose name is not valid UTF-8: {}",
                        dir.display(),
                        entry.file_name().to_string_lossy()
                    ))
                }
                None => entry.file_name().to_string_lossy().into_owned(),
            };
            let path = if rel.is_empty() {
                name.clone()
            } else {
                format!("{}/{}", rel, name)
            };
            if name == ".git" || (!self.docs && rel.is_empty() && is_reserved_top(&name)) {
                continue;
            }
            if entry.path().is_dir() {
                self.walk(&entry.path(), &path, found)?;
            } else {
                found.push(path);
            }
        }
        Ok(())
    }

    fn thread_create(
        &self,
        id: &str,
        mission_id: Option<&str>,
        briefing_path: Option<&str>,
    ) -> Result<(), String> {
        let dir = self.thread_dir(id);
        if dir.exists() {
            return Err(format!("error: threads/{} already exists", id));
        }
        std::fs::create_dir_all(&dir)
            .map_err(|e| format!("error: could not create {}: {}", dir.display(), e))?;
        let index = dir.join("index.md");
        std::fs::write(&index, index_md(id, mission_id, briefing_path))
            .map_err(|e| format!("error: could not write {}: {}", index.display(), e))?;
        let store = dir.join(STORE_FILE);
        std::fs::write(&store, "")
            .map_err(|e| format!("error: could not write {}: {}", store.display(), e))
    }

    fn thread_set_mission(
        &self,
        id: &str,
        mission_id: Option<&str>,
        briefing_path: Option<&str>,
    ) -> Result<(), String> {
        let dir = self.thread_dir(id);
        if !dir.is_dir() {
            return Err(format!("error: no such thread: {}", id));
        }
        let index = dir.join("index.md");
        std::fs::write(&index, index_md(id, mission_id, briefing_path))
            .map_err(|e| format!("error: could not write {}: {}", index.display(), e))
    }

    fn thread_delete(&self, id: &str) -> Result<(), String> {
        let dir = self.thread_dir(id);
        std::fs::remove_dir_all(&dir)
            .map_err(|e| format!("error: could not remove {}: {}", dir.display(), e))
    }

    fn thread_ids(&self) -> Result<Vec<String>, String> {
        let threads = self.root.join(THREADS_DIR);
        let mut names: Vec<String> = match std::fs::read_dir(&threads) {
            Ok(dir) => dir
                .filter_map(|e| e.ok())
                .filter(|e| e.path().is_dir())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|name| !name.starts_with('.'))
                .collect(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(e) => {
                return Err(format!(
                    "error: could not read {}: {}",
                    threads.display(),
                    e
                ))
            }
        };
        names.sort();
        Ok(names)
    }

    fn thread_info(&self, id: &str) -> ThreadInfo {
        match std::fs::read_to_string(self.thread_dir(id).join("index.md")) {
            Ok(text) => ThreadInfo {
                mission_id: index_mission(&text),
                briefing_path: Some(mission_link(&text)).filter(|p| !p.is_empty()),
            },
            Err(_) => ThreadInfo::default(),
        }
    }

    fn continuation_append(&self, id: &str, line: &str) -> Result<(), String> {
        let store = self.thread_dir(id).join(STORE_FILE);
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&store)
            .map_err(|e| format!("error: could not open {}: {}", store.display(), e))?;
        file.write_all(format!("{}\n", line).as_bytes())
            .map_err(|e| format!("error: could not append to {}: {}", store.display(), e))
    }

    fn continuation_list(&self, id: &str) -> Result<Vec<String>, String> {
        let store = self.thread_dir(id).join(STORE_FILE);
        Ok(read_optional(&store)?
            .map(|text| {
                text.lines()
                    .filter(|line| !line.trim().is_empty())
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default())
    }

    fn bindings_list(&self) -> Result<Vec<BindingRow>, String> {
        let lookup = Lookup::read(&lookup_path(&self.root))?.unwrap_or_default();
        let mut rows: Vec<BindingRow> = lookup
            .entries()
            .iter()
            .map(|(actor, binding)| BindingRow {
                actor: actor.clone(),
                kind: BindingKind::Cloud,
                thread_id: binding.thread_id.clone().unwrap_or_default(),
                registered_at: binding.registered_at.clone().unwrap_or_default(),
            })
            .collect();
        rows.extend(
            self.worktree_bindings()?
                .into_iter()
                .map(|binding| BindingRow {
                    actor: binding.actor,
                    kind: BindingKind::Worktree,
                    thread_id: binding.thread_id,
                    registered_at: binding.registered_at,
                }),
        );
        rows.sort_by(|a, b| (&a.registered_at, &a.actor).cmp(&(&b.registered_at, &b.actor)));
        Ok(rows)
    }

    fn binding_set(&self, row: &BindingRow) -> Result<(), String> {
        match row.kind {
            BindingKind::Cloud => self.edit_lookup(|lookup| {
                lookup.set(
                    &row.actor,
                    CloudBinding::new(&row.thread_id, &row.registered_at),
                )
            }),
            BindingKind::Worktree => {
                let path = self.worktree_binding_path(&row.actor);
                ensure_parent(&path)?;
                let binding = WorktreeBinding {
                    actor: row.actor.clone(),
                    thread_id: row.thread_id.clone(),
                    registered_at: row.registered_at.clone(),
                };
                std::fs::write(&path, binding.to_json())
                    .map_err(|e| format!("error: could not write {}: {}", path.display(), e))
            }
        }
    }

    fn binding_remove(&self, actor: &str) -> Result<(), String> {
        self.edit_lookup(|lookup| lookup.remove(actor))?;
        remove_if_present(&self.worktree_binding_path(actor))
    }

    fn binding_remove_thread(&self, thread_id: &str) -> Result<(), String> {
        self.edit_lookup(|lookup| lookup.remove_thread(thread_id))?;
        for binding in self.worktree_bindings()? {
            if binding.thread_id == thread_id {
                remove_if_present(&self.worktree_binding_path(&binding.actor))?;
            }
        }
        Ok(())
    }

    fn events_append(&self, lines: &str) -> Result<(), String> {
        let queue = self.queue_path();
        ensure_parent(&queue)?;
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&queue)
            .and_then(|mut file| file.write_all(lines.as_bytes()))
            .map_err(|e| format!("error: could not append to {}: {}", queue.display(), e))
    }

    fn events_list(&self) -> Result<Option<Vec<String>>, String> {
        Ok(read_optional(&self.queue_path())?.map(|text| {
            text.split_inclusive('\n')
                .map(|line| line.strip_suffix('\n').unwrap_or(line).to_string())
                .collect()
        }))
    }

    fn watermark_get(&self) -> Result<u64, String> {
        match read_optional(&self.watermark_path())? {
            Some(text) => parse_watermark(&text),
            None => Ok(0),
        }
    }

    fn watermark_set(&self, processed_through: u64, updated_at: &str) -> Result<(), String> {
        let path = self.watermark_path();
        ensure_parent(&path)?;
        std::fs::write(&path, render_watermark(processed_through, updated_at))
            .map_err(|e| format!("error: could not write {}: {}", path.display(), e))
    }
}

fn is_reserved_top(name: &str) -> bool {
    name == THREADS_DIR || name == EVENTS_DIR || name == RECORDS_DIR || name == MANIFEST_FILE
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::thread::lookup::LOOKUP_FILE;

    fn tree() -> (tempfile::TempDir, FileTree) {
        let dir = tempfile::tempdir().unwrap();
        let tree = FileTree {
            root: dir.path().to_path_buf(),
            docs: false,
        };
        (dir, tree)
    }

    #[test]
    fn a_thread_with_no_mission_reads_back_with_none_and_set_mission_rewrites_the_index() {
        let (_dir, tree) = tree();
        tree.thread_create("1234abcd", None, None).unwrap();
        assert_eq!(
            std::fs::read_to_string(tree.thread_dir("1234abcd").join("index.md")).unwrap(),
            "# Thread 1234abcd\n"
        );
        assert_eq!(tree.thread_info("1234abcd"), ThreadInfo::default());

        tree.thread_set_mission("1234abcd", Some("M-X"), Some("missions/M-X.md"))
            .unwrap();
        assert_eq!(
            tree.thread_info("1234abcd"),
            ThreadInfo {
                mission_id: Some("M-X".to_string()),
                briefing_path: Some("missions/M-X.md".to_string()),
            }
        );
        tree.thread_set_mission("1234abcd", Some("M-Y"), None)
            .unwrap();
        assert_eq!(
            tree.thread_info("1234abcd"),
            ThreadInfo {
                mission_id: Some("M-Y".to_string()),
                briefing_path: None,
            }
        );
        tree.thread_set_mission("1234abcd", None, None).unwrap();
        assert_eq!(tree.thread_info("1234abcd"), ThreadInfo::default());
        assert_eq!(
            tree.thread_set_mission("ffffffff", None, None).unwrap_err(),
            "error: no such thread: ffffffff"
        );
    }

    #[test]
    fn thread_create_writes_index_and_empty_store_and_refuses_an_existing_thread() {
        let (_dir, tree) = tree();
        tree.thread_create("1234abcd", Some("M-X"), Some("missions/M-X.md"))
            .unwrap();
        let thread = tree.thread_dir("1234abcd");
        assert_eq!(
            std::fs::read_to_string(thread.join("index.md")).unwrap(),
            index_md("1234abcd", Some("M-X"), Some("missions/M-X.md"))
        );
        assert_eq!(
            std::fs::read_to_string(thread.join(STORE_FILE)).unwrap(),
            ""
        );
        assert_eq!(
            tree.thread_info("1234abcd"),
            ThreadInfo {
                mission_id: Some("M-X".to_string()),
                briefing_path: Some("missions/M-X.md".to_string()),
            }
        );
        let err = tree
            .thread_create("1234abcd", Some("M-X"), Some("missions/M-X.md"))
            .unwrap_err();
        assert_eq!(err, "error: threads/1234abcd already exists");
    }

    #[test]
    fn continuation_lines_append_in_order_and_an_absent_log_is_empty() {
        let (_dir, tree) = tree();
        assert!(tree.continuation_list("1234abcd").unwrap().is_empty());
        tree.thread_create("1234abcd", Some("M-X"), Some("m.md"))
            .unwrap();
        tree.continuation_append("1234abcd", "{\"n\":1}").unwrap();
        tree.continuation_append("1234abcd", "{\"n\":2}").unwrap();
        assert_eq!(
            tree.continuation_list("1234abcd").unwrap(),
            vec!["{\"n\":1}".to_string(), "{\"n\":2}".to_string()]
        );
    }

    #[test]
    fn bind_cloud_creates_the_lookup_when_absent() {
        let (_dir, tree) = tree();
        let row = BindingRow {
            actor: "cse_a".into(),
            kind: BindingKind::Cloud,
            thread_id: "1234abcd".into(),
            registered_at: "2026-10-03T00:00:00Z".into(),
        };
        tree.binding_set(&row).unwrap();
        assert_eq!(tree.bindings_list().unwrap(), vec![row]);
        assert!(tree.root.join(THREADS_DIR).join(LOOKUP_FILE).is_file());
        assert!(!worktree_bindings_dir(&tree.root).exists());
        tree.binding_remove_thread("1234abcd").unwrap();
        assert!(tree.bindings_list().unwrap().is_empty());
    }

    #[test]
    fn a_worktree_binding_is_one_file_for_its_actor() {
        let (_dir, tree) = tree();
        let row = |actor: &str, thread: &str| BindingRow {
            actor: actor.into(),
            kind: BindingKind::Worktree,
            thread_id: thread.into(),
            registered_at: "2026-10-03T00:00:00Z".into(),
        };
        tree.binding_set(&row("c-1/main", "1234abcd")).unwrap();
        tree.binding_set(&row("c-1/side", "1234abcd")).unwrap();
        tree.binding_set(&row("c-1/main", "5678efgh")).unwrap();
        let dir = worktree_bindings_dir(&tree.root);
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 2);
        assert!(dir.join("c-1%2Fmain.json").is_file());
        assert_eq!(
            tree.bindings_list().unwrap(),
            vec![row("c-1/main", "5678efgh"), row("c-1/side", "1234abcd")]
        );
        // The directory is not a thread.
        assert!(tree.thread_ids().unwrap().is_empty());
        tree.binding_remove_thread("1234abcd").unwrap();
        assert_eq!(
            tree.bindings_list().unwrap(),
            vec![row("c-1/main", "5678efgh")]
        );
        tree.binding_remove("c-1/main").unwrap();
        tree.binding_remove("c-1/main").unwrap();
        assert!(tree.bindings_list().unwrap().is_empty());
    }

    #[test]
    fn documents_round_trip_and_list_sorted_without_managed_parts() {
        let (_dir, tree) = tree();
        std::fs::write(tree.root.join(MANIFEST_FILE), "version = 1\n").unwrap();
        tree.doc_put("missions/b.md", "b").unwrap();
        tree.doc_put("missions/a.md", "a").unwrap();
        tree.doc_put("index.md", "i").unwrap();
        tree.thread_create("1234abcd", Some("M-X"), Some("m.md"))
            .unwrap();
        tree.events_append("{}\n").unwrap();
        assert_eq!(tree.doc_get("missions/a.md").unwrap().as_deref(), Some("a"));
        assert_eq!(tree.doc_get("missions").unwrap(), None);
        assert_eq!(
            tree.doc_list("").unwrap(),
            vec!["index.md", "missions/a.md", "missions/b.md"]
        );
        assert_eq!(
            tree.doc_list("missions/").unwrap(),
            vec!["missions/a.md", "missions/b.md"]
        );
        assert!(tree.doc_remove("missions/a.md").unwrap());
        assert!(!tree.doc_remove("missions/a.md").unwrap());
    }

    #[test]
    fn a_docs_tree_lists_every_top_level_name_and_a_files_tree_hides_the_managed_ones() {
        let dir = tempfile::tempdir().unwrap();
        let docs = FileTree {
            root: dir.path().to_path_buf(),
            docs: true,
        };
        docs.doc_put("threads/note.md", "t").unwrap();
        docs.doc_put("external-events/e.md", "e").unwrap();
        docs.doc_put("a.md", "a").unwrap();
        assert_eq!(
            docs.doc_list("").unwrap(),
            vec!["a.md", "external-events/e.md", "threads/note.md"]
        );
        let files = FileTree {
            root: dir.path().to_path_buf(),
            docs: false,
        };
        assert_eq!(files.doc_list("").unwrap(), vec!["a.md"]);
    }

    #[test]
    fn events_list_strips_terminators_and_the_watermark_defaults_to_zero() {
        let (_dir, tree) = tree();
        assert_eq!(tree.events_list().unwrap(), None);
        assert_eq!(tree.watermark_get().unwrap(), 0);
        tree.events_append("{\"n\":1}\n{\"n\":2}\n").unwrap();
        assert_eq!(
            tree.events_list().unwrap().unwrap(),
            vec!["{\"n\":1}".to_string(), "{\"n\":2}".to_string()]
        );
        tree.watermark_set(2, "2026-10-03T00:00:00Z").unwrap();
        assert_eq!(tree.watermark_get().unwrap(), 2);
    }
}
