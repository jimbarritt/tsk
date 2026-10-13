use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use clap::{Args, ValueEnum};

pub const CLAUDE_BIN_VAR: &str = "TSK_CLAUDE_BIN";
pub const DEFAULT_MARKETPLACE_SOURCE: &str = "jimbarritt/claude-plugins";
pub const DEFAULT_MARKETPLACE_NAME: &str = "jimbarritt-claude-plugins";
pub const COPILOT_BIN_VAR: &str = "TSK_COPILOT_BIN";
pub const COPILOT_MARKETPLACE_SOURCE: &str = "jimbarritt/tsk";
pub const COPILOT_MARKETPLACE_NAME: &str = "tsk";
pub const DEFAULT_PLUGIN: &str = "tsk";

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Target {
    ClaudeCli,
    CopilotCli,
}

impl Target {
    pub fn binary(self) -> &'static str {
        match self {
            Target::ClaudeCli => "claude",
            Target::CopilotCli => "copilot",
        }
    }

    pub fn bin_var(self) -> &'static str {
        match self {
            Target::ClaudeCli => CLAUDE_BIN_VAR,
            Target::CopilotCli => COPILOT_BIN_VAR,
        }
    }

    fn product(self) -> &'static str {
        match self {
            Target::ClaudeCli => "Claude Code",
            Target::CopilotCli => "GitHub Copilot CLI",
        }
    }

    fn default_marketplace_source(self) -> &'static str {
        match self {
            Target::ClaudeCli => DEFAULT_MARKETPLACE_SOURCE,
            Target::CopilotCli => COPILOT_MARKETPLACE_SOURCE,
        }
    }

    fn default_marketplace_name(self) -> &'static str {
        match self {
            Target::ClaudeCli => DEFAULT_MARKETPLACE_NAME,
            Target::CopilotCli => COPILOT_MARKETPLACE_NAME,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Scope {
    Project,
    Local,
    User,
}

impl Scope {
    pub fn as_str(self) -> &'static str {
        match self {
            Scope::Project => "project",
            Scope::Local => "local",
            Scope::User => "user",
        }
    }
}

#[derive(Debug, Args)]
pub struct InstallPluginArgs {
    #[arg(
        value_enum,
        help = "The CLI to install the plugins into: claude-cli (Claude Code) or copilot-cli (GitHub Copilot CLI)"
    )]
    pub target: Target,
    #[arg(
        long,
        value_enum,
        help = "Scope the plugins are installed and updated at (claude-cli only, default project; copilot-cli installs per user and rejects this option)"
    )]
    pub scope: Option<Scope>,
    #[arg(
        long = "plugin",
        value_name = "NAME",
        default_value = DEFAULT_PLUGIN,
        help = "Plugin to install and update; repeat for more than one"
    )]
    pub plugins: Vec<String>,
    #[arg(
        long,
        value_name = "SOURCE",
        help = "Marketplace source passed to plugin marketplace add (default jimbarritt/claude-plugins for claude-cli, jimbarritt/tsk for copilot-cli)"
    )]
    pub marketplace: Option<String>,
    #[arg(
        long,
        value_name = "NAME",
        help = "Name the marketplace is registered under (default jimbarritt-claude-plugins for claude-cli, tsk for copilot-cli)"
    )]
    pub marketplace_name: Option<String>,
    #[arg(
        long,
        help = "Do not write the SessionStart hook into .claude/settings.json (claude-cli only)"
    )]
    pub no_hook: bool,
}

impl InstallPluginArgs {
    pub fn marketplace_source(&self) -> &str {
        self.marketplace
            .as_deref()
            .unwrap_or_else(|| self.target.default_marketplace_source())
    }

    pub fn marketplace_name(&self) -> &str {
        self.marketplace_name
            .as_deref()
            .unwrap_or_else(|| self.target.default_marketplace_name())
    }

    fn claude_scope(&self) -> Scope {
        self.scope.unwrap_or(Scope::Project)
    }

    /// The arguments of `tsk install-plugin claude-cli` with every default.
    pub fn claude_defaults() -> InstallPluginArgs {
        InstallPluginArgs {
            target: Target::ClaudeCli,
            scope: None,
            plugins: vec![DEFAULT_PLUGIN.to_string()],
            marketplace: None,
            marketplace_name: None,
            no_hook: false,
        }
    }
}

pub const HOOK_COMMAND: &str = "tsk thread session-start --install-plugin claude-cli";
const HOOK_MARKER: &str = "tsk thread session-start";
pub const SETTINGS_PATH: &str = ".claude/settings.json";

/// A JSON value whose objects keep their key order. serde_json's `preserve_order`
/// feature would change the key order of every other `Value` the binary prints or
/// writes, so the settings merge keeps its own.
#[derive(Debug, Clone, PartialEq)]
enum Ordered {
    Null,
    Bool(bool),
    Number(serde_json::Number),
    Text(String),
    List(Vec<Ordered>),
    Object(Vec<(String, Ordered)>),
}

impl<'de> serde::Deserialize<'de> for Ordered {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Ordered, D::Error> {
        struct Visit;
        impl<'de> serde::de::Visitor<'de> for Visit {
            type Value = Ordered;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("any JSON value")
            }
            fn visit_unit<E>(self) -> Result<Ordered, E> {
                Ok(Ordered::Null)
            }
            fn visit_bool<E>(self, v: bool) -> Result<Ordered, E> {
                Ok(Ordered::Bool(v))
            }
            fn visit_i64<E>(self, v: i64) -> Result<Ordered, E> {
                Ok(Ordered::Number(v.into()))
            }
            fn visit_u64<E>(self, v: u64) -> Result<Ordered, E> {
                Ok(Ordered::Number(v.into()))
            }
            fn visit_f64<E>(self, v: f64) -> Result<Ordered, E> {
                Ok(serde_json::Number::from_f64(v).map_or(Ordered::Null, Ordered::Number))
            }
            fn visit_str<E>(self, v: &str) -> Result<Ordered, E> {
                Ok(Ordered::Text(v.to_string()))
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> Result<Ordered, A::Error> {
                let mut items = Vec::new();
                while let Some(item) = seq.next_element()? {
                    items.push(item);
                }
                Ok(Ordered::List(items))
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<Ordered, A::Error> {
                let mut entries = Vec::new();
                while let Some(entry) = map.next_entry()? {
                    entries.push(entry);
                }
                Ok(Ordered::Object(entries))
            }
        }
        deserializer.deserialize_any(Visit)
    }
}

impl serde::Serialize for Ordered {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::{SerializeMap, SerializeSeq};
        match self {
            Ordered::Null => serializer.serialize_unit(),
            Ordered::Bool(v) => serializer.serialize_bool(*v),
            Ordered::Number(v) => v.serialize(serializer),
            Ordered::Text(v) => serializer.serialize_str(v),
            Ordered::List(items) => {
                let mut seq = serializer.serialize_seq(Some(items.len()))?;
                for item in items {
                    seq.serialize_element(item)?;
                }
                seq.end()
            }
            Ordered::Object(entries) => {
                let mut map = serializer.serialize_map(Some(entries.len()))?;
                for (key, value) in entries {
                    map.serialize_entry(key, value)?;
                }
                map.end()
            }
        }
    }
}

impl Ordered {
    fn text(value: &str) -> Ordered {
        Ordered::Text(value.to_string())
    }

    fn get(&self, key: &str) -> Option<&Ordered> {
        match self {
            Ordered::Object(entries) => entries.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }
}

/// The entry for `key` in an object, added with `default` when absent.
fn slot<'a>(
    entries: &'a mut Vec<(String, Ordered)>,
    key: &str,
    default: Ordered,
) -> &'a mut Ordered {
    let index = match entries.iter().position(|(k, _)| k == key) {
        Some(index) => index,
        None => {
            entries.push((key.to_string(), default));
            entries.len() - 1
        }
    };
    &mut entries[index].1
}

/// Adds the repo's `SessionStart` hook to the text of a settings file. Returns `None`
/// when a `SessionStart` command already contains `tsk thread session-start`. Keeps
/// every other key and hook entry, in their order.
pub fn merge_hook(existing: Option<&str>) -> Result<Option<String>, String> {
    let bad = |what: &str| format!("error: {} is malformed: {}", SETTINGS_PATH, what);
    let mut root = match existing.map(str::trim).filter(|t| !t.is_empty()) {
        None => Ordered::Object(Vec::new()),
        Some(text) => serde_json::from_str::<Ordered>(text).map_err(|e| bad(&e.to_string()))?,
    };
    let Ordered::Object(top) = &mut root else {
        return Err(bad("the top level is not an object"));
    };
    let Ordered::Object(hooks) = slot(top, "hooks", Ordered::Object(Vec::new())) else {
        return Err(bad("\"hooks\" is not an object"));
    };
    let Ordered::List(session_start) = slot(hooks, "SessionStart", Ordered::List(Vec::new()))
    else {
        return Err(bad("\"hooks.SessionStart\" is not an array"));
    };
    let present = session_start.iter().any(|entry| {
        matches!(entry.get("hooks"), Some(Ordered::List(inner)) if inner.iter().any(|hook| {
            matches!(hook.get("command"), Some(Ordered::Text(c)) if c.contains(HOOK_MARKER))
        }))
    });
    if present {
        return Ok(None);
    }
    session_start.push(Ordered::Object(vec![(
        "hooks".to_string(),
        Ordered::List(vec![Ordered::Object(vec![
            ("type".to_string(), Ordered::text("command")),
            ("command".to_string(), Ordered::text(HOOK_COMMAND)),
        ])]),
    )]));
    let mut text = serde_json::to_string_pretty(&root).map_err(|e| e.to_string())?;
    text.push('\n');
    Ok(Some(text))
}

fn repo_root() -> Result<PathBuf, String> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .map_err(|e| format!("error: could not run git to find the repo root: {}", e))?;
    if !output.status.success() {
        return Err(
            "error: not in a git repository: the SessionStart hook is written to <repo root>/.claude/settings.json (use --no-hook to skip it)"
                .to_string(),
        );
    }
    Ok(PathBuf::from(
        String::from_utf8_lossy(&output.stdout).trim_end(),
    ))
}

/// Writes the hook under `root`. Returns the line to print.
pub fn write_hook(root: &Path) -> Result<(String, bool), String> {
    let path = root.join(SETTINGS_PATH);
    let existing = match std::fs::read_to_string(&path) {
        Ok(text) => Some(text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(format!("error: could not read {}: {}", path.display(), e)),
    };
    match merge_hook(existing.as_deref())? {
        None => Ok((format!("hook already present: {}", SETTINGS_PATH), false)),
        Some(text) => {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)
                    .map_err(|e| format!("error: could not create {}: {}", dir.display(), e))?;
            }
            std::fs::write(&path, text)
                .map_err(|e| format!("error: could not write {}: {}", path.display(), e))?;
            Ok((
                format!("wrote the SessionStart hook to {}", SETTINGS_PATH),
                true,
            ))
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub description: String,
    pub args: Vec<String>,
    /// A failure whose output says "already" counts as success (the step is idempotent).
    pub tolerate_already: bool,
}

fn step(description: String, args: &[&str]) -> Step {
    Step {
        description,
        args: args.iter().map(|a| a.to_string()).collect(),
        tolerate_already: false,
    }
}

fn tolerant(mut step: Step) -> Step {
    step.tolerate_already = true;
    step
}

pub fn steps(args: &InstallPluginArgs) -> Vec<Step> {
    match args.target {
        Target::ClaudeCli => claude_steps(args),
        Target::CopilotCli => copilot_steps(args),
    }
}

fn claude_steps(args: &InstallPluginArgs) -> Vec<Step> {
    let source = args.marketplace_source();
    let name = args.marketplace_name();
    let scope = args.claude_scope().as_str();
    let mut steps = vec![step(
        format!("add the {} marketplace", source),
        &["plugin", "marketplace", "add", source],
    )];
    for plugin in &args.plugins {
        let qualified = format!("{}@{}", plugin, name);
        steps.push(step(
            format!("install {} at {} scope", qualified, scope),
            &["plugin", "install", &qualified, "--scope", scope, "-y"],
        ));
    }
    steps.push(step(
        format!("refresh the {} marketplace", name),
        &["plugin", "marketplace", "update", name],
    ));
    for plugin in &args.plugins {
        let qualified = format!("{}@{}", plugin, name);
        steps.push(step(
            format!("update {} at {} scope", qualified, scope),
            &["plugin", "update", &qualified, "--scope", scope],
        ));
    }
    steps
}

fn copilot_steps(args: &InstallPluginArgs) -> Vec<Step> {
    let source = args.marketplace_source();
    let name = args.marketplace_name();
    let mut steps = vec![tolerant(step(
        format!("add the {} marketplace", source),
        &["plugin", "marketplace", "add", source],
    ))];
    for plugin in &args.plugins {
        let qualified = format!("{}@{}", plugin, name);
        steps.push(tolerant(step(
            format!("install {}", qualified),
            &["plugin", "install", &qualified],
        )));
    }
    steps.push(step(
        format!("refresh the {} marketplace", name),
        &["plugin", "marketplace", "update", name],
    ));
    for plugin in &args.plugins {
        steps.push(step(
            format!("update {}", plugin),
            &["plugin", "update", plugin],
        ));
    }
    steps
}

pub fn validate(args: &InstallPluginArgs) -> Result<(), String> {
    if args.target == Target::CopilotCli && args.scope.is_some() {
        return Err(
            "error: --scope does not apply to copilot-cli: GitHub Copilot CLI installs plugins per user"
                .to_string(),
        );
    }
    Ok(())
}

pub fn find_binary(
    name: &str,
    env_override: Option<OsString>,
    path: Option<OsString>,
) -> Option<PathBuf> {
    if let Some(bin) = env_override.filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(bin));
    }
    std::env::split_paths(&path?)
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file())
}

pub fn run_steps(
    program: &str,
    bin: &Path,
    steps: &[Step],
    runner: &mut dyn FnMut(&Path, &[String]) -> std::io::Result<Output>,
    report: &mut dyn FnMut(String),
) -> Result<(), String> {
    for step in steps {
        let command = format!("{} {}", program, step.args.join(" "));
        let output = runner(bin, &step.args).map_err(|e| {
            format!(
                "error: could not {}: could not run {}: {}",
                step.description,
                bin.display(),
                e
            )
        })?;
        if !output.status.success() && step.tolerate_already && says_already(&output) {
            report(format!(
                "{}: {} (already done, continuing)",
                step.description, command
            ));
            continue;
        }
        if !output.status.success() {
            let status = match output.status.code() {
                Some(code) => format!("exit status {}", code),
                None => "terminated by a signal".to_string(),
            };
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stderr = stderr.trim();
            let mut message = format!(
                "error: could not {}: {} failed ({})",
                step.description, command, status
            );
            if !stderr.is_empty() {
                message.push_str(":\n");
                message.push_str(stderr);
            }
            return Err(message);
        }
        report(format!("{}: {}", step.description, command));
    }
    Ok(())
}

fn says_already(output: &Output) -> bool {
    [&output.stderr, &output.stdout].iter().any(|text| {
        String::from_utf8_lossy(text)
            .to_lowercase()
            .contains("already")
    })
}

/// Finds the CLI and runs the install steps for `args`, reporting each step line.
pub fn install(args: &InstallPluginArgs, report: &mut dyn FnMut(String)) -> Result<(), String> {
    validate(args)?;
    let target = args.target;
    let bin = find_binary(
        target.binary(),
        std::env::var_os(target.bin_var()),
        std::env::var_os("PATH"),
    )
    .ok_or_else(|| {
        format!(
            "error: {} is not on PATH; install {}, or set {} to the {} binary",
            target.binary(),
            target.product(),
            target.bin_var(),
            target.binary()
        )
    })?;
    run_steps(
        target.binary(),
        &bin,
        &steps(args),
        &mut |bin, args| Command::new(bin).args(args).output(),
        report,
    )
}

pub fn run(args: InstallPluginArgs) -> Result<(), String> {
    install(&args, &mut |line| println!("{}", line))?;
    if args.target != Target::ClaudeCli {
        return Ok(());
    }
    if args.claude_scope() == Scope::User {
        println!("skipped the SessionStart hook: a user-scope install is not about this repo");
        return Ok(());
    }
    if args.no_hook {
        return Ok(());
    }
    let (line, wrote) = write_hook(&repo_root()?)?;
    println!("{}", line);
    if wrote {
        println!(
            "commit {} so cloud sessions install the plugin",
            SETTINGS_PATH
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::process::ExitStatusExt;
    use std::process::ExitStatus;

    fn args(scope: Scope, plugins: &[&str]) -> InstallPluginArgs {
        InstallPluginArgs {
            target: Target::ClaudeCli,
            scope: Some(scope),
            plugins: plugins.iter().map(|p| p.to_string()).collect(),
            marketplace: None,
            marketplace_name: None,
            no_hook: false,
        }
    }

    fn copilot_args(plugins: &[&str]) -> InstallPluginArgs {
        InstallPluginArgs {
            target: Target::CopilotCli,
            scope: None,
            plugins: plugins.iter().map(|p| p.to_string()).collect(),
            marketplace: None,
            marketplace_name: None,
            no_hook: false,
        }
    }

    fn lines(steps: &[Step]) -> Vec<String> {
        steps.iter().map(|s| s.args.join(" ")).collect()
    }

    fn output(code: i32, stderr: &str) -> Output {
        Output {
            status: ExitStatus::from_raw(code << 8),
            stdout: Vec::new(),
            stderr: stderr.as_bytes().to_vec(),
        }
    }

    #[test]
    fn steps_mirror_the_session_start_script() {
        assert_eq!(
            lines(&steps(&args(Scope::Project, &["tsk"]))),
            vec![
                "plugin marketplace add jimbarritt/claude-plugins",
                "plugin install tsk@jimbarritt-claude-plugins --scope project -y",
                "plugin marketplace update jimbarritt-claude-plugins",
                "plugin update tsk@jimbarritt-claude-plugins --scope project",
            ]
        );
    }

    #[test]
    fn each_plugin_is_installed_then_each_is_updated_after_the_refresh() {
        assert_eq!(
            lines(&steps(&args(Scope::Local, &["swe", "tsk"]))),
            vec![
                "plugin marketplace add jimbarritt/claude-plugins",
                "plugin install swe@jimbarritt-claude-plugins --scope local -y",
                "plugin install tsk@jimbarritt-claude-plugins --scope local -y",
                "plugin marketplace update jimbarritt-claude-plugins",
                "plugin update swe@jimbarritt-claude-plugins --scope local",
                "plugin update tsk@jimbarritt-claude-plugins --scope local",
            ]
        );
    }

    #[test]
    fn a_custom_marketplace_names_both_source_and_name() {
        let mut custom = args(Scope::User, &["p"]);
        custom.marketplace = Some("acme/plugins".to_string());
        custom.marketplace_name = Some("acme".to_string());
        let lines = lines(&steps(&custom));
        assert_eq!(lines[0], "plugin marketplace add acme/plugins");
        assert_eq!(lines[1], "plugin install p@acme --scope user -y");
        assert_eq!(lines[2], "plugin marketplace update acme");
    }

    #[test]
    fn find_binary_prefers_the_override_then_searches_path() {
        let dir = tempfile::tempdir().unwrap();
        let empty = dir.path().join("empty");
        let bin = dir.path().join("bin");
        std::fs::create_dir_all(&empty).unwrap();
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(bin.join("claude"), "").unwrap();
        let path = std::env::join_paths([&empty, &bin]).unwrap();

        assert_eq!(
            find_binary("claude", Some("/x/claude".into()), Some(path.clone())),
            Some(PathBuf::from("/x/claude"))
        );
        assert_eq!(
            find_binary("claude", Some("".into()), Some(path)),
            Some(bin.join("claude"))
        );
        assert_eq!(
            find_binary("claude", None, Some(empty.into_os_string())),
            None
        );
        assert_eq!(find_binary("claude", None, None), None);
    }

    #[test]
    fn run_steps_reports_one_line_per_step() {
        let steps = steps(&args(Scope::Project, &["tsk"]));
        let mut seen = Vec::new();
        let mut report = Vec::new();
        run_steps(
            "claude",
            Path::new("claude"),
            &steps,
            &mut |_, args| {
                seen.push(args.join(" "));
                Ok(output(0, ""))
            },
            &mut |line| report.push(line),
        )
        .unwrap();
        assert_eq!(seen, lines(&steps));
        assert_eq!(report.len(), 4);
        assert_eq!(
            report[0],
            "add the jimbarritt/claude-plugins marketplace: claude plugin marketplace add jimbarritt/claude-plugins"
        );
    }

    #[test]
    fn run_steps_stops_at_a_failed_step_and_names_it_with_stderr() {
        let steps = steps(&args(Scope::Project, &["tsk"]));
        let mut calls = 0;
        let err = run_steps(
            "claude",
            Path::new("claude"),
            &steps,
            &mut |_, _| {
                calls += 1;
                Ok(if calls == 2 {
                    output(3, "plugin not found\n")
                } else {
                    output(0, "")
                })
            },
            &mut |_| {},
        )
        .unwrap_err();
        assert_eq!(calls, 2);
        assert_eq!(
            err,
            "error: could not install tsk@jimbarritt-claude-plugins at project scope: claude plugin install tsk@jimbarritt-claude-plugins --scope project -y failed (exit status 3):\nplugin not found"
        );
    }

    #[test]
    fn run_steps_reports_a_binary_that_cannot_start() {
        let steps = steps(&args(Scope::Project, &["tsk"]));
        let err = run_steps(
            "claude",
            Path::new("/no/claude"),
            &steps,
            &mut |_, _| Err(std::io::Error::from(std::io::ErrorKind::NotFound)),
            &mut |_| {},
        )
        .unwrap_err();
        assert!(err.starts_with("error: could not add the jimbarritt/claude-plugins marketplace: could not run /no/claude"), "{}", err);
    }

    #[test]
    fn copilot_steps_use_the_tsk_marketplace_and_no_scope() {
        assert_eq!(
            lines(&steps(&copilot_args(&["tsk", "swe"]))),
            vec![
                "plugin marketplace add jimbarritt/tsk",
                "plugin install tsk@tsk",
                "plugin install swe@tsk",
                "plugin marketplace update tsk",
                "plugin update tsk",
                "plugin update swe",
            ]
        );
        let mut custom = copilot_args(&["p"]);
        custom.marketplace = Some("acme/plugins".to_string());
        custom.marketplace_name = Some("acme".to_string());
        assert_eq!(lines(&steps(&custom))[1], "plugin install p@acme");
    }

    #[test]
    fn copilot_rejects_an_explicit_scope_but_not_the_default() {
        assert!(validate(&copilot_args(&["tsk"])).is_ok());
        let mut scoped = copilot_args(&["tsk"]);
        scoped.scope = Some(Scope::User);
        let err = validate(&scoped).unwrap_err();
        assert!(err.contains("installs plugins per user"), "{}", err);
        assert!(validate(&args(Scope::User, &["tsk"])).is_ok());
    }

    #[test]
    fn already_is_tolerated_on_copilot_add_and_install_only() {
        let steps = steps(&copilot_args(&["tsk"]));
        let flags: Vec<bool> = steps.iter().map(|s| s.tolerate_already).collect();
        assert_eq!(flags, vec![true, true, false, false]);
        let mut report = Vec::new();
        run_steps(
            "copilot",
            Path::new("copilot"),
            &steps[..2],
            &mut |_, _| Ok(output(1, "Marketplace tsk is Already registered")),
            &mut |line| report.push(line),
        )
        .unwrap();
        assert!(
            report[1].ends_with("(already done, continuing)"),
            "{:?}",
            report
        );
        let err = run_steps(
            "copilot",
            Path::new("copilot"),
            &steps[2..],
            &mut |_, _| Ok(output(1, "already up to date")),
            &mut |_| {},
        )
        .unwrap_err();
        assert!(
            err.contains("copilot plugin marketplace update tsk failed"),
            "{}",
            err
        );
    }

    const HOOK: &str = "{\"hooks\":[{\"type\":\"command\",\"command\":\"tsk thread session-start --install-plugin claude-cli\"}]}";

    fn parsed(text: &str) -> serde_json::Value {
        serde_json::from_str(text).unwrap()
    }

    #[test]
    fn merge_into_an_absent_file_writes_only_the_hook() {
        let text = merge_hook(None).unwrap().unwrap();
        assert!(text.ends_with("}\n"));
        assert_eq!(
            parsed(&text),
            serde_json::json!({"hooks": {"SessionStart": [parsed(HOOK)]}})
        );
        assert!(text.contains("\n  \"hooks\": {\n    \"SessionStart\": ["));
    }

    #[test]
    fn merge_keeps_other_keys_in_their_order() {
        let text = merge_hook(Some(
            "{\"permissions\":{\"allow\":[\"Bash\"]},\"env\":{\"A\":\"1\"},\"hooks\":{\"Stop\":[]}}",
        ))
        .unwrap()
        .unwrap();
        let at = |key: &str| text.find(&format!("\n  \"{}\"", key)).unwrap();
        assert!(at("permissions") < at("env") && at("env") < at("hooks"));
        let value = parsed(&text);
        assert_eq!(value["permissions"]["allow"][0], "Bash");
        assert_eq!(value["hooks"]["Stop"], serde_json::json!([]));
        assert_eq!(value["hooks"]["SessionStart"][0], parsed(HOOK));
    }

    #[test]
    fn merge_keeps_other_session_start_hooks_and_appends() {
        let text = merge_hook(Some(
            "{\"hooks\":{\"SessionStart\":[{\"hooks\":[{\"type\":\"command\",\"command\":\"./other.sh\"}]}]}}",
        ))
        .unwrap()
        .unwrap();
        let list = parsed(&text)["hooks"]["SessionStart"].clone();
        assert_eq!(list.as_array().unwrap().len(), 2);
        assert_eq!(list[0]["hooks"][0]["command"], "./other.sh");
        assert_eq!(list[1], parsed(HOOK));
    }

    #[test]
    fn merge_is_a_no_op_when_a_session_start_command_already_runs_tsk() {
        let existing = "{\"hooks\":{\"SessionStart\":[{\"hooks\":[{\"type\":\"command\",\"command\":\"tsk thread session-start\"}]}]}}";
        assert_eq!(merge_hook(Some(existing)).unwrap(), None);
        let first = merge_hook(None).unwrap().unwrap();
        assert_eq!(merge_hook(Some(&first)).unwrap(), None);
    }

    #[test]
    fn merge_refuses_malformed_settings() {
        for bad in [
            "{not json",
            "[]",
            "{\"hooks\":[]}",
            "{\"hooks\":{\"SessionStart\":{}}}",
        ] {
            let err = merge_hook(Some(bad)).unwrap_err();
            assert!(err.contains("malformed"), "{}: {}", bad, err);
        }
    }

    #[test]
    fn write_hook_creates_the_directory_and_leaves_a_malformed_file_alone() {
        let dir = tempfile::tempdir().unwrap();
        let (line, wrote) = write_hook(dir.path()).unwrap();
        assert!(wrote);
        assert_eq!(line, "wrote the SessionStart hook to .claude/settings.json");
        assert_eq!(
            write_hook(dir.path()).unwrap().0,
            "hook already present: .claude/settings.json"
        );
        let bad = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(bad.path().join(".claude")).unwrap();
        std::fs::write(bad.path().join(SETTINGS_PATH), "{oops").unwrap();
        assert!(write_hook(bad.path()).is_err());
        assert_eq!(
            std::fs::read_to_string(bad.path().join(SETTINGS_PATH)).unwrap(),
            "{oops"
        );
    }
}
