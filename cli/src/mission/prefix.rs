//! The mission prefix of a ledger (ADR 0027): 3 to 4 capital letters, kept as the manifest
//! key `mission_prefix`, that namespace every mission ID in the ledger. With a prefix a new
//! ID reads `M-<prefix>-<NAME>`, and a trailing number marks a part of a mission:
//! `M-<prefix>-<NAME>-nn` is a part of `M-<prefix>-<NAME>`. A ledger with no prefix keeps
//! the free-form IDs it had, and none of these rules apply to it.

use super::model::Mission;
use crate::ledger::store::LedgerStore;

/// `^[A-Z]{3,4}$`
pub fn valid_prefix(prefix: &str) -> bool {
    (3..=4).contains(&prefix.len()) && prefix.chars().all(|c| c.is_ascii_uppercase())
}

fn check_prefix(prefix: &str) -> Result<(), String> {
    if valid_prefix(prefix) {
        Ok(())
    } else {
        Err(format!(
            "error: '{}' is not a mission prefix; a prefix is 3 or 4 capital letters, for example TSK",
            prefix
        ))
    }
}

/// The ID with the prefix added after `M-`, unless it has it already.
pub fn with_prefix(prefix: &str, id: &str) -> String {
    let own = format!("M-{}", prefix);
    match id.strip_prefix("M-") {
        Some(_) if id == own || id.starts_with(&format!("{}-", own)) => id.to_string(),
        Some(rest) => format!("{}-{}", own, rest),
        None => id.to_string(),
    }
}

/// The ID without its trailing number, when it has one: `M-TSK-BOOT` for `M-TSK-BOOT-01`.
fn whole_of(id: &str) -> Option<&str> {
    let (whole, last) = id.rsplit_once('-')?;
    (!last.is_empty() && last.chars().all(|c| c.is_ascii_digit())).then_some(whole)
}

/// Where a mission ID goes: the ID itself, the parent it implies, and whether the prefix was
/// added.
pub struct Placed {
    pub id: String,
    pub parent: Option<String>,
    pub added: bool,
}

/// Place the ID of a new mission, or the new ID of a renamed one. `parent` is the parent the
/// person gave, or the parent the renamed mission has. `renamed` is the code of the mission
/// being renamed, which cannot be its own parent.
///
/// With no prefix the ID and the parent stay as given. With one, the prefix is added when
/// the ID lacks it, and a numbered ID has to be a part of a mission that exists. A new
/// numbered ID with no parent given takes that mission as its parent. A given parent has to
/// be that mission.
pub fn place(
    missions: &[Mission],
    prefix: Option<&str>,
    id: &str,
    parent: Option<&str>,
    renamed: Option<&str>,
) -> Result<Placed, String> {
    let Some(prefix) = prefix else {
        return Ok(Placed {
            id: id.to_string(),
            parent: parent.map(str::to_string),
            added: false,
        });
    };
    let placed = with_prefix(prefix, id);
    let added = placed != id;
    let mut parent = parent.map(str::to_string);
    if let Some(whole) = whole_of(&placed) {
        let own = format!("M-{}", prefix);
        let exists =
            whole != own && Some(whole) != renamed && missions.iter().any(|m| m.id == whole);
        if !exists {
            return Err(format!(
                "error: {} is numbered, so it is a part of {}, and no mission has that ID.\n       \
                 Create {} first, or give the mission a name of its own instead of a number",
                placed, whole, whole
            ));
        }
        match &parent {
            Some(given) if given != whole => {
                return Err(format!(
                    "error: {} is a part of {}, and its parent is {}",
                    placed, whole, given
                ));
            }
            _ => parent = Some(whole.to_string()),
        }
    }
    Ok(Placed {
        id: placed,
        parent,
        added,
    })
}

/// Read or set the prefix: `tsk mission prefix [<PREFIX>]`. Setting changes the IDs of
/// missions made from then on and renames none. It prints the prefix.
pub fn run(store: &dyn LedgerStore, prefix: Option<&str>) -> Result<(), String> {
    match prefix {
        Some(prefix) => {
            check_prefix(prefix)?;
            store.set_mission_prefix(prefix)?;
            println!("{}", prefix);
            Ok(())
        }
        None => match store.mission_prefix()? {
            Some(prefix) => {
                println!("{}", prefix);
                Ok(())
            }
            None => Err(
                "error: this ledger has no mission prefix. Choose 3 or 4 capital letters for it, for example from the name of the repo, and run tsk mission prefix <PREFIX>"
                    .to_string(),
            ),
        },
    }
}

/// The note that a ledger with missions and no prefix prints when a mission is created.
pub fn missing_note() -> &'static str {
    "note: this ledger has no mission prefix, so the ID is used as typed. Choose 3 or 4 capital letters for the repo and run tsk mission prefix <PREFIX>"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mission::model::{self, Category};

    fn missions(ids: &[&str]) -> Vec<Mission> {
        let mut all: Vec<Mission> = Vec::new();
        for id in ids {
            all.push(model::create(&all, id, "A mission", Category::Operational, None).unwrap());
        }
        all
    }

    #[test]
    fn a_prefix_is_three_or_four_capital_letters() {
        for good in ["TSK", "ABCD"] {
            assert!(valid_prefix(good), "{}", good);
        }
        for bad in ["", "TS", "TSKXY", "tsk", "TS1", "T-K", "TSK "] {
            assert!(!valid_prefix(bad), "{}", bad);
        }
    }

    #[test]
    fn the_prefix_is_added_once() {
        assert_eq!(with_prefix("TSK", "M-BOOT"), "M-TSK-BOOT");
        assert_eq!(with_prefix("TSK", "M-BOOT-02"), "M-TSK-BOOT-02");
        assert_eq!(with_prefix("TSK", "M-TSK-BOOT"), "M-TSK-BOOT");
        assert_eq!(with_prefix("TSK", "M-TSK"), "M-TSK");
        // Another letter group is a name, not a prefix.
        assert_eq!(with_prefix("TSK", "M-LAB-01"), "M-TSK-LAB-01");
        assert_eq!(with_prefix("TSK", "M-TSKY"), "M-TSK-TSKY");
    }

    #[test]
    fn with_no_prefix_nothing_changes() {
        let all = missions(&["M-BOOT"]);
        let placed = place(&all, None, "M-BOOT-02", Some("M-X"), None).unwrap();
        assert_eq!(placed.id, "M-BOOT-02");
        assert_eq!(placed.parent.as_deref(), Some("M-X"));
        assert!(!placed.added);
    }

    #[test]
    fn a_new_id_gets_the_prefix_and_a_numbered_one_takes_its_whole_as_parent() {
        let all = missions(&["M-TSK-BOOT"]);
        let placed = place(&all, Some("TSK"), "M-BOOT-01", None, None).unwrap();
        assert_eq!(placed.id, "M-TSK-BOOT-01");
        assert_eq!(placed.parent.as_deref(), Some("M-TSK-BOOT"));
        assert!(placed.added);
        let kept = place(&all, Some("TSK"), "M-TSK-BOOT-01", Some("M-TSK-BOOT"), None).unwrap();
        assert!(!kept.added);
        let plain = place(&all, Some("TSK"), "M-NAMES", None, None).unwrap();
        assert_eq!((plain.id.as_str(), plain.parent), ("M-TSK-NAMES", None));
    }

    #[test]
    fn a_numbered_id_needs_the_mission_it_is_a_part_of() {
        let all = missions(&["M-TSK-BOOT"]);
        let err = place(&all, Some("TSK"), "M-TSK-ADMIN-03", None, None)
            .err()
            .unwrap();
        assert!(err.contains("part of M-TSK-ADMIN"), "{}", err);
        assert!(err.contains("name of its own"), "{}", err);
        let err = place(&all, Some("TSK"), "M-TSK-2026", None, None)
            .err()
            .unwrap();
        assert!(err.contains("part of M-TSK,"), "{}", err);
        let err = place(
            &all,
            Some("TSK"),
            "M-TSK-BOOT-01",
            Some("M-TSK-OTHER"),
            None,
        )
        .err()
        .unwrap();
        assert!(err.contains("its parent is M-TSK-OTHER"), "{}", err);
    }

    #[test]
    fn a_renamed_mission_is_not_its_own_parent() {
        let all = missions(&["M-TSK-BOOT"]);
        let err = place(&all, Some("TSK"), "M-TSK-BOOT-01", None, Some("M-TSK-BOOT"))
            .err()
            .unwrap();
        assert!(err.contains("no mission has that ID"), "{}", err);
    }
}
