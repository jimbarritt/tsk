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
        about = "Create, change and list mission records in the local ledger store; tsk ledger push publishes (runs without tskd)"
    )]
    Mission {
        #[command(subcommand)]
        action: mission::MissionCommands,
    },
    #[command(
        about = "Add, change, block and list the tasks of a mission in the local ledger store; tsk ledger push publishes (runs without tskd)"
    )]
    Task {
        #[command(subcommand)]
        action: mission::records::TaskCommands,
    },
    #[command(
        about = "Record and list the decisions of a mission in the local ledger store; tsk ledger push publishes (runs without tskd)"
    )]
    Decision {
        #[command(subcommand)]
        action: mission::records::DecisionCommands,
    },
    #[command(
        about = "Ask, answer and list the questions of a mission in the local ledger store; tsk ledger push publishes (runs without tskd)"
    )]
    Question {
        #[command(subcommand)]
        action: mission::records::QuestionCommands,
    },
    #[command(about = "Read and write the user config: attach the nexus")]
    Config {
        #[command(subcommand)]
        action: config::ConfigCommands,
    },
    #[command(
        about = "Append to, read and advance the ledger's external event queue (runs without tskd)"
    )]
    Events {
        #[command(subcommand)]
        action: events::EventsCommands,
    },
    #[command(about = "Attach the nexus and list the territories and repos it indexes")]
    Nexus {
        #[command(subcommand)]
        action: nexus_cmd::NexusCommands,
    },
    #[command(
        about = "Add the plugin marketplace, then install and update the tsk plugin through a coding agent's CLI"
    )]
    InstallPlugin(install_plugin::InstallPluginArgs),
    #[command(
        name = "self-update",
        about = "Upgrade tsk to the latest release, by the method it was installed with (Homebrew, Cargo or a release binary)"
    )]
    SelfUpdate(self_update::SelfUpdateArgs),
    #[command(about = "Print guides for agents, such as importing an existing plan")]
    Guide {
        #[command(subcommand)]
        action: guide::GuideCommands,
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
        Some(Commands::Mission { action }) => mission::run(action).map(|()| 0),
        Some(Commands::Task { action }) => mission::records::run_task(action).map(|()| 0),
        Some(Commands::Decision { action }) => mission::records::run_decision(action).map(|()| 0),
        Some(Commands::Question { action }) => mission::records::run_question(action).map(|()| 0),
        Some(Commands::Config { action }) => config::run(action).map(|()| 0),
        Some(Commands::Nexus { action }) => nexus_cmd::run(action).map(|()| 0),
        Some(Commands::InstallPlugin(args)) => install_plugin::run(args).map(|()| 0),
        Some(Commands::SelfUpdate(args)) => self_update::run(args).map(|()| 0),
        Some(Commands::Guide { action }) => guide::run(action).map(|()| 0),
        None => Ok(0),
    }
}

// ---------------------------------------------------------------------------
// TUI mode
// ---------------------------------------------------------------------------

mod tui;

mod config;

mod ledger;

mod mission;

mod thread;

mod events;

mod install_plugin;

mod guide;

mod self_update;

mod nexus_cmd;

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
            Cli::try_parse_from(["tsk", "thread", "start", "M-X"]).unwrap().command,
            Some(Commands::Thread { action: thread::ThreadCommands::Start { mission_id, briefing_path: None } })
                if mission_id == "M-X"
        ));
        assert!(matches!(
            Cli::try_parse_from(["tsk", "thread", "start", "M-X", "missions/M-X.md"]).unwrap().command,
            Some(Commands::Thread { action: thread::ThreadCommands::Start { mission_id, briefing_path: Some(path) } })
                if mission_id == "M-X" && path == "missions/M-X.md"
        ));
        assert!(matches!(
            Cli::try_parse_from(["tsk", "thread", "pause", "1234abcd", "", "-next: tests"]).unwrap().command,
            Some(Commands::Thread { action: thread::ThreadCommands::Pause { task_id, whats_next, legacy_whats_next: None, .. } })
                if task_id.is_empty() && whats_next == "-next: tests"
        ));
        assert!(matches!(
            Cli::try_parse_from(["tsk", "thread", "pause", "1234abcd", "m.md", "T-01", "-next: tests"]).unwrap().command,
            Some(Commands::Thread { action: thread::ThreadCommands::Pause { task_id, whats_next, legacy_whats_next: Some(last), .. } })
                if task_id == "m.md" && whats_next == "T-01" && last == "-next: tests"
        ));
        assert!(matches!(
            Cli::try_parse_from(["tsk", "thread", "skip"])
                .unwrap()
                .command,
            Some(Commands::Thread {
                action: thread::ThreadCommands::Skip
            })
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
            vec!["tsk", "thread", "session-start", "--format", "copilot"],
        ] {
            assert!(Cli::try_parse_from(&args).is_ok(), "{:?}", args);
        }
        assert!(
            Cli::try_parse_from(["tsk", "thread", "session-start", "--format", "gemini"]).is_err()
        );
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
        assert!(matches!(
            Cli::try_parse_from(["tsk", "ledger", "status"])
                .unwrap()
                .command,
            Some(Commands::Ledger {
                action: ledger::LedgerCommands::Status
            })
        ));
        assert!(matches!(
            Cli::try_parse_from(["tsk", "ledger", "discard"])
                .unwrap()
                .command,
            Some(Commands::Ledger {
                action: ledger::LedgerCommands::Discard { yes: false }
            })
        ));
        assert!(matches!(
            Cli::try_parse_from(["tsk", "ledger", "discard", "--yes"])
                .unwrap()
                .command,
            Some(Commands::Ledger {
                action: ledger::LedgerCommands::Discard { yes: true }
            })
        ));
        assert!(Cli::try_parse_from(["tsk", "ledger", "status", "extra"]).is_err());
    }

    #[test]
    fn mission_subcommands_parse() {
        for args in [
            vec!["tsk", "mission", "create", "M-X", "A name"],
            vec![
                "tsk",
                "mission",
                "create",
                "M-X",
                "A name",
                "--category",
                "administrative",
                "--parent",
                "M-Y",
            ],
            vec!["tsk", "mission", "set", "M-X", "purpose", "-"],
            vec!["tsk", "mission", "block", "M-X", "M-Y"],
            vec!["tsk", "mission", "unblock", "M-X", "M-Y"],
            vec!["tsk", "mission", "objective", "add", "M-X", "-dash outcome"],
            vec!["tsk", "mission", "objective", "done", "M-X", "1"],
            vec!["tsk", "mission", "objective", "undone", "M-X", "1"],
            vec!["tsk", "mission", "objective", "remove", "M-X", "1"],
            vec!["tsk", "mission", "objective", "list", "M-X"],
            vec![
                "tsk", "mission", "link", "M-X", "a/b.md", "--role", "report",
            ],
            vec!["tsk", "mission", "unlink", "M-X", "a/b.md"],
            vec!["tsk", "mission", "list"],
            vec!["tsk", "mission", "list", "M-X", "--json"],
        ] {
            assert!(Cli::try_parse_from(&args).is_ok(), "{:?}", args);
        }
        assert!(Cli::try_parse_from(["tsk", "mission", "show", "M-X"]).is_err());
        assert!(Cli::try_parse_from(["tsk", "mission", "link", "M-X", "a.md"]).is_err());
    }

    #[test]
    fn task_decision_and_question_subcommands_parse() {
        for args in [
            vec!["tsk", "task", "add", "M-X", "Do the thing"],
            vec!["tsk", "task", "add", "M-X", "-dash description"],
            vec!["tsk", "task", "set", "M-X", "T-01", "status", "done"],
            vec!["tsk", "task", "block", "M-X", "T-01", "T-02"],
            vec!["tsk", "task", "unblock", "M-X", "T-01", "T-02"],
            vec!["tsk", "task", "list", "M-X"],
            vec!["tsk", "decision", "add", "M-X", "Use Dolt", "--by", "jim"],
            vec![
                "tsk", "decision", "add", "M-X", "Use Dolt", "--by", "jim", "--task", "T-01",
            ],
            vec![
                "tsk",
                "decision",
                "add",
                "M-X",
                "Use Dolt",
                "--by",
                "jim",
                "--on",
                "2026-02-27",
            ],
            vec!["tsk", "decision", "list", "M-X"],
            vec!["tsk", "question", "add", "M-X", "Why?"],
            vec![
                "tsk", "question", "answer", "M-X", "Q-01", "Because", "--by", "jim",
            ],
            vec!["tsk", "question", "list"],
            vec!["tsk", "question", "list", "M-X"],
        ] {
            assert!(Cli::try_parse_from(&args).is_ok(), "{:?}", args);
        }
        assert!(Cli::try_parse_from(["tsk", "decision", "add", "M-X", "No who"]).is_err());
        assert!(Cli::try_parse_from(["tsk", "task", "show", "M-X", "T-01"]).is_err());
        assert!(Cli::try_parse_from(["tsk", "task", "list"]).is_err());
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

    #[test]
    fn install_plugin_parses_with_defaults_and_options() {
        match Cli::try_parse_from(["tsk", "install-plugin", "claude-cli"])
            .unwrap()
            .command
        {
            Some(Commands::InstallPlugin(args)) => {
                assert_eq!(args.target, install_plugin::Target::ClaudeCli);
                assert_eq!(args.scope, None);
                assert_eq!(args.plugins, vec!["tsk".to_string()]);
                assert_eq!(args.marketplace_source(), "jimbarritt/claude-plugins");
                assert_eq!(args.marketplace_name(), "jimbarritt-claude-plugins");
            }
            _ => panic!("install-plugin did not parse"),
        }
        match Cli::try_parse_from([
            "tsk",
            "install-plugin",
            "claude-cli",
            "--scope",
            "local",
            "--plugin",
            "swe",
            "--plugin",
            "tsk",
        ])
        .unwrap()
        .command
        {
            Some(Commands::InstallPlugin(args)) => {
                assert_eq!(args.scope, Some(install_plugin::Scope::Local));
                assert_eq!(args.plugins, vec!["swe".to_string(), "tsk".to_string()]);
            }
            _ => panic!("install-plugin did not parse"),
        }
        match Cli::try_parse_from(["tsk", "install-plugin", "copilot-cli"])
            .unwrap()
            .command
        {
            Some(Commands::InstallPlugin(args)) => {
                assert_eq!(args.target, install_plugin::Target::CopilotCli);
                assert_eq!(args.marketplace_source(), "jimbarritt/tsk");
                assert_eq!(args.marketplace_name(), "tsk");
            }
            _ => panic!("install-plugin did not parse"),
        }
        assert!(Cli::try_parse_from(["tsk", "install-plugin"]).is_err());
        assert!(Cli::try_parse_from(["tsk", "install-plugin", "codex"]).is_err());
        assert!(
            Cli::try_parse_from(["tsk", "install-plugin", "claude-cli", "--scope", "team"])
                .is_err()
        );
    }

    #[test]
    fn self_update_parses_with_and_without_check() {
        assert!(matches!(
            Cli::try_parse_from(["tsk", "self-update"]).unwrap().command,
            Some(Commands::SelfUpdate(self_update::SelfUpdateArgs {
                check: false
            }))
        ));
        assert!(matches!(
            Cli::try_parse_from(["tsk", "self-update", "--check"])
                .unwrap()
                .command,
            Some(Commands::SelfUpdate(self_update::SelfUpdateArgs {
                check: true
            }))
        ));
        assert!(Cli::try_parse_from(["tsk", "self-update", "extra"]).is_err());
    }

    #[test]
    fn nexus_subcommands_parse() {
        assert!(matches!(
            Cli::try_parse_from(["tsk", "nexus", "add", "https://example.test/o/nexus"]).unwrap().command,
            Some(Commands::Nexus { action: nexus_cmd::NexusCommands::Add { url } })
                if url == "https://example.test/o/nexus"
        ));
        assert!(matches!(
            Cli::try_parse_from(["tsk", "nexus", "list"])
                .unwrap()
                .command,
            Some(Commands::Nexus {
                action: nexus_cmd::NexusCommands::List { json: false }
            })
        ));
        assert!(matches!(
            Cli::try_parse_from(["tsk", "nexus", "list", "--json"])
                .unwrap()
                .command,
            Some(Commands::Nexus {
                action: nexus_cmd::NexusCommands::List { json: true }
            })
        ));
        match Cli::try_parse_from(["tsk", "nexus", "register-repo"])
            .unwrap()
            .command
        {
            Some(Commands::Nexus {
                action:
                    nexus_cmd::NexusCommands::RegisterRepo {
                        id,
                        territory,
                        territory_name,
                        ledger,
                        local,
                    },
            }) => {
                assert_eq!(id, None);
                assert_eq!(territory, None);
                assert_eq!(territory_name, None);
                assert_eq!(ledger, nexus_cmd::LedgerChoice::Nexus);
                assert!(!local);
            }
            _ => panic!("register-repo did not parse"),
        }
        assert!(Cli::try_parse_from([
            "tsk",
            "nexus",
            "register-repo",
            "--id",
            "x",
            "--territory",
            "work",
            "--territory-name",
            "Work",
            "--ledger",
            "repo",
            "--local",
        ])
        .is_ok());
        assert!(
            Cli::try_parse_from(["tsk", "nexus", "register-repo", "--territory-name", "Work"])
                .is_err()
        );
        assert!(
            Cli::try_parse_from(["tsk", "nexus", "register-repo", "--ledger", "cloud"]).is_err()
        );
        assert!(Cli::try_parse_from(["tsk", "nexus", "add"]).is_err());
        assert!(Cli::try_parse_from(["tsk", "nexus"]).is_err());
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
