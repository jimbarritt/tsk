pub mod tasks_pane;
pub mod threads_pane;

use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind,
        KeyModifiers, MouseEventKind,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use std::io;
use std::time::{Duration, Instant};
use tsk_core::{send_request, socket_path, Task, Thread, ThreadState};

use crate::project_root;

// -----------------------------------------------------------------------
// Pane state
// -----------------------------------------------------------------------

enum Pane {
    Threads,
    Tasks { thread_id: u32, slug: String },
}

// -----------------------------------------------------------------------
// Data fetching
// -----------------------------------------------------------------------

fn fetch_threads(sock: &std::path::Path) -> Result<Vec<Thread>, String> {
    let result = send_request(sock, "thread.list", serde_json::json!({}))?;
    let threads: Vec<Thread> = serde_json::from_value(result["threads"].clone())
        .map_err(|e| format!("Failed to parse threads: {}", e))?;
    Ok(threads)
}

fn fetch_tasks(sock: &std::path::Path, thread_id: &str) -> Result<Vec<Task>, String> {
    let result = send_request(
        sock,
        "task.list",
        serde_json::json!({ "thread": thread_id }),
    )?;
    let tasks: Vec<Task> = serde_json::from_value(result["tasks"].clone())
        .map_err(|e| format!("Failed to parse tasks: {}", e))?;
    Ok(tasks)
}

// -----------------------------------------------------------------------
// Scroll helpers (pub for tests)
// -----------------------------------------------------------------------

pub fn scroll_down(scroll: usize, row_count: usize, height: usize, amount: usize) -> usize {
    let max = row_count.saturating_sub(height);
    (scroll + amount).min(max)
}

pub fn scroll_bottom(row_count: usize, height: usize) -> usize {
    row_count.saturating_sub(height)
}

// Re-export for tests
pub use threads_pane::count_rows;

// -----------------------------------------------------------------------
// Entry point
// -----------------------------------------------------------------------

const POLL_INTERVAL: Duration = Duration::from_millis(500);

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let root = project_root();
    let sock = socket_path(&root);

    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
        original_hook(info);
    }));

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = event_loop(&mut terminal, &sock);

    let _ = std::panic::take_hook();
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

// -----------------------------------------------------------------------
// Event loop
// -----------------------------------------------------------------------

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    sock: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut pane = Pane::Threads;
    let mut threads: Vec<Thread> = fetch_threads(sock).unwrap_or_default();
    let mut tasks: Vec<Task> = Vec::new();
    let mut error_msg: Option<String> = if !sock.exists() {
        Some("tskd is not running. Start it with: tskd".to_string())
    } else {
        None
    };
    let mut last_poll = Instant::now();
    let mut scroll: usize = 0;
    let mut task_scroll: usize = 0;
    let mut selected: usize = 0;
    let mut last_g = false;
    let mut show_help = false;

    loop {
        let height = terminal
            .size()
            .map(|r| r.height.saturating_sub(1) as usize)
            .unwrap_or(23);

        // Render
        terminal.draw(|frame| match &pane {
            Pane::Threads => {
                threads_pane::render(
                    frame,
                    &threads,
                    error_msg.as_deref(),
                    scroll,
                    selected,
                    show_help,
                );
            }
            Pane::Tasks {
                thread_id,
                slug,
            } => {
                tasks_pane::render(
                    frame,
                    &threads,
                    &tasks,
                    *thread_id,
                    slug,
                    task_scroll,
                    show_help,
                );
            }
        })?;

        // Poll daemon
        if last_poll.elapsed() >= POLL_INTERVAL {
            match fetch_threads(sock) {
                Ok(t) => {
                    threads = t;
                    error_msg = None;
                }
                Err(e) => {
                    error_msg = Some(e);
                }
            }
            if let Pane::Tasks { thread_id, .. } = &pane {
                if let Ok(t) = fetch_tasks(sock, &thread_id.to_string()) {
                    tasks = t;
                }
            }
            last_poll = Instant::now();
        }

        // Input
        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    if show_help {
                        show_help = false;
                        continue;
                    }

                    let g_pressed = last_g;
                    last_g = false;

                    match &pane {
                        Pane::Threads => {
                            let ordered = threads_pane::build_ordered_threads(&threads);
                            let thread_count = ordered.len();

                            match key.code {
                                KeyCode::Char('q') | KeyCode::Esc => break,
                                KeyCode::Char('?') => show_help = true,

                                KeyCode::Char('j') | KeyCode::Down => {
                                    if thread_count > 0 {
                                        selected = (selected + 1).min(thread_count.saturating_sub(1));
                                    }
                                    // Auto-scroll to keep selection visible
                                    let row_count = threads_pane::count_rows(&threads);
                                    scroll = scroll_down(scroll, row_count, height, 0)
                                        .max(scroll);
                                }
                                KeyCode::Char('k') | KeyCode::Up => {
                                    selected = selected.saturating_sub(1);
                                }
                                KeyCode::Char('d')
                                    if key.modifiers.contains(KeyModifiers::CONTROL) =>
                                {
                                    let page = (height / 2).max(1);
                                    let row_count = threads_pane::count_rows(&threads);
                                    scroll = scroll_down(scroll, row_count, height, page);
                                }
                                KeyCode::Char('u')
                                    if key.modifiers.contains(KeyModifiers::CONTROL) =>
                                {
                                    let page = (height / 2).max(1);
                                    scroll = scroll.saturating_sub(page);
                                }
                                KeyCode::Char('g') => {
                                    if g_pressed {
                                        scroll = 0;
                                        selected = 0;
                                    } else {
                                        last_g = true;
                                    }
                                }
                                KeyCode::Char('t') if g_pressed => {
                                    // gt → go to tasks for active thread
                                    if let Some(active) = threads.iter().find(|t| t.state == ThreadState::Active) {
                                        tasks = fetch_tasks(sock, &active.id.to_string()).unwrap_or_default();
                                        pane = Pane::Tasks {
                                            thread_id: active.id,
                                            slug: active.slug.clone(),
                                        };
                                        task_scroll = 0;
                                    }
                                }
                                KeyCode::Char('G') => {
                                    let row_count = threads_pane::count_rows(&threads);
                                    scroll = scroll_bottom(row_count, height);
                                    if thread_count > 0 {
                                        selected = thread_count - 1;
                                    }
                                }
                                KeyCode::Enter => {
                                    if !ordered.is_empty() && selected < ordered.len() {
                                        let thread = ordered[selected];
                                        tasks = fetch_tasks(sock, &thread.id.to_string()).unwrap_or_default();
                                        pane = Pane::Tasks {
                                            thread_id: thread.id,
                                            slug: thread.slug.clone(),
                                        };
                                        task_scroll = 0;
                                    }
                                }
                                _ => {}
                            }
                        }
                        Pane::Tasks { .. } => {
                            let row_count = tasks_pane::count_task_rows(&tasks);

                            match key.code {
                                KeyCode::Char('q') => break,
                                KeyCode::Esc => {
                                    pane = Pane::Threads;
                                }
                                KeyCode::Char('o')
                                    if key.modifiers.contains(KeyModifiers::CONTROL) =>
                                {
                                    pane = Pane::Threads;
                                }
                                KeyCode::Char('?') => show_help = true,

                                KeyCode::Char('j') | KeyCode::Down => {
                                    task_scroll = scroll_down(task_scroll, row_count, height, 1);
                                }
                                KeyCode::Char('k') | KeyCode::Up => {
                                    task_scroll = task_scroll.saturating_sub(1);
                                }
                                KeyCode::Char('d')
                                    if key.modifiers.contains(KeyModifiers::CONTROL) =>
                                {
                                    let page = (height / 2).max(1);
                                    task_scroll = scroll_down(task_scroll, row_count, height, page);
                                }
                                KeyCode::Char('u')
                                    if key.modifiers.contains(KeyModifiers::CONTROL) =>
                                {
                                    let page = (height / 2).max(1);
                                    task_scroll = task_scroll.saturating_sub(page);
                                }
                                KeyCode::Char('g') => {
                                    if g_pressed {
                                        task_scroll = 0;
                                    } else {
                                        last_g = true;
                                    }
                                }
                                KeyCode::Char('G') => {
                                    task_scroll = scroll_bottom(row_count, height);
                                }
                                _ => {}
                            }
                        }
                    }
                }
                Event::Mouse(mouse) => match &pane {
                    Pane::Threads => match mouse.kind {
                        MouseEventKind::ScrollDown => {
                            let row_count = threads_pane::count_rows(&threads);
                            scroll = scroll_down(scroll, row_count, height, 3);
                        }
                        MouseEventKind::ScrollUp => {
                            scroll = scroll.saturating_sub(3);
                        }
                        MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
                            // Map click row to a thread
                            let click_row = mouse.row as usize + scroll;
                            let ordered = threads_pane::build_ordered_threads(&threads);
                            if let Some(idx) = row_to_thread_index(&threads, click_row) {
                                if idx < ordered.len() {
                                    let thread = ordered[idx];
                                    tasks = fetch_tasks(sock, &thread.id.to_string())
                                        .unwrap_or_default();
                                    pane = Pane::Tasks {
                                        thread_id: thread.id,
                                        slug: thread.slug.clone(),
                                    };
                                    task_scroll = 0;
                                }
                            }
                        }
                        _ => {}
                    },
                    Pane::Tasks { .. } => match mouse.kind {
                        MouseEventKind::ScrollDown => {
                            let row_count = tasks_pane::count_task_rows(&tasks);
                            task_scroll = scroll_down(task_scroll, row_count, height, 3);
                        }
                        MouseEventKind::ScrollUp => {
                            task_scroll = task_scroll.saturating_sub(3);
                        }
                        _ => {}
                    },
                },
                _ => {}
            }
        }
    }

    Ok(())
}

// -----------------------------------------------------------------------
// Map a rendered row number to a thread index in the ordered list
// -----------------------------------------------------------------------

fn row_to_thread_index(threads: &[Thread], row: usize) -> Option<usize> {
    let is_inactive =
        |t: &&Thread| matches!(t.state, ThreadState::Paused | ThreadState::Waiting { .. });
    let active_count = threads.iter().filter(|t| t.state == ThreadState::Active).count();
    let focus_count = threads.iter().filter(|t| {
        is_inactive(t) && matches!(t.priority, tsk_core::Priority::Incident | tsk_core::Priority::Priority)
    }).count();
    let bg_count = threads.iter().filter(|t| {
        is_inactive(t) && matches!(t.priority, tsk_core::Priority::Background)
    }).count();

    let sections = [active_count, focus_count, bg_count];
    let mut current_row = 0;
    let mut thread_offset = 0;

    for (s_idx, &count) in sections.iter().enumerate() {
        if count == 0 {
            continue;
        }
        // Separator between non-first sections
        if s_idx > 0 && thread_offset > 0 {
            current_row += 1; // blank line
        }
        current_row += 1; // section title
        current_row += 1; // top border

        // Data rows
        for i in 0..count {
            if current_row == row {
                return Some(thread_offset + i);
            }
            current_row += 1;
        }

        current_row += 1; // bottom border
        thread_offset += count;
    }

    None
}
