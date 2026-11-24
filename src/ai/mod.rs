//! Dependency-free AI host foundation.
//!
//! The compiler stays small and predictable. AI providers, retrieval, tools,
//! and agents live behind an explicit host API and HTTP boundary. See
//! `docs/ai/AI_ARCHITECTURE.md` and ADR-0015.
//!
//! Invariants:
//!
//! - No implicit network access or model downloads.
//! - Unsupported parameters are rejected, not silently dropped.
//! - Model output is untrusted input and is validated before use.
//! - Token counts from [`tokens`] are labelled estimates, not model tokenizers.
//! - Tool calls require explicit capabilities and policy approval.
//! - Agent loops always have step, tool, token, and optional wall-clock limits.

pub mod agent;
pub mod bm25;
pub mod budget;
pub mod config;
pub mod embedding;
pub mod embedding_provider;
pub mod extract;
pub mod http;
pub mod json;
pub mod metrics;
pub mod mock;
pub mod openai;
pub mod provider;
pub mod rag;
pub mod redact;
pub mod schema;
pub mod sse;
pub mod tokens;
pub mod tools;

pub use agent::{Agent, AgentConfig, AgentResult};
pub use bm25::{Bm25Hit, Bm25Index};
pub use budget::Budget;
pub use config::AiConfig;
pub use embedding::{SearchHit, Vector, VectorEntry, VectorIndex};
pub use embedding_provider::{
    CachingEmbeddingProvider, DEFAULT_CACHE_BYTES, DEFAULT_CACHE_ENTRIES, DEFAULT_HASH_DIMENSIONS,
    EmbeddingCacheStats, HashEmbeddingProvider,
};
pub use extract::{Extraction, MAX_REPAIR_ATTEMPTS, extract};
pub use http::{
    ChunkCallback, HttpResponse, HttpTransport, NetworkPolicy, StartCallback, StreamHandlers,
    Transport,
};
pub use json::{Json, JsonNumber, parse};
pub use metrics::{MetricEvent, MetricsSink};
pub use mock::{EchoProvider, MockProvider};
pub use openai::OpenAiProvider;
pub use provider::{
    AiError, ChatRequest, ChatResponse, ContentPart, EmbeddingRequest, EmbeddingResponse,
    EmbeddingUsage, FinishReason, MAX_EMBEDDING_DIMENSIONS, MAX_EMBEDDING_INPUTS,
    MAX_EMBEDDING_ITEM_BYTES, MAX_EMBEDDING_REQUEST_BYTES, Message, Provider, ProviderCapabilities,
    ResponseFormat, Role, ToolCall, ToolSpec, Usage,
};
pub use rag::{
    Chunk, ChunkOptions, DEFAULT_MMR_LAMBDA, Document, EvalQuery, EvalReport, IncrementalPlan,
    IngestOptions, RagIndex, RetrievalHit, ReuseStats, SearchMode, chunk_document,
    collect_documents, embed_chunks, embed_query, evaluate, parse_eval_queries,
};
pub use schema::{SchemaError, validate as validate_schema};
pub use sse::{SseEvent, SseParser};
pub use tokens::{
    check_context_limit, estimate_embedding_tokens, estimate_request_tokens, estimate_tokens,
};
pub use tools::{
    Approval, Capability, CapabilitySet, EchoTool, ReadTextFileTool, Tool, ToolContext, ToolError,
    ToolPolicy, ToolRegistry, WordCountTool,
};
