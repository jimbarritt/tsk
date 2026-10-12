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
use crate::ledger::{fetch, push};
use crate::thread::entry::STORE_FILE;
use crate::thread::lookup::{lookup_path, CloudBinding, Lookup};
use crate::thread::ops::{index_md, mission_link};

use super::{LedgerStore, PushReport, RefreshOutcome};

const THREADS_DIR: &str = "threads";

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
        Ok(FileTree { root })
    }

    fn describe(&self, rel: &str) -> String {
        match self.hint() {
            Some(root) => root.join(rel).display().to_string(),
            None => rel.to_string(),
        }
    }
}

impl LedgerStore for FilesStore {
    fn refresh(&self) -> Result<RefreshOutcome, String> {
        let outcome = fetch::fetch(&self.repo, &self.state_root)?;
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
        Ok(self.tree()?.doc_path(path).is_file())
    }

    fn thread_exists(&self, id: &str) -> Result<bool, String> {
        Ok(self.tree()?.thread_dir(id).is_dir())
    }

    fn thread_create(&self, id: &str, briefing_path: &str) -> Result<(), String> {
        self.tree()?.thread_create(id, briefing_path)
    }

    fn thread_delete(&self, id: &str) -> Result<(), String> {
        self.tree()?.thread_delete(id)
    }

    fn thread_ids(&self) -> Result<Vec<String>, String> {
        self.tree()?.thread_ids()
    }

    fn thread_briefing(&self, id: &str) -> Result<Option<String>, String> {
        Ok(self.tree()?.thread_briefing(id))
    }

    fn continuation_append(&self, id: &str, line: &str) -> Result<(), String> {
        self.tree()?.continuation_append(id, line)
    }

    fn continuation_list(&self, id: &str) -> Result<Vec<String>, String> {
        self.tree()?.continuation_list(id)
    }

    fn cloud_lookup(&self) -> Result<Lookup, String> {
        self.tree()?.cloud_lookup()
    }

    fn cloud_bind(
        &self,
        session: &str,
        thread_id: &str,
        registered_at: &str,
    ) -> Result<(), String> {
        self.tree()?.cloud_bind(session, thread_id, registered_at)
    }

    fn cloud_unbind(&self, session: &str) -> Result<(), String> {
        self.tree()?.cloud_unbind(session)
    }

    fn cloud_unbind_thread(&self, thread_id: &str) -> Result<(), String> {
        self.tree()?.cloud_unbind_thread(thread_id)
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

    fn describe_document(&self, path: &str) -> String {
        self.describe(path)
    }

    fn describe_thread(&self, id: &str) -> String {
        self.describe(&format!("{}/{}/{}", THREADS_DIR, id, STORE_FILE))
    }

    fn describe_threads(&self) -> String {
        self.describe(THREADS_DIR)
    }
}

/// File operations on a materialised ledger worktree.
pub struct FileTree {
    pub root: PathBuf,
}

fn read_optional(path: &Path) -> Result<Option<String>, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("error: could not read {}: {}", path.display(), e)),
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
    fn doc_path(&self, path: &str) -> PathBuf {
        self.root.join(path)
    }

    fn thread_dir(&self, id: &str) -> PathBuf {
        self.root.join(THREADS_DIR).join(id)
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

    fn doc_get(&self, path: &str) -> Result<Option<String>, String> {
        let file = self.doc_path(path);
        if !file.is_file() {
            return Ok(None);
        }
        read_optional(&file)
    }

    fn doc_put(&self, path: &str, body: &str) -> Result<(), String> {
        let file = self.doc_path(path);
        ensure_parent(&file)?;
        std::fs::write(&file, body)
            .map_err(|e| format!("error: could not write {}: {}", file.display(), e))
    }

    fn doc_remove(&self, path: &str) -> Result<bool, String> {
        let file = self.doc_path(path);
        if !file.is_file() {
            return Ok(false);
        }
        std::fs::remove_file(&file)
            .map_err(|e| format!("error: could not remove {}: {}", file.display(), e))?;
        Ok(true)
    }

    fn doc_list(&self, prefix: &str) -> Result<Vec<String>, String> {
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
            let name = entry.file_name().to_string_lossy().into_owned();
            let path = if rel.is_empty() {
                name.clone()
            } else {
                format!("{}/{}", rel, name)
            };
            if name == ".git" || (rel.is_empty() && is_reserved_top(&name)) {
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

    fn thread_create(&self, id: &str, briefing_path: &str) -> Result<(), String> {
        let dir = self.thread_dir(id);
        if dir.exists() {
            return Err(format!("error: threads/{} already exists", id));
        }
        std::fs::create_dir_all(&dir)
            .map_err(|e| format!("error: could not create {}: {}", dir.display(), e))?;
        let index = dir.join("index.md");
        std::fs::write(&index, index_md(id, briefing_path))
            .map_err(|e| format!("error: could not write {}: {}", index.display(), e))?;
        let store = dir.join(STORE_FILE);
        std::fs::write(&store, "")
            .map_err(|e| format!("error: could not write {}: {}", store.display(), e))
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

    fn thread_briefing(&self, id: &str) -> Option<String> {
        std::fs::read_to_string(self.thread_dir(id).join("index.md"))
            .ok()
            .map(|text| mission_link(&text))
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

    fn cloud_lookup(&self) -> Result<Lookup, String> {
        Ok(Lookup::read(&lookup_path(&self.root))?.unwrap_or_default())
    }

    fn cloud_bind(
        &self,
        session: &str,
        thread_id: &str,
        registered_at: &str,
    ) -> Result<(), String> {
        let path = lookup_path(&self.root);
        let mut lookup = Lookup::read(&path)?.unwrap_or_default();
        lookup.set(session, CloudBinding::new(thread_id, registered_at));
        lookup.write(&path)
    }

    fn cloud_unbind(&self, session: &str) -> Result<(), String> {
        let path = lookup_path(&self.root);
        let mut lookup = Lookup::read(&path)?.unwrap_or_default();
        lookup.remove(session);
        lookup.write(&path)
    }

    fn cloud_unbind_thread(&self, thread_id: &str) -> Result<(), String> {
        let path = lookup_path(&self.root);
        if let Some(mut lookup) = Lookup::read(&path)? {
            lookup.remove_thread(thread_id);
            lookup.write(&path)?;
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
    name == THREADS_DIR || name == EVENTS_DIR || name == MANIFEST_FILE
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::thread::lookup::LOOKUP_FILE;

    fn tree() -> (tempfile::TempDir, FileTree) {
        let dir = tempfile::tempdir().unwrap();
        let tree = FileTree {
            root: dir.path().to_path_buf(),
        };
        (dir, tree)
    }

    #[test]
    fn thread_create_writes_index_and_empty_store_and_refuses_an_existing_thread() {
        let (_dir, tree) = tree();
        tree.thread_create("1234abcd", "missions/M-X.md").unwrap();
        let thread = tree.thread_dir("1234abcd");
        assert_eq!(
            std::fs::read_to_string(thread.join("index.md")).unwrap(),
            index_md("1234abcd", "missions/M-X.md")
        );
        assert_eq!(
            std::fs::read_to_string(thread.join(STORE_FILE)).unwrap(),
            ""
        );
        assert_eq!(
            tree.thread_briefing("1234abcd").as_deref(),
            Some("missions/M-X.md")
        );
        let err = tree
            .thread_create("1234abcd", "missions/M-X.md")
            .unwrap_err();
        assert_eq!(err, "error: threads/1234abcd already exists");
    }

    #[test]
    fn continuation_lines_append_in_order_and_an_absent_log_is_empty() {
        let (_dir, tree) = tree();
        assert!(tree.continuation_list("1234abcd").unwrap().is_empty());
        tree.thread_create("1234abcd", "m.md").unwrap();
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
        tree.cloud_bind("cse_a", "1234abcd", "2026-10-03T00:00:00Z")
            .unwrap();
        let lookup = tree.cloud_lookup().unwrap();
        assert_eq!(lookup.thread_for("cse_a"), Some("1234abcd"));
        assert!(tree.root.join(THREADS_DIR).join(LOOKUP_FILE).is_file());
        tree.cloud_unbind_thread("1234abcd").unwrap();
        assert_eq!(tree.cloud_lookup().unwrap().thread_for("cse_a"), None);
    }

    #[test]
    fn documents_round_trip_and_list_sorted_without_managed_parts() {
        let (_dir, tree) = tree();
        std::fs::write(tree.root.join(MANIFEST_FILE), "version = 1\n").unwrap();
        tree.doc_put("missions/b.md", "b").unwrap();
        tree.doc_put("missions/a.md", "a").unwrap();
        tree.doc_put("index.md", "i").unwrap();
        tree.thread_create("1234abcd", "m.md").unwrap();
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
