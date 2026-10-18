/// The manifest file of the removed files ledger. Its presence on a ledger ref tells a
/// file ledger from a Dolt one (see `fetch::ref_kind`).
pub const MANIFEST_FILE: &str = ".tsk-ledger.toml";
/// The versions a Dolt ledger's `manifest` table may hold. Version 1 keeps its documents
/// in a `documents` table. Version 2 keeps them as files on the `tsk/docs` branch.
/// Version 3 adds the mission tables. Version 4 adds the task, decision and question
/// tables. Version 5 adds `threads.mission_id` and lets `threads.briefing_path` be null.
/// Version 6 adds `decisions.adr_path`. Version 7 moves the constraints and out-of-scope
/// text of a mission into the `mission_constraints` and `mission_out_of_scope` tables.
/// Version 8 replaces `cloud_session_bindings` with `bindings`, which also holds the
/// bindings of local worktrees.
pub const DOLT_SUPPORTED_VERSIONS: &[i64] = &[1, 2, 3, 4, 5, 6, 7, 8];

/// The fields of a ledger manifest that a caller can read back.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestInfo {
    pub version: i64,
    pub repo_id: Option<String>,
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

    #[test]
    fn a_binary_that_knows_only_version_one_stops_at_a_version_two_ledger() {
        // What an older tsk does with a Dolt ledger at version 2.
        let err = check_values(2, None, None, "the manifest table", &[1]).unwrap_err();
        assert!(err.contains("unsupported ledger version 2"), "{}", err);
        assert!(err.contains("supports version 1"), "{}", err);
        assert!(err.contains("Update tsk"), "{}", err);
    }

    #[test]
    fn a_binary_that_knows_versions_one_to_six_stops_at_a_version_seven_ledger() {
        // What the previous release does with a Dolt ledger at version 7.
        let err =
            check_values(7, None, None, "the manifest table", &[1, 2, 3, 4, 5, 6]).unwrap_err();
        assert!(err.contains("unsupported ledger version 7"), "{}", err);
        assert!(err.contains("supports version 1, 2, 3, 4, 5, 6"), "{}", err);
        assert!(err.contains("Update tsk"), "{}", err);
    }

    #[test]
    fn a_binary_that_knows_versions_one_to_seven_stops_at_a_version_eight_ledger() {
        // What the previous release does with a Dolt ledger at version 8.
        let err =
            check_values(8, None, None, "the manifest table", &[1, 2, 3, 4, 5, 6, 7]).unwrap_err();
        assert!(err.contains("unsupported ledger version 8"), "{}", err);
        assert!(
            err.contains("supports version 1, 2, 3, 4, 5, 6, 7"),
            "{}",
            err
        );
        assert!(err.contains("Update tsk"), "{}", err);
    }

    #[test]
    fn the_dolt_list_accepts_versions_one_to_eight_and_rejects_nine() {
        for ok in [1, 2, 3, 4, 5, 6, 7, 8] {
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
            check_values(9, None, None, "the manifest table", DOLT_SUPPORTED_VERSIONS).unwrap_err();
        assert!(err.contains("unsupported ledger version 9"), "{}", err);
        assert!(
            err.contains("supports version 1, 2, 3, 4, 5, 6, 7, 8"),
            "{}",
            err
        );
    }

    #[test]
    fn a_matching_repo_id_passes_and_a_different_one_names_both() {
        let ok = |found, expected| {
            check_values(
                1,
                found,
                expected,
                "the manifest table",
                DOLT_SUPPORTED_VERSIONS,
            )
        };
        assert_eq!(ok(Some("tsk"), Some("tsk")).unwrap(), 1);
        let err = ok(Some("other"), Some("tsk")).unwrap_err();
        assert!(err.contains("repo_id \"other\""), "{}", err);
        assert!(err.contains("\"tsk\""), "{}", err);
    }

    #[test]
    fn a_missing_repo_id_passes_and_is_ignored_without_an_expectation() {
        let ok = |found, expected| {
            check_values(
                1,
                found,
                expected,
                "the manifest table",
                DOLT_SUPPORTED_VERSIONS,
            )
        };
        assert_eq!(ok(None, Some("tsk")).unwrap(), 1);
        assert_eq!(ok(Some("x"), None).unwrap(), 1);
    }
}
