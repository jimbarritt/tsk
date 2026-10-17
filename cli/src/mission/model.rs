//! The mission record and every rule that applies to it. The rules work on plain Rust
//! values, so both ledger backends share them: a backend only loads and saves
//! [`Mission`] values. Derived state (blocked) and all output are computed here.

use serde::{Deserialize, Serialize};

pub const FIELDS: &[&str] = &[
    "name",
    "status",
    "purpose",
    "constraints",
    "out-of-scope",
    "decision-authority",
];

const MAX_NAME_CHARS: usize = 255;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    Operational,
    Administrative,
}

impl Category {
    pub fn as_str(self) -> &'static str {
        match self {
            Category::Operational => "operational",
            Category::Administrative => "administrative",
        }
    }

    pub fn parse(value: &str) -> Result<Category, String> {
        match value {
            "operational" => Ok(Category::Operational),
            "administrative" => Ok(Category::Administrative),
            other => Err(format!(
                "error: '{}' is not a category; the categories are operational and administrative",
                other
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Status {
    #[serde(rename = "todo")]
    Todo,
    #[serde(rename = "in-progress")]
    InProgress,
    #[serde(rename = "done")]
    Done,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Todo => "todo",
            Status::InProgress => "in-progress",
            Status::Done => "done",
        }
    }

    pub fn parse(value: &str) -> Result<Status, String> {
        match value {
            "todo" => Ok(Status::Todo),
            "in-progress" => Ok(Status::InProgress),
            "done" => Ok(Status::Done),
            other => Err(format!(
                "error: '{}' is not a status; the statuses are todo, in-progress and done",
                other
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Intelligence,
    Report,
    Design,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Role::Intelligence => "intelligence",
            Role::Report => "report",
            Role::Design => "design",
        }
    }

    pub fn parse(value: &str) -> Result<Role, String> {
        match value {
            "intelligence" => Ok(Role::Intelligence),
            "report" => Ok(Role::Report),
            "design" => Ok(Role::Design),
            other => Err(format!(
                "error: '{}' is not a role; the roles are intelligence, report and design",
                other
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Outcome {
    pub seq: i64,
    pub outcome: String,
    pub met: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Document {
    pub path: String,
    pub role: Role,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Task {
    pub seq: i64,
    pub description: String,
    pub status: Status,
    /// The sequence numbers of the tasks of this mission this one waits for, sorted.
    /// Whether one of them still blocks is derived, never stored.
    pub blocked_by: Vec<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Decision {
    pub seq: i64,
    pub text: String,
    pub decided_by: String,
    /// `YYYY-MM-DD`, UTC.
    pub decided_on: String,
    pub task: Option<i64>,
    /// The path of an ADR in the code repository, relative to its root. A file written
    /// before this field existed reads as none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adr: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QuestionStatus {
    Open,
    Answered,
}

impl QuestionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            QuestionStatus::Open => "open",
            QuestionStatus::Answered => "answered",
        }
    }

    pub fn parse(value: &str) -> Option<QuestionStatus> {
        match value {
            "open" => Some(QuestionStatus::Open),
            "answered" => Some(QuestionStatus::Answered),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Question {
    pub seq: i64,
    pub text: String,
    pub status: QuestionStatus,
    pub answer: Option<String>,
    pub answered_by: Option<String>,
    pub answered_on: Option<String>,
    pub asked_on: String,
}

fn one() -> i64 {
    1
}

/// One mission. The field order is the key order of the file ledger's JSON file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mission {
    pub id: String,
    pub name: String,
    pub category: Category,
    pub status: Status,
    pub parent: Option<String>,
    pub purpose: String,
    pub decision_authority: String,
    pub constraints: String,
    pub out_of_scope: String,
    /// The number the next objective outcome gets. It only grows.
    pub next_objective: i64,
    pub objective: Vec<Outcome>,
    /// The IDs of the missions this one waits for, sorted. Whether one of them still
    /// blocks is derived, never stored.
    pub blocked_by: Vec<String>,
    pub documents: Vec<Document>,
    /// The number the next task gets. It only grows. A file written before tasks existed
    /// reads as 1.
    #[serde(default = "one")]
    pub next_task: i64,
    /// The number the next question gets. It only grows.
    #[serde(default = "one")]
    pub next_question: i64,
    #[serde(default)]
    pub tasks: Vec<Task>,
    #[serde(default)]
    pub decisions: Vec<Decision>,
    #[serde(default)]
    pub questions: Vec<Question>,
}

// --- Validation ---

/// `^M-[A-Z0-9]+(-[A-Z0-9]+)*$`
pub fn valid_id(id: &str) -> bool {
    let Some(rest) = id.strip_prefix("M-") else {
        return false;
    };
    !rest.is_empty()
        && rest.split('-').all(|part| {
            !part.is_empty()
                && part
                    .chars()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
        })
}

fn check_id(id: &str) -> Result<(), String> {
    if valid_id(id) && id.len() <= 64 {
        Ok(())
    } else {
        Err(format!(
            "error: '{}' is not a mission ID; a mission ID looks like M-TSK-SCHEMA (capitals and digits in groups after M-, at most 64 characters)",
            id
        ))
    }
}

fn check_name(name: &str) -> Result<(), String> {
    if name.trim().is_empty() {
        return Err("error: a mission name cannot be empty".to_string());
    }
    if name.chars().any(char::is_control) {
        return Err("error: a mission name is one line, with no tabs or line breaks".to_string());
    }
    if name.chars().count() > MAX_NAME_CHARS {
        return Err(format!(
            "error: a mission name is at most {} characters",
            MAX_NAME_CHARS
        ));
    }
    Ok(())
}

pub fn find<'a>(missions: &'a [Mission], id: &str) -> Result<&'a Mission, String> {
    missions
        .iter()
        .find(|m| m.id == id)
        .ok_or_else(|| no_such_mission(id))
}

pub fn no_such_mission(id: &str) -> String {
    format!("error: no such mission: {}", id)
}

/// Strip one trailing line break, which a value read from stdin carries.
pub fn strip_line_break(value: &str) -> &str {
    let value = value.strip_suffix('\n').unwrap_or(value);
    value.strip_suffix('\r').unwrap_or(value)
}

// --- Operations. Each takes the whole mission set and returns the missions to save. ---

pub fn create(
    missions: &[Mission],
    id: &str,
    name: &str,
    category: Category,
    parent: Option<&str>,
) -> Result<Mission, String> {
    check_id(id)?;
    check_name(name)?;
    if missions.iter().any(|m| m.id == id) {
        return Err(format!("error: mission {} already exists", id));
    }
    if let Some(parent) = parent {
        if !missions.iter().any(|m| m.id == parent) {
            return Err(format!("error: no such parent mission: {}", parent));
        }
    }
    Ok(Mission {
        id: id.to_string(),
        name: name.to_string(),
        category,
        // An administrative mission has no end, so it starts in progress.
        status: match category {
            Category::Operational => Status::Todo,
            Category::Administrative => Status::InProgress,
        },
        parent: parent.map(str::to_string),
        purpose: String::new(),
        decision_authority: String::new(),
        constraints: String::new(),
        out_of_scope: String::new(),
        next_objective: 1,
        objective: Vec::new(),
        blocked_by: Vec::new(),
        documents: Vec::new(),
        next_task: 1,
        next_question: 1,
        tasks: Vec::new(),
        decisions: Vec::new(),
        questions: Vec::new(),
    })
}

pub fn set(missions: &[Mission], id: &str, field: &str, value: &str) -> Result<Mission, String> {
    let mut mission = find(missions, id)?.clone();
    match field {
        "name" => {
            check_name(value)?;
            mission.name = value.to_string();
        }
        "status" => {
            let status = Status::parse(value)?;
            if mission.category == Category::Administrative && status != Status::InProgress {
                return Err(format!(
                    "error: {} is an administrative mission and has no end; its status stays in-progress",
                    id
                ));
            }
            mission.status = status;
        }
        "purpose" => mission.purpose = value.to_string(),
        "constraints" => mission.constraints = value.to_string(),
        "out-of-scope" => mission.out_of_scope = value.to_string(),
        "decision-authority" => mission.decision_authority = value.to_string(),
        other => {
            return Err(format!(
                "error: '{}' is not a mission field; the fields are {}",
                other,
                FIELDS.join(", ")
            ))
        }
    }
    Ok(mission)
}

pub fn block(missions: &[Mission], id: &str, by: &str) -> Result<Mission, String> {
    let mut mission = find(missions, id)?.clone();
    find(missions, by)?;
    if id == by {
        return Err(format!("error: {} cannot block itself", id));
    }
    if mission.blocked_by.iter().any(|b| b == by) {
        return Err(format!("error: {} is already blocked by {}", id, by));
    }
    // Adding id -> by closes a cycle when id is already reachable from by.
    if let Some(chain) = path_between(missions, by, id) {
        let mut shown = vec![id.to_string()];
        shown.extend(chain);
        return Err(format!(
            "error: {} cannot be blocked by {}: that makes a cycle ({})",
            id,
            by,
            shown.join(" -> ")
        ));
    }
    mission.blocked_by.push(by.to_string());
    mission.blocked_by.sort();
    Ok(mission)
}

/// The chain of blockers from `from` down to `to`, both included, or `None`.
fn path_between(missions: &[Mission], from: &str, to: &str) -> Option<Vec<String>> {
    fn walk(
        missions: &[Mission],
        at: &str,
        to: &str,
        seen: &mut Vec<String>,
        trail: &mut Vec<String>,
    ) -> bool {
        trail.push(at.to_string());
        if at == to {
            return true;
        }
        if !seen.iter().any(|s| s == at) {
            seen.push(at.to_string());
            if let Some(mission) = missions.iter().find(|m| m.id == at) {
                for next in &mission.blocked_by {
                    if walk(missions, next, to, seen, trail) {
                        return true;
                    }
                }
            }
        }
        trail.pop();
        false
    }
    let mut trail = Vec::new();
    walk(missions, from, to, &mut Vec::new(), &mut trail).then_some(trail)
}

pub fn unblock(missions: &[Mission], id: &str, by: &str) -> Result<Mission, String> {
    let mut mission = find(missions, id)?.clone();
    if !mission.blocked_by.iter().any(|b| b == by) {
        return Err(format!("error: {} is not blocked by {}", id, by));
    }
    mission.blocked_by.retain(|b| b != by);
    Ok(mission)
}

pub fn objective_add(missions: &[Mission], id: &str, outcome: &str) -> Result<Mission, String> {
    let mut mission = find(missions, id)?.clone();
    if outcome.trim().is_empty() {
        return Err("error: an objective outcome cannot be empty".to_string());
    }
    if outcome.contains(['\n', '\r']) {
        return Err("error: an objective outcome is one line".to_string());
    }
    let seq = mission.next_objective.max(
        mission
            .objective
            .iter()
            .map(|o| o.seq + 1)
            .max()
            .unwrap_or(1),
    );
    mission.objective.push(Outcome {
        seq,
        outcome: outcome.to_string(),
        met: false,
    });
    mission.next_objective = seq + 1;
    Ok(mission)
}

fn outcome_mut(mission: &mut Mission, n: i64) -> Result<&mut Outcome, String> {
    let id = mission.id.clone();
    mission
        .objective
        .iter_mut()
        .find(|o| o.seq == n)
        .ok_or_else(|| format!("error: {} has no objective outcome {}", id, n))
}

pub fn objective_met(missions: &[Mission], id: &str, n: i64, met: bool) -> Result<Mission, String> {
    let mut mission = find(missions, id)?.clone();
    outcome_mut(&mut mission, n)?.met = met;
    Ok(mission)
}

pub fn objective_remove(missions: &[Mission], id: &str, n: i64) -> Result<Mission, String> {
    let mut mission = find(missions, id)?.clone();
    outcome_mut(&mut mission, n)?;
    mission.objective.retain(|o| o.seq != n);
    Ok(mission)
}

pub fn link(
    missions: &[Mission],
    id: &str,
    path: &str,
    role: &str,
    exists: bool,
) -> Result<Mission, String> {
    let mut mission = find(missions, id)?.clone();
    let role = Role::parse(role)?;
    if !exists {
        return Err(format!("error: no such document: {}", path));
    }
    match mission.documents.iter_mut().find(|d| d.path == path) {
        Some(existing) => existing.role = role,
        None => mission.documents.push(Document {
            path: path.to_string(),
            role,
        }),
    }
    mission.documents.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(mission)
}

pub fn unlink(missions: &[Mission], id: &str, path: &str) -> Result<Mission, String> {
    let mut mission = find(missions, id)?.clone();
    if !mission.documents.iter().any(|d| d.path == path) {
        return Err(format!("error: {} does not link {}", id, path));
    }
    mission.documents.retain(|d| d.path != path);
    Ok(mission)
}

/// The first linked path that `exists` denies, as an error naming the mission and path.
/// `state` says what the push did and did not do before it stopped.
pub fn check_links(
    missions: &[Mission],
    exists: &dyn Fn(&str) -> bool,
    state: &str,
) -> Result<(), String> {
    for mission in missions {
        for doc in &mission.documents {
            if !exists(&doc.path) {
                return Err(format!(
                    "error: mission {} links the document {}, which is not on the tsk/docs branch.\n       \
                     Restore it with tsk ledger doc put, or run tsk mission unlink {} {}, then re-run tsk ledger push.\n       {}",
                    mission.id, doc.path, mission.id, doc.path, state
                ));
            }
        }
    }
    Ok(())
}

// --- Derived state and output ---

/// The IDs in `blocked_by` whose mission is not done. A blocker that is absent from the
/// set counts as still blocking, so a lost record never frees a mission.
pub fn blocking(missions: &[Mission], mission: &Mission) -> Vec<String> {
    mission
        .blocked_by
        .iter()
        .filter(|by| {
            missions
                .iter()
                .find(|m| &m.id == *by)
                .is_none_or(|m| m.status != Status::Done)
        })
        .cloned()
        .collect()
}

/// One `list` line: id, status, blocked-by, parent, name, tab-separated.
pub fn list_line(missions: &[Mission], mission: &Mission) -> String {
    let blockers = blocking(missions, mission);
    format!(
        "{}\t{}\t{}\t{}\t{}",
        mission.id,
        mission.status.as_str(),
        if blockers.is_empty() {
            "-".to_string()
        } else {
            blockers.join(",")
        },
        mission.parent.as_deref().unwrap_or("-"),
        mission.name
    )
}

/// One `objective list` line: n, met or open, outcome, tab-separated.
pub fn objective_line(outcome: &Outcome) -> String {
    format!(
        "{}\t{}\t{}",
        outcome.seq,
        if outcome.met { "met" } else { "open" },
        outcome.outcome
    )
}

#[derive(Serialize)]
pub struct JsonOutcome<'a> {
    n: i64,
    outcome: &'a str,
    met: bool,
}

#[derive(Serialize)]
pub struct JsonBlocker<'a> {
    id: &'a str,
    blocking: bool,
}

#[derive(Serialize)]
pub struct JsonDocument<'a> {
    path: &'a str,
    role: &'a str,
}

#[derive(Serialize)]
pub struct JsonMission<'a> {
    id: &'a str,
    name: &'a str,
    category: &'a str,
    status: &'a str,
    parent: Option<&'a str>,
    blocked: bool,
    purpose: &'a str,
    decision_authority: &'a str,
    constraints: &'a str,
    out_of_scope: &'a str,
    objective: Vec<JsonOutcome<'a>>,
    blocked_by: Vec<JsonBlocker<'a>>,
    documents: Vec<JsonDocument<'a>>,
    tasks: Vec<super::records::JsonTask<'a>>,
    decisions: Vec<super::records::JsonDecision<'a>>,
    questions: Vec<super::records::JsonQuestion<'a>>,
}

/// The full record for JSON output, with the derived `blocked` flag. The keys print in
/// the order of the struct.
pub fn to_json<'a>(missions: &[Mission], mission: &'a Mission) -> JsonMission<'a> {
    let blockers = blocking(missions, mission);
    JsonMission {
        id: &mission.id,
        name: &mission.name,
        category: mission.category.as_str(),
        status: mission.status.as_str(),
        parent: mission.parent.as_deref(),
        blocked: !blockers.is_empty(),
        purpose: &mission.purpose,
        decision_authority: &mission.decision_authority,
        constraints: &mission.constraints,
        out_of_scope: &mission.out_of_scope,
        objective: mission
            .objective
            .iter()
            .map(|o| JsonOutcome {
                n: o.seq,
                outcome: &o.outcome,
                met: o.met,
            })
            .collect(),
        blocked_by: mission
            .blocked_by
            .iter()
            .map(|id| JsonBlocker {
                id,
                blocking: blockers.contains(id),
            })
            .collect(),
        documents: mission
            .documents
            .iter()
            .map(|d| JsonDocument {
                path: &d.path,
                role: d.role.as_str(),
            })
            .collect(),
        tasks: super::records::json_tasks(mission),
        decisions: super::records::json_decisions(mission),
        questions: super::records::json_questions(mission),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk(missions: &mut Vec<Mission>, id: &str) {
        let m = create(missions, id, "Name", Category::Operational, None).unwrap();
        missions.push(m);
    }

    fn put(missions: &mut [Mission], updated: Mission) {
        let at = missions.iter().position(|m| m.id == updated.id).unwrap();
        missions[at] = updated;
    }

    #[test]
    fn ids_follow_the_pattern() {
        for ok in ["M-X", "M-TSK-SCHEMA", "M-A1-B2-03", "M-1"] {
            assert!(valid_id(ok), "{}", ok);
        }
        for bad in [
            "", "M-", "M", "m-x", "M-x", "M--X", "M-X-", "T-01", "M-X Y", "X-M", "M-É",
        ] {
            assert!(!valid_id(bad), "{}", bad);
        }
    }

    #[test]
    fn an_administrative_mission_starts_in_progress_and_keeps_it() {
        let mut all = Vec::new();
        let m = create(&all, "M-ADMIN", "Admin", Category::Administrative, None).unwrap();
        assert_eq!(m.status, Status::InProgress);
        all.push(m);
        for status in ["todo", "done"] {
            let err = set(&all, "M-ADMIN", "status", status).unwrap_err();
            assert!(err.contains("has no end"), "{}", err);
        }
        assert!(set(&all, "M-ADMIN", "status", "in-progress").is_ok());
        let err = set(&all, "M-ADMIN", "status", "blocked").unwrap_err();
        assert!(err.contains("todo, in-progress and done"), "{}", err);
    }

    #[test]
    fn a_block_refuses_self_unknown_and_cycles() {
        let mut all = Vec::new();
        for id in ["M-A", "M-B", "M-C"] {
            mk(&mut all, id);
        }
        assert!(block(&all, "M-A", "M-A").unwrap_err().contains("itself"));
        assert!(block(&all, "M-A", "M-Z").unwrap_err().contains("M-Z"));
        let a = block(&all, "M-A", "M-B").unwrap();
        put(&mut all, a);
        let b = block(&all, "M-B", "M-C").unwrap();
        put(&mut all, b);
        let err = block(&all, "M-C", "M-A").unwrap_err();
        assert!(err.contains("cycle (M-C -> M-A -> M-B -> M-C)"), "{}", err);
        assert!(block(&all, "M-A", "M-B")
            .unwrap_err()
            .contains("already blocked"));
        assert!(unblock(&all, "M-A", "M-C")
            .unwrap_err()
            .contains("not blocked"));
    }

    #[test]
    fn blocked_is_derived_from_blockers_that_are_not_done() {
        let mut all = Vec::new();
        mk(&mut all, "M-A");
        mk(&mut all, "M-B");
        let a = block(&all, "M-A", "M-B").unwrap();
        put(&mut all, a);
        let a = all[0].clone();
        assert_eq!(blocking(&all, &a), vec!["M-B".to_string()]);
        assert_eq!(list_line(&all, &a), "M-A\ttodo\tM-B\t-\tName".to_string());
        let b = set(&all, "M-B", "status", "done").unwrap();
        put(&mut all, b);
        assert!(blocking(&all, &a).is_empty());
        assert_eq!(list_line(&all, &a), "M-A\ttodo\t-\t-\tName");
    }

    #[test]
    fn objective_numbers_are_never_reused() {
        let mut all = Vec::new();
        mk(&mut all, "M-A");
        for text in ["one", "two", "three"] {
            let m = objective_add(&all, "M-A", text).unwrap();
            put(&mut all, m);
        }
        let m = objective_remove(&all, "M-A", 3).unwrap();
        put(&mut all, m);
        let m = objective_add(&all, "M-A", "four").unwrap();
        assert_eq!(m.objective.last().unwrap().seq, 4);
        put(&mut all, m);
        let m = objective_met(&all, "M-A", 2, true).unwrap();
        assert!(m.objective[1].met);
        assert!(objective_met(&all, "M-A", 3, true)
            .unwrap_err()
            .contains("no objective outcome 3"));
    }

    #[test]
    fn names_are_one_line() {
        assert!(check_name("ok name").is_ok());
        for bad in ["", "  ", "a\nb", "a\tb"] {
            assert!(check_name(bad).is_err(), "{:?}", bad);
        }
        assert!(check_name(&"x".repeat(256)).is_err());
        assert!(check_name(&"\u{e9}".repeat(255)).is_ok());
    }

    #[test]
    fn the_json_record_marks_the_blockers_that_still_block() {
        let mut all = Vec::new();
        for id in ["M-A", "M-B", "M-C"] {
            mk(&mut all, id);
        }
        let a = block(&all, "M-A", "M-B").unwrap();
        put(&mut all, a);
        let a = block(&all, "M-A", "M-C").unwrap();
        put(&mut all, a);
        let c = set(&all, "M-C", "status", "done").unwrap();
        put(&mut all, c);
        let json = serde_json::to_value(to_json(&all, &all[0])).unwrap();
        assert_eq!(json["blocked"], true);
        assert_eq!(json["blocked_by"][0]["id"], "M-B");
        assert_eq!(json["blocked_by"][0]["blocking"], true);
        assert_eq!(json["blocked_by"][1]["blocking"], false);
    }
}
