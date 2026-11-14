//! `tsk harness`: a harness is one code worktree, one tmux session and one `claude` process
//! on this machine (ADR 0032, and docs/design/session-and-harness-flows.md).
//!
//! - `new` asks for a thread, or a robot and a mission, makes the worktree and the tmux
//!   session, and starts `claude` with a first prompt that finishes the set-up. tsk makes no
//!   robot and no mission itself.
//! - `list`, `path` and `cd` find the harnesses of this clone.
//! - `sleep` pauses the thread and stops the tmux session. `wake` starts the session again.
//!   `delete` removes the worktree.
//!
//! The state of a harness is one JSON file under `<state>/harnesses/<clone-id>/`. The names
//! ever used are in `<state>/repos/<clone-id>/harness-names`, and a name is never used twice.
//! The worktree directory has the name of the harness, so the harness has the terminal
//! instance key `<clone-id>/<name>`.

use std::io::{BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use clap::{Args, Subcommand};

use crate::ledger::git::Git;
use crate::ledger::location::Repo;
use crate::ledger::store::LedgerStore;
use crate::thread::session::{worktree_actor, Session};
use crate::thread::{binding, ops};

mod names;
mod state;

use state::HarnessState;

/// What the first prompt says about branches when the worktree is detached: a harness worktree
/// can start with a detached HEAD, because git refuses one branch in two worktrees, and a
/// worktree holds several branches over its life.
pub const BRANCH_STEP: &str = "This worktree has no branch: its HEAD is detached at origin's default branch. Before you commit, follow this repo's rule for branches: in a repo that works on main, rebase on origin's main and push with git push origin HEAD:main; otherwise create a branch named for the work, and a new one for each pull request.";

/// The tmux session option that names the harness.
pub const TMUX_OPTION: &str = "@tsk_harness";

const NO_ROOT: &str = "error: no worktrees root is set. Set the environment variable GIT_WORKTREES_ROOT, or run tsk config worktrees-root <path> to write [worktrees] root in the user config";

#[derive(Subcommand)]
pub enum HarnessCommands {
    #[command(
        about = "Make a harness: a code worktree on a branch you name, or a detached HEAD, a tmux session in it, and claude with a first prompt",
        long_about = "Make a harness: a code worktree on a branch you name, or a detached HEAD, a tmux session in it, and claude with a first prompt.\n\nOn a terminal the command asks four things in turn. Pick up a paused thread? If so, it lists the paused threads and you pick one. If not, attach an existing robot? If so, it lists the robots, free ones first. Then it lists the missions that are not done, and you pick one or a new mission. Last it asks the branch name. A flag answers its question, so a script can run the command.\n\nThe branch question lists suggestions, in this order and only those that apply: the ticket of the current task of the thread, the mission code and the thread id, the mission code and the harness name, and a detached HEAD. Type a number, press Enter for the first suggestion, or type a branch name. tsk checks the name with git check-ref-format and refuses a branch that another worktree has checked out. A branch that exists is checked out. A new name is made from the default branch of origin. --branch <name> and --detach answer the question. With no terminal and no flag, the first suggestion is used.\n\nThe worktree is <root>/<repo name>/<harness name>. Git refuses one branch in two worktrees, so a detached HEAD at the default branch of origin leaves the agent to name the branch later. The root is GIT_WORKTREES_ROOT, else [worktrees] root in the user config (tsk config worktrees-root). The name is a random word that no harness of this clone has had. Ignored env files (.env, .env.local, .env.*.local) are copied from the main worktree.\n\nThe tmux session has the mission code as its name, else the harness name. claude starts in it with a first prompt that registers the robot, then resumes or starts the thread. tsk makes no robot and no mission itself: the new claude session does that. With no flag, the command then attaches to the session, or switches to it inside tmux."
    )]
    New(NewArgs),
    #[command(
        about = "List the harnesses of this clone: name, robot, mission, thread, awake or asleep, and directory",
        long_about = "List the harnesses of this clone: name, robot, mission, thread, awake or asleep, and directory.\n\nA harness is awake while its tmux session exists, and asleep otherwise. --porcelain prints tab-separated lines with no header and the full directory. --json prints an array."
    )]
    List {
        #[arg(
            long,
            conflicts_with = "json",
            help = "Print tab-separated lines with no header and the full path"
        )]
        porcelain: bool,
        #[arg(long, help = "Print a JSON array")]
        json: bool,
    },
    #[command(
        about = "Print the worktree path of a harness",
        long_about = "Print the worktree path of a harness.\n\nThe name can be a prefix, as a git short commit ID can. A name that matches none or more than one harness exits 1 and lists the candidates."
    )]
    Path {
        #[arg(help = "The harness name, or a prefix that matches one harness")]
        name: String,
    },
    #[command(
        about = "Change to the worktree of a harness. Needs the shell function from tsk install zshrc",
        long_about = "Change to the worktree of a harness.\n\nA program cannot change the directory of its shell, so this command only explains that. The shell function that tsk install zshrc writes does the change. Without it, run: cd \"$(tsk harness path <name>)\"."
    )]
    Cd {
        #[arg(help = "The harness name, or a prefix that matches one harness")]
        name: String,
    },
    #[command(
        about = "Put a harness to sleep: pause its thread and stop its tmux session. The worktree stays",
        long_about = "Put a harness to sleep: pause its thread and stop its tmux session. The worktree stays.\n\nWhen this harness holds a thread, the command writes a pause entry for it first. The commit at HEAD must be on origin for the pause, as for tsk thread pause. Then it stops the tmux session, which ends claude. The worktree and the state stay, so tsk harness wake can start the harness again."
    )]
    Sleep {
        #[arg(help = "The harness name, or a prefix that matches one harness")]
        name: String,
    },
    #[command(
        about = "Wake a harness: start its tmux session again and resume its thread in claude",
        long_about = "Wake a harness: start its tmux session again and resume its thread in claude.\n\nThe tmux session has the mission code as its name, else the harness name. claude starts with a first prompt that registers the robot, when it is known, and resumes the thread. With no flag, the command then attaches to the session, or switches to it inside tmux."
    )]
    Wake {
        #[arg(help = "The harness name, or a prefix that matches one harness")]
        name: String,
        #[arg(long, help = "Do not start claude in the session")]
        no_claude: bool,
        #[arg(long, help = "Do not attach to the tmux session")]
        no_attach: bool,
    },
    #[command(
        about = "Delete a harness: stop its tmux session and remove its worktree. The name stays retired",
        long_about = "Delete a harness: stop its tmux session and remove its worktree. The name stays retired.\n\nThe command refuses when the worktree has uncommitted changes, or when its HEAD has commits that are on no remote branch. --force deletes anyway. When this harness holds a thread, the command pauses it first. Branches made in the worktree stay, as every branch of the repo does."
    )]
    Delete {
        #[arg(help = "The harness name, or a prefix that matches one harness")]
        name: String,
        #[arg(
            long,
            help = "Delete although the worktree has changes or unpushed commits"
        )]
        force: bool,
    },
}

#[derive(Args)]
pub struct NewArgs {
    #[arg(
        long,
        value_name = "ID",
        conflicts_with_all = ["mission", "new_mission", "new_robot"],
        help = "Resume this paused thread. Skips the questions"
    )]
    thread: Option<String>,
    #[arg(
        long,
        value_name = "NAME",
        conflicts_with = "new_robot",
        help = "Attach the existing robot of this name. Skips the robot question"
    )]
    robot: Option<String>,
    #[arg(long, help = "Make a new robot. Skips the robot question")]
    new_robot: bool,
    #[arg(
        long,
        value_name = "ID",
        conflicts_with = "new_mission",
        help = "Start a thread on this mission. Skips the mission question"
    )]
    mission: Option<String>,
    #[arg(long, help = "Create a new mission. Skips the mission question")]
    new_mission: bool,
    #[arg(
        long,
        value_name = "WORD",
        help = "Use this name, which no harness of this clone has had. Lower case letters, digits and hyphens"
    )]
    name: Option<String>,
    #[arg(
        long,
        value_name = "NAME",
        conflicts_with = "detach",
        help = "Work on this branch. Skips the branch question. A new name is made from origin's default branch"
    )]
    branch: Option<String>,
    #[arg(
        long,
        help = "Detach HEAD at origin's default branch. Skips the branch question"
    )]
    detach: bool,
    #[arg(long, help = "Do not start claude in the session")]
    no_claude: bool,
    #[arg(long, help = "Do not attach to the tmux session")]
    no_attach: bool,
}

pub fn run(action: HarnessCommands) -> Result<i32, String> {
    match action {
        HarnessCommands::New(args) => new_harness(args),
        HarnessCommands::List { porcelain, json } => list(porcelain, json),
        HarnessCommands::Path { name } => {
            let ctx = Ctx::discover(false)?;
            println!("{}", ctx.find(&name)?.worktree.display());
            Ok(0)
        }
        HarnessCommands::Cd { name: _ } => {
            eprintln!(
                "tsk harness cd needs the shell function: run tsk install zshrc, or use: cd \"$(tsk harness path <name>)\""
            );
            Ok(1)
        }
        HarnessCommands::Sleep { name } => sleep(&name),
        HarnessCommands::Wake {
            name,
            no_claude,
            no_attach,
        } => wake(&name, no_claude, no_attach),
        HarnessCommands::Delete { name, force } => delete(&name, force),
    }
}

// ---------------------------------------------------------------------------
// The clone
// ---------------------------------------------------------------------------

struct Ctx {
    main_worktree: PathBuf,
    repo_name: String,
    state_root: PathBuf,
    clone_id: Option<String>,
}

impl Ctx {
    /// The clone of the current directory. `create` makes the clone ID when it has none.
    fn discover(create: bool) -> Result<Ctx, String> {
        let cwd = std::env::current_dir()
            .map_err(|e| format!("error: could not read the current directory: {}", e))?;
        let repo = Repo::discover(&cwd)?;
        let main_worktree = repo
            .common_dir
            .parent()
            .filter(|_| repo.common_dir.file_name().is_some_and(|n| n == ".git"))
            .map(Path::to_path_buf)
            .ok_or_else(|| {
                format!(
                    "error: {} is not the .git directory of a clone with a work tree",
                    repo.common_dir.display()
                )
            })?;
        let repo_name = main_worktree
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .filter(|n| !n.is_empty())
            .ok_or_else(|| format!("error: {} has no directory name", main_worktree.display()))?;
        let state_root = crate::ledger::location::state_root_from_env()?;
        let clone_id = if create {
            Some(repo.ensure_clone_id()?)
        } else {
            repo.read_clone_id()?
        };
        Ok(Ctx {
            main_worktree,
            repo_name,
            state_root,
            clone_id,
        })
    }

    fn state_dir(&self) -> Option<PathBuf> {
        self.clone_id
            .as_deref()
            .map(|id| state::harness_dir(&self.state_root, id))
    }

    fn names_file(&self) -> Result<PathBuf, String> {
        let id = self.clone_id.as_deref().ok_or_else(no_clone_id)?;
        Ok(state::names_file(&self.state_root, id))
    }

    fn load(&self) -> Result<Vec<HarnessState>, String> {
        match self.state_dir() {
            Some(dir) => state::load_all(&dir),
            None => Ok(Vec::new()),
        }
    }

    fn save(&self, harness: &HarnessState) -> Result<(), String> {
        let dir = self.state_dir().ok_or_else(no_clone_id)?;
        state::save(&dir, harness)
    }

    /// The harness with this name, or the only one whose name starts with it.
    fn find(&self, prefix: &str) -> Result<HarnessState, String> {
        let all = self.load()?;
        let names: Vec<String> = all.iter().map(|h| h.name.clone()).collect();
        let found = names::match_prefix(&names, prefix);
        match found.as_slice() {
            [one] => Ok(all
                .into_iter()
                .find(|h| &&h.name == one)
                .expect("the name came from the list")),
            [] if names.is_empty() => Err(format!(
                "error: no harness is named or starts with '{}'. This clone has no harnesses",
                prefix
            )),
            [] => Err(format!(
                "error: no harness is named or starts with '{}'. Harnesses: {}",
                prefix,
                names.join(", ")
            )),
            many => Err(format!(
                "error: '{}' starts the names of more than one harness: {}",
                prefix,
                many.iter()
                    .map(|n| n.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
        }
    }

    /// The instance key the harness has in the ledger: `<clone-id>/<worktree name>`.
    fn instance_key(&self, harness: &HarnessState) -> Result<String, String> {
        let clone_id = self.clone_id.as_deref().ok_or_else(no_clone_id)?;
        let repo = Repo::discover(&harness.worktree)?;
        Ok(worktree_actor(clone_id, &repo.git_dir))
    }

    fn git(&self) -> Git {
        Git::new(&self.main_worktree)
    }
}

fn no_clone_id() -> String {
    "error: this clone has no clone id yet. Run tsk harness new, or tsk ledger fetch, first"
        .to_string()
}

// ---------------------------------------------------------------------------
// tmux
// ---------------------------------------------------------------------------

fn tmux(args: &[&str]) -> Result<Output, String> {
    let bin = crate::tmux::tmux_bin();
    Command::new(&bin)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("error: could not run {}: {}", bin, e))
}

fn tmux_checked(args: &[&str]) -> Result<(), String> {
    let output = tmux(args)?;
    if output.status.success() {
        return Ok(());
    }
    Err(format!(
        "error: tmux {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr).trim()
    ))
}

fn has_session(session: &str) -> bool {
    tmux(&["has-session", "-t", &format!("={}", session)])
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// The tmux session name of a harness: the mission code when it is known, else the harness
/// name. A session of that name that exists already gives way to the harness name.
pub fn session_name(mission: Option<&str>, name: &str, exists: impl Fn(&str) -> bool) -> String {
    match mission.filter(|m| !m.is_empty()) {
        Some(code) => {
            let wanted = crate::tmux::session_name_for(code);
            if exists(&wanted) {
                name.to_string()
            } else {
                wanted
            }
        }
        None => name.to_string(),
    }
}

/// Quote text for a POSIX shell: single quotes, and `'\''` for a quote inside.
pub fn shell_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

fn start_session(
    session: &str,
    harness: &HarnessState,
    start_claude: Option<&str>,
) -> Result<(), String> {
    let dir = harness.worktree.to_string_lossy();
    tmux_checked(&["new-session", "-d", "-s", session, "-c", &dir])?;
    tmux_checked(&[
        "set-option",
        "-t",
        &format!("={}:", session),
        TMUX_OPTION,
        &harness.name,
    ])?;
    if let Some(prompt) = start_claude {
        let command = if prompt.is_empty() {
            "claude".to_string()
        } else {
            format!("claude {}", shell_quote(prompt))
        };
        let target = format!("={}:", session);
        tmux_checked(&["send-keys", "-t", &target, "-l", &command])?;
        tmux_checked(&["send-keys", "-t", &target, "Enter"])?;
    }
    Ok(())
}

fn attach(session: &str) -> Result<(), String> {
    let target = format!("={}", session);
    let inside = std::env::var_os("TMUX").is_some_and(|v| !v.is_empty());
    let args: &[&str] = if inside {
        &["switch-client", "-t", &target]
    } else {
        &["attach", "-t", &target]
    };
    let bin = crate::tmux::tmux_bin();
    let status = Command::new(&bin)
        .args(args)
        .status()
        .map_err(|e| format!("error: could not run {}: {}", bin, e))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("error: tmux {} failed", args.join(" ")))
    }
}

// ---------------------------------------------------------------------------
// The first prompt
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RobotChoice {
    Existing(String),
    New,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MissionChoice {
    Existing(String),
    New,
}

/// The branch step of the first prompt: the branch that was chosen, or the detached text.
pub fn branch_step(branch: Option<&str>) -> String {
    match branch {
        Some(branch) => format!("This worktree is on branch {}. Follow this repo's rule: in a repo that works on main, rebase on origin's main and push with git push origin HEAD:main; otherwise push {} and open a pull request from it.", branch, branch),
        None => BRANCH_STEP.to_string(),
    }
}

fn register_step(robot: &str) -> String {
    format!("Run tsk actor register \"{}\" --kind robot.", robot)
}

/// The first prompt of `claude` in a new harness: a list of plain steps. The new session
/// finishes the set-up, and tsk makes no robot and no mission.
pub fn first_prompt(
    thread: Option<&str>,
    robot: Option<&RobotChoice>,
    mission: Option<&MissionChoice>,
    branch: Option<&str>,
) -> String {
    let mut steps: Vec<String> = Vec::new();
    match robot {
        Some(RobotChoice::Existing(name)) => steps.push(register_step(name)),
        Some(RobotChoice::New) => steps.push("Run /tsk:construct-robot.".to_string()),
        None => {}
    }
    match (thread, mission) {
        (Some(thread), _) => steps.push(format!("Then run /tsk:resume-thread {}.", thread)),
        (None, Some(MissionChoice::Existing(id))) => {
            steps.push(format!("Then run /tsk:start-thread {}.", id))
        }
        (None, Some(MissionChoice::New)) => steps.push(
            "Then ask me what the new mission is, create it with tsk mission create, and run /tsk:start-thread on it."
                .to_string(),
        ),
        (None, None) => {}
    }
    if steps.len() == 1 {
        steps[0] = steps[0].replacen("Then run", "Run", 1);
    }
    steps.push(branch_step(branch));
    steps.join(" ")
}

/// The first prompt of `tsk harness wake`: register the robot when it is known, then resume
/// the thread. Empty when the harness has no thread to resume.
pub fn wake_prompt(robot: Option<&str>, thread: Option<&str>) -> String {
    match (robot, thread) {
        (_, None) => String::new(),
        (Some(robot), Some(thread)) => format!(
            "{} Then run /tsk:resume-thread {}.",
            register_step(robot),
            thread
        ),
        (None, Some(thread)) => format!("Run /tsk:resume-thread {}.", thread),
    }
}

// ---------------------------------------------------------------------------
// Questions
// ---------------------------------------------------------------------------

/// `y` or `n`, in either case, with or without the rest of the word.
pub fn parse_yes_no(answer: &str) -> Option<bool> {
    match answer.trim().to_ascii_lowercase().as_str() {
        "y" | "yes" => Some(true),
        "n" | "no" => Some(false),
        _ => None,
    }
}

/// A number from 1 to `count`, as an index from 0. With `allow_new`, `n` is the new item:
/// `Some(None)`.
pub fn parse_pick(answer: &str, count: usize, allow_new: bool) -> Option<Option<usize>> {
    let answer = answer.trim();
    if allow_new && answer.eq_ignore_ascii_case("n") {
        return Some(None);
    }
    let number: usize = answer.parse().ok()?;
    (1..=count).contains(&number).then(|| Some(number - 1))
}

fn read_answer(prompt: &str) -> Result<String, String> {
    print!("{} ", prompt);
    std::io::stdout()
        .flush()
        .map_err(|e| format!("error: could not write to the terminal: {}", e))?;
    let mut line = String::new();
    let read = std::io::stdin()
        .lock()
        .read_line(&mut line)
        .map_err(|e| format!("error: could not read the answer: {}", e))?;
    if read == 0 {
        return Err("error: the input ended before the question was answered".to_string());
    }
    Ok(line.trim().to_string())
}

fn ask_yes_no(prompt: &str) -> Result<bool, String> {
    loop {
        if let Some(answer) = parse_yes_no(&read_answer(prompt)?) {
            return Ok(answer);
        }
        println!("Answer y or n.");
    }
}

fn ask_pick(prompt: &str, count: usize, allow_new: bool) -> Result<Option<usize>, String> {
    loop {
        if let Some(pick) = parse_pick(&read_answer(prompt)?, count, allow_new) {
            return Ok(pick);
        }
        println!(
            "Answer a number from 1 to {}{}.",
            count,
            if allow_new { ", or n" } else { "" }
        );
    }
}

/// A paused thread, as the first question lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct PausedThread {
    id: String,
    mission: Option<String>,
    robot: Option<String>,
}

/// The robot of a thread: the actor that wrote its latest entry, else the last robot bound.
fn thread_robot(
    store: &dyn LedgerStore,
    writers: &crate::thread::writers::Writers,
    row: &ops::ListRow,
) -> Result<Option<String>, String> {
    let entries = ops::entries_of(store, &row.id)?;
    let by_pause = entries
        .last()
        .and_then(|stored| stored.entry.written_by.as_deref())
        .map(|written_by| writers.resolve(written_by))
        .filter(|writer| writer.actor_uid.is_some())
        .map(|writer| writer.label);
    Ok(by_pause.or_else(|| row.actors.iter().rev().find_map(|actor| actor.name.clone())))
}

fn paused_threads(store: &dyn LedgerStore) -> Result<Vec<PausedThread>, String> {
    let rows = ops::list_rows(store)?;
    let writers = crate::thread::writers::Writers::load(store)?;
    let mut found = Vec::new();
    for row in rows.iter().filter(|row| row.state == "paused") {
        found.push(PausedThread {
            id: row.id.clone(),
            mission: row.mission_id.clone(),
            robot: thread_robot(store, &writers, row)?,
        });
    }
    Ok(found)
}

/// Numbered rows with each column padded to its widest cell, the number right-aligned:
/// `   1) M-TSK-ENV   Credit Where It's Due  e6b3c55c`. The last column is not padded.
fn numbered(rows: &[Vec<String>]) -> Vec<String> {
    let cols = rows.iter().map(Vec::len).max().unwrap_or(0);
    let widths: Vec<usize> = (0..cols)
        .map(|c| {
            rows.iter()
                .filter_map(|r| r.get(c))
                .map(|cell| cell.chars().count())
                .max()
                .unwrap_or(0)
        })
        .collect();
    let num_width = rows.len().to_string().len();
    rows.iter()
        .enumerate()
        .map(|(i, row)| {
            let cells: Vec<String> = row
                .iter()
                .enumerate()
                .map(|(c, cell)| {
                    if c + 1 == row.len() {
                        cell.clone()
                    } else {
                        let pad = widths[c] - cell.chars().count();
                        format!("{}{}", cell, " ".repeat(pad))
                    }
                })
                .collect();
            format!("  {:>w$}) {}", i + 1, cells.join("  "), w = num_width)
        })
        .collect()
}

fn render_paused(threads: &[PausedThread]) -> Vec<String> {
    let rows: Vec<Vec<String>> = threads
        .iter()
        .map(|t| {
            vec![
                t.mission.as_deref().unwrap_or("-").to_string(),
                t.robot.as_deref().unwrap_or("-").to_string(),
                t.id.clone(),
            ]
        })
        .collect();
    numbered(&rows)
}

fn render_robots(robots: &[binding::RobotStatus]) -> Vec<String> {
    let rows: Vec<Vec<String>> = robots
        .iter()
        .map(|r| vec![r.name.clone(), r.state.render()])
        .collect();
    numbered(&rows)
}

fn render_missions(missions: &[&crate::mission::model::Mission]) -> Vec<String> {
    let rows: Vec<Vec<String>> = missions
        .iter()
        .map(|m| vec![m.id.clone(), m.name.clone()])
        .collect();
    let mut lines = numbered(&rows);
    let num_width = rows.len().to_string().len();
    lines.push(format!("  {:>w$}) new mission", "n", w = num_width));
    lines
}

/// The ledger of this clone, fetched when it can be.
fn ledger(ctx: &Ctx) -> Result<Session, String> {
    let session = Session::discover_in(&ctx.main_worktree)?;
    if let Err(e) = session.refresh() {
        eprintln!(
            "warning: could not fetch the ledger, so the lists may be out of date: {}",
            e.trim_start_matches("error: ")
        );
    }
    Ok(session)
}

/// The ticket ID of a task, as the branch question suggests it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Ticket {
    task: String,
    id: String,
}

/// The ticket ID of a task, such as `TEAM1-546`, or `None` when the task has none.
// TODO(ext_tracking_id): read the `ext_tracking_id` field of the task after the merge.
fn ticket_of_task(_mission: &str, _task: &str) -> Option<String> {
    None
}

/// The ticket of the task of the latest pause entry of a thread.
fn thread_ticket(
    store: &dyn LedgerStore,
    thread: &str,
    mission: Option<&str>,
) -> Result<Option<Ticket>, String> {
    let Some(mission) = mission else {
        return Ok(None);
    };
    let entries = ops::entries_of(store, thread)?;
    let task = entries
        .last()
        .and_then(|stored| stored.entry.task_id.clone())
        .filter(|task| !task.is_empty());
    Ok(task.and_then(|task| ticket_of_task(mission, &task).map(|id| Ticket { task, id })))
}

/// What the person or the flags chose.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Selection {
    thread: Option<String>,
    /// The current task of the thread and its ticket ID, when the task has one.
    ticket: Option<Ticket>,
    robot: Option<RobotChoice>,
    mission: Option<MissionChoice>,
}

fn select(ctx: &Ctx, args: &NewArgs) -> Result<Selection, String> {
    let tty = std::io::stdin().is_terminal();
    let robot_flag = args
        .robot
        .clone()
        .map(RobotChoice::Existing)
        .or(args.new_robot.then_some(RobotChoice::New));
    let mission_flag = args
        .mission
        .clone()
        .map(MissionChoice::Existing)
        .or(args.new_mission.then_some(MissionChoice::New));

    if let Some(thread) = &args.thread {
        let session = ledger(ctx)?;
        let store = session.store.as_ref();
        let rows = ops::list_rows(store)?;
        let row = rows
            .iter()
            .find(|row| &row.id == thread)
            .ok_or_else(|| format!("error: no such thread: {}", thread))?;
        let writers = crate::thread::writers::Writers::load(store)?;
        let robot = match robot_flag {
            Some(flag) => Some(flag),
            None => thread_robot(store, &writers, row)?.map(RobotChoice::Existing),
        };
        return Ok(Selection {
            thread: Some(thread.clone()),
            ticket: thread_ticket(store, thread, row.mission_id.as_deref())?,
            robot,
            mission: row.mission_id.clone().map(MissionChoice::Existing),
        });
    }

    let mut session: Option<Session> = None;
    if robot_flag.is_none() && mission_flag.is_none() {
        if !tty {
            return Err("error: nothing is chosen and the input is not a terminal. Pass --thread <id>, or --robot <name> or --new-robot with --mission <id> or --new-mission".to_string());
        }
        let opened = ledger(ctx)?;
        if ask_yes_no("Pick up an existing paused thread? (y/n)")? {
            let threads = paused_threads(opened.store.as_ref())?;
            if threads.is_empty() {
                println!("There are no paused threads.");
            } else {
                println!("Paused threads: mission, robot, thread");
                for line in render_paused(&threads) {
                    println!("{}", line);
                }
                let pick = ask_pick("Which thread?", threads.len(), false)?
                    .expect("a number was asked for");
                let thread = &threads[pick];
                return Ok(Selection {
                    thread: Some(thread.id.clone()),
                    ticket: thread_ticket(
                        opened.store.as_ref(),
                        &thread.id,
                        thread.mission.as_deref(),
                    )?,
                    robot: thread.robot.clone().map(RobotChoice::Existing),
                    mission: thread.mission.clone().map(MissionChoice::Existing),
                });
            }
        }
        session = Some(opened);
    }

    let robot = match robot_flag {
        Some(flag) => flag,
        None => {
            if !tty {
                return Err("error: no robot is chosen and the input is not a terminal. Pass --robot <name> or --new-robot".to_string());
            }
            let opened = match session.take() {
                Some(opened) => opened,
                None => ledger(ctx)?,
            };
            let choice = ask_robot(opened.store.as_ref())?;
            session = Some(opened);
            choice
        }
    };
    let mission = match mission_flag {
        Some(flag) => flag,
        None => {
            if !tty {
                return Err("error: no mission is chosen and the input is not a terminal. Pass --mission <id> or --new-mission".to_string());
            }
            let opened = match session.take() {
                Some(opened) => opened,
                None => ledger(ctx)?,
            };
            ask_mission(opened.store.as_ref())?
        }
    };
    Ok(Selection {
        thread: None,
        ticket: None,
        robot: Some(robot),
        mission: Some(mission),
    })
}

fn ask_robot(store: &dyn LedgerStore) -> Result<RobotChoice, String> {
    if !ask_yes_no("Attach an existing robot? (y/n)")? {
        return Ok(RobotChoice::New);
    }
    let mut robots = binding::robot_states(store)?;
    if robots.is_empty() {
        println!("There are no robots yet. A new robot is made.");
        return Ok(RobotChoice::New);
    }
    // Free robots first, busy ones after, each group in the order given.
    robots.sort_by_key(|r| matches!(r.state, binding::RobotState::Busy(_)));
    println!("Robots: free ones first");
    for line in render_robots(&robots) {
        println!("{}", line);
    }
    let pick = ask_pick("Which robot?", robots.len(), false)?.expect("a number was asked for");
    Ok(RobotChoice::Existing(robots[pick].name.clone()))
}

fn ask_mission(store: &dyn LedgerStore) -> Result<MissionChoice, String> {
    let missions = store.missions_load()?;
    let open: Vec<&crate::mission::model::Mission> = missions
        .iter()
        .filter(|m| m.status != crate::mission::model::Status::Done)
        .collect();
    println!("Missions that are not done");
    for line in render_missions(&open) {
        println!("{}", line);
    }
    Ok(match ask_pick("Which mission?", open.len(), true)? {
        Some(i) => MissionChoice::Existing(open[i].id.clone()),
        None => MissionChoice::New,
    })
}

// ---------------------------------------------------------------------------
// The branch question
// ---------------------------------------------------------------------------

/// One answer to the branch question: a branch name, or `None` for a detached HEAD.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchSuggestion {
    pub branch: Option<String>,
    pub note: String,
}

/// The suggestions of the branch question, in order and only those that apply: the ticket of
/// the current task of the thread, the mission and the thread, the mission and the harness
/// name, and a detached HEAD.
pub fn branch_suggestions(
    ticket: Option<(&str, &str)>,
    mission: Option<&str>,
    thread: Option<&str>,
    harness: &str,
) -> Vec<BranchSuggestion> {
    let mut found = Vec::new();
    if let Some((task, id)) = ticket {
        found.push(BranchSuggestion {
            branch: Some(id.to_string()),
            note: format!("the ticket of task {}", task),
        });
    }
    match (mission, thread) {
        (Some(mission), Some(thread)) => found.push(BranchSuggestion {
            branch: Some(format!("{}/{}", mission.to_lowercase(), thread)),
            note: "mission and thread".to_string(),
        }),
        (Some(mission), None) => found.push(BranchSuggestion {
            branch: Some(format!("{}/{}", mission.to_lowercase(), harness)),
            note: "mission and harness name".to_string(),
        }),
        _ => {}
    }
    found.push(BranchSuggestion {
        branch: None,
        note: "the agent names the branch later".to_string(),
    });
    found
}

/// The lines of the branch question, aligned like the other lists.
pub fn render_branch_question(suggestions: &[BranchSuggestion]) -> Vec<String> {
    let rows: Vec<Vec<String>> = suggestions
        .iter()
        .map(|s| {
            vec![
                s.branch
                    .clone()
                    .unwrap_or_else(|| "detached HEAD".to_string()),
                s.note.clone(),
            ]
        })
        .collect();
    let mut lines = vec!["Branch name?".to_string()];
    lines.extend(numbered(&rows));
    lines.push("  or type a branch name".to_string());
    lines
}

/// An answer to the branch question: a suggestion by index, or a typed name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BranchAnswer {
    Pick(usize),
    Typed(String),
}

/// Enter is the first suggestion, a number in range picks that suggestion, and anything else
/// is a branch name.
pub fn parse_branch_answer(answer: &str, count: usize) -> BranchAnswer {
    let answer = answer.trim();
    if answer.is_empty() {
        return BranchAnswer::Pick(0);
    }
    match answer.parse::<usize>() {
        Ok(number) if (1..=count).contains(&number) => BranchAnswer::Pick(number - 1),
        _ => BranchAnswer::Typed(answer.to_string()),
    }
}

/// The worktree that has `branch` checked out, when one does.
fn branch_worktree(git: &Git, branch: &str) -> Result<Option<String>, String> {
    let listing = git.run(["worktree", "list", "--porcelain"])?;
    let wanted = format!("branch refs/heads/{}", branch);
    let mut path = String::new();
    for line in listing.lines() {
        if let Some(rest) = line.strip_prefix("worktree ") {
            path = rest.to_string();
        } else if line == wanted {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

/// A branch name that git accepts, and that no worktree has checked out.
fn check_branch(git: &Git, branch: &str) -> Result<(), String> {
    let valid = git
        .output(["check-ref-format", "--branch", branch])
        .map(|(_, output)| output.status.success())
        .unwrap_or(false);
    if !valid {
        return Err(format!("error: '{}' is not a branch name", branch));
    }
    if let Some(path) = branch_worktree(git, branch)? {
        return Err(format!(
            "error: the branch {} is checked out in {}. Pick another name, or use --detach",
            branch, path
        ));
    }
    Ok(())
}

/// The branch of the new harness, or `None` for a detached HEAD. `--detach` and `--branch`
/// answer the question. With no terminal and no flag, the first suggestion is used.
fn choose_branch(
    git: &Git,
    args: &NewArgs,
    selection: &Selection,
    harness: &str,
) -> Result<Option<String>, String> {
    if args.detach {
        return Ok(None);
    }
    if let Some(branch) = &args.branch {
        check_branch(git, branch)?;
        return Ok(Some(branch.clone()));
    }
    let mission = match &selection.mission {
        Some(MissionChoice::Existing(id)) => Some(id.as_str()),
        _ => None,
    };
    let ticket = selection
        .ticket
        .as_ref()
        .map(|t| (t.task.as_str(), t.id.as_str()));
    let suggestions = branch_suggestions(ticket, mission, selection.thread.as_deref(), harness);
    if !std::io::stdin().is_terminal() {
        let first = &suggestions[0];
        if let Some(branch) = &first.branch {
            check_branch(git, branch)?;
        }
        println!("There is no terminal and no --branch or --detach: using the first suggestion.");
        return Ok(first.branch.clone());
    }
    for line in render_branch_question(&suggestions) {
        println!("{}", line);
    }
    loop {
        let answer = read_answer("[1]:")?;
        let branch = match parse_branch_answer(&answer, suggestions.len()) {
            BranchAnswer::Pick(i) => suggestions[i].branch.clone(),
            BranchAnswer::Typed(name) => Some(name),
        };
        match &branch {
            Some(name) => match check_branch(git, name) {
                Ok(()) => return Ok(branch),
                Err(e) => println!("{}", e),
            },
            None => return Ok(None),
        }
    }
}

// ---------------------------------------------------------------------------
// new
// ---------------------------------------------------------------------------

/// The ref a new harness branch starts from: the default branch of origin, else `origin/main`,
/// else `HEAD`.
fn base_ref(git: &Git) -> String {
    for candidate in ["origin/HEAD", "origin/main"] {
        let found = git
            .output([
                "rev-parse",
                "--verify",
                "--quiet",
                &format!("{}^{{commit}}", candidate),
            ])
            .map(|(_, output)| output.status.success())
            .unwrap_or(false);
        if found {
            return candidate.to_string();
        }
    }
    "HEAD".to_string()
}

/// A file name that holds local env settings: `.env`, `.env.local` or `.env.*.local`.
pub fn is_env_file_name(name: &str) -> bool {
    name == ".env"
        || name == ".env.local"
        || name
            .strip_prefix(".env.")
            .and_then(|rest| rest.strip_suffix(".local"))
            .is_some_and(|middle| !middle.is_empty())
}

/// Copy the env files that git ignores from the main worktree into the new one. Returns the
/// names it copied.
fn copy_env_files(main: &Path, worktree: &Path) -> Vec<String> {
    let git = Git::new(main);
    let Ok(entries) = std::fs::read_dir(main) else {
        return Vec::new();
    };
    let mut candidates: Vec<String> = entries
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|t| t.is_file()))
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| is_env_file_name(name))
        .collect();
    candidates.sort();
    let mut copied = Vec::new();
    for name in candidates {
        let ignored = git
            .output(["check-ignore", "-q", "--", &name])
            .map(|(_, output)| output.status.success())
            .unwrap_or(false);
        if ignored && std::fs::copy(main.join(&name), worktree.join(&name)).is_ok() {
            copied.push(name);
        }
    }
    copied
}

fn new_harness(args: NewArgs) -> Result<i32, String> {
    let ctx = Ctx::discover(true)?;
    let (root, _) = crate::config::worktrees_root_from_env().ok_or_else(|| NO_ROOT.to_string())?;
    let names_file = ctx.names_file()?;
    let used = state::used_names(&names_file)?;
    let name = match &args.name {
        Some(name) => {
            if !names::valid_name(name) {
                return Err(format!(
                    "error: '{}' is not a harness name. Use lower case letters, digits and hyphens, at most {} characters, and start with a letter or a digit",
                    name,
                    names::NAME_MAX
                ));
            }
            if used.contains(name) {
                return Err(format!(
                    "error: the name '{}' is used. A harness of this clone has or had it, and a name is never used twice",
                    name
                ));
            }
            name.clone()
        }
        None => names::pick_name(&names::words(), &used, names::seed()),
    };
    let worktree = root.join(&ctx.repo_name).join(&name);
    if worktree.exists() {
        return Err(format!(
            "error: {} exists already. Remove it, or set another worktrees root",
            worktree.display()
        ));
    }

    let selection = select(&ctx, &args)?;
    let mission_code = match &selection.mission {
        Some(MissionChoice::Existing(id)) => Some(id.clone()),
        _ => None,
    };
    let robot_name = match &selection.robot {
        Some(RobotChoice::Existing(robot)) => Some(robot.clone()),
        _ => None,
    };

    let git = ctx.git();
    let branch = choose_branch(&git, &args, &selection, &name)?;
    let base = base_ref(&git);
    if let Some(parent) = worktree.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("error: could not create {}: {}", parent.display(), e))?;
    }
    let path = worktree.to_string_lossy().into_owned();
    match &branch {
        None => git.run(["worktree", "add", "--detach", &path, &base])?,
        Some(branch) if branch_exists(&git, branch) => {
            git.run(["worktree", "add", &path, branch])?
        }
        Some(branch) => git.run(["worktree", "add", "-b", branch, &path, &base])?,
    };
    let copied = copy_env_files(&ctx.main_worktree, &worktree);

    let session = session_name(mission_code.as_deref(), &name, has_session);
    let harness = HarnessState {
        name: name.clone(),
        worktree: worktree.clone(),
        branch: branch.clone().unwrap_or_default(),
        main_worktree: ctx.main_worktree.clone(),
        tmux_session: session.clone(),
        thread_id: selection.thread.clone(),
        robot: robot_name,
        mission: mission_code,
        created_at: crate::thread::clock::utc_now(),
    };
    state::record_name(&names_file, &name)?;
    ctx.save(&harness)?;

    let prompt = first_prompt(
        selection.thread.as_deref(),
        selection.robot.as_ref(),
        selection.mission.as_ref(),
        branch.as_deref(),
    );
    start_session(
        &session,
        &harness,
        if args.no_claude { None } else { Some(&prompt) },
    )
    .map_err(|e| {
        format!(
            "{}\nThe harness exists. Run tsk harness wake {} to start its tmux session",
            e, name
        )
    })?;

    println!("harness: {}", name);
    println!("path: {}", worktree.display());
    println!("branch: {}", branch.as_deref().unwrap_or("detached HEAD"));
    println!("tmux session: {}", session);
    if !copied.is_empty() {
        println!("copied: {}", copied.join(", "));
    }
    if !args.no_attach {
        attach(&session)?;
    }
    Ok(0)
}

// ---------------------------------------------------------------------------
// list
// ---------------------------------------------------------------------------

/// A path with the home directory as `~`.
pub fn tilde(path: &Path, home: Option<&Path>) -> String {
    match home {
        Some(home) if !home.as_os_str().is_empty() => match path.strip_prefix(home) {
            Ok(rest) if rest.as_os_str().is_empty() => "~".to_string(),
            Ok(rest) => format!("~/{}", rest.display()),
            Err(_) => path.display().to_string(),
        },
        _ => path.display().to_string(),
    }
}

fn dash(value: &Option<String>) -> &str {
    value.as_deref().unwrap_or("-")
}

/// The table of `tsk harness list`: a header, then a row for each harness, in columns
/// aligned with spaces. Each row is `[name, robot, mission, thread, state, directory]`.
pub fn render_table(rows: &[[String; 6]]) -> Vec<String> {
    let header = ["NAME", "ROBOT", "MISSION", "THREAD", "STATE", "DIRECTORY"].map(String::from);
    let mut widths = [0usize; 6];
    for row in std::iter::once(&header).chain(rows.iter()) {
        for (width, cell) in widths.iter_mut().zip(row.iter()) {
            *width = (*width).max(cell.chars().count());
        }
    }
    std::iter::once(&header)
        .chain(rows.iter())
        .map(|row| {
            let mut line = String::new();
            for (i, cell) in row.iter().enumerate() {
                if i + 1 == row.len() {
                    line.push_str(cell);
                } else {
                    line.push_str(&format!("{:<width$}  ", cell, width = widths[i]));
                }
            }
            line
        })
        .collect()
}

fn list(porcelain: bool, json: bool) -> Result<i32, String> {
    let ctx = Ctx::discover(false)?;
    let harnesses = ctx.load()?;
    let awake: Vec<bool> = harnesses
        .iter()
        .map(|h| has_session(&h.tmux_session))
        .collect();
    let state_name = |awake: bool| if awake { "awake" } else { "asleep" };
    if json {
        let items: Vec<serde_json::Value> = harnesses
            .iter()
            .zip(&awake)
            .map(|(h, awake)| {
                serde_json::json!({
                    "name": h.name,
                    "robot": h.robot,
                    "mission": h.mission,
                    "thread": h.thread_id,
                    "state": state_name(*awake),
                    "directory": h.worktree,
                    "branch": h.branch,
                    "tmux_session": h.tmux_session,
                    "created_at": h.created_at,
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&items)
                .map_err(|e| format!("error: could not write the list: {}", e))?
        );
        return Ok(0);
    }
    if porcelain {
        for (h, awake) in harnesses.iter().zip(&awake) {
            println!(
                "{}\t{}\t{}\t{}\t{}\t{}",
                h.name,
                dash(&h.robot),
                dash(&h.mission),
                dash(&h.thread_id),
                state_name(*awake),
                h.worktree.display()
            );
        }
        return Ok(0);
    }
    if harnesses.is_empty() {
        println!("No harnesses. Run tsk harness new to make one.");
        return Ok(0);
    }
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let rows: Vec<[String; 6]> = harnesses
        .iter()
        .zip(&awake)
        .map(|(h, awake)| {
            [
                h.name.clone(),
                dash(&h.robot).to_string(),
                dash(&h.mission).to_string(),
                dash(&h.thread_id).to_string(),
                state_name(*awake).to_string(),
                tilde(&h.worktree, home.as_deref()),
            ]
        })
        .collect();
    for line in render_table(&rows) {
        println!("{}", line);
    }
    Ok(0)
}

// ---------------------------------------------------------------------------
// sleep, wake, delete
// ---------------------------------------------------------------------------

/// Pause the thread this harness holds, if it holds one. Returns the thread ID. The pause
/// runs as the harness instance: this binary, started in the worktree, so the binding that
/// the harness made is the one it pauses.
fn pause_if_held(
    ctx: &Ctx,
    harness: &mut HarnessState,
    verb: &str,
) -> Result<Option<String>, String> {
    if !harness.worktree.is_dir() {
        return Ok(None);
    }
    let instance = ctx.instance_key(harness)?;
    let held = (|| -> Result<_, String> {
        let session = Session::discover_in(&ctx.main_worktree)?;
        let rows = ops::list_rows(session.store.as_ref())?;
        Ok(rows
            .into_iter()
            .find(|row| {
                row.holder
                    .as_ref()
                    .is_some_and(|holder| holder.instance == instance)
            })
            .map(|row| (row.id, row.mission_id, row.holder.and_then(|h| h.robot))))
    })();
    let held = match held {
        Ok(held) => held,
        Err(e) => {
            eprintln!(
                "warning: could not read the ledger, so no thread is paused: {}",
                e.trim_start_matches("error: ")
            );
            None
        }
    };
    let Some((thread, mission, robot)) = held else {
        return Ok(None);
    };
    let exe = std::env::current_exe()
        .map_err(|e| format!("error: could not find the tsk program: {}", e))?;
    let note = format!(
        "Harness {} {} at {}; the previous pause entry holds the hand-over.",
        harness.name,
        verb,
        crate::thread::clock::utc_now()
    );
    let output = Command::new(exe)
        .args(["thread", "pause", "--bound", "", &note])
        .current_dir(&harness.worktree)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("error: could not run tsk thread pause: {}", e))?;
    if !output.status.success() {
        return Err(format!(
            "error: could not pause thread {} of harness {}:\n{}",
            thread,
            harness.name,
            String::from_utf8_lossy(&output.stderr).trim_end()
        ));
    }
    harness.thread_id = Some(thread.clone());
    if harness.mission.is_none() {
        harness.mission = mission;
    }
    if harness.robot.is_none() {
        harness.robot = robot;
    }
    ctx.save(harness)?;
    println!("paused thread {}", thread);
    Ok(Some(thread))
}

fn kill_session(harness: &HarnessState) -> Result<bool, String> {
    if !has_session(&harness.tmux_session) {
        return Ok(false);
    }
    tmux_checked(&["kill-session", "-t", &format!("={}", harness.tmux_session)])?;
    Ok(true)
}

fn sleep(name: &str) -> Result<i32, String> {
    let ctx = Ctx::discover(false)?;
    let mut harness = ctx.find(name)?;
    pause_if_held(&ctx, &mut harness, "put to sleep")?;
    let killed = kill_session(&harness)?;
    println!(
        "harness {} sleeps{}",
        harness.name,
        if killed {
            ": its tmux session is stopped"
        } else {
            ": it had no tmux session"
        }
    );
    Ok(0)
}

fn wake(name: &str, no_claude: bool, no_attach: bool) -> Result<i32, String> {
    let ctx = Ctx::discover(false)?;
    let mut harness = ctx.find(name)?;
    if !harness.worktree.is_dir() {
        return Err(format!(
            "error: the worktree {} of harness {} is gone. Run tsk harness delete {} --force to drop the harness",
            harness.worktree.display(),
            harness.name,
            harness.name
        ));
    }
    if has_session(&harness.tmux_session) {
        println!("harness {} is awake already", harness.name);
    } else {
        let session = session_name(harness.mission.as_deref(), &harness.name, has_session);
        harness.tmux_session = session.clone();
        ctx.save(&harness)?;
        let prompt = wake_prompt(harness.robot.as_deref(), harness.thread_id.as_deref());
        start_session(
            &session,
            &harness,
            if no_claude { None } else { Some(&prompt) },
        )?;
        println!("harness {} wakes", harness.name);
        println!("tmux session: {}", session);
    }
    if !no_attach {
        attach(&harness.tmux_session)?;
    }
    Ok(0)
}

fn git_count(git: &Git, args: &[&str]) -> Result<u64, String> {
    Ok(git
        .run_line(args.iter().copied())?
        .trim()
        .parse()
        .unwrap_or(0))
}

fn branch_exists(git: &Git, branch: &str) -> bool {
    git.output([
        "rev-parse",
        "--verify",
        "--quiet",
        &format!("refs/heads/{}", branch),
    ])
    .map(|(_, output)| output.status.success())
    .unwrap_or(false)
}

/// Why a harness cannot be deleted without `--force`, or `None`.
fn delete_blocker(harness: &HarnessState) -> Result<Option<String>, String> {
    let git = Git::new(&harness.worktree);
    if !git.run(["status", "--porcelain"])?.trim().is_empty() {
        return Ok(Some(format!(
            "the worktree {} has uncommitted changes",
            harness.worktree.display()
        )));
    }
    let head = git_count(&git, &["rev-list", "--count", "HEAD", "--not", "--remotes"])?;
    if head > 0 {
        return Ok(Some(format!(
            "the worktree {} has {} commit(s) on no remote branch",
            harness.worktree.display(),
            head
        )));
    }
    if !harness.branch.is_empty() && branch_exists(&git, &harness.branch) {
        let own = git_count(
            &git,
            &["rev-list", "--count", &harness.branch, "--not", "--remotes"],
        )?;
        if own > 0 {
            return Ok(Some(format!(
                "the branch {} has {} commit(s) on no remote branch",
                harness.branch, own
            )));
        }
    }
    Ok(None)
}

fn delete(name: &str, force: bool) -> Result<i32, String> {
    let ctx = Ctx::discover(false)?;
    let mut harness = ctx.find(name)?;
    let cwd = std::env::current_dir().unwrap_or_default();
    if cwd.starts_with(&harness.worktree) {
        return Err(format!(
            "error: this shell is inside {}. Change to another directory first",
            harness.worktree.display()
        ));
    }
    let present = harness.worktree.is_dir();
    if present && !force {
        if let Some(why) = delete_blocker(&harness)? {
            return Err(format!(
                "error: harness {} is not deleted: {}. Commit and push the work, or run tsk harness delete {} --force",
                harness.name, why, harness.name
            ));
        }
    }
    match pause_if_held(&ctx, &mut harness, "deleted") {
        Ok(_) => {}
        Err(e) if force => eprintln!("warning: {}", e.trim_start_matches("error: ")),
        Err(e) => return Err(e),
    }
    kill_session(&harness)?;
    let git = ctx.git();
    if present {
        let path = harness.worktree.to_string_lossy().into_owned();
        let mut remove = vec!["worktree", "remove"];
        if force {
            remove.push("--force");
        }
        remove.push(&path);
        git.run(remove)?;
    } else {
        git.run(["worktree", "prune"])?;
    }
    let branch_note = if harness.branch.is_empty() {
        String::new()
    } else if branch_exists(&git, &harness.branch) {
        let unpushed = git_count(
            &git,
            &["rev-list", "--count", &harness.branch, "--not", "--remotes"],
        )?;
        if unpushed == 0 {
            git.run(["branch", "-D", &harness.branch])?;
            format!("branch {} deleted", harness.branch)
        } else if git.run(["branch", "-d", &harness.branch]).is_ok() {
            format!("branch {} deleted: it is merged", harness.branch)
        } else {
            format!(
                "branch {} kept: it has {} commit(s) on no remote branch",
                harness.branch, unpushed
            )
        }
    } else {
        format!("branch {} was already gone", harness.branch)
    };
    if let Some(dir) = ctx.state_dir() {
        state::remove(&dir, &harness.name)?;
    }
    println!("harness {} deleted", harness.name);
    if !branch_note.is_empty() {
        println!("{}", branch_note);
    }
    println!("the name {} stays retired", harness.name);
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_session_is_named_after_the_mission_code_or_else_the_harness() {
        assert_eq!(
            session_name(Some("M-TSK-ENV"), "otter", |_| false),
            "M-TSK-ENV"
        );
        assert_eq!(session_name(None, "otter", |_| false), "otter");
        assert_eq!(session_name(Some(""), "otter", |_| false), "otter");
        assert_eq!(
            session_name(Some("M-TSK-ENV"), "otter", |s| s == "M-TSK-ENV"),
            "otter"
        );
        assert_eq!(session_name(Some("M.X:1"), "otter", |_| false), "M_X_1");
    }

    #[test]
    fn shell_quoting_keeps_a_quote_inside() {
        assert_eq!(shell_quote("a b"), "'a b'");
        assert_eq!(shell_quote("it's"), "'it'\\''s'");
    }

    #[test]
    fn the_first_prompt_follows_the_choices() {
        let robot = RobotChoice::Existing("Marvin".to_string());
        assert_eq!(
            first_prompt(
                Some("t1"),
                Some(&robot),
                Some(&MissionChoice::Existing("M-A".into())),
                None
            ),
            format!("Run tsk actor register \"Marvin\" --kind robot. Then run /tsk:resume-thread t1. {}", BRANCH_STEP)
        );
        assert_eq!(
            first_prompt(
                None,
                Some(&robot),
                Some(&MissionChoice::Existing("M-A".into())),
                Some("m-a/otter")
            ),
            format!("Run tsk actor register \"Marvin\" --kind robot. Then run /tsk:start-thread M-A. {}", branch_step(Some("m-a/otter")))
        );
        assert!(
            first_prompt(None, Some(&robot), Some(&MissionChoice::New), None)
                .contains("ask me what the new mission is")
        );
        assert_eq!(
            first_prompt(
                None,
                Some(&RobotChoice::New),
                Some(&MissionChoice::Existing("M-A".into())),
                None
            ),
            format!(
                "Run /tsk:construct-robot. Then run /tsk:start-thread M-A. {}",
                BRANCH_STEP
            )
        );
        assert_eq!(
            first_prompt(Some("t1"), None, None, None),
            format!("Run /tsk:resume-thread t1. {}", BRANCH_STEP)
        );
        assert_eq!(
            branch_step(Some("TEAM1-546")),
            "This worktree is on branch TEAM1-546. Follow this repo's rule: in a repo that works on main, rebase on origin's main and push with git push origin HEAD:main; otherwise push TEAM1-546 and open a pull request from it."
        );
    }

    #[test]
    fn the_wake_prompt_registers_a_known_robot_and_resumes_the_thread() {
        assert_eq!(
            wake_prompt(Some("Marvin"), Some("t1")),
            "Run tsk actor register \"Marvin\" --kind robot. Then run /tsk:resume-thread t1."
        );
        assert_eq!(wake_prompt(None, Some("t1")), "Run /tsk:resume-thread t1.");
        assert_eq!(wake_prompt(Some("Marvin"), None), "");
    }

    #[test]
    fn answers_parse() {
        assert_eq!(parse_yes_no(" Y "), Some(true));
        assert_eq!(parse_yes_no("no"), Some(false));
        assert_eq!(parse_yes_no("maybe"), None);
        assert_eq!(parse_pick("2", 3, false), Some(Some(1)));
        assert_eq!(parse_pick("4", 3, false), None);
        assert_eq!(parse_pick("0", 3, false), None);
        assert_eq!(parse_pick("n", 3, false), None);
        assert_eq!(parse_pick("n", 3, true), Some(None));
        assert_eq!(parse_pick("N", 0, true), Some(None));
    }

    #[test]
    fn env_file_names() {
        for name in [".env", ".env.local", ".env.test.local"] {
            assert!(is_env_file_name(name), "{}", name);
        }
        for name in [
            ".envrc",
            ".env.example",
            ".env..local",
            "env",
            ".env.local.bak",
        ] {
            assert!(!is_env_file_name(name), "{}", name);
        }
    }

    #[test]
    fn a_path_shows_the_home_directory_as_a_tilde() {
        let home = Path::new("/home/u");
        assert_eq!(
            tilde(Path::new("/home/u/trees/r/fox"), Some(home)),
            "~/trees/r/fox"
        );
        assert_eq!(tilde(Path::new("/home/u"), Some(home)), "~");
        assert_eq!(tilde(Path::new("/srv/r"), Some(home)), "/srv/r");
        assert_eq!(
            tilde(Path::new("/home/user2/r"), Some(home)),
            "/home/user2/r"
        );
        assert_eq!(tilde(Path::new("/x"), None), "/x");
    }

    #[test]
    fn the_table_has_a_header_and_aligned_columns() {
        let rows = [[
            "otter".to_string(),
            "Marvin".to_string(),
            "M-A".to_string(),
            "t1".to_string(),
            "awake".to_string(),
            "~/w/r/otter".to_string(),
        ]];
        let lines = render_table(&rows);
        assert_eq!(lines[0], "NAME   ROBOT   MISSION  THREAD  STATE  DIRECTORY");
        assert_eq!(
            lines[1],
            "otter  Marvin  M-A      t1      awake  ~/w/r/otter"
        );
    }

    #[test]
    fn the_lists_of_the_questions_are_numbered() {
        let threads = vec![PausedThread {
            id: "ab12".into(),
            mission: Some("M-A".into()),
            robot: None,
        }];
        assert_eq!(render_paused(&threads), vec!["  1) M-A  -  ab12"]);
    }

    #[test]
    fn numbered_rows_align_every_column_and_the_number() {
        let rows: Vec<Vec<String>> = (1..=10)
            .map(|i| {
                vec![
                    if i == 1 {
                        "M-TSK-PERF".to_string()
                    } else {
                        "M-X".to_string()
                    },
                    if i == 2 {
                        "-".to_string()
                    } else {
                        "Credit Where It's Due".to_string()
                    },
                    format!("t{}", i),
                ]
            })
            .collect();
        let lines = numbered(&rows);
        assert_eq!(lines[0], "   1) M-TSK-PERF  Credit Where It's Due  t1");
        assert_eq!(lines[1], "   2) M-X         -                      t2");
        assert_eq!(lines[9], "  10) M-X         Credit Where It's Due  t10");
    }

    #[test]
    fn the_branch_suggestions_come_in_order_and_only_those_that_apply() {
        let all = branch_suggestions(
            Some(("T-25", "TEAM1-546")),
            Some("M-TSK-PERF"),
            Some("25366ae1"),
            "kudu",
        );
        let branches: Vec<Option<&str>> = all.iter().map(|s| s.branch.as_deref()).collect();
        assert_eq!(
            branches,
            vec![Some("TEAM1-546"), Some("m-tsk-perf/25366ae1"), None]
        );
        let by_harness = branch_suggestions(None, Some("M-TSK-PERF"), None, "kudu");
        let branches: Vec<Option<&str>> = by_harness.iter().map(|s| s.branch.as_deref()).collect();
        assert_eq!(branches, vec![Some("m-tsk-perf/kudu"), None]);
        let none = branch_suggestions(None, None, None, "kudu");
        assert_eq!(none.len(), 1);
        assert_eq!(none[0].branch, None);
    }

    #[test]
    fn the_branch_question_is_aligned_like_the_other_lists() {
        let suggestions = branch_suggestions(
            Some(("T-25", "TEAM1-546")),
            Some("M-TSK-PERF"),
            Some("25366ae1"),
            "kudu",
        );
        assert_eq!(
            render_branch_question(&suggestions),
            vec![
                "Branch name?",
                "  1) TEAM1-546            the ticket of task T-25",
                "  2) m-tsk-perf/25366ae1  mission and thread",
                "  3) detached HEAD        the agent names the branch later",
                "  or type a branch name",
            ]
        );
    }

    #[test]
    fn a_branch_answer_is_enter_a_number_or_a_name() {
        assert_eq!(parse_branch_answer("", 3), BranchAnswer::Pick(0));
        assert_eq!(parse_branch_answer(" 2 ", 3), BranchAnswer::Pick(1));
        assert_eq!(
            parse_branch_answer("4", 3),
            BranchAnswer::Typed("4".to_string())
        );
        assert_eq!(
            parse_branch_answer("feature/x", 3),
            BranchAnswer::Typed("feature/x".to_string())
        );
    }
}
