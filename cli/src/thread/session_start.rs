use std::io::{Read, Write};
use std::path::Path;
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

use crate::ledger::location::state_root_from_env;
use crate::ledger::store::RefreshOutcome;
use crate::self_update::{session_notice, upgrade_for_cloud, CloudUpgrade};

use crate::ledger::store::dolt::VERSION_1_REFUSAL;
use crate::mission::model::{Mission, Status};
use crate::mission::records::task_line;

use super::binding::{is_skipped, open_mission_lines, resolve, unbound_prompt};
use super::ops::{latest_whats_next, thread_mission};
use super::session::Session;

pub const ENV_FILE_VAR: &str = "CLAUDE_ENV_FILE";
pub const LEDGER_WT_VAR: &str = "TSK_LEDGER_WT";
pub const CLAIM_DIR: &str = "session-start";
pub const CLAIM_WINDOW: Duration = Duration::from_secs(300);
pub const CLAIM_PRUNE_AGE: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum HookFormat {
    Claude,
    Copilot,
}

#[derive(Deserialize, Default)]
pub struct HookInput {
    #[serde(alias = "sessionId")]
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

#[derive(Serialize)]
struct CopilotOutput<'a> {
    #[serde(rename = "additionalContext")]
    additional_context: &'a str,
}

pub fn copilot_output(context: &str) -> String {
    serde_json::to_string(&CopilotOutput {
        additional_context: context,
    })
    .unwrap_or_default()
}

pub fn format_output(format: HookFormat, context: &str) -> String {
    match format {
        HookFormat::Claude => hook_output(context),
        HookFormat::Copilot => copilot_output(context),
    }
}

pub fn fetched_context(
    path: &Path,
    read_hint: &str,
    pending: &PendingNote,
    thread_message: &str,
) -> String {
    let mut context = format!(
        "The ledger was fetched and materialised at {} (also exported as ${}).",
        path.display(),
        LEDGER_WT_VAR,
    );
    if !read_hint.is_empty() {
        push_part(&mut context, read_hint);
    }
    if !pending.lines.is_empty() {
        push_part(&mut context, &pending_warning(pending));
    }
    push_part(&mut context, thread_message);
    context
}

/// Add a part after a single space, or directly after a line break when the context ends
/// with one, so that a list of lines never runs into the next sentence.
fn push_part(context: &mut String, part: &str) {
    if !context.ends_with('\n') {
        context.push(' ');
    }
    context.push_str(part);
}

/// Said when the session start leaves uncommitted changes of the ledger store in place.
pub const LOCAL_CHANGES_NOTE: &str = "The ledger store holds local changes that are not pushed (records or documents written since the last push). The refresh left them as they are. A thread command pushes them with its own change, or run `tsk ledger push`.";

pub struct PendingNote {
    pub remote_ref: String,
    /// The command that shows what the pending commits change, from the backend.
    pub inspect_hint: String,
    pub lines: Vec<String>,
}

impl PendingNote {
    pub fn from_outcome(outcome: &RefreshOutcome, inspect_hint: String) -> PendingNote {
        PendingNote {
            remote_ref: outcome.remote_label.clone(),
            inspect_hint,
            lines: outcome.pending.clone(),
        }
    }
}

fn pending_warning(pending: &PendingNote) -> String {
    format!(
        "WARNING: the ledger worktree holds a commit that is not on {}, so it was left as it is rather than reset: {}. A worker restart can end a turn between a commit and its push, which leaves exactly this state, so this may be your own work from a turn you hold no record of making. Do not assume another actor made it, and do not discard it on that basis. Read what it changes first, with: {}. Then push it with tsk ledger push, or, once you know what it is, run tsk ledger status to see what would go and tsk ledger discard to drop it deliberately.",
        pending.remote_ref,
        pending.lines.join(";"),
        pending.inspect_hint
    )
}

/// What the session start says about the thread: the binding to resume, or the prompt
/// that asks which mission to work.
pub fn thread_message(binding: Option<&str>, unbound_prompt: &str) -> String {
    match binding {
        Some(binding) => {
            let thread_id = binding.split_once(':').map(|(_, id)| id).unwrap_or(binding);
            format!(
                "An existing thread binding was found: {}. Run /tsk:resume-thread {} next.",
                binding, thread_id
            )
        }
        None => unbound_prompt.to_string(),
    }
}

/// What the session start says about a bound thread that has no mission, after the
/// binding sentence: it has no tasks, what its latest pause entry says is next, and the
/// command that attaches a mission.
pub fn no_mission_context(latest_whats_next: Option<&str>) -> String {
    let text = latest_whats_next.map_or("none yet", str::trim);
    let stop = if text.ends_with(['.', '!', '?']) {
        ""
    } else {
        "."
    };
    format!(
        "This thread has no mission, so it has no tasks. Its latest what's next: {}{} To attach a mission, run `tsk thread set mission <mission-id>`.",
        text, stop
    )
}

/// Said instead of the unbound prompt when this session or worktree skipped a thread.
pub const SKIPPED_MESSAGE: &str = "This session or worktree works without a thread (tsk thread skip). To bind one, run /tsk:start-thread <mission-id>, or /tsk:resume-thread <thread-id> for an existing thread.";

/// The mission a bound thread works and its open tasks, as `tsk task list` prints them.
pub fn mission_context(mission: &Mission) -> String {
    let mut text = format!(
        "This thread works mission {} ({}): {}.",
        mission.id,
        mission.status.as_str(),
        mission.name
    );
    let open: Vec<String> = mission
        .tasks
        .iter()
        .filter(|t| t.status != Status::Done)
        .map(|t| task_line(mission, t))
        .collect();
    if open.is_empty() {
        text.push_str(" It has no open tasks.");
    } else {
        text.push_str(
            " Its open tasks follow, one per line: id, status, blocked-by, description, tab-separated.\n",
        );
        text.push_str(&open.join("\n"));
        text.push('\n');
    }
    text
}

/// What a version 1 ledger needs, which no other command can do.
pub fn version_1_context(path: &Path) -> String {
    format!(
        "The ledger at {} (also exported as ${}) is at layout version 1: its documents are still in Dolt. Run `tsk ledger migrate-docs` once to move them to the tsk/docs branch. Until then every tsk command except `tsk ledger fetch` stops with an error that names this command.",
        path.display(),
        LEDGER_WT_VAR
    )
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
    let outcome = match session.store.refresh_keeping_local() {
        Ok(outcome) => outcome,
        Err(e) if e == VERSION_1_REFUSAL => {
            // The database is refreshed and the documents are not reachable.
            let path = session.store.path()?;
            let mut context = version_1_context(&path);
            if let Err(e) = export_to_env_file(&path) {
                context.push_str(&format!(
                    " WARNING: {} was not exported ({}); use the path above.",
                    LEDGER_WT_VAR, e
                ));
            }
            return Ok(context);
        }
        Err(e) => return Err(e),
    };
    let store = session.store.as_ref();
    let binding = resolve(session).ok().flatten();
    let missions = store.missions_load().unwrap_or_default();
    let no_documents = store
        .doc_list("missions/")
        .is_ok_and(|documents| documents.is_empty());

    // With mission records the context carries the mission list or the bound mission, and
    // does not send the agent to the index.
    let mut read_hint = if missions.is_empty() {
        store.read_index_hint(&outcome.path)
    } else {
        String::new()
    };
    let mut no_mission = None;
    if let Some(binding) = &binding {
        let id = binding.thread_id();
        let thread_mission = store
            .thread_info(id)
            .ok()
            .and_then(|info| thread_mission(&info));
        match thread_mission.as_deref() {
            Some(mission_id) => {
                if let Some(mission) = missions.iter().find(|m| m.id == mission_id) {
                    read_hint = mission_context(mission);
                }
            }
            None if store.thread_exists(id).unwrap_or(false) => {
                let latest = latest_whats_next(store, id).ok().flatten();
                no_mission = Some(no_mission_context(latest.as_deref()));
            }
            None => {}
        }
    }
    let prompt = if is_skipped(session) {
        SKIPPED_MESSAGE.to_string()
    } else {
        unbound_prompt(&open_mission_lines(&missions))
    };
    let mut thread_text = thread_message(binding.map(|b| b.to_string()).as_deref(), &prompt);
    if let Some(no_mission) = no_mission {
        thread_text.push(' ');
        thread_text.push_str(&no_mission);
    }
    let mut context = fetched_context(
        &outcome.path,
        &read_hint,
        &PendingNote::from_outcome(
            &outcome,
            store.pending_inspect_hint(&outcome.path, &outcome.remote_head),
        ),
        &thread_text,
    );
    if store.has_local_changes().unwrap_or(false) {
        push_part(&mut context, LOCAL_CHANGES_NOTE);
    }
    if missions.is_empty() && no_documents {
        context.push(' ');
        context.push_str(crate::guide::EMPTY_LEDGER_NOTE);
    }
    if let Err(e) = export_to_env_file(&outcome.path) {
        context.push_str(&format!(
            " WARNING: {} was not exported ({}); use the path above.",
            LEDGER_WT_VAR, e
        ));
    }
    Ok(context)
}

pub const REMOTE_VAR: &str = "CLAUDE_CODE_REMOTE";

pub fn is_remote() -> bool {
    std::env::var(REMOTE_VAR).is_ok_and(|v| v == "true")
}

pub fn install_warning(error: &str) -> String {
    format!(
        "Warning: the tsk plugin could not be installed at session start ({}). Run `tsk install-plugin claude-cli` manually.",
        error.replace('\n', " ").trim()
    )
}

pub fn upgrade_warning(error: &str) -> String {
    format!(
        "Warning: tsk could not be upgraded at session start ({}). Run `tsk self-update` manually.",
        error.replace('\n', " ").trim()
    )
}

/// Runs the plugin install for a cloud session. Step lines go to stderr. Returns the
/// warning sentence when a step fails.
fn install_plugin_warning() -> Option<String> {
    let args = crate::install_plugin::InstallPluginArgs::claude_defaults();
    match crate::install_plugin::install(&args, &mut |line| eprintln!("{}", line)) {
        Ok(()) => None,
        Err(e) => {
            eprintln!("{}", e);
            Some(install_warning(&e))
        }
    }
}

pub fn run(format: HookFormat, install_plugin: bool) -> i32 {
    let mut bytes = Vec::new();
    let _ = std::io::stdin().read_to_end(&mut bytes);
    let input = HookInput::parse(&bytes);
    if let Ok(state_root) = state_root_from_env() {
        if claim(&state_root.join(CLAIM_DIR), &input, SystemTime::now()) == Claim::Duplicate {
            return 0;
        }
    }
    let cloud_install = install_plugin && is_remote();
    let mut warnings = Vec::new();
    if cloud_install {
        // A cloud session reuses a cached setup result, so the installed tsk can be
        // old. Upgrade it before the plugin install. This run goes on with the old
        // binary already in memory, and the next command finds the new one.
        if let CloudUpgrade::Failed(e) = upgrade_for_cloud() {
            warnings.push(upgrade_warning(&e));
        }
        warnings.extend(install_plugin_warning());
    }
    let mut context = Session::discover()
        .and_then(|session| context_after_fetch(&session))
        .unwrap_or_else(|e| failure_context(&e));
    if !cloud_install {
        if let Some(notice) = session_notice() {
            context.push(' ');
            context.push_str(&notice);
        }
    }
    for warning in warnings {
        context.push(' ');
        context.push_str(&warning);
    }
    println!("{}", format_output(format, &context));
    0
}

#[cfg(test)]
mod tests {
    use super::super::binding::UNBOUND_PROMPT;
    use super::*;
    use std::path::PathBuf;

    fn no_pending() -> PendingNote {
        PendingNote {
            remote_ref: "origin's refs/heads/tsk/ledger".to_string(),
            inspect_hint: "git -C '/s/ledger' log -p abc123..HEAD".to_string(),
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
    fn copilot_output_is_one_compact_object_with_only_the_context() {
        assert_eq!(
            copilot_output("a \"b\""),
            "{\"additionalContext\":\"a \\\"b\\\"\"}"
        );
        assert_eq!(
            format_output(HookFormat::Copilot, "x"),
            "{\"additionalContext\":\"x\"}"
        );
        assert_eq!(format_output(HookFormat::Claude, "x"), hook_output("x"));
    }

    #[test]
    fn hook_input_accepts_the_copilot_camel_case_session_id() {
        let parsed = HookInput::parse(
            b"{\"sessionId\":\"abc-1\",\"timestamp\":1760000000000,\"cwd\":\"/x\",\"source\":\"new\",\"initialPrompt\":null}",
        );
        assert_eq!(parsed.session_id.as_deref(), Some("abc-1"));
        assert_eq!(parsed.source.as_deref(), Some("new"));
        assert_eq!(parsed.claim_name().as_deref(), Some("abc-1.new"));
        let unsafe_id = HookInput::parse(b"{\"sessionId\":\"../x\"}");
        assert!(unsafe_id.claim_name().is_none());
    }

    #[test]
    fn fetched_context_names_the_path_and_the_next_read() {
        let path = PathBuf::from("/s/ledger");
        let context = fetched_context(
            &path,
            "Read /s/ledger/index.md next.",
            &no_pending(),
            "Next.",
        );
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
        let context = fetched_context(&path, "Read /s/ledger/index.md next.", &pending, "Next.");
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
            thread_message(Some("worktree:1234abcd"), UNBOUND_PROMPT),
            "An existing thread binding was found: worktree:1234abcd. Run /tsk:resume-thread 1234abcd next."
        );
        assert_eq!(thread_message(None, UNBOUND_PROMPT), UNBOUND_PROMPT);
        assert_eq!(thread_message(None, "Pick."), "Pick.");
    }

    #[test]
    fn no_mission_context_gives_the_latest_whats_next_and_the_attach_command() {
        assert_eq!(
            no_mission_context(Some("Review the draft")),
            "This thread has no mission, so it has no tasks. Its latest what's next: Review the draft. To attach a mission, run `tsk thread set mission <mission-id>`."
        );
        assert_eq!(
            no_mission_context(Some("Done.")),
            "This thread has no mission, so it has no tasks. Its latest what's next: Done. To attach a mission, run `tsk thread set mission <mission-id>`."
        );
        assert_eq!(
            no_mission_context(None),
            "This thread has no mission, so it has no tasks. Its latest what's next: none yet. To attach a mission, run `tsk thread set mission <mission-id>`."
        );
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
    fn upgrade_warning_names_the_manual_command() {
        let warning = upgrade_warning("error: curl failed\nexit 22");
        assert!(warning.contains("(error: curl failed exit 22)"));
        assert!(warning.ends_with("Run `tsk self-update` manually."));
    }

    #[test]
    fn failure_context_asks_for_a_manual_fetch() {
        let context = failure_context("error: git fetch failed\nfatal: no remote");
        assert!(context.contains("error: git fetch failed fatal: no remote"));
        assert!(context.contains("Run `tsk ledger fetch` manually"));
    }
}
