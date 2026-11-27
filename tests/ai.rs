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

fn push_model_string(output: &mut Vec<u8>, value: &str) {
    output.extend((value.len() as u64).to_le_bytes());
    output.extend(value.as_bytes());
}

fn minimal_gguf() -> Vec<u8> {
    let mut output = Vec::new();
    output.extend(b"GGUF");
    output.extend(3_u32.to_le_bytes());
    output.extend(1_u64.to_le_bytes());
    output.extend(3_u64.to_le_bytes());

    push_model_string(&mut output, "general.architecture");
    output.extend(8_u32.to_le_bytes());
    push_model_string(&mut output, "llama");

    push_model_string(&mut output, "llama.context_length");
    output.extend(4_u32.to_le_bytes());
    output.extend(4096_u32.to_le_bytes());

    push_model_string(&mut output, "general.file_type");
    output.extend(4_u32.to_le_bytes());
    output.extend(15_u32.to_le_bytes());

    push_model_string(&mut output, "token_embd.weight");
    output.extend(2_u32.to_le_bytes());
    output.extend(4_u64.to_le_bytes());
    output.extend(2_u64.to_le_bytes());
    output.extend(0_u32.to_le_bytes());
    output.extend(0_u64.to_le_bytes());

    while output.len() % 32 != 0 {
        output.push(0);
    }
    output.extend([0_u8; 4 * 2 * 4]);
    output
}

fn minimal_safetensors() -> Vec<u8> {
    let mut header = br#"{"__metadata__":{"format":"pt","architecture":"llama"},"weight":{"dtype":"F32","shape":[2,3],"data_offsets":[0,24]}}"#.to_vec();
    while !header.len().is_multiple_of(8) {
        header.push(b' ');
    }
    let mut output = Vec::new();
    output.extend((header.len() as u64).to_le_bytes());
    output.extend(&header);
    output.extend([0_u8; 24]);
    output
}

fn safetensors_named(name: &str) -> Vec<u8> {
    let mut header = format!(
        r#"{{"__metadata__":{{"format":"pt"}},"{name}":{{"dtype":"F32","shape":[2],"data_offsets":[0,8]}}}}"#
    )
    .into_bytes();
    while !header.len().is_multiple_of(8) {
        header.push(b' ');
    }
    let mut output = Vec::new();
    output.extend((header.len() as u64).to_le_bytes());
    output.extend(&header);
    output.extend([0_u8; 8]);
    output
}

#[test]
fn inspect_reports_gguf_and_safetensors_provenance() {
    let directory = temp_path("inspect-models");
    fs::create_dir_all(&directory).unwrap();
    let gguf = directory.join("tiny.gguf");
    fs::write(&gguf, minimal_gguf()).unwrap();
    let safetensors = directory.join("tiny.safetensors");
    fs::write(&safetensors, minimal_safetensors()).unwrap();

    let run = vantyx(
        &["ai", "inspect", gguf.to_str().unwrap(), "--metadata"],
        None,
        &[],
    );
    assert_eq!(run.status, 0, "{}", run.stderr);
    assert!(
        run.stdout.contains("format: gguf (version 3)"),
        "{}",
        run.stdout
    );
    assert!(run.stdout.contains("architecture: llama"), "{}", run.stdout);
    assert!(run.stdout.contains("parameters: 8"), "{}", run.stdout);
    assert!(
        run.stdout.contains("quantization: MOSTLY_Q4_K_M"),
        "{}",
        run.stdout
    );
    assert!(run.stdout.contains("sha256: "), "{}", run.stdout);
    assert!(
        run.stdout.contains("general.architecture: llama"),
        "{}",
        run.stdout
    );

    let json_run = vantyx(
        &["ai", "inspect", gguf.to_str().unwrap(), "--json"],
        None,
        &[],
    );
    assert_eq!(json_run.status, 0, "{}", json_run.stderr);
    let value = vantyx::ai::parse(json_run.stdout.trim()).expect("inspect JSON");
    assert_eq!(value.get("format").and_then(|v| v.as_str()), Some("gguf"));
    assert_eq!(
        value.get("parameter_count").and_then(|v| v.as_u64()),
        Some(8)
    );
    assert_eq!(value.get("tensor_count").and_then(|v| v.as_u64()), Some(1));
    assert_eq!(
        value.get("sha256").and_then(|v| v.as_str()).map(str::len),
        Some(64)
    );

    let safetensors_run = vantyx(
        &["ai", "inspect", safetensors.to_str().unwrap(), "--json"],
        None,
        &[],
    );
    assert_eq!(safetensors_run.status, 0, "{}", safetensors_run.stderr);
    let value = vantyx::ai::parse(safetensors_run.stdout.trim()).expect("safetensors JSON");
    assert_eq!(
        value.get("format").and_then(|v| v.as_str()),
        Some("safetensors")
    );
    assert_eq!(
        value.get("parameter_count").and_then(|v| v.as_u64()),
        Some(6)
    );
    assert_eq!(
        value
            .get("metadata")
            .and_then(|v| v.get("format"))
            .and_then(|v| v.as_str()),
        Some("pt")
    );

    let _ = fs::remove_dir_all(&directory);
}

#[test]
fn inspect_aggregates_safetensors_shards() {
    let directory = temp_path("inspect-index");
    fs::create_dir_all(&directory).unwrap();
    fs::write(
        directory.join("model-00001-of-00002.safetensors"),
        safetensors_named("a"),
    )
    .unwrap();
    fs::write(
        directory.join("model-00002-of-00002.safetensors"),
        safetensors_named("b"),
    )
    .unwrap();
    let index = directory.join("model.safetensors.index.json");
    fs::write(
        &index,
        r#"{"metadata":{"total_size":16,"architecture":"llama"},"weight_map":{"a":"model-00001-of-00002.safetensors","b":"model-00002-of-00002.safetensors"}}"#,
    )
    .unwrap();

    let run = vantyx(
        &["ai", "inspect", index.to_str().unwrap(), "--metadata"],
        None,
        &[],
    );
    assert_eq!(run.status, 0, "{}", run.stderr);
    assert!(
        run.stdout.contains("format: safetensors-index"),
        "{}",
        run.stdout
    );
    assert!(run.stdout.contains("parameters: 4"), "{}", run.stdout);
    assert!(run.stdout.contains("shards:"), "{}", run.stdout);
    assert!(
        run.stdout.contains("model-00001-of-00002.safetensors"),
        "{}",
        run.stdout
    );

    let json_run = vantyx(
        &["ai", "inspect", index.to_str().unwrap(), "--json"],
        None,
        &[],
    );
    assert_eq!(json_run.status, 0, "{}", json_run.stderr);
    let value = vantyx::ai::parse(json_run.stdout.trim()).expect("shard-index JSON");
    assert_eq!(
        value.get("format").and_then(|v| v.as_str()),
        Some("safetensors-index")
    );
    assert_eq!(value.get("tensor_count").and_then(|v| v.as_u64()), Some(2));
    assert_eq!(
        value.get("parameter_count").and_then(|v| v.as_u64()),
        Some(4)
    );
    assert_eq!(
        value
            .get("shards")
            .and_then(|v| v.as_array())
            .map(|shards| shards.len()),
        Some(2)
    );

    let missing = directory.join("missing.safetensors.index.json");
    fs::write(&missing, r#"{"weight_map":{"a":"absent.safetensors"}}"#).unwrap();
    let run = vantyx(&["ai", "inspect", missing.to_str().unwrap()], None, &[]);
    assert_eq!(run.status, 1);
    assert!(run.stderr.contains("AI_E_IO"), "{}", run.stderr);

    let _ = fs::remove_dir_all(&directory);
}

#[test]
fn inspect_rejects_bad_inputs_and_usage_errors() {
    let directory = temp_path("inspect-errors");
    fs::create_dir_all(&directory).unwrap();
    let unknown = directory.join("unknown.bin");
    fs::write(&unknown, [0_u8; 32]).unwrap();
    let run = vantyx(&["ai", "inspect", unknown.to_str().unwrap()], None, &[]);
    assert_eq!(run.status, 1, "{}", run.stderr);
    assert!(run.stderr.contains("AI_E_FORMAT"), "{}", run.stderr);
    assert!(run.stderr.contains("help:"), "{}", run.stderr);

    let missing = vantyx(&["ai", "inspect"], None, &[]);
    assert_eq!(missing.status, 64);
    assert!(missing.stderr.contains("requires a model file path"));

    let two = vantyx(
        &[
            "ai",
            "inspect",
            unknown.to_str().unwrap(),
            unknown.to_str().unwrap(),
        ],
        None,
        &[],
    );
    assert_eq!(two.status, 64);
    assert!(two.stderr.contains("exactly one file path"));

    let absent = directory.join("absent.gguf");
    let run = vantyx(&["ai", "inspect", absent.to_str().unwrap()], None, &[]);
    assert_eq!(run.status, 1);
    assert!(run.stderr.contains("AI_E_IO"), "{}", run.stderr);

    let _ = fs::remove_dir_all(&directory);
}

#[test]
fn dataset_validates_jsonl_and_summarizes_fields() {
    let directory = temp_path("dataset-inspect");
    fs::create_dir_all(&directory).unwrap();
    let good = directory.join("train.jsonl");
    fs::write(
        &good,
        "\u{feff}{\"text\":\"hello\",\"label\":1}\r\n\n{\"text\":\"world\",\"label\":null}\n",
    )
    .unwrap();

    let run = vantyx(&["ai", "dataset", good.to_str().unwrap()], None, &[]);
    assert_eq!(run.status, 0, "{}", run.stderr);
    assert!(
        run.stdout.contains("format: jsonl-dataset"),
        "{}",
        run.stdout
    );
    assert!(run.stdout.contains("records: 2"), "{}", run.stdout);
    assert!(
        run.stdout.contains("text: 2 record(s) (string 2)"),
        "{}",
        run.stdout
    );
    assert!(run.stdout.contains("sha256: "), "{}", run.stdout);

    let json_run = vantyx(
        &["ai", "dataset", good.to_str().unwrap(), "--json"],
        None,
        &[],
    );
    assert_eq!(json_run.status, 0, "{}", json_run.stderr);
    let value = vantyx::ai::parse(json_run.stdout.trim()).expect("dataset JSON");
    assert_eq!(
        value.get("format").and_then(|v| v.as_str()),
        Some("jsonl-dataset")
    );
    assert_eq!(value.get("record_count").and_then(|v| v.as_u64()), Some(2));
    assert_eq!(value.get("blank_lines").and_then(|v| v.as_u64()), Some(1));
    assert_eq!(
        value
            .get("fields")
            .and_then(|v| v.as_array())
            .map(|fields| fields.len()),
        Some(2)
    );

    let bad = directory.join("bad.jsonl");
    fs::write(&bad, "{\"a\":1}\nnot json\n[1,2]\n").unwrap();
    let run = vantyx(&["ai", "dataset", bad.to_str().unwrap()], None, &[]);
    assert_eq!(run.status, 1);
    assert!(run.stdout.contains("invalid lines: 2"), "{}", run.stdout);
    assert!(run.stdout.contains("line 2:"), "{}", run.stdout);
    assert!(run.stderr.contains("invalid line(s)"), "{}", run.stderr);

    let _ = fs::remove_dir_all(&directory);
}

#[test]
fn dataset_rejects_usage_and_io_errors() {
    let directory = temp_path("dataset-errors");
    fs::create_dir_all(&directory).unwrap();

    let missing = vantyx(&["ai", "dataset"], None, &[]);
    assert_eq!(missing.status, 64);
    assert!(missing.stderr.contains("requires a JSONL file path"));

    let good = directory.join("data.jsonl");
    fs::write(&good, "{\"a\":1}\n").unwrap();
    let two = vantyx(
        &[
            "ai",
            "dataset",
            good.to_str().unwrap(),
            good.to_str().unwrap(),
        ],
        None,
        &[],
    );
    assert_eq!(two.status, 64);
    assert!(two.stderr.contains("exactly one file path"));

    let unknown = vantyx(
        &["ai", "dataset", good.to_str().unwrap(), "--wat"],
        None,
        &[],
    );
    assert_eq!(unknown.status, 64);
    assert!(unknown.stderr.contains("unknown option"));

    let absent = directory.join("absent.jsonl");
    let run = vantyx(&["ai", "dataset", absent.to_str().unwrap()], None, &[]);
    assert_eq!(run.status, 1);
    assert!(run.stderr.contains("AI_E_IO"), "{}", run.stderr);

    let run = vantyx(&["ai", "dataset", directory.to_str().unwrap()], None, &[]);
    assert_eq!(run.status, 1);
    assert!(run.stderr.contains("not a regular file"), "{}", run.stderr);

    let _ = fs::remove_dir_all(&directory);
}

#[test]
fn dataset_validates_records_against_a_schema() {
    let directory = temp_path("dataset-schema");
    fs::create_dir_all(&directory).unwrap();
    let schema = directory.join("record.schema.json");
    fs::write(
        &schema,
        r#"{"type":"object","required":["text","label"],"properties":{"text":{"type":"string"},"label":{"type":"string"}},"additionalProperties":false}"#,
    )
    .unwrap();
    let good = directory.join("good.jsonl");
    fs::write(&good, "{\"text\":\"a\",\"label\":\"x\"}\n").unwrap();

    let run = vantyx(
        &[
            "ai",
            "dataset",
            good.to_str().unwrap(),
            "--schema",
            schema.to_str().unwrap(),
        ],
        None,
        &[],
    );
    assert_eq!(run.status, 0, "{}", run.stderr);
    assert!(run.stdout.contains("schema: "), "{}", run.stdout);
    assert!(
        run.stdout.contains("schema-invalid records: 0"),
        "{}",
        run.stdout
    );

    let json_run = vantyx(
        &[
            "ai",
            "dataset",
            good.to_str().unwrap(),
            "--schema",
            schema.to_str().unwrap(),
            "--json",
        ],
        None,
        &[],
    );
    assert_eq!(json_run.status, 0, "{}", json_run.stderr);
    let value = vantyx::ai::parse(json_run.stdout.trim()).expect("dataset JSON");
    let schema_json = value.get("schema").expect("schema object");
    assert_eq!(
        schema_json
            .get("invalid_record_count")
            .and_then(|v| v.as_u64()),
        Some(0)
    );
    assert_eq!(
        schema_json
            .get("violations")
            .and_then(|v| v.as_array())
            .map(|violations| violations.len()),
        Some(0)
    );

    let bad = directory.join("bad-schema.jsonl");
    fs::write(
        &bad,
        "{\"text\":\"a\"}\n{\"text\":\"b\",\"label\":\"y\",\"extra\":1}\n",
    )
    .unwrap();
    let run = vantyx(
        &[
            "ai",
            "dataset",
            bad.to_str().unwrap(),
            "--schema",
            schema.to_str().unwrap(),
        ],
        None,
        &[],
    );
    assert_eq!(run.status, 1);
    assert!(
        run.stdout.contains("schema-invalid records: 2"),
        "{}",
        run.stdout
    );
    assert!(run.stdout.contains("line 1 at $.label"), "{}", run.stdout);
    assert!(
        run.stderr.contains("schema-invalid record(s)"),
        "{}",
        run.stderr
    );

    let missing_value = vantyx(
        &["ai", "dataset", good.to_str().unwrap(), "--schema"],
        None,
        &[],
    );
    assert_eq!(missing_value.status, 64);
    assert!(
        missing_value.stderr.contains("requires a JSON Schema path"),
        "{}",
        missing_value.stderr
    );

    let duplicate = vantyx(
        &[
            "ai",
            "dataset",
            good.to_str().unwrap(),
            "--schema",
            schema.to_str().unwrap(),
            "--schema",
            schema.to_str().unwrap(),
        ],
        None,
        &[],
    );
    assert_eq!(duplicate.status, 64);
    assert!(
        duplicate.stderr.contains("only once"),
        "{}",
        duplicate.stderr
    );

    let missing_schema = vantyx(
        &[
            "ai",
            "dataset",
            good.to_str().unwrap(),
            "--schema",
            directory.join("absent.json").to_str().unwrap(),
        ],
        None,
        &[],
    );
    assert_eq!(missing_schema.status, 1);
    assert!(
        missing_schema.stderr.contains("AI_E_IO"),
        "{}",
        missing_schema.stderr
    );

    let malformed = directory.join("malformed.schema.json");
    fs::write(&malformed, "{\"required\":\"text\"}").unwrap();
    let run = vantyx(
        &[
            "ai",
            "dataset",
            good.to_str().unwrap(),
            "--schema",
            malformed.to_str().unwrap(),
        ],
        None,
        &[],
    );
    assert_eq!(run.status, 1);
    assert!(run.stderr.contains("invalid JSON Schema"), "{}", run.stderr);

    let _ = fs::remove_dir_all(&directory);
}

#[test]
fn dataset_split_is_deterministic_and_reports_provenance() {
    let directory = temp_path("dataset-split");
    fs::create_dir_all(&directory).unwrap();
    let input = directory.join("corpus.jsonl");
    let mut text = String::from("\u{feff}");
    let mut records = Vec::new();
    for index in 0..30 {
        let line = format!("{{\"id\":{index},\"text\":\"row {index}\"}}");
        records.push(line.clone());
        text.push_str(&line);
        text.push('\n');
        if index == 4 {
            text.push('\n');
        }
    }
    fs::write(&input, &text).unwrap();

    let out_one = directory.join("one");
    let first = vantyx(
        &[
            "ai",
            "split",
            input.to_str().unwrap(),
            "--out-dir",
            out_one.to_str().unwrap(),
            "--train-ratio",
            "0.75",
            "--seed",
            "9",
            "--json",
        ],
        None,
        &[],
    );
    assert_eq!(first.status, 0, "{}", first.stderr);
    let value = vantyx::ai::parse(first.stdout.trim()).expect("split JSON");
    assert_eq!(
        value.get("format").and_then(|v| v.as_str()),
        Some("jsonl-split")
    );
    assert_eq!(value.get("records").and_then(|v| v.as_u64()), Some(30));
    assert_eq!(value.get("seed").and_then(|v| v.as_u64()), Some(9));
    assert!(value.get("stratify").expect("stratify key").is_null());
    assert_eq!(
        value
            .get("strata")
            .and_then(|v| v.as_array())
            .map(|v| v.len()),
        Some(0)
    );
    let train = value.get("train").expect("train part");
    let held_out = value.get("held_out").expect("held-out part");
    let train_records = train.get("records").and_then(|v| v.as_u64()).unwrap();
    let held_out_records = held_out.get("records").and_then(|v| v.as_u64()).unwrap();
    assert_eq!(train_records + held_out_records, 30);
    assert!(train_records > 0);
    assert!(held_out_records > 0);

    let mut actual = Vec::new();
    for part in [train, held_out] {
        let path = part.get("path").and_then(|v| v.as_str()).unwrap();
        let body = fs::read_to_string(path).unwrap();
        assert!(!body.contains('\r'), "split output uses LF endings");
        actual.extend(body.lines().map(|line| line.to_string()));
    }
    actual.sort();
    records.sort();
    assert_eq!(actual, records);

    let out_two = directory.join("two");
    let second = vantyx(
        &[
            "ai",
            "split",
            input.to_str().unwrap(),
            "--out-dir",
            out_two.to_str().unwrap(),
            "--train-ratio",
            "0.75",
            "--seed",
            "9",
            "--json",
        ],
        None,
        &[],
    );
    assert_eq!(second.status, 0, "{}", second.stderr);
    let value_two = vantyx::ai::parse(second.stdout.trim()).expect("second split JSON");
    assert_eq!(value.get("input_sha256"), value_two.get("input_sha256"));
    for part in ["train", "held_out"] {
        assert_eq!(
            value.get(part).unwrap().get("sha256"),
            value_two.get(part).unwrap().get("sha256")
        );
        assert_eq!(
            value.get(part).unwrap().get("records"),
            value_two.get(part).unwrap().get("records")
        );
    }

    let out_three = directory.join("three");
    let third = vantyx(
        &[
            "ai",
            "split",
            input.to_str().unwrap(),
            "--out-dir",
            out_three.to_str().unwrap(),
            "--train-ratio",
            "0.75",
            "--seed",
            "10",
            "--json",
        ],
        None,
        &[],
    );
    assert_eq!(third.status, 0, "{}", third.stderr);
    let value_three = vantyx::ai::parse(third.stdout.trim()).expect("third split JSON");
    assert_ne!(
        value.get("held_out").unwrap().get("sha256"),
        value_three.get("held_out").unwrap().get("sha256")
    );

    let _ = fs::remove_dir_all(&directory);
}

#[test]
fn dataset_split_rejects_usage_invalid_data_and_overwrite() {
    let directory = temp_path("dataset-split-errors");
    fs::create_dir_all(&directory).unwrap();
    let input = directory.join("data.jsonl");
    fs::write(&input, "{\"a\":1}\n{\"a\":2}\n").unwrap();
    let out = directory.join("out");

    let missing_out = vantyx(&["ai", "split", input.to_str().unwrap()], None, &[]);
    assert_eq!(missing_out.status, 64);
    assert!(
        missing_out.stderr.contains("--out-dir"),
        "{}",
        missing_out.stderr
    );

    let bad_ratio = vantyx(
        &[
            "ai",
            "split",
            input.to_str().unwrap(),
            "--out-dir",
            out.to_str().unwrap(),
            "--train-ratio",
            "1.0",
        ],
        None,
        &[],
    );
    assert_eq!(bad_ratio.status, 64);
    assert!(
        bad_ratio.stderr.contains("greater than 0"),
        "{}",
        bad_ratio.stderr
    );

    let bad_seed = vantyx(
        &[
            "ai",
            "split",
            input.to_str().unwrap(),
            "--out-dir",
            out.to_str().unwrap(),
            "--seed",
            "-1",
        ],
        None,
        &[],
    );
    assert_eq!(bad_seed.status, 64);
    assert!(
        bad_seed.stderr.contains("unsigned integer"),
        "{}",
        bad_seed.stderr
    );

    let unknown = vantyx(
        &[
            "ai",
            "split",
            input.to_str().unwrap(),
            "--out-dir",
            out.to_str().unwrap(),
            "--wat",
        ],
        None,
        &[],
    );
    assert_eq!(unknown.status, 64);
    assert!(
        unknown.stderr.contains("unknown option"),
        "{}",
        unknown.stderr
    );

    let invalid = directory.join("invalid.jsonl");
    fs::write(&invalid, "{\"a\":1}\nnot json\n").unwrap();
    let bad_out = directory.join("bad-out");
    let run = vantyx(
        &[
            "ai",
            "split",
            invalid.to_str().unwrap(),
            "--out-dir",
            bad_out.to_str().unwrap(),
        ],
        None,
        &[],
    );
    assert_eq!(run.status, 1);
    assert!(
        run.stderr.contains("fix them before splitting"),
        "{}",
        run.stderr
    );
    assert!(!bad_out.exists() || fs::read_dir(&bad_out).unwrap().count() == 0);

    let schema = directory.join("schema.json");
    fs::write(&schema, r#"{"type":"object","required":["text"]}"#).unwrap();
    let schema_out = directory.join("schema-out");
    let run = vantyx(
        &[
            "ai",
            "split",
            input.to_str().unwrap(),
            "--out-dir",
            schema_out.to_str().unwrap(),
            "--schema",
            schema.to_str().unwrap(),
        ],
        None,
        &[],
    );
    assert_eq!(run.status, 1);
    assert!(
        run.stderr.contains("schema-invalid record(s)"),
        "{}",
        run.stderr
    );
    assert!(!schema_out.exists() || fs::read_dir(&schema_out).unwrap().count() == 0);

    let first = vantyx(
        &[
            "ai",
            "split",
            input.to_str().unwrap(),
            "--out-dir",
            out.to_str().unwrap(),
            "--seed",
            "3",
        ],
        None,
        &[],
    );
    assert_eq!(first.status, 0, "{}", first.stderr);
    let before = fs::read_to_string(out.join("data.train.jsonl")).unwrap();
    let second = vantyx(
        &[
            "ai",
            "split",
            input.to_str().unwrap(),
            "--out-dir",
            out.to_str().unwrap(),
            "--seed",
            "3",
        ],
        None,
        &[],
    );
    assert_eq!(second.status, 1);
    assert!(
        second.stderr.contains("refusing to overwrite"),
        "{}",
        second.stderr
    );
    assert_eq!(
        fs::read_to_string(out.join("data.train.jsonl")).unwrap(),
        before
    );

    let _ = fs::remove_dir_all(&directory);
}

#[test]
fn dataset_split_stratify_reports_strata_and_is_deterministic() {
    let directory = temp_path("dataset-stratify");
    fs::create_dir_all(&directory).unwrap();
    let input = directory.join("corpus.jsonl");
    let mut text = String::new();
    for index in 0..12 {
        let label = if index < 6 { "\"a\"" } else { "\"b\"" };
        text.push_str(&format!("{{\"id\":{index},\"label\":{label}}}\n"));
    }
    fs::write(&input, &text).unwrap();

    let out_one = directory.join("one");
    let first = vantyx(
        &[
            "ai",
            "split",
            input.to_str().unwrap(),
            "--out-dir",
            out_one.to_str().unwrap(),
            "--train-ratio",
            "0.5",
            "--seed",
            "7",
            "--stratify",
            "label",
            "--json",
        ],
        None,
        &[],
    );
    assert_eq!(first.status, 0, "{}", first.stderr);
    let value = vantyx::ai::parse(first.stdout.trim()).expect("split JSON");
    assert_eq!(
        value.get("format").and_then(|v| v.as_str()),
        Some("jsonl-split")
    );
    assert_eq!(
        value.get("stratify").and_then(|v| v.as_str()),
        Some("label")
    );
    let strata = value
        .get("strata")
        .and_then(|v| v.as_array())
        .expect("strata array");
    assert_eq!(strata.len(), 2);
    assert_eq!(
        strata[0].get("label").and_then(|v| v.as_str()),
        Some("\"a\"")
    );
    assert_eq!(strata[0].get("records").and_then(|v| v.as_u64()), Some(6));
    assert_eq!(strata[0].get("train").and_then(|v| v.as_u64()), Some(3));
    assert_eq!(strata[0].get("held_out").and_then(|v| v.as_u64()), Some(3));
    assert_eq!(
        strata[1].get("label").and_then(|v| v.as_str()),
        Some("\"b\"")
    );
    assert_eq!(strata[1].get("records").and_then(|v| v.as_u64()), Some(6));
    let train_records = value
        .get("train")
        .and_then(|part| part.get("records"))
        .and_then(|v| v.as_u64())
        .unwrap();
    let held_out_records = value
        .get("held_out")
        .and_then(|part| part.get("records"))
        .and_then(|v| v.as_u64())
        .unwrap();
    assert_eq!(train_records, 6);
    assert_eq!(held_out_records, 6);

    let out_two = directory.join("two");
    let second = vantyx(
        &[
            "ai",
            "split",
            input.to_str().unwrap(),
            "--out-dir",
            out_two.to_str().unwrap(),
            "--train-ratio",
            "0.5",
            "--seed",
            "7",
            "--stratify",
            "label",
            "--json",
        ],
        None,
        &[],
    );
    assert_eq!(second.status, 0, "{}", second.stderr);
    let value_two = vantyx::ai::parse(second.stdout.trim()).expect("second split JSON");
    for part in ["train", "held_out"] {
        assert_eq!(
            value.get(part).unwrap().get("sha256"),
            value_two.get(part).unwrap().get("sha256")
        );
    }

    let out_three = directory.join("three");
    let third = vantyx(
        &[
            "ai",
            "split",
            input.to_str().unwrap(),
            "--out-dir",
            out_three.to_str().unwrap(),
            "--train-ratio",
            "0.5",
            "--seed",
            "8",
            "--stratify",
            "label",
            "--json",
        ],
        None,
        &[],
    );
    assert_eq!(third.status, 0, "{}", third.stderr);
    let value_three = vantyx::ai::parse(third.stdout.trim()).expect("third split JSON");
    assert_ne!(
        value.get("train").unwrap().get("sha256"),
        value_three.get("train").unwrap().get("sha256")
    );

    let out_text = directory.join("text");
    let text_run = vantyx(
        &[
            "ai",
            "split",
            input.to_str().unwrap(),
            "--out-dir",
            out_text.to_str().unwrap(),
            "--train-ratio",
            "0.5",
            "--seed",
            "7",
            "--stratify",
            "label",
        ],
        None,
        &[],
    );
    assert_eq!(text_run.status, 0, "{}", text_run.stderr);
    assert!(text_run.stdout.contains("stratify: label"));
    assert!(text_run.stdout.contains("strata:"));
    assert!(
        text_run
            .stdout
            .contains("\"a\": 6 records (3 train, 3 held-out)"),
        "{}",
        text_run.stdout
    );

    let schema = directory.join("schema.json");
    fs::write(&schema, r#"{"type":"object","required":["id","label"]}"#).unwrap();
    let out_schema = directory.join("schema");
    let with_schema = vantyx(
        &[
            "ai",
            "split",
            input.to_str().unwrap(),
            "--out-dir",
            out_schema.to_str().unwrap(),
            "--train-ratio",
            "0.5",
            "--seed",
            "7",
            "--stratify",
            "label",
            "--schema",
            schema.to_str().unwrap(),
            "--json",
        ],
        None,
        &[],
    );
    assert_eq!(with_schema.status, 0, "{}", with_schema.stderr);
    let schema_value = vantyx::ai::parse(with_schema.stdout.trim()).expect("schema split JSON");
    assert_eq!(
        schema_value.get("stratify").and_then(|v| v.as_str()),
        Some("label")
    );
    assert_eq!(
        schema_value
            .get("strata")
            .and_then(|v| v.as_array())
            .map(|v| v.len()),
        Some(2)
    );

    let _ = fs::remove_dir_all(&directory);
}

#[test]
fn dataset_split_stratify_rejects_usage_and_bad_labels() {
    let directory = temp_path("dataset-stratify-errors");
    fs::create_dir_all(&directory).unwrap();
    let input = directory.join("data.jsonl");
    fs::write(&input, "{\"label\":\"a\"}\n{\"label\":\"b\"}\n").unwrap();
    let out = directory.join("out");

    let missing = vantyx(
        &[
            "ai",
            "split",
            input.to_str().unwrap(),
            "--out-dir",
            out.to_str().unwrap(),
            "--stratify",
        ],
        None,
        &[],
    );
    assert_eq!(missing.status, 64);
    assert!(missing.stderr.contains("--stratify"), "{}", missing.stderr);

    let empty = vantyx(
        &[
            "ai",
            "split",
            input.to_str().unwrap(),
            "--out-dir",
            out.to_str().unwrap(),
            "--stratify",
            "",
        ],
        None,
        &[],
    );
    assert_eq!(empty.status, 64);
    assert!(empty.stderr.contains("non-empty"), "{}", empty.stderr);

    let duplicate = vantyx(
        &[
            "ai",
            "split",
            input.to_str().unwrap(),
            "--out-dir",
            out.to_str().unwrap(),
            "--stratify",
            "label",
            "--stratify",
            "other",
        ],
        None,
        &[],
    );
    assert_eq!(duplicate.status, 64);
    assert!(
        duplicate.stderr.contains("only once"),
        "{}",
        duplicate.stderr
    );

    let invalid = directory.join("invalid.jsonl");
    fs::write(&invalid, "{\"other\":1}\n{\"other\":2}\n").unwrap();
    let bad = vantyx(
        &[
            "ai",
            "split",
            invalid.to_str().unwrap(),
            "--out-dir",
            out.to_str().unwrap(),
            "--stratify",
            "label",
        ],
        None,
        &[],
    );
    assert_eq!(bad.status, 1);
    assert!(
        bad.stderr.contains("missing stratification field"),
        "{}",
        bad.stderr
    );
    assert!(
        bad.stderr.contains("dataset commands accept JSONL"),
        "{}",
        bad.stderr
    );
    assert!(!out.exists() || fs::read_dir(&out).unwrap().count() == 0);

    let _ = fs::remove_dir_all(&directory);
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
fn extract_rejects_malformed_schemas_before_calling_the_provider() {
    let run = vantyx(
        &[
            "ai",
            "extract",
            "--provider",
            "mock",
            "--model",
            "demo",
            "--schema",
            r#"{"type":"object","properties":{"age":{"enum":[]}}}"#,
            "--response",
            r#"{"age":1}"#,
            "extract a person",
        ],
        None,
        &[],
    );
    assert_eq!(run.status, 1, "{}", run.stderr);
    assert!(
        run.stderr.contains("AI_E_INVALID_REQUEST"),
        "{}",
        run.stderr
    );
    assert!(
        run.stderr.contains("extraction schema is invalid"),
        "{}",
        run.stderr
    );
    assert!(
        !run.stderr.contains("after 2 attempts"),
        "the provider must not be called: {}",
        run.stderr
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

#[test]
fn dataset_dedup_keeps_first_occurrence_and_reports_provenance() {
    let directory = temp_path("dataset-dedup");
    fs::create_dir_all(&directory).unwrap();
    let input = directory.join("corpus.jsonl");
    let text = "\u{feff}{\"id\":1,\"text\":\"a\"}\r\n{\"id\":1,\"text\":\"a\"}\n\n{\"id\":2,\"text\":\"b\"}\n{\"id\":1,\"text\":\"a\"}\n";
    fs::write(&input, text).unwrap();
    let out_one = directory.join("unique-one.jsonl");

    let first = vantyx(
        &[
            "ai",
            "dedup",
            input.to_str().unwrap(),
            "--out",
            out_one.to_str().unwrap(),
            "--json",
        ],
        None,
        &[],
    );
    assert_eq!(first.status, 0, "{}", first.stderr);
    let value = vantyx::ai::parse(first.stdout.trim()).expect("dedup JSON");
    assert_eq!(
        value.get("format").and_then(|v| v.as_str()),
        Some("jsonl-dedup")
    );
    assert_eq!(value.get("records").and_then(|v| v.as_u64()), Some(4));
    assert_eq!(
        value.get("unique_records").and_then(|v| v.as_u64()),
        Some(2)
    );
    assert_eq!(
        value.get("duplicates_removed").and_then(|v| v.as_u64()),
        Some(2)
    );
    assert_eq!(
        fs::read_to_string(&out_one).unwrap(),
        "{\"id\":1,\"text\":\"a\"}\n{\"id\":2,\"text\":\"b\"}\n"
    );
    assert_eq!(fs::read_to_string(&input).unwrap(), text);

    let out_two = directory.join("unique-two.jsonl");
    let second = vantyx(
        &[
            "ai",
            "dedup",
            input.to_str().unwrap(),
            "--out",
            out_two.to_str().unwrap(),
            "--json",
        ],
        None,
        &[],
    );
    assert_eq!(second.status, 0, "{}", second.stderr);
    let value_two = vantyx::ai::parse(second.stdout.trim()).expect("second dedup JSON");
    assert_eq!(value.get("input_sha256"), value_two.get("input_sha256"));
    assert_eq!(value.get("output_sha256"), value_two.get("output_sha256"));

    let overwrite = vantyx(
        &[
            "ai",
            "dedup",
            input.to_str().unwrap(),
            "--out",
            out_one.to_str().unwrap(),
        ],
        None,
        &[],
    );
    assert_eq!(overwrite.status, 1);
    assert!(
        overwrite.stderr.contains("refusing to overwrite"),
        "{}",
        overwrite.stderr
    );

    let _ = fs::remove_dir_all(&directory);
}

#[test]
fn dataset_dedup_rejects_usage_invalid_data_and_schema_errors() {
    let directory = temp_path("dataset-dedup-errors");
    fs::create_dir_all(&directory).unwrap();
    let input = directory.join("data.jsonl");
    fs::write(&input, "{\"text\":\"a\"}\n{\"text\":\"a\"}\n").unwrap();
    let out = directory.join("out.jsonl");

    let missing_path = vantyx(&["ai", "dedup"], None, &[]);
    assert_eq!(missing_path.status, 64);
    assert!(missing_path.stderr.contains("requires a JSONL file path"));

    let missing_out = vantyx(&["ai", "dedup", input.to_str().unwrap()], None, &[]);
    assert_eq!(missing_out.status, 64);
    assert!(
        missing_out.stderr.contains("--out"),
        "{}",
        missing_out.stderr
    );

    let unknown = vantyx(
        &[
            "ai",
            "dedup",
            input.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--wat",
        ],
        None,
        &[],
    );
    assert_eq!(unknown.status, 64);
    assert!(unknown.stderr.contains("--wat"), "{}", unknown.stderr);

    let duplicate = vantyx(
        &[
            "ai",
            "dedup",
            input.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
        ],
        None,
        &[],
    );
    assert_eq!(duplicate.status, 64);
    assert!(
        duplicate.stderr.contains("only once"),
        "{}",
        duplicate.stderr
    );

    let same = vantyx(
        &[
            "ai",
            "dedup",
            input.to_str().unwrap(),
            "--out",
            input.to_str().unwrap(),
        ],
        None,
        &[],
    );
    assert_eq!(same.status, 1);
    assert!(
        same.stderr.contains("must differ from the input"),
        "{}",
        same.stderr
    );

    fs::write(&input, "{\"text\":\"a\"}\nnot json\n").unwrap();
    let invalid = vantyx(
        &[
            "ai",
            "dedup",
            input.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
        ],
        None,
        &[],
    );
    assert_eq!(invalid.status, 1);
    assert!(
        invalid.stderr.contains("invalid line"),
        "{}",
        invalid.stderr
    );
    assert!(!out.exists());

    fs::write(&input, "{\"text\":\"a\"}\n{\"other\":1}\n").unwrap();
    let schema = directory.join("schema.json");
    fs::write(&schema, r#"{"type":"object","required":["text"]}"#).unwrap();
    let schema_invalid = vantyx(
        &[
            "ai",
            "dedup",
            input.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--schema",
            schema.to_str().unwrap(),
        ],
        None,
        &[],
    );
    assert_eq!(schema_invalid.status, 1);
    assert!(
        schema_invalid.stderr.contains("schema"),
        "{}",
        schema_invalid.stderr
    );
    assert!(!out.exists());

    let _ = fs::remove_dir_all(&directory);
}

#[test]
fn dataset_shuffle_is_deterministic_and_reports_provenance() {
    let directory = temp_path("dataset-shuffle");
    fs::create_dir_all(&directory).unwrap();
    let input = directory.join("corpus.jsonl");
    let mut text = String::new();
    let mut expected = Vec::new();
    for index in 0..20 {
        let line = format!("{{\"id\":{index},\"text\":\"row {index}\"}}");
        expected.push(line.clone());
        text.push_str(&line);
        text.push('\n');
    }
    fs::write(&input, &text).unwrap();
    let out_one = directory.join("shuffled-one.jsonl");
    let out_two = directory.join("shuffled-two.jsonl");

    let first = vantyx(
        &[
            "ai",
            "shuffle",
            input.to_str().unwrap(),
            "--out",
            out_one.to_str().unwrap(),
            "--seed",
            "7",
            "--json",
        ],
        None,
        &[],
    );
    assert_eq!(first.status, 0, "{}", first.stderr);
    let value = vantyx::ai::parse(first.stdout.trim()).expect("shuffle JSON");
    assert_eq!(
        value.get("format").and_then(|v| v.as_str()),
        Some("jsonl-shuffle")
    );
    assert_eq!(value.get("records").and_then(|v| v.as_u64()), Some(20));
    assert_eq!(value.get("seed").and_then(|v| v.as_u64()), Some(7));

    let second = vantyx(
        &[
            "ai",
            "shuffle",
            input.to_str().unwrap(),
            "--out",
            out_two.to_str().unwrap(),
            "--seed",
            "7",
            "--json",
        ],
        None,
        &[],
    );
    assert_eq!(second.status, 0, "{}", second.stderr);
    let value_two = vantyx::ai::parse(second.stdout.trim()).expect("second shuffle JSON");
    assert_eq!(value.get("input_sha256"), value_two.get("input_sha256"));
    assert_eq!(value.get("output_sha256"), value_two.get("output_sha256"));
    assert_eq!(
        fs::read(&out_one).unwrap(),
        fs::read(&out_two).unwrap(),
        "the same seed must produce the same output"
    );

    let mut actual: Vec<String> = fs::read_to_string(&out_one)
        .unwrap()
        .lines()
        .map(str::to_string)
        .collect();
    actual.sort();
    expected.sort();
    assert_eq!(actual, expected);
    assert_eq!(fs::read_to_string(&input).unwrap(), text);

    let out_three = directory.join("shuffled-three.jsonl");
    let third = vantyx(
        &[
            "ai",
            "shuffle",
            input.to_str().unwrap(),
            "--out",
            out_three.to_str().unwrap(),
            "--seed",
            "8",
            "--json",
        ],
        None,
        &[],
    );
    assert_eq!(third.status, 0, "{}", third.stderr);
    let value_three = vantyx::ai::parse(third.stdout.trim()).expect("third shuffle JSON");
    assert_ne!(value.get("output_sha256"), value_three.get("output_sha256"));

    let overwrite = vantyx(
        &[
            "ai",
            "shuffle",
            input.to_str().unwrap(),
            "--out",
            out_one.to_str().unwrap(),
        ],
        None,
        &[],
    );
    assert_eq!(overwrite.status, 1);
    assert!(
        overwrite.stderr.contains("refusing to overwrite"),
        "{}",
        overwrite.stderr
    );

    let _ = fs::remove_dir_all(&directory);
}

#[test]
fn dataset_shuffle_rejects_usage_invalid_data_and_schema_errors() {
    let directory = temp_path("dataset-shuffle-errors");
    fs::create_dir_all(&directory).unwrap();
    let input = directory.join("data.jsonl");
    fs::write(&input, "{\"text\":\"a\"}\n{\"text\":\"b\"}\n").unwrap();
    let out = directory.join("out.jsonl");

    let missing_path = vantyx(&["ai", "shuffle"], None, &[]);
    assert_eq!(missing_path.status, 64);
    assert!(missing_path.stderr.contains("requires a JSONL file path"));

    let missing_out = vantyx(&["ai", "shuffle", input.to_str().unwrap()], None, &[]);
    assert_eq!(missing_out.status, 64);
    assert!(
        missing_out.stderr.contains("--out"),
        "{}",
        missing_out.stderr
    );

    let bad_seed = vantyx(
        &[
            "ai",
            "shuffle",
            input.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--seed",
            "not-a-number",
        ],
        None,
        &[],
    );
    assert_eq!(bad_seed.status, 64);
    assert!(
        bad_seed.stderr.contains("unsigned integer"),
        "{}",
        bad_seed.stderr
    );

    let duplicate_seed = vantyx(
        &[
            "ai",
            "shuffle",
            input.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--seed",
            "1",
            "--seed",
            "2",
        ],
        None,
        &[],
    );
    assert_eq!(duplicate_seed.status, 64);
    assert!(
        duplicate_seed.stderr.contains("only once"),
        "{}",
        duplicate_seed.stderr
    );

    let unknown = vantyx(
        &[
            "ai",
            "shuffle",
            input.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--wat",
        ],
        None,
        &[],
    );
    assert_eq!(unknown.status, 64);
    assert!(unknown.stderr.contains("--wat"), "{}", unknown.stderr);

    let same = vantyx(
        &[
            "ai",
            "shuffle",
            input.to_str().unwrap(),
            "--out",
            input.to_str().unwrap(),
        ],
        None,
        &[],
    );
    assert_eq!(same.status, 1);
    assert!(
        same.stderr.contains("must differ from the input"),
        "{}",
        same.stderr
    );

    fs::write(&input, "{\"text\":\"a\"}\nnot json\n").unwrap();
    let invalid = vantyx(
        &[
            "ai",
            "shuffle",
            input.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
        ],
        None,
        &[],
    );
    assert_eq!(invalid.status, 1);
    assert!(
        invalid.stderr.contains("invalid line"),
        "{}",
        invalid.stderr
    );
    assert!(!out.exists());

    fs::write(&input, "{\"text\":\"a\"}\n{\"other\":1}\n").unwrap();
    let schema = directory.join("schema.json");
    fs::write(&schema, r#"{"type":"object","required":["text"]}"#).unwrap();
    let schema_invalid = vantyx(
        &[
            "ai",
            "shuffle",
            input.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--schema",
            schema.to_str().unwrap(),
        ],
        None,
        &[],
    );
    assert_eq!(schema_invalid.status, 1);
    assert!(
        schema_invalid.stderr.contains("schema"),
        "{}",
        schema_invalid.stderr
    );
    assert!(!out.exists());

    let _ = fs::remove_dir_all(&directory);
}
