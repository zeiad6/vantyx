//! Secret redaction for errors, diagnostics, and metrics.
//!
//! Redaction is best-effort defense in depth, not a guarantee. Callers must
//! still avoid putting secrets into prompts, logs, or error messages.

/// Replaces every known secret value and common bearer-token shapes.
pub fn redact(text: &str, secrets: &[&str]) -> String {
    let mut output = text.to_string();
    for secret in secrets {
        if !secret.is_empty() {
            output = output.replace(secret, "[REDACTED]");
        }
    }
    redact_bearer_tokens(&output)
}

/// Truncates untrusted provider text and redacts known secrets. The result is
/// safe to include in a diagnostic, but it is still untrusted content.
pub fn sanitize(text: &str, secrets: &[&str], max_chars: usize) -> String {
    let redacted = redact(text, secrets);
    let mut truncated: String = redacted.chars().take(max_chars).collect();
    if redacted.chars().count() > max_chars {
        truncated.push_str("… [truncated]");
    }
    truncated.replace(['\r', '\n'], " ")
}

fn redact_bearer_tokens(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(index) = rest.find("Bearer ") {
        output.push_str(&rest[..index]);
        output.push_str("Bearer [REDACTED]");
        let after = &rest[index + "Bearer ".len()..];
        let end = after
            .find(|character: char| {
                character.is_whitespace() || character == '"' || character == '\''
            })
            .unwrap_or(after.len());
        rest = &after[end..];
    }
    output.push_str(rest);
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_exact_secrets() {
        let text = "request failed with key sk-test-123 and sk-test-123";
        assert_eq!(
            redact(text, &["sk-test-123"]),
            "request failed with key [REDACTED] and [REDACTED]"
        );
    }

    #[test]
    fn removes_bearer_tokens() {
        assert_eq!(
            redact("Authorization: Bearer abc.def-123", &[]),
            "Authorization: Bearer [REDACTED]"
        );
    }

    #[test]
    fn truncates_and_flattens_untrusted_text() {
        let sanitized = sanitize("line one\nline two", &[], 8);
        assert!(sanitized.starts_with("line one"));
        assert!(sanitized.ends_with("[truncated]"));
        assert!(!sanitized.contains('\n'));
    }

    #[test]
    fn empty_secrets_are_ignored() {
        assert_eq!(redact("safe", &[""]), "safe");
    }
}
