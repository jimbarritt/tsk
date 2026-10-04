use std::io::Write;
use std::path::{Path, PathBuf};

use crate::ledger::location::Repo;
use crate::ledger::{fetch_reporting, push_reporting};
use crate::thread::clock::utc_now;

use super::batch::BatchEvent;
use super::json::first_value_or_null;
use super::queue::{
    envelope_line, new_events, parse_watermark, queue_path, render_watermark, watermark_path,
    NewEnvelope, NewEvents,
};

pub const ADVANCE_COMMAND: &str = "tsk events advance-watermark";

pub struct Ledger<'a> {
    pub repo: &'a Repo,
    pub state_root: &'a Path,
}

impl Ledger<'_> {
    fn refresh(&self) -> Result<PathBuf, String> {
        fetch_reporting(self.repo, self.state_root).map(|outcome| outcome.path)
    }

    fn push(&self, message: &str) -> Result<(), String> {
        push_reporting(self.repo, self.state_root, message).map(|_| ())
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
    let wt = ledger.refresh()?;
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
    append_line(&queue_path(&wt), &line)?;
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
    let wt = ledger.refresh()?;
    let lines = batch_lines(events, &utc_now())?;
    append_line(&queue_path(&wt), &lines)?;
    ledger.push(&batch_message(events.len()))?;
    Ok(events.len())
}

fn append_line(queue: &Path, line: &str) -> Result<(), String> {
    if let Some(dir) = queue.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("error: could not create {}: {}", dir.display(), e))?;
    }
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(queue)
        .and_then(|mut file| file.write_all(line.as_bytes()))
        .map_err(|e| format!("error: could not append to {}: {}", queue.display(), e))
}

fn read_optional(path: &Path) -> Result<Option<String>, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("error: could not read {}: {}", path.display(), e)),
    }
}

pub fn processed_through(wt: &Path) -> Result<u64, String> {
    match read_optional(&watermark_path(wt))? {
        Some(text) => parse_watermark(&text),
        None => Ok(0),
    }
}

pub fn read_new(ledger: &Ledger) -> Result<NewEvents, String> {
    let wt = ledger.refresh()?;
    let queue = read_optional(&queue_path(&wt))?;
    let mark = match queue {
        Some(_) => processed_through(&wt)?,
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
    let wt = ledger.refresh()?;
    let current = processed_through(&wt)?;
    if check_advance(current, count)?.is_none() {
        return Ok(AdvanceOutcome::AlreadyAt(current));
    }
    let path = watermark_path(&wt);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("error: could not create {}: {}", dir.display(), e))?;
    }
    std::fs::write(&path, render_watermark(count, &utc_now()))
        .map_err(|e| format!("error: could not write {}: {}", path.display(), e))?;
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
    fn processed_through_is_zero_without_a_watermark_file() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(processed_through(dir.path()).unwrap(), 0);
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

    #[test]
    fn append_line_creates_the_queue_and_appends() {
        let dir = tempfile::tempdir().unwrap();
        let queue = queue_path(dir.path());
        append_line(&queue, "{\"n\":1}\n").unwrap();
        append_line(&queue, "{\"n\":2}\n").unwrap();
        assert_eq!(
            std::fs::read_to_string(&queue).unwrap(),
            "{\"n\":1}\n{\"n\":2}\n"
        );
    }
}
