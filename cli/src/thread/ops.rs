use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

use serde::Serialize;

use crate::ledger::git::Git;
use crate::ledger::location::mint_token;

use super::binding::{bind_cloud, bind_current, remove_marker, resolve_at, Binding};
use super::clock::utc_now;
use super::entry::{self, read_store, CodeState, GitState, LedgerState, NewEntry, STORE_FILE};
use super::lookup::{lookup_path, Lookup};
use super::session::Session;

pub const THREAD_ID_LEN: usize = 8;
pub const MINT_ATTEMPTS: usize = 100;

pub enum StartOutcome {
    Started(String),
    ResumeRequired(String),
}

pub struct ResumeOutcome {
    pub thread_id: String,
    pub latest: String,
    pub warning: String,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct ListRow {
    pub id: String,
    pub mission_link: String,
    pub latest_whats_next: Option<String>,
    pub latest_timestamp: Option<String>,
}

fn check_thread_id(id: &str) -> Result<(), String> {
    if id.len() == THREAD_ID_LEN
        && id
            .chars()
            .all(|c| c.is_ascii_digit() || c.is_ascii_lowercase())
    {
        Ok(())
    } else {
        Err(format!(
            "error: invalid thread id '{}': expected {} characters from [0-9a-z]",
            id, THREAD_ID_LEN
        ))
    }
}

pub fn check_briefing_path(path: &str) -> Result<(), String> {
    let p = Path::new(path);
    let inside = !path.is_empty()
        && p.components()
            .all(|c| matches!(c, Component::Normal(_) | Component::CurDir));
    if inside {
        Ok(())
    } else {
        Err(format!(
            "error: mission briefing path '{}' must be relative to the ledger root, with no '..'",
            path
        ))
    }
}

pub fn thread_dir(wt: &Path, id: &str) -> PathBuf {
    wt.join("threads").join(id)
}

fn mint_id(wt: &Path) -> Result<String, String> {
    let seed = wt.to_string_lossy();
    for _ in 0..MINT_ATTEMPTS {
        let id = mint_token(THREAD_ID_LEN, &seed);
        if !thread_dir(wt, &id).exists() {
            return Ok(id);
        }
    }
    Err(format!(
        "error: no free thread id after {} attempts; is {} intact?",
        MINT_ATTEMPTS,
        wt.join("threads").display()
    ))
}

pub fn index_md(id: &str, briefing_path: &str) -> String {
    format!(
        "# Thread {}\n\nMission briefing: [{}]({})\n",
        id, briefing_path, briefing_path
    )
}

fn scaffold(wt: &Path, id: &str, briefing_path: &str) -> Result<(), String> {
    let dir = thread_dir(wt, id);
    if dir.exists() {
        return Err(format!("error: threads/{} already exists", id));
    }
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("error: could not create {}: {}", dir.display(), e))?;
    let index = dir.join("index.md");
    std::fs::write(&index, index_md(id, briefing_path))
        .map_err(|e| format!("error: could not write {}: {}", index.display(), e))?;
    let store = dir.join(STORE_FILE);
    std::fs::write(&store, "")
        .map_err(|e| format!("error: could not write {}: {}", store.display(), e))
}

pub fn start(
    session: &Session,
    mission_id: &str,
    briefing_path: &str,
) -> Result<StartOutcome, String> {
    let wt = session.refresh()?;
    if let Some(binding) = resolve_at(session, &wt)? {
        return Ok(StartOutcome::ResumeRequired(
            binding.thread_id().to_string(),
        ));
    }
    check_briefing_path(briefing_path)?;
    let briefing = wt.join(briefing_path);
    if !briefing.is_file() {
        return Err(format!(
            "error: mission briefing not found at ledger-relative path '{}' (checked {})",
            briefing_path,
            briefing.display()
        ));
    }
    let id = mint_id(&wt)?;
    scaffold(&wt, &id, briefing_path)?;
    bind_current(session, &id, &wt)?;
    session.push(&format!("Start thread {} for mission {}", id, mission_id))?;
    Ok(StartOutcome::Started(id))
}

pub fn pause(
    session: &Session,
    id: &str,
    mission_link: &str,
    task_id: &str,
    whats_next: &str,
) -> Result<(), String> {
    check_thread_id(id)?;
    let wt = session.refresh()?;
    let store = thread_dir(&wt, id).join(STORE_FILE);
    if !store.is_file() {
        return Err(format!(
            "error: no such thread: {} (expected {})",
            id,
            store.display()
        ));
    }
    let ledger_commit = Git::new(&wt).run_line(["rev-parse", "--verify", "HEAD^{commit}"])?;
    let code_git = &session.repo.git;
    let code_ref = current_branch_ref(code_git)?;
    let code_commit = code_git.run_line(["rev-parse", "--verify", "HEAD^{commit}"])?;
    ensure_on_an_origin_branch(code_git, &code_ref, &code_commit)?;
    let timestamp = utc_now();
    let written_by = session.actor_urn();
    entry::append(
        &store,
        &NewEntry {
            mission_link,
            task_id,
            whats_next,
            git: GitState {
                ledger: LedgerState {
                    commit: &ledger_commit,
                },
                code: CodeState {
                    r#ref: &code_ref,
                    commit: &code_commit,
                },
            },
            timestamp: &timestamp,
            written_by: &written_by,
        },
    )?;
    session.push(&format!("Pause thread {}: {}", id, task_id))
}

const BRANCH_PREFIX: &str = "refs/heads/";
const ORIGIN_HEADS_REFSPEC: &str = "+refs/heads/*:refs/remotes/origin/*";
const ORIGIN_REMOTE_PREFIX: &str = "refs/remotes/origin/";
const ORIGIN_REMOTE_HEAD: &str = "refs/remotes/origin/HEAD";

fn current_branch_ref(git: &Git) -> Result<String, String> {
    let (_, output) = git.output(["symbolic-ref", "--quiet", "HEAD"])?;
    match output.status.code() {
        Some(0) => {
            let text = String::from_utf8_lossy(&output.stdout);
            let name = text.trim_end_matches(['\n', '\r']);
            if is_branch_ref(name) {
                Ok(name.to_string())
            } else {
                Err(format!(
                    "error: HEAD points at '{}', which is not a branch.\n       \
                     Check out a branch before pausing.",
                    name
                ))
            }
        }
        Some(1) => Err(
            "error: HEAD is detached, so the branch the work is on cannot be recorded.\n       \
             Check out a branch, and push it to origin, before pausing."
                .to_string(),
        ),
        _ => Err(crate::ledger::git::failure_message(
            &[
                "symbolic-ref".to_string(),
                "--quiet".to_string(),
                "HEAD".to_string(),
            ],
            &output,
        )),
    }
}

fn ensure_on_an_origin_branch(git: &Git, code_ref: &str, commit: &str) -> Result<(), String> {
    git.run([
        "fetch",
        "--quiet",
        "--prune",
        "origin",
        ORIGIN_HEADS_REFSPEC,
    ])?;
    let listing = git.run([
        "for-each-ref",
        "--contains",
        commit,
        "--format=%(refname)",
        ORIGIN_REMOTE_PREFIX,
    ])?;
    if origin_branches(&listing).is_empty() {
        return Err(format!(
            "error: HEAD ({}) is not reachable from any branch on origin.\n       \
             A different actor resuming this thread elsewhere would not be able\n       \
             to see this commit. Push {} before pausing.",
            commit,
            code_ref.strip_prefix(BRANCH_PREFIX).unwrap_or(code_ref)
        ));
    }
    Ok(())
}

fn origin_branches(listing: &str) -> Vec<String> {
    listing
        .lines()
        .filter(|line| line.starts_with(ORIGIN_REMOTE_PREFIX) && *line != ORIGIN_REMOTE_HEAD)
        .map(str::to_string)
        .collect()
}

fn is_branch_ref(target: &str) -> bool {
    target
        .strip_prefix(BRANCH_PREFIX)
        .is_some_and(|name| !name.is_empty() && !name.contains(char::is_whitespace))
}

pub fn prior_actors(entries: &[entry::StoredEntry]) -> BTreeSet<String> {
    entries
        .iter()
        .filter_map(|stored| stored.entry.written_by.clone())
        .filter(|actor| !actor.is_empty())
        .collect()
}

pub fn takeover_warning(id: &str, prior: &BTreeSet<String>, current: &str) -> String {
    if prior.is_empty() || prior.contains(current) {
        return String::new();
    }
    format!(
        "thread {} is already associated with other actor(s): {}",
        id,
        prior.iter().cloned().collect::<Vec<_>>().join(",")
    )
}

pub fn resume(session: &Session, id: &str) -> Result<ResumeOutcome, String> {
    check_thread_id(id)?;
    let wt = session.refresh()?;
    let dir = thread_dir(&wt, id);
    if !dir.is_dir() {
        return Err(format!("error: no such thread: {}", id));
    }
    let entries = read_store(&dir.join(STORE_FILE))?;
    let current = session.actor_urn();
    let warning = takeover_warning(id, &prior_actors(&entries), &current);

    match &session.cloud_session {
        Some(cloud) => {
            if resolve_at(session, &wt)? != Some(Binding::Cloud(id.to_string())) {
                bind_cloud(cloud, id, &wt)?;
                session.push(&format!("Bind {} to thread {}", current, id))?;
            }
        }
        None => super::binding::write_marker(session, id)?,
    }

    Ok(ResumeOutcome {
        thread_id: id.to_string(),
        latest: entries
            .last()
            .map(|stored| stored.raw.clone())
            .unwrap_or_else(|| "{}".to_string()),
        warning,
    })
}

pub fn render_resume(outcome: &ResumeOutcome) -> String {
    format!(
        "{{\"thread_id\":{},\"latest\":{},\"warning\":{}}}",
        serde_json::Value::String(outcome.thread_id.clone()),
        outcome.latest,
        serde_json::Value::String(outcome.warning.clone())
    )
}

pub fn detach(session: &Session) -> Result<String, String> {
    let wt = session.refresh()?;
    let binding = resolve_at(session, &wt)?.ok_or_else(|| {
        "error: no thread binding found for this session or worktree; nothing to detach".to_string()
    })?;
    match (&binding, &session.cloud_session) {
        (Binding::Cloud(id), Some(cloud)) => {
            let path = lookup_path(&wt);
            let mut lookup = Lookup::read(&path)?.unwrap_or_default();
            lookup.remove(cloud);
            lookup.write(&path)?;
            session.push(&format!(
                "Detach {} from thread {}",
                session.actor_urn(),
                id
            ))?;
        }
        _ => remove_marker(session)?,
    }
    Ok(binding.thread_id().to_string())
}

pub fn stop(session: &Session, target: Option<&str>) -> Result<String, String> {
    let wt = session.refresh()?;
    let current = resolve_at(session, &wt)?;
    let id = match target {
        Some(id) => id.to_string(),
        None => current
            .as_ref()
            .map(|b| b.thread_id().to_string())
            .ok_or_else(|| {
                "error: no thread binding found for this session or worktree, and no thread id given; nothing to stop"
                    .to_string()
            })?,
    };
    check_thread_id(&id)?;
    let dir = thread_dir(&wt, &id);
    if !dir.is_dir() {
        return Err(format!("error: no such thread: {}", id));
    }
    if current == Some(Binding::Worktree(id.clone())) {
        remove_marker(session)?;
    }
    let path = lookup_path(&wt);
    if let Some(mut lookup) = Lookup::read(&path)? {
        lookup.remove_thread(&id);
        lookup.write(&path)?;
    }
    std::fs::remove_dir_all(&dir)
        .map_err(|e| format!("error: could not remove {}: {}", dir.display(), e))?;
    session.push(&format!("Stop thread {}", id))?;
    Ok(id)
}

pub fn mission_link(index_md: &str) -> String {
    const MARK: &str = "Mission briefing: [";
    index_md
        .lines()
        .find_map(|line| {
            line.find(MARK).map(|at| {
                let rest = &line[at + MARK.len()..];
                rest.split(']').next().unwrap_or("").to_string()
            })
        })
        .unwrap_or_default()
}

pub fn list_rows(wt: &Path) -> Result<Vec<ListRow>, String> {
    let threads = wt.join("threads");
    let mut names: Vec<String> = match std::fs::read_dir(&threads) {
        Ok(dir) => dir
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|name| !name.starts_with('.'))
            .collect(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => {
            return Err(format!(
                "error: could not read {}: {}",
                threads.display(),
                e
            ))
        }
    };
    names.sort();

    let mut rows = Vec::new();
    for name in names {
        let dir = threads.join(&name);
        let link = std::fs::read_to_string(dir.join("index.md"))
            .map(|text| mission_link(&text))
            .unwrap_or_default();
        let entries = read_store(&dir.join(STORE_FILE))?;
        let latest = entries.last().map(|stored| &stored.entry);
        rows.push(ListRow {
            id: name,
            mission_link: link,
            latest_whats_next: latest.and_then(|e| e.whats_next.clone()),
            latest_timestamp: latest.and_then(|e| e.timestamp.clone()),
        });
    }
    Ok(sort_rows(rows))
}

pub fn sort_rows(mut rows: Vec<ListRow>) -> Vec<ListRow> {
    rows.sort_by(|a, b| {
        a.latest_timestamp
            .as_deref()
            .unwrap_or("")
            .cmp(b.latest_timestamp.as_deref().unwrap_or(""))
    });
    rows.reverse();
    rows
}

pub fn list(session: &Session) -> Result<Vec<ListRow>, String> {
    let wt = session.refresh()?;
    list_rows(&wt)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thread_id_must_be_eight_lowercase_base36_characters() {
        assert!(check_thread_id("1234abcd").is_ok());
        assert!(check_thread_id("4onylfsg").is_ok());
        assert!(check_thread_id("1234abc").is_err());
        assert!(check_thread_id("1234ABCD").is_err());
        assert!(check_thread_id("../../xy").is_err());
        assert!(check_thread_id("").is_err());
    }

    #[test]
    fn briefing_path_stays_inside_the_ledger() {
        assert!(check_briefing_path("missions/operational/M-X.md").is_ok());
        assert!(check_briefing_path("./missions/M-X.md").is_ok());
        assert!(check_briefing_path("/etc/passwd").is_err());
        assert!(check_briefing_path("missions/../../x.md").is_err());
        assert!(check_briefing_path("").is_err());
    }

    #[test]
    fn is_branch_ref_accepts_only_named_branch_refs() {
        assert!(is_branch_ref("refs/heads/main"));
        assert!(is_branch_ref("refs/heads/feature/x"));
        assert!(!is_branch_ref("refs/heads/"));
        assert!(!is_branch_ref("refs/tags/v1"));
        assert!(!is_branch_ref("refs/heads/a b"));
        assert!(!is_branch_ref(""));
    }

    #[test]
    fn origin_branches_skips_the_symbolic_head_and_other_refs() {
        let listing = "refs/remotes/origin/HEAD\nrefs/remotes/origin/main\nrefs/remotes/origin/feature/x\nrefs/remotes/upstream/main\n";
        assert_eq!(
            origin_branches(listing),
            vec![
                "refs/remotes/origin/main".to_string(),
                "refs/remotes/origin/feature/x".to_string()
            ]
        );
        assert!(origin_branches("refs/remotes/origin/HEAD\n").is_empty());
        assert!(origin_branches("").is_empty());
    }

    #[test]
    fn index_md_matches_the_script_layout() {
        assert_eq!(
            index_md("1234abcd", "missions/M-X.md"),
            "# Thread 1234abcd\n\nMission briefing: [missions/M-X.md](missions/M-X.md)\n"
        );
    }

    #[test]
    fn mission_link_reads_the_first_briefing_link() {
        assert_eq!(
            mission_link(&index_md("1234abcd", "missions/M-X.md")),
            "missions/M-X.md"
        );
        assert_eq!(mission_link("# Thread\n\nno link\n"), "");
        assert_eq!(
            mission_link("Mission briefing: [open ended\n"),
            "open ended"
        );
    }

    #[test]
    fn scaffold_writes_index_and_empty_store_and_refuses_an_existing_thread() {
        let dir = tempfile::tempdir().unwrap();
        scaffold(dir.path(), "1234abcd", "missions/M-X.md").unwrap();
        let thread = thread_dir(dir.path(), "1234abcd");
        assert_eq!(
            std::fs::read_to_string(thread.join("index.md")).unwrap(),
            index_md("1234abcd", "missions/M-X.md")
        );
        assert_eq!(
            std::fs::read_to_string(thread.join(STORE_FILE)).unwrap(),
            ""
        );
        let err = scaffold(dir.path(), "1234abcd", "missions/M-X.md").unwrap_err();
        assert_eq!(err, "error: threads/1234abcd already exists");
    }

    #[test]
    fn mint_id_is_eight_hex_characters_not_already_taken() {
        let dir = tempfile::tempdir().unwrap();
        let id = mint_id(dir.path()).unwrap();
        assert!(check_thread_id(&id).is_ok());
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
        assert!(!thread_dir(dir.path(), &id).exists());
    }

    fn actors(list: &[&str]) -> BTreeSet<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn no_warning_without_prior_actors_or_when_current_is_among_them() {
        assert_eq!(takeover_warning("1234abcd", &actors(&[]), "urn:a"), "");
        assert_eq!(
            takeover_warning("1234abcd", &actors(&["urn:a", "urn:b"]), "urn:a"),
            ""
        );
    }

    #[test]
    fn warning_names_every_other_actor_sorted_and_comma_joined() {
        assert_eq!(
            takeover_warning("1234abcd", &actors(&["urn:c", "urn:b"]), "urn:a"),
            "thread 1234abcd is already associated with other actor(s): urn:b,urn:c"
        );
    }

    #[test]
    fn render_resume_passes_the_latest_entry_through_as_stored() {
        let outcome = ResumeOutcome {
            thread_id: "1234abcd".to_string(),
            latest: "{\"whats_next\":\"x\",\"git\":{\"ledger\":{\"commit\":\"b\"}}}".to_string(),
            warning: String::new(),
        };
        assert_eq!(
            render_resume(&outcome),
            "{\"thread_id\":\"1234abcd\",\"latest\":{\"whats_next\":\"x\",\"git\":{\"ledger\":{\"commit\":\"b\"}}},\"warning\":\"\"}"
        );
    }

    fn row(id: &str, ts: Option<&str>) -> ListRow {
        ListRow {
            id: id.to_string(),
            mission_link: String::new(),
            latest_whats_next: None,
            latest_timestamp: ts.map(str::to_string),
        }
    }

    #[test]
    fn sort_rows_puts_newest_first_and_threads_without_entries_last() {
        let sorted = sort_rows(vec![
            row("aaaaaaaa", None),
            row("bbbbbbbb", Some("2026-09-01T00:00:00Z")),
            row("cccccccc", None),
            row("dddddddd", Some("2026-10-01T00:00:00Z")),
        ]);
        let ids: Vec<_> = sorted.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, vec!["dddddddd", "bbbbbbbb", "cccccccc", "aaaaaaaa"]);
    }

    #[test]
    fn list_row_serialises_in_the_script_field_order() {
        let r = ListRow {
            id: "1234abcd".to_string(),
            mission_link: "m.md".to_string(),
            latest_whats_next: None,
            latest_timestamp: None,
        };
        assert_eq!(
            serde_json::to_string(&r).unwrap(),
            "{\"id\":\"1234abcd\",\"mission_link\":\"m.md\",\"latest_whats_next\":null,\"latest_timestamp\":null}"
        );
    }
}
