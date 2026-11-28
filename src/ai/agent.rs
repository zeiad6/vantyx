//! A bounded agent loop. Every run has deterministic step, tool, token, and
//! wall-clock limits plus cooperative cancellation.

use super::budget::Budget;
use super::json;
use super::provider::{
    AI_E_CANCELLED, AI_E_JSON, AiError, ChatRequest, ChatResponse, FinishReason, Message, Provider,
    Usage,
};
use super::tools::{CapabilitySet, ToolContext, ToolRegistry};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct AgentConfig {
    pub model: String,
    pub system_prompt: String,
    pub max_steps: u32,
    pub max_tool_calls: u32,
    pub max_tokens: Option<u64>,
    pub max_wall_time: Option<Duration>,
    pub max_completion_tokens: Option<u32>,
    pub approved_tools: Vec<String>,
    pub capabilities: CapabilitySet,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            model: "mock".into(),
            system_prompt: "You are a bounded assistant. Use tools only when needed.".into(),
            max_steps: 8,
            max_tool_calls: 16,
            max_tokens: None,
            max_wall_time: None,
            max_completion_tokens: Some(1024),
            approved_tools: Vec::new(),
            capabilities: CapabilitySet::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AgentResult {
    pub text: String,
    pub steps: u32,
    pub tool_calls: u32,
    pub usage: Usage,
    /// Metadata-only trace of the run. It never contains prompts, tool
    /// arguments, tool results, or provider payloads.
    pub trace: Vec<AgentTraceEvent>,
}

/// One bounded, metadata-only record from an agent run.
///
/// A trace event carries a step number and one of: a provider response summary,
/// a tool name with an outcome code, or the terminal stop code. It is safe to
/// log or persist because it excludes message text, arguments, and results.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentTraceEvent {
    pub step: u32,
    pub kind: AgentTraceKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AgentTraceKind {
    Provider {
        tool_calls: u32,
        finish_reason: FinishReason,
    },
    Tool {
        name: String,
        outcome: AgentTraceOutcome,
    },
    Stopped {
        code: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AgentTraceOutcome {
    Ok,
    Error { code: String },
}

impl AgentTraceOutcome {
    fn error(code: &str) -> Self {
        Self::Error {
            code: bounded_label(code, TRACE_CODE_MAX_CHARS),
        }
    }
}

/// Maximum characters retained for a model- or provider-supplied trace label.
const TRACE_LABEL_MAX_CHARS: usize = 128;
/// Maximum characters retained for a trace error or stop code.
const TRACE_CODE_MAX_CHARS: usize = 64;

/// Truncates a metadata label so an adversarial provider cannot make a trace
/// grow with model-controlled text.
fn bounded_label(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    let mut bounded: String = value.chars().take(max_chars).collect();
    bounded.push('…');
    bounded
}

fn bounded_finish_reason(reason: &FinishReason) -> FinishReason {
    match reason {
        FinishReason::Other(value) => {
            FinishReason::Other(bounded_label(value, TRACE_LABEL_MAX_CHARS))
        }
        other => other.clone(),
    }
}

impl AgentTraceEvent {
    fn provider(step: u32, tool_calls: usize, finish_reason: FinishReason) -> Self {
        Self {
            step,
            kind: AgentTraceKind::Provider {
                tool_calls: u32::try_from(tool_calls).unwrap_or(u32::MAX),
                finish_reason: bounded_finish_reason(&finish_reason),
            },
        }
    }

    fn tool(step: u32, name: &str, outcome: AgentTraceOutcome) -> Self {
        Self {
            step,
            kind: AgentTraceKind::Tool {
                name: bounded_label(name, TRACE_LABEL_MAX_CHARS),
                outcome,
            },
        }
    }

    fn stopped(step: u32, code: &str) -> Self {
        Self {
            step,
            kind: AgentTraceKind::Stopped {
                code: bounded_label(code, TRACE_CODE_MAX_CHARS),
            },
        }
    }
}

pub struct Agent {
    provider: Box<dyn Provider>,
    tools: ToolRegistry,
    config: AgentConfig,
    cancel: Option<Arc<AtomicBool>>,
    trace: Vec<AgentTraceEvent>,
}

impl Agent {
    pub fn new(provider: Box<dyn Provider>, tools: ToolRegistry, config: AgentConfig) -> Self {
        Self {
            provider,
            tools,
            config,
            cancel: None,
            trace: Vec::new(),
        }
    }

    pub fn with_cancel_flag(mut self, flag: Arc<AtomicBool>) -> Self {
        self.cancel = Some(flag);
        self
    }

    pub fn tools(&self) -> &ToolRegistry {
        &self.tools
    }

    /// The trace of the most recent run, including a failed run. It is cleared
    /// at the start of each call to [`Agent::run`].
    pub fn last_trace(&self) -> &[AgentTraceEvent] {
        &self.trace
    }

    pub fn run(&mut self, task: &str) -> Result<AgentResult, AiError> {
        self.trace.clear();
        let mut budget = Budget::new(
            self.config.max_tokens,
            self.config.max_steps,
            self.config.max_tool_calls,
            self.config.max_wall_time,
        );
        let mut messages = vec![
            Message::system(self.config.system_prompt.clone()),
            Message::user(task),
        ];
        let mut usage = Usage::default();

        loop {
            if let Err(error) = self.check_cancel() {
                self.trace
                    .push(AgentTraceEvent::stopped(budget.steps(), &error.code));
                return Err(error);
            }
            if let Err(error) = budget.begin_step() {
                self.trace
                    .push(AgentTraceEvent::stopped(budget.steps(), &error.code));
                return Err(error);
            }
            let mut request = ChatRequest::new(&self.config.model, messages.clone());
            request.tools = self.tools.specs_for(&self.config.capabilities);
            request.max_tokens = self.config.max_completion_tokens;
            let response = match self.provider.chat(&request, &mut budget) {
                Ok(response) => response,
                Err(error) => {
                    self.trace
                        .push(AgentTraceEvent::stopped(budget.steps(), &error.code));
                    return Err(error);
                }
            };
            add_usage(&mut usage, response.usage);
            let step = budget.steps();
            self.trace.push(AgentTraceEvent::provider(
                step,
                response.message.tool_calls.len(),
                response.finish_reason.clone(),
            ));

            if response.message.tool_calls.is_empty() {
                return Ok(AgentResult {
                    text: response.message.text_content(),
                    steps: budget.steps(),
                    tool_calls: budget.tool_calls(),
                    usage,
                    trace: self.trace.clone(),
                });
            }

            messages.push(response.message.clone());
            for call in &response.message.tool_calls {
                if let Err(error) = self.check_cancel() {
                    self.trace
                        .push(AgentTraceEvent::stopped(budget.steps(), &error.code));
                    return Err(error);
                }
                if let Err(error) = budget.record_tool_call() {
                    self.trace
                        .push(AgentTraceEvent::stopped(budget.steps(), &error.code));
                    return Err(error);
                }
                let arguments = match json::parse(&call.arguments) {
                    Ok(value) => value,
                    Err(error) => {
                        self.trace.push(AgentTraceEvent::tool(
                            step,
                            &call.name,
                            AgentTraceOutcome::error(AI_E_JSON),
                        ));
                        messages.push(Message::tool(
                            call.id.clone(),
                            format!("error: invalid JSON arguments: {error}"),
                        ));
                        continue;
                    }
                };
                let context = ToolContext {
                    capabilities: &self.config.capabilities,
                    approved: &self.config.approved_tools,
                    deadline: budget.deadline(),
                };
                let result = self.tools.call(&call.name, &arguments, &context);
                let outcome = match &result {
                    Ok(_) => AgentTraceOutcome::Ok,
                    Err(error) => AgentTraceOutcome::error(&error.code),
                };
                let text = match result {
                    Ok(value) => value.to_compact_string(),
                    Err(error) => format!("error: {error}"),
                };
                self.trace
                    .push(AgentTraceEvent::tool(step, &call.name, outcome));
                messages.push(Message::tool(call.id.clone(), text));
            }
        }
    }

    fn check_cancel(&self) -> Result<(), AiError> {
        if self
            .cancel
            .as_ref()
            .is_some_and(|flag| flag.load(Ordering::Relaxed))
        {
            return Err(AiError::new(AI_E_CANCELLED, "agent run cancelled"));
        }
        Ok(())
    }
}

fn add_usage(total: &mut Usage, usage: Usage) {
    total.prompt_tokens = total.prompt_tokens.saturating_add(usage.prompt_tokens);
    total.completion_tokens = total
        .completion_tokens
        .saturating_add(usage.completion_tokens);
    total.total_tokens = total.total_tokens.saturating_add(usage.total_tokens);
}

/// Convenience constructor for a provider response that requests one tool call.
pub fn tool_call_response(model: &str, id: &str, name: &str, arguments: &str) -> ChatResponse {
    ChatResponse {
        id: None,
        model: model.into(),
        message: Message {
            role: super::provider::Role::Assistant,
            content: Vec::new(),
            name: None,
            tool_call_id: None,
            tool_calls: vec![super::provider::ToolCall {
                id: id.into(),
                name: name.into(),
                arguments: arguments.into(),
            }],
        },
        finish_reason: super::provider::FinishReason::ToolCalls,
        usage: Usage::new(5, 5),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::mock::MockProvider;
    use crate::ai::provider::{
        AI_E_CANCELLED, AI_E_CAPABILITY, AI_E_JSON, AI_E_STEPS, FinishReason, Role, ToolCall,
    };
    use crate::ai::tools::{ToolPolicy, WordCountTool};

    fn config() -> AgentConfig {
        AgentConfig {
            model: "mock".into(),
            max_steps: 4,
            ..AgentConfig::default()
        }
    }

    #[test]
    fn executes_a_tool_and_returns_the_final_answer() {
        let provider = MockProvider::new(
            "mock",
            vec![
                Ok(tool_call_response(
                    "mock",
                    "1",
                    "word_count",
                    r#"{"text":"one two"}"#,
                )),
                Ok(ChatResponse::text("mock", "done")),
            ],
        );
        let mut tools = ToolRegistry::new();
        tools
            .register(Box::new(WordCountTool), ToolPolicy::auto(2))
            .unwrap();
        let mut agent = Agent::new(Box::new(provider), tools, config());
        let result = agent.run("count the words").unwrap();
        assert_eq!(result.text, "done");
        assert_eq!(result.steps, 2);
        assert_eq!(result.tool_calls, 1);
        assert_eq!(agent.tools().calls("word_count"), 1);
    }

    #[test]
    fn denied_tools_do_not_execute_and_the_agent_can_recover() {
        let provider = MockProvider::new(
            "mock",
            vec![
                Ok(tool_call_response(
                    "mock",
                    "1",
                    "read_text_file",
                    r#"{"path":"/etc/passwd"}"#,
                )),
                Ok(ChatResponse::text("mock", "recovered")),
            ],
        );
        let tools = ToolRegistry::new();
        let mut agent = Agent::new(Box::new(provider), tools, config());
        let result = agent.run("read a file").unwrap();
        assert_eq!(result.text, "recovered");
        assert_eq!(result.tool_calls, 1);
    }

    #[test]
    fn enforces_the_step_limit() {
        let provider = MockProvider::new(
            "mock",
            vec![
                Ok(tool_call_response(
                    "mock",
                    "1",
                    "word_count",
                    r#"{"text":"a"}"#,
                )),
                Ok(tool_call_response(
                    "mock",
                    "2",
                    "word_count",
                    r#"{"text":"b"}"#,
                )),
            ],
        );
        let mut tools = ToolRegistry::new();
        tools
            .register(Box::new(WordCountTool), ToolPolicy::auto(4))
            .unwrap();
        let mut config = config();
        config.max_steps = 1;
        let mut agent = Agent::new(Box::new(provider), tools, config);
        assert_eq!(agent.run("loop").unwrap_err().code, AI_E_STEPS);
    }

    #[test]
    fn honours_the_cancellation_flag() {
        let provider = MockProvider::from_texts("mock", &["never"]);
        let tools = ToolRegistry::new();
        let flag = Arc::new(AtomicBool::new(true));
        let mut agent = Agent::new(Box::new(provider), tools, config()).with_cancel_flag(flag);
        assert_eq!(agent.run("stop").unwrap_err().code, AI_E_CANCELLED);
    }

    #[test]
    fn reports_capability_errors_as_tool_messages() {
        let provider = MockProvider::new(
            "mock",
            vec![
                Ok(tool_call_response("mock", "1", "missing_tool", "{}")),
                Ok(ChatResponse::text("mock", "ok")),
            ],
        );
        let tools = ToolRegistry::new();
        let mut agent = Agent::new(Box::new(provider), tools, config());
        let result = agent.run("call a missing tool").unwrap();
        assert_eq!(result.text, "ok");
        assert_eq!(result.tool_calls, 1);
        assert_eq!(AI_E_CAPABILITY, "AI_E_CAPABILITY");
    }

    #[test]
    fn records_a_metadata_only_trace() {
        let provider = MockProvider::new(
            "mock",
            vec![
                Ok(tool_call_response(
                    "mock",
                    "1",
                    "word_count",
                    r#"{"text":"one two"}"#,
                )),
                Ok(ChatResponse::text("mock", "done")),
            ],
        );
        let mut tools = ToolRegistry::new();
        tools
            .register(Box::new(WordCountTool), ToolPolicy::auto(2))
            .unwrap();
        let mut agent = Agent::new(Box::new(provider), tools, config());
        let result = agent.run("count the words").unwrap();
        assert_eq!(
            result.trace,
            vec![
                AgentTraceEvent {
                    step: 1,
                    kind: AgentTraceKind::Provider {
                        tool_calls: 1,
                        finish_reason: FinishReason::ToolCalls,
                    },
                },
                AgentTraceEvent {
                    step: 1,
                    kind: AgentTraceKind::Tool {
                        name: "word_count".into(),
                        outcome: AgentTraceOutcome::Ok,
                    },
                },
                AgentTraceEvent {
                    step: 2,
                    kind: AgentTraceKind::Provider {
                        tool_calls: 0,
                        finish_reason: FinishReason::Stop,
                    },
                },
            ]
        );
        assert_eq!(agent.last_trace(), result.trace.as_slice());
    }

    #[test]
    fn traces_denied_tools_by_error_code() {
        let provider = MockProvider::new(
            "mock",
            vec![
                Ok(tool_call_response(
                    "mock",
                    "1",
                    "read_text_file",
                    r#"{"path":"/etc/passwd"}"#,
                )),
                Ok(ChatResponse::text("mock", "recovered")),
            ],
        );
        let tools = ToolRegistry::new();
        let mut agent = Agent::new(Box::new(provider), tools, config());
        let result = agent.run("read a file").unwrap();
        assert_eq!(
            result.trace[1],
            AgentTraceEvent {
                step: 1,
                kind: AgentTraceKind::Tool {
                    name: "read_text_file".into(),
                    outcome: AgentTraceOutcome::Error {
                        code: AI_E_CAPABILITY.into(),
                    },
                },
            }
        );
    }

    #[test]
    fn traces_invalid_tool_arguments_without_copying_them() {
        let provider = MockProvider::new(
            "mock",
            vec![
                Ok(tool_call_response(
                    "mock",
                    "1",
                    "word_count",
                    "secret-not-json",
                )),
                Ok(ChatResponse::text("mock", "ok")),
            ],
        );
        let tools = ToolRegistry::new();
        let mut agent = Agent::new(Box::new(provider), tools, config());
        let result = agent.run("bad arguments").unwrap();
        assert_eq!(
            result.trace[1],
            AgentTraceEvent {
                step: 1,
                kind: AgentTraceKind::Tool {
                    name: "word_count".into(),
                    outcome: AgentTraceOutcome::Error {
                        code: AI_E_JSON.into(),
                    },
                },
            }
        );
        assert!(!format!("{:?}", result.trace).contains("secret-not-json"));
    }

    #[test]
    fn traces_the_terminal_limit_error() {
        let provider = MockProvider::new(
            "mock",
            vec![
                Ok(tool_call_response(
                    "mock",
                    "1",
                    "word_count",
                    r#"{"text":"a"}"#,
                )),
                Ok(tool_call_response(
                    "mock",
                    "2",
                    "word_count",
                    r#"{"text":"b"}"#,
                )),
            ],
        );
        let mut tools = ToolRegistry::new();
        tools
            .register(Box::new(WordCountTool), ToolPolicy::auto(4))
            .unwrap();
        let mut config = config();
        config.max_steps = 1;
        let mut agent = Agent::new(Box::new(provider), tools, config);
        assert_eq!(agent.run("loop").unwrap_err().code, AI_E_STEPS);
        assert_eq!(
            agent.last_trace().last(),
            Some(&AgentTraceEvent {
                step: 1,
                kind: AgentTraceKind::Stopped {
                    code: AI_E_STEPS.into(),
                },
            })
        );
    }

    #[test]
    fn traces_initial_cancellation() {
        let provider = MockProvider::from_texts("mock", &["never"]);
        let tools = ToolRegistry::new();
        let flag = Arc::new(AtomicBool::new(true));
        let mut agent = Agent::new(Box::new(provider), tools, config()).with_cancel_flag(flag);
        assert_eq!(agent.run("stop").unwrap_err().code, AI_E_CANCELLED);
        assert_eq!(
            agent.last_trace(),
            &[AgentTraceEvent {
                step: 0,
                kind: AgentTraceKind::Stopped {
                    code: AI_E_CANCELLED.into(),
                },
            }]
        );
    }

    #[test]
    fn bounds_model_controlled_trace_labels() {
        let long_name = "é".repeat(300);
        let long_reason = "r".repeat(300);
        let response = ChatResponse {
            id: None,
            model: "mock".into(),
            message: Message {
                role: Role::Assistant,
                content: Vec::new(),
                name: None,
                tool_call_id: None,
                tool_calls: vec![ToolCall {
                    id: "1".into(),
                    name: long_name.clone(),
                    arguments: "{}".into(),
                }],
            },
            finish_reason: FinishReason::Other(long_reason),
            usage: Usage::default(),
        };
        let provider = MockProvider::new(
            "mock",
            vec![Ok(response), Ok(ChatResponse::text("mock", "done"))],
        );
        let tools = ToolRegistry::new();
        let mut agent = Agent::new(Box::new(provider), tools, config());
        let result = agent.run("bounded trace").unwrap();

        match &result.trace[0].kind {
            AgentTraceKind::Provider {
                finish_reason: FinishReason::Other(value),
                ..
            } => {
                assert_eq!(value.chars().count(), TRACE_LABEL_MAX_CHARS + 1);
                assert!(value.ends_with('…'));
            }
            other => panic!("unexpected provider trace: {other:?}"),
        }
        match &result.trace[1].kind {
            AgentTraceKind::Tool { name, .. } => {
                assert_eq!(name.chars().count(), TRACE_LABEL_MAX_CHARS + 1);
                assert!(name.ends_with('…'));
            }
            other => panic!("unexpected tool trace: {other:?}"),
        }
        assert!(!format!("{:?}", result.trace).contains(&long_name));
        assert!(!format!("{:?}", result.trace).contains(&"r".repeat(200)));
    }
}
