use serde::Serialize;
use serde_json::value::RawValue;

use super::json::compact_values;

pub const EVENTS_DIR: &str = "external-events";
pub const QUEUE_FILE: &str = "queue.ndjson";
#[derive(Serialize)]
struct Envelope<'a> {
    source: &'a str,
    event_type: &'a str,
    action: &'a str,
    repo: &'a str,
    received_at: &'a str,
    payload: &'a RawValue,
}

pub struct NewEnvelope<'a> {
    pub source: &'a str,
    pub event_type: &'a str,
    pub action: &'a str,
    pub repo: &'a str,
    pub received_at: &'a str,
}

pub fn envelope_line(envelope: &NewEnvelope, payload: &RawValue) -> String {
    let mut line = serde_json::to_string(&Envelope {
        source: envelope.source,
        event_type: envelope.event_type,
        action: envelope.action,
        repo: envelope.repo,
        received_at: envelope.received_at,
        payload,
    })
    .unwrap_or_default();
    line.push('\n');
    line
}

pub fn total_count(queue: &str) -> u64 {
    queue.bytes().filter(|b| *b == b'\n').count() as u64
}

pub fn lines_after(queue: &str, skip: u64) -> String {
    queue
        .split_inclusive('\n')
        .skip(usize::try_from(skip).unwrap_or(usize::MAX))
        .collect()
}

#[derive(Serialize)]
pub struct NewEvents {
    pub new_count: usize,
    pub total_count: u64,
    pub events: Vec<Box<RawValue>>,
}

impl NewEvents {
    pub fn render(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

pub fn new_events(queue: Option<&str>, processed_through: u64) -> Result<NewEvents, String> {
    let queue = match queue {
        Some(text) => text,
        None => {
            return Ok(NewEvents {
                new_count: 0,
                total_count: 0,
                events: Vec::new(),
            })
        }
    };
    let total_count = total_count(queue);
    if processed_through >= total_count {
        return Ok(NewEvents {
            new_count: 0,
            total_count,
            events: Vec::new(),
        });
    }
    let events = compact_values(&lines_after(queue, processed_through)).map_err(|e| {
        format!(
            "error: {}/{} holds a line that is not JSON: {}",
            EVENTS_DIR, QUEUE_FILE, e
        )
    })?;
    Ok(NewEvents {
        new_count: events.len(),
        total_count,
        events,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::json::first_value_or_null;

    const QUEUE: &str = "{\"n\":1}\n{\"n\":2}\n{\"n\":3}\n";

    fn texts(events: &NewEvents) -> Vec<&str> {
        events.events.iter().map(|e| e.get()).collect()
    }

    #[test]
    fn envelope_line_has_the_field_order_and_a_compact_payload() {
        let payload = first_value_or_null("{\"z\": 1, \"a\": [1, 2]}").unwrap();
        let line = envelope_line(
            &NewEnvelope {
                source: "github",
                event_type: "dependabot_alert",
                action: "created",
                repo: "o/r",
                received_at: "2026-10-03T10:00:00Z",
            },
            &payload,
        );
        assert_eq!(
            line,
            "{\"source\":\"github\",\"event_type\":\"dependabot_alert\",\"action\":\"created\",\"repo\":\"o/r\",\"received_at\":\"2026-10-03T10:00:00Z\",\"payload\":{\"z\":1,\"a\":[1,2]}}\n"
        );
    }

    #[test]
    fn total_count_counts_terminated_lines_only() {
        assert_eq!(total_count(""), 0);
        assert_eq!(total_count(QUEUE), 3);
        assert_eq!(total_count("{\"n\":1}\n{\"n\":2}"), 1);
    }

    #[test]
    fn absent_queue_is_empty() {
        assert_eq!(
            new_events(None, 0).unwrap().render(),
            "{\"new_count\":0,\"total_count\":0,\"events\":[]}"
        );
    }

    #[test]
    fn empty_queue_has_nothing_new() {
        assert_eq!(
            new_events(Some(""), 0).unwrap().render(),
            "{\"new_count\":0,\"total_count\":0,\"events\":[]}"
        );
    }

    #[test]
    fn no_watermark_returns_every_event() {
        let events = new_events(Some(QUEUE), 0).unwrap();
        assert_eq!(
            events.render(),
            "{\"new_count\":3,\"total_count\":3,\"events\":[{\"n\":1},{\"n\":2},{\"n\":3}]}"
        );
    }

    #[test]
    fn watermark_skips_processed_events() {
        let events = new_events(Some(QUEUE), 2).unwrap();
        assert_eq!(events.new_count, 1);
        assert_eq!(events.total_count, 3);
        assert_eq!(texts(&events), vec!["{\"n\":3}"]);
    }

    #[test]
    fn watermark_at_or_past_the_end_has_nothing_new() {
        for mark in [3, 9] {
            assert_eq!(
                new_events(Some(QUEUE), mark).unwrap().render(),
                "{\"new_count\":0,\"total_count\":3,\"events\":[]}"
            );
        }
    }

    #[test]
    fn an_unterminated_last_line_is_read_but_not_counted() {
        let events = new_events(Some("{\"n\":1}\n{\"n\":2}"), 0).unwrap();
        assert_eq!(events.new_count, 2);
        assert_eq!(events.total_count, 1);
    }

    #[test]
    fn a_line_that_is_not_json_is_an_error() {
        let err = new_events(Some("{\"n\":1}\nnot json\n"), 0).err().unwrap();
        assert!(err.contains("external-events/queue.ndjson"), "{}", err);
    }
}
