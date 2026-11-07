//! Robot commit attribution: the `[attribution]` settings, and the git identity that
//! `tsk actor git-identity` writes to the repo's local git config.
//!
//! `identity` is `default` or `robot`, `default` when unset. `default` writes no git config.
//! `robot` writes the registered actor's name, the `email` setting, and the SSH signing
//! settings. `email` has no built-in value. The key comes from `TSK_ROBOT_SIGNING_KEY`.

use std::path::{Path, PathBuf};

use crate::actor::{self, Current};
use crate::ledger::refs;
use crate::ledger::store::ActorKind;
use crate::thread::session::Session;

/// The environment variable that overrides `[attribution] email`.
pub const EMAIL_ENV: &str = "TSK_ATTRIBUTION_EMAIL";

/// The environment variable that holds the private SSH signing key.
pub const KEY_ENV: &str = "TSK_ROBOT_SIGNING_KEY";

/// The name of the file the key is written to, beside the ledger store of the clone.
const KEY_FILE_NAME: &str = "robot-signing-key";

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
}

/// The settings in force, with where each came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    pub identity: Identity,
    pub identity_source: Source,
    pub email: Option<String>,
    pub email_source: Option<Source>,
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
    Ok(Section { identity, email })
}

/// Resolve the settings. `identity`: the repo config, then the user config, then `default`.
/// `email`: the environment, then the repo config, then the user config. An empty
/// environment value counts as unset.
pub fn resolve(env_email: Option<&str>, repo: &Section, user: &Section) -> Settings {
    let (identity, identity_source) = match (repo.identity, user.identity) {
        (Some(value), _) => (value, Source::RepoConfig),
        (None, Some(value)) => (value, Source::UserConfig),
        (None, None) => (Identity::Default, Source::Default),
    };
    let env_email = env_email.map(str::trim).filter(|v| !v.is_empty());
    let (email, email_source) = match (env_email, &repo.email, &user.email) {
        (Some(value), _, _) => (Some(value.to_string()), Some(Source::Env)),
        (None, Some(value), _) => (Some(value.clone()), Some(Source::RepoConfig)),
        (None, None, Some(value)) => (Some(value.clone()), Some(Source::UserConfig)),
        (None, None, None) => (None, None),
    };
    Settings {
        identity,
        identity_source,
        email,
        email_source,
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
    Ok(resolve(
        std::env::var(EMAIL_ENV).ok().as_deref(),
        &repo,
        &user,
    ))
}

/// Write `[attribution]` in a config file and keep every other key. A value of `None`
/// leaves that key as it is. Creates the directory.
pub fn write_section(
    file: &Path,
    identity: Option<Identity>,
    email: Option<&str>,
) -> Result<(), String> {
    let text = match std::fs::read_to_string(file) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(format!("error: could not read {}: {}", file.display(), e)),
    };
    let text = with_section(&text, identity, email, file)?;
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
    file: &Path,
) -> Result<String, String> {
    parse_section(text, file)?;
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

/// The git config keys and values the `robot` identity writes.
pub fn git_settings(name: &str, email: &str, key_file: &Path) -> Vec<(&'static str, String)> {
    vec![
        ("user.name", name.to_string()),
        ("user.email", email.to_string()),
        ("gpg.format", "ssh".to_string()),
        ("user.signingkey", key_file.display().to_string()),
        ("commit.gpgsign", "true".to_string()),
    ]
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

/// Write the git identity of the registered actor to the repo's local git config, when the
/// identity is `robot` and the actor is a robot. Does nothing for the `default` identity
/// and for a human actor.
pub fn apply(session: &Session) -> Result<Applied, String> {
    let repo_file = refs::repo_config_path(&session.repo);
    let user_file = crate::config::config_file_from_env();
    let settings = settings(repo_file.as_deref(), user_file.as_deref())?;
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
    let email = settings.email.ok_or_else(|| {
        format!(
            "error: [attribution] identity is robot and no email is set: run tsk config attribution robot --email <address>, or set {}",
            EMAIL_ENV
        )
    })?;
    let key = std::env::var(KEY_ENV)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .ok_or_else(|| format!("error: {} is not set", KEY_ENV))?;
    let key = normalise_key(&key)?;
    let key_file = key_file_path(session)?;
    write_key_file(&key_file, &key)?;
    for (config_key, value) in git_settings(&name, &email, &key_file) {
        session
            .repo
            .git
            .run(["config", "--local", config_key, value.as_str()])?;
    }
    Ok(Applied::Written { name, email })
}

fn key_file_path(session: &Session) -> Result<PathBuf, String> {
    let state_root = crate::ledger::location::state_root_from_env()?;
    let clone_id = session.repo.ensure_clone_id()?;
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

/// `tsk actor git-identity`.
pub fn run_git_identity() -> Result<i32, String> {
    let session = Session::discover()?;
    match apply(&session)? {
        Applied::Unchanged => println!("git identity unchanged: [attribution] identity is not robot, or the actor is not a robot"),
        Applied::Written { name, email } => println!("git identity: {} <{}>, signed", name, email),
    }
    Ok(0)
}

/// `tsk config attribution`: print the settings, or write them to the repo config.
pub fn run_config(identity: Option<String>, email: Option<String>) -> Result<(), String> {
    let session_repo = {
        let cwd = std::env::current_dir()
            .map_err(|e| format!("error: could not read the current directory: {}", e))?;
        crate::ledger::location::Repo::discover(&cwd)?
    };
    let repo_file = refs::repo_config_path(&session_repo);
    let user_file = crate::config::config_file_from_env();
    if identity.is_some() || email.is_some() {
        let file = repo_file.clone().ok_or_else(|| {
            "error: tsk config attribution needs a repo with a working tree: it writes .tsk/config.toml at the top of it".to_string()
        })?;
        let identity = match identity {
            Some(value) => Some(Identity::parse(&value).ok_or_else(|| {
                format!("error: unknown identity {}: use default or robot", value)
            })?),
            None => None,
        };
        write_section(&file, identity, email.as_deref())?;
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
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file() -> &'static Path {
        Path::new("config.toml")
    }

    fn section(identity: Option<Identity>, email: Option<&str>) -> Section {
        Section {
            identity,
            email: email.map(str::to_string),
        }
    }

    #[test]
    fn with_nothing_set_the_identity_is_default_and_there_is_no_email() {
        let settings = resolve(None, &Section::default(), &Section::default());
        assert_eq!(settings.identity, Identity::Default);
        assert_eq!(settings.identity_source, Source::Default);
        assert_eq!(settings.email, None);
        assert_eq!(settings.email_source, None);
    }

    #[test]
    fn the_repo_config_outranks_the_user_config_for_the_identity() {
        let repo = section(Some(Identity::Default), None);
        let user = section(Some(Identity::Robot), None);
        let settings = resolve(None, &repo, &user);
        assert_eq!(settings.identity, Identity::Default);
        assert_eq!(settings.identity_source, Source::RepoConfig);
        let settings = resolve(None, &Section::default(), &user);
        assert_eq!(settings.identity, Identity::Robot);
        assert_eq!(settings.identity_source, Source::UserConfig);
    }

    #[test]
    fn the_email_comes_from_the_environment_then_the_repo_then_the_user() {
        let repo = section(None, Some("repo@example.com"));
        let user = section(None, Some("user@example.com"));
        let settings = resolve(Some("env@example.com"), &repo, &user);
        assert_eq!(settings.email.as_deref(), Some("env@example.com"));
        assert_eq!(settings.email_source, Some(Source::Env));
        let settings = resolve(None, &repo, &user);
        assert_eq!(settings.email.as_deref(), Some("repo@example.com"));
        assert_eq!(settings.email_source, Some(Source::RepoConfig));
        let settings = resolve(Some("  "), &Section::default(), &user);
        assert_eq!(settings.email.as_deref(), Some("user@example.com"));
        assert_eq!(settings.email_source, Some(Source::UserConfig));
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
        let out = with_section(text, None, Some("b@example.com"), file()).unwrap();
        let read = parse_section(&out, file()).unwrap();
        assert_eq!(read, section(Some(Identity::Robot), Some("b@example.com")));
        assert!(out.contains("[ledger]\nrefs = \"custom\"\n"));
        assert_eq!(out.matches("email").count(), 1);
    }

    #[test]
    fn a_value_with_a_quote_is_refused() {
        let err = with_section("", None, Some("a\"b@example.com"), file()).unwrap_err();
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
    fn the_git_settings_name_the_actor_the_email_and_the_signing_key() {
        let settings = git_settings("Credit Where It's Due", "r@example.com", Path::new("/k"));
        assert_eq!(
            settings,
            vec![
                ("user.name", "Credit Where It's Due".to_string()),
                ("user.email", "r@example.com".to_string()),
                ("gpg.format", "ssh".to_string()),
                ("user.signingkey", "/k".to_string()),
                ("commit.gpgsign", "true".to_string()),
            ]
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
}
