//! Task, decision and question records of a mission: the rules, the output and the
//! `tsk task`, `tsk decision` and `tsk question` commands. Like the mission rules, they
//! work on plain [`Mission`] values, so both ledger backends share them. A task, a
//! decision and a question belong to one mission, and the mission record carries them.

use clap::Subcommand;
use serde::Serialize;

use crate::ledger::store::{self, LedgerStore};
use crate::thread::clock;

use super::change;
use super::model::{self, Decision, Mission, Question, QuestionStatus, Status, Task};
use super::table::print_lines;

pub const TASK_FIELDS: &[&str] = &["status", "description"];

// --- Labels ---

/// `T-01`: two digits, wider past 99.
pub fn task_label(seq: i64) -> String {
    format!("T-{:02}", seq)
}

/// `Q-01`: two digits, wider past 99.
pub fn question_label(seq: i64) -> String {
    format!("Q-{:02}", seq)
}

fn parse_label(prefix: &str, kind: &str, label: &str) -> Result<i64, String> {
    let digits = label
        .strip_prefix(prefix)
        .and_then(|rest| rest.strip_prefix('-'))
        .filter(|rest| !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit()));
    match digits
        .and_then(|d| d.parse::<i64>().ok())
        .filter(|n| *n > 0)
    {
        Some(n) => Ok(n),
        None => Err(format!(
            "error: '{}' is not a {} ID; a {} ID looks like {}",
            label,
            kind,
            kind,
            format_args!("{}-01", prefix)
        )),
    }
}

fn parse_task(label: &str) -> Result<i64, String> {
    parse_label("T", "task", label)
}

fn parse_question(label: &str) -> Result<i64, String> {
    parse_label("Q", "question", label)
}

// --- Validation ---

/// A one-line, non-empty text with no tab or line break, which the list output needs.
fn check_line(what: &str, text: &str) -> Result<(), String> {
    if text.trim().is_empty() {
        return Err(format!("error: {} cannot be empty", what));
    }
    if text.chars().any(char::is_control) {
        return Err(format!(
            "error: {} is one line, with no tabs or line breaks",
            what
        ));
    }
    Ok(())
}

/// A real calendar date written `YYYY-MM-DD`: four digits, two, two, and a day that the
/// month has in that year. `2026-02-30` is refused.
pub fn check_date(date: &str) -> Result<(), String> {
    let bad = || {
        format!(
            "error: '{}' is not a date; a date is written YYYY-MM-DD, for example 2026-03-14",
            date
        )
    };
    let b = date.as_bytes();
    let shaped = b.len() == 10
        && b.iter().enumerate().all(|(i, c)| match i {
            4 | 7 => *c == b'-',
            _ => c.is_ascii_digit(),
        });
    if !shaped {
        return Err(bad());
    }
    let year: u32 = date[0..4].parse().map_err(|_| bad())?;
    let month: u32 = date[5..7].parse().map_err(|_| bad())?;
    let day: u32 = date[8..10].parse().map_err(|_| bad())?;
    let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return Err(bad()),
    };
    if year == 0 || day == 0 || day > days {
        return Err(bad());
    }
    Ok(())
}

fn check_who(who: &str) -> Result<(), String> {
    check_line("the name given to --by", who)?;
    if who.chars().count() > 255 {
        return Err("error: the name given to --by is at most 255 characters".to_string());
    }
    Ok(())
}

fn task_ref(mission: &Mission, seq: i64) -> Result<&Task, String> {
    mission
        .tasks
        .iter()
        .find(|t| t.seq == seq)
        .ok_or_else(|| format!("error: {} has no task {}", mission.id, task_label(seq)))
}

fn task_mut(mission: &mut Mission, seq: i64) -> Result<&mut Task, String> {
    let id = mission.id.clone();
    mission
        .tasks
        .iter_mut()
        .find(|t| t.seq == seq)
        .ok_or_else(|| format!("error: {} has no task {}", id, task_label(seq)))
}

/// The label of a task of `mission` named by `label`, which may leave out the leading
/// zero: `T-1` is `T-01`. Fails when the label is not a task ID or the mission has no
/// such task.
pub fn resolve_task(mission: &Mission, label: &str) -> Result<String, String> {
    let seq = parse_task(label)?;
    task_ref(mission, seq).map(|task| task_label(task.seq))
}

// --- Operations. Each returns the changed mission to save. ---

pub fn task_add(missions: &[Mission], id: &str, description: &str) -> Result<Mission, String> {
    let mut mission = model::find(missions, id)?.clone();
    check_line("a task description", description)?;
    let seq = mission
        .next_task
        .max(mission.tasks.iter().map(|t| t.seq + 1).max().unwrap_or(1));
    mission.tasks.push(Task {
        seq,
        description: description.to_string(),
        status: Status::Todo,
        blocked_by: Vec::new(),
    });
    mission.next_task = seq + 1;
    Ok(mission)
}

pub fn task_set(
    missions: &[Mission],
    id: &str,
    task: &str,
    field: &str,
    value: &str,
) -> Result<Mission, String> {
    let mut mission = model::find(missions, id)?.clone();
    let seq = parse_task(task)?;
    match field {
        "status" => {
            let status = Status::parse(value)?;
            task_mut(&mut mission, seq)?.status = status;
        }
        "description" => {
            check_line("a task description", value)?;
            task_mut(&mut mission, seq)?.description = value.to_string();
        }
        other => {
            task_mut(&mut mission, seq)?;
            return Err(format!(
                "error: '{}' is not a task field; the fields are {}",
                other,
                TASK_FIELDS.join(", ")
            ));
        }
    }
    Ok(mission)
}

pub fn task_block(missions: &[Mission], id: &str, task: &str, by: &str) -> Result<Mission, String> {
    let mut mission = model::find(missions, id)?.clone();
    let (seq, by_seq) = (parse_task(task)?, parse_task(by)?);
    task_ref(&mission, seq)?;
    task_ref(&mission, by_seq)?;
    if seq == by_seq {
        return Err(format!("error: {} cannot block itself", task_label(seq)));
    }
    if task_ref(&mission, seq)?.blocked_by.contains(&by_seq) {
        return Err(format!(
            "error: {} is already blocked by {}",
            task_label(seq),
            task_label(by_seq)
        ));
    }
    // Adding seq -> by closes a cycle when seq is already reachable from by.
    if let Some(chain) = task_path(&mission, by_seq, seq) {
        let mut shown = vec![task_label(seq)];
        shown.extend(chain.into_iter().map(task_label));
        return Err(format!(
            "error: {} cannot be blocked by {}: that makes a cycle ({})",
            task_label(seq),
            task_label(by_seq),
            shown.join(" -> ")
        ));
    }
    let task = task_mut(&mut mission, seq)?;
    task.blocked_by.push(by_seq);
    task.blocked_by.sort_unstable();
    Ok(mission)
}

/// The chain of blockers from `from` down to `to`, both included, or `None`.
fn task_path(mission: &Mission, from: i64, to: i64) -> Option<Vec<i64>> {
    fn walk(
        mission: &Mission,
        at: i64,
        to: i64,
        seen: &mut Vec<i64>,
        trail: &mut Vec<i64>,
    ) -> bool {
        trail.push(at);
        if at == to {
            return true;
        }
        if !seen.contains(&at) {
            seen.push(at);
            if let Some(task) = mission.tasks.iter().find(|t| t.seq == at) {
                for next in &task.blocked_by {
                    if walk(mission, *next, to, seen, trail) {
                        return true;
                    }
                }
            }
        }
        trail.pop();
        false
    }
    let mut trail = Vec::new();
    walk(mission, from, to, &mut Vec::new(), &mut trail).then_some(trail)
}

pub fn task_unblock(
    missions: &[Mission],
    id: &str,
    task: &str,
    by: &str,
) -> Result<Mission, String> {
    let mut mission = model::find(missions, id)?.clone();
    let (seq, by_seq) = (parse_task(task)?, parse_task(by)?);
    if !task_ref(&mission, seq)?.blocked_by.contains(&by_seq) {
        return Err(format!(
            "error: {} is not blocked by {}",
            task_label(seq),
            task_label(by_seq)
        ));
    }
    task_mut(&mut mission, seq)?
        .blocked_by
        .retain(|b| *b != by_seq);
    Ok(mission)
}

/// An ADR path with the syntax rules applied and no look at the disk: relative, no `..`,
/// no control character, at most 1024 characters. A leading `./` and doubled slashes
/// go, so the stored path is one spelling. This is what `decision list --adr` compares.
pub fn normalize_adr(path: &str) -> Result<String, String> {
    let bad = |why: &str| {
        format!(
            "error: the ADR path '{}' {}; give a path relative to the root of the repository, for example docs/adr/0014-dolt-is-the-ledger-store-behind-a-storage-seam.md",
            path, why
        )
    };
    if path.trim().is_empty() {
        return Err("error: the ADR path cannot be empty".to_string());
    }
    if path.chars().any(char::is_control) {
        return Err(bad("holds a tab or a line break"));
    }
    let mut parts = Vec::new();
    for component in std::path::Path::new(path).components() {
        match component {
            std::path::Component::Normal(name) => parts.push(name.to_string_lossy().into_owned()),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => return Err(bad("contains '..'")),
            _ => return Err(bad("is absolute")),
        }
    }
    if parts.is_empty() {
        return Err(bad("names no file"));
    }
    let normal = parts.join("/");
    if normal.chars().count() > 1024 {
        return Err("error: an ADR path is at most 1024 characters".to_string());
    }
    Ok(normal)
}

/// The ADR path to store: [`normalize_adr`], and the file must exist in the working tree
/// whose top level is `root`.
pub fn check_adr_path(root: &std::path::Path, path: &str) -> Result<String, String> {
    let normal = normalize_adr(path)?;
    if !root.join(&normal).is_file() {
        return Err(format!(
            "error: no such file in the repository: {}\n       --adr takes the path of an ADR relative to the root of the repository ({})",
            normal,
            root.display()
        ));
    }
    Ok(normal)
}

pub fn decision_add(
    missions: &[Mission],
    id: &str,
    text: &str,
    by: &str,
    task: Option<&str>,
    adr: Option<&str>,
    today: &str,
) -> Result<Mission, String> {
    let mut mission = model::find(missions, id)?.clone();
    check_line("a decision", text)?;
    check_who(by)?;
    check_date(today)?;
    let task = match task {
        Some(label) => {
            let seq = parse_task(label)?;
            task_ref(&mission, seq)?;
            Some(seq)
        }
        None => None,
    };
    let seq = mission
        .decisions
        .iter()
        .map(|d| d.seq + 1)
        .max()
        .unwrap_or(1);
    mission.decisions.push(Decision {
        seq,
        text: text.to_string(),
        decided_by: by.to_string(),
        decided_on: today.to_string(),
        task,
        adr: adr.map(str::to_string),
    });
    Ok(mission)
}

pub fn question_add(
    missions: &[Mission],
    id: &str,
    text: &str,
    today: &str,
) -> Result<Mission, String> {
    let mut mission = model::find(missions, id)?.clone();
    check_line("a question", text)?;
    let seq = mission.next_question.max(
        mission
            .questions
            .iter()
            .map(|q| q.seq + 1)
            .max()
            .unwrap_or(1),
    );
    mission.questions.push(Question {
        seq,
        text: text.to_string(),
        status: QuestionStatus::Open,
        answer: None,
        answered_by: None,
        answered_on: None,
        asked_on: today.to_string(),
    });
    mission.next_question = seq + 1;
    Ok(mission)
}

pub fn question_edit(
    missions: &[Mission],
    id: &str,
    question: &str,
    text: &str,
) -> Result<Mission, String> {
    let mut mission = model::find(missions, id)?.clone();
    let seq = parse_question(question)?;
    check_line("a question", text)?;
    let mission_id = mission.id.clone();
    let found = mission
        .questions
        .iter_mut()
        .find(|q| q.seq == seq)
        .ok_or_else(|| {
            format!(
                "error: {} has no question {}",
                mission_id,
                question_label(seq)
            )
        })?;
    if found.status == QuestionStatus::Answered {
        return Err(format!(
            "error: {} of {} is answered; it cannot be edited",
            question_label(seq),
            mission_id
        ));
    }
    found.text = text.to_string();
    Ok(mission)
}

pub fn question_answer(
    missions: &[Mission],
    id: &str,
    question: &str,
    text: &str,
    by: &str,
    today: &str,
) -> Result<Mission, String> {
    let mut mission = model::find(missions, id)?.clone();
    let seq = parse_question(question)?;
    check_line("an answer", text)?;
    check_who(by)?;
    let mission_id = mission.id.clone();
    let found = mission
        .questions
        .iter_mut()
        .find(|q| q.seq == seq)
        .ok_or_else(|| {
            format!(
                "error: {} has no question {}",
                mission_id,
                question_label(seq)
            )
        })?;
    if found.status == QuestionStatus::Answered {
        return Err(format!(
            "error: {} {} is already answered",
            mission_id,
            question_label(seq)
        ));
    }
    found.status = QuestionStatus::Answered;
    found.answer = Some(text.to_string());
    found.answered_by = Some(by.to_string());
    found.answered_on = Some(today.to_string());
    Ok(mission)
}

// --- Derived state and output ---

/// The sequence numbers in `blocked_by` whose task is not done. A blocker that is absent
/// counts as still blocking, so a lost record never frees a task.
pub fn task_blocking(mission: &Mission, task: &Task) -> Vec<i64> {
    task.blocked_by
        .iter()
        .copied()
        .filter(|by| {
            mission
                .tasks
                .iter()
                .find(|t| t.seq == *by)
                .is_none_or(|t| t.status != Status::Done)
        })
        .collect()
}

/// One `task list` line: id, status, blocked-by, description, tab-separated.
pub fn task_line(mission: &Mission, task: &Task) -> String {
    let blockers = task_blocking(mission, task);
    format!(
        "{}\t{}\t{}\t{}",
        task_label(task.seq),
        task.status.as_str(),
        if blockers.is_empty() {
            "-".to_string()
        } else {
            blockers
                .iter()
                .map(|b| task_label(*b))
                .collect::<Vec<_>>()
                .join(",")
        },
        task.description
    )
}

/// One `decision list` line: date, who, task or `-`, text, ADR path or `-`,
/// tab-separated.
pub fn decision_line(decision: &Decision) -> String {
    format!(
        "{}\t{}\t{}\t{}\t{}",
        decision.decided_on,
        decision.decided_by,
        decision.task.map_or("-".to_string(), task_label),
        decision.text,
        decision.adr.as_deref().unwrap_or("-")
    )
}

/// The lines of `decision list`. With a mission, that mission's decisions in the five
/// columns of [`decision_line`]. With only an ADR path, the decisions of every mission
/// that name it, each line led by the mission ID. With both, the mission's decisions
/// that name the ADR, in the five columns.
pub fn decision_rows(
    missions: &[Mission],
    mission: Option<&str>,
    adr: Option<&str>,
) -> Result<Vec<String>, String> {
    let adr = adr.map(normalize_adr).transpose()?;
    let names = |d: &&Decision| adr.as_deref().is_none_or(|a| d.adr.as_deref() == Some(a));
    match mission {
        Some(id) => Ok(mission_of(missions, id)?
            .decisions
            .iter()
            .filter(names)
            .map(decision_line)
            .collect()),
        None if adr.is_some() => Ok(missions
            .iter()
            .flat_map(|m| {
                m.decisions
                    .iter()
                    .filter(names)
                    .map(move |d| format!("{}\t{}", m.id, decision_line(d)))
            })
            .collect()),
        None => Err(
            "error: decision list needs a mission, or --adr with the path of an ADR".to_string(),
        ),
    }
}

/// One `question list` line: mission, id, date asked, text, tab-separated.
pub fn question_line(mission: &Mission, question: &Question) -> String {
    format!(
        "{}\t{}\t{}\t{}",
        mission.id,
        question_label(question.seq),
        question.asked_on,
        question.text
    )
}

#[derive(Serialize)]
pub struct JsonBlocker {
    id: String,
    blocking: bool,
}

#[derive(Serialize)]
pub struct JsonTask<'a> {
    id: String,
    description: &'a str,
    status: &'a str,
    blocked: bool,
    blocked_by: Vec<JsonBlocker>,
}

#[derive(Serialize)]
pub struct JsonDecision<'a> {
    text: &'a str,
    decided_by: &'a str,
    decided_on: &'a str,
    task: Option<String>,
    adr: Option<&'a str>,
}

#[derive(Serialize)]
pub struct JsonQuestion<'a> {
    id: String,
    text: &'a str,
    status: &'a str,
    asked_on: &'a str,
    answer: Option<&'a str>,
    answered_by: Option<&'a str>,
    answered_on: Option<&'a str>,
}

pub fn json_tasks(mission: &Mission) -> Vec<JsonTask<'_>> {
    mission
        .tasks
        .iter()
        .map(|task| {
            let blockers = task_blocking(mission, task);
            JsonTask {
                id: task_label(task.seq),
                description: &task.description,
                status: task.status.as_str(),
                blocked: !blockers.is_empty(),
                blocked_by: task
                    .blocked_by
                    .iter()
                    .map(|by| JsonBlocker {
                        id: task_label(*by),
                        blocking: blockers.contains(by),
                    })
                    .collect(),
            }
        })
        .collect()
}

pub fn json_decisions(mission: &Mission) -> Vec<JsonDecision<'_>> {
    mission
        .decisions
        .iter()
        .map(|d| JsonDecision {
            text: &d.text,
            decided_by: &d.decided_by,
            decided_on: &d.decided_on,
            task: d.task.map(task_label),
            adr: d.adr.as_deref(),
        })
        .collect()
}

pub fn json_questions(mission: &Mission) -> Vec<JsonQuestion<'_>> {
    mission
        .questions
        .iter()
        .map(|q| JsonQuestion {
            id: question_label(q.seq),
            text: &q.text,
            status: q.status.as_str(),
            asked_on: &q.asked_on,
            answer: q.answer.as_deref(),
            answered_by: q.answered_by.as_deref(),
            answered_on: q.answered_on.as_deref(),
        })
        .collect()
}

// --- Commands ---

/// Today's date, `YYYY-MM-DD`, UTC, from the system clock.
fn today() -> String {
    clock::utc_now().chars().take(10).collect()
}

#[derive(Subcommand)]
pub enum TaskCommands {
    #[command(about = "Add a task to a mission; prints its ID, for example T-01")]
    Add {
        #[arg(help = "Mission ID")]
        mission: String,
        #[arg(allow_hyphen_values = true, help = "One-line description")]
        description: String,
    },
    #[command(about = "Set a task field: status (todo, in-progress or done) or description")]
    Set {
        mission: String,
        task: String,
        field: String,
        #[arg(allow_hyphen_values = true)]
        value: String,
    },
    #[command(
        about = "Record that a task is blocked by another task of the mission; refuses a cycle"
    )]
    Block {
        mission: String,
        task: String,
        #[arg(help = "ID of the blocking task")]
        by: String,
    },
    #[command(about = "Remove a task block")]
    Unblock {
        mission: String,
        task: String,
        by: String,
    },
    #[command(
        about = "List the tasks of a mission, one per line: id, status, blocked-by, description. The columns are aligned with spaces; --porcelain prints them tab-separated"
    )]
    List {
        mission: String,
        #[arg(
            long,
            help = "Print tab-separated columns, the stable form for scripts, instead of aligned columns"
        )]
        porcelain: bool,
    },
}

#[derive(Subcommand)]
pub enum DecisionCommands {
    #[command(about = "Record a decision of a mission, and optionally the task it concerns")]
    Add {
        mission: String,
        #[arg(allow_hyphen_values = true, help = "One-line text of the decision")]
        text: String,
        #[arg(long, help = "Who decided")]
        by: String,
        #[arg(long, help = "ID of a task of the mission, for example T-01")]
        task: Option<String>,
        #[arg(
            long,
            value_name = "YYYY-MM-DD",
            help = "The date of the decision, a real calendar date; today (UTC) when omitted"
        )]
        on: Option<String>,
        #[arg(
            long,
            value_name = "PATH",
            help = "The ADR this decision follows: a file in the repository you run in, as a path from its root, for example docs/adr/0014-dolt-is-the-ledger-store-behind-a-storage-seam.md"
        )]
        adr: Option<String>,
    },
    #[command(
        about = "List the decisions of a mission, oldest first: date, who, task, text, ADR path (- for none). The columns are aligned with spaces; --porcelain prints them tab-separated. With --adr and no mission, list the decisions of every mission that name the ADR, with the mission ID as a first column"
    )]
    List {
        mission: Option<String>,
        #[arg(
            long,
            help = "Print tab-separated columns, the stable form for scripts, instead of aligned columns"
        )]
        porcelain: bool,
        #[arg(
            long,
            value_name = "PATH",
            help = "Only the decisions that name this ADR (a path from the repository root)"
        )]
        adr: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum QuestionCommands {
    #[command(about = "Ask an open question of a mission; prints its ID, for example Q-01")]
    Add {
        mission: String,
        #[arg(allow_hyphen_values = true, help = "One-line text of the question")]
        text: String,
    },
    #[command(
        about = "Rewrite the text of an open question; an answered question cannot be edited"
    )]
    Edit {
        mission: String,
        question: String,
        #[arg(allow_hyphen_values = true, help = "One-line new text of the question")]
        text: String,
    },
    #[command(about = "Answer an open question")]
    Answer {
        mission: String,
        question: String,
        #[arg(allow_hyphen_values = true, help = "One-line answer")]
        text: String,
        #[arg(long, help = "Who answered")]
        by: String,
    },
    #[command(
        about = "List open questions, one per line: mission, id, date asked, text; all missions when none is given. The columns are aligned with spaces; --porcelain prints them tab-separated"
    )]
    List {
        mission: Option<String>,
        #[arg(
            long,
            help = "Print tab-separated columns, the stable form for scripts, instead of aligned columns"
        )]
        porcelain: bool,
    },
}

pub fn run_task(action: TaskCommands) -> Result<(), String> {
    run_task_with(open()?.as_ref(), action)
}

pub fn run_decision(action: DecisionCommands) -> Result<(), String> {
    run_decision_with(open()?.as_ref(), action)
}

pub fn run_question(action: QuestionCommands) -> Result<(), String> {
    run_question_with(open()?.as_ref(), action)
}

fn open() -> Result<Box<dyn LedgerStore>, String> {
    let cwd = std::env::current_dir()
        .map_err(|e| format!("error: could not read the current directory: {}", e))?;
    store::open(&cwd)
}

/// The top level of the working tree of the repository the command runs in. An ADR path
/// is checked against it, whichever repository holds the ledger.
fn repo_root() -> Result<std::path::PathBuf, String> {
    let cwd = std::env::current_dir()
        .map_err(|e| format!("error: could not read the current directory: {}", e))?;
    crate::ledger::git::Git::new(cwd)
        .run_line(["rev-parse", "--show-toplevel"])
        .map(std::path::PathBuf::from)
        .map_err(|_| {
            "error: --adr needs a working tree: run tsk inside the repository whose ADR you name"
                .to_string()
        })
}

fn mission_of<'a>(missions: &'a [Mission], id: &str) -> Result<&'a Mission, String> {
    model::find(missions, id)
}

pub fn run_task_with(store: &dyn LedgerStore, action: TaskCommands) -> Result<(), String> {
    match action {
        TaskCommands::Add {
            mission,
            description,
        } => {
            let saved = change(store, |all| task_add(all, &mission, &description))?;
            println!("{}", task_label(saved.next_task - 1));
        }
        TaskCommands::Set {
            mission,
            task,
            field,
            value,
        } => {
            change(store, |all| task_set(all, &mission, &task, &field, &value))?;
        }
        TaskCommands::Block { mission, task, by } => {
            change(store, |all| task_block(all, &mission, &task, &by))?;
        }
        TaskCommands::Unblock { mission, task, by } => {
            change(store, |all| task_unblock(all, &mission, &task, &by))?;
        }
        TaskCommands::List { mission, porcelain } => {
            let all = store.missions_load()?;
            let found = mission_of(&all, &mission)?;
            let lines = found.tasks.iter().map(|t| task_line(found, t)).collect();
            print_lines(lines, porcelain);
        }
    }
    Ok(())
}

pub fn run_decision_with(store: &dyn LedgerStore, action: DecisionCommands) -> Result<(), String> {
    match action {
        DecisionCommands::Add {
            mission,
            text,
            by,
            task,
            on,
            adr,
        } => {
            let date = on.unwrap_or_else(today);
            check_date(&date)?;
            let adr = adr
                .map(|path| check_adr_path(&repo_root()?, &path))
                .transpose()?;
            change(store, |all| {
                decision_add(
                    all,
                    &mission,
                    &text,
                    &by,
                    task.as_deref(),
                    adr.as_deref(),
                    &date,
                )
            })?;
        }
        DecisionCommands::List {
            mission,
            adr,
            porcelain,
        } => {
            let all = store.missions_load()?;
            print_lines(
                decision_rows(&all, mission.as_deref(), adr.as_deref())?,
                porcelain,
            );
        }
    }
    Ok(())
}

pub fn run_question_with(store: &dyn LedgerStore, action: QuestionCommands) -> Result<(), String> {
    match action {
        QuestionCommands::Add { mission, text } => {
            let date = today();
            let saved = change(store, |all| question_add(all, &mission, &text, &date))?;
            println!("{}", question_label(saved.next_question - 1));
        }
        QuestionCommands::Edit {
            mission,
            question,
            text,
        } => {
            change(store, |all| question_edit(all, &mission, &question, &text))?;
        }
        QuestionCommands::Answer {
            mission,
            question,
            text,
            by,
        } => {
            let date = today();
            change(store, |all| {
                question_answer(all, &mission, &question, &text, &by, &date)
            })?;
        }
        QuestionCommands::List { mission, porcelain } => {
            let all = store.missions_load()?;
            let shown: Vec<&Mission> = match &mission {
                Some(id) => vec![mission_of(&all, id)?],
                None => all.iter().collect(),
            };
            let lines = shown
                .into_iter()
                .flat_map(|found| {
                    found
                        .questions
                        .iter()
                        .filter(|q| q.status == QuestionStatus::Open)
                        .map(move |q| question_line(found, q))
                })
                .collect();
            print_lines(lines, porcelain);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mission::model::{create, Category};

    fn one_mission() -> Vec<Mission> {
        vec![create(&[], "M-A", "A", Category::Operational, None).unwrap()]
    }

    fn put(missions: &mut [Mission], updated: Mission) {
        let at = missions.iter().position(|m| m.id == updated.id).unwrap();
        missions[at] = updated;
    }

    fn with_tasks(n: usize) -> Vec<Mission> {
        let mut all = one_mission();
        for i in 0..n {
            let m = task_add(&all, "M-A", &format!("task {}", i + 1)).unwrap();
            put(&mut all, m);
        }
        all
    }

    #[test]
    fn labels_are_padded_to_two_digits_and_widen_past_ninety_nine() {
        assert_eq!(task_label(1), "T-01");
        assert_eq!(task_label(99), "T-99");
        assert_eq!(task_label(100), "T-100");
        assert_eq!(question_label(7), "Q-07");
        assert_eq!(parse_task("T-07").unwrap(), 7);
        assert_eq!(parse_task("T-7").unwrap(), 7);
        assert_eq!(parse_task("T-100").unwrap(), 100);
        for bad in [
            "", "T", "T-", "T-0", "T-00", "t-01", "Q-01", "T-1a", "T--1", "T-+1",
        ] {
            assert!(parse_task(bad).is_err(), "{}", bad);
        }
        assert!(parse_question("T-01").is_err());
    }

    #[test]
    fn task_numbers_only_grow() {
        let all = with_tasks(3);
        assert_eq!(
            all[0].tasks.iter().map(|t| t.seq).collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        assert_eq!(all[0].next_task, 4);
        // A counter that is ahead of the rows is honoured.
        let mut ahead = all[0].clone();
        ahead.next_task = 10;
        let m = task_add(&[ahead], "M-A", "later").unwrap();
        assert_eq!(m.tasks.last().unwrap().seq, 10);
    }

    #[test]
    fn a_block_refuses_self_unknown_duplicates_and_cycles() {
        let mut all = with_tasks(3);
        assert!(task_block(&all, "M-A", "T-01", "T-01")
            .unwrap_err()
            .contains("itself"));
        assert!(task_block(&all, "M-A", "T-01", "T-09")
            .unwrap_err()
            .contains("T-09"));
        assert!(task_block(&all, "M-Z", "T-01", "T-02")
            .unwrap_err()
            .contains("M-Z"));
        let m = task_block(&all, "M-A", "T-01", "T-02").unwrap();
        put(&mut all, m);
        let m = task_block(&all, "M-A", "T-02", "T-03").unwrap();
        put(&mut all, m);
        let err = task_block(&all, "M-A", "T-03", "T-01").unwrap_err();
        assert!(
            err.contains("cycle (T-03 -> T-01 -> T-02 -> T-03)"),
            "{}",
            err
        );
        assert!(task_block(&all, "M-A", "T-01", "T-02")
            .unwrap_err()
            .contains("already blocked"));
        assert!(task_unblock(&all, "M-A", "T-01", "T-03")
            .unwrap_err()
            .contains("not blocked"));
    }

    #[test]
    fn blocked_is_derived_from_blockers_that_are_not_done() {
        let mut all = with_tasks(2);
        let m = task_block(&all, "M-A", "T-01", "T-02").unwrap();
        put(&mut all, m);
        let line = |all: &[Mission]| task_line(&all[0], &all[0].tasks[0]);
        assert_eq!(line(&all), "T-01\ttodo\tT-02\ttask 1");
        let m = task_set(&all, "M-A", "T-02", "status", "done").unwrap();
        put(&mut all, m);
        assert_eq!(line(&all), "T-01\ttodo\t-\ttask 1");
    }

    #[test]
    fn text_is_one_line() {
        let all = one_mission();
        for bad in ["", "  ", "a\nb", "a\tb", "a\rb"] {
            assert!(task_add(&all, "M-A", bad).is_err(), "{:?}", bad);
            assert!(question_add(&all, "M-A", bad, "2026-01-01").is_err());
            assert!(decision_add(&all, "M-A", bad, "jim", None, None, "2026-01-01").is_err());
        }
    }

    #[test]
    fn a_question_is_answered_once() {
        let mut all = one_mission();
        let m = question_add(&all, "M-A", "Why?", "2026-01-02").unwrap();
        put(&mut all, m);
        let m = question_answer(&all, "M-A", "Q-01", "Because.", "jim", "2026-01-03").unwrap();
        assert_eq!(m.questions[0].status, QuestionStatus::Answered);
        assert_eq!(m.questions[0].answered_on.as_deref(), Some("2026-01-03"));
        put(&mut all, m);
        assert!(
            question_answer(&all, "M-A", "Q-01", "Again.", "jim", "2026-01-04")
                .unwrap_err()
                .contains("already answered")
        );
        assert!(
            question_answer(&all, "M-A", "Q-02", "x", "jim", "2026-01-04")
                .unwrap_err()
                .contains("Q-02")
        );
    }

    #[test]
    fn only_an_open_question_is_edited() {
        let mut all = one_mission();
        let m = question_add(&all, "M-A", "Why?", "2026-01-02").unwrap();
        put(&mut all, m);
        let m = question_edit(&all, "M-A", "Q-01", "Why not?").unwrap();
        assert_eq!(m.questions[0].text, "Why not?");
        assert_eq!(m.questions[0].asked_on, "2026-01-02");
        put(&mut all, m);
        assert!(question_edit(&all, "M-A", "Q-01", "a\nb").is_err());
        assert!(question_edit(&all, "M-A", "Q-02", "x")
            .unwrap_err()
            .contains("has no question Q-02"));
        let m = question_answer(&all, "M-A", "Q-01", "Because.", "jim", "2026-01-03").unwrap();
        put(&mut all, m);
        assert_eq!(
            question_edit(&all, "M-A", "Q-01", "Late").unwrap_err(),
            "error: Q-01 of M-A is answered; it cannot be edited"
        );
    }

    #[test]
    fn an_adr_path_is_relative_and_one_spelling() {
        assert_eq!(
            normalize_adr("docs/adr/0014-x.md").unwrap(),
            "docs/adr/0014-x.md"
        );
        assert_eq!(
            normalize_adr("./docs//adr/0014-x.md").unwrap(),
            "docs/adr/0014-x.md"
        );
        for bad in [
            "",
            "  ",
            "/docs/adr/x.md",
            "../x.md",
            "docs/../x.md",
            "a/b\tc.md",
            ".",
            "./",
        ] {
            assert!(normalize_adr(bad).is_err(), "{:?}", bad);
        }
        assert!(normalize_adr(&"a".repeat(1025)).is_err());
        assert!(normalize_adr(&"a".repeat(1024)).is_ok());
    }

    #[test]
    fn an_adr_path_must_be_a_file_in_the_working_tree() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("docs/adr")).unwrap();
        std::fs::write(dir.path().join("docs/adr/0001-a.md"), "# A\n").unwrap();
        assert_eq!(
            check_adr_path(dir.path(), "./docs/adr/0001-a.md").unwrap(),
            "docs/adr/0001-a.md"
        );
        for missing in ["docs/adr/0002-b.md", "docs/adr", "docs"] {
            let err = check_adr_path(dir.path(), missing).unwrap_err();
            assert!(err.contains("no such file in the repository"), "{}", err);
        }
        assert!(check_adr_path(dir.path(), "../outside").is_err());
    }

    #[test]
    fn decision_rows_filter_by_mission_and_adr() {
        let mut all = one_mission();
        all.push(create(&all, "M-B", "B", Category::Operational, None).unwrap());
        let m = decision_add(
            &all,
            "M-A",
            "One",
            "jim",
            None,
            Some("docs/adr/1.md"),
            "2026-01-01",
        )
        .unwrap();
        put(&mut all, m);
        let m = decision_add(&all, "M-A", "Two", "jim", None, None, "2026-01-02").unwrap();
        put(&mut all, m);
        let m = decision_add(
            &all,
            "M-B",
            "Three",
            "ann",
            None,
            Some("docs/adr/1.md"),
            "2026-01-03",
        )
        .unwrap();
        put(&mut all, m);
        assert_eq!(
            decision_rows(&all, Some("M-A"), None).unwrap(),
            vec![
                "2026-01-01\tjim\t-\tOne\tdocs/adr/1.md",
                "2026-01-02\tjim\t-\tTwo\t-"
            ]
        );
        assert_eq!(
            decision_rows(&all, None, Some("./docs/adr/1.md")).unwrap(),
            vec![
                "M-A\t2026-01-01\tjim\t-\tOne\tdocs/adr/1.md",
                "M-B\t2026-01-03\tann\t-\tThree\tdocs/adr/1.md"
            ]
        );
        assert_eq!(
            decision_rows(&all, Some("M-B"), Some("docs/adr/1.md")).unwrap(),
            vec!["2026-01-03\tann\t-\tThree\tdocs/adr/1.md"]
        );
        assert!(decision_rows(&all, Some("M-A"), Some("docs/adr/2.md"))
            .unwrap()
            .is_empty());
        assert!(decision_rows(&all, None, None).is_err());
        assert!(decision_rows(&all, Some("M-Z"), None).is_err());
    }

    #[test]
    fn a_date_is_a_real_calendar_date() {
        for ok in ["2026-01-31", "2024-02-29", "2000-02-29", "2026-12-01"] {
            assert!(check_date(ok).is_ok(), "{}", ok);
        }
        for bad in [
            "2026-02-30",
            "2026-02-29",
            "1900-02-29",
            "2026-04-31",
            "2026-13-01",
            "2026-00-10",
            "2026-01-00",
            "0000-01-01",
            "2026-1-01",
            "26-01-01",
            "2026/01/01",
            "2026-01-011",
            "",
            "tomorrow",
            "２０２６-01-01",
        ] {
            assert!(check_date(bad).is_err(), "{}", bad);
        }
        let all = one_mission();
        assert!(decision_add(&all, "M-A", "Use Dolt", "jim", None, None, "2026-02-30").is_err());
        let m = decision_add(&all, "M-A", "Use Dolt", "jim", None, None, "2026-02-27").unwrap();
        assert_eq!(m.decisions[0].decided_on, "2026-02-27");
    }

    #[test]
    fn the_date_is_ten_characters() {
        let d = today();
        assert_eq!(d.len(), 10);
        assert_eq!(&d[4..5], "-");
        assert_eq!(&d[7..8], "-");
    }
}
