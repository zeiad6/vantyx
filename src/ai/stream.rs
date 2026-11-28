//! Provider-neutral streaming events and bounded tool-call assembly.
//!
//! Streaming providers deliver a tool call in fragments: an index, sometimes
//! an id and name, then a sequence of JSON argument fragments. This module
//! accumulates those fragments under hard byte and count limits and only
//! produces a [`ToolCall`] when the stream ends with complete JSON arguments.
//! Nothing here executes a tool; callers must still validate arguments against
//! the tool schema and the caller's capabilities.

use super::json;
use super::provider::{AI_E_PROVIDER, AiError, ToolCall};

/// Maximum number of distinct tool calls in one streamed response.
pub const MAX_STREAM_TOOL_CALLS: usize = 8;
/// Maximum bytes in one streamed tool-call id.
pub const MAX_STREAM_TOOL_ID_BYTES: usize = 256;
/// Maximum bytes in one streamed tool name.
pub const MAX_STREAM_TOOL_NAME_BYTES: usize = 128;
/// Maximum bytes in one streamed tool-call argument object.
pub const MAX_STREAM_TOOL_ARGUMENT_BYTES: usize = 64 * 1024;
/// Maximum argument bytes across every tool call in one streamed response.
pub const MAX_STREAM_TOOL_ARGUMENTS_TOTAL_BYTES: usize = 256 * 1024;
/// Maximum number of tool-call delta frames in one streamed response.
pub const MAX_STREAM_TOOL_DELTAS: usize = 4096;

/// One provider-neutral streaming event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StreamEvent {
    /// A text fragment in source order.
    TextDelta(String),
    /// A tool-call fragment. Arguments are appended by index.
    ToolCallDelta(ToolCallDelta),
}

/// One tool-call fragment from a provider stream.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolCallDelta {
    /// Zero-based call position within the response.
    pub index: usize,
    /// Call id, normally present on the first fragment only.
    pub id: Option<String>,
    /// Function name, normally present on the first fragment only.
    pub name: Option<String>,
    /// JSON argument fragment; may be empty.
    pub arguments: String,
}

#[derive(Default)]
struct PendingCall {
    id: String,
    name: String,
    arguments: String,
}

/// Accumulates streamed tool-call fragments with hard limits.
///
/// The assembler rejects, rather than truncates: an over-limit or inconsistent
/// stream is a provider protocol error, and no partial call is returned.
#[derive(Default)]
pub struct ToolCallAssembler {
    calls: Vec<Option<PendingCall>>,
    argument_bytes: usize,
    deltas: usize,
}

impl ToolCallAssembler {
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of distinct calls seen so far.
    pub fn call_count(&self) -> usize {
        self.calls.iter().filter(|call| call.is_some()).count()
    }

    /// Total argument bytes accepted so far.
    pub fn argument_bytes(&self) -> usize {
        self.argument_bytes
    }

    /// Adds one fragment. Invalid or over-limit fragments are rejected before
    /// they can be observed by a caller.
    pub fn push(&mut self, delta: ToolCallDelta) -> Result<(), AiError> {
        self.deltas = self.deltas.saturating_add(1);
        if self.deltas > MAX_STREAM_TOOL_DELTAS {
            return Err(protocol_error(format!(
                "streamed tool calls exceed the {MAX_STREAM_TOOL_DELTAS} delta limit"
            )));
        }
        if delta.index >= MAX_STREAM_TOOL_CALLS {
            return Err(protocol_error(format!(
                "streamed tool call index {} exceeds the limit of {MAX_STREAM_TOOL_CALLS}",
                delta.index
            )));
        }
        if self.calls.len() <= delta.index {
            self.calls.resize_with(delta.index + 1, || None);
        }
        let slot = &mut self.calls[delta.index];
        if slot.is_none() {
            *slot = Some(PendingCall::default());
        }
        let call = slot.as_mut().expect("pending call was initialized");
        if let Some(id) = delta.id.filter(|id| !id.is_empty()) {
            set_identity_field("id", &mut call.id, id, MAX_STREAM_TOOL_ID_BYTES)?;
        }
        if let Some(name) = delta.name.filter(|name| !name.is_empty()) {
            set_identity_field("name", &mut call.name, name, MAX_STREAM_TOOL_NAME_BYTES)?;
        }
        if !delta.arguments.is_empty() {
            let call_bytes = call.arguments.len().saturating_add(delta.arguments.len());
            if call_bytes > MAX_STREAM_TOOL_ARGUMENT_BYTES {
                return Err(protocol_error(format!(
                    "streamed tool call {} arguments exceed the {MAX_STREAM_TOOL_ARGUMENT_BYTES} byte limit",
                    delta.index
                )));
            }
            let total_bytes = self.argument_bytes.saturating_add(delta.arguments.len());
            if total_bytes > MAX_STREAM_TOOL_ARGUMENTS_TOTAL_BYTES {
                return Err(protocol_error(format!(
                    "streamed tool call arguments exceed the {MAX_STREAM_TOOL_ARGUMENTS_TOTAL_BYTES} byte total limit"
                )));
            }
            call.arguments.push_str(&delta.arguments);
            self.argument_bytes = total_bytes;
        }
        Ok(())
    }

    /// Finishes assembly. Every call must have an id, a name, and complete
    /// JSON arguments; ids must be unique. Calls are returned by index.
    pub fn finish(self) -> Result<Vec<ToolCall>, AiError> {
        let mut calls = Vec::new();
        let mut ids: Vec<String> = Vec::new();
        for (index, slot) in self.calls.into_iter().enumerate() {
            let Some(call) = slot else {
                continue;
            };
            if call.id.is_empty() {
                return Err(protocol_error(format!(
                    "streamed tool call {index} is missing an id"
                )));
            }
            if call.name.is_empty() {
                return Err(protocol_error(format!(
                    "streamed tool call {index} is missing a name"
                )));
            }
            if ids.contains(&call.id) {
                return Err(protocol_error(format!(
                    "streamed tool call {index} repeats an earlier id"
                )));
            }
            ids.push(call.id.clone());
            if json::parse(&call.arguments).is_err() {
                return Err(protocol_error(format!(
                    "streamed tool call {index} has incomplete or invalid JSON arguments"
                )));
            }
            calls.push(ToolCall {
                id: call.id,
                name: call.name,
                arguments: call.arguments,
            });
        }
        Ok(calls)
    }
}

fn set_identity_field(
    field: &str,
    slot: &mut String,
    value: String,
    limit: usize,
) -> Result<(), AiError> {
    if value.len() > limit {
        return Err(protocol_error(format!(
            "streamed tool call {field} exceeds the {limit} byte limit"
        )));
    }
    if slot.is_empty() {
        *slot = value;
        return Ok(());
    }
    if *slot != value {
        return Err(protocol_error(format!(
            "streamed tool call {field} changed within one stream"
        )));
    }
    Ok(())
}

fn protocol_error(message: impl Into<String>) -> AiError {
    AiError::new(AI_E_PROVIDER, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn delta(index: usize, id: Option<&str>, name: Option<&str>, arguments: &str) -> ToolCallDelta {
        ToolCallDelta {
            index,
            id: id.map(ToString::to_string),
            name: name.map(ToString::to_string),
            arguments: arguments.to_string(),
        }
    }

    #[test]
    fn assembles_fragmented_arguments() {
        let mut assembler = ToolCallAssembler::new();
        assembler
            .push(delta(0, Some("call-1"), Some("lookup"), ""))
            .unwrap();
        assembler.push(delta(0, None, None, "{\"q\":")).unwrap();
        assembler.push(delta(0, None, None, "\"rust\"}")).unwrap();
        let calls = assembler.finish().unwrap();
        assert_eq!(
            calls,
            vec![ToolCall {
                id: "call-1".into(),
                name: "lookup".into(),
                arguments: "{\"q\":\"rust\"}".into(),
            }]
        );
    }

    #[test]
    fn assembles_interleaved_calls_in_index_order() {
        let mut assembler = ToolCallAssembler::new();
        assembler
            .push(delta(1, Some("call-b"), Some("second"), "{\"n\":2}"))
            .unwrap();
        assembler
            .push(delta(0, Some("call-a"), Some("first"), "{\"n\":"))
            .unwrap();
        assembler.push(delta(0, None, None, "1}")).unwrap();
        let calls = assembler.finish().unwrap();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].name, "first");
        assert_eq!(calls[0].arguments, "{\"n\":1}");
        assert_eq!(calls[1].name, "second");
        assert_eq!(calls[1].arguments, "{\"n\":2}");
    }

    #[test]
    fn accepts_repeated_identical_identity_fields() {
        let mut assembler = ToolCallAssembler::new();
        assembler
            .push(delta(0, Some("call-1"), Some("lookup"), ""))
            .unwrap();
        assembler
            .push(delta(0, Some("call-1"), Some("lookup"), "{}"))
            .unwrap();
        assert_eq!(assembler.finish().unwrap()[0].id, "call-1");
    }

    #[test]
    fn rejects_conflicting_identity_fields() {
        let mut assembler = ToolCallAssembler::new();
        assembler
            .push(delta(0, Some("call-1"), Some("lookup"), ""))
            .unwrap();
        let error = assembler
            .push(delta(0, Some("call-2"), None, ""))
            .unwrap_err();
        assert_eq!(error.code, AI_E_PROVIDER);
        assert!(error.message.contains("id changed"));

        let mut assembler = ToolCallAssembler::new();
        assembler
            .push(delta(0, Some("call-1"), Some("lookup"), ""))
            .unwrap();
        let error = assembler
            .push(delta(0, None, Some("other"), ""))
            .unwrap_err();
        assert!(error.message.contains("name changed"));
    }

    #[test]
    fn rejects_out_of_range_indexes_and_too_many_calls() {
        let mut assembler = ToolCallAssembler::new();
        let error = assembler
            .push(delta(
                MAX_STREAM_TOOL_CALLS,
                Some("call"),
                Some("tool"),
                "{}",
            ))
            .unwrap_err();
        assert!(error.message.contains("index"));
    }

    #[test]
    fn rejects_oversized_arguments_per_call_and_in_total() {
        let mut assembler = ToolCallAssembler::new();
        let oversized = "x".repeat(MAX_STREAM_TOOL_ARGUMENT_BYTES + 1);
        let error = assembler
            .push(delta(0, Some("call-1"), Some("tool"), &oversized))
            .unwrap_err();
        assert!(error.message.contains("byte limit"));

        let mut assembler = ToolCallAssembler::new();
        let chunk = "x".repeat(60 * 1024);
        let mut calls = 0;
        let error = loop {
            let result = assembler.push(delta(
                calls,
                Some(&format!("call-{calls}")),
                Some("tool"),
                &chunk,
            ));
            calls += 1;
            if let Err(error) = result {
                break error;
            }
            assert!(calls <= MAX_STREAM_TOOL_CALLS);
        };
        assert!(error.message.contains("total limit"));
    }

    #[test]
    fn rejects_oversized_identity_fields() {
        let mut assembler = ToolCallAssembler::new();
        let id = "x".repeat(MAX_STREAM_TOOL_ID_BYTES + 1);
        let error = assembler
            .push(delta(0, Some(&id), Some("tool"), "{}"))
            .unwrap_err();
        assert!(error.message.contains("id"));

        let mut assembler = ToolCallAssembler::new();
        let name = "x".repeat(MAX_STREAM_TOOL_NAME_BYTES + 1);
        let error = assembler
            .push(delta(0, Some("call"), Some(&name), "{}"))
            .unwrap_err();
        assert!(error.message.contains("name"));
    }

    #[test]
    fn rejects_missing_identity_fields_at_finish() {
        let mut assembler = ToolCallAssembler::new();
        assembler.push(delta(0, None, Some("tool"), "{}")).unwrap();
        assert!(assembler.finish().unwrap_err().message.contains("id"));

        let mut assembler = ToolCallAssembler::new();
        assembler.push(delta(0, Some("call"), None, "{}")).unwrap();
        assert!(assembler.finish().unwrap_err().message.contains("name"));
    }

    #[test]
    fn rejects_duplicate_ids_at_finish() {
        let mut assembler = ToolCallAssembler::new();
        assembler
            .push(delta(0, Some("call-1"), Some("a"), "{}"))
            .unwrap();
        assembler
            .push(delta(1, Some("call-1"), Some("b"), "{}"))
            .unwrap();
        let error = assembler.finish().unwrap_err();
        assert!(error.message.contains("repeats an earlier id"));
    }

    #[test]
    fn rejects_incomplete_or_invalid_arguments_at_finish() {
        for arguments in ["", "{\"q\":", "not json"] {
            let mut assembler = ToolCallAssembler::new();
            assembler
                .push(delta(0, Some("call-1"), Some("tool"), arguments))
                .unwrap();
            let error = assembler.finish().unwrap_err();
            assert_eq!(error.code, AI_E_PROVIDER);
            assert!(error.message.contains("invalid JSON"));
        }
    }

    #[test]
    fn empty_stream_finishes_with_no_calls() {
        assert!(ToolCallAssembler::new().finish().unwrap().is_empty());
    }

    #[test]
    fn bounds_the_number_of_delta_frames() {
        let mut assembler = ToolCallAssembler::new();
        for _ in 0..MAX_STREAM_TOOL_DELTAS {
            assembler.push(delta(0, None, None, "")).unwrap();
        }
        let error = assembler.push(delta(0, None, None, "")).unwrap_err();
        assert!(error.message.contains("delta limit"));
    }
}
