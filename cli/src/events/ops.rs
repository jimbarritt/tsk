use std::path::Path;

use crate::ledger::store::LedgerStore;
use crate::ledger::{fetch_reporting, push_reporting};
use crate::thread::clock::utc_now;

use super::batch::BatchEvent;
use super::json::first_value_or_null;
use super::queue::{envelope_line, new_events, NewEnvelope, NewEvents};

pub const ADVANCE_COMMAND: &str = "tsk events advance-watermark";

pub struct Ledger<'a> {
    pub store: &'a dyn LedgerStore,
}

impl Ledger<'_> {
    fn refresh(&self) -> Result<(), String> {
        fetch_reporting(self.store).map(|_| ())
    }

    fn push(&self, message: &str) -> Result<(), String> {
        push_reporting(self.store, message).map(|_| ())
    }
}

pub struct AppendRequest<'a> {
    pub source: &'a str,
    pub event_type: &'a str,
    pub action: &'a str,
    pub repo: &'a str,
    pub payload_file: &'a Path,
}

#[derive(Debug, PartialEq)]
pub enum AdvanceOutcome {
    Advanced(u64),
    AlreadyAt(u64),
}

pub fn read_payload(file: &Path) -> Result<Box<serde_json::value::RawValue>, String> {
    if !file.is_file() {
        return Err(format!("error: no such payload file: {}", file.display()));
    }
    let text = std::fs::read_to_string(file)
        .map_err(|e| format!("error: could not read {}: {}", file.display(), e))?;
    first_value_or_null(&text).map_err(|e| format!("error: {} is not JSON: {}", file.display(), e))
}

pub fn append(ledger: &Ledger, request: &AppendRequest) -> Result<(), String> {
    let payload = read_payload(request.payload_file)?;
    ledger.refresh()?;
    let received_at = utc_now();
    let line = envelope_line(
        &NewEnvelope {
            source: request.source,
            event_type: request.event_type,
            action: request.action,
            repo: request.repo,
            received_at: &received_at,
        },
        &payload,
    );
    ledger.store.events_append(&line)?;
    ledger.push(&format!(
        "External event queue: {} {} ({}) on {}",
        request.source, request.event_type, request.action, request.repo
    ))
}

pub fn batch_lines(events: &[BatchEvent], received_at: &str) -> Result<String, String> {
    events
        .iter()
        .map(|event| {
            let payload = serde_json::value::RawValue::from_string(event.payload.clone())
                .map_err(|e| format!("error: payload is not JSON: {}", e))?;
            Ok(envelope_line(
                &NewEnvelope {
                    source: &event.source,
                    event_type: &event.event_type,
                    action: &event.action,
                    repo: &event.repo,
                    received_at,
                },
                &payload,
            ))
        })
        .collect()
}

pub fn batch_message(count: usize) -> String {
    let noun = if count == 1 { "event" } else { "events" };
    format!("External event queue: append batch of {} {}", count, noun)
}

pub fn append_batch(ledger: &Ledger, events: &[BatchEvent]) -> Result<usize, String> {
    if events.is_empty() {
        return Ok(0);
    }
    ledger.refresh()?;
    let lines = batch_lines(events, &utc_now())?;
    ledger.store.events_append(&lines)?;
    ledger.push(&batch_message(events.len()))?;
    Ok(events.len())
}

pub fn read_new(ledger: &Ledger) -> Result<NewEvents, String> {
    ledger.refresh()?;
    let queue = ledger.store.events_list()?.map(|lines| {
        lines
            .iter()
            .map(|line| format!("{}\n", line))
            .collect::<String>()
    });
    let mark = match queue {
        Some(_) => ledger.store.watermark_get()?,
        None => 0,
    };
    new_events(queue.as_deref(), mark)
}

pub fn parse_count(text: &str) -> Result<u64, String> {
    let invalid = || {
        format!(
            "{}: count must be a non-negative integer, got '{}'",
            ADVANCE_COMMAND, text
        )
    };
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return Err(invalid());
    }
    text.parse::<u64>().map_err(|_| invalid())
}

pub fn check_advance(current: u64, count: u64) -> Result<Option<u64>, String> {
    if count < current {
        return Err(format!(
            "{}: refusing to move the watermark backwards ({} -> {})",
            ADVANCE_COMMAND, current, count
        ));
    }
    Ok((count > current).then_some(count))
}

pub fn advance(ledger: &Ledger, count: u64) -> Result<AdvanceOutcome, String> {
    ledger.refresh()?;
    let current = ledger.store.watermark_get()?;
    if check_advance(current, count)?.is_none() {
        return Ok(AdvanceOutcome::AlreadyAt(current));
    }
    ledger.store.watermark_set(count, &utc_now())?;
    ledger.push(&format!(
        "External event queue: advance watermark {} -> {}",
        current, count
    ))?;
    Ok(AdvanceOutcome::Advanced(count))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_count_accepts_digits_only() {
        assert_eq!(parse_count("0").unwrap(), 0);
        assert_eq!(parse_count("17").unwrap(), 17);
        for bad in ["", "-1", "1.5", "x", " 3"] {
            let err = parse_count(bad).unwrap_err();
            assert_eq!(
                err,
                format!(
                    "tsk events advance-watermark: count must be a non-negative integer, got '{}'",
                    bad
                )
            );
        }
    }

    #[test]
    fn check_advance_refuses_a_lower_count() {
        assert_eq!(
            check_advance(5, 4).unwrap_err(),
            "tsk events advance-watermark: refusing to move the watermark backwards (5 -> 4)"
        );
    }

    #[test]
    fn check_advance_is_a_no_op_at_the_same_count() {
        assert_eq!(check_advance(5, 5).unwrap(), None);
        assert_eq!(check_advance(0, 0).unwrap(), None);
    }

    #[test]
    fn check_advance_moves_forward() {
        assert_eq!(check_advance(5, 9).unwrap(), Some(9));
    }

    #[test]
    fn read_payload_names_a_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("absent.json");
        assert_eq!(
            read_payload(&missing).unwrap_err(),
            format!("error: no such payload file: {}", missing.display())
        );
    }

    #[test]
    fn read_payload_rejects_invalid_json() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("payload.json");
        std::fs::write(&file, "{not json").unwrap();
        assert!(read_payload(&file).unwrap_err().contains("is not JSON"));
    }

    #[test]
    fn batch_lines_write_one_envelope_per_event_with_one_timestamp() {
        let event = |n: u8| BatchEvent {
            source: "github".to_string(),
            event_type: "t".to_string(),
            action: "polled".to_string(),
            repo: "o/r".to_string(),
            payload: format!("{{\"n\":{}}}", n),
        };
        assert_eq!(
            batch_lines(&[event(1), event(2)], "2026-10-04T10:00:00Z").unwrap(),
            "{\"source\":\"github\",\"event_type\":\"t\",\"action\":\"polled\",\"repo\":\"o/r\",\"received_at\":\"2026-10-04T10:00:00Z\",\"payload\":{\"n\":1}}\n\
             {\"source\":\"github\",\"event_type\":\"t\",\"action\":\"polled\",\"repo\":\"o/r\",\"received_at\":\"2026-10-04T10:00:00Z\",\"payload\":{\"n\":2}}\n"
        );
    }

    #[test]
    fn batch_message_states_the_count() {
        assert_eq!(
            batch_message(1),
            "External event queue: append batch of 1 event"
        );
        assert_eq!(
            batch_message(3),
            "External event queue: append batch of 3 events"
        );
    }
}
