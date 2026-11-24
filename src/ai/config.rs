//! AI host configuration.
//!
//! Configuration comes from environment variables only. Secrets are read from
//! one explicitly named environment variable at call time and are never stored
//! in this struct, written to disk, or printed.

use super::http::{DEFAULT_MAX_RESPONSE_BYTES, DEFAULT_TIMEOUT, NetworkPolicy};
use super::provider::{AI_E_CONFIG, AiError};
use std::env;
use std::fmt;
use std::time::Duration;

pub const DEFAULT_PROVIDER: &str = "mock";
pub const DEFAULT_MODEL: &str = "mock";
pub const DEFAULT_ENDPOINT: &str = "http://127.0.0.1:11434/v1";
pub const DEFAULT_API_KEY_ENV: &str = "VANTYX_AI_API_KEY";

#[derive(Clone)]
pub struct AiConfig {
    pub provider: String,
    pub model: String,
    pub endpoint: String,
    pub api_key_env: String,
    pub allow_network: bool,
    pub timeout: Duration,
    pub max_response_bytes: usize,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            provider: DEFAULT_PROVIDER.into(),
            model: DEFAULT_MODEL.into(),
            endpoint: DEFAULT_ENDPOINT.into(),
            api_key_env: DEFAULT_API_KEY_ENV.into(),
            allow_network: false,
            timeout: DEFAULT_TIMEOUT,
            max_response_bytes: DEFAULT_MAX_RESPONSE_BYTES,
        }
    }
}

impl fmt::Debug for AiConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AiConfig")
            .field("provider", &self.provider)
            .field("model", &self.model)
            .field("endpoint", &self.endpoint)
            .field("api_key_env", &self.api_key_env)
            .field("allow_network", &self.allow_network)
            .field("timeout", &self.timeout)
            .field("max_response_bytes", &self.max_response_bytes)
            .finish()
    }
}

impl AiConfig {
    pub fn from_env() -> Self {
        Self::from_lookup(|name| env::var(name).ok())
    }

    /// Testable constructor. The lookup function receives a variable name.
    pub fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Self {
        let mut config = Self::default();
        if let Some(value) = lookup("VANTYX_AI_PROVIDER")
            && !value.trim().is_empty()
        {
            config.provider = value;
        }
        if let Some(value) = lookup("VANTYX_AI_MODEL")
            && !value.trim().is_empty()
        {
            config.model = value;
        }
        if let Some(value) = lookup("VANTYX_AI_ENDPOINT")
            && !value.trim().is_empty()
        {
            config.endpoint = value;
        }
        if let Some(value) = lookup("VANTYX_AI_API_KEY_ENV")
            && !value.trim().is_empty()
        {
            config.api_key_env = value;
        }
        if let Some(value) = lookup("VANTYX_AI_ALLOW_NETWORK") {
            config.allow_network = parse_bool(&value).unwrap_or(false);
        }
        if let Some(value) = lookup("VANTYX_AI_TIMEOUT_MS")
            && let Some(milliseconds) = parse_positive_u64(&value)
        {
            config.timeout = Duration::from_millis(milliseconds.clamp(1, 3_600_000));
        }
        if let Some(value) = lookup("VANTYX_AI_MAX_RESPONSE_BYTES")
            && let Some(bytes) = parse_positive_u64(&value)
        {
            config.max_response_bytes = usize::try_from(bytes)
                .unwrap_or(usize::MAX)
                .clamp(1024, 64 * 1024 * 1024);
        }
        config
    }

    pub fn network_policy(&self) -> NetworkPolicy {
        if self.allow_network {
            NetworkPolicy::with_network()
        } else {
            NetworkPolicy::local_only()
        }
    }

    /// Resolves the API key from the configured environment variable. An unset
    /// or empty variable is `None`; non-UTF-8 values are a configuration error.
    pub fn api_key(&self) -> Result<Option<String>, AiError> {
        match env::var(&self.api_key_env) {
            Ok(value) if value.trim().is_empty() => Ok(None),
            Ok(value) => Ok(Some(value)),
            Err(env::VarError::NotPresent) => Ok(None),
            Err(env::VarError::NotUnicode(_)) => Err(AiError::new(
                AI_E_CONFIG,
                format!(
                    "environment variable `{}` is not valid UTF-8",
                    self.api_key_env
                ),
            )),
        }
    }

    /// Reports whether a key is present without reading or printing it.
    pub fn api_key_present(&self) -> bool {
        env::var_os(&self.api_key_env).is_some_and(|value| !value.is_empty())
    }
}

fn parse_bool(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

fn parse_positive_u64(value: &str) -> Option<u64> {
    value.trim().parse::<u64>().ok().filter(|value| *value > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn lookup(values: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<String, String> = values
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect();
        move |name| map.get(name).cloned()
    }

    #[test]
    fn defaults_are_local_and_secret_free() {
        let config = AiConfig::from_lookup(|_| None);
        assert_eq!(config.provider, "mock");
        assert_eq!(config.endpoint, DEFAULT_ENDPOINT);
        assert!(!config.allow_network);
        assert!(!format!("{config:?}").contains("sk-"));
    }

    #[test]
    fn reads_environment_style_values() {
        let config = AiConfig::from_lookup(lookup(&[
            ("VANTYX_AI_PROVIDER", "openai"),
            ("VANTYX_AI_MODEL", "qwen"),
            ("VANTYX_AI_ENDPOINT", "https://example.com/v1"),
            ("VANTYX_AI_API_KEY_ENV", "MY_KEY"),
            ("VANTYX_AI_ALLOW_NETWORK", "yes"),
            ("VANTYX_AI_TIMEOUT_MS", "2500"),
            ("VANTYX_AI_MAX_RESPONSE_BYTES", "4096"),
        ]));
        assert_eq!(config.provider, "openai");
        assert_eq!(config.model, "qwen");
        assert_eq!(config.api_key_env, "MY_KEY");
        assert!(config.allow_network);
        assert_eq!(config.timeout, Duration::from_millis(2500));
        assert_eq!(config.max_response_bytes, 4096);
    }

    #[test]
    fn invalid_numbers_keep_defaults() {
        let config = AiConfig::from_lookup(lookup(&[
            ("VANTYX_AI_TIMEOUT_MS", "nope"),
            ("VANTYX_AI_MAX_RESPONSE_BYTES", "0"),
        ]));
        assert_eq!(config.timeout, DEFAULT_TIMEOUT);
        assert_eq!(config.max_response_bytes, DEFAULT_MAX_RESPONSE_BYTES);
    }

    #[test]
    fn clamps_extreme_limits() {
        let config = AiConfig::from_lookup(lookup(&[
            ("VANTYX_AI_TIMEOUT_MS", "999999999"),
            ("VANTYX_AI_MAX_RESPONSE_BYTES", "999999999"),
        ]));
        assert_eq!(config.timeout, Duration::from_secs(3600));
        assert_eq!(config.max_response_bytes, 64 * 1024 * 1024);
    }

    #[test]
    fn network_policy_follows_the_flag() {
        let local = AiConfig::default();
        assert!(!local.network_policy().allows_network());
        let remote = AiConfig {
            allow_network: true,
            ..AiConfig::default()
        };
        assert!(remote.network_policy().allows_network());
    }
}
