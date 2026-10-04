use clap::{Parser, Subcommand};

// ---------------------------------------------------------------------------
// Zoom / auto-context
// ---------------------------------------------------------------------------

pub fn resolve_zoom_thread(sock: &std::path::Path) -> Option<tsk_core::Thread> {
    // Use PWD env var if available (respects shell cwd), fallback to current_dir()
    let mut dir = std::env::var("PWD")
        .map(std::path::PathBuf::from)
        .or_else(|_| std::env::current_dir())
        .ok()?;
    loop {
        if dir.join("doc/tsk").is_dir() {
            let path_str = dir.to_string_lossy().into_owned();
            let result = tsk_core::send_request(
                sock,
                "thread.resolve_path",
                serde_json::json!({"path": path_str}),
            )
            .ok()?;
            if result.is_null() {
                return None;
            }
            return serde_json::from_value(result).ok();
        }
        if !dir.pop() {
            return None;
        }
    }
}

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
    #[command(
        about = "Start, pause, resume, detach, stop and list ledger threads, and resolve this session's or worktree's binding (runs without tskd)"
    )]
    Thread {
        #[command(subcommand)]
        action: thread::ThreadCommands,
    },
    #[command(about = "Fetch, locate and push the ledger branch (runs without tskd)")]
    Ledger {
        #[command(subcommand)]
        action: ledger::LedgerCommands,
    },
    #[command(
        about = "Append to, read and advance the ledger's external event queue (runs without tskd)"
    )]
    Events {
        #[command(subcommand)]
        action: events::EventsCommands,
    },
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // No args → TUI mode
    if args.len() == 1 {
        let sock = tsk_core::socket_path();
        let zoom = resolve_zoom_thread(&sock);
        if let Err(e) = tui::run(zoom) {
            eprintln!("TUI error: {}", e);
            std::process::exit(1);
        }
        return;
    }

    let cli = Cli::parse();
    match run_cli(cli) {
        Ok(0) => {}
        Ok(code) => std::process::exit(code),
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    }
}

fn run_cli(cli: Cli) -> Result<i32, String> {
    match cli.command {
        Some(Commands::Thread { action }) => thread::run(action),
        Some(Commands::Ledger { action }) => ledger::run(action).map(|()| 0),
        Some(Commands::Events { action }) => events::run(action),
        None => Ok(0),
    }
}

// ---------------------------------------------------------------------------
// TUI mode
// ---------------------------------------------------------------------------

mod tui;

mod ledger;

mod thread;

mod events;

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tsk_core::{Priority, Thread, ThreadState};

    #[test]
    fn thread_subcommands_parse() {
        assert!(matches!(
            Cli::try_parse_from(["tsk", "thread", "start", "M-X", "missions/M-X.md"]).unwrap().command,
            Some(Commands::Thread { action: thread::ThreadCommands::Start { mission_id, briefing_path } })
                if mission_id == "M-X" && briefing_path == "missions/M-X.md"
        ));
        assert!(matches!(
            Cli::try_parse_from(["tsk", "thread", "pause", "1234abcd", "m.md", "", "-next: tests"]).unwrap().command,
            Some(Commands::Thread { action: thread::ThreadCommands::Pause { task_id, whats_next, .. } })
                if task_id.is_empty() && whats_next == "-next: tests"
        ));
        assert!(matches!(
            Cli::try_parse_from(["tsk", "thread", "stop"])
                .unwrap()
                .command,
            Some(Commands::Thread {
                action: thread::ThreadCommands::Stop { thread_id: None }
            })
        ));
        assert!(matches!(
            Cli::try_parse_from(["tsk", "thread", "binding", "--no-fetch"])
                .unwrap()
                .command,
            Some(Commands::Thread {
                action: thread::ThreadCommands::Binding { no_fetch: true }
            })
        ));
        for args in [
            vec!["tsk", "thread", "resume", "1234abcd"],
            vec!["tsk", "thread", "detach"],
            vec!["tsk", "thread", "list"],
            vec!["tsk", "thread", "guard"],
            vec!["tsk", "thread", "session-start"],
        ] {
            assert!(Cli::try_parse_from(&args).is_ok(), "{:?}", args);
        }
        assert!(Cli::try_parse_from(["tsk", "thread", "pause", "1234abcd"]).is_err());
        assert!(Cli::try_parse_from(["tsk", "thread", "resume"]).is_err());
    }

    #[test]
    fn removed_daemon_commands_do_not_parse() {
        assert!(Cli::try_parse_from(["tsk", "task", "list"]).is_err());
        assert!(Cli::try_parse_from(["tsk", "context"]).is_err());
        assert!(Cli::try_parse_from(["tsk", "where"]).is_err());
        assert!(Cli::try_parse_from(["tsk", "thread", "create", "a", "PRIO", "d"]).is_err());
        assert!(Cli::try_parse_from(["tsk", "thread", "switch-to", "1"]).is_err());
    }

    #[test]
    fn ledger_subcommands_parse() {
        assert!(matches!(
            Cli::try_parse_from(["tsk", "ledger", "fetch"])
                .unwrap()
                .command,
            Some(Commands::Ledger {
                action: ledger::LedgerCommands::Fetch
            })
        ));
        assert!(matches!(
            Cli::try_parse_from(["tsk", "ledger", "path"])
                .unwrap()
                .command,
            Some(Commands::Ledger {
                action: ledger::LedgerCommands::Path
            })
        ));
        assert!(matches!(
            Cli::try_parse_from(["tsk", "ledger", "push", "Record a change"]).unwrap().command,
            Some(Commands::Ledger { action: ledger::LedgerCommands::Push { message } }) if message == "Record a change"
        ));
        assert!(Cli::try_parse_from(["tsk", "ledger", "push"]).is_err());
        assert!(Cli::try_parse_from(["tsk", "ledger"]).is_err());
        assert!(Cli::try_parse_from(["tsk", "ledger", "fetch", "extra"]).is_err());
    }

    #[test]
    fn events_subcommands_parse() {
        assert!(matches!(
            Cli::try_parse_from(["tsk", "events", "append", "github", "dependabot_alert", "created", "o/r", "p.json"]).unwrap().command,
            Some(Commands::Events { action: events::EventsCommands::Append { source, event_type, action, repo, payload_file } })
                if source == "github" && event_type == "dependabot_alert" && action == "created" && repo == "o/r" && payload_file == std::path::Path::new("p.json")
        ));
        assert!(matches!(
            Cli::try_parse_from(["tsk", "events", "append-batch"])
                .unwrap()
                .command,
            Some(Commands::Events {
                action: events::EventsCommands::AppendBatch {
                    source: None,
                    action: None
                }
            })
        ));
        assert!(matches!(
            Cli::try_parse_from(["tsk", "events", "append-batch", "--source", "github", "--action", "polled"]).unwrap().command,
            Some(Commands::Events { action: events::EventsCommands::AppendBatch { source: Some(source), action: Some(action) } })
                if source == "github" && action == "polled"
        ));
        assert!(Cli::try_parse_from(["tsk", "events", "append-batch", "github"]).is_err());
        assert!(matches!(
            Cli::try_parse_from(["tsk", "events", "read-new"])
                .unwrap()
                .command,
            Some(Commands::Events {
                action: events::EventsCommands::ReadNew
            })
        ));
        assert!(matches!(
            Cli::try_parse_from(["tsk", "events", "advance-watermark", "-1"]).unwrap().command,
            Some(Commands::Events { action: events::EventsCommands::AdvanceWatermark { count } }) if count == "-1"
        ));
        assert!(
            Cli::try_parse_from(["tsk", "events", "append", "github", "t", "a", "o/r"]).is_err()
        );
        assert!(Cli::try_parse_from(["tsk", "events", "advance-watermark"]).is_err());
        assert!(Cli::try_parse_from(["tsk", "events", "read-new", "extra"]).is_err());
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
            path: None,
        };
        // 1 section: title + top border + 1 row + bottom border = 4
        assert_eq!(tui::count_rows(&[t]), 4);
    }

    #[test]
    fn count_rows_two_sections() {
        let active = Thread {
            id: 1,
            slug: "a".to_string(),
            state: ThreadState::Active,
            priority: Priority::Priority,
            description: "".to_string(),
            path: None,
        };
        let paused = Thread {
            id: 2,
            slug: "b".to_string(),
            state: ThreadState::Paused,
            priority: Priority::Background,
            description: "".to_string(),
            path: None,
        };
        // active section: 4 rows; blank separator: 1; bg section: 4 rows = 9
        assert_eq!(tui::count_rows(&[active, paused]), 9);
    }
}
