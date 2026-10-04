use serde::Deserialize;

pub const MANIFEST_FILE: &str = ".tsk-ledger.toml";
pub const SUPPORTED_VERSIONS: &[i64] = &[1];

#[derive(Debug, Deserialize)]
struct Manifest {
    version: i64,
}

pub fn check(text: &str) -> Result<i64, String> {
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

    #[test]
    fn accepts_version_one() {
        assert_eq!(check("version = 1\n").unwrap(), 1);
    }

    #[test]
    fn ignores_unknown_keys() {
        assert_eq!(check("version = 1\nnote = \"x\"\n").unwrap(), 1);
    }

    #[test]
    fn rejects_unsupported_version() {
        let err = check("version = 2\n").unwrap_err();
        assert!(err.contains("unsupported ledger version 2"), "{}", err);
        assert!(err.contains("supports version 1"), "{}", err);
    }

    #[test]
    fn rejects_missing_version() {
        let err = check("other = 1\n").unwrap_err();
        assert!(err.contains("not a valid ledger manifest"), "{}", err);
        assert!(err.contains("version"), "{}", err);
    }

    #[test]
    fn rejects_non_integer_version() {
        assert!(check("version = \"1\"\n").is_err());
    }

    #[test]
    fn rejects_malformed_toml() {
        assert!(check("version = \n").is_err());
    }
}
