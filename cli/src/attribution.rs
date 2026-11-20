//! Robot commit attribution: the `[attribution]` settings, and the git identity that the
//! session start exports to the session environment. tsk writes no robot setting to a git
//! config file.
//!
//! `identity` is `default` or `robot`, `default` when unset. `default` exports nothing.
//! `robot` exports the registered actor's name, the `email` setting, and the SSH signing
//! settings. `email` has no built-in value. The key comes from `TSK_ROBOT_SIGNING_KEY`, or from
//! an SSH agent when `signing_key` is set.

use std::path::{Path, PathBuf};

use crate::actor::{self, Current};
use crate::ledger::location::Repo;
use crate::ledger::refs;
use crate::ledger::store::ActorKind;
use crate::thread::session::Session;

/// The environment variable that overrides `[attribution] email`.
pub const EMAIL_ENV: &str = "TSK_ATTRIBUTION_EMAIL";

/// The environment variable that overrides `[attribution] co_author`.
pub const CO_AUTHOR_ENV: &str = "TSK_ATTRIBUTION_CO_AUTHOR";

/// The environment variable that overrides `[attribution] identity`. A cloud environment
/// sets it, with `TSK_ATTRIBUTION_EMAIL` and `TSK_ROBOT_SIGNING_KEY`, to turn attribution on
/// for its own sessions only.
pub const IDENTITY_ENV: &str = "TSK_ATTRIBUTION_IDENTITY";

/// The environment variable that holds the private SSH signing key.
pub const KEY_ENV: &str = "TSK_ROBOT_SIGNING_KEY";

pub const SIGNING_KEY_ENV: &str = "TSK_ATTRIBUTION_SIGNING_KEY";

pub const SIGNING_PROGRAM_ENV: &str = "TSK_ATTRIBUTION_SIGNING_PROGRAM";

pub const DEFAULT_SIGNING_PROGRAM: &str = "ssh-keygen";

const AGENT_PREFIX: &str = "agent:";

/// The name of the file the key is written to, beside the ledger store of the clone.
const KEY_FILE_NAME: &str = "robot-signing-key";
const SIGNERS_FILE_NAME: &str = "robot-allowed-signers";

const KEY_HEADER: &str = "-----BEGIN OPENSSH PRIVATE KEY-----";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Identity {
    Default,
    Robot,
}

impl Identity {
    pub fn parse(value: &str) -> Option<Identity> {
        match value.trim() {
            "default" => Some(Identity::Default),
            "robot" => Some(Identity::Robot),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Identity::Default => "default",
            Identity::Robot => "robot",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Env,
    RepoConfig,
    UserConfig,
    Default,
}

impl Source {
    pub fn label(self) -> &'static str {
        match self {
            Source::Env => "environment",
            Source::RepoConfig => "repo config",
            Source::UserConfig => "user config",
            Source::Default => "default",
        }
    }
}

/// The `[attribution]` keys of one config file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Section {
    pub identity: Option<Identity>,
    pub email: Option<String>,
    pub co_author: Option<String>,
    pub signing_key: Option<String>,
    pub signing_program: Option<String>,
}

/// The settings in force, with where each came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    pub identity: Identity,
    pub identity_source: Source,
    pub email: Option<String>,
    pub email_source: Option<Source>,
    pub co_author: Option<String>,
    pub co_author_source: Option<Source>,
    pub signing_key: Option<String>,
    pub signing_key_source: Option<Source>,
    pub signing_program: Option<String>,
    pub signing_program_source: Option<Source>,
}

/// The `[attribution]` keys of a config file. A missing file or table gives an empty section.
pub fn read_section(file: &Path) -> Result<Section, String> {
    let text = match std::fs::read_to_string(file) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Section::default()),
        Err(e) => return Err(format!("error: could not read {}: {}", file.display(), e)),
    };
    parse_section(&text, file)
}

fn parse_section(text: &str, file: &Path) -> Result<Section, String> {
    let table = text.parse::<toml::Table>().map_err(|e| {
        format!(
            "error: {} is not valid TOML: {}",
            file.display(),
            e.message()
        )
    })?;
    let Some(section) = table.get("attribution") else {
        return Ok(Section::default());
    };
    let Some(section) = section.as_table() else {
        return Err(format!(
            "error: attribution in {} must be a table",
            file.display()
        ));
    };
    let identity = match section.get("identity") {
        None => None,
        Some(toml::Value::String(value)) => Some(Identity::parse(value).ok_or_else(|| {
            format!(
                "error: attribution.identity in {} is \"{}\"; it must be default or robot",
                file.display(),
                value.trim()
            )
        })?),
        Some(_) => {
            return Err(format!(
                "error: attribution.identity in {} must be a string",
                file.display()
            ))
        }
    };
    let email = match section.get("email") {
        None => None,
        Some(toml::Value::String(value)) if !value.trim().is_empty() => {
            Some(value.trim().to_string())
        }
        Some(_) => {
            return Err(format!(
                "error: attribution.email in {} must be a non-empty string",
                file.display()
            ))
        }
    };
    let co_author = match section.get("co_author") {
        None => None,
        Some(toml::Value::String(value)) => Some(
            check_co_author(value)
                .map_err(|e| format!("error: attribution.co_author in {} {}", file.display(), e))?,
        ),
        Some(_) => {
            return Err(format!(
                "error: attribution.co_author in {} must be a non-empty string",
                file.display()
            ))
        }
    };
    let signing_key =
        match section.get("signing_key") {
            None => None,
            Some(toml::Value::String(value)) => Some(check_signing_key(value).map_err(|e| {
                format!("error: attribution.signing_key in {} {}", file.display(), e)
            })?),
            Some(_) => {
                return Err(format!(
                    "error: attribution.signing_key in {} must be a non-empty string",
                    file.display()
                ))
            }
        };
    let signing_program = match section.get("signing_program") {
        None => None,
        Some(toml::Value::String(value)) if !value.trim().is_empty() => {
            Some(value.trim().to_string())
        }
        Some(_) => {
            return Err(format!(
                "error: attribution.signing_program in {} must be a non-empty string",
                file.display()
            ))
        }
    };
    Ok(Section {
        identity,
        email,
        co_author,
        signing_key,
        signing_program,
    })
}

pub fn check_signing_key(value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() {
        return Err("must be a non-empty string".to_string());
    }
    let Some(public) = value.strip_prefix(AGENT_PREFIX) else {
        return Err(format!(
            "must start with {} and hold a public key, such as {}ssh-ed25519 AAAA...",
            AGENT_PREFIX, AGENT_PREFIX
        ));
    };
    let mut fields = public.split_whitespace();
    let kind = fields.next().unwrap_or("");
    let blob = fields.next().unwrap_or("");
    let kind_ok = kind
        .strip_prefix("ssh-")
        .is_some_and(|rest| !rest.is_empty() && !rest.contains(':'));
    let blob_ok = !blob.is_empty()
        && blob
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '='));
    if !kind_ok || !blob_ok || value.contains(['\n', '\r']) {
        return Err(format!(
            "must be {}ssh-<type> <base64 key>, the public key the SSH agent holds",
            AGENT_PREFIX
        ));
    }
    Ok(format!("{}{} {}", AGENT_PREFIX, kind, blob))
}

pub fn agent_public_key(signing_key: &str) -> &str {
    signing_key
        .strip_prefix(AGENT_PREFIX)
        .unwrap_or(signing_key)
}

/// The trimmed co-author value, which must have the form `Name <email>`.
pub fn check_co_author(value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() {
        return Err("must be a non-empty string".to_string());
    }
    let named = value
        .find('<')
        .is_some_and(|open| value[open..].contains('>') && !value[..open].trim().is_empty());
    if !named || value.contains(['\n', '\r']) {
        return Err(format!(
            "is \"{}\"; it must have the form Name <email>",
            value
        ));
    }
    Ok(value.to_string())
}

/// Resolve the settings. `identity`: the repo config, then the user config, then `default`.
/// `email`: the environment, then the repo config, then the user config. An empty
/// environment value counts as unset.
pub fn resolve(
    env_identity: Option<Identity>,
    env_email: Option<&str>,
    env_co_author: Option<&str>,
    repo: &Section,
    user: &Section,
) -> Settings {
    let (identity, identity_source) = match (env_identity, repo.identity, user.identity) {
        (Some(value), _, _) => (value, Source::Env),
        (None, Some(value), _) => (value, Source::RepoConfig),
        (None, None, Some(value)) => (value, Source::UserConfig),
        (None, None, None) => (Identity::Default, Source::Default),
    };
    let env_email = env_email.map(str::trim).filter(|v| !v.is_empty());
    let (email, email_source) = match (env_email, &repo.email, &user.email) {
        (Some(value), _, _) => (Some(value.to_string()), Some(Source::Env)),
        (None, Some(value), _) => (Some(value.clone()), Some(Source::RepoConfig)),
        (None, None, Some(value)) => (Some(value.clone()), Some(Source::UserConfig)),
        (None, None, None) => (None, None),
    };
    let env_co_author = env_co_author.map(str::trim).filter(|v| !v.is_empty());
    let (co_author, co_author_source) = match (env_co_author, &repo.co_author, &user.co_author) {
        (Some(value), _, _) => (Some(value.to_string()), Some(Source::Env)),
        (None, Some(value), _) => (Some(value.clone()), Some(Source::RepoConfig)),
        (None, None, Some(value)) => (Some(value.clone()), Some(Source::UserConfig)),
        (None, None, None) => (None, None),
    };
    let (signing_key, signing_key_source) = resolve_signing_key(None, repo, user);
    let (signing_program, signing_program_source) = resolve_signing_program(None, repo, user);
    Settings {
        identity,
        identity_source,
        email,
        email_source,
        co_author,
        co_author_source,
        signing_key,
        signing_key_source,
        signing_program,
        signing_program_source,
    }
}

pub fn resolve_signing_key(
    env_key: Option<&str>,
    repo: &Section,
    user: &Section,
) -> (Option<String>, Option<Source>) {
    let env_key = env_key.map(str::trim).filter(|v| !v.is_empty());
    match (env_key, &repo.signing_key, &user.signing_key) {
        (Some(value), _, _) => (Some(value.to_string()), Some(Source::Env)),
        (None, Some(value), _) => (Some(value.clone()), Some(Source::RepoConfig)),
        (None, None, Some(value)) => (Some(value.clone()), Some(Source::UserConfig)),
        (None, None, None) => (None, None),
    }
}

pub fn resolve_signing_program(
    env_program: Option<&str>,
    repo: &Section,
    user: &Section,
) -> (Option<String>, Option<Source>) {
    let env_program = env_program.map(str::trim).filter(|v| !v.is_empty());
    match (env_program, &repo.signing_program, &user.signing_program) {
        (Some(value), _, _) => (Some(value.to_string()), Some(Source::Env)),
        (None, Some(value), _) => (Some(value.clone()), Some(Source::RepoConfig)),
        (None, None, Some(value)) => (Some(value.clone()), Some(Source::UserConfig)),
        (None, None, None) => (None, None),
    }
}

/// The settings in force for the repo of `repo_file` and the user config `user_file`.
pub fn settings(repo_file: Option<&Path>, user_file: Option<&Path>) -> Result<Settings, String> {
    let repo = match repo_file {
        Some(file) => read_section(file)?,
        None => Section::default(),
    };
    let user = match user_file {
        Some(file) => read_section(file)?,
        None => Section::default(),
    };
    let env_identity = match std::env::var(IDENTITY_ENV) {
        Ok(value) if !value.trim().is_empty() => {
            Some(Identity::parse(&value).ok_or_else(|| {
                format!(
                    "error: {} is {:?}: use default or robot",
                    IDENTITY_ENV,
                    value.trim()
                )
            })?)
        }
        _ => None,
    };
    let env_co_author = match std::env::var(CO_AUTHOR_ENV) {
        Ok(value) if !value.trim().is_empty() => {
            Some(check_co_author(&value).map_err(|e| format!("error: {} {}", CO_AUTHOR_ENV, e))?)
        }
        _ => None,
    };
    let env_signing_key = match std::env::var(SIGNING_KEY_ENV) {
        Ok(value) if !value.trim().is_empty() => Some(
            check_signing_key(&value).map_err(|e| format!("error: {} {}", SIGNING_KEY_ENV, e))?,
        ),
        _ => None,
    };
    let mut resolved = resolve(
        env_identity,
        std::env::var(EMAIL_ENV).ok().as_deref(),
        env_co_author.as_deref(),
        &repo,
        &user,
    );
    (resolved.signing_key, resolved.signing_key_source) =
        resolve_signing_key(env_signing_key.as_deref(), &repo, &user);
    (resolved.signing_program, resolved.signing_program_source) = resolve_signing_program(
        std::env::var(SIGNING_PROGRAM_ENV).ok().as_deref(),
        &repo,
        &user,
    );
    Ok(resolved)
}

/// Write `[attribution]` in a config file and keep every other key. A value of `None`
/// leaves that key as it is. Creates the directory.
pub fn write_section(
    file: &Path,
    identity: Option<Identity>,
    email: Option<&str>,
    co_author: Option<&str>,
    signing_key: Option<&str>,
    signing_program: Option<&str>,
) -> Result<(), String> {
    let text = match std::fs::read_to_string(file) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(format!("error: could not read {}: {}", file.display(), e)),
    };
    let text = with_section(
        &text,
        identity,
        email,
        co_author,
        signing_key,
        signing_program,
        file,
    )?;
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("error: could not create {}: {}", parent.display(), e))?;
    }
    std::fs::write(file, text)
        .map_err(|e| format!("error: could not write {}: {}", file.display(), e))
}

/// The text of a config file with `[attribution]` keys set. The other tables, keys and
/// comments stay as they are: an existing line of the `[attribution]` table is replaced,
/// else the line follows the table header, else a new table ends the file.
pub fn with_section(
    text: &str,
    identity: Option<Identity>,
    email: Option<&str>,
    co_author: Option<&str>,
    signing_key: Option<&str>,
    signing_program: Option<&str>,
    file: &Path,
) -> Result<String, String> {
    parse_section(text, file)?;
    let signing_key = match signing_key {
        Some(value) => Some(
            check_signing_key(value).map_err(|e| format!("error: the signing key value {}", e))?,
        ),
        None => None,
    };
    let signing_key = signing_key.as_deref();
    let co_author = match co_author {
        Some(value) => {
            Some(check_co_author(value).map_err(|e| format!("error: the co-author value {}", e))?)
        }
        None => None,
    };
    let co_author = co_author.as_deref();
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    if lines.iter().all(|l| l.trim() != "[attribution]") {
        if !lines.is_empty() && !lines.last().is_some_and(|l| l.trim().is_empty()) {
            lines.push(String::new());
        }
        lines.push("[attribution]".to_string());
    }
    for (key, value) in [
        ("identity", identity.map(|i| i.as_str().to_string())),
        ("email", email.map(str::to_string)),
        ("co_author", co_author.map(str::to_string)),
        ("signing_key", signing_key.map(str::to_string)),
        ("signing_program", signing_program.map(str::to_string)),
    ] {
        let Some(value) = value else { continue };
        if value.contains(['"', '\\', '\n']) {
            return Err(format!(
                "error: the {} value cannot hold a quote, a backslash or a line break",
                key
            ));
        }
        let line = format!("{} = \"{}\"", key, value);
        let start = lines
            .iter()
            .position(|l| l.trim() == "[attribution]")
            .expect("the table header was added above");
        let end = lines[start + 1..]
            .iter()
            .position(|l| l.trim_start().starts_with('['))
            .map_or(lines.len(), |i| start + 1 + i);
        let existing = (start + 1..end).find(|&i| {
            lines[i]
                .trim_start()
                .strip_prefix(key)
                .is_some_and(|rest| rest.trim_start().starts_with('='))
        });
        match existing {
            Some(i) => lines[i] = line,
            None => lines.insert(start + 1, line),
        }
    }
    let mut out = lines.join("\n");
    out.push('\n');
    let written = parse_section(&out, file)?;
    if identity.is_some_and(|i| written.identity != Some(i))
        || email.is_some_and(|e| written.email.as_deref() != Some(e))
        || co_author.is_some_and(|c| written.co_author.as_deref() != Some(c))
        || signing_key.is_some_and(|k| written.signing_key.as_deref() != Some(k))
        || signing_program.is_some_and(|p| written.signing_program.as_deref() != Some(p))
    {
        return Err(
            "error: could not set [attribution] in the config: edit the file by hand".to_string(),
        );
    }
    Ok(out)
}

/// The key text with real line breaks and one trailing line break. A key stored on one
/// line, with `\n` in place of each line break, is accepted.
pub fn normalise_key(raw: &str) -> Result<String, String> {
    let raw = raw.trim();
    let text = if raw.contains('\n') {
        raw.to_string()
    } else {
        raw.replace("\\n", "\n")
    };
    if !text.starts_with(KEY_HEADER) {
        return Err(format!(
            "error: {} does not hold an OpenSSH private key: it must start with {}",
            KEY_ENV, KEY_HEADER
        ));
    }
    Ok(format!("{}\n", text.trim_end()))
}

/// The git config keys and values of the robot's signing, without `user.name`: the session
/// start writes them before an actor is registered.
pub fn signing_settings(
    email: &str,
    key_file: &Path,
    signers_file: &Path,
) -> Vec<(&'static str, String)> {
    vec![
        ("user.email", email.to_string()),
        ("gpg.format", "ssh".to_string()),
        // A cloud image can set `gpg.ssh.program` to a platform signer that ignores
        // `user.signingkey`. `ssh-keygen` signs with the key file named here.
        ("gpg.ssh.program", "ssh-keygen".to_string()),
        ("user.signingkey", key_file.display().to_string()),
        // `git log --format=%G?` verifies a signature against this file.
        (
            "gpg.ssh.allowedSignersFile",
            signers_file.display().to_string(),
        ),
        ("commit.gpgsign", "true".to_string()),
    ]
}

pub fn agent_signing_settings(
    email: &str,
    public_key: &str,
    program: &str,
    signers_file: &Path,
) -> Vec<(&'static str, String)> {
    vec![
        ("user.email", email.to_string()),
        ("gpg.format", "ssh".to_string()),
        ("gpg.ssh.program", program.to_string()),
        ("user.signingkey", format!("key::{}", public_key.trim())),
        (
            "gpg.ssh.allowedSignersFile",
            signers_file.display().to_string(),
        ),
        ("commit.gpgsign", "true".to_string()),
    ]
}

/// The line of an allowed signers file: the principal, then the public key.
pub fn signers_line(email: &str, public_key: &str) -> String {
    format!("{} {}\n", email, public_key.trim())
}

/// The public key of a private key file, from `ssh-keygen -y`.
fn public_key(key_file: &Path) -> Result<String, String> {
    let output = std::process::Command::new("ssh-keygen")
        .arg("-y")
        .arg("-f")
        .arg(key_file)
        .output()
        .map_err(|e| {
            format!(
                "error: could not run ssh-keygen: {}: install openssh-client",
                e
            )
        })?;
    if !output.status.success() {
        return Err(format!(
            "error: ssh-keygen could not read the key in {}: {}",
            KEY_ENV,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// The problem with a name that git cannot hold as a user name, if any.
pub fn check_git_name(name: &str) -> Result<(), String> {
    if name.contains(['<', '>', '\n', '\r']) {
        return Err(format!(
            "error: the actor name {:?} holds <, > or a line break, which git cannot hold in an author name",
            name
        ));
    }
    Ok(())
}

/// What `apply` did.
#[derive(Debug, PartialEq, Eq)]
pub enum Applied {
    /// The identity is `default`: git config is as it was.
    Unchanged,
    /// The identity is `robot`: git config holds the actor's name and the settings.
    Written { name: String, email: String },
}

/// What `set_up_signing` did.
#[derive(Debug, PartialEq, Eq)]
pub enum Signing {
    /// The identity is not `robot`, and there was no robot set-up to remove.
    Off,
    /// The identity is not `robot`: the robot set-up of an earlier session was removed.
    Removed,
    /// The identity is `robot`: the key, the signing config and the commit-msg hook are
    /// written. `hook` is false when a commit-msg hook that tsk did not write is in place.
    Written { email: String, hook: bool },
}

const SUPERVISED_KEY: &str = "tsk.supervised";

pub fn clear_supervised(git: &crate::ledger::git::Git) {
    let _ = git.output(["config", "--worktree", "--unset-all", SUPERVISED_KEY]);
}

/// The first line of the commit-msg hook that tsk writes. tsk replaces or removes a hook
/// only when it holds this line.
const HOOK_MARKER: &str = "# tsk: robot attribution commit-msg hook";

const ROBOT_KEYS: [&str; 9] = [
    "user.name",
    "user.email",
    "gpg.format",
    "gpg.ssh.program",
    "user.signingkey",
    "gpg.ssh.allowedSignersFile",
    "commit.gpgsign",
    "tsk.attribution",
    "tsk.supervised",
];

const SIGNING_NOT_WRITTEN: &str = "error: [attribution] identity is robot and the signing set-up is not written: run tsk thread session-start before a ledger write";
const ROBOT_NAME_FILE: &str = "tsk-robot-name";

pub const SESSION_VAR: &str = "TSK_SESSION";

fn current_settings(repo: &Repo) -> Result<Settings, String> {
    let repo_file = refs::repo_config_path(repo);
    let user_file = crate::config::config_file_from_env();
    settings(repo_file.as_deref(), user_file.as_deref())
}

fn missing_email() -> String {
    format!(
        "error: [attribution] identity is robot and no email is set: run tsk config attribution robot --email <address>, or set {}",
        EMAIL_ENV
    )
}

pub fn set_up_signing(repo: &Repo) -> Result<Signing, String> {
    let cleaned = remove_leftover_config(repo)?;
    let settings = current_settings(repo)?;
    if settings.identity != Identity::Robot {
        return remove_signing(repo, cleaned);
    }
    let email = settings.email.ok_or_else(missing_email)?;
    let key_file = key_file_path(repo)?;
    let signers_file = key_file.with_file_name(SIGNERS_FILE_NAME);
    match settings.signing_key {
        Some(signing_key) => {
            let public = agent_public_key(&signing_key).to_string();
            if key_file.exists() {
                std::fs::remove_file(&key_file).map_err(|e| {
                    format!("error: could not remove {}: {}", key_file.display(), e)
                })?;
            }
            write_key_file(&signers_file, &signers_line(&email, &public))?;
        }
        None => {
            let key = std::env::var(KEY_ENV)
                .ok()
                .filter(|v| !v.trim().is_empty())
                .ok_or_else(|| format!("error: {} is not set", KEY_ENV))?;
            let key = normalise_key(&key)?;
            write_key_file(&key_file, &key)?;
            write_key_file(
                &signers_file,
                &signers_line(&email, &public_key(&key_file)?),
            )?;
        }
    }
    let hook = install_hook(repo)?;
    Ok(Signing::Written { email, hook })
}

fn remove_leftover_config(repo: &Repo) -> Result<bool, String> {
    let git = &repo.git;
    clear_supervised(git);
    let (_, marker) = git.output(["config", "--worktree", "--get", "tsk.attribution"])?;
    if !marker.status.success() || String::from_utf8_lossy(&marker.stdout).trim() != "robot" {
        return Ok(false);
    }
    for config_key in ROBOT_KEYS {
        let _ = git.output(["config", "--worktree", "--unset-all", config_key])?;
    }
    Ok(true)
}

fn remove_signing(repo: &Repo, cleaned: bool) -> Result<Signing, String> {
    let mut removed = cleaned;
    if let Some(hook) = hook_path(repo)? {
        if is_tsk_hook(&hook) {
            std::fs::remove_file(&hook)
                .map_err(|e| format!("error: could not remove {}: {}", hook.display(), e))?;
            removed = true;
        }
    }
    let name_file = robot_name_path(repo);
    if name_file.exists() {
        std::fs::remove_file(&name_file)
            .map_err(|e| format!("error: could not remove {}: {}", name_file.display(), e))?;
        removed = true;
    }
    if let Ok(key_file) = key_file_path(repo) {
        for file in [key_file.with_file_name(SIGNERS_FILE_NAME), key_file] {
            if file.exists() {
                std::fs::remove_file(&file)
                    .map_err(|e| format!("error: could not remove {}: {}", file.display(), e))?;
                removed = true;
            }
        }
    }
    Ok(if removed {
        Signing::Removed
    } else {
        Signing::Off
    })
}

fn robot_name_path(repo: &Repo) -> PathBuf {
    repo.git_dir.join(ROBOT_NAME_FILE)
}

/// The path of the commit-msg hook of this worktree, or `None` when git names no hooks
/// directory.
fn hook_path(repo: &Repo) -> Result<Option<PathBuf>, String> {
    // `repo.git` turns hooks off, and git then names its stand-in hooks path. A
    // code worktree helper reads the repository's own hooks path.
    let git = crate::ledger::git::Git::in_code_worktree(repo.git.dir());
    let dir = git.run_line(["rev-parse", "--git-path", "hooks"])?;
    if dir.is_empty() || Path::new(&dir) == Path::new("/dev/null") {
        return Ok(None);
    }
    let dir = PathBuf::from(dir);
    let dir = if dir.is_absolute() {
        dir
    } else {
        repo.git.dir().join(dir)
    };
    Ok(Some(dir.join("commit-msg")))
}

/// Whether `hook` is a commit-msg hook that tsk wrote: it holds the marker line, or it is a
/// link to the script `ops/local/git-commit-msg.sh` that tsk's own repo linked before T-15.
fn is_tsk_hook(hook: &Path) -> bool {
    if let Ok(target) = std::fs::read_link(hook) {
        if target.ends_with("ops/local/git-commit-msg.sh") {
            return true;
        }
    }
    std::fs::read_to_string(hook)
        .map(|text| text.lines().any(|line| line == HOOK_MARKER))
        .unwrap_or(false)
}

/// The text of the commit-msg hook. It exits 0 when `tsk` is not on the PATH or has no
/// `actor commit-msg` (older than 0.48.0), so a commit never fails because of the hook.
pub fn hook_text() -> String {
    format!(
        "#!/bin/sh\n{}\n# Written by tsk thread session-start. It writes the Agent-Session trailer.\ncommand -v tsk >/dev/null 2>&1 || exit 0\ntsk actor commit-msg \"$1\" 2>/dev/null\nexit 0\n",
        HOOK_MARKER
    )
}

/// Write the commit-msg hook, unless a hook that tsk did not write is in place. Returns
/// whether the tsk hook is in place.
fn install_hook(repo: &Repo) -> Result<bool, String> {
    let Some(hook) = hook_path(repo)? else {
        return Ok(false);
    };
    if hook.symlink_metadata().is_ok() && !is_tsk_hook(&hook) {
        return Ok(false);
    }
    if let Some(parent) = hook.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("error: could not create {}: {}", parent.display(), e))?;
    }
    // A link from before T-15 is replaced by a file.
    let _ = std::fs::remove_file(&hook);
    std::fs::write(&hook, hook_text())
        .map_err(|e| format!("error: could not write {}: {}", hook.display(), e))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("error: could not make {} executable: {}", hook.display(), e))?;
    }
    Ok(true)
}

pub fn remember_robot_name(session: &Session) -> Result<Applied, String> {
    let settings = current_settings(&session.repo)?;
    if settings.identity != Identity::Robot {
        return Ok(Applied::Unchanged);
    }
    let Some(Current { name, kind, .. }) = actor::current(session)? else {
        return Err("error: no actor is registered: run tsk actor register first".to_string());
    };
    if kind != Some(ActorKind::Robot) {
        return Ok(Applied::Unchanged);
    }
    check_git_name(&name)?;
    let file = robot_name_path(&session.repo);
    std::fs::write(&file, format!("{}\n", name))
        .map_err(|e| format!("error: could not write {}: {}", file.display(), e))?;
    let email = settings.email.unwrap_or_default();
    Ok(Applied::Written { name, email })
}

fn robot_variables(
    name: &str,
    email: &str,
    config: &[(&'static str, String)],
    session_urn: Option<&str>,
) -> Vec<(String, String)> {
    let mut variables = vec![
        ("GIT_AUTHOR_NAME".to_string(), name.to_string()),
        ("GIT_AUTHOR_EMAIL".to_string(), email.to_string()),
        ("GIT_COMMITTER_NAME".to_string(), name.to_string()),
        ("GIT_COMMITTER_EMAIL".to_string(), email.to_string()),
    ];
    let pairs: Vec<(&str, &str)> = [
        "commit.gpgsign",
        "gpg.format",
        "gpg.ssh.program",
        "user.signingkey",
    ]
    .iter()
    .filter_map(|key| {
        config
            .iter()
            .find(|(name, _)| name == key)
            .map(|(name, value)| (*name, value.as_str()))
    })
    .collect();
    variables.push(("GIT_CONFIG_COUNT".to_string(), pairs.len().to_string()));
    for (index, (key, value)) in pairs.iter().enumerate() {
        variables.push((format!("GIT_CONFIG_KEY_{}", index), key.to_string()));
        variables.push((format!("GIT_CONFIG_VALUE_{}", index), value.to_string()));
    }
    if let Some(urn) = session_urn {
        variables.push((SESSION_VAR.to_string(), urn.to_string()));
    }
    variables
}

pub fn session_variables(
    session: &Session,
    session_urn: Option<&str>,
) -> Result<Option<Vec<(String, String)>>, String> {
    let settings = current_settings(&session.repo)?;
    if settings.identity != Identity::Robot {
        return Ok(None);
    }
    let Some(Current { name, kind, .. }) = actor::current(session)? else {
        return Ok(None);
    };
    if kind != Some(ActorKind::Robot) {
        return Ok(None);
    }
    check_git_name(&name)?;
    let email = settings.email.clone().ok_or_else(missing_email)?;
    let key_file = key_file_path(&session.repo)?;
    let signers_file = key_file.with_file_name(SIGNERS_FILE_NAME);
    let config = match &settings.signing_key {
        Some(signing_key) => {
            let program = settings
                .signing_program
                .clone()
                .unwrap_or_else(|| DEFAULT_SIGNING_PROGRAM.to_string());
            agent_signing_settings(
                &email,
                agent_public_key(signing_key),
                &program,
                &signers_file,
            )
        }
        None => signing_settings(&email, &key_file, &signers_file),
    };
    Ok(Some(robot_variables(&name, &email, &config, session_urn)))
}

pub fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

pub fn export_lines(variables: &[(String, String)]) -> Vec<String> {
    variables
        .iter()
        .map(|(name, value)| format!("export {}={}", name, shell_quote(value)))
        .collect()
}

const MANAGED_VARIABLES: [&str; 6] = [
    "GIT_AUTHOR_NAME",
    "GIT_AUTHOR_EMAIL",
    "GIT_COMMITTER_NAME",
    "GIT_COMMITTER_EMAIL",
    "GIT_CONFIG_COUNT",
    SESSION_VAR,
];

fn is_managed_variable(name: &str) -> bool {
    let indexed = |prefix: &str| {
        name.strip_prefix(prefix)
            .is_some_and(|rest| !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit()))
    };
    MANAGED_VARIABLES.contains(&name) || indexed("GIT_CONFIG_KEY_") || indexed("GIT_CONFIG_VALUE_")
}

fn is_managed_line(line: &str) -> bool {
    let Some(rest) = line.trim_start().strip_prefix("export ") else {
        return false;
    };
    let name = rest.trim_start().split('=').next().unwrap_or("").trim();
    is_managed_variable(name)
}

pub fn edit_env_file(existing: &str, variables: &[(String, String)]) -> String {
    let mut lines: Vec<String> = existing
        .lines()
        .filter(|line| !is_managed_line(line))
        .map(str::to_string)
        .collect();
    lines.extend(export_lines(variables));
    if lines.is_empty() {
        return String::new();
    }
    format!("{}\n", lines.join("\n"))
}

pub fn update_env_file(file: &Path, variables: &[(String, String)]) -> Result<(), String> {
    let existing = match std::fs::read_to_string(file) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(format!("could not read {}: {}", file.display(), e)),
    };
    let edited = edit_env_file(&existing, variables);
    if edited == existing {
        return Ok(());
    }
    std::fs::write(file, edited).map_err(|e| format!("could not write {}: {}", file.display(), e))
}

pub fn update_session_env(
    session: &Session,
    session_urn: Option<&str>,
    robot: bool,
) -> Result<(), String> {
    let Some(file) =
        std::env::var_os(crate::thread::session_start::ENV_FILE_VAR).filter(|v| !v.is_empty())
    else {
        return Ok(());
    };
    let variables = if robot {
        session_variables(session, session_urn)
            .ok()
            .flatten()
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    update_env_file(Path::new(&file), &variables)
}

/// The robot identity of a ledger commit (T-19): the name, the email and the signing key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RobotIdentity {
    pub name: String,
    pub email: String,
    pub key_file: String,
    pub signers_file: String,
    pub program: String,
}

impl RobotIdentity {
    /// The `-c` options that make a `git` command commit as the robot, signed with its key.
    pub fn git_options(&self) -> Vec<String> {
        let settings = [
            ("user.name", self.name.as_str()),
            ("user.email", self.email.as_str()),
            ("gpg.format", "ssh"),
            ("gpg.ssh.program", self.program.as_str()),
            ("user.signingkey", self.key_file.as_str()),
            ("gpg.ssh.allowedSignersFile", self.signers_file.as_str()),
            ("commit.gpgsign", "true"),
        ];
        settings
            .iter()
            .flat_map(|(key, value)| ["-c".to_string(), format!("{}={}", key, value)])
            .collect()
    }

    /// The environment that makes `git` commit as the robot: it beats `GIT_AUTHOR_*` and
    /// `GIT_COMMITTER_*` that the platform or a person set.
    pub fn git_env(&self) -> Vec<(String, String)> {
        ["GIT_AUTHOR", "GIT_COMMITTER"]
            .iter()
            .flat_map(|prefix| {
                [
                    (format!("{}_NAME", prefix), self.name.clone()),
                    (format!("{}_EMAIL", prefix), self.email.clone()),
                ]
            })
            .collect()
    }

    /// The environment that makes `dolt` commit as the robot. Dolt has no SSH signing, so a
    /// Dolt commit carries the name and the email only.
    pub fn dolt_env(&self) -> [(&'static str, &str); 4] {
        [
            ("DOLT_AUTHOR_NAME", self.name.as_str()),
            ("DOLT_AUTHOR_EMAIL", self.email.as_str()),
            ("DOLT_COMMITTER_NAME", self.name.as_str()),
            ("DOLT_COMMITTER_EMAIL", self.email.as_str()),
        ]
    }
}

/// The identity of a ledger commit, from the code worktree of the current directory.
/// `Ok(None)` for the `default` identity, and outside a repository: commits are as before.
/// With `robot`, the identity is built from the settings, the key files that the session start
/// wrote and the robot name that the registration recorded. `Err` while the session has no
/// robot actor or no signing set-up: a commit stops (M-TSK-ATTRIB, T-19). Resolved once per
/// process.
pub fn ledger_identity() -> Result<Option<RobotIdentity>, String> {
    if LOOKING_UP_ROBOT.load(std::sync::atomic::Ordering::SeqCst) {
        return Ok(None);
    }
    static IDENTITY: std::sync::OnceLock<Result<Option<RobotIdentity>, String>> =
        std::sync::OnceLock::new();
    IDENTITY.get_or_init(resolve_ledger_identity).clone()
}

fn resolve_ledger_identity() -> Result<Option<RobotIdentity>, String> {
    let Ok(cwd) = std::env::current_dir() else {
        return Ok(None);
    };
    let Ok(repo) = crate::ledger::location::Repo::discover(&cwd) else {
        return Ok(None);
    };
    let repo_file = refs::repo_config_path(&repo);
    let user_file = crate::config::config_file_from_env();
    let settings = settings(repo_file.as_deref(), user_file.as_deref())?;
    if settings.identity != Identity::Robot {
        return Ok(None);
    }
    let key_file = key_file_for(&repo)?;
    let session = session_from_env().or_else(|| {
        crate::thread::session::session_urn_of(
            std::env::var(crate::thread::session::CLOUD_SESSION_VAR)
                .ok()
                .filter(|v| !v.is_empty())
                .as_deref(),
            &repo.git_dir,
        )
    });
    let name = robot_name_of(&repo.git_dir, &repo.git, session.as_deref(), |urn| {
        robot_of_session_within_timeout(&cwd, urn)
    });
    robot_identity_from(name, &settings, &key_file).map(Some)
}

pub fn robot_for_init() -> Result<Option<String>, String> {
    let cwd = std::env::current_dir()
        .map_err(|e| format!("error: could not read the current directory: {}", e))?;
    let repo = crate::ledger::location::Repo::discover(&cwd)?;
    let settings = current_settings(&repo)?;
    if settings.identity != Identity::Robot {
        return Ok(None);
    }
    let key_file = key_file_for(&repo)?;
    let signers_file = key_file.with_file_name(SIGNERS_FILE_NAME);
    let written = signers_file.exists() && (settings.signing_key.is_some() || key_file.exists());
    if !written {
        return Err(format!(
            "{}\nnothing was changed: the nexus has no new entry. Run the command with {}=default, or run tsk thread session-start first",
            SIGNING_NOT_WRITTEN, IDENTITY_ENV
        ));
    }
    if settings.email.is_none() {
        return Err(missing_email());
    }
    let name = read_robot_name(&repo.git_dir).or_else(|| robot_name_from_env(&repo.git_dir));
    match name {
        Some(name) => Ok(Some(name)),
        None => Err(format!(
            "error: [attribution] identity is robot and this session has no robot name, so the ledger commits have no author\nnothing was changed: the nexus has no new entry. Register a robot for this session first: tsk actor register \"<name>\" --kind robot, or the construct-robot skill (/tsk:construct-robot), then run the command again. To go on without a robot, run it with {}=default",
            IDENTITY_ENV
        )),
    }
}

fn robot_name_from_env(git_dir: &Path) -> Option<String> {
    session_from_env()?;
    let name = std::env::var("GIT_AUTHOR_NAME").ok()?;
    let name = name.trim();
    check_git_name(name).ok()?;
    std::fs::write(git_dir.join(ROBOT_NAME_FILE), format!("{}\n", name)).ok()?;
    Some(name.to_string())
}

fn read_robot_name(git_dir: &Path) -> Option<String> {
    std::fs::read_to_string(git_dir.join(ROBOT_NAME_FILE))
        .ok()
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

fn robot_name_of(
    git_dir: &Path,
    git: &crate::ledger::git::Git,
    session: Option<&str>,
    lookup: impl FnOnce(&str) -> Option<String>,
) -> Option<String> {
    if let Some(name) = read_robot_name(git_dir) {
        return Some(name);
    }
    if let Some(name) = session
        .and_then(lookup)
        .filter(|name| check_git_name(name).is_ok())
    {
        let _ = std::fs::write(git_dir.join(ROBOT_NAME_FILE), format!("{}\n", name));
        return Some(name);
    }
    legacy_robot_name(git)
}

fn legacy_robot_name(git: &crate::ledger::git::Git) -> Option<String> {
    let marker = git
        .run_line(["config", "--worktree", "--get", "tsk.attribution"])
        .ok()?;
    if marker.trim() != "robot" {
        return None;
    }
    let name = git
        .run_line(["config", "--worktree", "--get", "user.name"])
        .ok()?;
    let name = name.trim();
    (!name.is_empty()).then(|| name.to_string())
}

fn robot_of_session(cwd: &Path, session: &str) -> Option<String> {
    let store = crate::ledger::store::open(cwd).ok()?;
    if !store.exists_locally().ok()? {
        return None;
    }
    actor::actors_of_session(store.as_ref(), session)
        .ok()?
        .into_iter()
        .find(|(role, actor)| {
            *role == crate::ledger::store::ActorRole::Primary && actor.kind == ActorKind::Robot
        })
        .map(|(_, actor)| actor.name)
}

static LOOKING_UP_ROBOT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn robot_of_session_within_timeout(cwd: &Path, session: &str) -> Option<String> {
    LOOKING_UP_ROBOT.store(true, std::sync::atomic::Ordering::SeqCst);
    let found = robot_lookup_thread(cwd, session);
    LOOKING_UP_ROBOT.store(false, std::sync::atomic::Ordering::SeqCst);
    found
}

fn robot_lookup_thread(cwd: &Path, session: &str) -> Option<String> {
    let cwd = cwd.to_path_buf();
    let session = session.to_string();
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = sender.send(robot_of_session(&cwd, &session));
    });
    receiver.recv_timeout(LOOKUP_TIMEOUT).ok().flatten()
}

fn robot_identity_from(
    name: Option<String>,
    settings: &Settings,
    key_file: &Path,
) -> Result<RobotIdentity, String> {
    let signers_file = key_file.with_file_name(SIGNERS_FILE_NAME);
    let (signing_key, program) = match &settings.signing_key {
        Some(signing_key) => (
            format!("key::{}", agent_public_key(signing_key).trim()),
            settings
                .signing_program
                .clone()
                .unwrap_or_else(|| DEFAULT_SIGNING_PROGRAM.to_string()),
        ),
        None => (
            key_file.display().to_string(),
            DEFAULT_SIGNING_PROGRAM.to_string(),
        ),
    };
    let written = signers_file.exists() && (settings.signing_key.is_some() || key_file.exists());
    if !written {
        return Err(SIGNING_NOT_WRITTEN.to_string());
    }
    let Some(name) = name else {
        return Err(
            "error: [attribution] identity is robot and no robot actor is registered, so a ledger commit has no author: run tsk actor attach <name> for a robot that exists (tsk actor list shows them), or tsk actor register <name> --kind robot for a new one"
                .to_string(),
        );
    };
    let Some(email) = settings.email.clone() else {
        return Err(missing_email());
    };
    Ok(RobotIdentity {
        name,
        email,
        key_file: signing_key,
        signers_file: signers_file.display().to_string(),
        program,
    })
}

/// A git handle for a command that creates commits in a ledger worktree, with the `-c`
/// options that go before the command: the handle and no options for the `default`
/// identity, the robot's for `robot`, and an error while a robot commit is not possible.
pub fn ledger_git(
    wt: &crate::ledger::git::Git,
) -> Result<(crate::ledger::git::Git, Vec<String>), String> {
    Ok(git_for(wt, ledger_identity()?))
}

fn git_for(
    wt: &crate::ledger::git::Git,
    identity: Option<RobotIdentity>,
) -> (crate::ledger::git::Git, Vec<String>) {
    match identity {
        Some(identity) => (wt.with_env(identity.git_env()), identity.git_options()),
        None => (wt.clone(), Vec::new()),
    }
}

/// Whether the session start wrote the signing files of this clone.
pub fn signing_is_set_up(session: &Session) -> bool {
    key_file_path(&session.repo)
        .map(|key_file| key_file.with_file_name(SIGNERS_FILE_NAME).exists())
        .unwrap_or(false)
}

/// The signing set-up, then the robot name for ledger commits. Does nothing for the
/// `default` identity, and records no name for a human actor.
pub fn apply(session: &Session) -> Result<Applied, String> {
    if let Signing::Off | Signing::Removed = set_up_signing(&session.repo)? {
        return Ok(Applied::Unchanged);
    }
    remember_robot_name(session)
}

/// The commit message with the harness's session trailers replaced: `Claude-Session`,
/// `Agent-Session` and `Co-Authored-By: Claude` lines go, so the caller can add one
/// `Agent-Session` trailer.
pub fn strip_session_trailers(message: &str) -> String {
    let mut out = String::new();
    for line in message.lines() {
        let lower = line.to_ascii_lowercase();
        if lower.starts_with("claude-session:")
            || lower.starts_with("agent-session:")
            || lower.starts_with("co-authored-by: claude")
        {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

pub fn session_from_env() -> Option<String> {
    std::env::var(SESSION_VAR)
        .ok()
        .filter(|urn| urn.starts_with(crate::sessions::URN_PREFIX))
}

pub fn human_joined(
    rows: &[crate::ledger::store::SessionActorRow],
    actors: &[crate::ledger::store::ActorRow],
    session: &str,
) -> bool {
    rows.iter().filter(|row| row.session == session).any(|row| {
        actors.iter().any(|actor| {
            actor.uid.as_deref() == Some(row.actor_uid.as_str()) && actor.kind == ActorKind::Human
        })
    })
}

fn human_joined_in(cwd: &Path, session: &str) -> Result<bool, String> {
    let store = crate::ledger::store::open(cwd)?;
    if !store.exists_locally()? {
        return Ok(false);
    }
    let rows = store.session_actors_list()?;
    if !rows.iter().any(|row| row.session == session) {
        return Ok(false);
    }
    Ok(human_joined(&rows, &store.actors_list()?, session))
}

const LOOKUP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

fn a_human_joined(cwd: &Path, session: &str) -> bool {
    let cwd = cwd.to_path_buf();
    let session = session.to_string();
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = sender.send(human_joined_in(&cwd, &session).unwrap_or(false));
    });
    receiver.recv_timeout(LOOKUP_TIMEOUT).unwrap_or(false)
}

/// `tsk actor commit-msg <file>`: the commit-msg hook. With `identity = "robot"` and the
/// session URN in `TSK_SESSION`, it writes `Agent-Session: <session URN>` (ADR 0029) in place
/// of the harness's session trailers, and adds `Co-authored-by: <co_author>` when a human
/// joined the session. With `TSK_SESSION` unset it changes nothing. It exits 0 whatever
/// happens, so a commit never fails because of it.
pub fn run_commit_msg(file: &Path) -> Result<i32, String> {
    let Some(urn) = session_from_env() else {
        return Ok(0);
    };
    let Ok(cwd) = std::env::current_dir() else {
        return Ok(0);
    };
    let Ok(repo) = crate::ledger::location::Repo::discover(&cwd) else {
        return Ok(0);
    };
    let repo_file = refs::repo_config_path(&repo);
    let user_file = crate::config::config_file_from_env();
    let Ok(settings) = settings(repo_file.as_deref(), user_file.as_deref()) else {
        return Ok(0);
    };
    if settings.identity != Identity::Robot {
        return Ok(0);
    }
    let Ok(message) = std::fs::read_to_string(file) else {
        return Ok(0);
    };
    if std::fs::write(file, strip_session_trailers(&message)).is_err() {
        return Ok(0);
    }
    let file_arg = file.display().to_string();
    let co_author = settings
        .co_author
        .as_deref()
        .filter(|_| a_human_joined(&cwd, &urn));
    for args in trailer_commands(&file_arg, &urn, co_author) {
        let _ = repo.git.run(args.iter().map(String::as_str));
    }
    Ok(0)
}

fn trailer_commands(file: &str, urn: &str, co_author: Option<&str>) -> Vec<Vec<String>> {
    let command = |mode: &str, trailer: String| {
        vec![
            "interpret-trailers".to_string(),
            "--in-place".to_string(),
            "--if-exists".to_string(),
            mode.to_string(),
            "--trailer".to_string(),
            trailer,
            file.to_string(),
        ]
    };
    let mut commands = vec![command("replace", format!("Agent-Session: {}", urn))];
    if let Some(value) = co_author {
        commands.push(command(
            "addIfDifferent",
            format!("Co-authored-by: {}", value),
        ));
    }
    commands
}

fn key_file_path(repo: &Repo) -> Result<PathBuf, String> {
    key_file_for(repo)
}

fn key_file_for(repo: &crate::ledger::location::Repo) -> Result<PathBuf, String> {
    let state_root = crate::ledger::location::state_root_from_env()?;
    let clone_id = repo.ensure_clone_id()?;
    Ok(state_root.join("repos").join(clone_id).join(KEY_FILE_NAME))
}

/// Write the key to `file` with mode 0600, replacing the file through a temporary file and
/// a rename.
fn write_key_file(file: &Path, key: &str) -> Result<(), String> {
    use std::io::Write;
    let parent = file
        .parent()
        .ok_or_else(|| format!("error: {} has no parent directory", file.display()))?;
    std::fs::create_dir_all(parent)
        .map_err(|e| format!("error: could not create {}: {}", parent.display(), e))?;
    let temp = file.with_extension("tmp");
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut handle = options
        .open(&temp)
        .map_err(|e| format!("error: could not write {}: {}", temp.display(), e))?;
    handle
        .write_all(key.as_bytes())
        .map_err(|e| format!("error: could not write {}: {}", temp.display(), e))?;
    drop(handle);
    std::fs::rename(&temp, file)
        .map_err(|e| format!("error: could not write {}: {}", file.display(), e))
}

/// `tsk actor git-identity`: print the `export` lines that the session start writes to the
/// file named by `CLAUDE_ENV_FILE`. It writes nothing.
pub fn run_git_identity() -> Result<i32, String> {
    let session = Session::discover()?;
    let urn = session.session_urn();
    match session_variables(&session, urn.as_deref())? {
        None => println!("git identity unchanged: [attribution] identity is not robot, or the actor is not a robot"),
        Some(variables) => {
            for line in export_lines(&variables) {
                println!("{}", line);
            }
        }
    }
    Ok(0)
}

/// `tsk config attribution`: print the settings, or write them to the repo config.
pub fn run_config(
    identity: Option<String>,
    email: Option<String>,
    co_author: Option<String>,
    signing_key: Option<String>,
    signing_program: Option<String>,
    user: bool,
) -> Result<(), String> {
    // With --user the command needs no repo, so a cloud setup script can run it from any
    // directory. Outside a repo it reads the user config alone.
    let cwd = std::env::current_dir()
        .map_err(|e| format!("error: could not read the current directory: {}", e))?;
    let session_repo = match crate::ledger::location::Repo::discover(&cwd) {
        Ok(repo) => Some(repo),
        Err(_) if user => None,
        Err(e) => return Err(e),
    };
    let repo_file = session_repo.as_ref().and_then(refs::repo_config_path);
    let user_file = crate::config::config_file_from_env();
    if identity.is_some()
        || email.is_some()
        || co_author.is_some()
        || signing_key.is_some()
        || signing_program.is_some()
    {
        let file = if user {
            user_file.clone().ok_or_else(|| {
                "error: no user config path: set HOME or XDG_CONFIG_HOME".to_string()
            })?
        } else {
            repo_file.clone().ok_or_else(|| {
                "error: tsk config attribution needs a repo with a working tree: it writes .tsk/config.toml at the top of it; use --user for the user config".to_string()
            })?
        };
        let identity = match identity {
            Some(value) => Some(Identity::parse(&value).ok_or_else(|| {
                format!("error: unknown identity {}: use default or robot", value)
            })?),
            None => None,
        };
        write_section(
            &file,
            identity,
            email.as_deref(),
            co_author.as_deref(),
            signing_key.as_deref(),
            signing_program.as_deref(),
        )?;
        println!("wrote {}", file.display());
    }
    let settings = settings(repo_file.as_deref(), user_file.as_deref())?;
    println!("attribution identity: {}", settings.identity.as_str());
    println!("source: {}", settings.identity_source.label());
    match (&settings.email, settings.email_source) {
        (Some(email), Some(source)) => {
            println!("attribution email: {}", email);
            println!("source: {}", source.label());
        }
        _ => println!("attribution email: not set"),
    }
    match (&settings.co_author, settings.co_author_source) {
        (Some(co_author), Some(source)) => {
            println!("attribution co-author: {}", co_author);
            println!("source: {}", source.label());
        }
        _ => println!("attribution co-author: not set"),
    }
    match (&settings.signing_key, settings.signing_key_source) {
        (Some(key), Some(source)) => {
            println!("attribution signing key: {}", key);
            println!("source: {}", source.label());
        }
        _ => println!("attribution signing key: not set"),
    }
    match (&settings.signing_program, settings.signing_program_source) {
        (Some(program), Some(source)) => {
            println!("attribution signing program: {}", program);
            println!("source: {}", source.label());
        }
        _ => println!(
            "attribution signing program: {} (default)",
            DEFAULT_SIGNING_PROGRAM
        ),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn robot() -> RobotIdentity {
        RobotIdentity {
            name: "Credit Where It's Due".to_string(),
            email: "robots@example.test".to_string(),
            key_file: "/k/key".to_string(),
            signers_file: "/k/signers".to_string(),
            program: "ssh-keygen".to_string(),
        }
    }

    #[test]
    fn the_robot_identity_names_the_author_and_the_signing_key_for_git() {
        let options = robot().git_options().join(" ");
        assert!(options.contains("-c user.name=Credit Where It's Due"));
        assert!(options.contains("-c user.email=robots@example.test"));
        assert!(options.contains("-c user.signingkey=/k/key"));
        assert!(options.contains("-c gpg.ssh.allowedSignersFile=/k/signers"));
        assert!(options.contains("-c commit.gpgsign=true"));
        let env = robot().git_env();
        assert!(env.contains(&(
            "GIT_AUTHOR_NAME".to_string(),
            "Credit Where It's Due".to_string()
        )));
        assert!(env.contains(&(
            "GIT_COMMITTER_EMAIL".to_string(),
            "robots@example.test".to_string()
        )));
    }

    #[test]
    fn the_robot_identity_names_the_author_and_the_committer_for_dolt() {
        let identity = robot();
        let env = identity.dolt_env();
        assert!(env.contains(&("DOLT_AUTHOR_NAME", "Credit Where It's Due")));
        assert!(env.contains(&("DOLT_COMMITTER_EMAIL", "robots@example.test")));
    }

    fn file() -> &'static Path {
        Path::new("config.toml")
    }

    fn section(identity: Option<Identity>, email: Option<&str>) -> Section {
        Section {
            identity,
            email: email.map(str::to_string),
            co_author: None,
            signing_key: None,
            signing_program: None,
        }
    }

    fn co_author_section(value: &str) -> Section {
        Section {
            co_author: Some(value.to_string()),
            ..Section::default()
        }
    }

    const JANE: &str = "Jane Doe <1+jane@users.noreply.github.com>";

    #[test]
    fn with_nothing_set_the_identity_is_default_and_there_is_no_email() {
        let settings = resolve(None, None, None, &Section::default(), &Section::default());
        assert_eq!(settings.identity, Identity::Default);
        assert_eq!(settings.identity_source, Source::Default);
        assert_eq!(settings.email, None);
        assert_eq!(settings.email_source, None);
    }

    #[test]
    fn the_environment_outranks_both_configs_for_the_identity() {
        let repo = section(Some(Identity::Robot), None);
        let user = section(Some(Identity::Robot), None);
        let settings = resolve(Some(Identity::Default), None, None, &repo, &user);
        assert_eq!(settings.identity, Identity::Default);
        assert_eq!(settings.identity_source, Source::Env);
        let settings = resolve(
            Some(Identity::Robot),
            None,
            None,
            &Section::default(),
            &Section::default(),
        );
        assert_eq!(settings.identity, Identity::Robot);
        assert_eq!(settings.identity_source, Source::Env);
    }

    #[test]
    fn the_repo_config_outranks_the_user_config_for_the_identity() {
        let repo = section(Some(Identity::Default), None);
        let user = section(Some(Identity::Robot), None);
        let settings = resolve(None, None, None, &repo, &user);
        assert_eq!(settings.identity, Identity::Default);
        assert_eq!(settings.identity_source, Source::RepoConfig);
        let settings = resolve(None, None, None, &Section::default(), &user);
        assert_eq!(settings.identity, Identity::Robot);
        assert_eq!(settings.identity_source, Source::UserConfig);
    }

    #[test]
    fn the_email_comes_from_the_environment_then_the_repo_then_the_user() {
        let repo = section(None, Some("repo@example.com"));
        let user = section(None, Some("user@example.com"));
        let settings = resolve(None, Some("env@example.com"), None, &repo, &user);
        assert_eq!(settings.email.as_deref(), Some("env@example.com"));
        assert_eq!(settings.email_source, Some(Source::Env));
        let settings = resolve(None, None, None, &repo, &user);
        assert_eq!(settings.email.as_deref(), Some("repo@example.com"));
        assert_eq!(settings.email_source, Some(Source::RepoConfig));
        let settings = resolve(None, Some("  "), None, &Section::default(), &user);
        assert_eq!(settings.email.as_deref(), Some("user@example.com"));
        assert_eq!(settings.email_source, Some(Source::UserConfig));
    }

    #[test]
    fn a_co_author_needs_a_name_and_an_address_in_angle_brackets() {
        assert_eq!(
            check_co_author(&format!("  {}  ", JANE)),
            Ok(JANE.to_string())
        );
        for bad in [
            "",
            "   ",
            "Jane Doe",
            "Jane Doe 1@example.com>",
            "Jane <1@example.com",
            "<1@example.com>",
            "A <b>\nc",
        ] {
            assert!(check_co_author(bad).is_err(), "{:?}", bad);
        }
        let err = parse_section("[attribution]\nco_author = \"\"\n", file()).unwrap_err();
        assert!(
            err.contains("co_author") && err.contains("non-empty"),
            "{}",
            err
        );
        let err = parse_section("[attribution]\nco_author = \"Jane\"\n", file()).unwrap_err();
        assert!(err.contains("Name <email>"), "{}", err);
        let err = parse_section("[attribution]\nco_author = 3\n", file()).unwrap_err();
        assert!(err.contains("co_author"), "{}", err);
    }

    #[test]
    fn the_co_author_comes_from_the_environment_then_the_repo_then_the_user() {
        let repo = co_author_section("Repo <r@example.com>");
        let user = co_author_section("User <u@example.com>");
        let settings = resolve(None, None, Some(JANE), &repo, &user);
        assert_eq!(settings.co_author.as_deref(), Some(JANE));
        assert_eq!(settings.co_author_source, Some(Source::Env));
        let settings = resolve(None, None, None, &repo, &user);
        assert_eq!(settings.co_author.as_deref(), Some("Repo <r@example.com>"));
        assert_eq!(settings.co_author_source, Some(Source::RepoConfig));
        let settings = resolve(None, None, Some(" "), &Section::default(), &user);
        assert_eq!(settings.co_author.as_deref(), Some("User <u@example.com>"));
        assert_eq!(settings.co_author_source, Some(Source::UserConfig));
        let settings = resolve(None, None, None, &Section::default(), &Section::default());
        assert_eq!(settings.co_author, None);
        assert_eq!(settings.co_author_source, None);
    }

    #[test]
    fn writing_a_co_author_keeps_the_other_keys_and_validates_it() {
        let text = "[attribution]\nidentity = \"robot\"\nemail = \"a@example.com\"\n";
        let out = with_section(text, None, None, Some(JANE), None, None, file()).unwrap();
        let read = parse_section(&out, file()).unwrap();
        assert_eq!(read.co_author.as_deref(), Some(JANE));
        assert_eq!(read.identity, Some(Identity::Robot));
        assert_eq!(read.email.as_deref(), Some("a@example.com"));
        let out = with_section(
            &out,
            None,
            None,
            Some("Joe <j@example.com>"),
            None,
            None,
            file(),
        )
        .unwrap();
        assert_eq!(out.matches("co_author").count(), 1);
        assert!(with_section("", None, None, Some("Jane"), None, None, file()).is_err());
        assert!(with_section("", None, None, Some(""), None, None, file()).is_err());
    }

    fn rewrite(message: &str, co_author: Option<&str>) -> String {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("MSG");
        std::fs::write(&file, strip_session_trailers(message)).unwrap();
        let file_arg = file.display().to_string();
        for args in trailer_commands(&file_arg, "urn:tsk:session:claude-cli:s1", co_author) {
            let status = std::process::Command::new("git")
                .args(&args)
                .status()
                .unwrap();
            assert!(status.success());
        }
        std::fs::read_to_string(&file).unwrap()
    }

    #[test]
    fn the_hook_text_has_the_co_author_after_the_agent_session_when_it_is_set() {
        let out = rewrite("Add a thing\n\nBody.\n", Some(JANE));
        assert_eq!(
            out,
            format!(
                "Add a thing\n\nBody.\n\nAgent-Session: urn:tsk:session:claude-cli:s1\nCo-authored-by: {}\n",
                JANE
            )
        );
    }

    #[test]
    fn the_hook_text_has_no_co_author_when_it_is_not_set() {
        let out = rewrite("Add a thing\n\nBody.\n", None);
        assert_eq!(
            out,
            "Add a thing\n\nBody.\n\nAgent-Session: urn:tsk:session:claude-cli:s1\n"
        );
    }

    #[test]
    fn the_hook_does_not_repeat_a_co_author_and_keeps_other_trailers() {
        let message = format!(
            "Add a thing\n\nSigned-off-by: A <a@example.com>\nco-authored-by: {}\nCo-authored-by: Joe <j@example.com>\n",
            JANE
        );
        let out = rewrite(&message, Some(JANE));
        assert_eq!(out.to_lowercase().matches(&JANE.to_lowercase()).count(), 1);
        assert!(out.contains("Signed-off-by: A <a@example.com>"));
        assert!(out.contains("Co-authored-by: Joe <j@example.com>"));
        assert!(out.contains("Agent-Session: urn:tsk:session:claude-cli:s1"));
    }

    fn actor_row(name: &str, kind: ActorKind, uid: &str) -> crate::ledger::store::ActorRow {
        crate::ledger::store::ActorRow {
            name: name.to_string(),
            kind,
            registered_at: "2026-01-01T00:00:00Z".to_string(),
            uid: Some(uid.to_string()),
        }
    }

    fn joined(session: &str, uid: &str) -> crate::ledger::store::SessionActorRow {
        crate::ledger::store::SessionActorRow {
            session: session.to_string(),
            actor_uid: uid.to_string(),
            role: crate::ledger::store::ActorRole::Supporting,
            joined_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn a_session_is_supervised_when_a_human_actor_joined_that_session() {
        let actors = [
            actor_row("Robot", ActorKind::Robot, "r1"),
            actor_row("Jane", ActorKind::Human, "h1"),
        ];
        let session = "urn:tsk:session:claude-cli:s1";
        let other = "urn:tsk:session:claude-cli:s2";
        assert!(!human_joined(&[joined(session, "r1")], &actors, session));
        assert!(human_joined(
            &[joined(session, "r1"), joined(session, "h1")],
            &actors,
            session
        ));
        assert!(!human_joined(&[joined(other, "h1")], &actors, session));
        assert!(!human_joined(&[], &actors, session));
    }

    fn variables() -> Vec<(String, String)> {
        let config = signing_settings("r@example.com", Path::new("/k"), Path::new("/s"));
        robot_variables(
            "Credit Where It's Due",
            "r@example.com",
            &config,
            Some("urn:tsk:session:claude-cli:s1"),
        )
    }

    #[test]
    fn the_robot_variables_name_the_author_the_signing_config_and_the_session() {
        let lines = export_lines(&variables());
        assert_eq!(
            lines,
            vec![
                "export GIT_AUTHOR_NAME='Credit Where It'\\''s Due'",
                "export GIT_AUTHOR_EMAIL='r@example.com'",
                "export GIT_COMMITTER_NAME='Credit Where It'\\''s Due'",
                "export GIT_COMMITTER_EMAIL='r@example.com'",
                "export GIT_CONFIG_COUNT='4'",
                "export GIT_CONFIG_KEY_0='commit.gpgsign'",
                "export GIT_CONFIG_VALUE_0='true'",
                "export GIT_CONFIG_KEY_1='gpg.format'",
                "export GIT_CONFIG_VALUE_1='ssh'",
                "export GIT_CONFIG_KEY_2='gpg.ssh.program'",
                "export GIT_CONFIG_VALUE_2='ssh-keygen'",
                "export GIT_CONFIG_KEY_3='user.signingkey'",
                "export GIT_CONFIG_VALUE_3='/k'",
                "export TSK_SESSION='urn:tsk:session:claude-cli:s1'",
            ]
        );
    }

    #[test]
    fn a_value_with_an_apostrophe_is_quoted_so_a_shell_reads_it_back() {
        let quoted = shell_quote("Credit Where It's Due");
        let output = std::process::Command::new("sh")
            .arg("-c")
            .arg(format!("printf %s {}", quoted))
            .output()
            .unwrap();
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            "Credit Where It's Due"
        );
    }

    #[test]
    fn the_env_file_keeps_foreign_lines_and_replaces_its_own() {
        let existing = "export TSK_LEDGER_WT=\"/l\"\nexport GIT_AUTHOR_NAME='old'\nexport GIT_CONFIG_COUNT='1'\nexport GIT_CONFIG_KEY_7='x'\nexport OTHER='keep'\nexport TSK_SESSION='old'\n";
        let edited = edit_env_file(existing, &variables());
        assert!(edited.starts_with("export TSK_LEDGER_WT=\"/l\"\nexport OTHER='keep'\n"));
        assert_eq!(edited.matches("GIT_AUTHOR_NAME").count(), 1);
        assert_eq!(edited.matches("GIT_CONFIG_COUNT").count(), 1);
        assert_eq!(edited.matches("GIT_CONFIG_KEY_7").count(), 0);
        assert_eq!(edited.matches("TSK_SESSION").count(), 1);
        assert!(!edited.contains("'old'"));
    }

    #[test]
    fn repeated_sources_leave_one_copy_of_each_line() {
        let mut text = "export OTHER='keep'\n".to_string();
        for _ in 0..3 {
            text = edit_env_file(&text, &variables());
        }
        assert_eq!(text, edit_env_file("export OTHER='keep'\n", &variables()));
        assert_eq!(text.matches("export GIT_AUTHOR_NAME=").count(), 1);
    }

    #[test]
    fn no_robot_removes_the_lines_and_keeps_the_others() {
        let text = edit_env_file("export OTHER='keep'\n", &variables());
        assert_eq!(edit_env_file(&text, &[]), "export OTHER='keep'\n");
        assert_eq!(edit_env_file("", &[]), "");
    }

    #[test]
    fn the_env_file_is_created_only_for_a_robot_and_rewritten_in_place() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("env");
        update_env_file(&file, &[]).unwrap();
        assert!(!file.exists());
        update_env_file(&file, &variables()).unwrap();
        let first = std::fs::read_to_string(&file).unwrap();
        update_env_file(&file, &variables()).unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), first);
        update_env_file(&file, &[]).unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "");
    }

    #[test]
    fn the_exported_config_reaches_git_as_command_line_settings_and_not_as_a_file() {
        let dir = tempfile::tempdir().unwrap();
        git_in(dir.path(), &["init", "-q", "-b", "main"]);
        let mut command = std::process::Command::new("git");
        command
            .current_dir(dir.path())
            .args(["config", "--list", "--show-origin"]);
        for (name, value) in variables() {
            command.env(name, value);
        }
        let output = command.output().unwrap();
        let listing = String::from_utf8_lossy(&output.stdout).to_string();
        for expected in [
            "command line:\tcommit.gpgsign=true",
            "command line:\tgpg.format=ssh",
            "command line:\tgpg.ssh.program=ssh-keygen",
            "command line:\tuser.signingkey=/k",
        ] {
            assert!(
                listing.lines().any(|l| l == expected),
                "{}\n{}",
                expected,
                listing
            );
        }
        let config = std::fs::read_to_string(dir.path().join(".git/config")).unwrap();
        assert!(!config.contains("/k"), "{}", config);
        assert!(!config.contains("gpgsign"), "{}", config);
        assert!(!dir.path().join(".git/config.worktree").exists());
    }

    #[test]
    fn a_section_is_read_from_the_attribution_table() {
        let text = "[attribution]\nidentity = \"robot\"\nemail = \"robots@jimbarritt.com\"\n";
        assert_eq!(
            parse_section(text, file()),
            Ok(section(
                Some(Identity::Robot),
                Some("robots@jimbarritt.com")
            ))
        );
        assert_eq!(parse_section("", file()), Ok(Section::default()));
        assert_eq!(
            parse_section("[ledger]\nrefs = \"branches\"\n", file()),
            Ok(Section::default())
        );
    }

    #[test]
    fn a_bad_identity_or_email_is_an_error() {
        let err = parse_section("[attribution]\nidentity = \"bot\"\n", file()).unwrap_err();
        assert!(err.contains("must be default or robot"), "{}", err);
        let err = parse_section("[attribution]\nemail = \"\"\n", file()).unwrap_err();
        assert!(err.contains("non-empty string"), "{}", err);
        let err = parse_section("attribution = 3\n", file()).unwrap_err();
        assert!(err.contains("must be a table"), "{}", err);
    }

    #[test]
    fn writing_a_section_keeps_other_tables_and_comments() {
        let text = "# a comment\n[ledger]\nrefs = \"branches\"\n";
        let out = with_section(
            text,
            Some(Identity::Robot),
            Some("robots@jimbarritt.com"),
            None,
            None,
            None,
            file(),
        )
        .unwrap();
        assert!(out.starts_with("# a comment\n[ledger]\nrefs = \"branches\"\n"));
        let read = parse_section(&out, file()).unwrap();
        assert_eq!(
            read,
            section(Some(Identity::Robot), Some("robots@jimbarritt.com"))
        );
    }

    #[test]
    fn writing_a_section_replaces_an_existing_line_and_keeps_the_other_key() {
        let text = "[attribution]\nidentity = \"robot\"\nemail = \"a@example.com\"\n\n[ledger]\nrefs = \"custom\"\n";
        let out =
            with_section(text, None, Some("b@example.com"), None, None, None, file()).unwrap();
        let read = parse_section(&out, file()).unwrap();
        assert_eq!(read, section(Some(Identity::Robot), Some("b@example.com")));
        assert!(out.contains("[ledger]\nrefs = \"custom\"\n"));
        assert_eq!(out.matches("email").count(), 1);
    }

    #[test]
    fn a_value_with_a_quote_is_refused() {
        let err =
            with_section("", None, Some("a\"b@example.com"), None, None, None, file()).unwrap_err();
        assert!(err.contains("cannot hold a quote"), "{}", err);
    }

    #[test]
    fn a_key_with_line_breaks_or_with_backslash_n_is_accepted() {
        let multi = format!("{}\nabc\n-----END OPENSSH PRIVATE KEY-----\n", KEY_HEADER);
        let single = format!("{}\\nabc\\n-----END OPENSSH PRIVATE KEY-----", KEY_HEADER);
        assert_eq!(normalise_key(&multi).unwrap(), multi);
        assert_eq!(normalise_key(&single).unwrap(), multi);
    }

    #[test]
    fn a_value_that_is_not_an_openssh_key_is_refused_without_printing_it() {
        let err = normalise_key("hunter2").unwrap_err();
        assert!(!err.contains("hunter2"), "{}", err);
        assert!(err.contains(KEY_ENV), "{}", err);
    }

    #[test]
    fn the_signing_settings_name_the_email_and_the_signing_key_and_no_user_name() {
        let settings = signing_settings("r@example.com", Path::new("/k"), Path::new("/s"));
        assert_eq!(
            settings,
            vec![
                ("user.email", "r@example.com".to_string()),
                ("gpg.format", "ssh".to_string()),
                ("gpg.ssh.program", "ssh-keygen".to_string()),
                ("user.signingkey", "/k".to_string()),
                ("gpg.ssh.allowedSignersFile", "/s".to_string()),
                ("commit.gpgsign", "true".to_string()),
            ]
        );
    }

    const PUBLIC: &str =
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIExampleExampleExampleExampleExampleExample";

    fn agent_value() -> String {
        format!("agent:{}", PUBLIC)
    }

    fn signing_section(key: Option<&str>, program: Option<&str>) -> Section {
        Section {
            signing_key: key.map(str::to_string),
            signing_program: program.map(str::to_string),
            ..Section::default()
        }
    }

    #[test]
    fn a_signing_key_needs_the_agent_prefix_and_an_ssh_public_key() {
        assert_eq!(
            check_signing_key(&format!("  {}  ", agent_value())),
            Ok(agent_value())
        );
        assert_eq!(
            check_signing_key(&format!("{} comment", agent_value())),
            Ok(agent_value())
        );
        for bad in [
            "",
            "  ",
            PUBLIC,
            "agent:",
            "agent:ssh-ed25519",
            "agent:ed25519 AAAA",
            "agent:ssh- AAAA",
            "agent:ssh-ed25519 AA!AA",
            "file:ssh-ed25519 AAAA",
        ] {
            assert!(check_signing_key(bad).is_err(), "{:?}", bad);
        }
        let err = parse_section("[attribution]\nsigning_key = \"nope\"\n", file()).unwrap_err();
        assert!(
            err.contains("signing_key") && err.contains("agent:"),
            "{}",
            err
        );
        let err = parse_section("[attribution]\nsigning_key = 3\n", file()).unwrap_err();
        assert!(err.contains("signing_key"), "{}", err);
        let err = parse_section("[attribution]\nsigning_program = \"\"\n", file()).unwrap_err();
        assert!(err.contains("signing_program"), "{}", err);
    }

    #[test]
    fn the_signing_key_and_program_come_from_the_environment_then_the_repo_then_the_user() {
        let repo = signing_section(Some("agent:ssh-rsa REPO"), Some("/repo/sign"));
        let user = signing_section(Some("agent:ssh-rsa USER"), Some("/user/sign"));
        let (key, source) = resolve_signing_key(Some(&agent_value()), &repo, &user);
        assert_eq!(key, Some(agent_value()));
        assert_eq!(source, Some(Source::Env));
        let (key, source) = resolve_signing_key(None, &repo, &user);
        assert_eq!(key.as_deref(), Some("agent:ssh-rsa REPO"));
        assert_eq!(source, Some(Source::RepoConfig));
        let (key, source) = resolve_signing_key(Some(" "), &Section::default(), &user);
        assert_eq!(key.as_deref(), Some("agent:ssh-rsa USER"));
        assert_eq!(source, Some(Source::UserConfig));
        assert_eq!(
            resolve_signing_key(None, &Section::default(), &Section::default()),
            (None, None)
        );
        let (program, source) = resolve_signing_program(Some("/env/sign"), &repo, &user);
        assert_eq!(program.as_deref(), Some("/env/sign"));
        assert_eq!(source, Some(Source::Env));
        let (program, source) = resolve_signing_program(None, &repo, &user);
        assert_eq!(program.as_deref(), Some("/repo/sign"));
        assert_eq!(source, Some(Source::RepoConfig));
        let (program, source) = resolve_signing_program(None, &Section::default(), &user);
        assert_eq!(program.as_deref(), Some("/user/sign"));
        assert_eq!(source, Some(Source::UserConfig));
        let settings = resolve(None, None, None, &repo, &user);
        assert_eq!(settings.signing_key_source, Some(Source::RepoConfig));
    }

    #[test]
    fn writing_a_signing_key_and_program_keeps_the_other_keys_and_validates_them() {
        let text = "[attribution]\nidentity = \"robot\"\n";
        let out = with_section(
            text,
            None,
            None,
            None,
            Some(&agent_value()),
            Some("/p/op-ssh-sign"),
            file(),
        )
        .unwrap();
        let read = parse_section(&out, file()).unwrap();
        assert_eq!(read.signing_key, Some(agent_value()));
        assert_eq!(read.signing_program.as_deref(), Some("/p/op-ssh-sign"));
        assert_eq!(read.identity, Some(Identity::Robot));
        assert!(with_section("", None, None, None, Some(PUBLIC), None, file()).is_err());
    }

    #[test]
    fn the_agent_signing_settings_name_the_public_key_and_the_program() {
        let settings = agent_signing_settings(
            "1+robot@users.noreply.github.com",
            PUBLIC,
            "/p/op-ssh-sign",
            Path::new("/s"),
        );
        assert_eq!(
            settings,
            vec![
                ("user.email", "1+robot@users.noreply.github.com".to_string()),
                ("gpg.format", "ssh".to_string()),
                ("gpg.ssh.program", "/p/op-ssh-sign".to_string()),
                ("user.signingkey", format!("key::{}", PUBLIC)),
                ("gpg.ssh.allowedSignersFile", "/s".to_string()),
                ("commit.gpgsign", "true".to_string()),
            ]
        );
        assert_eq!(agent_public_key(&agent_value()), PUBLIC);
    }

    #[test]
    fn the_ledger_identity_signs_with_the_program_it_holds() {
        let mut identity = robot();
        identity.program = "/p/op-ssh-sign".to_string();
        assert!(identity
            .git_options()
            .join(" ")
            .contains("-c gpg.ssh.program=/p/op-ssh-sign"));
    }

    #[test]
    fn the_session_trailers_of_the_harness_are_stripped_and_other_trailers_kept() {
        let message = "Subject\n\nBody.\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nco-authored-by: claude <x@y>\nClaude-Session: https://claude.ai/code/session_1\nAgent-Session: urn:tsk:session:claude-cloud:cse_old\nCo-Authored-By: Jim Barritt <jim@example.com>\n";
        assert_eq!(
            strip_session_trailers(message),
            "Subject\n\nBody.\n\nCo-Authored-By: Jim Barritt <jim@example.com>\n"
        );
    }

    #[test]
    fn the_hook_holds_the_marker_and_does_nothing_without_tsk() {
        let text = hook_text();
        assert!(text.starts_with("#!/bin/sh\n"));
        assert!(text.lines().any(|line| line == HOOK_MARKER));
        assert!(text.contains("command -v tsk >/dev/null 2>&1 || exit 0"));
        assert!(text.contains("tsk actor commit-msg \"$1\" 2>/dev/null\nexit 0\n"));
    }

    #[test]
    fn a_hook_is_the_tsk_hook_only_with_the_marker_or_the_old_link() {
        let dir = tempfile::tempdir().unwrap();
        let ours = dir.path().join("ours");
        std::fs::write(&ours, hook_text()).unwrap();
        assert!(is_tsk_hook(&ours));
        let theirs = dir.path().join("theirs");
        std::fs::write(&theirs, "#!/bin/sh\nexit 0\n").unwrap();
        assert!(!is_tsk_hook(&theirs));
        #[cfg(unix)]
        {
            let old = dir.path().join("old");
            std::os::unix::fs::symlink("/repo/ops/local/git-commit-msg.sh", &old).unwrap();
            assert!(is_tsk_hook(&old));
        }
    }

    #[test]
    fn a_signers_line_names_the_email_then_the_public_key() {
        assert_eq!(
            signers_line("r@example.com", "ssh-ed25519 AAAA\n"),
            "r@example.com ssh-ed25519 AAAA\n"
        );
    }

    #[test]
    fn a_name_with_angle_brackets_or_a_line_break_is_refused() {
        assert!(check_git_name("Mistake Not…").is_ok());
        assert!(check_git_name("a <b>").is_err());
        assert!(check_git_name("a\nb").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn the_key_file_has_mode_0600() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("tsk-attribution-{}", std::process::id()));
        let file = dir.join("robot-signing-key");
        write_key_file(&file, "key\n").unwrap();
        let mode = std::fs::metadata(&file).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "key\n");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    fn git_in(dir: &Path, args: &[&str]) {
        let output = std::process::Command::new("git")
            .current_dir(dir)
            .args(args)
            .output()
            .unwrap();
        assert!(output.status.success(), "git {:?}: {:?}", args, output);
    }

    fn fake_program(dir: &Path) -> (PathBuf, PathBuf) {
        use std::os::unix::fs::PermissionsExt;
        let log = dir.join("fake-signer.log");
        let program = dir.join("fake-signer");
        std::fs::write(
            &program,
            format!("#!/bin/sh\necho \"$@\" >> {}\nexit 1\n", log.display()),
        )
        .unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        (program, log)
    }

    fn settings_with(signing_key: Option<&str>, program: Option<&str>) -> Settings {
        Settings {
            identity: Identity::Robot,
            identity_source: Source::Env,
            email: Some("robot@example.test".to_string()),
            email_source: Some(Source::Env),
            co_author: None,
            co_author_source: None,
            signing_key: signing_key.map(str::to_string),
            signing_key_source: signing_key.map(|_| Source::Env),
            signing_program: program.map(str::to_string),
            signing_program_source: program.map(|_| Source::Env),
        }
    }

    fn written_set_up(dir: &Path, key_file: bool) -> PathBuf {
        let key = dir.join(KEY_FILE_NAME);
        if key_file {
            std::fs::write(&key, "k").unwrap();
        }
        std::fs::write(dir.join(SIGNERS_FILE_NAME), "s").unwrap();
        key
    }

    #[test]
    fn a_ledger_commit_signs_with_the_program_of_the_settings_in_agent_mode() {
        let code = tempfile::tempdir().unwrap();
        let ledger = tempfile::tempdir().unwrap();
        let tools = tempfile::tempdir().unwrap();
        let (program, log) = fake_program(tools.path());
        let public = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIPUBLIC";
        let key = written_set_up(code.path(), false);
        git_in(ledger.path(), &["init", "-q", "-b", "main"]);
        std::fs::write(ledger.path().join("a"), "a").unwrap();
        git_in(ledger.path(), &["add", "-A"]);

        let settings = settings_with(
            Some(&format!("agent:{}", public)),
            Some(program.to_str().unwrap()),
        );
        let identity = robot_identity_from(Some("Robot".to_string()), &settings, &key).unwrap();
        assert_eq!(identity.program, program.to_str().unwrap());
        assert_eq!(identity.key_file, format!("key::{}", public));

        let wt = crate::ledger::git::Git::new(ledger.path());
        let (git, mut args) = git_for(&wt, Some(identity));
        args.extend(["commit", "--quiet", "-m", "x"].map(String::from));
        assert!(git.run(args).is_err());
        let recorded = std::fs::read_to_string(&log).unwrap();
        assert!(recorded.contains("-Y sign"), "{recorded}");
    }

    #[test]
    fn the_ledger_identity_in_key_file_mode_names_the_key_file_and_ssh_keygen() {
        let dir = tempfile::tempdir().unwrap();
        let key = written_set_up(dir.path(), true);
        let identity =
            robot_identity_from(Some("Robot".to_string()), &settings_with(None, None), &key)
                .unwrap();
        assert_eq!(identity.program, "ssh-keygen");
        assert_eq!(identity.key_file, key.display().to_string());
        assert_eq!(identity.email, "robot@example.test");
    }

    #[test]
    fn the_ledger_identity_needs_the_set_up_and_a_robot_name() {
        let dir = tempfile::tempdir().unwrap();
        let key = dir.path().join(KEY_FILE_NAME);
        let settings = settings_with(None, None);
        let err = robot_identity_from(Some("Robot".to_string()), &settings, &key).unwrap_err();
        assert!(err.contains("signing set-up is not written"), "{}", err);
        written_set_up(dir.path(), true);
        let err = robot_identity_from(None, &settings, &key).unwrap_err();
        assert!(err.contains("no robot actor is registered"), "{}", err);
    }

    fn fresh_repo(dir: &Path) -> crate::ledger::git::Git {
        let git = crate::ledger::git::Git::new(dir);
        git.run(["init", "-q"]).unwrap();
        git
    }

    #[test]
    fn a_missing_name_file_resolves_from_the_session_actor_and_writes_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let git = fresh_repo(dir.path());
        let git_dir = dir.path().join(".git");
        let name = robot_name_of(&git_dir, &git, Some("urn:tsk:session:x"), |urn| {
            assert_eq!(urn, "urn:tsk:session:x");
            Some("Gurnard".to_string())
        });
        assert_eq!(name.as_deref(), Some("Gurnard"));
        assert_eq!(read_robot_name(&git_dir).as_deref(), Some("Gurnard"));
    }

    #[test]
    fn a_missing_name_file_and_no_actor_gives_no_name() {
        let dir = tempfile::tempdir().unwrap();
        let git = fresh_repo(dir.path());
        let git_dir = dir.path().join(".git");
        assert_eq!(robot_name_of(&git_dir, &git, Some("urn:x"), |_| None), None);
        assert_eq!(robot_name_of(&git_dir, &git, None, |_| None), None);
        assert!(read_robot_name(&git_dir).is_none());
    }

    #[test]
    fn a_present_name_file_is_used_without_a_lookup() {
        let dir = tempfile::tempdir().unwrap();
        let git = fresh_repo(dir.path());
        let git_dir = dir.path().join(".git");
        std::fs::write(git_dir.join(ROBOT_NAME_FILE), "Present\n").unwrap();
        let name = robot_name_of(&git_dir, &git, Some("urn:x"), |_| {
            panic!("the lookup must not run")
        });
        assert_eq!(name.as_deref(), Some("Present"));
    }

    #[test]
    fn an_older_robot_set_up_in_the_worktree_config_names_the_robot() {
        let dir = tempfile::tempdir().unwrap();
        let git = fresh_repo(dir.path());
        let git_dir = dir.path().join(".git");
        git.run(["config", "--worktree", "user.name", "Older"])
            .unwrap();
        git.run(["config", "--worktree", "tsk.attribution", "robot"])
            .unwrap();
        let name = robot_name_of(&git_dir, &git, None, |_| None);
        assert_eq!(name.as_deref(), Some("Older"));
    }
}
