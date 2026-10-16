use serde::Deserialize;

pub const MANIFEST_FILE: &str = ".tsk-ledger.toml";
/// The versions a file ledger's `.tsk-ledger.toml` may hold.
pub const SUPPORTED_VERSIONS: &[i64] = &[1];
/// The versions a Dolt ledger's `manifest` table may hold. Version 1 keeps its documents
/// in a `documents` table. Version 2 keeps them as files on the `tsk/docs` branch.
/// Version 3 adds the mission tables. Version 4 adds the task, decision and question
/// tables.
pub const DOLT_SUPPORTED_VERSIONS: &[i64] = &[1, 2, 3, 4];

#[derive(Debug, Deserialize)]
struct Manifest {
    version: i64,
    repo_id: Option<String>,
}

/// The fields of a ledger manifest that a caller can read back.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestInfo {
    pub version: i64,
    pub repo_id: Option<String>,
}

#[allow(dead_code)]
pub fn read(text: &str) -> Result<ManifestInfo, String> {
    let manifest: Manifest = toml::from_str(text).map_err(|e| {
        format!(
            "error: {} is not a valid ledger manifest: {}",
            MANIFEST_FILE,
            e.message()
        )
    })?;
    Ok(ManifestInfo {
        version: manifest.version,
        repo_id: manifest.repo_id,
    })
}

pub fn initial_text(repo_id: Option<&str>) -> String {
    match repo_id {
        Some(id) => format!("version = 1\nrepo_id = \"{}\"\n", id),
        None => "version = 1\n".to_string(),
    }
}

pub fn check(text: &str, expected_repo_id: Option<&str>) -> Result<i64, String> {
    let manifest: Manifest = toml::from_str(text).map_err(|e| {
        format!(
            "error: {} is not a valid ledger manifest: {}",
            MANIFEST_FILE,
            e.message()
        )
    })?;
    check_values(
        manifest.version,
        manifest.repo_id.as_deref(),
        expected_repo_id,
        MANIFEST_FILE,
        SUPPORTED_VERSIONS,
    )
}

/// The version and repo ID checks, for a manifest already read from any store. `source`
/// names where the values came from, for the error text.
pub fn check_values(
    version: i64,
    found_repo_id: Option<&str>,
    expected_repo_id: Option<&str>,
    source: &str,
    supported: &[i64],
) -> Result<i64, String> {
    if !supported.contains(&version) {
        return Err(format!(
            "error: unsupported ledger version {} in {}; this tsk supports version {}. Update tsk",
            version,
            source,
            supported_list(supported)
        ));
    }
    if let (Some(expected), Some(found)) = (expected_repo_id, found_repo_id) {
        if expected != found {
            return Err(format!(
                "error: {} names repo_id \"{}\" but the nexus entry is \"{}\"; this ledger belongs to another repo",
                source, found, expected
            ));
        }
    }
    Ok(version)
}

fn supported_list(supported: &[i64]) -> String {
    supported
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check_v(text: &str) -> Result<i64, String> {
        check(text, None)
    }

    #[test]
    fn accepts_version_one() {
        assert_eq!(check_v("version = 1\n").unwrap(), 1);
    }

    #[test]
    fn ignores_unknown_keys() {
        assert_eq!(check_v("version = 1\nnote = \"x\"\n").unwrap(), 1);
    }

    #[test]
    fn rejects_unsupported_version() {
        let err = check_v("version = 2\n").unwrap_err();
        assert!(err.contains("unsupported ledger version 2"), "{}", err);
        assert!(err.contains("supports version 1"), "{}", err);
    }

    #[test]
    fn a_binary_that_knows_only_version_one_stops_at_a_version_two_ledger() {
        // What an older tsk does with a Dolt ledger at version 2.
        let err = check_values(2, None, None, "the manifest table", &[1]).unwrap_err();
        assert!(err.contains("unsupported ledger version 2"), "{}", err);
        assert!(err.contains("supports version 1"), "{}", err);
        assert!(err.contains("Update tsk"), "{}", err);
    }

    #[test]
    fn a_binary_that_knows_versions_one_to_three_stops_at_a_version_four_ledger() {
        // What the previous release does with a Dolt ledger at version 4.
        let err = check_values(4, None, None, "the manifest table", &[1, 2, 3]).unwrap_err();
        assert!(err.contains("unsupported ledger version 4"), "{}", err);
        assert!(err.contains("supports version 1, 2, 3"), "{}", err);
        assert!(err.contains("Update tsk"), "{}", err);
    }

    #[test]
    fn the_dolt_list_accepts_versions_one_to_four_and_rejects_five() {
        for ok in [1, 2, 3, 4] {
            assert_eq!(
                check_values(
                    ok,
                    None,
                    None,
                    "the manifest table",
                    DOLT_SUPPORTED_VERSIONS
                )
                .unwrap(),
                ok
            );
        }
        let err =
            check_values(5, None, None, "the manifest table", DOLT_SUPPORTED_VERSIONS).unwrap_err();
        assert!(err.contains("unsupported ledger version 5"), "{}", err);
        assert!(err.contains("supports version 1, 2, 3, 4"), "{}", err);
    }

    #[test]
    fn rejects_missing_version() {
        let err = check_v("other = 1\n").unwrap_err();
        assert!(err.contains("not a valid ledger manifest"), "{}", err);
        assert!(err.contains("version"), "{}", err);
    }

    #[test]
    fn rejects_non_integer_version() {
        assert!(check_v("version = \"1\"\n").is_err());
    }

    #[test]
    fn rejects_malformed_toml() {
        assert!(check_v("version = \n").is_err());
    }

    #[test]
    fn accepts_a_matching_repo_id() {
        assert_eq!(
            check("version = 1\nrepo_id = \"tsk\"\n", Some("tsk")).unwrap(),
            1
        );
    }

    #[test]
    fn rejects_a_different_repo_id() {
        let err = check("version = 1\nrepo_id = \"other\"\n", Some("tsk")).unwrap_err();
        assert!(err.contains("repo_id \"other\""), "{}", err);
        assert!(err.contains("\"tsk\""), "{}", err);
    }

    #[test]
    fn accepts_a_missing_repo_id_and_ignores_it_without_an_expectation() {
        assert_eq!(check("version = 1\n", Some("tsk")).unwrap(), 1);
        assert_eq!(check("version = 1\nrepo_id = \"x\"\n", None).unwrap(), 1);
    }

    #[test]
    fn initial_text_carries_the_repo_id_for_a_nexus_ledger() {
        assert_eq!(initial_text(None), "version = 1\n");
        assert_eq!(
            initial_text(Some("work-api")),
            "version = 1\nrepo_id = \"work-api\"\n"
        );
        assert_eq!(
            check(&initial_text(Some("work-api")), Some("work-api")).unwrap(),
            1
        );
    }
}
