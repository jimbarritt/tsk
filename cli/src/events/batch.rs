use std::collections::HashMap;

use serde_json::value::RawValue;

use super::json::compact;

pub const BATCH_COMMAND: &str = "tsk events append-batch";

const FIELDS: [&str; 5] = ["source", "event_type", "action", "repo", "payload"];

#[derive(Debug, PartialEq)]
pub struct BatchEvent {
    pub source: String,
    pub event_type: String,
    pub action: String,
    pub repo: String,
    pub payload: String,
}

pub struct BatchDefaults<'a> {
    pub source: Option<&'a str>,
    pub action: Option<&'a str>,
}

pub fn parse_batch(text: &str, defaults: &BatchDefaults) -> Result<Vec<BatchEvent>, String> {
    text.lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(index, line)| {
            parse_line(line, defaults).map_err(|detail| {
                format!("{}: stdin line {}: {}", BATCH_COMMAND, index + 1, detail)
            })
        })
        .collect()
}

fn parse_line(line: &str, defaults: &BatchDefaults) -> Result<BatchEvent, String> {
    let mut fields: HashMap<String, Box<RawValue>> =
        serde_json::from_str(line).map_err(|e| format!("not a JSON object: {}", e))?;
    let mut unknown: Vec<&String> = fields
        .keys()
        .filter(|key| !FIELDS.contains(&key.as_str()))
        .collect();
    if !unknown.is_empty() {
        unknown.sort();
        let names: Vec<&str> = unknown.iter().map(|key| key.as_str()).collect();
        return Err(format!("unknown field(s): {}", names.join(", ")));
    }
    let payload = fields
        .remove("payload")
        .ok_or_else(|| "missing field payload".to_string())?;
    Ok(BatchEvent {
        source: field_or_default(&fields, "source", defaults.source, "--source")?,
        event_type: string_field(&fields, "event_type")?
            .ok_or_else(|| "missing field event_type".to_string())?,
        action: field_or_default(&fields, "action", defaults.action, "--action")?,
        repo: string_field(&fields, "repo")?.ok_or_else(|| "missing field repo".to_string())?,
        payload: compact(payload.get()),
    })
}

fn string_field(
    fields: &HashMap<String, Box<RawValue>>,
    name: &str,
) -> Result<Option<String>, String> {
    match fields.get(name) {
        None => Ok(None),
        Some(raw) => match serde_json::from_str::<String>(raw.get()) {
            Ok(value) if !value.is_empty() => Ok(Some(value)),
            _ => Err(format!(
                "field {} must be a non-empty string, got {}",
                name,
                raw.get()
            )),
        },
    }
}

fn field_or_default(
    fields: &HashMap<String, Box<RawValue>>,
    name: &str,
    default: Option<&str>,
    flag: &str,
) -> Result<String, String> {
    match string_field(fields, name)? {
        Some(value) => Ok(value),
        None => default.map(str::to_string).ok_or_else(|| {
            format!(
                "missing field {}, and no {} given for the batch",
                name, flag
            )
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NONE: BatchDefaults = BatchDefaults {
        source: None,
        action: None,
    };

    const GITHUB_POLLED: BatchDefaults = BatchDefaults {
        source: Some("github"),
        action: Some("polled"),
    };

    #[test]
    fn empty_text_is_an_empty_batch() {
        assert_eq!(parse_batch("", &NONE).unwrap(), vec![]);
        assert_eq!(parse_batch("\n  \n", &NONE).unwrap(), vec![]);
    }

    #[test]
    fn a_line_carries_every_field() {
        let events = parse_batch(
            "{\"source\":\"s\",\"event_type\":\"t\",\"action\":\"a\",\"repo\":\"o/r\",\"payload\":{\"z\": 1, \"a\": [1, 2]}}\n",
            &NONE,
        )
        .unwrap();
        assert_eq!(
            events,
            vec![BatchEvent {
                source: "s".to_string(),
                event_type: "t".to_string(),
                action: "a".to_string(),
                repo: "o/r".to_string(),
                payload: "{\"z\":1,\"a\":[1,2]}".to_string(),
            }]
        );
    }

    #[test]
    fn source_and_action_come_from_the_defaults_when_the_line_has_none() {
        let events = parse_batch(
            "{\"event_type\":\"t\",\"repo\":\"o/r\",\"payload\":null}",
            &GITHUB_POLLED,
        )
        .unwrap();
        assert_eq!(events[0].source, "github");
        assert_eq!(events[0].action, "polled");
        assert_eq!(events[0].payload, "null");
    }

    #[test]
    fn a_line_value_overrides_the_default() {
        let events = parse_batch(
            "{\"source\":\"s\",\"action\":\"fixed\",\"event_type\":\"t\",\"repo\":\"o/r\",\"payload\":1}",
            &GITHUB_POLLED,
        )
        .unwrap();
        assert_eq!(events[0].source, "s");
        assert_eq!(events[0].action, "fixed");
    }

    #[test]
    fn a_missing_source_without_a_default_names_the_flag() {
        let err = parse_batch(
            "{\"action\":\"a\",\"event_type\":\"t\",\"repo\":\"o/r\",\"payload\":1}",
            &NONE,
        )
        .unwrap_err();
        assert_eq!(
            err,
            "tsk events append-batch: stdin line 1: missing field source, and no --source given for the batch"
        );
    }

    #[test]
    fn errors_name_the_physical_line_number() {
        let text = "{\"event_type\":\"t\",\"repo\":\"o/r\",\"payload\":1}\n\nnot json\n";
        let err = parse_batch(text, &GITHUB_POLLED).unwrap_err();
        assert!(
            err.starts_with("tsk events append-batch: stdin line 3: not a JSON object"),
            "{}",
            err
        );
    }

    #[test]
    fn required_fields_are_checked() {
        let cases = [
            (
                "{\"repo\":\"o/r\",\"payload\":1}",
                "missing field event_type",
            ),
            ("{\"event_type\":\"t\",\"payload\":1}", "missing field repo"),
            (
                "{\"event_type\":\"t\",\"repo\":\"o/r\"}",
                "missing field payload",
            ),
            (
                "{\"event_type\":3,\"repo\":\"o/r\",\"payload\":1}",
                "field event_type must be a non-empty string, got 3",
            ),
            (
                "{\"event_type\":\"\",\"repo\":\"o/r\",\"payload\":1}",
                "field event_type must be a non-empty string, got \"\"",
            ),
            (
                "{\"event_type\":\"t\",\"repo\":\"o/r\",\"payload\":1,\"received_at\":\"x\"}",
                "unknown field(s): received_at",
            ),
            ("[1]", "not a JSON object"),
        ];
        for (line, expected) in cases {
            let err = parse_batch(line, &GITHUB_POLLED).unwrap_err();
            assert!(err.contains(expected), "{}: {}", line, err);
        }
    }
}
