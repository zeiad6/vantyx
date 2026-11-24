//! End-to-end tests for the host-level AI surface.
//!
//! These tests are offline: the only HTTP server is a loopback stub started by
//! the test itself. No model, API key, or internet access is required.

use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use vantyx::ai::{
    ChatRequest, EmbeddingRequest, HttpTransport, Message, NetworkPolicy, OpenAiProvider, Provider,
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

fn serve_sse_once(chunks: Vec<String>) -> (String, thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
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
        let body = chunks.concat();
        let head = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        stream.write_all(head.as_bytes()).unwrap();
        stream.flush().unwrap();
        for chunk in &chunks {
            stream.write_all(chunk.as_bytes()).unwrap();
            stream.flush().unwrap();
            thread::sleep(Duration::from_millis(10));
        }
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
fn streamed_chat_prints_deltas_and_sends_a_stream_request() {
    let (endpoint, handle) = serve_sse_once(vec![
        "data: {\"choices\":[{\"delta\":{\"content\":\"hel\"},\"finish_reason\":null}]}\n\n"
            .to_string(),
        "data: {\"choices\":[{\"delta\":{\"content\":\"lo \"},\"finish_reason\":null}]}\n\n"
            .to_string(),
        "data: {\"choices\":[{\"delta\":{\"content\":\"world\"},\"finish_reason\":\"stop\"}]}\n\n"
            .to_string(),
        "data: {\"choices\":[],\"usage\":{\"prompt_tokens\":4,\"completion_tokens\":3,\"total_tokens\":7}}\n\n"
            .to_string(),
        "data: [DONE]\n\n".to_string(),
    ]);
    let run = vantyx(
        &[
            "ai",
            "chat",
            "--provider",
            "openai",
            "--endpoint",
            &endpoint,
            "--model",
            "demo",
            "--stream",
            "hello",
        ],
        None,
        &[],
    );
    assert_eq!(run.status, 0, "{}", run.stderr);
    assert_eq!(run.stdout, "hello world\n");
    let request = handle.join().unwrap();
    assert!(request.contains("Accept: text/event-stream"));
    assert!(request.contains("\"stream\":true"));
    assert!(request.contains("\"include_usage\":true"));
    assert!(request.contains("\"stream_options\""));
}

#[test]
fn stream_rejects_non_streaming_providers_and_json() {
    let run = vantyx(
        &["ai", "chat", "--provider", "mock", "--stream", "hello"],
        None,
        &[],
    );
    assert_eq!(run.status, 64);
    assert!(run.stderr.contains("does not support streaming"));

    let run = vantyx(
        &[
            "ai",
            "chat",
            "--provider",
            "openai",
            "--endpoint",
            "http://127.0.0.1:1/v1",
            "--model",
            "demo",
            "--stream",
            "--json",
            "hello",
        ],
        None,
        &[],
    );
    assert_eq!(run.status, 64);
    assert!(run.stderr.contains("cannot be combined"));
}

#[test]
fn streamed_chat_reports_provider_error_frames() {
    let (endpoint, handle) = serve_sse_once(vec![
        "data: {\"error\":{\"message\":\"bad request\"}}\n\n".to_string(),
    ]);
    let run = vantyx(
        &[
            "ai",
            "chat",
            "--provider",
            "openai",
            "--endpoint",
            &endpoint,
            "--model",
            "demo",
            "--stream",
            "hello",
        ],
        None,
        &[],
    );
    assert_eq!(run.status, 1);
    assert!(run.stderr.contains("provider error"), "{}", run.stderr);
    handle.join().unwrap();
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
fn openai_capabilities_claim_text_streaming() {
    let capabilities = ProviderCapabilities::openai_compatible();
    assert!(capabilities.chat);
    assert!(capabilities.streaming);
    // Tool calls remain available in non-streaming mode; streaming tool calls
    // are rejected explicitly by the adapter.
    assert!(capabilities.tools);
    assert!(capabilities.embeddings);
}

#[test]
fn openai_embedding_adapter_round_trips_over_loopback() {
    let body = r#"{"model":"embed-demo","data":[{"index":1,"embedding":[0.0,1.0]},{"index":0,"embedding":[1.0,0.0]}],"usage":{"prompt_tokens":4,"total_tokens":4}}"#;
    let (endpoint, handle) = serve_openai_once(body);
    let transport = HttpTransport::new(NetworkPolicy::local_only());
    let provider = OpenAiProvider::new(endpoint, Some("test-key".into()), Box::new(transport));
    let mut request =
        EmbeddingRequest::new("embed-demo", vec!["alpha".to_string(), "beta".to_string()]);
    request.dimensions = Some(2);
    let mut budget = vantyx::ai::Budget::default();
    let response = provider.embed(&request, &mut budget).unwrap();
    assert_eq!(response.vectors[0].values(), &[1.0, 0.0]);
    assert_eq!(response.vectors[1].values(), &[0.0, 1.0]);
    assert_eq!(response.usage.total_tokens, 4);
    let captured = handle.join().unwrap();
    assert!(captured.starts_with("POST /v1/embeddings HTTP/1.1"));
    assert!(captured.contains("Authorization: Bearer test-key"));
    assert!(captured.contains(r#""input":["alpha","beta"]"#));
    assert!(captured.contains(r#""dimensions":2"#));
}

#[test]
fn openai_embed_cli_uses_a_loopback_server() {
    let body = r#"{"model":"embed-demo","data":[{"index":0,"embedding":[0.5,0.5,0.5,0.5]}],"usage":{"prompt_tokens":2,"total_tokens":2}}"#;
    let (endpoint, handle) = serve_openai_once(body);
    let run = vantyx(
        &[
            "ai",
            "embed",
            "--provider",
            "openai",
            "--model",
            "embed-demo",
            "--endpoint",
            &endpoint,
            "--dimensions",
            "4",
            "--json",
            "hello",
        ],
        None,
        &[],
    );
    assert_eq!(run.status, 0, "{}", run.stderr);
    let value = vantyx::ai::json::parse(run.stdout.trim()).unwrap();
    assert_eq!(value.get("provider").unwrap().as_str(), Some("openai"));
    assert_eq!(value.get("dimensions").unwrap().as_u64(), Some(4));
    assert_eq!(value.get("input_count").unwrap().as_u64(), Some(1));
    assert_eq!(
        value.get("vectors").unwrap().as_array().unwrap()[0]
            .as_array()
            .unwrap()
            .len(),
        4
    );
    let captured = handle.join().unwrap();
    assert!(captured.starts_with("POST /v1/embeddings HTTP/1.1"));
    assert!(captured.contains(r#""model":"embed-demo""#));
}

#[test]
fn mock_embed_cli_is_deterministic_and_offline() {
    let first = vantyx(
        &[
            "ai",
            "embed",
            "--provider",
            "mock",
            "--dimensions",
            "8",
            "--json",
            "hello world",
        ],
        None,
        &[],
    );
    assert_eq!(first.status, 0, "{}", first.stderr);
    let second = vantyx(
        &[
            "ai",
            "embed",
            "--provider",
            "mock",
            "--dimensions",
            "8",
            "--json",
            "hello world",
        ],
        None,
        &[],
    );
    assert_eq!(second.status, 0, "{}", second.stderr);
    assert_eq!(first.stdout, second.stdout);
    let value = vantyx::ai::json::parse(first.stdout.trim()).unwrap();
    assert_eq!(value.get("provider").unwrap().as_str(), Some("hash"));
    assert_eq!(value.get("dimensions").unwrap().as_u64(), Some(8));
    assert_eq!(value.get("input_count").unwrap().as_u64(), Some(1));
    let vector = value.get("vectors").unwrap().as_array().unwrap()[0]
        .as_array()
        .unwrap();
    assert_eq!(vector.len(), 8);
    let norm: f64 = vector
        .iter()
        .map(|value| {
            let component = value.as_f64().unwrap();
            component * component
        })
        .sum::<f64>()
        .sqrt();
    assert!((norm - 1.0).abs() < 1e-5);
}

#[test]
fn mock_embed_cli_reads_a_batch_from_stdin() {
    let run = vantyx(
        &[
            "ai",
            "embed",
            "--provider",
            "hash",
            "--dimensions",
            "4",
            "--json",
            "-",
        ],
        Some("alpha\n\nbeta\n"),
        &[],
    );
    assert_eq!(run.status, 0, "{}", run.stderr);
    let value = vantyx::ai::json::parse(run.stdout.trim()).unwrap();
    assert_eq!(value.get("input_count").unwrap().as_u64(), Some(2));
    assert_eq!(value.get("vectors").unwrap().as_array().unwrap().len(), 2);
}

#[test]
fn echo_embed_provider_fails_clearly() {
    let run = vantyx(&["ai", "embed", "--provider", "echo", "hello"], None, &[]);
    assert_eq!(run.status, 64);
    assert!(run.stderr.contains("has no embeddings"), "{}", run.stderr);
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

#[test]
fn rag_index_search_and_eval_are_offline_and_cited() {
    let root = temp_path("rag-corpus");
    fs::create_dir_all(root.join("sub")).unwrap();
    fs::write(
        root.join("rust.md"),
        "# Rust\nRust enforces memory safety without a garbage collector.\nOwnership prevents use-after-free.\n",
    )
    .unwrap();
    fs::write(
        root.join("sub/python.md"),
        "# Python\nPython is used for data science and automation.\n",
    )
    .unwrap();
    let index = temp_path("rag-index.json");
    let run = vantyx(
        &[
            "ai",
            "index",
            "--root",
            root.to_str().unwrap(),
            "--out",
            index.to_str().unwrap(),
            "--provider",
            "hash",
            "--dimensions",
            "24",
            "--json",
        ],
        None,
        &[],
    );
    assert_eq!(run.status, 0, "{}", run.stderr);
    let value = vantyx::ai::parse(run.stdout.trim()).expect("index JSON");
    assert_eq!(
        value.get("chunks").and_then(|value| value.as_u64()),
        Some(2)
    );
    assert_eq!(
        value.get("dimensions").and_then(|value| value.as_u64()),
        Some(24)
    );
    assert_eq!(
        value.get("mode").and_then(|value| value.as_str()),
        Some("semantic")
    );
    assert!(index.is_file());

    let search = vantyx(
        &[
            "ai",
            "search",
            "--index",
            index.to_str().unwrap(),
            "--query",
            "memory safety",
            "--mode",
            "lexical",
            "--top-k",
            "2",
            "--json",
        ],
        None,
        &[],
    );
    assert_eq!(search.status, 0, "{}", search.stderr);
    let value = vantyx::ai::parse(search.stdout.trim()).expect("search JSON");
    let hits = value
        .get("hits")
        .and_then(|value| value.as_array())
        .expect("hits");
    assert!(!hits.is_empty());
    assert_eq!(
        hits[0].get("source").and_then(|value| value.as_str()),
        Some("rust.md")
    );
    assert_eq!(
        hits[0].get("start_line").and_then(|value| value.as_u64()),
        Some(1)
    );
    assert!(
        hits[0]
            .get("text")
            .and_then(|value| value.as_str())
            .expect("text")
            .contains("memory safety")
    );

    let filtered = vantyx(
        &[
            "ai",
            "search",
            "--index",
            index.to_str().unwrap(),
            "--query",
            "data science",
            "--mode",
            "lexical",
            "--source-prefix",
            "sub/",
            "--top-k",
            "2",
            "--json",
        ],
        None,
        &[],
    );
    assert_eq!(filtered.status, 0, "{}", filtered.stderr);
    let value = vantyx::ai::parse(filtered.stdout.trim()).expect("filtered search JSON");
    let hits = value
        .get("hits")
        .and_then(|value| value.as_array())
        .expect("filtered hits");
    assert!(!hits.is_empty());
    assert!(hits.iter().all(|hit| {
        hit.get("source")
            .and_then(|value| value.as_str())
            .is_some_and(|source| source.starts_with("sub/"))
    }));

    let fixture = temp_path("rag-eval.json");
    fs::write(
        &fixture,
        r#"{"queries":[{"query":"memory safety","relevant":["rust.md"]}]}"#,
    )
    .unwrap();
    let eval = vantyx(
        &[
            "ai",
            "eval",
            "--index",
            index.to_str().unwrap(),
            "--fixture",
            fixture.to_str().unwrap(),
            "--mode",
            "lexical",
            "--top-k",
            "2",
            "--json",
        ],
        None,
        &[],
    );
    assert_eq!(eval.status, 0, "{}", eval.stderr);
    let value = vantyx::ai::parse(eval.stdout.trim()).expect("eval JSON");
    assert_eq!(
        value.get("recall_at_k").and_then(|value| value.as_f64()),
        Some(1.0)
    );
    assert_eq!(value.get("mrr").and_then(|value| value.as_f64()), Some(1.0));
    assert_eq!(
        value.get("queries").and_then(|value| value.as_u64()),
        Some(1)
    );

    let _ = fs::remove_dir_all(&root);
    let _ = fs::remove_file(&index);
    let _ = fs::remove_file(&fixture);
}

#[test]
fn rag_diverse_search_deduplicates_and_marks_reranking() {
    let root = temp_path("rag-diverse-corpus");
    fs::create_dir_all(&root).unwrap();
    let duplicate = "# Rust\nRust enforces memory safety without a garbage collector.\n";
    fs::write(root.join("a.md"), duplicate).unwrap();
    fs::write(root.join("b.md"), duplicate).unwrap();
    fs::write(
        root.join("c.md"),
        "# Rust\nMemory safety depends on ownership and borrowing rules.\n",
    )
    .unwrap();
    fs::write(
        root.join("d.md"),
        "# Python\nPython is used for data science and automation.\n",
    )
    .unwrap();
    let index = temp_path("rag-diverse-index.json");
    let root_arg = root.to_str().unwrap();
    let index_arg = index.to_str().unwrap();

    let build = vantyx(
        &[
            "ai", "index", "--root", root_arg, "--out", index_arg, "--mode", "lexical", "--json",
        ],
        None,
        &[],
    );
    assert_eq!(build.status, 0, "{}", build.stderr);

    let baseline = vantyx(
        &[
            "ai",
            "search",
            "--index",
            index_arg,
            "--query",
            "memory safety",
            "--mode",
            "lexical",
            "--top-k",
            "3",
            "--json",
        ],
        None,
        &[],
    );
    assert_eq!(baseline.status, 0, "{}", baseline.stderr);
    let baseline_value = vantyx::ai::parse(baseline.stdout.trim()).expect("baseline JSON");
    assert_eq!(
        baseline_value
            .get("rerank")
            .and_then(|value| value.as_str()),
        Some("none")
    );
    let baseline_hits = baseline_value
        .get("hits")
        .and_then(|value| value.as_array())
        .expect("baseline hits");
    assert_eq!(baseline_hits.len(), 3);
    assert!(
        baseline_hits
            .iter()
            .filter_map(|hit| hit.get("text").and_then(|value| value.as_str()))
            .any(|text| text.contains("garbage collector"))
    );

    let diverse = vantyx(
        &[
            "ai",
            "search",
            "--index",
            index_arg,
            "--query",
            "memory safety",
            "--mode",
            "lexical",
            "--top-k",
            "3",
            "--diverse",
            "--json",
        ],
        None,
        &[],
    );
    assert_eq!(diverse.status, 0, "{}", diverse.stderr);
    let diverse_value = vantyx::ai::parse(diverse.stdout.trim()).expect("diverse JSON");
    assert_eq!(
        diverse_value.get("rerank").and_then(|value| value.as_str()),
        Some("mmr")
    );
    let diverse_hits = diverse_value
        .get("hits")
        .and_then(|value| value.as_array())
        .expect("diverse hits");
    assert_eq!(diverse_hits.len(), 2);
    let texts: Vec<&str> = diverse_hits
        .iter()
        .filter_map(|hit| hit.get("text").and_then(|value| value.as_str()))
        .collect();
    assert_eq!(texts.len(), 2);
    assert_ne!(texts[0].trim(), texts[1].trim());
    assert_eq!(
        diverse_hits[0]
            .get("source")
            .and_then(|value| value.as_str()),
        Some("a.md")
    );

    let rejected = vantyx(
        &[
            "ai",
            "index",
            "--root",
            root_arg,
            "--out",
            index_arg,
            "--diverse",
        ],
        None,
        &[],
    );
    assert_eq!(rejected.status, 64);
    assert!(rejected.stderr.contains("--diverse"), "{}", rejected.stderr);

    let _ = fs::remove_dir_all(&root);
    let _ = fs::remove_file(&index);
}

#[test]
fn rag_incremental_index_reuses_vectors_and_prunes_deleted_documents() {
    let root = temp_path("rag-incremental-corpus");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("a.md"), "# A\nRust enforces memory safety.\n").unwrap();
    fs::write(root.join("b.md"), "# B\nPython is used for data science.\n").unwrap();
    fs::write(
        root.join("keep.md"),
        "# Keep\nWebAssembly runs in browsers.\n",
    )
    .unwrap();
    let index = temp_path("rag-incremental-index.json");
    let root_arg = root.to_str().unwrap();
    let index_arg = index.to_str().unwrap();

    let first = vantyx(
        &[
            "ai",
            "index",
            "--root",
            root_arg,
            "--out",
            index_arg,
            "--provider",
            "hash",
            "--dimensions",
            "24",
            "--json",
        ],
        None,
        &[],
    );
    assert_eq!(first.status, 0, "{}", first.stderr);
    let value = vantyx::ai::parse(first.stdout.trim()).expect("initial index JSON");
    assert_eq!(
        value.get("incremental").and_then(|value| value.as_bool()),
        Some(false)
    );
    let previous_chunks = value
        .get("chunks")
        .and_then(|value| value.as_u64())
        .expect("initial chunk count");
    assert_eq!(previous_chunks, 3);

    fs::write(
        root.join("a.md"),
        "# A\nRust enforces memory safety without a garbage collector.\n",
    )
    .unwrap();
    fs::write(root.join("c.md"), "# C\nGo compiles quickly.\n").unwrap();
    fs::remove_file(root.join("b.md")).unwrap();

    let second = vantyx(
        &[
            "ai",
            "index",
            "--root",
            root_arg,
            "--out",
            index_arg,
            "--incremental",
            "--json",
        ],
        None,
        &[],
    );
    assert_eq!(second.status, 0, "{}", second.stderr);
    let value = vantyx::ai::parse(second.stdout.trim()).expect("incremental index JSON");
    assert_eq!(
        value.get("incremental").and_then(|value| value.as_bool()),
        Some(true)
    );
    assert_eq!(
        value
            .get("previous_chunks")
            .and_then(|value| value.as_u64()),
        Some(previous_chunks)
    );
    assert_eq!(
        value.get("reused_chunks").and_then(|value| value.as_u64()),
        Some(1)
    );
    assert_eq!(
        value
            .get("embedded_chunks")
            .and_then(|value| value.as_u64()),
        Some(2)
    );
    // The edited `a.md` chunk and the deleted `b.md` chunk are both removed.
    assert_eq!(
        value.get("removed_chunks").and_then(|value| value.as_u64()),
        Some(2)
    );
    assert_eq!(
        value
            .get("removed_sources")
            .and_then(|value| value.as_u64()),
        Some(1)
    );
    assert_eq!(
        value.get("added_sources").and_then(|value| value.as_u64()),
        Some(1)
    );
    assert_eq!(
        value.get("chunks").and_then(|value| value.as_u64()),
        Some(3)
    );

    // A second update with no corpus changes must not need an embedding call.
    let third = vantyx(
        &[
            "ai",
            "index",
            "--root",
            root_arg,
            "--out",
            index_arg,
            "--incremental",
            "--json",
        ],
        None,
        &[],
    );
    assert_eq!(third.status, 0, "{}", third.stderr);
    let value = vantyx::ai::parse(third.stdout.trim()).expect("no-op incremental JSON");
    assert_eq!(
        value.get("reused_chunks").and_then(|value| value.as_u64()),
        Some(3)
    );
    assert_eq!(
        value
            .get("embedded_chunks")
            .and_then(|value| value.as_u64()),
        Some(0)
    );

    let search = vantyx(
        &[
            "ai",
            "search",
            "--index",
            index_arg,
            "--query",
            "Go compiles quickly",
            "--mode",
            "lexical",
            "--top-k",
            "5",
            "--json",
        ],
        None,
        &[],
    );
    assert_eq!(search.status, 0, "{}", search.stderr);
    let value = vantyx::ai::parse(search.stdout.trim()).expect("search JSON");
    let hits = value
        .get("hits")
        .and_then(|value| value.as_array())
        .expect("hits");
    assert!(
        hits.iter()
            .any(|hit| hit.get("source").and_then(|value| value.as_str()) == Some("c.md"))
    );
    assert!(
        !hits
            .iter()
            .any(|hit| hit.get("source").and_then(|value| value.as_str()) == Some("b.md"))
    );

    let missing = temp_path("rag-incremental-missing.json");
    let rejected = vantyx(
        &[
            "ai",
            "index",
            "--root",
            root_arg,
            "--out",
            missing.to_str().unwrap(),
            "--incremental",
        ],
        None,
        &[],
    );
    assert_eq!(rejected.status, 64);
    assert!(
        rejected.stderr.contains("requires an existing index"),
        "{}",
        rejected.stderr
    );

    let changed = vantyx(
        &[
            "ai",
            "index",
            "--root",
            root_arg,
            "--out",
            index_arg,
            "--incremental",
            "--dimensions",
            "16",
        ],
        None,
        &[],
    );
    assert_eq!(changed.status, 64);
    assert!(
        changed.stderr.contains("cannot change dimensions"),
        "{}",
        changed.stderr
    );

    let lexical = vantyx(
        &[
            "ai",
            "index",
            "--root",
            root_arg,
            "--out",
            index_arg,
            "--incremental",
            "--mode",
            "lexical",
        ],
        None,
        &[],
    );
    assert_eq!(lexical.status, 64);
    assert!(
        lexical.stderr.contains("requires a semantic index"),
        "{}",
        lexical.stderr
    );

    let _ = fs::remove_dir_all(&root);
    let _ = fs::remove_file(&index);
    let _ = fs::remove_file(&missing);
}

#[test]
fn lexical_rag_index_rejects_semantic_search_and_bad_limits() {
    let root = temp_path("rag-lexical-corpus");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("a.md"), "Alpha notes about memory safety.\n").unwrap();
    let index = temp_path("rag-lexical-index.json");
    let run = vantyx(
        &[
            "ai",
            "index",
            "--root",
            root.to_str().unwrap(),
            "--out",
            index.to_str().unwrap(),
            "--mode",
            "lexical",
            "--json",
        ],
        None,
        &[],
    );
    assert_eq!(run.status, 0, "{}", run.stderr);
    let value = vantyx::ai::parse(run.stdout.trim()).expect("index JSON");
    assert_eq!(
        value.get("provider").and_then(|value| value.as_str()),
        Some("lexical")
    );
    assert_eq!(
        value.get("dimensions").and_then(|value| value.as_u64()),
        Some(0)
    );

    let search = vantyx(
        &[
            "ai",
            "search",
            "--index",
            index.to_str().unwrap(),
            "--query",
            "memory",
            "--mode",
            "semantic",
        ],
        None,
        &[],
    );
    assert_ne!(search.status, 0);
    assert!(search.stderr.contains("no vectors"), "{}", search.stderr);

    let bad = vantyx(
        &[
            "ai",
            "search",
            "--index",
            index.to_str().unwrap(),
            "--query",
            "memory",
            "--top-k",
            "0",
        ],
        None,
        &[],
    );
    assert_eq!(bad.status, 64);
    assert!(bad.stderr.contains("--top-k"), "{}", bad.stderr);

    let hybrid_build = vantyx(
        &[
            "ai",
            "index",
            "--root",
            root.to_str().unwrap(),
            "--out",
            index.to_str().unwrap(),
            "--mode",
            "hybrid",
        ],
        None,
        &[],
    );
    assert_eq!(hybrid_build.status, 64);
    assert!(hybrid_build.stderr.contains("hybrid is a search mode"));

    let _ = fs::remove_dir_all(&root);
    let _ = fs::remove_file(&index);
}
