//! `tsk mission show <id>`: one mission as plain text for a person. It renders the same data
//! as `tsk mission list <id> --json` and adds no field.

use super::model::{self, Mission, QuestionStatus};
use super::records::{question_label, task_label};
use crate::ledger::store::LedgerStore;

/// The lines of the view of `mission`. `missions` is every mission, which decides whether
/// the blockers still block.
pub fn render(missions: &[Mission], mission: &Mission) -> Vec<String> {
    let mut lines = vec![format!("{}  {}", mission.id, mission.name)];
    let blockers = model::blocking(missions, mission);
    lines.push(format!(
        "Status: {}. Category: {}. Parent: {}.",
        mission.status.as_str(),
        mission.category.as_str(),
        mission.parent.as_deref().unwrap_or("none")
    ));
    lines.push(if mission.blocked_by.is_empty() {
        "Blocked by: none.".to_string()
    } else {
        let waits = mission.blocked_by.join(", ");
        if blockers.is_empty() {
            format!("Blocked by: {} (all done, so not blocked).", waits)
        } else {
            format!("Blocked by: {}.", waits)
        }
    });
    text_section(&mut lines, "Purpose", &mission.purpose);
    text_section(
        &mut lines,
        "Decision authority",
        &mission.decision_authority,
    );
    section(
        &mut lines,
        "Objectives",
        mission
            .objective
            .iter()
            .map(|o| {
                format!(
                    "{} [{}] {}",
                    o.seq,
                    if o.met { "met" } else { "open" },
                    o.outcome
                )
            })
            .collect(),
    );
    section(
        &mut lines,
        "Constraints",
        mission
            .constraints
            .iter()
            .map(|c| format!("{} {}", c.seq, c.text))
            .collect(),
    );
    section(
        &mut lines,
        "Out of scope",
        mission
            .out_of_scope
            .iter()
            .map(|c| format!("{} {}", c.seq, c.text))
            .collect(),
    );
    section(
        &mut lines,
        "Tasks",
        mission
            .tasks
            .iter()
            .map(|t| {
                let waits = if t.blocked_by.is_empty() {
                    String::new()
                } else {
                    let by: Vec<String> = t.blocked_by.iter().map(|n| task_label(*n)).collect();
                    format!(" (blocked by {})", by.join(", "))
                };
                format!(
                    "{} {}{}: {}",
                    task_label(t.seq),
                    t.status.as_str(),
                    waits,
                    t.description
                )
            })
            .collect(),
    );
    section(
        &mut lines,
        "Decisions",
        mission
            .decisions
            .iter()
            .enumerate()
            .map(|(at, d)| {
                let task = d
                    .task
                    .map(|n| format!(", {}", task_label(n)))
                    .unwrap_or_default();
                let adr = d
                    .adr
                    .as_deref()
                    .map(|a| format!(" [ADR {}]", a))
                    .unwrap_or_default();
                format!(
                    "{} {}, {}{}: {}{}",
                    at + 1,
                    d.decided_on,
                    d.decided_by,
                    task,
                    d.text,
                    adr
                )
            })
            .collect(),
    );
    section(
        &mut lines,
        "Open questions",
        mission
            .questions
            .iter()
            .filter(|q| q.status == QuestionStatus::Open)
            .map(|q| format!("{} {}: {}", question_label(q.seq), q.asked_on, q.text))
            .collect(),
    );
    section(
        &mut lines,
        "Answered questions",
        mission
            .questions
            .iter()
            .filter(|q| q.status != QuestionStatus::Open)
            .map(|q| {
                let by = q.answered_by.as_deref().unwrap_or("unknown");
                let on = q.answered_on.as_deref().unwrap_or("unknown date");
                format!(
                    "{} {}: {} Answer ({}, {}): {}",
                    question_label(q.seq),
                    q.asked_on,
                    q.text,
                    by,
                    on,
                    q.answer.as_deref().unwrap_or("")
                )
            })
            .collect(),
    );
    section(
        &mut lines,
        "Documents",
        mission
            .documents
            .iter()
            .map(|d| format!("{}  {}", d.role.as_str(), d.path))
            .collect(),
    );
    lines
}

fn text_section(lines: &mut Vec<String>, title: &str, text: &str) {
    lines.push(String::new());
    lines.push(format!("{}:", title));
    if text.trim().is_empty() {
        lines.push("  none".to_string());
    } else {
        lines.extend(text.lines().map(|l| format!("  {}", l)));
    }
}

fn section(lines: &mut Vec<String>, title: &str, items: Vec<String>) {
    lines.push(String::new());
    lines.push(format!("{}:", title));
    if items.is_empty() {
        lines.push("  none".to_string());
    }
    for item in items {
        let mut parts = item.lines();
        if let Some(first) = parts.next() {
            lines.push(format!("  {}", first));
        }
        lines.extend(parts.map(|l| format!("    {}", l)));
    }
}

pub fn run(store: &dyn LedgerStore, id: &str) -> Result<(), String> {
    let missions = store.missions_load()?;
    let mission = model::find(&missions, id)?;
    for line in render(&missions, mission) {
        println!("{}", line);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mission(id: &str) -> Mission {
        let json = format!(
            r#"{{"id":"{id}","name":"Name of {id}","category":"operational","status":"todo","parent":null,
"purpose":"Line one.\nLine two.","decision_authority":"","next_objective":2,
"objective":[{{"seq":1,"outcome":"Do it","met":true}}],"blocked_by":[],"documents":[],
"tasks":[],"decisions":[],"questions":[]}}"#
        );
        serde_json::from_str(&json).unwrap()
    }

    #[test]
    fn a_mission_prints_every_section_with_none_for_an_empty_one() {
        let m = mission("M-A");
        let lines = render(std::slice::from_ref(&m), &m);
        assert_eq!(lines[0], "M-A  Name of M-A");
        assert_eq!(
            lines[1],
            "Status: todo. Category: operational. Parent: none."
        );
        assert_eq!(lines[2], "Blocked by: none.");
        let text = lines.join("\n");
        assert!(
            text.contains("Purpose:\n  Line one.\n  Line two."),
            "{}",
            text
        );
        assert!(text.contains("Decision authority:\n  none"), "{}", text);
        assert!(text.contains("Objectives:\n  1 [met] Do it"), "{}", text);
        for title in [
            "Constraints",
            "Out of scope",
            "Tasks",
            "Decisions",
            "Open questions",
            "Answered questions",
            "Documents",
        ] {
            assert!(text.contains(&format!("{}:\n  none", title)), "{}", title);
        }
    }

    #[test]
    fn records_print_with_their_blockers_answers_and_documents() {
        let mut a = mission("M-A");
        let mut b = mission("M-B");
        b.blocked_by = vec!["M-A".to_string()];
        a.status = model::Status::Done;
        let blocked = render(&[a.clone(), b.clone()], &b);
        assert_eq!(blocked[2], "Blocked by: M-A (all done, so not blocked).");
        b.tasks.push(model::Task {
            seq: 1,
            description: "First".to_string(),
            status: model::Status::Done,
            blocked_by: vec![],
            uid: String::new(),
        });
        b.tasks.push(model::Task {
            seq: 2,
            description: "Second".to_string(),
            status: model::Status::Todo,
            blocked_by: vec![1],
            uid: String::new(),
        });
        b.decisions.push(model::Decision {
            seq: 1,
            text: "Go".to_string(),
            decided_by: "Jim".to_string(),
            decided_on: "2026-10-25".to_string(),
            task: Some(2),
            adr: Some("docs/adr/0001-x.md".to_string()),
            uid: String::new(),
        });
        b.questions.push(model::Question {
            seq: 1,
            text: "Why?".to_string(),
            status: QuestionStatus::Answered,
            answer: Some("Because.".to_string()),
            answered_by: Some("Jim".to_string()),
            answered_on: Some("2026-10-25".to_string()),
            asked_on: "2026-10-24".to_string(),
            uid: String::new(),
        });
        b.documents.push(model::Document {
            path: "missions/x.md".to_string(),
            role: model::Role::Report,
        });
        let text = render(&[a, b.clone()], &b).join("\n");
        assert!(text.contains("T-01 done: First"), "{}", text);
        assert!(
            text.contains("T-02 todo (blocked by T-01): Second"),
            "{}",
            text
        );
        assert!(
            text.contains("1 2026-10-25, Jim, T-02: Go [ADR docs/adr/0001-x.md]"),
            "{}",
            text
        );
        assert!(
            text.contains("Q-01 2026-10-24: Why? Answer (Jim, 2026-10-25): Because."),
            "{}",
            text
        );
        assert!(text.contains("Open questions:\n  none"), "{}", text);
        assert!(text.contains("report  missions/x.md"), "{}", text);
    }
}
