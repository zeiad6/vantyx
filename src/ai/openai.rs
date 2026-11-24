//! OpenAI-compatible chat and model-list adapter.
//!
//! The adapter speaks the `/v1/chat/completions` and `/v1/models` shapes used
//! by OpenAI, Ollama, llama.cpp, LM Studio, mistral.rs, and compatible cloud
//! gateways. It maps unsupported parameters to explicit errors rather than
//! dropping them.

use super::budget::Budget;
use super::http::{HttpResponse, Transport};
use super::json::{self, Json};
use super::provider::{
    AI_E_AUTH, AI_E_CONFIG, AI_E_JSON, AI_E_PROVIDER, AI_E_RATE_LIMIT, AiError, ChatRequest,
    ChatResponse, ContentPart, FinishReason, Message, Provider, ProviderCapabilities,
    ResponseFormat, Role, ToolCall, Usage,
};
use super::redact;
use super::tokens::{check_context_limit, estimate_request_tokens, estimate_tokens};
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
        let detail = redact::sanitize(&provider_detail(&response.body), &self.secrets(), 512);
        let suffix = if detail.is_empty() {
            String::new()
        } else {
            format!(": {detail}")
        };
        match response.status {
            401 | 403 => AiError::new(
                AI_E_AUTH,
                format!(
                    "provider authentication failed (HTTP {}){suffix}",
                    response.status
                ),
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
                format!("provider failed with HTTP {}{suffix}", response.status),
            ),
            status => AiError::new(
                AI_E_PROVIDER,
                format!("provider returned HTTP {status}{suffix}"),
            ),
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

impl Provider for OpenAiProvider {
    fn id(&self) -> &str {
        &self.id
    }

    fn capabilities(&self) -> ProviderCapabilities {
        self.capabilities
    }

    fn chat(&self, request: &ChatRequest, budget: &mut Budget) -> Result<ChatResponse, AiError> {
        request.validate()?;
        self.check_capabilities(request)?;
        check_context_limit(request, self.capabilities.max_context_tokens)?;
        let prompt_estimate = estimate_request_tokens(request) as u64;
        budget.check_tokens(prompt_estimate)?;

        let body = build_request_json(request).to_compact_string();
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

fn build_request_json(request: &ChatRequest) -> Json {
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
    }
}
