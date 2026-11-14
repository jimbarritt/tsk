//! The tmux adapter (ADR 0013): tsk sets user options on the tmux pane of an agent, and
//! a key binding that tsk installs shows them in the session chooser.
//!
//! - `tsk tmux install` writes `${XDG_CONFIG_HOME:-~/.config}/tsk/tmux.conf` and adds one
//!   `source-file` line to the tmux config.
//! - `tsk tmux sync` sets `@tsk_actor`, `@tsk_mission` and `@tsk_mission_name` on the pane
//!   in `$TMUX_PANE`, and names the tmux session after the mission code. The thread
//!   commands and the session start run it too.
//! - `tsk tmux state <state>` sets `@tsk_state`. The plugin's hooks run it.
//!
//! Outside tmux (`$TMUX_PANE` unset) `sync` and `state` do nothing and exit 0. A failure
//! of `tmux` never fails the command that called it.

use std::path::{Path, PathBuf};
use std::process::Command;

use clap::{Subcommand, ValueEnum};

use crate::thread::session::Session;

mod sidebar;

pub const TMUX_BIN_VAR: &str = "TSK_TMUX_BIN";

/// The line that marks the `source-file` line in the tmux config.
pub const SOURCE_MARKER: &str = "# tsk tmux install: shows the mission, the robot and its state";

/// The longest mission name the chooser shows, in characters, before it is cut.
pub const MISSION_NAME_MAX: usize = 24;

/// One line of the chooser: the state, the short mission name, the actor and the last
/// component of the pane's working directory, in columns. tmux prints the session name
/// before the format, so the format starts with spaces that make the name 18 characters
/// wide; the mission name is padded to 24 and the actor to 32. Needs tmux 3.2 or later.
pub const CHOOSER_FORMAT: &str = "#{s/^#{session_name}//:#{p18:session_name}}#{?#{==:#{@tsk_state},waiting},● ,#{?#{==:#{@tsk_state},working},◐ ,#{?#{==:#{@tsk_state},error},✗ ,  }}}#{p24:#{@tsk_mission_name}}  #{p32:#{@tsk_actor}}  #{b:pane_current_path}";

#[derive(Subcommand)]
pub enum TmuxCommands {
    #[command(
        about = "Install the tsk tmux settings: write ${XDG_CONFIG_HOME:-~/.config}/tsk/tmux.conf (a session chooser on prefix s that shows each agent's state, mission and robot, and session names after mission codes) and add one source-file line to the tmux config. Run it again to update the settings. With a tmux server running, it loads them at once"
    )]
    Install {
        #[arg(
            long,
            value_name = "PATH",
            help = "The tmux config to add the source-file line to (default: $XDG_CONFIG_HOME/tmux/tmux.conf or ~/.config/tmux/tmux.conf when it exists, else ~/.tmux.conf)"
        )]
        tmux_conf: Option<PathBuf>,
        #[arg(long, help = "Print the settings and change no file")]
        print: bool,
    },
    #[command(
        about = "Set the tsk options on this tmux pane ($TMUX_PANE): @tsk_actor, @tsk_mission and @tsk_mission_name, and name the tmux session after the mission code when @tsk_rename_session is on. Reads the ledger store as it is and never fetches. Does nothing outside tmux"
    )]
    Sync,
    #[command(
        about = "Set the agent state of this tmux pane ($TMUX_PANE) in @tsk_state, which the session chooser shows. The plugin's hooks run it. Does nothing outside tmux, and never reads the ledger"
    )]
    State {
        #[arg(value_enum)]
        state: AgentState,
    },
    #[command(
        about = "Run the tsk sidebar in this pane: every tmux session with its agent's state, mission and robot. j and k move, Enter or a digit switches to the session, q closes it. prefix b opens it, focuses it, or closes it when focused, and while it is open it follows you to each session and window (tsk tmux install sets up the key and the hooks)"
    )]
    Sidebar {
        #[command(subcommand)]
        action: Option<SidebarCommands>,
    },
}

#[derive(Subcommand)]
pub enum SidebarCommands {
    #[command(
        about = "Open the sidebar in the window, focus it when it is open and not focused, or close it when it is focused. The prefix b binding runs it"
    )]
    Toggle {
        #[arg(value_name = "WINDOW_ID")]
        window: String,
    },
    #[command(
        about = "Move the open sidebar into the window, and keep the focus where it is. The tmux hooks run it when the client changes session or window"
    )]
    Follow {
        #[arg(value_name = "WINDOW_ID")]
        window: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum AgentState {
    /// The agent waits for input: a finished turn, a permission prompt, a notification.
    Waiting,
    /// The agent works on a turn.
    Working,
    /// The session ended with an error.
    Error,
    /// No agent runs in the pane: the option is removed.
    None,
}

impl AgentState {
    fn value(self) -> Option<&'static str> {
        match self {
            AgentState::Waiting => Some("waiting"),
            AgentState::Working => Some("working"),
            AgentState::Error => Some("error"),
            AgentState::None => None,
        }
    }
}

pub fn run(action: TmuxCommands) -> Result<i32, String> {
    match action {
        TmuxCommands::Install { tmux_conf, print } => install(tmux_conf, print).map(|()| 0),
        TmuxCommands::Sync => {
            if let Some(pane) = current_pane() {
                if let Ok(session) = Session::discover() {
                    sync_pane(&session, &pane);
                }
            }
            Ok(0)
        }
        TmuxCommands::Sidebar { action } => match action {
            None => sidebar::run(),
            Some(SidebarCommands::Toggle { window }) => {
                sidebar::toggle(&window);
                Ok(0)
            }
            Some(SidebarCommands::Follow { window }) => {
                sidebar::follow(&window);
                Ok(0)
            }
        },
        TmuxCommands::State { state } => {
            if let Some(pane) = current_pane() {
                set_option(&pane, "@tsk_state", state.value());
            }
            Ok(0)
        }
    }
}

/// The tmux pane of this process, from `$TMUX_PANE`. `None` outside tmux.
pub fn current_pane() -> Option<String> {
    std::env::var("TMUX_PANE")
        .ok()
        .filter(|pane| is_pane_id(pane))
}

/// A tmux pane ID: `%` and digits.
fn is_pane_id(value: &str) -> bool {
    value.len() > 1 && value.starts_with('%') && value[1..].bytes().all(|b| b.is_ascii_digit())
}

/// Set the options on this pane when the process runs in tmux. For the thread commands and
/// the session start: it never fails and prints nothing.
pub fn sync_if_in_tmux(session: &Session) {
    if let Some(pane) = current_pane() {
        sync_pane(session, &pane);
    }
}

/// The values that `sync` writes: the actor name, the mission code and the short mission
/// name. Each is `None` when it is unknown.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct PaneValues {
    pub actor: Option<String>,
    pub mission: Option<String>,
    pub mission_name: Option<String>,
}

fn sync_pane(session: &Session, pane: &str) {
    let values = crate::thread::pane_values(session);
    set_option(pane, "@tsk_actor", values.actor.as_deref());
    set_option(pane, "@tsk_mission", values.mission.as_deref());
    set_option(pane, "@tsk_mission_name", values.mission_name.as_deref());
    if let Some(code) = values.mission.as_deref() {
        rename_session(pane, code);
    }
}

/// The tmux program: `$TSK_TMUX_BIN`, else `tmux`.
pub fn tmux_bin() -> String {
    std::env::var(TMUX_BIN_VAR).unwrap_or_else(|_| "tmux".to_string())
}

fn tmux(args: &[&str]) -> Option<String> {
    let output = Command::new(tmux_bin()).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(
        String::from_utf8_lossy(&output.stdout)
            .trim_end()
            .to_string(),
    )
}

/// Set a pane option, or remove it with `None`.
fn set_option(pane: &str, name: &str, value: Option<&str>) {
    let _ = match value {
        Some(value) => tmux(&["set-option", "-p", "-t", pane, name, value]),
        None => tmux(&["set-option", "-p", "-u", "-t", pane, name]),
    };
}

/// Name the tmux session of the pane after the mission code, when the global option
/// `@tsk_rename_session` is `on`. tmux refuses a name that another session has, and the
/// session then keeps its name.
fn rename_session(pane: &str, code: &str) {
    let enabled = tmux(&["show-options", "-gqv", "@tsk_rename_session"]);
    if enabled.as_deref() != Some("on") {
        return;
    }
    let Some(current) = tmux(&["display-message", "-p", "-t", pane, "#{session_name}"]) else {
        return;
    };
    let target = session_name_for(code);
    if current != target {
        let _ = tmux(&["rename-session", "-t", &current, &target]);
    }
}

/// tmux replaces `.` and `:` in a session name with `_`. A mission code has neither, but a
/// name that tmux changes would never match the current name, so tsk changes it the same way.
pub fn session_name_for(code: &str) -> String {
    code.replace(['.', ':'], "_")
}

/// A mission name cut to `max` characters at a word boundary, with `…` when cut. A first
/// word longer than `max` is cut inside the word.
pub fn short_name(name: &str, max: usize) -> String {
    let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
    if name.chars().count() <= max {
        return name;
    }
    let mut out = String::new();
    for word in name.split(' ') {
        let len = out.chars().count() + usize::from(!out.is_empty()) + word.chars().count();
        if len > max.saturating_sub(1) {
            break;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(word);
    }
    if out.is_empty() {
        out = name.chars().take(max.saturating_sub(1)).collect();
    }
    let out = out.trim_end_matches([',', ';', ':', '-', ' ']).to_string();
    format!("{}…", out)
}

/// The text of the tsk tmux config. tsk overwrites the file on each install.
pub fn tsk_conf() -> String {
    format!(
        "# Written by tsk tmux install, which overwrites this file. Put your own settings in\n\
         # your tmux config.\n\
         #\n\
         # tsk sets these pane options: @tsk_state (waiting, working or error) from the\n\
         # plugin's hooks, and @tsk_actor, @tsk_mission and @tsk_mission_name from the thread\n\
         # binding.\n\
         \n\
         # Name each tmux session after the mission code of its thread. Set it to off to keep\n\
         # your own session names.\n\
         set -g @tsk_rename_session on\n\
         \n\
         # Room in the status line for a mission code such as M-TSK-BOOT-06.\n\
         if -F '#{{e|<:#{{status-left-length}},30}}' 'set -g status-left-length 30'\n\
         \n\
         # The session chooser: state, mission, robot and directory on each line.\n\
         # ● waits for input, ◐ works, ✗ ended with an error.\n\
         bind s choose-tree -Zs -F '{}'\n\
         \n\
         # The sidebar: prefix b opens it, focuses it, or closes it when it is focused. While\n\
         # it is open it follows you to each session and window. Set @tsk_sidebar_width in\n\
         # your tmux config to change its width.\n\
         if -F '#{{==:#{{@tsk_sidebar_width}},}}' 'set -g @tsk_sidebar_width 32'\n\
         bind b run-shell -b 'tsk tmux sidebar toggle #{{window_id}}'\n\
         set-hook -g 'client-session-changed[42]' \"run-shell -b 'tsk tmux sidebar follow #{{window_id}}'\"\n\
         set-hook -g 'session-window-changed[42]' \"run-shell -b 'tsk tmux sidebar follow #{{window_id}}'\"\n",
        CHOOSER_FORMAT
    )
}

/// The `source-file` lines for the tmux config.
pub fn source_lines(tsk_conf: &Path) -> String {
    format!(
        "{}\nsource-file -q \"{}\"\n",
        SOURCE_MARKER,
        tsk_conf.display()
    )
}

/// The tmux config with the `source-file` lines added at the end. `None` when the config
/// already has the marker.
pub fn with_source_lines(conf: &str, tsk_conf: &Path) -> Option<String> {
    if conf.lines().any(|line| line.trim() == SOURCE_MARKER) {
        return None;
    }
    let mut out = conf.to_string();
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    if !out.is_empty() {
        out.push('\n');
    }
    out.push_str(&source_lines(tsk_conf));
    Some(out)
}

fn home() -> Result<PathBuf, String> {
    std::env::var_os("HOME")
        .filter(|h| !h.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| "error: HOME is not set".to_string())
}

fn config_home(home: &Path) -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".config"))
}

/// The tmux config that tmux reads: the XDG file when it exists, else `~/.tmux.conf`.
pub fn default_tmux_conf(home: &Path, config_home: &Path) -> PathBuf {
    let xdg = config_home.join("tmux").join("tmux.conf");
    if xdg.is_file() {
        return xdg;
    }
    let dot_config = home.join(".config").join("tmux").join("tmux.conf");
    if dot_config.is_file() {
        return dot_config;
    }
    home.join(".tmux.conf")
}

fn install(tmux_conf: Option<PathBuf>, print: bool) -> Result<(), String> {
    let home = home()?;
    let config_home = config_home(&home);
    let tsk_conf_path = config_home.join("tsk").join("tmux.conf");
    let tmux_conf = tmux_conf.unwrap_or_else(|| default_tmux_conf(&home, &config_home));
    if print {
        println!("# {}", tsk_conf_path.display());
        print!("{}", tsk_conf());
        println!();
        println!("# added to {}", tmux_conf.display());
        print!("{}", source_lines(&tsk_conf_path));
        return Ok(());
    }

    if let Some(dir) = tsk_conf_path.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("error: could not create {}: {}", dir.display(), e))?;
    }
    std::fs::write(&tsk_conf_path, tsk_conf())
        .map_err(|e| format!("error: could not write {}: {}", tsk_conf_path.display(), e))?;
    println!("wrote {}", tsk_conf_path.display());

    let current = match std::fs::read_to_string(&tmux_conf) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => {
            return Err(format!(
                "error: could not read {}: {}",
                tmux_conf.display(),
                e
            ))
        }
    };
    match with_source_lines(&current, &tsk_conf_path) {
        Some(text) => {
            if let Some(dir) = tmux_conf.parent() {
                std::fs::create_dir_all(dir)
                    .map_err(|e| format!("error: could not create {}: {}", dir.display(), e))?;
            }
            std::fs::write(&tmux_conf, text)
                .map_err(|e| format!("error: could not write {}: {}", tmux_conf.display(), e))?;
            println!("added a source-file line to {}", tmux_conf.display());
        }
        None => println!("{} already sources it", tmux_conf.display()),
    }

    let path = tsk_conf_path.display().to_string();
    if tmux(&["source-file", &path]).is_some() {
        println!("loaded it into the running tmux server");
    } else {
        println!("no tmux server runs: tmux loads it at the next start");
    }
    println!();
    println!("Done. prefix s opens the session chooser. Each agent pane shows:");
    println!("  ● waits for input, ◐ works, ✗ ended with an error");
    println!("  the short mission name, the robot name and the directory");
    println!("The tsk plugin's hooks set the state. Update the plugin with tsk install-plugin.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_short_name_is_unchanged() {
        assert_eq!(short_name("Mission Control", 24), "Mission Control");
        assert_eq!(short_name("  Mission   Control ", 24), "Mission Control");
    }

    #[test]
    fn a_long_name_is_cut_at_a_word_with_an_ellipsis() {
        let cut = short_name("Large scale orchestration of agents", 24);
        assert_eq!(cut, "Large scale…");
        assert!(cut.chars().count() <= 24);
        assert_eq!(
            short_name("Replace Claude attribution with robot attribution", 24),
            "Replace Claude…"
        );
    }

    #[test]
    fn a_long_first_word_is_cut_inside_the_word() {
        assert_eq!(short_name("Supercalifragilistic", 10), "Supercali…");
    }

    #[test]
    fn a_cut_drops_trailing_punctuation() {
        assert_eq!(short_name("Ideas: things to try later on", 10), "Ideas…");
    }

    #[test]
    fn pane_ids_are_a_percent_and_digits() {
        assert!(is_pane_id("%0"));
        assert!(is_pane_id("%123"));
        assert!(!is_pane_id("%"));
        assert!(!is_pane_id("12"));
        assert!(!is_pane_id("%1;rm"));
    }

    #[test]
    fn session_names_replace_the_characters_tmux_replaces() {
        assert_eq!(session_name_for("M-TSK-IDEA"), "M-TSK-IDEA");
        assert_eq!(session_name_for("a.b:c"), "a_b_c");
    }

    #[test]
    fn the_source_lines_are_added_once() {
        let path = Path::new("/h/.config/tsk/tmux.conf");
        let once = with_source_lines("set -g mouse on", path).unwrap();
        assert_eq!(
            once,
            format!(
                "set -g mouse on\n\n{}\nsource-file -q \"/h/.config/tsk/tmux.conf\"\n",
                SOURCE_MARKER
            )
        );
        assert_eq!(with_source_lines(&once, path), None);
        assert_eq!(with_source_lines("", path).unwrap(), source_lines(path));
    }

    #[test]
    fn the_tsk_conf_binds_the_chooser_with_the_format() {
        let conf = tsk_conf();
        assert!(conf.contains(&format!("bind s choose-tree -Zs -F '{}'", CHOOSER_FORMAT)));
        assert!(conf.contains("set -g @tsk_rename_session on"));
        assert!(conf.contains("'#{e|<:#{status-left-length},30}'"));
        assert!(!CHOOSER_FORMAT.contains('\''));
    }

    #[test]
    fn the_xdg_tmux_conf_is_used_when_it_exists() {
        let dir = std::env::temp_dir().join(format!("tsk-tmux-test-{}", std::process::id()));
        let home = dir.join("home");
        let xdg = dir.join("xdg");
        std::fs::create_dir_all(&home).unwrap();
        assert_eq!(default_tmux_conf(&home, &xdg), home.join(".tmux.conf"));
        std::fs::create_dir_all(xdg.join("tmux")).unwrap();
        std::fs::write(xdg.join("tmux").join("tmux.conf"), "").unwrap();
        assert_eq!(
            default_tmux_conf(&home, &xdg),
            xdg.join("tmux").join("tmux.conf")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
