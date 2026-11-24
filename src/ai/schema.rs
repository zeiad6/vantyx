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
        for option in options {
            match validate_inner(option, value, path, depth + 1) {
                Ok(()) => return Ok(()),
                Err(error) => last_error = Some(error),
            }
        }
        return Err(last_error.unwrap_or_else(|| {
            SchemaError::new(path, "`anyOf` must contain at least one schema")
        }));
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
        && constant != value
    {
        return Err(SchemaError::new(path, "value does not match `const`"));
    }

    if let Some(enum_values) = schema.get("enum") {
        let options = enum_values
            .as_array()
            .ok_or_else(|| SchemaError::new(path, "`enum` must be an array"))?;
        if !options.contains(value) {
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
        let Json::Object(entries) = value else {
            return Err(SchemaError::new(path, "`required` applies only to objects"));
        };
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

    if let Some(additional) = schema.get("additionalProperties") {
        let Json::Object(entries) = value else {
            // The `type` check is responsible for non-objects.
            return Ok(());
        };
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

    if let Some(items) = schema.get("items") {
        let Json::Array(values) = value else {
            return Ok(());
        };
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
        let numeric = number.as_f64();
        if let Some(minimum) = schema.get("minimum") {
            let minimum = minimum
                .as_f64()
                .ok_or_else(|| SchemaError::new(path, "`minimum` must be a number"))?;
            if numeric < minimum {
                return Err(SchemaError::new(
                    path,
                    format!("number is less than the minimum {minimum}"),
                ));
            }
        }
        if let Some(maximum) = schema.get("maximum") {
            let maximum = maximum
                .as_f64()
                .ok_or_else(|| SchemaError::new(path, "`maximum` must be a number"))?;
            if numeric > maximum {
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
}
