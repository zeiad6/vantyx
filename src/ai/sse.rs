//! Incremental server-sent-events parser with a hard event-size limit.
//!
//! The parser follows the event-stream format: `event`, `id`, `retry`, and
//! multi-line `data` fields, comments, CRLF or LF line endings, and blank-line
//! dispatch. It does not perform HTTP; the transport feeds it bytes.

use std::fmt;

pub const DEFAULT_MAX_EVENT_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SseEvent {
    /// `None` means the default `message` event.
    pub event: Option<String>,
    pub id: Option<String>,
    pub data: String,
    pub retry: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SseError {
    pub message: String,
}

impl SseError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for SseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for SseError {}

pub struct SseParser {
    buffer: String,
    byte_carry: Vec<u8>,
    max_event_bytes: usize,
    event: Option<String>,
    id: Option<String>,
    retry: Option<u64>,
    data: Vec<String>,
    data_bytes: usize,
}

impl Default for SseParser {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_EVENT_BYTES)
    }
}

impl SseParser {
    pub fn new(max_event_bytes: usize) -> Self {
        Self {
            buffer: String::new(),
            byte_carry: Vec::new(),
            max_event_bytes,
            event: None,
            id: None,
            retry: None,
            data: Vec::new(),
            data_bytes: 0,
        }
    }

    /// Feeds a chunk and returns every event completed by it. Chunks may split
    /// lines or UTF-8 sequences at arbitrary boundaries.
    pub fn push(&mut self, chunk: &str) -> Result<Vec<SseEvent>, SseError> {
        self.buffer.push_str(chunk);
        if self.buffer.len() > self.max_event_bytes {
            return Err(SseError::new(format!(
                "SSE event exceeds the {} byte limit",
                self.max_event_bytes
            )));
        }
        let mut events = Vec::new();
        while let Some(newline) = self.buffer.find('\n') {
            let mut line = self.buffer[..newline].to_string();
            self.buffer.drain(..=newline);
            if line.ends_with('\r') {
                line.pop();
            }
            if let Some(event) = self.process_line(&line)? {
                events.push(event);
            }
        }
        Ok(events)
    }

    /// Feeds raw bytes and returns every event completed by them. UTF-8
    /// sequences may be split across chunks; incomplete trailing bytes are
    /// retained until the next call.
    pub fn push_bytes(&mut self, bytes: &[u8]) -> Result<Vec<SseEvent>, SseError> {
        self.byte_carry.extend_from_slice(bytes);
        let mut events = Vec::new();
        loop {
            match std::str::from_utf8(&self.byte_carry) {
                Ok(text) => {
                    if !text.is_empty() {
                        let text = text.to_string();
                        self.byte_carry.clear();
                        events.extend(self.push(&text)?);
                    }
                    break;
                }
                Err(error) => {
                    let valid = error.valid_up_to();
                    if valid > 0 {
                        let text = std::str::from_utf8(&self.byte_carry[..valid])
                            .expect("valid UTF-8 prefix")
                            .to_string();
                        self.byte_carry.drain(..valid);
                        events.extend(self.push(&text)?);
                        continue;
                    }
                    if error.error_len().is_some() {
                        return Err(SseError::new("SSE stream is not valid UTF-8"));
                    }
                    break;
                }
            }
        }
        Ok(events)
    }

    /// Flushes a final unterminated line and dispatches any pending event.
    pub fn finish(&mut self) -> Result<Vec<SseEvent>, SseError> {
        if !self.byte_carry.is_empty() {
            return Err(SseError::new(
                "SSE stream ended in the middle of a UTF-8 sequence",
            ));
        }
        let mut events = Vec::new();
        if !self.buffer.is_empty() {
            let mut line = std::mem::take(&mut self.buffer);
            if line.ends_with('\r') {
                line.pop();
            }
            if let Some(event) = self.process_line(&line)? {
                events.push(event);
            }
        }
        if let Some(event) = self.dispatch() {
            events.push(event);
        }
        Ok(events)
    }

    fn process_line(&mut self, line: &str) -> Result<Option<SseEvent>, SseError> {
        if line.is_empty() {
            return Ok(self.dispatch());
        }
        if line.starts_with(':') {
            return Ok(None);
        }
        let (field, value) = match line.split_once(':') {
            Some((field, value)) => (field, value.strip_prefix(' ').unwrap_or(value)),
            None => (line, ""),
        };
        match field {
            "event" => self.event = Some(value.to_string()),
            "id" if !value.contains('\0') => self.id = Some(value.to_string()),
            "retry" => {
                if !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()) {
                    self.retry = value.parse::<u64>().ok();
                }
            }
            "data" => {
                self.data_bytes = self.data_bytes.saturating_add(value.len() + 1);
                if self.data_bytes > self.max_event_bytes {
                    return Err(SseError::new(format!(
                        "SSE data exceeds the {} byte limit",
                        self.max_event_bytes
                    )));
                }
                self.data.push(value.to_string());
            }
            _ => {}
        }
        Ok(None)
    }

    fn dispatch(&mut self) -> Option<SseEvent> {
        if self.data.is_empty() {
            self.event = None;
            self.id = None;
            self.retry = None;
            return None;
        }
        let data = std::mem::take(&mut self.data).join("\n");
        self.data_bytes = 0;
        let event = SseEvent {
            event: self.event.take(),
            id: self.id.take(),
            data,
            retry: self.retry.take(),
        };
        Some(event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_multiline_data_and_fields() {
        let mut parser = SseParser::default();
        let events = parser
            .push("event: delta\nid: 7\ndata: first\ndata: second\n\n")
            .unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event.as_deref(), Some("delta"));
        assert_eq!(events[0].id.as_deref(), Some("7"));
        assert_eq!(events[0].data, "first\nsecond");
    }

    #[test]
    fn handles_crlf_split_across_chunks() {
        let mut parser = SseParser::default();
        assert!(parser.push("data: hel").unwrap().is_empty());
        assert!(parser.push("lo\r").unwrap().is_empty());
        let events = parser.push("\n\r\n").unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "hello");
    }

    #[test]
    fn ignores_comments_and_empty_dispatch() {
        let mut parser = SseParser::default();
        assert!(parser.push(": keep-alive\n\n").unwrap().is_empty());
        let events = parser.push("data: value\n\n").unwrap();
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn parses_retry_only_when_numeric() {
        let mut parser = SseParser::default();
        let events = parser
            .push("retry: 250\ndata: x\n\nretry: nope\ndata: y\n\n")
            .unwrap();
        assert_eq!(events[0].retry, Some(250));
        assert_eq!(events[1].retry, None);
    }

    #[test]
    fn dispatches_pending_data_on_finish() {
        let mut parser = SseParser::default();
        assert!(parser.push("data: tail").unwrap().is_empty());
        let events = parser.finish().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "tail");
    }

    #[test]
    fn enforces_the_event_limit() {
        let mut parser = SseParser::new(8);
        assert!(parser.push("data: 123456789\n\n").is_err());
    }

    #[test]
    fn preserves_unicode_across_chunks() {
        let mut parser = SseParser::default();
        let encoded = "data: héllo 😀\n\n";
        let (first, second) = encoded.split_at(9);
        assert!(parser.push(first).unwrap().is_empty());
        let events = parser.push(second).unwrap();
        assert_eq!(events[0].data, "héllo 😀");
    }

    #[test]
    fn parses_raw_bytes_split_inside_utf8() {
        let mut parser = SseParser::default();
        let encoded = "data: héllo 😀\n\n".as_bytes();
        // Split inside the two-byte `é` sequence.
        let mut events = parser.push_bytes(&encoded[..8]).unwrap();
        assert!(events.is_empty());
        events.extend(parser.push_bytes(&encoded[8..]).unwrap());
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "héllo 😀");
    }

    #[test]
    fn parses_raw_bytes_split_inside_every_codepoint() {
        let mut parser = SseParser::default();
        let encoded = "data: aé😀\n\n".as_bytes();
        let mut events = Vec::new();
        for byte in encoded {
            events.extend(parser.push_bytes(&[*byte]).unwrap());
        }
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "aé😀");
    }

    #[test]
    fn rejects_invalid_utf8_bytes() {
        let mut parser = SseParser::default();
        assert!(parser.push_bytes(b"data: \xff\n\n").is_err());
    }

    #[test]
    fn finish_rejects_a_dangling_utf8_prefix() {
        let mut parser = SseParser::default();
        assert!(
            parser
                .push_bytes(&"data: é".as_bytes()[..7])
                .unwrap()
                .is_empty()
        );
        assert!(parser.finish().is_err());
    }
}
