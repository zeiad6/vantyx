//! Bounded, read-only inspection of local JSONL datasets.
//!
//! A JSONL dataset is one UTF-8 JSON object per line. Blank lines are allowed
//! and ignored. The inspector streams the file through SHA-256, validates every
//! record, and summarizes top-level fields and JSON value types without
//! loading the dataset into memory or modifying it. An optional JSON Schema
//! file can validate every structurally valid record against the supported
//! subset.

use super::json::{self, Json, JsonNumber};
use super::model::Sha256;
use super::provider::{AI_E_FORMAT, AI_E_INVALID_REQUEST, AI_E_IO, AiError};
use super::schema;
use std::collections::{BTreeMap, HashSet};
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::Path;

/// Maximum dataset size accepted by one inspection.
pub const MAX_DATASET_BYTES: u64 = 256 * 1024 * 1024;
/// Maximum bytes accepted in one JSONL line, including its newline.
pub const MAX_DATASET_LINE_BYTES: usize = 16 * 1024 * 1024;
/// Maximum distinct top-level field names tracked in one dataset.
pub const MAX_DATASET_FIELDS: usize = 10_000;
/// Maximum invalid lines reported with line numbers and messages.
pub const MAX_REPORTED_INVALID_LINES: usize = 20;
/// Maximum bytes accepted in one JSON Schema file.
pub const MAX_SCHEMA_BYTES: u64 = 16 * 1024 * 1024;
/// Maximum schema violations reported with line numbers and paths.
pub const MAX_REPORTED_SCHEMA_VIOLATIONS: usize = 20;
/// Default fraction of records assigned to the train split.
pub const DEFAULT_TRAIN_RATIO: f64 = 0.9;
/// Maximum distinct records retained by one deduplication. The bound keeps the
/// in-memory digest set predictable on large corpora.
pub const MAX_DEDUP_UNIQUE_RECORDS: usize = 1_000_000;
/// Maximum dataset size accepted by one deterministic shuffle. The whole
/// dataset is held in memory, so this bound is lower than inspection's.
pub const MAX_SHUFFLE_BYTES: u64 = 64 * 1024 * 1024;
/// Maximum records accepted by one deterministic shuffle.
pub const MAX_SHUFFLE_RECORDS: usize = 1_000_000;
/// Domain separator for deterministic per-record split assignment.
const SPLIT_HASH_DOMAIN: &[u8] = b"vantyx-dataset-split-v1\0";
/// Domain separator for exact-record deduplication digests.
const DEDUP_HASH_DOMAIN: &[u8] = b"vantyx-dataset-dedup-v1\0";
/// Domain separator for deterministic shuffle ordering.
const SHUFFLE_HASH_DOMAIN: &[u8] = b"vantyx-dataset-shuffle-v1\0";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypeCount {
    pub kind: String,
    pub count: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldSummary {
    pub name: String,
    pub records: u64,
    pub types: Vec<TypeCount>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InvalidLine {
    pub line: u64,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SchemaViolation {
    pub line: u64,
    pub path: String,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SchemaSummary {
    pub path: String,
    pub invalid_record_count: u64,
    pub violations: Vec<SchemaViolation>,
}

/// A validated, read-only summary of a local JSONL dataset.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DatasetSummary {
    pub path: String,
    pub file_bytes: u64,
    pub sha256: String,
    pub line_count: u64,
    pub blank_lines: u64,
    pub record_count: u64,
    pub invalid_line_count: u64,
    pub invalid_lines: Vec<InvalidLine>,
    pub fields: Vec<FieldSummary>,
    pub schema: Option<SchemaSummary>,
    pub warnings: Vec<String>,
}

impl DatasetSummary {
    /// A dataset is valid when every non-blank line is one JSON object and,
    /// when a schema was supplied, every record satisfies it.
    pub fn is_valid(&self) -> bool {
        self.invalid_line_count == 0
            && self
                .schema
                .as_ref()
                .is_none_or(|schema| schema.invalid_record_count == 0)
    }

    /// Serializes the summary as a stable JSON object suitable for scripts.
    pub fn to_json(&self) -> Json {
        Json::object([
            ("path", Json::String(self.path.clone())),
            ("format", Json::String("jsonl-dataset".to_string())),
            (
                "file_bytes",
                Json::Number(JsonNumber::UInt(self.file_bytes)),
            ),
            ("sha256", Json::String(self.sha256.clone())),
            (
                "line_count",
                Json::Number(JsonNumber::UInt(self.line_count)),
            ),
            (
                "blank_lines",
                Json::Number(JsonNumber::UInt(self.blank_lines)),
            ),
            (
                "record_count",
                Json::Number(JsonNumber::UInt(self.record_count)),
            ),
            (
                "invalid_line_count",
                Json::Number(JsonNumber::UInt(self.invalid_line_count)),
            ),
            (
                "invalid_lines",
                Json::Array(
                    self.invalid_lines
                        .iter()
                        .map(|invalid| {
                            Json::object([
                                ("line", Json::Number(JsonNumber::UInt(invalid.line))),
                                ("message", Json::String(invalid.message.clone())),
                            ])
                        })
                        .collect(),
                ),
            ),
            (
                "fields",
                Json::Array(
                    self.fields
                        .iter()
                        .map(|field| {
                            Json::object([
                                ("name", Json::String(field.name.clone())),
                                ("records", Json::Number(JsonNumber::UInt(field.records))),
                                (
                                    "types",
                                    Json::Array(
                                        field
                                            .types
                                            .iter()
                                            .map(|entry| {
                                                Json::object([
                                                    ("type", Json::String(entry.kind.clone())),
                                                    (
                                                        "count",
                                                        Json::Number(JsonNumber::UInt(entry.count)),
                                                    ),
                                                ])
                                            })
                                            .collect(),
                                    ),
                                ),
                            ])
                        })
                        .collect(),
                ),
            ),
            (
                "schema",
                match &self.schema {
                    None => Json::Null,
                    Some(schema) => Json::object([
                        ("path", Json::String(schema.path.clone())),
                        (
                            "invalid_record_count",
                            Json::Number(JsonNumber::UInt(schema.invalid_record_count)),
                        ),
                        (
                            "violations",
                            Json::Array(
                                schema
                                    .violations
                                    .iter()
                                    .map(|violation| {
                                        Json::object([
                                            (
                                                "line",
                                                Json::Number(JsonNumber::UInt(violation.line)),
                                            ),
                                            ("path", Json::String(violation.path.clone())),
                                            ("message", Json::String(violation.message.clone())),
                                        ])
                                    })
                                    .collect(),
                            ),
                        ),
                    ]),
                },
            ),
            (
                "warnings",
                Json::Array(
                    self.warnings
                        .iter()
                        .map(|warning| Json::String(warning.clone()))
                        .collect(),
                ),
            ),
        ])
    }
}

/// One output file of a deterministic dataset split.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SplitPart {
    pub path: String,
    pub records: u64,
    pub bytes: u64,
    pub sha256: String,
}

/// A validated, deterministic train/held-out split of a JSONL dataset.
#[derive(Clone, Debug, PartialEq)]
pub struct SplitSummary {
    pub input_path: String,
    pub input_sha256: String,
    pub input_bytes: u64,
    pub record_count: u64,
    pub seed: u64,
    pub train_ratio: f64,
    pub train: SplitPart,
    pub held_out: SplitPart,
    pub warnings: Vec<String>,
}

impl SplitSummary {
    /// Serializes the split report as a stable JSON object suitable for scripts.
    pub fn to_json(&self) -> Json {
        fn part(part: &SplitPart) -> Json {
            Json::object([
                ("path", Json::String(part.path.clone())),
                ("records", Json::Number(JsonNumber::UInt(part.records))),
                ("bytes", Json::Number(JsonNumber::UInt(part.bytes))),
                ("sha256", Json::String(part.sha256.clone())),
            ])
        }
        Json::object([
            ("format", Json::String("jsonl-split".to_string())),
            ("input", Json::String(self.input_path.clone())),
            ("input_sha256", Json::String(self.input_sha256.clone())),
            (
                "input_bytes",
                Json::Number(JsonNumber::UInt(self.input_bytes)),
            ),
            ("records", Json::Number(JsonNumber::UInt(self.record_count))),
            ("seed", Json::Number(JsonNumber::UInt(self.seed))),
            (
                "train_ratio",
                Json::Number(JsonNumber::Float(self.train_ratio)),
            ),
            ("train", part(&self.train)),
            ("held_out", part(&self.held_out)),
            (
                "warnings",
                Json::Array(
                    self.warnings
                        .iter()
                        .map(|warning| Json::String(warning.clone()))
                        .collect(),
                ),
            ),
        ])
    }
}

/// A validated, deterministic removal of exact duplicate JSONL records.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DedupSummary {
    pub input_path: String,
    pub input_sha256: String,
    pub input_bytes: u64,
    pub output_path: String,
    pub output_bytes: u64,
    pub output_sha256: String,
    pub record_count: u64,
    pub unique_records: u64,
    pub duplicates_removed: u64,
    pub warnings: Vec<String>,
}

impl DedupSummary {
    /// Serializes the deduplication report as a stable JSON object.
    pub fn to_json(&self) -> Json {
        Json::object([
            ("format", Json::String("jsonl-dedup".to_string())),
            ("input", Json::String(self.input_path.clone())),
            ("input_sha256", Json::String(self.input_sha256.clone())),
            (
                "input_bytes",
                Json::Number(JsonNumber::UInt(self.input_bytes)),
            ),
            ("output", Json::String(self.output_path.clone())),
            (
                "output_bytes",
                Json::Number(JsonNumber::UInt(self.output_bytes)),
            ),
            ("output_sha256", Json::String(self.output_sha256.clone())),
            ("records", Json::Number(JsonNumber::UInt(self.record_count))),
            (
                "unique_records",
                Json::Number(JsonNumber::UInt(self.unique_records)),
            ),
            (
                "duplicates_removed",
                Json::Number(JsonNumber::UInt(self.duplicates_removed)),
            ),
            (
                "warnings",
                Json::Array(
                    self.warnings
                        .iter()
                        .map(|warning| Json::String(warning.clone()))
                        .collect(),
                ),
            ),
        ])
    }
}

/// A validated, deterministic permutation of a JSONL dataset.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShuffleSummary {
    pub input_path: String,
    pub input_sha256: String,
    pub input_bytes: u64,
    pub output_path: String,
    pub output_bytes: u64,
    pub output_sha256: String,
    pub record_count: u64,
    pub seed: u64,
    pub warnings: Vec<String>,
}

impl ShuffleSummary {
    /// Serializes the shuffle report as a stable JSON object.
    pub fn to_json(&self) -> Json {
        Json::object([
            ("format", Json::String("jsonl-shuffle".to_string())),
            ("input", Json::String(self.input_path.clone())),
            ("input_sha256", Json::String(self.input_sha256.clone())),
            (
                "input_bytes",
                Json::Number(JsonNumber::UInt(self.input_bytes)),
            ),
            ("output", Json::String(self.output_path.clone())),
            (
                "output_bytes",
                Json::Number(JsonNumber::UInt(self.output_bytes)),
            ),
            ("output_sha256", Json::String(self.output_sha256.clone())),
            ("records", Json::Number(JsonNumber::UInt(self.record_count))),
            ("seed", Json::Number(JsonNumber::UInt(self.seed))),
            (
                "warnings",
                Json::Array(
                    self.warnings
                        .iter()
                        .map(|warning| Json::String(warning.clone()))
                        .collect(),
                ),
            ),
        ])
    }
}

/// Inspects a UTF-8 JSONL dataset without modifying it.
pub fn inspect(path: &Path) -> Result<DatasetSummary, AiError> {
    inspect_with_limits(
        path,
        MAX_DATASET_BYTES,
        MAX_DATASET_LINE_BYTES,
        MAX_DATASET_FIELDS,
    )
}

/// Inspects a UTF-8 JSONL dataset and validates every structurally valid
/// record against a JSON Schema file.
pub fn inspect_with_schema(path: &Path, schema_path: &Path) -> Result<DatasetSummary, AiError> {
    let schema = load_schema(schema_path)?;
    inspect_inner(
        path,
        Some((schema_path, &schema)),
        MAX_DATASET_BYTES,
        MAX_DATASET_LINE_BYTES,
        MAX_DATASET_FIELDS,
    )
}

/// Splits a validated UTF-8 JSONL dataset into deterministic train and
/// held-out files.
///
/// Each non-blank record is assigned by a seeded SHA-256 of its bytes, so the
/// same input, seed, and ratio always produce the same split regardless of
/// line order or process. Output files are created with `create_new`, never
/// overwritten, and removed again if writing fails. Blank lines, a leading
/// byte-order mark, and CRLF endings are not copied; records are written with
/// one LF terminator each.
pub fn split(
    path: &Path,
    out_dir: &Path,
    train_ratio: f64,
    seed: u64,
) -> Result<SplitSummary, AiError> {
    split_inner(path, out_dir, train_ratio, seed, None)
}

/// Splits a UTF-8 JSONL dataset after validating every record against a JSON
/// Schema file. A record that violates the schema aborts the split before any
/// output file is kept.
pub fn split_with_schema(
    path: &Path,
    out_dir: &Path,
    train_ratio: f64,
    seed: u64,
    schema_path: &Path,
) -> Result<SplitSummary, AiError> {
    let schema = load_schema(schema_path)?;
    split_inner(
        path,
        out_dir,
        train_ratio,
        seed,
        Some((schema_path, &schema)),
    )
}

/// Removes exact duplicate records from a validated UTF-8 JSONL dataset.
///
/// The first occurrence of each record is kept in input order. Record identity
/// is the exact bytes of the normalized record (BOM, CRLF, and the trailing
/// newline excluded), so JSON formatting or key-order differences are not
/// normalized away. The output file is created with `create_new`, never
/// overwritten, and removed again if writing fails. The input is never
/// modified.
pub fn deduplicate(path: &Path, out: &Path) -> Result<DedupSummary, AiError> {
    dedup_inner(path, out, None, MAX_DEDUP_UNIQUE_RECORDS)
}

/// Removes exact duplicate records after validating every record against a
/// JSON Schema file. A record that violates the schema aborts the operation
/// before any output file is kept.
pub fn deduplicate_with_schema(
    path: &Path,
    out: &Path,
    schema_path: &Path,
) -> Result<DedupSummary, AiError> {
    let schema = load_schema(schema_path)?;
    dedup_inner(
        path,
        out,
        Some((schema_path, &schema)),
        MAX_DEDUP_UNIQUE_RECORDS,
    )
}

/// Writes a deterministic permutation of a validated UTF-8 JSONL dataset.
///
/// Records are ordered by a domain-separated SHA-256 of the seed and the exact
/// record bytes, with the record bytes as a tie-breaker. The output therefore
/// depends only on the set of records and the seed, not on their input order.
/// The whole dataset is held in memory under `MAX_SHUFFLE_BYTES` and
/// `MAX_SHUFFLE_RECORDS`. The output file is created with `create_new`, never
/// overwritten, and removed again if writing fails. The input is never
/// modified.
pub fn shuffle(path: &Path, out: &Path, seed: u64) -> Result<ShuffleSummary, AiError> {
    shuffle_inner(
        path,
        out,
        seed,
        None,
        MAX_SHUFFLE_BYTES,
        MAX_SHUFFLE_RECORDS,
    )
}

/// Writes a deterministic permutation after validating every record against a
/// JSON Schema file. A record that violates the schema aborts the operation
/// before any output file is kept.
pub fn shuffle_with_schema(
    path: &Path,
    out: &Path,
    seed: u64,
    schema_path: &Path,
) -> Result<ShuffleSummary, AiError> {
    let schema = load_schema(schema_path)?;
    shuffle_inner(
        path,
        out,
        seed,
        Some((schema_path, &schema)),
        MAX_SHUFFLE_BYTES,
        MAX_SHUFFLE_RECORDS,
    )
}

fn shuffle_inner(
    path: &Path,
    out: &Path,
    seed: u64,
    schema: Option<(&Path, &Json)>,
    max_bytes: u64,
    max_records: usize,
) -> Result<ShuffleSummary, AiError> {
    if path == out {
        return Err(AiError::new(
            AI_E_INVALID_REQUEST,
            "the shuffled output must differ from the input file",
        ));
    }
    if out.file_name().is_none() {
        return Err(AiError::new(
            AI_E_INVALID_REQUEST,
            format!("cannot derive an output file name from `{}`", out.display()),
        ));
    }
    if out.exists() {
        return Err(AiError::new(
            AI_E_IO,
            format!(
                "refusing to overwrite `{}`; choose another output path or remove the file",
                out.display()
            ),
        ));
    }

    let scan = read_shuffle_records(path, schema.map(|(_, value)| value), max_bytes, max_records)?;
    if scan.records.is_empty() {
        return Err(AiError::new(
            AI_E_INVALID_REQUEST,
            "dataset has 0 records; at least 1 record is required to shuffle",
        ));
    }

    let file = create_output(out)?;
    match write_shuffle(file, out, seed, &scan) {
        Ok(output) => {
            if output.records != scan.records.len() as u64 {
                let _ = std::fs::remove_file(out);
                return Err(AiError::new(
                    AI_E_IO,
                    "dataset changed while it was being shuffled; no output was kept",
                ));
            }
            let mut warnings = Vec::new();
            if scan.blank_lines > 0 {
                warnings.push(format!(
                    "{} blank line(s) were not copied",
                    scan.blank_lines
                ));
            }
            if scan.crlf_lines > 0 {
                warnings.push(format!(
                    "{} CRLF line ending(s) were normalized to LF",
                    scan.crlf_lines
                ));
            }
            if scan.bom {
                warnings.push("the UTF-8 byte-order mark was removed".to_string());
            }
            Ok(ShuffleSummary {
                input_path: path.display().to_string(),
                input_sha256: scan.sha256,
                input_bytes: scan.file_bytes,
                output_path: out.display().to_string(),
                output_bytes: output.bytes,
                output_sha256: output.sha256,
                record_count: scan.records.len() as u64,
                seed,
                warnings,
            })
        }
        Err(error) => {
            let _ = std::fs::remove_file(out);
            Err(error)
        }
    }
}

fn dedup_inner(
    path: &Path,
    out: &Path,
    schema: Option<(&Path, &Json)>,
    max_unique: usize,
) -> Result<DedupSummary, AiError> {
    if path == out {
        return Err(AiError::new(
            AI_E_INVALID_REQUEST,
            "the deduplicated output must differ from the input file",
        ));
    }
    if out.file_name().is_none() {
        return Err(AiError::new(
            AI_E_INVALID_REQUEST,
            format!("cannot derive an output file name from `{}`", out.display()),
        ));
    }
    if out.exists() {
        return Err(AiError::new(
            AI_E_IO,
            format!(
                "refusing to overwrite `{}`; choose another output path or remove the file",
                out.display()
            ),
        ));
    }

    let scan = scan_dedup_input(path, schema.map(|(_, value)| value), max_unique)?;
    if scan.records == 0 {
        return Err(AiError::new(
            AI_E_INVALID_REQUEST,
            "dataset has 0 records; at least 1 record is required to deduplicate",
        ));
    }

    let file = create_output(out)?;
    match write_dedup(path, file, out, &scan) {
        Ok(output) => {
            if output.records != scan.unique_records {
                let _ = std::fs::remove_file(out);
                return Err(AiError::new(
                    AI_E_IO,
                    "dataset changed while it was being deduplicated; no output was kept",
                ));
            }
            let mut warnings = Vec::new();
            if scan.blank_lines > 0 {
                warnings.push(format!(
                    "{} blank line(s) were not copied",
                    scan.blank_lines
                ));
            }
            if scan.crlf_lines > 0 {
                warnings.push(format!(
                    "{} CRLF line ending(s) were normalized to LF",
                    scan.crlf_lines
                ));
            }
            if scan.bom {
                warnings.push("the UTF-8 byte-order mark was removed".to_string());
            }
            Ok(DedupSummary {
                input_path: path.display().to_string(),
                input_sha256: scan.sha256,
                input_bytes: scan.file_bytes,
                output_path: out.display().to_string(),
                output_bytes: output.bytes,
                output_sha256: output.sha256,
                record_count: scan.records,
                unique_records: scan.unique_records,
                duplicates_removed: scan.duplicates_removed,
                warnings,
            })
        }
        Err(error) => {
            let _ = std::fs::remove_file(out);
            Err(error)
        }
    }
}

struct DedupScan {
    file_bytes: u64,
    sha256: String,
    records: u64,
    unique_records: u64,
    duplicates_removed: u64,
    blank_lines: u64,
    crlf_lines: u64,
    bom: bool,
}

fn scan_dedup_input(
    path: &Path,
    schema: Option<&Json>,
    max_unique: usize,
) -> Result<DedupScan, AiError> {
    let file = File::open(path).map_err(|error| {
        AiError::new(
            AI_E_IO,
            format!("cannot open dataset `{}`: {error}", path.display()),
        )
    })?;
    let metadata = file.metadata().map_err(|error| {
        AiError::new(
            AI_E_IO,
            format!("cannot stat dataset `{}`: {error}", path.display()),
        )
    })?;
    if !metadata.is_file() {
        return Err(AiError::new(
            AI_E_IO,
            format!(
                "cannot deduplicate `{}`: not a regular file",
                path.display()
            ),
        ));
    }
    if metadata.len() > MAX_DATASET_BYTES {
        return Err(format_error(format!(
            "JSONL dataset is {} bytes, above the {MAX_DATASET_BYTES} byte limit",
            metadata.len()
        )));
    }

    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buffer = Vec::new();
    let mut line_number = 0_u64;
    let mut hashed_bytes = 0_u64;
    let mut seen = HashSet::new();
    let mut scan = DedupScan {
        file_bytes: metadata.len(),
        sha256: String::new(),
        records: 0,
        unique_records: 0,
        duplicates_removed: 0,
        blank_lines: 0,
        crlf_lines: 0,
        bom: false,
    };

    loop {
        let outcome = read_line_capped(
            &mut reader,
            &mut buffer,
            MAX_DATASET_LINE_BYTES,
            &mut hasher,
        )
        .map_err(io_failure)?;
        let (bytes, too_long) = match outcome {
            LineOutcome::Eof => break,
            LineOutcome::Line { bytes, too_long } => (bytes, too_long),
        };
        line_number += 1;
        hashed_bytes = hashed_bytes.saturating_add(bytes as u64);
        if too_long {
            return Err(format_error(format!(
                "line {line_number} is longer than {MAX_DATASET_LINE_BYTES} bytes"
            )));
        }
        if line_number == 1 && buffer.starts_with(&[0xEF, 0xBB, 0xBF]) {
            scan.bom = true;
        }
        if buffer.ends_with(b"\r\n") {
            scan.crlf_lines += 1;
        }
        let Some(record) = record_bytes(&buffer, line_number == 1) else {
            scan.blank_lines += 1;
            continue;
        };
        let value = parse_split_record(record, line_number)?;
        if let Some(schema_value) = schema
            && let Err(error) = schema::validate(schema_value, &value)
        {
            return Err(format_error(format!(
                "line {line_number} does not satisfy the schema at {}: {}",
                error.path, error.message
            )));
        }
        scan.records += 1;
        if seen.insert(dedup_record_hash(record)) {
            scan.unique_records += 1;
            if scan.unique_records > max_unique as u64 {
                return Err(format_error(format!(
                    "dataset has more than {max_unique} distinct records; the deduplication limit was reached"
                )));
            }
        } else {
            scan.duplicates_removed += 1;
        }
    }
    if hashed_bytes != metadata.len() {
        return Err(format_error(format!(
            "dataset size changed while it was being read: expected {} bytes, read {hashed_bytes}",
            metadata.len()
        )));
    }
    scan.sha256 = hasher.finalize_hex();
    Ok(scan)
}

fn write_dedup(
    path: &Path,
    file: File,
    out: &Path,
    scan: &DedupScan,
) -> Result<SplitPart, AiError> {
    let input = File::open(path).map_err(|error| {
        AiError::new(
            AI_E_IO,
            format!("cannot reopen dataset `{}`: {error}", path.display()),
        )
    })?;
    let mut reader = BufReader::new(input);
    let mut hasher = Sha256::new();
    let mut buffer = Vec::new();
    let mut line_number = 0_u64;
    let mut hashed_bytes = 0_u64;
    let mut records = 0_u64;
    let mut seen = HashSet::new();
    let mut writer = RecordWriter {
        file,
        hasher: Sha256::new(),
        bytes: 0,
        records: 0,
    };

    loop {
        let outcome = read_line_capped(
            &mut reader,
            &mut buffer,
            MAX_DATASET_LINE_BYTES,
            &mut hasher,
        )
        .map_err(io_failure)?;
        let (bytes, too_long) = match outcome {
            LineOutcome::Eof => break,
            LineOutcome::Line { bytes, too_long } => (bytes, too_long),
        };
        line_number += 1;
        hashed_bytes = hashed_bytes.saturating_add(bytes as u64);
        if too_long {
            return Err(format_error(format!(
                "line {line_number} is longer than {MAX_DATASET_LINE_BYTES} bytes"
            )));
        }
        let Some(record) = record_bytes(&buffer, line_number == 1) else {
            continue;
        };
        parse_split_record(record, line_number)?;
        if seen.insert(dedup_record_hash(record)) {
            writer.write_record(record, out)?;
        }
        records += 1;
    }
    if hashed_bytes != scan.file_bytes || hasher.finalize_hex() != scan.sha256 {
        return Err(format_error(
            "dataset changed while it was being deduplicated; no output was kept",
        ));
    }
    if records != scan.records || writer.records != scan.unique_records {
        return Err(format_error(format!(
            "dataset changed while it was being deduplicated: expected {} records, read {records}",
            scan.records
        )));
    }
    Ok(writer.finish(out))
}

fn dedup_record_hash(record: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(DEDUP_HASH_DOMAIN);
    hasher.update(record);
    hasher.finalize()
}

struct ShuffleScan {
    file_bytes: u64,
    sha256: String,
    blank_lines: u64,
    crlf_lines: u64,
    bom: bool,
    records: Vec<Vec<u8>>,
}

fn read_shuffle_records(
    path: &Path,
    schema: Option<&Json>,
    max_bytes: u64,
    max_records: usize,
) -> Result<ShuffleScan, AiError> {
    let file = File::open(path).map_err(|error| {
        AiError::new(
            AI_E_IO,
            format!("cannot open dataset `{}`: {error}", path.display()),
        )
    })?;
    let metadata = file.metadata().map_err(|error| {
        AiError::new(
            AI_E_IO,
            format!("cannot stat dataset `{}`: {error}", path.display()),
        )
    })?;
    if !metadata.is_file() {
        return Err(AiError::new(
            AI_E_IO,
            format!("cannot shuffle `{}`: not a regular file", path.display()),
        ));
    }
    if metadata.len() > max_bytes {
        return Err(format_error(format!(
            "JSONL dataset is {} bytes, above the {max_bytes} byte shuffle limit",
            metadata.len()
        )));
    }

    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buffer = Vec::new();
    let mut line_number = 0_u64;
    let mut hashed_bytes = 0_u64;
    let mut scan = ShuffleScan {
        file_bytes: metadata.len(),
        sha256: String::new(),
        blank_lines: 0,
        crlf_lines: 0,
        bom: false,
        records: Vec::new(),
    };

    loop {
        let outcome = read_line_capped(
            &mut reader,
            &mut buffer,
            MAX_DATASET_LINE_BYTES,
            &mut hasher,
        )
        .map_err(io_failure)?;
        let (bytes, too_long) = match outcome {
            LineOutcome::Eof => break,
            LineOutcome::Line { bytes, too_long } => (bytes, too_long),
        };
        line_number += 1;
        hashed_bytes = hashed_bytes.saturating_add(bytes as u64);
        if too_long {
            return Err(format_error(format!(
                "line {line_number} is longer than {MAX_DATASET_LINE_BYTES} bytes"
            )));
        }
        if line_number == 1 && buffer.starts_with(&[0xEF, 0xBB, 0xBF]) {
            scan.bom = true;
        }
        if buffer.ends_with(b"\r\n") {
            scan.crlf_lines += 1;
        }
        let Some(record) = record_bytes(&buffer, line_number == 1) else {
            scan.blank_lines += 1;
            continue;
        };
        let value = parse_split_record(record, line_number)?;
        if let Some(schema_value) = schema
            && let Err(error) = schema::validate(schema_value, &value)
        {
            return Err(format_error(format!(
                "line {line_number} does not satisfy the schema at {}: {}",
                error.path, error.message
            )));
        }
        if scan.records.len() >= max_records {
            return Err(format_error(format!(
                "dataset has more than {max_records} records; the shuffle limit was reached"
            )));
        }
        scan.records.push(record.to_vec());
    }
    if hashed_bytes != metadata.len() {
        return Err(format_error(format!(
            "dataset size changed while it was being read: expected {} bytes, read {hashed_bytes}",
            metadata.len()
        )));
    }
    scan.sha256 = hasher.finalize_hex();
    Ok(scan)
}

fn write_shuffle(
    file: File,
    out: &Path,
    seed: u64,
    scan: &ShuffleScan,
) -> Result<SplitPart, AiError> {
    let mut keyed: Vec<([u8; 32], &[u8])> = scan
        .records
        .iter()
        .map(|record| (shuffle_record_digest(seed, record), record.as_slice()))
        .collect();
    keyed.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(right.1)));

    let mut writer = RecordWriter {
        file,
        hasher: Sha256::new(),
        bytes: 0,
        records: 0,
    };
    for (_, record) in keyed {
        writer.write_record(record, out)?;
    }
    Ok(writer.finish(out))
}

fn shuffle_record_digest(seed: u64, record: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(SHUFFLE_HASH_DOMAIN);
    hasher.update(&seed.to_le_bytes());
    hasher.update(record);
    hasher.finalize()
}

fn split_inner(
    path: &Path,
    out_dir: &Path,
    train_ratio: f64,
    seed: u64,
    schema: Option<(&Path, &Json)>,
) -> Result<SplitSummary, AiError> {
    if !train_ratio.is_finite() || train_ratio <= 0.0 || train_ratio >= 1.0 {
        return Err(AiError::new(
            AI_E_INVALID_REQUEST,
            format!("train ratio must be greater than 0 and less than 1, got {train_ratio}"),
        ));
    }
    let cutoff = (train_ratio * (u32::MAX as f64 + 1.0))
        .floor()
        .min(u32::MAX as f64 + 1.0) as u64;

    std::fs::create_dir_all(out_dir).map_err(|error| {
        AiError::new(
            AI_E_IO,
            format!(
                "cannot create output directory `{}`: {error}",
                out_dir.display()
            ),
        )
    })?;
    if !out_dir.is_dir() {
        return Err(AiError::new(
            AI_E_IO,
            format!("output path `{}` is not a directory", out_dir.display()),
        ));
    }
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .ok_or_else(|| {
            AiError::new(
                AI_E_INVALID_REQUEST,
                format!(
                    "cannot derive output names from `{}`: the file name has no usable stem",
                    path.display()
                ),
            )
        })?;
    let train_path = out_dir.join(format!("{stem}.train.jsonl"));
    let held_out_path = out_dir.join(format!("{stem}.heldout.jsonl"));
    if train_path == held_out_path {
        return Err(AiError::new(
            AI_E_INVALID_REQUEST,
            "train and held-out output paths must differ",
        ));
    }
    if train_path.exists() || held_out_path.exists() {
        return Err(overwrite_error(&train_path, &held_out_path));
    }

    let scan = scan_split_input(path, seed, cutoff, schema.map(|(_, value)| value))?;
    if scan.records < 2 {
        return Err(AiError::new(
            AI_E_INVALID_REQUEST,
            format!(
                "dataset has {} record(s); at least 2 are required to create a train/held-out split",
                scan.records
            ),
        ));
    }
    let force = if scan.held_out_records == 0 {
        scan.max_record.map(|(ordinal, _)| (ordinal, false))
    } else if scan.train_records == 0 {
        scan.min_record.map(|(ordinal, _)| (ordinal, true))
    } else {
        None
    };

    let train_file = create_output(&train_path)?;
    let held_out_file = match OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&held_out_path)
    {
        Ok(file) => file,
        Err(error) => {
            let _ = std::fs::remove_file(&train_path);
            return Err(output_error(&held_out_path, error));
        }
    };

    let result = write_split(
        path,
        train_file,
        held_out_file,
        &train_path,
        &held_out_path,
        seed,
        cutoff,
        &scan,
        force,
    );
    match result {
        Ok((train, held_out)) => {
            let mut expected_train = scan.train_records;
            let mut expected_held_out = scan.held_out_records;
            if let Some((_, to_train)) = force {
                if to_train {
                    expected_train += 1;
                    expected_held_out -= 1;
                } else {
                    expected_held_out += 1;
                    expected_train -= 1;
                }
            }
            if train.records != expected_train || held_out.records != expected_held_out {
                let _ = std::fs::remove_file(&train_path);
                let _ = std::fs::remove_file(&held_out_path);
                return Err(AiError::new(
                    AI_E_IO,
                    "dataset changed while it was being split; no output was kept",
                ));
            }
            let mut warnings = Vec::new();
            if scan.blank_lines > 0 {
                warnings.push(format!(
                    "{} blank line(s) were not copied",
                    scan.blank_lines
                ));
            }
            if scan.crlf_lines > 0 {
                warnings.push(format!(
                    "{} CRLF line ending(s) were normalized to LF",
                    scan.crlf_lines
                ));
            }
            if scan.bom {
                warnings.push("the UTF-8 byte-order mark was removed".to_string());
            }
            if let Some((_, to_train)) = force {
                warnings.push(format!(
                    "one record was moved to the {} split to keep both splits non-empty",
                    if to_train { "train" } else { "held-out" }
                ));
            }
            Ok(SplitSummary {
                input_path: path.display().to_string(),
                input_sha256: scan.sha256,
                input_bytes: scan.file_bytes,
                record_count: scan.records,
                seed,
                train_ratio,
                train,
                held_out,
                warnings,
            })
        }
        Err(error) => {
            let _ = std::fs::remove_file(&train_path);
            let _ = std::fs::remove_file(&held_out_path);
            Err(error)
        }
    }
}

struct SplitScan {
    file_bytes: u64,
    sha256: String,
    records: u64,
    blank_lines: u64,
    crlf_lines: u64,
    bom: bool,
    train_records: u64,
    held_out_records: u64,
    min_record: Option<(u64, u64)>,
    max_record: Option<(u64, u64)>,
}

fn scan_split_input(
    path: &Path,
    seed: u64,
    cutoff: u64,
    schema: Option<&Json>,
) -> Result<SplitScan, AiError> {
    let file = File::open(path).map_err(|error| {
        AiError::new(
            AI_E_IO,
            format!("cannot open dataset `{}`: {error}", path.display()),
        )
    })?;
    let metadata = file.metadata().map_err(|error| {
        AiError::new(
            AI_E_IO,
            format!("cannot stat dataset `{}`: {error}", path.display()),
        )
    })?;
    if !metadata.is_file() {
        return Err(AiError::new(
            AI_E_IO,
            format!("cannot split `{}`: not a regular file", path.display()),
        ));
    }
    if metadata.len() > MAX_DATASET_BYTES {
        return Err(format_error(format!(
            "JSONL dataset is {} bytes, above the {MAX_DATASET_BYTES} byte limit",
            metadata.len()
        )));
    }

    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buffer = Vec::new();
    let mut line_number = 0_u64;
    let mut hashed_bytes = 0_u64;
    let mut scan = SplitScan {
        file_bytes: metadata.len(),
        sha256: String::new(),
        records: 0,
        blank_lines: 0,
        crlf_lines: 0,
        bom: false,
        train_records: 0,
        held_out_records: 0,
        min_record: None,
        max_record: None,
    };

    loop {
        let outcome = read_line_capped(
            &mut reader,
            &mut buffer,
            MAX_DATASET_LINE_BYTES,
            &mut hasher,
        )
        .map_err(io_failure)?;
        let (bytes, too_long) = match outcome {
            LineOutcome::Eof => break,
            LineOutcome::Line { bytes, too_long } => (bytes, too_long),
        };
        line_number += 1;
        hashed_bytes = hashed_bytes.saturating_add(bytes as u64);
        if too_long {
            return Err(format_error(format!(
                "line {line_number} is longer than {MAX_DATASET_LINE_BYTES} bytes"
            )));
        }
        if line_number == 1 && buffer.starts_with(&[0xEF, 0xBB, 0xBF]) {
            scan.bom = true;
        }
        if buffer.ends_with(b"\r\n") {
            scan.crlf_lines += 1;
        }
        let Some(record) = record_bytes(&buffer, line_number == 1) else {
            scan.blank_lines += 1;
            continue;
        };
        let value = parse_split_record(record, line_number)?;
        if let Some(schema_value) = schema
            && let Err(error) = schema::validate(schema_value, &value)
        {
            return Err(format_error(format!(
                "line {line_number} does not satisfy the schema at {}: {}",
                error.path, error.message
            )));
        }
        let ordinal = scan.records;
        let hash = split_record_hash(seed, record);
        if (hash >> 32) < cutoff {
            scan.train_records += 1;
        } else {
            scan.held_out_records += 1;
        }
        match scan.min_record {
            None => scan.min_record = Some((ordinal, hash)),
            Some((_, current)) if hash < current => scan.min_record = Some((ordinal, hash)),
            _ => {}
        }
        match scan.max_record {
            None => scan.max_record = Some((ordinal, hash)),
            Some((_, current)) if hash > current => scan.max_record = Some((ordinal, hash)),
            _ => {}
        }
        scan.records += 1;
    }
    if hashed_bytes != metadata.len() {
        return Err(format_error(format!(
            "dataset size changed while it was being read: expected {} bytes, read {hashed_bytes}",
            metadata.len()
        )));
    }
    scan.sha256 = hasher.finalize_hex();
    Ok(scan)
}

struct RecordWriter {
    file: File,
    hasher: Sha256,
    bytes: u64,
    records: u64,
}

impl RecordWriter {
    fn write_record(&mut self, record: &[u8], path: &Path) -> Result<(), AiError> {
        self.file
            .write_all(record)
            .and_then(|_| self.file.write_all(b"\n"))
            .map_err(|error| {
                AiError::new(
                    AI_E_IO,
                    format!("cannot write `{}`: {error}", path.display()),
                )
            })?;
        self.hasher.update(record);
        self.hasher.update(b"\n");
        self.bytes += record.len() as u64 + 1;
        self.records += 1;
        Ok(())
    }

    fn finish(self, path: &Path) -> SplitPart {
        SplitPart {
            path: path.display().to_string(),
            records: self.records,
            bytes: self.bytes,
            sha256: self.hasher.finalize_hex(),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn write_split(
    path: &Path,
    train_file: File,
    held_out_file: File,
    train_path: &Path,
    held_out_path: &Path,
    seed: u64,
    cutoff: u64,
    scan: &SplitScan,
    force: Option<(u64, bool)>,
) -> Result<(SplitPart, SplitPart), AiError> {
    let file = File::open(path).map_err(|error| {
        AiError::new(
            AI_E_IO,
            format!("cannot reopen dataset `{}`: {error}", path.display()),
        )
    })?;
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buffer = Vec::new();
    let mut line_number = 0_u64;
    let mut hashed_bytes = 0_u64;
    let mut records = 0_u64;
    let mut train = RecordWriter {
        file: train_file,
        hasher: Sha256::new(),
        bytes: 0,
        records: 0,
    };
    let mut held_out = RecordWriter {
        file: held_out_file,
        hasher: Sha256::new(),
        bytes: 0,
        records: 0,
    };

    loop {
        let outcome = read_line_capped(
            &mut reader,
            &mut buffer,
            MAX_DATASET_LINE_BYTES,
            &mut hasher,
        )
        .map_err(io_failure)?;
        let (bytes, too_long) = match outcome {
            LineOutcome::Eof => break,
            LineOutcome::Line { bytes, too_long } => (bytes, too_long),
        };
        line_number += 1;
        hashed_bytes = hashed_bytes.saturating_add(bytes as u64);
        if too_long {
            return Err(format_error(format!(
                "line {line_number} is longer than {MAX_DATASET_LINE_BYTES} bytes"
            )));
        }
        let Some(record) = record_bytes(&buffer, line_number == 1) else {
            continue;
        };
        parse_split_record(record, line_number)?;
        let ordinal = records;
        let is_train = match force {
            Some((forced, to_train)) if forced == ordinal => to_train,
            _ => (split_record_hash(seed, record) >> 32) < cutoff,
        };
        if is_train {
            train.write_record(record, train_path)?;
        } else {
            held_out.write_record(record, held_out_path)?;
        }
        records += 1;
    }
    if hashed_bytes != scan.file_bytes || hasher.finalize_hex() != scan.sha256 {
        return Err(format_error(
            "dataset changed while it was being split; no output was kept",
        ));
    }
    if records != scan.records {
        return Err(format_error(format!(
            "dataset changed while it was being split: expected {} records, read {records}",
            scan.records
        )));
    }
    Ok((train.finish(train_path), held_out.finish(held_out_path)))
}

fn split_record_hash(seed: u64, record: &[u8]) -> u64 {
    let mut hasher = Sha256::new();
    hasher.update(SPLIT_HASH_DOMAIN);
    hasher.update(&seed.to_le_bytes());
    hasher.update(record);
    let digest = hasher.finalize();
    u64::from_be_bytes(digest[..8].try_into().expect("SHA-256 digest is 32 bytes"))
}

fn parse_split_record(record: &[u8], line: u64) -> Result<Json, AiError> {
    let text = std::str::from_utf8(record)
        .map_err(|_| format_error(format!("line {line} is not valid UTF-8")))?;
    let value = json::parse(text)
        .map_err(|error| format_error(format!("line {line}: invalid JSON: {error}")))?;
    if value.as_object().is_none() {
        return Err(format_error(format!(
            "line {line}: record is a JSON {}, not an object",
            json_type_name(&value)
        )));
    }
    Ok(value)
}

fn create_output(train_path: &Path) -> Result<File, AiError> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(train_path)
        .map_err(|error| output_error(train_path, error))
}

fn output_error(path: &Path, error: io::Error) -> AiError {
    if error.kind() == io::ErrorKind::AlreadyExists {
        AiError::new(
            AI_E_IO,
            format!(
                "refusing to overwrite `{}`; choose another output directory or remove the file",
                path.display()
            ),
        )
    } else {
        AiError::new(
            AI_E_IO,
            format!("cannot create `{}`: {error}", path.display()),
        )
    }
}

fn overwrite_error(train_path: &Path, held_out_path: &Path) -> AiError {
    AiError::new(
        AI_E_IO,
        format!(
            "refusing to overwrite `{}` or `{}`; choose another output directory or remove the files",
            train_path.display(),
            held_out_path.display()
        ),
    )
}

fn inspect_with_limits(
    path: &Path,
    max_bytes: u64,
    max_line_bytes: usize,
    max_fields: usize,
) -> Result<DatasetSummary, AiError> {
    inspect_inner(path, None, max_bytes, max_line_bytes, max_fields)
}

fn inspect_inner(
    path: &Path,
    schema: Option<(&Path, &Json)>,
    max_bytes: u64,
    max_line_bytes: usize,
    max_fields: usize,
) -> Result<DatasetSummary, AiError> {
    let file = File::open(path).map_err(|error| {
        AiError::new(
            AI_E_IO,
            format!("cannot open dataset `{}`: {error}", path.display()),
        )
    })?;
    let metadata = file.metadata().map_err(|error| {
        AiError::new(
            AI_E_IO,
            format!("cannot stat dataset `{}`: {error}", path.display()),
        )
    })?;
    if !metadata.is_file() {
        return Err(AiError::new(
            AI_E_IO,
            format!("cannot inspect `{}`: not a regular file", path.display()),
        ));
    }
    let file_bytes = metadata.len();
    if file_bytes > max_bytes {
        return Err(format_error(format!(
            "JSONL dataset is {file_bytes} bytes, above the {max_bytes} byte limit"
        )));
    }

    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buffer = Vec::new();
    let mut hashed_bytes = 0_u64;
    let mut line_count = 0_u64;
    let mut blank_lines = 0_u64;
    let mut record_count = 0_u64;
    let mut invalid_line_count = 0_u64;
    let mut invalid_lines = Vec::new();
    let mut schema_invalid_count = 0_u64;
    let mut schema_violations = Vec::new();
    let mut fields: BTreeMap<String, FieldStats> = BTreeMap::new();

    loop {
        let outcome = read_line_capped(&mut reader, &mut buffer, max_line_bytes, &mut hasher)
            .map_err(io_failure)?;
        let (bytes, too_long) = match outcome {
            LineOutcome::Eof => break,
            LineOutcome::Line { bytes, too_long } => (bytes, too_long),
        };
        hashed_bytes = hashed_bytes.saturating_add(bytes as u64);
        line_count += 1;

        if too_long {
            record_invalid(
                &mut invalid_lines,
                &mut invalid_line_count,
                line_count,
                format!("line is longer than {max_line_bytes} bytes"),
            );
            continue;
        }

        let Some(text) = record_bytes(&buffer, line_count == 1) else {
            blank_lines += 1;
            continue;
        };

        let text = match std::str::from_utf8(text) {
            Ok(text) => text,
            Err(_) => {
                record_invalid(
                    &mut invalid_lines,
                    &mut invalid_line_count,
                    line_count,
                    "line is not valid UTF-8".to_string(),
                );
                continue;
            }
        };
        let value = match json::parse(text) {
            Ok(value) => value,
            Err(error) => {
                record_invalid(
                    &mut invalid_lines,
                    &mut invalid_line_count,
                    line_count,
                    format!("invalid JSON: {error}"),
                );
                continue;
            }
        };
        let Some(entries) = value.as_object() else {
            record_invalid(
                &mut invalid_lines,
                &mut invalid_line_count,
                line_count,
                format!("record is a JSON {}, not an object", json_type_name(&value)),
            );
            continue;
        };

        record_count += 1;
        if let Some((_, schema_value)) = schema
            && let Err(error) = schema::validate(schema_value, &value)
        {
            schema_invalid_count += 1;
            if schema_violations.len() < MAX_REPORTED_SCHEMA_VIOLATIONS {
                schema_violations.push(SchemaViolation {
                    line: line_count,
                    path: error.path,
                    message: error.message,
                });
            }
        }
        for (name, value) in entries {
            if !fields.contains_key(name) && fields.len() >= max_fields {
                return Err(format_error(format!(
                    "dataset has more than {max_fields} distinct top-level fields"
                )));
            }
            let stats = fields.entry(name.clone()).or_default();
            stats.records += 1;
            *stats.types.entry(json_type_name(value)).or_insert(0) += 1;
        }
    }

    let sha256 = hasher.finalize_hex();
    let mut warnings = Vec::new();
    if hashed_bytes != file_bytes {
        warnings.push(format!(
            "file size changed during inspection: expected {file_bytes} bytes, hashed {hashed_bytes}"
        ));
    }
    if invalid_line_count > MAX_REPORTED_INVALID_LINES as u64 {
        warnings.push(format!(
            "{} invalid line(s) are not listed",
            invalid_line_count - MAX_REPORTED_INVALID_LINES as u64
        ));
    }
    if schema_invalid_count > MAX_REPORTED_SCHEMA_VIOLATIONS as u64 {
        warnings.push(format!(
            "{} schema violation(s) are not listed",
            schema_invalid_count - MAX_REPORTED_SCHEMA_VIOLATIONS as u64
        ));
    }

    let fields = fields
        .into_iter()
        .map(|(name, stats)| FieldSummary {
            name,
            records: stats.records,
            types: stats
                .types
                .into_iter()
                .map(|(kind, count)| TypeCount {
                    kind: kind.to_string(),
                    count,
                })
                .collect(),
        })
        .collect();

    Ok(DatasetSummary {
        path: path.display().to_string(),
        file_bytes: hashed_bytes,
        sha256,
        line_count,
        blank_lines,
        record_count,
        invalid_line_count,
        invalid_lines,
        fields,
        schema: schema.map(|(schema_path, _)| SchemaSummary {
            path: schema_path.display().to_string(),
            invalid_record_count: schema_invalid_count,
            violations: schema_violations,
        }),
        warnings,
    })
}

fn load_schema(path: &Path) -> Result<Json, AiError> {
    let file = File::open(path).map_err(|error| {
        AiError::new(
            AI_E_IO,
            format!("cannot open schema `{}`: {error}", path.display()),
        )
    })?;
    let metadata = file.metadata().map_err(|error| {
        AiError::new(
            AI_E_IO,
            format!("cannot stat schema `{}`: {error}", path.display()),
        )
    })?;
    if !metadata.is_file() {
        return Err(AiError::new(
            AI_E_IO,
            format!(
                "cannot inspect schema `{}`: not a regular file",
                path.display()
            ),
        ));
    }
    if metadata.len() > MAX_SCHEMA_BYTES {
        return Err(format_error(format!(
            "JSON Schema is {} bytes, above the {MAX_SCHEMA_BYTES} byte limit",
            metadata.len()
        )));
    }
    let mut bytes = Vec::new();
    file.take(MAX_SCHEMA_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| {
            AiError::new(
                AI_E_IO,
                format!("cannot read schema `{}`: {error}", path.display()),
            )
        })?;
    if bytes.len() as u64 > MAX_SCHEMA_BYTES {
        return Err(format_error(format!(
            "JSON Schema is larger than the {MAX_SCHEMA_BYTES} byte limit"
        )));
    }
    let mut text = bytes.as_slice();
    if text.starts_with(&[0xEF, 0xBB, 0xBF]) {
        text = &text[3..];
    }
    let text =
        std::str::from_utf8(text).map_err(|_| format_error("JSON Schema is not valid UTF-8"))?;
    let schema =
        json::parse(text).map_err(|error| format_error(format!("invalid JSON Schema: {error}")))?;
    schema::check_schema(&schema)
        .map_err(|error| format_error(format!("invalid JSON Schema: {error}")))?;
    Ok(schema)
}

#[derive(Default)]
struct FieldStats {
    records: u64,
    types: BTreeMap<&'static str, u64>,
}

enum LineOutcome {
    Eof,
    Line { bytes: usize, too_long: bool },
}

/// Reads one line into `buffer`, hashing every byte, while keeping memory
/// bounded by `limit`. The rest of an oversized line is consumed and dropped.
fn read_line_capped(
    reader: &mut impl BufRead,
    buffer: &mut Vec<u8>,
    limit: usize,
    hasher: &mut Sha256,
) -> io::Result<LineOutcome> {
    buffer.clear();
    let mut total = 0_usize;
    let mut too_long = false;
    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            return Ok(if total == 0 {
                LineOutcome::Eof
            } else {
                LineOutcome::Line {
                    bytes: total,
                    too_long,
                }
            });
        }
        let newline = available.iter().position(|&byte| byte == b'\n');
        let take = newline.map_or(available.len(), |index| index + 1);
        hasher.update(&available[..take]);
        total += take;
        if !too_long {
            if buffer.len() + take > limit {
                too_long = true;
                buffer.clear();
            } else {
                buffer.extend_from_slice(&available[..take]);
            }
        }
        reader.consume(take);
        if newline.is_some() {
            return Ok(LineOutcome::Line {
                bytes: total,
                too_long,
            });
        }
    }
}

/// Returns one line's record bytes after stripping the newline, an optional
/// trailing `\r`, and a first-line UTF-8 BOM. `None` means the line is blank.
fn record_bytes(line: &[u8], first_line: bool) -> Option<&[u8]> {
    let mut text = line;
    if text.last() == Some(&b'\n') {
        text = &text[..text.len() - 1];
    }
    if text.last() == Some(&b'\r') {
        text = &text[..text.len() - 1];
    }
    if first_line && text.starts_with(&[0xEF, 0xBB, 0xBF]) {
        text = &text[3..];
    }
    if text.iter().all(u8::is_ascii_whitespace) {
        None
    } else {
        Some(text)
    }
}

fn record_invalid(
    invalid_lines: &mut Vec<InvalidLine>,
    invalid_line_count: &mut u64,
    line: u64,
    message: String,
) {
    *invalid_line_count += 1;
    if invalid_lines.len() < MAX_REPORTED_INVALID_LINES {
        invalid_lines.push(InvalidLine { line, message });
    }
}

fn json_type_name(value: &Json) -> &'static str {
    match value {
        Json::Null => "null",
        Json::Bool(_) => "bool",
        Json::Number(_) => "number",
        Json::String(_) => "string",
        Json::Array(_) => "array",
        Json::Object(_) => "object",
    }
}

fn format_error(message: impl Into<String>) -> AiError {
    AiError::new(AI_E_FORMAT, message)
}

fn io_failure(error: io::Error) -> AiError {
    AiError::new(AI_E_IO, format!("cannot read dataset file: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_file(name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "vantyx-dataset-{}-{}-{name}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&path, bytes).expect("write dataset fixture");
        path
    }

    #[test]
    fn inspects_valid_dataset_with_bom_crlf_and_blank_lines() {
        let text = "\u{feff}{\"text\":\"hello\",\"label\":1}\r\n\n{\"text\":\"world\",\"label\":2}\n{\"text\":\"third\",\"label\":null}\n";
        let path = temp_file("valid.jsonl", text.as_bytes());
        let summary = inspect(&path).expect("inspect dataset");

        assert!(summary.is_valid());
        assert_eq!(summary.line_count, 4);
        assert_eq!(summary.blank_lines, 1);
        assert_eq!(summary.record_count, 3);
        assert_eq!(summary.invalid_line_count, 0);
        assert_eq!(summary.invalid_lines.len(), 0);
        assert_eq!(summary.file_bytes, text.len() as u64);
        let mut hasher = Sha256::new();
        hasher.update(text.as_bytes());
        assert_eq!(summary.sha256, hasher.finalize_hex());
        assert_eq!(
            summary.fields,
            vec![
                FieldSummary {
                    name: "label".to_string(),
                    records: 3,
                    types: vec![
                        TypeCount {
                            kind: "null".to_string(),
                            count: 1,
                        },
                        TypeCount {
                            kind: "number".to_string(),
                            count: 2,
                        },
                    ],
                },
                FieldSummary {
                    name: "text".to_string(),
                    records: 3,
                    types: vec![TypeCount {
                        kind: "string".to_string(),
                        count: 3,
                    }],
                },
            ]
        );

        let json = summary.to_json();
        assert_eq!(
            json.get("format").and_then(Json::as_str),
            Some("jsonl-dataset")
        );
        assert_eq!(json.get("record_count").and_then(Json::as_u64), Some(3));
        assert_eq!(
            json.get("fields").and_then(Json::as_array).map(|f| f.len()),
            Some(2)
        );
        let reparsed = json::parse(&json.to_compact_string()).expect("dataset JSON is valid");
        assert_eq!(reparsed.get("sha256"), json.get("sha256"));

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn reports_invalid_json_utf8_and_non_objects() {
        let mut bytes = b"{\"a\":1}\nnot json\n[1,2]\n".to_vec();
        bytes.extend([b'{', b'"', 0xFF_u8, b'"', b':', b'1', b'}', b'\n']);
        let path = temp_file("invalid.jsonl", &bytes);
        let summary = inspect(&path).expect("inspect dataset");

        assert!(!summary.is_valid());
        assert_eq!(summary.line_count, 4);
        assert_eq!(summary.record_count, 1);
        assert_eq!(summary.invalid_line_count, 3);
        assert_eq!(summary.invalid_lines.len(), 3);
        assert_eq!(summary.invalid_lines[0].line, 2);
        assert!(summary.invalid_lines[0].message.contains("invalid JSON"));
        assert_eq!(summary.invalid_lines[1].line, 3);
        assert!(
            summary.invalid_lines[1].message.contains("not an object"),
            "{}",
            summary.invalid_lines[1].message
        );
        assert_eq!(summary.invalid_lines[2].line, 4);
        assert!(
            summary.invalid_lines[2].message.contains("not valid UTF-8"),
            "{}",
            summary.invalid_lines[2].message
        );

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn caps_reported_invalid_lines_but_counts_all() {
        let text = (1..=25)
            .map(|line| format!("invalid {line}\n"))
            .collect::<String>();
        let path = temp_file("many-invalid.jsonl", text.as_bytes());
        let summary = inspect(&path).expect("inspect dataset");

        assert_eq!(summary.line_count, 25);
        assert_eq!(summary.invalid_line_count, 25);
        assert_eq!(summary.invalid_lines.len(), MAX_REPORTED_INVALID_LINES);
        assert!(
            summary
                .warnings
                .iter()
                .any(|warning| warning.contains("5 invalid line(s) are not listed")),
            "{:?}",
            summary.warnings
        );

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn empty_dataset_is_valid() {
        let path = temp_file("empty.jsonl", b"");
        let summary = inspect(&path).expect("inspect empty dataset");
        assert!(summary.is_valid());
        assert_eq!(summary.line_count, 0);
        assert_eq!(summary.record_count, 0);
        assert_eq!(summary.file_bytes, 0);
        assert_eq!(
            summary.sha256,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn enforces_byte_line_and_field_limits() {
        let path = temp_file("limits.jsonl", b"{\"a\":1,\"b\":2}\n");
        let error = inspect_with_limits(&path, 4, 1024, 10).expect_err("byte limit must fail");
        assert_eq!(error.code, AI_E_FORMAT);
        assert!(error.message.contains("above the 4 byte limit"));

        let error = inspect_with_limits(&path, 1024, 1024, 1).expect_err("field limit must fail");
        assert_eq!(error.code, AI_E_FORMAT);
        assert!(
            error
                .message
                .contains("more than 1 distinct top-level fields")
        );

        let summary = inspect_with_limits(&path, 1024, 1024, 10).expect("normal limits");
        assert_eq!(summary.record_count, 1);
        assert_eq!(summary.fields.len(), 2);

        let path = temp_file("long-line.jsonl", b"{\"a\":1}\n");
        let summary = inspect_with_limits(&path, 1024, 7, 10).expect("inspect long line");
        assert_eq!(summary.line_count, 1);
        assert_eq!(summary.invalid_line_count, 1);
        assert_eq!(summary.record_count, 0);
        assert!(
            summary.invalid_lines[0]
                .message
                .contains("longer than 7 bytes"),
            "{}",
            summary.invalid_lines[0].message
        );

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn validates_records_against_a_schema() {
        let dataset = temp_file(
            "schema-records.jsonl",
            b"{\"text\":\"a\",\"label\":\"x\"}\n{\"text\":\"b\"}\n{\"label\":\"y\"}\n",
        );
        let schema_path = temp_file(
            "schema-object.json",
            br#"{"type":"object","required":["text","label"],"properties":{"text":{"type":"string"},"label":{"type":"string"}},"additionalProperties":false}"#,
        );
        let summary = inspect_with_schema(&dataset, &schema_path).expect("inspect with schema");

        assert_eq!(summary.record_count, 3);
        assert_eq!(summary.invalid_line_count, 0);
        let schema = summary.schema.as_ref().expect("schema summary");
        assert_eq!(schema.path, schema_path.display().to_string());
        assert_eq!(schema.invalid_record_count, 2);
        assert_eq!(schema.violations.len(), 2);
        assert_eq!(schema.violations[0].line, 2);
        assert_eq!(schema.violations[0].path, "$.label");
        assert!(schema.violations[0].message.contains("required"));
        assert!(!summary.is_valid());

        let json = summary.to_json();
        let schema_json = json.get("schema").expect("schema JSON");
        assert_eq!(
            schema_json
                .get("invalid_record_count")
                .and_then(Json::as_u64),
            Some(2)
        );
        assert_eq!(
            schema_json
                .get("violations")
                .and_then(Json::as_array)
                .map(|violations| violations.len()),
            Some(2)
        );

        let valid_path = temp_file("schema-valid.jsonl", b"{\"text\":\"a\",\"label\":\"x\"}\n");
        let summary =
            inspect_with_schema(&valid_path, &schema_path).expect("inspect valid dataset");
        assert!(summary.is_valid());
        assert_eq!(
            summary
                .schema
                .as_ref()
                .map(|schema| schema.invalid_record_count),
            Some(0)
        );

        let _ = std::fs::remove_file(dataset);
        let _ = std::fs::remove_file(schema_path);
        let _ = std::fs::remove_file(valid_path);
    }

    #[test]
    fn caps_reported_schema_violations() {
        let text = (1..=25)
            .map(|line| format!("{{\"n\":{line}}}\n"))
            .collect::<String>();
        let dataset = temp_file("schema-many.jsonl", text.as_bytes());
        let schema_path = temp_file(
            "schema-many.json",
            br#"{"type":"object","required":["missing"]}"#,
        );
        let summary = inspect_with_schema(&dataset, &schema_path).expect("inspect with schema");

        assert_eq!(summary.record_count, 25);
        let schema = summary.schema.as_ref().expect("schema summary");
        assert_eq!(schema.invalid_record_count, 25);
        assert_eq!(schema.violations.len(), MAX_REPORTED_SCHEMA_VIOLATIONS);
        assert!(
            summary
                .warnings
                .iter()
                .any(|warning| warning.contains("5 schema violation(s) are not listed")),
            "{:?}",
            summary.warnings
        );

        let _ = std::fs::remove_file(dataset);
        let _ = std::fs::remove_file(schema_path);
    }

    #[test]
    fn schema_validation_skips_structurally_invalid_lines() {
        let dataset = temp_file(
            "schema-invalid-line.jsonl",
            b"not json\n{\"text\":\"ok\"}\n",
        );
        let schema_path = temp_file(
            "schema-text.json",
            br#"{"type":"object","required":["text"],"properties":{"text":{"type":"string"}}}"#,
        );
        let summary = inspect_with_schema(&dataset, &schema_path).expect("inspect with schema");

        assert_eq!(summary.invalid_line_count, 1);
        assert_eq!(summary.record_count, 1);
        assert_eq!(
            summary
                .schema
                .as_ref()
                .map(|schema| schema.invalid_record_count),
            Some(0)
        );

        let _ = std::fs::remove_file(dataset);
        let _ = std::fs::remove_file(schema_path);
    }

    #[test]
    fn rejects_malformed_or_unreadable_schemas() {
        let dataset = temp_file("schema-dataset.jsonl", b"{\"a\":1}\n");
        let cases: [(&str, &[u8]); 4] = [
            ("bad-json", b"{"),
            ("not-object", b"[]"),
            ("bad-required", br#"{"required":"a"}"#),
            ("bad-type", br#"{"type":"wat"}"#),
        ];
        for (name, bytes) in cases {
            let schema_path = temp_file(name, bytes);
            let error = inspect_with_schema(&dataset, &schema_path)
                .expect_err("malformed schema must fail");
            assert_eq!(error.code, AI_E_FORMAT);
            assert!(
                error.message.contains("invalid JSON Schema"),
                "{}",
                error.message
            );
            let _ = std::fs::remove_file(schema_path);
        }

        let missing = std::env::temp_dir().join(format!(
            "vantyx-dataset-missing-schema-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let error = inspect_with_schema(&dataset, &missing).expect_err("missing schema must fail");
        assert_eq!(error.code, AI_E_IO);

        let directory = std::env::temp_dir().join(format!(
            "vantyx-dataset-schema-dir-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&directory).expect("create schema directory");
        let error =
            inspect_with_schema(&dataset, &directory).expect_err("directory is not a schema");
        assert_eq!(error.code, AI_E_IO);

        let _ = std::fs::remove_dir_all(directory);
        let _ = std::fs::remove_file(dataset);
    }

    #[test]
    fn schema_file_accepts_a_utf8_bom() {
        let dataset = temp_file("schema-bom.jsonl", b"{\"text\":\"ok\"}\n");
        let schema_path = temp_file(
            "schema-bom.json",
            b"\xEF\xBB\xBF{\"type\":\"object\",\"required\":[\"text\"]}",
        );
        let summary = inspect_with_schema(&dataset, &schema_path).expect("inspect with BOM schema");
        assert!(summary.is_valid());

        let _ = std::fs::remove_file(dataset);
        let _ = std::fs::remove_file(schema_path);
    }

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "vantyx-dataset-dir-{}-{}-{name}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).expect("create dataset directory");
        path
    }

    #[test]
    fn split_is_deterministic_and_partitions_records() {
        let directory = temp_dir("split-deterministic");
        let input = directory.join("corpus.jsonl");
        let mut text = String::from("\u{feff}");
        let mut expected = Vec::new();
        for index in 0..40 {
            let line = format!("{{\"id\":{index},\"text\":\"row {index}\"}}");
            expected.push(line.clone());
            text.push_str(&line);
            if index < 10 {
                text.push_str("\r\n");
            } else {
                text.push('\n');
            }
            if index == 9 {
                text.push('\n');
            }
        }
        std::fs::write(&input, &text).expect("write dataset");

        let first = split(&input, &directory.join("first"), 0.75, 42).expect("first split");
        let second = split(&input, &directory.join("second"), 0.75, 42).expect("second split");
        assert_eq!(first.record_count, 40);
        assert_eq!(first.train.records + first.held_out.records, 40);
        assert!(first.train.records > 0);
        assert!(first.held_out.records > 0);
        assert_eq!(first.input_sha256, second.input_sha256);
        assert_eq!(first.train.sha256, second.train.sha256);
        assert_eq!(first.held_out.sha256, second.held_out.sha256);
        assert_eq!(first.train.records, second.train.records);

        let mut actual = Vec::new();
        for part in [&first.train, &first.held_out] {
            let body = std::fs::read_to_string(&part.path).expect("read split part");
            assert!(!body.contains('\r'), "output line endings are LF");
            for line in body.lines() {
                actual.push(line.to_string());
            }
        }
        actual.sort();
        expected.sort();
        assert_eq!(actual, expected);
        assert!(first.warnings.iter().any(|w| w.contains("blank line")));
        assert!(first.warnings.iter().any(|w| w.contains("CRLF")));
        assert!(first.warnings.iter().any(|w| w.contains("byte-order mark")));
        assert_eq!(
            std::fs::read_to_string(&input).expect("reread input"),
            text,
            "the input dataset must not be modified"
        );
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn split_seed_changes_assignment_and_rejects_bad_ratios() {
        let directory = temp_dir("split-seed");
        let input = directory.join("corpus.jsonl");
        let text: String = (0..100)
            .map(|index| format!("{{\"id\":{index}}}\n"))
            .collect();
        std::fs::write(&input, &text).expect("write dataset");

        let first = split(&input, &directory.join("first"), 0.8, 1).expect("seed 1");
        let second = split(&input, &directory.join("second"), 0.8, 2).expect("seed 2");
        assert_eq!(first.record_count, 100);
        assert_ne!(first.held_out.sha256, second.held_out.sha256);
        assert_ne!(first.train.sha256, second.train.sha256);

        for ratio in [0.0, 1.0, -0.5, f64::NAN, f64::INFINITY] {
            let error = split(&input, &directory.join("bad"), ratio, 1)
                .expect_err("invalid ratio must fail");
            assert_eq!(error.code, AI_E_INVALID_REQUEST);
        }
        assert!(!directory.join("bad").exists());
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn split_requires_two_records_and_refuses_overwrite() {
        let directory = temp_dir("split-overwrite");
        let one = directory.join("one.jsonl");
        std::fs::write(&one, "{\"a\":1}\n").expect("write one record");
        let error =
            split(&one, &directory.join("out-one"), 0.5, 0).expect_err("one record must fail");
        assert_eq!(error.code, AI_E_INVALID_REQUEST);
        assert!(error.message.contains("at least 2"), "{}", error.message);

        let two = directory.join("two.jsonl");
        std::fs::write(&two, "{\"a\":1}\n{\"a\":2}\n").expect("write two records");
        let out = directory.join("out-two");
        let first = split(&two, &out, 0.5, 0).expect("two records split");
        assert_eq!(first.train.records + first.held_out.records, 2);
        let before = std::fs::read_to_string(&first.train.path).expect("read train output");
        let error = split(&two, &out, 0.5, 0).expect_err("overwrite must fail");
        assert_eq!(error.code, AI_E_IO);
        assert!(
            error.message.contains("refusing to overwrite"),
            "{}",
            error.message
        );
        assert_eq!(
            std::fs::read_to_string(&first.train.path).expect("reread train output"),
            before
        );

        let file_as_dir = directory.join("not-a-dir");
        std::fs::write(&file_as_dir, "x").expect("write file");
        let error = split(&two, &file_as_dir, 0.5, 0).expect_err("file as output dir must fail");
        assert_eq!(error.code, AI_E_IO);
        assert!(
            error.message.contains("not a directory") || error.message.contains("cannot create"),
            "{}",
            error.message
        );
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn split_rejects_invalid_lines_before_writing() {
        let directory = temp_dir("split-invalid");
        let input = directory.join("bad.jsonl");
        let out = directory.join("out");
        std::fs::write(&input, "{\"a\":1}\nnot json\n").expect("write dataset");
        let error = split(&input, &out, 0.5, 0).expect_err("invalid JSON must fail");
        assert_eq!(error.code, AI_E_FORMAT);
        assert!(error.message.contains("line 2"), "{}", error.message);
        assert_eq!(std::fs::read_dir(&out).expect("read output dir").count(), 0);

        std::fs::write(&input, "[1,2]\n{\"a\":1}\n").expect("write non-object dataset");
        let error = split(&input, &out, 0.5, 0).expect_err("non-object must fail");
        assert_eq!(error.code, AI_E_FORMAT);
        assert!(error.message.contains("not an object"), "{}", error.message);
        assert_eq!(std::fs::read_dir(&out).expect("read output dir").count(), 0);
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn split_schema_gate_and_non_empty_fallback() {
        let directory = temp_dir("split-schema-fallback");
        let input = directory.join("data.jsonl");
        std::fs::write(&input, "{\"text\":\"a\"}\n{\"other\":1}\n").expect("write dataset");
        let schema = directory.join("schema.json");
        std::fs::write(&schema, r#"{"type":"object","required":["text"]}"#).expect("write schema");
        let out = directory.join("schema-out");
        let error = split_with_schema(&input, &out, 0.5, 0, &schema)
            .expect_err("schema violation must fail");
        assert_eq!(error.code, AI_E_FORMAT);
        assert!(
            error.message.contains("does not satisfy the schema"),
            "{}",
            error.message
        );
        assert_eq!(std::fs::read_dir(&out).expect("read output dir").count(), 0);

        std::fs::write(&input, "{\"a\":1}\n{\"a\":1}\n").expect("write identical records");
        let high = split(&input, &directory.join("high"), 0.999999, 0).expect("high ratio");
        assert!(high.train.records > 0);
        assert!(high.held_out.records > 0);
        assert!(high.warnings.iter().any(|w| w.contains("moved to the")));
        let low = split(&input, &directory.join("low"), 0.000001, 0).expect("low ratio");
        assert!(low.train.records > 0);
        assert!(low.held_out.records > 0);
        assert!(low.warnings.iter().any(|w| w.contains("moved to the")));
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn dedup_keeps_first_occurrence_and_normalizes() {
        let directory = temp_dir("dedup-basic");
        let input = directory.join("corpus.jsonl");
        let original = b"\xEF\xBB\xBF{\"a\":1}\r\n{\"a\":1}\n\n{\"b\":2}\n{\"a\":1}\n";
        std::fs::write(&input, original).expect("write dataset");
        let out = directory.join("unique.jsonl");
        let summary = deduplicate(&input, &out).expect("deduplicate dataset");

        assert_eq!(summary.record_count, 4);
        assert_eq!(summary.unique_records, 2);
        assert_eq!(summary.duplicates_removed, 2);
        assert_eq!(summary.input_bytes, original.len() as u64);
        assert_eq!(summary.output_bytes, 16);
        assert_eq!(
            std::fs::read_to_string(&out).expect("read output"),
            "{\"a\":1}\n{\"b\":2}\n"
        );
        assert_eq!(std::fs::read(&input).expect("read input"), original);
        assert!(summary.warnings.iter().any(|w| w.contains("blank line")));
        assert!(summary.warnings.iter().any(|w| w.contains("CRLF")));
        assert!(
            summary
                .warnings
                .iter()
                .any(|w| w.contains("byte-order mark"))
        );
        let mut hasher = Sha256::new();
        hasher.update(b"{\"a\":1}\n{\"b\":2}\n");
        assert_eq!(summary.output_sha256, hasher.finalize_hex());

        let json = summary.to_json();
        assert_eq!(
            json.get("format").and_then(Json::as_str),
            Some("jsonl-dedup")
        );
        assert_eq!(
            json.get("duplicates_removed").and_then(Json::as_u64),
            Some(2)
        );
        let reparsed = json::parse(&json.to_compact_string()).expect("dedup JSON is valid");
        assert_eq!(
            reparsed.get("unique_records").and_then(Json::as_u64),
            json.get("unique_records").and_then(Json::as_u64)
        );

        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn dedup_rejects_invalid_input_and_refuses_overwrite() {
        let directory = temp_dir("dedup-errors");
        let input = directory.join("data.jsonl");
        let out = directory.join("out.jsonl");

        std::fs::write(&input, "{\"a\":1}\nnot json\n").expect("write invalid dataset");
        let error = deduplicate(&input, &out).expect_err("invalid JSON must fail");
        assert_eq!(error.code, AI_E_FORMAT);
        assert!(error.message.contains("line 2"), "{}", error.message);
        assert!(!out.exists());

        std::fs::write(&input, "{\"a\":1}\n").expect("write valid dataset");
        std::fs::write(&out, "keep").expect("write existing output");
        let error = deduplicate(&input, &out).expect_err("overwrite must fail");
        assert_eq!(error.code, AI_E_IO);
        assert!(
            error.message.contains("refusing to overwrite"),
            "{}",
            error.message
        );
        assert_eq!(std::fs::read_to_string(&out).expect("read output"), "keep");

        let error = deduplicate(&input, &input).expect_err("same path must fail");
        assert_eq!(error.code, AI_E_INVALID_REQUEST);
        assert!(
            error.message.contains("must differ from the input"),
            "{}",
            error.message
        );

        let missing = directory.join("missing.jsonl");
        let error = deduplicate(&missing, &directory.join("new.jsonl"))
            .expect_err("missing input must fail");
        assert_eq!(error.code, AI_E_IO);

        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn dedup_schema_gate_and_unique_record_limit() {
        let directory = temp_dir("dedup-schema-limit");
        let input = directory.join("data.jsonl");
        let schema = directory.join("schema.json");
        let out = directory.join("out.jsonl");
        std::fs::write(&input, "{\"text\":\"a\"}\n{\"other\":1}\n").expect("write dataset");
        std::fs::write(&schema, r#"{"type":"object","required":["text"]}"#).expect("write schema");
        let error =
            deduplicate_with_schema(&input, &out, &schema).expect_err("schema violation must fail");
        assert_eq!(error.code, AI_E_FORMAT);
        assert!(
            error.message.contains("does not satisfy the schema"),
            "{}",
            error.message
        );
        assert!(!out.exists());

        std::fs::write(&input, "{\"a\":1}\n{\"a\":2}\n{\"a\":3}\n").expect("write records");
        let capped = directory.join("capped.jsonl");
        let error = dedup_inner(&input, &capped, None, 2).expect_err("unique limit must fail");
        assert_eq!(error.code, AI_E_FORMAT);
        assert!(
            error.message.contains("more than 2 distinct records"),
            "{}",
            error.message
        );
        assert!(!capped.exists());

        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn dedup_rejects_empty_and_all_blank_inputs() {
        let directory = temp_dir("dedup-empty");
        let input = directory.join("empty.jsonl");
        let out = directory.join("out.jsonl");

        std::fs::write(&input, b"").expect("write empty dataset");
        let error = deduplicate(&input, &out).expect_err("empty dataset must fail");
        assert_eq!(error.code, AI_E_INVALID_REQUEST);
        assert!(error.message.contains("0 records"), "{}", error.message);
        assert!(!out.exists());

        std::fs::write(&input, b"\n\r\n").expect("write blank dataset");
        let error = deduplicate(&input, &out).expect_err("blank dataset must fail");
        assert_eq!(error.code, AI_E_INVALID_REQUEST);
        assert!(error.message.contains("0 records"), "{}", error.message);
        assert!(!out.exists());

        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn shuffle_is_deterministic_and_permutes_records() {
        let directory = temp_dir("shuffle-deterministic");
        let input = directory.join("corpus.jsonl");
        let mut text = String::from("\u{feff}");
        let mut expected = Vec::new();
        for index in 0..30 {
            let line = format!("{{\"id\":{index},\"text\":\"row {index}\"}}");
            expected.push(line.clone());
            text.push_str(&line);
            if index < 5 {
                text.push_str("\r\n");
            } else {
                text.push('\n');
            }
            if index == 5 {
                text.push('\n');
            }
        }
        std::fs::write(&input, &text).expect("write dataset");

        let first = shuffle(&input, &directory.join("first.jsonl"), 7).expect("first shuffle");
        let second = shuffle(&input, &directory.join("second.jsonl"), 7).expect("second shuffle");
        assert_eq!(first.record_count, 30);
        assert_eq!(first.seed, 7);
        assert_eq!(first.input_sha256, second.input_sha256);
        assert_eq!(first.output_sha256, second.output_sha256);
        assert_eq!(first.output_bytes, second.output_bytes);

        let first_bytes = std::fs::read(&first.output_path).expect("read first output");
        let second_bytes = std::fs::read(&second.output_path).expect("read second output");
        assert_eq!(first_bytes, second_bytes);
        assert!(!first_bytes.contains(&b'\r'), "output line endings are LF");

        let mut actual: Vec<String> = String::from_utf8(first_bytes)
            .expect("output is UTF-8")
            .lines()
            .map(str::to_string)
            .collect();
        actual.sort();
        expected.sort();
        assert_eq!(actual, expected, "shuffle must permute, not lose records");
        assert_eq!(
            std::fs::read_to_string(&input).expect("reread input"),
            text,
            "the input dataset must not be modified"
        );
        assert!(first.warnings.iter().any(|w| w.contains("blank line")));
        assert!(first.warnings.iter().any(|w| w.contains("CRLF")));
        assert!(first.warnings.iter().any(|w| w.contains("byte-order mark")));

        let third = shuffle(&input, &directory.join("third.jsonl"), 8).expect("third shuffle");
        assert_ne!(
            first.output_sha256, third.output_sha256,
            "a different seed must produce a different order"
        );

        let json = first.to_json();
        assert_eq!(
            json.get("format").and_then(Json::as_str),
            Some("jsonl-shuffle")
        );
        assert_eq!(json.get("seed").and_then(Json::as_u64), Some(7));
        let reparsed = json::parse(&json.to_compact_string()).expect("shuffle JSON is valid");
        assert_eq!(
            reparsed.get("records").and_then(Json::as_u64),
            json.get("records").and_then(Json::as_u64)
        );

        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn shuffle_refuses_overwrite_same_input_and_empty() {
        let directory = temp_dir("shuffle-errors");
        let input = directory.join("data.jsonl");
        let out = directory.join("out.jsonl");

        std::fs::write(&input, "{\"a\":1}\n").expect("write dataset");
        std::fs::write(&out, "keep").expect("write existing output");
        let error = shuffle(&input, &out, 0).expect_err("overwrite must fail");
        assert_eq!(error.code, AI_E_IO);
        assert!(
            error.message.contains("refusing to overwrite"),
            "{}",
            error.message
        );
        assert_eq!(std::fs::read_to_string(&out).expect("read output"), "keep");

        let error = shuffle(&input, &input, 0).expect_err("same path must fail");
        assert_eq!(error.code, AI_E_INVALID_REQUEST);
        assert!(
            error.message.contains("must differ from the input"),
            "{}",
            error.message
        );

        std::fs::write(&input, b"").expect("write empty dataset");
        let error = shuffle(&input, &directory.join("empty-out.jsonl"), 0)
            .expect_err("empty dataset must fail");
        assert_eq!(error.code, AI_E_INVALID_REQUEST);
        assert!(error.message.contains("0 records"), "{}", error.message);

        let missing = directory.join("missing.jsonl");
        let error = shuffle(&missing, &directory.join("missing-out.jsonl"), 0)
            .expect_err("missing input must fail");
        assert_eq!(error.code, AI_E_IO);

        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn shuffle_schema_gate_and_limits() {
        let directory = temp_dir("shuffle-schema-limits");
        let input = directory.join("data.jsonl");
        let schema = directory.join("schema.json");
        let out = directory.join("out.jsonl");
        std::fs::write(&input, "{\"text\":\"a\"}\n{\"other\":1}\n").expect("write dataset");
        std::fs::write(&schema, r#"{"type":"object","required":["text"]}"#).expect("write schema");
        let error =
            shuffle_with_schema(&input, &out, 0, &schema).expect_err("schema violation must fail");
        assert_eq!(error.code, AI_E_FORMAT);
        assert!(
            error.message.contains("does not satisfy the schema"),
            "{}",
            error.message
        );
        assert!(!out.exists());

        std::fs::write(&input, "{\"a\":1}\n{\"a\":2}\n{\"a\":3}\n").expect("write records");
        let capped = directory.join("capped.jsonl");
        let error = shuffle_inner(&input, &capped, 0, None, MAX_SHUFFLE_BYTES, 2)
            .expect_err("record limit must fail");
        assert_eq!(error.code, AI_E_FORMAT);
        assert!(
            error.message.contains("more than 2 records"),
            "{}",
            error.message
        );
        assert!(!capped.exists());

        let small = directory.join("small.jsonl");
        let error = shuffle_inner(&input, &small, 0, None, 10, MAX_SHUFFLE_RECORDS)
            .expect_err("byte limit must fail");
        assert_eq!(error.code, AI_E_FORMAT);
        assert!(
            error.message.contains("byte shuffle limit"),
            "{}",
            error.message
        );
        assert!(!small.exists());

        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn shuffle_rejects_invalid_lines_before_writing() {
        let directory = temp_dir("shuffle-invalid");
        let input = directory.join("bad.jsonl");
        let out = directory.join("out.jsonl");
        std::fs::write(&input, "{\"a\":1}\nnot json\n").expect("write dataset");
        let error = shuffle(&input, &out, 0).expect_err("invalid JSON must fail");
        assert_eq!(error.code, AI_E_FORMAT);
        assert!(error.message.contains("line 2"), "{}", error.message);
        assert!(!out.exists());

        std::fs::write(&input, "[1,2]\n{\"a\":1}\n").expect("write non-object dataset");
        let error = shuffle(&input, &out, 0).expect_err("non-object must fail");
        assert_eq!(error.code, AI_E_FORMAT);
        assert!(error.message.contains("not an object"), "{}", error.message);
        assert!(!out.exists());

        let _ = std::fs::remove_dir_all(directory);
    }
}
