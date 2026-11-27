# JSONL Dataset Inspection

`vantyx ai dataset <file>` validates a local JSONL dataset, optionally
validates every record against a JSON Schema file, and reports provenance
without modifying it. It is the dataset counterpart of `vantyx ai inspect` and
is intended to gate data before tokenization or training work.

The command is offline: it never downloads data, executes dataset content, or
contacts a provider.

## Format

A JSONL dataset is UTF-8 text with one JSON object per line. Blank lines are
allowed and ignored. A UTF-8 byte-order mark on the first line is accepted.
Both LF and CRLF line endings are accepted.

```jsonl
{"text":"hello","label":1}
{"text":"world","label":0}
```

A non-blank line that is not a JSON object is invalid. Arrays, scalars,
malformed JSON, and invalid UTF-8 are reported with line numbers.

## Usage

```sh
vantyx ai dataset train.jsonl
vantyx ai dataset train.jsonl --json
vantyx ai dataset train.jsonl --schema record.schema.json
vantyx ai dataset train.jsonl --schema record.schema.json --json
```

Text output:

```text
format: jsonl-dataset
file: train.jsonl
size: 512 bytes
sha256: 9f2c...
lines: 3 (blank: 1)
records: 2
invalid lines: 0
fields:
  label: 2 record(s) (number 2)
  text: 2 record(s) (string 2)
```

With `--schema`, the text output adds:

```text
schema: record.schema.json
schema-invalid records: 0
```

`--json` prints one compact JSON object with stable keys: `path`, `format`,
`file_bytes`, `sha256`, `line_count`, `blank_lines`, `record_count`,
`invalid_line_count`, `invalid_lines`, `fields`, `schema`, and `warnings`.
`schema` is `null` unless `--schema` was supplied; otherwise it contains
`path`, `invalid_record_count`, and `violations`, where each violation has
`line`, `path`, and `message`.

Each field entry contains its name, the number of records that contain it, and
a type histogram (`null`, `bool`, `number`, `string`, `array`, `object`).
Fields are sorted by name; types are sorted by name.

## Schema validation

`--schema <file>` reads one JSON Schema file and validates every structurally
valid record against the supported subset:

`type`, `enum`, `const`, `anyOf`, `allOf`, `required`, `properties`,
`additionalProperties`, `items`, `minItems`, `maxItems`, `minLength`,
`maxLength`, `minimum`, and `maximum`.

The schema file is checked once before the dataset is read. Malformed JSON,
invalid UTF-8, a non-object schema, or a supported keyword with the wrong shape
is an `AI_E_FORMAT` error with exit `1`. Unsupported keywords are ignored, as
they are during `vantyx ai extract`; this is not full JSON Schema compliance.

The same preflight runs in `vantyx ai extract` and before a tool call, so a
schema that dataset validation rejects is never sent to a provider or used to
judge tool arguments. Keywords are evaluated cumulatively: a matching `anyOf`
does not suppress sibling `minimum`, `required`, or other constraints.
`const` and `enum` use JSON Schema instance equality (`1` equals `1.0`, and
object member order is ignored), and `minimum`/`maximum` compare integers and
mixed integer/float values exactly beyond `f64`'s 53-bit range. See ADR-0045.

Only records that already parse as JSON objects are schema-validated. Invalid
JSON, invalid UTF-8, and non-object lines are counted as invalid lines instead.
Each schema-invalid record is counted once with its first violation. The first
20 violations are listed; the total is always reported. A valid schema with no
records is accepted.

Schema validation is structural, not semantic: it can require fields and check
types and bounds, but it cannot know which fields a trainer needs, detect
label leakage, or certify licensing.

## Exit codes

| Code | Meaning |
| --- | --- |
| `0` | Every non-blank line is a JSON object and, when `--schema` was given, every record satisfies the schema. |
| `1` | The dataset has invalid lines or schema-invalid records, the schema cannot be read or parsed, or the dataset cannot be read. The summary is still printed when dataset parsing succeeded. |
| `64` | Usage error (missing path, two paths, unknown option, missing `--schema` value, or duplicate `--schema`). |

Invalid content exits `1` so a shell pipeline can gate on it:

```sh
vantyx ai dataset train.jsonl --schema record.schema.json --json > summary.json
```

The summary is written to stdout before the error line on stderr, so callers
can inspect both the report and the failure.

## Limits

| Limit | Value |
| --- | --- |
| Dataset file size | 256 MiB |
| Bytes per line | 16 MiB |
| Distinct top-level fields | 10,000 |
| Invalid lines listed individually | 20 (the total is always counted) |
| Schema file size | 16 MiB |
| Schema violations listed individually | 20 (the total is always counted) |
| Schema nesting depth | 32 |

The dataset is streamed line by line; the inspector does not load the whole
dataset into memory. The digest covers every byte read, including blank lines,
line endings, and the BOM. The schema file is read into memory because schemas
are small and bounded.

## What it does not do

- It does not tokenize, shuffle, or normalize records. Use `vantyx ai split`
  for deterministic train/held-out files and `vantyx ai dedup` to remove exact
  duplicate records.
- It does not inspect nested objects beyond reporting them as `object`.
- It does not implement full JSON Schema; unsupported keywords are ignored.
- It does not detect duplicate JSON keys inside one object; each occurrence is
  counted as an entry.
- It does not modify, move, or delete the dataset or the schema.

## Related

- `vantyx ai inspect` — bounded GGUF/Safetensors model inspection.
- `vantyx ai split` — deterministic train/held-out JSONL splits.
- `vantyx ai dedup` — exact duplicate-record removal with provenance.
- `vantyx ai extract` — validated structured extraction with the same schema
  subset.
- `docs/ai/AI_ARCHITECTURE.md` — AI host module map.
- ADR-0043 — decision record for bounded JSONL dataset inspection.
- ADR-0044 — decision record for bounded dataset schema validation.
- ADR-0045 — decision record for JSON Schema conformance and shared preflight.
- ADR-0046 — decision record for deterministic train/held-out splits.
