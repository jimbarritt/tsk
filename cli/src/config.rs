use std::ffi::OsString;
use std::path::{Path, PathBuf};

use clap::Subcommand;

pub const CONFIG_DIR_NAME: &str = "tsk";
pub const CONFIG_FILE_NAME: &str = "config.toml";

#[derive(Subcommand)]
pub enum ConfigCommands {
    #[command(
        about = "Record the nexus repo URL in the user config, replacing any URL already recorded"
    )]
    AttachNexus {
        #[arg(help = "Clone URL of the nexus repo")]
        url: String,
    },
    #[command(about = "Print the user config file path and the nexus URL it records")]
    Show,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttachOutcome {
    Attached,
    Unchanged,
    Replaced { previous: String },
}

pub fn config_file(xdg_config_home: Option<OsString>, home: Option<OsString>) -> Option<PathBuf> {
    let base = match xdg_config_home.filter(|v| !v.is_empty()) {
        Some(xdg) => PathBuf::from(xdg),
        None => PathBuf::from(home.filter(|v| !v.is_empty())?).join(".config"),
    };
    Some(base.join(CONFIG_DIR_NAME).join(CONFIG_FILE_NAME))
}

pub fn config_file_from_env() -> Option<PathBuf> {
    config_file(
        std::env::var_os("XDG_CONFIG_HOME"),
        std::env::var_os("HOME"),
    )
}

fn read_table(file: &Path) -> Result<toml::Table, String> {
    match std::fs::read_to_string(file) {
        Ok(text) => text.parse::<toml::Table>().map_err(|e| {
            format!(
                "error: {} is not valid TOML: {}",
                file.display(),
                e.message()
            )
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(toml::Table::new()),
        Err(e) => Err(format!("error: could not read {}: {}", file.display(), e)),
    }
}

pub fn read_nexus_url(file: &Path) -> Result<Option<String>, String> {
    let table = read_table(file)?;
    match table.get("nexus") {
        None => Ok(None),
        Some(toml::Value::Table(nexus)) => match nexus.get("url") {
            None => Ok(None),
            Some(toml::Value::String(url)) if !url.trim().is_empty() => {
                Ok(Some(url.trim().to_string()))
            }
            Some(_) => Err(format!(
                "error: nexus.url in {} must be a non-empty string",
                file.display()
            )),
        },
        Some(_) => Err(format!(
            "error: nexus in {} must be a table with a url key",
            file.display()
        )),
    }
}

/// Environment variable that overrides `[ledger] backend` in the user config.
pub const BACKEND_ENV: &str = "TSK_LEDGER_BACKEND";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LedgerBackend {
    Dolt,
    Git,
}

fn parse_backend(value: &str, source: &str) -> Result<LedgerBackend, String> {
    match value.trim() {
        "dolt" => Ok(LedgerBackend::Dolt),
        "git" => Ok(LedgerBackend::Git),
        other => Err(format!(
            "error: unknown ledger backend '{}' in {}; expected 'dolt' or 'git'",
            other, source
        )),
    }
}

/// The `[ledger] backend` value in the config file, unvalidated. `None` when the file,
/// the table or the key is absent.
pub fn read_ledger_backend(file: &Path) -> Result<Option<String>, String> {
    let table = read_table(file)?;
    match table.get("ledger") {
        None => Ok(None),
        Some(toml::Value::Table(ledger)) => match ledger.get("backend") {
            None => Ok(None),
            Some(toml::Value::String(value)) => Ok(Some(value.clone())),
            Some(_) => Err(format!(
                "error: ledger.backend in {} must be a string",
                file.display()
            )),
        },
        Some(_) => Err(format!(
            "error: ledger in {} must be a table with a backend key",
            file.display()
        )),
    }
}

/// Choose the ledger backend. The environment value wins over the config file, and an
/// empty value counts as unset. The default is Dolt.
pub fn ledger_backend(env: Option<String>, file: Option<&Path>) -> Result<LedgerBackend, String> {
    if let Some(value) = env.filter(|v| !v.trim().is_empty()) {
        return parse_backend(&value, &format!("${}", BACKEND_ENV));
    }
    if let Some(file) = file {
        if let Some(value) = read_ledger_backend(file)? {
            return parse_backend(&value, &file.display().to_string());
        }
    }
    Ok(LedgerBackend::Dolt)
}

pub fn ledger_backend_from_env() -> Result<LedgerBackend, String> {
    ledger_backend(
        std::env::var(BACKEND_ENV).ok(),
        config_file_from_env().as_deref(),
    )
}

pub fn attach_nexus(file: &Path, url: &str) -> Result<AttachOutcome, String> {
    let url = url.trim();
    if url.is_empty() {
        return Err("error: the nexus URL is empty".to_string());
    }
    let previous = read_nexus_url(file)?;
    if previous.as_deref() == Some(url) {
        return Ok(AttachOutcome::Unchanged);
    }
    let mut table = read_table(file)?;
    let mut nexus = toml::Table::new();
    nexus.insert("url".to_string(), toml::Value::String(url.to_string()));
    table.insert("nexus".to_string(), toml::Value::Table(nexus));
    let text = toml::to_string(&table)
        .map_err(|e| format!("error: could not serialise the config: {}", e))?;
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("error: could not create {}: {}", parent.display(), e))?;
    }
    std::fs::write(file, text)
        .map_err(|e| format!("error: could not write {}: {}", file.display(), e))?;
    Ok(match previous {
        Some(previous) => AttachOutcome::Replaced { previous },
        None => AttachOutcome::Attached,
    })
}

pub fn required_config_file() -> Result<PathBuf, String> {
    config_file_from_env().ok_or_else(|| {
        "error: neither XDG_CONFIG_HOME nor HOME is set; cannot locate the user config".to_string()
    })
}

pub fn attach_and_report(file: &Path, url: &str) -> Result<(), String> {
    let url = url.trim();
    match attach_nexus(file, url)? {
        AttachOutcome::Attached => println!("attached nexus {}", url),
        AttachOutcome::Unchanged => println!("nexus already attached: {}", url),
        AttachOutcome::Replaced { previous } => {
            println!("replaced nexus {} with {}", previous, url)
        }
    }
    Ok(())
}

pub fn run(action: ConfigCommands) -> Result<(), String> {
    let file = required_config_file()?;
    match action {
        ConfigCommands::AttachNexus { url } => attach_and_report(&file, &url),
        ConfigCommands::Show => {
            println!("config: {}", file.display());
            match read_nexus_url(&file)? {
                Some(url) => println!("nexus: {}", url),
                None => println!("nexus: none attached"),
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_file_prefers_xdg_config_home() {
        assert_eq!(
            config_file(Some("/x/cfg".into()), Some("/home/u".into())),
            Some(PathBuf::from("/x/cfg/tsk/config.toml"))
        );
    }

    #[test]
    fn config_file_treats_empty_xdg_as_unset() {
        assert_eq!(
            config_file(Some("".into()), Some("/home/u".into())),
            Some(PathBuf::from("/home/u/.config/tsk/config.toml"))
        );
    }

    #[test]
    fn config_file_is_none_without_xdg_or_home() {
        assert_eq!(config_file(None, None), None);
    }

    #[test]
    fn read_nexus_url_is_none_for_a_missing_file_or_key() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("tsk").join("config.toml");
        assert_eq!(read_nexus_url(&file).unwrap(), None);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, "other = 1\n").unwrap();
        assert_eq!(read_nexus_url(&file).unwrap(), None);
    }

    #[test]
    fn backend_defaults_to_dolt_and_the_environment_overrides_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("config.toml");
        assert_eq!(
            ledger_backend(None, Some(&file)).unwrap(),
            LedgerBackend::Dolt
        );
        std::fs::write(&file, "[ledger]\nbackend = \"git\"\n").unwrap();
        assert_eq!(
            ledger_backend(None, Some(&file)).unwrap(),
            LedgerBackend::Git
        );
        assert_eq!(
            ledger_backend(Some("dolt".into()), Some(&file)).unwrap(),
            LedgerBackend::Dolt
        );
        assert_eq!(
            ledger_backend(Some("".into()), Some(&file)).unwrap(),
            LedgerBackend::Git
        );
    }

    #[test]
    fn an_unknown_backend_is_named_in_the_error() {
        let err = ledger_backend(Some("sqlite".into()), None).unwrap_err();
        assert!(err.contains("'sqlite'"), "{}", err);
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("config.toml");
        std::fs::write(&file, "[ledger]\nbackend = \"mongo\"\n").unwrap();
        let err = ledger_backend(None, Some(&file)).unwrap_err();
        assert!(err.contains("'mongo'"), "{}", err);
    }

    #[test]
    fn attaching_a_nexus_keeps_the_ledger_backend() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("config.toml");
        std::fs::write(&file, "[ledger]\nbackend = \"git\"\n").unwrap();
        attach_nexus(&file, "https://a.test/n").unwrap();
        assert_eq!(read_ledger_backend(&file).unwrap().as_deref(), Some("git"));
    }

    #[test]
    fn attach_creates_the_file_and_reads_back() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("tsk").join("config.toml");
        assert_eq!(
            attach_nexus(&file, "https://example.test/o/nexus").unwrap(),
            AttachOutcome::Attached
        );
        assert_eq!(
            read_nexus_url(&file).unwrap().as_deref(),
            Some("https://example.test/o/nexus")
        );
    }

    #[test]
    fn attach_is_idempotent_and_reports_a_replacement() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("config.toml");
        attach_nexus(&file, "https://a.test/n").unwrap();
        assert_eq!(
            attach_nexus(&file, "https://a.test/n").unwrap(),
            AttachOutcome::Unchanged
        );
        assert_eq!(
            attach_nexus(&file, "https://b.test/n").unwrap(),
            AttachOutcome::Replaced {
                previous: "https://a.test/n".to_string()
            }
        );
        assert_eq!(
            read_nexus_url(&file).unwrap().as_deref(),
            Some("https://b.test/n")
        );
    }

    #[test]
    fn attach_keeps_other_keys() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("config.toml");
        std::fs::write(&file, "colour = \"blue\"\n").unwrap();
        attach_nexus(&file, "https://a.test/n").unwrap();
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(text.contains("colour = \"blue\""), "{}", text);
    }

    #[test]
    fn attach_rejects_an_empty_url() {
        let dir = tempfile::tempdir().unwrap();
        assert!(attach_nexus(&dir.path().join("c.toml"), "  ").is_err());
    }

    #[test]
    fn read_nexus_url_rejects_malformed_toml_and_wrong_types() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("config.toml");
        std::fs::write(&file, "nexus = \n").unwrap();
        assert!(read_nexus_url(&file).is_err());
        std::fs::write(&file, "nexus = \"x\"\n").unwrap();
        assert!(read_nexus_url(&file).is_err());
        std::fs::write(&file, "[nexus]\nurl = 3\n").unwrap();
        assert!(read_nexus_url(&file).is_err());
    }
}
