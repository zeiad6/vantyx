//! Provider-neutral AI request and response types.

use super::budget::Budget;
use super::embedding::Vector;
use super::json::Json;
use std::fmt;

pub const AI_E_CONFIG: &str = "AI_E_CONFIG";
pub const AI_E_INVALID_REQUEST: &str = "AI_E_INVALID_REQUEST";
pub const AI_E_NETWORK_DISABLED: &str = "AI_E_NETWORK_DISABLED";
pub const AI_E_CONNECT: &str = "AI_E_CONNECT";
pub const AI_E_AUTH: &str = "AI_E_AUTH";
pub const AI_E_RATE_LIMIT: &str = "AI_E_RATE_LIMIT";
pub const AI_E_TIMEOUT: &str = "AI_E_TIMEOUT";
pub const AI_E_UNSUPPORTED: &str = "AI_E_UNSUPPORTED";
pub const AI_E_JSON: &str = "AI_E_JSON";
pub const AI_E_SCHEMA: &str = "AI_E_SCHEMA";
pub const AI_E_BUDGET: &str = "AI_E_BUDGET";
pub const AI_E_CAPABILITY: &str = "AI_E_CAPABILITY";
pub const AI_E_STEPS: &str = "AI_E_STEPS";
pub const AI_E_PROVIDER: &str = "AI_E_PROVIDER";
pub const AI_E_RESPONSE_TOO_LARGE: &str = "AI_E_RESPONSE_TOO_LARGE";
pub const AI_E_CANCELLED: &str = "AI_E_CANCELLED";
pub const AI_E_IO: &str = "AI_E_IO";
pub const AI_E_FORMAT: &str = "AI_E_FORMAT";

/// Hard limits for one embedding request. They are conservative request
/// guards, not model-specific limits.
pub const MAX_EMBEDDING_INPUTS: usize = 128;
pub const MAX_EMBEDDING_ITEM_BYTES: usize = 8 * 1024;
pub const MAX_EMBEDDING_REQUEST_BYTES: usize = 32 * 1024;
pub const MAX_EMBEDDING_DIMENSIONS: usize = 4096;

/// A stable, redacted error from the AI layer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AiError {
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

impl AiError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            retryable: false,
        }
    }

    pub fn retryable(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            retryable: true,
        }
    }

    pub fn unsupported(parameter: &str, provider: &str) -> Self {
        Self::new(
            AI_E_UNSUPPORTED,
            format!("provider `{provider}` does not support `{parameter}`"),
        )
    }

    pub fn budget(message: impl Into<String>) -> Self {
        Self::new(AI_E_BUDGET, message)
    }
}

impl fmt::Display for AiError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for AiError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::User => "user",
            Self::Assistant => "assistant",
            Self::Tool => "tool",
        }
    }
}

/// A typed content part. Unsupported modalities fail at the capability check,
/// not by silently degrading to text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ContentPart {
    Text(String),
    ImageUrl { url: String, detail: Option<String> },
    AudioUrl { url: String },
    VideoUrl { url: String },
}

impl ContentPart {
    pub fn text(value: impl Into<String>) -> Self {
        Self::Text(value.into())
    }

    pub fn text_value(&self) -> Option<&str> {
        match self {
            Self::Text(value) => Some(value),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    /// Raw JSON arguments. They are validated before execution.
    pub arguments: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Message {
    pub role: Role,
    pub content: Vec<ContentPart>,
    pub name: Option<String>,
    pub tool_call_id: Option<String>,
    pub tool_calls: Vec<ToolCall>,
}

impl Message {
    pub fn system(text: impl Into<String>) -> Self {
        Self::text(Role::System, text)
    }

    pub fn user(text: impl Into<String>) -> Self {
        Self::text(Role::User, text)
    }

    pub fn assistant(text: impl Into<String>) -> Self {
        Self::text(Role::Assistant, text)
    }

    pub fn tool(call_id: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            role: Role::Tool,
            content: vec![ContentPart::text(text)],
            name: None,
            tool_call_id: Some(call_id.into()),
            tool_calls: Vec::new(),
        }
    }

    pub fn text(role: Role, text: impl Into<String>) -> Self {
        Self {
            role,
            content: vec![ContentPart::text(text)],
            name: None,
            tool_call_id: None,
            tool_calls: Vec::new(),
        }
    }

    pub fn text_content(&self) -> String {
        self.content
            .iter()
            .filter_map(ContentPart::text_value)
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    /// JSON Schema for the tool arguments.
    pub parameters: Json,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ResponseFormat {
    Text,
    JsonObject,
    JsonSchema { name: String, schema: Json },
}

#[derive(Clone, Debug, PartialEq)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub max_tokens: Option<u32>,
    pub stop: Vec<String>,
    pub tools: Vec<ToolSpec>,
    pub response_format: Option<ResponseFormat>,
    pub seed: Option<i64>,
    pub timeout_ms: Option<u64>,
}

impl ChatRequest {
    pub fn new(model: impl Into<String>, messages: Vec<Message>) -> Self {
        Self {
            model: model.into(),
            messages,
            temperature: None,
            top_p: None,
            max_tokens: None,
            stop: Vec::new(),
            tools: Vec::new(),
            response_format: None,
            seed: None,
            timeout_ms: None,
        }
    }

    pub fn validate(&self) -> Result<(), AiError> {
        if self.model.trim().is_empty() {
            return Err(AiError::new(
                AI_E_INVALID_REQUEST,
                "model must not be empty",
            ));
        }
        if self.messages.is_empty() {
            return Err(AiError::new(
                AI_E_INVALID_REQUEST,
                "at least one message is required",
            ));
        }
        if let Some(temperature) = self.temperature
            && (!(0.0..=2.0).contains(&temperature) || !temperature.is_finite())
        {
            return Err(AiError::new(
                AI_E_INVALID_REQUEST,
                "temperature must be finite and between 0 and 2",
            ));
        }
        if let Some(top_p) = self.top_p
            && (!(0.0..=1.0).contains(&top_p) || !top_p.is_finite())
        {
            return Err(AiError::new(
                AI_E_INVALID_REQUEST,
                "top_p must be finite and between 0 and 1",
            ));
        }
        if self.max_tokens == Some(0) {
            return Err(AiError::new(
                AI_E_INVALID_REQUEST,
                "max_tokens must be greater than zero",
            ));
        }
        if self.stop.len() > 8 {
            return Err(AiError::new(
                AI_E_INVALID_REQUEST,
                "at most 8 stop sequences are supported",
            ));
        }
        let mut names = Vec::new();
        for tool in &self.tools {
            if tool.name.trim().is_empty() {
                return Err(AiError::new(
                    AI_E_INVALID_REQUEST,
                    "tool names must not be empty",
                ));
            }
            if names.contains(&tool.name) {
                return Err(AiError::new(
                    AI_E_INVALID_REQUEST,
                    format!("duplicate tool name `{}`", tool.name),
                ));
            }
            names.push(tool.name.clone());
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Usage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
}

impl Usage {
    pub fn new(prompt_tokens: u64, completion_tokens: u64) -> Self {
        Self {
            prompt_tokens,
            completion_tokens,
            total_tokens: prompt_tokens.saturating_add(completion_tokens),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FinishReason {
    Stop,
    Length,
    ToolCalls,
    ContentFilter,
    Other(String),
}

impl FinishReason {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Stop => "stop",
            Self::Length => "length",
            Self::ToolCalls => "tool_calls",
            Self::ContentFilter => "content_filter",
            Self::Other(value) => value,
        }
    }

    pub fn from_wire(value: Option<&str>) -> Self {
        match value {
            Some("stop") | None => Self::Stop,
            Some("length") => Self::Length,
            Some("tool_calls") => Self::ToolCalls,
            Some("content_filter") => Self::ContentFilter,
            Some(other) => Self::Other(other.to_string()),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ChatResponse {
    pub id: Option<String>,
    pub model: String,
    pub message: Message,
    pub finish_reason: FinishReason,
    pub usage: Usage,
}

impl ChatResponse {
    pub fn text(model: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            id: None,
            model: model.into(),
            message: Message::assistant(text),
            finish_reason: FinishReason::Stop,
            usage: Usage::default(),
        }
    }

    /// Serializes the public response fields for CLI and test output.
    pub fn to_json(&self) -> Json {
        let tool_calls = self
            .message
            .tool_calls
            .iter()
            .map(|call| {
                Json::object([
                    ("id", Json::from(call.id.clone())),
                    ("name", Json::from(call.name.clone())),
                    ("arguments", Json::from(call.arguments.clone())),
                ])
            })
            .collect();
        let mut entries = vec![
            ("model", Json::from(self.model.clone())),
            ("text", Json::from(self.message.text_content())),
            ("finish_reason", Json::from(self.finish_reason.as_str())),
            (
                "usage",
                Json::object([
                    ("prompt_tokens", Json::from(self.usage.prompt_tokens)),
                    (
                        "completion_tokens",
                        Json::from(self.usage.completion_tokens),
                    ),
                    ("total_tokens", Json::from(self.usage.total_tokens)),
                ]),
            ),
            ("tool_calls", Json::Array(tool_calls)),
        ];
        if let Some(id) = &self.id {
            entries.push(("id", Json::from(id.clone())));
        }
        Json::object(entries)
    }
}

/// A provider-neutral embedding request.
///
/// `input` is always a batch, even for one string, so the same request type is
/// used for single and batch calls. Limits are enforced by [`Self::validate`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmbeddingRequest {
    pub model: String,
    pub input: Vec<String>,
    /// Requested output dimensions. Providers that cannot honor an explicit
    /// dimension must reject the request rather than ignore it.
    pub dimensions: Option<usize>,
    pub timeout_ms: Option<u64>,
}

impl EmbeddingRequest {
    pub fn new(model: impl Into<String>, input: Vec<String>) -> Self {
        Self {
            model: model.into(),
            input,
            dimensions: None,
            timeout_ms: None,
        }
    }

    pub fn single(model: impl Into<String>, text: impl Into<String>) -> Self {
        Self::new(model, vec![text.into()])
    }

    pub fn validate(&self) -> Result<(), AiError> {
        if self.model.trim().is_empty() {
            return Err(AiError::new(
                AI_E_INVALID_REQUEST,
                "embedding model must not be empty",
            ));
        }
        if self.input.is_empty() {
            return Err(AiError::new(
                AI_E_INVALID_REQUEST,
                "at least one embedding input is required",
            ));
        }
        if self.input.len() > MAX_EMBEDDING_INPUTS {
            return Err(AiError::new(
                AI_E_INVALID_REQUEST,
                format!(
                    "embedding batch has {} inputs; the limit is {MAX_EMBEDDING_INPUTS}",
                    self.input.len()
                ),
            ));
        }
        let mut total = 0usize;
        for (index, text) in self.input.iter().enumerate() {
            if text.trim().is_empty() {
                return Err(AiError::new(
                    AI_E_INVALID_REQUEST,
                    format!("embedding input at index {index} is empty"),
                ));
            }
            if text.len() > MAX_EMBEDDING_ITEM_BYTES {
                return Err(AiError::new(
                    AI_E_INVALID_REQUEST,
                    format!(
                        "embedding input at index {index} is {} bytes; the per-input limit is {MAX_EMBEDDING_ITEM_BYTES}",
                        text.len()
                    ),
                ));
            }
            total = total.saturating_add(text.len());
        }
        if total > MAX_EMBEDDING_REQUEST_BYTES {
            return Err(AiError::new(
                AI_E_INVALID_REQUEST,
                format!(
                    "embedding batch is {total} bytes; the request limit is {MAX_EMBEDDING_REQUEST_BYTES}"
                ),
            ));
        }
        if let Some(dimensions) = self.dimensions
            && !(1..=MAX_EMBEDDING_DIMENSIONS).contains(&dimensions)
        {
            return Err(AiError::new(
                AI_E_INVALID_REQUEST,
                format!("embedding dimensions must be between 1 and {MAX_EMBEDDING_DIMENSIONS}"),
            ));
        }
        if self.timeout_ms == Some(0) {
            return Err(AiError::new(
                AI_E_INVALID_REQUEST,
                "embedding timeout_ms must be greater than zero",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EmbeddingUsage {
    pub prompt_tokens: u64,
    pub total_tokens: u64,
}

impl EmbeddingUsage {
    pub fn new(prompt_tokens: u64) -> Self {
        Self {
            prompt_tokens,
            total_tokens: prompt_tokens,
        }
    }
}

/// A provider-neutral embedding response. Vectors are in the same order as the
/// request inputs; adapters must reorder server responses by index.
#[derive(Clone, Debug, PartialEq)]
pub struct EmbeddingResponse {
    pub model: String,
    pub vectors: Vec<Vector>,
    pub usage: EmbeddingUsage,
}

impl EmbeddingResponse {
    pub fn dimensions(&self) -> Option<usize> {
        self.vectors.first().map(Vector::len)
    }

    /// Serializes the public response fields for CLI and test output.
    pub fn to_json(&self) -> Json {
        Json::object([
            ("model", Json::from(self.model.clone())),
            (
                "dimensions",
                self.dimensions()
                    .map_or(Json::Null, |dimensions| Json::from(dimensions as u64)),
            ),
            (
                "vectors",
                Json::Array(
                    self.vectors
                        .iter()
                        .map(|vector| {
                            Json::Array(
                                vector
                                    .values()
                                    .iter()
                                    .map(|value| Json::from(f64::from(*value)))
                                    .collect(),
                            )
                        })
                        .collect(),
                ),
            ),
            (
                "usage",
                Json::object([
                    ("prompt_tokens", Json::from(self.usage.prompt_tokens)),
                    ("total_tokens", Json::from(self.usage.total_tokens)),
                ]),
            ),
        ])
    }
}

/// Explicit capability declaration. A provider must not claim what it cannot
/// verify, and callers must check before sending parameters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProviderCapabilities {
    pub chat: bool,
    pub streaming: bool,
    pub tools: bool,
    pub json_object: bool,
    pub json_schema: bool,
    pub embeddings: bool,
    pub vision: bool,
    pub audio_input: bool,
    pub audio_output: bool,
    pub seed: bool,
    pub max_context_tokens: Option<usize>,
}

impl Default for ProviderCapabilities {
    fn default() -> Self {
        Self {
            chat: true,
            streaming: false,
            tools: false,
            json_object: false,
            json_schema: false,
            embeddings: false,
            vision: false,
            audio_input: false,
            audio_output: false,
            seed: false,
            max_context_tokens: None,
        }
    }
}

impl ProviderCapabilities {
    pub fn mock() -> Self {
        Self {
            chat: true,
            streaming: false,
            tools: true,
            json_object: true,
            json_schema: true,
            embeddings: false,
            vision: false,
            audio_input: false,
            audio_output: false,
            seed: true,
            max_context_tokens: Some(8_192),
        }
    }

    pub fn openai_compatible() -> Self {
        Self {
            chat: true,
            // Text streaming over SSE; tool-call streaming is not implemented.
            streaming: true,
            tools: true,
            json_object: true,
            // Server-side JSON-schema support varies; enable explicitly.
            json_schema: false,
            // The adapter speaks `/v1/embeddings`; model/server support varies
            // and is reported by the endpoint at request time.
            embeddings: true,
            // Vision is model-dependent; the adapter can express image parts.
            vision: true,
            audio_input: false,
            audio_output: false,
            seed: true,
            max_context_tokens: None,
        }
    }

    /// Capabilities for the deterministic offline feature-hash provider. It is
    /// an embedding-only provider, not a chat provider.
    pub fn hash_embedding() -> Self {
        Self {
            chat: false,
            embeddings: true,
            ..Self::default()
        }
    }
}

pub trait Provider {
    fn id(&self) -> &str;
    fn capabilities(&self) -> ProviderCapabilities;

    /// Sends a chat request. Providers that only support embeddings return a
    /// stable `AI_E_UNSUPPORTED` error by default.
    fn chat(&self, _request: &ChatRequest, _budget: &mut Budget) -> Result<ChatResponse, AiError> {
        Err(AiError::unsupported("chat", self.id()))
    }

    /// Sends a streaming chat request. Text deltas are delivered to `on_delta`
    /// as they arrive; the returned response aggregates the full text and
    /// usage. Providers without streaming support return `AI_E_UNSUPPORTED`.
    fn chat_stream(
        &self,
        _request: &ChatRequest,
        _budget: &mut Budget,
        _on_delta: &mut dyn FnMut(&str),
    ) -> Result<ChatResponse, AiError> {
        Err(AiError::unsupported("streaming", self.id()))
    }

    /// Sends an embedding request. Providers that do not support embeddings
    /// return a stable `AI_E_UNSUPPORTED` error by default.
    fn embed(
        &self,
        _request: &EmbeddingRequest,
        _budget: &mut Budget,
    ) -> Result<EmbeddingResponse, AiError> {
        Err(AiError::unsupported("embeddings", self.id()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_embedding_requests() {
        let valid = EmbeddingRequest::single("model", "hello");
        assert!(valid.validate().is_ok());

        let empty_model = EmbeddingRequest::single(" ", "hello");
        assert_eq!(
            empty_model.validate().unwrap_err().code,
            AI_E_INVALID_REQUEST
        );

        let empty_batch = EmbeddingRequest::new("model", Vec::new());
        assert!(empty_batch.validate().is_err());

        let empty_item = EmbeddingRequest::new("model", vec![" ".into()]);
        assert!(empty_item.validate().is_err());

        let too_many = EmbeddingRequest::new(
            "model",
            (0..MAX_EMBEDDING_INPUTS + 1)
                .map(|index| format!("item {index}"))
                .collect(),
        );
        assert!(too_many.validate().is_err());

        let oversized_item =
            EmbeddingRequest::single("model", "x".repeat(MAX_EMBEDDING_ITEM_BYTES + 1));
        assert!(oversized_item.validate().is_err());

        let oversized_batch = EmbeddingRequest::new(
            "model",
            (0..5)
                .map(|_| "x".repeat(MAX_EMBEDDING_ITEM_BYTES))
                .collect(),
        );
        assert!(oversized_batch.validate().is_err());

        let mut bad_dimensions = EmbeddingRequest::single("model", "hello");
        bad_dimensions.dimensions = Some(0);
        assert!(bad_dimensions.validate().is_err());
        bad_dimensions.dimensions = Some(MAX_EMBEDDING_DIMENSIONS + 1);
        assert!(bad_dimensions.validate().is_err());

        let mut bad_timeout = EmbeddingRequest::single("model", "hello");
        bad_timeout.timeout_ms = Some(0);
        assert!(bad_timeout.validate().is_err());
    }

    #[test]
    fn rejects_invalid_sampling_parameters() {
        let mut request = ChatRequest::new("m", vec![Message::user("hi")]);
        request.temperature = Some(3.0);
        assert_eq!(request.validate().unwrap_err().code, AI_E_INVALID_REQUEST);
        request.temperature = Some(0.7);
        request.top_p = Some(1.5);
        assert_eq!(request.validate().unwrap_err().code, AI_E_INVALID_REQUEST);
    }

    #[test]
    fn rejects_duplicate_tool_names() {
        let mut request = ChatRequest::new("m", vec![Message::user("hi")]);
        let tool = ToolSpec {
            name: "lookup".into(),
            description: "lookup".into(),
            parameters: Json::object([] as [(&str, Json); 0]),
        };
        request.tools = vec![tool.clone(), tool];
        assert!(request.validate().is_err());
    }

    #[test]
    fn maps_finish_reasons() {
        assert_eq!(
            FinishReason::from_wire(Some("length")),
            FinishReason::Length
        );
        assert_eq!(
            FinishReason::from_wire(Some("weird")),
            FinishReason::Other("weird".into())
        );
    }
}
