use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use clap::{Args, ValueEnum};

pub const CLAUDE_BIN_VAR: &str = "TSK_CLAUDE_BIN";
pub const DEFAULT_MARKETPLACE_SOURCE: &str = "jimbarritt/claude-plugins";
pub const DEFAULT_MARKETPLACE_NAME: &str = "jimbarritt-claude-plugins";
pub const DEFAULT_PLUGIN: &str = "tsk";

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Target {
    ClaudeCli,
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
    #[arg(value_enum, help = "The CLI to install the plugins into")]
    pub target: Target,
    #[arg(
        long,
        value_enum,
        default_value = "project",
        help = "Scope the plugins are installed and updated at"
    )]
    pub scope: Scope,
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
        default_value = DEFAULT_MARKETPLACE_SOURCE,
        help = "Marketplace source passed to claude plugin marketplace add"
    )]
    pub marketplace: String,
    #[arg(
        long,
        value_name = "NAME",
        default_value = DEFAULT_MARKETPLACE_NAME,
        help = "Name the marketplace is registered under"
    )]
    pub marketplace_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub description: String,
    pub args: Vec<String>,
}

fn step(description: String, args: &[&str]) -> Step {
    Step {
        description,
        args: args.iter().map(|a| a.to_string()).collect(),
    }
}

pub fn steps(args: &InstallPluginArgs) -> Vec<Step> {
    let source = args.marketplace.as_str();
    let name = args.marketplace_name.as_str();
    let scope = args.scope.as_str();
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

pub fn find_claude(env_override: Option<OsString>, path: Option<OsString>) -> Option<PathBuf> {
    if let Some(bin) = env_override.filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(bin));
    }
    std::env::split_paths(&path?)
        .map(|dir| dir.join("claude"))
        .find(|candidate| candidate.is_file())
}

pub fn run_steps(
    bin: &Path,
    steps: &[Step],
    runner: &mut dyn FnMut(&Path, &[String]) -> std::io::Result<Output>,
    report: &mut dyn FnMut(String),
) -> Result<(), String> {
    for step in steps {
        let command = format!("claude {}", step.args.join(" "));
        let output = runner(bin, &step.args).map_err(|e| {
            format!(
                "error: could not {}: could not run {}: {}",
                step.description,
                bin.display(),
                e
            )
        })?;
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

pub fn run(args: InstallPluginArgs) -> Result<(), String> {
    let Target::ClaudeCli = args.target;
    let bin = find_claude(std::env::var_os(CLAUDE_BIN_VAR), std::env::var_os("PATH")).ok_or_else(
        || {
            format!(
                "error: claude is not on PATH; install Claude Code, or set {} to the claude binary",
                CLAUDE_BIN_VAR
            )
        },
    )?;
    run_steps(
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
            scope,
            plugins: plugins.iter().map(|p| p.to_string()).collect(),
            marketplace: DEFAULT_MARKETPLACE_SOURCE.to_string(),
            marketplace_name: DEFAULT_MARKETPLACE_NAME.to_string(),
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
        custom.marketplace = "acme/plugins".to_string();
        custom.marketplace_name = "acme".to_string();
        let lines = lines(&steps(&custom));
        assert_eq!(lines[0], "plugin marketplace add acme/plugins");
        assert_eq!(lines[1], "plugin install p@acme --scope user -y");
        assert_eq!(lines[2], "plugin marketplace update acme");
    }

    #[test]
    fn find_claude_prefers_the_override_then_searches_path() {
        let dir = tempfile::tempdir().unwrap();
        let empty = dir.path().join("empty");
        let bin = dir.path().join("bin");
        std::fs::create_dir_all(&empty).unwrap();
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(bin.join("claude"), "").unwrap();
        let path = std::env::join_paths([&empty, &bin]).unwrap();

        assert_eq!(
            find_claude(Some("/x/claude".into()), Some(path.clone())),
            Some(PathBuf::from("/x/claude"))
        );
        assert_eq!(
            find_claude(Some("".into()), Some(path)),
            Some(bin.join("claude"))
        );
        assert_eq!(find_claude(None, Some(empty.into_os_string())), None);
        assert_eq!(find_claude(None, None), None);
    }

    #[test]
    fn run_steps_reports_one_line_per_step() {
        let steps = steps(&args(Scope::Project, &["tsk"]));
        let mut seen = Vec::new();
        let mut report = Vec::new();
        run_steps(
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
            Path::new("/no/claude"),
            &steps,
            &mut |_, _| Err(std::io::Error::from(std::io::ErrorKind::NotFound)),
            &mut |_| {},
        )
        .unwrap_err();
        assert!(err.starts_with("error: could not add the jimbarritt/claude-plugins marketplace: could not run /no/claude"), "{}", err);
    }
}
