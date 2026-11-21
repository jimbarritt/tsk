use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::attribution::{self, Settings};
use crate::ledger::git::Git;

pub const RETRY_DELAY_VAR: &str = "TSK_SIGNING_RETRY_DELAY_MS";

const DEFAULT_RETRY_DELAY: Duration = Duration::from_millis(500);
const ATTEMPTS: usize = 3;
const AGENT_TIMEOUT: Duration = Duration::from_secs(3);
const AGENT_FAILURES: [&str; 2] = ["failed to fill whole buffer", "agent returned an error"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Availability {
    Available,
    KeyMissing,
    NoIdentities,
    NoAgent,
    TimedOut,
}

impl Availability {
    pub fn is_available(&self) -> bool {
        *self == Availability::Available
    }

    pub fn describe(&self, socket: Option<&str>) -> String {
        let agent = match socket {
            Some(socket) => format!("the SSH agent at {}", socket),
            None => "the SSH agent of SSH_AUTH_SOCK".to_string(),
        };
        match self {
            Availability::Available => format!("the robot signing key is available in {}", agent),
            Availability::KeyMissing => format!(
                "{} holds keys but not the robot signing key: unlock 1Password, and check that the vault of the key is in the agent.toml of its SSH agent",
                agent
            ),
            Availability::NoIdentities => format!(
                "{} holds no keys: unlock 1Password, and check that its SSH agent is on",
                agent
            ),
            Availability::NoAgent => format!(
                "tsk could not reach {}: start 1Password and turn on its SSH agent, or set ssh_auth_sock with tsk config attribution --ssh-auth-sock",
                agent
            ),
            Availability::TimedOut => format!(
                "{} did not answer within {} s: approve the 1Password prompt, then run tsk signing check again",
                agent,
                AGENT_TIMEOUT.as_secs()
            ),
        }
    }
}

pub fn key_blob(public_key: &str) -> Option<&str> {
    public_key.split_whitespace().nth(1)
}

pub fn classify(code: Option<i32>, stdout: &str, public_key: &str) -> Availability {
    match code {
        Some(0) => {
            let wanted = key_blob(attribution::agent_public_key(public_key));
            let listed = stdout.lines().any(|line| key_blob(line) == wanted);
            if wanted.is_some() && listed {
                Availability::Available
            } else {
                Availability::KeyMissing
            }
        }
        Some(1) => Availability::NoIdentities,
        _ => Availability::NoAgent,
    }
}

pub fn check_agent(socket: Option<&str>, public_key: &str, timeout: Duration) -> Availability {
    let mut command = Command::new("ssh-add");
    command
        .arg("-L")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    if let Some(socket) = socket {
        command.env("SSH_AUTH_SOCK", socket);
    }
    let Ok(mut child) = command.spawn() else {
        return Availability::NoAgent;
    };
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() < timeout => {
                std::thread::sleep(Duration::from_millis(20))
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Availability::TimedOut;
            }
        }
    };
    let mut stdout = String::new();
    if let Some(mut pipe) = child.stdout.take() {
        let _ = pipe.read_to_string(&mut stdout);
    }
    classify(status.code(), &stdout, public_key)
}

fn current_settings() -> Result<Settings, String> {
    let cwd = std::env::current_dir()
        .map_err(|e| format!("error: could not read the current directory: {}", e))?;
    let repo = crate::ledger::location::Repo::discover(&cwd).ok();
    let repo_file = repo
        .as_ref()
        .and_then(crate::ledger::refs::repo_config_path);
    let user_file = crate::config::config_file_from_env();
    attribution::settings(repo_file.as_deref(), user_file.as_deref())
}

pub fn run_check() -> Result<i32, String> {
    let settings = current_settings()?;
    if settings.signing_key.is_none() {
        if let Some(value) = settings.signing_key_file.as_deref() {
            return Ok(check_key_file_mode(value));
        }
    }
    let Some(signing_key) = settings.signing_key.as_deref() else {
        println!(
            "no signing key is configured: signing_key is not set, so no SSH agent signs. Set it with tsk config attribution --signing-key \"agent:ssh-ed25519 <base64 key>\""
        );
        return Ok(1);
    };
    let socket = settings.ssh_auth_sock.as_deref();
    let availability = check_agent(socket, signing_key, AGENT_TIMEOUT);
    println!("{}", availability.describe(socket));
    Ok(if availability.is_available() { 0 } else { 1 })
}

fn check_key_file_mode(value: &str) -> i32 {
    match attribution::key_file_public_key(value) {
        Ok((path, public)) => {
            println!(
                "the robot signing key file {} exists, is readable and is owner-only. Its public key is {}. No SSH agent is needed",
                path.display(),
                public
            );
            0
        }
        Err(message) => {
            println!("{}", message);
            1
        }
    }
}

pub fn status_line() -> Option<String> {
    let settings = current_settings().ok()?;
    if settings.signing_key.is_none() {
        if let Some(value) = settings.signing_key_file.as_deref() {
            let text = match attribution::check_key_file(value) {
                Ok(path) => format!("robot key file {}", path.display()),
                Err(_) => "robot key file not usable, run tsk signing check".to_string(),
            };
            return Some(format!("{:<8}{}", "Signing", text));
        }
    }
    let signing_key = settings.signing_key.as_deref()?;
    let socket = settings.ssh_auth_sock.as_deref();
    let availability = check_agent(socket, signing_key, Duration::from_secs(1));
    let text = if availability.is_available() {
        "robot key available".to_string()
    } else {
        format!(
            "robot key not available ({}), run tsk signing check",
            short(&availability)
        )
    };
    Some(format!("{:<8}{}", "Signing", text))
}

fn short(availability: &Availability) -> &'static str {
    match availability {
        Availability::Available => "available",
        Availability::KeyMissing => "the agent lacks the key",
        Availability::NoIdentities => "the agent holds no keys",
        Availability::NoAgent => "no agent",
        Availability::TimedOut => "the agent did not answer",
    }
}

pub fn retry_delay() -> Duration {
    std::env::var(RETRY_DELAY_VAR)
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .map(Duration::from_millis)
        .unwrap_or(DEFAULT_RETRY_DELAY)
}

pub fn is_agent_failure(message: &str) -> bool {
    AGENT_FAILURES.iter().any(|text| message.contains(text))
}

pub fn with_retries<T>(
    delay: Duration,
    mut attempt: impl FnMut() -> Result<T, String>,
) -> Result<T, String> {
    let mut tried = 0;
    loop {
        tried += 1;
        match attempt() {
            Ok(value) => return Ok(value),
            Err(message) if is_agent_failure(&message) => {
                if tried >= ATTEMPTS {
                    return Err(format!(
                        "{}\nerror: the SSH agent failed {} times when git signed the commit. A 1Password approval may be pending: approve it, then run tsk signing check to see whether the robot key is available",
                        message, ATTEMPTS
                    ));
                }
                std::thread::sleep(delay);
            }
            Err(message) => return Err(message),
        }
    }
}

pub fn run_signed(git: &Git, args: Vec<String>) -> Result<String, String> {
    with_retries(retry_delay(), || git.run(args.clone()))
}

pub fn run_signed_output(
    git: &Git,
    args: Vec<String>,
    undo: impl Fn(),
) -> Result<std::process::Output, String> {
    with_retries(retry_delay(), || {
        let (args, output) = git.output(args.clone())?;
        if !output.status.success() {
            let message = crate::ledger::git::failure_message(&args, &output);
            if is_agent_failure(&message) {
                undo();
                return Err(message);
            }
        }
        Ok(output)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    const KEY: &str = "agent:ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOne comment";

    #[test]
    fn a_listed_key_is_available() {
        let out = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOne vault key\n";
        assert_eq!(classify(Some(0), out, KEY), Availability::Available);
    }

    #[test]
    fn another_key_in_the_list_is_a_missing_key() {
        let out = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAITwo other\n";
        assert_eq!(classify(Some(0), out, KEY), Availability::KeyMissing);
    }

    #[test]
    fn exit_codes_of_ssh_add_map_to_the_outcomes() {
        assert_eq!(classify(Some(1), "", KEY), Availability::NoIdentities);
        assert_eq!(classify(Some(2), "", KEY), Availability::NoAgent);
        assert_eq!(classify(None, "", KEY), Availability::NoAgent);
    }

    #[test]
    fn every_outcome_but_available_says_what_to_do() {
        for outcome in [
            Availability::KeyMissing,
            Availability::NoIdentities,
            Availability::NoAgent,
            Availability::TimedOut,
        ] {
            let text = outcome.describe(Some("/s/agent.sock"));
            assert!(text.contains("/s/agent.sock"), "{}", text);
            assert!(text.contains("1Password"), "{}", text);
        }
    }

    #[test]
    fn an_agent_failure_is_retried_until_it_passes() {
        let calls = Cell::new(0);
        let out = with_retries(Duration::ZERO, || {
            calls.set(calls.get() + 1);
            if calls.get() < 3 {
                Err("error: failed to fill whole buffer".to_string())
            } else {
                Ok("done")
            }
        });
        assert_eq!(out, Ok("done"));
        assert_eq!(calls.get(), 3);
    }

    #[test]
    fn three_agent_failures_name_tsk_signing_check_and_the_pending_approval() {
        let calls = Cell::new(0);
        let err = with_retries(Duration::ZERO, || -> Result<(), String> {
            calls.set(calls.get() + 1);
            Err("agent returned an error".to_string())
        })
        .unwrap_err();
        assert_eq!(calls.get(), 3);
        assert!(err.contains("tsk signing check"), "{}", err);
        assert!(err.contains("approval may be pending"), "{}", err);
    }

    #[test]
    fn another_failure_is_not_retried() {
        let calls = Cell::new(0);
        let err = with_retries(Duration::ZERO, || -> Result<(), String> {
            calls.set(calls.get() + 1);
            Err("error: nothing to commit".to_string())
        })
        .unwrap_err();
        assert_eq!(calls.get(), 1);
        assert_eq!(err, "error: nothing to commit");
    }
}
