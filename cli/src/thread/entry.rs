use std::path::Path;

use serde::{Deserialize, Serialize};

pub const STORE_FILE: &str = "continuation-state.jsonl";

#[derive(Serialize)]
pub struct NewEntry<'a> {
    pub mission_link: &'a str,
    pub task_id: &'a str,
    pub whats_next: &'a str,
    pub git: GitState<'a>,
    pub timestamp: &'a str,
    pub written_by: &'a str,
}

#[derive(Serialize)]
pub struct GitState<'a> {
    pub ledger: LedgerState<'a>,
    pub code: CodeState<'a>,
}

#[derive(Serialize)]
pub struct LedgerState<'a> {
    pub commit: &'a str,
}

#[derive(Serialize)]
pub struct CodeState<'a> {
    pub r#ref: &'a str,
    pub commit: &'a str,
}

impl NewEntry<'_> {
    pub fn to_line(&self) -> String {
        let mut line = serde_json::to_string(self).unwrap_or_default();
        line.push('\n');
        line
    }
}

#[derive(Debug, Default, Deserialize)]
pub struct Entry {
    pub whats_next: Option<String>,
    pub timestamp: Option<String>,
    pub written_by: Option<String>,
}

#[derive(Debug)]
pub struct StoredEntry {
    pub raw: String,
    pub entry: Entry,
}

pub fn parse_store(text: &str, store: &Path) -> Result<Vec<StoredEntry>, String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .enumerate()
        .map(|(n, line)| {
            serde_json::from_str::<serde_json::Value>(line)
                .and_then(serde_json::from_value::<Entry>)
                .map(|entry| StoredEntry {
                    raw: line.to_string(),
                    entry,
                })
                .map_err(|e| {
                    format!(
                        "error: entry {} in {} is not a continuation state entry: {}",
                        n + 1,
                        store.display(),
                        e
                    )
                })
        })
        .collect()
}

pub fn read_store(store: &Path) -> Result<Vec<StoredEntry>, String> {
    match std::fs::read_to_string(store) {
        Ok(text) => parse_store(&text, store),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(format!("error: could not read {}: {}", store.display(), e)),
    }
}

pub fn append(store: &Path, entry: &NewEntry) -> Result<(), String> {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(store)
        .map_err(|e| format!("error: could not open {}: {}", store.display(), e))?;
    file.write_all(entry.to_line().as_bytes())
        .map_err(|e| format!("error: could not append to {}: {}", store.display(), e))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_entry() -> NewEntry<'static> {
        NewEntry {
            mission_link: "missions/operational/M-X.md",
            task_id: "T-05",
            whats_next: "Write the \"thread\" commands",
            git: GitState {
                ledger: LedgerState {
                    commit: "1111111111111111111111111111111111111111",
                },
                code: CodeState {
                    r#ref: "refs/heads/feature/x",
                    commit: "2222222222222222222222222222222222222222",
                },
            },
            timestamp: "2026-10-03T10:00:00Z",
            written_by: "urn:tsk:worktree:.git",
        }
    }

    #[test]
    fn new_entry_is_one_compact_line_in_field_order() {
        assert_eq!(
            new_entry().to_line(),
            "{\"mission_link\":\"missions/operational/M-X.md\",\"task_id\":\"T-05\",\
             \"whats_next\":\"Write the \\\"thread\\\" commands\",\
             \"git\":{\"ledger\":{\"commit\":\"1111111111111111111111111111111111111111\"},\
             \"code\":{\"ref\":\"refs/heads/feature/x\",\"commit\":\"2222222222222222222222222222222222222222\"}},\
             \"timestamp\":\"2026-10-03T10:00:00Z\",\"written_by\":\"urn:tsk:worktree:.git\"}\n"
        );
    }

    #[test]
    fn new_entry_writes_no_flat_commit_fields() {
        let line = new_entry().to_line();
        for old in ["commit_on_bootstrap", "commit_on_ledger", "commit_on_main"] {
            assert!(!line.contains(old), "{}", old);
        }
    }

    #[test]
    fn keeps_entries_as_stored() {
        let first = "{\"whats_next\":\"a\",\"git\":{\"ledger\":{\"commit\":\"bbb\"}},\"written_by\":\"urn:x\"}";
        let second = "{\"whats_next\":\"b\",\"written_by\":\"urn:y\"}";
        let entries = parse_store(&format!("{}\n{}\n", first, second), Path::new("s")).unwrap();
        assert_eq!(entries[0].raw, first);
        assert_eq!(entries[0].entry.written_by.as_deref(), Some("urn:x"));
        assert_eq!(entries[1].raw, second);
        assert_eq!(entries[1].entry.whats_next.as_deref(), Some("b"));
    }

    #[test]
    fn keeps_each_raw_line_and_skips_blank_lines() {
        let entries = parse_store(
            "{\"whats_next\":\"a\"}\n\n{\"whats_next\":\"b\",\"timestamp\":\"t\"}\n\n",
            Path::new("s"),
        )
        .unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[1].raw, "{\"whats_next\":\"b\",\"timestamp\":\"t\"}");
        assert_eq!(entries[1].entry.timestamp.as_deref(), Some("t"));
    }

    #[test]
    fn rejects_a_line_that_is_not_json() {
        let err = parse_store("{\"a\":1}\nnot json\n", Path::new("store")).unwrap_err();
        assert!(err.contains("entry 2 in store"), "{}", err);
    }

    #[test]
    fn read_store_treats_an_absent_file_as_empty() {
        let dir = tempfile::tempdir().unwrap();
        assert!(read_store(&dir.path().join(STORE_FILE)).unwrap().is_empty());
    }
}
