use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tsk_core::{
    event_log_dir, event_log_path, index_path, socket_path, thread_dir, threads_dir,
    tsk_dir, JsonRpcRequest, JsonRpcResponse, Priority, Thread, ThreadCreatedEvent,
    ThreadSwitchedEvent, ThreadState,
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
        "thread.create" => handle_thread_create(request, state, project_root),
        "thread.list" => handle_thread_list(request, state),
        "thread.switch_to" => handle_thread_switch_to(request, state, project_root),
        _ => JsonRpcResponse::error(request.id, -32601, "Method not found"),
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

    // Resolve by numeric id (accepts "1" or "0001") or by slug
    let target_idx = if let Ok(n) = target_id_str.trim_start_matches('0').parse::<u32>().or_else(|_| target_id_str.parse::<u32>()) {
        locked.iter().position(|t| t.id == n)
    } else {
        locked.iter().position(|t| t.slug == target_id_str)
    };

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
        t.state = if i == target_idx {
            ThreadState::Active
        } else {
            ThreadState::Paused
        };
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
