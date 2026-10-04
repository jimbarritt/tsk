use serde_json::value::RawValue;

pub fn compact(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut in_string = false;
    let mut escaped = false;
    for c in raw.chars() {
        if in_string {
            out.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
        } else if c == '"' {
            in_string = true;
            out.push(c);
        } else if !matches!(c, ' ' | '\t' | '\n' | '\r') {
            out.push(c);
        }
    }
    out
}

pub fn parse_values(text: &str) -> Result<Vec<Box<RawValue>>, serde_json::Error> {
    serde_json::Deserializer::from_str(text)
        .into_iter::<Box<RawValue>>()
        .collect()
}

pub fn compact_values(text: &str) -> Result<Vec<Box<RawValue>>, serde_json::Error> {
    parse_values(text)?
        .into_iter()
        .map(|value| RawValue::from_string(compact(value.get())))
        .collect()
}

pub fn first_value_or_null(text: &str) -> Result<Box<RawValue>, serde_json::Error> {
    match compact_values(text)?.into_iter().next() {
        Some(value) => Ok(value),
        None => RawValue::from_string("null".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_removes_whitespace_outside_strings_and_keeps_key_order() {
        let raw = "{\n  \"z\": 1,\n  \"a\": { \"b c\": [1, 2.50, \"x \\\" y\"] }\n}";
        assert_eq!(
            compact(raw),
            "{\"z\":1,\"a\":{\"b c\":[1,2.50,\"x \\\" y\"]}}"
        );
    }

    #[test]
    fn compact_keeps_an_escaped_backslash_before_a_closing_quote() {
        assert_eq!(compact("[\"a\\\\\" , 1]"), "[\"a\\\\\",1]");
    }

    #[test]
    fn compact_values_reads_a_stream_of_values() {
        let values = compact_values("{\"a\": 1}\n[1, 2]\n\"s\"\n").unwrap();
        let texts: Vec<&str> = values.iter().map(|v| v.get()).collect();
        assert_eq!(texts, vec!["{\"a\":1}", "[1,2]", "\"s\""]);
    }

    #[test]
    fn compact_values_rejects_invalid_json() {
        assert!(compact_values("{\"a\": 1}\n{not json}\n").is_err());
    }

    #[test]
    fn first_value_or_null_takes_the_first_value() {
        assert_eq!(
            first_value_or_null("{\"b\": 2, \"a\": 1} {\"c\": 3}")
                .unwrap()
                .get(),
            "{\"b\":2,\"a\":1}"
        );
    }

    #[test]
    fn first_value_or_null_is_null_for_an_empty_file() {
        assert_eq!(first_value_or_null("  \n").unwrap().get(), "null");
    }
}
