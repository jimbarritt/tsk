use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tsk_core::{
    event_log_dir, event_log_path, index_path, socket_path, thread_dir, thread_hash, threads_dir,
    tsk_dir, JsonRpcRequest, JsonRpcResponse, Priority, Thread, ThreadStartedEvent, ThreadState,
};

fn main() {
    let project_root = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().expect("Cannot determine current directory"));

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
    let _ = fs::remove_file(&sock); // ignore error if it doesn't exist

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
        "thread.start" => handle_thread_start(request, state, project_root),
        "thread.list" => handle_thread_list(request, state),
        _ => JsonRpcResponse::error(request.id, -32601, "Method not found"),
    }
}

fn handle_thread_start(
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

    let (hash, short_hash) = thread_hash(&slug);

    let thread = Thread {
        hash: hash.clone(),
        short_hash: short_hash.clone(),
        slug: slug.clone(),
        state: ThreadState::Active,
        priority: priority.clone(),
        description: description.clone(),
    };

    // Create per-thread context directory
    let dir = thread_dir(project_root, &short_hash, &slug);
    if let Err(e) = fs::create_dir_all(&dir) {
        return JsonRpcResponse::error(
            request.id,
            -32603,
            format!("Failed to create thread dir: {}", e),
        );
    }

    // Append event to NDJSON log
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let event = ThreadStartedEvent {
        event: "ThreadStarted".to_string(),
        hash: hash.clone(),
        slug: slug.clone(),
        priority: priority.clone(),
        description: description.clone(),
        timestamp,
    };

    let event_line = serde_json::to_string(&event).unwrap() + "\n";
    if let Err(e) = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(event_log_path(project_root))
        .and_then(|mut f| f.write_all(event_line.as_bytes()))
    {
        return JsonRpcResponse::error(
            request.id,
            -32603,
            format!("Failed to write event: {}", e),
        );
    }

    locked.push(thread.clone());

    // Write authoritative index.json
    let index_content = serde_json::to_string_pretty(&*locked).unwrap();
    if let Err(e) = fs::write(index_path(project_root), index_content) {
        return JsonRpcResponse::error(
            request.id,
            -32603,
            format!("Failed to write index: {}", e),
        );
    }

    JsonRpcResponse::success(request.id, serde_json::to_value(&thread).unwrap())
}

fn handle_thread_list(
    request: JsonRpcRequest,
    state: &Arc<Mutex<Vec<Thread>>>,
) -> JsonRpcResponse {
    let locked = state.lock().unwrap();
    JsonRpcResponse::success(request.id, serde_json::json!({ "threads": *locked }))
}
