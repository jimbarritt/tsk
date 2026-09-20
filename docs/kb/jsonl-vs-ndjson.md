# JSON Lines vs NDJSON

Both names describe the same format: one JSON value per line, newline-delimited,
no wrapping array, no commas between records.

## The two specs

- **JSON Lines** (jsonlines.org, Ian Ward, 2013): permits blank lines between
  records, keeps whitespace handling loose. `.jsonl` is its extension.
- **NDJSON** (ndjson-spec on GitHub, published 2013-07-05, 1.0.0 in 2014):
  stricter. `\n` only as separator, no blank lines, UTF-8 mandated. `.ndjson`
  is its extension.

Both emerged from the same 2010-2013 wave of tools (Hadoop, Elasticsearch's
bulk API, Docker log output) that needed one-record-per-line streaming JSON
and converged on the same idea independently. A third, rarer format exists
for the same problem: JSON Text Sequences (RFC 7464), which prefixes each
record with an `RS` (0x1E) control character instead of relying on newlines.

## Which to use in this repo

Use `.jsonl` as the file extension. It is the more common convention: OpenAI's
fine-tuning and batch APIs use it, Hugging Face `datasets` defaults to it, and
most LLM tooling follows the same convention. `.ndjson` shows up mainly in
streaming-protocol contexts, such as Elasticsearch's bulk API docs.

Parsers for either format are interchangeable in practice: `jq`, pandas, and
most language JSON libraries with a streaming reader accept both without
configuration.
