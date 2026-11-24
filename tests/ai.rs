//! End-to-end tests for the host-level AI surface.
//!
//! These tests are offline: the only HTTP server is a loopback stub started by
//! the test itself. No model, API key, or internet access is required.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use vantyx::ai::{
    ChatRequest, HttpTransport, Message, NetworkPolicy, OpenAiProvider, Provider,
    ProviderCapabilities,
};

struct Run {
    status: i32,
    stdout: String,
    stderr: String,
}

fn vantyx(args: &[&str], stdin: Option<&str>, envs: &[(&str, &str)]) -> Run {
    let mut command = Command::new(env!("CARGO_BIN_EXE_vantyx"));
    command
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if stdin.is_some() {
        command.stdin(Stdio::piped());
    } else {
        command.stdin(Stdio::null());
    }
    for (key, value) in envs {
        command.env(key, value);
    }
    let mut child = command.spawn().expect("spawn vantyx");
    if let Some(input) = stdin {
        child
            .stdin
            .take()
            .expect("stdin")
            .write_all(input.as_bytes())
            .expect("write stdin");
    }
    let output = child.wait_with_output().expect("wait for vantyx");
    Run {
        status: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    }
}

fn serve_openai_once(response_body: &str) -> (String, thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let body = response_body.to_string();
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut request = String::new();
        let mut content_length = 0;
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            if line == "\r\n" {
                break;
            }
            if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                content_length = value.trim().parse().unwrap_or(0);
            }
            request.push_str(&line);
        }
        let mut request_body = vec![0_u8; content_length];
        reader.read_exact(&mut request_body).unwrap();
        request.push_str(&String::from_utf8_lossy(&request_body));
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream.write_all(response.as_bytes()).unwrap();
        stream.flush().unwrap();
        request
    });
    (format!("http://{address}/v1"), handle)
}

fn temp_path(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "vantyx-ai-test-{}-{stamp}-{name}",
        std::process::id()
    ))
}

#[test]
fn providers_lists_the_offline_and_http_options() {
    let run = vantyx(&["ai", "providers"], None, &[]);
    assert_eq!(run.status, 0, "{}", run.stderr);
    assert!(run.stdout.contains("mock"));
    assert!(run.stdout.contains("echo"));
    assert!(run.stdout.contains("openai"));
    assert!(run.stdout.contains("loopback only"));
}

#[test]
fn mock_chat_is_deterministic_and_offline() {
    let run = vantyx(
        &[
            "ai",
            "chat",
            "--provider",
            "mock",
            "--model",
            "demo",
            "hello",
        ],
        None,
        &[],
    );
    assert_eq!(run.status, 0, "{}", run.stderr);
    assert!(run.stdout.contains("[mock]"));
    assert!(run.stdout.contains("no network call"));
}

#[test]
fn echo_chat_returns_the_user_text() {
    let run = vantyx(
        &[
            "ai",
            "chat",
            "--provider",
            "echo",
            "--model",
            "demo",
            "hello",
        ],
        None,
        &[],
    );
    assert_eq!(run.status, 0, "{}", run.stderr);
    assert_eq!(run.stdout.trim(), "hello");
}

#[test]
fn chat_can_read_a_prompt_from_stdin() {
    let run = vantyx(
        &["ai", "chat", "--provider", "echo", "--model", "demo", "-"],
        Some("from stdin\n"),
        &[],
    );
    assert_eq!(run.status, 0, "{}", run.stderr);
    assert_eq!(run.stdout.trim(), "from stdin");
}

#[test]
fn json_output_includes_usage_and_finish_reason() {
    let run = vantyx(
        &[
            "ai",
            "chat",
            "--provider",
            "mock",
            "--model",
            "demo",
            "--json",
            "hello",
        ],
        None,
        &[],
    );
    assert_eq!(run.status, 0, "{}", run.stderr);
    let value = vantyx::ai::json::parse(run.stdout.trim()).unwrap();
    assert_eq!(value.get("model").unwrap().as_str(), Some("demo"));
    assert_eq!(value.get("finish_reason").unwrap().as_str(), Some("stop"));
    assert!(
        value
            .get("usage")
            .unwrap()
            .get("total_tokens")
            .unwrap()
            .as_u64()
            .unwrap()
            > 0
    );
}

#[test]
fn tokens_are_labelled_as_an_estimate() {
    let run = vantyx(&["ai", "tokens", "hello world"], None, &[]);
    assert_eq!(run.status, 0, "{}", run.stderr);
    assert!(run.stdout.contains("estimate"));
    assert!(run.stdout.contains("not a model tokenizer"));
}

#[test]
fn remote_https_requires_an_explicit_flag() {
    let run = vantyx(
        &[
            "ai",
            "chat",
            "--provider",
            "openai",
            "--model",
            "m",
            "--endpoint",
            "https://example.com/v1",
            "hi",
        ],
        None,
        &[],
    );
    assert_eq!(run.status, 1);
    assert!(run.stderr.contains("AI_E_NETWORK_DISABLED"));
    assert!(run.stderr.contains("--allow-network"));
}

#[test]
fn doctor_reports_mock_without_network() {
    let run = vantyx(&["ai", "doctor", "--provider", "mock"], None, &[]);
    assert_eq!(run.status, 0, "{}", run.stderr);
    assert!(run.stdout.contains("status: ok"));
    assert!(run.stdout.contains("offline"));
}

#[test]
fn unknown_providers_fail_clearly() {
    let run = vantyx(
        &["ai", "chat", "--provider", "nope", "--model", "m", "hi"],
        None,
        &[],
    );
    assert_eq!(run.status, 1);
    assert!(run.stderr.contains("unknown provider"));
}

#[test]
fn invalid_temperature_is_rejected_before_transport() {
    let run = vantyx(
        &[
            "ai",
            "chat",
            "--provider",
            "mock",
            "--model",
            "m",
            "--temperature",
            "3",
            "hi",
        ],
        None,
        &[],
    );
    assert_eq!(run.status, 1);
    assert!(run.stderr.contains("AI_E_INVALID_REQUEST"));
}

#[test]
fn metrics_are_opt_in_and_never_include_the_prompt() {
    let path = temp_path("metrics.jsonl");
    let run = vantyx(
        &[
            "ai",
            "chat",
            "--provider",
            "mock",
            "--model",
            "m",
            "--metrics",
            path.to_str().unwrap(),
            "super-secret-prompt",
        ],
        None,
        &[],
    );
    assert_eq!(run.status, 0, "{}", run.stderr);
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(!text.contains("super-secret-prompt"));
    let event = vantyx::ai::json::parse(text.lines().next().unwrap()).unwrap();
    assert_eq!(event.get("operation").unwrap().as_str(), Some("chat"));
    assert_eq!(event.get("ok").unwrap().as_bool(), Some(true));
    let _ = std::fs::remove_file(path);
}

#[test]
fn openai_adapter_round_trips_over_loopback() {
    let body = r#"{"id":"chat-1","model":"demo","choices":[{"message":{"role":"assistant","content":"from the server"},"finish_reason":"stop"}],"usage":{"prompt_tokens":3,"completion_tokens":4,"total_tokens":7}}"#;
    let (endpoint, handle) = serve_openai_once(body);
    let transport = HttpTransport::new(NetworkPolicy::local_only());
    let provider = OpenAiProvider::new(endpoint, Some("test-key".into()), Box::new(transport));
    let request = ChatRequest::new("demo", vec![Message::user("hello")]);
    let mut budget = vantyx::ai::Budget::default();
    let response = provider.chat(&request, &mut budget).unwrap();
    assert_eq!(response.message.text_content(), "from the server");
    assert_eq!(response.usage.total_tokens, 7);
    let captured = handle.join().unwrap();
    assert!(captured.starts_with("POST /v1/chat/completions HTTP/1.1"));
    assert!(captured.contains("Authorization: Bearer test-key"));
    assert!(captured.contains(r#""model":"demo""#));
}

#[test]
fn openai_chat_cli_uses_a_loopback_server() {
    let body = r#"{"model":"demo","choices":[{"message":{"role":"assistant","content":"hello from server"},"finish_reason":"stop"}],"usage":{"prompt_tokens":2,"completion_tokens":3,"total_tokens":5}}"#;
    let (endpoint, handle) = serve_openai_once(body);
    let run = vantyx(
        &[
            "ai",
            "chat",
            "--provider",
            "openai",
            "--model",
            "demo",
            "--endpoint",
            &endpoint,
            "--api-key-env",
            "VANTYX_TEST_KEY",
            "hello",
        ],
        None,
        &[("VANTYX_TEST_KEY", "local-test-key")],
    );
    assert_eq!(run.status, 0, "{}", run.stderr);
    assert_eq!(run.stdout.trim(), "hello from server");
    let captured = handle.join().unwrap();
    assert!(captured.contains("Authorization: Bearer local-test-key"));
}

#[test]
fn openai_capabilities_do_not_claim_unimplemented_streaming() {
    let capabilities = ProviderCapabilities::openai_compatible();
    assert!(capabilities.chat);
    assert!(!capabilities.streaming);
    assert!(!capabilities.embeddings);
}

const PERSON_SCHEMA: &str = r#"{"type":"object","required":["name"],"properties":{"name":{"type":"string"},"age":{"type":"integer","minimum":0}},"additionalProperties":false}"#;

#[test]
fn extract_validates_mock_output_offline() {
    let run = vantyx(
        &[
            "ai",
            "extract",
            "--provider",
            "mock",
            "--model",
            "demo",
            "--schema",
            PERSON_SCHEMA,
            "--response",
            r#"{"name":"Ada","age":36}"#,
            "extract a person",
        ],
        None,
        &[],
    );
    assert_eq!(run.status, 0, "{}", run.stderr);
    let value = vantyx::ai::json::parse(run.stdout.trim()).unwrap();
    assert_eq!(value.get("name").unwrap().as_str(), Some("Ada"));
    assert_eq!(value.get("age").unwrap().as_u64(), Some(36));
}

#[test]
fn extract_repairs_invalid_output_within_the_limit() {
    let run = vantyx(
        &[
            "ai",
            "extract",
            "--provider",
            "mock",
            "--model",
            "demo",
            "--schema",
            PERSON_SCHEMA,
            "--response",
            "not json",
            "--response",
            r#"{"name":"Ada"}"#,
            "--repair-attempts",
            "1",
            "--json",
            "extract a person",
        ],
        None,
        &[],
    );
    assert_eq!(run.status, 0, "{}", run.stderr);
    let envelope = vantyx::ai::json::parse(run.stdout.trim()).unwrap();
    assert_eq!(envelope.get("attempts").unwrap().as_u64(), Some(2));
    assert_eq!(
        envelope.get("value").unwrap().get("name").unwrap().as_str(),
        Some("Ada")
    );
}

#[test]
fn extract_fails_after_the_repair_limit_with_a_schema_error() {
    let run = vantyx(
        &[
            "ai",
            "extract",
            "--provider",
            "mock",
            "--model",
            "demo",
            "--schema",
            PERSON_SCHEMA,
            "--response",
            r#"{"name":42}"#,
            "--response",
            r#"{"name":false}"#,
            "--repair-attempts",
            "1",
            "extract a person",
        ],
        None,
        &[],
    );
    assert_eq!(run.status, 1);
    assert!(run.stderr.contains("AI_E_SCHEMA"), "{}", run.stderr);
    assert!(run.stderr.contains("after 2 attempts"), "{}", run.stderr);
}

#[test]
fn extract_reads_a_schema_file() {
    let path = temp_path("schema.json");
    std::fs::write(&path, PERSON_SCHEMA).unwrap();
    let run = vantyx(
        &[
            "ai",
            "extract",
            "--provider",
            "mock",
            "--model",
            "demo",
            "--schema",
            path.to_str().unwrap(),
            "--response",
            r#"{"name":"Ada"}"#,
            "extract a person",
        ],
        None,
        &[],
    );
    let _ = std::fs::remove_file(&path);
    assert_eq!(run.status, 0, "{}", run.stderr);
    let value = vantyx::ai::json::parse(run.stdout.trim()).unwrap();
    assert_eq!(value.get("name").unwrap().as_str(), Some("Ada"));
}

#[test]
fn extract_requires_a_schema_and_a_mock_fixture() {
    let missing_schema = vantyx(
        &[
            "ai",
            "extract",
            "--provider",
            "mock",
            "--model",
            "demo",
            "hi",
        ],
        None,
        &[],
    );
    assert_eq!(missing_schema.status, 64);
    assert!(missing_schema.stderr.contains("--schema"));

    let missing_response = vantyx(
        &[
            "ai",
            "extract",
            "--provider",
            "mock",
            "--model",
            "demo",
            "--schema",
            PERSON_SCHEMA,
            "hi",
        ],
        None,
        &[],
    );
    assert_eq!(missing_response.status, 64);
    assert!(missing_response.stderr.contains("--response"));
}

#[test]
fn extract_validates_a_loopback_openai_response() {
    let body = r#"{"model":"demo","choices":[{"message":{"role":"assistant","content":"{\"name\":\"Ada\",\"age\":36}"},"finish_reason":"stop"}],"usage":{"prompt_tokens":4,"completion_tokens":6,"total_tokens":10}}"#;
    let (endpoint, handle) = serve_openai_once(body);
    let run = vantyx(
        &[
            "ai",
            "extract",
            "--provider",
            "openai",
            "--model",
            "demo",
            "--endpoint",
            &endpoint,
            "--schema",
            PERSON_SCHEMA,
            "--json",
            "extract a person",
        ],
        None,
        &[],
    );
    assert_eq!(run.status, 0, "{}", run.stderr);
    let envelope = vantyx::ai::json::parse(run.stdout.trim()).unwrap();
    assert_eq!(
        envelope.get("value").unwrap().get("name").unwrap().as_str(),
        Some("Ada")
    );
    assert_eq!(
        envelope
            .get("usage")
            .unwrap()
            .get("total_tokens")
            .unwrap()
            .as_u64(),
        Some(10)
    );
    let captured = handle.join().unwrap();
    assert!(captured.starts_with("POST /v1/chat/completions HTTP/1.1"));
    assert!(captured.contains(r#""response_format":{"type":"json_object"}"#));
}
