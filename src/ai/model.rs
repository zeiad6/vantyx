//! Bounded, offline inspection of local model files (GGUF and Safetensors).
//!
//! The inspector never downloads a model, never executes model code, and never
//! allocates memory proportional to the tensor payload. It parses only the
//! metadata and tensor directory, validates the declared tensor ranges, and
//! streams the whole file through SHA-256 for provenance.
//!
//! Supported inputs:
//!
//! * GGUF versions 2 and 3, little-endian. Version 1 is rejected because its
//!   length encoding differs and is obsolete.
//! * Safetensors with a little-endian 8-byte header length followed by a JSON
//!   header and a data section.
//! * Safetensors shard indexes (`*.safetensors.index.json`) that map tensor
//!   names to sibling shard files. The inspector verifies the index against
//!   every referenced shard header and aggregates their summaries.
//!
//! Limits are deliberately explicit: a corrupt or hostile file must fail with
//! a stable `AI_E_FORMAT` error instead of exhausting memory or panicking.
//! Text metadata is decoded lossily because real GGUF tokenizer arrays can
//! contain byte strings that are not valid UTF-8; inspection is not loading.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs::File;
use std::io::{self, BufReader, Read};
use std::path::{Component, Path, PathBuf};

use super::json::{self, Json, JsonNumber};
use super::provider::{AI_E_FORMAT, AI_E_IO, AiError};

/// Maximum bytes of GGUF metadata and tensor directory that the inspector will
/// read. Tokenizer tables can be large, but anything above this is treated as a
/// hostile or malformed file.
pub const MAX_METADATA_BYTES: u64 = 256 * 1024 * 1024;

/// Maximum length of one GGUF string value.
pub const MAX_STRING_BYTES: u64 = 64 * 1024 * 1024;

/// Maximum number of elements in one GGUF metadata array.
pub const MAX_ARRAY_ITEMS: u64 = 50_000_000;

/// Maximum number of GGUF tensor descriptors.
pub const MAX_TENSORS: u64 = 1_000_000;

/// Maximum number of GGUF metadata key/value pairs.
pub const MAX_METADATA_ENTRIES: u64 = 100_000;

/// Maximum tensor rank accepted from either format.
pub const MAX_DIMS: usize = 8;

/// Maximum Safetensors JSON header length.
pub const MAX_SAFETENSORS_HEADER_BYTES: u64 = 100 * 1024 * 1024;

/// Maximum number of metadata entries rendered from a Safetensors header.
pub const MAX_SAFETENSORS_METADATA: usize = 10_000;

/// Maximum bytes of a Safetensors shard-index JSON file.
pub const MAX_SHARD_INDEX_BYTES: u64 = 64 * 1024 * 1024;

/// Maximum number of shard files referenced by one index.
pub const MAX_SHARDS: usize = 10_000;

/// Number of GGUF metadata entries included in the text/JSON summary before
/// the list is truncated with a warning.
pub const MAX_SUMMARY_METADATA: usize = 1_024;

/// A model container format recognized by the inspector.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelFormat {
    Gguf,
    Safetensors,
    SafetensorsIndex,
}

impl ModelFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Gguf => "gguf",
            Self::Safetensors => "safetensors",
            Self::SafetensorsIndex => "safetensors-index",
        }
    }
}

/// Aggregate tensor count and parameter count for one storage type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TensorTypeSummary {
    pub dtype: String,
    pub tensors: u64,
    pub parameters: u64,
}

/// One shard referenced by a Safetensors shard index.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShardSummary {
    pub path: String,
    pub file_bytes: u64,
    pub sha256: String,
    pub tensor_count: u64,
    pub parameter_count: u64,
    pub data_bytes: u64,
}

/// A validated, read-only summary of a local model file or shard index.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelSummary {
    pub path: String,
    pub format: ModelFormat,
    pub format_version: Option<u32>,
    pub file_bytes: u64,
    pub sha256: String,
    pub tensor_count: u64,
    pub parameter_count: u64,
    pub data_bytes: u64,
    pub architecture: Option<String>,
    pub name: Option<String>,
    pub context_length: Option<u64>,
    pub embedding_length: Option<u64>,
    pub block_count: Option<u64>,
    pub quantization: Option<String>,
    pub tokenizer_model: Option<String>,
    pub tokenizer_tokens: Option<u64>,
    pub metadata: Vec<(String, String)>,
    pub dtypes: Vec<TensorTypeSummary>,
    pub shards: Vec<ShardSummary>,
    pub warnings: Vec<String>,
}

impl ModelSummary {
    /// Serializes the summary as a JSON object suitable for machine use.
    pub fn to_json(&self) -> Json {
        let mut entries = vec![
            ("path".to_string(), Json::String(self.path.clone())),
            (
                "format".to_string(),
                Json::String(self.format.as_str().to_string()),
            ),
            (
                "format_version".to_string(),
                self.format_version
                    .map(|value| Json::Number(JsonNumber::UInt(u64::from(value))))
                    .unwrap_or(Json::Null),
            ),
            (
                "file_bytes".to_string(),
                Json::Number(JsonNumber::UInt(self.file_bytes)),
            ),
            ("sha256".to_string(), Json::String(self.sha256.clone())),
            (
                "tensor_count".to_string(),
                Json::Number(JsonNumber::UInt(self.tensor_count)),
            ),
            (
                "parameter_count".to_string(),
                Json::Number(JsonNumber::UInt(self.parameter_count)),
            ),
            (
                "data_bytes".to_string(),
                Json::Number(JsonNumber::UInt(self.data_bytes)),
            ),
        ];
        for (key, value) in [
            ("architecture", &self.architecture),
            ("name", &self.name),
            ("quantization", &self.quantization),
            ("tokenizer_model", &self.tokenizer_model),
        ] {
            entries.push((
                key.to_string(),
                value
                    .as_ref()
                    .map(|value| Json::String(value.clone()))
                    .unwrap_or(Json::Null),
            ));
        }
        for (key, value) in [
            ("context_length", self.context_length),
            ("embedding_length", self.embedding_length),
            ("block_count", self.block_count),
            ("tokenizer_tokens", self.tokenizer_tokens),
        ] {
            entries.push((
                key.to_string(),
                value
                    .map(|value| Json::Number(JsonNumber::UInt(value)))
                    .unwrap_or(Json::Null),
            ));
        }
        entries.push((
            "metadata".to_string(),
            Json::Object(
                self.metadata
                    .iter()
                    .map(|(key, value)| (key.clone(), Json::String(value.clone())))
                    .collect(),
            ),
        ));
        entries.push((
            "dtypes".to_string(),
            Json::Array(
                self.dtypes
                    .iter()
                    .map(|dtype| {
                        Json::object([
                            ("dtype", Json::String(dtype.dtype.clone())),
                            ("tensors", Json::Number(JsonNumber::UInt(dtype.tensors))),
                            (
                                "parameters",
                                Json::Number(JsonNumber::UInt(dtype.parameters)),
                            ),
                        ])
                    })
                    .collect(),
            ),
        ));
        entries.push((
            "shards".to_string(),
            Json::Array(
                self.shards
                    .iter()
                    .map(|shard| {
                        Json::object([
                            ("path", Json::String(shard.path.clone())),
                            (
                                "file_bytes",
                                Json::Number(JsonNumber::UInt(shard.file_bytes)),
                            ),
                            ("sha256", Json::String(shard.sha256.clone())),
                            (
                                "tensor_count",
                                Json::Number(JsonNumber::UInt(shard.tensor_count)),
                            ),
                            (
                                "parameter_count",
                                Json::Number(JsonNumber::UInt(shard.parameter_count)),
                            ),
                            (
                                "data_bytes",
                                Json::Number(JsonNumber::UInt(shard.data_bytes)),
                            ),
                        ])
                    })
                    .collect(),
            ),
        ));
        entries.push((
            "warnings".to_string(),
            Json::Array(
                self.warnings
                    .iter()
                    .map(|warning| Json::String(warning.clone()))
                    .collect(),
            ),
        ));
        Json::Object(entries)
    }
}

/// Inspects a GGUF file, a Safetensors file, or a Safetensors shard index
/// without modifying it.
pub fn inspect(path: &Path) -> Result<ModelSummary, AiError> {
    inspect_inner(path, true)
}

fn inspect_inner(path: &Path, allow_index: bool) -> Result<ModelSummary, AiError> {
    let file = File::open(path).map_err(|error| {
        AiError::new(
            AI_E_IO,
            format!("cannot open model `{}`: {error}", path.display()),
        )
    })?;
    let metadata = file.metadata().map_err(|error| {
        AiError::new(
            AI_E_IO,
            format!("cannot stat model `{}`: {error}", path.display()),
        )
    })?;
    if !metadata.is_file() {
        return Err(AiError::new(
            AI_E_IO,
            format!("cannot inspect `{}`: not a regular file", path.display()),
        ));
    }
    let file_bytes = metadata.len();
    if file_bytes < 8 {
        return Err(format_error(
            "model file is smaller than the 8-byte format header",
        ));
    }
    let mut file = file;
    let mut first = [0_u8; 8];
    file.read_exact(&mut first).map_err(io_failure)?;
    let json_path = path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("json"));
    let mut reader = HashingReader::new(BufReader::new(file), &first);
    let mut summary = if &first[..4] == b"GGUF" {
        let version = u32::from_le_bytes(first[4..8].try_into().expect("4-byte version"));
        inspect_gguf(&mut reader, file_bytes, version)?
    } else {
        let header_len = u64::from_le_bytes(first);
        let plausible_safetensors_header = header_len > 0
            && header_len <= MAX_SAFETENSORS_HEADER_BYTES
            && header_len <= file_bytes.saturating_sub(8);
        if plausible_safetensors_header {
            let mut budget = header_len;
            let header = read_bytes(&mut reader, &mut budget, header_len)?;
            if !starts_with_json_object(&header) {
                return Err(unsupported_format());
            }
            inspect_safetensors(file_bytes, &header)?
        } else if starts_with_json_object(&first) || json_path {
            if !allow_index {
                return Err(format_error(
                    "Safetensors shard is a shard index, not a Safetensors data file",
                ));
            }
            inspect_index_from_reader(path, file_bytes, &first, &mut reader)?
        } else if header_len > 0
            && header_len <= MAX_SAFETENSORS_HEADER_BYTES
            && header_len > file_bytes.saturating_sub(8)
        {
            return Err(format_error(
                "Safetensors header length exceeds the model file size",
            ));
        } else {
            return Err(unsupported_format());
        }
    };
    let (sha256, hashed_bytes) = reader.finish()?;
    if hashed_bytes != file_bytes {
        summary.warnings.push(format!(
            "file size changed during inspection: expected {file_bytes} bytes, hashed {hashed_bytes}"
        ));
    }
    summary.path = path.display().to_string();
    summary.file_bytes = hashed_bytes;
    summary.sha256 = sha256;
    Ok(summary)
}

fn unsupported_format() -> AiError {
    format_error(
        "unsupported model format: expected a GGUF file (`GGUF` magic), a Safetensors \
         file (8-byte little-endian header length followed by a JSON object), or a \
         Safetensors shard index (JSON object with a `weight_map`)",
    )
}

fn format_error(message: impl Into<String>) -> AiError {
    AiError::new(AI_E_FORMAT, message)
}

fn io_failure(error: io::Error) -> AiError {
    AiError::new(AI_E_IO, format!("cannot read model file: {error}"))
}

struct HashingReader<R> {
    inner: R,
    hasher: Sha256,
    bytes: u64,
}

impl<R: Read> HashingReader<R> {
    fn new(inner: R, prefix: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(prefix);
        Self {
            inner,
            hasher,
            bytes: prefix.len() as u64,
        }
    }

    fn finish(mut self) -> Result<(String, u64), AiError> {
        io::copy(&mut self, &mut io::sink()).map_err(io_failure)?;
        Ok((self.hasher.finalize_hex(), self.bytes))
    }
}

impl<R: Read> Read for HashingReader<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let read = self.inner.read(buffer)?;
        self.hasher.update(&buffer[..read]);
        self.bytes += read as u64;
        Ok(read)
    }
}

fn read_exact_budget(
    reader: &mut impl Read,
    budget: &mut u64,
    buffer: &mut [u8],
    what: &str,
) -> Result<(), AiError> {
    let length = buffer.len() as u64;
    if length > *budget {
        return Err(format_error(format!(
            "{what} exceeds the {} byte inspection limit",
            MAX_METADATA_BYTES
        )));
    }
    reader.read_exact(buffer).map_err(|error| {
        if error.kind() == io::ErrorKind::UnexpectedEof {
            format_error(format!("{what} is truncated"))
        } else {
            io_failure(error)
        }
    })?;
    *budget -= length;
    Ok(())
}

fn read_bytes(reader: &mut impl Read, budget: &mut u64, length: u64) -> Result<Vec<u8>, AiError> {
    let length = usize::try_from(length)
        .map_err(|_| format_error("model header length does not fit in memory"))?;
    let mut bytes = vec![0_u8; length];
    read_exact_budget(reader, budget, &mut bytes, "model header")?;
    Ok(bytes)
}

fn read_u8(reader: &mut impl Read, budget: &mut u64) -> Result<u8, AiError> {
    let mut bytes = [0_u8; 1];
    read_exact_budget(reader, budget, &mut bytes, "GGUF metadata")?;
    Ok(bytes[0])
}

fn read_u32(reader: &mut impl Read, budget: &mut u64) -> Result<u32, AiError> {
    let mut bytes = [0_u8; 4];
    read_exact_budget(reader, budget, &mut bytes, "GGUF metadata")?;
    Ok(u32::from_le_bytes(bytes))
}

fn read_u64(reader: &mut impl Read, budget: &mut u64) -> Result<u64, AiError> {
    let mut bytes = [0_u8; 8];
    read_exact_budget(reader, budget, &mut bytes, "GGUF metadata")?;
    Ok(u64::from_le_bytes(bytes))
}

fn skip_bytes(reader: &mut impl Read, budget: &mut u64, length: u64) -> Result<(), AiError> {
    let mut remaining = length;
    let mut scratch = [0_u8; 8 * 1024];
    while remaining > 0 {
        let chunk = remaining.min(scratch.len() as u64) as usize;
        read_exact_budget(reader, budget, &mut scratch[..chunk], "GGUF metadata")?;
        remaining -= chunk as u64;
    }
    Ok(())
}

// === SECTION: SHA-256 ===

const SHA256_INITIAL: [u32; 8] = [
    0x6a09_e667,
    0xbb67_ae85,
    0x3c6e_f372,
    0xa54f_f53a,
    0x510e_527f,
    0x9b05_688c,
    0x1f83_d9ab,
    0x5be0_cd19,
];

const SHA256_ROUND: [u32; 64] = [
    0x428a_2f98,
    0x7137_4491,
    0xb5c0_fbcf,
    0xe9b5_dba5,
    0x3956_c25b,
    0x59f1_11f1,
    0x923f_82a4,
    0xab1c_5ed5,
    0xd807_aa98,
    0x1283_5b01,
    0x2431_85be,
    0x550c_7dc3,
    0x72be_5d74,
    0x80de_b1fe,
    0x9bdc_06a7,
    0xc19b_f174,
    0xe49b_69c1,
    0xefbe_4786,
    0x0fc1_9dc6,
    0x240c_a1cc,
    0x2de9_2c6f,
    0x4a74_84aa,
    0x5cb0_a9dc,
    0x76f9_88da,
    0x983e_5152,
    0xa831_c66d,
    0xb003_27c8,
    0xbf59_7fc7,
    0xc6e0_0bf3,
    0xd5a7_9147,
    0x06ca_6351,
    0x1429_2967,
    0x27b7_0a85,
    0x2e1b_2138,
    0x4d2c_6dfc,
    0x5338_0d13,
    0x650a_7354,
    0x766a_0abb,
    0x81c2_c92e,
    0x9272_2c85,
    0xa2bf_e8a1,
    0xa81a_664b,
    0xc24b_8b70,
    0xc76c_51a3,
    0xd192_e819,
    0xd699_0624,
    0xf40e_3585,
    0x106a_a070,
    0x19a4_c116,
    0x1e37_6c08,
    0x2748_774c,
    0x34b0_bcb5,
    0x391c_0cb3,
    0x4ed8_aa4a,
    0x5b9c_ca4f,
    0x682e_6ff3,
    0x748f_82ee,
    0x78a5_636f,
    0x84c8_7814,
    0x8cc7_0208,
    0x90be_fffa,
    0xa450_6ceb,
    0xbef9_a3f7,
    0xc671_78f2,
];

/// Incremental SHA-256. It is implemented here because the repository has no
/// third-party dependencies; model provenance must not add one.
#[derive(Clone)]
pub struct Sha256 {
    state: [u32; 8],
    buffer: [u8; 64],
    buffered: usize,
    length: u64,
}

impl Default for Sha256 {
    fn default() -> Self {
        Self::new()
    }
}

impl Sha256 {
    pub fn new() -> Self {
        Self {
            state: SHA256_INITIAL,
            buffer: [0_u8; 64],
            buffered: 0,
            length: 0,
        }
    }

    pub fn update(&mut self, mut data: &[u8]) {
        self.length = self.length.wrapping_add(data.len() as u64);
        if self.buffered > 0 {
            let take = (64 - self.buffered).min(data.len());
            self.buffer[self.buffered..self.buffered + take].copy_from_slice(&data[..take]);
            self.buffered += take;
            data = &data[take..];
            if self.buffered == 64 {
                let block = self.buffer;
                self.compress(&block);
                self.buffered = 0;
            }
        }
        while data.len() >= 64 {
            let (block, rest) = data.split_at(64);
            let block: &[u8; 64] = block.try_into().expect("64-byte block");
            self.compress(block);
            data = rest;
        }
        if !data.is_empty() {
            self.buffer[..data.len()].copy_from_slice(data);
            self.buffered = data.len();
        }
    }

    pub fn finalize(mut self) -> [u8; 32] {
        let bit_length = self.length.wrapping_mul(8);
        self.update(&[0x80]);
        while self.buffered != 56 {
            self.update(&[0]);
        }
        self.update(&bit_length.to_be_bytes());
        let mut digest = [0_u8; 32];
        for (index, word) in self.state.iter().enumerate() {
            digest[index * 4..index * 4 + 4].copy_from_slice(&word.to_be_bytes());
        }
        digest
    }

    pub fn finalize_hex(self) -> String {
        let digest = self.finalize();
        let mut text = String::with_capacity(64);
        for byte in digest {
            use std::fmt::Write;
            let _ = write!(text, "{byte:02x}");
        }
        text
    }

    fn compress(&mut self, block: &[u8; 64]) {
        let mut words = [0_u32; 64];
        for (index, word) in words.iter_mut().take(16).enumerate() {
            let offset = index * 4;
            *word = u32::from_be_bytes([
                block[offset],
                block[offset + 1],
                block[offset + 2],
                block[offset + 3],
            ]);
        }
        for index in 16..64 {
            let s0 = words[index - 15].rotate_right(7)
                ^ words[index - 15].rotate_right(18)
                ^ (words[index - 15] >> 3);
            let s1 = words[index - 2].rotate_right(17)
                ^ words[index - 2].rotate_right(19)
                ^ (words[index - 2] >> 10);
            words[index] = words[index - 16]
                .wrapping_add(s0)
                .wrapping_add(words[index - 7])
                .wrapping_add(s1);
        }
        let mut a = self.state[0];
        let mut b = self.state[1];
        let mut c = self.state[2];
        let mut d = self.state[3];
        let mut e = self.state[4];
        let mut f = self.state[5];
        let mut g = self.state[6];
        let mut h = self.state[7];
        for index in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choice = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(choice)
                .wrapping_add(SHA256_ROUND[index])
                .wrapping_add(words[index]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(majority);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        self.state[0] = self.state[0].wrapping_add(a);
        self.state[1] = self.state[1].wrapping_add(b);
        self.state[2] = self.state[2].wrapping_add(c);
        self.state[3] = self.state[3].wrapping_add(d);
        self.state[4] = self.state[4].wrapping_add(e);
        self.state[5] = self.state[5].wrapping_add(f);
        self.state[6] = self.state[6].wrapping_add(g);
        self.state[7] = self.state[7].wrapping_add(h);
    }
}

// === SECTION: GGUF ===

#[derive(Clone, Debug)]
enum MetaValue {
    UInt(u64),
    Int(i64),
    Float(f64),
    Bool(bool),
    Text(String),
    Array { element: String, len: u64 },
}

impl MetaValue {
    fn as_u64(&self) -> Option<u64> {
        match self {
            Self::UInt(value) => Some(*value),
            Self::Int(value) if *value >= 0 => Some(*value as u64),
            _ => None,
        }
    }

    fn as_str(&self) -> Option<&str> {
        match self {
            Self::Text(value) => Some(value),
            _ => None,
        }
    }

    fn display(&self) -> String {
        match self {
            Self::UInt(value) => value.to_string(),
            Self::Int(value) => value.to_string(),
            Self::Float(value) => value.to_string(),
            Self::Bool(value) => value.to_string(),
            Self::Text(value) => value.clone(),
            Self::Array { element, len } => format!("[{len} x {element}]"),
        }
    }
}

fn gguf_type_name(type_id: u32) -> String {
    match type_id {
        0 => "UINT8".to_string(),
        1 => "INT8".to_string(),
        2 => "UINT16".to_string(),
        3 => "INT16".to_string(),
        4 => "UINT32".to_string(),
        5 => "INT32".to_string(),
        6 => "FLOAT32".to_string(),
        7 => "BOOL".to_string(),
        8 => "STRING".to_string(),
        9 => "ARRAY".to_string(),
        10 => "UINT64".to_string(),
        11 => "INT64".to_string(),
        12 => "FLOAT64".to_string(),
        other => format!("TYPE_{other}"),
    }
}

fn gguf_scalar_size(type_id: u32) -> Option<u64> {
    match type_id {
        0..=1 | 7 => Some(1),
        2..=3 => Some(2),
        4..=6 => Some(4),
        10..=12 => Some(8),
        _ => None,
    }
}

fn gguf_string(
    reader: &mut impl Read,
    budget: &mut u64,
    what: &str,
    maximum: u64,
) -> Result<String, AiError> {
    let length = read_u64(reader, budget)?;
    if length > maximum {
        return Err(format_error(format!(
            "{what} is {length} bytes, above the {maximum} byte limit"
        )));
    }
    let bytes = read_bytes(reader, budget, length)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn gguf_value(
    reader: &mut impl Read,
    budget: &mut u64,
    type_id: u32,
    depth: u32,
) -> Result<MetaValue, AiError> {
    match type_id {
        0 => Ok(MetaValue::UInt(u64::from(read_u8(reader, budget)?))),
        1 => Ok(MetaValue::Int(i64::from(read_u8(reader, budget)? as i8))),
        2 => {
            let mut bytes = [0_u8; 2];
            read_exact_budget(reader, budget, &mut bytes, "GGUF metadata")?;
            Ok(MetaValue::UInt(u64::from(u16::from_le_bytes(bytes))))
        }
        3 => {
            let mut bytes = [0_u8; 2];
            read_exact_budget(reader, budget, &mut bytes, "GGUF metadata")?;
            Ok(MetaValue::Int(i64::from(i16::from_le_bytes(bytes))))
        }
        4 => Ok(MetaValue::UInt(u64::from(read_u32(reader, budget)?))),
        5 => Ok(MetaValue::Int(i64::from(read_u32(reader, budget)? as i32))),
        6 => Ok(MetaValue::Float(f64::from(f32::from_bits(read_u32(
            reader, budget,
        )?)))),
        7 => {
            let value = read_u8(reader, budget)?;
            if value > 1 {
                return Err(format_error(format!(
                    "GGUF boolean metadata has invalid value {value}"
                )));
            }
            Ok(MetaValue::Bool(value == 1))
        }
        8 => Ok(MetaValue::Text(gguf_string(
            reader,
            budget,
            "GGUF string",
            MAX_STRING_BYTES,
        )?)),
        9 => {
            if depth >= 2 {
                return Err(format_error(
                    "GGUF metadata arrays are nested more than two levels deep",
                ));
            }
            let element_type = read_u32(reader, budget)?;
            if element_type > 12 {
                return Err(format_error(format!(
                    "GGUF metadata array uses unknown element type {element_type}"
                )));
            }
            let count = read_u64(reader, budget)?;
            if count > MAX_ARRAY_ITEMS {
                return Err(format_error(format!(
                    "GGUF metadata array has {count} elements, above the {MAX_ARRAY_ITEMS} limit"
                )));
            }
            skip_gguf_array(reader, budget, element_type, count, depth)?;
            Ok(MetaValue::Array {
                element: gguf_type_name(element_type),
                len: count,
            })
        }
        10 => Ok(MetaValue::UInt(read_u64(reader, budget)?)),
        11 => Ok(MetaValue::Int(read_u64(reader, budget)? as i64)),
        12 => Ok(MetaValue::Float(f64::from_bits(read_u64(reader, budget)?))),
        other => Err(format_error(format!(
            "GGUF metadata uses unknown value type {other}"
        ))),
    }
}

fn skip_gguf_array(
    reader: &mut impl Read,
    budget: &mut u64,
    element_type: u32,
    count: u64,
    depth: u32,
) -> Result<(), AiError> {
    if let Some(size) = gguf_scalar_size(element_type) {
        let total = size
            .checked_mul(count)
            .ok_or_else(|| format_error("GGUF metadata array length overflows"))?;
        return skip_bytes(reader, budget, total);
    }
    match element_type {
        8 => {
            for _ in 0..count {
                let length = read_u64(reader, budget)?;
                if length > MAX_STRING_BYTES {
                    return Err(format_error(format!(
                        "GGUF array string is {length} bytes, above the {MAX_STRING_BYTES} byte limit"
                    )));
                }
                skip_bytes(reader, budget, length)?;
            }
            Ok(())
        }
        9 => {
            if depth >= 2 {
                return Err(format_error(
                    "GGUF metadata arrays are nested more than two levels deep",
                ));
            }
            for _ in 0..count {
                let nested_type = read_u32(reader, budget)?;
                if nested_type > 12 {
                    return Err(format_error(format!(
                        "GGUF metadata array uses unknown element type {nested_type}"
                    )));
                }
                let nested_count = read_u64(reader, budget)?;
                if nested_count > MAX_ARRAY_ITEMS {
                    return Err(format_error(format!(
                        "GGUF metadata array has {nested_count} elements, above the {MAX_ARRAY_ITEMS} limit"
                    )));
                }
                skip_gguf_array(reader, budget, nested_type, nested_count, depth + 1)?;
            }
            Ok(())
        }
        other => Err(format_error(format!(
            "GGUF metadata array uses unsupported element type {other}"
        ))),
    }
}

struct GgmlType {
    name: &'static str,
    block_elements: u64,
    block_bytes: u64,
}

fn ggml_type(type_id: u32) -> Option<GgmlType> {
    let (name, block_elements, block_bytes) = match type_id {
        0 => ("F32", 1, 4),
        1 => ("F16", 1, 2),
        2 => ("Q4_0", 32, 18),
        3 => ("Q4_1", 32, 20),
        6 => ("Q5_0", 32, 22),
        7 => ("Q5_1", 32, 24),
        8 => ("Q8_0", 32, 34),
        9 => ("Q8_1", 32, 36),
        10 => ("Q2_K", 256, 84),
        11 => ("Q3_K", 256, 110),
        12 => ("Q4_K", 256, 144),
        13 => ("Q5_K", 256, 176),
        14 => ("Q6_K", 256, 210),
        15 => ("Q8_K", 256, 292),
        16 => ("IQ2_XXS", 256, 66),
        17 => ("IQ2_XS", 256, 74),
        18 => ("IQ3_XXS", 256, 98),
        19 => ("IQ1_S", 256, 50),
        20 => ("IQ4_NL", 32, 18),
        21 => ("IQ3_S", 256, 110),
        22 => ("IQ2_S", 256, 82),
        23 => ("IQ4_XS", 256, 136),
        24 => ("I8", 1, 1),
        25 => ("I16", 1, 2),
        26 => ("I32", 1, 4),
        27 => ("I64", 1, 8),
        28 => ("F64", 1, 8),
        29 => ("IQ1_M", 256, 56),
        30 => ("BF16", 1, 2),
        31 => ("Q4_0_4_4", 32, 18),
        32 => ("Q4_0_4_8", 32, 18),
        33 => ("Q4_0_8_8", 32, 18),
        34 => ("TQ1_0", 256, 54),
        35 => ("TQ2_0", 256, 66),
        36 => ("IQ4_NL_4_4", 32, 18),
        37 => ("IQ4_NL_4_8", 32, 18),
        38 => ("IQ4_NL_8_8", 32, 18),
        39 => ("MXFP4", 32, 17),
        _ => return None,
    };
    Some(GgmlType {
        name,
        block_elements,
        block_bytes,
    })
}

fn file_type_label(value: u64) -> String {
    let name = match value {
        0 => "ALL_F32",
        1 => "MOSTLY_F16",
        2 => "MOSTLY_Q4_0",
        3 => "MOSTLY_Q4_1",
        7 => "MOSTLY_Q8_0",
        8 => "MOSTLY_Q5_0",
        9 => "MOSTLY_Q5_1",
        10 => "MOSTLY_Q2_K",
        11 => "MOSTLY_Q3_K_S",
        12 => "MOSTLY_Q3_K_M",
        13 => "MOSTLY_Q3_K_L",
        14 => "MOSTLY_Q4_K_S",
        15 => "MOSTLY_Q4_K_M",
        16 => "MOSTLY_Q5_K_S",
        17 => "MOSTLY_Q5_K_M",
        18 => "MOSTLY_Q6_K",
        19 => "MOSTLY_IQ2_XXS",
        20 => "MOSTLY_IQ2_XS",
        21 => "MOSTLY_Q2_K_S",
        22 => "MOSTLY_IQ3_XS",
        23 => "MOSTLY_IQ3_XXS",
        24 => "MOSTLY_IQ1_S",
        25 => "MOSTLY_IQ4_NL",
        26 => "MOSTLY_IQ3_S",
        27 => "MOSTLY_IQ3_M",
        28 => "MOSTLY_IQ2_S",
        29 => "MOSTLY_IQ2_M",
        30 => "MOSTLY_IQ4_XS",
        31 => "MOSTLY_IQ1_M",
        32 => "MOSTLY_BF16",
        33 => "MOSTLY_Q4_0_4_4",
        34 => "MOSTLY_Q4_0_4_8",
        35 => "MOSTLY_Q4_0_8_8",
        36 => "MOSTLY_TQ1_0",
        37 => "MOSTLY_TQ2_0",
        38 => "MOSTLY_MXFP4_MOE",
        _ => return format!("file_type_{value}"),
    };
    name.to_string()
}

fn inspect_gguf(
    reader: &mut HashingReader<impl Read>,
    file_bytes: u64,
    version: u32,
) -> Result<ModelSummary, AiError> {
    let mut budget = MAX_METADATA_BYTES;
    if version == 1 {
        return Err(format_error(
            "GGUF version 1 is obsolete and is not supported; re-convert the model with a current tool",
        ));
    }
    if !(2..=3).contains(&version) {
        return Err(format_error(format!(
            "unsupported GGUF version {version}; this inspector supports versions 2 and 3"
        )));
    }
    let tensor_count = read_u64(reader, &mut budget)?;
    if tensor_count > MAX_TENSORS {
        return Err(format_error(format!(
            "GGUF declares {tensor_count} tensors, above the {MAX_TENSORS} limit"
        )));
    }
    let metadata_count = read_u64(reader, &mut budget)?;
    if metadata_count > MAX_METADATA_ENTRIES {
        return Err(format_error(format!(
            "GGUF declares {metadata_count} metadata entries, above the {MAX_METADATA_ENTRIES} limit"
        )));
    }

    let mut metadata = BTreeMap::new();
    let mut warnings = Vec::new();
    for _ in 0..metadata_count {
        let key = gguf_string(reader, &mut budget, "GGUF metadata key", 1024 * 1024)?;
        let type_id = read_u32(reader, &mut budget)?;
        let value = gguf_value(reader, &mut budget, type_id, 0)?;
        if metadata.insert(key.clone(), value).is_some() {
            warnings.push(format!(
                "duplicate metadata key `{key}`; the last value is shown"
            ));
        }
    }

    let mut dtypes: BTreeMap<String, (u64, u64)> = BTreeMap::new();
    let mut ranges: Vec<(u64, u64, String)> = Vec::new();
    let mut unknown_offsets: Vec<(u64, String)> = Vec::new();
    let mut names = HashSet::new();
    let mut parameter_count = 0_u64;
    for _ in 0..tensor_count {
        let name = gguf_string(reader, &mut budget, "GGUF tensor name", 1024 * 1024)?;
        if name.is_empty() {
            return Err(format_error("GGUF tensor has an empty name"));
        }
        if !names.insert(name.clone()) {
            return Err(format_error(format!("duplicate GGUF tensor name `{name}`")));
        }
        let dimensions = read_u32(reader, &mut budget)?;
        if dimensions == 0 || dimensions as usize > MAX_DIMS {
            return Err(format_error(format!(
                "GGUF tensor `{name}` has {dimensions} dimensions; expected 1 to {MAX_DIMS}"
            )));
        }
        let mut elements = 1_u64;
        for _ in 0..dimensions {
            let dimension = read_u64(reader, &mut budget)?;
            if dimension == 0 {
                return Err(format_error(format!(
                    "GGUF tensor `{name}` has a zero-sized dimension"
                )));
            }
            elements = elements.checked_mul(dimension).ok_or_else(|| {
                format_error(format!("GGUF tensor `{name}` element count overflows"))
            })?;
        }
        let type_id = read_u32(reader, &mut budget)?;
        let offset = read_u64(reader, &mut budget)?;
        parameter_count = parameter_count
            .checked_add(elements)
            .ok_or_else(|| format_error("GGUF parameter count overflows a 64-bit integer"))?;
        let dtype_name = ggml_type(type_id)
            .map(|info| info.name.to_string())
            .unwrap_or_else(|| format!("GGML_TYPE_{type_id}"));
        let entry = dtypes.entry(dtype_name).or_insert((0, 0));
        entry.0 += 1;
        entry.1 = entry
            .1
            .checked_add(elements)
            .ok_or_else(|| format_error("GGUF dtype parameter count overflows"))?;
        if let Some(info) = ggml_type(type_id) {
            if !elements.is_multiple_of(info.block_elements) {
                return Err(format_error(format!(
                    "GGUF tensor `{name}` has {elements} elements, which is not a multiple of the {} element block for {}",
                    info.block_elements, info.name
                )));
            }
            let byte_length = (elements / info.block_elements)
                .checked_mul(info.block_bytes)
                .ok_or_else(|| format_error(format!("GGUF tensor `{name}` size overflows")))?;
            let end = offset.checked_add(byte_length).ok_or_else(|| {
                format_error(format!("GGUF tensor `{name}` data range overflows"))
            })?;
            ranges.push((offset, end, name));
        } else {
            warnings.push(format!(
                "tensor `{name}` uses unknown GGML type {type_id}; its data size was not validated"
            ));
            unknown_offsets.push((offset, name));
        }
    }

    let header_end = reader.bytes;
    let alignment = metadata
        .get("general.alignment")
        .and_then(MetaValue::as_u64)
        .unwrap_or(32);
    if alignment == 0 || !alignment.is_power_of_two() || alignment > 1024 * 1024 {
        return Err(format_error(format!(
            "GGUF alignment {alignment} is not a power of two between 1 and 1048576"
        )));
    }
    let data_offset = header_end
        .checked_add(alignment - 1)
        .map(|value| value & !(alignment - 1))
        .ok_or_else(|| format_error("GGUF data offset overflows"))?;
    if data_offset > file_bytes {
        return Err(format_error(format!(
            "GGUF tensor data starts at byte {data_offset}, beyond the {file_bytes} byte file"
        )));
    }
    let data_bytes = file_bytes - data_offset;
    for (start, end, name) in &ranges {
        if *start > data_bytes || *end > data_bytes {
            return Err(format_error(format!(
                "GGUF tensor `{name}` data range {start}..{end} exceeds the {data_bytes} byte data section"
            )));
        }
    }
    for (offset, name) in &unknown_offsets {
        if *offset > data_bytes {
            return Err(format_error(format!(
                "GGUF tensor `{name}` offset {offset} exceeds the {data_bytes} byte data section"
            )));
        }
    }
    ranges.sort_by_key(|range| range.0);
    for pair in ranges.windows(2) {
        if pair[1].0 < pair[0].1 {
            return Err(format_error(format!(
                "GGUF tensors `{}` and `{}` have overlapping data ranges",
                pair[0].2, pair[1].2
            )));
        }
    }

    let padding = data_offset - header_end;
    skip_exact(reader, padding, "GGUF tensor data")?;

    let architecture = metadata
        .get("general.architecture")
        .and_then(MetaValue::as_str)
        .map(ToString::to_string);
    let name = metadata
        .get("general.name")
        .and_then(MetaValue::as_str)
        .map(ToString::to_string);
    let field = |suffix: &str| {
        architecture
            .as_ref()
            .and_then(|architecture| metadata.get(&format!("{architecture}.{suffix}")))
            .and_then(MetaValue::as_u64)
    };
    let context_length = field("context_length");
    let embedding_length = field("embedding_length");
    let block_count = field("block_count");
    let quantization = metadata
        .get("general.file_type")
        .and_then(MetaValue::as_u64)
        .map(file_type_label);
    let tokenizer_model = metadata
        .get("tokenizer.ggml.model")
        .and_then(MetaValue::as_str)
        .map(ToString::to_string);
    let tokenizer_tokens = metadata
        .get("tokenizer.ggml.tokens")
        .and_then(|value| match value {
            MetaValue::Array { len, .. } => Some(*len),
            _ => None,
        });
    if let Some(declared) = metadata
        .get("general.parameter_count")
        .and_then(MetaValue::as_u64)
        .filter(|declared| *declared != parameter_count)
    {
        warnings.push(format!(
            "declared general.parameter_count {declared} differs from the computed {parameter_count}"
        ));
    }

    let mut metadata_display = Vec::new();
    for (key, value) in &metadata {
        if metadata_display.len() == MAX_SUMMARY_METADATA {
            warnings.push(format!(
                "metadata display truncated to the first {MAX_SUMMARY_METADATA} entries"
            ));
            break;
        }
        metadata_display.push((key.clone(), value.display()));
    }

    let mut dtypes: Vec<TensorTypeSummary> = dtypes
        .into_iter()
        .map(|(dtype, (tensors, parameters))| TensorTypeSummary {
            dtype,
            tensors,
            parameters,
        })
        .collect();
    dtypes.sort_by(|left, right| left.dtype.cmp(&right.dtype));

    Ok(ModelSummary {
        path: String::new(),
        format: ModelFormat::Gguf,
        format_version: Some(version),
        file_bytes,
        sha256: String::new(),
        tensor_count,
        parameter_count,
        data_bytes,
        architecture,
        name,
        context_length,
        embedding_length,
        block_count,
        quantization,
        tokenizer_model,
        tokenizer_tokens,
        metadata: metadata_display,
        dtypes,
        shards: Vec::new(),
        warnings,
    })
}

// === SECTION: SAFETENSORS ===

fn safetensors_dtype_size(dtype: &str) -> Option<u64> {
    match dtype {
        "BOOL" | "U8" | "I8" | "F8_E5M2" | "F8_E4M3" => Some(1),
        "I16" | "U16" | "F16" | "BF16" => Some(2),
        "I32" | "U32" | "F32" => Some(4),
        "I64" | "U64" | "F64" => Some(8),
        _ => None,
    }
}

fn inspect_safetensors(file_bytes: u64, header: &[u8]) -> Result<ModelSummary, AiError> {
    let header_text = std::str::from_utf8(header)
        .map_err(|_| format_error("Safetensors header is not valid UTF-8"))?;
    let root = json::parse(header_text)
        .map_err(|error| format_error(format!("invalid Safetensors header JSON: {error}")))?;
    let entries = root
        .as_object()
        .ok_or_else(|| format_error("Safetensors header must be a JSON object"))?;
    let header_len = header.len() as u64;
    let data_bytes = file_bytes
        .checked_sub(8 + header_len)
        .ok_or_else(|| format_error("Safetensors header is larger than the file"))?;

    let mut metadata: BTreeMap<String, String> = BTreeMap::new();
    let mut dtypes: BTreeMap<String, (u64, u64)> = BTreeMap::new();
    let mut ranges: Vec<(u64, u64, String)> = Vec::new();
    let mut warnings = Vec::new();
    let mut tensor_count = 0_u64;
    let mut parameter_count = 0_u64;

    for (key, value) in entries {
        if key == "__metadata__" {
            let metadata_entries = value.as_object().ok_or_else(|| {
                format_error("Safetensors `__metadata__` must be a JSON object of strings")
            })?;
            if metadata_entries.len() > MAX_SAFETENSORS_METADATA {
                return Err(format_error(format!(
                    "Safetensors metadata has {} entries, above the {MAX_SAFETENSORS_METADATA} limit",
                    metadata_entries.len()
                )));
            }
            for (name, value) in metadata_entries {
                let text = value.as_str().ok_or_else(|| {
                    format_error(format!(
                        "Safetensors metadata value `{name}` must be a string"
                    ))
                })?;
                metadata.insert(name.clone(), text.to_string());
            }
            continue;
        }

        tensor_count = tensor_count
            .checked_add(1)
            .ok_or_else(|| format_error("Safetensors tensor count overflows"))?;
        if tensor_count > MAX_TENSORS {
            return Err(format_error(format!(
                "Safetensors declares more than {MAX_TENSORS} tensors"
            )));
        }
        let tensor = value.as_object().ok_or_else(|| {
            format_error(format!("Safetensors tensor `{key}` must be a JSON object"))
        })?;
        let dtype = tensor
            .iter()
            .find_map(|(name, value)| (name == "dtype").then_some(value))
            .and_then(Json::as_str)
            .ok_or_else(|| {
                format_error(format!("Safetensors tensor `{key}` has no string `dtype`"))
            })?;
        let element_size = safetensors_dtype_size(dtype).ok_or_else(|| {
            format_error(format!(
                "Safetensors tensor `{key}` uses unsupported dtype `{dtype}`"
            ))
        })?;
        let shape = tensor
            .iter()
            .find_map(|(name, value)| (name == "shape").then_some(value))
            .and_then(Json::as_array)
            .ok_or_else(|| {
                format_error(format!("Safetensors tensor `{key}` has no array `shape`"))
            })?;
        if shape.len() > MAX_DIMS {
            return Err(format_error(format!(
                "Safetensors tensor `{key}` has rank {}, above the {MAX_DIMS} limit",
                shape.len()
            )));
        }
        let mut elements = 1_u64;
        for dimension in shape {
            let dimension = dimension.as_u64().ok_or_else(|| {
                format_error(format!(
                    "Safetensors tensor `{key}` has a non-integer shape dimension"
                ))
            })?;
            elements = elements.checked_mul(dimension).ok_or_else(|| {
                format_error(format!(
                    "Safetensors tensor `{key}` element count overflows"
                ))
            })?;
        }
        let offsets = tensor
            .iter()
            .find_map(|(name, value)| (name == "data_offsets").then_some(value))
            .and_then(Json::as_array)
            .ok_or_else(|| {
                format_error(format!(
                    "Safetensors tensor `{key}` has no array `data_offsets`"
                ))
            })?;
        if offsets.len() != 2 {
            return Err(format_error(format!(
                "Safetensors tensor `{key}` must have exactly two data offsets"
            )));
        }
        let start = offsets[0].as_u64().ok_or_else(|| {
            format_error(format!(
                "Safetensors tensor `{key}` has a non-integer start offset"
            ))
        })?;
        let end = offsets[1].as_u64().ok_or_else(|| {
            format_error(format!(
                "Safetensors tensor `{key}` has a non-integer end offset"
            ))
        })?;
        if start > end {
            return Err(format_error(format!(
                "Safetensors tensor `{key}` has start offset {start} after end offset {end}"
            )));
        }
        if end > data_bytes {
            return Err(format_error(format!(
                "Safetensors tensor `{key}` ends at byte {end}, beyond the {data_bytes} byte data section"
            )));
        }
        if start % element_size != 0 || end % element_size != 0 {
            return Err(format_error(format!(
                "Safetensors tensor `{key}` offsets {start}..{end} are not aligned to the {element_size} byte `{dtype}` element"
            )));
        }
        let expected_bytes = elements.checked_mul(element_size).ok_or_else(|| {
            format_error(format!("Safetensors tensor `{key}` byte length overflows"))
        })?;
        let actual_bytes = end - start;
        if actual_bytes != expected_bytes {
            return Err(format_error(format!(
                "Safetensors tensor `{key}` has {actual_bytes} data bytes but its shape and dtype require {expected_bytes}"
            )));
        }
        parameter_count = parameter_count.checked_add(elements).ok_or_else(|| {
            format_error("Safetensors parameter count overflows a 64-bit integer")
        })?;
        let entry = dtypes.entry(dtype.to_string()).or_insert((0, 0));
        entry.0 += 1;
        entry.1 = entry
            .1
            .checked_add(elements)
            .ok_or_else(|| format_error("Safetensors dtype parameter count overflows"))?;
        if actual_bytes > 0 {
            ranges.push((start, end, key.clone()));
        }
    }

    ranges.sort_by_key(|range| range.0);
    for pair in ranges.windows(2) {
        if pair[1].0 < pair[0].1 {
            return Err(format_error(format!(
                "Safetensors tensors `{}` and `{}` have overlapping data ranges",
                pair[0].2, pair[1].2
            )));
        }
    }
    if !header_len.is_multiple_of(8) {
        warnings.push(
            "header length is not a multiple of 8; some Safetensors readers may reject this file"
                .to_string(),
        );
    }

    let architecture = metadata
        .get("architecture")
        .or_else(|| metadata.get("general.architecture"))
        .cloned();
    let name = metadata
        .get("name")
        .or_else(|| metadata.get("general.name"))
        .cloned();
    let context_length = metadata
        .get("context_length")
        .and_then(|value| value.parse::<u64>().ok());
    let embedding_length = metadata
        .get("embedding_length")
        .or_else(|| metadata.get("hidden_size"))
        .and_then(|value| value.parse::<u64>().ok());
    let block_count = metadata
        .get("block_count")
        .or_else(|| metadata.get("num_hidden_layers"))
        .and_then(|value| value.parse::<u64>().ok());
    let quantization = metadata.get("quantization").cloned();
    let tokenizer_model = metadata
        .get("tokenizer_model")
        .or_else(|| metadata.get("tokenizer.ggml.model"))
        .cloned();

    let metadata_display = metadata.into_iter().collect();
    let mut dtypes: Vec<TensorTypeSummary> = dtypes
        .into_iter()
        .map(|(dtype, (tensors, parameters))| TensorTypeSummary {
            dtype,
            tensors,
            parameters,
        })
        .collect();
    dtypes.sort_by(|left, right| left.dtype.cmp(&right.dtype));

    Ok(ModelSummary {
        path: String::new(),
        format: ModelFormat::Safetensors,
        format_version: None,
        file_bytes,
        sha256: String::new(),
        tensor_count,
        parameter_count,
        data_bytes,
        architecture,
        name,
        context_length,
        embedding_length,
        block_count,
        quantization,
        tokenizer_model,
        tokenizer_tokens: None,
        metadata: metadata_display,
        dtypes,
        shards: Vec::new(),
        warnings,
    })
}

// === SECTION: SAFETENSORS SHARD INDEXES ===

fn starts_with_json_object(bytes: &[u8]) -> bool {
    bytes
        .iter()
        .copied()
        .find(|byte| !byte.is_ascii_whitespace())
        == Some(b'{')
}

fn inspect_index_from_reader(
    path: &Path,
    file_bytes: u64,
    first: &[u8; 8],
    reader: &mut impl Read,
) -> Result<ModelSummary, AiError> {
    if file_bytes > MAX_SHARD_INDEX_BYTES {
        return Err(format_error(format!(
            "Safetensors shard index is {file_bytes} bytes, above the {MAX_SHARD_INDEX_BYTES} byte limit"
        )));
    }
    let remaining = usize::try_from(file_bytes.saturating_sub(8))
        .map_err(|_| format_error("Safetensors shard index is too large to inspect"))?;
    let mut bytes = Vec::with_capacity(8 + remaining);
    bytes.extend_from_slice(first);
    let mut rest = vec![0_u8; remaining];
    reader.read_exact(&mut rest).map_err(|error| {
        if error.kind() == io::ErrorKind::UnexpectedEof {
            format_error("Safetensors shard index is truncated")
        } else {
            io_failure(error)
        }
    })?;
    bytes.extend_from_slice(&rest);
    inspect_index(path, &bytes)
}

fn inspect_index(path: &Path, bytes: &[u8]) -> Result<ModelSummary, AiError> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| format_error("Safetensors shard index is not valid UTF-8"))?;
    let root = json::parse(text)
        .map_err(|error| format_error(format!("invalid Safetensors shard-index JSON: {error}")))?;
    root.as_object()
        .ok_or_else(|| format_error("Safetensors shard index must be a JSON object"))?;
    let weight_map = root
        .get("weight_map")
        .and_then(Json::as_object)
        .ok_or_else(|| {
            format_error("Safetensors shard index must contain an object `weight_map`")
        })?;
    if weight_map.len() > MAX_TENSORS as usize {
        return Err(format_error(format!(
            "Safetensors shard index lists more than {MAX_TENSORS} tensors"
        )));
    }

    let mut metadata = BTreeMap::new();
    if let Some(value) = root.get("metadata") {
        let entries = value.as_object().ok_or_else(|| {
            format_error("Safetensors shard-index `metadata` must be a JSON object")
        })?;
        if entries.len() > MAX_SAFETENSORS_METADATA {
            return Err(format_error(format!(
                "Safetensors shard-index metadata has {} entries, above the {MAX_SAFETENSORS_METADATA} limit",
                entries.len()
            )));
        }
        for (key, value) in entries {
            let text = match value {
                Json::String(text) => text.clone(),
                other => other.to_compact_string(),
            };
            metadata.insert(key.clone(), text);
        }
    }

    let mut grouped: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (tensor, shard) in weight_map {
        if tensor.is_empty() {
            return Err(format_error(
                "Safetensors shard index contains an empty tensor name",
            ));
        }
        let shard = shard.as_str().ok_or_else(|| {
            format_error(format!(
                "Safetensors shard-index entry `{tensor}` must name a string shard"
            ))
        })?;
        let shard = normalize_shard_path(shard)?;
        grouped.entry(shard).or_default().insert(tensor.clone());
    }
    if grouped.len() > MAX_SHARDS {
        return Err(format_error(format!(
            "Safetensors shard index references {} shards, above the {MAX_SHARDS} limit",
            grouped.len()
        )));
    }

    let base = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let base_canonical = std::fs::canonicalize(base).map_err(|error| {
        AiError::new(
            AI_E_IO,
            format!(
                "cannot resolve Safetensors shard-index directory `{}`: {error}",
                base.display()
            ),
        )
    })?;

    let mut shards = Vec::new();
    let mut dtypes: BTreeMap<String, (u64, u64)> = BTreeMap::new();
    let mut warnings = Vec::new();
    let mut all_tensors = HashSet::new();
    let mut tensor_count = 0_u64;
    let mut parameter_count = 0_u64;
    let mut data_bytes = 0_u64;

    for (relative, expected) in &grouped {
        let full = base.join(relative);
        let canonical = std::fs::canonicalize(&full).map_err(|error| {
            AiError::new(
                AI_E_IO,
                format!("cannot resolve Safetensors shard `{relative}`: {error}"),
            )
        })?;
        if !canonical.starts_with(&base_canonical) {
            return Err(format_error(format!(
                "Safetensors shard `{relative}` escapes the index directory"
            )));
        }
        let summary = inspect_inner(&full, false)?;
        if summary.format != ModelFormat::Safetensors {
            return Err(format_error(format!(
                "Safetensors shard `{relative}` is {}, not a Safetensors file",
                summary.format.as_str()
            )));
        }
        let actual = safetensors_tensor_names(&full)?;
        for name in &actual {
            if !expected.contains(name) {
                return Err(format_error(format!(
                    "Safetensors shard `{relative}` contains tensor `{name}` that the index does not list"
                )));
            }
            if !all_tensors.insert(name.clone()) {
                return Err(format_error(format!(
                    "Safetensors tensor `{name}` appears in more than one shard"
                )));
            }
        }
        for name in expected {
            if !actual.contains(name) {
                return Err(format_error(format!(
                    "Safetensors index lists tensor `{name}` for shard `{relative}`, but the shard does not contain it"
                )));
            }
        }

        tensor_count = tensor_count
            .checked_add(summary.tensor_count)
            .ok_or_else(|| format_error("Safetensors shard-index tensor count overflows"))?;
        parameter_count = parameter_count
            .checked_add(summary.parameter_count)
            .ok_or_else(|| format_error("Safetensors shard-index parameter count overflows"))?;
        data_bytes = data_bytes
            .checked_add(summary.data_bytes)
            .ok_or_else(|| format_error("Safetensors shard-index data byte count overflows"))?;
        for dtype in &summary.dtypes {
            let entry = dtypes.entry(dtype.dtype.clone()).or_insert((0, 0));
            entry.0 = entry
                .0
                .checked_add(dtype.tensors)
                .ok_or_else(|| format_error("Safetensors dtype tensor count overflows"))?;
            entry.1 = entry
                .1
                .checked_add(dtype.parameters)
                .ok_or_else(|| format_error("Safetensors dtype parameter count overflows"))?;
        }
        for warning in &summary.warnings {
            warnings.push(format!("{relative}: {warning}"));
        }
        shards.push(ShardSummary {
            path: relative.clone(),
            file_bytes: summary.file_bytes,
            sha256: summary.sha256,
            tensor_count: summary.tensor_count,
            parameter_count: summary.parameter_count,
            data_bytes: summary.data_bytes,
        });
    }

    if let Some(total) = metadata.get("total_size") {
        match total.parse::<u64>() {
            Ok(total) if total != data_bytes => warnings.push(format!(
                "index metadata total_size {total} does not match summed shard data bytes {data_bytes}"
            )),
            Ok(_) => {}
            Err(_) => warnings.push(
                "index metadata `total_size` is not a non-negative integer".to_string(),
            ),
        }
    }

    let architecture = metadata
        .get("architecture")
        .or_else(|| metadata.get("general.architecture"))
        .cloned();
    let name = metadata
        .get("name")
        .or_else(|| metadata.get("general.name"))
        .cloned();
    let context_length = metadata
        .get("context_length")
        .and_then(|value| value.parse::<u64>().ok());
    let embedding_length = metadata
        .get("embedding_length")
        .or_else(|| metadata.get("hidden_size"))
        .and_then(|value| value.parse::<u64>().ok());
    let block_count = metadata
        .get("block_count")
        .or_else(|| metadata.get("num_hidden_layers"))
        .and_then(|value| value.parse::<u64>().ok());
    let quantization = metadata.get("quantization").cloned();
    let tokenizer_model = metadata
        .get("tokenizer_model")
        .or_else(|| metadata.get("tokenizer.ggml.model"))
        .cloned();

    let metadata_display = metadata.into_iter().collect();
    let mut dtypes: Vec<TensorTypeSummary> = dtypes
        .into_iter()
        .map(|(dtype, (tensors, parameters))| TensorTypeSummary {
            dtype,
            tensors,
            parameters,
        })
        .collect();
    dtypes.sort_by(|left, right| left.dtype.cmp(&right.dtype));

    Ok(ModelSummary {
        path: String::new(),
        format: ModelFormat::SafetensorsIndex,
        format_version: None,
        file_bytes: bytes.len() as u64,
        sha256: String::new(),
        tensor_count,
        parameter_count,
        data_bytes,
        architecture,
        name,
        context_length,
        embedding_length,
        block_count,
        quantization,
        tokenizer_model,
        tokenizer_tokens: None,
        metadata: metadata_display,
        dtypes,
        shards,
        warnings,
    })
}

fn normalize_shard_path(name: &str) -> Result<String, AiError> {
    if name.is_empty() {
        return Err(format_error(
            "Safetensors shard index contains an empty shard path",
        ));
    }
    if name.contains('\0') {
        return Err(format_error(format!(
            "Safetensors shard path `{name}` contains a NUL byte"
        )));
    }
    if name.contains('\\') {
        return Err(format_error(format!(
            "Safetensors shard path `{name}` must use `/` separators"
        )));
    }
    let mut normalized = PathBuf::new();
    for component in Path::new(name).components() {
        match component {
            Component::Normal(part) => normalized.push(part),
            Component::CurDir => {}
            Component::ParentDir => {
                return Err(format_error(format!(
                    "Safetensors shard path `{name}` must not contain `..`"
                )));
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(format_error(format!(
                    "Safetensors shard path `{name}` must be relative"
                )));
            }
        }
    }
    if normalized.as_os_str().is_empty() {
        return Err(format_error(format!(
            "Safetensors shard path `{name}` does not name a file"
        )));
    }
    if normalized
        .extension()
        .and_then(|extension| extension.to_str())
        != Some("safetensors")
    {
        return Err(format_error(format!(
            "Safetensors shard path `{name}` must end in `.safetensors`"
        )));
    }
    normalized.to_str().map(str::to_string).ok_or_else(|| {
        format_error(format!(
            "Safetensors shard path `{name}` is not valid UTF-8"
        ))
    })
}

fn safetensors_tensor_names(path: &Path) -> Result<BTreeSet<String>, AiError> {
    let mut file = File::open(path).map_err(|error| {
        AiError::new(
            AI_E_IO,
            format!(
                "cannot open Safetensors shard `{}`: {error}",
                path.display()
            ),
        )
    })?;
    let mut length = [0_u8; 8];
    file.read_exact(&mut length)
        .map_err(|error| format_error(format!("Safetensors shard is truncated: {error}")))?;
    let header_len = u64::from_le_bytes(length);
    if header_len == 0 || header_len > MAX_SAFETENSORS_HEADER_BYTES {
        return Err(format_error(
            "Safetensors shard has an invalid header length",
        ));
    }
    let header_len = usize::try_from(header_len)
        .map_err(|_| format_error("Safetensors shard header is too large to inspect"))?;
    let mut header = vec![0_u8; header_len];
    file.read_exact(&mut header)
        .map_err(|error| format_error(format!("Safetensors shard is truncated: {error}")))?;
    let text = std::str::from_utf8(&header)
        .map_err(|_| format_error("Safetensors shard header is not valid UTF-8"))?;
    let root = json::parse(text)
        .map_err(|error| format_error(format!("invalid Safetensors shard header JSON: {error}")))?;
    let entries = root
        .as_object()
        .ok_or_else(|| format_error("Safetensors shard header must be a JSON object"))?;
    let mut names = BTreeSet::new();
    for (key, _) in entries {
        if key != "__metadata__" {
            names.insert(key.clone());
        }
    }
    Ok(names)
}

fn skip_exact(reader: &mut impl Read, length: u64, what: &str) -> Result<(), AiError> {
    let mut remaining = length;
    let mut scratch = [0_u8; 8 * 1024];
    while remaining > 0 {
        let chunk = remaining.min(scratch.len() as u64) as usize;
        reader.read_exact(&mut scratch[..chunk]).map_err(|error| {
            if error.kind() == io::ErrorKind::UnexpectedEof {
                format_error(format!("{what} is truncated"))
            } else {
                io_failure(error)
            }
        })?;
        remaining -= chunk as u64;
    }
    Ok(())
}

// === SECTION: TESTS ===

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_file(name: &str, bytes: &[u8]) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "vantyx-model-{}-{}-{name}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&path, bytes).expect("write model fixture");
        path
    }

    fn push_string(output: &mut Vec<u8>, value: &str) {
        output.extend((value.len() as u64).to_le_bytes());
        output.extend(value.as_bytes());
    }

    fn minimal_gguf() -> Vec<u8> {
        let mut output = Vec::new();
        output.extend(b"GGUF");
        output.extend(3_u32.to_le_bytes());
        output.extend(1_u64.to_le_bytes());
        output.extend(4_u64.to_le_bytes());

        push_string(&mut output, "general.architecture");
        output.extend(8_u32.to_le_bytes());
        push_string(&mut output, "llama");

        push_string(&mut output, "llama.context_length");
        output.extend(4_u32.to_le_bytes());
        output.extend(4096_u32.to_le_bytes());

        push_string(&mut output, "general.file_type");
        output.extend(4_u32.to_le_bytes());
        output.extend(15_u32.to_le_bytes());

        push_string(&mut output, "tokenizer.ggml.tokens");
        output.extend(9_u32.to_le_bytes());
        output.extend(8_u32.to_le_bytes());
        output.extend(2_u64.to_le_bytes());
        push_string(&mut output, "a");
        push_string(&mut output, "b");

        push_string(&mut output, "token_embd.weight");
        output.extend(2_u32.to_le_bytes());
        output.extend(4_u64.to_le_bytes());
        output.extend(2_u64.to_le_bytes());
        output.extend(0_u32.to_le_bytes());
        output.extend(0_u64.to_le_bytes());

        while output.len() % 32 != 0 {
            output.push(0);
        }
        output.extend([0_u8; 4 * 2 * 4]);
        output
    }

    fn minimal_safetensors() -> Vec<u8> {
        let mut header = br#"{"__metadata__":{"format":"pt","architecture":"llama"},"weight":{"dtype":"F32","shape":[2,3],"data_offsets":[0,24]}}"#.to_vec();
        while !header.len().is_multiple_of(8) {
            header.push(b' ');
        }
        let mut output = Vec::new();
        output.extend((header.len() as u64).to_le_bytes());
        output.extend(&header);
        output.extend([0_u8; 24]);
        output
    }

    fn safetensors_with(tensors: &[(&str, &str, &[u64])]) -> Vec<u8> {
        let mut header = String::from("{");
        let mut offset = 0_u64;
        for (index, (name, dtype, shape)) in tensors.iter().enumerate() {
            if index > 0 {
                header.push(',');
            }
            let element_size = safetensors_dtype_size(dtype).expect("test dtype is supported");
            let elements = shape.iter().copied().product::<u64>();
            let length = elements * element_size;
            let shape_text = shape
                .iter()
                .map(u64::to_string)
                .collect::<Vec<_>>()
                .join(",");
            header.push_str(&format!(
                "\"{name}\":{{\"dtype\":\"{dtype}\",\"shape\":[{shape_text}],\"data_offsets\":[{offset},{}]}}",
                offset + length
            ));
            offset += length;
        }
        header.push('}');
        let mut header = header.into_bytes();
        while !header.len().is_multiple_of(8) {
            header.push(b' ');
        }
        let mut output = Vec::new();
        output.extend((header.len() as u64).to_le_bytes());
        output.extend(&header);
        output.extend(vec![0_u8; offset as usize]);
        output
    }

    fn temp_dir(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "vantyx-model-dir-{}-{}-{name}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).expect("create model fixture directory");
        path
    }

    #[test]
    fn sha256_matches_known_vectors() {
        let cases = [
            (
                "",
                "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            ),
            (
                "abc",
                "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
            ),
            (
                "abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq",
                "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1",
            ),
        ];
        for (input, expected) in cases {
            let mut hasher = Sha256::new();
            hasher.update(input.as_bytes());
            assert_eq!(hasher.finalize_hex(), expected, "input {input:?}");
        }
        let mut hasher = Sha256::new();
        for chunk in [b"a".as_slice(), b"b".as_slice(), b"c".as_slice()] {
            hasher.update(chunk);
        }
        assert_eq!(
            hasher.finalize_hex(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let million = vec![b'a'; 1_000_000];
        let mut hasher = Sha256::new();
        hasher.update(&million);
        assert_eq!(
            hasher.finalize_hex(),
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
        );
    }

    #[test]
    fn inspects_minimal_gguf() {
        let bytes = minimal_gguf();
        let path = temp_file("minimal.gguf", &bytes);
        let summary = inspect(&path).expect("inspect GGUF");
        assert_eq!(summary.format, ModelFormat::Gguf);
        assert_eq!(summary.format_version, Some(3));
        assert_eq!(summary.tensor_count, 1);
        assert_eq!(summary.parameter_count, 8);
        assert_eq!(summary.data_bytes, 32);
        assert_eq!(summary.architecture.as_deref(), Some("llama"));
        assert_eq!(summary.context_length, Some(4096));
        assert_eq!(summary.quantization.as_deref(), Some("MOSTLY_Q4_K_M"));
        assert_eq!(summary.tokenizer_tokens, Some(2));
        assert_eq!(
            summary.dtypes,
            vec![TensorTypeSummary {
                dtype: "F32".to_string(),
                tensors: 1,
                parameters: 8,
            }]
        );
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        assert_eq!(summary.sha256, hasher.finalize_hex());
        assert_eq!(summary.sha256.len(), 64);
        assert_eq!(summary.file_bytes, bytes.len() as u64);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn inspects_minimal_safetensors() {
        let bytes = minimal_safetensors();
        let path = temp_file("minimal.safetensors", &bytes);
        let summary = inspect(&path).expect("inspect Safetensors");
        assert_eq!(summary.format, ModelFormat::Safetensors);
        assert_eq!(summary.format_version, None);
        assert_eq!(summary.tensor_count, 1);
        assert_eq!(summary.parameter_count, 6);
        assert_eq!(summary.data_bytes, 24);
        assert_eq!(summary.architecture.as_deref(), Some("llama"));
        assert_eq!(
            summary
                .metadata
                .iter()
                .find(|(key, _)| key == "format")
                .map(|(_, value)| value.as_str()),
            Some("pt")
        );
        assert_eq!(
            summary.dtypes,
            vec![TensorTypeSummary {
                dtype: "F32".to_string(),
                tensors: 1,
                parameters: 6,
            }]
        );
        assert_eq!(summary.file_bytes, bytes.len() as u64);
        let _ = std::fs::remove_file(path);

        // A header length of 123 has 0x7b (`{`) as its first byte. Content
        // detection must prefer the plausible length over the JSON prefix.
        let mut header = br#"{"w":{"dtype":"F32","shape":[1],"data_offsets":[0,4]}}"#.to_vec();
        while header.len() < 123 {
            header.push(b' ');
        }
        assert_eq!(header.len(), 123);
        let mut brace_bytes = Vec::new();
        brace_bytes.extend(123_u64.to_le_bytes());
        brace_bytes.extend(&header);
        brace_bytes.extend([0_u8; 4]);
        assert_eq!(brace_bytes[0], b'{');
        let path = temp_file("brace-header.safetensors", &brace_bytes);
        let summary =
            inspect(&path).expect("a header length starting with `{` is still Safetensors");
        assert_eq!(summary.format, ModelFormat::Safetensors);
        assert_eq!(summary.tensor_count, 1);
        assert_eq!(summary.parameter_count, 1);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn summary_json_has_stable_fields() {
        let bytes = minimal_safetensors();
        let path = temp_file("summary.safetensors", &bytes);
        let summary = inspect(&path).expect("inspect Safetensors");
        let json = summary.to_json();
        assert_eq!(
            json.get("format").and_then(Json::as_str),
            Some("safetensors")
        );
        assert_eq!(json.get("parameter_count").and_then(Json::as_u64), Some(6));
        assert_eq!(
            json.get("sha256").and_then(Json::as_str),
            Some(summary.sha256.as_str())
        );
        let reparsed = json::parse(&json.to_compact_string()).expect("summary JSON is valid");
        assert_eq!(reparsed.get("tensor_count").and_then(Json::as_u64), Some(1));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn rejects_truncated_gguf() {
        let mut bytes = minimal_gguf();
        bytes.truncate(20);
        let path = temp_file("truncated.gguf", &bytes);
        let error = inspect(&path).expect_err("truncated GGUF must fail");
        assert_eq!(error.code, AI_E_FORMAT);
        assert!(error.message.contains("truncated"), "{}", error.message);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn rejects_unknown_format_and_bad_json() {
        let path = temp_file("unknown.bin", &[0_u8; 32]);
        let error = inspect(&path).expect_err("zero header must fail");
        assert_eq!(error.code, AI_E_FORMAT);
        assert!(error.message.contains("unsupported model format"));
        let _ = std::fs::remove_file(path);

        let mut header = b"{not json}".to_vec();
        while !header.len().is_multiple_of(8) {
            header.push(b' ');
        }
        let mut bytes = Vec::new();
        bytes.extend((header.len() as u64).to_le_bytes());
        bytes.extend(&header);
        let path = temp_file("bad.safetensors", &bytes);
        let error = inspect(&path).expect_err("bad JSON must fail");
        assert_eq!(error.code, AI_E_FORMAT);
        assert!(error.message.contains("invalid Safetensors header JSON"));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn rejects_safetensors_overlap_and_range_errors() {
        let header = br#"{"a":{"dtype":"F32","shape":[2],"data_offsets":[0,8]},"b":{"dtype":"F32","shape":[2],"data_offsets":[4,12]}}"#;
        let mut header = header.to_vec();
        while !header.len().is_multiple_of(8) {
            header.push(b' ');
        }
        let mut bytes = Vec::new();
        bytes.extend((header.len() as u64).to_le_bytes());
        bytes.extend(&header);
        bytes.extend([0_u8; 12]);
        let path = temp_file("overlap.safetensors", &bytes);
        let error = inspect(&path).expect_err("overlap must fail");
        assert!(error.message.contains("overlapping"), "{}", error.message);
        let _ = std::fs::remove_file(path);

        let header = br#"{"a":{"dtype":"F32","shape":[2],"data_offsets":[0,12]}}"#;
        let mut header = header.to_vec();
        while !header.len().is_multiple_of(8) {
            header.push(b' ');
        }
        let mut bytes = Vec::new();
        bytes.extend((header.len() as u64).to_le_bytes());
        bytes.extend(&header);
        bytes.extend([0_u8; 8]);
        let path = temp_file("range.safetensors", &bytes);
        let error = inspect(&path).expect_err("range must fail");
        assert!(
            error.message.contains("beyond the 8 byte data section"),
            "{}",
            error.message
        );
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn rejects_gguf_tensor_beyond_data_section() {
        let mut bytes = minimal_gguf();
        // The single tensor is 4 x 2 F32 values. Claim 4 x 2 x 4 bytes but
        // leave only 32 bytes of data by shrinking the final payload.
        bytes.truncate(bytes.len() - 1);
        let path = temp_file("short-data.gguf", &bytes);
        let error = inspect(&path).expect_err("short data must fail");
        assert_eq!(error.code, AI_E_FORMAT);
        assert!(
            error.message.contains("exceeds the") && error.message.contains("data section"),
            "{}",
            error.message
        );
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn rejects_non_utf8_safetensors_header() {
        let mut header = vec![b'{', 0xff_u8, 0xfe, 0xfd];
        while !header.len().is_multiple_of(8) {
            header.push(b' ');
        }
        let mut bytes = Vec::new();
        bytes.extend((header.len() as u64).to_le_bytes());
        bytes.extend(&header);
        let path = temp_file("nonutf8.safetensors", &bytes);
        let error = inspect(&path).expect_err("non-UTF-8 header must fail");
        assert!(error.message.contains("not valid UTF-8"));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn inspects_safetensors_index_and_aggregates_shards() {
        let directory = temp_dir("index");
        std::fs::write(
            directory.join("a.safetensors"),
            safetensors_with(&[("a", "F32", &[2])]),
        )
        .unwrap();
        std::fs::write(
            directory.join("b.safetensors"),
            safetensors_with(&[("b", "I16", &[3])]),
        )
        .unwrap();
        let index = r#"{"metadata":{"total_size":14,"architecture":"llama","name":"tiny"},"weight_map":{"a":"a.safetensors","b":"b.safetensors"}}"#;
        let index_path = directory.join("model.safetensors.index.json");
        std::fs::write(&index_path, index).unwrap();

        let summary = inspect(&index_path).expect("inspect shard index");
        assert_eq!(summary.format, ModelFormat::SafetensorsIndex);
        assert_eq!(summary.tensor_count, 2);
        assert_eq!(summary.parameter_count, 5);
        assert_eq!(summary.data_bytes, 14);
        assert_eq!(summary.architecture.as_deref(), Some("llama"));
        assert_eq!(summary.name.as_deref(), Some("tiny"));
        assert_eq!(summary.shards.len(), 2);
        assert_eq!(summary.shards[0].path, "a.safetensors");
        assert_eq!(summary.shards[0].tensor_count, 1);
        assert_eq!(summary.shards[0].parameter_count, 2);
        assert_eq!(summary.shards[0].sha256.len(), 64);
        assert_eq!(summary.shards[1].path, "b.safetensors");
        assert_eq!(summary.shards[1].parameter_count, 3);
        assert_eq!(
            summary.dtypes,
            vec![
                TensorTypeSummary {
                    dtype: "F32".to_string(),
                    tensors: 1,
                    parameters: 2,
                },
                TensorTypeSummary {
                    dtype: "I16".to_string(),
                    tensors: 1,
                    parameters: 3,
                },
            ]
        );
        assert!(summary.warnings.is_empty(), "{:?}", summary.warnings);
        let mut hasher = Sha256::new();
        hasher.update(index.as_bytes());
        assert_eq!(summary.sha256, hasher.finalize_hex());
        assert_eq!(summary.file_bytes, index.len() as u64);
        let json = summary.to_json();
        assert_eq!(
            json.get("format").and_then(Json::as_str),
            Some("safetensors-index")
        );
        assert_eq!(
            json.get("shards").and_then(Json::as_array).map(|s| s.len()),
            Some(2)
        );
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn shard_index_warns_when_total_size_disagrees() {
        let directory = temp_dir("index-size-warning");
        std::fs::write(
            directory.join("a.safetensors"),
            safetensors_with(&[("a", "F32", &[2])]),
        )
        .unwrap();
        let index_path = directory.join("model.safetensors.index.json");
        std::fs::write(
            &index_path,
            r#"{"metadata":{"total_size":99},"weight_map":{"a":"a.safetensors"}}"#,
        )
        .unwrap();
        let summary = inspect(&index_path).expect("inspect shard index");
        assert_eq!(summary.data_bytes, 8);
        assert!(
            summary
                .warnings
                .iter()
                .any(|warning| warning.contains("total_size 99")),
            "{:?}",
            summary.warnings
        );
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn shard_index_rejects_inconsistent_or_unsafe_entries() {
        let directory = temp_dir("index-errors");
        std::fs::write(
            directory.join("a.safetensors"),
            safetensors_with(&[("a", "F32", &[2])]),
        )
        .unwrap();

        let path = directory.join("missing-map.safetensors.index.json");
        std::fs::write(&path, "{}      ").unwrap();
        let error = inspect(&path).expect_err("missing weight_map must fail");
        assert_eq!(error.code, AI_E_FORMAT);
        assert!(error.message.contains("weight_map"), "{}", error.message);

        for (name, shard) in [
            ("escape", "../a.safetensors"),
            ("absolute", "/tmp/a.safetensors"),
            ("wrong-ext", "a.bin"),
            ("backslash", r"a\\b.safetensors"),
        ] {
            let path = directory.join(format!("{name}.safetensors.index.json"));
            std::fs::write(&path, format!(r#"{{"weight_map":{{"a":"{shard}"}}}}"#)).unwrap();
            let error = inspect(&path).expect_err("unsafe shard path must fail");
            assert_eq!(error.code, AI_E_FORMAT, "{name}");
        }

        std::fs::write(
            directory.join("extra.safetensors"),
            safetensors_with(&[("a", "F32", &[2]), ("x", "F32", &[1])]),
        )
        .unwrap();
        let path = directory.join("extra.safetensors.index.json");
        std::fs::write(&path, r#"{"weight_map":{"a":"extra.safetensors"}}"#).unwrap();
        let error = inspect(&path).expect_err("extra tensor must fail");
        assert!(error.message.contains("does not list"), "{}", error.message);

        let path = directory.join("missing-tensor.safetensors.index.json");
        std::fs::write(
            &path,
            r#"{"weight_map":{"a":"a.safetensors","b":"a.safetensors"}}"#,
        )
        .unwrap();
        let error = inspect(&path).expect_err("missing tensor must fail");
        assert!(
            error.message.contains("does not contain"),
            "{}",
            error.message
        );

        let path = directory.join("absent.safetensors.index.json");
        std::fs::write(&path, r#"{"weight_map":{"a":"absent.safetensors"}}"#).unwrap();
        let error = inspect(&path).expect_err("missing shard must fail");
        assert_eq!(error.code, AI_E_IO);

        std::fs::write(directory.join("gguf.safetensors"), minimal_gguf()).unwrap();
        let path = directory.join("gguf.safetensors.index.json");
        std::fs::write(&path, r#"{"weight_map":{"a":"gguf.safetensors"}}"#).unwrap();
        let error = inspect(&path).expect_err("wrong shard format must fail");
        assert!(
            error.message.contains("not a Safetensors file"),
            "{}",
            error.message
        );

        let path = directory.join("bad-value.safetensors.index.json");
        std::fs::write(&path, r#"{"weight_map":{"a":1}}"#).unwrap();
        let error = inspect(&path).expect_err("non-string shard must fail");
        assert_eq!(error.code, AI_E_FORMAT);

        std::fs::write(
            directory.join("nested.safetensors"),
            r#"{"weight_map":{"a":"a.safetensors"}}"#,
        )
        .unwrap();
        let path = directory.join("nested.safetensors.index.json");
        std::fs::write(&path, r#"{"weight_map":{"a":"nested.safetensors"}}"#).unwrap();
        let error = inspect(&path).expect_err("nested index must fail");
        assert_eq!(error.code, AI_E_FORMAT);
        assert!(
            error
                .message
                .contains("shard index, not a Safetensors data file"),
            "{}",
            error.message
        );

        let _ = std::fs::remove_dir_all(directory);
    }

    #[cfg(unix)]
    #[test]
    fn shard_index_rejects_symlink_escape() {
        let directory = temp_dir("index-symlink");
        let outside = temp_file(
            "outside.safetensors",
            &safetensors_with(&[("a", "F32", &[1])]),
        );
        std::os::unix::fs::symlink(&outside, directory.join("link.safetensors")).unwrap();
        let index_path = directory.join("model.safetensors.index.json");
        std::fs::write(&index_path, r#"{"weight_map":{"a":"link.safetensors"}}"#).unwrap();
        let error = inspect(&index_path).expect_err("symlink escape must fail");
        assert_eq!(error.code, AI_E_FORMAT);
        assert!(error.message.contains("escapes"), "{}", error.message);
        let _ = std::fs::remove_file(outside);
        let _ = std::fs::remove_dir_all(directory);
    }
}
