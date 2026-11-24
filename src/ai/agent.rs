//! A bounded agent loop. Every run has deterministic step, tool, token, and
//! wall-clock limits plus cooperative cancellation.

use super::budget::Budget;
use super::json;
use super::provider::{
    AI_E_CANCELLED, AiError, ChatRequest, ChatResponse, Message, Provider, Usage,
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
}

pub struct Agent {
    provider: Box<dyn Provider>,
    tools: ToolRegistry,
    config: AgentConfig,
    cancel: Option<Arc<AtomicBool>>,
}

impl Agent {
    pub fn new(provider: Box<dyn Provider>, tools: ToolRegistry, config: AgentConfig) -> Self {
        Self {
            provider,
            tools,
            config,
            cancel: None,
        }
    }

    pub fn with_cancel_flag(mut self, flag: Arc<AtomicBool>) -> Self {
        self.cancel = Some(flag);
        self
    }

    pub fn tools(&self) -> &ToolRegistry {
        &self.tools
    }

    pub fn run(&mut self, task: &str) -> Result<AgentResult, AiError> {
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
            self.check_cancel()?;
            budget.begin_step()?;
            let mut request = ChatRequest::new(&self.config.model, messages.clone());
            request.tools = self.tools.specs_for(&self.config.capabilities);
            request.max_tokens = self.config.max_completion_tokens;
            let response = self.provider.chat(&request, &mut budget)?;
            add_usage(&mut usage, response.usage);

            if response.message.tool_calls.is_empty() {
                return Ok(AgentResult {
                    text: response.message.text_content(),
                    steps: budget.steps(),
                    tool_calls: budget.tool_calls(),
                    usage,
                });
            }

            messages.push(response.message.clone());
            for call in &response.message.tool_calls {
                self.check_cancel()?;
                budget.record_tool_call()?;
                let arguments = match json::parse(&call.arguments) {
                    Ok(value) => value,
                    Err(error) => {
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
                let text = match result {
                    Ok(value) => value.to_compact_string(),
                    Err(error) => format!("error: {error}"),
                };
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
    use crate::ai::provider::{AI_E_CAPABILITY, AI_E_STEPS};
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
}
