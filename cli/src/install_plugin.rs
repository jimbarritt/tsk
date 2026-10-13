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

pub fn run(args: InstallPluginArgs) -> Result<(), String> {
    validate(&args)?;
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
        &steps(&args),
        &mut |bin, args| Command::new(bin).args(args).output(),
        &mut |line| println!("{}", line),
    )
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
        }
    }

    fn copilot_args(plugins: &[&str]) -> InstallPluginArgs {
        InstallPluginArgs {
            target: Target::CopilotCli,
            scope: None,
            plugins: plugins.iter().map(|p| p.to_string()).collect(),
            marketplace: None,
            marketplace_name: None,
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
}
