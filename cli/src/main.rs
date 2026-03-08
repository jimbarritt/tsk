use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tsk_core::{send_request, socket_path, Priority, Thread};

// ---------------------------------------------------------------------------
// CLI argument schema
// ---------------------------------------------------------------------------

#[derive(Parser)]
#[command(name = "tsk", about = "tsk — work with a clear context")]
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
}

#[derive(Subcommand)]
enum ThreadCommands {
    /// Start a new work thread
    Start {
        /// Unique slug identifier for the thread (e.g. fix-login)
        slug: String,
        /// Priority: BG (background), PRIO (priority), INC (incident)
        priority: String,
        /// Short description of the thread
        description: String,
    },
    /// List all threads
    List,
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // No subcommand → TUI mode
    if args.len() == 1 {
        if let Err(e) = tui::run() {
            eprintln!("TUI error: {}", e);
            std::process::exit(1);
        }
        return;
    }

    // Subcommand → CLI mode
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
            ThreadCommands::Start {
                slug,
                priority,
                description,
            } => {
                // Validate priority
                let _: Priority = priority.parse()?;

                let result = send_request(
                    &sock,
                    "thread.start",
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
        },
        None => Ok(()),
    }
}

// ---------------------------------------------------------------------------
// TUI mode
// ---------------------------------------------------------------------------

mod tui {
    use super::*;
    use crossterm::{
        event::{self, Event, KeyCode, KeyEventKind},
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    };
    use ratatui::{
        layout::Constraint,
        prelude::*,
        widgets::{Block, Borders, Cell, Paragraph, Row, Table},
    };
    use std::io;
    use std::time::Duration;

    pub fn run() -> Result<(), Box<dyn std::error::Error>> {
        let root = project_root();
        let sock = socket_path(&root);

        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let result = event_loop(&mut terminal, &sock);

        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        terminal.show_cursor()?;

        result
    }

    fn fetch_threads(sock: &std::path::Path) -> Result<Vec<Thread>, String> {
        let result = send_request(sock, "thread.list", serde_json::json!({}))?;
        let threads: Vec<Thread> = serde_json::from_value(result["threads"].clone())
            .map_err(|e| format!("Failed to parse threads: {}", e))?;
        Ok(threads)
    }

    fn event_loop(
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
        sock: &std::path::Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut threads: Vec<Thread> = fetch_threads(sock).unwrap_or_default();
        let mut error_msg: Option<String> = if !sock.exists() {
            Some("tskd is not running. Start it with: tskd".to_string())
        } else {
            None
        };

        loop {
            terminal.draw(|frame| render(frame, &threads, error_msg.as_deref()))?;

            if event::poll(Duration::from_secs(2))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press && key.code == KeyCode::Char('q') {
                        break;
                    }
                }
            } else {
                // Timeout — refresh thread list
                match fetch_threads(sock) {
                    Ok(t) => {
                        threads = t;
                        error_msg = None;
                    }
                    Err(e) => {
                        error_msg = Some(e);
                    }
                }
            }
        }

        Ok(())
    }

    fn render(frame: &mut Frame, threads: &[Thread], error: Option<&str>) {
        let area = frame.area();

        if let Some(msg) = error {
            let paragraph = Paragraph::new(msg)
                .block(Block::default().title(" tsk ").borders(Borders::ALL));
            frame.render_widget(paragraph, area);
            return;
        }

        let header = Row::new(vec![
            Cell::from("ID").style(Style::default().bold()),
            Cell::from("SLUG").style(Style::default().bold()),
            Cell::from("PRIO").style(Style::default().bold()),
            Cell::from("STATE").style(Style::default().bold()),
            Cell::from("DESCRIPTION").style(Style::default().bold()),
        ]);

        let rows: Vec<Row> = threads
            .iter()
            .map(|t| {
                Row::new(vec![
                    Cell::from(t.short_hash.clone()),
                    Cell::from(t.slug.clone()),
                    Cell::from(t.priority.to_string()),
                    Cell::from(t.state.to_string()),
                    Cell::from(t.description.clone()),
                ])
            })
            .collect();

        let widths = [
            Constraint::Length(8),
            Constraint::Percentage(25),
            Constraint::Length(6),
            Constraint::Length(8),
            Constraint::Fill(1),
        ];

        let table = Table::new(rows, widths)
            .header(header)
            .block(
                Block::default()
                    .title(" tsk — work with a clear context (q to quit) ")
                    .borders(Borders::ALL),
            );

        frame.render_widget(table, area);
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_args_would_launch_tui() {
        // Verify the routing logic: args.len() == 1 → TUI path
        // We can't actually launch the TUI in a unit test, but we can verify
        // the Cli parser correctly handles subcommands.
        let cli = Cli::try_parse_from(["tsk", "thread", "list"]);
        assert!(cli.is_ok(), "thread list should parse ok");

        let cli = Cli::try_parse_from(["tsk", "thread", "start", "fix-login", "PRIO", "Fix it"]);
        assert!(cli.is_ok(), "thread start should parse ok");
    }

    #[test]
    fn cli_validates_priority() {
        // Valid priorities
        assert!("BG".parse::<Priority>().is_ok());
        assert!("PRIO".parse::<Priority>().is_ok());
        assert!("INC".parse::<Priority>().is_ok());
        // Invalid priority
        assert!("NORMAL".parse::<Priority>().is_err());
    }
}
