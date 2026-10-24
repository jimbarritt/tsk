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
    #[command(
        name = "ledger-refs",
        about = "Print which refs hold this repo's ledger and where that was decided, or set them in .tsk/config.toml",
        long_about = "Print which refs hold this repo's ledger and where that was decided, or set them in .tsk/config.toml.\n\nbranches: refs/heads/tsk/ledger and refs/heads/tsk/docs. GitHub lists them as branches. A repo used from Claude Code cloud sessions needs them, because the cloud proxy refuses to push a ref outside refs/heads/*.\ncustom: refs/tsk/ledger and refs/tsk/docs. GitHub does not list them. The default for a new ledger outside a cloud session.\n\nThe mode is read from the nexus entry of the repo (ledger_refs), then .tsk/config.toml, then discovered on origin. tsk does not move a ledger that origin holds from one mode to the other."
    )]
    #[command(
        name = "docs-hook",
        about = "Print which documents hook script applies and where it came from, or none",
        long_about = "Print which documents hook script applies and where it came from, or none.\n\nThe first of these that exists wins: the file TSK_DOCS_HOOK names, <state>/repos/<clone-id>/hooks/pre-docs-commit, <config>/tsk/hooks/pre-docs-commit in the user config directory, and .tsk/hooks/pre-docs-commit in the repo. A script that exists and is not executable is an error."
    )]
    DocsHook,
    LedgerRefs {
        #[arg(
            value_parser = ["branches", "custom"],
            help = "The mode to write to .tsk/config.toml; omit it to print the mode in force"
        )]
        mode: Option<String>,
    },
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

/// Environment variable that once chose the ledger backend. A ledger is a Dolt database
/// now. The variable is read only to refuse the value `git`.
pub const BACKEND_ENV: &str = "TSK_LEDGER_BACKEND";

/// What a user who still selects the removed files backend is told.
pub const FILES_BACKEND_REMOVED: &str = "error: the files ledger backend was removed in 0.16.0; use tsk 0.15.1 to read an old file ledger";

/// The `[ledger] backend` value in the config file. `None` when the file, the table, the
/// key or its string value is absent.
fn read_ledger_backend(file: &Path) -> Option<String> {
    let table = read_table(file).ok()?;
    match table.get("ledger")?.as_table()?.get("backend")? {
        toml::Value::String(value) => Some(value.clone()),
        _ => None,
    }
}

/// The `[ledger] server` value in the config file: `on` or `off`. `None` when the file,
/// the table, the key or its string value is absent.
pub fn read_ledger_server(file: &Path) -> Option<String> {
    let table = read_table(file).ok()?;
    match table.get("ledger")?.as_table()?.get("server")? {
        toml::Value::String(value) => Some(value.clone()),
        _ => None,
    }
}

/// Refuse the removed files backend. The environment value wins over the config file,
/// and an empty value counts as unset. Any value other than `git` is ignored.
pub fn refuse_files_backend(env: Option<String>, file: Option<&Path>) -> Result<(), String> {
    let chosen = env
        .filter(|v| !v.trim().is_empty())
        .or_else(|| file.and_then(read_ledger_backend));
    match chosen {
        Some(value) if value.trim() == "git" => Err(FILES_BACKEND_REMOVED.to_string()),
        _ => Ok(()),
    }
}

pub fn refuse_files_backend_from_env() -> Result<(), String> {
    refuse_files_backend(
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
    if let ConfigCommands::LedgerRefs { mode } = action {
        return match mode
            .as_deref()
            .and_then(crate::ledger::refs::LedgerRefs::parse)
        {
            Some(mode) => crate::ledger::refs_cmd::set(mode),
            None => crate::ledger::refs_cmd::show(),
        };
    }
    if let ConfigCommands::DocsHook = action {
        return crate::ledger::doc_hook::show();
    }
    let file = required_config_file()?;
    match action {
        ConfigCommands::DocsHook => unreachable!("handled above"),
        ConfigCommands::LedgerRefs { .. } => unreachable!("handled above"),
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
    fn the_files_backend_is_refused_from_the_environment_or_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("config.toml");
        assert!(refuse_files_backend(None, Some(&file)).is_ok());
        assert_eq!(
            refuse_files_backend(Some("git".into()), None).unwrap_err(),
            FILES_BACKEND_REMOVED
        );
        std::fs::write(&file, "[ledger]\nbackend = \"git\"\n").unwrap();
        assert_eq!(
            refuse_files_backend(None, Some(&file)).unwrap_err(),
            FILES_BACKEND_REMOVED
        );
        // The environment wins over the file, and an empty value is unset.
        assert!(refuse_files_backend(Some("dolt".into()), Some(&file)).is_ok());
        assert!(refuse_files_backend(Some("".into()), Some(&file)).is_err());
    }

    #[test]
    fn any_other_backend_value_is_ignored() {
        assert!(refuse_files_backend(Some("sqlite".into()), None).is_ok());
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("config.toml");
        std::fs::write(&file, "[ledger]\nbackend = \"mongo\"\n").unwrap();
        assert!(refuse_files_backend(None, Some(&file)).is_ok());
        std::fs::write(&file, "[ledger]\nbackend = 3\n").unwrap();
        assert!(refuse_files_backend(None, Some(&file)).is_ok());
    }

    #[test]
    fn attaching_a_nexus_keeps_the_ledger_table() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("config.toml");
        std::fs::write(&file, "[ledger]\nbackend = \"dolt\"\n").unwrap();
        attach_nexus(&file, "https://a.test/n").unwrap();
        assert_eq!(read_ledger_backend(&file).as_deref(), Some("dolt"));
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
