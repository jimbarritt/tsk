use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tsk_core::{send_request, socket_path, Priority, Thread, ThreadState};

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
    use notify::{recommended_watcher, RecursiveMode, Watcher};
    use ratatui::{prelude::*, widgets::Paragraph};
    use std::io;
    use std::sync::mpsc;
    use std::time::Duration;

    pub fn run() -> Result<(), Box<dyn std::error::Error>> {
        let root = project_root();
        let sock = socket_path(&root);

        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let result = event_loop(&mut terminal, &sock, &root);

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
        project_root: &std::path::Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut threads: Vec<Thread> = fetch_threads(sock).unwrap_or_default();
        let mut error_msg: Option<String> = if !sock.exists() {
            Some("tskd is not running. Start it with: tskd".to_string())
        } else {
            None
        };

        // Watch index.json for changes — fires instantly when the daemon writes state.
        let (tx, rx) = mpsc::channel();
        let mut watcher = recommended_watcher(move |_| { let _ = tx.send(()); })?;
        let index = tsk_core::index_path(project_root);
        // Watch the parent dir: index.json may not exist yet when the TUI starts.
        let watch_dir = index.parent().unwrap_or(project_root);
        let _ = watcher.watch(watch_dir, RecursiveMode::NonRecursive);

        loop {
            terminal.draw(|frame| render(frame, &threads, error_msg.as_deref()))?;

            // Drain any pending file-change notifications (non-blocking).
            let file_changed = rx.try_recv().is_ok();
            // Also consume any extra events that arrived in the same burst.
            while rx.try_recv().is_ok() {}

            if file_changed {
                match fetch_threads(sock) {
                    Ok(t) => { threads = t; error_msg = None; }
                    Err(e) => { error_msg = Some(e); }
                }
            }

            // Block briefly for a keypress; short timeout keeps UI responsive.
            if event::poll(Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press && key.code == KeyCode::Char('q') {
                        break;
                    }
                }
            }
        }

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Rendering
    // -----------------------------------------------------------------------

    // Fixed column widths (inner content, excluding borders).
    // Col 0: thread (short_hash-slug), Col 1: PRIO, Col 2: STATE, Col 3: description (fills)
    const W_PRIO: u16 = 6;   // "PRIO" + spaces
    const W_STATE: u16 = 8;  // "active" + spaces

    struct ColWidths {
        thread: u16,
        prio: u16,
        state: u16,
        desc: u16,
    }

    impl ColWidths {
        fn from_area(area_width: u16) -> Self {
            // total = outer_borders(2) + col_separators(3) + thread + prio + state + desc
            let fixed = 2 + 3 + W_PRIO + W_STATE;
            let remaining = area_width.saturating_sub(fixed);
            let thread = (remaining * 35 / 100).max(10);
            let desc = remaining.saturating_sub(thread);
            ColWidths { thread, prio: W_PRIO, state: W_STATE, desc }
        }

    }

    fn pad(s: &str, width: u16) -> String {
        let w = width as usize;
        if s.len() >= w {
            s[..w].to_string()
        } else {
            format!("{:<width$}", s, width = w)
        }
    }

    fn top_border(ws: &ColWidths) -> Line<'static> {
        let t = ws.thread as usize;
        let p = ws.prio as usize;
        let s = ws.state as usize;
        let d = ws.desc as usize;
        let line = format!(
            "┌{}┬{}┬{}┬{}┐",
            "─".repeat(t),
            "─".repeat(p),
            "─".repeat(s),
            "─".repeat(d),
        );
        Line::from(Span::styled(line, Style::default().fg(Color::White)))
    }

    fn bottom_border(ws: &ColWidths) -> Line<'static> {
        let t = ws.thread as usize;
        let p = ws.prio as usize;
        let s = ws.state as usize;
        let d = ws.desc as usize;
        let line = format!(
            "└{}┴{}┴{}┴{}┘",
            "─".repeat(t),
            "─".repeat(p),
            "─".repeat(s),
            "─".repeat(d),
        );
        Line::from(Span::styled(line, Style::default().fg(Color::White)))
    }

    fn data_line<'a>(
        col0: &str,
        col1: &str,
        col2: &str,
        col3: &str,
        ws: &ColWidths,
        style: Style,
    ) -> Line<'a> {
        let line = format!(
            "│ {}│ {}│ {}│ {}│",
            pad(col0, ws.thread.saturating_sub(1)),
            pad(col1, ws.prio.saturating_sub(1)),
            pad(col2, ws.state.saturating_sub(1)),
            pad(col3, ws.desc.saturating_sub(1)),
        );
        Line::from(Span::styled(line, style))
    }

    fn section_lines(title: &str, threads: &[&Thread], ws: &ColWidths) -> Vec<Line<'static>> {
        if threads.is_empty() {
            return vec![];
        }
        let mut lines: Vec<Line<'static>> = Vec::new();
        lines.push(Line::from(Span::styled(
            format!("> {}", title),
            Style::default().bold(),
        )));
        lines.push(top_border(ws));
        for t in threads {
            let id = format!("{} {}", t.id_str(), t.slug);
            lines.push(data_line(
                &id,
                &t.priority.to_string(),
                &t.state.to_string(),
                &t.description,
                ws,
                Style::default(),
            ));
        }
        lines.push(bottom_border(ws));
        lines
    }

    fn render(frame: &mut Frame, threads: &[Thread], error: Option<&str>) {
        let area = frame.area();

        if let Some(msg) = error {
            frame.render_widget(Paragraph::new(msg), area);
            return;
        }

        let ws = ColWidths::from_area(area.width);

        // Partition threads
        let active: Vec<&Thread> = threads
            .iter()
            .filter(|t| t.state == ThreadState::Active)
            .collect();

        let mut focus: Vec<&Thread> = threads
            .iter()
            .filter(|t| {
                t.state == ThreadState::Paused
                    && matches!(t.priority, Priority::Incident | Priority::Priority)
            })
            .collect();
        focus.sort_by_key(|t| match t.priority {
            Priority::Incident => 0u8,
            _ => 1,
        });

        let background: Vec<&Thread> = threads
            .iter()
            .filter(|t| {
                t.state == ThreadState::Paused && matches!(t.priority, Priority::Background)
            })
            .collect();

        let mut lines: Vec<Line> = Vec::new();

        let sections = [
            ("Active", active.as_slice()),
            ("Priority & Incidents", focus.as_slice()),
            ("Background", background.as_slice()),
        ];

        for (i, (title, threads)) in sections.iter().enumerate() {
            let s = section_lines(title, threads, &ws);
            if s.is_empty() {
                continue;
            }
            if i > 0 && !lines.is_empty() {
                lines.push(Line::from(""));
            }
            lines.extend(s);
        }

        frame.render_widget(Paragraph::new(lines), area);
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
}
