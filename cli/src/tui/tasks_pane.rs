use ratatui::prelude::*;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
use tsk_core::{Task, TaskState, Thread};

use super::threads_pane::pad;

// -----------------------------------------------------------------------
// Task sort & symbols
// -----------------------------------------------------------------------

pub fn task_state_sort_key(state: &TaskState) -> u8 {
    match state {
        TaskState::InProgress => 0,
        TaskState::Blocked    => 1,
        TaskState::NotStarted => 2,
        TaskState::Done       => 3,
        TaskState::Cancelled  => 4,
    }
}

pub fn task_state_symbol(state: &TaskState) -> &'static str {
    match state {
        TaskState::NotStarted => "\u{25cb}",  // ○
        TaskState::InProgress => "\u{25b6}",  // ▶
        TaskState::Blocked    => "\u{231b}",  // ⏳
        TaskState::Done       => "\u{2713}",  // ✓
        TaskState::Cancelled  => "\u{2717}",  // ✗
    }
}

pub fn sort_tasks(tasks: &[Task]) -> Vec<&Task> {
    let mut sorted: Vec<&Task> = tasks.iter().collect();
    sorted.sort_by_key(|t| task_state_sort_key(&t.state));
    sorted
}

// -----------------------------------------------------------------------
// Task column widths
// -----------------------------------------------------------------------

const W_INDEX: u16 = 6;
const W_STATUS: u16 = 4;

struct TaskColWidths {
    index: u16,
    status: u16,
    title: u16,
}

impl TaskColWidths {
    fn from_area(w: u16) -> Self {
        let overhead = 2 + 2 + W_INDEX + W_STATUS; // 2 borders + 2 separators
        let title = w.saturating_sub(overhead).max(1);
        TaskColWidths { index: W_INDEX, status: W_STATUS, title }
    }

    fn active_widths(&self) -> Vec<u16> {
        vec![self.index, self.status, self.title]
    }
}

fn top_border(ws: &TaskColWidths) -> Line<'static> {
    let inner: Vec<String> = ws.active_widths().iter()
        .map(|&w| "\u{2500}".repeat(w as usize))
        .collect();
    Line::from(Span::raw(format!("\u{250c}{}\u{2510}", inner.join("\u{252c}"))))
}

fn bottom_border(ws: &TaskColWidths) -> Line<'static> {
    let inner: Vec<String> = ws.active_widths().iter()
        .map(|&w| "\u{2500}".repeat(w as usize))
        .collect();
    Line::from(Span::raw(format!("\u{2514}{}\u{2518}", inner.join("\u{2534}"))))
}

fn task_data_line<'a>(
    index: &str,
    symbol: &str,
    title: &str,
    ws: &TaskColWidths,
    style: Style,
) -> Line<'a> {
    let cells = vec![
        format!(" {}", pad(index,  ws.index.saturating_sub(1))),
        format!(" {}", pad(symbol, ws.status.saturating_sub(1))),
        format!(" {}", pad(title,  ws.title.saturating_sub(1))),
    ];
    let line = format!("\u{2502}{}\u{2502}", cells.join("\u{2502}"));
    Line::from(Span::styled(line, style))
}

// -----------------------------------------------------------------------
// Count rows in task pane (for scrolling)
// -----------------------------------------------------------------------

pub fn count_task_rows(tasks: &[Task]) -> usize {
    if tasks.is_empty() {
        return 4; // summary box (3 lines) + "No tasks" message
    }
    // summary box: 3 lines (top border, content, bottom border)
    // blank line: 1
    // section title: 1
    // table: top border + N rows + bottom border = N + 2
    3 + 1 + 1 + tasks.len() + 2
}

// -----------------------------------------------------------------------
// Main render function for tasks pane
// -----------------------------------------------------------------------

pub fn render(
    frame: &mut Frame,
    threads: &[Thread],
    tasks: &[Task],
    thread_id: u32,
    thread_slug: &str,
    scroll: usize,
    show_help: bool,
) {
    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(area);
    let main_area = chunks[0];
    let status_area = chunks[1];

    // Find the thread for priority display
    let thread = threads.iter().find(|t| t.id == thread_id);
    let priority_str = thread.map(|t| t.priority.to_string()).unwrap_or_default();

    // --- Main area: build lines ---
    let mut lines: Vec<Line> = Vec::new();

    // Thread summary box
    let summary_w = main_area.width.saturating_sub(2).max(1) as usize;
    let summary_text = format!(
        " #{:04} {}  {}",
        thread_id, thread_slug, priority_str,
    );
    let padded_summary = if summary_text.chars().count() < summary_w {
        format!("{}{}", summary_text, " ".repeat(summary_w - summary_text.chars().count()))
    } else {
        summary_text.chars().take(summary_w).collect()
    };

    lines.push(Line::from(Span::raw(format!(
        "\u{250c}{}\u{2510}",
        "\u{2500}".repeat(summary_w)
    ))));
    lines.push(Line::from(Span::styled(
        format!("\u{2502}{}\u{2502}", padded_summary),
        Style::default().bold(),
    )));
    lines.push(Line::from(Span::raw(format!(
        "\u{2514}{}\u{2518}",
        "\u{2500}".repeat(summary_w)
    ))));

    lines.push(Line::from(""));

    // Tasks section
    let sorted = sort_tasks(tasks);

    if sorted.is_empty() {
        lines.push(Line::from(Span::styled(
            "  No tasks yet. Use: tsk task create <description>",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            "> Tasks",
            Style::default().bold(),
        )));

        let ws = TaskColWidths::from_area(main_area.width);
        lines.push(top_border(&ws));

        for (i, task) in sorted.iter().enumerate() {
            let index_str = format!("{}", i + 1);
            let symbol = task_state_symbol(&task.state);
            let style = match task.state {
                TaskState::Done | TaskState::Cancelled => Style::default().fg(Color::DarkGray),
                _ => Style::default(),
            };
            lines.push(task_data_line(&index_str, symbol, &task.description, &ws, style));
        }

        lines.push(bottom_border(&ws));
    }

    frame.render_widget(
        Paragraph::new(lines).scroll((scroll as u16, 0)),
        main_area,
    );

    // --- Status bar ---
    let status_text = format!(
        "  #{:04} {} > tasks   |   esc back   |   ? help",
        thread_id, thread_slug,
    );
    frame.render_widget(
        Paragraph::new(status_text).style(Style::default().bg(Color::DarkGray)),
        status_area,
    );

    // --- Help popup ---
    if show_help {
        render_help_popup(frame, area);
    }
}

fn render_help_popup(frame: &mut Frame, area: Rect) {
    const POPUP_W: u16 = 40;
    const POPUP_H: u16 = 14;

    let x = area.x + area.width.saturating_sub(POPUP_W) / 2;
    let y = area.y + area.height.saturating_sub(POPUP_H) / 2;
    let popup_area = Rect::new(x, y, POPUP_W.min(area.width), POPUP_H.min(area.height));

    let key_style = Style::default().bold();

    let bindings: &[(&str, &str)] = &[
        ("j / \u{2193}",    "scroll down"),
        ("k / \u{2191}",    "scroll up"),
        ("ctrl-d",   "page down"),
        ("ctrl-u",   "page up"),
        ("gg",       "jump to top"),
        ("G",        "jump to bottom"),
        ("esc",      "back to threads"),
        ("ctrl-o",   "back to threads"),
        ("?",        "toggle this help"),
        ("q",        "quit"),
    ];

    let rows: Vec<Line> = bindings
        .iter()
        .map(|(key, desc)| {
            Line::from(vec![
                Span::raw("  "),
                Span::styled(format!("{:<12}", key), key_style),
                Span::raw("  "),
                Span::styled(*desc, Style::default()),
            ])
        })
        .collect();

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded);

    frame.render_widget(Clear, popup_area);
    frame.render_widget(Paragraph::new(rows).block(block), popup_area);
}
