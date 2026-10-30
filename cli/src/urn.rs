//! The `urn:tsk:` names of the domain types (`docs/domain/urn-scheme.md`). The session and
//! instance forms are in `sessions` and `thread::session`; the actor form in `actor`. This
//! module holds the forms that carry the ledger: ledger, thread and mission.

/// `urn:tsk:ledger:<ledger-id>`.
pub fn ledger_urn(ledger_id: &str) -> String {
    format!("urn:tsk:ledger:{}", ledger_id)
}

/// `urn:tsk:thread:<ledger-id>:<thread-id>`.
pub fn thread_urn(ledger_id: &str, thread_id: &str) -> String {
    format!("urn:tsk:thread:{}:{}", ledger_id, thread_id)
}

/// `urn:tsk:mission:<ledger-id>:<mission-internal-id>`. The internal ID, never the code.
pub fn mission_urn(ledger_id: &str, mission_internal_id: &str) -> String {
    format!("urn:tsk:mission:{}:{}", ledger_id, mission_internal_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_forms_carry_the_ledger_and_the_record_id() {
        assert_eq!(ledger_urn("L"), "urn:tsk:ledger:L");
        assert_eq!(thread_urn("L", "01d143d5"), "urn:tsk:thread:L:01d143d5");
        assert_eq!(mission_urn("L", "uuid-1"), "urn:tsk:mission:L:uuid-1");
    }
}
