//! Bounded retries for provider calls.
//!
//! A retry reissues the same validated request. It never bypasses the network
//! policy, a budget, or a capability check. Only errors explicitly marked
//! [`AiError::retryable`] are retried; authentication, configuration,
//! validation, capability, schema, budget, and cancellation errors return
//! immediately.
//!
//! Streaming calls are retried only before the first text delta. Once output
//! has been delivered, retrying would duplicate it, so the error is returned
//! to the caller instead.
//!
//! `Retry-After` is honored in its integer-seconds form. HTTP-date values are
//! ignored rather than guessed, and an oversized delay stops automatic retries
//! instead of retrying earlier than the server requested.

use super::budget::Budget;
use super::provider::{
    AI_E_CANCELLED, AiError, ChatRequest, ChatResponse, EmbeddingRequest, EmbeddingResponse,
    Provider, ProviderCapabilities,
};
use std::cell::Cell;
use std::thread;
use std::time::Duration;

/// Default number of automatic retries after the initial attempt.
pub const DEFAULT_MAX_RETRIES: u32 = 2;
/// Hard ceiling for a caller-configured retry count.
pub const MAX_RETRIES: u32 = 8;
/// Default first backoff delay.
pub const DEFAULT_BASE_DELAY_MS: u64 = 250;
/// Default ceiling for one automatic retry delay.
pub const DEFAULT_MAX_DELAY_MS: u64 = 4_000;
/// Longest `Retry-After` value accepted from a provider.
pub const MAX_RETRY_AFTER: Duration = Duration::from_secs(60);

/// A deterministic, bounded retry policy.
///
/// `max_retries` counts retries after the first attempt, so a value of `2`
/// permits at most three attempts. Backoff is exponential with a hard ceiling.
/// A provider `Retry-After` value is honored only when it does not exceed
/// `max_delay`; a larger value stops automatic retries rather than retrying
/// earlier than the server requested.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RetryPolicy {
    pub max_retries: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: DEFAULT_MAX_RETRIES,
            base_delay: Duration::from_millis(DEFAULT_BASE_DELAY_MS),
            max_delay: Duration::from_millis(DEFAULT_MAX_DELAY_MS),
        }
    }
}

impl RetryPolicy {
    /// Creates a policy with the default delays and a clamped retry count.
    pub fn new(max_retries: u32) -> Self {
        Self {
            max_retries: max_retries.min(MAX_RETRIES),
            ..Self::default()
        }
    }

    /// A policy that never retries.
    pub fn none() -> Self {
        Self {
            max_retries: 0,
            ..Self::default()
        }
    }

    /// Replaces the backoff delays. `max_delay` is raised to `base_delay` when
    /// the caller supplies a smaller ceiling.
    pub fn with_delays(mut self, base_delay: Duration, max_delay: Duration) -> Self {
        self.base_delay = base_delay;
        self.max_delay = max_delay.max(base_delay);
        self
    }

    /// Total attempts permitted by this policy, including the first one.
    pub fn max_attempts(&self) -> u32 {
        self.max_retries.saturating_add(1)
    }

    /// Whether another attempt is allowed. `retry_number` starts at 1 for the
    /// first retry.
    pub fn should_retry(&self, retry_number: u32, error: &AiError) -> bool {
        retry_number > 0
            && retry_number <= self.max_retries
            && error.retryable
            && error.code != AI_E_CANCELLED
            && error
                .retry_after
                .is_none_or(|delay| delay <= self.max_delay)
    }

    /// Delay before `retry_number`. A provider `Retry-After` overrides the
    /// exponential backoff but is still capped by `max_delay`.
    pub fn delay(&self, retry_number: u32, error: &AiError) -> Duration {
        if let Some(retry_after) = error.retry_after {
            return retry_after.min(self.max_delay);
        }
        let exponent = retry_number.saturating_sub(1).min(16);
        self.base_delay
            .checked_mul(1_u32 << exponent)
            .unwrap_or(self.max_delay)
            .min(self.max_delay)
    }
}

/// Runs `operation` with bounded retries and returns the number of retries that
/// occurred. The injected `sleeper` keeps tests deterministic and free of real
/// waits.
pub fn retry_with_count<T>(
    policy: &RetryPolicy,
    mut operation: impl FnMut() -> Result<T, AiError>,
    mut sleeper: impl FnMut(Duration),
) -> (Result<T, AiError>, u32) {
    let mut retries = 0;
    loop {
        match operation() {
            Ok(value) => return (Ok(value), retries),
            Err(error) => {
                if !policy.should_retry(retries.saturating_add(1), &error) {
                    return (Err(error), retries);
                }
                retries += 1;
                let delay = policy.delay(retries, &error);
                if !delay.is_zero() {
                    sleeper(delay);
                }
            }
        }
    }
}

/// Runs `operation` with bounded retries and returns only the result.
pub fn retry_call<T>(
    policy: &RetryPolicy,
    operation: impl FnMut() -> Result<T, AiError>,
) -> Result<T, AiError> {
    retry_call_count(policy, operation).0
}

/// Runs `operation` with bounded retries and returns the result plus the retry
/// count.
pub fn retry_call_count<T>(
    policy: &RetryPolicy,
    operation: impl FnMut() -> Result<T, AiError>,
) -> (Result<T, AiError>, u32) {
    retry_with_count(policy, operation, thread::sleep)
}

/// Parses the integer-seconds form of `Retry-After`. HTTP-date values and
/// malformed values are ignored rather than guessed. The result is capped at
/// [`MAX_RETRY_AFTER`].
pub fn retry_after_from_header(value: Option<&str>) -> Option<Duration> {
    let seconds = value?.trim().parse::<u64>().ok()?;
    Some(Duration::from_secs(seconds.min(MAX_RETRY_AFTER.as_secs())))
}

/// A provider decorator that applies a [`RetryPolicy`] to chat, streaming,
/// model-list, and embedding calls.
///
/// The decorator is provider-neutral. It does not inspect payloads and does not
/// change a request between attempts. `last_retry_count` reports the retries
/// performed by the most recent call so metrics can record them without
/// capturing prompts or responses.
pub struct RetryingProvider {
    inner: Box<dyn Provider>,
    policy: RetryPolicy,
    last_retries: Cell<u32>,
}

impl RetryingProvider {
    pub fn new(inner: Box<dyn Provider>, policy: RetryPolicy) -> Self {
        Self {
            inner,
            policy,
            last_retries: Cell::new(0),
        }
    }

    pub fn policy(&self) -> RetryPolicy {
        self.policy
    }

    /// Runs one operation with bounded retries and a wall-clock budget check
    /// before and after each backoff. The operation receives the budget so the
    /// borrow is not split between the closure and the loop.
    fn run<T>(
        &self,
        budget: &mut Budget,
        mut operation: impl FnMut(&mut Budget) -> Result<T, AiError>,
    ) -> (Result<T, AiError>, u32) {
        let mut retries = 0;
        loop {
            match operation(budget) {
                Ok(value) => return (Ok(value), retries),
                Err(error) => {
                    if !self.policy.should_retry(retries.saturating_add(1), &error) {
                        return (Err(error), retries);
                    }
                    if let Err(budget_error) = budget.check_time() {
                        return (Err(budget_error), retries);
                    }
                    retries += 1;
                    let delay = self.policy.delay(retries, &error);
                    if !delay.is_zero() {
                        thread::sleep(delay);
                    }
                    if let Err(budget_error) = budget.check_time() {
                        return (Err(budget_error), retries);
                    }
                }
            }
        }
    }
}

impl Provider for RetryingProvider {
    fn id(&self) -> &str {
        self.inner.id()
    }

    fn capabilities(&self) -> ProviderCapabilities {
        self.inner.capabilities()
    }

    fn models(&self) -> Result<Vec<String>, AiError> {
        let (result, retries) =
            retry_with_count(&self.policy, || self.inner.models(), thread::sleep);
        self.last_retries.set(retries);
        result
    }

    fn last_retry_count(&self) -> u32 {
        self.last_retries.get()
    }

    fn chat(&self, request: &ChatRequest, budget: &mut Budget) -> Result<ChatResponse, AiError> {
        let (result, retries) = self.run(budget, |budget| self.inner.chat(request, budget));
        self.last_retries.set(retries);
        result
    }

    fn chat_stream(
        &self,
        request: &ChatRequest,
        budget: &mut Budget,
        on_delta: &mut dyn FnMut(&str),
    ) -> Result<ChatResponse, AiError> {
        let mut retries = 0;
        let mut emitted = false;
        loop {
            let result = {
                let mut attempt_delta = |delta: &str| {
                    emitted = true;
                    on_delta(delta);
                };
                self.inner.chat_stream(request, budget, &mut attempt_delta)
            };
            match result {
                Ok(response) => {
                    self.last_retries.set(retries);
                    return Ok(response);
                }
                Err(error) => {
                    if emitted || !self.policy.should_retry(retries + 1, &error) {
                        self.last_retries.set(retries);
                        return Err(error);
                    }
                    if let Err(budget_error) = budget.check_time() {
                        self.last_retries.set(retries);
                        return Err(budget_error);
                    }
                    retries += 1;
                    let delay = self.policy.delay(retries, &error);
                    if !delay.is_zero() {
                        thread::sleep(delay);
                    }
                    if let Err(budget_error) = budget.check_time() {
                        self.last_retries.set(retries);
                        return Err(budget_error);
                    }
                }
            }
        }
    }

    fn embed(
        &self,
        request: &EmbeddingRequest,
        budget: &mut Budget,
    ) -> Result<EmbeddingResponse, AiError> {
        let (result, retries) = self.run(budget, |budget| self.inner.embed(request, budget));
        self.last_retries.set(retries);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::embedding::Vector;
    use crate::ai::provider::{
        AI_E_AUTH, AI_E_BUDGET, AI_E_INVALID_REQUEST, AI_E_PROVIDER, EmbeddingUsage, Message,
    };
    use std::cell::RefCell;
    use std::collections::VecDeque;

    fn fast_policy(max_retries: u32) -> RetryPolicy {
        RetryPolicy::new(max_retries).with_delays(Duration::ZERO, Duration::from_secs(1))
    }

    fn retryable() -> AiError {
        AiError::retryable(AI_E_PROVIDER, "temporary provider failure")
    }

    enum StreamScript {
        Ok(Vec<&'static str>),
        Fail(AiError, Vec<&'static str>),
    }

    struct StubProvider {
        chat: RefCell<VecDeque<Result<ChatResponse, AiError>>>,
        stream: RefCell<VecDeque<StreamScript>>,
        embed: RefCell<VecDeque<Result<EmbeddingResponse, AiError>>>,
        models: RefCell<VecDeque<Result<Vec<String>, AiError>>>,
        chat_calls: Cell<u32>,
        stream_calls: Cell<u32>,
        embed_calls: Cell<u32>,
        models_calls: Cell<u32>,
    }

    impl StubProvider {
        fn new(
            chat: Vec<Result<ChatResponse, AiError>>,
            stream: Vec<StreamScript>,
            embed: Vec<Result<EmbeddingResponse, AiError>>,
        ) -> Self {
            Self {
                chat: RefCell::new(chat.into()),
                stream: RefCell::new(stream.into()),
                embed: RefCell::new(embed.into()),
                models: RefCell::new(VecDeque::new()),
                chat_calls: Cell::new(0),
                stream_calls: Cell::new(0),
                embed_calls: Cell::new(0),
                models_calls: Cell::new(0),
            }
        }

        fn with_models(mut self, models: Vec<Result<Vec<String>, AiError>>) -> Self {
            self.models = RefCell::new(models.into());
            self
        }
    }

    impl Provider for StubProvider {
        fn id(&self) -> &str {
            "stub"
        }

        fn capabilities(&self) -> ProviderCapabilities {
            ProviderCapabilities::mock()
        }

        fn models(&self) -> Result<Vec<String>, AiError> {
            self.models_calls.set(self.models_calls.get() + 1);
            self.models
                .borrow_mut()
                .pop_front()
                .unwrap_or_else(|| Err(AiError::new(AI_E_PROVIDER, "no scripted model list")))
        }

        fn chat(
            &self,
            _request: &ChatRequest,
            _budget: &mut Budget,
        ) -> Result<ChatResponse, AiError> {
            self.chat_calls.set(self.chat_calls.get() + 1);
            self.chat
                .borrow_mut()
                .pop_front()
                .unwrap_or_else(|| Err(AiError::new(AI_E_PROVIDER, "no scripted chat response")))
        }

        fn chat_stream(
            &self,
            _request: &ChatRequest,
            _budget: &mut Budget,
            on_delta: &mut dyn FnMut(&str),
        ) -> Result<ChatResponse, AiError> {
            self.stream_calls.set(self.stream_calls.get() + 1);
            match self
                .stream
                .borrow_mut()
                .pop_front()
                .unwrap_or(StreamScript::Fail(
                    AiError::new(AI_E_PROVIDER, "no scripted stream"),
                    Vec::new(),
                )) {
                StreamScript::Ok(deltas) => {
                    for delta in deltas {
                        on_delta(delta);
                    }
                    Ok(ChatResponse::text("stub", "complete"))
                }
                StreamScript::Fail(error, deltas) => {
                    for delta in deltas {
                        on_delta(delta);
                    }
                    Err(error)
                }
            }
        }

        fn embed(
            &self,
            _request: &EmbeddingRequest,
            _budget: &mut Budget,
        ) -> Result<EmbeddingResponse, AiError> {
            self.embed_calls.set(self.embed_calls.get() + 1);
            self.embed
                .borrow_mut()
                .pop_front()
                .unwrap_or_else(|| Err(AiError::new(AI_E_PROVIDER, "no scripted embedding")))
        }
    }

    fn embedding() -> EmbeddingResponse {
        EmbeddingResponse {
            model: "stub".into(),
            vectors: vec![Vector::new(vec![1.0, 0.0]).unwrap()],
            usage: EmbeddingUsage::new(1),
        }
    }

    #[test]
    fn retries_retryable_errors_until_success() {
        let provider = RetryingProvider::new(
            Box::new(StubProvider::new(
                vec![Err(retryable()), Ok(ChatResponse::text("stub", "ok"))],
                Vec::new(),
                Vec::new(),
            )),
            fast_policy(1),
        );
        let request = ChatRequest::new("stub", vec![Message::user("hi")]);
        let mut budget = Budget::default();
        let response = provider.chat(&request, &mut budget).unwrap();
        assert_eq!(response.message.text_content(), "ok");
        assert_eq!(provider.last_retry_count(), 1);
    }

    #[test]
    fn does_not_retry_non_retryable_errors() {
        let provider = RetryingProvider::new(
            Box::new(StubProvider::new(
                vec![Err(AiError::new(AI_E_AUTH, "denied"))],
                Vec::new(),
                Vec::new(),
            )),
            fast_policy(3),
        );
        let request = ChatRequest::new("stub", vec![Message::user("hi")]);
        let mut budget = Budget::default();
        let error = provider.chat(&request, &mut budget).unwrap_err();
        assert_eq!(error.code, AI_E_AUTH);
        assert_eq!(provider.last_retry_count(), 0);
    }

    #[test]
    fn stops_at_the_retry_limit() {
        let provider = RetryingProvider::new(
            Box::new(StubProvider::new(
                vec![Err(retryable()), Err(retryable()), Err(retryable())],
                Vec::new(),
                Vec::new(),
            )),
            fast_policy(1),
        );
        let request = ChatRequest::new("stub", vec![Message::user("hi")]);
        let mut budget = Budget::default();
        let error = provider.chat(&request, &mut budget).unwrap_err();
        assert_eq!(error.code, AI_E_PROVIDER);
        assert_eq!(provider.last_retry_count(), 1);
    }

    #[test]
    fn a_none_policy_never_retries() {
        let provider = RetryingProvider::new(
            Box::new(StubProvider::new(
                vec![Err(retryable()), Ok(ChatResponse::text("stub", "ok"))],
                Vec::new(),
                Vec::new(),
            )),
            RetryPolicy::none(),
        );
        let request = ChatRequest::new("stub", vec![Message::user("hi")]);
        let mut budget = Budget::default();
        assert!(provider.chat(&request, &mut budget).is_err());
        assert_eq!(provider.last_retry_count(), 0);
    }

    #[test]
    fn honours_retry_after_and_rejects_an_oversized_value() {
        let mut error = retryable().with_retry_after(Some(Duration::from_millis(500)));
        let policy = fast_policy(1);
        assert!(policy.should_retry(1, &error));
        assert_eq!(policy.delay(1, &error), Duration::from_millis(500));

        error = retryable().with_retry_after(Some(Duration::from_secs(30)));
        assert!(!policy.should_retry(1, &error));

        let mut delays = Vec::new();
        let (result, retries) = retry_with_count(
            &fast_policy(2),
            {
                let mut calls = 0;
                move || {
                    calls += 1;
                    if calls == 1 {
                        Err(retryable().with_retry_after(Some(Duration::from_millis(250))))
                    } else {
                        Ok("done")
                    }
                }
            },
            |delay| delays.push(delay),
        );
        assert_eq!(result.unwrap(), "done");
        assert_eq!(retries, 1);
        assert_eq!(delays, vec![Duration::from_millis(250)]);
    }

    #[test]
    fn exponential_backoff_is_capped() {
        let policy =
            RetryPolicy::new(4).with_delays(Duration::from_millis(100), Duration::from_millis(250));
        let error = retryable();
        assert_eq!(policy.delay(1, &error), Duration::from_millis(100));
        assert_eq!(policy.delay(2, &error), Duration::from_millis(200));
        assert_eq!(policy.delay(3, &error), Duration::from_millis(250));
        assert_eq!(policy.delay(4, &error), Duration::from_millis(250));
    }

    #[test]
    fn cancellation_is_never_retried() {
        let policy = fast_policy(3);
        let error = AiError::retryable(AI_E_CANCELLED, "cancelled");
        assert!(!policy.should_retry(1, &error));
    }

    #[test]
    fn parses_retry_after_seconds_and_ignores_other_forms() {
        assert_eq!(
            retry_after_from_header(Some("2")),
            Some(Duration::from_secs(2))
        );
        assert_eq!(retry_after_from_header(Some(" 0 ")), Some(Duration::ZERO));
        assert_eq!(
            retry_after_from_header(Some("999999")),
            Some(MAX_RETRY_AFTER)
        );
        assert_eq!(
            retry_after_from_header(Some("Wed, 21 Oct 2015 07:28:00 GMT")),
            None
        );
        assert_eq!(retry_after_from_header(Some("-1")), None);
        assert_eq!(retry_after_from_header(None), None);
    }

    #[test]
    fn streaming_retries_before_the_first_delta() {
        let provider = RetryingProvider::new(
            Box::new(StubProvider::new(
                Vec::new(),
                vec![
                    StreamScript::Fail(retryable(), Vec::new()),
                    StreamScript::Ok(vec!["hello"]),
                ],
                Vec::new(),
            )),
            fast_policy(1),
        );
        let request = ChatRequest::new("stub", vec![Message::user("hi")]);
        let mut budget = Budget::default();
        let mut text = String::new();
        let response = provider
            .chat_stream(&request, &mut budget, &mut |delta| text.push_str(delta))
            .unwrap();
        assert_eq!(response.message.text_content(), "complete");
        assert_eq!(text, "hello");
        assert_eq!(provider.last_retry_count(), 1);
    }

    #[test]
    fn streaming_does_not_retry_after_output() {
        let provider = RetryingProvider::new(
            Box::new(StubProvider::new(
                Vec::new(),
                vec![
                    StreamScript::Fail(retryable(), vec!["partial"]),
                    StreamScript::Ok(vec!["never"]),
                ],
                Vec::new(),
            )),
            fast_policy(2),
        );
        let request = ChatRequest::new("stub", vec![Message::user("hi")]);
        let mut budget = Budget::default();
        let mut text = String::new();
        let error = provider
            .chat_stream(&request, &mut budget, &mut |delta| text.push_str(delta))
            .unwrap_err();
        assert_eq!(error.code, AI_E_PROVIDER);
        assert_eq!(text, "partial");
        assert_eq!(provider.last_retry_count(), 0);
    }

    #[test]
    fn embeddings_are_retried() {
        let provider = RetryingProvider::new(
            Box::new(StubProvider::new(
                Vec::new(),
                Vec::new(),
                vec![Err(retryable()), Ok(embedding())],
            )),
            fast_policy(1),
        );
        let request = EmbeddingRequest::single("stub", "hello");
        let mut budget = Budget::default();
        let response = provider.embed(&request, &mut budget).unwrap();
        assert_eq!(response.dimensions(), Some(2));
        assert_eq!(provider.last_retry_count(), 1);
    }

    #[test]
    fn model_lists_are_retried() {
        let provider = RetryingProvider::new(
            Box::new(
                StubProvider::new(Vec::new(), Vec::new(), Vec::new())
                    .with_models(vec![Err(retryable()), Ok(vec!["model-a".into()])]),
            ),
            fast_policy(1),
        );
        let models = provider.models().unwrap();
        assert_eq!(models, vec!["model-a".to_string()]);
        assert_eq!(provider.last_retry_count(), 1);
    }

    #[test]
    fn invalid_requests_are_not_retried() {
        let provider = RetryingProvider::new(
            Box::new(StubProvider::new(
                vec![Err(AiError::new(AI_E_INVALID_REQUEST, "bad"))],
                Vec::new(),
                Vec::new(),
            )),
            fast_policy(3),
        );
        let request = ChatRequest::new("stub", vec![Message::user("hi")]);
        let mut budget = Budget::default();
        let error = provider.chat(&request, &mut budget).unwrap_err();
        assert_eq!(error.code, AI_E_INVALID_REQUEST);
        assert_eq!(provider.last_retry_count(), 0);
    }

    #[test]
    fn an_exhausted_wall_clock_budget_stops_retries() {
        let provider = RetryingProvider::new(
            Box::new(StubProvider::new(
                vec![Err(retryable()), Ok(ChatResponse::text("stub", "ok"))],
                Vec::new(),
                Vec::new(),
            )),
            fast_policy(1),
        );
        let request = ChatRequest::new("stub", vec![Message::user("hi")]);
        let mut budget = Budget::default().with_wall_time(Duration::ZERO);
        let error = provider.chat(&request, &mut budget).unwrap_err();
        assert_eq!(error.code, AI_E_BUDGET);
        assert_eq!(provider.last_retry_count(), 0);
    }
}
