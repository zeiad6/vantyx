# Label-Stratified JSONL Dataset Splits

`vantyx ai split <file> --out-dir <dir> --stratify <field>` is the
label-aware form of [deterministic dataset splitting](DATASET_SPLITTING.md).
It groups records by the compact JSON spelling of a top-level scalar field and
assigns a proportional number of records from every group to each split. It is
offline, never executes dataset content, never contacts a provider, and does not
modify the input file.

Without `--stratify`, `vantyx ai split` keeps its original streaming,
hash-threshold behavior and is unchanged.

## Usage

```sh
vantyx ai split train.jsonl --out-dir splits --stratify label
vantyx ai split train.jsonl --out-dir splits --stratify label \
  --train-ratio 0.8 --seed 7
vantyx ai split train.jsonl --out-dir splits --stratify label \
  --schema record.schema.json --json
```

For an input named `train.jsonl`, the outputs are the same as an unstratified
split:

```text
splits/train.train.jsonl
splits/train.heldout.jsonl
```

Both files are created with `create_new`; the command refuses to overwrite an
existing file. If writing fails after one file was created, that file is
removed again.

Text output adds the field name and one line per label before the output paths:

```text
format: jsonl-split
input: train.jsonl
input size: 310 bytes
input sha256: deaef7...
records: 10
seed: 7
train ratio: 0.7
stratify: label
strata:
  "negative": 4 records (2 train, 2 held-out)
  "positive": 6 records (4 train, 2 held-out)
train: splits/train.train.jsonl (6 records, 180 bytes)
  sha256: 60f3e9...
held-out: splits/train.heldout.jsonl (4 records, 120 bytes)
  sha256: fd5cca...
```

`--json` emits the same stable keys as an unstratified split plus:

- `stratify`: the field name, or `null`.
- `strata`: an array sorted by label. Each entry contains `label`, `records`,
  `train`, and `held_out`.

## Labels

The field must exist in every record and must be a JSON string, number, boolean,
or null. Arrays and objects are rejected. The label is the compact JSON spelling
of the value, not a coerced string:

- `"1"` (string) and `1` (number) are different labels.
- `true`, `false`, and `null` are distinct labels.
- Integral floats share the compact spelling of integers: `1` and `1.0` both
  label as `1`. Fractional values such as `1.5` keep their decimal spelling.
- A string label includes its JSON quotes in reports. The quotes are part of the
  report spelling, not part of the dataset value.

A missing field, a non-scalar value, a label longer than 1,024 bytes, or a
dataset with more than 1,000 distinct labels is a format error. No output file
is kept.

Duplicate JSON object keys are already rejected by the dataset parser, so a
record cannot contain two candidate values for the same field.

## Assignment

Records are grouped by label. Within each group they are ordered by the first
64 bits (big-endian) of

```text
SHA-256("vantyx-dataset-split-v1\0" || seed_le64 || record_bytes)
```

with the exact record bytes as a tie-breaker. For a label with `n` records and a
requested train ratio `r`:

```text
train(n) = floor(n * r)
```

For `n >= 2`, the count is clamped to `[1, n - 1]`, so every multi-record label
contributes at least one record to each split. A singleton label cannot be
divided; it goes to the majority split: train when `r >= 0.5`, held-out when
`r < 0.5`.

If the per-label rules leave one split empty, one record is moved
deterministically: the minimum-hash record to an empty train split, or the
maximum-hash record to an empty held-out split. The report includes a warning.
This preserves the unstratified split's non-empty guarantee.

Output records preserve their relative input order within each split. The
assignment is a function of the set of records, the seed, the ratio, and the
labels; it does not depend on input line order. Reordering the input therefore
changes only the order inside the output files, not which records are in them.

The achieved per-label counts are exact for the rule above, but the overall
train fraction can differ slightly from `r` because every label is rounded down
and multi-record labels are clamped. The report states actual counts.

## Validation gate

The CLI runs the same inspection as `vantyx ai dataset` before splitting, then
the stratified split re-reads and revalidates every record:

- The input must be a regular file no larger than 64 MiB.
- Each non-blank line must be valid UTF-8 and one JSON object.
- With `--schema`, every record must satisfy the supported JSON Schema subset
  before its label is read.
- Every record must contain the stratification field with a scalar value.
- At most 1,000,000 records, 1,000 distinct labels, and 1,024 bytes per compact
  label are accepted.

Invalid lines, schema violations, missing or non-scalar labels, and exceeded
limits exit `1`; no split files are kept.

## Determinism

For the same input bytes, seed, ratio, field name, and schema, the command
produces byte-identical output files on any machine and in any process. A
different seed or ratio normally changes the assignment. There is no random
state, wall-clock input, or hidden entropy.

Because assignment is content-based, an input record's split membership is
stable when other records are added, removed, or reordered, as long as its label
group size and the ratio do not change the per-label count. This is useful for
incremental corpus growth, but it is not a substitute for a frozen test set when
reproducible evaluation across corpus revisions is required.

## Record framing

The output normalizes line framing only:

- A leading UTF-8 byte-order mark is removed.
- CRLF line endings are normalized to LF.
- Blank lines are not copied.
- Each record is written as its original bytes followed by one LF.

Warnings report each normalization that occurred. Output SHA-256 values cover
exactly the bytes written.

## Limits and memory

| Limit | Value |
| --- | --- |
| Dataset file size | 64 MiB |
| Bytes per line | 16 MiB |
| Records | 1,000,000 |
| Distinct labels | 1,000 |
| Compact label size | 1,024 bytes |
| Schema file size | 16 MiB |
| Minimum records | 2 |

The stratified path holds every record in memory to group and order it, so its
input cap is lower than the streaming inspection cap. Memory is proportional to
the dataset bytes plus one label id and one 8-byte split hash per record. This
is an explicit bound, not a claim of unlimited scale; external-memory grouping
is not implemented.

## What it does not do

- It does not support multiple stratification fields or weighted labels.
- It does not tokenize records or balance token counts.
- It does not shuffle the output; records keep their relative input order.
  Run `vantyx ai shuffle` first if a shuffled corpus is required.
- It does not deduplicate. Exact duplicates share a label and therefore share a
  split; run `vantyx ai dedup` first when duplicates must be removed.
- It does not create a validation or test split beyond the single held-out set.
- It does not canonicalize JSON or normalize label Unicode.
- It does not make the pair atomic across a crash; a failed run removes the
  files it created in that run.

## Exit codes

| Code | Meaning |
| --- | --- |
| `0` | Both split files were written. |
| `1` | The dataset or schema is invalid, a label is missing or non-scalar, a limit was exceeded, an output file exists, or writing failed. |
| `64` | Usage error (missing path or `--out-dir`, bad `--train-ratio`, bad `--seed`, missing, empty, or duplicate `--stratify`, unknown option). |

## Related

- `vantyx ai split` — deterministic unstratified splits.
- `vantyx ai dedup` — exact duplicate-record removal before splitting.
- `vantyx ai shuffle` — deterministic seeded permutation.
- `docs/ai/DATASET_SPLITTING.md` — the base split contract.
- `docs/ai/DATASET_DEDUPLICATION.md` — exact record identity and limits.
- `docs/ai/DATASET_SHUFFLING.md` — deterministic ordering.
- ADR-0046 — deterministic train/held-out splitting.
- ADR-0048 — deterministic dataset shuffle.
- ADR-0049 — label-stratified dataset splits.
