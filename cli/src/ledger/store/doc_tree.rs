//! File operations on the documents worktree: the checked-out `tsk/docs` branch of a
//! ledger. Every top-level name is a document, and a file name that is not valid UTF-8
//! is an error. The `.git` entry is not a document.

use std::path::{Path, PathBuf};

use crate::ledger::doc_id;

/// A materialised documents worktree.
pub struct DocTree {
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

/// How many times a minted document id is drawn again when the ledger already uses it.
const MINT_ATTEMPTS: usize = 100;

impl DocTree {
    pub(crate) fn doc_path(&self, path: &str) -> PathBuf {
        self.root.join(path)
    }

    /// The path of the document a reference names, or `None`. A reference is the path with
    /// the id, the path without it, or the id alone (`docs/domain/ledger-layout.md`).
    pub(crate) fn resolve(&self, reference: &str) -> Result<Option<String>, String> {
        if self.doc_path(reference).is_file() {
            return Ok(Some(reference.to_string()));
        }
        if doc_id::is_id(reference) {
            return Ok(self.paths_with_id(reference)?.into_iter().next());
        }
        if let (Some(id), bare) = doc_id::split_id(reference) {
            // A document the ledger has not renamed yet (layout 13) has the name without the
            // id, and the id its path will give it.
            let legacy = self.doc_path(&bare).is_file() && doc_id::derived_id(&bare) == id;
            return Ok(legacy.then_some(bare));
        }
        // A path without its id names the file of that directory that has one.
        let dir = match reference.rfind('/') {
            Some(at) => &reference[..at],
            None => "",
        };
        let mut found = Vec::new();
        for path in self.list_dir(dir)? {
            if doc_id::split_id(&path).1 == reference {
                found.push(path);
            }
        }
        found.sort();
        Ok(found.into_iter().next())
    }

    fn paths_with_id(&self, id: &str) -> Result<Vec<String>, String> {
        let mut all = self.doc_list("")?;
        all.retain(|path| doc_id::id_of(path).as_deref() == Some(id));
        Ok(all)
    }

    /// The files directly in `dir` (a path relative to the root, empty for the root), as
    /// paths from the root.
    fn list_dir(&self, dir: &str) -> Result<Vec<String>, String> {
        let at = self.doc_path(dir);
        let Ok(entries) = std::fs::read_dir(&at) else {
            return Ok(Vec::new());
        };
        let mut found = Vec::new();
        for entry in entries.flatten() {
            let Some(name) = entry.file_name().to_str().map(str::to_string) else {
                continue;
            };
            if entry.path().is_file() {
                found.push(if dir.is_empty() {
                    name
                } else {
                    format!("{}/{}", dir, name)
                });
            }
        }
        Ok(found)
    }

    pub(crate) fn doc_get(&self, reference: &str) -> Result<Option<String>, String> {
        match self.resolve(reference)? {
            Some(path) => read_optional(&self.doc_path(&path)),
            None => Ok(None),
        }
    }

    /// Write a document and return its path. A reference that names a document writes that
    /// file. A new path with an id is written as it is, when no other document has the id.
    /// A new path without one gets a minted id.
    pub(crate) fn doc_put(&self, reference: &str, body: &str) -> Result<String, String> {
        let mut replaced = None;
        let path = match self.resolve(reference)? {
            // A file with no id (a seed file) gets the id of its path when it is written.
            Some(path) if doc_id::id_of(&path).is_none() => {
                let (_, named) = doc_id::upgraded(&path);
                replaced = Some(path);
                named
            }
            Some(path) => path,
            None if doc_id::is_id(reference) => {
                return Err(format!("error: no such document: {}", reference));
            }
            None => match doc_id::id_of(reference) {
                Some(id) => {
                    if !self.paths_with_id(&id)?.is_empty() {
                        return Err(format!(
                            "error: the document id {} is already used by another document",
                            id
                        ));
                    }
                    reference.to_string()
                }
                None => doc_id::with_id(reference, &self.mint_id()?),
            },
        };
        let file = self.doc_path(&path);
        ensure_parent(&file)?;
        std::fs::write(&file, body)
            .map_err(|e| format!("error: could not write {}: {}", file.display(), e))?;
        if let Some(old) = replaced {
            if old != path {
                std::fs::remove_file(self.doc_path(&old))
                    .map_err(|e| format!("error: could not remove {}: {}", old, e))?;
            }
        }
        Ok(path)
    }

    /// An id no document of the tree has.
    fn mint_id(&self) -> Result<String, String> {
        let taken: std::collections::HashSet<String> = self
            .doc_list("")?
            .iter()
            .filter_map(|path| doc_id::id_of(path))
            .collect();
        for _ in 0..MINT_ATTEMPTS {
            let id = crate::ledger::location::mint_token(doc_id::ID_LEN, "document");
            if !taken.contains(&id) {
                return Ok(id);
            }
        }
        Err(format!(
            "error: no free document id after {} attempts; is {} intact?",
            MINT_ATTEMPTS,
            self.root.display()
        ))
    }

    /// Rename the document `from` names to `to`, which keeps the id. `to` may carry the id
    /// or not. Returns the old and the new path.
    pub(crate) fn doc_mv(&self, from: &str, to: &str) -> Result<(String, String), String> {
        let old = self
            .resolve(from)?
            .ok_or_else(|| format!("error: no such document: {}", from))?;
        let new = match (doc_id::id_of(&old), doc_id::id_of(to)) {
            (Some(id), None) => doc_id::with_id(to, &id),
            (Some(id), Some(given)) if id != given => {
                return Err(format!(
                    "error: a document keeps its id: {} has {}, and {} has {}",
                    old, id, to, given
                ));
            }
            _ => to.to_string(),
        };
        if old == new {
            return Err(format!("error: {} is already at {}", old, new));
        }
        if self.doc_path(&new).exists() {
            return Err(format!("error: {} exists; move or remove it first", new));
        }
        let target = self.doc_path(&new);
        ensure_parent(&target)?;
        std::fs::rename(self.doc_path(&old), &target)
            .map_err(|e| format!("error: could not rename {}: {}", old, e))?;
        Ok((old, new))
    }

    pub(crate) fn doc_remove(&self, reference: &str) -> Result<bool, String> {
        let Some(path) = self.resolve(reference)? else {
            return Ok(false);
        };
        let file = self.doc_path(&path);
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

    /// True when the file is one of the two seed files of a new documents branch, which have
    /// no id.
    pub(crate) fn is_seed_file(&self, path: &str) -> bool {
        matches!(read_optional(&self.doc_path(path)), Ok(Some(body)) if crate::ledger::docs::is_seed(path, &body))
    }

    /// Give every document that has no id the id of its path (`doc_id::derived_id`), as the
    /// layout 14 upgrade does for the links. The two seed files of a new branch stay as they
    /// are. A document that has an id is left alone, so the step can run again. Returns the
    /// number of files renamed.
    pub(crate) fn heal_ids(&self) -> Result<usize, String> {
        let mut renamed = 0;
        for path in self.doc_list("")? {
            if doc_id::id_of(&path).is_some() {
                continue;
            }
            if self.is_seed_file(&path) {
                continue;
            }
            let (_, new) = doc_id::upgraded(&path);
            if self.doc_path(&new).exists() {
                return Err(format!(
                    "error: cannot give {} its id: {} exists",
                    path, new
                ));
            }
            let target = self.doc_path(&new);
            ensure_parent(&target)?;
            std::fs::rename(self.doc_path(&path), &target)
                .map_err(|e| format!("error: could not rename {}: {}", path, e))?;
            renamed += 1;
        }
        Ok(renamed)
    }

    fn walk(&self, dir: &Path, rel: &str, found: &mut Vec<String>) -> Result<(), String> {
        let entries = std::fs::read_dir(dir)
            .map_err(|e| format!("error: could not read {}: {}", dir.display(), e))?;
        for entry in entries {
            let entry =
                entry.map_err(|e| format!("error: could not read {}: {}", dir.display(), e))?;
            let Some(name) = entry.file_name().to_str().map(str::to_string) else {
                return Err(format!(
                    "error: {} holds a file whose name is not valid UTF-8: {}",
                    dir.display(),
                    entry.file_name().to_string_lossy()
                ));
            };
            let path = if rel.is_empty() {
                name.clone()
            } else {
                format!("{}/{}", rel, name)
            };
            if name == ".git" {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree() -> (tempfile::TempDir, DocTree) {
        let dir = tempfile::tempdir().unwrap();
        let tree = DocTree {
            root: dir.path().to_path_buf(),
        };
        (dir, tree)
    }

    #[test]
    fn documents_round_trip_by_path_and_list_sorted() {
        let (_dir, tree) = tree();
        let b = tree.doc_put("missions/b.md", "b").unwrap();
        let a = tree.doc_put("missions/a.md", "a").unwrap();
        let i = tree.doc_put("index.md", "i").unwrap();
        // The id is before the extension, and the path without it names the same file.
        assert!(
            a.starts_with("missions/a.")
                && a.ends_with(".md")
                && a.len() == "missions/a.md".len() + 9
        );
        assert_eq!(tree.doc_get("missions/a.md").unwrap().as_deref(), Some("a"));
        assert_eq!(tree.doc_get(&a).unwrap().as_deref(), Some("a"));
        let id = doc_id::id_of(&a).unwrap();
        assert_eq!(tree.doc_get(&id).unwrap().as_deref(), Some("a"));
        assert_eq!(tree.doc_get("missions").unwrap(), None);
        let mut expected = vec![i, a.clone(), b];
        expected.sort();
        assert_eq!(tree.doc_list("").unwrap(), expected);
        assert_eq!(tree.doc_list("missions/").unwrap().len(), 2);
        assert!(tree.doc_remove("missions/a.md").unwrap());
        assert!(!tree.doc_remove("missions/a.md").unwrap());
        assert!(!tree.doc_remove(&a).unwrap());
    }

    #[test]
    fn a_second_put_by_the_path_without_the_id_rewrites_the_same_file() {
        let (_dir, tree) = tree();
        let first = tree.doc_put("m/report.md", "one").unwrap();
        let second = tree.doc_put("m/report.md", "two").unwrap();
        let by_id = tree
            .doc_put(&doc_id::id_of(&first).unwrap(), "three")
            .unwrap();
        assert_eq!(first, second);
        assert_eq!(first, by_id);
        assert_eq!(tree.doc_list("").unwrap(), vec![first.clone()]);
        assert_eq!(tree.doc_get(&first).unwrap().as_deref(), Some("three"));
        assert!(tree.doc_put("m/none00aa", "x").is_ok());
        assert!(tree.doc_put("deadbeef", "x").is_err());
    }

    #[test]
    fn a_path_given_with_an_id_is_written_as_it_is_unless_the_id_is_taken() {
        let (_dir, tree) = tree();
        let path = tree.doc_put("a/x.3f2a9c1e.md", "x").unwrap();
        assert_eq!(path, "a/x.3f2a9c1e.md");
        let err = tree.doc_put("b/y.3f2a9c1e.md", "y").unwrap_err();
        assert!(err.contains("already used"), "{}", err);
    }

    #[test]
    fn a_move_keeps_the_id() {
        let (_dir, tree) = tree();
        let old = tree.doc_put("a/x.md", "x").unwrap();
        let id = doc_id::id_of(&old).unwrap();
        let (from, to) = tree.doc_mv("a/x.md", "b/z.md").unwrap();
        assert_eq!(
            (from.as_str(), to.as_str()),
            (old.as_str(), format!("b/z.{}.md", id).as_str())
        );
        assert_eq!(tree.doc_get(&id).unwrap().as_deref(), Some("x"));
        let err = tree.doc_mv(&id, "b/w.00000000.md").unwrap_err();
        assert!(err.contains("keeps its id"), "{}", err);
        assert!(tree.doc_mv("nope.md", "q.md").is_err());
    }

    #[test]
    fn healing_gives_a_file_the_id_of_its_path_and_runs_again_with_no_change() {
        let (dir, tree) = tree();
        std::fs::create_dir_all(dir.path().join("m")).unwrap();
        std::fs::write(dir.path().join("m/r.md"), "r").unwrap();
        std::fs::write(
            dir.path().join("README.md"),
            crate::ledger::docs::README_TEXT,
        )
        .unwrap();
        assert_eq!(tree.heal_ids().unwrap(), 1);
        let id = doc_id::derived_id("m/r.md");
        assert_eq!(
            tree.doc_list("").unwrap(),
            vec!["README.md".to_string(), format!("m/r.{}.md", id)]
        );
        assert_eq!(tree.heal_ids().unwrap(), 0);
    }

    #[test]
    fn every_top_level_name_is_a_document() {
        let (_dir, tree) = tree();
        let t = tree.doc_put("threads/note.md", "t").unwrap();
        let e = tree.doc_put("external-events/e.md", "e").unwrap();
        let a = tree.doc_put("a.md", "a").unwrap();
        let mut expected = vec![a, e, t];
        expected.sort();
        assert_eq!(tree.doc_list("").unwrap(), expected);
    }
}
