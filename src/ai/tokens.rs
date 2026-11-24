//! Conservative token estimates.
//!
//! This is **not** a model tokenizer. It is a labelled estimate for budgets and
//! context guards. Exact counts require the model's tokenizer files and a
//! dependency decision (ADR-0015).

use super::provider::{AI_E_INVALID_REQUEST, AiError, ChatRequest, Message};

/// Approximate tokens for a string: roughly one token per four characters, plus
/// one token for a short or empty tail. It deliberately rounds up.
pub fn estimate_tokens(text: &str) -> usize {
    let characters = text.chars().count();
    if characters == 0 {
        0
    } else {
        characters.div_ceil(4) + 1
    }
}

pub fn estimate_message_tokens(message: &Message) -> usize {
    let content: usize = message
        .content
        .iter()
        .map(|part| match part {
            super::provider::ContentPart::Text(text) => estimate_tokens(text),
            _ => 256,
        })
        .sum();
    let calls: usize = message
        .tool_calls
        .iter()
        .map(|call| estimate_tokens(&call.name) + estimate_tokens(&call.arguments))
        .sum();
    4 + content + calls
}

pub fn estimate_request_tokens(request: &ChatRequest) -> usize {
    let messages: usize = request.messages.iter().map(estimate_message_tokens).sum();
    let tools: usize = request
        .tools
        .iter()
        .map(|tool| estimate_tokens(&tool.name) + estimate_tokens(&tool.description))
        .sum();
    messages + tools
}

/// Checks the estimate against a declared context window. This is a guard, not
/// a hard model limit.
pub fn check_context_limit(
    request: &ChatRequest,
    max_context_tokens: Option<usize>,
) -> Result<(), AiError> {
    let Some(limit) = max_context_tokens else {
        return Ok(());
    };
    let estimate = estimate_request_tokens(request);
    if estimate > limit {
        return Err(AiError::new(
            AI_E_INVALID_REQUEST,
            format!(
                "estimated request size {estimate} exceeds the declared context window {limit}; \
                 shorten the prompt or choose a larger model"
            ),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::provider::{ChatRequest, Message};

    #[test]
    fn estimates_empty_and_nonempty() {
        assert_eq!(estimate_tokens(""), 0);
        assert!(estimate_tokens("hello") >= 2);
        assert!(estimate_tokens("hello world") >= estimate_tokens("hello"));
    }

    #[test]
    fn estimates_requests_with_overhead() {
        let request = ChatRequest::new("m", vec![Message::system("rules"), Message::user("hi")]);
        let estimate = estimate_request_tokens(&request);
        assert!(estimate >= 8);
    }

    #[test]
    fn rejects_oversized_requests() {
        let request = ChatRequest::new("m", vec![Message::user("x".repeat(1000))]);
        assert_eq!(
            check_context_limit(&request, Some(10)).unwrap_err().code,
            AI_E_INVALID_REQUEST
        );
        assert!(check_context_limit(&request, None).is_ok());
    }
}
