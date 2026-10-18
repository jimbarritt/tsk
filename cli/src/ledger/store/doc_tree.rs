//! File operations on the documents worktree: the checked-out `tsk/docs` branch of a
//! ledger. Every top-level name is a document, and a file name that is not valid UTF-8
//! is an error. The `.git` entry is not a document.

use std::path::{Path, PathBuf};

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

impl DocTree {
    pub(crate) fn doc_path(&self, path: &str) -> PathBuf {
        self.root.join(path)
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
    fn documents_round_trip_and_list_sorted() {
        let (_dir, tree) = tree();
        tree.doc_put("missions/b.md", "b").unwrap();
        tree.doc_put("missions/a.md", "a").unwrap();
        tree.doc_put("index.md", "i").unwrap();
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
    fn every_top_level_name_is_a_document() {
        let (_dir, tree) = tree();
        tree.doc_put("threads/note.md", "t").unwrap();
        tree.doc_put("external-events/e.md", "e").unwrap();
        tree.doc_put("a.md", "a").unwrap();
        assert_eq!(
            tree.doc_list("").unwrap(),
            vec!["a.md", "external-events/e.md", "threads/note.md"]
        );
    }
}
