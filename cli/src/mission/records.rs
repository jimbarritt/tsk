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

pub fn decision_add(
    missions: &[Mission],
    id: &str,
    text: &str,
    by: &str,
    task: Option<&str>,
    today: &str,
) -> Result<Mission, String> {
    let mut mission = model::find(missions, id)?.clone();
    check_line("a decision", text)?;
    check_who(by)?;
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

/// One `decision list` line: date, who, task or `-`, text, tab-separated.
pub fn decision_line(decision: &Decision) -> String {
    format!(
        "{}\t{}\t{}\t{}",
        decision.decided_on,
        decision.decided_by,
        decision.task.map_or("-".to_string(), task_label),
        decision.text
    )
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
        about = "List the tasks of a mission, one per line: id, status, blocked-by, description"
    )]
    List { mission: String },
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
    },
    #[command(about = "List the decisions of a mission, oldest first: date, who, task, text")]
    List { mission: String },
}

#[derive(Subcommand)]
pub enum QuestionCommands {
    #[command(about = "Ask an open question of a mission; prints its ID, for example Q-01")]
    Add {
        mission: String,
        #[arg(allow_hyphen_values = true, help = "One-line text of the question")]
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
        about = "List open questions, one per line: mission, id, date asked, text; all missions when none is given"
    )]
    List { mission: Option<String> },
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
        TaskCommands::List { mission } => {
            let all = store.missions_load()?;
            let found = mission_of(&all, &mission)?;
            for task in &found.tasks {
                println!("{}", task_line(found, task));
            }
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
        } => {
            let date = today();
            change(store, |all| {
                decision_add(all, &mission, &text, &by, task.as_deref(), &date)
            })?;
        }
        DecisionCommands::List { mission } => {
            let all = store.missions_load()?;
            for decision in &mission_of(&all, &mission)?.decisions {
                println!("{}", decision_line(decision));
            }
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
        QuestionCommands::List { mission } => {
            let all = store.missions_load()?;
            let shown: Vec<&Mission> = match &mission {
                Some(id) => vec![mission_of(&all, id)?],
                None => all.iter().collect(),
            };
            for found in shown {
                for question in found
                    .questions
                    .iter()
                    .filter(|q| q.status == QuestionStatus::Open)
                {
                    println!("{}", question_line(found, question));
                }
            }
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
            assert!(decision_add(&all, "M-A", bad, "jim", None, "2026-01-01").is_err());
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
    fn the_date_is_ten_characters() {
        let d = today();
        assert_eq!(d.len(), 10);
        assert_eq!(&d[4..5], "-");
        assert_eq!(&d[7..8], "-");
    }
}
