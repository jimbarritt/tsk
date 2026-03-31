use ratatui::prelude::*;
use tsk_core::{Priority, Thread, ThreadState};

// -----------------------------------------------------------------------
// Thread column widths
// -----------------------------------------------------------------------

const W_ID: u16 = 6;
const W_STATE: u16 = 9;
const W_PRIO: u16 = 6;
const SLUG_FULL: u16 = 40;

pub struct ColWidths {
    pub id: u16,
    pub slug: u16,
    pub state: u16,
    pub prio: u16,
    pub desc: u16,
}

impl ColWidths {
    pub fn from_area(w: u16) -> Self {
        let try5 = 2 + 4 + W_ID + W_STATE + W_PRIO;
        let try4 = 2 + 3 + W_ID + W_STATE + W_PRIO;
        let try3 = 2 + 2 + W_ID + W_STATE;
        let try2 = 2 + 1 + W_ID;

        if w > try5 + SLUG_FULL {
            let desc = w - try5 - SLUG_FULL;
            return Self { id: W_ID, slug: SLUG_FULL, state: W_STATE, prio: W_PRIO, desc };
        }
        if w > try4 + SLUG_FULL {
            return Self { id: W_ID, slug: SLUG_FULL, state: W_STATE, prio: W_PRIO, desc: 0 };
        }
        if w > try3 + SLUG_FULL {
            return Self { id: W_ID, slug: SLUG_FULL, state: W_STATE, prio: 0, desc: 0 };
        }
        let slug = w.saturating_sub(try2).max(1);
        Self { id: W_ID, slug, state: 0, prio: 0, desc: 0 }
    }

    pub fn active_widths(&self) -> Vec<u16> {
        [self.id, self.slug, self.state, self.prio, self.desc]
            .iter()
            .copied()
            .filter(|&w| w > 0)
            .collect()
    }
}

// -----------------------------------------------------------------------
// Helpers
// -----------------------------------------------------------------------

pub fn pad(s: &str, width: u16) -> String {
    let w = width as usize;
    let char_count = s.chars().count();
    if char_count >= w {
        s.chars().take(w).collect()
    } else {
        format!("{}{}", s, " ".repeat(w - char_count))
    }
}

fn top_border(ws: &ColWidths) -> Line<'static> {
    let inner: Vec<String> = ws.active_widths().iter()
        .map(|&w| "\u{2500}".repeat(w as usize))
        .collect();
    let line = format!("\u{250c}{}\u{2510}", inner.join("\u{252c}"));
    Line::from(Span::raw(line))
}

fn bottom_border(ws: &ColWidths) -> Line<'static> {
    let inner: Vec<String> = ws.active_widths().iter()
        .map(|&w| "\u{2500}".repeat(w as usize))
        .collect();
    let line = format!("\u{2514}{}\u{2518}", inner.join("\u{2534}"));
    Line::from(Span::raw(line))
}

fn data_line<'a>(
    id: &str,
    slug: &str,
    state: &str,
    prio: &str,
    desc: &str,
    ws: &ColWidths,
    style: Style,
) -> Line<'a> {
    let mut cells: Vec<String> = vec![
        format!(" {}", pad(id,   ws.id.saturating_sub(1))),
        format!(" {}", pad(slug, ws.slug.saturating_sub(1))),
    ];
    if ws.state > 0 { cells.push(format!(" {}", pad(state, ws.state.saturating_sub(1)))); }
    if ws.prio  > 0 { cells.push(format!(" {}", pad(prio,  ws.prio.saturating_sub(1)))); }
    if ws.desc  > 0 { cells.push(format!(" {}", pad(desc,  ws.desc.saturating_sub(1)))); }
    let line = format!("\u{2502}{}\u{2502}", cells.join("\u{2502}"));
    Line::from(Span::styled(line, style))
}

// -----------------------------------------------------------------------
// Ordered thread list (flat, matching display order)
// -----------------------------------------------------------------------

pub fn build_ordered_threads(threads: &[Thread]) -> Vec<&Thread> {
    let mut ordered: Vec<&Thread> = Vec::new();

    // Active threads
    ordered.extend(threads.iter().filter(|t| t.state == ThreadState::Active));

    // Priority & Incidents (incidents first)
    let is_inactive = |t: &&Thread| matches!(t.state, ThreadState::Paused | ThreadState::Waiting { .. });
    let mut focus: Vec<&Thread> = threads
        .iter()
        .filter(|t| is_inactive(t) && matches!(t.priority, Priority::Incident | Priority::Priority))
        .collect();
    focus.sort_by_key(|t| match t.priority {
        Priority::Incident => 0u8,
        _ => 1,
    });
    ordered.extend(focus);

    // Background
    ordered.extend(
        threads.iter().filter(|t| {
            is_inactive(t) && matches!(t.priority, Priority::Background)
        }),
    );

    ordered
}

// -----------------------------------------------------------------------
// Section rendering
// -----------------------------------------------------------------------

fn section_lines(
    title: &str,
    section_threads: &[&Thread],
    ws: &ColWidths,
    selected: usize,
    start_idx: usize,
) -> Vec<Line<'static>> {
    if section_threads.is_empty() {
        return vec![];
    }
    let mut lines: Vec<Line<'static>> = Vec::new();
    lines.push(Line::from(Span::styled(
        format!("> {}", title),
        Style::default().bold(),
    )));
    lines.push(top_border(ws));
    for (i, t) in section_threads.iter().enumerate() {
        let idx = start_idx + i;
        let style = if idx == selected {
            Style::default().bg(Color::DarkGray).fg(Color::White)
        } else {
            Style::default()
        };
        lines.push(data_line(
            &t.id_str(),
            &t.slug,
            &t.state.to_string(),
            &t.priority.to_string(),
            &t.description,
            ws,
            style,
        ));
    }
    lines.push(bottom_border(ws));
    lines
}

// -----------------------------------------------------------------------
// Thread row counting
// -----------------------------------------------------------------------

pub fn count_rows(threads: &[Thread]) -> usize {
    let is_inactive = |t: &&Thread| matches!(t.state, ThreadState::Paused | ThreadState::Waiting { .. });
    let active = threads.iter().filter(|t| t.state == ThreadState::Active).count();
    let focus = threads.iter().filter(|t| {
        is_inactive(t) && matches!(t.priority, Priority::Incident | Priority::Priority)
    }).count();
    let bg = threads.iter().filter(|t| {
        is_inactive(t) && matches!(t.priority, Priority::Background)
    }).count();

    let counts = [active, focus, bg];
    let non_empty = counts.iter().filter(|&&n| n > 0).count();
    let separators = non_empty.saturating_sub(1);

    let mut rows = separators;
    for n in counts {
        if n > 0 {
            rows += 1 + 1 + n + 1;
        }
    }
    rows
}

// -----------------------------------------------------------------------
// Main render function for threads pane
// -----------------------------------------------------------------------

use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};

pub fn render(
    frame: &mut Frame,
    threads: &[Thread],
    error: Option<&str>,
    scroll: usize,
    selected: usize,
    show_help: bool,
) {
    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(area);
    let main_area = chunks[0];
    let status_area = chunks[1];

    // --- Main area ---
    if let Some(msg) = error {
        frame.render_widget(Paragraph::new(msg), main_area);
    } else {
        let ws = ColWidths::from_area(main_area.width);
        let ordered = build_ordered_threads(threads);

        // Split ordered list into sections
        let active_threads: Vec<&Thread> = ordered.iter().copied().filter(|t| t.state == ThreadState::Active).collect();
        let focus_threads: Vec<&Thread> = ordered.iter().copied().filter(|t| {
            matches!(t.state, ThreadState::Paused | ThreadState::Waiting { .. })
                && matches!(t.priority, Priority::Incident | Priority::Priority)
        }).collect();
        let bg_threads: Vec<&Thread> = ordered.iter().copied().filter(|t| {
            matches!(t.state, ThreadState::Paused | ThreadState::Waiting { .. })
                && matches!(t.priority, Priority::Background)
        }).collect();

        let mut lines: Vec<Line> = Vec::new();
        let mut idx_offset = 0;

        let sections: Vec<(&str, &[&Thread])> = vec![
            ("Active", &active_threads),
            ("Priority & Incidents", &focus_threads),
            ("Background", &bg_threads),
        ];

        for (title, section_threads) in &sections {
            let s = section_lines(title, section_threads, &ws, selected, idx_offset);
            if s.is_empty() { continue; }
            if !lines.is_empty() { lines.push(Line::from("")); }
            lines.extend(s);
            idx_offset += section_threads.len();
        }

        frame.render_widget(Paragraph::new(lines).scroll((scroll as u16, 0)), main_area);
    }

    // --- Status bar ---
    let active_thread = threads.iter().find(|t| t.state == ThreadState::Active);
    let status_text = match active_thread {
        Some(t) => format!("  #{} {}   |   ? help   |   gt tasks", t.id_str(), t.slug),
        None    => "  (no active thread)   |   ? help   |   gt tasks".to_string(),
    };
    frame.render_widget(
        Paragraph::new(status_text).style(Style::default().bg(Color::DarkGray)),
        status_area,
    );

    // --- Help popup overlay ---
    if show_help {
        render_help_popup(frame, area);
    }
}

fn render_help_popup(frame: &mut Frame, area: Rect) {
    const POPUP_W: u16 = 40;
    const POPUP_H: u16 = 16;

    let x = area.x + area.width.saturating_sub(POPUP_W) / 2;
    let y = area.y + area.height.saturating_sub(POPUP_H) / 2;
    let popup_area = Rect::new(x, y, POPUP_W.min(area.width), POPUP_H.min(area.height));

    let key_style = Style::default().bold();

    let bindings: &[(&str, &str)] = &[
        ("j / \u{2193}",    "move down"),
        ("k / \u{2191}",    "move up"),
        ("Enter",    "view tasks for thread"),
        ("gt",       "tasks for active thread"),
        ("ctrl-d",   "page down"),
        ("ctrl-u",   "page up"),
        ("gg",       "jump to top"),
        ("G",        "jump to bottom"),
        ("?",        "toggle this help"),
        ("q / esc",  "quit"),
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
