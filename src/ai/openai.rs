//! OpenAI-compatible chat and model-list adapter.
//!
//! The adapter speaks the `/v1/chat/completions` and `/v1/models` shapes used
//! by OpenAI, Ollama, llama.cpp, LM Studio, mistral.rs, and compatible cloud
//! gateways. It maps unsupported parameters to explicit errors rather than
//! dropping them.

use super::budget::Budget;
use super::embedding::Vector;
use super::http::{HttpResponse, StreamHandlers, Transport};
use super::json::{self, Json};
use super::provider::{
    AI_E_AUTH, AI_E_CONFIG, AI_E_JSON, AI_E_PROVIDER, AI_E_RATE_LIMIT, AiError, ChatRequest,
    ChatResponse, ContentPart, EmbeddingRequest, EmbeddingResponse, EmbeddingUsage, FinishReason,
    Message, Provider, ProviderCapabilities, ResponseFormat, Role, ToolCall, Usage,
};
use super::redact;
use super::retry::retry_after_from_header;
use super::sse::{SseEvent, SseParser};
use super::stream::{StreamEvent, ToolCallAssembler, ToolCallDelta};
use super::tokens::{
    check_context_limit, estimate_embedding_tokens, estimate_request_tokens, estimate_tokens,
};
use std::cell::Cell;
use std::time::Duration;

pub struct OpenAiProvider {
    id: String,
    base_url: String,
    api_key: Option<String>,
    transport: Box<dyn Transport>,
    capabilities: ProviderCapabilities,
}

impl OpenAiProvider {
    pub fn new(
        base_url: impl Into<String>,
        api_key: Option<String>,
        transport: Box<dyn Transport>,
    ) -> Self {
        Self {
            id: "openai".into(),
            base_url: base_url.into().trim_end_matches('/').to_string(),
            api_key,
            transport,
            capabilities: ProviderCapabilities::openai_compatible(),
        }
    }

    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = id.into();
        self
    }

    pub fn with_capabilities(mut self, capabilities: ProviderCapabilities) -> Self {
        self.capabilities = capabilities;
        self
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn models(&self) -> Result<Vec<String>, AiError> {
        let response =
            self.transport
                .execute("GET", &models_url(&self.base_url), &self.headers(), None)?;
        if !(200..300).contains(&response.status) {
            return Err(self.status_error(&response));
        }
        let value = json::parse(&response.body).map_err(|error| {
            AiError::new(
                AI_E_JSON,
                format!("provider returned invalid JSON: {error}"),
            )
        })?;
        let data = value.get("data").and_then(Json::as_array).ok_or_else(|| {
            AiError::new(
                AI_E_PROVIDER,
                "provider model list is missing the `data` array",
            )
        })?;
        let mut models = Vec::new();
        for item in data {
            if let Some(id) = item.get("id").and_then(Json::as_str) {
                models.push(id.to_string());
            }
        }
        if models.is_empty() {
            return Err(AiError::new(
                AI_E_PROVIDER,
                "provider returned no models; pull or configure a model first",
            ));
        }
        Ok(models)
    }

    fn headers(&self) -> Vec<(String, String)> {
        let mut headers = vec![("Content-Type".to_string(), "application/json".to_string())];
        if let Some(api_key) = &self.api_key {
            headers.push(("Authorization".to_string(), format!("Bearer {api_key}")));
        }
        headers
    }

    fn secrets(&self) -> Vec<&str> {
        self.api_key
            .as_deref()
            .filter(|key| !key.is_empty())
            .into_iter()
            .collect()
    }

    fn status_error(&self, response: &HttpResponse) -> AiError {
        self.status_error_body_with_retry_after(
            response.status,
            &response.body,
            retry_after_from_header(response.header("retry-after")),
        )
    }

    fn status_error_body_with_retry_after(
        &self,
        status: u16,
        body: &str,
        retry_after: Option<Duration>,
    ) -> AiError {
        let detail = redact::sanitize(&provider_detail(body), &self.secrets(), 512);
        let suffix = if detail.is_empty() {
            String::new()
        } else {
            format!(": {detail}")
        };
        let error = match status {
            401 | 403 => AiError::new(
                AI_E_AUTH,
                format!("provider authentication failed (HTTP {status}){suffix}"),
            ),
            429 => AiError::retryable(
                AI_E_RATE_LIMIT,
                format!("provider rate limit reached (HTTP 429){suffix}"),
            ),
            404 => AiError::new(
                AI_E_CONFIG,
                format!(
                    "provider endpoint or model was not found (HTTP 404){suffix}; check the endpoint and model name"
                ),
            ),
            500..=599 => AiError::retryable(
                AI_E_PROVIDER,
                format!("provider failed with HTTP {status}{suffix}"),
            ),
            status => AiError::new(
                AI_E_PROVIDER,
                format!("provider returned HTTP {status}{suffix}"),
            ),
        };
        if error.retryable {
            error.with_retry_after(retry_after)
        } else {
            error
        }
    }

    fn check_capabilities(&self, request: &ChatRequest) -> Result<(), AiError> {
        if !request.tools.is_empty() && !self.capabilities.tools {
            return Err(AiError::unsupported("tools", &self.id));
        }
        match &request.response_format {
            Some(ResponseFormat::JsonObject) if !self.capabilities.json_object => {
                return Err(AiError::unsupported(
                    "response_format=json_object",
                    &self.id,
                ));
            }
            Some(ResponseFormat::JsonSchema { .. }) if !self.capabilities.json_schema => {
                return Err(AiError::unsupported(
                    "response_format=json_schema",
                    &self.id,
                ));
            }
            _ => {}
        }
        if request.seed.is_some() && !self.capabilities.seed {
            return Err(AiError::unsupported("seed", &self.id));
        }
        for message in &request.messages {
            for part in &message.content {
                match part {
                    ContentPart::Text(_) => {}
                    ContentPart::ImageUrl { .. } if !self.capabilities.vision => {
                        return Err(AiError::unsupported("image content", &self.id));
                    }
                    ContentPart::AudioUrl { .. } if !self.capabilities.audio_input => {
                        return Err(AiError::unsupported("audio content", &self.id));
                    }
                    ContentPart::VideoUrl { .. } => {
                        return Err(AiError::unsupported("video content", &self.id));
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }
}

struct ChatStreamState<'a> {
    model: String,
    id: Option<String>,
    text: String,
    finish_reason: FinishReason,
    usage: Option<Usage>,
    done: bool,
    tool_calls: ToolCallAssembler,
    on_event: &'a mut dyn FnMut(StreamEvent),
    secrets: &'a [&'a str],
}

impl<'a> ChatStreamState<'a> {
    fn new(model: &str, on_event: &'a mut dyn FnMut(StreamEvent), secrets: &'a [&'a str]) -> Self {
        Self {
            model: model.to_string(),
            id: None,
            text: String::new(),
            finish_reason: FinishReason::Stop,
            usage: None,
            done: false,
            tool_calls: ToolCallAssembler::new(),
            on_event,
            secrets,
        }
    }

    fn handle(&mut self, event: SseEvent) -> Result<(), AiError> {
        if self.done {
            return Ok(());
        }
        let data = event.data.trim();
        if data.is_empty() {
            return Ok(());
        }
        if data == "[DONE]" {
            self.done = true;
            return Ok(());
        }
        let value = json::parse(data).map_err(|error| {
            AiError::new(
                AI_E_JSON,
                format!("provider stream frame is invalid JSON: {error}"),
            )
        })?;
        if let Some(error) = value.get("error") {
            let detail = redact::sanitize(
                &provider_detail(&error.to_compact_string()),
                self.secrets,
                512,
            );
            return Err(AiError::new(
                AI_E_PROVIDER,
                format!("provider error: {detail}"),
            ));
        }
        if let Some(id) = value.get("id").and_then(Json::as_str) {
            self.id = Some(id.to_string());
        }
        if let Some(model) = value.get("model").and_then(Json::as_str) {
            self.model = model.to_string();
        }
        if let Some(usage) = value.get("usage").filter(|usage| !usage.is_null()) {
            let prompt_tokens = usage
                .get("prompt_tokens")
                .and_then(Json::as_u64)
                .unwrap_or(0);
            let completion_tokens = usage
                .get("completion_tokens")
                .and_then(Json::as_u64)
                .unwrap_or(0);
            let total_tokens = usage
                .get("total_tokens")
                .and_then(Json::as_u64)
                .unwrap_or_else(|| prompt_tokens.saturating_add(completion_tokens));
            self.usage = Some(Usage {
                prompt_tokens,
                completion_tokens,
                total_tokens,
            });
        }
        let Some(choice) = value
            .get("choices")
            .and_then(Json::as_array)
            .and_then(|choices| choices.first())
        else {
            return Ok(());
        };
        if let Some(reason) = choice.get("finish_reason").and_then(Json::as_str) {
            self.finish_reason = FinishReason::from_wire(Some(reason));
        }
        let Some(delta) = choice.get("delta") else {
            return Ok(());
        };
        if let Some(calls) = delta.get("tool_calls").and_then(Json::as_array) {
            for call in calls {
                let delta = parse_streamed_tool_call(call)?;
                self.tool_calls.push(delta.clone())?;
                (self.on_event)(StreamEvent::ToolCallDelta(delta));
            }
        }
        let content = parse_content(delta.get("content"));
        if !content.is_empty() {
            self.text.push_str(&content);
            (self.on_event)(StreamEvent::TextDelta(content));
        }
        Ok(())
    }
}

fn parse_streamed_tool_call(call: &Json) -> Result<ToolCallDelta, AiError> {
    let index = call
        .get("index")
        .and_then(Json::as_u64)
        .and_then(|index| usize::try_from(index).ok())
        .ok_or_else(|| {
            AiError::new(
                AI_E_PROVIDER,
                "streamed tool call frame is missing a valid index",
            )
        })?;
    let id = call
        .get("id")
        .and_then(Json::as_str)
        .filter(|id| !id.is_empty())
        .map(ToString::to_string);
    let function = call.get("function");
    let name = function
        .and_then(|function| function.get("name"))
        .and_then(Json::as_str)
        .filter(|name| !name.is_empty())
        .map(ToString::to_string);
    let arguments = function
        .and_then(|function| function.get("arguments"))
        .map_or_else(String::new, |arguments| {
            arguments
                .as_str()
                .map_or_else(|| arguments.to_compact_string(), ToString::to_string)
        });
    Ok(ToolCallDelta {
        index,
        id,
        name,
        arguments,
    })
}

impl Provider for OpenAiProvider {
    fn id(&self) -> &str {
        &self.id
    }

    fn capabilities(&self) -> ProviderCapabilities {
        self.capabilities
    }

    fn models(&self) -> Result<Vec<String>, AiError> {
        OpenAiProvider::models(self)
    }

    fn chat(&self, request: &ChatRequest, budget: &mut Budget) -> Result<ChatResponse, AiError> {
        request.validate()?;
        self.check_capabilities(request)?;
        check_context_limit(request, self.capabilities.max_context_tokens)?;
        let prompt_estimate = estimate_request_tokens(request) as u64;
        budget.check_tokens(prompt_estimate)?;

        let body = build_request_json(request, false).to_compact_string();
        let timeout = request.timeout_ms.map(Duration::from_millis);
        let response = self.transport.execute_with_timeout(
            "POST",
            &chat_url(&self.base_url),
            &self.headers(),
            Some(&body),
            timeout,
        )?;
        if !(200..300).contains(&response.status) {
            return Err(self.status_error(&response));
        }
        let value = json::parse(&response.body).map_err(|error| {
            AiError::new(
                AI_E_JSON,
                format!("provider returned invalid JSON: {error}"),
            )
        })?;
        if let Some(error) = value.get("error") {
            let detail = redact::sanitize(
                &provider_detail(&error.to_compact_string()),
                &self.secrets(),
                512,
            );
            return Err(AiError::new(
                AI_E_PROVIDER,
                format!("provider error: {detail}"),
            ));
        }
        let choice = value
            .get("choices")
            .and_then(Json::as_array)
            .and_then(|choices| choices.first())
            .ok_or_else(|| AiError::new(AI_E_PROVIDER, "provider response is missing `choices`"))?;
        let message = choice
            .get("message")
            .ok_or_else(|| AiError::new(AI_E_PROVIDER, "provider response is missing `message`"))?;
        let text = parse_content(message.get("content"));
        let tool_calls = parse_tool_calls(message.get("tool_calls"));
        let prompt_tokens = value
            .get("usage")
            .and_then(|usage| usage.get("prompt_tokens"))
            .and_then(Json::as_u64)
            .unwrap_or(prompt_estimate);
        let completion_tokens = value
            .get("usage")
            .and_then(|usage| usage.get("completion_tokens"))
            .and_then(Json::as_u64)
            .unwrap_or_else(|| estimate_tokens(&text) as u64);
        let total_tokens = value
            .get("usage")
            .and_then(|usage| usage.get("total_tokens"))
            .and_then(Json::as_u64)
            .unwrap_or(prompt_tokens.saturating_add(completion_tokens));
        budget.record_tokens(total_tokens)?;

        Ok(ChatResponse {
            id: value
                .get("id")
                .and_then(Json::as_str)
                .map(ToString::to_string),
            model: value
                .get("model")
                .and_then(Json::as_str)
                .unwrap_or(&request.model)
                .to_string(),
            message: Message {
                role: Role::Assistant,
                content: if text.is_empty() {
                    Vec::new()
                } else {
                    vec![ContentPart::text(text)]
                },
                name: None,
                tool_call_id: None,
                tool_calls,
            },
            finish_reason: FinishReason::from_wire(
                choice.get("finish_reason").and_then(Json::as_str),
            ),
            usage: Usage {
                prompt_tokens,
                completion_tokens,
                total_tokens,
            },
        })
    }

    fn chat_stream(
        &self,
        request: &ChatRequest,
        budget: &mut Budget,
        on_delta: &mut dyn FnMut(&str),
    ) -> Result<ChatResponse, AiError> {
        let mut on_event = |event: StreamEvent| {
            if let StreamEvent::TextDelta(text) = event {
                on_delta(&text);
            }
        };
        self.chat_stream_events(request, budget, &mut on_event)
    }

    fn chat_stream_events(
        &self,
        request: &ChatRequest,
        budget: &mut Budget,
        on_event: &mut dyn FnMut(StreamEvent),
    ) -> Result<ChatResponse, AiError> {
        request.validate()?;
        self.check_capabilities(request)?;
        if !self.capabilities.streaming {
            return Err(AiError::unsupported("streaming", &self.id));
        }
        check_context_limit(request, self.capabilities.max_context_tokens)?;
        let prompt_estimate = estimate_request_tokens(request) as u64;
        budget.check_tokens(prompt_estimate)?;

        let body = build_request_json(request, true).to_compact_string();
        let timeout = request.timeout_ms.map(Duration::from_millis);
        let mut headers = self.headers();
        headers.retain(|(name, _)| !name.eq_ignore_ascii_case("accept"));
        headers.push(("Accept".into(), "text/event-stream".into()));

        let status = Cell::new(0_u16);
        let failed = Cell::new(false);
        let retry_after = Cell::new(None);
        let mut error_body = String::new();
        let mut parser = SseParser::default();
        let secrets = self.secrets();
        let mut state = ChatStreamState::new(&request.model, on_event, &secrets);
        let mut on_start = |response_status: u16, headers: &[(String, String)]| {
            status.set(response_status);
            failed.set(!(200..300).contains(&response_status));
            retry_after.set(
                headers
                    .iter()
                    .find(|(name, _)| name.eq_ignore_ascii_case("retry-after"))
                    .and_then(|(_, value)| retry_after_from_header(Some(value))),
            );
            Ok(())
        };
        let mut on_chunk = |chunk: &[u8]| {
            if failed.get() {
                if error_body.len() < 64 * 1024 {
                    let remaining = 64 * 1024 - error_body.len();
                    let take = remaining.min(chunk.len());
                    error_body.push_str(&String::from_utf8_lossy(&chunk[..take]));
                }
                return Ok(());
            }
            let events = parser.push_bytes(chunk).map_err(|error| {
                AiError::new(
                    AI_E_PROVIDER,
                    format!("provider event stream is invalid: {error}"),
                )
            })?;
            for event in events {
                state.handle(event)?;
            }
            Ok(())
        };
        let mut handlers = StreamHandlers {
            on_start: &mut on_start,
            on_chunk: &mut on_chunk,
        };
        self.transport.stream(
            "POST",
            &chat_url(&self.base_url),
            &headers,
            Some(&body),
            timeout,
            &mut handlers,
        )?;
        if failed.get() {
            return Err(self.status_error_body_with_retry_after(
                status.get(),
                &error_body,
                retry_after.get(),
            ));
        }
        let events = parser.finish().map_err(|error| {
            AiError::new(
                AI_E_PROVIDER,
                format!("provider event stream is invalid: {error}"),
            )
        })?;
        for event in events {
            state.handle(event)?;
        }
        let tool_calls = state.tool_calls.finish()?;
        let text = state.text;
        let prompt_tokens = state
            .usage
            .as_ref()
            .map_or(prompt_estimate, |usage| usage.prompt_tokens);
        let completion_estimate = estimate_tokens(&text)
            + tool_calls
                .iter()
                .map(|call| estimate_tokens(&call.arguments))
                .sum::<usize>();
        let completion_tokens = state.usage.as_ref().map_or_else(
            || u64::try_from(completion_estimate).unwrap_or(u64::MAX),
            |usage| usage.completion_tokens,
        );
        let total_tokens = state.usage.as_ref().map_or_else(
            || prompt_tokens.saturating_add(completion_tokens),
            |usage| usage.total_tokens,
        );
        budget.record_tokens(total_tokens)?;
        Ok(ChatResponse {
            id: state.id,
            model: state.model,
            message: Message {
                role: Role::Assistant,
                content: if text.is_empty() {
                    Vec::new()
                } else {
                    vec![ContentPart::text(text)]
                },
                name: None,
                tool_call_id: None,
                tool_calls,
            },
            finish_reason: state.finish_reason,
            usage: Usage {
                prompt_tokens,
                completion_tokens,
                total_tokens,
            },
        })
    }

    fn embed(
        &self,
        request: &EmbeddingRequest,
        budget: &mut Budget,
    ) -> Result<EmbeddingResponse, AiError> {
        request.validate()?;
        if !self.capabilities.embeddings {
            return Err(AiError::unsupported("embeddings", &self.id));
        }
        let estimate = u64::try_from(estimate_embedding_tokens(request)).unwrap_or(u64::MAX);
        budget.check_tokens(estimate)?;

        let body = build_embedding_request_json(request).to_compact_string();
        let timeout = request.timeout_ms.map(Duration::from_millis);
        let response = self.transport.execute_with_timeout(
            "POST",
            &embeddings_url(&self.base_url),
            &self.headers(),
            Some(&body),
            timeout,
        )?;
        if !(200..300).contains(&response.status) {
            return Err(self.status_error(&response));
        }
        let value = json::parse(&response.body).map_err(|error| {
            AiError::new(
                AI_E_JSON,
                format!("provider returned invalid JSON: {error}"),
            )
        })?;
        if let Some(error) = value.get("error") {
            let detail = redact::sanitize(
                &provider_detail(&error.to_compact_string()),
                &self.secrets(),
                512,
            );
            return Err(AiError::new(
                AI_E_PROVIDER,
                format!("provider error: {detail}"),
            ));
        }
        let data = value.get("data").and_then(Json::as_array).ok_or_else(|| {
            AiError::new(
                AI_E_PROVIDER,
                "provider embedding response is missing the `data` array",
            )
        })?;

        let mut indexed = Vec::with_capacity(data.len());
        for (position, item) in data.iter().enumerate() {
            let index = match item.get("index") {
                Some(value) => value.as_u64().ok_or_else(|| {
                    AiError::new(
                        AI_E_PROVIDER,
                        "provider embedding `index` must be a non-negative integer",
                    )
                })?,
                None => position as u64,
            };
            let index = usize::try_from(index).map_err(|_| {
                AiError::new(AI_E_PROVIDER, "provider embedding `index` is out of range")
            })?;
            indexed.push((index, item));
        }
        indexed.sort_by_key(|(index, _)| *index);
        let mut seen = vec![false; request.input.len()];
        for (index, _) in &indexed {
            if *index >= seen.len() || seen[*index] {
                return Err(AiError::new(
                    AI_E_PROVIDER,
                    "provider embedding indexes are duplicate or out of range",
                ));
            }
            seen[*index] = true;
        }
        if indexed.len() != request.input.len() {
            return Err(AiError::new(
                AI_E_PROVIDER,
                format!(
                    "provider returned {} embeddings for {} inputs",
                    indexed.len(),
                    request.input.len()
                ),
            ));
        }

        let mut vectors = Vec::with_capacity(indexed.len());
        for (_, item) in indexed {
            let embedding = item
                .get("embedding")
                .and_then(Json::as_array)
                .ok_or_else(|| {
                    AiError::new(
                        AI_E_PROVIDER,
                        "provider embedding item is missing the `embedding` array",
                    )
                })?;
            let mut values = Vec::with_capacity(embedding.len());
            for value in embedding {
                let number = value.as_f64().ok_or_else(|| {
                    AiError::new(
                        AI_E_PROVIDER,
                        "provider embedding values must be finite numbers",
                    )
                })?;
                values.push(number as f32);
            }
            vectors.push(Vector::new(values)?);
        }
        let dimensions = vectors.first().map_or(0, Vector::len);
        if vectors.iter().any(|vector| vector.len() != dimensions) {
            return Err(AiError::new(
                AI_E_PROVIDER,
                "provider returned embeddings with inconsistent dimensions",
            ));
        }
        if let Some(expected) = request.dimensions
            && dimensions != expected
        {
            return Err(AiError::new(
                AI_E_PROVIDER,
                format!(
                    "provider returned {dimensions}-dimensional embeddings, but {expected} were requested"
                ),
            ));
        }

        let prompt_tokens = value
            .get("usage")
            .and_then(|usage| usage.get("prompt_tokens"))
            .and_then(Json::as_u64)
            .unwrap_or(estimate);
        let total_tokens = value
            .get("usage")
            .and_then(|usage| usage.get("total_tokens"))
            .and_then(Json::as_u64)
            .unwrap_or(prompt_tokens);
        let usage = EmbeddingUsage {
            prompt_tokens,
            total_tokens,
        };
        budget.record_tokens(usage.total_tokens)?;

        Ok(EmbeddingResponse {
            model: value
                .get("model")
                .and_then(Json::as_str)
                .unwrap_or(&request.model)
                .to_string(),
            vectors,
            usage,
        })
    }
}

fn chat_url(base_url: &str) -> String {
    let base = base_url.trim_end_matches('/');
    if base.ends_with("/chat/completions") {
        base.to_string()
    } else if base.ends_with("/v1") {
        format!("{base}/chat/completions")
    } else {
        format!("{base}/v1/chat/completions")
    }
}

fn models_url(base_url: &str) -> String {
    let base = base_url.trim_end_matches('/');
    if base.ends_with("/models") {
        base.to_string()
    } else if base.ends_with("/chat/completions") {
        format!("{}/models", base.trim_end_matches("/chat/completions"))
    } else if base.ends_with("/v1") {
        format!("{base}/models")
    } else {
        format!("{base}/v1/models")
    }
}

fn embeddings_url(base_url: &str) -> String {
    let base = base_url.trim_end_matches('/');
    if base.ends_with("/embeddings") {
        base.to_string()
    } else if base.ends_with("/chat/completions") {
        format!("{}/embeddings", base.trim_end_matches("/chat/completions"))
    } else if base.ends_with("/v1") {
        format!("{base}/embeddings")
    } else {
        format!("{base}/v1/embeddings")
    }
}

fn build_embedding_request_json(request: &EmbeddingRequest) -> Json {
    let mut entries: Vec<(&str, Json)> = vec![
        ("model", Json::from(request.model.clone())),
        (
            "input",
            Json::Array(
                request
                    .input
                    .iter()
                    .map(|text| Json::from(text.clone()))
                    .collect(),
            ),
        ),
    ];
    if let Some(dimensions) = request.dimensions {
        entries.push(("dimensions", Json::from(dimensions as u64)));
    }
    Json::object(entries)
}

fn build_request_json(request: &ChatRequest, stream: bool) -> Json {
    let mut entries: Vec<(&str, Json)> = Vec::new();
    entries.push(("model", Json::from(request.model.clone())));
    entries.push((
        "messages",
        Json::Array(request.messages.iter().map(message_json).collect()),
    ));
    if let Some(temperature) = request.temperature {
        entries.push(("temperature", Json::from(temperature)));
    }
    if let Some(top_p) = request.top_p {
        entries.push(("top_p", Json::from(top_p)));
    }
    if let Some(max_tokens) = request.max_tokens {
        entries.push(("max_tokens", Json::from(u64::from(max_tokens))));
    }
    if !request.stop.is_empty() {
        entries.push((
            "stop",
            Json::Array(
                request
                    .stop
                    .iter()
                    .map(|stop| Json::from(stop.clone()))
                    .collect(),
            ),
        ));
    }
    if let Some(seed) = request.seed {
        entries.push(("seed", Json::from(seed)));
    }
    if !request.tools.is_empty() {
        entries.push((
            "tools",
            Json::Array(
                request
                    .tools
                    .iter()
                    .map(|tool| {
                        Json::object([
                            ("type", Json::from("function")),
                            (
                                "function",
                                Json::object([
                                    ("name", Json::from(tool.name.clone())),
                                    ("description", Json::from(tool.description.clone())),
                                    ("parameters", tool.parameters.clone()),
                                ]),
                            ),
                        ])
                    })
                    .collect(),
            ),
        ));
    }
    if let Some(response_format) = &request.response_format {
        let value = match response_format {
            ResponseFormat::Text => None,
            ResponseFormat::JsonObject => Some(Json::object([("type", Json::from("json_object"))])),
            ResponseFormat::JsonSchema { name, schema } => Some(Json::object([
                ("type", Json::from("json_schema")),
                (
                    "json_schema",
                    Json::object([
                        ("name", Json::from(name.clone())),
                        ("schema", schema.clone()),
                    ]),
                ),
            ])),
        };
        if let Some(value) = value {
            entries.push(("response_format", value));
        }
    }
    if stream {
        entries.push(("stream", Json::Bool(true)));
        entries.push((
            "stream_options",
            Json::object([("include_usage", Json::Bool(true))]),
        ));
    }
    Json::object(entries)
}

fn message_json(message: &Message) -> Json {
    let mut entries: Vec<(&str, Json)> = vec![("role", Json::from(message.role.as_str()))];
    let has_non_text = message
        .content
        .iter()
        .any(|part| !matches!(part, ContentPart::Text(_)));
    if has_non_text {
        entries.push((
            "content",
            Json::Array(message.content.iter().map(content_part_json).collect()),
        ));
    } else {
        let text = message
            .content
            .iter()
            .filter_map(ContentPart::text_value)
            .collect::<Vec<_>>()
            .join("\n");
        if text.is_empty() && !message.tool_calls.is_empty() {
            entries.push(("content", Json::Null));
        } else {
            entries.push(("content", Json::from(text)));
        }
    }
    if let Some(name) = &message.name {
        entries.push(("name", Json::from(name.clone())));
    }
    if let Some(tool_call_id) = &message.tool_call_id {
        entries.push(("tool_call_id", Json::from(tool_call_id.clone())));
    }
    if !message.tool_calls.is_empty() {
        entries.push((
            "tool_calls",
            Json::Array(
                message
                    .tool_calls
                    .iter()
                    .map(|call| {
                        Json::object([
                            ("id", Json::from(call.id.clone())),
                            ("type", Json::from("function")),
                            (
                                "function",
                                Json::object([
                                    ("name", Json::from(call.name.clone())),
                                    ("arguments", Json::from(call.arguments.clone())),
                                ]),
                            ),
                        ])
                    })
                    .collect(),
            ),
        ));
    }
    Json::object(entries)
}

fn content_part_json(part: &ContentPart) -> Json {
    match part {
        ContentPart::Text(text) => Json::object([
            ("type", Json::from("text")),
            ("text", Json::from(text.clone())),
        ]),
        ContentPart::ImageUrl { url, detail } => {
            let mut image = vec![("url", Json::from(url.clone()))];
            if let Some(detail) = detail {
                image.push(("detail", Json::from(detail.clone())));
            }
            Json::object([
                ("type", Json::from("image_url")),
                ("image_url", Json::object(image)),
            ])
        }
        ContentPart::AudioUrl { url } => Json::object([
            ("type", Json::from("input_audio")),
            (
                "input_audio",
                Json::object([("url", Json::from(url.clone()))]),
            ),
        ]),
        ContentPart::VideoUrl { url } => Json::object([
            ("type", Json::from("video_url")),
            (
                "video_url",
                Json::object([("url", Json::from(url.clone()))]),
            ),
        ]),
    }
}

fn parse_content(value: Option<&Json>) -> String {
    match value {
        Some(Json::String(text)) => text.clone(),
        Some(Json::Array(parts)) => parts
            .iter()
            .filter_map(|part| part.get("text").and_then(Json::as_str))
            .collect::<Vec<_>>()
            .join(""),
        _ => String::new(),
    }
}

fn parse_tool_calls(value: Option<&Json>) -> Vec<ToolCall> {
    let Some(calls) = value.and_then(Json::as_array) else {
        return Vec::new();
    };
    calls
        .iter()
        .enumerate()
        .filter_map(|(index, call)| {
            let function = call.get("function")?;
            let name = function.get("name").and_then(Json::as_str)?;
            let arguments = function
                .get("arguments")
                .map_or_else(String::new, |arguments| {
                    arguments
                        .as_str()
                        .map_or_else(|| arguments.to_compact_string(), ToString::to_string)
                });
            let id = call
                .get("id")
                .and_then(Json::as_str)
                .filter(|id| !id.is_empty())
                .map_or_else(|| format!("call-{index}"), ToString::to_string);
            Some(ToolCall {
                id,
                name: name.to_string(),
                arguments,
            })
        })
        .collect()
}

fn provider_detail(body: &str) -> String {
    if let Ok(value) = json::parse(body) {
        if let Some(error) = value.get("error") {
            if let Some(message) = error.get("message").and_then(Json::as_str) {
                return message.to_string();
            }
            if let Some(message) = error.as_str() {
                return message.to_string();
            }
        }
        if let Some(message) = value.get("message").and_then(Json::as_str) {
            return message.to_string();
        }
    }
    body.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::http::HttpResponse;
    use crate::ai::provider::{AI_E_BUDGET, AI_E_UNSUPPORTED, ToolSpec};
    use std::cell::RefCell;
    use std::rc::Rc;

    type Captured = (
        String,
        String,
        Vec<(String, String)>,
        Option<String>,
        Option<Duration>,
    );
    type Capture = Rc<RefCell<Vec<Captured>>>;

    struct StubTransport {
        status: u16,
        body: String,
        captured: Capture,
    }

    impl StubTransport {
        fn new(status: u16, body: impl Into<String>) -> (Self, Capture) {
            let captured = Rc::new(RefCell::new(Vec::new()));
            (
                Self {
                    status,
                    body: body.into(),
                    captured: Rc::clone(&captured),
                },
                captured,
            )
        }
    }

    impl Transport for StubTransport {
        fn execute(
            &self,
            method: &str,
            url: &str,
            headers: &[(String, String)],
            body: Option<&str>,
        ) -> Result<HttpResponse, AiError> {
            self.captured.borrow_mut().push((
                method.to_string(),
                url.to_string(),
                headers.to_vec(),
                body.map(ToString::to_string),
                None,
            ));
            Ok(HttpResponse {
                status: self.status,
                headers: Vec::new(),
                body: self.body.clone(),
            })
        }

        fn execute_with_timeout(
            &self,
            method: &str,
            url: &str,
            headers: &[(String, String)],
            body: Option<&str>,
            timeout: Option<Duration>,
        ) -> Result<HttpResponse, AiError> {
            self.captured.borrow_mut().push((
                method.to_string(),
                url.to_string(),
                headers.to_vec(),
                body.map(ToString::to_string),
                timeout,
            ));
            Ok(HttpResponse {
                status: self.status,
                headers: Vec::new(),
                body: self.body.clone(),
            })
        }
    }

    struct StreamStubTransport {
        status: u16,
        chunks: Vec<Vec<u8>>,
        captured: Capture,
    }

    impl StreamStubTransport {
        fn new(status: u16, chunks: Vec<Vec<u8>>) -> (Self, Capture) {
            let captured = Rc::new(RefCell::new(Vec::new()));
            (
                Self {
                    status,
                    chunks,
                    captured: Rc::clone(&captured),
                },
                captured,
            )
        }
    }

    impl Transport for StreamStubTransport {
        fn execute(
            &self,
            _method: &str,
            _url: &str,
            _headers: &[(String, String)],
            _body: Option<&str>,
        ) -> Result<HttpResponse, AiError> {
            Ok(HttpResponse {
                status: self.status,
                headers: Vec::new(),
                body: String::new(),
            })
        }

        fn stream(
            &self,
            method: &str,
            url: &str,
            headers: &[(String, String)],
            body: Option<&str>,
            timeout: Option<Duration>,
            handlers: &mut StreamHandlers<'_>,
        ) -> Result<(), AiError> {
            self.captured.borrow_mut().push((
                method.to_string(),
                url.to_string(),
                headers.to_vec(),
                body.map(ToString::to_string),
                timeout,
            ));
            (handlers.on_start)(self.status, &[])?;
            for chunk in &self.chunks {
                (handlers.on_chunk)(chunk)?;
            }
            Ok(())
        }
    }

    fn stream_provider(status: u16, chunks: Vec<Vec<u8>>) -> (OpenAiProvider, Capture) {
        let (transport, capture) = StreamStubTransport::new(status, chunks);
        (
            OpenAiProvider::new(
                "http://127.0.0.1:11434/v1",
                Some("sk-secret".into()),
                Box::new(transport),
            ),
            capture,
        )
    }

    fn sse(data: &str) -> Vec<u8> {
        format!("data: {data}\n\n").into_bytes()
    }

    fn response_body() -> String {
        r#"{"id":"chat-1","model":"test-model","choices":[{"message":{"role":"assistant","content":"hello"},"finish_reason":"stop"}],"usage":{"prompt_tokens":7,"completion_tokens":3,"total_tokens":10}}"#.to_string()
    }

    fn provider_with_capture(status: u16, body: String) -> (OpenAiProvider, Capture) {
        let (transport, capture) = StubTransport::new(status, body);
        (
            OpenAiProvider::new(
                "http://127.0.0.1:11434/v1",
                Some("sk-secret".into()),
                Box::new(transport),
            ),
            capture,
        )
    }

    fn make_provider(status: u16, body: String) -> OpenAiProvider {
        provider_with_capture(status, body).0
    }

    #[test]
    fn sends_and_parses_an_openai_chat_request() {
        let (provider, capture) = provider_with_capture(200, response_body());
        let mut budget = Budget::default();
        let mut request = ChatRequest::new(
            "test-model",
            vec![Message::system("be brief"), Message::user("hi")],
        );
        request.temperature = Some(0.2);
        request.max_tokens = Some(64);
        request.timeout_ms = Some(1500);
        let response = provider.chat(&request, &mut budget).unwrap();
        assert_eq!(response.message.text_content(), "hello");
        assert_eq!(response.usage.total_tokens, 10);
        assert_eq!(budget.used_tokens(), 10);

        let captured = capture.borrow();
        assert_eq!(captured[0].0, "POST");
        assert_eq!(captured[0].1, "http://127.0.0.1:11434/v1/chat/completions");
        assert_eq!(captured[0].4, Some(Duration::from_millis(1500)));
        assert!(
            captured[0]
                .2
                .iter()
                .any(|(name, value)| { name == "Authorization" && value == "Bearer sk-secret" })
        );
        let body = json::parse(captured[0].3.as_deref().unwrap()).unwrap();
        assert_eq!(body.get("model").unwrap().as_str(), Some("test-model"));
        assert_eq!(body.get("temperature").unwrap().as_f64(), Some(0.2));
        assert_eq!(body.get("max_tokens").unwrap().as_u64(), Some(64));
        assert!(body.get("timeout_ms").is_none());
        assert!(body.get("stream").is_none());
    }

    #[test]
    fn streams_text_deltas_and_usage() {
        let (provider, capture) = stream_provider(
            200,
            vec![
                sse(
                    r#"{"id":"chat-9","model":"test-model","choices":[{"delta":{"role":"assistant","content":"hel"},"finish_reason":null}]}"#,
                ),
                sse(r#"{"choices":[{"delta":{"content":"lo"},"finish_reason":null}]}"#),
                sse(r#"{"choices":[{"delta":{},"finish_reason":"stop"}]}"#),
                sse(
                    r#"{"choices":[],"usage":{"prompt_tokens":7,"completion_tokens":2,"total_tokens":9}}"#,
                ),
                sse("[DONE]"),
            ],
        );
        let mut budget = Budget::default();
        let mut request = ChatRequest::new("test-model", vec![Message::user("hi")]);
        request.timeout_ms = Some(1500);
        let mut deltas = Vec::new();
        let response = provider
            .chat_stream(&request, &mut budget, &mut |delta| {
                deltas.push(delta.to_string());
            })
            .unwrap();
        assert_eq!(deltas, vec!["hel", "lo"]);
        assert_eq!(response.message.text_content(), "hello");
        assert_eq!(response.id.as_deref(), Some("chat-9"));
        assert_eq!(response.model, "test-model");
        assert_eq!(response.finish_reason, FinishReason::Stop);
        assert_eq!(response.usage.total_tokens, 9);
        assert_eq!(budget.used_tokens(), 9);

        let captured = capture.borrow();
        assert_eq!(captured[0].0, "POST");
        assert_eq!(captured[0].1, "http://127.0.0.1:11434/v1/chat/completions");
        assert_eq!(captured[0].4, Some(Duration::from_millis(1500)));
        assert!(
            captured[0]
                .2
                .iter()
                .any(|(name, value)| name == "Accept" && value == "text/event-stream")
        );
        let body = json::parse(captured[0].3.as_deref().unwrap()).unwrap();
        assert_eq!(body.get("stream").unwrap().as_bool(), Some(true));
        assert_eq!(
            body.get("stream_options")
                .unwrap()
                .get("include_usage")
                .unwrap()
                .as_bool(),
            Some(true)
        );
    }

    #[test]
    fn stream_preserves_utf8_split_across_chunks() {
        let encoded = format!(
            "data: {}\n\n",
            r#"{"choices":[{"delta":{"content":"hé😀"},"finish_reason":"stop"}]}"#
        )
        .into_bytes();
        let split = encoded
            .iter()
            .position(|byte| *byte == 0xC3)
            .expect("é start byte")
            + 1;
        let (provider, _) = stream_provider(
            200,
            vec![encoded[..split].to_vec(), encoded[split..].to_vec()],
        );
        let mut budget = Budget::default();
        let request = ChatRequest::new("m", vec![Message::user("hi")]);
        let mut deltas = Vec::new();
        let response = provider
            .chat_stream(&request, &mut budget, &mut |delta| {
                deltas.push(delta.to_string());
            })
            .unwrap();
        assert_eq!(deltas, vec!["hé😀"]);
        assert_eq!(response.message.text_content(), "hé😀");
    }

    #[test]
    fn stream_ignores_frames_after_done() {
        let (provider, _) = stream_provider(
            200,
            vec![
                sse(r#"{"choices":[{"delta":{"content":"A"},"finish_reason":null}]}"#),
                sse("[DONE]"),
                sse(r#"{"choices":[{"delta":{"content":"B"},"finish_reason":null}]}"#),
            ],
        );
        let mut budget = Budget::default();
        let request = ChatRequest::new("m", vec![Message::user("hi")]);
        let mut deltas = Vec::new();
        let response = provider
            .chat_stream(&request, &mut budget, &mut |delta| {
                deltas.push(delta.to_string());
            })
            .unwrap();
        assert_eq!(deltas, vec!["A"]);
        assert_eq!(response.message.text_content(), "A");
    }

    #[test]
    fn stream_redacts_error_frames() {
        let (provider, _) = stream_provider(
            200,
            vec![sse(r#"{"error":{"message":"bad key sk-secret"}}"#)],
        );
        let mut budget = Budget::default();
        let request = ChatRequest::new("m", vec![Message::user("hi")]);
        let error = provider
            .chat_stream(&request, &mut budget, &mut |_| {})
            .unwrap_err();
        assert_eq!(error.code, AI_E_PROVIDER);
        assert!(error.message.contains("[REDACTED]"));
        assert!(!error.message.contains("sk-secret"));
    }

    #[test]
    fn stream_maps_http_errors() {
        let (provider, _) =
            stream_provider(429, vec![br#"{"error":{"message":"slow down"}}"#.to_vec()]);
        let mut budget = Budget::default();
        let request = ChatRequest::new("m", vec![Message::user("hi")]);
        let error = provider
            .chat_stream(&request, &mut budget, &mut |_| {})
            .unwrap_err();
        assert_eq!(error.code, AI_E_RATE_LIMIT);
        assert!(error.retryable);
    }

    #[test]
    fn streams_and_assembles_tool_call_arguments() {
        let (provider, _) = stream_provider(
            200,
            vec![
                sse(
                    r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call-1","function":{"name":"word_count","arguments":""}}]},"finish_reason":null}]}"#,
                ),
                sse(
                    r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"text\":"}}]},"finish_reason":null}]}"#,
                ),
                sse(
                    r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"\"a b c\"}"}}]},"finish_reason":null}]}"#,
                ),
                sse(r#"{"choices":[{"delta":{},"finish_reason":"tool_calls"}]}"#),
                sse("[DONE]"),
            ],
        );
        let mut budget = Budget::default();
        let mut request = ChatRequest::new("m", vec![Message::user("hi")]);
        request.tools.push(ToolSpec {
            name: "word_count".into(),
            description: "count words".into(),
            parameters: Json::object([("type", Json::from("object"))]),
        });
        let mut events = Vec::new();
        let response = provider
            .chat_stream_events(&request, &mut budget, &mut |event| events.push(event))
            .unwrap();
        assert_eq!(
            response.message.tool_calls,
            vec![ToolCall {
                id: "call-1".into(),
                name: "word_count".into(),
                arguments: r#"{"text":"a b c"}"#.into(),
            }]
        );
        assert_eq!(response.finish_reason, FinishReason::ToolCalls);
        assert!(response.message.content.is_empty());
        assert_eq!(events.len(), 3);
        assert!(
            events
                .iter()
                .all(|event| matches!(event, StreamEvent::ToolCallDelta(_)))
        );
        assert_eq!(budget.used_tokens(), response.usage.total_tokens);
    }

    #[test]
    fn chat_stream_forwards_only_text_deltas_for_tool_calls() {
        let (provider, _) = stream_provider(
            200,
            vec![
                sse(r#"{"choices":[{"delta":{"content":"checking "},"finish_reason":null}]}"#),
                sse(
                    r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call-1","function":{"name":"word_count","arguments":"{}"}}]},"finish_reason":null}]}"#,
                ),
                sse(r#"{"choices":[{"delta":{},"finish_reason":"tool_calls"}]}"#),
                sse("[DONE]"),
            ],
        );
        let mut budget = Budget::default();
        let request = ChatRequest::new("m", vec![Message::user("hi")]);
        let mut deltas = Vec::new();
        let response = provider
            .chat_stream(&request, &mut budget, &mut |delta| {
                deltas.push(delta.to_string());
            })
            .unwrap();
        assert_eq!(deltas, vec!["checking "]);
        assert_eq!(response.message.text_content(), "checking ");
        assert_eq!(response.message.tool_calls.len(), 1);
    }

    #[test]
    fn stream_rejects_incomplete_tool_calls() {
        let (provider, _) = stream_provider(
            200,
            vec![
                sse(
                    r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"name":"word_count","arguments":"{}"}}]},"finish_reason":null}]}"#,
                ),
                sse("[DONE]"),
            ],
        );
        let mut budget = Budget::default();
        let request = ChatRequest::new("m", vec![Message::user("hi")]);
        let error = provider
            .chat_stream_events(&request, &mut budget, &mut |_| {})
            .unwrap_err();
        assert_eq!(error.code, AI_E_PROVIDER);
        assert!(error.message.contains("missing an id"));

        let (provider, _) = stream_provider(
            200,
            vec![
                sse(
                    r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call-1","function":{"name":"word_count","arguments":"{\"text\":"}}]},"finish_reason":null}]}"#,
                ),
                sse("[DONE]"),
            ],
        );
        let error = provider
            .chat_stream_events(&request, &mut budget, &mut |_| {})
            .unwrap_err();
        assert_eq!(error.code, AI_E_PROVIDER);
        assert!(error.message.contains("invalid JSON"));
    }

    #[test]
    fn stream_estimates_usage_when_the_provider_omits_it() {
        let (provider, _) = stream_provider(
            200,
            vec![sse(
                r#"{"choices":[{"delta":{"content":"hello"},"finish_reason":"stop"}]}"#,
            )],
        );
        let mut budget = Budget::default();
        let request = ChatRequest::new("m", vec![Message::user("hi")]);
        let response = provider
            .chat_stream(&request, &mut budget, &mut |_| {})
            .unwrap();
        assert!(response.usage.prompt_tokens > 0);
        assert!(response.usage.completion_tokens > 0);
        assert_eq!(
            response.usage.total_tokens,
            response.usage.prompt_tokens + response.usage.completion_tokens
        );
        assert_eq!(budget.used_tokens(), response.usage.total_tokens);
    }

    #[test]
    fn parses_tool_calls() {
        let body = r#"{"model":"m","choices":[{"message":{"role":"assistant","content":null,"tool_calls":[{"id":"call-9","type":"function","function":{"name":"word_count","arguments":"{\"text\":\"a b\"}"}}]},"finish_reason":"tool_calls"}],"usage":{"prompt_tokens":4,"completion_tokens":2,"total_tokens":6}}"#;
        let provider = make_provider(200, body.to_string());
        let mut budget = Budget::default();
        let request = ChatRequest::new("m", vec![Message::user("count")]);
        let response = provider.chat(&request, &mut budget).unwrap();
        assert_eq!(response.message.tool_calls.len(), 1);
        assert_eq!(response.message.tool_calls[0].name, "word_count");
        assert_eq!(
            response.message.tool_calls[0].arguments,
            r#"{"text":"a b"}"#
        );
        assert_eq!(response.finish_reason, FinishReason::ToolCalls);
    }

    #[test]
    fn maps_http_errors_without_leaking_the_key() {
        let body = r#"{"error":{"message":"bad key sk-secret"}}"#;
        let provider = make_provider(401, body.to_string());
        let mut budget = Budget::default();
        let request = ChatRequest::new("m", vec![Message::user("hi")]);
        let error = provider.chat(&request, &mut budget).unwrap_err();
        assert_eq!(error.code, AI_E_AUTH);
        assert!(!error.message.contains("sk-secret"));
        assert!(error.message.contains("[REDACTED]"));

        let provider = make_provider(429, r#"{"error":{"message":"slow down"}}"#.to_string());
        let error = provider.chat(&request, &mut budget).unwrap_err();
        assert_eq!(error.code, AI_E_RATE_LIMIT);
        assert!(error.retryable);
    }

    #[test]
    fn attaches_retry_after_only_to_retryable_statuses() {
        let provider = make_provider(200, response_body());
        let delay = Some(Duration::from_secs(3));

        let error = provider.status_error_body_with_retry_after(429, "", delay);
        assert_eq!(error.code, AI_E_RATE_LIMIT);
        assert_eq!(error.retry_after, delay);

        let error = provider.status_error_body_with_retry_after(503, "", delay);
        assert_eq!(error.code, AI_E_PROVIDER);
        assert_eq!(error.retry_after, delay);

        let error = provider.status_error_body_with_retry_after(401, "", delay);
        assert_eq!(error.code, AI_E_AUTH);
        assert_eq!(error.retry_after, None);
    }

    #[test]
    fn rejects_unsupported_parameters_instead_of_dropping_them() {
        let mut capabilities = ProviderCapabilities::openai_compatible();
        capabilities.tools = false;
        capabilities.seed = false;
        let provider = make_provider(200, response_body()).with_capabilities(capabilities);
        let mut budget = Budget::default();
        let mut request = ChatRequest::new("m", vec![Message::user("hi")]);
        request.tools = vec![ToolSpec {
            name: "lookup".into(),
            description: "lookup".into(),
            parameters: Json::object([] as [(&str, Json); 0]),
        }];
        assert_eq!(
            provider.chat(&request, &mut budget).unwrap_err().code,
            AI_E_UNSUPPORTED
        );
        request.tools.clear();
        request.seed = Some(1);
        assert_eq!(
            provider.chat(&request, &mut budget).unwrap_err().code,
            AI_E_UNSUPPORTED
        );
    }

    #[test]
    fn enforces_the_token_budget_before_transport() {
        let provider = make_provider(200, response_body());
        let mut budget = Budget::default().with_tokens(1);
        let request = ChatRequest::new("m", vec![Message::user("a long prompt")]);
        assert_eq!(
            provider.chat(&request, &mut budget).unwrap_err().code,
            AI_E_BUDGET
        );
    }

    #[test]
    fn rejects_malformed_responses() {
        let provider = make_provider(200, "not json".into());
        let mut budget = Budget::default();
        let request = ChatRequest::new("m", vec![Message::user("hi")]);
        assert_eq!(
            provider.chat(&request, &mut budget).unwrap_err().code,
            AI_E_JSON
        );
    }

    #[test]
    fn lists_models() {
        let (provider, capture) =
            provider_with_capture(200, r#"{"data":[{"id":"a"},{"id":"b"}]}"#.to_string());
        let models = provider.models().unwrap();
        assert_eq!(models, vec!["a", "b"]);
        let captured = capture.borrow();
        assert_eq!(captured[0].0, "GET");
        assert_eq!(captured[0].1, "http://127.0.0.1:11434/v1/models");
    }

    fn embedding_body() -> String {
        r#"{"model":"embed-demo","data":[{"index":1,"embedding":[0.0,1.0]},{"index":0,"embedding":[1.0,0.0]}],"usage":{"prompt_tokens":4,"total_tokens":4}}"#.to_string()
    }

    #[test]
    fn sends_and_parses_an_embedding_batch() {
        let (provider, capture) = provider_with_capture(200, embedding_body());
        let mut budget = Budget::default();
        let mut request =
            EmbeddingRequest::new("embed-demo", vec!["alpha".to_string(), "beta".to_string()]);
        request.dimensions = Some(2);
        request.timeout_ms = Some(900);
        let response = provider.embed(&request, &mut budget).unwrap();
        assert_eq!(response.model, "embed-demo");
        assert_eq!(response.vectors[0].values(), &[1.0, 0.0]);
        assert_eq!(response.vectors[1].values(), &[0.0, 1.0]);
        assert_eq!(response.usage.prompt_tokens, 4);
        assert_eq!(response.usage.total_tokens, 4);
        assert_eq!(budget.used_tokens(), 4);

        let captured = capture.borrow();
        assert_eq!(captured[0].0, "POST");
        assert_eq!(captured[0].1, "http://127.0.0.1:11434/v1/embeddings");
        assert_eq!(captured[0].4, Some(Duration::from_millis(900)));
        assert!(
            captured[0]
                .2
                .iter()
                .any(|(name, value)| { name == "Authorization" && value == "Bearer sk-secret" })
        );
        let body = json::parse(captured[0].3.as_deref().unwrap()).unwrap();
        assert_eq!(body.get("model").unwrap().as_str(), Some("embed-demo"));
        assert_eq!(body.get("dimensions").unwrap().as_u64(), Some(2));
        let input = body.get("input").unwrap().as_array().unwrap();
        assert_eq!(input.len(), 2);
        assert_eq!(input[0].as_str(), Some("alpha"));
    }

    #[test]
    fn rejects_embeddings_when_the_capability_is_disabled() {
        let mut capabilities = ProviderCapabilities::openai_compatible();
        capabilities.embeddings = false;
        let provider = make_provider(200, embedding_body()).with_capabilities(capabilities);
        let mut budget = Budget::default();
        let request = EmbeddingRequest::single("embed-demo", "alpha");
        let error = provider.embed(&request, &mut budget).unwrap_err();
        assert_eq!(error.code, AI_E_UNSUPPORTED);
    }

    #[test]
    fn rejects_malformed_embedding_responses() {
        let duplicate =
            r#"{"model":"m","data":[{"index":0,"embedding":[1.0]},{"index":0,"embedding":[2.0]}]}"#;
        let provider = make_provider(200, duplicate.to_string());
        let mut budget = Budget::default();
        let request = EmbeddingRequest::new("m", vec!["a".into(), "b".into()]);
        assert_eq!(
            provider.embed(&request, &mut budget).unwrap_err().code,
            AI_E_PROVIDER
        );

        let short = r#"{"model":"m","data":[{"index":0,"embedding":[1.0]}]}"#;
        let provider = make_provider(200, short.to_string());
        assert_eq!(
            provider.embed(&request, &mut budget).unwrap_err().code,
            AI_E_PROVIDER
        );

        let wrong_dimensions =
            r#"{"model":"m","data":[{"index":0,"embedding":[1.0]},{"index":1,"embedding":[1.0]}]}"#;
        let provider = make_provider(200, wrong_dimensions.to_string());
        let mut sized = request.clone();
        sized.dimensions = Some(2);
        assert_eq!(
            provider.embed(&sized, &mut budget).unwrap_err().code,
            AI_E_PROVIDER
        );
    }

    #[test]
    fn maps_embedding_http_errors_without_leaking_the_key() {
        let provider = make_provider(
            401,
            r#"{"error":{"message":"bad key sk-secret"}}"#.to_string(),
        );
        let mut budget = Budget::default();
        let request = EmbeddingRequest::single("m", "alpha");
        let error = provider.embed(&request, &mut budget).unwrap_err();
        assert_eq!(error.code, AI_E_AUTH);
        assert!(!error.message.contains("sk-secret"));
        assert!(error.message.contains("[REDACTED]"));
    }

    #[test]
    fn embedding_budget_is_checked_before_transport() {
        let (provider, capture) = provider_with_capture(200, embedding_body());
        let mut budget = Budget::default().with_tokens(1);
        let request = EmbeddingRequest::single("m", "a long embedding input");
        assert_eq!(
            provider.embed(&request, &mut budget).unwrap_err().code,
            AI_E_BUDGET
        );
        assert!(capture.borrow().is_empty());
    }

    #[test]
    fn joins_endpoint_variants() {
        assert_eq!(
            chat_url("http://localhost:1234/v1"),
            "http://localhost:1234/v1/chat/completions"
        );
        assert_eq!(
            chat_url("http://localhost:1234"),
            "http://localhost:1234/v1/chat/completions"
        );
        assert_eq!(
            models_url("http://localhost:1234/v1/chat/completions"),
            "http://localhost:1234/v1/models"
        );
        assert_eq!(
            embeddings_url("http://localhost:1234/v1"),
            "http://localhost:1234/v1/embeddings"
        );
        assert_eq!(
            embeddings_url("http://localhost:1234/v1/chat/completions"),
            "http://localhost:1234/v1/embeddings"
        );
    }
}
