use clap::Args;

use crate::self_update::Version;

include!(concat!(env!("OUT_DIR"), "/embedded_releases.rs"));

#[derive(Args, Debug)]
pub struct ReleaseNotesArgs {
    #[arg(
        value_name = "VERSION",
        conflicts_with = "since",
        help = "Print the notes of this version, such as 0.80.0. The default is the installed version"
    )]
    pub version: Option<String>,
    #[arg(
        long,
        value_name = "VERSION",
        help = "Print the notes of every release after this version, newest first, each under a heading"
    )]
    pub since: Option<String>,
}

pub fn run(args: ReleaseNotesArgs) -> Result<(), String> {
    let installed = env!("CARGO_PKG_VERSION");
    let text = render(
        EMBEDDED,
        installed,
        args.version.as_deref(),
        args.since.as_deref(),
    )?;
    print!("{}", text);
    Ok(())
}

fn parse(text: &str) -> Result<Version, String> {
    let bare = text.strip_prefix('v').unwrap_or(text);
    Version::parse(bare).ok_or_else(|| format!("error: {:?} is not a version such as 0.80.0", text))
}

fn find<'a>(releases: &'a [(&'a str, &'a str)], version: Version) -> Option<&'a str> {
    releases
        .iter()
        .find(|(v, _)| Version::parse(v) == Some(version))
        .map(|(_, notes)| *notes)
}

fn known(releases: &[(&str, &str)]) -> String {
    let mut versions: Vec<Version> = releases
        .iter()
        .filter_map(|(v, _)| Version::parse(v))
        .collect();
    versions.sort();
    match (versions.first(), versions.last()) {
        (Some(first), Some(last)) => format!("The notes cover {} to {}.", first, last),
        _ => "This build has no release notes.".to_string(),
    }
}

pub fn render(
    releases: &[(&str, &str)],
    installed: &str,
    version: Option<&str>,
    since: Option<&str>,
) -> Result<String, String> {
    if let Some(since) = since {
        let since = parse(since)?;
        let mut newer: Vec<(Version, &str)> = releases
            .iter()
            .filter_map(|(v, notes)| Version::parse(v).map(|v| (v, *notes)))
            .filter(|(v, _)| *v > since)
            .collect();
        newer.sort_by(|a, b| b.0.cmp(&a.0));
        let mut out = String::new();
        for (version, notes) in newer {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(&format!("# {}\n\n{}", version, notes));
            if !notes.ends_with('\n') {
                out.push('\n');
            }
        }
        return Ok(out);
    }
    let wanted = parse(version.unwrap_or(installed))?;
    match find(releases, wanted) {
        Some(notes) => Ok(notes.to_string()),
        None => Err(format!(
            "error: no release notes for version {}. {}",
            wanted,
            known(releases)
        )),
    }
}

pub fn session_line(old: Version) -> String {
    if EMBEDDED.is_empty() {
        return format!("tsk was updated from {}.", old);
    }
    format!(
        "tsk was updated from {}. To read what changed, run `tsk release-notes --since {}`.",
        old, old
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const FAKE: &[(&str, &str)] = &[
        ("0.9.0", "nine\n"),
        ("0.10.0", "ten\n"),
        ("0.11.0", "eleven"),
    ];

    #[test]
    fn a_named_version_prints_its_notes() {
        assert_eq!(
            render(FAKE, "0.11.0", Some("0.10.0"), None).unwrap(),
            "ten\n"
        );
        assert_eq!(
            render(FAKE, "0.11.0", Some("v0.9.0"), None).unwrap(),
            "nine\n"
        );
    }

    #[test]
    fn no_argument_prints_the_installed_version() {
        assert_eq!(render(FAKE, "0.10.0", None, None).unwrap(), "ten\n");
    }

    #[test]
    fn since_prints_newer_releases_newest_first_under_headings() {
        let out = render(FAKE, "0.11.0", None, Some("0.9.0")).unwrap();
        assert_eq!(out, "# 0.11.0\n\neleven\n\n# 0.10.0\n\nten\n");
    }

    #[test]
    fn since_the_latest_prints_nothing() {
        assert_eq!(render(FAKE, "0.11.0", None, Some("0.11.0")).unwrap(), "");
    }

    #[test]
    fn an_unknown_version_is_an_error_that_invents_nothing() {
        let err = render(FAKE, "0.11.0", Some("0.12.0"), None).unwrap_err();
        assert!(err.starts_with("error: no release notes for version 0.12.0."));
        assert!(err.contains("0.9.0 to 0.11.0"));
    }

    #[test]
    fn a_malformed_version_is_an_error() {
        assert!(render(FAKE, "0.11.0", Some("abc"), None).is_err());
        assert!(render(FAKE, "0.11.0", None, Some("1.x")).is_err());
    }

    #[test]
    fn the_session_line_names_the_command() {
        assert!(session_line(Version(0, 79, 0)).contains("`tsk release-notes --since 0.79.0`"));
    }

    #[test]
    fn the_binary_embeds_the_notes_of_its_own_version() {
        let installed = env!("CARGO_PKG_VERSION");
        if EMBEDDED.is_empty() {
            return;
        }
        assert!(EMBEDDED.iter().any(|(v, _)| *v == installed));
    }
}
