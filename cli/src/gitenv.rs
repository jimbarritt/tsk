//! The git transport setting for every git process tsk starts, and for the git
//! processes that `dolt` starts on its behalf.
//!
//! Protocol version 2 makes a fetch with nothing new send two HTTP requests, and a
//! fetch that moves refs send more. Version 0 sends fewer, and tsk's remotes gain
//! nothing from version 2. The setting goes in through `GIT_CONFIG_COUNT`, so it
//! reaches the git processes that `dolt` starts, and the user's own git configuration
//! stays as it is.

use std::process::Command;

/// Set to `default` to leave the protocol version to git, for a server that rejects 0.
pub const PROTOCOL_VAR: &str = "TSK_GIT_PROTOCOL";

const COUNT: &str = "GIT_CONFIG_COUNT";
const KEY: &str = "protocol.version";
const VALUE: &str = "0";

/// The variables to add to a child's environment, given a lookup of the current one.
///
/// - `TSK_GIT_PROTOCOL=default` adds nothing.
/// - Without `GIT_CONFIG_COUNT`, the result sets one entry.
/// - With `GIT_CONFIG_COUNT=n`, the result adds the entry at index `n` and raises the
///   count to `n + 1`, so the user's entries stay.
/// - When the user's entries already set `protocol.version`, or `GIT_CONFIG_COUNT` is
///   not a number, the result adds nothing. The user's choice stands.
pub fn config_env(get: &dyn Fn(&str) -> Option<String>) -> Vec<(String, String)> {
    if get(PROTOCOL_VAR).is_some_and(|v| v.trim().eq_ignore_ascii_case("default")) {
        return Vec::new();
    }
    let count = match get(COUNT) {
        None => 0,
        Some(text) => match text.trim().parse::<usize>() {
            Ok(count) => count,
            Err(_) => return Vec::new(),
        },
    };
    let user_sets_it = (0..count).any(|i| {
        get(&format!("GIT_CONFIG_KEY_{}", i)).is_some_and(|key| key.eq_ignore_ascii_case(KEY))
    });
    if user_sets_it {
        return Vec::new();
    }
    vec![
        (COUNT.to_string(), (count + 1).to_string()),
        (format!("GIT_CONFIG_KEY_{}", count), KEY.to_string()),
        (format!("GIT_CONFIG_VALUE_{}", count), VALUE.to_string()),
    ]
}

/// Sets the protocol version on `command`, which starts `git` or `dolt`.
pub fn apply(command: &mut Command) -> &mut Command {
    for (name, value) in config_env(&|name| std::env::var(name).ok()) {
        command.env(name, value);
    }
    command
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn env_of(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    fn added(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        let env = env_of(pairs);
        config_env(&|name| env.get(name).cloned())
    }

    fn pair(name: &str, value: &str) -> (String, String) {
        (name.to_string(), value.to_string())
    }

    #[test]
    fn an_empty_environment_gets_one_entry_that_sets_protocol_version_0() {
        assert_eq!(
            added(&[]),
            vec![
                pair("GIT_CONFIG_COUNT", "1"),
                pair("GIT_CONFIG_KEY_0", "protocol.version"),
                pair("GIT_CONFIG_VALUE_0", "0"),
            ]
        );
    }

    #[test]
    fn existing_entries_stay_and_the_new_entry_goes_after_them() {
        assert_eq!(
            added(&[
                ("GIT_CONFIG_COUNT", "2"),
                ("GIT_CONFIG_KEY_0", "http.sslVerify"),
                ("GIT_CONFIG_VALUE_0", "true"),
                ("GIT_CONFIG_KEY_1", "core.compression"),
                ("GIT_CONFIG_VALUE_1", "1"),
            ]),
            vec![
                pair("GIT_CONFIG_COUNT", "3"),
                pair("GIT_CONFIG_KEY_2", "protocol.version"),
                pair("GIT_CONFIG_VALUE_2", "0"),
            ]
        );
    }

    #[test]
    fn a_count_of_zero_is_an_empty_list() {
        assert_eq!(
            added(&[("GIT_CONFIG_COUNT", "0")])[0],
            pair("GIT_CONFIG_COUNT", "1")
        );
    }

    #[test]
    fn the_opt_out_adds_nothing() {
        assert!(added(&[("TSK_GIT_PROTOCOL", "default")]).is_empty());
        assert!(added(&[("TSK_GIT_PROTOCOL", " Default ")]).is_empty());
        assert!(!added(&[("TSK_GIT_PROTOCOL", "0")]).is_empty());
        assert!(!added(&[("TSK_GIT_PROTOCOL", "")]).is_empty());
    }

    #[test]
    fn a_protocol_version_the_user_set_stands() {
        assert!(added(&[
            ("GIT_CONFIG_COUNT", "1"),
            ("GIT_CONFIG_KEY_0", "protocol.version"),
            ("GIT_CONFIG_VALUE_0", "2"),
        ])
        .is_empty());
    }

    #[test]
    fn a_count_that_is_not_a_number_is_left_alone() {
        assert!(added(&[("GIT_CONFIG_COUNT", "many")]).is_empty());
    }

    #[test]
    fn apply_puts_the_entry_in_the_child_environment() {
        let mut command = Command::new("git");
        apply(&mut command);
        let envs: HashMap<_, _> = command
            .get_envs()
            .map(|(k, v)| {
                (
                    k.to_string_lossy().into_owned(),
                    v.map(|v| v.to_string_lossy().into_owned()),
                )
            })
            .collect();
        // The test process may carry its own GIT_CONFIG_COUNT, so only the value of
        // the new entry is fixed. The entry sits at the index of the old count.
        let count: usize = envs["GIT_CONFIG_COUNT"].clone().unwrap().parse().unwrap();
        assert_eq!(
            envs[&format!("GIT_CONFIG_KEY_{}", count - 1)].as_deref(),
            Some("protocol.version")
        );
        assert_eq!(
            envs[&format!("GIT_CONFIG_VALUE_{}", count - 1)].as_deref(),
            Some("0")
        );
    }
}
