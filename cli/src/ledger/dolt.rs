//! A runner for the external `dolt` binary, shaped like [`super::git::Git`], plus the
//! ledger's table definitions and the SQL literal helpers. tsk never links Dolt: every
//! operation is a `dolt` process.

use std::ffi::OsStr;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use serde_json::Value;

/// The branch every Dolt ledger database uses locally and on its remote.
pub const BRANCH: &str = "main";
/// The name of the remote inside the Dolt database.
pub const REMOTE: &str = "origin";

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
    "create table threads (thread_id varchar(64) primary key, briefing_path varchar(1024) not null)",
    "create table continuation_entries (thread_id varchar(64), seq int, entry longtext not null, primary key (thread_id, seq))",
    "create table cloud_session_bindings (session_id varchar(255) primary key, thread_id varchar(64) not null, registered_at varchar(32) not null)",
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
/// removed outcome's number is never reused.
pub const MISSION_SCHEMA: &[&str] = &[
    "create table missions (\
     id varchar(64) not null primary key, \
     name varchar(255) not null, \
     category enum('operational','administrative') not null, \
     status enum('todo','in-progress','done') not null default 'todo', \
     parent varchar(64) null, \
     purpose longtext null, \
     decision_authority longtext null, \
     constraints_text longtext null, \
     out_of_scope longtext null, \
     next_objective int not null default 1, \
     next_task int not null default 1, \
     next_question int not null default 1, \
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

/// The layout version a new Dolt ledger records: documents live on the `tsk/docs` branch
/// and missions, tasks, decisions and questions are rows.
pub const LAYOUT_VERSION: i64 = 4;
/// The layout version with the mission tables and no task, decision or question tables.
pub const MISSIONS_LAYOUT_VERSION: i64 = 3;
/// The layout version with documents on the `tsk/docs` branch and no mission tables.
pub const DOCS_LAYOUT_VERSION: i64 = 2;
/// The layout version whose documents are rows in the `documents` table.
pub const LEGACY_LAYOUT_VERSION: i64 = 1;

/// Statements that create the mission, task, decision and question tables and record the
/// current layout version. Run on a version 1 ledger after its documents move, and on a
/// version 2 ledger at the first mission write.
pub fn mission_upgrade_sql() -> String {
    let mut sql = String::new();
    for statement in MISSION_SCHEMA.iter().chain(TASK_SCHEMA) {
        sql.push_str(statement);
        sql.push_str(";\n");
    }
    sql.push_str(&version_sql());
    sql
}

/// Statements that take a version 3 ledger to the current version: the counters on
/// `missions`, the task, decision and question tables, and the manifest row.
pub fn task_upgrade_sql() -> String {
    let mut sql = String::from(
        "alter table missions add column next_task int not null default 1;\n\
         alter table missions add column next_question int not null default 1;\n",
    );
    for statement in TASK_SCHEMA {
        sql.push_str(statement);
        sql.push_str(";\n");
    }
    sql.push_str(&version_sql());
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
    for statement in SCHEMA.iter().chain(MISSION_SCHEMA).chain(TASK_SCHEMA) {
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
}

impl Dolt {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Dolt { dir: dir.into() }
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
        let output = self
            .command()
            .args(&args)
            .output()
            .map_err(Self::spawn_error)?;
        Ok((args, output))
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
        let args = vec!["sql".to_string(), "-r".to_string(), "json".to_string()];
        let stdout = self.pipe(&args, sql)?;
        parse_rows(&stdout)
    }

    /// Run write statements in one transaction: all of them apply, or none.
    pub fn exec(&self, sql: &str) -> Result<(), String> {
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
            return Err(failure_message(args, &output));
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }

    /// The first column of the first row of a query, read as an integer.
    pub fn count(&self, sql: &str) -> Result<i64, String> {
        let rows = self.rows(sql)?;
        Ok(rows
            .first()
            .and_then(|row| row.as_object())
            .and_then(|map| map.values().next())
            .and_then(Value::as_i64)
            .unwrap_or(0))
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

/// Make sure Dolt has a commit identity. A machine with none gets `tsk <tsk@localhost>`
/// globally; an identity the operator set is left alone.
pub fn ensure_identity() -> Result<(), String> {
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
    row.get(column).and_then(Value::as_i64)
}

/// A boolean column, whether Dolt rendered it as `true` or as `1`.
pub fn flag(row: &Value, column: &str) -> bool {
    match row.get(column) {
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_i64() == Some(1),
        _ => false,
    }
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
            "cloud_session_bindings",
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
        assert!(sql.contains("('version', '4')"));
        assert!(sql.contains("'repo_id'"));
        assert!(!init_sql(None).contains("repo_id"));
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
