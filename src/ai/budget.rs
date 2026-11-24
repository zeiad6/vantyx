//! Deterministic budgets for AI calls and agent loops.

use super::provider::AiError;
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
pub struct Budget {
    max_tokens: Option<u64>,
    max_steps: u32,
    max_tool_calls: u32,
    max_wall_time: Option<Duration>,
    started: Instant,
    used_tokens: u64,
    steps: u32,
    tool_calls: u32,
}

impl Default for Budget {
    fn default() -> Self {
        Self {
            max_tokens: None,
            max_steps: 8,
            max_tool_calls: 16,
            max_wall_time: None,
            started: Instant::now(),
            used_tokens: 0,
            steps: 0,
            tool_calls: 0,
        }
    }
}

impl Budget {
    pub fn new(
        max_tokens: Option<u64>,
        max_steps: u32,
        max_tool_calls: u32,
        max_wall_time: Option<Duration>,
    ) -> Self {
        Self {
            max_tokens,
            max_steps,
            max_tool_calls,
            max_wall_time,
            started: Instant::now(),
            used_tokens: 0,
            steps: 0,
            tool_calls: 0,
        }
    }

    pub fn with_tokens(mut self, max_tokens: u64) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    pub fn with_steps(mut self, max_steps: u32) -> Self {
        self.max_steps = max_steps;
        self
    }

    pub fn with_tool_calls(mut self, max_tool_calls: u32) -> Self {
        self.max_tool_calls = max_tool_calls;
        self
    }

    pub fn with_wall_time(mut self, max_wall_time: Duration) -> Self {
        self.max_wall_time = Some(max_wall_time);
        self
    }

    pub fn check_time(&self) -> Result<(), AiError> {
        if let Some(limit) = self.max_wall_time
            && self.started.elapsed() >= limit
        {
            return Err(AiError::budget(format!(
                "wall-clock budget of {} ms exhausted",
                limit.as_millis()
            )));
        }
        Ok(())
    }

    pub fn check_tokens(&self, additional: u64) -> Result<(), AiError> {
        if let Some(limit) = self.max_tokens {
            let total = self.used_tokens.saturating_add(additional);
            if total > limit {
                return Err(AiError::budget(format!(
                    "token budget exhausted: {} used, {} requested, {} allowed",
                    self.used_tokens, additional, limit
                )));
            }
        }
        Ok(())
    }

    pub fn record_tokens(&mut self, tokens: u64) -> Result<(), AiError> {
        self.check_tokens(tokens)?;
        self.used_tokens = self.used_tokens.saturating_add(tokens);
        Ok(())
    }

    pub fn begin_step(&mut self) -> Result<(), AiError> {
        self.check_time()?;
        if self.steps >= self.max_steps {
            return Err(AiError::new(
                super::provider::AI_E_STEPS,
                format!("agent step limit of {} reached", self.max_steps),
            ));
        }
        self.steps += 1;
        Ok(())
    }

    pub fn record_tool_call(&mut self) -> Result<(), AiError> {
        self.check_time()?;
        if self.tool_calls >= self.max_tool_calls {
            return Err(AiError::budget(format!(
                "tool-call limit of {} reached",
                self.max_tool_calls
            )));
        }
        self.tool_calls += 1;
        Ok(())
    }

    pub fn remaining_tokens(&self) -> Option<u64> {
        self.max_tokens
            .map(|limit| limit.saturating_sub(self.used_tokens))
    }

    pub fn used_tokens(&self) -> u64 {
        self.used_tokens
    }

    pub fn steps(&self) -> u32 {
        self.steps
    }

    pub fn tool_calls(&self) -> u32 {
        self.tool_calls
    }

    pub fn elapsed(&self) -> Duration {
        self.started.elapsed()
    }

    pub fn deadline(&self) -> Option<Instant> {
        self.max_wall_time.map(|duration| self.started + duration)
    }

    pub fn max_steps(&self) -> u32 {
        self.max_steps
    }

    pub fn max_tool_calls(&self) -> u32 {
        self.max_tool_calls
    }

    pub fn summary(&self) -> String {
        let tokens = self.max_tokens.map_or_else(
            || format!("{} used", self.used_tokens),
            |limit| format!("{}/{}", self.used_tokens, limit),
        );
        format!(
            "steps={}/{}, tool_calls={}/{}, tokens={}, elapsed_ms={}",
            self.steps,
            self.max_steps,
            self.tool_calls,
            self.max_tool_calls,
            tokens,
            self.started.elapsed().as_millis()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::provider::AI_E_BUDGET;

    #[test]
    fn rejects_over_budget_tokens() {
        let mut budget = Budget::new(Some(10), 2, 2, None);
        budget.record_tokens(7).unwrap();
        assert_eq!(budget.remaining_tokens(), Some(3));
        assert_eq!(budget.record_tokens(4).unwrap_err().code, AI_E_BUDGET);
    }

    #[test]
    fn enforces_step_and_tool_limits() {
        let mut budget = Budget::new(None, 1, 1, None);
        budget.begin_step().unwrap();
        assert_eq!(
            budget.begin_step().unwrap_err().code,
            super::super::provider::AI_E_STEPS
        );
        budget.record_tool_call().unwrap();
        assert_eq!(budget.record_tool_call().unwrap_err().code, AI_E_BUDGET);
    }

    #[test]
    fn enforces_wall_time() {
        let budget = Budget::new(None, 1, 1, Some(Duration::ZERO));
        assert_eq!(budget.check_time().unwrap_err().code, AI_E_BUDGET);
    }
}
