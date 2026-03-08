use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Priority
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Priority {
    #[serde(rename = "BG")]
    Background,
    #[serde(rename = "PRIO")]
    Priority,
    #[serde(rename = "INC")]
    Incident,
}

impl Priority {
    pub fn full_name(&self) -> &'static str {
        match self {
            Priority::Background => "background",
            Priority::Priority => "priority",
            Priority::Incident => "incident",
        }
    }

    pub fn abbrev(&self) -> &'static str {
        match self {
            Priority::Background => "BG",
            Priority::Priority => "PRIO",
            Priority::Incident => "INC",
        }
    }
}

impl std::fmt::Display for Priority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.abbrev())
    }
}

impl std::str::FromStr for Priority {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "BG" => Ok(Priority::Background),
            "PRIO" => Ok(Priority::Priority),
            "INC" => Ok(Priority::Incident),
            _ => Err(format!("Invalid priority '{}'. Use BG, PRIO, or INC", s)),
        }
    }
}

// ---------------------------------------------------------------------------
// Thread state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThreadState {
    Active,
    Paused,
}

impl std::fmt::Display for ThreadState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ThreadState::Active => write!(f, "active"),
            ThreadState::Paused => write!(f, "paused"),
        }
    }
}

// ---------------------------------------------------------------------------
// Thread model
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Thread {
    pub hash: String,
    pub short_hash: String,
    pub slug: String,
    pub state: ThreadState,
    pub priority: Priority,
    pub description: String,
}

// ---------------------------------------------------------------------------
// Event types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadStartedEvent {
    pub event: String,
    pub hash: String,
    pub slug: String,
    pub priority: Priority,
    pub description: String,
    pub timestamp: u64,
}

// ---------------------------------------------------------------------------
// JSON-RPC 2.0 types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: u64,
    pub method: String,
    pub params: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
}

impl JsonRpcResponse {
    pub fn success(id: u64, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: u64, code: i64, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.into(),
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// Thread ID generation
// ---------------------------------------------------------------------------

/// Returns (full_sha256_hex, short_7_char_hash) for the given slug.
pub fn thread_hash(slug: &str) -> (String, String) {
    let mut hasher = Sha256::new();
    hasher.update(slug.as_bytes());
    let result = hasher.finalize();
    let full: String = result.iter().map(|b| format!("{:02x}", b)).collect();
    let short = full[..7].to_string();
    (full, short)
}

// ---------------------------------------------------------------------------
// Storage paths
// ---------------------------------------------------------------------------

pub fn tsk_dir(project_root: &Path) -> PathBuf {
    project_root.join("tsk")
}

pub fn event_log_dir(project_root: &Path) -> PathBuf {
    tsk_dir(project_root).join("event-log")
}

pub fn event_log_path(project_root: &Path) -> PathBuf {
    event_log_dir(project_root).join("events.ndjson")
}

pub fn threads_dir(project_root: &Path) -> PathBuf {
    tsk_dir(project_root).join("threads")
}

pub fn index_path(project_root: &Path) -> PathBuf {
    threads_dir(project_root).join("index.json")
}

pub fn thread_dir(project_root: &Path, short_hash: &str, slug: &str) -> PathBuf {
    threads_dir(project_root).join(format!("{}-{}", short_hash, slug))
}

// ---------------------------------------------------------------------------
// Socket path
// ---------------------------------------------------------------------------

/// Derives a stable socket path from the project root directory.
/// Uses first 8 chars of SHA-256 of the path string, so multiple projects
/// can have daemons running simultaneously.
pub fn socket_path(project_root: &Path) -> PathBuf {
    let mut hasher = Sha256::new();
    hasher.update(project_root.to_string_lossy().as_bytes());
    let result = hasher.finalize();
    let hash: String = result.iter().map(|b| format!("{:02x}", b)).collect();
    PathBuf::from(format!("/tmp/tsk-{}.sock", &hash[..8]))
}

// ---------------------------------------------------------------------------
// Client helper
// ---------------------------------------------------------------------------

/// Connect to the daemon socket, send a JSON-RPC request, return the result.
pub fn send_request(
    socket: &Path,
    method: &str,
    params: serde_json::Value,
) -> Result<serde_json::Value, String> {
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixStream;

    let mut stream = UnixStream::connect(socket).map_err(|_| {
        "tskd is not running. Start it with: tskd".to_string()
    })?;

    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .map_err(|e| format!("Failed to set timeout: {}", e))?;

    let request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: 1,
        method: method.to_string(),
        params,
    };

    let mut line =
        serde_json::to_string(&request).map_err(|e| format!("Serialisation error: {}", e))?;
    line.push('\n');

    stream
        .write_all(line.as_bytes())
        .map_err(|e| format!("Write error: {}", e))?;

    let reader = BufReader::new(&stream);
    let response_line = reader
        .lines()
        .next()
        .ok_or("No response from daemon")?
        .map_err(|e| format!("Read error: {}", e))?;

    let response: JsonRpcResponse = serde_json::from_str(&response_line)
        .map_err(|e| format!("Parse error: {}", e))?;

    if let Some(err) = response.error {
        return Err(err.message);
    }

    response.result.ok_or_else(|| "Empty response".to_string())
}

// ---------------------------------------------------------------------------
// Unit tests — written first, drive the implementation above
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- thread_hash ---

    #[test]
    fn thread_hash_returns_64_char_full_and_7_char_short() {
        let (full, short) = thread_hash("fix-login");
        assert_eq!(full.len(), 64, "full hash should be 64 hex chars");
        assert_eq!(short.len(), 7, "short hash should be 7 chars");
        assert!(full.starts_with(&short), "short hash should be prefix of full");
    }

    #[test]
    fn thread_hash_is_deterministic() {
        let (full1, short1) = thread_hash("fix-login");
        let (full2, short2) = thread_hash("fix-login");
        assert_eq!(full1, full2);
        assert_eq!(short1, short2);
    }

    #[test]
    fn thread_hash_differs_for_different_slugs() {
        let (full1, _) = thread_hash("fix-login");
        let (full2, _) = thread_hash("update-deps");
        assert_ne!(full1, full2);
    }

    // --- Priority ---

    #[test]
    fn priority_serialises_to_abbreviations() {
        assert_eq!(
            serde_json::to_string(&Priority::Background).unwrap(),
            "\"BG\""
        );
        assert_eq!(
            serde_json::to_string(&Priority::Priority).unwrap(),
            "\"PRIO\""
        );
        assert_eq!(
            serde_json::to_string(&Priority::Incident).unwrap(),
            "\"INC\""
        );
    }

    #[test]
    fn priority_deserialises_from_abbreviations() {
        assert_eq!(
            serde_json::from_str::<Priority>("\"BG\"").unwrap(),
            Priority::Background
        );
        assert_eq!(
            serde_json::from_str::<Priority>("\"PRIO\"").unwrap(),
            Priority::Priority
        );
        assert_eq!(
            serde_json::from_str::<Priority>("\"INC\"").unwrap(),
            Priority::Incident
        );
    }

    #[test]
    fn priority_from_str_parses_abbreviations() {
        assert_eq!("BG".parse::<Priority>().unwrap(), Priority::Background);
        assert_eq!("PRIO".parse::<Priority>().unwrap(), Priority::Priority);
        assert_eq!("INC".parse::<Priority>().unwrap(), Priority::Incident);
        assert!("unknown".parse::<Priority>().is_err());
    }

    #[test]
    fn priority_expands_to_full_names() {
        assert_eq!(Priority::Background.full_name(), "background");
        assert_eq!(Priority::Priority.full_name(), "priority");
        assert_eq!(Priority::Incident.full_name(), "incident");
    }

    #[test]
    fn priority_display_shows_abbreviation() {
        assert_eq!(format!("{}", Priority::Background), "BG");
        assert_eq!(format!("{}", Priority::Priority), "PRIO");
        assert_eq!(format!("{}", Priority::Incident), "INC");
    }

    // --- JSON-RPC ---

    #[test]
    fn jsonrpc_request_serialises_correctly() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: 1,
            method: "thread.start".to_string(),
            params: serde_json::json!({"slug": "fix-login"}),
        };
        let s = serde_json::to_string(&req).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["jsonrpc"], "2.0");
        assert_eq!(v["id"], 1);
        assert_eq!(v["method"], "thread.start");
        assert_eq!(v["params"]["slug"], "fix-login");
    }

    #[test]
    fn jsonrpc_success_response_has_result_no_error() {
        let resp = JsonRpcResponse::success(1, serde_json::json!({"ok": true}));
        let s = serde_json::to_string(&resp).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["result"]["ok"], true);
        assert!(v.get("error").is_none() || v["error"].is_null());
    }

    #[test]
    fn jsonrpc_error_response_has_error_no_result() {
        let resp = JsonRpcResponse::error(1, -32600, "slug already exists");
        let s = serde_json::to_string(&resp).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["error"]["code"], -32600);
        assert_eq!(v["error"]["message"], "slug already exists");
        assert!(v.get("result").is_none() || v["result"].is_null());
    }

    // --- ThreadStartedEvent ---

    #[test]
    fn thread_started_event_roundtrips() {
        let (hash, _) = thread_hash("fix-login");
        let event = ThreadStartedEvent {
            event: "ThreadStarted".to_string(),
            hash: hash.clone(),
            slug: "fix-login".to_string(),
            priority: Priority::Priority,
            description: "Fix the login bug".to_string(),
            timestamp: 1234567890,
        };
        let s = serde_json::to_string(&event).unwrap();
        let back: ThreadStartedEvent = serde_json::from_str(&s).unwrap();
        assert_eq!(back.slug, "fix-login");
        assert_eq!(back.priority, Priority::Priority);
        assert_eq!(back.hash, hash);
    }

    // --- Thread model ---

    #[test]
    fn thread_serialises_with_hash_fields() {
        let (hash, short_hash) = thread_hash("fix-login");
        let thread = Thread {
            hash: hash.clone(),
            short_hash: short_hash.clone(),
            slug: "fix-login".to_string(),
            state: ThreadState::Active,
            priority: Priority::Priority,
            description: "Fix it".to_string(),
        };
        let s = serde_json::to_string(&thread).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["hash"], hash);
        assert_eq!(v["short_hash"], short_hash);
        assert_eq!(v["slug"], "fix-login");
        assert_eq!(v["state"], "active");
        assert_eq!(v["priority"], "PRIO");
    }

    // --- socket_path ---

    #[test]
    fn socket_path_is_under_tmp() {
        let path = socket_path(std::path::Path::new("/some/project"));
        assert!(path.starts_with("/tmp/"));
        let name = path.file_name().unwrap().to_str().unwrap();
        assert!(name.starts_with("tsk-"));
        assert!(name.ends_with(".sock"));
    }

    #[test]
    fn socket_path_is_deterministic_for_same_root() {
        let p1 = socket_path(std::path::Path::new("/some/project"));
        let p2 = socket_path(std::path::Path::new("/some/project"));
        assert_eq!(p1, p2);
    }

    #[test]
    fn socket_path_differs_for_different_roots() {
        let p1 = socket_path(std::path::Path::new("/project/a"));
        let p2 = socket_path(std::path::Path::new("/project/b"));
        assert_ne!(p1, p2);
    }
}
