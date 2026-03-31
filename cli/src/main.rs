use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tsk_core::{send_request, socket_path, Priority};

// ---------------------------------------------------------------------------
// CLI argument schema
// ---------------------------------------------------------------------------

#[derive(Parser)]
#[command(name = "tsk", about = "tsk — work with a clear context", version = env!("CARGO_PKG_VERSION"))]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Manage work threads
    Thread {
        #[command(subcommand)]
        action: ThreadCommands,
    },
    /// Manage tasks within a thread
    Task {
        #[command(subcommand)]
        action: TaskCommands,
    },
    /// Print agent context: conceptual overview, commands, and current thread state
    Context,
}

#[derive(Subcommand)]
enum ThreadCommands {
    /// Create a new work thread (starts paused; use switch-to to activate)
    Create {
        /// Unique slug identifier (e.g. fix-login)
        slug: String,
        /// Priority: BG (background), PRIO (priority), INC (incident)
        priority: String,
        /// Short description
        description: String,
    },
    /// List all threads
    List,
    /// Switch to a thread by its short ID or slug (makes it active, pauses others)
    SwitchTo {
        /// Short hash ID or slug of the thread to activate
        id: String,
    },
    /// Mark a thread as waiting on an external dependency
    Wait {
        /// ID or slug of the thread
        id: String,
        /// Optional reason (what are you waiting for?)
        reason: Option<String>,
    },
    /// Resume a waiting thread, restoring its previous state
    Resume {
        /// ID or slug of the thread
        id: String,
        /// Optional note (e.g. what unblocked it)
        note: Option<String>,
    },
    /// Update metadata on an existing thread (all flags optional)
    Update {
        /// ID or slug of the thread to update
        id: String,
        /// New slug (renames the thread directory)
        #[arg(long)]
        slug: Option<String>,
        /// New description
        #[arg(long)]
        description: Option<String>,
        /// New priority: BG, PRIO, or INC
        #[arg(long)]
        priority: Option<String>,
    },
}

#[derive(Subcommand)]
enum TaskCommands {
    /// Create a new task on the active thread (or a named thread with --thread)
    Create {
        /// Task description
        description: String,
        /// Optional due date (ISO 8601, e.g. 2026-03-15)
        #[arg(long)]
        due_by: Option<String>,
        /// Target thread id or slug (defaults to active thread)
        #[arg(long)]
        thread: Option<String>,
    },
    /// List tasks on the active thread (or a named thread with --thread)
    List {
        /// Target thread id or slug (defaults to active thread)
        #[arg(long)]
        thread: Option<String>,
    },
    /// Start a task (not-started or blocked → in-progress)
    Start {
        /// Task id (e.g. TSK-0001-0001)
        id: String,
        /// Target thread id or slug (defaults to active thread)
        #[arg(long)]
        thread: Option<String>,
    },
    /// Block a task (in-progress → blocked); requires a reason
    Block {
        /// Task id (e.g. TSK-0001-0001)
        id: String,
        /// Reason the task is blocked
        reason: String,
        /// Target thread id or slug (defaults to active thread)
        #[arg(long)]
        thread: Option<String>,
    },
    /// Mark a task as done
    Complete {
        /// Task id (e.g. TSK-0001-0001)
        id: String,
        /// Target thread id or slug (defaults to active thread)
        #[arg(long)]
        thread: Option<String>,
    },
    /// Cancel a task (any state → cancelled)
    Cancel {
        /// Task id (e.g. TSK-0001-0001)
        id: String,
        /// Target thread id or slug (defaults to active thread)
        #[arg(long)]
        thread: Option<String>,
    },
    /// Update task fields (all flags optional)
    Update {
        /// Task id (e.g. TSK-0001-0001)
        id: String,
        /// New description
        #[arg(long)]
        description: Option<String>,
        /// New due date (ISO 8601)
        #[arg(long)]
        due_by: Option<String>,
        /// New sequence number (for manual ordering)
        #[arg(long)]
        seq: Option<u32>,
        /// Target thread id or slug (defaults to active thread)
        #[arg(long)]
        thread: Option<String>,
    },
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // No args → TUI mode
    if args.len() == 1 {
        if let Err(e) = tui::run() {
            eprintln!("TUI error: {}", e);
            std::process::exit(1);
        }
        return;
    }

    // All other args (subcommands, --version, --help) → clap
    let cli = Cli::parse();
    if let Err(e) = run_cli(cli) {
        eprintln!("{}", e);
        std::process::exit(1);
    }
}

// ---------------------------------------------------------------------------
// Project root resolution
//
// Uses TSK_PROJECT_ROOT env var if set (useful for tests and scripting),
// otherwise falls back to the current directory.
// ---------------------------------------------------------------------------

fn project_root() -> PathBuf {
    std::env::var("TSK_PROJECT_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::current_dir().expect("Cannot determine current directory"))
}

// ---------------------------------------------------------------------------
// CLI mode
// ---------------------------------------------------------------------------

fn run_cli(cli: Cli) -> Result<(), String> {
    let root = project_root();
    let sock = socket_path(&root);

    match cli.command {
        Some(Commands::Thread { action }) => match action {
            ThreadCommands::Create {
                slug,
                priority,
                description,
            } => {
                let _: Priority = priority.parse()?;
                let result = send_request(
                    &sock,
                    "thread.create",
                    serde_json::json!({
                        "slug": slug,
                        "priority": priority,
                        "description": description,
                    }),
                )?;
                println!("{}", serde_json::to_string_pretty(&result).unwrap());
                Ok(())
            }
            ThreadCommands::List => {
                let result = send_request(&sock, "thread.list", serde_json::json!({}))?;
                println!("{}", serde_json::to_string_pretty(&result).unwrap());
                Ok(())
            }
            ThreadCommands::SwitchTo { id } => {
                let result = send_request(
                    &sock,
                    "thread.switch_to",
                    serde_json::json!({ "id": id }),
                )?;
                println!("{}", serde_json::to_string_pretty(&result).unwrap());
                Ok(())
            }
            ThreadCommands::Wait { id, reason } => {
                let mut params = serde_json::json!({ "id": id });
                if let Some(r) = reason { params["reason"] = r.into(); }
                let result = send_request(&sock, "thread.wait", params)?;
                println!("{}", serde_json::to_string_pretty(&result).unwrap());
                Ok(())
            }
            ThreadCommands::Resume { id, note } => {
                let mut params = serde_json::json!({ "id": id });
                if let Some(n) = note { params["note"] = n.into(); }
                let result = send_request(&sock, "thread.resume", params)?;
                println!("{}", serde_json::to_string_pretty(&result).unwrap());
                Ok(())
            }
            ThreadCommands::Update { id, slug, description, priority } => {
                if let Some(ref p) = priority {
                    let _: Priority = p.parse()?;
                }
                let mut params = serde_json::json!({ "id": id });
                if let Some(s) = slug        { params["slug"]        = s.into(); }
                if let Some(d) = description { params["description"] = d.into(); }
                if let Some(p) = priority    { params["priority"]    = p.into(); }
                let result = send_request(&sock, "thread.update", params)?;
                println!("{}", serde_json::to_string_pretty(&result).unwrap());
                Ok(())
            }
        },
        Some(Commands::Task { action }) => match action {
            TaskCommands::Create { description, due_by, thread } => {
                let mut params = serde_json::json!({ "description": description });
                if let Some(d) = due_by   { params["due_by"] = d.into(); }
                if let Some(t) = thread   { params["thread"] = t.into(); }
                let result = send_request(&sock, "task.create", params)?;
                println!("{}", serde_json::to_string_pretty(&result).unwrap());
                Ok(())
            }
            TaskCommands::List { thread } => {
                let mut params = serde_json::json!({});
                if let Some(t) = thread { params["thread"] = t.into(); }
                let result = send_request(&sock, "task.list", params)?;
                println!("{}", serde_json::to_string_pretty(&result).unwrap());
                Ok(())
            }
            TaskCommands::Start { id, thread } => {
                let mut params = serde_json::json!({ "id": id });
                if let Some(t) = thread { params["thread"] = t.into(); }
                let result = send_request(&sock, "task.start", params)?;
                println!("{}", serde_json::to_string_pretty(&result).unwrap());
                Ok(())
            }
            TaskCommands::Block { id, reason, thread } => {
                let mut params = serde_json::json!({ "id": id, "reason": reason });
                if let Some(t) = thread { params["thread"] = t.into(); }
                let result = send_request(&sock, "task.block", params)?;
                println!("{}", serde_json::to_string_pretty(&result).unwrap());
                Ok(())
            }
            TaskCommands::Complete { id, thread } => {
                let mut params = serde_json::json!({ "id": id });
                if let Some(t) = thread { params["thread"] = t.into(); }
                let result = send_request(&sock, "task.complete", params)?;
                println!("{}", serde_json::to_string_pretty(&result).unwrap());
                Ok(())
            }
            TaskCommands::Cancel { id, thread } => {
                let mut params = serde_json::json!({ "id": id });
                if let Some(t) = thread { params["thread"] = t.into(); }
                let result = send_request(&sock, "task.cancel", params)?;
                println!("{}", serde_json::to_string_pretty(&result).unwrap());
                Ok(())
            }
            TaskCommands::Update { id, description, due_by, seq, thread } => {
                let mut params = serde_json::json!({ "id": id });
                if let Some(d) = description { params["description"] = d.into(); }
                if let Some(d) = due_by      { params["due_by"]      = d.into(); }
                if let Some(s) = seq         { params["seq"]         = s.into(); }
                if let Some(t) = thread      { params["thread"]      = t.into(); }
                let result = send_request(&sock, "task.update", params)?;
                println!("{}", serde_json::to_string_pretty(&result).unwrap());
                Ok(())
            }
        },
        Some(Commands::Context) => {
            const AGENT_CONTEXT: &str = include_str!("agent-context.md");
            print!("{}", AGENT_CONTEXT);

            // Append live thread state if the daemon is running
            match send_request(&sock, "thread.list", serde_json::json!({})) {
                Ok(result) => {
                    let threads = result["threads"].as_array().cloned().unwrap_or_default();
                    if threads.is_empty() {
                        println!("No threads exist yet.");
                    } else {
                        for t in &threads {
                            let id = t["id"].as_u64().unwrap_or(0);
                            let slug = t["slug"].as_str().unwrap_or("?");
                            let state = t["state"].as_str().unwrap_or("?");
                            let priority = t["priority"].as_str().unwrap_or("?");
                            let description = t["description"].as_str().unwrap_or("");
                            println!(
                                "- {:04} {:20} {:6} {:6} {}",
                                id, slug, priority, state, description
                            );
                        }
                    }
                }
                Err(_) => {
                    println!("(tskd is not running — start it with: tskd)");
                }
            }
            Ok(())
        }
        None => Ok(()),
    }
}

// ---------------------------------------------------------------------------
// TUI mode
// ---------------------------------------------------------------------------

mod tui;

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tsk_core::{Thread, ThreadState};

    #[test]
    fn no_args_would_launch_tui() {
        let cli = Cli::try_parse_from(["tsk", "thread", "list"]);
        assert!(cli.is_ok());

        let cli = Cli::try_parse_from(["tsk", "thread", "create", "fix-login", "PRIO", "Fix it"]);
        assert!(cli.is_ok());

        let cli = Cli::try_parse_from(["tsk", "thread", "switch-to", "a3f1b2c"]);
        assert!(cli.is_ok());
    }

    #[test]
    fn cli_validates_priority() {
        assert!("BG".parse::<Priority>().is_ok());
        assert!("PRIO".parse::<Priority>().is_ok());
        assert!("INC".parse::<Priority>().is_ok());
        assert!("NORMAL".parse::<Priority>().is_err());
    }

    // --- TUI scroll helpers ---

    #[test]
    fn scroll_down_clamps_to_max() {
        // 10 rows, 24 height → can't scroll (all fits)
        assert_eq!(tui::scroll_down(0, 10, 24, 5), 0);
        // 30 rows, 24 height → max scroll is 6
        assert_eq!(tui::scroll_down(0, 30, 24, 10), 6);
        assert_eq!(tui::scroll_down(4, 30, 24, 10), 6);
    }

    #[test]
    fn scroll_down_advances_by_amount() {
        assert_eq!(tui::scroll_down(0, 30, 24, 3), 3);
        assert_eq!(tui::scroll_down(2, 30, 24, 3), 5);
    }

    #[test]
    fn scroll_bottom_goes_to_last_page() {
        assert_eq!(tui::scroll_bottom(30, 24), 6);
        assert_eq!(tui::scroll_bottom(10, 24), 0); // fits entirely
    }

    #[test]
    fn count_rows_empty_threads() {
        assert_eq!(tui::count_rows(&[]), 0);
    }

    #[test]
    fn count_rows_one_active() {
        let t = Thread {
            id: 1,
            slug: "foo".to_string(),
            state: ThreadState::Active,
            priority: Priority::Priority,
            description: "".to_string(),
        };
        // 1 section: title + top border + 1 row + bottom border = 4
        assert_eq!(tui::count_rows(&[t]), 4);
    }

    #[test]
    fn count_rows_two_sections() {
        let active = Thread {
            id: 1, slug: "a".to_string(), state: ThreadState::Active,
            priority: Priority::Priority, description: "".to_string(),
        };
        let paused = Thread {
            id: 2, slug: "b".to_string(), state: ThreadState::Paused,
            priority: Priority::Background, description: "".to_string(),
        };
        // active section: 4 rows; blank separator: 1; bg section: 4 rows = 9
        assert_eq!(tui::count_rows(&[active, paused]), 9);
    }
}
