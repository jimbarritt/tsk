//! Shell integration: `tsk shell-init <shell>` prints a shell function named `tsk`, and
//! `tsk install zshrc` adds one line to the zsh config that loads it.
//!
//! A program cannot change the directory of its parent shell, so `tsk harness cd <name>`
//! is a function: it runs `tsk harness path <name>` and calls the shell's own `cd` on the
//! result. Every other call goes to the binary.

use std::path::{Path, PathBuf};

use clap::Subcommand;

/// The line that marks the `eval` line in the zsh config.
pub const ZSHRC_MARKER: &str = "# tsk install zshrc: tsk harness cd";

/// The line that loads the function.
pub const ZSHRC_EVAL: &str = "eval \"$(tsk shell-init zsh)\"";

#[derive(Subcommand)]
pub enum InstallCommands {
    #[command(
        about = "Add one line to the zsh config, $ZDOTDIR/.zshrc or ~/.zshrc, that loads the tsk shell function: eval \"$(tsk shell-init zsh)\". The function makes tsk harness cd <name> change the directory of your shell. The command creates the file when it is absent, and changes nothing when the line is there"
    )]
    Zshrc {
        #[arg(long, help = "Print the lines and change no file")]
        print: bool,
    },
}

/// The shell code of `tsk shell-init`. The same text works in zsh and bash.
pub fn init_code() -> &'static str {
    "tsk() {\n\
     \x20 if [ \"$1\" = harness ] && [ \"$2\" = cd ]; then\n\
     \x20   if [ -z \"$3\" ]; then\n\
     \x20     echo \"usage: tsk harness cd <name>\" >&2\n\
     \x20     return 1\n\
     \x20   fi\n\
     \x20   local dir\n\
     \x20   dir=\"$(command tsk harness path \"$3\")\" || return\n\
     \x20   cd \"$dir\" || return\n\
     \x20 else\n\
     \x20   command tsk \"$@\"\n\
     \x20 fi\n\
     }\n"
}

/// Print the shell code for the named shell. An unknown shell is an error, exit 1.
pub fn run_shell_init(shell: &str) -> Result<i32, String> {
    match shell {
        "zsh" | "bash" => {
            print!("{}", init_code());
            Ok(0)
        }
        other => Err(format!(
            "error: tsk shell-init supports zsh and bash, not {}",
            other
        )),
    }
}

pub fn run_install(action: InstallCommands) -> Result<i32, String> {
    match action {
        InstallCommands::Zshrc { print } => install_zshrc(print).map(|()| 0),
    }
}

/// The lines that `tsk install zshrc` adds.
pub fn zshrc_lines() -> String {
    format!("{}\n{}\n", ZSHRC_MARKER, ZSHRC_EVAL)
}

/// The zsh config with the lines added at the end. `None` when the config has the marker.
pub fn with_zshrc_lines(conf: &str) -> Option<String> {
    if conf.lines().any(|line| line.trim() == ZSHRC_MARKER) {
        return None;
    }
    let mut out = conf.to_string();
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    if !out.is_empty() {
        out.push('\n');
    }
    out.push_str(&zshrc_lines());
    Some(out)
}

/// The zsh config: `$ZDOTDIR/.zshrc` when `ZDOTDIR` is set, else `~/.zshrc`.
fn zshrc_path() -> Result<PathBuf, String> {
    if let Some(dir) = std::env::var_os("ZDOTDIR").filter(|v| !v.is_empty()) {
        return Ok(Path::new(&dir).join(".zshrc"));
    }
    std::env::var_os("HOME")
        .filter(|h| !h.is_empty())
        .map(|h| PathBuf::from(h).join(".zshrc"))
        .ok_or_else(|| "error: HOME is not set".to_string())
}

fn install_zshrc(print: bool) -> Result<(), String> {
    let path = zshrc_path()?;
    if print {
        println!("# added to {}", path.display());
        print!("{}", zshrc_lines());
        return Ok(());
    }
    let current = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(format!("error: could not read {}: {}", path.display(), e)),
    };
    match with_zshrc_lines(&current) {
        Some(text) => {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)
                    .map_err(|e| format!("error: could not create {}: {}", dir.display(), e))?;
            }
            std::fs::write(&path, text)
                .map_err(|e| format!("error: could not write {}: {}", path.display(), e))?;
            println!("added the tsk shell function to {}", path.display());
            println!("Open a new shell, or run: source {}", path.display());
        }
        None => println!("{} already loads it", path.display()),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_function_is_named_tsk_and_calls_the_binary() {
        let code = init_code();
        assert!(code.starts_with("tsk() {\n"));
        assert!(code.contains("command tsk harness path \"$3\""));
        assert!(code.contains("cd \"$dir\" || return"));
        assert!(code.contains("command tsk \"$@\""));
    }

    #[test]
    fn the_function_prints_usage_when_the_name_is_missing() {
        let code = init_code();
        assert!(code.contains("usage: tsk harness cd <name>\" >&2"));
        assert!(code.contains("return 1"));
    }

    #[test]
    fn zsh_and_bash_are_accepted_and_others_are_not() {
        assert_eq!(run_shell_init("zsh"), Ok(0));
        assert_eq!(run_shell_init("bash"), Ok(0));
        assert!(run_shell_init("fish").unwrap_err().contains("fish"));
    }

    #[test]
    fn the_lines_are_added_once() {
        let first = with_zshrc_lines("export A=1").unwrap();
        assert_eq!(first, format!("export A=1\n\n{}", zshrc_lines()));
        assert_eq!(with_zshrc_lines(&first), None);
        assert_eq!(with_zshrc_lines("").unwrap(), zshrc_lines());
    }
}
