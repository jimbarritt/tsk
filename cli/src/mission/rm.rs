//! `tsk task rm`, `decision rm`, `question rm` and `mission rm`. A removal is a row delete in
//! a local Dolt commit, so the history of the ledger keeps what was there. Each command
//! refuses a record that another record refers to, and says what the refusal needs.

use super::model::{self, Mission};
use super::records::{parse_question, parse_task, question_label, task_label, task_ref};
use crate::ledger::store::LedgerStore;

/// The missions to save and the lines to print.
pub struct Removed {
    pub missions: Vec<Mission>,
    pub lines: Vec<String>,
}

pub fn task_rm(missions: &[Mission], id: &str, task: &str, force: bool) -> Result<Removed, String> {
    let mut mission = model::find(missions, id)?.clone();
    let seq = parse_task(task)?;
    task_ref(&mission, seq)?;
    let label = task_label(seq);
    let mut refs: Vec<String> = Vec::new();
    if let Some(own) = mission.tasks.iter().find(|t| t.seq == seq) {
        refs.extend(
            own.blocked_by
                .iter()
                .map(|by| format!("{} is blocked by {}", label, task_label(*by))),
        );
    }
    refs.extend(
        mission
            .tasks
            .iter()
            .filter(|t| t.blocked_by.contains(&seq))
            .map(|t| format!("{} is blocked by {}", task_label(t.seq), label)),
    );
    let named: Vec<usize> = mission
        .decisions
        .iter()
        .enumerate()
        .filter(|(_, d)| d.task == Some(seq))
        .map(|(at, _)| at + 1)
        .collect();
    refs.extend(
        named
            .iter()
            .map(|at| format!("decision {} names {}", at, label)),
    );
    if !refs.is_empty() && !force {
        return Err(format!(
            "error: {} {} is referred to by other records: {}.\n       \
             Remove the blocks with tsk task unblock and move or remove the decisions, or run the command again with --force to drop the blocks and clear the task of the decisions",
            id,
            label,
            refs.join("; ")
        ));
    }
    let mut lines: Vec<String> = refs
        .iter()
        .map(|r| format!("dropped in {}: {}", id, r))
        .collect();
    for other in &mut mission.tasks {
        other.blocked_by.retain(|by| *by != seq);
    }
    for decision in &mut mission.decisions {
        if decision.task == Some(seq) {
            decision.task = None;
        }
    }
    mission.tasks.retain(|t| t.seq != seq);
    lines.push(format!("removed task {} {}", id, label));
    Ok(Removed {
        missions: vec![mission],
        lines,
    })
}

pub fn decision_rm(missions: &[Mission], id: &str, n: usize) -> Result<Removed, String> {
    let mut mission = model::find(missions, id)?.clone();
    let mut ordered: Vec<i64> = mission.decisions.iter().map(|d| d.seq).collect();
    ordered.sort();
    let seq = n
        .checked_sub(1)
        .and_then(|at| ordered.get(at))
        .copied()
        .ok_or_else(|| {
            format!(
                "error: {} has {} decisions, and no decision {}; a decision is numbered by its place in tsk decision list {}, from 1",
                id,
                mission.decisions.len(),
                n,
                id
            )
        })?;
    mission.decisions.retain(|d| d.seq != seq);
    Ok(Removed {
        missions: vec![mission],
        lines: vec![format!("removed decision {} {}", id, n)],
    })
}

pub fn question_rm(missions: &[Mission], id: &str, question: &str) -> Result<Removed, String> {
    let mut mission = model::find(missions, id)?.clone();
    let seq = parse_question(question)?;
    if !mission.questions.iter().any(|q| q.seq == seq) {
        return Err(format!(
            "error: {} has no question {}",
            id,
            question_label(seq)
        ));
    }
    mission.questions.retain(|q| q.seq != seq);
    Ok(Removed {
        missions: vec![mission],
        lines: vec![format!("removed question {} {}", id, question_label(seq))],
    })
}

/// What removing a mission needs and does.
pub struct MissionRemoval {
    /// The other missions, with this one no longer in their blockers.
    pub others: Vec<Mission>,
    /// The threads bound to the mission, to detach from it.
    pub threads: Vec<String>,
    pub lines: Vec<String>,
}

/// Plan the removal of the mission `id`. `threads` are the threads bound to it. Child
/// missions always refuse. Everything else the mission holds or is linked to refuses unless
/// `force` removes it too: the mission's tasks, decisions and questions go with it, blocks
/// in both directions are dropped, linked documents are unlinked and kept, and bound
/// threads are detached from the mission and kept.
pub fn mission_rm(
    missions: &[Mission],
    threads: Vec<String>,
    id: &str,
    force: bool,
) -> Result<MissionRemoval, String> {
    let mission = model::find(missions, id)?;
    let children: Vec<&str> = missions
        .iter()
        .filter(|m| m.parent.as_deref() == Some(mission.id.as_str()))
        .map(|m| m.id.as_str())
        .collect();
    if !children.is_empty() {
        return Err(format!(
            "error: {} has child missions: {}.\n       \
             Remove the children first",
            id,
            children.join(", ")
        ));
    }
    let blocking_others: Vec<&str> = missions
        .iter()
        .filter(|m| m.blocked_by.contains(&mission.id))
        .map(|m| m.id.as_str())
        .collect();
    let mut holds: Vec<String> = Vec::new();
    let count = |n: usize, noun: &str| format!("{} {}{}", n, noun, if n == 1 { "" } else { "s" });
    if !mission.tasks.is_empty() {
        holds.push(count(mission.tasks.len(), "task"));
    }
    if !mission.decisions.is_empty() {
        holds.push(count(mission.decisions.len(), "decision"));
    }
    if !mission.questions.is_empty() {
        holds.push(count(mission.questions.len(), "question"));
    }
    if !threads.is_empty() {
        holds.push(count(threads.len(), "thread"));
    }
    if !mission.blocked_by.is_empty() {
        holds.push(format!("blocked by {}", mission.blocked_by.join(", ")));
    }
    if !blocking_others.is_empty() {
        holds.push(format!("blocks {}", blocking_others.join(", ")));
    }
    if !mission.documents.is_empty() {
        holds.push(count(mission.documents.len(), "linked document"));
    }
    if !holds.is_empty() && !force {
        return Err(format!(
            "error: {} is referred to or holds records: {}.\n       \
             Run the command again with --force to remove them with the mission (blocks are dropped, linked documents are unlinked and kept, threads are detached and kept)",
            id,
            holds.join(", ")
        ));
    }
    let mut lines: Vec<String> = Vec::new();
    for thread in &threads {
        lines.push(format!("detached thread {} from {}", thread, id));
    }
    for by in &mission.blocked_by {
        lines.push(format!("dropped block: {} is blocked by {}", id, by));
    }
    for other in &blocking_others {
        lines.push(format!("dropped block: {} is blocked by {}", other, id));
    }
    for document in &mission.documents {
        lines.push(format!("unlinked document {}", document.path));
    }
    for task in &mission.tasks {
        lines.push(format!("removed task {} {}", id, task_label(task.seq)));
    }
    if !mission.decisions.is_empty() {
        lines.push(format!(
            "removed {} from {}",
            count(mission.decisions.len(), "decision"),
            id
        ));
    }
    for question in &mission.questions {
        lines.push(format!(
            "removed question {} {}",
            id,
            question_label(question.seq)
        ));
    }
    lines.push(format!("removed mission {}", id));
    let others = missions
        .iter()
        .filter(|m| m.blocked_by.contains(&mission.id))
        .map(|m| {
            let mut other = m.clone();
            other.blocked_by.retain(|by| *by != mission.id);
            other
        })
        .collect();
    Ok(MissionRemoval {
        others,
        threads,
        lines,
    })
}

pub fn save_removed(store: &dyn LedgerStore, removed: Removed) -> Result<(), String> {
    store.missions_save(&removed.missions)?;
    for line in removed.lines {
        println!("{}", line);
    }
    Ok(())
}

/// `tsk mission rm <id>`. The plan is made first; nothing changes when it refuses.
pub fn mission_rm_run(store: &dyn LedgerStore, id: &str, force: bool) -> Result<(), String> {
    let missions = store.missions_load()?;
    let mission = model::find(&missions, id)?;
    let mut bound = Vec::new();
    for thread in store.thread_ids()? {
        if store.thread_info(&thread)?.mission_id.as_deref() == Some(mission.id.as_str()) {
            bound.push(thread);
        }
    }
    let plan = mission_rm(&missions, bound, id, force)?;
    if !plan.others.is_empty() {
        store.missions_save(&plan.others)?;
    }
    for thread in &plan.threads {
        let briefing = store.thread_info(thread)?.briefing_path;
        store.thread_set_mission(thread, None, briefing.as_deref())?;
    }
    store.mission_remove(id)?;
    for line in plan.lines {
        println!("{}", line);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mission(id: &str) -> Mission {
        let json = format!(
            r#"{{"id":"{id}","name":"{id}","category":"operational","status":"todo","parent":null,
"purpose":"","decision_authority":"","next_objective":1,"objective":[],"blocked_by":[],"documents":[],
"tasks":[],"decisions":[],"questions":[]}}"#
        );
        serde_json::from_str(&json).unwrap()
    }

    fn task(seq: i64, blocked_by: Vec<i64>) -> model::Task {
        model::Task {
            seq,
            description: format!("t{}", seq),
            status: model::Status::Todo,
            blocked_by,
            uid: String::new(),
        }
    }

    fn decision(seq: i64, task: Option<i64>) -> model::Decision {
        model::Decision {
            seq,
            text: format!("d{}", seq),
            decided_by: "Jim".to_string(),
            decided_on: "2026-10-25".to_string(),
            task,
            adr: None,
            uid: String::new(),
        }
    }

    #[test]
    fn a_task_with_no_references_is_removed() {
        let mut a = mission("M-A");
        a.tasks = vec![task(1, vec![]), task(2, vec![])];
        let done = task_rm(&[a], "M-A", "T-02", false).unwrap();
        assert_eq!(done.lines, vec!["removed task M-A T-02"]);
        assert_eq!(done.missions[0].tasks.len(), 1);
    }

    #[test]
    fn a_task_that_blocks_or_is_blocked_or_is_named_is_refused_without_force() {
        let mut a = mission("M-A");
        a.tasks = vec![task(1, vec![]), task(2, vec![1]), task(3, vec![])];
        a.decisions = vec![decision(1, Some(1))];
        for label in ["T-01", "T-02"] {
            let err = task_rm(std::slice::from_ref(&a), "M-A", label, false)
                .err()
                .unwrap();
            assert!(err.contains("referred to by other records"), "{}", err);
            assert!(err.contains("T-02 is blocked by T-01"), "{}", err);
        }
        let err = task_rm(std::slice::from_ref(&a), "M-A", "T-01", false)
            .err()
            .unwrap();
        assert!(err.contains("decision 1 names T-01"), "{}", err);
        assert!(err.contains("--force"), "{}", err);
        assert!(task_rm(std::slice::from_ref(&a), "M-A", "T-03", false).is_ok());
        let forced = task_rm(&[a], "M-A", "T-01", true).unwrap();
        let m = &forced.missions[0];
        assert!(m
            .tasks
            .iter()
            .all(|t| t.blocked_by.is_empty() && t.seq != 1));
        assert_eq!(m.decisions[0].task, None);
        assert!(forced
            .lines
            .contains(&"dropped in M-A: decision 1 names T-01".to_string()));
        assert_eq!(forced.lines.last().unwrap(), "removed task M-A T-01");
    }

    #[test]
    fn a_decision_and_a_question_are_removed_by_place_and_by_number() {
        let mut a = mission("M-A");
        a.decisions = vec![decision(5, None), decision(9, None)];
        let done = decision_rm(&[a.clone()], "M-A", 2).unwrap();
        assert_eq!(done.missions[0].decisions.len(), 1);
        assert_eq!(done.missions[0].decisions[0].seq, 5);
        assert!(decision_rm(&[a], "M-A", 3)
            .err()
            .unwrap()
            .contains("no decision 3"));
        let mut b = mission("M-B");
        b.questions = vec![model::Question {
            seq: 1,
            text: "Why?".to_string(),
            status: model::QuestionStatus::Open,
            answer: None,
            answered_by: None,
            answered_on: None,
            asked_on: "2026-10-25".to_string(),
            uid: String::new(),
        }];
        assert!(question_rm(std::slice::from_ref(&b), "M-B", "Q-02").is_err());
        let done = question_rm(&[b], "M-B", "Q-01").unwrap();
        assert!(done.missions[0].questions.is_empty());
    }

    #[test]
    fn a_mission_with_records_is_refused_and_force_lists_what_goes() {
        let mut a = mission("M-A");
        let mut b = mission("M-B");
        a.tasks = vec![task(1, vec![])];
        a.blocked_by = vec!["M-B".to_string()];
        a.documents = vec![model::Document {
            path: "x.md".to_string(),
            role: model::Role::Report,
        }];
        let mut c = mission("M-C");
        c.blocked_by = vec!["M-A".to_string()];
        b.purpose = "stays".to_string();
        let all = vec![a, b, c];
        let err = mission_rm(&all, vec!["th1".to_string()], "M-A", false)
            .err()
            .unwrap();
        assert!(
            err.contains("1 task, 1 thread, blocked by M-B, blocks M-C, 1 linked document"),
            "{}",
            err
        );
        let plan = mission_rm(&all, vec!["th1".to_string()], "M-A", true).unwrap();
        assert_eq!(plan.others.len(), 1);
        assert_eq!(plan.others[0].id, "M-C");
        assert!(plan.others[0].blocked_by.is_empty());
        assert_eq!(plan.lines.last().unwrap(), "removed mission M-A");
        assert!(plan
            .lines
            .contains(&"detached thread th1 from M-A".to_string()));
        assert!(plan.lines.contains(&"unlinked document x.md".to_string()));
    }

    #[test]
    fn a_mission_with_a_child_is_always_refused_and_a_bare_one_is_removed() {
        let a = mission("M-A");
        let mut b = mission("M-B");
        b.parent = Some("M-A".to_string());
        let err = mission_rm(&[a.clone(), b], vec![], "M-A", true)
            .err()
            .unwrap();
        assert!(err.contains("has child missions: M-B"), "{}", err);
        let plan = mission_rm(&[a], vec![], "M-A", false).unwrap();
        assert_eq!(plan.lines, vec!["removed mission M-A"]);
    }
}
