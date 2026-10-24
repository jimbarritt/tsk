//! Rename and move: `tsk mission mv`, `tsk task mv`, `tsk decision mv`, `tsk question mv`
//! and `tsk ledger doc mv`. The rules work on plain [`Mission`] values, like the other
//! record rules, and each command prints one line per change, so the output is the record.
//!
//! A mission is renamed by changing its code. Every other table names the mission by its
//! internal ID, so no other row changes. A task, a decision or a question moves by being
//! removed from one mission and added to another with the next free number there. The
//! number it had in the first mission is not used again.

use crate::ledger::store::{check_docs_path, LedgerStore};

use super::model::{self, Decision, Mission, Question, Task};
use super::records::{parse_question, parse_task, question_label, task_label, task_ref};

/// What a move changed: the two missions to save, and the lines that say what moved.
pub struct Moved {
    pub source: Mission,
    pub target: Mission,
    pub lines: Vec<String>,
}

fn two<'a>(
    missions: &'a [Mission],
    from: &str,
    to: &str,
) -> Result<(&'a Mission, &'a Mission), String> {
    let source = model::find(missions, from)?;
    let target = model::find(missions, to)?;
    if source.id == target.id {
        return Err(format!(
            "error: {} is both the mission it moves from and the mission it moves to",
            from
        ));
    }
    Ok((source, target))
}

// --- Tasks ---

/// Move a task, and the decisions that name it, from mission `from` to mission `to`. A
/// task with blocks, in either direction, is refused unless `force` drops them.
pub fn task_mv(
    missions: &[Mission],
    from: &str,
    task: &str,
    to: &str,
    force: bool,
) -> Result<Moved, String> {
    let (source, target) = two(missions, from, to)?;
    let (mut source, mut target) = (source.clone(), target.clone());
    let seq = parse_task(task)?;
    let moved = task_ref(&source, seq)?.clone();
    let label = task_label(seq);

    // The blocks of a task are rows of its own mission, so they cannot follow it.
    let blocks: Vec<String> = moved
        .blocked_by
        .iter()
        .map(|by| format!("{} is blocked by {}", label, task_label(*by)))
        .chain(
            source
                .tasks
                .iter()
                .filter(|other| other.blocked_by.contains(&seq))
                .map(|other| format!("{} is blocked by {}", task_label(other.seq), label)),
        )
        .collect();
    if !blocks.is_empty() && !force {
        return Err(format!(
            "error: {} {} has blocks, which stay in {}: {}.\n       \
             Remove them with tsk task unblock, or run the command again with --force to drop them",
            from,
            label,
            from,
            blocks.join("; ")
        ));
    }
    let mut lines: Vec<String> = blocks
        .iter()
        .map(|block| format!("dropped block in {}: {}", from, block))
        .collect();
    for other in &mut source.tasks {
        other.blocked_by.retain(|by| *by != seq);
    }

    let new_seq = target
        .next_task
        .max(target.tasks.iter().map(|t| t.seq + 1).max().unwrap_or(1));
    source.tasks.retain(|t| t.seq != seq);
    target.tasks.push(Task {
        seq: new_seq,
        description: moved.description,
        status: moved.status,
        blocked_by: Vec::new(),
    });
    target.next_task = new_seq + 1;
    lines.push(format!(
        "task {} {} -> {} {}",
        from,
        label,
        to,
        task_label(new_seq)
    ));

    // Decisions that name the task move with it, and name it under its new number.
    let (named, kept): (Vec<Decision>, Vec<Decision>) = source
        .decisions
        .iter()
        .cloned()
        .partition(|d| d.task == Some(seq));
    let before: Vec<i64> = source.decisions.iter().map(|d| d.seq).collect();
    source.decisions = kept;
    for decision in named {
        let from_position = before.iter().position(|s| *s == decision.seq).unwrap_or(0) + 1;
        let new = next_decision(&target);
        target.decisions.push(Decision {
            seq: new,
            task: Some(new_seq),
            ..decision
        });
        lines.push(format!(
            "decision {} {} -> {} {}",
            from,
            from_position,
            to,
            target.decisions.len()
        ));
    }
    Ok(Moved {
        source,
        target,
        lines,
    })
}

fn next_decision(mission: &Mission) -> i64 {
    mission
        .decisions
        .iter()
        .map(|d| d.seq + 1)
        .max()
        .unwrap_or(1)
}

// --- Decisions ---

/// Move the decision at position `n` of `tsk decision list` of mission `from` to mission
/// `to`. It keeps its text, author, date and ADR. Its task link is cleared, because a task
/// belongs to the mission it was made in, unless `task` names a task of the target.
pub fn decision_mv(
    missions: &[Mission],
    from: &str,
    n: usize,
    to: &str,
    task: Option<&str>,
) -> Result<Moved, String> {
    let (source, target) = two(missions, from, to)?;
    let (mut source, mut target) = (source.clone(), target.clone());
    let mut ordered: Vec<&Decision> = source.decisions.iter().collect();
    ordered.sort_by_key(|d| d.seq);
    let found = n
        .checked_sub(1)
        .and_then(|at| ordered.get(at))
        .map(|d| (*d).clone())
        .ok_or_else(|| {
            format!(
                "error: {} has {} decisions, and no decision {}; a decision is numbered by its place in tsk decision list {}, from 1",
                from,
                source.decisions.len(),
                n,
                from
            )
        })?;
    let link = match task {
        Some(label) => {
            let seq = parse_task(label)?;
            task_ref(&target, seq)?;
            Some(seq)
        }
        None => None,
    };
    source.decisions.retain(|d| d.seq != found.seq);
    let new = next_decision(&target);
    target.decisions.push(Decision {
        seq: new,
        task: link,
        ..found.clone()
    });
    let mut lines = vec![format!(
        "decision {} {} -> {} {}",
        from,
        n,
        to,
        target.decisions.len()
    )];
    match (found.task, link) {
        (Some(was), None) => lines.push(format!(
            "task link cleared (was {} of {})",
            task_label(was),
            from
        )),
        (_, Some(seq)) => lines.push(format!("task link {} of {}", task_label(seq), to)),
        (None, None) => {}
    }
    Ok(Moved {
        source,
        target,
        lines,
    })
}

// --- Questions ---

/// Move a question from mission `from` to mission `to`. It keeps its text, status,
/// answer and dates.
pub fn question_mv(
    missions: &[Mission],
    from: &str,
    question: &str,
    to: &str,
) -> Result<Moved, String> {
    let (source, target) = two(missions, from, to)?;
    let (mut source, mut target) = (source.clone(), target.clone());
    let seq = parse_question(question)?;
    let found: Question = source
        .questions
        .iter()
        .find(|q| q.seq == seq)
        .cloned()
        .ok_or_else(|| format!("error: {} has no question {}", from, question_label(seq)))?;
    source.questions.retain(|q| q.seq != seq);
    let new_seq = target.next_question.max(
        target
            .questions
            .iter()
            .map(|q| q.seq + 1)
            .max()
            .unwrap_or(1),
    );
    target.questions.push(Question {
        seq: new_seq,
        ..found
    });
    target.next_question = new_seq + 1;
    let lines = vec![format!(
        "question {} {} -> {} {}",
        from,
        question_label(seq),
        to,
        question_label(new_seq)
    )];
    Ok(Moved {
        source,
        target,
        lines,
    })
}

/// Save the two missions of a move in one local change, and print its lines.
pub fn save_moved(store: &dyn LedgerStore, moved: Moved) -> Result<(), String> {
    store.missions_save(&[moved.source, moved.target])?;
    for line in moved.lines {
        println!("{}", line);
    }
    Ok(())
}

// --- Missions ---

/// What a rename does, before it does it.
pub struct Rename {
    /// The documents to rename, as `(old path, new path)`.
    pub docs: Vec<(String, String)>,
    /// One line per change, then one per record whose text names the old ID.
    pub lines: Vec<String>,
}

/// Whether a path component belongs to the mission with the code `code`: it is the code, or
/// starts with it and a `-` or `.` (`M-X.md`, `M-X-slug.md`, a directory `M-X`). A component
/// that starts with the code of another mission that is longer belongs to that one.
fn names_mission(component: &str, code: &str, missions: &[Mission]) -> bool {
    let after = |component: &str, code: &str| -> bool {
        component == code
            || component
                .strip_prefix(code)
                .is_some_and(|rest| rest.starts_with('-') || rest.starts_with('.'))
    };
    after(component, code)
        && !missions
            .iter()
            .any(|m| m.id.len() > code.len() && after(component, &m.id))
}

/// The path with `from` replaced by `to` in each component that names the mission, or
/// `None` when no component does.
pub fn renamed_path(path: &str, from: &str, to: &str, missions: &[Mission]) -> Option<String> {
    let mut changed = false;
    let parts: Vec<String> = path
        .split('/')
        .map(|component| {
            if names_mission(component, from, missions) {
                changed = true;
                format!("{}{}", to, &component[from.len()..])
            } else {
                component.to_string()
            }
        })
        .collect();
    changed.then(|| parts.join("/"))
}

/// Whether `text` holds `code` as a whole word: not part of a longer ID.
fn mentions_code(text: &str, code: &str) -> bool {
    let id_char = |c: char| c.is_ascii_alphanumeric() || c == '-';
    text.match_indices(code).any(|(at, _)| {
        let before = text[..at].chars().next_back();
        let after = text[at + code.len()..].chars().next();
        !before.is_some_and(id_char) && !after.is_some_and(id_char)
    })
}

/// One line for each record, of any mission, whose text names the old ID. They are not
/// changed: a person decides what a mention means.
fn mention_lines(missions: &[Mission], code: &str) -> Vec<String> {
    let mut lines = Vec::new();
    for mission in missions {
        let id = &mission.id;
        let mut note = |what: String, text: &str| {
            if mentions_code(text, code) {
                lines.push(format!("mentions {} {}", id, what));
            }
        };
        note("name".to_string(), &mission.name);
        note("purpose".to_string(), &mission.purpose);
        for o in &mission.objective {
            note(format!("objective {}", o.seq), &o.outcome);
        }
        for c in &mission.constraints {
            note(format!("constraint {}", c.seq), &c.text);
        }
        for o in &mission.out_of_scope {
            note(format!("out-of-scope {}", o.seq), &o.text);
        }
        for t in &mission.tasks {
            note(task_label(t.seq), &t.description);
        }
        for (at, d) in mission.decisions.iter().enumerate() {
            note(format!("decision {}", at + 1), &d.text);
        }
        for q in &mission.questions {
            note(question_label(q.seq), &q.text);
        }
    }
    lines
}

/// Plan the rename of the mission with the code `from` to `to`. `exists` says whether a
/// document is on the documents branch. Fails when `to` is not a mission ID, is the ID of
/// a mission, or is the path of a document that a rename would overwrite.
pub fn plan_rename(
    missions: &[Mission],
    from: &str,
    to: &str,
    exists: &dyn Fn(&str) -> bool,
) -> Result<Rename, String> {
    let mission = model::find(missions, from)?;
    model::check_id(to)?;
    if from == to {
        return Err(format!("error: {} already has the ID {}", from, to));
    }
    if missions.iter().any(|m| m.id.eq_ignore_ascii_case(to)) {
        return Err(format!("error: mission {} already exists", to));
    }
    let mut docs = Vec::new();
    for document in &mission.documents {
        if let Some(new) = renamed_path(&document.path, from, to, missions) {
            if exists(&new) {
                return Err(format!(
                    "error: {} would be renamed to {}, which exists; move that document away first",
                    document.path, new
                ));
            }
            if exists(&document.path) {
                docs.push((document.path.clone(), new));
            }
        }
    }
    let mut lines = vec![format!("mission {} -> {}", from, to)];
    for (old, new) in &docs {
        lines.push(format!("doc {} -> {}", old, new));
    }
    lines.extend(mention_lines(missions, from));
    Ok(Rename { docs, lines })
}

/// Rename a mission and the documents named for it, and print one line per change. With
/// `dry_run` it prints the same lines and changes nothing.
pub fn mission_mv(
    store: &dyn LedgerStore,
    from: &str,
    to: &str,
    dry_run: bool,
) -> Result<(), String> {
    let missions = store.missions_load()?;
    let mut plan = plan_rename(&missions, from, to, &|path| {
        store.doc_exists(path).unwrap_or(false)
    })?;
    let threads = threads_to_follow(store, &plan.docs)?;
    // The thread lines come after the document lines and before the mentions.
    let at = 1 + plan.docs.len();
    for (offset, (id, old, new)) in threads.iter().enumerate() {
        plan.lines
            .insert(at + offset, format!("thread {} {} -> {}", id, old, new));
    }
    if dry_run {
        for line in &plan.lines {
            println!("{}", line);
        }
        eprintln!("note: dry run, nothing changed");
        return Ok(());
    }
    // The documents first: a rename of the code that follows needs the paths in place.
    for (old, new) in &plan.docs {
        move_document(store, old, new)?;
    }
    store.mission_rename(from, to)?;
    // A thread that stores the path of its briefing, and not the internal ID of a mission,
    // reads its mission from the file name. It now names the mission by the internal ID.
    for (id, _, new) in &threads {
        store.thread_set_mission(id, Some(to), Some(new))?;
    }
    if !plan.docs.is_empty() {
        let mut renamed = store.missions_load()?;
        if let Some(mission) = renamed.iter_mut().find(|m| m.id == to) {
            for document in &mut mission.documents {
                if let Some((_, new)) = plan.docs.iter().find(|(old, _)| *old == document.path) {
                    document.path = new.clone();
                }
            }
            document_sort(mission);
            store.mission_save(mission)?;
        }
    }
    for line in &plan.lines {
        println!("{}", line);
    }
    Ok(())
}

/// The threads whose briefing is a document the rename moves, as `(thread, old path, new
/// path)`. A thread made before layout 5, or from a briefing document, stores that path.
/// Its mission ID is the one in the file name, so it shows the old ID after a rename.
fn threads_to_follow(
    store: &dyn LedgerStore,
    docs: &[(String, String)],
) -> Result<Vec<(String, String, String)>, String> {
    let mut found = Vec::new();
    for id in store.thread_ids()? {
        let Some(path) = store.thread_info(&id)?.briefing_path else {
            continue;
        };
        if let Some((_, new)) = docs.iter().find(|(old, _)| *old == path) {
            found.push((id, path, new.clone()));
        }
    }
    Ok(found)
}

fn document_sort(mission: &mut Mission) {
    mission.documents.sort_by(|a, b| a.path.cmp(&b.path));
}

// --- Documents ---

/// Copy a document to a new path and remove the old one.
fn move_document(store: &dyn LedgerStore, from: &str, to: &str) -> Result<(), String> {
    let body = store
        .doc_get(from)?
        .ok_or_else(|| format!("error: no such document: {}", from))?;
    store.doc_put(to, &body)?;
    store.doc_remove(from)?;
    Ok(())
}

/// Rename a document, and update the link of every mission that names its old path. It
/// prints `doc FROM -> TO` and one `link` line for each mission. Fails for a document that
/// does not exist and for a target that does.
pub fn doc_mv(store: &dyn LedgerStore, from: &str, to: &str) -> Result<(), String> {
    let from = check_docs_path(from).unwrap_or_else(|_| from.to_string());
    let to = check_docs_path(to)?;
    if from == to {
        return Err(format!("error: {} is already at {}", from, to));
    }
    if !store.doc_exists(&from)? {
        return Err(format!("error: no such document: {}", from));
    }
    if store.doc_exists(&to)? {
        return Err(format!("error: {} exists; move or remove it first", to));
    }
    let missions = store.missions_load()?;
    let mut linked: Vec<Mission> = Vec::new();
    for mission in &missions {
        if mission.documents.iter().any(|d| d.path == from) {
            let mut updated = mission.clone();
            for document in &mut updated.documents {
                if document.path == from {
                    document.path = to.clone();
                }
            }
            document_sort(&mut updated);
            linked.push(updated);
        }
    }
    move_document(store, &from, &to)?;
    if !linked.is_empty() {
        store.missions_save(&linked)?;
    }
    println!("doc {} -> {}", from, to);
    for mission in &linked {
        let role = mission
            .documents
            .iter()
            .find(|d| d.path == to)
            .map_or("", |d| d.role.as_str());
        println!("link {} {}", mission.id, role);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mission::model::{Category, Status};
    use crate::mission::records;

    fn mission(id: &str) -> Mission {
        model::create(&[], id, "A mission", Category::Operational, None).unwrap()
    }

    fn with(missions: &[Mission], changed: Mission) -> Vec<Mission> {
        let mut all: Vec<Mission> = missions
            .iter()
            .filter(|m| m.id != changed.id)
            .cloned()
            .collect();
        all.push(changed);
        all.sort_by(|a, b| a.id.cmp(&b.id));
        all
    }

    /// M-A with three tasks (T-02 blocked by T-01), a decision on T-01, a decision on no
    /// task, a decision on T-03 and two questions, and an empty M-B.
    fn world() -> Vec<Mission> {
        let mut all = vec![mission("M-A"), mission("M-B")];
        for text in ["one", "two", "three"] {
            all = with(&all, records::task_add(&all, "M-A", text).unwrap());
        }
        all = with(
            &all,
            records::task_block(&all, "M-A", "T-02", "T-01").unwrap(),
        );
        for (text, task) in [
            ("on one", Some("T-01")),
            ("on none", None),
            ("on three", Some("T-03")),
        ] {
            all = with(
                &all,
                records::decision_add(&all, "M-A", text, "Jim", task, None, "2026-10-24").unwrap(),
            );
        }
        for text in ["Why?", "How?"] {
            all = with(
                &all,
                records::question_add(&all, "M-A", text, "2026-10-24").unwrap(),
            );
        }
        all
    }

    #[test]
    fn a_task_with_blocks_is_refused_and_names_them() {
        let err = task_mv(&world(), "M-A", "T-01", "M-B", false)
            .err()
            .unwrap();
        assert!(err.contains("T-02 is blocked by T-01"), "{}", err);
        assert!(err.contains("--force"), "{}", err);
        let err = task_mv(&world(), "M-A", "T-02", "M-B", false)
            .err()
            .unwrap();
        assert!(err.contains("T-02 is blocked by T-01"), "{}", err);
    }

    #[test]
    fn a_task_moves_with_the_decisions_that_name_it_and_takes_the_next_number() {
        let moved = task_mv(&world(), "M-A", "T-03", "M-B", false).unwrap();
        assert_eq!(
            moved.lines,
            vec![
                "task M-A T-03 -> M-B T-01".to_string(),
                "decision M-A 3 -> M-B 1".to_string()
            ]
        );
        assert_eq!(moved.source.tasks.len(), 2);
        assert_eq!(moved.source.decisions.len(), 2);
        // The first mission does not reuse the number.
        assert_eq!(moved.source.next_task, 4);
        assert_eq!(moved.target.tasks[0].description, "three");
        assert_eq!(moved.target.tasks[0].seq, 1);
        assert_eq!(moved.target.next_task, 2);
        assert_eq!(moved.target.decisions[0].task, Some(1));
        assert_eq!(moved.target.decisions[0].text, "on three");
        assert_eq!(moved.target.decisions[0].decided_by, "Jim");
    }

    #[test]
    fn a_force_drops_the_blocks_and_says_so() {
        let moved = task_mv(&world(), "M-A", "T-01", "M-B", true).unwrap();
        assert_eq!(
            moved.lines,
            vec![
                "dropped block in M-A: T-02 is blocked by T-01".to_string(),
                "task M-A T-01 -> M-B T-01".to_string(),
                "decision M-A 1 -> M-B 1".to_string()
            ]
        );
        assert!(moved.source.tasks.iter().all(|t| t.blocked_by.is_empty()));
        assert_eq!(moved.source.tasks.len(), 2);
    }

    #[test]
    fn a_move_keeps_the_status_and_counts_on_from_the_target() {
        let mut all = world();
        all = with(
            &all,
            records::task_set(&all, "M-A", "T-03", "status", "done").unwrap(),
        );
        let moved = task_mv(&all, "M-A", "T-3", "M-B", false).unwrap();
        assert_eq!(moved.target.tasks[0].status, Status::Done);
        let all = with(&all, moved.target);
        let again = task_mv(&all, "M-A", "T-01", "M-B", true).unwrap();
        assert_eq!(again.target.tasks[1].seq, 2);
    }

    #[test]
    fn a_decision_moves_by_its_place_in_the_list_and_loses_its_task_link() {
        let moved = decision_mv(&world(), "M-A", 1, "M-B", None).unwrap();
        assert_eq!(
            moved.lines,
            vec![
                "decision M-A 1 -> M-B 1".to_string(),
                "task link cleared (was T-01 of M-A)".to_string()
            ]
        );
        assert_eq!(moved.source.decisions.len(), 2);
        assert_eq!(moved.target.decisions[0].task, None);
        assert_eq!(moved.target.decisions[0].text, "on one");
        let none = decision_mv(&world(), "M-A", 2, "M-B", None).unwrap();
        assert_eq!(none.lines, vec!["decision M-A 2 -> M-B 1".to_string()]);
        for bad in [0, 4] {
            let err = decision_mv(&world(), "M-A", bad, "M-B", None)
                .err()
                .unwrap();
            assert!(err.contains("has 3 decisions"), "{}", err);
        }
    }

    #[test]
    fn a_decision_can_name_a_task_of_the_target() {
        let all = with(
            &world(),
            records::task_add(&world(), "M-B", "target task").unwrap(),
        );
        let moved = decision_mv(&all, "M-A", 1, "M-B", Some("T-01")).unwrap();
        assert_eq!(moved.target.decisions[0].task, Some(1));
        assert!(moved.lines[1].contains("task link T-01 of M-B"));
        let err = decision_mv(&all, "M-A", 1, "M-B", Some("T-09"))
            .err()
            .unwrap();
        assert!(err.contains("no task T-09"), "{}", err);
    }

    #[test]
    fn a_question_keeps_its_answer_and_takes_the_next_number() {
        let mut all = world();
        all = with(
            &all,
            records::question_answer(&all, "M-A", "Q-02", "Because", "Jim", "2026-10-24").unwrap(),
        );
        let moved = question_mv(&all, "M-A", "Q-02", "M-B").unwrap();
        assert_eq!(
            moved.lines,
            vec!["question M-A Q-02 -> M-B Q-01".to_string()]
        );
        assert_eq!(moved.source.questions.len(), 1);
        assert_eq!(moved.target.questions[0].answer.as_deref(), Some("Because"));
        assert_eq!(moved.target.next_question, 2);
        let err = question_mv(&all, "M-A", "Q-07", "M-B").err().unwrap();
        assert!(err.contains("no question Q-07"), "{}", err);
    }

    #[test]
    fn a_move_to_the_same_or_an_unknown_mission_is_refused() {
        assert!(task_mv(&world(), "M-A", "T-01", "M-A", true)
            .err()
            .unwrap()
            .contains("both the mission"));
        assert!(question_mv(&world(), "M-A", "Q-01", "M-NONE")
            .err()
            .unwrap()
            .contains("no such mission: M-NONE"));
    }

    #[test]
    fn a_component_names_a_mission_by_its_code_and_a_separator() {
        let all = vec![mission("M-BOOT"), mission("M-BOOT-06")];
        let path = |p: &str, code: &str, to: &str| renamed_path(p, code, to, &all);
        assert_eq!(
            path("missions/operational/M-BOOT.md", "M-BOOT", "M-NEW").as_deref(),
            Some("missions/operational/M-NEW.md")
        );
        assert_eq!(
            path("missions/M-BOOT-intelligence.md", "M-BOOT", "M-NEW").as_deref(),
            Some("missions/M-NEW-intelligence.md")
        );
        assert_eq!(
            path("missions/M-BOOT/notes.md", "M-BOOT", "M-NEW").as_deref(),
            Some("missions/M-NEW/notes.md")
        );
        // A document of the mission with the longer code is not named for this one.
        assert_eq!(
            path("missions/M-BOOT-06-control.md", "M-BOOT", "M-NEW"),
            None
        );
        assert_eq!(
            path("missions/M-BOOT-06-control.md", "M-BOOT-06", "M-NEW").as_deref(),
            Some("missions/M-NEW-control.md")
        );
        assert_eq!(path("missions/M-BOOTS.md", "M-BOOT", "M-NEW"), None);
        assert_eq!(path("notes/design.md", "M-BOOT", "M-NEW"), None);
    }

    #[test]
    fn a_rename_names_the_documents_and_the_records_that_mention_the_old_id() {
        let mut all = world();
        let mut a = all[0].clone();
        a.documents.push(model::Document {
            path: "missions/M-A-notes.md".to_string(),
            role: model::Role::Intelligence,
        });
        a.documents.push(model::Document {
            path: "other/design.md".to_string(),
            role: model::Role::Design,
        });
        a.purpose = "Work that M-A does for M-A-2".to_string();
        all = with(&all, a);
        let b = records::task_add(&all, "M-B", "Follow M-A, then stop").unwrap();
        all = with(&all, b);
        let exists = |p: &str| p == "missions/M-A-notes.md" || p == "other/design.md";
        let plan = plan_rename(&all, "M-A", "M-C", &exists).unwrap();
        assert_eq!(
            plan.docs,
            vec![(
                "missions/M-A-notes.md".to_string(),
                "missions/M-C-notes.md".to_string()
            )]
        );
        assert_eq!(
            plan.lines,
            vec![
                "mission M-A -> M-C".to_string(),
                "doc missions/M-A-notes.md -> missions/M-C-notes.md".to_string(),
                // M-A-2 is another ID, so only the first of the two in the purpose counts.
                "mentions M-A purpose".to_string(),
                "mentions M-B T-01".to_string(),
            ]
        );
    }

    #[test]
    fn a_rename_refuses_a_bad_or_taken_id_and_a_path_that_exists() {
        let all = world();
        let none = |_: &str| false;
        assert!(plan_rename(&all, "M-A", "M-B", &none)
            .err()
            .unwrap()
            .contains("already exists"));
        assert!(plan_rename(&all, "M-A", "m-c", &none)
            .err()
            .unwrap()
            .contains("is not a mission ID"));
        assert!(plan_rename(&all, "M-A", "M-A", &none)
            .err()
            .unwrap()
            .contains("already has the ID"));
        assert!(plan_rename(&all, "M-NONE", "M-C", &none)
            .err()
            .unwrap()
            .contains("no such mission"));
        let mut a = all[0].clone();
        a.documents.push(model::Document {
            path: "m/M-A.md".to_string(),
            role: model::Role::Report,
        });
        let all = with(&all, a);
        let both = |p: &str| p == "m/M-A.md" || p == "m/M-C.md";
        assert!(plan_rename(&all, "M-A", "M-C", &both)
            .err()
            .unwrap()
            .contains("m/M-C.md, which exists"));
    }
}
