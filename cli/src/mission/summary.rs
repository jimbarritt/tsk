//! `tsk mission summary`: counts of missions by status, and of the tasks of each mission
//! by status. Every number is a count of records. A mission counts under its own status
//! and its tasks under its own, so a parent does not take in the missions or the tasks of
//! its children.

use serde::Serialize;

use super::model::{Mission, Status};

/// The statuses in the order the summary prints them.
const STATUSES: [Status; 3] = [Status::Todo, Status::InProgress, Status::Done];

/// A count for each status.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Counts {
    pub todo: usize,
    #[serde(rename = "in-progress")]
    pub in_progress: usize,
    pub done: usize,
    pub total: usize,
}

impl Counts {
    fn add(&mut self, status: Status) {
        match status {
            Status::Todo => self.todo += 1,
            Status::InProgress => self.in_progress += 1,
            Status::Done => self.done += 1,
        }
        self.total += 1;
    }

    fn of(&self, status: Status) -> usize {
        match status {
            Status::Todo => self.todo,
            Status::InProgress => self.in_progress,
            Status::Done => self.done,
        }
    }
}

/// The tasks of one mission by status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MissionTasks {
    pub mission: String,
    pub status: &'static str,
    pub tasks: Counts,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Summary {
    pub missions: Counts,
    pub tasks: Counts,
    pub by_mission: Vec<MissionTasks>,
}

/// Count `missions` and their tasks, in the order given.
pub fn summarize(missions: &[Mission]) -> Summary {
    let mut summary = Summary {
        missions: Counts::default(),
        tasks: Counts::default(),
        by_mission: Vec::new(),
    };
    for mission in missions {
        summary.missions.add(mission.status);
        let mut tasks = Counts::default();
        for task in &mission.tasks {
            tasks.add(task.status);
            summary.tasks.add(task.status);
        }
        summary.by_mission.push(MissionTasks {
            mission: mission.id.clone(),
            status: mission.status.as_str(),
            tasks,
        });
    }
    summary
}

/// The tab-separated lines. With `porcelain`, each line starts with its kind, `missions`
/// or `tasks`, and there are no headers. Otherwise two blocks with a header each, for
/// [`super::table::print_lines`] to align.
pub fn lines(summary: &Summary, porcelain: bool) -> Vec<Vec<String>> {
    let mut by_status: Vec<String> = Vec::new();
    let mut by_task: Vec<String> = Vec::new();
    let head = |kind: &str| {
        if porcelain {
            format!("{}\t", kind)
        } else {
            String::new()
        }
    };
    if !porcelain {
        by_status.push("status\tmissions".to_string());
    }
    for status in STATUSES {
        by_status.push(format!(
            "{}{}\t{}",
            head("missions"),
            status.as_str(),
            summary.missions.of(status)
        ));
    }
    by_status.push(format!(
        "{}total\t{}",
        head("missions"),
        summary.missions.total
    ));
    if !porcelain {
        by_task.push("mission\tstatus\ttodo\tin-progress\tdone\ttotal".to_string());
    }
    for row in &summary.by_mission {
        by_task.push(task_line(
            &head("tasks"),
            &row.mission,
            row.status,
            &row.tasks,
        ));
    }
    if !summary.by_mission.is_empty() {
        by_task.push(task_line(&head("tasks"), "total", "-", &summary.tasks));
    }
    vec![by_status, by_task]
}

fn task_line(head: &str, mission: &str, status: &str, counts: &Counts) -> String {
    format!(
        "{}{}\t{}\t{}\t{}\t{}\t{}",
        head, mission, status, counts.todo, counts.in_progress, counts.done, counts.total
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mission::model::{create, Category, Task};

    fn mission(id: &str, status: Status, tasks: &[Status], parent: Option<&str>) -> Mission {
        let mut m = create(&[], id, "Name", Category::Operational, None).unwrap();
        m.status = status;
        m.parent = parent.map(str::to_string);
        for (i, s) in tasks.iter().enumerate() {
            m.tasks.push(Task {
                seq: i as i64 + 1,
                description: "t".into(),
                status: *s,
                blocked_by: Vec::new(),
                uid: String::new(),
            });
        }
        m
    }

    fn sample() -> Vec<Mission> {
        vec![
            mission(
                "M-A",
                Status::InProgress,
                &[Status::Done, Status::Todo],
                None,
            ),
            mission("M-B", Status::Done, &[Status::Done], Some("M-A")),
            mission("M-C", Status::Todo, &[], Some("M-A")),
        ]
    }

    #[test]
    fn missions_count_under_their_own_status_without_roll_up() {
        let s = summarize(&sample());
        assert_eq!(
            s.missions,
            Counts {
                todo: 1,
                in_progress: 1,
                done: 1,
                total: 3
            }
        );
        let a = &s.by_mission[0];
        assert_eq!(
            a.tasks,
            Counts {
                todo: 1,
                in_progress: 0,
                done: 1,
                total: 2
            }
        );
        assert_eq!(
            s.tasks,
            Counts {
                todo: 1,
                in_progress: 0,
                done: 2,
                total: 3
            }
        );
        assert_eq!(s.by_mission[2].tasks.total, 0);
    }

    #[test]
    fn no_missions_gives_zero_counts() {
        let s = summarize(&[]);
        assert_eq!(s.missions, Counts::default());
        assert!(s.by_mission.is_empty());
        let l = lines(&s, true);
        assert_eq!(l[0].len(), 4);
        assert!(l[1].is_empty());
    }

    #[test]
    fn porcelain_lines_have_a_kind_and_no_header() {
        let l = lines(&summarize(&sample()), true);
        assert_eq!(
            l[0],
            vec![
                "missions\ttodo\t1",
                "missions\tin-progress\t1",
                "missions\tdone\t1",
                "missions\ttotal\t3"
            ]
        );
        assert_eq!(l[1][0], "tasks\tM-A\tin-progress\t1\t0\t1\t2");
        assert_eq!(l[1][3], "tasks\ttotal\t-\t1\t0\t2\t3");
    }

    #[test]
    fn person_lines_have_headers() {
        let l = lines(&summarize(&sample()), false);
        assert_eq!(l[0][0], "status\tmissions");
        assert_eq!(l[0][1], "todo\t1");
        assert_eq!(l[1][0], "mission\tstatus\ttodo\tin-progress\tdone\ttotal");
        assert_eq!(l[1][1], "M-A\tin-progress\t1\t0\t1\t2");
    }
}
