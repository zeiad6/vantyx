//! Validated structured extraction from untrusted model output.
//!
//! The model never decides whether its own output is valid. Every response is
//! parsed as strict JSON and validated against a caller-supplied JSON Schema
//! subset before it is returned. Invalid output triggers a bounded repair loop;
//! the loop always has a hard attempt limit and consumes the shared budget.

use super::budget::Budget;
use super::json::Json;
use super::provider::{
    AI_E_CONFIG, AI_E_INVALID_REQUEST, AI_E_JSON, AI_E_RESPONSE_TOO_LARGE, AI_E_SCHEMA, AiError,
    ChatRequest, FinishReason, Message, Provider, ResponseFormat, Usage,
};
use super::schema;

/// Maximum serialized schema size accepted by [`extract`].
pub const MAX_SCHEMA_BYTES: usize = 1024 * 1024;

/// Maximum size of one model response considered for extraction.
pub const MAX_EXTRACTION_BYTES: usize = 1024 * 1024;

/// Hard ceiling on repair attempts. Callers may request fewer, never more.
pub const MAX_REPAIR_ATTEMPTS: usize = 3;

/// The validated result of a structured extraction.
#[derive(Clone, Debug, PartialEq)]
pub struct Extraction {
    pub value: Json,
    /// Total provider calls, including the initial call and repairs.
    pub attempts: u32,
    pub model: String,
    pub finish_reason: FinishReason,
    /// Usage accumulated across every attempt.
    pub usage: Usage,
}

impl Extraction {
    /// Serializes the extraction envelope for CLI and test output.
    pub fn to_json(&self) -> Json {
        Json::object([
            ("value", self.value.clone()),
            ("attempts", Json::from(u64::from(self.attempts))),
            ("model", Json::from(self.model.clone())),
            (
                "finish_reason",
                Json::from(self.finish_reason.as_str().to_string()),
            ),
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
        ])
    }
}

/// Requests a JSON value that conforms to `schema`, repairing invalid output up
/// to `max_repair_attempts` times.
///
/// The provider must advertise `json_schema` or `json_object` support. A
/// provider without either capability is rejected instead of silently sending a
/// format it may ignore.
pub fn extract(
    provider: &dyn Provider,
    mut request: ChatRequest,
    schema_value: &Json,
    budget: &mut Budget,
    max_repair_attempts: usize,
) -> Result<Extraction, AiError> {
    if !matches!(schema_value, Json::Object(_)) {
        return Err(AiError::new(
            AI_E_INVALID_REQUEST,
            "extraction schema must be a JSON object",
        ));
    }
    schema::check_schema(schema_value).map_err(|error| {
        AiError::new(
            AI_E_INVALID_REQUEST,
            format!("extraction schema is invalid: {error}"),
        )
    })?;
    let schema_text = schema_value.to_compact_string();
    if schema_text.len() > MAX_SCHEMA_BYTES {
        return Err(AiError::new(
            AI_E_RESPONSE_TOO_LARGE,
            format!(
                "extraction schema is {} bytes; the limit is {MAX_SCHEMA_BYTES}",
                schema_text.len()
            ),
        ));
    }
    if max_repair_attempts > MAX_REPAIR_ATTEMPTS {
        return Err(AiError::new(
            AI_E_CONFIG,
            format!("at most {MAX_REPAIR_ATTEMPTS} repair attempts are supported"),
        ));
    }

    let capabilities = provider.capabilities();
    request.response_format = Some(if capabilities.json_schema {
        ResponseFormat::JsonSchema {
            name: "vantyx_extraction".to_string(),
            schema: schema_value.clone(),
        }
    } else if capabilities.json_object {
        ResponseFormat::JsonObject
    } else {
        return Err(AiError::unsupported("response_format", provider.id()));
    });
    request.validate()?;

    let mut usage = Usage::default();
    let mut last_error = None;
    for attempt in 0..=max_repair_attempts {
        budget.check_time()?;
        let response = provider.chat(&request, budget)?;
        usage.prompt_tokens = usage
            .prompt_tokens
            .saturating_add(response.usage.prompt_tokens);
        usage.completion_tokens = usage
            .completion_tokens
            .saturating_add(response.usage.completion_tokens);
        usage.total_tokens = usage
            .total_tokens
            .saturating_add(response.usage.total_tokens);

        let text = response.message.text_content();
        if text.len() > MAX_EXTRACTION_BYTES {
            return Err(AiError::new(
                AI_E_RESPONSE_TOO_LARGE,
                format!(
                    "model response is {} bytes; the extraction limit is {MAX_EXTRACTION_BYTES}",
                    text.len()
                ),
            ));
        }
        let candidate = strip_code_fence(&text);
        let parsed = match super::json::parse(candidate) {
            Ok(value) => value,
            Err(error) => {
                last_error = Some(AiError::new(
                    AI_E_JSON,
                    format!("model output is not valid JSON: {error}"),
                ));
                if attempt >= max_repair_attempts {
                    break;
                }
                push_repair(
                    &mut request,
                    &text,
                    &format!("the response was not valid JSON: {error}"),
                    &schema_text,
                );
                continue;
            }
        };
        match schema::validate(schema_value, &parsed) {
            Ok(()) => {
                return Ok(Extraction {
                    value: parsed,
                    attempts: attempt as u32 + 1,
                    model: response.model,
                    finish_reason: response.finish_reason,
                    usage,
                });
            }
            Err(error) => {
                last_error = Some(AiError::new(
                    AI_E_SCHEMA,
                    format!("model output failed schema validation: {error}"),
                ));
                if attempt >= max_repair_attempts {
                    break;
                }
                push_repair(
                    &mut request,
                    &text,
                    &format!("the response failed schema validation: {error}"),
                    &schema_text,
                );
            }
        }
    }

    let error = last_error.unwrap_or_else(|| {
        AiError::new(
            AI_E_SCHEMA,
            "model output was not valid after the configured attempts",
        )
    });
    Err(AiError::new(
        error.code,
        format!(
            "{} (after {} attempt{})",
            error.message,
            max_repair_attempts + 1,
            if max_repair_attempts == 0 { "" } else { "s" }
        ),
    ))
}

fn push_repair(request: &mut ChatRequest, invalid_text: &str, reason: &str, schema: &str) {
    // Keep the repair context bounded. The model sees enough of its previous
    // output to correct it, but a pathological response cannot grow the prompt
    // without limit.
    const MAX_INVALID_TEXT: usize = 8 * 1024;
    let mut invalid = invalid_text.to_string();
    if invalid.len() > MAX_INVALID_TEXT {
        invalid.truncate(MAX_INVALID_TEXT);
        invalid.push_str("\n[truncated]");
    }
    request.messages.push(Message::assistant(invalid));
    request.messages.push(Message::user(format!(
        "The previous response was rejected because {reason}. Return only a JSON value \
         that conforms to this schema:\n{schema}\nDo not include prose, markdown fences, \
         or explanations."
    )));
}

/// Removes one surrounding Markdown code fence. Strict JSON validation still
/// applies to the contents; this is a convenience for models that wrap JSON in
/// a fence despite the instruction not to.
fn strip_code_fence(text: &str) -> &str {
    let trimmed = text.trim();
    let Some(rest) = trimmed.strip_prefix("```") else {
        return trimmed;
    };
    let Some(newline) = rest.find('\n') else {
        return trimmed;
    };
    let body = &rest[newline + 1..];
    let Some(end) = body.rfind("```") else {
        return trimmed;
    };
    body[..end].trim()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::mock::MockProvider;
    use crate::ai::provider::{ChatResponse, ProviderCapabilities};

    fn schema() -> Json {
        super::super::json::parse(
            r#"{"type":"object","required":["name"],"properties":{"name":{"type":"string"},"age":{"type":"integer","minimum":0}},"additionalProperties":false}"#,
        )
        .unwrap()
    }

    fn request() -> ChatRequest {
        ChatRequest::new("mock-model", vec![Message::user("extract a person")])
    }

    #[test]
    fn accepts_valid_json_on_the_first_attempt() {
        let provider = MockProvider::from_texts("mock-model", &[r#"{"name":"Ada","age":36}"#]);
        let mut budget = Budget::default();
        let result = extract(&provider, request(), &schema(), &mut budget, 2).unwrap();
        assert_eq!(result.value.get("name").unwrap().as_str(), Some("Ada"));
        assert_eq!(result.attempts, 1);
        assert_eq!(provider.remaining(), 0);
    }

    #[test]
    fn repairs_invalid_json_within_the_limit() {
        let provider = MockProvider::from_texts("mock-model", &["not json", r#"{"name":"Ada"}"#]);
        let mut budget = Budget::default();
        let result = extract(&provider, request(), &schema(), &mut budget, 2).unwrap();
        assert_eq!(result.value.get("name").unwrap().as_str(), Some("Ada"));
        assert_eq!(result.attempts, 2);
        assert_eq!(provider.remaining(), 0);
    }

    #[test]
    fn repairs_schema_mismatches_within_the_limit() {
        let provider =
            MockProvider::from_texts("mock-model", &[r#"{"name":42}"#, r#"{"name":"Ada"}"#]);
        let mut budget = Budget::default();
        let result = extract(&provider, request(), &schema(), &mut budget, 2).unwrap();
        assert_eq!(result.attempts, 2);
    }

    #[test]
    fn stops_after_the_repair_limit() {
        let provider =
            MockProvider::from_texts("mock-model", &["not json", "still not json", "nope"]);
        let mut budget = Budget::default();
        let error = extract(&provider, request(), &schema(), &mut budget, 2).unwrap_err();
        assert_eq!(error.code, AI_E_JSON);
        assert!(error.message.contains("after 3 attempts"));
        assert_eq!(provider.remaining(), 0);
    }

    #[test]
    fn accepts_a_single_markdown_fence_but_still_validates() {
        let provider =
            MockProvider::from_texts("mock-model", &["```json\n{\"name\":\"Ada\"}\n```"]);
        let mut budget = Budget::default();
        let result = extract(&provider, request(), &schema(), &mut budget, 0).unwrap();
        assert_eq!(result.value.get("name").unwrap().as_str(), Some("Ada"));
    }

    #[test]
    fn rejects_providers_without_a_json_mode() {
        let provider = MockProvider::from_texts("mock-model", &[r#"{"name":"Ada"}"#])
            .with_capabilities(ProviderCapabilities {
                json_object: false,
                json_schema: false,
                ..ProviderCapabilities::default()
            });
        let mut budget = Budget::default();
        let error = extract(&provider, request(), &schema(), &mut budget, 1).unwrap_err();
        assert_eq!(error.code, crate::ai::provider::AI_E_UNSUPPORTED);
    }

    #[test]
    fn rejects_excessive_repair_limits() {
        let provider = MockProvider::from_texts("mock-model", &[r#"{"name":"Ada"}"#]);
        let mut budget = Budget::default();
        let error = extract(
            &provider,
            request(),
            &schema(),
            &mut budget,
            MAX_REPAIR_ATTEMPTS + 1,
        )
        .unwrap_err();
        assert_eq!(error.code, AI_E_CONFIG);
    }

    #[test]
    fn rejects_non_object_schemas() {
        let provider = MockProvider::from_texts("mock-model", &[r#"{"name":"Ada"}"#]);
        let mut budget = Budget::default();
        let error = extract(&provider, request(), &Json::from("nope"), &mut budget, 1).unwrap_err();
        assert_eq!(error.code, AI_E_INVALID_REQUEST);
    }

    #[test]
    fn strips_only_a_complete_fence() {
        assert_eq!(strip_code_fence("```json\n{\"a\":1}\n```"), "{\"a\":1}");
        assert_eq!(strip_code_fence("{\"a\":1}"), "{\"a\":1}");
        assert_eq!(strip_code_fence("```json\n{\"a\":1}"), "```json\n{\"a\":1}");
    }

    #[test]
    fn accumulates_usage_across_attempts() {
        let first = ChatResponse {
            usage: Usage::new(10, 2),
            ..ChatResponse::text("mock-model", "not json")
        };
        let second = ChatResponse {
            usage: Usage::new(12, 4),
            ..ChatResponse::text("mock-model", r#"{"name":"Ada"}"#)
        };
        let provider = MockProvider::new("mock", vec![Ok(first), Ok(second)]);
        let mut budget = Budget::default();
        let result = extract(&provider, request(), &schema(), &mut budget, 1).unwrap();
        assert_eq!(result.usage.prompt_tokens, 22);
        assert_eq!(result.usage.completion_tokens, 6);
        assert_eq!(result.usage.total_tokens, 28);
    }
}
