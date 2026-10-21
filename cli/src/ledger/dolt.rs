//! A runner for the external `dolt` binary, shaped like [`super::git::Git`], plus the
//! ledger's table definitions and the SQL literal helpers. tsk never links Dolt: every
//! operation is a `dolt` process.

use std::ffi::OsStr;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use serde_json::Value;

use crate::ledger::store::mysql::{Conn, Error as SqlError};
use crate::ledger::store::server::{self, Failure};
use crate::ledger::transport::{self, Transport};

/// The branch every Dolt ledger database uses locally and on its remote.
pub const BRANCH: &str = "main";
/// The name of the remote inside the Dolt database.
pub const REMOTE: &str = "origin";

/// Set to anything but empty or `0` to turn every batched read and write off. Each of them
/// then takes its older path, one `dolt` process per query, which gives the same result.
/// The conformance cases run with it set to check that the two paths agree.
pub const NO_BATCH_VAR: &str = "TSK_NO_BATCH";

/// False when [`NO_BATCH_VAR`] asks for the query-by-query paths.
pub fn batching() -> bool {
    std::env::var_os(NO_BATCH_VAR).is_none_or(|value| value.is_empty() || value == "0")
}

/// The Dolt remote branch name as `dolt_remote_branches` lists it.
pub const REMOTE_BRANCH_NAME: &str = "remotes/origin/main";

pub const NOT_INSTALLED: &str = "error: dolt is not installed.\n       \
     The SessionStart hook (ops/local/claude-session-start.sh) installs it; \
     otherwise download a release from https://github.com/dolthub/dolt/releases";

/// The ledger tables, in the one form every writer uses: a new ledger, and the migration
/// from a file ledger. There is no documents table: documents are files on the `tsk/docs`
/// branch from layout version 2. A version 1 ledger still holds the table
/// ([`LEGACY_DOCUMENTS_TABLE`]).
pub const SCHEMA: &[&str] = &[
    "create table manifest (k varchar(64) primary key, v text)",
    "create table threads (thread_id varchar(64) primary key, briefing_path varchar(1024) null, mission_id varchar(64) null)",
    "create table continuation_entries (thread_id varchar(64), seq int, entry longtext not null, primary key (thread_id, seq))",
    "create table bindings (actor varchar(255) primary key, kind enum('cloud','worktree') not null, thread_id varchar(64) not null, registered_at varchar(32) not null)",
    "create table external_events (position bigint primary key, envelope longtext not null)",
    "create table watermark (id int primary key, processed_through bigint not null, updated_at varchar(32) not null)",
];

/// The table a version 1 ledger holds its documents in. A new ledger does not create it.
/// Only `tsk ledger migrate-docs` reads it, with a plain `select path, body`, and drops it.
#[allow(dead_code)]
pub const LEGACY_DOCUMENTS_TABLE: &str =
    "create table documents (path varchar(1024) primary key, body longtext not null)";

/// The mission tables, created with a new ledger and by the first mission write on a
/// version 2 ledger. Foreign keys keep a record from naming a mission that does not exist.
/// `next_objective` is the number the next objective outcome gets: it only grows, so a
/// removed outcome's number is never reused. `next_constraint` and `next_out_of_scope`
/// do the same for the two lists in [`LIST_SCHEMA`].
pub const MISSION_SCHEMA: &[&str] = &[
    "create table missions (\
     id varchar(64) not null primary key, \
     name varchar(255) not null, \
     category enum('operational','administrative') not null, \
     status enum('todo','in-progress','done') not null default 'todo', \
     parent varchar(64) null, \
     purpose longtext null, \
     decision_authority longtext null, \
     next_objective int not null default 1, \
     next_task int not null default 1, \
     next_question int not null default 1, \
     next_constraint int not null default 1, \
     next_out_of_scope int not null default 1, \
     constraint missions_parent_fk foreign key (parent) references missions (id))",
    "create table objective_outcomes (\
     mission_id varchar(64) not null, \
     seq int not null, \
     outcome longtext not null, \
     met boolean not null default false, \
     primary key (mission_id, seq), \
     constraint objective_outcomes_mission_fk foreign key (mission_id) references missions (id))",
    "create table mission_blocks (\
     mission_id varchar(64) not null, \
     blocked_by_id varchar(64) not null, \
     primary key (mission_id, blocked_by_id), \
     constraint mission_blocks_mission_fk foreign key (mission_id) references missions (id), \
     constraint mission_blocks_by_fk foreign key (blocked_by_id) references missions (id))",
    "create table mission_documents (\
     mission_id varchar(64) not null, \
     path varchar(1024) not null, \
     role enum('intelligence','report','design') not null, \
     primary key (mission_id, path), \
     constraint mission_documents_mission_fk foreign key (mission_id) references missions (id))",
];

/// The constraint and out-of-scope tables, one row per one-line item. Created with a new
/// ledger, and by the upgrade to version 7, which moves the two text columns of
/// `missions` into them.
pub const LIST_SCHEMA: &[&str] = &[
    "create table mission_constraints (\
     mission_id varchar(64) not null, \
     seq int not null, \
     text longtext not null, \
     primary key (mission_id, seq), \
     constraint mission_constraints_mission_fk foreign key (mission_id) references missions (id))",
    "create table mission_out_of_scope (\
     mission_id varchar(64) not null, \
     seq int not null, \
     text longtext not null, \
     primary key (mission_id, seq), \
     constraint mission_out_of_scope_mission_fk foreign key (mission_id) references missions (id))",
];

/// The task, decision and question tables, created with a new ledger and by the first
/// record write on a ledger at version 3 or lower. `next_task` and `next_question` on
/// `missions` are the numbers the next task and question get: they only grow.
pub const TASK_SCHEMA: &[&str] = &[
    "create table tasks (\
     mission_id varchar(64) not null, \
     seq int not null, \
     description longtext not null, \
     status enum('todo','in-progress','done') not null default 'todo', \
     primary key (mission_id, seq), \
     constraint tasks_mission_fk foreign key (mission_id) references missions (id))",
    "create table task_blocks (\
     mission_id varchar(64) not null, \
     task_seq int not null, \
     blocked_by_seq int not null, \
     primary key (mission_id, task_seq, blocked_by_seq), \
     constraint task_blocks_task_fk foreign key (mission_id, task_seq) references tasks (mission_id, seq), \
     constraint task_blocks_by_fk foreign key (mission_id, blocked_by_seq) references tasks (mission_id, seq))",
    "create table decisions (\
     mission_id varchar(64) not null, \
     seq int not null, \
     text longtext not null, \
     decided_by varchar(255) not null, \
     decided_on date not null, \
     task_seq int null, \
     adr_path varchar(1024) null, \
     primary key (mission_id, seq), \
     constraint decisions_mission_fk foreign key (mission_id) references missions (id), \
     constraint decisions_task_fk foreign key (mission_id, task_seq) references tasks (mission_id, seq))",
    "create table questions (\
     mission_id varchar(64) not null, \
     seq int not null, \
     text longtext not null, \
     status enum('open','answered') not null default 'open', \
     answer longtext null, \
     answered_by varchar(255) null, \
     answered_on date null, \
     asked_on date not null, \
     primary key (mission_id, seq), \
     constraint questions_mission_fk foreign key (mission_id) references missions (id))",
];

/// The layout version a new Dolt ledger records: documents live on the `tsk/docs` branch,
/// missions, tasks, decisions and questions are rows, a thread names its mission in
/// `threads.mission_id`, a decision may name an ADR in `decisions.adr_path`, a
/// mission's constraints and out-of-scope items are rows of their own, and every
/// session and worktree binding is a row of `bindings`.
pub const LAYOUT_VERSION: i64 = 8;
/// The layout version with the `bindings` table in place of `cloud_session_bindings`.
pub const BINDINGS_LAYOUT_VERSION: i64 = 8;
/// The layout version with `threads.mission_id` and no `decisions.adr_path`.
pub const THREAD_MISSION_LAYOUT_VERSION: i64 = 5;
/// The layout version with `decisions.adr_path` and the two text columns on `missions`.
pub const ADR_LAYOUT_VERSION: i64 = 6;
/// The layout version with the constraint and out-of-scope tables.
pub const LIST_LAYOUT_VERSION: i64 = 7;
/// The layout version with the task, decision and question tables and a `threads` table
/// that holds only `briefing_path`.
pub const TASKS_LAYOUT_VERSION: i64 = 4;
/// The layout version with the mission tables and no task, decision or question tables.
pub const MISSIONS_LAYOUT_VERSION: i64 = 3;
/// The layout version with documents on the `tsk/docs` branch and no mission tables.
pub const DOCS_LAYOUT_VERSION: i64 = 2;
/// The layout version whose documents are rows in the `documents` table.
pub const LEGACY_LAYOUT_VERSION: i64 = 1;

/// Statements that take a ledger at `from` (2 to 7, or 1 once its documents have moved,
/// which counts as 2) to the current version, then record it. Version 2 gains the
/// mission, task, decision and question tables. Version 3 gains the counters on
/// `missions` and the task, decision and question tables. Version 4 gains
/// `threads.mission_id`, and `threads.briefing_path` becomes nullable. Version 5 gains
/// `decisions.adr_path`. Version 6 gains the constraint and out-of-scope tables, and
/// `missions` loses its two text columns. Version 8 creates `bindings`, copies every
/// `cloud_session_bindings` row into it as kind `cloud` and drops the old table. The tables a step creates are created in their
/// newest shape, so a later step alters only a table that an older layout made. Run at
/// the first write that needs the new layout, and when `tsk ledger migrate-docs` moves a
/// version 1 ledger.
///
/// The items that the version 7 step moves out of the text columns are SQL the caller
/// builds from the rows, because splitting text into lines is not SQL. `moved` is that
/// SQL: `inserts` for the two tables and the counters. It runs after the tables exist and
/// before the columns go. Pass [`move_lists_sql`] of the rows for a ledger from 3 to 6,
/// and an empty string otherwise.
pub fn upgrade_sql_moving(from: i64, moved: &str) -> String {
    let mut sql = String::new();
    if from <= DOCS_LAYOUT_VERSION {
        for statement in MISSION_SCHEMA.iter().chain(LIST_SCHEMA).chain(TASK_SCHEMA) {
            sql.push_str(statement);
            sql.push_str(";\n");
        }
    } else if from == MISSIONS_LAYOUT_VERSION {
        sql.push_str(
            "alter table missions add column next_task int not null default 1;\n\
             alter table missions add column next_question int not null default 1;\n",
        );
        for statement in TASK_SCHEMA {
            sql.push_str(statement);
            sql.push_str(";\n");
        }
    }
    if from < THREAD_MISSION_LAYOUT_VERSION {
        sql.push_str(
            "alter table threads modify column briefing_path varchar(1024) null;\n\
             alter table threads add column mission_id varchar(64) null;\n",
        );
    }
    // A decisions table made above, from version 3 or lower, already has the column.
    if from == TASKS_LAYOUT_VERSION || from == THREAD_MISSION_LAYOUT_VERSION {
        sql.push_str("alter table decisions add column adr_path varchar(1024) null;\n");
    }
    // A missions table made above, from version 2 or lower, has no text columns.
    if (MISSIONS_LAYOUT_VERSION..LIST_LAYOUT_VERSION).contains(&from) {
        sql.push_str(
            "alter table missions add column next_constraint int not null default 1;\n\
             alter table missions add column next_out_of_scope int not null default 1;\n",
        );
        for statement in LIST_SCHEMA {
            sql.push_str(statement);
            sql.push_str(";\n");
        }
        sql.push_str(moved);
        sql.push_str(
            "alter table missions drop column constraints_text;\n\
             alter table missions drop column out_of_scope;\n",
        );
    }
    if from < BINDINGS_LAYOUT_VERSION {
        sql.push_str(BINDINGS_UPGRADE_SQL);
    }
    sql.push_str(&version_sql());
    sql
}

/// The version 8 step: the old table's rows become cloud rows of `bindings`.
const BINDINGS_UPGRADE_SQL: &str = "create table bindings (actor varchar(255) primary key, kind enum('cloud','worktree') not null, thread_id varchar(64) not null, registered_at varchar(32) not null);\n\
     insert into bindings (actor, kind, thread_id, registered_at) select session_id, 'cloud', thread_id, registered_at from cloud_session_bindings;\n\
     drop table cloud_session_bindings;\n";

/// [`upgrade_sql_moving`] for a ledger that has no mission text to move.
pub fn upgrade_sql(from: i64) -> String {
    upgrade_sql_moving(from, "")
}

/// The statements that put the lines of each mission's old text columns into the two list
/// tables and set the counters after the last item. `rows` are `(mission ID, constraints
/// text, out-of-scope text)`. A line is trimmed, an empty line is skipped, and the items
/// are numbered from 1 in order.
pub fn move_lists_sql(rows: &[(String, String, String)]) -> String {
    let mut sql = String::new();
    for (id, constraints, out_of_scope) in rows {
        for (table, column, text) in [
            ("mission_constraints", "next_constraint", constraints),
            ("mission_out_of_scope", "next_out_of_scope", out_of_scope),
        ] {
            let items = crate::mission::model::lines_to_items(text);
            let Some(last) = items.last() else { continue };
            let values: Vec<String> = items
                .iter()
                .map(|i| format!("({}, {}, {})", lit(id), i.seq, lit(&i.text)))
                .collect();
            sql.push_str(&format!(
                "insert into {} (mission_id, seq, text) values {};\n\
                 update missions set {} = {} where id = {};\n",
                table,
                values.join(", "),
                column,
                last.seq + 1,
                lit(id)
            ));
        }
    }
    sql
}

fn version_sql() -> String {
    format!(
        "replace into manifest (k, v) values ('version', '{}');\n",
        LAYOUT_VERSION
    )
}

/// Statements that create every table and the manifest rows (`version`, and `repo_id`
/// for a nexus ledger), separated by `;\n`.
pub fn init_sql(repo_id: Option<&str>) -> String {
    let mut sql = String::new();
    for statement in SCHEMA
        .iter()
        .chain(MISSION_SCHEMA)
        .chain(LIST_SCHEMA)
        .chain(TASK_SCHEMA)
    {
        sql.push_str(statement);
        sql.push_str(";\n");
    }
    sql.push_str(&format!(
        "insert into manifest (k, v) values ('version', '{}');\n",
        LAYOUT_VERSION
    ));
    if let Some(id) = repo_id {
        sql.push_str(&format!(
            "insert into manifest (k, v) values ('repo_id', {});\n",
            lit(id)
        ));
    }
    sql
}

/// A SQL string literal that cannot carry an injection: the text is hex-encoded and
/// converted, so no byte of it is ever parsed as SQL.
pub fn lit(text: &str) -> String {
    if text.is_empty() {
        return "''".to_string();
    }
    let mut hex = String::with_capacity(text.len() * 2);
    for byte in text.as_bytes() {
        hex.push_str(&format!("{:02x}", byte));
    }
    format!("CONVERT(X'{}' USING utf8mb4)", hex)
}

#[derive(Clone)]
pub struct Dolt {
    dir: PathBuf,
    /// The SQL of this handle goes to the local Dolt server (see
    /// [`crate::ledger::store::server`]). Everything else still starts `dolt`.
    served: bool,
}

/// What a call through the server gave: its result, or the news that no server could be
/// used, which sends the call down the `dolt` command line path.
enum Served<T> {
    Done(Result<T, String>),
    Fallback,
}

/// What the server runs in place of a `dolt` command: the stored procedure, and the
/// session settings around it.
struct Procedure {
    before: Vec<String>,
    call: String,
    after: Vec<String>,
}

impl Procedure {
    fn call(call: String) -> Option<Procedure> {
        Some(Procedure {
            before: Vec::new(),
            call,
            after: Vec::new(),
        })
    }

    /// A merge that leaves a conflict in the working set, as `dolt merge` does. Over SQL
    /// a transaction that holds a conflict or a constraint violation is rolled back unless
    /// the session allows it. `merge_remote` reads both tables and aborts, so it must
    /// find them.
    fn merge(call: String) -> Option<Procedure> {
        Some(Procedure {
            before: vec![
                "set @@session.dolt_allow_commit_conflicts = 1, @@session.dolt_force_transaction_commit = 1"
                    .to_string(),
            ],
            call,
            after: vec![
                "set @@session.dolt_allow_commit_conflicts = 0, @@session.dolt_force_transaction_commit = 0"
                    .to_string(),
            ],
        })
    }
}

/// The stored procedure that does what a `dolt` command does, for the commands that the
/// server runs in place of a process. The others (`remote`, `clone`, `init`, `gc`,
/// `config`) stay on the command line. Every argument is a literal that the server's
/// parser takes as it is.
fn procedure_call(args: &[String]) -> Option<Procedure> {
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match args.as_slice() {
        ["fetch", remote] => Procedure::call(format!("call dolt_fetch({})", lit(remote))),
        ["push", remote, branch] => {
            Procedure::call(format!("call dolt_push({}, {})", lit(remote), lit(branch)))
        }
        ["reset", "--hard", target] => {
            Procedure::call(format!("call dolt_reset('--hard', {})", lit(target)))
        }
        ["clean"] => Procedure::call("call dolt_clean()".to_string()),
        ["add", "-A"] => Procedure::call("call dolt_add('-A')".to_string()),
        ["commit", "-m", message] => {
            Procedure::call(format!("call dolt_commit('-m', {})", lit(message)))
        }
        ["merge", "--abort"] => Procedure::call("call dolt_merge('--abort')".to_string()),
        ["merge", "--no-commit", target] => {
            Procedure::merge(format!("call dolt_merge('--no-commit', {})", lit(target)))
        }
        ["merge", target] => Procedure::merge(format!("call dolt_merge({})", lit(target))),
        _ => None,
    }
}

/// True when `dolt` says it could not write because another process holds the store.
fn is_read_only_refusal(stderr: &str) -> bool {
    stderr.to_lowercase().contains("read only")
}

/// True for a Dolt database whose `origin` is a file remote.
fn is_file_remote_store(dir: &Path) -> bool {
    dir.join(".dolt").is_dir()
        && transport::store_url(dir)
            .is_some_and(|url| Transport::of_url(&url) == Transport::FileRemote)
}

impl Dolt {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Dolt {
            dir: dir.into(),
            served: false,
        }
    }

    /// A handle on a ledger store. With the server switch on, and a store that uses a file
    /// remote, its SQL goes through the local server. A store on Dolt's blobstore remote
    /// stays on the command line: its `fetch` and `push` start git, and the environment
    /// of the command that asked is the one that must reach it.
    pub fn store(dir: impl Into<PathBuf>) -> Self {
        let dir = dir.into();
        let served = server::enabled() && is_file_remote_store(&dir);
        Dolt { dir, served }
    }

    fn serve<T>(
        &self,
        read_only: bool,
        run: impl FnMut(&mut Conn) -> Result<T, SqlError>,
    ) -> Served<T> {
        match server::with_connection(&self.dir, read_only, run) {
            Ok(value) => Served::Done(Ok(value)),
            Err(Failure::Unavailable) => Served::Fallback,
            Err(Failure::Failed(e)) => Served::Done(Err(server::failure_text(&e))),
        }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn command(&self) -> Command {
        let mut command = Command::new("dolt");
        command
            .current_dir(&self.dir)
            .env("DOLT_DISABLE_EVENT_FLUSH", "1")
            // Empty, so a push to a git remote writes no `__dolt_remote_info__` branch.
            .env("DOLT_REMOTE_INFO_BRANCH", "");
        // Reaches the git processes that dolt starts for a fetch or a push.
        crate::gitenv::apply(&mut command);
        command
    }

    fn spawn_error(e: std::io::Error) -> String {
        if e.kind() == std::io::ErrorKind::NotFound {
            NOT_INSTALLED.to_string()
        } else {
            format!("error: could not run dolt: {}", e)
        }
    }

    pub fn output<I, S>(&self, args: I) -> Result<(Vec<String>, Output), String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let args: Vec<String> = args
            .into_iter()
            .map(|a| a.as_ref().to_string_lossy().into_owned())
            .collect();
        if self.served {
            if let Some(procedure) = procedure_call(&args) {
                if let Some(output) = self.serve_procedure(&procedure) {
                    return Ok((args, output));
                }
            }
        }
        let mut output = self
            .command()
            .args(&args)
            .output()
            .map_err(Self::spawn_error)?;
        // A running server holds the store for writing, and a `dolt` that finds it open
        // reads the store and refuses to write it. A write from a handle that does not use
        // the server (the switch is off now, say) stops the server that an earlier command
        // left running, then runs again. Nothing was written the first time.
        if !output.status.success()
            && is_read_only_refusal(&String::from_utf8_lossy(&output.stderr))
            && server::stop_if_running(&self.dir)
        {
            output = self
                .command()
                .args(&args)
                .output()
                .map_err(Self::spawn_error)?;
        }
        Ok((args, output))
    }

    /// Run a Dolt stored procedure through the server and give its result the shape of
    /// the command's: the exit status, and the server's message as the standard error.
    /// `None` when no server could be used, which sends the call to the command line. The
    /// procedures that cross the network get a long time limit.
    fn serve_procedure(&self, procedure: &Procedure) -> Option<Output> {
        let outcome = server::with_connection(&self.dir, false, |conn| {
            for statement in &procedure.before {
                conn.query(statement)?;
            }
            conn.set_timeout(Some(server::network_timeout()))?;
            let result = conn.query(&procedure.call);
            conn.set_timeout(Some(server::query_timeout()))?;
            for statement in &procedure.after {
                conn.query(statement)?;
            }
            result
        });
        let (code, stderr) = match outcome {
            Ok(_) => (0, String::new()),
            Err(Failure::Unavailable) => return None,
            Err(Failure::Failed(SqlError::Server { message, .. })) => (1, message),
            Err(Failure::Failed(other)) => (1, other.to_string()),
        };
        Some(Output {
            status: std::os::unix::process::ExitStatusExt::from_raw(code << 8),
            stdout: Vec::new(),
            stderr: stderr.into_bytes(),
        })
    }

    pub fn run<I, S>(&self, args: I) -> Result<String, String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let (args, output) = self.output(args)?;
        if !output.status.success() {
            return Err(failure_message(&args, &output));
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }

    /// Run one query with `dolt sql -r json` and return the rows of every result set.
    /// The query is not passed on the command line, so its length is not limited by the
    /// operating system.
    pub fn rows(&self, sql: &str) -> Result<Vec<Value>, String> {
        if self.served {
            if let Served::Done(result) =
                self.serve(server::is_read_only(sql), |conn| conn.query(sql))
            {
                return result.map(server::flatten);
            }
        }
        let args = vec!["sql".to_string(), "-r".to_string(), "json".to_string()];
        let stdout = self.pipe(&args, sql)?;
        parse_rows(&stdout)
    }

    /// Run several queries in one `dolt sql` process and return the rows of each, in
    /// order. A statement with no rows prints an empty object, so the result always
    /// holds one entry per query. The first failing statement stops the script and fails
    /// the whole call. Every query must be one statement.
    pub fn rows_each(&self, queries: &[String]) -> Result<Vec<Vec<Value>>, String> {
        if queries.is_empty() {
            return Ok(Vec::new());
        }
        if !batching() {
            return Err(format!(
                "error: batched reads are off ({} is set)",
                NO_BATCH_VAR
            ));
        }
        if self.served {
            let read_only = queries.iter().all(|q| server::is_read_only(q));
            let served = self.serve(read_only, |conn| {
                let mut results = Vec::with_capacity(queries.len());
                for query in queries {
                    results.push(server::flatten(conn.query(query)?));
                }
                Ok(results)
            });
            if let Served::Done(result) = served {
                return result;
            }
        }
        let script: String = queries.iter().map(|q| format!("{};\n", q)).collect();
        let args = vec!["sql".to_string(), "-r".to_string(), "json".to_string()];
        let stdout = self.pipe(&args, &script)?;
        let results = parse_result_sets(&stdout)?;
        if results.len() != queries.len() {
            return Err(format!(
                "error: dolt printed {} result sets for {} queries",
                results.len(),
                queries.len()
            ));
        }
        Ok(results)
    }

    /// [`Self::rows_each`] after some write statements, in the same process. The writes
    /// run first, each as one statement, and print nothing that the result counts: the
    /// call returns the rows of the last `queries.len()` result sets. A statement that
    /// fails stops the script, so the writes before it have applied and the queries after
    /// it have not run.
    pub fn rows_each_after(
        &self,
        writes: &[String],
        queries: &[String],
    ) -> Result<Vec<Vec<Value>>, String> {
        if writes.is_empty() {
            return self.rows_each(queries);
        }
        if queries.is_empty() {
            return Ok(Vec::new());
        }
        if !batching() {
            return Err(format!(
                "error: batched reads are off ({} is set)",
                NO_BATCH_VAR
            ));
        }
        if self.served {
            let served = self.serve(false, |conn| {
                for write in writes {
                    conn.query(write.trim_end().trim_end_matches(';'))?;
                }
                let mut results = Vec::with_capacity(queries.len());
                for query in queries {
                    results.push(server::flatten(conn.query(query)?));
                }
                Ok(results)
            });
            if let Served::Done(result) = served {
                return result;
            }
        }
        let script: String = writes
            .iter()
            .chain(queries.iter())
            .map(|q| format!("{};\n", q.trim_end().trim_end_matches(';')))
            .collect();
        let args = vec!["sql".to_string(), "-r".to_string(), "json".to_string()];
        let stdout = self.pipe(&args, &script)?;
        let mut results = parse_result_sets(&stdout)?;
        if results.len() < queries.len() {
            return Err(format!(
                "error: dolt printed {} result sets for {} queries",
                results.len(),
                queries.len()
            ));
        }
        Ok(results.split_off(results.len() - queries.len()))
    }

    /// Run write statements in one transaction: all of them apply, or none.
    pub fn exec(&self, sql: &str) -> Result<(), String> {
        if self.served {
            let script = format!("start transaction;\n{}\ncommit;\n", sql);
            if let Served::Done(result) = self.serve(false, |conn| conn.query(&script).map(|_| ()))
            {
                return result;
            }
        }
        let script = format!("start transaction;\n{}\ncommit;\n", sql);
        let args = vec!["sql".to_string()];
        self.pipe(&args, &script).map(|_| ())
    }

    fn pipe(&self, args: &[String], input: &str) -> Result<String, String> {
        let mut child = self
            .command()
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(Self::spawn_error)?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(input.as_bytes())
                .map_err(|e| format!("error: could not write to dolt {}: {}", args.join(" "), e))?;
        }
        let output = child
            .wait_with_output()
            .map_err(|e| format!("error: could not run dolt: {}", e))?;
        if !output.status.success() {
            // See `output`: a write that a running server's lock refused runs again once
            // the server is stopped.
            if is_read_only_refusal(&String::from_utf8_lossy(&output.stderr))
                && server::stop_if_running(&self.dir)
            {
                return self.pipe(args, input);
            }
            return Err(failure_message(args, &output));
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }

    /// The first column of the first row of a query, read as an integer.
    pub fn count(&self, sql: &str) -> Result<i64, String> {
        Ok(first_count(&self.rows(sql)?))
    }

    /// The commit hash of `HEAD`.
    pub fn head_hash(&self) -> Result<String, String> {
        self.hash_of("HEAD")
    }

    /// The commit hash a revision names.
    pub fn hash_of(&self, revision: &str) -> Result<String, String> {
        let rows = self.rows(&format!("select hashof({}) as h", lit(revision)))?;
        rows.first()
            .and_then(|row| text(row, "h"))
            .ok_or_else(|| format!("error: dolt could not resolve {}", revision))
    }
}

/// The first column of the first row, read as an integer, or 0.
pub fn first_count(rows: &[Value]) -> i64 {
    rows.first()
        .and_then(|row| row.as_object())
        .and_then(|map| map.values().next())
        .and_then(number)
        .unwrap_or(0)
}

/// An integer, whether Dolt printed it as a JSON number or as a string. `dolt sql -r json`
/// prints numbers when it opens the store itself and strings when a running Dolt server
/// answers it (see ADR 0018), so a reader takes both.
fn number(value: &Value) -> Option<i64> {
    match value {
        Value::Number(n) => n.as_i64(),
        Value::String(text) => text.trim().parse().ok(),
        _ => None,
    }
}

/// Make sure Dolt has a commit identity. A machine with none gets `tsk <tsk@localhost>`
/// globally; an identity the operator set is left alone.
pub fn ensure_identity() -> Result<(), String> {
    static ENSURED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if ENSURED.load(std::sync::atomic::Ordering::Relaxed) {
        return Ok(());
    }
    ensure_identity_once()?;
    ENSURED.store(true, std::sync::atomic::Ordering::Relaxed);
    Ok(())
}

/// True when Dolt's global configuration file names a user. The file is `config_global.json`
/// in `.dolt` under `DOLT_ROOT_PATH`, or under the home directory when that is unset. Reading
/// it saves the `dolt` process that `dolt config --get` would start. A file that is absent
/// or unreadable says no, and the process answers.
fn identity_in_global_config() -> bool {
    let root = ["DOLT_ROOT_PATH", "HOME"]
        .iter()
        .find_map(|name| std::env::var_os(name).filter(|value| !value.is_empty()));
    match root {
        Some(root) => identity_in_global_config_at(Path::new(&root)),
        None => false,
    }
}

fn identity_in_global_config_at(root: &Path) -> bool {
    let file = root.join(".dolt").join("config_global.json");
    std::fs::read_to_string(file)
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .and_then(|config| {
            config
                .get("user.name")
                .and_then(Value::as_str)
                .map(|name| !name.trim().is_empty())
        })
        .unwrap_or(false)
}

fn ensure_identity_once() -> Result<(), String> {
    if identity_in_global_config() {
        return Ok(());
    }
    let here = Dolt::new(".");
    let (_, output) = here.output(["config", "--global", "--get", "user.name"])?;
    let name = String::from_utf8_lossy(&output.stdout);
    if output.status.success() && !name.trim().is_empty() {
        return Ok(());
    }
    here.run(["config", "--global", "--add", "user.name", "tsk"])?;
    here.run(["config", "--global", "--add", "user.email", "tsk@localhost"])?;
    Ok(())
}

/// The string in a row's column, or `None` when it is absent or NULL.
pub fn text(row: &Value, column: &str) -> Option<String> {
    row.get(column).and_then(Value::as_str).map(str::to_string)
}

pub fn integer(row: &Value, column: &str) -> Option<i64> {
    row.get(column).and_then(number)
}

/// A boolean column, whether Dolt rendered it as `true` or as `1`.
pub fn flag(row: &Value, column: &str) -> bool {
    match row.get(column) {
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_i64() == Some(1),
        Some(Value::String(text)) => matches!(text.trim(), "1" | "true"),
        _ => false,
    }
}

fn parse_result_sets(stdout: &str) -> Result<Vec<Vec<Value>>, String> {
    let mut sets = Vec::new();
    for result in serde_json::Deserializer::from_str(stdout).into_iter::<Value>() {
        let result =
            result.map_err(|e| format!("error: dolt printed output that is not JSON: {}", e))?;
        sets.push(match result.get("rows") {
            Some(Value::Array(found)) => found.clone(),
            _ => Vec::new(),
        });
    }
    Ok(sets)
}

fn parse_rows(stdout: &str) -> Result<Vec<Value>, String> {
    let mut rows = Vec::new();
    for result in serde_json::Deserializer::from_str(stdout).into_iter::<Value>() {
        let result =
            result.map_err(|e| format!("error: dolt printed output that is not JSON: {}", e))?;
        if let Some(Value::Array(found)) = result.get("rows") {
            rows.extend(found.iter().cloned());
        }
    }
    Ok(rows)
}

pub fn failure_message(args: &[String], output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let detail = if stderr.trim().is_empty() {
        stdout.trim().to_string()
    } else {
        stderr.trim().to_string()
    };
    let status = match output.status.code() {
        Some(code) => format!("exit status {}", code),
        None => "terminated by a signal".to_string(),
    };
    let shown = args.join(" ");
    let shown: String = shown.chars().take(200).collect();
    if detail.is_empty() {
        format!("error: dolt {} failed ({})", shown, status)
    } else {
        format!("error: dolt {} failed ({}):\n{}", shown, status, detail)
    }
}

/// True when `dolt` can be started. Used by tests to skip when it is missing.
#[allow(dead_code)]
pub fn available() -> bool {
    Command::new("dolt")
        .arg("version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lit_hex_encodes_so_quotes_and_semicolons_stay_data() {
        assert_eq!(lit(""), "''");
        assert_eq!(lit("a"), "CONVERT(X'61' USING utf8mb4)");
        let hostile = lit("x'); drop table threads; --");
        assert!(!hostile.contains("drop"));
        assert!(hostile.starts_with("CONVERT(X'"));
    }

    #[test]
    fn writes_run_before_the_queries_in_one_process_and_are_not_counted() {
        if !available() {
            eprintln!("note: dolt is not installed; skipping");
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let db = Dolt::new(dir.path());
        db.run(["init", "--name", "t", "--email", "t@t"]).unwrap();
        db.exec("create table t (a int primary key);").unwrap();

        // The write has a trailing semicolon, as the statements of the store do. It prints
        // nothing, and the result sets are those of the queries.
        let sets = db
            .rows_each_after(
                &["insert into t values (1);".to_string()],
                &[
                    "select count(*) as c from t".to_string(),
                    "select a from t".to_string(),
                ],
            )
            .unwrap();
        assert_eq!(sets.len(), 2);
        assert_eq!(first_count(&sets[0]), 1);
        assert_eq!(integer(&sets[1][0], "a"), Some(1));

        // No writes is the plain batch.
        let sets = db
            .rows_each_after(&[], &["select count(*) as c from t".to_string()])
            .unwrap();
        assert_eq!(first_count(&sets[0]), 1);

        // A write that fails stops the script and fails the call.
        assert!(db
            .rows_each_after(
                &["insert into t values (1)".to_string()],
                &["select 1".to_string()],
            )
            .is_err());
    }

    #[test]
    fn rows_are_read_from_every_result_set() {
        let rows =
            parse_rows("{\"rows\": [{\"a\":1}]}\n{}\n{\"rows\": [{\"b\":\"x\"}]}\n").unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(integer(&rows[0], "a"), Some(1));
        assert_eq!(text(&rows[1], "b").as_deref(), Some("x"));
        assert!(parse_rows("{}\n").unwrap().is_empty());
        assert!(parse_rows("").unwrap().is_empty());
    }

    #[test]
    fn init_sql_creates_every_table_and_the_manifest_rows() {
        let sql = init_sql(Some("work-api"));
        for table in [
            "manifest",
            "threads",
            "continuation_entries",
            "bindings",
            "external_events",
            "watermark",
            "missions",
            "objective_outcomes",
            "mission_blocks",
            "mission_documents",
            "tasks",
            "task_blocks",
            "decisions",
            "questions",
        ] {
            assert!(
                sql.contains(&format!("create table {} ", table)),
                "{}",
                table
            );
        }
        assert!(!sql.contains("create table documents"));
        assert!(!sql.contains("cloud_session_bindings"));
        assert!(sql.contains("kind enum('cloud','worktree') not null"));
        assert!(sql.contains("('version', '8')"));
        assert!(sql.contains("mission_id varchar(64) null"));
        assert!(sql.contains("briefing_path varchar(1024) null"));
        assert!(sql.contains("'repo_id'"));
        assert!(!init_sql(None).contains("repo_id"));
    }

    #[test]
    fn the_upgrade_from_each_older_version_ends_at_the_current_one() {
        let from_four = upgrade_sql(4);
        assert!(from_four.contains("alter table threads add column mission_id"));
        assert!(from_four.contains("modify column briefing_path varchar(1024) null"));
        assert!(!from_four.contains("create table missions "));
        assert!(!from_four.contains("create table tasks "));
        assert!(from_four.contains("alter table decisions add column adr_path varchar(1024) null"));
        assert!(from_four.ends_with("('version', '8');\n"));
        let from_five = upgrade_sql(5);
        assert!(!from_five.contains("alter table threads"));
        assert!(!from_five.contains("create table tasks "));
        assert!(from_five.contains("alter table decisions add column adr_path"));
        assert!(from_five.ends_with("('version', '8');\n"));
        let from_six = upgrade_sql(6);
        assert!(!from_six.contains("alter table decisions"));
        assert!(!from_six.contains("alter table threads"));
        assert!(from_six.contains("create table mission_constraints "));
        assert!(from_six.contains("create table mission_out_of_scope "));
        assert!(from_six.contains("add column next_constraint"));
        assert!(from_six.contains("drop column constraints_text"));
        assert!(from_six.contains("drop column out_of_scope"));
        assert!(!from_six.contains("create table missions "));
        assert!(from_six.ends_with("('version', '8');\n"));
        let from_seven = upgrade_sql(7);
        assert!(from_seven.contains("create table bindings "));
        assert!(from_seven.contains(
            "select session_id, 'cloud', thread_id, registered_at from cloud_session_bindings"
        ));
        assert!(from_seven.contains("drop table cloud_session_bindings"));
        assert!(!from_seven.contains("mission_constraints"));
        assert!(!from_seven.contains("alter table"));
        assert!(from_seven.ends_with("('version', '8');\n"));
        // Every older step ends with the same bindings step, after the tables it makes.
        assert!(from_six.contains("drop table cloud_session_bindings"));
        let from_three = upgrade_sql(3);
        assert!(from_three.contains("alter table missions add column next_task"));
        assert!(from_three.contains("create table tasks "));
        assert!(!from_three.contains("create table missions "));
        assert!(from_three.contains("alter table threads add column mission_id"));
        // The decisions table is created with the column, so no step alters it.
        assert!(from_three.contains("adr_path varchar(1024) null"));
        assert!(!from_three.contains("alter table decisions"));
        // A missions table that exists is altered; the lists are created beside it.
        assert!(from_three.contains("drop column constraints_text"));
        let from_two = upgrade_sql(2);
        assert!(from_two.contains("create table missions "));
        assert!(from_two.contains("create table questions "));
        assert!(from_two.contains("alter table threads add column mission_id"));
        assert!(!from_two.contains("alter table decisions"));
        // Every table above is made in its newest shape, so nothing is moved or dropped.
        assert!(from_two.contains("create table mission_constraints "));
        assert!(!from_two.contains("constraints_text"));
        assert!(!from_two.contains("drop column"));
        assert!(from_two.contains("drop table cloud_session_bindings"));
        assert_eq!(upgrade_sql(1), from_two);
    }

    #[test]
    fn the_old_text_moves_into_the_list_tables_one_item_per_non_empty_line() {
        let rows = vec![
            (
                "M-A".to_string(),
                "  first\n\n second  \n\t\nthird\r\n".to_string(),
                String::new(),
            ),
            ("M-B".to_string(), String::new(), "only".to_string()),
            ("M-C".to_string(), " \n ".to_string(), String::new()),
        ];
        let sql = move_lists_sql(&rows);
        assert_eq!(sql.matches("insert into mission_constraints").count(), 1);
        assert_eq!(sql.matches("insert into mission_out_of_scope").count(), 1);
        assert!(sql.contains("update missions set next_constraint = 4 where id = "));
        assert!(sql.contains("update missions set next_out_of_scope = 2 where id = "));
        // Three items for M-A, numbered 1 to 3, and nothing for M-C.
        assert_eq!(sql.matches(", 3, ").count(), 1);
        assert!(!sql.contains(&lit("M-C")));
        let upgrade = upgrade_sql_moving(6, &sql);
        let at = |needle: &str| upgrade.find(needle).unwrap();
        assert!(at("create table mission_constraints") < at("insert into mission_constraints"));
        assert!(at("insert into mission_constraints") < at("drop column constraints_text"));
    }

    #[test]
    fn a_user_in_the_global_config_file_is_found_without_dolt() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join(".dolt");
        std::fs::create_dir_all(&config).unwrap();
        let file = config.join("config_global.json");
        let check = |text: Option<&str>| {
            match text {
                Some(text) => std::fs::write(&file, text).unwrap(),
                None => {
                    let _ = std::fs::remove_file(&file);
                }
            }
            identity_in_global_config_at(dir.path())
        };
        assert!(!check(None));
        assert!(!check(Some("not json")));
        assert!(!check(Some("{\"user.email\":\"a@b\"}")));
        assert!(!check(Some("{\"user.name\":\" \"}")));
        assert!(check(Some(
            "{\"user.email\":\"a@b\",\"user.name\":\"tsk\"}"
        )));
    }

    #[test]
    fn flag_reads_booleans_and_ones() {
        let row: Value = serde_json::from_str("{\"a\":true,\"b\":1,\"c\":0}").unwrap();
        assert!(flag(&row, "a"));
        assert!(flag(&row, "b"));
        assert!(!flag(&row, "c"));
        assert!(!flag(&row, "missing"));
    }
}
