use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tsk_core::{
    event_log_dir, event_log_path, index_path, socket_path, thread_dir, threads_dir,
    tsk_dir, JsonRpcRequest, JsonRpcResponse, Priority, Thread, ThreadCreatedEvent,
    ThreadResumedEvent, ThreadSwitchedEvent, ThreadUpdatedEvent, ThreadWaitedEvent, ThreadState,
};

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("tskd — tsk daemon");
        println!();
        println!("USAGE:");
        println!("    tskd");
        println!();
        println!("Starts the tsk daemon in the current directory.");
        println!("Creates tsk/ directory structure if it does not exist.");
        return;
    }

    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("tskd {}", env!("CARGO_PKG_VERSION"));
        return;
    }

    let project_root = std::env::var("TSK_PROJECT_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::current_dir().expect("Cannot determine current directory"));

    eprintln!("tskd starting. project root: {:?}", project_root);
    run_daemon(&project_root);
}

fn run_daemon(project_root: &Path) {
    // Create storage directories
    fs::create_dir_all(tsk_dir(project_root)).expect("Failed to create tsk dir");
    fs::create_dir_all(event_log_dir(project_root)).expect("Failed to create event-log dir");
    fs::create_dir_all(threads_dir(project_root)).expect("Failed to create threads dir");

    // Load state from index.json if it exists
    let initial_state: Vec<Thread> = {
        let index = index_path(project_root);
        if index.exists() {
            let content = fs::read_to_string(&index).unwrap_or_default();
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            Vec::new()
        }
    };

    let state = Arc::new(Mutex::new(initial_state));

    // Bind socket — always remove stale socket first, then bind
    let sock = socket_path(project_root);
    let _ = fs::remove_file(&sock);

    let listener = UnixListener::bind(&sock).expect("Failed to bind socket");
    eprintln!("tskd listening on {:?}", sock);

    let project_root = project_root.to_path_buf();

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let state = Arc::clone(&state);
                let root = project_root.clone();
                std::thread::spawn(move || {
                    handle_connection(stream, state, &root);
                });
            }
            Err(e) => eprintln!("Accept error: {}", e),
        }
    }
}

fn handle_connection(
    stream: std::os::unix::net::UnixStream,
    state: Arc<Mutex<Vec<Thread>>>,
    project_root: &Path,
) {
    let read_stream = match stream.try_clone() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to clone stream: {}", e);
            return;
        }
    };

    let mut reader = BufReader::new(read_stream);
    let mut line = String::new();

    if reader.read_line(&mut line).is_err() || line.trim().is_empty() {
        return;
    }

    let request: JsonRpcRequest = match serde_json::from_str(line.trim()) {
        Ok(r) => r,
        Err(e) => {
            let resp = JsonRpcResponse::error(0, -32700, format!("Parse error: {}", e));
            let _ = writeln!(&stream, "{}", serde_json::to_string(&resp).unwrap());
            return;
        }
    };

    let response = handle_request(request, &state, project_root);
    let _ = writeln!(&stream, "{}", serde_json::to_string(&response).unwrap());
}

fn handle_request(
    request: JsonRpcRequest,
    state: &Arc<Mutex<Vec<Thread>>>,
    project_root: &Path,
) -> JsonRpcResponse {
    match request.method.as_str() {
        "thread.create"    => handle_thread_create(request, state, project_root),
        "thread.list"      => handle_thread_list(request, state),
        "thread.switch_to" => handle_thread_switch_to(request, state, project_root),
        "thread.update"    => handle_thread_update(request, state, project_root),
        "thread.wait"      => handle_thread_wait(request, state, project_root),
        "thread.resume"    => handle_thread_resume(request, state, project_root),
        _ => JsonRpcResponse::error(request.id, -32601, "Method not found"),
    }
}

/// Resolve a thread index by numeric id (e.g. "1", "0001") or slug.
fn resolve_thread_idx(locked: &[Thread], id_str: &str) -> Option<usize> {
    if let Ok(n) = id_str.trim_start_matches('0').parse::<u32>()
        .or_else(|_| id_str.parse::<u32>())
    {
        locked.iter().position(|t| t.id == n)
    } else {
        locked.iter().position(|t| t.slug == id_str)
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn append_event(project_root: &Path, event: &impl serde::Serialize) -> Result<(), String> {
    let line = serde_json::to_string(event).map_err(|e| e.to_string())? + "\n";
    fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(event_log_path(project_root))
        .and_then(|mut f| f.write_all(line.as_bytes()))
        .map_err(|e| e.to_string())
}

fn write_index(project_root: &Path, threads: &[Thread]) -> Result<(), String> {
    let content = serde_json::to_string_pretty(threads).map_err(|e| e.to_string())?;
    fs::write(index_path(project_root), content).map_err(|e| e.to_string())
}

/// Format a Unix timestamp as a UTC date string (YYYY-MM-DD).
/// Uses the proleptic Gregorian calendar algorithm — no external crate needed.
fn utc_date(secs: u64) -> String {
    let z = secs / 86400 + 719468;
    let era = z / 146097;
    let doe = z % 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{:04}-{:02}-{:02}", y, m, d)
}

fn next_id(threads: &[Thread]) -> u32 {
    threads.iter().map(|t| t.id).max().unwrap_or(0) + 1
}

fn handle_thread_create(
    request: JsonRpcRequest,
    state: &Arc<Mutex<Vec<Thread>>>,
    project_root: &Path,
) -> JsonRpcResponse {
    let params = &request.params;

    let slug = match params["slug"].as_str() {
        Some(s) => s.to_string(),
        None => return JsonRpcResponse::error(request.id, -32602, "Missing slug"),
    };

    let priority_str = match params["priority"].as_str() {
        Some(s) => s,
        None => return JsonRpcResponse::error(request.id, -32602, "Missing priority"),
    };

    let priority: Priority = match priority_str.parse() {
        Ok(p) => p,
        Err(e) => return JsonRpcResponse::error(request.id, -32602, e),
    };

    let description = params["description"].as_str().unwrap_or("").to_string();

    let mut locked = state.lock().unwrap();

    if locked.iter().any(|t| t.slug == slug) {
        return JsonRpcResponse::error(request.id, -32600, "slug already exists");
    }

    let id = next_id(&locked);
    let dir = thread_dir(project_root, id, &slug);

    // Create per-thread context directory
    if let Err(e) = fs::create_dir_all(&dir) {
        return JsonRpcResponse::error(
            request.id,
            -32603,
            format!("Failed to create thread dir: {}", e),
        );
    }

    // Scaffold index.md with thread metadata
    let date = utc_date(now_secs());
    let index_md = format!(
        "# {id:04} {slug}\n\n- **Priority**: {priority}\n- **Created**: {date}\n- **Description**: {description}\n\n## Notes\n\n",
        id = id,
        slug = slug,
        priority = priority_str,
        date = date,
        description = params["description"].as_str().unwrap_or(""),
    );
    if let Err(e) = fs::write(dir.join("index.md"), index_md) {
        return JsonRpcResponse::error(
            request.id,
            -32603,
            format!("Failed to write index.md: {}", e),
        );
    }

    let thread = Thread {
        id,
        slug: slug.clone(),
        state: ThreadState::Paused,
        priority,
        description,
    };

    let event = ThreadCreatedEvent {
        event: "ThreadCreated".to_string(),
        id,
        slug: slug.clone(),
        priority: thread.priority.clone(),
        description: thread.description.clone(),
        timestamp: now_secs(),
    };

    if let Err(e) = append_event(project_root, &event) {
        return JsonRpcResponse::error(request.id, -32603, format!("Failed to write event: {}", e));
    }

    locked.push(thread.clone());

    if let Err(e) = write_index(project_root, &locked) {
        return JsonRpcResponse::error(request.id, -32603, format!("Failed to write index: {}", e));
    }

    let mut result = serde_json::to_value(&thread).unwrap();
    result["dir"] = serde_json::Value::String(dir.to_string_lossy().into_owned());
    JsonRpcResponse::success(request.id, result)
}

fn handle_thread_list(
    request: JsonRpcRequest,
    state: &Arc<Mutex<Vec<Thread>>>,
) -> JsonRpcResponse {
    let locked = state.lock().unwrap();
    JsonRpcResponse::success(request.id, serde_json::json!({ "threads": *locked }))
}

fn handle_thread_switch_to(
    request: JsonRpcRequest,
    state: &Arc<Mutex<Vec<Thread>>>,
    project_root: &Path,
) -> JsonRpcResponse {
    let params = &request.params;

    let target_id_str = match params["id"].as_str() {
        Some(s) => s.to_string(),
        None => return JsonRpcResponse::error(request.id, -32602, "Missing id"),
    };

    let mut locked = state.lock().unwrap();
    let target_idx = resolve_thread_idx(&locked, &target_id_str);

    let target_idx = match target_idx {
        Some(i) => i,
        None => {
            return JsonRpcResponse::error(
                request.id,
                -32604,
                format!("Thread not found: {}", target_id_str),
            )
        }
    };

    let paused_ids: Vec<u32> = locked
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != target_idx)
        .map(|(_, t)| t.id)
        .collect();

    for (i, t) in locked.iter_mut().enumerate() {
        if i == target_idx {
            t.state = ThreadState::Active;
        } else {
            // Waiting threads keep their waiting state; others go to Paused
            if !matches!(t.state, ThreadState::Waiting { .. }) {
                t.state = ThreadState::Paused;
            }
        }
    }

    let active_thread = locked[target_idx].clone();

    let event = ThreadSwitchedEvent {
        event: "ThreadSwitched".to_string(),
        active_id: active_thread.id,
        paused_ids,
        timestamp: now_secs(),
    };

    if let Err(e) = append_event(project_root, &event) {
        return JsonRpcResponse::error(request.id, -32603, format!("Failed to write event: {}", e));
    }

    if let Err(e) = write_index(project_root, &locked) {
        return JsonRpcResponse::error(request.id, -32603, format!("Failed to write index: {}", e));
    }

    let dir = thread_dir(project_root, active_thread.id, &active_thread.slug);
    let mut result = serde_json::to_value(&active_thread).unwrap();
    result["dir"] = serde_json::Value::String(dir.to_string_lossy().into_owned());
    JsonRpcResponse::success(request.id, result)
}

fn handle_thread_update(
    request: JsonRpcRequest,
    state: &Arc<Mutex<Vec<Thread>>>,
    project_root: &Path,
) -> JsonRpcResponse {
    let params = &request.params;

    let id_str = match params["id"].as_str() {
        Some(s) => s.to_string(),
        None => return JsonRpcResponse::error(request.id, -32602, "Missing id"),
    };

    let new_slug        = params["slug"].as_str().map(|s| s.to_string());
    let new_description = params["description"].as_str().map(|s| s.to_string());
    let new_priority: Option<Priority> = match params["priority"].as_str() {
        Some(s) => match s.parse() {
            Ok(p)  => Some(p),
            Err(e) => return JsonRpcResponse::error(request.id, -32602, e),
        },
        None => None,
    };

    let mut locked = state.lock().unwrap();

    let idx = match resolve_thread_idx(&locked, &id_str) {
        Some(i) => i,
        None => return JsonRpcResponse::error(request.id, -32604, format!("Thread not found: {}", id_str)),
    };

    // Reject slug collision with a different thread
    if let Some(ref slug) = new_slug {
        if locked.iter().enumerate().any(|(i, t)| i != idx && &t.slug == slug) {
            return JsonRpcResponse::error(request.id, -32600, "slug already exists");
        }
    }

    let old_slug = locked[idx].slug.clone();
    let id       = locked[idx].id;

    // Rename directory if slug is changing
    if let Some(ref slug) = new_slug {
        if *slug != old_slug {
            let old_dir = thread_dir(project_root, id, &old_slug);
            let new_dir = thread_dir(project_root, id, slug);
            if let Err(e) = fs::rename(&old_dir, &new_dir) {
                return JsonRpcResponse::error(request.id, -32603, format!("Failed to rename thread dir: {}", e));
            }
        }
    }

    // Apply updates
    let thread = &mut locked[idx];
    if let Some(slug)        = new_slug        { thread.slug        = slug; }
    if let Some(description) = new_description { thread.description = description; }
    if let Some(priority)    = new_priority    { thread.priority    = priority; }

    // Update index.md header
    let dir = thread_dir(project_root, thread.id, &thread.slug);
    let index_md_path = dir.join("index.md");
    if index_md_path.exists() {
        if let Ok(content) = fs::read_to_string(&index_md_path) {
            let updated = update_index_md(&content, &thread.slug, &thread.priority.to_string(), &thread.description);
            let _ = fs::write(&index_md_path, updated);
        }
    }

    let thread = locked[idx].clone();

    let event = ThreadUpdatedEvent {
        event: "ThreadUpdated".to_string(),
        id: thread.id,
        slug: thread.slug.clone(),
        priority: thread.priority.clone(),
        description: thread.description.clone(),
        timestamp: now_secs(),
    };

    if let Err(e) = append_event(project_root, &event) {
        return JsonRpcResponse::error(request.id, -32603, format!("Failed to write event: {}", e));
    }

    if let Err(e) = write_index(project_root, &locked) {
        return JsonRpcResponse::error(request.id, -32603, format!("Failed to write index: {}", e));
    }

    let mut result = serde_json::to_value(&thread).unwrap();
    result["dir"] = serde_json::Value::String(dir.to_string_lossy().into_owned());
    JsonRpcResponse::success(request.id, result)
}

/// Update the frontmatter lines in index.md for slug, priority, and description.
fn update_index_md(content: &str, slug: &str, priority: &str, description: &str) -> String {
    content
        .lines()
        .map(|line| {
            if line.starts_with("# ") {
                // Rewrite header: "# 0001 old-slug" → "# 0001 new-slug"
                let parts: Vec<&str> = line.splitn(3, ' ').collect();
                if parts.len() >= 2 {
                    format!("{} {} {}", parts[0], parts[1], slug)
                } else {
                    line.to_string()
                }
            } else if line.starts_with("- **Priority**:") {
                format!("- **Priority**: {}", priority)
            } else if line.starts_with("- **Description**:") {
                format!("- **Description**: {}", description)
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn handle_thread_wait(
    request: JsonRpcRequest,
    state: &Arc<Mutex<Vec<Thread>>>,
    project_root: &Path,
) -> JsonRpcResponse {
    let params = &request.params;

    let id_str = match params["id"].as_str() {
        Some(s) => s.to_string(),
        None => return JsonRpcResponse::error(request.id, -32602, "Missing id"),
    };
    let reason = params["reason"].as_str().map(|s| s.to_string());

    let mut locked = state.lock().unwrap();

    let idx = match resolve_thread_idx(&locked, &id_str) {
        Some(i) => i,
        None => return JsonRpcResponse::error(request.id, -32604, format!("Thread not found: {}", id_str)),
    };

    if matches!(locked[idx].state, ThreadState::Waiting { .. }) {
        return JsonRpcResponse::error(request.id, -32600, "Thread is already waiting");
    }

    locked[idx].state = ThreadState::Waiting { reason: reason.clone() };

    let event = ThreadWaitedEvent {
        event: "ThreadWaited".to_string(),
        id: locked[idx].id,
        reason,
        timestamp: now_secs(),
    };

    if let Err(e) = append_event(project_root, &event) {
        return JsonRpcResponse::error(request.id, -32603, format!("Failed to write event: {}", e));
    }

    if let Err(e) = write_index(project_root, &locked) {
        return JsonRpcResponse::error(request.id, -32603, format!("Failed to write index: {}", e));
    }

    let thread = locked[idx].clone();
    JsonRpcResponse::success(request.id, serde_json::to_value(&thread).unwrap())
}

fn handle_thread_resume(
    request: JsonRpcRequest,
    state: &Arc<Mutex<Vec<Thread>>>,
    project_root: &Path,
) -> JsonRpcResponse {
    let params = &request.params;

    let id_str = match params["id"].as_str() {
        Some(s) => s.to_string(),
        None => return JsonRpcResponse::error(request.id, -32602, "Missing id"),
    };
    let note = params["note"].as_str().map(|s| s.to_string());

    let mut locked = state.lock().unwrap();

    let idx = match resolve_thread_idx(&locked, &id_str) {
        Some(i) => i,
        None => return JsonRpcResponse::error(request.id, -32604, format!("Thread not found: {}", id_str)),
    };

    if !matches!(locked[idx].state, ThreadState::Waiting { .. }) {
        return JsonRpcResponse::error(request.id, -32600, "Thread is not waiting");
    }

    locked[idx].state = ThreadState::Paused;

    let event = ThreadResumedEvent {
        event: "ThreadResumed".to_string(),
        id: locked[idx].id,
        note,
        timestamp: now_secs(),
    };

    if let Err(e) = append_event(project_root, &event) {
        return JsonRpcResponse::error(request.id, -32603, format!("Failed to write event: {}", e));
    }

    if let Err(e) = write_index(project_root, &locked) {
        return JsonRpcResponse::error(request.id, -32603, format!("Failed to write index: {}", e));
    }

    let thread = locked[idx].clone();
    JsonRpcResponse::success(request.id, serde_json::to_value(&thread).unwrap())
}
