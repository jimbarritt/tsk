use std::fmt;
use std::path::{Path, PathBuf};

use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::ledger::location::mint_token;

pub const LOOKUP_FILE: &str = "lookup-by-cloud-session.json";
/// The directory under `threads/` that holds the file ledger's bindings of local
/// worktrees, one file for each worktree actor. A directory with a leading dot is not a
/// thread. One file per actor keeps two clones that bind at the same time from changing
/// the same file, which would conflict in a rebase.
pub const WORKTREE_BINDINGS_DIR: &str = ".worktree-bindings";

/// One worktree binding in the file ledger: the content of its file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorktreeBinding {
    pub actor: String,
    pub thread_id: String,
    pub registered_at: String,
}

impl WorktreeBinding {
    pub fn parse(text: &str, path: &Path) -> Result<WorktreeBinding, String> {
        serde_json::from_str(text)
            .map_err(|e| format!("error: {} is not a worktree binding: {}", path.display(), e))
    }

    pub fn to_json(&self) -> String {
        let mut text = serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string());
        text.push('\n');
        text
    }
}

/// The file name of a worktree actor's binding: the actor key with every byte outside
/// `A-Za-z0-9._-` as `%XX`, and `.json`.
pub fn worktree_binding_file_name(actor: &str) -> String {
    let mut name = String::new();
    for byte in actor.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-') {
            name.push(byte as char);
        } else {
            name.push_str(&format!("%{:02X}", byte));
        }
    }
    name.push_str(".json");
    name
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CloudBinding {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub registered_at: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl CloudBinding {
    pub fn new(thread_id: &str, registered_at: &str) -> CloudBinding {
        CloudBinding {
            thread_id: Some(thread_id.to_string()),
            registered_at: Some(registered_at.to_string()),
            extra: serde_json::Map::new(),
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Lookup {
    entries: Vec<(String, CloudBinding)>,
}

pub fn lookup_path(wt: &Path) -> PathBuf {
    wt.join("threads").join(LOOKUP_FILE)
}

pub fn worktree_bindings_dir(wt: &Path) -> PathBuf {
    wt.join("threads").join(WORKTREE_BINDINGS_DIR)
}

impl Lookup {
    pub fn parse(text: &str, path: &Path) -> Result<Lookup, String> {
        serde_json::from_str(text).map_err(|e| {
            format!(
                "error: {} is not a cloud session lookup: {}",
                path.display(),
                e
            )
        })
    }

    pub fn read(path: &Path) -> Result<Option<Lookup>, String> {
        match std::fs::read_to_string(path) {
            Ok(text) => Lookup::parse(&text, path).map(Some),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(format!("error: could not read {}: {}", path.display(), e)),
        }
    }

    /// Every session and its binding, in file order.
    pub fn entries(&self) -> &[(String, CloudBinding)] {
        &self.entries
    }

    #[cfg(test)]
    pub fn thread_for(&self, session: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|(key, _)| key == session)
            .and_then(|(_, binding)| binding.thread_id.as_deref())
            .filter(|id| !id.is_empty())
    }

    pub fn set(&mut self, session: &str, binding: CloudBinding) {
        match self.entries.iter_mut().find(|(key, _)| key == session) {
            Some(entry) => entry.1 = binding,
            None => self.entries.push((session.to_string(), binding)),
        }
    }

    pub fn remove(&mut self, session: &str) {
        self.entries.retain(|(key, _)| key != session);
    }

    pub fn remove_thread(&mut self, thread_id: &str) {
        self.entries
            .retain(|(_, binding)| binding.thread_id.as_deref() != Some(thread_id));
    }

    pub fn to_json(&self) -> String {
        let mut text = serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string());
        text.push('\n');
        text
    }

    pub fn write(&self, path: &Path) -> Result<(), String> {
        let dir = path
            .parent()
            .ok_or_else(|| format!("error: {} has no parent directory", path.display()))?;
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("error: could not create {}: {}", dir.display(), e))?;
        let tmp = dir.join(format!(
            ".{}.tmp-{}",
            LOOKUP_FILE,
            mint_token(8, &path.to_string_lossy())
        ));
        std::fs::write(&tmp, self.to_json())
            .map_err(|e| format!("error: could not write {}: {}", tmp.display(), e))?;
        std::fs::rename(&tmp, path).map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            format!("error: could not replace {}: {}", path.display(), e)
        })
    }
}

impl Serialize for Lookup {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_map(self.entries.iter().map(|(k, v)| (k, v)))
    }
}

struct LookupVisitor;

impl<'de> Visitor<'de> for LookupVisitor {
    type Value = Lookup;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a JSON object keyed by cloud session id")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Lookup, A::Error> {
        let mut lookup = Lookup::default();
        while let Some((key, binding)) = map.next_entry::<String, CloudBinding>()? {
            lookup.set(&key, binding);
        }
        Ok(lookup)
    }
}

impl<'de> Deserialize<'de> for Lookup {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Lookup, D::Error> {
        deserializer.deserialize_map(LookupVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEXT: &str = "{\n  \"cse_b\": {\n    \"thread_id\": \"2222bbbb\",\n    \"registered_at\": \"2026-09-17T18:26:21Z\"\n  },\n  \"cse_a\": {\n    \"thread_id\": \"1111aaaa\",\n    \"registered_at\": \"2026-09-18T03:30:40Z\"\n  }\n}\n";

    fn parse(text: &str) -> Lookup {
        Lookup::parse(text, Path::new("lookup")).unwrap()
    }

    #[test]
    fn a_worktree_binding_file_name_encodes_the_actor_key() {
        assert_eq!(
            worktree_binding_file_name("tsk-1a2b3c4d/main"),
            "tsk-1a2b3c4d%2Fmain.json"
        );
        assert_eq!(
            worktree_binding_file_name("work.2_x/feat ure"),
            "work.2_x%2Ffeat%20ure.json"
        );
    }

    #[test]
    fn a_worktree_binding_round_trips() {
        let binding = WorktreeBinding {
            actor: "c-1/main".into(),
            thread_id: "1234abcd".into(),
            registered_at: "2026-10-17T00:00:00Z".into(),
        };
        assert_eq!(
            WorktreeBinding::parse(&binding.to_json(), Path::new("f")).unwrap(),
            binding
        );
        assert!(WorktreeBinding::parse("[]", Path::new("f")).is_err());
    }

    #[test]
    fn round_trips_in_the_jq_layout_and_keeps_key_order() {
        assert_eq!(parse(TEXT).to_json(), TEXT);
    }

    #[test]
    fn empty_lookup_is_an_empty_object() {
        assert_eq!(parse("{}").to_json(), "{}\n");
    }

    #[test]
    fn thread_for_finds_the_session() {
        let lookup = parse(TEXT);
        assert_eq!(lookup.thread_for("cse_a"), Some("1111aaaa"));
        assert_eq!(lookup.thread_for("cse_x"), None);
    }

    #[test]
    fn set_replaces_in_place_and_appends_new_sessions() {
        let mut lookup = parse(TEXT);
        lookup.set(
            "cse_b",
            CloudBinding::new("3333cccc", "2026-10-03T00:00:00Z"),
        );
        lookup.set(
            "cse_c",
            CloudBinding::new("4444dddd", "2026-10-03T00:00:01Z"),
        );
        let json = lookup.to_json();
        let b = json.find("cse_b").unwrap();
        let a = json.find("cse_a").unwrap();
        let c = json.find("cse_c").unwrap();
        assert!(b < a && a < c, "{}", json);
        assert_eq!(lookup.thread_for("cse_b"), Some("3333cccc"));
    }

    #[test]
    fn remove_deletes_one_session() {
        let mut lookup = parse(TEXT);
        lookup.remove("cse_b");
        assert_eq!(lookup.thread_for("cse_b"), None);
        assert_eq!(lookup.thread_for("cse_a"), Some("1111aaaa"));
    }

    #[test]
    fn remove_thread_deletes_every_session_bound_to_it() {
        let mut lookup = parse(TEXT);
        lookup.set(
            "cse_c",
            CloudBinding::new("1111aaaa", "2026-10-03T00:00:00Z"),
        );
        lookup.remove_thread("1111aaaa");
        assert_eq!(lookup.thread_for("cse_a"), None);
        assert_eq!(lookup.thread_for("cse_c"), None);
        assert_eq!(lookup.thread_for("cse_b"), Some("2222bbbb"));
    }

    #[test]
    fn keeps_unknown_fields() {
        let lookup = parse("{\"cse_a\":{\"thread_id\":\"1111aaaa\",\"note\":\"x\"}}");
        assert!(lookup.to_json().contains("\"note\": \"x\""));
    }

    #[test]
    fn rejects_a_non_object() {
        assert!(Lookup::parse("[]", Path::new("lookup")).is_err());
    }

    #[test]
    fn write_replaces_the_file_and_leaves_no_temporary_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("threads").join(LOOKUP_FILE);
        parse(TEXT).write(&path).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), TEXT);
        let names: Vec<_> = std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, vec![std::ffi::OsString::from(LOOKUP_FILE)]);
    }
}
