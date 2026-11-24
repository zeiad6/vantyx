//! Deterministic providers for offline tests and CLI demos.

use super::budget::Budget;
use super::provider::{
    AI_E_PROVIDER, AiError, ChatRequest, ChatResponse, FinishReason, Message, Provider,
    ProviderCapabilities, Usage,
};
use super::tokens::{check_context_limit, estimate_request_tokens, estimate_tokens};
use std::cell::RefCell;
use std::collections::VecDeque;

/// Returns pre-scripted responses in order. An empty queue is an error, never a
/// silent empty answer.
pub struct MockProvider {
    id: String,
    capabilities: ProviderCapabilities,
    responses: RefCell<VecDeque<Result<ChatResponse, AiError>>>,
}

impl MockProvider {
    pub fn new(id: impl Into<String>, responses: Vec<Result<ChatResponse, AiError>>) -> Self {
        Self {
            id: id.into(),
            capabilities: ProviderCapabilities::mock(),
            responses: RefCell::new(responses.into()),
        }
    }

    pub fn from_texts(model: &str, texts: &[&str]) -> Self {
        let responses = texts
            .iter()
            .map(|text| Ok(ChatResponse::text(model, *text)))
            .collect();
        Self::new("mock", responses)
    }

    pub fn with_capabilities(mut self, capabilities: ProviderCapabilities) -> Self {
        self.capabilities = capabilities;
        self
    }

    pub fn remaining(&self) -> usize {
        self.responses.borrow().len()
    }
}

impl Provider for MockProvider {
    fn id(&self) -> &str {
        &self.id
    }

    fn capabilities(&self) -> ProviderCapabilities {
        self.capabilities
    }

    fn chat(&self, request: &ChatRequest, budget: &mut Budget) -> Result<ChatResponse, AiError> {
        request.validate()?;
        check_context_limit(request, self.capabilities.max_context_tokens)?;
        let estimate = estimate_request_tokens(request) as u64;
        budget.check_tokens(estimate)?;
        let response = self.responses.borrow_mut().pop_front().ok_or_else(|| {
            AiError::new(AI_E_PROVIDER, "mock provider has no scripted response")
        })??;
        let completion = if response.usage.completion_tokens == 0 {
            estimate_tokens(&response.message.text_content()) as u64
        } else {
            response.usage.completion_tokens
        };
        let usage = if response.usage.total_tokens == 0 {
            Usage::new(estimate, completion)
        } else {
            response.usage
        };
        budget.record_tokens(usage.total_tokens)?;
        Ok(ChatResponse { usage, ..response })
    }
}

/// Echoes the last user message. Useful for a zero-configuration CLI demo.
#[derive(Default)]
pub struct EchoProvider {
    capabilities: ProviderCapabilities,
}

impl EchoProvider {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Provider for EchoProvider {
    fn id(&self) -> &str {
        "echo"
    }

    fn capabilities(&self) -> ProviderCapabilities {
        self.capabilities
    }

    fn chat(&self, request: &ChatRequest, budget: &mut Budget) -> Result<ChatResponse, AiError> {
        request.validate()?;
        check_context_limit(request, self.capabilities.max_context_tokens)?;
        let text = request
            .messages
            .iter()
            .rev()
            .find(|message| message.role == super::provider::Role::User)
            .map(Message::text_content)
            .unwrap_or_default();
        let prompt = estimate_request_tokens(request) as u64;
        let completion = estimate_tokens(&text) as u64;
        budget.record_tokens(prompt + completion)?;
        Ok(ChatResponse {
            id: Some("echo-1".into()),
            model: request.model.clone(),
            message: Message::assistant(text),
            finish_reason: FinishReason::Stop,
            usage: Usage::new(prompt, completion),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_scripted_responses_in_order() {
        let provider = MockProvider::from_texts("test-model", &["one", "two"]);
        let mut budget = Budget::default();
        let request = ChatRequest::new("test-model", vec![Message::user("hi")]);
        assert_eq!(
            provider
                .chat(&request, &mut budget)
                .unwrap()
                .message
                .text_content(),
            "one"
        );
        assert_eq!(
            provider
                .chat(&request, &mut budget)
                .unwrap()
                .message
                .text_content(),
            "two"
        );
        assert_eq!(
            provider.chat(&request, &mut budget).unwrap_err().code,
            AI_E_PROVIDER
        );
    }

    #[test]
    fn echo_provider_returns_the_user_text() {
        let provider = EchoProvider::new();
        let mut budget = Budget::default();
        let request = ChatRequest::new("echo", vec![Message::user("hello")]);
        let response = provider.chat(&request, &mut budget).unwrap();
        assert_eq!(response.message.text_content(), "hello");
        assert!(budget.used_tokens() > 0);
    }

    #[test]
    fn mock_provider_respects_the_token_budget() {
        let provider = MockProvider::from_texts("test-model", &["answer"]);
        let mut budget = Budget::default().with_tokens(1);
        let request = ChatRequest::new("test-model", vec![Message::user("a long question")]);
        assert_eq!(
            provider.chat(&request, &mut budget).unwrap_err().code,
            super::super::provider::AI_E_BUDGET
        );
    }
}
