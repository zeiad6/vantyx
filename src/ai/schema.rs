//! A small, strict JSON Schema subset for tool arguments and structured output.
//!
//! Supported: `type`, `enum`, `const`, `anyOf`, `allOf`, `required`,
//! `properties`, `additionalProperties`, `items`, `minItems`, `maxItems`,
//! `minLength`, `maxLength`, `minimum`, `maximum`.
//!
//! Unsupported keywords are ignored by design; callers must not claim full
//! JSON Schema compliance.

use super::json::Json;
use std::fmt;

pub const MAX_SCHEMA_DEPTH: usize = 32;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SchemaError {
    pub path: String,
    pub message: String,
}

impl SchemaError {
    fn new(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            message: message.into(),
        }
    }
}

impl fmt::Display for SchemaError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} at {}", self.message, self.path)
    }
}

impl std::error::Error for SchemaError {}

pub fn validate(schema: &Json, value: &Json) -> Result<(), SchemaError> {
    validate_inner(schema, value, "$", 0)
}

/// Checks that a schema uses the supported keywords with valid shapes.
///
/// This is a structural preflight, not full JSON Schema validation. Unsupported
/// keywords are ignored, as they are during [`validate`].
pub fn check_schema(schema: &Json) -> Result<(), SchemaError> {
    check_schema_inner(schema, "$", 0)
}

fn check_schema_inner(schema: &Json, path: &str, depth: usize) -> Result<(), SchemaError> {
    if depth > MAX_SCHEMA_DEPTH {
        return Err(SchemaError::new(path, "schema nesting depth exceeded"));
    }
    if !matches!(schema, Json::Object(_)) {
        return Err(SchemaError::new(path, "schema must be a JSON object"));
    }

    if let Some(any_of) = schema.get("anyOf") {
        let options = any_of
            .as_array()
            .ok_or_else(|| SchemaError::new(path, "`anyOf` must be an array"))?;
        if options.is_empty() {
            return Err(SchemaError::new(
                path,
                "`anyOf` must contain at least one schema",
            ));
        }
        for (index, option) in options.iter().enumerate() {
            check_schema_inner(option, &format!("{path}.anyOf[{index}]"), depth + 1)?;
        }
    }

    if let Some(all_of) = schema.get("allOf") {
        let options = all_of
            .as_array()
            .ok_or_else(|| SchemaError::new(path, "`allOf` must be an array"))?;
        if options.is_empty() {
            return Err(SchemaError::new(
                path,
                "`allOf` must contain at least one schema",
            ));
        }
        for (index, option) in options.iter().enumerate() {
            check_schema_inner(option, &format!("{path}.allOf[{index}]"), depth + 1)?;
        }
    }

    if let Some(enum_values) = schema.get("enum") {
        let options = enum_values
            .as_array()
            .ok_or_else(|| SchemaError::new(path, "`enum` must be an array"))?;
        if options.is_empty() {
            return Err(SchemaError::new(
                path,
                "`enum` must contain at least one value",
            ));
        }
    }

    if let Some(type_value) = schema.get("type") {
        check_type_schema(type_value, path)?;
    }

    if let Some(required) = schema.get("required") {
        let names = required
            .as_array()
            .ok_or_else(|| SchemaError::new(path, "`required` must be an array"))?;
        for (index, name) in names.iter().enumerate() {
            if name.as_str().is_none() {
                return Err(SchemaError::new(
                    path,
                    format!("`required[{index}]` must be a string"),
                ));
            }
        }
    }

    if let Some(properties) = schema.get("properties") {
        let properties = properties
            .as_object()
            .ok_or_else(|| SchemaError::new(path, "`properties` must be an object"))?;
        for (name, property_schema) in properties {
            check_schema_inner(
                property_schema,
                &format!("{path}.properties.{name}"),
                depth + 1,
            )?;
        }
    }

    if let Some(additional) = schema.get("additionalProperties") {
        match additional {
            Json::Bool(_) => {}
            Json::Object(_) => check_schema_inner(
                additional,
                &format!("{path}.additionalProperties"),
                depth + 1,
            )?,
            _ => {
                return Err(SchemaError::new(
                    path,
                    "`additionalProperties` must be a boolean or a schema",
                ));
            }
        }
    }

    if let Some(items) = schema.get("items") {
        check_schema_inner(items, &format!("{path}.items"), depth + 1)?;
    }

    for keyword in ["minItems", "maxItems", "minLength", "maxLength"] {
        if let Some(value) = schema.get(keyword)
            && value.as_u64().is_none()
        {
            return Err(SchemaError::new(
                path,
                format!("`{keyword}` must be a non-negative integer"),
            ));
        }
    }
    for keyword in ["minimum", "maximum"] {
        if let Some(value) = schema.get(keyword)
            && value.as_f64().is_none()
        {
            return Err(SchemaError::new(
                path,
                format!("`{keyword}` must be a number"),
            ));
        }
    }

    Ok(())
}

fn check_type_schema(type_value: &Json, path: &str) -> Result<(), SchemaError> {
    let names: Vec<&str> = match type_value {
        Json::String(name) => vec![name.as_str()],
        Json::Array(names) => {
            if names.is_empty() {
                return Err(SchemaError::new(path, "`type` array must not be empty"));
            }
            let mut result = Vec::new();
            for name in names {
                result.push(
                    name.as_str()
                        .ok_or_else(|| SchemaError::new(path, "`type` entries must be strings"))?,
                );
            }
            result
        }
        _ => return Err(SchemaError::new(path, "`type` must be a string or array")),
    };
    for name in names {
        if !matches!(
            name,
            "object" | "array" | "string" | "number" | "integer" | "boolean" | "null"
        ) {
            return Err(SchemaError::new(
                path,
                format!("unknown schema type `{name}`"),
            ));
        }
    }
    Ok(())
}

fn validate_inner(
    schema: &Json,
    value: &Json,
    path: &str,
    depth: usize,
) -> Result<(), SchemaError> {
    if depth > MAX_SCHEMA_DEPTH {
        return Err(SchemaError::new(path, "schema nesting depth exceeded"));
    }
    if !matches!(schema, Json::Object(_)) {
        return Err(SchemaError::new(path, "schema must be a JSON object"));
    }

    if let Some(any_of) = schema.get("anyOf") {
        let options = any_of
            .as_array()
            .ok_or_else(|| SchemaError::new(path, "`anyOf` must be an array"))?;
        let mut last_error = None;
        let mut matched = false;
        for option in options {
            match validate_inner(option, value, path, depth + 1) {
                Ok(()) => {
                    matched = true;
                    break;
                }
                Err(error) => last_error = Some(error),
            }
        }
        if !matched {
            return Err(last_error.unwrap_or_else(|| {
                SchemaError::new(path, "`anyOf` must contain at least one schema")
            }));
        }
    }

    if let Some(all_of) = schema.get("allOf") {
        let options = all_of
            .as_array()
            .ok_or_else(|| SchemaError::new(path, "`allOf` must be an array"))?;
        for option in options {
            validate_inner(option, value, path, depth + 1)?;
        }
    }

    if let Some(constant) = schema.get("const")
        && !super::json::value_eq(constant, value)
    {
        return Err(SchemaError::new(path, "value does not match `const`"));
    }

    if let Some(enum_values) = schema.get("enum") {
        let options = enum_values
            .as_array()
            .ok_or_else(|| SchemaError::new(path, "`enum` must be an array"))?;
        if !options
            .iter()
            .any(|option| super::json::value_eq(option, value))
        {
            return Err(SchemaError::new(
                path,
                "value is not one of the allowed enum values",
            ));
        }
    }

    if let Some(type_value) = schema.get("type") {
        check_type(type_value, value, path)?;
    }

    if let Some(required) = schema.get("required") {
        let required = required
            .as_array()
            .ok_or_else(|| SchemaError::new(path, "`required` must be an array"))?;
        // Like `properties`, `required` applies only to objects. The `type`
        // keyword is responsible for rejecting a value of the wrong type.
        if let Json::Object(entries) = value {
            for name in required {
                let name = name
                    .as_str()
                    .ok_or_else(|| SchemaError::new(path, "`required` entries must be strings"))?;
                if !entries.iter().any(|(key, _)| key == name) {
                    return Err(SchemaError::new(
                        format!("{path}.{name}"),
                        format!("missing required property `{name}`"),
                    ));
                }
            }
        }
    }

    if let Some(properties) = schema.get("properties") {
        let properties = properties
            .as_object()
            .ok_or_else(|| SchemaError::new(path, "`properties` must be an object"))?;
        if let Json::Object(entries) = value {
            for (name, property_schema) in properties {
                if let Some(property_value) = entries
                    .iter()
                    .find_map(|(key, value)| (key == name).then_some(value))
                {
                    validate_inner(
                        property_schema,
                        property_value,
                        &format!("{path}.{name}"),
                        depth + 1,
                    )?;
                }
            }
        }
    }

    if let Some(additional) = schema.get("additionalProperties")
        && let Json::Object(entries) = value
    {
        let known: Vec<&str> = schema
            .get("properties")
            .and_then(Json::as_object)
            .map(|properties| properties.iter().map(|(name, _)| name.as_str()).collect())
            .unwrap_or_default();
        for (name, property_value) in entries {
            if known.contains(&name.as_str()) {
                continue;
            }
            match additional {
                Json::Bool(false) => {
                    return Err(SchemaError::new(
                        format!("{path}.{name}"),
                        format!("additional property `{name}` is not allowed"),
                    ));
                }
                Json::Bool(true) => {}
                schema @ Json::Object(_) => {
                    validate_inner(schema, property_value, &format!("{path}.{name}"), depth + 1)?;
                }
                _ => {
                    return Err(SchemaError::new(
                        path,
                        "`additionalProperties` must be a boolean or a schema",
                    ));
                }
            }
        }
    }

    if let Some(items) = schema.get("items")
        && let Json::Array(values) = value
    {
        for (index, item) in values.iter().enumerate() {
            validate_inner(items, item, &format!("{path}[{index}]"), depth + 1)?;
        }
    }

    if let Some(min_items) = schema.get("minItems") {
        let minimum = min_items
            .as_u64()
            .ok_or_else(|| SchemaError::new(path, "`minItems` must be an integer"))?;
        if let Json::Array(values) = value
            && (values.len() as u64) < minimum
        {
            return Err(SchemaError::new(
                path,
                format!("array has fewer than {minimum} items"),
            ));
        }
    }

    if let Some(max_items) = schema.get("maxItems") {
        let maximum = max_items
            .as_u64()
            .ok_or_else(|| SchemaError::new(path, "`maxItems` must be an integer"))?;
        if let Json::Array(values) = value
            && values.len() as u64 > maximum
        {
            return Err(SchemaError::new(
                path,
                format!("array has more than {maximum} items"),
            ));
        }
    }

    if let Json::String(text) = value {
        let length = text.chars().count() as u64;
        if let Some(min_length) = schema.get("minLength") {
            let minimum = min_length
                .as_u64()
                .ok_or_else(|| SchemaError::new(path, "`minLength` must be an integer"))?;
            if length < minimum {
                return Err(SchemaError::new(
                    path,
                    format!("string is shorter than {minimum} characters"),
                ));
            }
        }
        if let Some(max_length) = schema.get("maxLength") {
            let maximum = max_length
                .as_u64()
                .ok_or_else(|| SchemaError::new(path, "`maxLength` must be an integer"))?;
            if length > maximum {
                return Err(SchemaError::new(
                    path,
                    format!("string is longer than {maximum} characters"),
                ));
            }
        }
    }

    if let Json::Number(number) = value {
        if let Some(minimum) = schema.get("minimum") {
            let Json::Number(minimum) = minimum else {
                return Err(SchemaError::new(path, "`minimum` must be a number"));
            };
            if super::json::number_cmp(*number, *minimum) == Some(std::cmp::Ordering::Less) {
                return Err(SchemaError::new(
                    path,
                    format!("number is less than the minimum {minimum}"),
                ));
            }
        }
        if let Some(maximum) = schema.get("maximum") {
            let Json::Number(maximum) = maximum else {
                return Err(SchemaError::new(path, "`maximum` must be a number"));
            };
            if super::json::number_cmp(*number, *maximum) == Some(std::cmp::Ordering::Greater) {
                return Err(SchemaError::new(
                    path,
                    format!("number is greater than the maximum {maximum}"),
                ));
            }
        }
    }

    Ok(())
}

fn check_type(type_value: &Json, value: &Json, path: &str) -> Result<(), SchemaError> {
    let names: Vec<&str> = match type_value {
        Json::String(name) => vec![name.as_str()],
        Json::Array(names) => {
            let mut result = Vec::new();
            for name in names {
                result.push(
                    name.as_str()
                        .ok_or_else(|| SchemaError::new(path, "`type` entries must be strings"))?,
                );
            }
            result
        }
        _ => return Err(SchemaError::new(path, "`type` must be a string or array")),
    };
    if names.is_empty() {
        return Err(SchemaError::new(path, "`type` array must not be empty"));
    }
    for name in &names {
        if type_matches(name, value)? {
            return Ok(());
        }
    }
    Err(SchemaError::new(
        path,
        format!(
            "expected type {}, found {}",
            names.join(" or "),
            type_name(value)
        ),
    ))
}

fn type_matches(name: &str, value: &Json) -> Result<bool, SchemaError> {
    Ok(match name {
        "object" => matches!(value, Json::Object(_)),
        "array" => matches!(value, Json::Array(_)),
        "string" => matches!(value, Json::String(_)),
        "number" => matches!(value, Json::Number(_)),
        "integer" => match value {
            Json::Number(number) => number.as_i64().is_some() || number.as_u64().is_some(),
            _ => false,
        },
        "boolean" => matches!(value, Json::Bool(_)),
        "null" => matches!(value, Json::Null),
        other => {
            return Err(SchemaError::new(
                "$",
                format!("unknown schema type `{other}`"),
            ));
        }
    })
}

fn type_name(value: &Json) -> &'static str {
    match value {
        Json::Null => "null",
        Json::Bool(_) => "boolean",
        Json::Number(_) => "number",
        Json::String(_) => "string",
        Json::Array(_) => "array",
        Json::Object(_) => "object",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::json::parse;

    fn schema_json(text: &str) -> Json {
        parse(text).unwrap()
    }

    #[test]
    fn validates_object_properties_and_required() {
        let schema = schema_json(
            r#"{"type":"object","required":["name"],"properties":{"name":{"type":"string"},"age":{"type":"integer","minimum":0}},"additionalProperties":false}"#,
        );
        assert!(validate(&schema, &schema_json(r#"{"name":"Ada","age":36}"#)).is_ok());
        let missing = validate(&schema, &schema_json(r#"{"age":36}"#)).unwrap_err();
        assert!(missing.message.contains("required"));
        let extra = validate(&schema, &schema_json(r#"{"name":"Ada","extra":1}"#)).unwrap_err();
        assert!(extra.message.contains("additional"));
        let wrong = validate(&schema, &schema_json(r#"{"name":"Ada","age":-1}"#)).unwrap_err();
        assert!(wrong.path.contains("age"));
    }

    #[test]
    fn required_is_ignored_for_non_objects() {
        let schema =
            schema_json(r#"{"required":["name"],"properties":{"name":{"type":"string"}}}"#);
        assert!(validate(&schema, &schema_json(r#""text""#)).is_ok());
        assert!(validate(&schema, &schema_json("3")).is_ok());
        assert!(validate(&schema, &schema_json("null")).is_ok());
        assert!(validate(&schema, &schema_json(r#"["x"]"#)).is_ok());
        assert!(validate(&schema, &schema_json(r#"{"name":"Ada"}"#)).is_ok());
        let missing = validate(&schema, &schema_json("{}")).unwrap_err();
        assert!(missing.message.contains("required"));
    }

    #[test]
    fn numeric_bounds_preserve_integer_precision() {
        let minimum = schema_json(r#"{"minimum":9007199254740993}"#);
        assert!(validate(&minimum, &schema_json("9007199254740993")).is_ok());
        assert!(validate(&minimum, &schema_json("9007199254740994")).is_ok());
        let low = validate(&minimum, &schema_json("9007199254740992")).unwrap_err();
        assert!(low.message.contains("minimum"));
        let low = validate(&minimum, &schema_json("9007199254740992.0")).unwrap_err();
        assert!(low.message.contains("minimum"));

        let maximum = schema_json(r#"{"maximum":9007199254740993}"#);
        assert!(validate(&maximum, &schema_json("9007199254740993")).is_ok());
        assert!(validate(&maximum, &schema_json("9007199254740992")).is_ok());
        let high = validate(&maximum, &schema_json("9007199254740994")).unwrap_err();
        assert!(high.message.contains("maximum"));
        let high = validate(&maximum, &schema_json("9007199254740994.0")).unwrap_err();
        assert!(high.message.contains("maximum"));

        let minimum = schema_json(r#"{"minimum":18446744073709551614}"#);
        assert!(validate(&minimum, &schema_json("18446744073709551615")).is_ok());
        let low = validate(&minimum, &schema_json("18446744073709551613")).unwrap_err();
        assert!(low.message.contains("minimum"));

        let minimum = schema_json(r#"{"minimum":1.5}"#);
        assert!(validate(&minimum, &schema_json("2")).is_ok());
        let low = validate(&minimum, &schema_json("1")).unwrap_err();
        assert!(low.message.contains("minimum"));

        let maximum = schema_json(r#"{"maximum":1.5}"#);
        assert!(validate(&maximum, &schema_json("1")).is_ok());
        let high = validate(&maximum, &schema_json("2")).unwrap_err();
        assert!(high.message.contains("maximum"));
    }

    #[test]
    fn const_and_enum_use_json_value_equality() {
        let schema = schema_json(r#"{"enum":[1, 2.5, {"a": 1, "b": [true]}]}"#);
        assert!(validate(&schema, &schema_json("1.0")).is_ok());
        assert!(validate(&schema, &schema_json("2.50")).is_ok());
        assert!(validate(&schema, &schema_json(r#"{"b":[true],"a":1.0}"#)).is_ok());
        assert!(validate(&schema, &schema_json("3")).is_err());
        assert!(validate(&schema, &schema_json(r#"{"a":1}"#)).is_err());

        let schema = schema_json(r#"{"const":1}"#);
        assert!(validate(&schema, &schema_json("1.0")).is_ok());
        assert!(validate(&schema, &schema_json("2")).is_err());

        let schema = schema_json(r#"{"const":9007199254740993}"#);
        assert!(validate(&schema, &schema_json("9007199254740993")).is_ok());
        assert!(validate(&schema, &schema_json("9007199254740992.0")).is_err());

        // A float at 2^64 must not saturate onto u64::MAX.
        let schema = schema_json(r#"{"const":18446744073709551615}"#);
        assert!(validate(&schema, &schema_json("18446744073709551615")).is_ok());
        assert!(validate(&schema, &schema_json("18446744073709551616.0")).is_err());

        let schema = schema_json(r#"{"enum":[0, -1]}"#);
        assert!(validate(&schema, &schema_json("-0.0")).is_ok());
        assert!(validate(&schema, &schema_json("-1.0")).is_ok());
        assert!(validate(&schema, &schema_json("1.0")).is_err());
    }

    #[test]
    fn sibling_keywords_are_not_skipped() {
        let any_of =
            schema_json(r#"{"anyOf":[{"type":"string"},{"type":"integer"}],"minimum":10}"#);
        assert!(validate(&any_of, &schema_json(r#""x""#)).is_ok());
        assert!(validate(&any_of, &schema_json("15")).is_ok());
        let low = validate(&any_of, &schema_json("5")).unwrap_err();
        assert!(low.message.contains("minimum"));

        let any_of_required =
            schema_json(r#"{"anyOf":[{"type":"object"},{"type":"string"}],"required":["name"]}"#);
        assert!(validate(&any_of_required, &schema_json(r#""x""#)).is_ok());
        let missing = validate(&any_of_required, &schema_json("{}")).unwrap_err();
        assert!(missing.message.contains("required"));

        let additional = schema_json(r#"{"additionalProperties":false,"minLength":3}"#);
        assert!(validate(&additional, &schema_json(r#""abc""#)).is_ok());
        let short = validate(&additional, &schema_json(r#""a""#)).unwrap_err();
        assert!(short.message.contains("shorter"));

        let items = schema_json(r#"{"items":{"type":"integer"},"minLength":3}"#);
        assert!(validate(&items, &schema_json(r#""abc""#)).is_ok());
        let short = validate(&items, &schema_json(r#""a""#)).unwrap_err();
        assert!(short.message.contains("shorter"));
    }

    #[test]
    fn validates_arrays_and_enum() {
        let schema =
            schema_json(r#"{"type":"array","minItems":1,"maxItems":2,"items":{"enum":["a","b"]}}"#);
        assert!(validate(&schema, &schema_json(r#"["a"]"#)).is_ok());
        assert!(validate(&schema, &schema_json(r#"[]"#)).is_err());
        assert!(validate(&schema, &schema_json(r#"["c"]"#)).is_err());
    }

    #[test]
    fn validates_any_of_and_const() {
        let schema = schema_json(r#"{"anyOf":[{"const":"x"},{"type":"integer"}]}"#);
        assert!(validate(&schema, &schema_json(r#""x""#)).is_ok());
        assert!(validate(&schema, &schema_json("3")).is_ok());
        assert!(validate(&schema, &schema_json(r#""y""#)).is_err());
    }

    #[test]
    fn reports_the_failing_path() {
        let schema = schema_json(
            r#"{"type":"object","properties":{"user":{"type":"object","properties":{"id":{"type":"integer"}}}}}"#,
        );
        let error = validate(&schema, &schema_json(r#"{"user":{"id":"nope"}}"#)).unwrap_err();
        assert_eq!(error.path, "$.user.id");
    }

    #[test]
    fn rejects_invalid_schema_shapes() {
        assert!(validate(&Json::Null, &Json::Null).is_err());
        assert!(validate(&schema_json(r#"{"type":"wat"}"#), &Json::Null).is_err());
        assert!(validate(&schema_json(r#"{"required":"name"}"#), &Json::Null).is_err());
    }

    #[test]
    fn rejects_excessive_schema_depth() {
        let mut schema = schema_json(r#"{"type":"integer"}"#);
        let mut value = Json::from(1_i64);
        for _ in 0..MAX_SCHEMA_DEPTH + 2 {
            schema = Json::object([("items", schema)]);
            value = Json::Array(vec![value]);
        }
        assert!(validate(&schema, &value).is_err());
    }

    #[test]
    fn check_schema_accepts_supported_keywords() {
        let schema = schema_json(
            r#"{"type":"object","required":["name"],"properties":{"name":{"type":"string","minLength":1},"tags":{"type":"array","items":{"enum":["a","b"]}}},"additionalProperties":false,"anyOf":[{"required":["name"]},{"required":["id"]}]}"#,
        );
        assert!(check_schema(&schema).is_ok());
    }

    #[test]
    fn check_schema_rejects_malformed_keywords() {
        let cases = [
            r#"null"#,
            r#"{"type":"wat"}"#,
            r#"{"type":[]}"#,
            r#"{"type":[1]}"#,
            r#"{"required":"name"}"#,
            r#"{"required":[1]}"#,
            r#"{"properties":[]}"#,
            r#"{"properties":{"name":"string"}}"#,
            r#"{"additionalProperties":"no"}"#,
            r#"{"items":[]}"#,
            r#"{"minItems":"1"}"#,
            r#"{"minimum":"0"}"#,
            r#"{"enum":"a"}"#,
            r#"{"enum":[]}"#,
            r#"{"anyOf":[]}"#,
            r#"{"anyOf":[null]}"#,
            r#"{"allOf":"no"}"#,
            r#"{"allOf":[]}"#,
        ];
        for text in cases {
            let schema = schema_json(text);
            assert!(
                check_schema(&schema).is_err(),
                "schema should be rejected: {text}"
            );
        }
    }

    #[test]
    fn check_schema_rejects_excessive_depth() {
        let mut schema = schema_json(r#"{"type":"integer"}"#);
        for _ in 0..MAX_SCHEMA_DEPTH + 2 {
            schema = Json::object([("items", schema)]);
        }
        assert!(check_schema(&schema).is_err());
    }
}
