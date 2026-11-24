//! Strict JSON value, parser, and writer used by the AI host layer.
//!
//! The parser rejects trailing data, duplicate object keys, non-finite numbers,
//! invalid escapes, and excessive nesting. It is deliberately small: it is a
//! security boundary for model output and tool arguments, not a general-purpose
//! JSON library.

use std::fmt;

/// Maximum nesting depth accepted by the parser.
pub const MAX_DEPTH: usize = 128;

/// A JSON number that preserves integer precision where possible.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum JsonNumber {
    Int(i64),
    UInt(u64),
    Float(f64),
}

impl JsonNumber {
    pub fn as_i64(self) -> Option<i64> {
        match self {
            Self::Int(value) => Some(value),
            Self::UInt(value) => i64::try_from(value).ok(),
            Self::Float(value) => {
                if value.is_finite() && value.fract() == 0.0 {
                    let rounded = value as i64;
                    if (rounded as f64) == value {
                        return Some(rounded);
                    }
                }
                None
            }
        }
    }

    pub fn as_u64(self) -> Option<u64> {
        match self {
            Self::Int(value) => u64::try_from(value).ok(),
            Self::UInt(value) => Some(value),
            Self::Float(value) => {
                if value.is_finite() && value.fract() == 0.0 && value >= 0.0 {
                    Some(value as u64)
                } else {
                    None
                }
            }
        }
    }

    pub fn as_f64(self) -> f64 {
        match self {
            Self::Int(value) => value as f64,
            Self::UInt(value) => value as f64,
            Self::Float(value) => value,
        }
    }
}

impl fmt::Display for JsonNumber {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Int(value) => write!(formatter, "{value}"),
            Self::UInt(value) => write!(formatter, "{value}"),
            Self::Float(value) => write!(formatter, "{value}"),
        }
    }
}

/// A JSON value. Object entries preserve insertion order.
#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Number(JsonNumber),
    String(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

impl Json {
    pub fn object(entries: impl IntoIterator<Item = (impl Into<String>, Json)>) -> Self {
        Self::Object(
            entries
                .into_iter()
                .map(|(key, value)| (key.into(), value))
                .collect(),
        )
    }

    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Self::Object(entries) => entries
                .iter()
                .find_map(|(name, value)| (name == key).then_some(value)),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Number(value) => value.as_i64(),
            _ => None,
        }
    }

    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Self::Number(value) => value.as_u64(),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Number(value) => Some(value.as_f64()),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Json]> {
        match self {
            Self::Array(values) => Some(values),
            _ => None,
        }
    }

    pub fn as_object(&self) -> Option<&[(String, Json)]> {
        match self {
            Self::Object(entries) => Some(entries),
            _ => None,
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    /// Serializes compactly. Object order is preserved.
    pub fn to_compact_string(&self) -> String {
        let mut output = String::new();
        write_json(self, &mut output);
        output
    }
}

impl fmt::Display for Json {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.to_compact_string())
    }
}

impl From<&str> for Json {
    fn from(value: &str) -> Self {
        Self::String(value.to_string())
    }
}

impl From<String> for Json {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<bool> for Json {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<i64> for Json {
    fn from(value: i64) -> Self {
        Self::Number(JsonNumber::Int(value))
    }
}

impl From<u64> for Json {
    fn from(value: u64) -> Self {
        Self::Number(JsonNumber::UInt(value))
    }
}

impl From<f64> for Json {
    fn from(value: f64) -> Self {
        Self::Number(JsonNumber::Float(value))
    }
}

impl From<Vec<Json>> for Json {
    fn from(value: Vec<Json>) -> Self {
        Self::Array(value)
    }
}

/// A parse failure with a byte offset into the input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    pub offset: usize,
    pub message: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "JSON error at byte {}: {}",
            self.offset, self.message
        )
    }
}

impl std::error::Error for ParseError {}

/// Parses one complete JSON document.
pub fn parse(input: &str) -> Result<Json, ParseError> {
    let mut parser = Parser {
        bytes: input.as_bytes(),
        position: 0,
    };
    parser.skip_whitespace();
    let value = parser.parse_value(0)?;
    parser.skip_whitespace();
    if parser.position != parser.bytes.len() {
        return Err(parser.error("trailing characters after the JSON value"));
    }
    Ok(value)
}

struct Parser<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl Parser<'_> {
    fn parse_value(&mut self, depth: usize) -> Result<Json, ParseError> {
        if depth > MAX_DEPTH {
            return Err(self.error("maximum JSON nesting depth exceeded"));
        }
        match self.peek() {
            Some(b'n') => {
                self.expect_literal(b"null")?;
                Ok(Json::Null)
            }
            Some(b't') => {
                self.expect_literal(b"true")?;
                Ok(Json::Bool(true))
            }
            Some(b'f') => {
                self.expect_literal(b"false")?;
                Ok(Json::Bool(false))
            }
            Some(b'"') => self.parse_string().map(Json::String),
            Some(b'[') => self.parse_array(depth),
            Some(b'{') => self.parse_object(depth),
            Some(b'-' | b'0'..=b'9') => self.parse_number(),
            Some(_) => Err(self.error("expected a JSON value")),
            None => Err(self.error("unexpected end of input")),
        }
    }

    fn parse_array(&mut self, depth: usize) -> Result<Json, ParseError> {
        self.bump();
        let mut values = Vec::new();
        self.skip_whitespace();
        if self.peek() == Some(b']') {
            self.bump();
            return Ok(Json::Array(values));
        }
        loop {
            self.skip_whitespace();
            values.push(self.parse_value(depth + 1)?);
            self.skip_whitespace();
            match self.peek() {
                Some(b',') => {
                    self.bump();
                }
                Some(b']') => {
                    self.bump();
                    return Ok(Json::Array(values));
                }
                _ => return Err(self.error("expected `,` or `]` in array")),
            }
        }
    }

    fn parse_object(&mut self, depth: usize) -> Result<Json, ParseError> {
        self.bump();
        let mut entries: Vec<(String, Json)> = Vec::new();
        self.skip_whitespace();
        if self.peek() == Some(b'}') {
            self.bump();
            return Ok(Json::Object(entries));
        }
        loop {
            self.skip_whitespace();
            if self.peek() != Some(b'"') {
                return Err(self.error("expected a string object key"));
            }
            let key = self.parse_string()?;
            if entries.iter().any(|(name, _)| name == &key) {
                return Err(self.error(&format!("duplicate object key `{key}`")));
            }
            self.skip_whitespace();
            if self.peek() != Some(b':') {
                return Err(self.error("expected `:` after object key"));
            }
            self.bump();
            self.skip_whitespace();
            let value = self.parse_value(depth + 1)?;
            entries.push((key, value));
            self.skip_whitespace();
            match self.peek() {
                Some(b',') => {
                    self.bump();
                }
                Some(b'}') => {
                    self.bump();
                    return Ok(Json::Object(entries));
                }
                _ => return Err(self.error("expected `,` or `}` in object")),
            }
        }
    }

    fn parse_string(&mut self) -> Result<String, ParseError> {
        debug_assert_eq!(self.peek(), Some(b'"'));
        self.bump();
        let mut bytes = Vec::new();
        loop {
            match self.bump() {
                Some(b'"') => {
                    return String::from_utf8(bytes)
                        .map_err(|_| self.error("invalid UTF-8 in JSON string"));
                }
                Some(b'\\') => match self.bump() {
                    Some(b'"') => bytes.push(b'"'),
                    Some(b'\\') => bytes.push(b'\\'),
                    Some(b'/') => bytes.push(b'/'),
                    Some(b'b') => bytes.push(0x08),
                    Some(b'f') => bytes.push(0x0c),
                    Some(b'n') => bytes.push(b'\n'),
                    Some(b'r') => bytes.push(b'\r'),
                    Some(b't') => bytes.push(b'\t'),
                    Some(b'u') => {
                        let code_point = self.parse_unicode_escape()?;
                        let character = char::from_u32(code_point)
                            .ok_or_else(|| self.error("invalid Unicode code point"))?;
                        let mut buffer = [0_u8; 4];
                        bytes.extend_from_slice(character.encode_utf8(&mut buffer).as_bytes());
                    }
                    _ => return Err(self.error("invalid escape sequence in string")),
                },
                Some(byte) if byte < 0x20 => {
                    return Err(self.error("unescaped control character in string"));
                }
                Some(byte) => bytes.push(byte),
                None => return Err(self.error("unterminated string")),
            }
        }
    }

    fn parse_unicode_escape(&mut self) -> Result<u32, ParseError> {
        let first = self.parse_hex_quad()?;
        if (0xD800..=0xDBFF).contains(&first) {
            if self.bump() != Some(b'\\') || self.bump() != Some(b'u') {
                return Err(self.error("high surrogate without a following low surrogate"));
            }
            let second = self.parse_hex_quad()?;
            if !(0xDC00..=0xDFFF).contains(&second) {
                return Err(self.error("invalid low surrogate"));
            }
            Ok(0x1_0000 + ((first - 0xD800) << 10) + (second - 0xDC00))
        } else if (0xDC00..=0xDFFF).contains(&first) {
            Err(self.error("low surrogate without a preceding high surrogate"))
        } else {
            Ok(first)
        }
    }

    fn parse_hex_quad(&mut self) -> Result<u32, ParseError> {
        let mut value = 0_u32;
        for _ in 0..4 {
            let digit = self
                .bump()
                .and_then(hex_value)
                .ok_or_else(|| self.error("expected four hexadecimal digits"))?;
            value = (value << 4) | u32::from(digit);
        }
        Ok(value)
    }

    fn parse_number(&mut self) -> Result<Json, ParseError> {
        let start = self.position;
        if self.peek() == Some(b'-') {
            self.bump();
        }
        match self.peek() {
            Some(b'0') => {
                self.bump();
                if matches!(self.peek(), Some(b'0'..=b'9')) {
                    return Err(self.error("leading zeros are not allowed in numbers"));
                }
            }
            Some(b'1'..=b'9') => {
                while matches!(self.peek(), Some(b'0'..=b'9')) {
                    self.bump();
                }
            }
            _ => return Err(self.error("expected a digit in number")),
        }

        let mut is_float = false;
        if self.peek() == Some(b'.') {
            is_float = true;
            self.bump();
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(self.error("expected a digit after the decimal point"));
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.bump();
            }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            is_float = true;
            self.bump();
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.bump();
            }
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(self.error("expected a digit in the exponent"));
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.bump();
            }
        }

        let text = std::str::from_utf8(&self.bytes[start..self.position])
            .map_err(|_| self.error("invalid UTF-8 in number"))?;
        if !is_float {
            if let Ok(value) = text.parse::<i64>() {
                return Ok(Json::Number(JsonNumber::Int(value)));
            }
            if let Ok(value) = text.parse::<u64>() {
                return Ok(Json::Number(JsonNumber::UInt(value)));
            }
        }
        let value = text
            .parse::<f64>()
            .map_err(|_| self.error("invalid number"))?;
        if !value.is_finite() {
            return Err(self.error("JSON numbers must be finite"));
        }
        Ok(Json::Number(JsonNumber::Float(value)))
    }

    fn expect_literal(&mut self, literal: &[u8]) -> Result<(), ParseError> {
        for expected in literal {
            if self.bump() != Some(*expected) {
                return Err(self.error("invalid JSON literal"));
            }
        }
        Ok(())
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\n' | b'\r' | b'\t')) {
            self.bump();
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.position).copied()
    }

    fn bump(&mut self) -> Option<u8> {
        let byte = self.peek()?;
        self.position += 1;
        Some(byte)
    }

    fn error(&self, message: &str) -> ParseError {
        ParseError {
            offset: self.position,
            message: message.to_string(),
        }
    }
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn write_json(value: &Json, output: &mut String) {
    match value {
        Json::Null => output.push_str("null"),
        Json::Bool(true) => output.push_str("true"),
        Json::Bool(false) => output.push_str("false"),
        Json::Number(number) => output.push_str(&number.to_string()),
        Json::String(text) => write_string(text, output),
        Json::Array(values) => {
            output.push('[');
            for (index, value) in values.iter().enumerate() {
                if index > 0 {
                    output.push(',');
                }
                write_json(value, output);
            }
            output.push(']');
        }
        Json::Object(entries) => {
            output.push('{');
            for (index, (key, value)) in entries.iter().enumerate() {
                if index > 0 {
                    output.push(',');
                }
                write_string(key, output);
                output.push(':');
                write_json(value, output);
            }
            output.push('}');
        }
    }
}

fn write_string(text: &str, output: &mut String) {
    output.push('"');
    for character in text.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            '\u{08}' => output.push_str("\\b"),
            '\u{0c}' => output.push_str("\\f"),
            character if character <= '\u{1f}' => {
                output.push_str(&format!("\\u{:04x}", character as u32));
            }
            character => output.push(character),
        }
    }
    output.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nested_documents() {
        let value = parse(r#"{"a":[1,true,null,"x"],"b":{"c":-2.5e2}}"#).unwrap();
        assert_eq!(value.get("a").unwrap().as_array().unwrap().len(), 4);
        assert_eq!(
            value.get("b").unwrap().get("c").unwrap().as_f64(),
            Some(-250.0)
        );
    }

    #[test]
    fn preserves_integer_precision() {
        let value = parse("9007199254740993").unwrap();
        assert_eq!(value.as_i64(), Some(9_007_199_254_740_993));
    }

    #[test]
    fn handles_unicode_escapes_and_surrogate_pairs() {
        let value = parse(r#""\u0041\uD83D\uDE00""#).unwrap();
        assert_eq!(value.as_str(), Some("A😀"));
    }

    #[test]
    fn rejects_invalid_surrogates() {
        assert!(parse(r#""\uD800""#).is_err());
        assert!(parse(r#""\uDC00""#).is_err());
    }

    #[test]
    fn rejects_duplicate_keys() {
        let error = parse(r#"{"a":1,"a":2}"#).unwrap_err();
        assert!(error.message.contains("duplicate"));
    }

    #[test]
    fn rejects_trailing_characters() {
        assert!(parse("{} garbage").is_err());
    }

    #[test]
    fn rejects_leading_zeroes() {
        assert!(parse("01").is_err());
    }

    #[test]
    fn rejects_excessive_depth() {
        let input = "[".repeat(MAX_DEPTH + 2);
        assert!(parse(&input).is_err());
    }

    #[test]
    fn round_trips_escaped_strings() {
        let original = Json::from("line\n\"quoted\"\\path\u{1}");
        let encoded = original.to_compact_string();
        assert_eq!(parse(&encoded).unwrap(), original);
    }

    #[test]
    fn serializes_objects_in_insertion_order() {
        let value = Json::object([("z", Json::from(1_i64)), ("a", Json::from("two"))]);
        assert_eq!(value.to_compact_string(), r#"{"z":1,"a":"two"}"#);
    }
}
