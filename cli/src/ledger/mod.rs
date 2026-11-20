pub mod absent;
pub mod background;
pub mod clone_lock;
pub mod doc_hook;
pub mod doc_id;
pub mod docs;
pub mod dolt;
pub mod emails;
pub mod erase;
pub mod fetch;
pub mod git;
pub mod guard;
pub mod home;
pub mod keep_session;
pub mod location;
pub mod manifest;
pub mod migrate_docs;
pub mod migrate_home;
pub mod migrate_transport;
pub mod nexus;
pub mod nexus_edit;
pub mod private_email;
pub mod push;
pub mod refs;
pub mod refs_cmd;
pub mod register;
pub mod remote_move;
pub mod status;
pub mod store;
pub mod tips;
pub mod transport;

use clap::Subcommand;
use std::io::{Read, Write};
use std::path::PathBuf;

use store::{LedgerStore, PushReport, RefreshOutcome};

#[derive(Subcommand)]
pub enum LedgerCommands {
    #[command(
        about = "Fetch the ledger branch and materialise it at the fixed worktree path, refreshing in place; prints the path"
    )]
    Fetch,
    #[command(about = "Print the ledger worktree path without fetching or writing anything")]
    Path,
    #[command(
        about = "Create if absent and print the scratch directory: a local place beside the ledger store for input documents (transcripts, chats, web pages) that stay out of Dolt, the documents branch and source control"
    )]
    Scratch,
    #[command(
        about = "Print the ledger's URN, urn:tsk:ledger:<ledger-id>, from the manifest key ledger_id; exit status 1 on a ledger below layout 12, which has none until its next write"
    )]
    Id,
    #[command(
        about = "Commit every change in the ledger worktree, rebase onto the latest ledger branch, and push it with a bounded compare and swap retry; prints the commit the branch holds"
    )]
    Push {
        #[arg(
            help = "Commit message for the worktree changes; with --background it defaults to \"Background publish\"",
            required_unless_present = "background"
        )]
        message: Option<String>,
        #[arg(
            long,
            help = "Publish in the background: exit 0 at once with no output when nothing is unpushed or a push already runs, else start a detached one-shot child that runs this push and writes background-push.result. A failure shows as a warning on the next tsk command"
        )]
        background: bool,
    },
    #[command(
        about = "The PreToolUse hook check of ADR 0028: reads the hook input on stdin and prints a deny decision for a tool call that edits a file in the ledger store, or runs git or dolt against the ledger store, its refs or its branches. Prints nothing otherwise and always exits 0. Accepts the Claude input (tool_name, tool_input) and the Copilot input (toolName, toolArgs as an object or a JSON string), and prints the decision in the --format shape (claude by default, or copilot: a top-level permissionDecision and permissionDecisionReason). TSK_LEDGER_GUARD=off in the environment of the hook switches it off"
    )]
    Guard {
        #[arg(
            long,
            value_enum,
            default_value = "claude",
            help = "Hook output format: claude (hookSpecificOutput JSON) or copilot (top-level permissionDecision JSON)"
        )]
        format: crate::thread::session_start::HookFormat,
    },
    #[command(
        name = "background-push-run",
        hide = true,
        about = "The child of tsk ledger push --background. Not for direct use"
    )]
    BackgroundPushRun {
        #[arg(long, help = "The inherited descriptor that holds the push lock")]
        lock_fd: i32,
        message: String,
    },
    #[command(
        about = "Print two answers, Local (what is not pushed, with a line per changed table or file, and one line for a schema upgrade) and Origin (whether it is ahead or behind); --porcelain prints tab-separated facts instead; fetches the remote refs and changes nothing else"
    )]
    Status {
        #[arg(
            long,
            help = "Print one tab-separated fact per line, the stable form for scripts, instead of the Local and Origin summary"
        )]
        porcelain: bool,
        #[arg(
            long,
            help = "Do not ask origin anything: print the Local block from what was last fetched, and the Origin line as not checked. No network"
        )]
        local: bool,
    },
    #[command(
        about = "Create the ledger of a repo that has none, and push it: in the attached nexus (--nexus, which first adds the repo to nexus.json with ledger nexus) or in the repo itself (--repo; with a nexus attached, the repo is added with ledger repo). Refuses when this clone or its remote holds a ledger already: tsk ledger status names the next action. Prints the path of the ledger store"
    )]
    Init {
        #[arg(
            long,
            conflicts_with = "repo",
            required_unless_present = "repo",
            help = "Hold the ledger in the attached nexus"
        )]
        nexus: bool,
        #[arg(long, help = "Hold the ledger in this repo, on origin")]
        repo: bool,
        #[arg(
            long,
            value_name = "PREFIX",
            help = "The mission prefix of the new ledger, 3 or 4 capital letters. Required when a nexus is attached, which checks that no other repo uses it"
        )]
        prefix: Option<String>,
        #[arg(
            long,
            help = "Repo ID for the nexus entry; defaults to the repo name, lowercased, with other characters as hyphens"
        )]
        id: Option<String>,
        #[arg(
            long,
            value_name = "TERRITORY_ID",
            help = "Territory of the nexus entry; required when the nexus has more than one"
        )]
        territory: Option<String>,
        #[arg(
            long,
            value_name = "NAME",
            requires = "territory",
            help = "Name of a territory the command creates; defaults to its id. An existing territory keeps its name"
        )]
        territory_name: Option<String>,
        #[arg(
            long,
            value_name = "WORKFLOW",
            value_parser = crate::harness::push::parse_flag,
            help = "Push workflow of the repo, main or pr, written to the workflow field of its nexus entry. Without it the command asks on a terminal and saves nothing elsewhere"
        )]
        workflow: Option<crate::harness::push::PushWorkflow>,
    },
    #[command(
        about = "Record that this repo works without tsk: in the tsk field of its nexus entry when the attached nexus lists the repo, else in .git/tsk-not-used of the clone. Session start then asks no question and the stop guard blocks nothing. --forget removes the record from both places. tsk ledger init removes the file too"
    )]
    NotUsed {
        #[arg(long, help = "Remove the record")]
        forget: bool,
    },
    #[command(
        about = "Drop every local change that is not pushed, in the data and the documents, and reset both to origin's state, except this session's thread binding and robot; without --yes it prints what it would drop and exits 1"
    )]
    Discard {
        #[arg(
            long,
            help = "Discard now. Without it nothing changes and the changes are printed"
        )]
        yes: bool,
        #[arg(
            long,
            help = "Print the changes as tab-separated lines, the stable form for scripts, instead of the Local summary"
        )]
        porcelain: bool,
    },
    #[command(
        about = "Remove a documents commit, and every file it added or changed, or with --file every version of one file, from the whole documents history, and force-push the rewritten branch; without --yes it prints what it would do and exits 1",
        long_about = "Remove a documents commit, and every file it added or changed, from the whole documents history, and force-push the rewritten branch. With --file PATH it removes every version of that one file instead.\n\nEvery file the commit added or changed is removed from every commit of the documents branch, its earlier and later versions included, and the commit itself is dropped. A commit that becomes empty is dropped too. The rewritten branch gets a marker file, .tsk-erased, that names the erased commit, so a clone that still holds it replaces its copy at its next fetch or push, or stops when it holds local work. Erasing covers the documents only, not the Dolt data.\n\nThe documents worktree must be clean and at the remote tip. Without --yes the command prints the commits and files it would rewrite and exits 1."
    )]
    Erase {
        #[arg(
            help = "The documents commit to erase, as a commit id (tsk ledger doc log lists them)",
            required_unless_present = "file",
            conflicts_with = "file"
        )]
        commit: Option<String>,
        #[arg(
            long,
            value_name = "PATH",
            help = "Erase every version of this file in place of a commit: a document path, with or without its id"
        )]
        file: Option<String>,
        #[arg(
            long,
            help = "Rewrite and force-push now. Without it nothing changes and the plan is printed"
        )]
        yes: bool,
    },
    #[command(
        about = "List the author and committer emails in the history of the ledger data ref and the documents ref of origin, with the number of commits of each. Fetches both refs. With --porcelain: ref, email, authored, committed, tab-separated",
        long_about = "List the author and committer emails in the git history of the two refs of the ledger on origin: the ledger data ref and the documents ref. Each email has the number of commits it authored and the number it committed. The command fetches both refs and changes nothing else.\n\nThe emails of the Dolt commits inside the data are not listed: they are in the table files, not in git.\n\nWith --porcelain each line is: ref, email, authored, committed, separated by tabs. tsk ledger rewrite-emails replaces them."
    )]
    Emails {
        #[arg(
            long,
            help = "One tab-separated line per ref and email: ref, email, authored, committed"
        )]
        porcelain: bool,
    },
    #[command(
        name = "rewrite-emails",
        about = "Rewrite the author and committer emails in the history of the ledger data ref and the documents ref to a noreply address, re-sign each rewritten commit with the robot key, and leave the result in the local ledger clone. Pushes nothing: tsk ledger push-rewritten does. Without --yes it prints the plan and exits 1",
        long_about = "Rewrite the author and committer emails in the git history of the ledger data ref and the documents ref to the noreply address given by --to. Names, dates, messages and trees stay. Every commit that changes, and every commit after it, gets a new id.\n\nWithout --from every email that is not a noreply address is replaced. With --from only the addresses named are replaced, whatever their case. Each rewritten commit is signed with the robot key when [attribution] identity is robot, and unsigned with a note otherwise. The documents ref gets the marker file .tsk-erased, so a clone that holds the old history replaces its copy at its next fetch or push, or stops when it holds local documents.\n\nThis command rewrites local refs of the ledger clone (refs/tsk-rewrite/) only. It prints the clones on this machine that hold the old commits, the old and new tip of each ref and the exact git push. tsk ledger push-rewritten --yes then force-pushes with a lease on each old tip. An agent stops and asks before that push.\n\nThe emails of the Dolt commits inside the data and the Co-authored-by lines of messages stay as they are. Without --yes the command prints the plan, creates no commit and exits 1."
    )]
    RewriteEmails {
        #[arg(
            long,
            value_name = "ADDRESS",
            help = "The noreply address that replaces the emails, for example 12345+name@users.noreply.github.com"
        )]
        to: String,
        #[arg(
            long,
            value_name = "EMAIL",
            help = "Replace only this email, whatever its case; repeat for several. Without it every email that is not a noreply address is replaced"
        )]
        from: Vec<String>,
        #[arg(
            long,
            help = "Rewrite the local refs now. Without it nothing changes and the plan is printed"
        )]
        yes: bool,
    },
    #[command(
        name = "push-rewritten",
        about = "Force-push the refs that tsk ledger rewrite-emails wrote, each with a lease on the tip it replaces, then prune the old commits; without --yes it prints the push and exits 1",
        long_about = "Force-push the data ref and the documents ref that tsk ledger rewrite-emails wrote, in one atomic git push, each with --force-with-lease on the tip it replaced. When origin moved since the rewrite, git refuses, nothing changes on origin, and the rewrite stays local: fetch, then run tsk ledger rewrite-emails again.\n\nAfter the push the documents worktree and the ledger directory move to the new history, the old commits are pruned from the ledger clone, and from origin too when origin is a directory on this machine. The documents worktree must be clean and at the tip the rewrite started from.\n\nWithout --yes the command prints the git push and exits 1. An agent runs it only after the person agrees."
    )]
    PushRewritten {
        #[arg(
            long,
            help = "Force-push now. Without it nothing changes and the push is printed"
        )]
        yes: bool,
    },
    #[command(
        name = "migrate-docs",
        about = "Move the documents of a version 1 Dolt ledger from the documents table to the tsk/docs branch; prints migrated-docs:<count>"
    )]
    MigrateDocs {
        #[arg(
            long,
            help = "Do everything except the drop and the pushes: write and check the files, print the counts and dry-run:ok"
        )]
        dry_run: bool,
    },
    #[command(
        name = "migrate-transport",
        about = "Convert a ledger whose ref holds Dolt's blobstore to a Dolt file remote carried by plain git; prints migrated-transport:file-remote and the commit the ref now holds"
    )]
    MigrateTransport,
    #[command(
        about = "Read, write, list and remove ledger documents in the local store; none of them fetches or pushes"
    )]
    Doc {
        #[command(subcommand)]
        action: DocCommands,
    },
    #[command(
        about = "Inspect and control the local Dolt server of this clone's ledger store (the switch TSK_DOLT_SERVER or [ledger] server turns its use on; see ADR 0018)"
    )]
    Server {
        #[command(subcommand)]
        action: store::server::ServerCommands,
    },
}

#[derive(Subcommand)]
pub enum DocCommands {
    #[command(
        about = "Print the absolute path of a document's file, for opening it in another tool; exit status 1 when it does not exist"
    )]
    Path {
        #[arg(
            help = "Document path relative to the ledger root, with or without its id, or the id alone"
        )]
        path: String,
    },
    #[command(
        about = "Print a document exactly as stored, with no added newline; exit status 1 when it does not exist"
    )]
    Get {
        #[arg(
            help = "Document path relative to the ledger root, with or without its id, or the id alone"
        )]
        path: String,
    },
    #[command(
        about = "Write a document in the local store from a file or stdin, and print its path. A new path gets an 8 character id before its extension (report.md is stored as report.3f2a9c1e.md); a path that names an existing document, with or without the id, rewrites it. Does not push"
    )]
    Put {
        #[arg(help = "Document path relative to the ledger root, with or without its id")]
        path: String,
        #[arg(help = "File holding the body; reads stdin when omitted")]
        file: Option<PathBuf>,
    },
    #[command(
        about = "List the documents whose path starts with a prefix, sorted: the path, then the id (- for a file with none). With --porcelain the two are separated by a tab and the id is empty for a file with none"
    )]
    List {
        #[arg(help = "Path prefix, for example missions/; lists every document when omitted")]
        prefix: Option<String>,
        #[arg(long, help = "Separate the path and the id with a tab, for a script")]
        porcelain: bool,
    },
    #[command(
        about = "List the commits of the local documents branch, newest first: the short id, the date, the author and the subject, tab-separated. With a path, only the commits that changed that file. Does not fetch. tsk ledger erase takes an id from this list"
    )]
    Log {
        #[arg(
            help = "Document path relative to the ledger root, with or without its id, or the id alone; lists every commit when omitted"
        )]
        path: Option<String>,
    },
    #[command(
        about = "Remove a document from the local store; exit status 1 when it does not exist"
    )]
    Rm {
        #[arg(
            help = "Document path relative to the ledger root, with or without its id, or the id alone"
        )]
        path: String,
    },
    #[command(
        about = "Rename a document in the local store, keep its id, and update the path in the link of every mission that links it. Prints doc FROM -> TO and one link line for each mission. Exit status 1 when the document does not exist or the new path does. Does not push"
    )]
    Mv {
        #[arg(help = "The document path now")]
        from: String,
        #[arg(help = "The new document path")]
        to: String,
    },
}

pub fn run(action: LedgerCommands) -> Result<(), String> {
    if let LedgerCommands::Guard { format } = action {
        return guard::run(format);
    }
    crate::config::refuse_files_backend_from_env()?;
    let cwd = std::env::current_dir()
        .map_err(|e| format!("error: could not read the current directory: {}", e))?;
    if let LedgerCommands::MigrateDocs { dry_run } = action {
        return migrate_docs::run(&cwd, dry_run);
    }
    if let LedgerCommands::MigrateTransport = action {
        return migrate_transport::run(&cwd);
    }
    if let LedgerCommands::Server { action } = action {
        return store::server::run_command(action, &cwd);
    }
    if let LedgerCommands::BackgroundPushRun { lock_fd, message } = action {
        return background::run_child(&cwd, lock_fd, &message);
    }
    if matches!(action, LedgerCommands::Push { .. }) {
        // A push reports its own outcome; an earlier failure is not news to it.
        background::be_quiet();
        // A move to the new ref names that failed is tried again at a push.
        remote_move::request_retry();
    }
    if matches!(action, LedgerCommands::Fetch) {
        remote_move::request_full_check();
    }
    if let LedgerCommands::Init {
        nexus,
        prefix,
        id,
        territory,
        territory_name,
        workflow,
        ..
    } = action
    {
        return run_init(
            &cwd,
            nexus,
            InitArgs {
                prefix,
                id,
                territory,
                territory_name,
                workflow,
            },
        );
    }
    if let LedgerCommands::NotUsed { forget } = action {
        return run_not_used(&cwd, forget);
    }
    if let LedgerCommands::Status { porcelain, local } = action {
        if let Some(report) = absent::report(&cwd, local)? {
            if !porcelain
                && location::Repo::discover(&cwd).is_ok_and(|repo| nexus::marked_not_used(&repo))
            {
                println!("Local   marked as not under tsk: tsk ledger not-used --forget removes the mark");
            }
            let lines = if porcelain {
                report.porcelain_lines(local)
            } else {
                report.human_lines(local)
            };
            for line in lines {
                println!("{}", line);
            }
            return Ok(());
        }
    }
    let store = store::open(&cwd)?;
    match action {
        LedgerCommands::Fetch => {
            let outcome = report_refresh(store.refresh_for_fetch()?);
            ensure_scratch(&outcome.path)?;
            if !outcome.unpublished {
                if let Ok(session) = crate::thread::session::Session::discover() {
                    if let Err(e) = crate::sessions::ensure_recorded_in_cloud(&session) {
                        eprintln!("warning: the session is not recorded: {}", e);
                    }
                }
            }
            println!("{}", outcome.path.display());
            Ok(())
        }
        LedgerCommands::Scratch => {
            let scratch = ensure_scratch(&store.path()?)?;
            println!("{}", scratch.display());
            Ok(())
        }
        LedgerCommands::Path => {
            println!("{}", store.path()?.display());
            Ok(())
        }
        LedgerCommands::Id => match store.ledger_id()? {
            Some(id) => {
                println!("{}", crate::urn::ledger_urn(&id));
                Ok(())
            }
            None => Err(
                "error: this ledger has no ledger_id yet: it is below layout 12, and its next write records one"
                    .to_string(),
            ),
        },
        LedgerCommands::Push {
            message,
            background: true,
        } => {
            let message = message.unwrap_or_else(|| background::DEFAULT_MESSAGE.to_string());
            background::start(store.as_ref(), &message)?;
            Ok(())
        }
        LedgerCommands::Push {
            message,
            background: false,
        } => {
            let message = message.unwrap_or_default();
            // A push does not ask origin first. The record of the last fetch stands in for
            // what origin holds, and a push that finds origin moved is rejected and
            // retried after a fetch.
            store.trust_last_fetch();
            let report = push_reporting(store.as_ref(), &message)?;
            println!("{}", report.head);
            Ok(())
        }
        LedgerCommands::Status { porcelain, local } => {
            let status = if local {
                store.status_local()?
            } else {
                store.status()?
            };
            if let Some(note) = &status.fetch_note {
                eprintln!("{}", note);
            }
            let lines = if porcelain {
                status.lines()
            } else {
                status.human_lines()
            };
            for line in lines {
                println!("{}", line);
            }
            if porcelain {
                if let Some(value) = workflow_value() {
                    println!("workflow\t{}", value.as_deref().unwrap_or("-"));
                }
            }
            if !porcelain {
                if let Ok(path) = store.path() {
                    println!("{}", server_line(&path));
                    if let Some(line) = crate::signing::status_line() {
                        println!("{}", line);
                    }
                    if let Some(line) = background::status_line(&path) {
                        println!("{}", line);
                    }
                }
                for line in ledger_block(store.as_ref()) {
                    println!("{}", line);
                }
            }
            Ok(())
        }
        LedgerCommands::Discard { yes, porcelain } => run_discard(store.as_ref(), yes, porcelain),
        LedgerCommands::Erase { commit, file, yes } => {
            let target = match (commit, file) {
                (_, Some(path)) => erase::Target::File(path),
                (Some(commit), None) => erase::Target::Commit(commit),
                (None, None) => {
                    return Err("error: name a commit, or a file with --file".to_string())
                }
            };
            let (lines, done) = store.erase(&target, yes)?;
            for line in lines {
                println!("{}", line);
            }
            if done {
                Ok(())
            } else {
                Err("error: this would rewrite the documents history above and force-push it. Rerun with --yes to erase.".to_string())
            }
        }
        LedgerCommands::Emails { porcelain } => {
            let refs = store.emails()?;
            let lines = if porcelain {
                emails::porcelain_lines(&refs)
            } else {
                emails::listing_lines(&refs)
            };
            for line in lines {
                println!("{}", line);
            }
            Ok(())
        }
        LedgerCommands::RewriteEmails { to, from, yes } => {
            let selection = emails::Selection::new(&to, &from)?;
            let (lines, done) = store.rewrite_emails(&selection, yes)?;
            for line in lines {
                println!("{}", line);
            }
            if done {
                Ok(())
            } else {
                Err("error: this would rewrite the ledger history above in the local ledger clone. Rerun with --yes to rewrite. Nothing is pushed until tsk ledger push-rewritten --yes.".to_string())
            }
        }
        LedgerCommands::PushRewritten { yes } => {
            let (lines, done) = store.push_rewritten(yes)?;
            for line in lines {
                println!("{}", line);
            }
            if done {
                Ok(())
            } else {
                Err("error: this would force-push the rewritten history above. Rerun with --yes to push.".to_string())
            }
        }
        LedgerCommands::Doc { action } => run_doc(store.as_ref(), action),
        LedgerCommands::MigrateDocs { .. }
        | LedgerCommands::Init { .. }
        | LedgerCommands::MigrateTransport
        | LedgerCommands::Guard { .. }
        | LedgerCommands::NotUsed { .. }
        | LedgerCommands::BackgroundPushRun { .. }
        | LedgerCommands::Server { .. } => {
            unreachable!("handled before the store opens")
        }
    }
}

/// The `Ledger` block of `tsk ledger status`: the ledger store path, the refs in use and
/// what the store holds. A part that cannot be read is left out.
fn ledger_block(store: &dyn LedgerStore) -> Vec<String> {
    let mut lines = Vec::new();
    let label = |text: String| format!("{:<8}{}", "Ledger", text);
    let pad = |text: String| format!("{:<8}{}", "", text);
    match store.path() {
        Ok(path) => lines.push(label(format!("store {}", path.display()))),
        Err(_) => return lines,
    }
    if let Ok(text) = refs_cmd::summary() {
        lines.push(pad(text));
    }
    if let Some(text) = refs_cmd::warning() {
        lines.push(pad(text));
    }
    if let Some(text) = workflow_line() {
        lines.push(pad(text));
    }
    // A move of the nexus refs to the new names that did not finish (ADR 0035).
    if let Some(root) = path_of_home(store) {
        if let Ok(text) = std::fs::read_to_string(root.join(remote_move::LAST_FILE)) {
            let why = text
                .lines()
                .nth(1)
                .unwrap_or("")
                .trim_start_matches("error: ");
            lines.push(pad(format!(
                "the nexus refs are not at the new names yet; tsk tries again at the next push: {}",
                why
            )));
        }
    }
    let count = |n: usize, noun: &str| format!("{} {}{}", n, noun, if n == 1 { "" } else { "s" });
    if let Ok(missions) = store.missions_load() {
        let tasks: usize = missions.iter().map(|m| m.tasks.len()).sum();
        let threads = store.thread_ids().map(|t| t.len()).unwrap_or(0);
        let actors = store.actors_list().map(|a| a.len()).unwrap_or(0);
        lines.push(pad(format!(
            "holds {}, {}, {}, {}",
            count(missions.len(), "mission"),
            count(tasks, "task"),
            count(threads, "thread"),
            count(actors, "actor")
        )));
    }
    lines
}

/// The ledger home that holds the store, from the path of the store.
fn path_of_home(store: &dyn LedgerStore) -> Option<PathBuf> {
    store.path().ok()?.parent().map(PathBuf::from)
}

/// The `Server` line of `tsk ledger status`: whether the local Dolt server runs, and the
/// switch. Asks origin nothing.
/// Create the scratch directory beside the ledger store, if it is absent, and return it.
fn ensure_scratch(db_path: &std::path::Path) -> Result<PathBuf, String> {
    let scratch = location::scratch_path(db_path);
    std::fs::create_dir_all(&scratch)
        .map_err(|e| format!("error: cannot create {}: {e}", scratch.display()))?;
    Ok(scratch)
}

fn server_line(db: &std::path::Path) -> String {
    let status = store::server::status(db);
    let text = match (status.state, &status.info) {
        ("running", Some(info)) => format!(
            "running (pid {}, idle stop {} s)",
            info.dolt.unwrap_or(info.supervisor),
            info.idle
        ),
        (state, _) => {
            let (on, _) = store::server::switch();
            format!("{} (switch {})", state, if on { "on" } else { "off" })
        }
    };
    format!("{:<8}{}", "Server", text)
}

/// `tsk ledger discard`. Without `yes` it prints the local changes and refuses.
fn run_discard(store: &dyn LedgerStore, yes: bool, porcelain: bool) -> Result<(), String> {
    if !yes {
        let status = store.status()?;
        if let Some(note) = &status.fetch_note {
            eprintln!("{}", note);
        }
        let lines = if porcelain {
            status.changes_lines()
        } else {
            status.human_local()
        };
        for line in lines {
            println!("{}", line);
        }
        if status.is_clean() {
            return Ok(());
        }
        return Err(
            "error: this would discard the local changes above. Rerun with --yes to discard them."
                .to_string(),
        );
    }
    let kept = keep_session::capture(store);
    store.discard()?;
    println!("discarded");
    for note in keep_session::restore(store, &kept) {
        eprintln!("{}", note);
    }
    Ok(())
}

fn no_such_document(path: &str) -> String {
    format!("error: no such document: {}", path)
}

fn run_doc(store: &dyn LedgerStore, action: DocCommands) -> Result<(), String> {
    match action {
        DocCommands::Path { path } => {
            let file = store
                .doc_path(&path)?
                .ok_or_else(|| no_such_document(&path))?;
            let file = std::fs::canonicalize(&file).unwrap_or(file);
            println!("{}", file.display());
            Ok(())
        }
        DocCommands::Get { path } => {
            let body = store
                .doc_get(&path)?
                .ok_or_else(|| no_such_document(&path))?;
            let mut out = std::io::stdout();
            out.write_all(body.as_bytes())
                .and_then(|()| out.flush())
                .map_err(|e| format!("error: could not write to stdout: {}", e))
        }
        DocCommands::Put { path, file } => {
            let body = match file {
                Some(file) => std::fs::read_to_string(&file)
                    .map_err(|e| format!("error: could not read {}: {}", file.display(), e))?,
                None => {
                    let mut text = String::new();
                    std::io::stdin()
                        .read_to_string(&mut text)
                        .map_err(|e| format!("error: could not read stdin: {}", e))?;
                    text
                }
            };
            println!("{}", store.doc_put(&path, &body)?);
            Ok(())
        }
        DocCommands::List { prefix, porcelain } => {
            let paths = store.doc_list(prefix.as_deref().unwrap_or(""))?;
            let width = paths.iter().map(String::len).max().unwrap_or(0);
            for path in paths {
                let id = self::doc_id::id_of(&path);
                if porcelain {
                    println!("{}\t{}", path, id.unwrap_or_default());
                } else {
                    println!("{:width$}  {}", path, id.as_deref().unwrap_or("-"));
                }
            }
            Ok(())
        }
        DocCommands::Rm { path } => {
            if store.doc_remove(&path)? {
                Ok(())
            } else {
                Err(no_such_document(&path))
            }
        }
        DocCommands::Log { path } => {
            for line in store.doc_log(path.as_deref())? {
                println!("{}", line);
            }
            Ok(())
        }
        DocCommands::Mv { from, to } => crate::mission::mv::doc_mv(store, &from, &to),
    }
}

pub fn fetch_reporting(store: &dyn LedgerStore) -> Result<RefreshOutcome, String> {
    Ok(report_refresh(store.refresh()?))
}

/// [`fetch_reporting`] for the thread commands: local changes are kept, not refused.
pub fn fetch_keeping_local_reporting(store: &dyn LedgerStore) -> Result<RefreshOutcome, String> {
    Ok(report_refresh(store.refresh_keeping_local()?))
}

/// Print the notes of a refresh to stderr and hand the outcome back.
pub(crate) fn report_refresh(outcome: RefreshOutcome) -> RefreshOutcome {
    if let Some(note) = &outcome.replaced_note {
        eprintln!("{}", note);
    }
    if let Some(note) = &outcome.pending_note {
        eprintln!("{}", note);
    }
    if let Some(note) = &outcome.unpublished_note {
        eprintln!("{}", note);
        if let Some(cloud) = refs::cloud_default_note() {
            eprintln!("{}", cloud);
        }
    }
    outcome
}

pub fn push_reporting(store: &dyn LedgerStore, message: &str) -> Result<PushReport, String> {
    let report = store.commit_and_push(message, &mut |note| eprintln!("{}", note))?;
    if let Ok(path) = store.path() {
        background::clear_failure(&path);
    }
    for line in &report.renumbered {
        println!("{}", line);
    }
    Ok(report)
}

fn entry_prefix(cwd: &std::path::Path, url: &str) -> Option<String> {
    let repo = location::Repo::discover(cwd).ok()?;
    let root = location::state_root_from_env().ok()?;
    let index = nexus::load_index(&root, url).ok()?;
    let cached = repo.read_repo_id().ok()?;
    let origin = repo.raw_origin_url().ok()?;
    nexus::select_entry(
        &index,
        cached.as_deref(),
        origin.as_deref(),
        nexus::machine_name().as_deref(),
    )?
    .prefix
    .clone()
}

fn nexus_entry_of(repo: &location::Repo) -> Option<(String, String)> {
    let file = crate::config::config_file_from_env()?;
    let url = crate::config::read_nexus_url(&file).ok().flatten()?;
    let root = location::state_root_from_env().ok()?;
    let index = nexus::load_index(&root, &url).ok()?;
    let cached = repo.read_repo_id().ok()?;
    let origin = repo.raw_origin_url().ok()?;
    let entry = nexus::select_entry(
        &index,
        cached.as_deref(),
        origin.as_deref(),
        nexus::machine_name().as_deref(),
    )?;
    Some((url, entry.id.clone()))
}

fn set_entry_not_used(url: &str, id: &str, value: Option<&str>) -> Result<bool, String> {
    let root = location::state_root_from_env()?;
    let published = nexus_edit::set_repo_tsk(&root, url, id, value, &mut || {})?;
    Ok(matches!(published, nexus_edit::Published::Changed { .. }))
}

fn run_not_used(cwd: &std::path::Path, forget: bool) -> Result<(), String> {
    let repo = location::Repo::discover(cwd)?;
    let entry = nexus_entry_of(&repo);
    if forget {
        let mut removed = Vec::new();
        if let Some((url, id)) = &entry {
            if set_entry_not_used(url, id, None)? {
                removed.push(format!("the tsk field of the nexus entry {}", id));
            }
        }
        if repo.clear_not_used()? {
            removed.push(repo.not_used_file().display().to_string());
        }
        if removed.is_empty() {
            println!("nothing to remove: this clone is not marked as not under tsk");
        } else {
            println!("removed {}", removed.join(" and "));
        }
        return Ok(());
    }
    match entry {
        Some((url, id)) => {
            set_entry_not_used(&url, &id, Some(nexus::NOT_USED))?;
            println!(
                "wrote tsk: not-used to the nexus entry {}: this repo works without tsk on every machine. To undo: tsk ledger not-used --forget",
                id
            );
        }
        None => {
            repo.write_not_used()?;
            println!(
                "wrote {}: this clone works without tsk. To undo: tsk ledger not-used --forget",
                repo.not_used_file().display()
            );
        }
    }
    Ok(())
}

struct InitArgs {
    prefix: Option<String>,
    id: Option<String>,
    territory: Option<String>,
    territory_name: Option<String>,
    workflow: Option<crate::harness::push::PushWorkflow>,
}

/// `tsk ledger init`: create the ledger where the person chose, after the checks that
/// `tsk ledger status` reports, and push it.
fn run_init(cwd: &std::path::Path, want_nexus: bool, args: InitArgs) -> Result<(), String> {
    use crate::harness::push::{self, InitWorkflow};
    use absent::{Next, NexusState};
    use std::io::IsTerminal;
    let InitArgs {
        prefix,
        id,
        territory,
        territory_name,
        workflow,
    } = args;
    let report = absent::report(cwd, false)?.ok_or_else(|| {
        "error: this clone holds a ledger already; tsk ledger status shows it".to_string()
    })?;
    match report.next {
        Next::Init => {}
        Next::Fetch => {
            return Err(
                "error: a ledger exists already; run tsk ledger fetch to read it".to_string(),
            )
        }
        Next::AttachNexus | Next::CheckAccess => {
            return Err(format!(
                "error: tsk cannot tell yet whether a ledger exists. {}",
                report.human_lines(false).join("\n")
            ))
        }
    }
    if want_nexus && report.nexus_url.is_none() {
        return Err(
            "error: no nexus is attached; run tsk nexus add <url> first, or use --repo".to_string(),
        );
    }
    let robot = crate::attribution::robot_for_init()?;
    if let Some(prefix) = &prefix {
        crate::mission::prefix::check_nexus(prefix)?;
    }
    let mut pending_workflow = None;
    match &report.nexus {
        NexusState::NoEntry => {
            let prefix = prefix.clone().ok_or_else(|| {
                "error: --prefix is required: adding the repo to the nexus needs the mission prefix of its ledger, 3 or 4 capital letters that no other repo in the nexus uses".to_string()
            })?;
            let choice = if want_nexus {
                crate::nexus_cmd::LedgerChoice::Nexus
            } else {
                crate::nexus_cmd::LedgerChoice::Repo
            };
            let file = crate::config::config_file_from_env()
                .ok_or_else(|| "error: could not locate the user config file".to_string())?;
            let chosen =
                match push::init_workflow(workflow, None, std::io::stdin().is_terminal(), true) {
                    InitWorkflow::Use(chosen) => Some(chosen),
                    InitWorkflow::Ask => Some(crate::harness::ask_push()?),
                    InitWorkflow::Skip => None,
                };
            crate::nexus_cmd::register_for_init(
                &file,
                id,
                territory,
                territory_name,
                choice,
                prefix,
                chosen.map(|w| w.as_str().to_string()),
            )?;
        }
        NexusState::Entry { in_nexus, .. } if *in_nexus != want_nexus => {
            return Err(format!(
                "error: the nexus entry for this repo says its ledger is in the {}; run tsk ledger init --{}",
                if *in_nexus { "nexus" } else { "repo" },
                if *in_nexus { "nexus" } else { "repo" }
            ));
        }
        NexusState::Entry {
            id: entry_id,
            workflow: stored,
            ..
        } => {
            let chosen = match push::init_workflow(
                workflow,
                stored.as_deref(),
                std::io::stdin().is_terminal(),
                true,
            ) {
                InitWorkflow::Use(chosen) => Some(chosen),
                InitWorkflow::Ask => Some(crate::harness::ask_push()?),
                InitWorkflow::Skip => None,
            };
            pending_workflow = match (chosen, &report.nexus_url) {
                (Some(chosen), Some(url)) => Some((url.clone(), entry_id.clone(), chosen)),
                _ => None,
            };
        }
        _ => {}
    }
    if workflow.is_some() && report.nexus_url.is_none() {
        eprintln!("note: the workflow was not saved: no nexus is attached");
    }
    let prefix = match (prefix, &report.nexus, &report.nexus_url) {
        (Some(prefix), _, _) => Some(prefix),
        (None, NexusState::Entry { .. }, Some(url)) => entry_prefix(cwd, url),
        _ => None,
    };
    let store = store::open(cwd)?;
    let outcome = report_refresh(store.refresh_for_fetch()?);
    ensure_scratch(&outcome.path)?;
    if let Some(name) = &robot {
        register_init_robot(cwd, store.as_ref(), name)?;
    }
    if let Some(prefix) = &prefix {
        store.set_mission_prefix(prefix)?;
    }
    let pushed = push_reporting(store.as_ref(), "Create the ledger")?;
    if let Some((url, entry_id, chosen)) = pending_workflow {
        save_init_workflow(&url, &entry_id, chosen);
    }
    if let Ok(repo) = location::Repo::discover(cwd) {
        let _ = repo.clear_not_used();
    }
    eprintln!("{}", pushed.head);
    println!("{}", outcome.path.display());
    Ok(())
}

fn register_init_robot(
    cwd: &std::path::Path,
    store: &dyn LedgerStore,
    name: &str,
) -> Result<(), String> {
    let session = crate::thread::session::Session::discover_in(cwd)?;
    let (instance, instance_kind) = session.actor_key()?.ok_or_else(|| {
        "error: this clone has no clone ID, so the robot cannot be registered".to_string()
    })?;
    let urn = session.session_urn();
    crate::actor::register(
        store,
        &instance,
        instance_kind,
        urn.as_deref(),
        name,
        store::ActorKind::Robot,
        &crate::thread::clock::utc_now(),
    )?;
    eprintln!("registered the robot {} for this session", name);
    Ok(())
}

fn workflow_value() -> Option<Option<String>> {
    let cwd = std::env::current_dir().ok()?;
    let repo = location::Repo::discover(&cwd).ok()?;
    let root = location::state_root_from_env().ok()?;
    let config_file = crate::config::config_file_from_env();
    let lookup =
        crate::harness::push::nexus_lookup(&repo, &root, config_file.as_deref()).ok()??;
    lookup.repo_id.as_ref()?;
    Some(lookup.workflow)
}

fn workflow_line() -> Option<String> {
    workflow_value().map(|value| format!("workflow: {}", value.as_deref().unwrap_or("not set")))
}

fn save_init_workflow(url: &str, repo_id: &str, workflow: crate::harness::push::PushWorkflow) {
    let saved = match location::state_root_from_env() {
        Ok(root) => {
            nexus_edit::set_repo_workflow(&root, url, repo_id, workflow.as_str(), &mut || {})
        }
        Err(e) => Err(e),
    };
    match saved {
        Ok(_) => eprintln!(
            "saved the workflow {} to the nexus entry {}",
            workflow.as_str(),
            repo_id
        ),
        Err(e) => {
            let why = e.trim_start_matches("error: ").lines().next().unwrap_or("");
            eprintln!(
                "warning: the workflow was not saved ({}). Save it with: tsk nexus set-repo {} --workflow {}",
                why,
                repo_id,
                workflow.as_str()
            );
        }
    }
}

pub(crate) fn retry_note(attempt: usize, status: &push::PushStatus) -> String {
    format!(
        "note: push rejected (attempt {}/{}): {}; fetching again and retrying",
        attempt,
        push::MAX_ATTEMPTS,
        status.summary
    )
}

pub(crate) fn push_notes(outcome: &push::PushOutcome) -> Vec<String> {
    let mut notes = Vec::new();
    if !outcome.committed {
        notes.push(format!(
            "note: nothing to commit in {}; checking whether an earlier run left a commit to push",
            outcome.path.display()
        ));
    }
    if outcome.result == push::PushResult::AlreadyOnRemote {
        notes.push(format!(
            "note: {}'s {} already holds this worktree's state; nothing to push",
            outcome.location.label(),
            outcome.location.ref_name()
        ));
    }
    notes
}

pub(crate) fn unpublished_note(outcome: &fetch::FetchOutcome) -> String {
    unpublished_text(
        outcome.location.label(),
        outcome.location.ref_name(),
        &outcome.path,
    )
}

pub(crate) fn unpublished_text(label: &str, ref_name: &str, path: &std::path::Path) -> String {
    let kind = if ref_name.starts_with("refs/heads/") {
        "branch"
    } else {
        "ref"
    };
    format!(
        "note: {}'s {} does not exist yet; {} holds a new ledger.\n      The first tsk ledger push creates the {}",
        label,
        ref_name,
        path.display(),
        kind
    )
}

pub(crate) fn pending_note(outcome: &fetch::FetchOutcome) -> String {
    let location = &outcome.location;
    let mut note = format!(
        "note: {} holds a commit that is not on {}'s {}:\n",
        outcome.path.display(),
        location.label(),
        location.ref_name()
    );
    for line in &outcome.pending {
        note.push_str(&format!("        {}\n", line));
    }
    note.push_str("      Leaving the worktree as it is rather than resetting over it.\n");
    note.push_str(
        "      Push it with tsk ledger push. To see what would be lost, run tsk ledger status.\n",
    );
    note.push_str("      To drop it deliberately, run tsk ledger discard.");
    note
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn pending_note_names_each_commit_and_the_reset_command() {
        let outcome = fetch::FetchOutcome {
            location: location::LedgerLocation::in_repo(),
            path: PathBuf::from("/s/tsk/ledgers/x/ledger"),
            pending: vec!["1111111 first".to_string(), "2222222 second".to_string()],
            unpublished: false,
        };
        let note = pending_note(&outcome);
        assert!(note.starts_with("note: /s/tsk/ledgers/x/ledger holds a commit"));
        assert!(note.contains("refs/heads/tsk/ledger"));
        assert!(note.contains("        1111111 first\n"));
        assert!(note.contains("        2222222 second\n"));
        assert!(note.contains("run tsk ledger status"));
        assert!(note.ends_with("To drop it deliberately, run tsk ledger discard."));
        assert!(!note.contains("reset --hard"));
    }

    #[test]
    fn unpublished_note_names_the_ref_and_the_first_push() {
        let outcome = fetch::FetchOutcome {
            location: location::LedgerLocation::nexus("https://example.test/o/nexus", "work-api"),
            path: PathBuf::from("/s/tsk/ledgers/x/ledger"),
            pending: Vec::new(),
            unpublished: true,
        };
        let note = unpublished_note(&outcome);
        assert!(note.contains("the nexus's refs/heads/ledgers/work-api/ledger does not exist yet"));
        assert!(note.contains("/s/tsk/ledgers/x/ledger holds a new ledger"));
        assert!(note.ends_with("The first tsk ledger push creates the branch"));
    }

    fn push_outcome(committed: bool, result: push::PushResult) -> push::PushOutcome {
        push::PushOutcome {
            location: location::LedgerLocation::in_repo(),
            path: PathBuf::from("/s/tsk/ledgers/x/ledger"),
            committed,
            commit: "abc123".to_string(),
            result,
        }
    }

    #[test]
    fn push_notes_are_empty_after_a_commit_and_push() {
        let outcome = push_outcome(true, push::PushResult::Pushed { attempts: 1 });
        assert!(push_notes(&outcome).is_empty());
    }

    #[test]
    fn push_notes_name_nothing_to_commit_and_already_on_remote() {
        let outcome = push_outcome(false, push::PushResult::AlreadyOnRemote);
        let notes = push_notes(&outcome);
        assert_eq!(notes.len(), 2);
        assert!(notes[0].contains("nothing to commit in /s/tsk/ledgers/x/ledger"));
        assert!(notes[1].contains("origin's refs/heads/tsk/ledger already holds"));
    }

    #[test]
    fn retry_note_names_the_attempt_and_the_summary() {
        let status = push::PushStatus {
            flag: '!',
            summary: "[rejected] (stale info)".to_string(),
        };
        assert_eq!(
            retry_note(2, &status),
            "note: push rejected (attempt 2/10): [rejected] (stale info); fetching again and retrying"
        );
    }
}
