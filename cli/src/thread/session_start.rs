use std::io::{Read, Write};
use std::path::Path;
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

use crate::ledger::fetch::{fetch, FetchOutcome};
use crate::ledger::location::state_root_from_env;

use super::binding::{resolve_at, UNBOUND_PROMPT};
use super::session::Session;

pub const ENV_FILE_VAR: &str = "CLAUDE_ENV_FILE";
pub const LEDGER_WT_VAR: &str = "TSK_LEDGER_WT";
pub const CLAIM_DIR: &str = "session-start";
pub const CLAIM_WINDOW: Duration = Duration::from_secs(300);
pub const CLAIM_PRUNE_AGE: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Deserialize, Default)]
pub struct HookInput {
    pub session_id: Option<String>,
    pub source: Option<String>,
}

impl HookInput {
    pub fn parse(bytes: &[u8]) -> HookInput {
        serde_json::from_slice(bytes).unwrap_or_default()
    }

    fn claim_name(&self) -> Option<String> {
        let session_id = self.session_id.as_deref().filter(|v| is_safe_name(v))?;
        let source = self
            .source
            .as_deref()
            .filter(|v| is_safe_name(v))
            .unwrap_or("unknown");
        Some(format!("{}.{}", session_id, source))
    }
}

fn is_safe_name(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

#[derive(Debug, PartialEq)]
pub enum Claim {
    First,
    Duplicate,
}

/// Two hooks can run this command for one `SessionStart` event: the plugin's and a
/// repo's own. The first to create the claim file for the session ID and source does
/// the work. Any other run inside `CLAIM_WINDOW` of that claim is a duplicate. A claim
/// older than the window belongs to an earlier event (a second compact, say) and is
/// replaced. Any I/O error counts as `First`, so a broken state directory never
/// suppresses session start.
pub fn claim(dir: &Path, input: &HookInput, now: SystemTime) -> Claim {
    let Some(name) = input.claim_name() else {
        return Claim::First;
    };
    if std::fs::create_dir_all(dir).is_err() {
        return Claim::First;
    }
    prune_claims(dir, now);
    let path = dir.join(name);
    match create_claim(&path) {
        Ok(()) => return Claim::First,
        Err(e) if e.kind() != std::io::ErrorKind::AlreadyExists => return Claim::First,
        Err(_) => {}
    }
    if !is_older_than(&path, now, CLAIM_WINDOW) {
        return Claim::Duplicate;
    }
    let _ = std::fs::remove_file(&path);
    match create_claim(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Claim::Duplicate,
        _ => Claim::First,
    }
}

fn create_claim(path: &Path) -> std::io::Result<()> {
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map(|_| ())
}

fn is_older_than(path: &Path, now: SystemTime, age: Duration) -> bool {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|modified| now.duration_since(modified).ok())
        .is_some_and(|elapsed| elapsed >= age)
}

fn prune_claims(dir: &Path, now: SystemTime) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if is_older_than(&path, now, CLAIM_PRUNE_AGE) {
            let _ = std::fs::remove_file(path);
        }
    }
}

#[derive(Serialize)]
struct SpecificOutput<'a> {
    #[serde(rename = "hookEventName")]
    hook_event_name: &'a str,
    #[serde(rename = "additionalContext")]
    additional_context: &'a str,
}

#[derive(Serialize)]
struct HookOutput<'a> {
    #[serde(rename = "hookSpecificOutput")]
    hook_specific_output: SpecificOutput<'a>,
}

pub fn hook_output(context: &str) -> String {
    serde_json::to_string(&HookOutput {
        hook_specific_output: SpecificOutput {
            hook_event_name: "SessionStart",
            additional_context: context,
        },
    })
    .unwrap_or_default()
}

pub fn fetched_context(path: &Path, pending: &PendingNote, thread_message: &str) -> String {
    let wt = path.display();
    let mut context = format!(
        "The ledger was fetched and materialised at {} (also exported as ${}). Read {}/index.md next.",
        wt, LEDGER_WT_VAR, wt
    );
    if !pending.lines.is_empty() {
        context.push(' ');
        context.push_str(&pending_warning(path, pending));
    }
    context.push(' ');
    context.push_str(thread_message);
    context
}

pub struct PendingNote {
    pub remote_ref: String,
    pub origin_commit: String,
    pub lines: Vec<String>,
}

impl PendingNote {
    pub fn from_outcome(outcome: &FetchOutcome) -> PendingNote {
        PendingNote {
            remote_ref: format!(
                "{}'s {}",
                outcome.location.label(),
                outcome.location.ref_name()
            ),
            origin_commit: outcome.commit.clone(),
            lines: outcome.pending.clone(),
        }
    }
}

fn pending_warning(path: &Path, pending: &PendingNote) -> String {
    format!(
        "WARNING: the ledger worktree holds a commit that is not on {}, so it was left as it is rather than reset: {}. A worker restart can end a turn between a commit and its push, which leaves exactly this state, so this may be your own work from a turn you hold no record of making. Do not assume another actor made it, and do not discard it on that basis. Read what it changes first, with: git -C '{}' log -p {}..HEAD. Then push it with tsk ledger push, or discard it deliberately once you know what it is.",
        pending.remote_ref,
        pending.lines.join(";"),
        path.display(),
        pending.origin_commit
    )
}

pub fn thread_message(binding: Option<&str>) -> String {
    match binding {
        Some(binding) => {
            let thread_id = binding.split_once(':').map(|(_, id)| id).unwrap_or(binding);
            format!(
                "An existing thread binding was found: {}. Run /tsk:resume-thread {} next.",
                binding, thread_id
            )
        }
        None => UNBOUND_PROMPT.to_string(),
    }
}

pub fn failure_context(error: &str) -> String {
    format!(
        "Warning: the ledger could not be fetched automatically at session start ({}). Run `tsk ledger fetch` manually before reading index.md.",
        error.replace('\n', " ").trim()
    )
}

fn export_to_env_file(path: &Path) -> Result<(), String> {
    let Some(file) = std::env::var_os(ENV_FILE_VAR).filter(|v| !v.is_empty()) else {
        return Ok(());
    };
    let mut handle = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&file)
        .map_err(|e| format!("could not open {}: {}", Path::new(&file).display(), e))?;
    writeln!(handle, "export {}=\"{}\"", LEDGER_WT_VAR, path.display())
        .map_err(|e| format!("could not write {}: {}", Path::new(&file).display(), e))
}

fn context_after_fetch(session: &Session) -> Result<String, String> {
    let outcome = fetch(&session.repo, &session.state_root)?;
    let binding = resolve_at(session, &outcome.path)
        .ok()
        .flatten()
        .map(|b| b.to_string());
    let mut context = fetched_context(
        &outcome.path,
        &PendingNote::from_outcome(&outcome),
        &thread_message(binding.as_deref()),
    );
    if let Err(e) = export_to_env_file(&outcome.path) {
        context.push_str(&format!(
            " WARNING: {} was not exported ({}); use the path above.",
            LEDGER_WT_VAR, e
        ));
    }
    Ok(context)
}

pub fn run() -> i32 {
    let mut bytes = Vec::new();
    let _ = std::io::stdin().read_to_end(&mut bytes);
    let input = HookInput::parse(&bytes);
    if let Ok(state_root) = state_root_from_env() {
        if claim(&state_root.join(CLAIM_DIR), &input, SystemTime::now()) == Claim::Duplicate {
            return 0;
        }
    }
    let context = Session::discover()
        .and_then(|session| context_after_fetch(&session))
        .unwrap_or_else(|e| failure_context(&e));
    println!("{}", hook_output(&context));
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn no_pending() -> PendingNote {
        PendingNote {
            remote_ref: "origin's refs/heads/tsk/ledger".to_string(),
            origin_commit: "abc123".to_string(),
            lines: Vec::new(),
        }
    }

    #[test]
    fn hook_output_is_one_compact_object_in_hook_order() {
        let text = hook_output("a \"b\"");
        assert_eq!(
            text,
            "{\"hookSpecificOutput\":{\"hookEventName\":\"SessionStart\",\"additionalContext\":\"a \\\"b\\\"\"}}"
        );
    }

    #[test]
    fn fetched_context_names_the_path_and_the_next_read() {
        let path = PathBuf::from("/s/ledger");
        let context = fetched_context(&path, &no_pending(), "Next.");
        assert_eq!(
            context,
            "The ledger was fetched and materialised at /s/ledger (also exported as $TSK_LEDGER_WT). Read /s/ledger/index.md next. Next."
        );
    }

    #[test]
    fn fetched_context_adds_the_pending_warning() {
        let path = PathBuf::from("/s/ledger");
        let pending = PendingNote {
            lines: vec!["1111111 first".to_string(), "2222222 second".to_string()],
            ..no_pending()
        };
        let context = fetched_context(&path, &pending, "Next.");
        assert!(context.contains(
            "WARNING: the ledger worktree holds a commit that is not on origin's refs/heads/tsk/ledger"
        ));
        assert!(context.contains("1111111 first;2222222 second"));
        assert!(context.contains("git -C '/s/ledger' log -p abc123..HEAD"));
        assert!(context.contains("push it with tsk ledger push"));
        assert!(context.ends_with(" Next."));
    }

    #[test]
    fn thread_message_names_the_binding_and_the_resume_command() {
        assert_eq!(
            thread_message(Some("worktree:1234abcd")),
            "An existing thread binding was found: worktree:1234abcd. Run /tsk:resume-thread 1234abcd next."
        );
        assert_eq!(thread_message(None), UNBOUND_PROMPT);
    }

    fn input(session_id: Option<&str>, source: Option<&str>) -> HookInput {
        HookInput {
            session_id: session_id.map(str::to_string),
            source: source.map(str::to_string),
        }
    }

    fn age(path: &Path, by: Duration) {
        std::fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(SystemTime::now() - by)
            .unwrap();
    }

    #[test]
    fn hook_input_parses_the_session_id_and_source_and_ignores_bad_input() {
        let parsed =
            HookInput::parse(b"{\"session_id\":\"abc\",\"source\":\"clear\",\"cwd\":\"/x\"}");
        assert_eq!(parsed.session_id.as_deref(), Some("abc"));
        assert_eq!(parsed.source.as_deref(), Some("clear"));
        assert!(HookInput::parse(b"").session_id.is_none());
        assert!(HookInput::parse(b"not json").session_id.is_none());
    }

    #[test]
    fn claim_marks_a_second_run_for_the_same_event_as_a_duplicate() {
        let dir = tempfile::tempdir().unwrap();
        let event = input(Some("s-1"), Some("startup"));
        assert_eq!(claim(dir.path(), &event, SystemTime::now()), Claim::First);
        assert_eq!(
            claim(dir.path(), &event, SystemTime::now()),
            Claim::Duplicate
        );
        assert_eq!(
            claim(
                dir.path(),
                &input(Some("s-1"), Some("compact")),
                SystemTime::now()
            ),
            Claim::First
        );
        assert_eq!(
            claim(
                dir.path(),
                &input(Some("s-2"), Some("startup")),
                SystemTime::now()
            ),
            Claim::First
        );
    }

    #[test]
    fn claim_replaces_a_claim_older_than_the_window() {
        let dir = tempfile::tempdir().unwrap();
        let event = input(Some("s-1"), Some("compact"));
        assert_eq!(claim(dir.path(), &event, SystemTime::now()), Claim::First);
        age(&dir.path().join("s-1.compact"), CLAIM_WINDOW);
        assert_eq!(claim(dir.path(), &event, SystemTime::now()), Claim::First);
        assert_eq!(
            claim(dir.path(), &event, SystemTime::now()),
            Claim::Duplicate
        );
    }

    #[test]
    fn claim_needs_a_safe_session_id() {
        let dir = tempfile::tempdir().unwrap();
        for event in [input(None, Some("startup")), input(Some("../x"), None)] {
            assert_eq!(claim(dir.path(), &event, SystemTime::now()), Claim::First);
            assert_eq!(claim(dir.path(), &event, SystemTime::now()), Claim::First);
        }
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[test]
    fn claim_prunes_claims_older_than_a_day() {
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("old.startup");
        std::fs::write(&old, "").unwrap();
        age(&old, CLAIM_PRUNE_AGE);
        claim(
            dir.path(),
            &input(Some("new"), Some("startup")),
            SystemTime::now(),
        );
        assert!(!old.exists());
        assert!(dir.path().join("new.startup").exists());
    }

    #[test]
    fn failure_context_asks_for_a_manual_fetch() {
        let context = failure_context("error: git fetch failed\nfatal: no remote");
        assert!(context.contains("error: git fetch failed fatal: no remote"));
        assert!(context.contains("Run `tsk ledger fetch` manually"));
    }
}
