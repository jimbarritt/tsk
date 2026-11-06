//! The tmux sidebar: a narrow pane on the left that lists every tmux session with the
//! state, mission and robot of its agent. `prefix b` opens it, focuses it, or closes it
//! when it is focused. While it is open, the tmux hooks of `tsk tmux install` move the
//! same pane into each session and window the client switches to, so the program keeps
//! running and its selection stays.
//!
//! - `tsk tmux sidebar` runs the list in the current pane.
//! - `tsk tmux sidebar toggle <window>` opens, focuses or closes it.
//! - `tsk tmux sidebar follow <window>` moves it into the window, while it is open.

use super::tmux;

/// The global option that records whether the sidebar is open.
pub const OPEN_OPTION: &str = "@tsk_sidebar_open";
/// The pane option that marks the sidebar pane.
pub const PANE_OPTION: &str = "@tsk_sidebar";
/// The global option that sets the width of the sidebar, in columns.
pub const WIDTH_OPTION: &str = "@tsk_sidebar_width";
pub const DEFAULT_WIDTH: u16 = 32;

/// The command that the sidebar pane runs.
const SIDEBAR_COMMAND: &str = "tsk tmux sidebar";

/// The fields of one pane, in the order of [`PANE_FORMAT`].
const PANE_FORMAT: &str = "#{session_name}\t#{pane_id}\t#{window_id}\t#{pane_active}\t#{@tsk_sidebar}\t#{@tsk_state}\t#{@tsk_mission_name}\t#{@tsk_actor}";

/// One session in the list, with the values of its agent pane.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Row {
    pub session: String,
    pub state: String,
    pub mission_name: String,
    pub actor: String,
}

/// The sidebar pane and its window, when one exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidebarPane {
    pub pane: String,
    pub window: String,
    pub active: bool,
}

/// Parse `list-panes -a` output in [`PANE_FORMAT`] into one row per session, in tmux's
/// order. The agent pane of a session is its first pane, other than the sidebar, that has
/// a tsk option set.
pub fn parse_rows(text: &str) -> Vec<Row> {
    let mut rows: Vec<Row> = Vec::new();
    let mut has_agent: Vec<bool> = Vec::new();
    for line in text.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 8 {
            continue;
        }
        let index = match rows.iter().position(|r| r.session == f[0]) {
            Some(i) => i,
            None => {
                rows.push(Row {
                    session: f[0].to_string(),
                    ..Row::default()
                });
                has_agent.push(false);
                rows.len() - 1
            }
        };
        let is_sidebar = f[4] == "1";
        let is_agent = !(f[5].is_empty() && f[6].is_empty() && f[7].is_empty());
        if is_sidebar || !is_agent || has_agent[index] {
            continue;
        }
        has_agent[index] = true;
        rows[index].state = f[5].to_string();
        rows[index].mission_name = f[6].to_string();
        rows[index].actor = f[7].to_string();
    }
    rows
}

/// Find the sidebar pane in `list-panes -a` output in [`PANE_FORMAT`].
pub fn parse_sidebar(text: &str) -> Option<SidebarPane> {
    text.lines().find_map(|line| {
        let f: Vec<&str> = line.split('\t').collect();
        (f.len() >= 8 && f[4] == "1").then(|| SidebarPane {
            pane: f[1].to_string(),
            window: f[2].to_string(),
            active: f[3] == "1",
        })
    })
}

fn list_panes() -> String {
    tmux(&["list-panes", "-a", "-F", PANE_FORMAT]).unwrap_or_default()
}

fn glyph(state: &str) -> &'static str {
    match state {
        "waiting" => "●",
        "working" => "◐",
        "error" => "✗",
        _ => " ",
    }
}

/// Cut a line to `width` characters, with `…` when cut.
pub fn fit(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        return text.to_string();
    }
    let mut out: String = text.chars().take(width.saturating_sub(1)).collect();
    out.push('…');
    out
}

/// The lines of the list. `current` is the session the sidebar is in, `selected` the
/// index of the selected row. Each session has a line for its name, and a line each for
/// its mission name and its robot when it has them.
pub fn render(rows: &[Row], current: &str, selected: usize, width: usize) -> Vec<String> {
    let mut lines = vec![fit(" tsk sessions", width), String::new()];
    for (i, row) in rows.iter().enumerate() {
        let marker = if row.session == current { "▸" } else { " " };
        let number = if i < 9 {
            format!("{}", i + 1)
        } else {
            " ".to_string()
        };
        let head = fit(
            &format!("{}{} {} {}", marker, number, glyph(&row.state), row.session),
            width,
        );
        if i == selected {
            lines.push(format!("\x1b[7m{:<w$}\x1b[0m", head, w = width));
        } else {
            lines.push(head);
        }
        if !row.mission_name.is_empty() {
            lines.push(fit(&format!("     {}", row.mission_name), width));
        }
        if !row.actor.is_empty() {
            lines.push(format!(
                "\x1b[2m{}\x1b[0m",
                fit(&format!("     {}", row.actor), width)
            ));
        }
        lines.push(String::new());
    }
    lines
}

const HELP: &str = " j k move  ⏎ go  q close";

fn width_option() -> u16 {
    tmux(&["show-options", "-gqv", WIDTH_OPTION])
        .and_then(|v| v.trim().parse::<u16>().ok())
        .filter(|w| *w >= 12)
        .unwrap_or(DEFAULT_WIDTH)
}

fn set_open(open: bool) {
    let _ = tmux(&[
        "set-option",
        "-g",
        OPEN_OPTION,
        if open { "on" } else { "off" },
    ]);
}

fn is_open() -> bool {
    tmux(&["show-options", "-gqv", OPEN_OPTION]).as_deref() == Some("on")
}

/// Split a new sidebar pane on the left of the window and mark it. `focus` makes it the
/// active pane.
fn create(window: &str, focus: bool) {
    let width = width_option().to_string();
    let mut args = vec![
        "split-window",
        "-h",
        "-b",
        "-f",
        "-l",
        &width,
        "-t",
        window,
        "-P",
        "-F",
        "#{pane_id}",
    ];
    if !focus {
        args.push("-d");
    }
    args.push(SIDEBAR_COMMAND);
    if let Some(pane) = tmux(&args) {
        let _ = tmux(&["set-option", "-p", "-t", pane.trim(), PANE_OPTION, "1"]);
    }
}

/// Move the sidebar pane into the window, on the left, at its width.
fn join(sidebar: &SidebarPane, window: &str, focus: bool) {
    let width = width_option().to_string();
    let mut args = vec!["join-pane", "-h", "-b", "-f", "-l", &width];
    if !focus {
        args.push("-d");
    }
    args.extend(["-s", &sidebar.pane, "-t", window]);
    let _ = tmux(&args);
}

/// `prefix b`: open the sidebar in the window, focus it when it is open and not focused,
/// and close it when it is focused.
pub fn toggle(window: &str) {
    match parse_sidebar(&list_panes()) {
        None => {
            set_open(true);
            create(window, true);
        }
        Some(sidebar) if sidebar.window != window => {
            set_open(true);
            join(&sidebar, window, true);
        }
        Some(sidebar) if !sidebar.active => {
            let _ = tmux(&["select-pane", "-t", &sidebar.pane]);
        }
        Some(sidebar) => {
            set_open(false);
            let _ = tmux(&["kill-pane", "-t", &sidebar.pane]);
        }
    }
}

/// The hooks: while the sidebar is open, move it into the window the client now shows,
/// and leave the focus where it is.
pub fn follow(window: &str) {
    if !is_open() {
        return;
    }
    match parse_sidebar(&list_panes()) {
        None => create(window, false),
        Some(sidebar) if sidebar.window != window => join(&sidebar, window, false),
        Some(_) => {}
    }
}

#[cfg(unix)]
pub fn run() -> Result<i32, String> {
    let pane = super::current_pane()
        .ok_or_else(|| "error: tsk tmux sidebar runs in a tmux pane".to_string())?;
    let _ = tmux(&["set-option", "-p", "-t", &pane, PANE_OPTION, "1"]);
    let _ = tmux(&["select-pane", "-t", &pane, "-T", "tsk sessions"]);
    let terminal = raw::Raw::enter();
    let mut selected = 0usize;
    let mut out = std::io::stdout();
    loop {
        let rows = parse_rows(&list_panes());
        let current =
            tmux(&["display-message", "-p", "-t", &pane, "#{session_name}"]).unwrap_or_default();
        if selected >= rows.len() {
            selected = rows.len().saturating_sub(1);
        }
        let (width, height) = raw::size();
        draw(&mut out, &rows, &current, selected, width, height);
        match raw::key(1000) {
            Some(Key::Down) if selected + 1 < rows.len() => selected += 1,
            Some(Key::Up) => selected = selected.saturating_sub(1),
            Some(Key::Number(n)) if n < rows.len() => {
                selected = n;
                go(&rows[n].session);
            }
            Some(Key::Enter) => {
                if let Some(row) = rows.get(selected) {
                    go(&row.session);
                }
            }
            Some(Key::Quit) => break,
            _ => {}
        }
    }
    drop(terminal);
    set_open(false);
    Ok(0)
}

#[cfg(not(unix))]
pub fn run() -> Result<i32, String> {
    Err("error: tsk tmux sidebar needs a Unix terminal".to_string())
}

/// Switch the client to the session. The hooks then move the sidebar after it.
fn go(session: &str) {
    let target = format!("={}", session);
    let _ = tmux(&["switch-client", "-t", &target]);
}

#[cfg(unix)]
fn draw(
    out: &mut impl std::io::Write,
    rows: &[Row],
    current: &str,
    selected: usize,
    width: usize,
    height: usize,
) {
    let mut lines = render(rows, current, selected, width);
    let body = height.saturating_sub(1);
    lines.truncate(body);
    let mut text = String::from("\x1b[H\x1b[2J");
    for line in &lines {
        text.push_str(line);
        text.push_str("\r\n");
    }
    for _ in lines.len()..body {
        text.push_str("\r\n");
    }
    text.push_str(&format!("\x1b[2m{}\x1b[0m", fit(HELP, width)));
    let _ = out.write_all(text.as_bytes());
    let _ = out.flush();
}

#[derive(Debug, PartialEq, Eq)]
pub enum Key {
    Up,
    Down,
    Enter,
    Quit,
    Number(usize),
}

/// The key in the bytes of one read.
pub fn parse_key(bytes: &[u8]) -> Option<Key> {
    match bytes {
        [b'k', ..] | [0x1b, b'[', b'A', ..] | [0x1b, b'O', b'A', ..] => Some(Key::Up),
        [b'j', ..] | [0x1b, b'[', b'B', ..] | [0x1b, b'O', b'B', ..] => Some(Key::Down),
        [b'\r', ..] | [b'\n', ..] => Some(Key::Enter),
        [b'q', ..] => Some(Key::Quit),
        [d @ b'1'..=b'9', ..] => Some(Key::Number(usize::from(d - b'1'))),
        _ => None,
    }
}

#[cfg(unix)]
mod raw {
    use super::Key;

    /// The terminal in non-canonical mode with no echo and the cursor hidden, until drop.
    pub struct Raw {
        saved: Option<libc::termios>,
    }

    impl Raw {
        pub fn enter() -> Raw {
            // SAFETY: tcgetattr and tcsetattr read and write a termios struct for fd 0,
            // which lives on this stack frame for the call.
            let saved = unsafe {
                let mut t: libc::termios = std::mem::zeroed();
                if libc::tcgetattr(0, &mut t) != 0 {
                    None
                } else {
                    let saved = t;
                    t.c_lflag &= !(libc::ICANON | libc::ECHO);
                    t.c_cc[libc::VMIN] = 1;
                    t.c_cc[libc::VTIME] = 0;
                    libc::tcsetattr(0, libc::TCSANOW, &t);
                    Some(saved)
                }
            };
            print!("\x1b[?25l");
            Raw { saved }
        }
    }

    impl Drop for Raw {
        fn drop(&mut self) {
            print!("\x1b[?25h\x1b[H\x1b[2J");
            if let Some(saved) = self.saved {
                // SAFETY: restores the termios struct read in `enter`.
                unsafe {
                    libc::tcsetattr(0, libc::TCSANOW, &saved);
                }
            }
        }
    }

    /// The terminal size in columns and rows, 32 by 24 when the call fails.
    pub fn size() -> (usize, usize) {
        // SAFETY: TIOCGWINSZ writes a winsize struct that lives on this stack frame.
        unsafe {
            let mut w: libc::winsize = std::mem::zeroed();
            if libc::ioctl(1, libc::TIOCGWINSZ, &mut w) == 0 && w.ws_col > 0 && w.ws_row > 0 {
                (usize::from(w.ws_col), usize::from(w.ws_row))
            } else {
                (32, 24)
            }
        }
    }

    /// Wait up to `timeout_ms` for a key. `None` on a timeout or an unknown key. End of
    /// input reads as `Quit`, so the program ends when its pane closes.
    pub fn key(timeout_ms: i32) -> Option<Key> {
        let mut fds = libc::pollfd {
            fd: 0,
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: poll and read use a pollfd and a buffer that live on this stack frame.
        unsafe {
            if libc::poll(&mut fds, 1, timeout_ms) <= 0 {
                return None;
            }
            let mut buf = [0u8; 16];
            let n = libc::read(0, buf.as_mut_ptr().cast(), buf.len());
            if n <= 0 {
                return Some(Key::Quit);
            }
            super::parse_key(&buf[..n as usize])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PANES: &str = "M-TSK-IDEA\t%3\t@2\t0\t1\t\t\t\n\
M-TSK-IDEA\t%0\t@0\t1\t\twaiting\tIdea capture\tRetracted Before Anyone Noticed\n\
M-TSK-IDEA\t%4\t@0\t0\t\tworking\tOther\tSecond agent\n\
notes\t%1\t@1\t1\t\t\t\t\n\
M-TSK-SCALE\t%2\t@3\t1\t\tworking\tLarge scale…\tOther Robot\n";

    #[test]
    fn each_session_is_one_row_with_its_first_agent_pane() {
        let rows = parse_rows(PANES);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].session, "M-TSK-IDEA");
        assert_eq!(rows[0].state, "waiting");
        assert_eq!(rows[0].mission_name, "Idea capture");
        assert_eq!(rows[0].actor, "Retracted Before Anyone Noticed");
        assert_eq!(
            rows[1],
            Row {
                session: "notes".into(),
                ..Row::default()
            }
        );
        assert_eq!(rows[2].session, "M-TSK-SCALE");
        assert_eq!(rows[2].state, "working");
    }

    #[test]
    fn the_sidebar_pane_is_the_marked_pane() {
        assert_eq!(
            parse_sidebar(PANES),
            Some(SidebarPane {
                pane: "%3".into(),
                window: "@2".into(),
                active: false
            })
        );
        assert_eq!(parse_sidebar("a\t%1\t@1\t1\t\t\t\t\n"), None);
    }

    #[test]
    fn the_list_marks_the_current_session_and_the_selection() {
        let rows = parse_rows(PANES);
        let lines = render(&rows, "M-TSK-SCALE", 0, 32);
        assert_eq!(lines[0], " tsk sessions");
        assert!(lines[2].starts_with("\x1b[7m 1 ● M-TSK-IDEA"));
        assert_eq!(lines[3], "     Idea capture");
        assert!(lines[4].contains("     Retracted Before Anyone"));
        assert!(lines.iter().any(|l| l == "▸3 ◐ M-TSK-SCALE"));
        assert!(lines.iter().any(|l| l == " 2   notes"));
    }

    #[test]
    fn a_long_line_is_cut_with_an_ellipsis() {
        assert_eq!(fit("abcdef", 4), "abc…");
        assert_eq!(fit("abc", 4), "abc");
    }

    #[test]
    fn keys_parse_from_letters_arrows_and_digits() {
        assert_eq!(parse_key(b"j"), Some(Key::Down));
        assert_eq!(parse_key(b"\x1b[A"), Some(Key::Up));
        assert_eq!(parse_key(b"\x1bOB"), Some(Key::Down));
        assert_eq!(parse_key(b"\r"), Some(Key::Enter));
        assert_eq!(parse_key(b"q"), Some(Key::Quit));
        assert_eq!(parse_key(b"3"), Some(Key::Number(2)));
        assert_eq!(parse_key(b"0"), None);
        assert_eq!(parse_key(b"x"), None);
    }
}
