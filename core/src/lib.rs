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
    Waiting { reason: Option<String> },
}

impl std::fmt::Display for ThreadState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ThreadState::Active => write!(f, "active"),
            ThreadState::Paused => write!(f, "paused"),
            ThreadState::Waiting { .. } => write!(f, "waiting"),
        }
    }
}

// ---------------------------------------------------------------------------
// Thread model
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Thread {
    pub id: u32,
    pub slug: String,
    pub state: ThreadState,
    pub priority: Priority,
    pub description: String,
}

impl Thread {
    /// Zero-padded 4-digit string representation of the id (e.g. "0001").
    pub fn id_str(&self) -> String {
        format!("{:04}", self.id)
    }
}

// ---------------------------------------------------------------------------
// Task state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TaskState {
    NotStarted,
    InProgress,
    Blocked,
    Done,
    Cancelled,
}

impl std::fmt::Display for TaskState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskState::NotStarted => write!(f, "not-started"),
            TaskState::InProgress => write!(f, "in-progress"),
            TaskState::Blocked    => write!(f, "blocked"),
            TaskState::Done       => write!(f, "done"),
            TaskState::Cancelled  => write!(f, "cancelled"),
        }
    }
}

// ---------------------------------------------------------------------------
// Task model
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Task {
    /// Human-readable id: `TSK-{thread_id:04}-{seq:04}` e.g. `TSK-0001-0001`
    pub id: String,
    pub description: String,
    pub state: TaskState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub due_by: Option<String>,
    /// Sequence number for manual ordering (1-based, not displayed)
    pub seq: u32,
    /// Only meaningful when state is Blocked
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocked_reason: Option<String>,
}

impl Task {
    /// Format a task id from thread id and sequence number.
    pub fn make_id(thread_id: u32, seq: u32) -> String {
        format!("TSK-{:04}-{:04}", thread_id, seq)
    }
}

// ---------------------------------------------------------------------------
// Storage paths (tasks)
// ---------------------------------------------------------------------------

/// Path to the tasks file for a thread: `tsk/threads/{id}-{slug}/tasks.json`
pub fn tasks_path(project_root: &Path, thread_id: u32, slug: &str) -> PathBuf {
    thread_dir(project_root, thread_id, slug).join("tasks.json")
}

// ---------------------------------------------------------------------------
// Event types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadCreatedEvent {
    pub event: String,
    pub id: u32,
    pub slug: String,
    pub priority: Priority,
    pub description: String,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadSwitchedEvent {
    pub event: String,
    pub active_id: u32,
    pub paused_ids: Vec<u32>,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadWaitedEvent {
    pub event: String,
    pub id: u32,
    pub reason: Option<String>,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadResumedEvent {
    pub event: String,
    pub id: u32,
    pub note: Option<String>,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadUpdatedEvent {
    pub event: String,
    pub id: u32,
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

/// Thread working directory: `tsk/threads/{id:04}-{slug}/`
pub fn thread_dir(project_root: &Path, id: u32, slug: &str) -> PathBuf {
    threads_dir(project_root).join(format!("{:04}-{}", id, slug))
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
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- Thread id_str ---

    #[test]
    fn thread_id_str_zero_pads_to_four_digits() {
        let t = Thread {
            id: 1,
            slug: "fix-login".to_string(),
            state: ThreadState::Active,
            priority: Priority::Priority,
            description: "Fix it".to_string(),
        };
        assert_eq!(t.id_str(), "0001");
    }

    #[test]
    fn thread_id_str_handles_larger_numbers() {
        let t = Thread {
            id: 42,
            slug: "foo".to_string(),
            state: ThreadState::Paused,
            priority: Priority::Background,
            description: "".to_string(),
        };
        assert_eq!(t.id_str(), "0042");
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
            method: "thread.create".to_string(),
            params: serde_json::json!({"slug": "fix-login"}),
        };
        let s = serde_json::to_string(&req).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["jsonrpc"], "2.0");
        assert_eq!(v["id"], 1);
        assert_eq!(v["method"], "thread.create");
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

    // --- ThreadCreatedEvent ---

    #[test]
    fn thread_created_event_roundtrips() {
        let event = ThreadCreatedEvent {
            event: "ThreadCreated".to_string(),
            id: 1,
            slug: "fix-login".to_string(),
            priority: Priority::Priority,
            description: "Fix the login bug".to_string(),
            timestamp: 1234567890,
        };
        let s = serde_json::to_string(&event).unwrap();
        let back: ThreadCreatedEvent = serde_json::from_str(&s).unwrap();
        assert_eq!(back.id, 1);
        assert_eq!(back.slug, "fix-login");
        assert_eq!(back.priority, Priority::Priority);
    }

    // --- Thread model ---

    #[test]
    fn thread_serialises_with_id_field() {
        let thread = Thread {
            id: 1,
            slug: "fix-login".to_string(),
            state: ThreadState::Active,
            priority: Priority::Priority,
            description: "Fix it".to_string(),
        };
        let s = serde_json::to_string(&thread).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["id"], 1);
        assert_eq!(v["slug"], "fix-login");
        assert_eq!(v["state"], "active");
        assert_eq!(v["priority"], "PRIO");
    }

    // --- ThreadState::Waiting ---

    #[test]
    fn waiting_state_serialises_as_nested_object() {
        let state = ThreadState::Waiting { reason: Some("waiting for PR review".to_string()) };
        let v: serde_json::Value = serde_json::to_value(&state).unwrap();
        assert_eq!(v["waiting"]["reason"], "waiting for PR review");
    }

    #[test]
    fn waiting_state_with_no_reason_serialises_correctly() {
        let state = ThreadState::Waiting { reason: None };
        let v: serde_json::Value = serde_json::to_value(&state).unwrap();
        assert!(v["waiting"].is_object());
        assert!(v["waiting"]["reason"].is_null());
    }

    #[test]
    fn waiting_state_roundtrips_via_json() {
        let state = ThreadState::Waiting { reason: Some("blocked on deploy".to_string()) };
        let s = serde_json::to_string(&state).unwrap();
        let back: ThreadState = serde_json::from_str(&s).unwrap();
        assert_eq!(back, state);
    }

    #[test]
    fn waiting_state_display_shows_waiting() {
        let state = ThreadState::Waiting { reason: Some("blocked".to_string()) };
        assert_eq!(format!("{}", state), "waiting");
    }

    #[test]
    fn thread_with_waiting_state_roundtrips_via_json() {
        let thread = Thread {
            id: 1,
            slug: "fix-login".to_string(),
            state: ThreadState::Waiting { reason: Some("waiting for review".to_string()) },
            priority: Priority::Priority,
            description: "Fix it".to_string(),
        };
        let s = serde_json::to_string(&thread).unwrap();
        let back: Thread = serde_json::from_str(&s).unwrap();
        assert_eq!(back, thread);
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

    // --- thread_dir ---

    #[test]
    fn thread_dir_uses_zero_padded_id() {
        let dir = thread_dir(std::path::Path::new("/proj"), 1, "fix-login");
        assert!(dir.to_str().unwrap().contains("0001-fix-login"));
    }

    // --- Task ---

    #[test]
    fn task_make_id_formats_correctly() {
        assert_eq!(Task::make_id(1, 1), "TSK-0001-0001");
        assert_eq!(Task::make_id(42, 100), "TSK-0042-0100");
    }

    #[test]
    fn task_state_serialises_with_kebab_case() {
        assert_eq!(serde_json::to_string(&TaskState::NotStarted).unwrap(), "\"not-started\"");
        assert_eq!(serde_json::to_string(&TaskState::InProgress).unwrap(), "\"in-progress\"");
        assert_eq!(serde_json::to_string(&TaskState::Blocked).unwrap(), "\"blocked\"");
        assert_eq!(serde_json::to_string(&TaskState::Done).unwrap(), "\"done\"");
        assert_eq!(serde_json::to_string(&TaskState::Cancelled).unwrap(), "\"cancelled\"");
    }

    #[test]
    fn task_state_deserialises_from_kebab_case() {
        assert_eq!(serde_json::from_str::<TaskState>("\"not-started\"").unwrap(), TaskState::NotStarted);
        assert_eq!(serde_json::from_str::<TaskState>("\"in-progress\"").unwrap(), TaskState::InProgress);
        assert_eq!(serde_json::from_str::<TaskState>("\"blocked\"").unwrap(), TaskState::Blocked);
        assert_eq!(serde_json::from_str::<TaskState>("\"done\"").unwrap(), TaskState::Done);
        assert_eq!(serde_json::from_str::<TaskState>("\"cancelled\"").unwrap(), TaskState::Cancelled);
    }

    #[test]
    fn task_state_display() {
        assert_eq!(format!("{}", TaskState::NotStarted), "not-started");
        assert_eq!(format!("{}", TaskState::InProgress), "in-progress");
        assert_eq!(format!("{}", TaskState::Blocked),    "blocked");
        assert_eq!(format!("{}", TaskState::Done),       "done");
        assert_eq!(format!("{}", TaskState::Cancelled),  "cancelled");
    }

    #[test]
    fn task_roundtrips_via_json() {
        let task = Task {
            id: Task::make_id(1, 1),
            description: "Write unit tests".to_string(),
            state: TaskState::InProgress,
            due_by: Some("2026-03-31".to_string()),
            seq: 1,
            blocked_reason: None,
        };
        let s = serde_json::to_string(&task).unwrap();
        let back: Task = serde_json::from_str(&s).unwrap();
        assert_eq!(back, task);
    }

    #[test]
    fn task_due_by_is_omitted_when_none() {
        let task = Task {
            id: Task::make_id(1, 1),
            description: "Test".to_string(),
            state: TaskState::NotStarted,
            due_by: None,
            seq: 1,
            blocked_reason: None,
        };
        let v: serde_json::Value = serde_json::to_value(&task).unwrap();
        assert!(v.get("due_by").is_none(), "due_by should be omitted when None");
    }

    #[test]
    fn task_blocked_reason_is_omitted_when_none() {
        let task = Task {
            id: Task::make_id(1, 1),
            description: "Test".to_string(),
            state: TaskState::NotStarted,
            due_by: None,
            seq: 1,
            blocked_reason: None,
        };
        let v: serde_json::Value = serde_json::to_value(&task).unwrap();
        assert!(v.get("blocked_reason").is_none(), "blocked_reason should be omitted when None");
    }

    #[test]
    fn tasks_path_is_inside_thread_dir() {
        let path = tasks_path(std::path::Path::new("/proj"), 1, "fix-login");
        assert!(path.to_str().unwrap().contains("0001-fix-login"));
        assert!(path.to_str().unwrap().ends_with("tasks.json"));
    }
}
