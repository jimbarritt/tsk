//! The `PreToolUse` hook check of ADR 0028: every change to the ledger goes through the
//! `tsk` binary. The check reads the hook input on stdin and denies a tool call that
//! would edit a file in the ledger store, or that would run `git` or `dolt` against the
//! ledger store, its refs or its branches. It reads no ledger, makes no network call and
//! fails open: a malformed input, or a failure of its own, allows the call.

use std::io::Read;
use std::path::Path;

use serde::Deserialize;

/// `TSK_LEDGER_GUARD=off` in the environment of the hook switches the check off. A person
/// sets it in the shell that starts the agent. A variable set inside the checked command
/// does not reach the hook.
const SWITCH_ENV: &str = "TSK_LEDGER_GUARD";

const REASON: &str = "Blocked by tsk (ADR 0028): every change to the ledger goes through the tsk binary. \
Run no git or dolt command against the ledger store, its refs or its branches, and edit no file in the ledger store. \
Use tsk ledger status, fetch, push and discard, tsk ledger doc get, put and path, or the record commands (tsk mission, task, decision, question, thread, actor). \
If no command does what you need, record a task for the command and ask for it.";

/// The refs and branch names of the ledger, after the prefixes that name a remote or a
/// namespace are removed.
const LEDGER_REFS: [&str; 3] = ["tsk/ledger", "tsk/docs", "tsk/bootstrap"];

const WRAPPERS: [&str; 7] = ["sudo", "env", "command", "exec", "time", "nohup", "nice"];

/// Commands that print or set a value and read no file.
const HARMLESS: [&str; 8] = [
    "echo", "printf", "export", "unset", "true", "test", "[", "type",
];

#[derive(Deserialize, Default)]
struct HookInput {
    #[serde(default)]
    tool_name: String,
    #[serde(default)]
    tool_input: serde_json::Value,
    #[serde(default)]
    cwd: String,
}

/// Where the ledger store is on this machine.
#[derive(Debug, Default, Clone)]
pub struct Places {
    /// Absolute path prefixes under which the ledger store and the documents worktree sit.
    prefixes: Vec<String>,
    home: String,
    xdg_state_home: String,
}

impl Places {
    pub fn from_env() -> Places {
        let mut prefixes = Vec::new();
        if let Ok(root) = super::location::state_root_from_env() {
            prefixes.push(format!("{}/repos", root.display()));
        }
        if let Some(wt) = std::env::var_os("TSK_LEDGER_WT").filter(|v| !v.is_empty()) {
            prefixes.push(Path::new(&wt).display().to_string());
        }
        Places {
            prefixes,
            home: std::env::var("HOME").unwrap_or_default(),
            xdg_state_home: std::env::var("XDG_STATE_HOME").unwrap_or_default(),
        }
    }

    /// Whether a word names a path in the ledger store: a literal path, a path written
    /// with `~` or `$HOME`, the variable `TSK_LEDGER_WT`, or the database directory.
    fn names_path(&self, word: &str) -> bool {
        if word.contains("TSK_LEDGER_WT") || word.contains(super::location::DOLT_DIR_NAME) {
            return true;
        }
        let word = self.expand(word);
        self.prefixes
            .iter()
            .any(|prefix| !prefix.is_empty() && word.contains(prefix.as_str()))
    }

    fn contains_dir(&self, dir: &str) -> bool {
        let dir = dir.trim_end_matches('/');
        !dir.is_empty()
            && self.prefixes.iter().any(|prefix| {
                !prefix.is_empty() && (dir == prefix || dir.starts_with(&format!("{}/", prefix)))
            })
    }

    fn expand(&self, word: &str) -> String {
        let mut text = word.to_string();
        if !self.home.is_empty() {
            if let Some(rest) = text.strip_prefix("~/") {
                text = format!("{}/{}", self.home, rest);
            }
            text = text
                .replace("${HOME}", &self.home)
                .replace("$HOME", &self.home);
        }
        if !self.xdg_state_home.is_empty() {
            text = text
                .replace("${XDG_STATE_HOME}", &self.xdg_state_home)
                .replace("$XDG_STATE_HOME", &self.xdg_state_home);
        }
        text
    }
}

/// Whether a word names a ledger ref or branch: `tsk/ledger`, `refs/heads/tsk/docs`,
/// `origin/tsk/ledger`, `refs/tsk/ledger`, `+tsk/ledger:refs/remotes/origin/tsk/ledger`,
/// `main..tsk/ledger`.
fn names_ref(word: &str) -> bool {
    let word = word.replace("...", ":").replace("..", ":");
    word.split(':').any(|part| {
        let part = part.trim_start_matches('+');
        if let Some(rest) = part.strip_prefix("refs/tsk/") {
            return matches!(rest, "ledger" | "docs" | "bootstrap");
        }
        let mut part = part;
        for prefix in [
            "refs/remotes/origin/",
            "refs/remotes/",
            "refs/heads/",
            "origin/",
        ] {
            if let Some(rest) = part.strip_prefix(prefix) {
                part = rest;
                break;
            }
        }
        LEDGER_REFS.contains(&part)
    })
}

/// The words of a command, split into simple commands. A quote, a backslash escape and a
/// `$(` or backtick are read, and a redirect is a word `>` followed by its target.
fn segments(command: &str) -> Vec<Vec<String>> {
    let mut all: Vec<Vec<String>> = Vec::new();
    let mut words: Vec<String> = Vec::new();
    let mut word = String::new();
    let mut in_word = false;
    let mut chars = command.chars().peekable();

    macro_rules! end_word {
        () => {
            if in_word {
                words.push(std::mem::take(&mut word));
                in_word = false;
            }
        };
    }
    macro_rules! end_segment {
        () => {
            end_word!();
            if !words.is_empty() {
                all.push(std::mem::take(&mut words));
            }
        };
    }

    while let Some(c) = chars.next() {
        match c {
            '\'' => {
                in_word = true;
                for q in chars.by_ref() {
                    if q == '\'' {
                        break;
                    }
                    word.push(q);
                }
            }
            '"' => {
                in_word = true;
                while let Some(q) = chars.next() {
                    match q {
                        '"' => break,
                        '\\' => {
                            if let Some(next) = chars.next() {
                                word.push(next);
                            }
                        }
                        _ => word.push(q),
                    }
                }
            }
            '\\' => {
                in_word = true;
                if let Some(next) = chars.next() {
                    word.push(next);
                }
            }
            ' ' | '\t' => {
                end_word!();
            }
            ';' | '&' | '|' | '\n' | '(' | ')' | '`' => {
                end_segment!();
            }
            '>' | '<' => {
                end_word!();
                while chars.peek() == Some(&'>') {
                    chars.next();
                }
                words.push(c.to_string());
            }
            _ => {
                in_word = true;
                word.push(c);
            }
        }
    }
    if in_word {
        words.push(word);
    }
    if !words.is_empty() {
        all.push(words);
    }
    all
}

fn is_assignment(word: &str) -> bool {
    match word.split_once('=') {
        Some((name, _)) => {
            !name.is_empty()
                && !name.starts_with(|c: char| c.is_ascii_digit())
                && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        }
        None => false,
    }
}

/// The command word of a simple command, and its arguments, after redirects, variable
/// assignments and wrappers. A redirect target that names the ledger store is returned as
/// `Err`.
fn split_command<'a>(
    segment: &'a [String],
    places: &Places,
) -> Result<Option<(String, Vec<&'a str>)>, ()> {
    let mut plain: Vec<&str> = Vec::new();
    let mut iter = segment.iter();
    while let Some(word) = iter.next() {
        if word == ">" || word == "<" {
            if let Some(target) = iter.next() {
                if word == ">" && places.names_path(target) {
                    return Err(());
                }
            }
            continue;
        }
        plain.push(word);
    }
    let mut at = 0;
    while at < plain.len() && (is_assignment(plain[at]) || WRAPPERS.contains(&plain[at])) {
        at += 1;
    }
    if at >= plain.len() {
        return Ok(None);
    }
    let name = plain[at]
        .rsplit('/')
        .next()
        .unwrap_or(plain[at])
        .to_string();
    Ok(Some((name, plain[at + 1..].to_vec())))
}

fn command_denied(command: &str, cwd: &str, places: &Places) -> bool {
    let mut in_ledger = places.contains_dir(cwd);
    for segment in segments(command) {
        let (name, args) = match split_command(&segment, places) {
            Err(()) => return true,
            Ok(None) => continue,
            Ok(Some(parsed)) => parsed,
        };
        match name.as_str() {
            "tsk" => continue,
            "cd" | "pushd" => {
                if args.iter().any(|a| places.names_path(a)) {
                    in_ledger = true;
                }
                continue;
            }
            n if HARMLESS.contains(&n) => continue,
            "git" | "dolt" => {
                // A word with a space is a message or a script, not a path or a ref.
                let names_ledger = |a: &&str| {
                    !a.contains(char::is_whitespace) && (names_ref(a) || places.names_path(a))
                };
                if in_ledger || args.iter().any(names_ledger) {
                    return true;
                }
            }
            _ => {
                if in_ledger || args.iter().any(|a| places.names_path(a)) {
                    return true;
                }
            }
        }
    }
    false
}

/// The reason to deny the tool call in the hook input, or `None` to allow it.
pub fn check(input: &str, places: &Places) -> Option<&'static str> {
    let input: HookInput = serde_json::from_str(input).ok()?;
    let denied = match input.tool_name.as_str() {
        "Bash" => input
            .tool_input
            .get("command")
            .and_then(|v| v.as_str())
            .is_some_and(|command| command_denied(command, &input.cwd, places)),
        "Edit" | "Write" | "MultiEdit" | "NotebookEdit" => ["file_path", "notebook_path"]
            .iter()
            .filter_map(|key| input.tool_input.get(key).and_then(|v| v.as_str()))
            .any(|path| places.names_path(path)),
        _ => false,
    };
    denied.then_some(REASON)
}

/// The deny decision in the form a `PreToolUse` hook prints.
pub fn deny_decision(reason: &str) -> String {
    serde_json::json!({
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "deny",
            "permissionDecisionReason": reason,
        }
    })
    .to_string()
}

/// `tsk ledger guard`: prints a deny decision, or nothing. Always exits 0.
pub fn run() -> Result<(), String> {
    let mut input = String::new();
    let _ = std::io::stdin().read_to_string(&mut input);
    if std::env::var(SWITCH_ENV).is_ok_and(|v| v == "off") {
        return Ok(());
    }
    if let Some(reason) = check(&input, &Places::from_env()) {
        println!("{}", deny_decision(reason));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn places() -> Places {
        Places {
            prefixes: vec!["/home/me/.local/state/tsk/repos".to_string()],
            home: "/home/me".to_string(),
            xdg_state_home: String::new(),
        }
    }

    fn bash(command: &str) -> Option<&'static str> {
        bash_in(command, "/work/repo")
    }

    fn bash_in(command: &str, cwd: &str) -> Option<&'static str> {
        let input = serde_json::json!({
            "tool_name": "Bash",
            "tool_input": {"command": command},
            "cwd": cwd,
        });
        check(&input.to_string(), &places())
    }

    fn tool(name: &str, key: &str, path: &str) -> Option<&'static str> {
        let input = serde_json::json!({"tool_name": name, "tool_input": {key: path}});
        check(&input.to_string(), &places())
    }

    #[test]
    fn git_against_a_ledger_ref_is_denied() {
        for command in [
            "git fetch origin refs/heads/tsk/ledger",
            "git fetch origin tsk/ledger",
            "git push origin HEAD:refs/heads/tsk/docs",
            "git push +tsk/ledger:refs/tsk/ledger",
            "git log origin/tsk/ledger",
            "git diff main..tsk/ledger",
            "git branch -D tsk/docs",
            "git fetch origin refs/tsk/bootstrap",
            "cd /work/repo && git reset --hard origin/tsk/ledger",
            "echo done; git rebase tsk/ledger",
        ] {
            assert!(bash(command).is_some(), "{}", command);
        }
    }

    #[test]
    fn git_and_dolt_against_the_ledger_store_are_denied() {
        for command in [
            "git -C /home/me/.local/state/tsk/repos/tsk-abc/docs status",
            "git -C ~/.local/state/tsk/repos/tsk-abc/docs status",
            "git -C \"$TSK_LEDGER_WT\" status",
            "dolt --data-dir /home/me/.local/state/tsk/repos/tsk-abc/ledger-db sql",
            "cd $TSK_LEDGER_WT && git status",
            "cd /home/me/.local/state/tsk/repos/tsk-abc/docs; dolt sql -q 'select 1'",
            "WT=$(tsk ledger path); git -C ledger-db log",
        ] {
            assert!(bash(command).is_some(), "{}", command);
        }
        assert!(bash_in("git status", "/home/me/.local/state/tsk/repos/tsk-abc/docs").is_some());
        assert!(bash_in(
            "dolt sql",
            "/home/me/.local/state/tsk/repos/tsk-abc/ledger-db"
        )
        .is_some());
    }

    #[test]
    fn other_commands_on_the_ledger_store_are_denied() {
        for command in [
            "rm -rf /home/me/.local/state/tsk/repos/tsk-abc/ledger-db",
            "sed -i s/a/b/ \"$TSK_LEDGER_WT/missions/a.md\"",
            "ls /home/me/.local/state/tsk/repos/tsk-abc/",
            "cat ~/.local/state/tsk/repos/tsk-abc/docs/missions/a.md",
            "echo x > /home/me/.local/state/tsk/repos/tsk-abc/docs/a.md",
            "echo x >> $TSK_LEDGER_WT/a.md",
            "cat a.md | tee /home/me/.local/state/tsk/repos/tsk-abc/docs/a.md",
        ] {
            assert!(bash(command).is_some(), "{}", command);
        }
    }

    #[test]
    fn tsk_commands_and_code_worktree_git_are_allowed() {
        for command in [
            "tsk ledger push \"Claim M-X T-01\"",
            "tsk ledger fetch",
            "tsk ledger doc get missions/a.md",
            "tsk ledger doc put missions/a.md /tmp/a.md",
            "git status --porcelain",
            "git branch -r --contains HEAD",
            "git push origin main",
            "git log main..origin/main",
            "git fetch origin refs/heads/main",
            "git commit -m \"Explain tsk/ledger and the ledger store at /home/me/.local/state/tsk/repos/x\"",
            "WT=\"${TSK_LEDGER_WT:-$(tsk ledger fetch)}\"",
            "cat \"$(tsk ledger doc path missions/a.md)\"",
            "echo $TSK_LEDGER_WT",
            "TSK_LEDGER_GUARD=off tsk ledger status",
            "cargo test -q 2>&1 | tail -3",
            "git add docs && git commit -q -m x",
        ] {
            assert!(bash(command).is_none(), "{}", command);
        }
    }

    #[test]
    fn a_file_tool_on_the_ledger_store_is_denied() {
        let path = "/home/me/.local/state/tsk/repos/tsk-abc/docs/missions/a.md";
        for name in ["Edit", "Write", "MultiEdit"] {
            assert!(tool(name, "file_path", path).is_some(), "{}", name);
        }
        assert!(tool("NotebookEdit", "notebook_path", path).is_some());
        assert!(tool(
            "Write",
            "file_path",
            "/home/me/.local/state/tsk/repos/x/ledger-db/f"
        )
        .is_some());
        assert!(tool("Write", "file_path", "/work/repo/docs/a.md").is_none());
        assert!(tool("Read", "file_path", path).is_none());
    }

    #[test]
    fn a_malformed_input_is_allowed() {
        assert!(check("", &places()).is_none());
        assert!(check("not json", &places()).is_none());
        assert!(check("{\"tool_name\":\"Bash\"}", &places()).is_none());
    }

    #[test]
    fn the_decision_is_a_pretooluse_deny() {
        let value: serde_json::Value = serde_json::from_str(&deny_decision("why")).unwrap();
        assert_eq!(value["hookSpecificOutput"]["hookEventName"], "PreToolUse");
        assert_eq!(value["hookSpecificOutput"]["permissionDecision"], "deny");
        assert_eq!(
            value["hookSpecificOutput"]["permissionDecisionReason"],
            "why"
        );
    }
}
