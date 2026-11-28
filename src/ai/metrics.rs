//! Opt-in, redacted JSONL metrics.
//!
//! Prompts, responses, tool arguments, and secrets are never part of a metric
//! event. A caller that needs payload tracing must implement a separate,
//! explicit mechanism and warn the user.

use super::json::Json;
use super::provider::{AI_E_IO, AiError};
use std::fs::{OpenOptions, create_dir_all};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, PartialEq)]
pub struct MetricEvent {
    pub provider: String,
    pub model: String,
    pub operation: String,
    pub ok: bool,
    pub latency_ms: u64,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    pub finish_reason: Option<String>,
    pub error_code: Option<String>,
    pub retry_count: u32,
    pub tool: Option<String>,
}

impl MetricEvent {
    pub fn new(
        provider: impl Into<String>,
        model: impl Into<String>,
        operation: impl Into<String>,
    ) -> Self {
        Self {
            provider: provider.into(),
            model: model.into(),
            operation: operation.into(),
            ok: false,
            latency_ms: 0,
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
            finish_reason: None,
            error_code: None,
            retry_count: 0,
            tool: None,
        }
    }

    pub fn success(mut self, latency_ms: u64) -> Self {
        self.ok = true;
        self.latency_ms = latency_ms;
        self
    }

    pub fn failure(mut self, latency_ms: u64, error_code: impl Into<String>) -> Self {
        self.ok = false;
        self.latency_ms = latency_ms;
        self.error_code = Some(error_code.into());
        self
    }

    pub fn usage(mut self, prompt_tokens: u64, completion_tokens: u64, total_tokens: u64) -> Self {
        self.prompt_tokens = prompt_tokens;
        self.completion_tokens = completion_tokens;
        self.total_tokens = total_tokens;
        self
    }

    pub fn finish_reason(mut self, finish_reason: impl Into<String>) -> Self {
        self.finish_reason = Some(finish_reason.into());
        self
    }

    pub fn tool(mut self, tool: impl Into<String>) -> Self {
        self.tool = Some(tool.into());
        self
    }

    /// Records how many automatic retries the operation performed. This is a
    /// count only; prompts and responses remain excluded.
    pub fn retries(mut self, retry_count: u32) -> Self {
        self.retry_count = retry_count;
        self
    }

    pub fn to_json(&self) -> Json {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis() as u64)
            .unwrap_or(0);
        let mut entries = vec![
            ("timestamp_ms", Json::from(timestamp)),
            ("provider", Json::from(self.provider.clone())),
            ("model", Json::from(self.model.clone())),
            ("operation", Json::from(self.operation.clone())),
            ("ok", Json::from(self.ok)),
            ("latency_ms", Json::from(self.latency_ms)),
            ("prompt_tokens", Json::from(self.prompt_tokens)),
            ("completion_tokens", Json::from(self.completion_tokens)),
            ("total_tokens", Json::from(self.total_tokens)),
            ("retry_count", Json::from(u64::from(self.retry_count))),
        ];
        if let Some(finish_reason) = &self.finish_reason {
            entries.push(("finish_reason", Json::from(finish_reason.clone())));
        }
        if let Some(error_code) = &self.error_code {
            entries.push(("error_code", Json::from(error_code.clone())));
        }
        if let Some(tool) = &self.tool {
            entries.push(("tool", Json::from(tool.clone())));
        }
        Json::object(entries)
    }
}

#[derive(Clone, Debug, Default)]
pub struct MetricsSink {
    path: Option<PathBuf>,
}

impl MetricsSink {
    pub fn disabled() -> Self {
        Self::default()
    }

    pub fn file(path: impl Into<PathBuf>) -> Self {
        Self {
            path: Some(path.into()),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.path.is_some()
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Appends one JSON object. The file is created owner-only on Unix.
    pub fn record(&self, event: &MetricEvent) -> Result<(), AiError> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            create_dir_all(parent).map_err(|error| {
                AiError::new(
                    AI_E_IO,
                    format!(
                        "cannot create metrics directory `{}`: {error}",
                        parent.display()
                    ),
                )
            })?;
        }
        let mut options = OpenOptions::new();
        options.create(true).append(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(path).map_err(|error| {
            AiError::new(
                AI_E_IO,
                format!("cannot open metrics file `{}`: {error}", path.display()),
            )
        })?;
        writeln!(file, "{}", event.to_json().to_compact_string()).map_err(|error| {
            AiError::new(
                AI_E_IO,
                format!("cannot write metrics file `{}`: {error}", path.display()),
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::json::parse;

    fn temp_path(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "vantyx-metrics-{}-{stamp}-{name}.jsonl",
            std::process::id()
        ))
    }

    #[test]
    fn disabled_sink_writes_nothing() {
        let sink = MetricsSink::disabled();
        assert!(!sink.is_enabled());
        sink.record(&MetricEvent::new("mock", "m", "chat")).unwrap();
    }

    #[test]
    fn writes_redacted_jsonl_events() {
        let path = temp_path("events");
        let sink = MetricsSink::file(&path);
        let event = MetricEvent::new("openai", "model", "chat")
            .success(42)
            .usage(10, 5, 15)
            .finish_reason("stop");
        sink.record(&event).unwrap();
        sink.record(&MetricEvent::new("openai", "model", "chat").failure(3, "AI_E_TIMEOUT"))
            .unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2);
        let first = parse(lines[0]).unwrap();
        assert_eq!(first.get("ok").unwrap().as_bool(), Some(true));
        assert_eq!(first.get("total_tokens").unwrap().as_u64(), Some(15));
        assert!(!text.contains("prompt text"));
        assert!(!text.contains("sk-"));
        let second = parse(lines[1]).unwrap();
        assert_eq!(
            second.get("error_code").unwrap().as_str(),
            Some("AI_E_TIMEOUT")
        );
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn event_json_has_no_payload_fields() {
        let event = MetricEvent::new("p", "m", "chat").tool("word_count");
        let json = event.to_json();
        assert!(json.get("prompt").is_none());
        assert!(json.get("response").is_none());
        assert_eq!(json.get("tool").unwrap().as_str(), Some("word_count"));
    }
}
