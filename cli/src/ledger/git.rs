use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

pub struct Git {
    dir: PathBuf,
}

impl Git {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Git { dir: dir.into() }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
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
        let output = Command::new("git")
            .args(&args)
            .current_dir(&self.dir)
            .output()
            .map_err(|e| format!("error: could not run git: {}", e))?;
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
        String::from_utf8(output.stdout).map_err(|_| {
            format!(
                "error: git {} printed output that is not UTF-8",
                args.join(" ")
            )
        })
    }

    pub fn run_line<I, S>(&self, args: I) -> Result<String, String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        Ok(self.run(args)?.trim_end_matches(['\n', '\r']).to_string())
    }
}

pub fn failure_message(args: &[String], output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stderr = stderr.trim();
    let status = match output.status.code() {
        Some(code) => format!("exit status {}", code),
        None => "terminated by a signal".to_string(),
    };
    if stderr.is_empty() {
        format!("error: git {} failed ({})", args.join(" "), status)
    } else {
        format!(
            "error: git {} failed ({}):\n{}",
            args.join(" "),
            status,
            stderr
        )
    }
}

pub fn split_nul(text: &str) -> Vec<&str> {
    text.split('\0').filter(|s| !s.is_empty()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_nul_drops_empty_records() {
        assert_eq!(split_nul("a\0b c\0\0d\0"), vec!["a", "b c", "d"]);
        assert!(split_nul("").is_empty());
    }

    #[test]
    fn run_reports_failure_with_args_and_stderr() {
        let dir = tempfile::tempdir().unwrap();
        let git = Git::new(dir.path());
        let err = git
            .run(["rev-parse", "--verify", "refs/heads/no-such-ref"])
            .unwrap_err();
        assert!(
            err.contains("git rev-parse --verify refs/heads/no-such-ref failed"),
            "{}",
            err
        );
    }

    #[test]
    fn run_line_strips_trailing_newline() {
        let dir = tempfile::tempdir().unwrap();
        let git = Git::new(dir.path());
        let version = git.run_line(["--version"]).unwrap();
        assert!(version.starts_with("git version"));
        assert!(!version.ends_with('\n'));
    }
}
