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

/// `urn:tsk:<kind>:<ledger-id>:<uid>` for a task, a decision or a question. `None` for a
/// record that has no uid (a ledger below layout 13).
pub fn record_urn(kind: &str, ledger_id: &str, uid: &str) -> Option<String> {
    (!uid.is_empty()).then(|| format!("urn:tsk:{}:{}:{}", kind, ledger_id, uid))
}

/// `urn:tsk:document:<ledger-id>:<id>`. The id is the 8 hexadecimal characters in the file name
/// of the document, which a rename of the file keeps.
pub fn document_urn(ledger_id: &str, doc_id: &str) -> String {
    format!("urn:tsk:document:{}:{}", ledger_id, doc_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_forms_carry_the_ledger_and_the_record_id() {
        assert_eq!(ledger_urn("L"), "urn:tsk:ledger:L");
        assert_eq!(thread_urn("L", "01d143d5"), "urn:tsk:thread:L:01d143d5");
        assert_eq!(mission_urn("L", "uuid-1"), "urn:tsk:mission:L:uuid-1");
        assert_eq!(
            record_urn("task", "L", "u1").as_deref(),
            Some("urn:tsk:task:L:u1")
        );
        assert_eq!(record_urn("task", "L", ""), None);
        assert_eq!(document_urn("L", "3f2a9c1e"), "urn:tsk:document:L:3f2a9c1e");
    }
}
