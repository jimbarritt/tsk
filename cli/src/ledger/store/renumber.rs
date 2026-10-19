//! Renumber at push. Rows of the record tables are keyed `(mission_id, seq)`, and two
//! clones that each add a task, question, outcome, constraint, out-of-scope item or
//! decision to one mission before they push both mint the same number. Dolt would stop the
//! second push with a conflict on the table.
//!
//! Before the merge of a push, [`prepare`] finds the keys the local unpushed history added
//! that the remote also added since the merge base, and gives each local row the next free
//! number. Rows that name a renumbered task follow it. The counters on the mission row are
//! the one cell both clones change to different values, so [`prepare`] also resets such a
//! local counter to its value at the merge base, which leaves the remote's value to win
//! the merge, and returns the statements that raise every counter above the merged rows
//! once the merge has run. Text that names an old number does not change: the lines the
//! caller prints are the record.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::ledger::dolt::{batching, integer, lit, text, Dolt};
use crate::mission::records::{question_label, task_label};

use super::dolt::plain;

/// A record table, the column of the mission row that counts its numbers (none for
/// decisions, which take the highest number plus one), and how a number reads in a line.
struct Kind {
    table: &'static str,
    counter: Option<&'static str>,
    label: fn(i64) -> String,
}

const KINDS: [Kind; 6] = [
    Kind {
        table: "tasks",
        counter: Some("next_task"),
        label: task_label,
    },
    Kind {
        table: "questions",
        counter: Some("next_question"),
        label: question_label,
    },
    Kind {
        table: "objective_outcomes",
        counter: Some("next_objective"),
        label: |n| format!("objective {}", n),
    },
    Kind {
        table: "mission_constraints",
        counter: Some("next_constraint"),
        label: |n| format!("constraint {}", n),
    },
    Kind {
        table: "mission_out_of_scope",
        counter: Some("next_out_of_scope"),
        label: |n| format!("out-of-scope {}", n),
    },
    Kind {
        table: "decisions",
        counter: None,
        label: |n| format!("decision {}", n),
    },
];

/// The message of the local commit that renumbers.
const MESSAGE: &str = "Renumber after a collision";
/// The message when only a counter needed to stay out of the merge.
const COUNTER_MESSAGE: &str = "Hold counters back for a merge";

/// One row that took a new number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Renumbered {
    kind: usize,
    mission: String,
    from: i64,
    to: i64,
}

impl Renumbered {
    /// `T-03 is now T-05 in M-X`.
    pub fn line(&self) -> String {
        let label = KINDS[self.kind].label;
        format!(
            "{} is now {} in {}",
            label(self.from),
            label(self.to),
            self.mission
        )
    }
}

/// Record a renumbering. A row that was renumbered earlier in this push (a merge, then a
/// rejected push, then a second merge) keeps one line, from its first number to its last.
fn record(changes: &mut Vec<Renumbered>, change: Renumbered) {
    match changes
        .iter_mut()
        .find(|c| c.kind == change.kind && c.mission == change.mission && c.to == change.from)
    {
        Some(earlier) => earlier.to = change.to,
        None => changes.push(change),
    }
}

/// What [`prepare`] did to the local branch.
pub struct Prepared {
    /// The commit `HEAD` was at before the renumbering commit.
    pub before: String,
    /// Statements to run after the merge and before it is committed.
    pub fixup_sql: String,
}

/// A counter cell that both clones moved to different values since the merge base.
struct CounterClash {
    mission: String,
    column: &'static str,
    base: i64,
}

type Key = (String, i64);
type Collisions = Vec<BTreeSet<Key>>;

/// Renumber the local rows that collide with rows the remote added, and reset the
/// counters that both sides moved. Returns `None` when there is nothing to do, and also
/// when the histories cannot be compared (no common ancestor, a table missing on one
/// side), which leaves the merge to behave as it always has. The renumbering is one local
/// commit.
pub fn prepare(
    db: &Dolt,
    remote_ref: &str,
    changes: &mut Vec<Renumbered>,
) -> Result<Option<Prepared>, String> {
    let Some((collisions, clashes)) = detect(db, remote_ref) else {
        return Ok(None);
    };
    if collisions.iter().all(|c| c.is_empty()) && clashes.is_empty() {
        return Ok(None);
    }
    let Some(plan) = plan(db, remote_ref, &collisions, &clashes) else {
        return Ok(None);
    };
    let before = db.head_hash()?;
    let applied = db.exec(&plan.sql).and_then(|_| {
        db.run(["add", "-A"])?;
        let message = if plan.renames.is_empty() {
            COUNTER_MESSAGE
        } else {
            MESSAGE
        };
        db.run(["commit", "-m", message]).map(|_| ())
    });
    if let Err(error) = applied {
        let _ = db.run(["reset", "--hard", before.as_str()]);
        return Err(error);
    }
    for rename in plan.renames {
        record(changes, rename);
    }
    Ok(Some(Prepared {
        before,
        fixup_sql: plan.fixup,
    }))
}

/// Put the local branch back where [`prepare`] found it, after a merge that failed.
pub fn undo(db: &Dolt, prepared: &Prepared) -> Result<(), String> {
    db.run(["reset", "--hard", prepared.before.as_str()])
        .map(|_| ())
}

/// The rows of each query: one process for all of them, or one process each when the
/// batched reads are off. `None` when any query fails.
fn each(db: &Dolt, queries: &[String]) -> Option<Vec<Vec<Value>>> {
    if batching() {
        db.rows_each(queries).ok()
    } else {
        queries.iter().map(|query| db.rows(query).ok()).collect()
    }
}

/// The keys both sides added, per record table, and the counter cells both sides moved.
/// `None` when a query fails.
fn detect(db: &Dolt, remote_ref: &str) -> Option<(Collisions, Vec<CounterClash>)> {
    let base = format!("dolt_merge_base('HEAD', {})", lit(remote_ref));
    let remote = lit(remote_ref);
    let mut queries = Vec::new();
    for kind in &KINDS {
        for to in ["'HEAD'", remote.as_str()] {
            queries.push(format!(
                "select to_mission_id as m, to_seq as s from dolt_diff({}, {}, '{}') \
                 where diff_type = 'added'",
                base, to, kind.table
            ));
        }
    }
    for to in ["'HEAD'", remote.as_str()] {
        queries.push(format!(
            "select * from dolt_diff({}, {}, 'missions')",
            base, to
        ));
    }
    let sets = each(db, &queries)?;
    let keys = |rows: &[Value]| -> BTreeSet<Key> {
        rows.iter()
            .filter_map(|row| Some((text(row, "m")?, integer(row, "s")?)))
            .collect()
    };
    let n = KINDS.len();
    let (local_missions, remote_missions) = (&sets[2 * n], &sets[2 * n + 1]);
    // A mission both sides created is a conflict of its own. Its rows are left to it.
    let created = |rows: &[Value]| -> BTreeSet<String> {
        rows.iter()
            .filter(|row| text(row, "diff_type").as_deref() == Some("added"))
            .filter_map(|row| text(row, "to_id"))
            .collect()
    };
    let both_created: BTreeSet<String> = created(local_missions)
        .intersection(&created(remote_missions))
        .cloned()
        .collect();
    let collisions: Collisions = (0..n)
        .map(|k| {
            keys(&sets[2 * k])
                .intersection(&keys(&sets[2 * k + 1]))
                .filter(|(mission, _)| !both_created.contains(mission))
                .cloned()
                .collect()
        })
        .collect();

    let moved = |rows: &[Value]| -> BTreeMap<String, Value> {
        rows.iter()
            .filter(|row| text(row, "diff_type").as_deref() == Some("modified"))
            .filter_map(|row| Some((text(row, "to_id")?, row.clone())))
            .collect()
    };
    let (local_moved, remote_moved) = (moved(local_missions), moved(remote_missions));
    let mut clashes = Vec::new();
    for (mission, ours) in &local_moved {
        let Some(theirs) = remote_moved.get(mission) else {
            continue;
        };
        for column in KINDS.iter().filter_map(|k| k.counter) {
            let cell = |row: &Value, side: &str| integer(row, &format!("{}_{}", side, column));
            let (Some(from), Some(to), Some(their_from), Some(their_to)) = (
                cell(ours, "from"),
                cell(ours, "to"),
                cell(theirs, "from"),
                cell(theirs, "to"),
            ) else {
                continue;
            };
            if from != to && their_from != their_to && to != their_to {
                clashes.push(CounterClash {
                    mission: mission.clone(),
                    column,
                    base: from,
                });
            }
        }
    }
    Some((collisions, clashes))
}

/// What to run: the statements before the merge, the statements after it, and the lines.
struct Plan {
    sql: String,
    fixup: String,
    renames: Vec<Renumbered>,
}

/// The highest number per mission in each record table, and each counter, on one side.
struct Side {
    max: Vec<BTreeMap<String, i64>>,
    counters: BTreeMap<(String, &'static str), i64>,
}

impl Side {
    fn top(&self, kind: usize, mission: &str) -> i64 {
        self.max[kind].get(mission).copied().unwrap_or(0)
    }

    fn counter(&self, mission: &str, column: &'static str) -> i64 {
        self.counters
            .get(&(mission.to_string(), column))
            .copied()
            .unwrap_or(1)
    }
}

fn read_side(db: &Dolt, missions: &BTreeSet<String>, as_of: Option<&str>) -> Option<Side> {
    let list: Vec<String> = missions.iter().map(|m| lit(m)).collect();
    let list = list.join(", ");
    let at = as_of.map_or(String::new(), |r| format!(" as of {}", plain(r)));
    let mut queries: Vec<String> = KINDS
        .iter()
        .map(|kind| {
            format!(
                "select mission_id as m, max(seq) as s from {}{} where mission_id in ({}) \
                 group by mission_id",
                kind.table, at, list
            )
        })
        .collect();
    let columns: Vec<&'static str> = KINDS.iter().filter_map(|k| k.counter).collect();
    queries.push(format!(
        "select id, {} from missions{} where id in ({})",
        columns.join(", "),
        at,
        list
    ));
    let sets = each(db, &queries)?;
    let max = (0..KINDS.len())
        .map(|k| {
            sets[k]
                .iter()
                .filter_map(|row| Some((text(row, "m")?, integer(row, "s")?)))
                .collect()
        })
        .collect();
    let mut counters = BTreeMap::new();
    for row in &sets[KINDS.len()] {
        let Some(id) = text(row, "id") else { continue };
        for column in &columns {
            counters.insert((id.clone(), *column), integer(row, column).unwrap_or(1));
        }
    }
    Some(Side { max, counters })
}

fn plan(
    db: &Dolt,
    remote_ref: &str,
    collisions: &Collisions,
    clashes: &[CounterClash],
) -> Option<Plan> {
    let missions: BTreeSet<String> = collisions
        .iter()
        .flatten()
        .map(|(mission, _)| mission.clone())
        .chain(clashes.iter().map(|c| c.mission.clone()))
        .collect();
    let local = read_side(db, &missions, None)?;
    let remote = read_side(db, &missions, Some(remote_ref))?;

    let mut sql = String::new();
    let mut renames = Vec::new();
    // The counters to raise once the merge has run, with the table each one counts and the
    // higher of the two sides' values.
    let mut raise: BTreeMap<(String, &'static str), (&'static str, i64)> = BTreeMap::new();
    for (k, kind) in KINDS.iter().enumerate() {
        let mut by_mission: BTreeMap<&str, Vec<i64>> = BTreeMap::new();
        for (mission, seq) in &collisions[k] {
            by_mission.entry(mission).or_default().push(*seq);
        }
        for (mission, seqs) in by_mission {
            // Past every number either side holds, and past one that a removal freed.
            let mut next = local.top(k, mission).max(remote.top(k, mission));
            if let Some(column) = kind.counter {
                let (ours, theirs) = (
                    local.counter(mission, column),
                    remote.counter(mission, column),
                );
                next = next.max(ours - 1).max(theirs - 1);
                raise.insert(
                    (mission.to_string(), column),
                    (kind.table, ours.max(theirs)),
                );
            }
            for from in seqs {
                next += 1;
                sql.push_str(&rename_sql(k, mission, from, next));
                renames.push(Renumbered {
                    kind: k,
                    mission: mission.to_string(),
                    from,
                    to: next,
                });
            }
        }
    }
    if !renames.is_empty() {
        sql.insert_str(0, "set foreign_key_checks = 0;\n");
    }
    for clash in clashes {
        sql.push_str(&format!(
            "update missions set {} = {} where id = {};\n",
            clash.column,
            clash.base,
            lit(&clash.mission)
        ));
        let table = KINDS
            .iter()
            .find(|k| k.counter == Some(clash.column))?
            .table;
        let floor = local
            .counter(&clash.mission, clash.column)
            .max(remote.counter(&clash.mission, clash.column));
        raise.insert((clash.mission.clone(), clash.column), (table, floor));
    }
    let mut fixup = String::new();
    for ((mission, column), (table, floor)) in raise {
        fixup.push_str(&format!(
            "update missions set {c} = greatest({c}, {floor}, \
             (select coalesce(max(seq), 0) + 1 from {table} where mission_id = {m})) where id = {m};\n",
            c = column,
            floor = floor,
            table = table,
            m = lit(&mission)
        ));
    }
    Some(Plan {
        sql,
        fixup,
        renames,
    })
}

/// The statements that move one row to a new number, and the rows that name it.
fn rename_sql(kind: usize, mission: &str, from: i64, to: i64) -> String {
    let m = lit(mission);
    let table = KINDS[kind].table;
    let mut sql = format!(
        "update {} set seq = {} where mission_id = {} and seq = {};\n",
        table, to, m, from
    );
    if table == "tasks" {
        sql.push_str(&format!(
            "update task_blocks set task_seq = {to} where mission_id = {m} and task_seq = {from};\n\
             update task_blocks set blocked_by_seq = {to} where mission_id = {m} and blocked_by_seq = {from};\n\
             update decisions set task_seq = {to} where mission_id = {m} and task_seq = {from};\n",
            to = to,
            from = from,
            m = m
        ));
    }
    sql
}

#[cfg(test)]
mod tests {
    use super::*;

    fn change(kind: usize, from: i64, to: i64) -> Renumbered {
        Renumbered {
            kind,
            mission: "M-X".to_string(),
            from,
            to,
        }
    }

    #[test]
    fn a_line_names_both_numbers_the_way_the_lists_print_them() {
        assert_eq!(change(0, 3, 5).line(), "T-03 is now T-05 in M-X");
        assert_eq!(change(1, 2, 12).line(), "Q-02 is now Q-12 in M-X");
        assert_eq!(
            change(2, 1, 4).line(),
            "objective 1 is now objective 4 in M-X"
        );
        assert_eq!(
            change(3, 1, 4).line(),
            "constraint 1 is now constraint 4 in M-X"
        );
        assert_eq!(
            change(4, 1, 4).line(),
            "out-of-scope 1 is now out-of-scope 4 in M-X"
        );
        assert_eq!(
            change(5, 2, 3).line(),
            "decision 2 is now decision 3 in M-X"
        );
        assert_eq!(change(0, 99, 100).line(), "T-99 is now T-100 in M-X");
    }

    #[test]
    fn a_row_renumbered_twice_keeps_one_line_from_its_first_number() {
        let mut changes = vec![change(0, 3, 5), change(1, 3, 5)];
        record(&mut changes, change(0, 5, 7));
        record(&mut changes, change(0, 3, 9));
        assert_eq!(
            changes
                .iter()
                .map(Renumbered::line)
                .collect::<Vec<String>>(),
            vec![
                "T-03 is now T-07 in M-X",
                "Q-03 is now Q-05 in M-X",
                "T-03 is now T-09 in M-X"
            ]
        );
    }

    #[test]
    fn a_renamed_task_takes_the_blocks_and_decisions_that_name_it() {
        let sql = rename_sql(0, "M-X", 2, 7);
        assert!(sql.contains("update tasks set seq = 7"));
        assert!(sql.contains("update task_blocks set task_seq = 7"));
        assert!(sql.contains("update task_blocks set blocked_by_seq = 7"));
        assert!(sql.contains("update decisions set task_seq = 7"));
        let sql = rename_sql(5, "M-X", 2, 7);
        assert!(sql.contains("update decisions set seq = 7"));
        assert!(!sql.contains("task_blocks"));
    }
}
