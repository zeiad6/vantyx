//! Local-first retrieval-augmented generation foundations.
//!
//! This module deliberately stops before generation. It ingests text documents,
//! chunks them with stable ids and line citations, embeds chunks through the
//! provider boundary, builds a hybrid lexical/vector index, persists that index
//! as strict JSON, and evaluates retrieval quality. It never downloads a model,
//! calls a network by itself, or claims that a generated answer is grounded.
//!
//! Security boundaries:
//!
//! - Ingestion is confined to one canonical root, skips symlinks and hidden
//!   paths, and enforces per-file, total-byte, file-count, and depth limits.
//! - The index loader validates every field, vector dimension, id, and size
//!   before rebuilding the lexical index. A malformed or hostile index is
//!   rejected rather than partially accepted.
//! - Saved indexes are written atomically with owner-only permissions on Unix,
//!   because document text is frequently private.

use super::bm25::Bm25Index;
use super::embedding::Vector;
use super::json::Json;
use super::provider::{
    AI_E_CAPABILITY, AI_E_INVALID_REQUEST, AI_E_IO, AI_E_JSON, AiError, EmbeddingRequest,
    EmbeddingUsage, MAX_EMBEDDING_DIMENSIONS, MAX_EMBEDDING_ITEM_BYTES,
    Provider,
};
use super::budget::Budget;
use std::collections::HashSet;
use std::fs;
use std::io::Write as _;
use std::path::Path;

/// Version of the persisted index schema.
pub const INDEX_FORMAT_VERSION: u64 = 1;

pub const MAX_FILES: usize = 4_096;
pub const MAX_FILE_BYTES: usize = 1024 * 1024;
pub const MAX_TOTAL_SOURCE_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_TOTAL_CHUNK_BYTES: usize = 48 * 1024 * 1024;
pub const MAX_CHUNKS: usize = 10_000;
pub const MAX_CHUNK_CHARS: usize = 4_096;
pub const MIN_CHUNK_CHARS: usize = 64;
pub const MAX_OVERLAP_CHARS: usize = 2_048;
pub const DEFAULT_CHUNK_CHARS: usize = 900;
pub const DEFAULT_OVERLAP_CHARS: usize = 120;
pub const MAX_QUERY_BYTES: usize = 8 * 1024;
pub const MAX_TOP_K: usize = 100;
pub const MAX_INDEX_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_EVAL_QUERIES: usize = 1_000;
pub const MAX_RELEVANT_PER_QUERY: usize = 64;
pub const MAX_SOURCE_BYTES: usize = 1_024;
pub const MAX_MODEL_BYTES: usize = 256;
const MAX_WALK_DEPTH: usize = 32;
const MAX_EMBED_BATCH_INPUTS: usize = 64;
const MAX_EMBED_BATCH_BYTES: usize = 24 * 1024;
const RRF_K: f32 = 60.0;

/// Extensions accepted by the recursive ingester unless overridden.
pub const DEFAULT_EXTENSIONS: &[&str] = &[
    "c", "cc", "cpp", "css", "go", "h", "hpp", "html", "java", "js", "json", "jsx", "kt", "md",
    "markdown", "org", "php", "py", "rb", "rst", "rs", "sh", "sql", "toml", "ts", "tsx", "txt",
    "vtx", "yaml", "yml",
];

const SKIPPED_DIRECTORIES: &[&str] = &[
    ".git",
    ".hg",
    ".svn",
    "node_modules",
    "target",
    "__pycache__",
    ".venv",
    "venv",
    "dist",
    "build",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChunkOptions {
    pub max_chars: usize,
    pub overlap_chars: usize,
}

impl Default for ChunkOptions {
    fn default() -> Self {
        Self {
            max_chars: DEFAULT_CHUNK_CHARS,
            overlap_chars: DEFAULT_OVERLAP_CHARS,
        }
    }
}

impl ChunkOptions {
    pub fn new(max_chars: usize, overlap_chars: usize) -> Result<Self, AiError> {
        let options = Self {
            max_chars,
            overlap_chars,
        };
        options.validate()?;
        Ok(options)
    }

    pub fn validate(&self) -> Result<(), AiError> {
        if !(MIN_CHUNK_CHARS..=MAX_CHUNK_CHARS).contains(&self.max_chars) {
            return Err(AiError::new(
                AI_E_INVALID_REQUEST,
                format!(
                    "chunk size must be between {MIN_CHUNK_CHARS} and {MAX_CHUNK_CHARS} characters"
                ),
            ));
        }
        if self.overlap_chars > MAX_OVERLAP_CHARS || self.overlap_chars >= self.max_chars {
            return Err(AiError::new(
                AI_E_INVALID_REQUEST,
                format!(
                    "chunk overlap must be at most {MAX_OVERLAP_CHARS} characters and smaller than the chunk size"
                ),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chunk {
    pub id: String,
    pub source: String,
    pub start_line: usize,
    pub end_line: usize,
    pub text: String,
}

#[derive(Clone, Debug)]
pub struct IngestOptions {
    pub extensions: Vec<String>,
    pub max_files: usize,
    pub max_file_bytes: usize,
    pub max_total_bytes: usize,
    pub follow_symlinks: bool,
}

impl Default for IngestOptions {
    fn default() -> Self {
        Self {
            extensions: DEFAULT_EXTENSIONS.iter().map(|ext| ext.to_string()).collect(),
            max_files: MAX_FILES,
            max_file_bytes: MAX_FILE_BYTES,
            max_total_bytes: MAX_TOTAL_SOURCE_BYTES,
            // Kept as an explicit field so callers can see that following
            // symlinks is denied; it is not configurable to true today.
            follow_symlinks: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Document {
    /// Path relative to the ingestion root, using `/` separators.
    pub source: String,
    pub text: String,
}

/// Splits one document into stable, line-cited chunks.
pub fn chunk_document(
    source: &str,
    text: &str,
    options: &ChunkOptions,
) -> Result<Vec<Chunk>, AiError> {
    options.validate()?;
    if source.trim().is_empty() {
        return Err(AiError::new(
            AI_E_INVALID_REQUEST,
            "chunk source must not be empty",
        ));
    }
    if source.len() > MAX_SOURCE_BYTES || source.chars().any(char::is_control) {
        return Err(AiError::new(
            AI_E_INVALID_REQUEST,
            format!("chunk source must be at most {MAX_SOURCE_BYTES} bytes without control characters"),
        ));
    }
    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        return Ok(Vec::new());
    }

    let mut chunks = Vec::new();
    let mut current: Vec<(usize, &str)> = Vec::new();
    let mut current_chars = 0usize;

    for (index, line) in lines.iter().enumerate() {
        let line_number = index + 1;
        let line_chars = line.chars().count();

        // A single pathological line is split at character boundaries so the
        // index never silently drops data.
        if line_chars > options.max_chars {
            push_lines(&mut chunks, source, &current)?;
            current.clear();
            current_chars = 0;
            let mut byte_start = 0usize;
            while byte_start < line.len() {
                let mut byte_end = byte_start;
                for (count, (offset, character)) in
                    line[byte_start..].char_indices().enumerate()
                {
                    if count == options.max_chars {
                        break;
                    }
                    byte_end = byte_start + offset + character.len_utf8();
                }
                if byte_end == byte_start {
                    break;
                }
                let piece = &line[byte_start..byte_end];
                push_lines(&mut chunks, source, &[(line_number, piece)])?;
                byte_start = byte_end;
            }
            continue;
        }

        let projected = if current.is_empty() {
            line_chars
        } else {
            current_chars.saturating_add(1).saturating_add(line_chars)
        };
        if !current.is_empty() && projected > options.max_chars {
            push_lines(&mut chunks, source, &current)?;
            let tail = overlap_tail(&current, options.overlap_chars);
            current = tail;
            current_chars = current
                .iter()
                .map(|(_, text)| text.chars().count())
                .sum::<usize>()
                .saturating_add(current.len().saturating_sub(1));
        }
        current.push((line_number, line));
        current_chars = current_chars.saturating_add(line_chars);
        if current.len() > 1 {
            current_chars = current_chars.saturating_add(1);
        }
    }
    push_lines(&mut chunks, source, &current)?;

    if chunks.len() > MAX_CHUNKS {
        return Err(AiError::new(
            AI_E_INVALID_REQUEST,
            format!(
                "document `{source}` produced {} chunks; the limit is {MAX_CHUNKS}",
                chunks.len()
            ),
        ));
    }
    Ok(chunks)
}

fn overlap_tail<'a>(lines: &[(usize, &'a str)], overlap_chars: usize) -> Vec<(usize, &'a str)> {
    if overlap_chars == 0 || lines.len() < 2 {
        return Vec::new();
    }
    let mut tail = Vec::new();
    let mut chars = 0usize;
    for &(number, line) in lines.iter().rev() {
        // Always leave at least one line in the previous chunk and never copy
        // the whole chunk, which guarantees forward progress.
        if tail.len() + 1 >= lines.len() {
            break;
        }
        let cost = line.chars().count().saturating_add(1);
        if chars.saturating_add(cost) > overlap_chars {
            break;
        }
        chars = chars.saturating_add(cost);
        tail.push((number, line));
    }
    tail.reverse();
    tail
}

fn push_lines(
    chunks: &mut Vec<Chunk>,
    source: &str,
    lines: &[(usize, &str)],
) -> Result<(), AiError> {
    let Some(&(start_line, _)) = lines.first() else {
        return Ok(());
    };
    let end_line = lines.last().map(|(number, _)| *number).unwrap_or(start_line);
    let text = lines
        .iter()
        .map(|(_, line)| *line)
        .collect::<Vec<_>>()
        .join("\n");
    if text.trim().is_empty() {
        return Ok(());
    }
    if text.len() > MAX_EMBEDDING_ITEM_BYTES {
        return Err(AiError::new(
            AI_E_INVALID_REQUEST,
            format!(
                "chunk at `{source}` lines {start_line}-{end_line} is {} bytes; the embedding input limit is {MAX_EMBEDDING_ITEM_BYTES}; reduce --chunk-chars",
                text.len()
            ),
        ));
    }
    if chunks.len() >= MAX_CHUNKS {
        return Err(AiError::new(
            AI_E_INVALID_REQUEST,
            format!("chunk count exceeds the limit of {MAX_CHUNKS}"),
        ));
    }
    let id = stable_chunk_id(source, start_line, end_line, &text);
    chunks.push(Chunk {
        id,
        source: source.to_string(),
        start_line,
        end_line,
        text,
    });
    Ok(())
}

/// Stable FNV-1a id over source, line range, and text. Not a cryptographic hash.
pub fn stable_chunk_id(source: &str, start_line: usize, end_line: usize, text: &str) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in source
        .bytes()
        .chain(std::iter::once(0))
        .chain(start_line.to_le_bytes())
        .chain(end_line.to_le_bytes())
        .chain(text.as_bytes().iter().copied())
    {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{source}#L{start_line}-L{end_line}#{hash:016x}")
}

/// Recursively ingests text documents under `root`.
///
/// Hidden files and directories, symlinks, VCS/build directories, and files
/// whose extension is not allowlisted are skipped. Sorted traversal makes the
/// result deterministic across platforms.
pub fn collect_documents(root: &Path, options: &IngestOptions) -> Result<Vec<Document>, AiError> {
    if options.max_files == 0 || options.max_file_bytes == 0 || options.max_total_bytes == 0 {
        return Err(AiError::new(
            AI_E_INVALID_REQUEST,
            "ingestion limits must be greater than zero",
        ));
    }
    if options.extensions.is_empty() {
        return Err(AiError::new(
            AI_E_INVALID_REQUEST,
            "ingestion requires at least one allowed extension",
        ));
    }
    let extensions: HashSet<String> = options
        .extensions
        .iter()
        .map(|extension| extension.trim_start_matches('.').to_ascii_lowercase())
        .collect();
    let canonical = fs::canonicalize(root).map_err(|error| io_error(root, error))?;
    let metadata = fs::symlink_metadata(&canonical).map_err(|error| io_error(root, error))?;
    if metadata.file_type().is_symlink() {
        return Err(AiError::new(
            AI_E_INVALID_REQUEST,
            format!("ingestion root `{}` is a symlink", root.display()),
        ));
    }
    if metadata.is_file() {
        let name = canonical
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        if name.starts_with('.') || !allowed_extension(&canonical, &extensions) {
            return Err(AiError::new(
                AI_E_INVALID_REQUEST,
                format!(
                    "document `{}` is hidden or has an extension outside the allowlist",
                    root.display()
                ),
            ));
        }
        let mut total = 0usize;
        let document = read_document(&canonical, &name, options, &mut total)?;
        return Ok(document.into_iter().collect());
    }
    if !metadata.is_dir() {
        return Err(AiError::new(
            AI_E_INVALID_REQUEST,
            format!("ingestion root `{}` is not a file or directory", root.display()),
        ));
    }

    let mut documents = Vec::new();
    let mut total_bytes = 0usize;
    let mut stack = vec![(canonical.clone(), 0usize)];
    while let Some((directory, depth)) = stack.pop() {
        let mut entries = Vec::new();
        for entry in fs::read_dir(&directory).map_err(|error| io_error(&directory, error))? {
            let entry = entry.map_err(|error| io_error(&directory, error))?;
            entries.push(entry);
        }
        entries.sort_by_key(|entry| entry.file_name());
        let mut directories = Vec::new();
        for entry in entries {
            let path = entry.path();
            let metadata =
                fs::symlink_metadata(&path).map_err(|error| io_error(&path, error))?;
            let file_name = entry.file_name();
            let name = file_name.to_string_lossy();
            if name.starts_with('.') {
                continue;
            }
            if metadata.file_type().is_symlink() {
                continue;
            }
            if metadata.is_dir() {
                if depth < MAX_WALK_DEPTH && !SKIPPED_DIRECTORIES.contains(&name.as_ref()) {
                    directories.push((path, depth + 1));
                }
                continue;
            }
            if !metadata.is_file() || !allowed_extension(&path, &extensions) {
                continue;
            }
            if documents.len() >= options.max_files {
                return Err(AiError::new(
                    AI_E_INVALID_REQUEST,
                    format!("document count exceeds the limit of {}", options.max_files),
                ));
            }
            let relative = path
                .strip_prefix(&canonical)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            if let Some(document) =
                read_document(&path, &relative, options, &mut total_bytes)?
            {
                documents.push(document);
            }
        }
        // Push in reverse so LIFO traversal visits sibling directories in
        // alphabetical order after the current directory's files.
        for directory in directories.into_iter().rev() {
            stack.push(directory);
        }
    }
    Ok(documents)
}

fn allowed_extension(path: &Path, extensions: &HashSet<String>) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|value| extensions.contains(&value.to_ascii_lowercase()))
        .unwrap_or(false)
}

fn read_document(
    path: &Path,
    source: &str,
    options: &IngestOptions,
    total_bytes: &mut usize,
) -> Result<Option<Document>, AiError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| io_error(path, error))?;
    if metadata.file_type().is_symlink() {
        return Ok(None);
    }
    if metadata.len() > options.max_file_bytes as u64 {
        return Err(AiError::new(
            AI_E_INVALID_REQUEST,
            format!(
                "document `{}` is {} bytes; the per-file limit is {}",
                path.display(),
                metadata.len(),
                options.max_file_bytes
            ),
        ));
    }
    let next_total = total_bytes.saturating_add(metadata.len() as usize);
    if next_total > options.max_total_bytes {
        return Err(AiError::new(
            AI_E_INVALID_REQUEST,
            format!(
                "document corpus exceeds the {} byte source limit",
                options.max_total_bytes
            ),
        ));
    }
    let bytes = fs::read(path).map_err(|error| io_error(path, error))?;
    if bytes.contains(&0) {
        return Err(AiError::new(
            AI_E_INVALID_REQUEST,
            format!("document `{}` contains NUL bytes", path.display()),
        ));
    }
    let text = String::from_utf8(bytes).map_err(|_| {
        AiError::new(
            AI_E_INVALID_REQUEST,
            format!("document `{}` is not valid UTF-8", path.display()),
        )
    })?;
    *total_bytes = next_total;
    if text.trim().is_empty() {
        return Ok(None);
    }
    Ok(Some(Document {
        source: source.to_string(),
        text,
    }))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchMode {
    Lexical,
    Semantic,
    Hybrid,
}

impl SearchMode {
    pub fn parse(value: &str) -> Result<Self, AiError> {
        match value {
            "lexical" => Ok(Self::Lexical),
            "semantic" => Ok(Self::Semantic),
            "hybrid" => Ok(Self::Hybrid),
            other => Err(AiError::new(
                AI_E_INVALID_REQUEST,
                format!("unknown search mode `{other}`; expected lexical, semantic, or hybrid"),
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Lexical => "lexical",
            Self::Semantic => "semantic",
            Self::Hybrid => "hybrid",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RetrievalHit {
    pub id: String,
    pub source: String,
    pub start_line: usize,
    pub end_line: usize,
    pub text: String,
    /// Mode-specific score. Hybrid uses reciprocal-rank fusion.
    pub score: f32,
    pub lexical_score: Option<f32>,
    pub semantic_score: Option<f32>,
    pub lexical_rank: Option<usize>,
    pub semantic_rank: Option<usize>,
}

#[derive(Clone, Debug)]
struct IndexedChunk {
    chunk: Chunk,
    vector: Option<Vector>,
}

/// A persisted retrieval index. Search is linear in the number of chunks and is
/// honest about that cost; it is not a production vector database.
#[derive(Clone, Debug)]
pub struct RagIndex {
    provider: String,
    model: String,
    dimensions: usize,
    chunks: Vec<IndexedChunk>,
    bm25: Bm25Index,
}

impl RagIndex {
    pub fn new(
        provider: impl Into<String>,
        model: impl Into<String>,
        dimensions: usize,
        chunks: Vec<Chunk>,
        vectors: Vec<Vector>,
    ) -> Result<Self, AiError> {
        let provider = provider.into();
        let model = model.into();
        validate_names(&provider, &model)?;
        if !(1..=MAX_EMBEDDING_DIMENSIONS).contains(&dimensions) {
            return Err(AiError::new(
                AI_E_INVALID_REQUEST,
                format!(
                    "index dimensions must be between 1 and {MAX_EMBEDDING_DIMENSIONS}"
                ),
            ));
        }
        if chunks.len() != vectors.len() {
            return Err(AiError::new(
                AI_E_INVALID_REQUEST,
                format!(
                    "index has {} chunks but {} vectors",
                    chunks.len(),
                    vectors.len()
                ),
            ));
        }
        if chunks.len() > MAX_CHUNKS {
            return Err(AiError::new(
                AI_E_INVALID_REQUEST,
                format!("index has {} chunks; the limit is {MAX_CHUNKS}", chunks.len()),
            ));
        }
        let mut ids = HashSet::new();
        let mut bm25 = Bm25Index::new();
        let mut total_bytes = 0usize;
        let mut indexed = Vec::with_capacity(chunks.len());
        for (chunk, vector) in chunks.into_iter().zip(vectors) {
            validate_chunk(&chunk)?;
            if vector.len() != dimensions {
                return Err(AiError::new(
                    AI_E_INVALID_REQUEST,
                    format!(
                        "chunk `{}` vector has {} dimensions; the index declares {dimensions}",
                        chunk.id,
                        vector.len()
                    ),
                ));
            }
            if !ids.insert(chunk.id.clone()) {
                return Err(AiError::new(
                    AI_E_INVALID_REQUEST,
                    format!("duplicate chunk id `{}`", chunk.id),
                ));
            }
            total_bytes = total_bytes.saturating_add(chunk.text.len());
            if total_bytes > MAX_TOTAL_CHUNK_BYTES {
                return Err(AiError::new(
                    AI_E_INVALID_REQUEST,
                    format!(
                        "index text exceeds the {} byte limit",
                        MAX_TOTAL_CHUNK_BYTES
                    ),
                ));
            }
            bm25.insert(chunk.id.clone(), chunk.text.clone());
            indexed.push(IndexedChunk {
                chunk,
                vector: Some(vector),
            });
        }
        Ok(Self {
            provider,
            model,
            dimensions,
            chunks: indexed,
            bm25,
        })
    }

    /// Builds a lexical-only index with no stored vectors.
    pub fn lexical(
        provider: impl Into<String>,
        model: impl Into<String>,
        chunks: Vec<Chunk>,
    ) -> Result<Self, AiError> {
        let provider = provider.into();
        let model = model.into();
        validate_names(&provider, &model)?;
        if chunks.len() > MAX_CHUNKS {
            return Err(AiError::new(
                AI_E_INVALID_REQUEST,
                format!("index has {} chunks; the limit is {MAX_CHUNKS}", chunks.len()),
            ));
        }
        let mut ids = HashSet::new();
        let mut bm25 = Bm25Index::new();
        let mut total_bytes = 0usize;
        let mut indexed = Vec::with_capacity(chunks.len());
        for chunk in chunks {
            validate_chunk(&chunk)?;
            if !ids.insert(chunk.id.clone()) {
                return Err(AiError::new(
                    AI_E_INVALID_REQUEST,
                    format!("duplicate chunk id `{}`", chunk.id),
                ));
            }
            total_bytes = total_bytes.saturating_add(chunk.text.len());
            if total_bytes > MAX_TOTAL_CHUNK_BYTES {
                return Err(AiError::new(
                    AI_E_INVALID_REQUEST,
                    format!(
                        "index text exceeds the {} byte limit",
                        MAX_TOTAL_CHUNK_BYTES
                    ),
                ));
            }
            bm25.insert(chunk.id.clone(), chunk.text.clone());
            indexed.push(IndexedChunk {
                chunk,
                vector: None,
            });
        }
        Ok(Self {
            provider,
            model,
            dimensions: 0,
            chunks: indexed,
            bm25,
        })
    }

    pub fn provider(&self) -> &str {
        &self.provider
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    pub fn dimensions(&self) -> usize {
        self.dimensions
    }

    pub fn len(&self) -> usize {
        self.chunks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }

    pub fn has_vectors(&self) -> bool {
        self.dimensions > 0
            && !self.chunks.is_empty()
            && self.chunks.iter().all(|chunk| chunk.vector.is_some())
    }

    pub fn chunks(&self) -> impl Iterator<Item = &Chunk> {
        self.chunks.iter().map(|indexed| &indexed.chunk)
    }

    pub fn chunk(&self, id: &str) -> Option<&Chunk> {
        self.chunks
            .iter()
            .find(|indexed| indexed.chunk.id == id)
            .map(|indexed| &indexed.chunk)
    }

    pub fn search(
        &self,
        query: &str,
        query_vector: Option<&Vector>,
        mode: SearchMode,
        limit: usize,
    ) -> Result<Vec<RetrievalHit>, AiError> {
        self.search_with_source(query, query_vector, mode, limit, None)
    }

    /// Search only chunks whose relative source path starts with `source_prefix`.
    /// The prefix is metadata, not a security boundary; ingestion confinement is.
    pub fn search_with_source(
        &self,
        query: &str,
        query_vector: Option<&Vector>,
        mode: SearchMode,
        limit: usize,
        source_prefix: Option<&str>,
    ) -> Result<Vec<RetrievalHit>, AiError> {
        if query.trim().is_empty() {
            return Err(AiError::new(
                AI_E_INVALID_REQUEST,
                "search query must not be empty",
            ));
        }
        if query.len() > MAX_QUERY_BYTES {
            return Err(AiError::new(
                AI_E_INVALID_REQUEST,
                format!("search query exceeds the {MAX_QUERY_BYTES} byte limit"),
            ));
        }
        if !(1..=MAX_TOP_K).contains(&limit) {
            return Err(AiError::new(
                AI_E_INVALID_REQUEST,
                format!("top-k must be between 1 and {MAX_TOP_K}"),
            ));
        }
        if let Some(prefix) = source_prefix {
            if prefix.is_empty() {
                return Err(AiError::new(
                    AI_E_INVALID_REQUEST,
                    "source prefix must not be empty",
                ));
            }
            if prefix.len() > MAX_SOURCE_BYTES {
                return Err(AiError::new(
                    AI_E_INVALID_REQUEST,
                    format!("source prefix exceeds the {MAX_SOURCE_BYTES} byte limit"),
                ));
            }
        }
        if mode != SearchMode::Lexical && !self.has_vectors() {
            return Err(AiError::new(
                AI_E_CAPABILITY,
                "this index has no vectors; rebuild it with an embedding provider or use --mode lexical",
            ));
        }
        if let Some(vector) = query_vector
            && self.has_vectors()
            && vector.len() != self.dimensions
        {
            return Err(AiError::new(
                AI_E_INVALID_REQUEST,
                format!(
                    "query vector has {} dimensions; the index has {}",
                    vector.len(),
                    self.dimensions
                ),
            ));
        }

        // A filter may discard most candidates, so rank the whole index before
        // truncating. Unfiltered searches keep the cheaper bounded candidate set.
        let candidate_limit = if source_prefix.is_some() {
            self.chunks.len()
        } else {
            match mode {
                SearchMode::Lexical | SearchMode::Semantic => limit,
                SearchMode::Hybrid => limit.saturating_mul(4).clamp(20, MAX_TOP_K),
            }
        };
        let lexical = if mode != SearchMode::Semantic {
            self.bm25.search(query, candidate_limit)
        } else {
            Vec::new()
        };
        let semantic = if mode != SearchMode::Lexical {
            let vector = query_vector.ok_or_else(|| {
                AiError::new(
                    AI_E_INVALID_REQUEST,
                    format!(
                        "{0} search requires a query vector; pass a provider with embeddings",
                        mode.as_str()
                    ),
                )
            })?;
            self.semantic_search(vector, candidate_limit)?
        } else {
            Vec::new()
        };

        match mode {
            SearchMode::Lexical => {
                let mut hits: Vec<RetrievalHit> = lexical
                    .into_iter()
                    .filter(|hit| self.source_matches(&hit.id, source_prefix))
                    .filter_map(|hit| {
                        let chunk = self.chunk(&hit.id)?;
                        Some(hit_from_chunk(
                            chunk,
                            hit.score,
                            Some(hit.score),
                            None,
                            None,
                            None,
                        ))
                    })
                    .collect();
                hits.truncate(limit);
                Ok(hits)
            }
            SearchMode::Semantic => {
                let mut hits: Vec<RetrievalHit> = semantic
                    .into_iter()
                    .filter(|hit| self.source_matches(&hit.id, source_prefix))
                    .filter_map(|hit| {
                        let chunk = self.chunk(&hit.id)?;
                        Some(hit_from_chunk(
                            chunk,
                            hit.score,
                            None,
                            Some(hit.score),
                            None,
                            None,
                        ))
                    })
                    .collect();
                hits.truncate(limit);
                Ok(hits)
            }
            SearchMode::Hybrid => {
                let mut fused: Vec<RetrievalHit> = Vec::new();
                for (rank, hit) in lexical
                    .iter()
                    .filter(|hit| self.source_matches(&hit.id, source_prefix))
                    .enumerate()
                {
                    if let Some(chunk) = self.chunk(&hit.id) {
                        let score = reciprocal_rank(rank);
                        fused.push(hit_from_chunk(
                            chunk,
                            score,
                            Some(hit.score),
                            None,
                            Some(rank + 1),
                            None,
                        ));
                    }
                }
                for (rank, hit) in semantic
                    .iter()
                    .filter(|hit| self.source_matches(&hit.id, source_prefix))
                    .enumerate()
                {
                    let score = reciprocal_rank(rank);
                    if let Some(existing) = fused.iter_mut().find(|item| item.id == hit.id) {
                        existing.score += score;
                        existing.semantic_score = Some(hit.score);
                        existing.semantic_rank = Some(rank + 1);
                    } else if let Some(chunk) = self.chunk(&hit.id) {
                        fused.push(hit_from_chunk(
                            chunk,
                            score,
                            None,
                            Some(hit.score),
                            None,
                            Some(rank + 1),
                        ));
                    }
                }
                fused.sort_by(|left, right| {
                    right
                        .score
                        .partial_cmp(&left.score)
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then_with(|| left.id.cmp(&right.id))
                });
                fused.truncate(limit);
                Ok(fused)
            }
        }
    }

    fn source_matches(&self, id: &str, source_prefix: Option<&str>) -> bool {
        source_prefix.is_none_or(|prefix| {
            self.chunk(id)
                .is_some_and(|chunk| chunk.source.starts_with(prefix))
        })
    }

    fn semantic_search(&self, query: &Vector, limit: usize) -> Result<Vec<ScoredId>, AiError> {
        let mut hits = Vec::with_capacity(self.chunks.len());
        for indexed in &self.chunks {
            let vector = indexed.vector.as_ref().ok_or_else(|| {
                AiError::new(
                    AI_E_CAPABILITY,
                    "index contains a chunk without a vector",
                )
            })?;
            let score = query.cosine_similarity(vector)?;
            if score.is_finite() {
                hits.push(ScoredId {
                    id: indexed.chunk.id.clone(),
                    score,
                });
            }
        }
        hits.sort_by(|left, right| {
            right
                .score
                .partial_cmp(&left.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| left.id.cmp(&right.id))
        });
        hits.truncate(limit);
        Ok(hits)
    }

    pub fn to_json(&self) -> Json {
        let chunks = self
            .chunks
            .iter()
            .map(|indexed| {
                let vector = indexed
                    .vector
                    .as_ref()
                    .map(|vector| {
                        Json::Array(
                            vector
                                .values()
                                .iter()
                                .map(|value| Json::from(f64::from(*value)))
                                .collect(),
                        )
                    })
                    .unwrap_or(Json::Null);
                Json::object([
                    ("id", Json::from(indexed.chunk.id.clone())),
                    ("source", Json::from(indexed.chunk.source.clone())),
                    (
                        "start_line",
                        Json::from(indexed.chunk.start_line as u64),
                    ),
                    ("end_line", Json::from(indexed.chunk.end_line as u64)),
                    ("text", Json::from(indexed.chunk.text.clone())),
                    ("vector", vector),
                ])
            })
            .collect();
        Json::object([
            ("version", Json::from(INDEX_FORMAT_VERSION)),
            ("provider", Json::from(self.provider.clone())),
            ("model", Json::from(self.model.clone())),
            ("dimensions", Json::from(self.dimensions as u64)),
            ("chunks", Json::Array(chunks)),
        ])
    }

    pub fn from_json(value: &Json) -> Result<Self, AiError> {
        if value.get("version").and_then(Json::as_u64) != Some(INDEX_FORMAT_VERSION) {
            return Err(AiError::new(
                AI_E_INVALID_REQUEST,
                format!("unsupported RAG index version; expected {INDEX_FORMAT_VERSION}"),
            ));
        }
        let provider = required_string(value, "provider", MAX_MODEL_BYTES)?;
        let model = required_string(value, "model", MAX_MODEL_BYTES)?;
        let dimensions = required_usize(value, "dimensions")?;
        if dimensions > MAX_EMBEDDING_DIMENSIONS {
            return Err(AiError::new(
                AI_E_INVALID_REQUEST,
                format!("index dimensions exceed the {MAX_EMBEDDING_DIMENSIONS} limit"),
            ));
        }
        let entries = value
            .get("chunks")
            .and_then(Json::as_array)
            .ok_or_else(|| invalid("index is missing array `chunks`"))?;
        if entries.len() > MAX_CHUNKS {
            return Err(AiError::new(
                AI_E_INVALID_REQUEST,
                format!("index has {} chunks; the limit is {MAX_CHUNKS}", entries.len()),
            ));
        }
        let mut chunks = Vec::with_capacity(entries.len());
        let mut vector_count = 0usize;
        let mut total_text = 0usize;
        for entry in entries {
            if entry.as_object().is_none() {
                return Err(invalid("every index chunk must be an object"));
            }
            let id = required_string(entry, "id", MAX_SOURCE_BYTES + 128)?;
            let source = required_string(entry, "source", MAX_SOURCE_BYTES)?;
            let text = required_text(entry, "text", MAX_EMBEDDING_ITEM_BYTES)?;
            let start_line = required_usize(entry, "start_line")?;
            let end_line = required_usize(entry, "end_line")?;
            if start_line == 0 || end_line < start_line {
                return Err(invalid(format!(
                    "chunk `{id}` has an invalid line range {start_line}-{end_line}"
                )));
            }
            total_text = total_text.saturating_add(text.len());
            if total_text > MAX_TOTAL_CHUNK_BYTES {
                return Err(invalid(format!(
                    "index text exceeds the {MAX_TOTAL_CHUNK_BYTES} byte limit"
                )));
            }
            let vector = match entry.get("vector") {
                None | Some(Json::Null) => None,
                Some(Json::Array(values)) => {
                    if dimensions == 0 || values.len() != dimensions {
                        return Err(invalid(format!(
                            "chunk `{id}` has {} vector values; expected {dimensions}",
                            values.len()
                        )));
                    }
                    let values = values
                        .iter()
                        .map(|value| {
                            value
                                .as_f64()
                                .filter(|number| number.is_finite())
                                .ok_or_else(|| {
                                    invalid(format!("chunk `{id}` has a non-finite vector value"))
                                })
                        })
                        .collect::<Result<Vec<f64>, AiError>>()?;
                    vector_count += 1;
                    Some(Vector::new(
                        values.into_iter().map(|value| value as f32).collect(),
                    )?)
                }
                Some(_) => {
                    return Err(invalid(format!(
                        "chunk `{id}` vector must be an array or null"
                    )));
                }
            };
            chunks.push((Chunk {
                id,
                source,
                start_line,
                end_line,
                text,
            }, vector));
        }

        if vector_count == 0 {
            if dimensions != 0 {
                return Err(invalid(
                    "a lexical-only index must declare `dimensions` as 0",
                ));
            }
            return Self::lexical(
                provider,
                model,
                chunks.into_iter().map(|(chunk, _)| chunk).collect(),
            );
        }
        if vector_count != chunks.len() {
            return Err(invalid(
                "index mixes chunks with and without vectors; rebuild it",
            ));
        }
        if dimensions == 0 {
            return Err(invalid("vector indexes must declare non-zero dimensions"));
        }
        let (chunks, vectors): (Vec<Chunk>, Vec<Vector>) = chunks
            .into_iter()
            .map(|(chunk, vector)| {
                let vector = vector.expect("vector count was validated");
                (chunk, vector)
            })
            .unzip();
        Self::new(provider, model, dimensions, chunks, vectors)
    }

    /// Writes the index atomically with owner-only permissions on Unix.
    pub fn save(&self, path: &Path) -> Result<(), AiError> {
        let text = self.to_json().to_string();
        if text.len() > MAX_INDEX_BYTES {
            return Err(AiError::new(
                AI_E_INVALID_REQUEST,
                format!(
                    "serialized index is {} bytes; the limit is {MAX_INDEX_BYTES}",
                    text.len()
                ),
            ));
        }
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        let file_name = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "rag-index.json".to_string());
        let temporary = parent.join(format!(".{file_name}.{}.tmp", std::process::id()));
        let result = (|| {
            let mut options = fs::OpenOptions::new();
            options.write(true).create(true).truncate(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options
                .open(&temporary)
                .map_err(|error| io_error(&temporary, error))?;
            file.write_all(text.as_bytes())
                .map_err(|error| io_error(&temporary, error))?;
            file.sync_all().map_err(|error| io_error(&temporary, error))?;
            drop(file);
            fs::rename(&temporary, path).map_err(|error| io_error(path, error))?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }

    pub fn load(path: &Path) -> Result<Self, AiError> {
        let metadata = fs::metadata(path).map_err(|error| io_error(path, error))?;
        if metadata.len() > MAX_INDEX_BYTES as u64 {
            return Err(AiError::new(
                AI_E_INVALID_REQUEST,
                format!(
                    "index `{}` is {} bytes; the limit is {MAX_INDEX_BYTES}",
                    path.display(),
                    metadata.len()
                ),
            ));
        }
        let text = fs::read_to_string(path).map_err(|error| io_error(path, error))?;
        let value = super::json::parse(&text).map_err(|error| {
            AiError::new(
                AI_E_JSON,
                format!("invalid RAG index `{}`: {error}", path.display()),
            )
        })?;
        Self::from_json(&value)
    }
}

#[derive(Clone, Debug)]
struct ScoredId {
    id: String,
    score: f32,
}

fn reciprocal_rank(zero_based_rank: usize) -> f32 {
    1.0 / (RRF_K + (zero_based_rank as f32) + 1.0)
}

fn hit_from_chunk(
    chunk: &Chunk,
    score: f32,
    lexical_score: Option<f32>,
    semantic_score: Option<f32>,
    lexical_rank: Option<usize>,
    semantic_rank: Option<usize>,
) -> RetrievalHit {
    RetrievalHit {
        id: chunk.id.clone(),
        source: chunk.source.clone(),
        start_line: chunk.start_line,
        end_line: chunk.end_line,
        text: chunk.text.clone(),
        score,
        lexical_score,
        semantic_score,
        lexical_rank,
        semantic_rank,
    }
}

fn validate_chunk(chunk: &Chunk) -> Result<(), AiError> {
    if chunk.id.trim().is_empty() || chunk.id.len() > MAX_SOURCE_BYTES + 128 {
        return Err(AiError::new(
            AI_E_INVALID_REQUEST,
            "chunk id must be non-empty and bounded",
        ));
    }
    if chunk.source.trim().is_empty() || chunk.source.len() > MAX_SOURCE_BYTES {
        return Err(AiError::new(
            AI_E_INVALID_REQUEST,
            format!("chunk source must be at most {MAX_SOURCE_BYTES} bytes"),
        ));
    }
    if chunk.source.chars().any(char::is_control)
        || chunk.id.chars().any(char::is_control)
    {
        return Err(AiError::new(
            AI_E_INVALID_REQUEST,
            "chunk id and source must not contain control characters",
        ));
    }
    if chunk.start_line == 0 || chunk.end_line < chunk.start_line {
        return Err(AiError::new(
            AI_E_INVALID_REQUEST,
            format!(
                "chunk `{}` has invalid line range {}-{}",
                chunk.id, chunk.start_line, chunk.end_line
            ),
        ));
    }
    if chunk.text.trim().is_empty() {
        return Err(AiError::new(
            AI_E_INVALID_REQUEST,
            format!("chunk `{}` is empty", chunk.id),
        ));
    }
    if chunk.text.len() > MAX_EMBEDDING_ITEM_BYTES {
        return Err(AiError::new(
            AI_E_INVALID_REQUEST,
            format!(
                "chunk `{}` is {} bytes; the embedding limit is {MAX_EMBEDDING_ITEM_BYTES}",
                chunk.id,
                chunk.text.len()
            ),
        ));
    }
    Ok(())
}

fn required_string(value: &Json, key: &str, limit: usize) -> Result<String, AiError> {
    let text = value
        .get(key)
        .and_then(Json::as_str)
        .ok_or_else(|| invalid(format!("index is missing string `{key}`")))?;
    if text.is_empty() || text.len() > limit || text.chars().any(char::is_control) {
        return Err(invalid(format!(
            "index field `{key}` must be non-empty, at most {limit} bytes, and free of control characters"
        )));
    }
    Ok(text.to_string())
}

fn required_text(value: &Json, key: &str, limit: usize) -> Result<String, AiError> {
    let text = value
        .get(key)
        .and_then(Json::as_str)
        .ok_or_else(|| invalid(format!("index is missing string `{key}`")))?;
    if text.trim().is_empty() || text.len() > limit || text.contains('\0') {
        return Err(invalid(format!(
            "index field `{key}` must be non-empty, at most {limit} bytes, and free of NUL bytes"
        )));
    }
    Ok(text.to_string())
}

fn validate_names(provider: &str, model: &str) -> Result<(), AiError> {
    for (label, value) in [("provider", provider), ("model", model)] {
        if value.trim().is_empty()
            || value.len() > MAX_MODEL_BYTES
            || value.chars().any(char::is_control)
        {
            return Err(AiError::new(
                AI_E_INVALID_REQUEST,
                format!(
                    "index {label} name must be non-empty, at most {MAX_MODEL_BYTES} bytes, and free of control characters"
                ),
            ));
        }
    }
    Ok(())
}

fn required_usize(value: &Json, key: &str) -> Result<usize, AiError> {
    let number = value
        .get(key)
        .and_then(Json::as_u64)
        .ok_or_else(|| invalid(format!("index is missing integer `{key}`")))?;
    usize::try_from(number).map_err(|_| invalid(format!("index field `{key}` is too large")))
}

fn invalid(message: impl Into<String>) -> AiError {
    AiError::new(AI_E_INVALID_REQUEST, message)
}

fn io_error(path: &Path, error: std::io::Error) -> AiError {
    AiError::new(
        AI_E_IO,
        format!("cannot access `{}`: {error}", path.display()),
    )
}

/// Embeds chunks in bounded batches through any provider.
pub fn embed_chunks(
    provider: &dyn Provider,
    model: &str,
    chunks: &[Chunk],
    dimensions: Option<usize>,
    budget: &mut Budget,
) -> Result<(Vec<Vector>, EmbeddingUsage), AiError> {
    if !provider.capabilities().embeddings {
        return Err(AiError::new(
            AI_E_CAPABILITY,
            format!("provider `{}` does not support embeddings", provider.id()),
        ));
    }
    if chunks.is_empty() {
        return Err(AiError::new(
            AI_E_INVALID_REQUEST,
            "at least one chunk is required",
        ));
    }
    let mut vectors = Vec::with_capacity(chunks.len());
    let mut usage = EmbeddingUsage::default();
    let mut batch: Vec<&Chunk> = Vec::new();
    let mut batch_bytes = 0usize;

    let mut flush = |batch: &mut Vec<&Chunk>,
                 batch_bytes: &mut usize,
                 vectors: &mut Vec<Vector>,
                 usage: &mut EmbeddingUsage|
     -> Result<(), AiError> {
        if batch.is_empty() {
            return Ok(());
        }
        let request = EmbeddingRequest {
            model: model.to_string(),
            input: batch.iter().map(|chunk| chunk.text.clone()).collect(),
            dimensions,
            timeout_ms: None,
        };
        let response = provider.embed(&request, budget)?;
        if response.vectors.len() != batch.len() {
            return Err(AiError::new(
                AI_E_INVALID_REQUEST,
                format!(
                    "provider `{}` returned {} vectors for {} inputs",
                    provider.id(),
                    response.vectors.len(),
                    batch.len()
                ),
            ));
        }
        vectors.extend(response.vectors);
        usage.prompt_tokens = usage
            .prompt_tokens
            .saturating_add(response.usage.prompt_tokens);
        usage.total_tokens = usage
            .total_tokens
            .saturating_add(response.usage.total_tokens);
        batch.clear();
        *batch_bytes = 0;
        Ok(())
    };

    for chunk in chunks {
        let bytes = chunk.text.len();
        if !batch.is_empty()
            && (batch.len() >= MAX_EMBED_BATCH_INPUTS
                || batch_bytes.saturating_add(bytes) > MAX_EMBED_BATCH_BYTES)
        {
            flush(&mut batch, &mut batch_bytes, &mut vectors, &mut usage)?;
        }
        if bytes > MAX_EMBEDDING_ITEM_BYTES {
            return Err(AiError::new(
                AI_E_INVALID_REQUEST,
                format!(
                    "chunk `{}` is {bytes} bytes; the per-input limit is {MAX_EMBEDDING_ITEM_BYTES}",
                    chunk.id
                ),
            ));
        }
        batch_bytes = batch_bytes.saturating_add(bytes);
        batch.push(chunk);
    }
    flush(&mut batch, &mut batch_bytes, &mut vectors, &mut usage)?;
    if vectors.len() != chunks.len() {
        return Err(AiError::new(
            AI_E_INVALID_REQUEST,
            "embedding provider returned an unexpected vector count",
        ));
    }
    if let Some(dimensions) = dimensions {
        for vector in &vectors {
            if vector.len() != dimensions {
                return Err(AiError::new(
                    AI_E_INVALID_REQUEST,
                    format!(
                        "embedding provider returned {} dimensions; expected {dimensions}",
                        vector.len()
                    ),
                ));
            }
        }
    }
    Ok((vectors, usage))
}

/// Embeds one query string.
pub fn embed_query(
    provider: &dyn Provider,
    model: &str,
    query: &str,
    dimensions: Option<usize>,
    budget: &mut Budget,
) -> Result<Vector, AiError> {
    let request = EmbeddingRequest {
        model: model.to_string(),
        input: vec![query.to_string()],
        dimensions,
        timeout_ms: None,
    };
    let response = provider.embed(&request, budget)?;
    let vector = response.vectors.into_iter().next().ok_or_else(|| {
        AiError::new(
            AI_E_INVALID_REQUEST,
            "embedding provider returned no query vector",
        )
    })?;
    if let Some(dimensions) = dimensions
        && vector.len() != dimensions
    {
        return Err(AiError::new(
            AI_E_INVALID_REQUEST,
            format!(
                "query vector has {} dimensions; expected {dimensions}",
                vector.len()
            ),
        ));
    }
    Ok(vector)
}

#[derive(Clone, Debug, PartialEq)]
pub struct EvalQuery {
    pub query: String,
    /// Exact chunk ids or source paths. A hit matches when either is equal.
    pub relevant: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EvalQueryResult {
    pub query: String,
    pub recall_at_k: f32,
    pub precision_at_k: f32,
    pub reciprocal_rank: f32,
    pub retrieved: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EvalReport {
    pub mode: SearchMode,
    pub k: usize,
    pub queries: usize,
    pub recall_at_k: f32,
    pub precision_at_k: f32,
    pub mrr: f32,
    pub per_query: Vec<EvalQueryResult>,
}

impl EvalReport {
    pub fn to_json(&self) -> Json {
        Json::object([
            ("mode", Json::from(self.mode.as_str())),
            ("k", Json::from(self.k as u64)),
            ("queries", Json::from(self.queries as u64)),
            ("recall_at_k", Json::from(f64::from(self.recall_at_k))),
            (
                "precision_at_k",
                Json::from(f64::from(self.precision_at_k)),
            ),
            ("mrr", Json::from(f64::from(self.mrr))),
            (
                "per_query",
                Json::Array(
                    self.per_query
                        .iter()
                        .map(|result| {
                            Json::object([
                                ("query", Json::from(result.query.clone())),
                                (
                                    "recall_at_k",
                                    Json::from(f64::from(result.recall_at_k)),
                                ),
                                (
                                    "precision_at_k",
                                    Json::from(f64::from(result.precision_at_k)),
                                ),
                                (
                                    "reciprocal_rank",
                                    Json::from(f64::from(result.reciprocal_rank)),
                                ),
                                (
                                    "retrieved",
                                    Json::Array(
                                        result
                                            .retrieved
                                            .iter()
                                            .cloned()
                                            .map(Json::from)
                                            .collect(),
                                    ),
                                ),
                            ])
                        })
                        .collect(),
                ),
            ),
        ])
    }
}

/// Evaluates retrieval against exact-id or source-path relevance labels.
///
/// `encoder` is only called for semantic or hybrid modes. Lexical evaluation
/// stays fully offline even when an index has vectors.
pub fn evaluate(
    index: &RagIndex,
    queries: &[EvalQuery],
    mode: SearchMode,
    k: usize,
    encoder: &mut dyn FnMut(&str) -> Result<Vector, AiError>,
) -> Result<EvalReport, AiError> {
    if queries.is_empty() || queries.len() > MAX_EVAL_QUERIES {
        return Err(AiError::new(
            AI_E_INVALID_REQUEST,
            format!("evaluation requires between 1 and {MAX_EVAL_QUERIES} queries"),
        ));
    }
    if !(1..=MAX_TOP_K).contains(&k) {
        return Err(AiError::new(
            AI_E_INVALID_REQUEST,
            format!("top-k must be between 1 and {MAX_TOP_K}"),
        ));
    }
    let mut results = Vec::with_capacity(queries.len());
    let mut recall_total = 0.0_f32;
    let mut precision_total = 0.0_f32;
    let mut reciprocal_total = 0.0_f32;
    for query in queries {
        if query.query.trim().is_empty() || query.query.len() > MAX_QUERY_BYTES {
            return Err(AiError::new(
                AI_E_INVALID_REQUEST,
                "evaluation query must be non-empty and bounded",
            ));
        }
        if query.relevant.is_empty() || query.relevant.len() > MAX_RELEVANT_PER_QUERY {
            return Err(AiError::new(
                AI_E_INVALID_REQUEST,
                format!(
                    "each evaluation query needs between 1 and {MAX_RELEVANT_PER_QUERY} relevant ids"
                ),
            ));
        }
        let vector = if mode == SearchMode::Lexical {
            None
        } else {
            Some(encoder(&query.query)?)
        };
        let hits = index.search(&query.query, vector.as_ref(), mode, k)?;
        let retrieved: Vec<String> = hits.iter().map(|hit| hit.id.clone()).collect();
        let mut found = 0usize;
        let mut first_rank = None;
        for (rank, hit) in hits.iter().enumerate() {
            if query
                .relevant
                .iter()
                .any(|relevant| relevant == &hit.id || relevant == &hit.source)
            {
                found += 1;
                if first_rank.is_none() {
                    first_rank = Some(rank + 1);
                }
            }
        }
        let recall = found as f32 / query.relevant.len() as f32;
        let precision = found as f32 / k as f32;
        let reciprocal = first_rank.map_or(0.0, |rank| 1.0 / rank as f32);
        recall_total += recall;
        precision_total += precision;
        reciprocal_total += reciprocal;
        results.push(EvalQueryResult {
            query: query.query.clone(),
            recall_at_k: recall,
            precision_at_k: precision,
            reciprocal_rank: reciprocal,
            retrieved,
        });
    }
    let count = results.len() as f32;
    Ok(EvalReport {
        mode,
        k,
        queries: results.len(),
        recall_at_k: recall_total / count,
        precision_at_k: precision_total / count,
        mrr: reciprocal_total / count,
        per_query: results,
    })
}

/// Parses an evaluation fixture:
///
/// ```json
/// {"queries":[{"query":"...","relevant":["path.md","chunk-id"]}]}
/// ```
pub fn parse_eval_queries(value: &Json) -> Result<Vec<EvalQuery>, AiError> {
    let entries = match value.get("queries").and_then(Json::as_array) {
        Some(entries) => entries,
        None => value
            .as_array()
            .ok_or_else(|| invalid("evaluation fixture must contain `queries` or be an array"))?,
    };
    if entries.is_empty() || entries.len() > MAX_EVAL_QUERIES {
        return Err(invalid(format!(
            "evaluation fixture must contain between 1 and {MAX_EVAL_QUERIES} queries"
        )));
    }
    let mut queries = Vec::with_capacity(entries.len());
    for entry in entries {
        let query = entry
            .get("query")
            .and_then(Json::as_str)
            .ok_or_else(|| invalid("evaluation query is missing string `query`"))?;
        if query.trim().is_empty() || query.len() > MAX_QUERY_BYTES {
            return Err(invalid("evaluation query is empty or too long"));
        }
        let relevant = entry
            .get("relevant")
            .and_then(Json::as_array)
            .ok_or_else(|| invalid("evaluation query is missing array `relevant`"))?;
        if relevant.is_empty() || relevant.len() > MAX_RELEVANT_PER_QUERY {
            return Err(invalid(format!(
                "evaluation `relevant` must contain 1-{MAX_RELEVANT_PER_QUERY} entries"
            )));
        }
        let mut labels = Vec::with_capacity(relevant.len());
        for label in relevant {
            let label = label
                .as_str()
                .ok_or_else(|| invalid("evaluation relevance labels must be strings"))?;
            if label.is_empty() || label.len() > MAX_SOURCE_BYTES + 128 {
                return Err(invalid("evaluation relevance label is empty or too long"));
            }
            labels.push(label.to_string());
        }
        queries.push(EvalQuery {
            query: query.to_string(),
            relevant: labels,
        });
    }
    Ok(queries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::{Budget, HashEmbeddingProvider};

    fn sample_chunks() -> Vec<Chunk> {
        vec![
            Chunk {
                id: "a.md#L1-L2#a".into(),
                source: "a.md".into(),
                start_line: 1,
                end_line: 2,
                text: "Rust enforces memory safety without a garbage collector.".into(),
            },
            Chunk {
                id: "b.md#L1-L2#b".into(),
                source: "b.md".into(),
                start_line: 1,
                end_line: 2,
                text: "Python is popular for data science and automation.".into(),
            },
            Chunk {
                id: "c.md#L1-L2#c".into(),
                source: "c.md".into(),
                start_line: 1,
                end_line: 2,
                text: "WebAssembly runs in browsers and edge runtimes.".into(),
            },
        ]
    }

    fn vector_index() -> RagIndex {
        let provider = HashEmbeddingProvider::new(32).unwrap();
        let mut budget = Budget::default();
        let chunks = sample_chunks();
        let (vectors, _) =
            embed_chunks(&provider, "hash-embedding-32", &chunks, Some(32), &mut budget).unwrap();
        RagIndex::new("hash", "hash-embedding-32", 32, chunks, vectors).unwrap()
    }

    #[test]
    fn chunking_tracks_lines_and_has_stable_ids() {
        let text = "first line of the document\nsecond line of the document\nthird line of the document\nfourth line of the document\n";
        let options = ChunkOptions::new(64, 16).unwrap();
        let chunks = chunk_document("doc.txt", text, &options).unwrap();
        assert!(chunks.len() >= 2);
        assert_eq!(chunks[0].start_line, 1);
        assert!(chunks[0].end_line < 4);
        for chunk in &chunks {
            assert_eq!(
                chunk.id,
                stable_chunk_id(&chunk.source, chunk.start_line, chunk.end_line, &chunk.text)
            );
            assert!(chunk.text.chars().count() <= options.max_chars);
        }
        let again = chunk_document("doc.txt", text, &options).unwrap();
        assert_eq!(chunks, again);
    }

    #[test]
    fn chunking_splits_pathological_lines_without_loss() {
        let text = "x".repeat(300);
        let options = ChunkOptions::new(64, 0).unwrap();
        let chunks = chunk_document("long.txt", &text, &options).unwrap();
        assert!(chunks.len() > 1);
        assert_eq!(chunks.iter().map(|chunk| chunk.text.len()).sum::<usize>(), 300);
        assert!(chunks.iter().all(|chunk| chunk.start_line == 1));
    }

    #[test]
    fn chunking_rejects_invalid_limits() {
        assert!(ChunkOptions::new(10, 1).is_err());
        assert!(ChunkOptions::new(64, 64).is_err());
        assert!(chunk_document("", "text", &ChunkOptions::default()).is_err());
    }

    #[test]
    fn ingestion_skips_hidden_files_and_honors_extensions() {
        let directory = std::env::temp_dir().join(format!(
            "vantyx-rag-ingest-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(directory.join("sub")).unwrap();
        fs::write(directory.join("a.md"), "# A\n").unwrap();
        fs::write(directory.join("sub/b.txt"), "B\n").unwrap();
        fs::write(directory.join(".secret"), "do not index\n").unwrap();
        fs::write(directory.join("image.png"), "not really an image\n").unwrap();
        let documents = collect_documents(&directory, &IngestOptions::default()).unwrap();
        assert_eq!(documents.len(), 2);
        assert_eq!(documents[0].source, "a.md");
        assert_eq!(documents[1].source, "sub/b.txt");
        let _ = fs::remove_dir_all(&directory);
    }

    #[test]
    fn lexical_search_returns_line_citations() {
        let index = RagIndex::lexical("hash", "lexical", sample_chunks()).unwrap();
        let hits = index
            .search("memory safety", None, SearchMode::Lexical, 2)
            .unwrap();
        assert_eq!(hits[0].source, "a.md");
        assert_eq!(hits[0].start_line, 1);
        assert!(hits[0].text.contains("memory safety"));
        assert!(hits[0].lexical_score.is_some());
        assert!(hits[0].semantic_score.is_none());
    }

    #[test]
    fn source_prefix_filters_lexical_and_hybrid_results() {
        let index = vector_index();
        let lexical = index
            .search_with_source("memory safety", None, SearchMode::Lexical, 10, Some("a."))
            .unwrap();
        assert!(!lexical.is_empty());
        assert!(lexical.iter().all(|hit| hit.source.starts_with("a.")));

        let provider = HashEmbeddingProvider::new(32).unwrap();
        let mut budget = Budget::default();
        let query = embed_query(
            &provider,
            "hash-embedding-32",
            "memory safety",
            Some(32),
            &mut budget,
        )
        .unwrap();
        let hybrid = index
            .search_with_source(
                "memory safety",
                Some(&query),
                SearchMode::Hybrid,
                10,
                Some("b.md"),
            )
            .unwrap();
        assert_eq!(hybrid.len(), 1);
        assert_eq!(hybrid[0].source, "b.md");

        let none = index
            .search_with_source("memory safety", None, SearchMode::Lexical, 10, Some("missing/"))
            .unwrap();
        assert!(none.is_empty());
        assert!(
            index
                .search_with_source("x", None, SearchMode::Lexical, 1, Some(""))
                .is_err()
        );
    }

    #[test]
    fn semantic_and_hybrid_search_require_vectors() {
        let lexical = RagIndex::lexical("hash", "lexical", sample_chunks()).unwrap();
        assert!(matches!(
            lexical.search("memory", None, SearchMode::Semantic, 1),
            Err(error) if error.code == AI_E_CAPABILITY
        ));
        let index = vector_index();
        let provider = HashEmbeddingProvider::new(32).unwrap();
        let mut budget = Budget::default();
        let query = embed_query(&provider, "hash-embedding-32", "memory safety", Some(32), &mut budget)
            .unwrap();
        let semantic = index
            .search("memory safety", Some(&query), SearchMode::Semantic, 3)
            .unwrap();
        assert_eq!(semantic.len(), 3);
        assert!(semantic[0].semantic_score.unwrap() >= semantic[1].semantic_score.unwrap());
        let hybrid = index
            .search("memory safety", Some(&query), SearchMode::Hybrid, 3)
            .unwrap();
        assert!(hybrid.iter().all(|hit| hit.score > 0.0));
        assert!(hybrid.iter().any(|hit| hit.lexical_rank.is_some()));
        assert!(hybrid.iter().any(|hit| hit.semantic_rank.is_some()));
    }

    #[test]
    fn persisted_index_round_trips_and_rebuilds_lexical_state() {
        let index = vector_index();
        let directory = std::env::temp_dir().join(format!(
            "vantyx-rag-save-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("index.json");
        index.save(&path).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }
        let loaded = RagIndex::load(&path).unwrap();
        assert_eq!(loaded.len(), index.len());
        assert_eq!(loaded.dimensions(), 32);
        assert_eq!(loaded.provider(), "hash");
        assert!(loaded.has_vectors());
        let hits = loaded
            .search("memory safety", None, SearchMode::Lexical, 2)
            .unwrap();
        assert_eq!(hits[0].source, "a.md");
        let _ = fs::remove_dir_all(&directory);
    }

    #[test]
    fn loader_rejects_malformed_or_mixed_indexes() {
        let mut value = vector_index().to_json();
        if let Json::Object(entries) = &mut value {
            entries.retain(|(key, _)| key != "version");
        }
        assert!(RagIndex::from_json(&value).is_err());

        let mut value = vector_index().to_json();
        if let Json::Object(entries) = &mut value {
            for (key, entry) in entries {
                if key == "chunks"
                    && let Json::Array(chunks) = entry
                    && let Json::Object(chunk) = &mut chunks[0]
                {
                    chunk.retain(|(field, _)| field != "vector");
                }
            }
        }
        assert!(RagIndex::from_json(&value).is_err());
    }

    #[test]
    fn evaluation_reports_recall_precision_and_mrr() {
        let index = RagIndex::lexical("hash", "lexical", sample_chunks()).unwrap();
        let queries = vec![
            EvalQuery {
                query: "memory safety".into(),
                relevant: vec!["a.md".into()],
            },
            EvalQuery {
                query: "webassembly browsers".into(),
                relevant: vec!["c.md".into()],
            },
        ];
        let report = evaluate(
            &index,
            &queries,
            SearchMode::Lexical,
            3,
            &mut |_| unreachable!("lexical evaluation must not embed"),
        )
        .unwrap();
        assert_eq!(report.queries, 2);
        assert_eq!(report.recall_at_k, 1.0);
        assert_eq!(report.mrr, 1.0);
        assert!(report.precision_at_k > 0.0);
        assert_eq!(report.per_query[0].retrieved[0], "a.md#L1-L2#a");
    }

    #[test]
    fn eval_fixture_parser_rejects_empty_queries() {
        let value = crate::ai::json::parse(r#"{"queries":[]}"#).unwrap();
        assert!(parse_eval_queries(&value).is_err());
        let value = crate::ai::json::parse(
            r#"{"queries":[{"query":"x","relevant":["a.md"]}]}"#,
        )
        .unwrap();
        assert_eq!(parse_eval_queries(&value).unwrap().len(), 1);
    }

    #[test]
    fn embedding_helpers_enforce_provider_capability() {
        let provider = crate::ai::EchoProvider::new();
        let mut budget = Budget::default();
        let chunks = sample_chunks();
        assert!(matches!(
            embed_chunks(&provider, "echo", &chunks, None, &mut budget),
            Err(error) if error.code == AI_E_CAPABILITY
        ));
    }

    #[test]
    fn query_limits_are_enforced() {
        let index = vector_index();
        assert!(index.search("", None, SearchMode::Lexical, 1).is_err());
        assert!(index.search("x", None, SearchMode::Lexical, 0).is_err());
        assert!(index.search("x", None, SearchMode::Lexical, MAX_TOP_K + 1).is_err());
        assert!(index.search("x", None, SearchMode::Hybrid, 1).is_err());
    }

    #[test]
    fn search_mode_parsing_is_explicit() {
        assert_eq!(SearchMode::parse("lexical").unwrap(), SearchMode::Lexical);
        assert_eq!(SearchMode::parse("semantic").unwrap(), SearchMode::Semantic);
        assert_eq!(SearchMode::parse("hybrid").unwrap(), SearchMode::Hybrid);
        assert!(SearchMode::parse("fuzzy").is_err());
    }
}
