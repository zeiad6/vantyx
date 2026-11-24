//! Provider-neutral AI request and response types.

use super::budget::Budget;
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
            // Streaming is planned for AI-3; no streaming method exists yet.
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
            // The adapter currently performs non-streaming requests only.
            streaming: false,
            tools: true,
            json_object: true,
            // Server-side JSON-schema support varies; enable explicitly.
            json_schema: false,
            embeddings: false,
            // Vision is model-dependent; the adapter can express image parts.
            vision: true,
            audio_input: false,
            audio_output: false,
            seed: true,
            max_context_tokens: None,
        }
    }
}

pub trait Provider {
    fn id(&self) -> &str;
    fn capabilities(&self) -> ProviderCapabilities;
    fn chat(&self, request: &ChatRequest, budget: &mut Budget) -> Result<ChatResponse, AiError>;
}

#[cfg(test)]
mod tests {
    use super::*;

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
