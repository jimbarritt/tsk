use serde::Deserialize;

pub const MANIFEST_FILE: &str = ".tsk-ledger.toml";
pub const SUPPORTED_VERSIONS: &[i64] = &[1];

#[derive(Debug, Deserialize)]
struct Manifest {
    version: i64,
    repo_id: Option<String>,
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
    if !SUPPORTED_VERSIONS.contains(&manifest.version) {
        return Err(format!(
            "error: unsupported ledger version {} in {}; this tsk supports version {}",
            manifest.version,
            MANIFEST_FILE,
            supported_list()
        ));
    }
    if let (Some(expected), Some(found)) = (expected_repo_id, manifest.repo_id.as_deref()) {
        if expected != found {
            return Err(format!(
                "error: {} names repo_id \"{}\" but the nexus entry is \"{}\"; this ledger belongs to another repo",
                MANIFEST_FILE, found, expected
            ));
        }
    }
    Ok(manifest.version)
}

fn supported_list() -> String {
    SUPPORTED_VERSIONS
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
